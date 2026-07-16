use crate::{
    common_data::MeleeCommonData,
    stage::{
        StageCollisionLineKind, StageLedge, StageLedgeSide, StageProfile, StageSurface,
        StageSurfaceKind,
    },
    state::{is_source_damage_action_state_id, MeleeActionStateId, SourceActionKey, Vec2},
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

    pub fn transform_vector(&self, vector: Vec3) -> Vec3 {
        Vec3::new(
            self.rows[0][0] * vector.x + self.rows[0][1] * vector.y + self.rows[0][2] * vector.z,
            self.rows[1][0] * vector.x + self.rows[1][1] * vector.y + self.rows[1][2] * vector.z,
            self.rows[2][0] * vector.x + self.rows[2][1] * vector.y + self.rows[2][2] * vector.z,
        )
    }

    pub fn inverse_linear(self) -> Option<Self> {
        let m = self.rows;
        let a = m[0][0];
        let b = m[0][1];
        let c = m[0][2];
        let d = m[1][0];
        let e = m[1][1];
        let f = m[1][2];
        let g = m[2][0];
        let h = m[2][1];
        let i = m[2][2];
        let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
        if det.abs() <= f32::EPSILON {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Self::from_rows([
            [
                (e * i - f * h) * inv_det,
                (c * h - b * i) * inv_det,
                (b * f - c * e) * inv_det,
                0.0,
            ],
            [
                (f * g - d * i) * inv_det,
                (a * i - c * g) * inv_det,
                (c * d - a * f) * inv_det,
                0.0,
            ],
            [
                (d * h - e * g) * inv_det,
                (b * g - a * h) * inv_det,
                (a * e - b * d) * inv_det,
                0.0,
            ],
        ]))
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

fn closest_points_and_distance_sq_between_segments(
    p1: Vec3,
    q1: Vec3,
    p2: Vec3,
    q2: Vec3,
) -> (Vec3, Vec3, f32) {
    let d1 = sub3(q1, p1);
    let d2 = sub3(q2, p2);
    let r = sub3(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);

    let (mut s, mut t);
    if a <= f32::EPSILON && e <= f32::EPSILON {
        return (p1, p2, dot(r, r));
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
    (c1, c2, dot(sub3(c1, c2), sub3(c1, c2)))
}

fn closest_distance_sq_between_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    closest_points_and_distance_sq_between_segments(p1, q1, p2, q2).2
}

fn source_point_segment_distance_sq(start: Vec3, end: Vec3, point: Vec3) -> (f32, f32) {
    let delta = sub3(end, start);
    let from_point = sub3(start, point);
    let param = (-dot(delta, from_point) / dot(delta, delta)).clamp(0.0, 1.0);
    let closest = add3(start, mul3(delta, param));
    let offset = sub3(closest, point);
    (dot(offset, offset), param)
}

fn source_closest_points_between_segments(
    hit_start: Vec3,
    hit_end: Vec3,
    hurt_start: Vec3,
    hurt_end: Vec3,
) -> (Vec3, Vec3) {
    const NEAR_ZERO: f32 = 1.0e-5;

    let hit_delta = sub3(hit_end, hit_start);
    let hurt_delta = sub3(hurt_end, hurt_start);
    let start_delta = sub3(hit_start, hurt_start);
    let hit_len_sq = dot(hit_delta, hit_delta);
    let hurt_len_sq = dot(hurt_delta, hurt_delta);
    let segment_dot = dot(hit_delta, hurt_delta);
    let hit_start_dot = dot(hit_delta, start_delta);
    let hurt_start_dot = dot(hurt_delta, start_delta);
    let denom = hit_len_sq * hurt_len_sq - segment_dot * segment_dot;

    let (hit_param, hurt_param) = if hurt_len_sq.abs() < NEAR_ZERO {
        if hit_len_sq.abs() < NEAR_ZERO {
            (0.0, 0.0)
        } else {
            ((-hit_start_dot / hit_len_sq).clamp(0.0, 1.0), 0.0)
        }
    } else if denom.abs() < NEAR_ZERO {
        let hurt_mid = add3(hurt_start, mul3(hurt_delta, 0.5));
        let start_mid = sub3(hit_start, hurt_mid);
        let end_mid = sub3(hit_end, hurt_mid);
        if dot(start_mid, start_mid) < dot(end_mid, end_mid) {
            let (_, hurt_param) = source_point_segment_distance_sq(hurt_start, hurt_end, hit_start);
            (0.0, hurt_param)
        } else {
            let (_, hurt_param) = source_point_segment_distance_sq(hurt_start, hurt_end, hit_end);
            (1.0, hurt_param)
        }
    } else {
        let unconstrained_hit =
            (segment_dot * hurt_start_dot - hurt_len_sq * hit_start_dot) / denom;
        let unconstrained_hurt =
            (hit_len_sq * hurt_start_dot - segment_dot * hit_start_dot) / denom;
        if !(0.0..=1.0).contains(&unconstrained_hit) || !(0.0..=1.0).contains(&unconstrained_hurt) {
            let hit_endpoint_param = if unconstrained_hit < 0.0 { 0.0 } else { 1.0 };
            let hit_endpoint = if hit_endpoint_param == 0.0 {
                hit_start
            } else {
                hit_end
            };
            let (hit_endpoint_dist_sq, candidate_hurt_param) =
                source_point_segment_distance_sq(hurt_start, hurt_end, hit_endpoint);

            let hurt_endpoint_param = if unconstrained_hurt < 0.0 { 0.0 } else { 1.0 };
            let hurt_endpoint = if hurt_endpoint_param == 0.0 {
                hurt_start
            } else {
                hurt_end
            };
            let (hurt_endpoint_dist_sq, candidate_hit_param) =
                source_point_segment_distance_sq(hit_start, hit_end, hurt_endpoint);

            if hit_endpoint_dist_sq < hurt_endpoint_dist_sq {
                (hit_endpoint_param, candidate_hurt_param)
            } else {
                (candidate_hit_param, hurt_endpoint_param)
            }
        } else {
            (unconstrained_hit, unconstrained_hurt)
        }
    };

    (
        add3(hit_start, mul3(hit_delta, hit_param)),
        add3(hurt_start, mul3(hurt_delta, hurt_param)),
    )
}

fn source_capsule_broadphase_rejects(hit: &Capsule3, hurt: &Capsule3) -> bool {
    // lbColl_8000805C passes 3 * hurt fighter scale. Runtime capsules already
    // include fighter scale, so the remaining source broadphase factor is 3.
    let radius = hit.radius + hurt.radius * 3.0;
    for (hit_start, hit_end, hurt_start, hurt_end) in [
        (hit.a.x, hit.b.x, hurt.a.x, hurt.b.x),
        (hit.a.y, hit.b.y, hurt.a.y, hurt.b.y),
        (hit.a.z, hit.b.z, hurt.a.z, hurt.b.z),
    ] {
        if hit_start > hit_end {
            if hit_start + radius < hurt_start && hit_start + radius < hurt_end {
                return true;
            }
            if hit_end - radius > hurt_start && hit_end - radius > hurt_end {
                return true;
            }
        } else {
            if hit_start - radius > hurt_start && hit_start - radius > hurt_end {
                return true;
            }
            if hit_end + radius < hurt_start && hit_end + radius < hurt_end {
                return true;
            }
        }
    }
    false
}

pub fn capsules_intersect_3d(hit: &Capsule3, hurt: &Capsule3) -> bool {
    let allowed = hit.radius + hurt.radius;
    closest_distance_sq_between_segments(hit.a, hit.b, hurt.a, hurt.b) <= allowed * allowed
}

pub fn capsules_intersect_3d_with_hurt_matrix(
    hit: &Capsule3,
    hurt: &Capsule3,
    hurt_matrix: Option<Mat3x4>,
) -> bool {
    source_capsule_overlap_3d_with_hurt_matrix(hit, hurt, hurt_matrix)
        .is_some_and(|overlap| overlap >= 0.0)
}

pub fn source_capsule_overlap_3d_with_hurt_matrix(
    hit: &Capsule3,
    hurt: &Capsule3,
    hurt_matrix: Option<Mat3x4>,
) -> Option<f32> {
    if source_capsule_broadphase_rejects(hit, hurt) {
        return None;
    }
    let Some(hurt_matrix) = hurt_matrix else {
        let (hit_closest, hurt_closest) =
            source_closest_points_between_segments(hit.a, hit.b, hurt.a, hurt.b);
        let overlap = hit.radius + hurt.radius
            - dot(
                sub3(hit_closest, hurt_closest),
                sub3(hit_closest, hurt_closest),
            )
            .sqrt();
        return (overlap >= 0.0).then_some(overlap);
    };
    let (hit_closest, hurt_closest) =
        source_closest_points_between_segments(hit.a, hit.b, hurt.a, hurt.b);
    let closest_delta = sub3(hit_closest, hurt_closest);
    let closest_dist_sq = dot(closest_delta, closest_delta);
    let closest_dist = closest_dist_sq.sqrt();
    if closest_dist.abs() < 1.0e-5 {
        return Some(hit.radius + hurt.radius);
    }
    let Some(inv_hurt_matrix) = hurt_matrix.inverse_linear() else {
        let overlap = hit.radius + hurt.radius - closest_dist;
        return (overlap >= 0.0).then_some(overlap);
    };
    let local_delta = inv_hurt_matrix.transform_vector(sub3(hit_closest, hurt_closest));
    let local_dist = dot(local_delta, local_delta).sqrt();
    if local_dist <= f32::EPSILON {
        return Some(hit.radius + hurt.radius);
    }
    let scaled_hurt_radius = (hurt.radius * closest_dist) / local_dist;
    let overlap = hit.radius + scaled_hurt_radius - closest_dist;
    (overlap >= 0.0).then_some(overlap)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceHitboxLifecycleId(u64);

pub const SOURCE_HURT_HEIGHT_LOW: u8 = 0;
pub const SOURCE_HURT_HEIGHT_MID: u8 = 1;
pub const SOURCE_HURT_HEIGHT_HIGH: u8 = 2;

impl SourceHitboxLifecycleId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceHitboxFlags {
    bits: u8,
}

impl SourceHitboxFlags {
    const ITEM_HIT_INTERACTION: u8 = 1 << 0;
    const IGNORE_THROWN_FIGHTERS: u8 = 1 << 1;
    const IGNORE_FIGHTER_SCALE: u8 = 1 << 2;
    const CLANK: u8 = 1 << 3;
    const REBOUND: u8 = 1 << 4;
    const SKIP_IF_THROWN_HITBOX_OWNER_ABSENT: u8 = 1 << 5;
    const HIT_GRABBED_VICTIM_ONLY: u8 = 1 << 6;
    const ALL: u8 = Self::ITEM_HIT_INTERACTION
        | Self::IGNORE_THROWN_FIGHTERS
        | Self::IGNORE_FIGHTER_SCALE
        | Self::CLANK
        | Self::REBOUND
        | Self::SKIP_IF_THROWN_HITBOX_OWNER_ABSENT
        | Self::HIT_GRABBED_VICTIM_ONLY;

    pub const fn none() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self {
            bits: bits & Self::ALL,
        }
    }

    pub const fn from_decomp_spawn_hitbox_3(
        item_hit_interaction: bool,
        ignore_thrown_fighters: bool,
        ignore_fighter_scale: bool,
        clank: bool,
        rebound: bool,
    ) -> Self {
        let mut bits = 0;
        if item_hit_interaction {
            bits |= Self::ITEM_HIT_INTERACTION;
        }
        if ignore_thrown_fighters {
            bits |= Self::IGNORE_THROWN_FIGHTERS;
        }
        if ignore_fighter_scale {
            bits |= Self::IGNORE_FIGHTER_SCALE;
        }
        if clank {
            bits |= Self::CLANK;
        }
        if rebound {
            bits |= Self::REBOUND;
        }
        Self { bits }
    }

    pub const fn with_skip_if_thrown_hitbox_owner_absent(mut self, skip: bool) -> Self {
        if skip {
            self.bits |= Self::SKIP_IF_THROWN_HITBOX_OWNER_ABSENT;
        } else {
            self.bits &= !Self::SKIP_IF_THROWN_HITBOX_OWNER_ABSENT;
        }
        self
    }

    pub const fn with_hit_grabbed_victim_only(mut self, enabled: bool) -> Self {
        if enabled {
            self.bits |= Self::HIT_GRABBED_VICTIM_ONLY;
        } else {
            self.bits &= !Self::HIT_GRABBED_VICTIM_ONLY;
        }
        self
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn item_hit_interaction(self) -> bool {
        self.bits & Self::ITEM_HIT_INTERACTION != 0
    }

    pub const fn ignore_thrown_fighters(self) -> bool {
        self.bits & Self::IGNORE_THROWN_FIGHTERS != 0
    }

    pub const fn ignore_fighter_scale(self) -> bool {
        self.bits & Self::IGNORE_FIGHTER_SCALE != 0
    }

    pub const fn clank(self) -> bool {
        self.bits & Self::CLANK != 0
    }

    pub const fn rebound(self) -> bool {
        self.bits & Self::REBOUND != 0
    }

    pub const fn skip_if_thrown_hitbox_owner_absent(self) -> bool {
        self.bits & Self::SKIP_IF_THROWN_HITBOX_OWNER_ABSENT != 0
    }

    pub const fn hit_grabbed_victim_only(self) -> bool {
        self.bits & Self::HIT_GRABBED_VICTIM_ONLY != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceCollisionCapsule {
    pub owner_index: usize,
    pub capsule_id: u64,
    pub capsule: Capsule3,
    pub previous_capsule: Option<Capsule3>,
    pub hurt_matrix: Option<Mat3x4>,
    pub owner_grounded: Option<bool>,
    pub action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_frame: Option<u8>,
    pub hitbox_lifecycle_id: Option<SourceHitboxLifecycleId>,
    pub hitbox: Option<SourceHitboxAttributes>,
    pub hitbox_flags: SourceHitboxFlags,
    pub hurt_height: u8,
}

impl SourceCollisionCapsule {
    pub const fn new(owner_index: usize, capsule_id: u64, capsule: Capsule3) -> Self {
        Self {
            owner_index,
            capsule_id,
            capsule,
            previous_capsule: None,
            hurt_matrix: None,
            owner_grounded: None,
            action_state_id: None,
            source_action_key: None,
            source_frame: None,
            hitbox_lifecycle_id: None,
            hitbox: None,
            hitbox_flags: SourceHitboxFlags::none(),
            hurt_height: SOURCE_HURT_HEIGHT_MID,
        }
    }

    pub const fn with_previous_capsule(mut self, previous_capsule: Capsule3) -> Self {
        self.previous_capsule = Some(previous_capsule);
        self
    }

    pub const fn with_hurt_matrix(mut self, hurt_matrix: Mat3x4) -> Self {
        self.hurt_matrix = Some(hurt_matrix);
        self
    }

    pub const fn with_optional_hurt_matrix(mut self, hurt_matrix: Option<Mat3x4>) -> Self {
        self.hurt_matrix = hurt_matrix;
        self
    }

    pub const fn with_owner_grounded(mut self, grounded: bool) -> Self {
        self.owner_grounded = Some(grounded);
        self
    }

    pub const fn with_source_pose(
        mut self,
        action_state_id: Option<MeleeActionStateId>,
        source_action_key: Option<SourceActionKey>,
        source_frame: u8,
    ) -> Self {
        self.action_state_id = action_state_id;
        self.source_action_key = source_action_key;
        self.source_frame = Some(source_frame);
        self
    }

    pub const fn with_hitbox_lifecycle(mut self, lifecycle_id: SourceHitboxLifecycleId) -> Self {
        self.hitbox_lifecycle_id = Some(lifecycle_id);
        self
    }

    pub const fn with_optional_hitbox_lifecycle(
        mut self,
        lifecycle_id: Option<SourceHitboxLifecycleId>,
    ) -> Self {
        self.hitbox_lifecycle_id = lifecycle_id;
        self
    }

    pub const fn with_hitbox_attributes(mut self, hitbox: SourceHitboxAttributes) -> Self {
        self.hitbox = Some(hitbox);
        self
    }

    pub const fn with_optional_hitbox_attributes(
        mut self,
        hitbox: Option<SourceHitboxAttributes>,
    ) -> Self {
        self.hitbox = hitbox;
        self
    }

    pub const fn with_hitbox_flags(mut self, hitbox_flags: SourceHitboxFlags) -> Self {
        self.hitbox_flags = hitbox_flags;
        self
    }

    pub const fn with_hurt_height(mut self, hurt_height: u8) -> Self {
        self.hurt_height = hurt_height;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceHitboxAttributes {
    pub bone: u16,
    pub hit_group: u8,
    pub damage: u16,
    pub angle: u16,
    pub knockback_growth: u16,
    pub weight_set_knockback: u16,
    pub base_knockback: u16,
    pub element: u8,
    pub shield_damage: i16,
    pub hit_grounded: bool,
    pub hit_aerial: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceThrowHitboxAttributes {
    pub hitbox_idx: u8,
    pub damage: u32,
    pub angle: u16,
    pub hit_x24: u16,
    pub hit_x28: u16,
    pub hit_x2c: u16,
    pub element: u8,
    pub sfx_severity: u8,
    pub sfx_kind: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceInstalledThrowHitbox {
    pub hitbox: SourceThrowHitboxAttributes,
    pub damage: f32,
    pub unk_count: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceCollisionFrame {
    pub hits: Vec<SourceCollisionCapsule>,
    pub hurts: Vec<SourceCollisionCapsule>,
}

impl SourceCollisionFrame {
    pub fn empty() -> Self {
        Self {
            hits: Vec::new(),
            hurts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceCollisionHit {
    pub hit: SourceCollisionCapsule,
    pub hurt: SourceCollisionCapsule,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceHitConfirm {
    pub attacker_index: usize,
    pub victim_index: usize,
    pub hitbox_id: u64,
    pub hurtbox_id: u64,
    pub action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_frame: Option<u8>,
    pub damaged_hurt_height: u8,
    pub hitbox: SourceHitboxAttributes,
    pub collision: SourceCollisionHit,
}

pub const SOURCE_HIT_ELEMENT_CATCH: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceGrabConfirm {
    pub grabber_index: usize,
    pub victim_index: usize,
    pub hitbox_id: u64,
    pub hurtbox_id: u64,
    pub action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_frame: Option<u8>,
    pub hitbox: SourceHitboxAttributes,
    pub collision: SourceCollisionHit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceDamageStage {
    pub attacker_index: usize,
    pub victim_index: usize,
    pub hitbox_id: u64,
    pub hurtbox_id: u64,
    pub action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_frame: Option<u8>,
    pub damaged_hurt_height: u8,
    pub damage: f32,
    pub env_damage: u16,
    pub unk_count: u16,
    pub hitbox: SourceHitboxAttributes,
}

pub const SOURCE_SHIELD_HURTBOX_ID: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceDamageAccumulator {
    pub victim_index: usize,
    pub percent_temp: f32,
    pub applied_damage: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceKnockbackInput {
    pub victim_percent: f32,
    pub victim_percent_temp: f32,
    pub unk_count: u16,
    pub stage: f32,
    pub attack: f32,
    pub defense: f32,
    pub victim_weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceDamageResultInput {
    pub victim_index: usize,
    pub victim_percent: f32,
    pub victim_percent_temp: f32,
    pub victim_weight: f32,
    pub stage: f32,
    pub attack: f32,
    pub defense: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceDamageResult {
    pub stage: SourceDamageStage,
    pub knockback: f32,
    pub angle: u16,
    pub element: u8,
}

pub fn source_collision_hits(frame: &SourceCollisionFrame) -> Vec<SourceCollisionHit> {
    let mut collisions = Vec::new();
    for hit in &frame.hits {
        for hurt in &frame.hurts {
            if hit.owner_index == hurt.owner_index {
                continue;
            }
            let pair_hurt = source_pair_hurt_capsule(*hit, *hurt);
            if capsules_intersect_3d_with_hurt_matrix(
                &hit.capsule,
                &pair_hurt.capsule,
                pair_hurt.hurt_matrix,
            ) {
                collisions.push(SourceCollisionHit {
                    hit: *hit,
                    hurt: pair_hurt,
                });
            }
        }
    }
    collisions
}

pub fn source_hit_confirms(frame: &SourceCollisionFrame) -> Vec<SourceHitConfirm> {
    let mut confirms: Vec<SourceHitConfirm> = Vec::new();
    for hit in &frame.hits {
        let Some(hitbox) = hit.hitbox else {
            continue;
        };
        if source_hitbox_is_catch(hitbox) {
            continue;
        }
        let mut confirmed_victims = Vec::new();
        for hurt in &frame.hurts {
            if hit.owner_index == hurt.owner_index {
                continue;
            }
            let pair_hurt = source_pair_hurt_capsule(*hit, *hurt);
            let hurt_targets_shield = hurt.capsule_id == SOURCE_SHIELD_HURTBOX_ID;
            if confirmed_victims.contains(&hurt.owner_index) && !hurt_targets_shield {
                continue;
            }
            if !source_hitbox_can_hit_hurtbox(hitbox, pair_hurt) {
                continue;
            }
            if capsules_intersect_3d_with_hurt_matrix(
                &hit.capsule,
                &pair_hurt.capsule,
                pair_hurt.hurt_matrix,
            ) {
                let confirm = SourceHitConfirm {
                    attacker_index: hit.owner_index,
                    victim_index: hurt.owner_index,
                    hitbox_id: hit.capsule_id,
                    hurtbox_id: hurt.capsule_id,
                    action_state_id: hit.action_state_id,
                    source_action_key: hit.source_action_key,
                    source_frame: hit.source_frame,
                    damaged_hurt_height: hurt.hurt_height,
                    hitbox,
                    collision: SourceCollisionHit {
                        hit: *hit,
                        hurt: pair_hurt,
                    },
                };
                if hurt_targets_shield {
                    if let Some(existing) = confirms.iter_mut().find(|existing| {
                        existing.attacker_index == confirm.attacker_index
                            && existing.victim_index == confirm.victim_index
                            && existing.hitbox_id == confirm.hitbox_id
                    }) {
                        *existing = confirm;
                    } else {
                        confirms.push(confirm);
                    }
                    if !confirmed_victims.contains(&hurt.owner_index) {
                        confirmed_victims.push(hurt.owner_index);
                    }
                } else {
                    confirms.push(confirm);
                    confirmed_victims.push(hurt.owner_index);
                }
            }
        }
    }
    confirms
}

pub fn source_grab_confirms(frame: &SourceCollisionFrame) -> Vec<SourceGrabConfirm> {
    let mut confirms = Vec::new();
    for hit in &frame.hits {
        let Some(hitbox) = hit.hitbox else {
            continue;
        };
        if !source_hitbox_is_catch(hitbox) {
            continue;
        }
        let mut confirmed_victims = Vec::new();
        for hurt in &frame.hurts {
            if hit.owner_index == hurt.owner_index {
                continue;
            }
            if confirmed_victims.contains(&hurt.owner_index) {
                continue;
            }
            if !source_hitbox_can_hit_hurtbox(hitbox, *hurt) {
                continue;
            }
            if capsules_intersect_3d(&hit.capsule, &hurt.capsule) {
                confirms.push(SourceGrabConfirm {
                    grabber_index: hit.owner_index,
                    victim_index: hurt.owner_index,
                    hitbox_id: hit.capsule_id,
                    hurtbox_id: hurt.capsule_id,
                    action_state_id: hit.action_state_id,
                    source_action_key: hit.source_action_key,
                    source_frame: hit.source_frame,
                    hitbox,
                    collision: SourceCollisionHit {
                        hit: *hit,
                        hurt: *hurt,
                    },
                });
                confirmed_victims.push(hurt.owner_index);
            }
        }
    }
    confirms
}

fn source_pair_hurt_capsule(
    hit: SourceCollisionCapsule,
    hurt: SourceCollisionCapsule,
) -> SourceCollisionCapsule {
    let hurt_is_in_damage_motion = hurt
        .action_state_id
        .is_some_and(is_source_damage_action_state_id);
    if hit.owner_index < hurt.owner_index
        && source_hitbox_lifecycle_starts_on_source_frame(hit)
        && !hurt_is_in_damage_motion
    {
        if let Some(previous_capsule) = hurt.previous_capsule {
            return SourceCollisionCapsule {
                capsule: previous_capsule,
                ..hurt
            };
        }
    }
    hurt
}

fn source_hitbox_lifecycle_starts_on_source_frame(hit: SourceCollisionCapsule) -> bool {
    let (Some(lifecycle_id), Some(source_frame)) = (hit.hitbox_lifecycle_id, hit.source_frame)
    else {
        return false;
    };
    (lifecycle_id.get() >> 32) == u64::from(source_frame)
}

pub fn source_env_damage(damage: f32) -> u16 {
    if damage == 0.0 {
        return 0;
    }
    let int_damage = damage as u16;
    if int_damage == 0 {
        1
    } else {
        int_damage
    }
}

pub fn source_damage_stages_from_confirms(confirms: &[SourceHitConfirm]) -> Vec<SourceDamageStage> {
    confirms
        .iter()
        .map(|confirm| {
            let damage = confirm.hitbox.damage as f32;
            SourceDamageStage {
                attacker_index: confirm.attacker_index,
                victim_index: confirm.victim_index,
                hitbox_id: confirm.hitbox_id,
                hurtbox_id: confirm.hurtbox_id,
                action_state_id: confirm.action_state_id,
                source_action_key: confirm.source_action_key,
                source_frame: confirm.source_frame,
                damaged_hurt_height: confirm.damaged_hurt_height,
                damage,
                env_damage: source_env_damage(damage),
                unk_count: damage as u16,
                hitbox: confirm.hitbox,
            }
        })
        .collect()
}

pub fn source_damage_accumulator_after_stages(
    mut accumulator: SourceDamageAccumulator,
    stages: &[SourceDamageStage],
) -> SourceDamageAccumulator {
    for stage in stages {
        if stage.victim_index != accumulator.victim_index {
            continue;
        }
        accumulator.percent_temp += stage.damage;
        if stage.env_damage > accumulator.applied_damage {
            accumulator.applied_damage = stage.env_damage;
        }
    }
    accumulator
}

pub fn source_knockback(
    common_data: MeleeCommonData,
    hitbox: SourceHitboxAttributes,
    input: SourceKnockbackInput,
) -> f32 {
    let weight = input.victim_weight * common_data.knockback_weight_multiplier;
    let mut decay = common_data.knockback_decay;
    let weight_decay = (weight * decay) / (1.0 + weight);
    decay -= weight_decay;

    let mut result = if hitbox.weight_set_knockback != 0 {
        let weight_set =
            common_data.knockback_weight_set_damage * hitbox.weight_set_knockback as f32;
        let scaled = common_data.knockback_weight_set_damage * common_data.knockback_damage_scale
            + common_data.knockback_hit_count_scale * weight_set;
        decay * scaled
    } else {
        let count = input.victim_percent as i32;
        let damage = count as f32 + input.victim_percent_temp;
        let hit_count_damage = input.unk_count as f32 * damage;
        let scaled = common_data.knockback_damage_scale * damage
            + common_data.knockback_hit_count_scale * hit_count_damage;
        decay * scaled
    };

    result = common_data.knockback_result_scale * result + common_data.knockback_result_offset;
    result = (0.01 * hitbox.knockback_growth as f32) * result + hitbox.base_knockback as f32;
    result *= input.stage;
    result *= input.attack;
    result *= input.defense;

    if result >= common_data.knockback_cap {
        common_data.knockback_cap
    } else {
        result
    }
}

pub fn source_damage_result_for_victim(
    common_data: MeleeCommonData,
    stages: &[SourceDamageStage],
    input: SourceDamageResultInput,
) -> Option<SourceDamageResult> {
    let mut best: Option<SourceDamageResult> = None;
    for stage in stages {
        if stage.victim_index != input.victim_index {
            continue;
        }
        if stage.damage <= 0.0 && stage.env_damage == 0 {
            continue;
        }
        let knockback = source_knockback(
            common_data,
            stage.hitbox,
            SourceKnockbackInput {
                victim_percent: input.victim_percent,
                victim_percent_temp: input.victim_percent_temp,
                unk_count: stage.unk_count,
                stage: input.stage,
                attack: input.attack,
                defense: input.defense,
                victim_weight: input.victim_weight,
            },
        );
        let result = SourceDamageResult {
            stage: *stage,
            knockback,
            angle: stage.hitbox.angle,
            element: stage.hitbox.element,
        };
        if best
            .as_ref()
            .is_none_or(|best_result| knockback > best_result.knockback)
        {
            best = Some(result);
        }
    }
    best
}

fn source_hitbox_can_hit_hurtbox(
    hitbox: SourceHitboxAttributes,
    hurtbox: SourceCollisionCapsule,
) -> bool {
    match hurtbox.owner_grounded {
        Some(true) => hitbox.hit_grounded,
        Some(false) => hitbox.hit_aerial,
        None => hitbox.hit_grounded || hitbox.hit_aerial,
    }
}

fn source_hitbox_is_catch(hitbox: SourceHitboxAttributes) -> bool {
    hitbox.element == SOURCE_HIT_ELEMENT_CATCH
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterEcb {
    pub top: Vec2,
    pub right: Vec2,
    pub bottom: Vec2,
    pub left: Vec2,
}

impl FighterEcb {
    /// Decomp `ftECB` bootstrap used by `mpColl_SetECBSource_JObj` and
    /// `mpColl_SetECBSource_Fixed` when `CollData.x34_flags.b0` is set.
    pub const SOURCE_DEFAULT: Self = Self {
        top: Vec2 { x: 0, y: 8_000 },
        right: Vec2 { x: 4_000, y: 4_000 },
        bottom: Vec2 { x: 0, y: 0 },
        left: Vec2 {
            x: -4_000,
            y: 4_000,
        },
    };

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

    pub const fn debug_root_cross(self, root: Vec2, radius: i32) -> [Vec2; 4] {
        [
            Vec2 {
                x: root.x - radius,
                y: root.y,
            },
            Vec2 {
                x: root.x + radius,
                y: root.y,
            },
            Vec2 {
                x: root.x,
                y: root.y - radius,
            },
            Vec2 {
                x: root.x,
                y: root.y + radius,
            },
        ]
    }
}

pub type EcbDiamond = FighterEcb;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageLandingContact {
    pub surface: StageSurface,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceLedgeGrabContact {
    pub ledge: StageLedge,
    pub side: StageLedgeSide,
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

pub fn floor_friction_multiplier_for_bottom(stage: StageProfile, bottom: Vec2) -> f32 {
    floor_surface_for_bottom(stage, bottom)
        .map(|surface| surface.friction_multiplier)
        .unwrap_or(1.0)
}

pub fn source_ledge_grab_contact(
    stage: StageProfile,
    previous_root_position: Vec2,
    current_root_position: Vec2,
    current_ecb: EcbDiamond,
    ledge_snap_x_milli: i32,
    ledge_snap_y_milli: i32,
    ledge_snap_height_milli: i32,
) -> Option<SourceLedgeGrabContact> {
    source_ledge_grab_contact_for_facing(
        stage,
        previous_root_position,
        current_root_position,
        current_ecb,
        ledge_snap_x_milli,
        ledge_snap_y_milli,
        ledge_snap_height_milli,
        0,
    )
}

pub fn source_ledge_grab_contact_for_facing(
    stage: StageProfile,
    previous_root_position: Vec2,
    current_root_position: Vec2,
    current_ecb: EcbDiamond,
    ledge_snap_x_milli: i32,
    ledge_snap_y_milli: i32,
    ledge_snap_height_milli: i32,
    facing: i8,
) -> Option<SourceLedgeGrabContact> {
    if facing >= 0 {
        if let Some(contact) = source_ledge_grab_contact_on_side(
            stage,
            previous_root_position,
            current_root_position,
            current_ecb,
            ledge_snap_x_milli,
            ledge_snap_y_milli,
            ledge_snap_height_milli,
            StageLedgeSide::Left,
        ) {
            return Some(contact);
        }
    }
    if facing <= 0 {
        return source_ledge_grab_contact_on_side(
            stage,
            previous_root_position,
            current_root_position,
            current_ecb,
            ledge_snap_x_milli,
            ledge_snap_y_milli,
            ledge_snap_height_milli,
            StageLedgeSide::Right,
        );
    }
    None
}

fn source_ledge_grab_contact_on_side(
    stage: StageProfile,
    previous_root_position: Vec2,
    current_root_position: Vec2,
    current_ecb: EcbDiamond,
    ledge_snap_x_milli: i32,
    ledge_snap_y_milli: i32,
    ledge_snap_height_milli: i32,
    side: StageLedgeSide,
) -> Option<SourceLedgeGrabContact> {
    let Some(melee_stage) = stage.melee_stage_profile() else {
        return None;
    };
    let half_height = ledge_snap_height_milli / 2;
    let local_left_x = current_ecb.left.x - current_root_position.x;
    let local_right_x = current_ecb.right.x - current_root_position.x;

    let (left, right) = match side {
        StageLedgeSide::Left => {
            if previous_root_position.x < current_root_position.x {
                (
                    previous_root_position.x,
                    ledge_snap_x_milli + current_root_position.x + local_right_x,
                )
            } else {
                (
                    current_root_position.x,
                    ledge_snap_x_milli + previous_root_position.x + local_right_x,
                )
            }
        }
        StageLedgeSide::Right => {
            let snap_x = -ledge_snap_x_milli;
            if previous_root_position.x > current_root_position.x {
                (
                    snap_x + current_root_position.x + local_left_x,
                    previous_root_position.x,
                )
            } else {
                (
                    snap_x + previous_root_position.x + local_left_x,
                    current_root_position.x,
                )
            }
        }
    };
    let (bounds_bottom, bounds_top) = if previous_root_position.y < current_root_position.y {
        (
            previous_root_position.y + ledge_snap_y_milli - half_height,
            current_root_position.y + ledge_snap_y_milli + half_height,
        )
    } else {
        (
            current_root_position.y + ledge_snap_y_milli - half_height,
            previous_root_position.y + ledge_snap_y_milli + half_height,
        )
    };

    let mut best: Option<(StageLedge, Vec2)> = None;
    for ledge in stage
        .ledges
        .iter()
        .copied()
        .filter(|ledge| ledge.side == side)
    {
        let Some((line_array_index, source_line)) = melee_stage
            .collision
            .lines
            .iter()
            .copied()
            .enumerate()
            .find(|(_, line)| line.index == ledge.line_index)
        else {
            continue;
        };
        if source_line.kind != StageCollisionLineKind::Floor
            || source_line.passable
            || !source_line.has_ledge_flag()
        {
            continue;
        }
        let Some(line) = melee_stage.collision.scaled_line(line_array_index) else {
            continue;
        };
        if !source_ledge_line_bounds_overlap(
            line.x0_milli,
            line.y0_milli,
            line.x1_milli,
            line.y1_milli,
            left,
            bounds_bottom,
            right,
            bounds_top,
        ) {
            continue;
        }
        let edge = Vec2 {
            x: ledge.x_milli,
            y: ledge.y_milli,
        };
        let replace = match (side, best) {
            (_, None) => true,
            (StageLedgeSide::Left, Some((_, current_edge))) => edge.x < current_edge.x,
            (StageLedgeSide::Right, Some((_, current_edge))) => edge.x > current_edge.x,
        };
        if replace {
            best = Some((ledge, edge));
        }
    }

    let (ledge, edge) = best?;
    if !source_ledge_grab_reaches_collision_edge(current_root_position, current_ecb, edge, side) {
        return None;
    }

    Some(SourceLedgeGrabContact { ledge, side })
}

fn source_ledge_line_bounds_overlap(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    left: i32,
    bottom: i32,
    right: i32,
    top: i32,
) -> bool {
    let line_left = x0.min(x1);
    let line_right = x0.max(x1);
    let line_bottom = y0.min(y1);
    let line_top = y0.max(y1);
    (line_right + line_left - (right + left)).abs() < (line_right - line_left) + (right - left)
        && (line_top + line_bottom - (top + bottom)).abs()
            < (line_top - line_bottom) + (top - bottom)
}

fn source_ledge_grab_reaches_collision_edge(
    current_root_position: Vec2,
    current_ecb: EcbDiamond,
    edge: Vec2,
    side: StageLedgeSide,
) -> bool {
    match side {
        StageLedgeSide::Left => {
            edge.x - current_root_position.x < 5_000
                && current_ecb.bottom.x < edge.x
                && current_ecb.bottom.y < edge.y
        }
        StageLedgeSide::Right => {
            current_root_position.x - edge.x < 5_000
                && current_ecb.bottom.x > edge.x
                && current_ecb.bottom.y < edge.y
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hurt_matrix_scales_radius_like_lb_coll_80006e58() {
        let hit = Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0), 1.0);
        let hurt = Capsule3::new(Vec3::new(4.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), 1.0);
        let hurt_matrix = Mat3x4::from_rows([
            [4.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]);

        assert!(!capsules_intersect_3d(&hit, &hurt));
        assert!(capsules_intersect_3d_with_hurt_matrix(
            &hit,
            &hurt,
            Some(hurt_matrix),
        ));
    }

    #[test]
    fn lb_coll_80006e58_frame3268_nair_slot_zero_remains_a_real_overlap() {
        let hit = Capsule3::new(
            Vec3::new(-14982.361, 18751.096, -7751.8584),
            Vec3::new(-7983.7065, 22236.332, -2287.4084),
            4296.875,
        );
        let hurt = Capsule3::new(
            Vec3::new(-10431.543, 18021.605, 2022.5189),
            Vec3::new(-11773.329, 18342.662, 269.26648),
            1440.0,
        );
        let hurt_matrix = Mat3x4::from_rows([
            [0.10491562, -0.5906068, -0.7622837, 0.0],
            [-0.89780337, -0.33965224, 0.13959011, 0.0],
            [-0.35191157, 0.6904491, -0.5833852, 0.0],
        ]);

        let overlap = source_capsule_overlap_3d_with_hurt_matrix(&hit, &hurt, Some(hurt_matrix))
            .expect("the decomp segment solver still intersects this pair");
        assert!((overlap - 349.5957).abs() < 0.01, "overlap={overlap}");
    }
}
