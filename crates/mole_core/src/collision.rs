use crate::{
    stage::{StageProfile, StageSurface, StageSurfaceKind},
    state::Vec2,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule3 {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl Capsule3 {
    pub const fn new(a: Vec3, b: Vec3, radius: f32) -> Self {
        Self { a, b, radius }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat3x4 {
    pub rows: [[f32; 4]; 3],
}

impl Mat3x4 {
    pub const fn from_rows(rows: [[f32; 4]; 3]) -> Self {
        Self { rows }
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        Vec3::new(
            self.rows[0][0] * point.x
                + self.rows[0][1] * point.y
                + self.rows[0][2] * point.z
                + self.rows[0][3],
            self.rows[1][0] * point.x
                + self.rows[1][1] * point.y
                + self.rows[1][2] * point.z
                + self.rows[1][3],
            self.rows[2][0] * point.x
                + self.rows[2][1] * point.y
                + self.rows[2][2] * point.z
                + self.rows[2][3],
        )
    }
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn sub3(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn add3(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn mul3(v: Vec3, scalar: f32) -> Vec3 {
    Vec3::new(v.x * scalar, v.y * scalar, v.z * scalar)
}

fn closest_distance_sq_between_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    let d1 = sub3(q1, p1);
    let d2 = sub3(q2, p2);
    let r = sub3(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);

    let (mut s, mut t);
    if a <= f32::EPSILON && e <= f32::EPSILON {
        return dot(r, r);
    }
    if a <= f32::EPSILON {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = dot(d1, r);
        if e <= f32::EPSILON {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            s = if denom.abs() > f32::EPSILON {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
        }
    }

    let c1 = add3(p1, mul3(d1, s));
    let c2 = add3(p2, mul3(d2, t));
    dot(sub3(c1, c2), sub3(c1, c2))
}

pub fn capsules_intersect_3d(hit: &Capsule3, hurt: &Capsule3) -> bool {
    let allowed = hit.radius + hurt.radius;
    closest_distance_sq_between_segments(hit.a, hit.b, hurt.a, hurt.b) <= allowed * allowed
}

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
    landing_contact_for_bottom_with_floor_skip(
        stage,
        previous_bottom,
        current_bottom,
        None,
        drop_through_soft_platforms,
    )
}

pub fn landing_contact_for_bottom_with_floor_skip(
    stage: StageProfile,
    previous_bottom: Vec2,
    current_bottom: Vec2,
    floor_skip_surface: Option<u8>,
    drop_through_soft_platforms: bool,
) -> Option<StageLandingContact> {
    if current_bottom.y >= previous_bottom.y {
        return None;
    }

    let mut best = None;
    for (surface_index, surface) in stage.collision_surfaces().into_iter().enumerate() {
        if floor_skip_surface == Some(surface_index as u8) {
            continue;
        }
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

pub fn has_floor_support(stage: StageProfile, bottom: Vec2) -> bool {
    floor_surface_for_bottom(stage, bottom).is_some()
}

pub fn floor_surface_for_bottom(stage: StageProfile, bottom: Vec2) -> Option<StageSurface> {
    floor_surface_index_for_bottom(stage, bottom).map(|(_, surface)| surface)
}

pub(crate) fn floor_surface_index_for_bottom(
    stage: StageProfile,
    bottom: Vec2,
) -> Option<(u8, StageSurface)> {
    stage
        .collision_surfaces()
        .into_iter()
        .enumerate()
        .find(|(_, surface)| {
            bottom.y == surface.y && bottom.x >= surface.left_x && bottom.x <= surface.right_x
        })
        .map(|(index, surface)| (index as u8, surface))
}
