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
pub struct StageProfile {
    pub name: &'static str,
    pub main_floor: StageSurface,
    pub soft_platforms: [StageSurface; 3],
    pub blast_zones: StageBlastZones,
}

impl StageProfile {
    pub fn battlefield_test() -> Self {
        Self {
            name: "battlefield_test",
            main_floor: StageSurface {
                name: "main_floor",
                kind: StageSurfaceKind::Solid,
                left_x: melee_units_f32(-68.4000015259),
                right_x: melee_units_f32(68.4000015259),
                y: 0,
            },
            soft_platforms: [
                StageSurface {
                    name: "left_platform",
                    kind: StageSurfaceKind::Soft,
                    left_x: melee_units_f32(-57.60000228881836),
                    right_x: melee_units_f32(-20.0),
                    y: melee_units_f32(27.20009994506836),
                },
                StageSurface {
                    name: "right_platform",
                    kind: StageSurfaceKind::Soft,
                    left_x: melee_units_f32(20.0),
                    right_x: melee_units_f32(57.60000228881836),
                    y: melee_units_f32(27.20009994506836),
                },
                StageSurface {
                    name: "top_platform",
                    kind: StageSurfaceKind::Soft,
                    left_x: melee_units_f32(-18.80000114440918),
                    right_x: melee_units_f32(18.80000114440918),
                    y: melee_units_f32(54.40010070800781),
                },
            ],
            blast_zones: StageBlastZones {
                left_x: melee_units_f32(-224.0),
                right_x: melee_units_f32(224.0),
                top_y: melee_units_f32(200.0),
                bottom_y: melee_units_f32(-108.8),
            },
        }
    }
}
