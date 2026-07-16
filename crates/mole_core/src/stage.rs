use crate::units::{melee_units_f32, source_units_to_milli};

#[path = "generated/stages.rs"]
mod generated_stages;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageSurfaceKind {
    Solid,
    Soft,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageSurface {
    pub name: &'static str,
    pub kind: StageSurfaceKind,
    pub left_x: i32,
    pub right_x: i32,
    pub y: i32,
    pub friction_multiplier: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageBlastZones {
    pub left_x: i32,
    pub right_x: i32,
    pub top_y: i32,
    pub bottom_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageSpawnPoint {
    pub x: i32,
    pub y: i32,
    pub facing: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageRespawnPlatform {
    pub platform_index: u16,
    pub stage_point_index: u16,
    pub final_x: i32,
    pub final_y: i32,
    pub top_y: i32,
    pub facing: i8,
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageProfile {
    pub name: &'static str,
    pub main_floor: StageSurface,
    pub soft_platforms: [StageSurface; 3],
    pub ledges: &'static [StageLedge],
    pub blast_zones: StageBlastZones,
    pub spawn_points: [StageSpawnPoint; 4],
    pub respawn_platforms: [StageRespawnPlatform; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageSource {
    pub kind: &'static str,
    pub dat_file: &'static str,
    pub decomp_ref: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeleeStageProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub source: StageSource,
    pub collision: StageCollisionProfile,
    pub ledges: &'static [StageLedge],
    pub dynamic_collision: StageDynamicCollisionProfile,
    pub camera: StageCameraInfo,
    pub source_blast_zones: StageFloatBounds,
    pub map_head: StageMapHeadProfile,
    pub callbacks: StageCallbackProfile,
    pub main_floor: StageSurface,
    pub soft_platforms: [StageSurface; 3],
    pub blast_zones: StageBlastZones,
    pub spawn_points: [StageSpawnPoint; 4],
    pub respawn_platforms: [StageRespawnPlatform; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageCollisionProfile {
    pub scale: f32,
    pub vertices: &'static [StageCollisionVertex],
    pub lines: &'static [StageCollisionLine],
    pub joints: &'static [StageCollisionJoint],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageLedge {
    pub index: u16,
    pub line_index: u16,
    pub side: StageLedgeSide,
    pub x_milli: i32,
    pub y_milli: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageLedgeSide {
    Left,
    Right,
}

const EMPTY_STAGE_LEDGES: &[StageLedge] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageDynamicCollisionProfile {
    pub line_start: i16,
    pub line_count: i16,
    pub joint_count_with_dynamic_lines: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageFloatBounds {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageCameraInfo {
    pub cam_bounds: StageFloatBounds,
    pub cam_x_offset: f32,
    pub cam_y_offset: f32,
    pub cam_vertical_tilt: f32,
    pub cam_pan_degrees: f32,
    pub x20: f32,
    pub x24: f32,
    pub cam_track_ratio: f32,
    pub cam_fixed_zoom: f32,
    pub cam_track_smooth: f32,
    pub cam_zoom_rate: f32,
    pub cam_max_depth: f32,
    pub x3c: f32,
    pub pausecam_zpos_min: f32,
    pub pausecam_zpos_init: f32,
    pub pausecam_zpos_max: f32,
    pub cam_angle_up: f32,
    pub cam_angle_down: f32,
    pub cam_angle_left: f32,
    pub cam_angle_right: f32,
    pub fixed_cam_pos: StageVec3,
    pub fixed_cam_fov: f32,
    pub fixed_cam_vert_angle: f32,
    pub fixed_cam_horz_angle: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageCallbackProfile {
    pub stage_data_symbol: &'static str,
    pub callback_table_symbol: &'static str,
    pub object_callbacks: &'static [StageObjectCallbacks],
    pub on_init: &'static str,
    pub on_demo_init: &'static str,
    pub on_load: &'static str,
    pub on_start: &'static str,
    pub callback4: &'static str,
    pub on_touch_line: &'static str,
    pub on_check_shadow_render: &'static str,
    pub flags2: u32,
    pub spawn_table_symbol: &'static str,
    pub spawn_table: &'static [StageSpawnMapping],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageObjectCallbacks {
    pub object_id: u16,
    pub callback0: &'static str,
    pub callback1: &'static str,
    pub callback2: &'static str,
    pub callback3: &'static str,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageSpawnMapping {
    pub index: u16,
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageMapHeadProfile {
    pub stage_dat_offset: u32,
    pub unk0_offset: u32,
    pub unk4: i32,
    pub entries_offset: u32,
    pub entry_count: i32,
    pub splines_offset: u32,
    pub spline_count: i32,
    pub unk18_offset: u32,
    pub unk1c: i32,
    pub unk20_offset: u32,
    pub unk24: i32,
    pub internals_offset: u32,
    pub internal_count: i32,
    pub entries: &'static [StageMapHeadEntry],
    pub joints: &'static [StageMapHeadJoint],
    pub point_mappings: &'static [StageMapHeadPointMapping],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageMapHeadPointMapping {
    pub tree_index: u16,
    pub stage_info_index: u16,
    pub joint_index: Option<u16>,
    pub source_position: StageVec3,
    pub scaled_x: i32,
    pub scaled_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageMapHeadEntry {
    pub index: u16,
    pub joint_root_offset: u32,
    pub joint_root_index: Option<u16>,
    pub camera_desc_offset: u32,
    pub x14_offset: u32,
    pub x18_offset: u32,
    pub fog_desc_offset: u32,
    pub vector_offset: u32,
    pub vector_count: i32,
    pub x28_offset: u32,
    pub x2c_offset: u32,
    pub x30: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageMapHeadJoint {
    pub index: u16,
    pub node_offset: u32,
    pub class_name_offset: u32,
    pub flags: u32,
    pub child_offset: u32,
    pub next_offset: u32,
    pub child_index: Option<u16>,
    pub next_index: Option<u16>,
    pub dobjdesc_offset: u32,
    pub rotation: StageVec3,
    pub scale: StageVec3,
    pub position: StageVec3,
    pub mtx_offset: u32,
    pub robjdesc_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageCollisionVertex {
    pub index: u16,
    pub source_x: f32,
    pub source_y: f32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub x10: f32,
    pub x14: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageCollisionLineKind {
    Floor,
    SoftFloor,
    Ceiling,
    RightWall,
    LeftWall,
    Dynamic,
}

pub const STAGE_LINE_FLAG_PLATFORM: u16 = 0x0100;
pub const STAGE_LINE_FLAG_LEDGE: u16 = 0x0200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageCollisionLine {
    pub index: u16,
    pub kind: StageCollisionLineKind,
    pub passable: bool,
    pub v0_idx: u16,
    pub v1_idx: u16,
    pub prev_id0: i16,
    pub next_id0: i16,
    pub prev_id1: i16,
    pub next_id1: i16,
    pub hi_flags: u16,
    pub lo_flags: u16,
}

impl StageCollisionLine {
    pub const fn has_ledge_flag(self) -> bool {
        self.lo_flags & STAGE_LINE_FLAG_LEDGE != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageScaledCollisionLine {
    pub index: u16,
    pub kind: StageCollisionLineKind,
    pub passable: bool,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub x0_milli: i32,
    pub y0_milli: i32,
    pub x1_milli: i32,
    pub y1_milli: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageCollisionJoint {
    pub index: u16,
    pub floor_start: i16,
    pub floor_count: i16,
    pub ceiling_start: i16,
    pub ceiling_count: i16,
    pub right_wall_start: i16,
    pub right_wall_count: i16,
    pub left_wall_start: i16,
    pub left_wall_count: i16,
    pub dynamic_start: i16,
    pub dynamic_count: i16,
    pub left_bound_milli: i32,
    pub bottom_bound_milli: i32,
    pub right_bound_milli: i32,
    pub top_bound_milli: i32,
    pub vtx_start: i16,
    pub vtx_count: i16,
}

impl StageProfile {
    pub fn collision_surfaces(self) -> [StageSurface; 4] {
        [
            self.main_floor,
            self.soft_platforms[0],
            self.soft_platforms[1],
            self.soft_platforms[2],
        ]
    }

    pub fn battlefield() -> Self {
        MeleeStageProfile::battlefield().compat_stage_profile()
    }

    pub fn battlefield_test() -> Self {
        Self {
            name: "battlefield_test",
            ..Self::battlefield()
        }
    }

    pub fn dev_flat_test() -> Self {
        let hidden_platform = StageSurface {
            name: "unused_dev_platform",
            kind: StageSurfaceKind::Soft,
            left_x: melee_units_f32(10_000.0),
            right_x: melee_units_f32(10_000.0),
            y: melee_units_f32(10_000.0),
            friction_multiplier: 1.0,
        };
        Self {
            name: "dev_flat_test",
            main_floor: StageSurface {
                name: "dev_floor",
                kind: StageSurfaceKind::Solid,
                left_x: melee_units_f32(-96.0),
                right_x: melee_units_f32(96.0),
                y: 0,
                friction_multiplier: 1.0,
            },
            soft_platforms: [hidden_platform; 3],
            ledges: EMPTY_STAGE_LEDGES,
            blast_zones: StageBlastZones {
                left_x: melee_units_f32(-224.0),
                right_x: melee_units_f32(224.0),
                top_y: melee_units_f32(200.0),
                bottom_y: melee_units_f32(-108.8),
            },
            spawn_points: [
                StageSpawnPoint {
                    x: 0,
                    y: 0,
                    facing: 1,
                },
                StageSpawnPoint {
                    x: melee_units_f32(16.0),
                    y: 0,
                    facing: -1,
                },
                StageSpawnPoint {
                    x: melee_units_f32(-16.0),
                    y: 0,
                    facing: 1,
                },
                StageSpawnPoint {
                    x: 0,
                    y: melee_units_f32(20.0),
                    facing: -1,
                },
            ],
            respawn_platforms: [StageRespawnPlatform {
                platform_index: 0,
                stage_point_index: 0,
                final_x: 0,
                final_y: 0,
                top_y: melee_units_f32(200.0),
                facing: 1,
                offset_x: 0,
                offset_y: 0,
            }; 4],
        }
    }

    pub fn melee_stage_profile(self) -> Option<MeleeStageProfile> {
        match self.name {
            "battlefield" | "battlefield_test" => Some(MeleeStageProfile::battlefield()),
            _ => None,
        }
    }
}

impl MeleeStageProfile {
    pub fn battlefield() -> Self {
        generated_stages::BATTLEFIELD_STAGE
    }

    pub fn compat_stage_profile(self) -> StageProfile {
        StageProfile {
            name: self.id,
            main_floor: self.main_floor,
            soft_platforms: self.soft_platforms,
            ledges: self.ledges,
            blast_zones: self.blast_zones,
            spawn_points: self.spawn_points,
            respawn_platforms: self.respawn_platforms,
        }
    }
}

impl StageCollisionProfile {
    pub fn scaled_vertex(self, index: usize) -> Option<(f32, f32)> {
        self.vertices
            .get(index)
            .map(|vertex| (vertex.pos_x, vertex.pos_y))
    }

    pub fn previous_vertex(self, index: usize) -> Option<(f32, f32)> {
        self.vertices
            .get(index)
            .map(|vertex| (vertex.x10, vertex.x14))
    }

    pub fn scaled_line(self, index: usize) -> Option<StageScaledCollisionLine> {
        let line = *self.lines.get(index)?;
        let (x0, y0) = self.scaled_vertex(line.v0_idx as usize)?;
        let (x1, y1) = self.scaled_vertex(line.v1_idx as usize)?;
        Some(StageScaledCollisionLine {
            index: line.index,
            kind: line.kind,
            passable: line.passable,
            x0,
            y0,
            x1,
            y1,
            x0_milli: source_units_to_milli(x0),
            y0_milli: source_units_to_milli(y0),
            x1_milli: source_units_to_milli(x1),
            y1_milli: source_units_to_milli(y1),
        })
    }
}
