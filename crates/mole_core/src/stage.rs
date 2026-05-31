use crate::units::melee_units_f32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageSurfaceKind {
    Solid,
    Soft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageSurface {
    pub name: &'static str,
    pub kind: StageSurfaceKind,
    pub left_x: i32,
    pub right_x: i32,
    pub y: i32,
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
pub struct StageProfile {
    pub name: &'static str,
    pub main_floor: StageSurface,
    pub soft_platforms: [StageSurface; 3],
    pub blast_zones: StageBlastZones,
    pub spawn_points: [StageSpawnPoint; 4],
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

    pub fn battlefield_test() -> Self {
        Self {
            name: "battlefield_test",
            main_floor: StageSurface {
                name: "main_floor",
                kind: StageSurfaceKind::Solid,
                left_x: melee_units_f32(-68.4),
                right_x: melee_units_f32(68.4),
                y: 0,
            },
            soft_platforms: [
                StageSurface {
                    name: "left_platform",
                    kind: StageSurfaceKind::Soft,
                    left_x: melee_units_f32(-57.600_002),
                    right_x: melee_units_f32(-20.0),
                    y: melee_units_f32(27.200_1),
                },
                StageSurface {
                    name: "right_platform",
                    kind: StageSurfaceKind::Soft,
                    left_x: melee_units_f32(20.0),
                    right_x: melee_units_f32(57.600_002),
                    y: melee_units_f32(27.200_1),
                },
                StageSurface {
                    name: "top_platform",
                    kind: StageSurfaceKind::Soft,
                    left_x: melee_units_f32(-18.800_001),
                    right_x: melee_units_f32(18.800_001),
                    y: melee_units_f32(54.400_1),
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
                    x: melee_units_f32(-38.8),
                    y: melee_units_f32(35.2),
                    facing: 1,
                },
                StageSpawnPoint {
                    x: melee_units_f32(38.8),
                    y: melee_units_f32(35.2),
                    facing: -1,
                },
                StageSpawnPoint {
                    x: 0,
                    y: melee_units_f32(8.0),
                    facing: 1,
                },
                StageSpawnPoint {
                    x: 0,
                    y: melee_units_f32(62.4),
                    facing: 1,
                },
            ],
        }
    }
}
