use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateGraphsSurface {
    pub edges: Vec<StateGraphEntry>,
    pub missing_edge_count: usize,
    pub missing_node_count: usize,
    pub nodes: Vec<StateGraphEntry>,
    pub total_edge_count: usize,
    pub total_node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateGraphEntry {
    pub detail: String,
    pub kind: String,
    pub primary: String,
    pub secondary: String,
    pub status: String,
}

impl StateGraphsSurface {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let path = root
            .as_ref()
            .join("docs/state_graphs/mole_current_graph.json");
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let graph: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse state graph: {error}"))?;

        let nodes = graph
            .get("nodes")
            .and_then(serde_json::Value::as_array)
            .map(|nodes| {
                nodes
                    .iter()
                    .filter(|node| {
                        node.get("status").and_then(serde_json::Value::as_str) == Some("missing")
                    })
                    .map(state_graph_node_entry)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let edges = graph
            .get("edges")
            .and_then(serde_json::Value::as_array)
            .map(|edges| {
                edges
                    .iter()
                    .filter(|edge| {
                        edge.get("status").and_then(serde_json::Value::as_str) == Some("missing")
                    })
                    .map(state_graph_edge_entry)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(Self {
            total_node_count: graph
                .get("nodes")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len),
            total_edge_count: graph
                .get("edges")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len),
            missing_node_count: nodes.len(),
            missing_edge_count: edges.len(),
            nodes,
            edges,
        })
    }

    pub fn summary(&self) -> String {
        format!(
            "Nodes: {} total | {} missing | Edges: {} total | {} missing",
            self.total_node_count,
            self.missing_node_count,
            self.total_edge_count,
            self.missing_edge_count
        )
    }

    pub fn rows(&self) -> Vec<StateGraphEntry> {
        let mut rows = Vec::with_capacity(self.nodes.len() + self.edges.len());
        rows.extend(self.nodes.iter().cloned());
        rows.extend(self.edges.iter().cloned());
        rows
    }
}

impl From<&StateGraphsSurface> for LedgerTabTemplate {
    fn from(surface: &StateGraphsSurface) -> Self {
        Self {
            title: "State Graphs".to_string(),
            summary: surface.summary(),
            headers: vec![
                "Kind".to_string(),
                "Primary".to_string(),
                "Secondary".to_string(),
                "Status".to_string(),
            ],
            rows: surface
                .rows()
                .iter()
                .map(LedgerTabTemplateRow::from)
                .collect(),
        }
    }
}

impl From<&StateGraphEntry> for LedgerTabTemplateRow {
    fn from(row: &StateGraphEntry) -> Self {
        Self {
            cells: vec![
                row.kind.clone(),
                row.primary.clone(),
                row.secondary.clone(),
                row.status.clone(),
            ],
            detail: row.detail.clone(),
            status: Some(row.status.clone()),
        }
    }
}

fn state_graph_node_entry(node: &serde_json::Value) -> StateGraphEntry {
    let id = node
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .to_string();
    let label = node
        .get("label")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .to_string();
    StateGraphEntry {
        kind: "node".to_string(),
        primary: id.clone(),
        secondary: label.clone(),
        status: node
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        detail: format!(
            "id: {}\nlabel: {}\nstatus: {}\nnotes: {}\nsource_refs: {}\nrust_refs: {}\nvalue_refs: {}\nknown_gaps: {}",
            id,
            label,
            node.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown"),
            compact_json(node.get("notes")),
            compact_json(node.get("source_refs")),
            compact_json(node.get("rust_refs")),
            compact_json(node.get("value_refs")),
            compact_json(node.get("known_gaps")),
        ),
    }
}

fn state_graph_edge_entry(edge: &serde_json::Value) -> StateGraphEntry {
    let from = edge
        .get("from")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .to_string();
    let to = edge
        .get("to")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .to_string();
    StateGraphEntry {
        kind: "edge".to_string(),
        primary: from.clone(),
        secondary: to.clone(),
        status: edge
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        detail: format!(
            "from: {}\nto: {}\nlabel: {}\nstatus: {}\nnotes: {}\nsource_refs: {}\nrust_refs: {}\nvalue_refs: {}",
            from,
            to,
            compact_json(edge.get("label")),
            edge.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown"),
            compact_json(edge.get("notes")),
            compact_json(edge.get("source_refs")),
            compact_json(edge.get("rust_refs")),
            compact_json(edge.get("value_refs")),
        ),
    }
}

fn compact_json(value: Option<&serde_json::Value>) -> String {
    value
        .map(|value| match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(bool) => bool.to_string(),
            serde_json::Value::Number(number) => number.to_string(),
            serde_json::Value::String(string) => string.clone(),
            other => {
                serde_json::to_string(other).unwrap_or_else(|_| "<unserializable>".to_string())
            }
        })
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn state_graphs_loads_missing_entries_into_a_sheet() {
        let surface = StateGraphsSurface::load(workspace_root()).unwrap();
        let template = LedgerTabTemplate::from(&surface);

        assert_eq!(template.title, "State Graphs");
        assert_eq!(template.headers.len(), 4);
        assert_eq!(
            template.rows.len(),
            surface.nodes.len() + surface.edges.len()
        );
    }
}
