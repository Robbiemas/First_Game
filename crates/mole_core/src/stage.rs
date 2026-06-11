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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageProfile {
    pub name: &'static str,
    pub main_floor: StageSurface,
    pub soft_platforms: [StageSurface; 3],
    pub blast_zones: StageBlastZones,
    pub spawn_points: [StageSpawnPoint; 4],
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
    pub main_floor: StageSurface,
    pub soft_platforms: [StageSurface; 3],
    pub blast_zones: StageBlastZones,
    pub spawn_points: [StageSpawnPoint; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageCollisionProfile {
    pub scale: f32,
    pub vertices: &'static [StageCollisionVertex],
    pub lines: &'static [StageCollisionLine],
    pub joints: &'static [StageCollisionJoint],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageCollisionVertex {
    pub index: u16,
    pub source_x: f32,
    pub source_y: f32,
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
            blast_zones: self.blast_zones,
            spawn_points: self.spawn_points,
        }
    }
}

impl StageCollisionProfile {
    pub fn scaled_vertex(self, index: usize) -> Option<(f32, f32)> {
        self.vertices
            .get(index)
            .map(|vertex| (vertex.source_x * self.scale, vertex.source_y * self.scale))
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
