use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateGraphCanvasPair {
    pub graphs: Vec<StateGraphDocument>,
    pub layout_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateGraphDocument {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_graph_zoom")]
    pub zoom: f64,
    #[serde(default)]
    pub nodes: Vec<StateGraphNode>,
    #[serde(default)]
    pub edges: Vec<StateGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateGraphNode {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub pos: [f64; 2],
    #[serde(default)]
    pub status: String,
    #[serde(flatten)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateGraphEdge {
    #[serde(rename = "from")]
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub frames: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(flatten)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StateGraphLayoutFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    graphs: BTreeMap<String, StateGraphLayoutGraph>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StateGraphLayoutGraph {
    #[serde(default = "default_graph_zoom")]
    zoom: f64,
    #[serde(default)]
    nodes: BTreeMap<String, [f64; 2]>,
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

impl StateGraphCanvasPair {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let layout_path = root.join("config/state_graph_layout.json");
        let layout = load_state_graph_layout(&layout_path)?;
        let mut graphs = Vec::new();
        for filename in ["melee_reference_graph.json", "mole_current_graph.json"] {
            let path = root.join("docs/state_graphs").join(filename);
            let mut graph = load_state_graph_document(&path)?;
            apply_state_graph_layout(&mut graph, &layout);
            graphs.push(graph);
        }
        Ok(Self {
            graphs,
            layout_path: "config/state_graph_layout.json".to_string(),
        })
    }

    pub fn graph(&self, id: &str) -> Option<&StateGraphDocument> {
        self.graphs.iter().find(|graph| graph.id == id)
    }

    pub fn validation_errors(&self) -> Vec<String> {
        self.graphs
            .iter()
            .flat_map(StateGraphDocument::validation_errors)
            .collect()
    }
}

impl StateGraphDocument {
    pub fn node_position(&self, id: &str) -> Option<[f64; 2]> {
        self.nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.pos)
    }

    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.is_empty() {
            errors.push("graph missing id".to_string());
        }
        if self.title.is_empty() {
            errors.push(format!("graph {} missing title", self.id));
        }
        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            if node.id.is_empty() {
                errors.push(format!("graph {} has node with empty id", self.id));
            } else if !node_ids.insert(node.id.clone()) {
                errors.push(format!("graph {} has duplicate node {}", self.id, node.id));
            }
            if node.status.is_empty() {
                errors.push(format!("graph {} node {} missing status", self.id, node.id));
            }
        }
        if !self.root.is_empty() && !node_ids.contains(&self.root) {
            errors.push(format!(
                "graph {} root {} is not present in nodes",
                self.id, self.root
            ));
        }
        for (index, edge) in self.edges.iter().enumerate() {
            if !node_ids.contains(&edge.from) {
                errors.push(format!(
                    "graph {} edge {} source {} is not a node",
                    self.id, index, edge.from
                ));
            }
            if !node_ids.contains(&edge.to) {
                errors.push(format!(
                    "graph {} edge {} target {} is not a node",
                    self.id, index, edge.to
                ));
            }
            if edge.status.is_empty() {
                errors.push(format!("graph {} edge {} missing status", self.id, index));
            }
        }
        errors
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

fn load_state_graph_document(path: &Path) -> Result<StateGraphDocument, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn load_state_graph_layout(path: &Path) -> Result<StateGraphLayoutFile, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn apply_state_graph_layout(graph: &mut StateGraphDocument, layout: &StateGraphLayoutFile) {
    let Some(saved) = layout.graphs.get(&graph.id) else {
        return;
    };
    graph.zoom = clamp_graph_zoom(saved.zoom);
    for node in &mut graph.nodes {
        if let Some(position) = saved.nodes.get(&node.id) {
            node.pos = *position;
        }
    }
}

fn clamp_graph_zoom(value: f64) -> f64 {
    value.clamp(0.35, 2.75)
}

fn default_graph_zoom() -> f64 {
    1.0
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

    #[test]
    fn state_graph_canvas_pair_loads_reference_current_and_saved_layout() {
        let pair = StateGraphCanvasPair::load(workspace_root()).unwrap();
        let melee = pair.graph("melee_reference").expect("melee graph");
        let mole = pair.graph("mole_current").expect("mole graph");

        assert_eq!(pair.graphs.len(), 2);
        assert_eq!(pair.layout_path, "config/state_graph_layout.json");
        assert!(melee.nodes.len() > 30);
        assert!(mole.edges.len() > 30);
        assert_eq!(melee.zoom, 0.712);
        assert_eq!(mole.node_position("Wait"), Some([3.816, -0.569]));
        assert!(pair.validation_errors().is_empty());
    }
}
