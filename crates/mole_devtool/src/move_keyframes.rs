use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use mole_core::{
    melee_units_f32, EcbDiamond, MeleeActionStateId, MotionState, SourceActionKey, StageBlastZones,
    StageProfile, StageSpawnPoint, StageSurface, StageSurfaceKind, Vec2, World,
};
use mole_runtime::{RenderFrame, RenderScene};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveKeyframesSurface {
    pub gaps: Vec<serde_json::Value>,
    pub keyframes: Vec<MoveKeyframe>,
    pub sources: Vec<serde_json::Value>,
    pub state: String,
    pub summary: MoveKeyframesSummary,
    pub target_character: String,
    pub target_character_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveKeyframesSummary {
    #[serde(default)]
    pub active_body_volume_windows: Vec<serde_json::Value>,
    #[serde(default)]
    pub active_hitbox_windows: Vec<serde_json::Value>,
    #[serde(default)]
    pub active_hurtbox_windows: Vec<serde_json::Value>,
    pub iasa_frame: String,
    pub landing_lag_frames: String,
    pub total_frames: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveKeyframe {
    #[serde(default)]
    pub body_volumes: Vec<serde_json::Value>,
    pub frame: usize,
    #[serde(default)]
    pub hitboxes: Vec<serde_json::Value>,
    #[serde(default)]
    pub hurtboxes: Vec<serde_json::Value>,
    #[serde(default)]
    pub interpolates_from_previous: bool,
    #[serde(default)]
    pub pose: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct MoveKeyframeVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveKeyframeJobjJoint {
    pub index: usize,
    #[serde(default)]
    pub parent_index: Option<usize>,
    pub depth: usize,
    #[serde(default)]
    pub position_milli: MoveKeyframeVec3,
    #[serde(default)]
    pub position_raw: MoveKeyframeVec3,
    #[serde(default)]
    pub rotation_milli: MoveKeyframeVec3,
    #[serde(default)]
    pub rotation_raw: MoveKeyframeVec3,
    #[serde(default)]
    pub scale_milli: MoveKeyframeVec3,
    #[serde(default)]
    pub scale_raw: MoveKeyframeVec3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveKeyframeJobjSource {
    pub file: String,
    pub format: String,
    pub joint_count: usize,
    pub root: String,
    pub root_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MoveKeyframeJobjTreeFile {
    pub joints: Vec<MoveKeyframeJobjJoint>,
    pub source: MoveKeyframeJobjSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveKeyframeJobjTree {
    pub root: String,
    pub joints: Vec<MoveKeyframeJobjJoint>,
    pub source: MoveKeyframeJobjSource,
}

impl From<MoveKeyframeJobjTreeFile> for MoveKeyframeJobjTree {
    fn from(tree: MoveKeyframeJobjTreeFile) -> Self {
        Self {
            root: tree.source.root.clone(),
            joints: tree.joints,
            source: tree.source,
        }
    }
}

impl MoveKeyframeJobjTree {
    pub fn flattened_joint_points_right_facing(&self) -> Vec<[f64; 2]> {
        let mut points: Vec<[f64; 2]> = Vec::with_capacity(self.joints.len());
        for joint in &self.joints {
            let local = joint.position_milli;
            let mut point = [local.x, local.y];
            if let Some(parent_index) = joint.parent_index {
                if let Some(parent) = points.get(parent_index) {
                    point[0] += parent[0];
                    point[1] += parent[1];
                }
            }
            points.push(point);
        }
        points
    }
}

pub fn move_keyframe_pose_joint_positions(frame: &MoveKeyframe) -> Vec<[f64; 3]> {
    frame
        .pose
        .get("joints")
        .and_then(Value::as_array)
        .map(|joints| {
            joints
                .iter()
                .filter_map(move_keyframe_joint_position)
                .collect()
        })
        .unwrap_or_default()
}

impl MoveKeyframesSurface {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let path = root
            .as_ref()
            .join("resources/melee/frame_data/dolphin_mole/AttackAirN.json");
        Self::load_from(&path)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse move keyframes artifact: {error}"))
    }

    pub fn summary(&self) -> String {
        format!(
            "Target: {} | State: {} | Total frames: {} | Keyframes: {} | Gaps: {} | Sources: {}",
            self.target_character_label,
            self.state,
            self.summary.total_frames,
            self.keyframes.len(),
            self.gaps.len(),
            self.sources.len()
        )
    }

    pub fn source_path_for_kind(&self, kind: &str) -> Option<PathBuf> {
        self.sources.iter().find_map(|source| {
            let matches_kind = source.get("kind").and_then(Value::as_str) == Some(kind);
            if !matches_kind {
                return None;
            }
            source
                .get("path")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
    }
}

impl From<&MoveKeyframesSurface> for LedgerTabTemplate {
    fn from(surface: &MoveKeyframesSurface) -> Self {
        Self {
            title: format!("{} Move Keyframes", surface.target_character_label),
            summary: surface.summary(),
            headers: vec![
                "Frame".to_string(),
                "Pose".to_string(),
                "Hitboxes".to_string(),
                "Hurtboxes".to_string(),
                "Body Volumes".to_string(),
                "Interpolates".to_string(),
            ],
            rows: surface
                .keyframes
                .iter()
                .map(LedgerTabTemplateRow::from)
                .collect(),
        }
    }
}

impl From<&MoveKeyframe> for LedgerTabTemplateRow {
    fn from(row: &MoveKeyframe) -> Self {
        Self {
            cells: vec![
                row.frame.to_string(),
                if row.pose.is_null() {
                    "-".to_string()
                } else {
                    row.pose.to_string()
                },
                row.hitboxes.len().to_string(),
                row.hurtboxes.len().to_string(),
                row.body_volumes.len().to_string(),
                row.interpolates_from_previous.to_string(),
            ],
            detail: format!(
                "frame: {}\npose: {}\ninterpolates_from_previous: {}\nhitboxes: {}\nhurtboxes: {}\nbody_volumes: {}",
                row.frame,
                if row.pose.is_null() { "-" } else { "<pose json>" },
                row.interpolates_from_previous,
                row.hitboxes.len(),
                row.hurtboxes.len(),
                row.body_volumes.len()
            ),
            status: Some(if row.interpolates_from_previous { "derived" } else { "match" }.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKeyframeEndpoint {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKeyframeBodyPoint {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MoveKeyframeHandleKind {
    HurtboxEndpoint {
        hurtbox_index: usize,
        endpoint: MoveKeyframeEndpoint,
    },
    HitboxEndpoint {
        hitbox_index: usize,
        endpoint: MoveKeyframeEndpoint,
    },
    EcbPoint {
        body_volume_index: usize,
        point: MoveKeyframeBodyPoint,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveKeyframeHandle {
    pub kind: MoveKeyframeHandleKind,
    pub position: [f64; 3],
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveKeyframeSelection {
    pub kind: MoveKeyframeHandleKind,
    pub label: String,
    pub position: [f64; 3],
}

#[derive(Debug, Clone)]
pub struct MoveKeyframesRuntimePreview {
    pub frame: RenderFrame,
    pub scene: RenderScene,
}

#[derive(Debug, Clone)]
pub struct MoveKeyframesEditorSurface {
    surface: MoveKeyframesSurface,
    pose_tree: Option<MoveKeyframeJobjTree>,
    artifact_path: Option<PathBuf>,
    selected_frame_index: usize,
    selected_handle: Option<MoveKeyframeHandleKind>,
    dirty: bool,
}

impl MoveKeyframesEditorSurface {
    pub fn from_surface(surface: MoveKeyframesSurface) -> Self {
        Self {
            surface,
            pose_tree: None,
            artifact_path: None,
            selected_frame_index: 0,
            selected_handle: None,
            dirty: false,
        }
    }

    pub fn from_surface_with_workspace(
        surface: MoveKeyframesSurface,
        root: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let pose_tree = load_move_keyframe_pose_tree(root.as_ref(), &surface)?;
        Ok(Self {
            surface,
            pose_tree: Some(pose_tree),
            artifact_path: Some(
                root.as_ref()
                    .join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
            ),
            selected_frame_index: 0,
            selected_handle: None,
            dirty: false,
        })
    }

    pub fn selected_frame_index(&self) -> usize {
        self.selected_frame_index
    }

    pub fn set_selected_frame_index(&mut self, index: usize) {
        if index < self.surface.keyframes.len() {
            self.selected_frame_index = index;
            self.prune_selected_handle();
        }
    }

    pub fn select_previous_frame(&mut self) -> bool {
        if self.selected_frame_index == 0 {
            return false;
        }
        self.selected_frame_index -= 1;
        self.prune_selected_handle();
        true
    }

    pub fn select_next_frame(&mut self) -> bool {
        if self.selected_frame_index + 1 >= self.surface.keyframes.len() {
            return false;
        }
        self.selected_frame_index += 1;
        self.prune_selected_handle();
        true
    }

    pub fn frame_count(&self) -> usize {
        self.surface.keyframes.len()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn selected_frame(&self) -> Option<&MoveKeyframe> {
        self.surface.keyframes.get(self.selected_frame_index)
    }

    pub fn selected_frame_mut(&mut self) -> Option<&mut MoveKeyframe> {
        self.surface.keyframes.get_mut(self.selected_frame_index)
    }

    pub fn set_selected_frame_interpolates_from_previous(&mut self, value: bool) -> bool {
        let Some(frame) = self.selected_frame_mut() else {
            return false;
        };
        if frame.interpolates_from_previous == value {
            return false;
        }
        frame.interpolates_from_previous = value;
        self.mark_dirty();
        true
    }

    pub fn pose_tree(&self) -> Option<&MoveKeyframeJobjTree> {
        self.pose_tree.as_ref()
    }

    pub fn artifact_path(&self) -> Option<&Path> {
        self.artifact_path.as_deref()
    }

    pub fn handles_for_selected_frame(&self) -> Vec<MoveKeyframeHandle> {
        self.selected_frame()
            .map(move_keyframe_handles_for_frame)
            .unwrap_or_default()
    }

    pub fn select_handle(&mut self, kind: MoveKeyframeHandleKind) -> bool {
        if self
            .handles_for_selected_frame()
            .iter()
            .any(|handle| handle.kind == kind)
        {
            self.selected_handle = Some(kind);
            true
        } else {
            false
        }
    }

    pub fn selected_handle(&self) -> Option<&MoveKeyframeHandleKind> {
        self.selected_handle.as_ref()
    }

    pub fn selected_handle_detail(&self) -> Option<MoveKeyframeSelection> {
        let selected = self.selected_handle.as_ref()?;
        self.handles_for_selected_frame()
            .into_iter()
            .find(|handle| &handle.kind == selected)
            .map(|handle| MoveKeyframeSelection {
                kind: handle.kind,
                label: handle.label,
                position: handle.position,
            })
    }

    pub fn drag_selected_handle(&mut self, delta: [f64; 2]) -> bool {
        let Some(kind) = self.selected_handle.clone() else {
            return false;
        };
        self.drag_handle(kind, delta)
    }

    pub fn drag_handle(&mut self, kind: MoveKeyframeHandleKind, delta: [f64; 2]) -> bool {
        let Some(frame) = self.selected_frame_mut() else {
            return false;
        };
        let changed = match kind {
            MoveKeyframeHandleKind::HurtboxEndpoint {
                hurtbox_index,
                endpoint,
            } => move_keyframe_drag_capsule_endpoint(
                &mut frame.hurtboxes,
                hurtbox_index,
                endpoint,
                delta,
            ),
            MoveKeyframeHandleKind::HitboxEndpoint {
                hitbox_index,
                endpoint,
            } => move_keyframe_drag_capsule_endpoint(
                &mut frame.hitboxes,
                hitbox_index,
                endpoint,
                delta,
            ),
            MoveKeyframeHandleKind::EcbPoint {
                body_volume_index,
                point,
            } => move_keyframe_drag_body_volume_point(
                &mut frame.body_volumes,
                body_volume_index,
                point,
                delta,
            ),
        };
        if changed {
            self.dirty = true;
        }
        changed
    }

    fn prune_selected_handle(&mut self) {
        let Some(kind) = self.selected_handle.clone() else {
            return;
        };
        if !self
            .handles_for_selected_frame()
            .iter()
            .any(|handle| handle.kind == kind)
        {
            self.selected_handle = None;
        }
    }

    pub fn save_to(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        let text = serde_json::to_string_pretty(&self.surface)
            .map_err(|error| format!("failed to serialize move keyframes artifact: {error}"))?;
        fs::write(path, text)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        self.clear_dirty();
        Ok(())
    }

    pub fn save_to_source(&mut self) -> Result<(), String> {
        let Some(path) = self.artifact_path.clone() else {
            return Err("move keyframes editor has no source artifact path".to_string());
        };
        self.save_to(path)
    }

    pub fn runtime_preview(
        &self,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Option<MoveKeyframesRuntimePreview> {
        let frame = self.selected_frame()?;
        let source_frame = frame.frame.min(u8::MAX as usize) as u8;
        let mut runtime_frame = RenderFrame::from_world(&World::for_two_players_on_stage(
            move_keyframes_preview_stage(),
        ));

        runtime_frame.player_positions[0] = Vec2 { x: 0, y: 0 };
        runtime_frame.player_positions[1] = Vec2 { x: 10_000, y: 0 };
        runtime_frame.player_velocities = [Vec2 { x: 0, y: 0 }, Vec2 { x: 0, y: 0 }];
        runtime_frame.player_ecbs[0] = selected_frame_ecb(frame).unwrap_or_else(|| {
            EcbDiamond::from_bottom_center_and_size(Vec2 { x: 0, y: 0 }, 36, 72)
        });
        runtime_frame.player_ecbs[1] =
            EcbDiamond::from_bottom_center_and_size(Vec2 { x: 10_000, y: 0 }, 36, 72);
        runtime_frame.player_grounded = [true, true];
        runtime_frame.player_facings = [1, -1];
        runtime_frame.player_motion_states = [MotionState::AttackAirN, MotionState::Wait];
        runtime_frame.player_source_pose_motion_states =
            [MotionState::AttackAirN, MotionState::Wait];
        runtime_frame.player_source_pose_action_state_ids = [
            Some(MeleeActionStateId::new(65)),
            Some(MeleeActionStateId::new(14)),
        ];
        runtime_frame.player_source_action_keys = [
            Some(SourceActionKey::new("AttackAirN")),
            Some(SourceActionKey::new("Wait1")),
        ];
        runtime_frame.player_source_pose_action_keys = [
            Some(SourceActionKey::new("AttackAirN")),
            Some(SourceActionKey::new("Wait1")),
        ];
        runtime_frame.player_source_pose_frames = [source_frame, 1];
        runtime_frame.player_source_pose_model_facings = [1, -1];
        runtime_frame.player_state_frames = [source_frame, 1];
        runtime_frame.player_animation_frames = [source_frame, 1];
        runtime_frame.player_action_state_ids = [
            Some(MeleeActionStateId::new(65)),
            Some(MeleeActionStateId::new(14)),
        ];
        runtime_frame.player_profile_weights = [104.0, 104.0];

        let mut scene = RenderScene::from_frame(&runtime_frame, viewport_width, viewport_height);
        scene.stage_surfaces = vec![scene.stage];
        scene.entry_platforms = [None, None];
        Some(MoveKeyframesRuntimePreview {
            frame: runtime_frame,
            scene,
        })
    }
}

fn move_keyframes_preview_stage() -> StageProfile {
    StageProfile {
        name: "move_keyframes_flat_preview",
        main_floor: StageSurface {
            name: "debug_floor",
            kind: StageSurfaceKind::Solid,
            left_x: melee_units_f32(-96.0),
            right_x: melee_units_f32(96.0),
            y: 0,
            friction_multiplier: 1.0,
        },
        soft_platforms: [
            StageSurface {
                name: "left_platform",
                kind: StageSurfaceKind::Soft,
                left_x: melee_units_f32(-16.0),
                right_x: melee_units_f32(-12.0),
                y: melee_units_f32(96.0),
                friction_multiplier: 1.0,
            },
            StageSurface {
                name: "right_platform",
                kind: StageSurfaceKind::Soft,
                left_x: melee_units_f32(12.0),
                right_x: melee_units_f32(16.0),
                y: melee_units_f32(96.0),
                friction_multiplier: 1.0,
            },
            StageSurface {
                name: "top_platform",
                kind: StageSurfaceKind::Soft,
                left_x: melee_units_f32(-4.0),
                right_x: melee_units_f32(4.0),
                y: melee_units_f32(120.0),
                friction_multiplier: 1.0,
            },
        ],
        blast_zones: StageBlastZones {
            left_x: melee_units_f32(-224.0),
            right_x: melee_units_f32(224.0),
            top_y: melee_units_f32(200.0),
            bottom_y: melee_units_f32(-108.8),
        },
        spawn_points: [
            StageSpawnPoint {
                x: melee_units_f32(-16.0),
                y: 0,
                facing: 1,
            },
            StageSpawnPoint {
                x: melee_units_f32(16.0),
                y: 0,
                facing: -1,
            },
            StageSpawnPoint {
                x: 0,
                y: melee_units_f32(8.0),
                facing: 1,
            },
            StageSpawnPoint {
                x: 0,
                y: melee_units_f32(20.0),
                facing: -1,
            },
        ],
    }
}

fn load_move_keyframe_pose_tree(
    root: &Path,
    surface: &MoveKeyframesSurface,
) -> Result<MoveKeyframeJobjTree, String> {
    let relative_path = surface
        .source_path_for_kind("extracted_costume_skeleton")
        .ok_or_else(|| {
            "move keyframes surface is missing extracted_costume_skeleton source path".to_string()
        })?;
    let path = root.join(relative_path);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let tree: MoveKeyframeJobjTreeFile = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse pose tree: {error}"))?;
    Ok(tree.into())
}

fn move_keyframe_handles_for_frame(frame: &MoveKeyframe) -> Vec<MoveKeyframeHandle> {
    let mut handles = Vec::new();

    for (index, hurtbox) in frame.hurtboxes.iter().enumerate() {
        push_endpoints(&mut handles, index, hurtbox, true, "hurtbox", |endpoint| {
            MoveKeyframeHandleKind::HurtboxEndpoint {
                hurtbox_index: index,
                endpoint,
            }
        });
    }

    for (index, hitbox) in frame.hitboxes.iter().enumerate() {
        push_endpoints(&mut handles, index, hitbox, false, "hitbox", |endpoint| {
            MoveKeyframeHandleKind::HitboxEndpoint {
                hitbox_index: index,
                endpoint,
            }
        });
    }

    for (index, body_volume) in frame.body_volumes.iter().enumerate() {
        for point in [
            MoveKeyframeBodyPoint::Left,
            MoveKeyframeBodyPoint::Right,
            MoveKeyframeBodyPoint::Top,
            MoveKeyframeBodyPoint::Bottom,
            MoveKeyframeBodyPoint::Center,
        ] {
            if let Some(position) = move_keyframe_body_volume_point(body_volume, point) {
                handles.push(MoveKeyframeHandle {
                    kind: MoveKeyframeHandleKind::EcbPoint {
                        body_volume_index: index,
                        point,
                    },
                    position,
                    label: format!("body_volume_{index}_{point:?}"),
                });
            }
        }
    }

    handles
}

fn move_keyframe_drag_capsule_endpoint(
    shapes: &mut [Value],
    index: usize,
    endpoint: MoveKeyframeEndpoint,
    delta: [f64; 2],
) -> bool {
    let Some(shape) = shapes.get_mut(index) else {
        return false;
    };
    let keys: &[&str] = match endpoint {
        MoveKeyframeEndpoint::A => &["a_pos", "a", "center"],
        MoveKeyframeEndpoint::B => &["b_pos", "b", "center"],
    };
    move_keyframe_drag_point_by_keys(shape, keys, delta)
}

fn move_keyframe_drag_body_volume_point(
    shapes: &mut [Value],
    index: usize,
    point: MoveKeyframeBodyPoint,
    delta: [f64; 2],
) -> bool {
    let Some(shape) = shapes.get_mut(index) else {
        return false;
    };
    let key = match point {
        MoveKeyframeBodyPoint::Left => "left",
        MoveKeyframeBodyPoint::Right => "right",
        MoveKeyframeBodyPoint::Top => "top",
        MoveKeyframeBodyPoint::Bottom => "bottom",
        MoveKeyframeBodyPoint::Center => "center",
    };
    move_keyframe_drag_point_by_keys(shape, &[key], delta)
}

fn move_keyframe_drag_point_by_keys(shape: &mut Value, keys: &[&str], delta: [f64; 2]) -> bool {
    for key in keys {
        if let Some(point) = shape.get_mut(key) {
            if move_keyframe_drag_point(point, delta) {
                return true;
            }
        }
    }
    false
}

fn move_keyframe_drag_point(point: &mut Value, delta: [f64; 2]) -> bool {
    if let Some(array) = point.as_array_mut() {
        if array.len() >= 3 {
            let Some(z) = array[2].as_f64() else {
                return false;
            };
            let Some(y) = array[1].as_f64() else {
                return false;
            };
            array[2] = Value::from(z + delta[0]);
            array[1] = Value::from(y + delta[1]);
            return true;
        }
        if array.len() >= 2 {
            let Some(x) = array[0].as_f64() else {
                return false;
            };
            let Some(y) = array[1].as_f64() else {
                return false;
            };
            array[0] = Value::from(x + delta[0]);
            array[1] = Value::from(y + delta[1]);
            return true;
        }
    }

    if let Some(object) = point.as_object_mut() {
        let x = object.get("x").and_then(Value::as_f64);
        let y = object.get("y").and_then(Value::as_f64);
        if let (Some(x), Some(y)) = (x, y) {
            object.insert("x".to_string(), Value::from(x + delta[0]));
            object.insert("y".to_string(), Value::from(y + delta[1]));
            return true;
        }
        let z = object.get("z").and_then(Value::as_f64);
        if let (Some(z), Some(y)) = (z, y) {
            object.insert("z".to_string(), Value::from(z + delta[0]));
            object.insert("y".to_string(), Value::from(y + delta[1]));
            return true;
        }
    }

    false
}

fn push_endpoints<F>(
    handles: &mut Vec<MoveKeyframeHandle>,
    index: usize,
    shape: &Value,
    prefer_pos: bool,
    label_prefix: &str,
    kind_for: F,
) where
    F: Fn(MoveKeyframeEndpoint) -> MoveKeyframeHandleKind,
{
    let first = if prefer_pos {
        move_keyframe_point3(shape, "a_pos")
            .or_else(|| move_keyframe_point3(shape, "a"))
            .or_else(|| move_keyframe_point3(shape, "center"))
    } else {
        move_keyframe_point3(shape, "a")
            .or_else(|| move_keyframe_point3(shape, "a_pos"))
            .or_else(|| move_keyframe_point3(shape, "center"))
    };
    let second = if prefer_pos {
        move_keyframe_point3(shape, "b_pos")
            .or_else(|| move_keyframe_point3(shape, "b"))
            .or_else(|| first)
    } else {
        move_keyframe_point3(shape, "b")
            .or_else(|| move_keyframe_point3(shape, "b_pos"))
            .or_else(|| first)
    };

    if let Some(position) = first {
        handles.push(MoveKeyframeHandle {
            kind: kind_for(MoveKeyframeEndpoint::A),
            position,
            label: format!("{label_prefix}_{index}_a"),
        });
    }
    if let Some(position) = second {
        handles.push(MoveKeyframeHandle {
            kind: kind_for(MoveKeyframeEndpoint::B),
            position,
            label: format!("{label_prefix}_{index}_b"),
        });
    }
}

fn move_keyframe_joint_position(joint: &Value) -> Option<[f64; 3]> {
    move_keyframe_point3(joint, "world_position")
        .or_else(|| move_keyframe_point3(joint, "translation"))
        .or_else(|| {
            joint
                .get("world_matrix")
                .and_then(Value::as_array)
                .and_then(|matrix| {
                    if matrix.len() >= 4 {
                        let row = matrix[3].as_array()?;
                        if row.len() >= 3 {
                            Some([row[0].as_f64()?, row[1].as_f64()?, row[2].as_f64()?])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
        })
        .or_else(|| {
            joint
                .get("world_matrix")
                .and_then(Value::as_array)
                .and_then(|matrix| {
                    if matrix.len() >= 3 {
                        let x = matrix[0].as_array()?.get(3)?.as_f64()?;
                        let y = matrix[1].as_array()?.get(3)?.as_f64()?;
                        let z = matrix[2].as_array()?.get(3)?.as_f64()?;
                        Some([x, y, z])
                    } else {
                        None
                    }
                })
        })
}

fn move_keyframe_body_volume_point(
    shape: &Value,
    point: MoveKeyframeBodyPoint,
) -> Option<[f64; 3]> {
    let key = match point {
        MoveKeyframeBodyPoint::Left => "left",
        MoveKeyframeBodyPoint::Right => "right",
        MoveKeyframeBodyPoint::Top => "top",
        MoveKeyframeBodyPoint::Bottom => "bottom",
        MoveKeyframeBodyPoint::Center => "center",
    };
    move_keyframe_point3(shape, key)
        .or_else(|| move_keyframe_point3(shape, &format!("source_{key}")))
        .or_else(|| move_keyframe_point3(shape, &format!("{key}_pos")))
}

fn move_keyframe_point3(value: &Value, key: &str) -> Option<[f64; 3]> {
    let point = value.get(key)?;
    if let Some(array) = point.as_array() {
        if array.len() >= 3 {
            return Some([array[0].as_f64()?, array[1].as_f64()?, array[2].as_f64()?]);
        }
    }
    let x = point.get("x")?.as_f64()?;
    let y = point.get("y")?.as_f64()?;
    let z = point.get("z").and_then(Value::as_f64).unwrap_or(0.0);
    Some([x, y, z])
}

fn selected_frame_ecb(frame: &MoveKeyframe) -> Option<EcbDiamond> {
    let body_volume = frame.body_volumes.first()?;
    let top = move_keyframe_point2(body_volume, "top")?;
    let right = move_keyframe_point2(body_volume, "right")?;
    let bottom = move_keyframe_point2(body_volume, "bottom")?;
    let left = move_keyframe_point2(body_volume, "left")?;
    Some(EcbDiamond {
        top: Vec2 {
            x: top.0 as i32,
            y: top.1 as i32,
        },
        right: Vec2 {
            x: right.0 as i32,
            y: right.1 as i32,
        },
        bottom: Vec2 {
            x: bottom.0 as i32,
            y: bottom.1 as i32,
        },
        left: Vec2 {
            x: left.0 as i32,
            y: left.1 as i32,
        },
    })
}

fn move_keyframe_point2(value: &Value, key: &str) -> Option<(f64, f64)> {
    let point = value.get(key)?;
    if let Some(array) = point.as_array() {
        if array.len() >= 2 {
            return Some((array[0].as_f64()?, array[1].as_f64()?));
        }
    }
    let x = point.get("x")?.as_f64()?;
    let y = point.get("y")?.as_f64()?;
    Some((x, y))
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
    fn move_keyframes_loads_attack_air_n_and_templates_the_frames() {
        let surface = MoveKeyframesSurface::load(workspace_root()).unwrap();
        let template = LedgerTabTemplate::from(&surface);

        assert_eq!(surface.target_character_label, "Dolphin Mole");
        assert_eq!(surface.state, "AttackAirN");
        assert_eq!(template.title, "Dolphin Mole Move Keyframes");
        assert_eq!(template.headers.len(), 6);
        assert_eq!(template.rows.len(), surface.keyframes.len());
        assert_eq!(
            template.rows.first().unwrap().status.as_deref(),
            Some("match")
        );
    }

    #[test]
    fn move_keyframes_editor_exposes_frame_handles_and_dirty_state() {
        let surface = MoveKeyframesSurface::load(workspace_root()).unwrap();
        let mut editor = MoveKeyframesEditorSurface::from_surface(surface.clone());
        editor.set_selected_frame_index(6);

        assert_eq!(editor.selected_frame_index(), 6);
        assert_eq!(editor.frame_count(), surface.keyframes.len());
        let handles = editor.handles_for_selected_frame();
        assert!(
            !handles.is_empty(),
            "runtime edit handles should be backed by rendered collision or capsule data"
        );
        assert!(handles
            .iter()
            .any(|handle| matches!(handle.kind, MoveKeyframeHandleKind::EcbPoint { .. })));
        assert!(handles
            .iter()
            .any(|handle| matches!(handle.kind, MoveKeyframeHandleKind::HurtboxEndpoint { .. })));
        assert!(handles
            .iter()
            .any(|handle| matches!(handle.kind, MoveKeyframeHandleKind::HitboxEndpoint { .. })));
        assert!(!editor.is_dirty());
    }

    #[test]
    fn move_keyframes_editor_loads_the_figatree_joint_hierarchy_from_the_imported_skeleton() {
        let root = workspace_root();
        let surface = MoveKeyframesSurface::load(&root).unwrap();
        let editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(surface, &root).unwrap();
        let pose_tree = editor.pose_tree().expect("pose tree");

        assert_eq!(pose_tree.root, "PlyCaptain5K_Share_joint");
        assert_eq!(
            pose_tree.joints.first().map(|joint| joint.parent_index),
            Some(None)
        );
        assert!(pose_tree.joints.iter().any(|joint| joint.depth > 0));
        assert!(pose_tree
            .joints
            .iter()
            .any(|joint| joint.position_raw.y > 0.0));
    }

    #[test]
    fn move_keyframes_runtime_preview_uses_a_single_flat_ground_plane() {
        let root = workspace_root();
        let surface = MoveKeyframesSurface::load(&root).unwrap();
        let editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(surface, &root).unwrap();
        let preview = editor.runtime_preview(1280, 720).expect("runtime preview");

        assert_eq!(preview.scene.stage_surfaces.len(), 1);
        assert!(preview.scene.stage.width > preview.scene.stage.height);
    }

    #[test]
    fn move_keyframes_editor_selects_and_drags_the_active_handle() {
        let root = workspace_root();
        let surface = MoveKeyframesSurface::load(&root).unwrap();
        let mut editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(surface, &root).unwrap();
        editor.set_selected_frame_index(6);

        let handle = editor
            .handles_for_selected_frame()
            .into_iter()
            .find(|handle| {
                matches!(
                    handle.kind,
                    MoveKeyframeHandleKind::HitboxEndpoint {
                        endpoint: MoveKeyframeEndpoint::A,
                        ..
                    }
                )
            })
            .expect("hitbox endpoint handle");
        let before = handle.position;

        assert!(editor.select_handle(handle.kind.clone()));
        assert_eq!(editor.selected_handle(), Some(&handle.kind));
        assert_eq!(
            editor.selected_handle_detail().map(|detail| detail.label),
            Some(handle.label)
        );
        assert!(editor.drag_selected_handle([2.0, -3.0]));

        let after = editor
            .selected_handle_detail()
            .expect("selected handle after drag")
            .position;
        assert_eq!(after[0], before[0] + 2.0);
        assert_eq!(after[1], before[1] - 3.0);
        assert_eq!(after[2], before[2]);
        assert!(editor.is_dirty());
    }

    #[test]
    fn move_keyframes_runtime_handles_do_not_offer_raw_pose_joint_dragging() {
        let root = workspace_root();
        let surface = MoveKeyframesSurface::load(&root).unwrap();
        let editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(surface, &root).unwrap();

        assert!(
            editor
                .handles_for_selected_frame()
                .iter()
                .all(|handle| !handle.label.starts_with("joint_")),
            "raw pose JSON joints are preserved as source data until the runtime exposes typed pose edit handles"
        );
    }

    #[test]
    fn move_keyframes_dragging_a_hurtbox_endpoint_updates_only_that_endpoint() {
        let root = workspace_root();
        let surface = MoveKeyframesSurface::load(&root).unwrap();
        let mut editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(surface, &root).unwrap();
        editor.set_selected_frame_index(6);

        let before = editor.selected_frame().unwrap().hurtboxes[0].clone();
        assert!(editor.drag_handle(
            MoveKeyframeHandleKind::HurtboxEndpoint {
                hurtbox_index: 0,
                endpoint: MoveKeyframeEndpoint::A,
            },
            [1.0, -2.0],
        ));
        let after = editor.selected_frame().unwrap().hurtboxes[0].clone();

        assert_ne!(before, after);
        assert!(editor.is_dirty());
    }

    #[test]
    fn move_keyframes_save_round_trips_the_existing_json_shape() {
        let root = workspace_root();
        let source_path = root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json");
        let temp_path = root
            .join("target")
            .join("move_keyframes_editor_round_trip.json");
        let _ = std::fs::remove_file(&temp_path);

        let surface = MoveKeyframesSurface::load_from(&source_path).unwrap();
        let mut editor = MoveKeyframesEditorSurface::from_surface(surface);
        editor.set_selected_frame_index(6);
        assert!(editor.drag_handle(
            MoveKeyframeHandleKind::HurtboxEndpoint {
                hurtbox_index: 0,
                endpoint: MoveKeyframeEndpoint::A,
            },
            [1.0, -1.0]
        ));
        assert!(editor.is_dirty());

        editor.save_to(&temp_path).unwrap();
        assert!(!editor.is_dirty());

        let round_trip = MoveKeyframesSurface::load_from(&temp_path).unwrap();
        assert_eq!(round_trip.target_character_label, "Dolphin Mole");
        assert_eq!(round_trip.state, "AttackAirN");
        assert_eq!(round_trip.keyframes.len(), editor.frame_count());

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn move_keyframes_runtime_preview_builds_a_runtime_scene_snapshot() {
        let root = workspace_root();
        let surface = MoveKeyframesSurface::load(&root).unwrap();
        let mut editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(surface, &root).unwrap();
        editor.set_selected_frame_index(6);

        let preview = editor.runtime_preview(960, 540).expect("runtime preview");

        assert_eq!(preview.scene.stage_surfaces.len(), 1);
        assert_eq!(preview.scene.player_ecbs[0].points.len(), 4);
        assert_eq!(
            preview.frame.player_source_pose_motion_states[0],
            MotionState::AttackAirN
        );
        assert_eq!(
            preview.frame.player_source_pose_action_state_ids[0],
            Some(MeleeActionStateId::new(65))
        );
    }

    #[test]
    fn move_keyframes_editor_steps_selected_runtime_preview_frame() {
        let surface = MoveKeyframesSurface::load(workspace_root()).expect("move keyframes");
        let mut editor = MoveKeyframesEditorSurface::from_surface(surface);

        assert_eq!(editor.selected_frame_index(), 0);
        assert!(!editor.select_previous_frame());
        assert!(editor.select_next_frame());
        assert_eq!(editor.selected_frame_index(), 1);

        let preview = editor.runtime_preview(960, 540).expect("runtime preview");
        assert_eq!(
            preview.frame.player_source_pose_frames[0] as usize,
            editor.selected_frame().expect("frame").frame
        );

        assert!(editor.select_previous_frame());
        assert_eq!(editor.selected_frame_index(), 0);
    }
}
