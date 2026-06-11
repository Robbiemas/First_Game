use serde_json::{json, Value};
use std::path::Path;

use crate::{read_json, GraphCommand, SCHEMA_VERSION};
use mole_devtool::{StateGraphCanvasPair, StateGraphDocument};

pub(crate) fn graph_report(root: &Path, command: &GraphCommand) -> Value {
    match command {
        GraphCommand::Missing => graph_missing_report(root),
        GraphCommand::Next => graph_next_report(root),
        GraphCommand::Inspect { target } => graph_inspect_report(root, target),
        GraphCommand::Layout => graph_layout_report(root),
    }
}

pub(crate) fn graph_layout_report(root: &Path) -> Value {
    let layout_path = "config/state_graph_layout.json";
    match StateGraphCanvasPair::load(root) {
        Ok(pair) => {
            let graph_reports = pair
                .graphs
                .iter()
                .map(graph_layout_graph_report)
                .collect::<Vec<_>>();
            let errors = pair.validation_errors();
            json!({
                "schema_version": SCHEMA_VERSION,
                "command": "graph layout",
                "project_root": root.display().to_string(),
                "layout_path": layout_path,
                "graph_paths": [
                    "docs/state_graphs/melee_reference_graph.json",
                    "docs/state_graphs/mole_current_graph.json"
                ],
                "mutated": false,
                "ok": errors.is_empty(),
                "graph_count": pair.graphs.len(),
                "graphs": graph_reports,
                "errors": errors,
            })
        }
        Err(error) => json!({
            "schema_version": SCHEMA_VERSION,
            "command": "graph layout",
            "project_root": root.display().to_string(),
            "layout_path": layout_path,
            "graph_paths": [
                "docs/state_graphs/melee_reference_graph.json",
                "docs/state_graphs/mole_current_graph.json"
            ],
            "mutated": false,
            "ok": false,
            "graph_count": 0,
            "graphs": [],
            "errors": [error],
        }),
    }
}

fn graph_layout_graph_report(graph: &StateGraphDocument) -> Value {
    json!({
        "id": graph.id,
        "title": graph.title,
        "root": graph.root,
        "zoom": graph.zoom,
        "node_count": graph.nodes.len(),
        "edge_count": graph.edges.len(),
        "nodes_with_positions": graph.nodes.iter().filter(|node| node.pos != [0.0, 0.0]).count(),
        "statuses": graph_status_counts(graph),
        "validation_errors": graph.validation_errors(),
    })
}

fn graph_status_counts(graph: &StateGraphDocument) -> Value {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for node in &graph.nodes {
        *counts.entry(node.status.clone()).or_default() += 1;
    }
    for edge in &graph.edges {
        *counts.entry(edge.status.clone()).or_default() += 1;
    }
    json!(counts)
}

pub(crate) fn graph_missing_report(root: &Path) -> Value {
    let graph_path = root.join("docs/state_graphs/mole_current_graph.json");
    let mut errors = Vec::new();
    let graph = read_json(graph_path.clone())
        .inspect_err(|error| errors.push(format!("{}: {error}", graph_path.display())))
        .ok();
    let nodes = graph
        .as_ref()
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .map(|nodes| missing_graph_nodes(nodes))
        .unwrap_or_default();
    let edges = graph
        .as_ref()
        .and_then(|graph| graph.get("edges"))
        .and_then(Value::as_array)
        .map(|edges| missing_graph_edges(edges))
        .unwrap_or_default();
    let missing_count = nodes.len() + edges.len();
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "graph missing",
        "project_root": root.display().to_string(),
        "graph_path": "docs/state_graphs/mole_current_graph.json",
        "mutated": false,
        "missing_count": missing_count,
        "nodes": nodes,
        "edges": edges,
        "recommended_next": missing_graph_recommendations(&nodes, &edges),
        "errors": errors,
    })
}

pub(crate) fn graph_next_report(root: &Path) -> Value {
    let graph_path = root.join("docs/state_graphs/mole_current_graph.json");
    let mut errors = Vec::new();
    let graph = read_json(graph_path.clone())
        .inspect_err(|error| errors.push(format!("{}: {error}", graph_path.display())))
        .ok();
    let mut entries = graph.as_ref().map(graph_ranked_entries).unwrap_or_default();
    entries.sort_by(|left, right| {
        right
            .get("score")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .cmp(&left.get("score").and_then(Value::as_i64).unwrap_or(0))
            .then_with(|| graph_entry_name(left).cmp(&graph_entry_name(right)))
    });
    let total_ranked = entries.len();
    let omitted_count = total_ranked.saturating_sub(12);
    entries.truncate(12);

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "graph next",
        "project_root": root.display().to_string(),
        "graph_path": "docs/state_graphs/mole_current_graph.json",
        "mutated": false,
        "ranked_count": total_ranked,
        "omitted_count": omitted_count,
        "ranked_entries": entries,
        "errors": errors,
    })
}

pub(crate) fn graph_inspect_report(root: &Path, target: &str) -> Value {
    let graph_path = root.join("docs/state_graphs/mole_current_graph.json");
    let mut errors = Vec::new();
    let graph = read_json(graph_path.clone())
        .inspect_err(|error| errors.push(format!("{}: {error}", graph_path.display())))
        .ok();
    let entry = graph
        .as_ref()
        .and_then(|graph| inspect_graph_entry(graph, target));
    let suggestions = graph
        .as_ref()
        .map(|graph| graph_inspect_suggestions(graph, target))
        .unwrap_or_default();

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "graph inspect",
        "project_root": root.display().to_string(),
        "graph_path": "docs/state_graphs/mole_current_graph.json",
        "mutated": false,
        "target": target,
        "found": entry.is_some(),
        "entry": entry.unwrap_or(Value::Null),
        "audit_checklist": graph_inspect_audit_checklist(),
        "suggestions": suggestions,
        "errors": errors,
    })
}

fn inspect_graph_entry(graph: &Value, target: &str) -> Option<Value> {
    if let Some((from, to)) = parse_edge_target(target) {
        return graph
            .get("edges")
            .and_then(Value::as_array)
            .and_then(|edges| {
                edges.iter().find(|edge| {
                    value_matches(edge.get("from"), &from) && value_matches(edge.get("to"), &to)
                })
            })
            .map(|edge| graph_entry_with_kind(edge, "edge"));
    }

    graph
        .get("nodes")
        .and_then(Value::as_array)
        .and_then(|nodes| {
            nodes.iter().find(|node| {
                value_matches(node.get("id"), target) || value_matches(node.get("label"), target)
            })
        })
        .map(|node| graph_entry_with_kind(node, "node"))
}

fn graph_entry_with_kind(entry: &Value, kind: &str) -> Value {
    let mut object = entry.as_object().cloned().unwrap_or_default();
    object.insert("kind".to_string(), json!(kind));
    Value::Object(object)
}

fn parse_edge_target(target: &str) -> Option<(String, String)> {
    if let Some((from, to)) = target.split_once("->") {
        return Some((from.trim().to_string(), to.trim().to_string()));
    }
    let parts = target.split_whitespace().collect::<Vec<_>>();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

fn value_matches(value: Option<&Value>, target: &str) -> bool {
    value
        .and_then(Value::as_str)
        .map(|value| value.eq_ignore_ascii_case(target.trim()))
        .unwrap_or(false)
}

fn graph_inspect_audit_checklist() -> Vec<&'static str> {
    vec![
        "Compare source refs against the local Melee decomp before changing behavior.",
        "Check Rust refs for current ownership and tests before editing.",
        "Update graph metadata, value refs, and known gaps if the implementation changes.",
        "Run `mole verify changed --json` after edits to select the verification slice.",
    ]
}

fn graph_inspect_suggestions(graph: &Value, target: &str) -> Vec<String> {
    let target = target.to_ascii_lowercase();
    let mut suggestions = Vec::new();
    if let Some(nodes) = graph.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            let id = node.get("id").and_then(Value::as_str).unwrap_or("");
            if id.to_ascii_lowercase().contains(&target)
                || target.contains(&id.to_ascii_lowercase())
            {
                suggestions.push(id.to_string());
            }
        }
    }
    if let Some(edges) = graph.get("edges").and_then(Value::as_array) {
        for edge in edges {
            let from = edge.get("from").and_then(Value::as_str).unwrap_or("");
            let to = edge.get("to").and_then(Value::as_str).unwrap_or("");
            let label = format!("{from} -> {to}");
            if label.to_ascii_lowercase().contains(&target) {
                suggestions.push(label);
            }
        }
    }
    suggestions.truncate(8);
    suggestions
}

fn missing_graph_nodes(nodes: &[Value]) -> Vec<Value> {
    nodes
        .iter()
        .filter(|node| node.get("status").and_then(Value::as_str) == Some("missing"))
        .map(|node| {
            json!({
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "label": node.get("label").cloned().unwrap_or(Value::Null),
                "status": "missing",
                "notes": node.get("notes").cloned().unwrap_or(Value::Null),
                "source_refs": node.get("source_refs").cloned().unwrap_or_else(|| json!([])),
                "rust_refs": node.get("rust_refs").cloned().unwrap_or_else(|| json!([])),
                "value_refs": node.get("value_refs").cloned().unwrap_or_else(|| json!([])),
                "known_gaps": node.get("known_gaps").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect()
}

fn missing_graph_edges(edges: &[Value]) -> Vec<Value> {
    edges
        .iter()
        .filter(|edge| edge.get("status").and_then(Value::as_str) == Some("missing"))
        .map(|edge| {
            json!({
                "from": edge.get("from").cloned().unwrap_or(Value::Null),
                "to": edge.get("to").cloned().unwrap_or(Value::Null),
                "label": edge.get("label").cloned().unwrap_or(Value::Null),
                "status": "missing",
                "input": edge.get("input").cloned().unwrap_or(Value::Null),
                "frames": edge.get("frames").cloned().unwrap_or(Value::Null),
                "notes": edge.get("notes").cloned().unwrap_or(Value::Null),
                "source_refs": edge.get("source_refs").cloned().unwrap_or_else(|| json!([])),
                "rust_refs": edge.get("rust_refs").cloned().unwrap_or_else(|| json!([])),
                "value_refs": edge.get("value_refs").cloned().unwrap_or_else(|| json!([])),
                "known_gaps": edge.get("known_gaps").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect()
}

fn graph_ranked_entries(graph: &Value) -> Vec<Value> {
    let mut entries = Vec::new();
    if let Some(nodes) = graph.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            if let Some(entry) = ranked_graph_node(node) {
                entries.push(entry);
            }
        }
    }
    if let Some(edges) = graph.get("edges").and_then(Value::as_array) {
        for edge in edges {
            if let Some(entry) = ranked_graph_edge(edge) {
                entries.push(entry);
            }
        }
    }
    entries
}

fn ranked_graph_node(node: &Value) -> Option<Value> {
    let status = node.get("status").and_then(Value::as_str)?;
    if !matches!(status, "missing" | "partial") {
        return None;
    }
    let id = node.get("id").and_then(Value::as_str).unwrap_or("unknown");
    let notes = node.get("notes").and_then(Value::as_str).unwrap_or("");
    let score = graph_priority_score(status, &[id, notes]);
    Some(json!({
        "kind": "node",
        "id": id,
        "label": node.get("label").cloned().unwrap_or(Value::Null),
        "status": status,
        "score": score,
        "score_reasons": graph_priority_score_reasons(status, &[id, notes]),
        "reason": graph_priority_reason(status, &[id, notes]),
        "recommended_next": graph_entry_recommendation(status, &format!("node `{id}`")),
        "notes": node.get("notes").cloned().unwrap_or(Value::Null),
        "source_refs": node.get("source_refs").cloned().unwrap_or_else(|| json!([])),
        "rust_refs": node.get("rust_refs").cloned().unwrap_or_else(|| json!([])),
        "value_refs": node.get("value_refs").cloned().unwrap_or_else(|| json!([])),
        "known_gaps": node.get("known_gaps").cloned().unwrap_or_else(|| json!([])),
    }))
}

fn ranked_graph_edge(edge: &Value) -> Option<Value> {
    let status = edge.get("status").and_then(Value::as_str)?;
    if !matches!(status, "missing" | "partial") {
        return None;
    }
    let from = edge
        .get("from")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let to = edge.get("to").and_then(Value::as_str).unwrap_or("unknown");
    let notes = edge.get("notes").and_then(Value::as_str).unwrap_or("");
    let label = format!("{from} -> {to}");
    let score = graph_priority_score(status, &[from, to, notes]);
    Some(json!({
        "kind": "edge",
        "from": from,
        "to": to,
        "label": edge.get("label").cloned().unwrap_or(Value::Null),
        "status": status,
        "score": score,
        "score_reasons": graph_priority_score_reasons(status, &[from, to, notes]),
        "reason": graph_priority_reason(status, &[from, to, notes]),
        "recommended_next": graph_entry_recommendation(status, &format!("edge `{label}`")),
        "input": edge.get("input").cloned().unwrap_or(Value::Null),
        "frames": edge.get("frames").cloned().unwrap_or(Value::Null),
        "notes": edge.get("notes").cloned().unwrap_or(Value::Null),
        "source_refs": edge.get("source_refs").cloned().unwrap_or_else(|| json!([])),
        "rust_refs": edge.get("rust_refs").cloned().unwrap_or_else(|| json!([])),
        "value_refs": edge.get("value_refs").cloned().unwrap_or_else(|| json!([])),
        "known_gaps": edge.get("known_gaps").cloned().unwrap_or_else(|| json!([])),
    }))
}

fn graph_priority_score(status: &str, fields: &[&str]) -> i32 {
    let mut score = if status == "missing" { 1_000 } else { 100 };
    let haystack = fields.join(" ").to_ascii_lowercase();
    for (keyword, weight) in graph_priority_keywords() {
        if haystack.contains(keyword) {
            score += weight;
        }
    }
    score
}

fn graph_priority_score_reasons(status: &str, fields: &[&str]) -> Vec<String> {
    let mut reasons = if status == "missing" {
        vec!["status `missing` starts at 1000".to_string()]
    } else {
        vec!["status `partial` starts at 100".to_string()]
    };
    let haystack = fields.join(" ").to_ascii_lowercase();
    for (keyword, weight) in graph_priority_keywords() {
        if haystack.contains(keyword) {
            reasons.push(format!("matched `{keyword}` keyword: +{weight}"));
        }
    }
    reasons
}

fn graph_priority_keywords() -> [(&'static str, i32); 18] {
    [
        ("rundirect", 220),
        ("dash", 200),
        ("moonwalk", 200),
        ("turn", 170),
        ("run", 160),
        ("walk", 150),
        ("ground", 140),
        ("jump", 130),
        ("fall", 120),
        ("escapeair", 120),
        ("landing", 110),
        ("platform", 100),
        ("ledge", 100),
        ("cliff", 100),
        ("shield", 90),
        ("guard", 90),
        ("squat", 80),
        ("pass", 80),
    ]
}

fn graph_priority_reason(status: &str, fields: &[&str]) -> String {
    if status == "missing" {
        return "Missing graph entry blocks ledger completeness.".to_string();
    }
    let haystack = fields.join(" ").to_ascii_lowercase();
    if haystack.contains("dash") || haystack.contains("moonwalk") {
        "Grounded movement feel target from the macro plan.".to_string()
    } else if haystack.contains("platform")
        || haystack.contains("ledge")
        || haystack.contains("cliff")
    {
        "Environment-contact movement target from the macro plan.".to_string()
    } else {
        "Partial graph entry with remaining source-backed parity work.".to_string()
    }
}

fn graph_entry_recommendation(status: &str, name: &str) -> String {
    if status == "missing" {
        format!("Source-audit and implement or intentionally justify missing {name}.")
    } else {
        format!("Continue source-backed parity audit for partial {name}.")
    }
}

fn graph_entry_name(entry: &Value) -> String {
    if let Some(id) = entry.get("id").and_then(Value::as_str) {
        id.to_string()
    } else {
        format!(
            "{} -> {}",
            entry
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            entry.get("to").and_then(Value::as_str).unwrap_or("unknown")
        )
    }
}

fn missing_graph_recommendations(nodes: &[Value], edges: &[Value]) -> Vec<String> {
    let mut recommendations = Vec::new();
    for node in nodes {
        let id = node.get("id").and_then(Value::as_str).unwrap_or("unknown");
        recommendations.push(format!(
            "Source-audit and implement or intentionally justify missing node `{id}`."
        ));
    }
    for edge in edges {
        let from = edge
            .get("from")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let to = edge.get("to").and_then(Value::as_str).unwrap_or("unknown");
        recommendations.push(format!(
            "Source-audit and implement or intentionally justify missing edge `{from} -> {to}`."
        ));
    }
    if recommendations.is_empty() {
        recommendations.push("No missing graph entries detected.".to_string());
    }
    recommendations
}
