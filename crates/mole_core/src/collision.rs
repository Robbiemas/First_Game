use crate::{
    stage::{StageProfile, StageSurface, StageSurfaceKind},
    state::Vec2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcbDiamond {
    pub top: Vec2,
    pub right: Vec2,
    pub bottom: Vec2,
    pub left: Vec2,
}

impl EcbDiamond {
    pub const fn from_bottom_center_and_size(bottom_center: Vec2, width: i32, height: i32) -> Self {
        let half_width = width / 2;
        let half_height = height / 2;

        Self {
            top: Vec2 {
                x: bottom_center.x,
                y: bottom_center.y + height,
            },
            right: Vec2 {
                x: bottom_center.x + half_width,
                y: bottom_center.y + half_height,
            },
            bottom: bottom_center,
            left: Vec2 {
                x: bottom_center.x - half_width,
                y: bottom_center.y + half_height,
            },
        }
    }

    pub const fn points(self) -> [Vec2; 4] {
        [self.top, self.right, self.bottom, self.left]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageLandingContact {
    pub surface: StageSurface,
    pub y: i32,
}

pub fn landing_contact_for_bottom(
    stage: StageProfile,
    previous_bottom: Vec2,
    current_bottom: Vec2,
    drop_through_soft_platforms: bool,
) -> Option<StageLandingContact> {
    if current_bottom.y > previous_bottom.y {
        return None;
    }

    let mut best = None;
    for surface in stage.collision_surfaces() {
        if drop_through_soft_platforms && surface.kind == StageSurfaceKind::Soft {
            continue;
        }
        if current_bottom.x < surface.left_x || current_bottom.x > surface.right_x {
            continue;
        }
        if previous_bottom.y >= surface.y && current_bottom.y <= surface.y {
            let contact = StageLandingContact {
                surface,
                y: surface.y,
            };
            if best
                .map(|existing: StageLandingContact| contact.y > existing.y)
                .unwrap_or(true)
            {
                best = Some(contact);
            }
        }
    }

    best
}
