use crate::{
    common_data::MeleeCommonData,
    stage::{StageProfile, StageSurface, StageSurfaceKind},
    state::{MeleeActionStateId, SourceActionKey, Vec2},
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
pub struct SourceHitboxLifecycleId(u64);

impl SourceHitboxLifecycleId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceCollisionCapsule {
    pub owner_index: usize,
    pub capsule_id: u64,
    pub capsule: Capsule3,
    pub owner_grounded: Option<bool>,
    pub action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_frame: Option<u8>,
    pub hitbox_lifecycle_id: Option<SourceHitboxLifecycleId>,
    pub hitbox: Option<SourceHitboxAttributes>,
}

impl SourceCollisionCapsule {
    pub const fn new(owner_index: usize, capsule_id: u64, capsule: Capsule3) -> Self {
        Self {
            owner_index,
            capsule_id,
            capsule,
            owner_grounded: None,
            action_state_id: None,
            source_action_key: None,
            source_frame: None,
            hitbox_lifecycle_id: None,
            hitbox: None,
        }
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
    pub damage: f32,
    pub env_damage: u16,
    pub unk_count: u16,
    pub hitbox: SourceHitboxAttributes,
}

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
            if capsules_intersect_3d(&hit.capsule, &hurt.capsule) {
                collisions.push(SourceCollisionHit {
                    hit: *hit,
                    hurt: *hurt,
                });
            }
        }
    }
    collisions
}

pub fn source_hit_confirms(frame: &SourceCollisionFrame) -> Vec<SourceHitConfirm> {
    let mut confirms = Vec::new();
    for hit in &frame.hits {
        let Some(hitbox) = hit.hitbox else {
            continue;
        };
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
                confirms.push(SourceHitConfirm {
                    attacker_index: hit.owner_index,
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
