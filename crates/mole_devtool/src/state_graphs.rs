use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use mole_core::{source_state_sequence_for_motion_state, MotionState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateGraphsSurface {
    pub edges: Vec<StateGraphEntry>,
    pub missing_edge_count: usize,
    pub missing_node_count: usize,
    pub nodes: Vec<StateGraphEntry>,
    pub source_gap_count: usize,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateGraphLayoutSaveReport {
    pub layout_path: String,
    pub mutated: bool,
    pub graph_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateGraphBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateGraphCanvasView {
    pub zoom: f64,
    pub pan: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateGraphSelection {
    Node { graph_id: String, id: String },
    Edge { graph_id: String, index: usize },
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
        let source_gap_count = count_action_motion_source_gaps(
            &root
                .as_ref()
                .join("docs/state_graphs/action_motion_tables.json"),
        );

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
            source_gap_count,
            nodes,
            edges,
        })
    }

    pub fn summary(&self) -> String {
        format!(
            "Nodes: {} total | {} missing | Edges: {} total | {} missing | {} source gaps",
            self.total_node_count,
            self.missing_node_count,
            self.total_edge_count,
            self.missing_edge_count,
            self.source_gap_count
        )
    }

    pub fn rows(&self) -> Vec<StateGraphEntry> {
        let mut rows = Vec::with_capacity(self.nodes.len() + self.edges.len());
        rows.extend(self.nodes.iter().cloned());
        rows.extend(self.edges.iter().cloned());
        rows
    }
}

fn count_action_motion_source_gaps(path: &Path) -> usize {
    let Some(artifact) = load_optional_action_motion_tables(path) else {
        return 0;
    };
    artifact
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.get("completeness_class").and_then(Value::as_str),
                        Some("absent" | "placeholder" | "present_partial")
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

impl StateGraphCanvasPair {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let layout_path = root.join("config/state_graph_layout.json");
        let layout = load_state_graph_layout(&layout_path)?;
        let action_motion_tables = load_optional_action_motion_tables(
            &root.join("docs/state_graphs/action_motion_tables.json"),
        );
        let mut graphs = Vec::new();
        for filename in ["melee_reference_graph.json", "mole_current_graph.json"] {
            let path = root.join("docs/state_graphs").join(filename);
            let mut graph = load_state_graph_document(&path)?;
            overlay_source_state_sequences(&mut graph);
            overlay_action_motion_tables(&mut graph, action_motion_tables.as_ref());
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

    pub fn empty() -> Self {
        Self {
            graphs: Vec::new(),
            layout_path: "config/state_graph_layout.json".to_string(),
        }
    }

    pub fn validation_errors(&self) -> Vec<String> {
        self.graphs
            .iter()
            .flat_map(StateGraphDocument::validation_errors)
            .collect()
    }

    pub fn set_node_position(
        &mut self,
        graph_id: &str,
        node_id: &str,
        position: [f64; 2],
    ) -> Result<(), String> {
        let graph = self
            .graphs
            .iter_mut()
            .find(|graph| graph.id == graph_id)
            .ok_or_else(|| format!("graph {graph_id} is not loaded"))?;
        let node = graph
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
            .ok_or_else(|| format!("graph {graph_id} has no node {node_id}"))?;
        node.pos = position;
        Ok(())
    }

    pub fn validate_layout_save(&self) -> StateGraphLayoutSaveReport {
        StateGraphLayoutSaveReport {
            layout_path: self.layout_path.clone(),
            mutated: false,
            graph_count: self.graphs.len(),
            errors: self.validation_errors(),
        }
    }

    pub fn save_layout(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<StateGraphLayoutSaveReport, String> {
        let mut report = self.validate_layout_save();
        if !report.errors.is_empty() {
            return Ok(report);
        }
        let layout = StateGraphLayoutFile {
            version: 1,
            graphs: self
                .graphs
                .iter()
                .map(|graph| {
                    (
                        graph.id.clone(),
                        StateGraphLayoutGraph {
                            zoom: clamp_graph_zoom(graph.zoom),
                            nodes: graph
                                .nodes
                                .iter()
                                .map(|node| (node.id.clone(), node.pos))
                                .collect(),
                        },
                    )
                })
                .collect(),
        };
        let path = root.as_ref().join(&self.layout_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(&layout)
            .map_err(|error| format!("failed to serialize {}: {error}", self.layout_path))?;
        fs::write(&path, format!("{text}\n"))
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        report.mutated = true;
        Ok(report)
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

    pub fn bounds(&self) -> Option<StateGraphBounds> {
        let first = self.nodes.first()?;
        let mut bounds = StateGraphBounds {
            min_x: first.pos[0],
            min_y: first.pos[1],
            max_x: first.pos[0],
            max_y: first.pos[1],
        };
        for node in &self.nodes {
            bounds.min_x = bounds.min_x.min(node.pos[0]);
            bounds.min_y = bounds.min_y.min(node.pos[1]);
            bounds.max_x = bounds.max_x.max(node.pos[0]);
            bounds.max_y = bounds.max_y.max(node.pos[1]);
        }
        Some(bounds)
    }

    pub fn node_canvas_position(
        &self,
        view: &StateGraphCanvasView,
        canvas_size: [f64; 2],
        id: &str,
    ) -> Option<[f64; 2]> {
        let position = self.node_position(id)?;
        self.graph_position_to_canvas(view, canvas_size, position)
    }

    pub fn edge_canvas_midpoint(
        &self,
        view: &StateGraphCanvasView,
        canvas_size: [f64; 2],
        index: usize,
    ) -> Option<[f64; 2]> {
        let edge = self.edges.get(index)?;
        let from = self.node_canvas_position(view, canvas_size, &edge.from)?;
        let to = self.node_canvas_position(view, canvas_size, &edge.to)?;
        Some([(from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5])
    }

    pub fn graph_delta_from_canvas_delta(
        &self,
        view: &StateGraphCanvasView,
        canvas_size: [f64; 2],
        delta: [f64; 2],
    ) -> Option<[f64; 2]> {
        let bounds = self.bounds()?;
        let width = (bounds.max_x - bounds.min_x).max(1.0);
        let height = (bounds.max_y - bounds.min_y).max(1.0);
        let zoom = view.zoom.max(0.01);
        Some([
            delta[0] * width / canvas_size[0].max(1.0) / zoom,
            delta[1] * height / canvas_size[1].max(1.0) / zoom,
        ])
    }

    pub fn selection_at_canvas_point(
        &self,
        view: &StateGraphCanvasView,
        canvas_size: [f64; 2],
        point: [f64; 2],
    ) -> Option<StateGraphSelection> {
        for node in self.nodes.iter().rev() {
            let Some(center) = self.graph_position_to_canvas(view, canvas_size, node.pos) else {
                continue;
            };
            if (point[0] - center[0]).abs() <= STATE_GRAPH_NODE_WIDTH * 0.5
                && (point[1] - center[1]).abs() <= STATE_GRAPH_NODE_HEIGHT * 0.5
            {
                return Some(StateGraphSelection::Node {
                    graph_id: self.id.clone(),
                    id: node.id.clone(),
                });
            }
        }

        for (index, edge) in self.edges.iter().enumerate() {
            let Some(from) = self.node_canvas_position(view, canvas_size, &edge.from) else {
                continue;
            };
            let Some(to) = self.node_canvas_position(view, canvas_size, &edge.to) else {
                continue;
            };
            if point_to_segment_distance(point, from, to) <= STATE_GRAPH_EDGE_HIT_RADIUS {
                return Some(StateGraphSelection::Edge {
                    graph_id: self.id.clone(),
                    index,
                });
            }
        }

        None
    }

    pub fn graph_position_to_canvas(
        &self,
        view: &StateGraphCanvasView,
        canvas_size: [f64; 2],
        pos: [f64; 2],
    ) -> Option<[f64; 2]> {
        let bounds = self.bounds()?;
        let width = (bounds.max_x - bounds.min_x).max(1.0);
        let height = (bounds.max_y - bounds.min_y).max(1.0);
        let base_x = ((pos[0] - bounds.min_x) / width) * canvas_size[0];
        let base_y = ((pos[1] - bounds.min_y) / height) * canvas_size[1];
        let center = [canvas_size[0] * 0.5, canvas_size[1] * 0.5];
        Some([
            center[0] + (base_x - center[0]) * view.zoom + view.pan[0],
            center[1] + (base_y - center[1]) * view.zoom + view.pan[1],
        ])
    }
}

impl StateGraphCanvasView {
    pub fn from_graph_zoom(zoom: f64) -> Self {
        Self {
            zoom: clamp_graph_zoom(zoom),
            pan: [0.0, 0.0],
        }
    }

    pub fn zoom_by(&mut self, factor: f64) {
        self.zoom = clamp_graph_zoom(self.zoom * factor);
    }

    pub fn pan_by(&mut self, delta: [f64; 2]) {
        self.pan[0] += delta[0];
        self.pan[1] += delta[1];
    }
}

impl StateGraphSelection {
    pub fn graph_id(&self) -> &str {
        match self {
            StateGraphSelection::Node { graph_id, .. }
            | StateGraphSelection::Edge { graph_id, .. } => graph_id,
        }
    }

    pub fn detail(&self, graph: &StateGraphDocument) -> Option<String> {
        if self.graph_id() != graph.id {
            return None;
        }
        match self {
            StateGraphSelection::Node { id, .. } => {
                let node = graph.nodes.iter().find(|node| node.id == *id)?;
                Some(format!(
                    "node: {}\nlabel: {}\nstatus: {}\nposition: {:.3}, {:.3}\n{}",
                    node.id,
                    non_empty_or_dash(&node.label),
                    non_empty_or_dash(&node.status),
                    node.pos[0],
                    node.pos[1],
                    metadata_detail(&node.metadata)
                ))
            }
            StateGraphSelection::Edge { index, .. } => {
                let edge = graph.edges.get(*index)?;
                Some(format!(
                    "edge: {} -> {}\nlabel: {}\ninput: {}\nframes: {}\nstatus: {}\n{}",
                    edge.from,
                    edge.to,
                    edge.label.as_deref().unwrap_or("-"),
                    non_empty_or_dash(&edge.input),
                    non_empty_or_dash(&edge.frames),
                    non_empty_or_dash(&edge.status),
                    metadata_detail(&edge.metadata)
                ))
            }
        }
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

const STATE_GRAPH_NODE_WIDTH: f64 = 94.0;
const STATE_GRAPH_NODE_HEIGHT: f64 = 42.0;
const STATE_GRAPH_EDGE_HIT_RADIUS: f64 = 8.0;

fn point_to_segment_distance(point: [f64; 2], from: [f64; 2], to: [f64; 2]) -> f64 {
    let segment = [to[0] - from[0], to[1] - from[1]];
    let length_squared = segment[0] * segment[0] + segment[1] * segment[1];
    if length_squared <= f64::EPSILON {
        return ((point[0] - from[0]).powi(2) + (point[1] - from[1]).powi(2)).sqrt();
    }
    let t = (((point[0] - from[0]) * segment[0] + (point[1] - from[1]) * segment[1])
        / length_squared)
        .clamp(0.0, 1.0);
    let projection = [from[0] + segment[0] * t, from[1] + segment[1] * t];
    ((point[0] - projection[0]).powi(2) + (point[1] - projection[1]).powi(2)).sqrt()
}

fn metadata_detail(metadata: &BTreeMap<String, Value>) -> String {
    if metadata.is_empty() {
        return "metadata: -".to_string();
    }
    metadata
        .iter()
        .map(|(key, value)| format!("{key}: {}", compact_json(Some(value))))
        .collect::<Vec<_>>()
        .join("\n")
}

fn non_empty_or_dash(value: &str) -> &str {
    if value.is_empty() {
        "-"
    } else {
        value
    }
}

fn load_state_graph_document(path: &Path) -> Result<StateGraphDocument, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn load_optional_action_motion_tables(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn overlay_action_motion_tables(graph: &mut StateGraphDocument, artifact: Option<&Value>) {
    let Some(entries) = artifact
        .and_then(|artifact| artifact.get("entries"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let entries_by_state = entries
        .iter()
        .filter_map(|entry| Some((entry.get("state")?.as_str()?.to_string(), entry)))
        .collect::<BTreeMap<_, _>>();
    for node in &mut graph.nodes {
        let Some(entry) = entries_by_state.get(&node.id) else {
            continue;
        };
        if let Some(value) = entry.get("source_action_key").cloned() {
            node.metadata.insert("source_action_key".to_string(), value);
        }
        if let Some(value) = entry.get("source_action_name").cloned() {
            node.metadata
                .insert("source_action_name".to_string(), value);
        }
        if let Some(value) = entry.get("runtime_binding").cloned() {
            node.metadata
                .insert("source_runtime_binding".to_string(), value);
        }
        if let Some(value) = entry.get("completeness_class").cloned() {
            node.metadata
                .insert("source_completeness_class".to_string(), value);
        }
    }
}

fn overlay_source_state_sequences(graph: &mut StateGraphDocument) {
    for node in &mut graph.nodes {
        let Some(motion_state) = motion_state_from_graph_id(&node.id) else {
            continue;
        };
        let Some(sequence) = source_state_sequence_for_motion_state(motion_state) else {
            continue;
        };
        node.metadata.insert(
            "source_sequence_file".to_string(),
            Value::String(sequence.source_file.to_string()),
        );
        node.metadata.insert(
            "source_callback_order".to_string(),
            Value::Array(
                sequence
                    .callbacks
                    .iter()
                    .map(|callback| {
                        json!({
                            "phase": callback.phase,
                            "function": callback.function,
                            "effect": callback.effect,
                        })
                    })
                    .collect(),
            ),
        );
    }

    for edge in &mut graph.edges {
        let Some(from) = motion_state_from_graph_id(&edge.from) else {
            continue;
        };
        let Some(to) = motion_state_from_graph_id(&edge.to) else {
            continue;
        };
        let Some(sequence) = source_state_sequence_for_motion_state(from) else {
            continue;
        };
        let Some(transition) = sequence
            .transitions
            .iter()
            .find(|transition| transition.to == to)
        else {
            continue;
        };
        edge.metadata.insert(
            "source_transition_trigger".to_string(),
            Value::String(transition.trigger.to_string()),
        );
        edge.metadata.insert(
            "source_transition_function".to_string(),
            Value::String(transition.function.to_string()),
        );
        edge.metadata.insert(
            "source_transition_ordering".to_string(),
            Value::String(transition.ordering.to_string()),
        );
    }
}

fn motion_state_from_graph_id(id: &str) -> Option<MotionState> {
    match id {
        "Landing" => Some(MotionState::Landing),
        "KneeBend" => Some(MotionState::KneeBend),
        "Wait" => Some(MotionState::Wait),
        "Fall" => Some(MotionState::Fall),
        "JumpF" => Some(MotionState::JumpF),
        "JumpB" => Some(MotionState::JumpB),
        _ => None,
    }
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
        assert!(surface.summary().contains("source gaps"));
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
        let bounds = mole.bounds().expect("bounds");
        assert!(bounds.max_x > bounds.min_x);
        assert!(bounds.max_y > bounds.min_y);
    }

    #[test]
    fn state_graph_canvas_view_clamps_zoom_and_tracks_pan() {
        let mut view = StateGraphCanvasView::from_graph_zoom(1.0);

        view.zoom_by(10.0);
        assert_eq!(view.zoom, 2.75);

        view.zoom_by(0.01);
        assert_eq!(view.zoom, 0.35);

        view.pan_by([12.0, -8.0]);
        assert_eq!(view.pan, [12.0, -8.0]);
    }

    #[test]
    fn state_graph_canvas_hit_testing_selects_nodes_and_edges() {
        let graph = StateGraphDocument {
            id: "test".to_string(),
            title: "Test".to_string(),
            root: "Wait".to_string(),
            description: String::new(),
            zoom: 1.0,
            nodes: vec![
                StateGraphNode {
                    id: "Wait".to_string(),
                    label: "Wait".to_string(),
                    pos: [0.0, 0.0],
                    status: "complete".to_string(),
                    metadata: BTreeMap::new(),
                },
                StateGraphNode {
                    id: "Dash".to_string(),
                    label: "Dash".to_string(),
                    pos: [10.0, 0.0],
                    status: "missing".to_string(),
                    metadata: BTreeMap::new(),
                },
            ],
            edges: vec![StateGraphEdge {
                from: "Wait".to_string(),
                to: "Dash".to_string(),
                input: "stick_x".to_string(),
                frames: "1".to_string(),
                label: Some("Dash input".to_string()),
                status: "partial".to_string(),
                metadata: BTreeMap::new(),
            }],
        };
        let view = StateGraphCanvasView::from_graph_zoom(graph.zoom);
        let canvas_size = [320.0, 180.0];
        let wait = graph
            .node_canvas_position(&view, canvas_size, "Wait")
            .expect("wait position");
        let edge_midpoint = graph
            .edge_canvas_midpoint(&view, canvas_size, 0)
            .expect("edge midpoint");

        assert_eq!(
            graph.selection_at_canvas_point(&view, canvas_size, wait),
            Some(StateGraphSelection::Node {
                graph_id: "test".to_string(),
                id: "Wait".to_string()
            })
        );
        assert_eq!(
            graph.selection_at_canvas_point(&view, canvas_size, edge_midpoint),
            Some(StateGraphSelection::Edge {
                graph_id: "test".to_string(),
                index: 0
            })
        );
    }

    #[test]
    fn state_graph_canvas_converts_screen_drag_into_graph_delta() {
        let graph = StateGraphDocument {
            id: "test".to_string(),
            title: "Test".to_string(),
            root: "Wait".to_string(),
            description: String::new(),
            zoom: 1.0,
            nodes: vec![
                StateGraphNode {
                    id: "Wait".to_string(),
                    label: "Wait".to_string(),
                    pos: [0.0, 0.0],
                    status: "complete".to_string(),
                    metadata: BTreeMap::new(),
                },
                StateGraphNode {
                    id: "Dash".to_string(),
                    label: "Dash".to_string(),
                    pos: [10.0, 20.0],
                    status: "partial".to_string(),
                    metadata: BTreeMap::new(),
                },
            ],
            edges: Vec::new(),
        };
        let view = StateGraphCanvasView::from_graph_zoom(2.0);

        assert_eq!(
            graph.graph_delta_from_canvas_delta(&view, [100.0, 200.0], [10.0, 20.0]),
            Some([0.5, 1.0])
        );
    }

    #[test]
    fn state_graph_selection_details_expose_node_and_edge_values() {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "notes".to_string(),
            Value::String("grounded idle".to_string()),
        );
        let graph = StateGraphDocument {
            id: "test".to_string(),
            title: "Test".to_string(),
            root: "Wait".to_string(),
            description: String::new(),
            zoom: 1.0,
            nodes: vec![StateGraphNode {
                id: "Wait".to_string(),
                label: "Wait".to_string(),
                pos: [0.0, 0.0],
                status: "complete".to_string(),
                metadata: metadata.clone(),
            }],
            edges: vec![StateGraphEdge {
                from: "Wait".to_string(),
                to: "Wait".to_string(),
                input: "none".to_string(),
                frames: "1".to_string(),
                label: Some("loop".to_string()),
                status: "complete".to_string(),
                metadata,
            }],
        };

        let node_detail = StateGraphSelection::Node {
            graph_id: "test".to_string(),
            id: "Wait".to_string(),
        }
        .detail(&graph)
        .expect("node detail");
        assert!(node_detail.contains("node: Wait"));
        assert!(node_detail.contains("status: complete"));
        assert!(node_detail.contains("notes: grounded idle"));

        let edge_detail = StateGraphSelection::Edge {
            graph_id: "test".to_string(),
            index: 0,
        }
        .detail(&graph)
        .expect("edge detail");
        assert!(edge_detail.contains("edge: Wait -> Wait"));
        assert!(edge_detail.contains("input: none"));
        assert!(edge_detail.contains("label: loop"));
    }

    #[test]
    fn state_graph_canvas_pair_overlays_source_state_sequence_metadata() {
        let pair = StateGraphCanvasPair::load(workspace_root()).unwrap();
        let melee = pair.graph("melee_reference").expect("melee graph");
        let landing = melee
            .nodes
            .iter()
            .find(|node| node.id == "Landing")
            .expect("Landing node");
        assert!(landing
            .metadata
            .get("source_callback_order")
            .and_then(Value::as_array)
            .is_some_and(|callbacks| {
                callbacks
                    .iter()
                    .filter_map(|callback| callback.get("phase").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    == ["Anim", "IASA", "Phys", "Coll"]
            }));

        let jump_edge = melee
            .edges
            .iter()
            .find(|edge| edge.from == "KneeBend" && edge.to == "JumpF")
            .expect("KneeBend -> JumpF edge");
        assert_eq!(
            jump_edge
                .metadata
                .get("source_transition_function")
                .and_then(Value::as_str),
            Some("ftCo_KneeBend_Anim / ftCo_Jump_Enter")
        );
    }

    #[test]
    fn state_graph_canvas_pair_overlays_action_motion_table_metadata() {
        let root = std::env::temp_dir().join(format!(
            "mole_devtool_state_graph_action_motion_{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let graphs_dir = root.join("docs/state_graphs");
        let config_dir = root.join("config");
        fs::create_dir_all(&graphs_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        let graph = |id: &str, title: &str| {
            serde_json::json!({
                "id": id,
                "title": title,
                "root": "Wait",
                "nodes": [
                    {"id": "Wait", "label": "Wait", "pos": [0.0, 0.0], "status": "reference"},
                    {"id": "Attack12", "label": "Attack12", "pos": [1.0, 0.0], "status": "reference"}
                ],
                "edges": []
            })
        };
        fs::write(
            graphs_dir.join("melee_reference_graph.json"),
            serde_json::to_string_pretty(&graph("melee_reference", "Melee Reference")).unwrap(),
        )
        .unwrap();
        fs::write(
            graphs_dir.join("mole_current_graph.json"),
            serde_json::to_string_pretty(&graph("mole_current", "Mole Current")).unwrap(),
        )
        .unwrap();
        fs::write(
            graphs_dir.join("action_motion_tables.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "artifact_kind": "action_motion_tables",
                "entries": [
                    {
                        "state": "Attack12",
                        "source_action_key": "Attack12",
                        "source_action_name": "PlyCaptain5K_Share_ACTION_Attack12_figatree",
                        "runtime_binding": null,
                        "completeness_class": "absent"
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            config_dir.join("state_graph_layout.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "graphs": {
                    "melee_reference": {"zoom": 1.0, "nodes": {"Wait": [0.0, 0.0], "Attack12": [1.0, 0.0]}},
                    "mole_current": {"zoom": 1.0, "nodes": {"Wait": [0.0, 0.0], "Attack12": [1.0, 0.0]}}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let pair = StateGraphCanvasPair::load(&root).unwrap();
        let graph = pair.graph("melee_reference").unwrap();
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == "Attack12")
            .unwrap();

        assert_eq!(
            node.metadata
                .get("source_action_key")
                .and_then(Value::as_str),
            Some("Attack12")
        );
        assert_eq!(
            node.metadata
                .get("source_completeness_class")
                .and_then(Value::as_str),
            Some("absent")
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn state_graph_canvas_pair_saves_updated_layout_positions() {
        let root = std::env::temp_dir().join(format!(
            "mole_devtool_state_graph_layout_{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let graphs_dir = root.join("docs/state_graphs");
        let config_dir = root.join("config");
        fs::create_dir_all(&graphs_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        let graph = |id: &str, title: &str| {
            serde_json::json!({
                "id": id,
                "title": title,
                "root": "Wait",
                "nodes": [
                    {"id": "Wait", "label": "Wait", "pos": [0.0, 0.0], "status": "reference"},
                    {"id": "Dash", "label": "Dash", "pos": [1.0, 0.0], "status": "partial"}
                ],
                "edges": [
                    {"from": "Wait", "to": "Dash", "input": "tap", "frames": "1", "status": "partial"}
                ]
            })
        };
        fs::write(
            graphs_dir.join("melee_reference_graph.json"),
            serde_json::to_string_pretty(&graph("melee_reference", "Melee Reference")).unwrap(),
        )
        .unwrap();
        fs::write(
            graphs_dir.join("mole_current_graph.json"),
            serde_json::to_string_pretty(&graph("mole_current", "Mole Current")).unwrap(),
        )
        .unwrap();
        fs::write(
            config_dir.join("state_graph_layout.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "graphs": {
                    "melee_reference": {"zoom": 0.5, "nodes": {"Wait": [3.0, 4.0], "Dash": [4.0, 4.0]}},
                    "mole_current": {"zoom": 0.75, "nodes": {"Wait": [5.0, 6.0], "Dash": [6.0, 6.0]}}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let mut pair = StateGraphCanvasPair::load(&root).unwrap();
        pair.set_node_position("mole_current", "Wait", [9.25, -2.5])
            .unwrap();
        let report = pair.save_layout(&root).unwrap();
        let reloaded = StateGraphCanvasPair::load(&root).unwrap();

        assert_eq!(report.mutated, true);
        assert_eq!(report.graph_count, 2);
        assert!(report.errors.is_empty());
        assert_eq!(
            reloaded
                .graph("mole_current")
                .unwrap()
                .node_position("Wait"),
            Some([9.25, -2.5])
        );
        assert_eq!(
            reloaded
                .graph("melee_reference")
                .unwrap()
                .node_position("Wait"),
            Some([3.0, 4.0])
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
