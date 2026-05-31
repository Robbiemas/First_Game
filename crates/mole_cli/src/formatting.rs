use serde_json::Value;

pub(crate) fn format_text_report(report: &Value) -> String {
    let command = report
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("mole");
    let mut lines = vec![format!("Mole CLI {command}")];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("root: {root}"));
    }
    if let Some(git) = report.get("git") {
        if let Some(branch) = git.get("branch").and_then(Value::as_str) {
            lines.push(format!("branch: {branch}"));
        }
        if let Some(total) = git.get("total_changed").and_then(Value::as_u64) {
            lines.push(format!("changed files: {total}"));
        }
    }
    if let Some(parity) = report.get("parity") {
        if let Some(actionable) = parity.get("value_actionable").and_then(Value::as_u64) {
            lines.push(format!("value actionable rows: {actionable}"));
        }
        if let Some(mapped) = parity
            .get("ecb_mapped_motion_states")
            .and_then(Value::as_u64)
        {
            lines.push(format!("ecb mapped motion states: {mapped}"));
        }
    }
    if let Some(next) = report.get("recommended_next").and_then(Value::as_array) {
        lines.push("recommended next:".to_string());
        lines.extend(
            next.iter()
                .filter_map(Value::as_str)
                .map(|item| format!("- {item}")),
        );
    }
    lines.join("\n")
}

pub(crate) fn format_markdown_report(report: &Value) -> String {
    match report.get("command").and_then(Value::as_str) {
        Some("agent brief") => format_agent_brief_markdown(report),
        Some("graph next") => format_graph_next_markdown(report),
        Some("graph inspect") => format_graph_inspect_markdown(report),
        Some("verify changed") => format_verify_changed_markdown(report),
        Some("generated check") => format_generated_check_markdown(report),
        Some("finish check") => format_finish_check_markdown(report),
        _ => format_handoff_markdown(report),
    }
}

fn format_agent_brief_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Agent Brief".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    if let Some(git) = report.get("git") {
        if let Some(branch) = git.get("branch").and_then(Value::as_str) {
            lines.push(format!("- Branch: `{branch}`"));
        }
        if let Some(remote) = git.get("remote_origin").and_then(Value::as_str) {
            lines.push(format!("- Remote: `{remote}`"));
        }
    }
    if let Some(plan) = report.get("macro_plan") {
        if let Some(path) = plan.get("path").and_then(Value::as_str) {
            lines.push(format!("- Macro plan: `{path}`"));
        }
    }
    lines.push(String::new());
    lines.push("## Compaction Anchor".to_string());
    if let Some(anchor) = report.get("compaction_anchor").and_then(Value::as_array) {
        for item in anchor.iter().filter_map(Value::as_str) {
            lines.push(format!("- {item}"));
        }
    }
    lines.push(String::new());
    lines.push("## Missing Graph".to_string());
    if let Some(missing) = report.get("missing_graph") {
        if let Some(count) = missing.get("missing_count").and_then(Value::as_u64) {
            lines.push(format!("- Missing entries: `{count}`"));
        }
        if let Some(next) = missing.get("recommended_next").and_then(Value::as_array) {
            for item in next.iter().filter_map(Value::as_str) {
                lines.push(format!("- {item}"));
            }
        }
    }
    if let Some(graph_next) = report.get("graph_next") {
        lines.push(String::new());
        lines.push("## Next Graph Targets".to_string());
        let ranked_count = graph_next
            .get("ranked_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let omitted_count = graph_next
            .get("omitted_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        lines.push(format!(
            "- Ranked entries: `{ranked_count}`; omitted from brief: `{omitted_count}`"
        ));
        if let Some(entries) = graph_next.get("ranked_entries").and_then(Value::as_array) {
            for entry in entries.iter().take(5) {
                let name = markdown_graph_entry_name(entry);
                let score = entry.get("score").and_then(Value::as_i64).unwrap_or(0);
                let status = entry
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                lines.push(format!("- `{name}`: {status}, score `{score}`"));
            }
        }
    }
    lines.push(String::new());
    lines.push("## Verification".to_string());
    if let Some(commands) = report
        .get("verification_commands")
        .and_then(Value::as_array)
    {
        for command in commands.iter().filter_map(Value::as_str) {
            lines.push(format!("- `{command}`"));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_graph_next_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Graph Next".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    let ranked_count = report
        .get("ranked_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let omitted_count = report
        .get("omitted_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    lines.push(format!("- Ranked entries: `{ranked_count}`"));
    lines.push(format!("- Omitted from output: `{omitted_count}`"));
    lines.push(String::new());
    lines.push("## Ranked Targets".to_string());
    if let Some(entries) = report.get("ranked_entries").and_then(Value::as_array) {
        for (index, entry) in entries.iter().enumerate() {
            let name = markdown_graph_entry_name(entry);
            let status = entry
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let score = entry.get("score").and_then(Value::as_i64).unwrap_or(0);
            lines.push(format!(
                "{}. `{name}` - {status}; Score: `{score}`",
                index + 1
            ));
            if let Some(reason) = entry.get("reason").and_then(Value::as_str) {
                lines.push(format!("   - {reason}"));
            }
            if let Some(score_reasons) = entry.get("score_reasons").and_then(Value::as_array) {
                let joined = score_reasons
                    .iter()
                    .filter_map(Value::as_str)
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("; ");
                if !joined.is_empty() {
                    lines.push(format!("   - Score reasons: {joined}"));
                }
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_verify_changed_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Verify Changed".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    lines.push(String::new());
    lines.push("## Changed Paths".to_string());
    if let Some(paths) = report.get("changed_paths").and_then(Value::as_array) {
        if paths.is_empty() {
            lines.push("- No changed paths reported.".to_string());
        } else {
            for path in paths.iter().filter_map(Value::as_str).take(30) {
                lines.push(format!("- `{path}`"));
            }
            if paths.len() > 30 {
                lines.push(format!("- ... `{}` more", paths.len() - 30));
            }
        }
    }
    lines.push(String::new());
    lines.push("## Commands".to_string());
    if let Some(commands) = report.get("commands").and_then(Value::as_array) {
        for command in commands.iter().filter_map(Value::as_str) {
            lines.push(format!("- `{command}`"));
            if let Some(reasons) = report
                .get("command_reasons")
                .and_then(|value| value.get(command))
                .and_then(Value::as_array)
            {
                for reason in reasons.iter().filter_map(Value::as_str) {
                    lines.push(format!("  - {reason}"));
                }
            }
        }
    }
    if let Some(errors) = report.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            lines.push(String::new());
            lines.push("## Errors".to_string());
            for error in errors.iter().filter_map(Value::as_str) {
                lines.push(format!("- {error}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_graph_inspect_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Graph Inspect".to_string(), String::new()];
    if let Some(target) = report.get("target").and_then(Value::as_str) {
        lines.push(format!("- Target: `{target}`"));
    }
    let found = report
        .get("found")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    lines.push(format!("- Found: `{found}`"));
    if let Some(entry) = report.get("entry").filter(|entry| !entry.is_null()) {
        lines.push(String::new());
        lines.push("## Entry".to_string());
        let name = markdown_graph_entry_name(entry);
        lines.push(format!("- Name: `{name}`"));
        if let Some(kind) = entry.get("kind").and_then(Value::as_str) {
            lines.push(format!("- Kind: `{kind}`"));
        }
        if let Some(status) = entry.get("status").and_then(Value::as_str) {
            lines.push(format!("- Status: `{status}`"));
        }
        if let Some(notes) = entry.get("notes").and_then(Value::as_str) {
            lines.push(format!("- Notes: {notes}"));
        }
    }
    if let Some(checklist) = report.get("audit_checklist").and_then(Value::as_array) {
        lines.push(String::new());
        lines.push("## Audit Checklist".to_string());
        for item in checklist.iter().filter_map(Value::as_str) {
            lines.push(format!("- {item}"));
        }
    }
    if let Some(suggestions) = report.get("suggestions").and_then(Value::as_array) {
        if !suggestions.is_empty() {
            lines.push(String::new());
            lines.push("## Suggestions".to_string());
            for item in suggestions.iter().filter_map(Value::as_str) {
                lines.push(format!("- `{item}`"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_finish_check_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Finish Check".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    if let Some(gate) = report.get("completion_gate") {
        let ok = gate.get("ok").and_then(Value::as_bool).unwrap_or(false);
        lines.push(format!("- Gate OK: `{ok}`"));
        lines.push(String::new());
        lines.push("## Completion Gate".to_string());
        if let Some(checks) = gate.get("checks").and_then(Value::as_array) {
            for check in checks {
                let name = check
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let ok = check.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let detail = check.get("detail").and_then(Value::as_str).unwrap_or("");
                lines.push(format!("- `{name}`: `{ok}` - {detail}"));
            }
        }
    }
    if let Some(help) = report.get("help_catalog") {
        lines.push(String::new());
        lines.push("## Help Catalog".to_string());
        let ok = help.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let count = help
            .get("documented_command_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        lines.push(format!("- OK: `{ok}`"));
        lines.push(format!("- Documented commands: `{count}`"));
    }
    if let Some(queue) = report.get("request_queue") {
        lines.push(String::new());
        lines.push("## Request Queue".to_string());
        let inbox = queue
            .get("inbox_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let completed = queue
            .get("completed_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        lines.push(format!("- Inbox: `{inbox}`"));
        lines.push(format!("- Completed: `{completed}`"));
    }
    if let Some(verification) = report.get("verification") {
        lines.push(String::new());
        lines.push("## Verification".to_string());
        if let Some(commands) = verification.get("commands").and_then(Value::as_array) {
            for command in commands.iter().filter_map(Value::as_str) {
                lines.push(format!("- `{command}`"));
                if let Some(reasons) = verification
                    .get("command_reasons")
                    .and_then(|value| value.get(command))
                    .and_then(Value::as_array)
                {
                    for reason in reasons.iter().filter_map(Value::as_str) {
                        lines.push(format!("  - {reason}"));
                    }
                }
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_generated_check_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Generated Check".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!("- OK: `{ok}`"));

    if let Some(summary) = report.get("summary") {
        lines.push(String::new());
        lines.push("## Summary".to_string());
        for field in [
            "total_groups",
            "ok_groups",
            "missing_input_groups",
            "missing_output_groups",
            "stale_groups",
            "dirty_output_groups",
        ] {
            let count = summary.get(field).and_then(Value::as_u64).unwrap_or(0);
            lines.push(format!("- `{field}`: `{count}`"));
        }
    }

    lines.push(String::new());
    lines.push("## Artifact Groups".to_string());
    if let Some(groups) = report.get("artifact_groups").and_then(Value::as_array) {
        for group in groups {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("unknown");
            let ok = group.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let stale = group.get("stale").and_then(Value::as_bool).unwrap_or(false);
            lines.push(format!("- `{id}`: ok `{ok}`, stale `{stale}`"));
            for field in [
                "missing_inputs",
                "missing_outputs",
                "dirty_outputs",
                "newer_inputs",
            ] {
                let values = string_array(group.get(field));
                if !values.is_empty() {
                    lines.push(format!("  - {field}: `{}`", values.join("`, `")));
                }
            }
            if let Some(command) = group.get("recommended_command").and_then(Value::as_str) {
                lines.push(format!("  - command: `{command}`"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn string_array(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn markdown_graph_entry_name(entry: &Value) -> String {
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

fn format_handoff_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole CLI Handoff".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    if let Some(git) = report.get("git") {
        lines.push(format!(
            "- Branch: `{}`",
            git.get("branch")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        lines.push(format!(
            "- Remote: `{}`",
            git.get("remote_origin")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        lines.push(format!(
            "- Dirty files: {}",
            git.get("total_changed")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
    }
    if let Some(parity) = report.get("parity") {
        lines.push(format!(
            "- Value rows: {} total, {} actionable, {} derived",
            parity
                .get("value_total_rows")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            parity
                .get("value_actionable")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            parity
                .get("value_derived")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
        lines.push(format!(
            "- ECB mapped states: {}",
            parity
                .get("ecb_mapped_motion_states")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
        if let Some(states) = parity
            .get("ecb_unmapped_derived_states")
            .and_then(Value::as_array)
        {
            let names = states.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            lines.push(format!("- Unmapped derived states: {}", names.join(", ")));
        }
    }
    if let Some(next) = report.get("recommended_next").and_then(Value::as_array) {
        lines.extend([String::new(), "## Recommended Next".to_string()]);
        lines.extend(
            next.iter()
                .filter_map(Value::as_str)
                .map(|item| format!("- {item}")),
        );
    }
    if let Some(commands) = report
        .get("verification_commands")
        .and_then(Value::as_array)
    {
        lines.extend([String::new(), "## Verification".to_string()]);
        lines.extend(
            commands
                .iter()
                .filter_map(Value::as_str)
                .map(|command| format!("- `{command}`")),
        );
    }
    lines.push(String::new());
    lines.join("\n")
}
