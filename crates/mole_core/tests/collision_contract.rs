use mole_core::collision::{capsules_intersect_3d, Capsule3, Mat3x4, Vec3};
use mole_core::collision::{
    source_collision_hits, source_damage_accumulator_after_stages, source_damage_result_for_victim,
    source_damage_stages_from_confirms, source_env_damage, source_hit_confirms, source_knockback,
    SourceCollisionCapsule, SourceCollisionFrame, SourceCollisionHit, SourceDamageAccumulator,
    SourceDamageResultInput, SourceHitboxAttributes, SourceHitboxLifecycleId, SourceKnockbackInput,
    SOURCE_SHIELD_HURTBOX_ID,
};
use mole_core::{MeleeActionStateId, MeleeCommonData, SourceActionKey};

#[test]
fn capsule_collision_preserves_z_axis_separation() {
    let hit = Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.5);
    let hurt = Capsule3::new(Vec3::new(1.0, 0.0, 1.25), Vec3::new(1.0, 2.0, 1.25), 0.5);

    assert!(!capsules_intersect_3d(&hit, &hurt));
}

#[test]
fn capsule_collision_hits_when_3d_distance_is_inside_combined_radius() {
    let hit = Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75);
    let hurt = Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75);

    assert!(capsules_intersect_3d(&hit, &hurt));
}

#[test]
fn mat3x4_transform_point_keeps_source_z() {
    let matrix = Mat3x4::from_rows([
        [1.0, 0.0, 0.0, 10.0],
        [0.0, 1.0, 0.0, 20.0],
        [0.0, 0.0, 1.0, 30.0],
    ]);

    let point = matrix.transform_point(Vec3::new(1.5, 2.5, 3.5));

    assert_eq!(point, Vec3::new(11.5, 22.5, 33.5));
}

#[test]
fn source_collision_detects_enemy_hits_and_ignores_owner_self_hits() {
    let p1_hit = SourceCollisionCapsule::new(
        0,
        7,
        Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
    );
    let p1_hurt = SourceCollisionCapsule::new(
        0,
        3,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 0.0), 0.75),
    );
    let p2_hurt = SourceCollisionCapsule::new(
        1,
        5,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
    );
    let frame = SourceCollisionFrame {
        hits: vec![p1_hit],
        hurts: vec![p1_hurt, p2_hurt],
    };

    assert_eq!(
        source_collision_hits(&frame),
        vec![SourceCollisionHit {
            hit: SourceCollisionCapsule::new(
                0,
                7,
                Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
            ),
            hurt: SourceCollisionCapsule::new(
                1,
                5,
                Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
            ),
        }]
    );
}

#[test]
fn source_hit_confirms_carry_hitbox_attributes_and_respect_target_air_ground_flags() {
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 6,
        angle: 82,
        knockback_growth: 100,
        weight_set_knockback: 40,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: false,
        hit_aerial: true,
    };
    let p1_hit = SourceCollisionCapsule::new(
        0,
        0,
        Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
    )
    .with_hitbox_attributes(hitbox)
    .with_owner_grounded(false);
    let p2_grounded_hurt = SourceCollisionCapsule::new(
        1,
        5,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
    )
    .with_owner_grounded(true);
    let p2_airborne_hurt = SourceCollisionCapsule::new(
        1,
        6,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
    )
    .with_owner_grounded(false);

    let confirms = source_hit_confirms(&SourceCollisionFrame {
        hits: vec![p1_hit],
        hurts: vec![p2_grounded_hurt, p2_airborne_hurt],
    });

    assert_eq!(confirms.len(), 1);
    assert_eq!(confirms[0].attacker_index, 0);
    assert_eq!(confirms[0].victim_index, 1);
    assert_eq!(confirms[0].hitbox_id, 0);
    assert_eq!(confirms[0].hurtbox_id, 6);
    assert_eq!(confirms[0].hitbox, hitbox);
}

#[test]
fn source_hit_confirms_stop_at_first_hurtbox_per_hitbox_and_victim_like_decomp() {
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 5,
        angle: 78,
        knockback_growth: 100,
        weight_set_knockback: 40,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let hit = SourceCollisionCapsule::new(
        0,
        1,
        Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
    )
    .with_hitbox_attributes(hitbox)
    .with_owner_grounded(false);
    let first_hurtbox = SourceCollisionCapsule::new(
        1,
        10,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
    )
    .with_owner_grounded(false);
    let second_hurtbox = SourceCollisionCapsule::new(
        1,
        11,
        Capsule3::new(Vec3::new(1.1, 0.0, 0.8), Vec3::new(1.1, 2.0, 0.8), 0.75),
    )
    .with_owner_grounded(false);

    let confirms = source_hit_confirms(&SourceCollisionFrame {
        hits: vec![hit],
        hurts: vec![first_hurtbox, second_hurtbox],
    });

    assert_eq!(confirms.len(), 1);
    assert_eq!(confirms[0].hitbox_id, 1);
    assert_eq!(confirms[0].hurtbox_id, 10);
}

#[test]
fn source_hit_confirms_prioritize_guard_shield_over_body_hurtboxes() {
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 5,
        angle: 78,
        knockback_growth: 100,
        weight_set_knockback: 40,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let hit = SourceCollisionCapsule::new(
        0,
        1,
        Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
    )
    .with_hitbox_attributes(hitbox)
    .with_owner_grounded(false);
    let body_hurtbox = SourceCollisionCapsule::new(
        1,
        10,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
    )
    .with_owner_grounded(true);
    let shield_hurtbox = SourceCollisionCapsule::new(
        1,
        SOURCE_SHIELD_HURTBOX_ID,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 2.0),
    )
    .with_owner_grounded(true);

    let confirms = source_hit_confirms(&SourceCollisionFrame {
        hits: vec![hit],
        hurts: vec![body_hurtbox, shield_hurtbox],
    });

    assert_eq!(confirms.len(), 1);
    assert_eq!(confirms[0].hitbox_id, 1);
    assert_eq!(confirms[0].hurtbox_id, SOURCE_SHIELD_HURTBOX_ID);
}

#[test]
fn source_new_hitbox_does_not_sweep_against_damage_victim_predamage_root() {
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 1,
        angle: 361,
        knockback_growth: 20,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(47)),
            Some(SourceActionKey::new("Attack100Start")),
            4,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(4_u64 << 32))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(20.0, 0.0, 0.0), Vec3::new(21.0, 0.0, 0.0), 1.0),
        )
        .with_previous_capsule(Capsule3::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
        ))
        .with_owner_grounded(false)
        .with_source_pose(
            Some(MeleeActionStateId::new(79)),
            Some(SourceActionKey::new("DamageHi1")),
            1,
        )],
    };

    assert!(
        source_hit_confirms(&frame).is_empty(),
        "after ftColl damage routing enters a damage motion, rapid-jab hitboxes must collide against the victim's current damage root rather than stale pre-damage root"
    );
}

#[test]
fn source_damage_staging_matches_decomp_percent_temp_and_applied_damage() {
    assert_eq!(source_env_damage(0.0), 0);
    assert_eq!(source_env_damage(0.5), 1);
    assert_eq!(source_env_damage(5.75), 5);

    let strong_hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 5,
        angle: 78,
        knockback_growth: 100,
        weight_set_knockback: 40,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let weak_hitbox = SourceHitboxAttributes {
        damage: 3,
        ..strong_hitbox
    };
    let strong_hit = SourceCollisionCapsule::new(
        0,
        1,
        Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
    )
    .with_hitbox_attributes(strong_hitbox)
    .with_owner_grounded(false);
    let weak_hit = SourceCollisionCapsule::new(
        0,
        2,
        Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
    )
    .with_hitbox_attributes(weak_hitbox)
    .with_owner_grounded(false);
    let hurt = SourceCollisionCapsule::new(
        1,
        10,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
    )
    .with_owner_grounded(false);

    let confirms = source_hit_confirms(&SourceCollisionFrame {
        hits: vec![strong_hit, weak_hit],
        hurts: vec![hurt],
    });
    let stages = source_damage_stages_from_confirms(&confirms);
    let accumulated = source_damage_accumulator_after_stages(
        SourceDamageAccumulator {
            victim_index: 1,
            percent_temp: 2.0,
            applied_damage: 4,
        },
        &stages,
    );

    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].attacker_index, 0);
    assert_eq!(stages[0].victim_index, 1);
    assert_eq!(stages[0].hitbox_id, 1);
    assert_eq!(stages[0].damage, 5.0);
    assert_eq!(stages[0].env_damage, 5);
    assert_eq!(stages[1].hitbox_id, 2);
    assert_eq!(stages[1].damage, 3.0);
    assert_eq!(stages[1].env_damage, 3);
    assert_eq!(
        accumulated,
        SourceDamageAccumulator {
            victim_index: 1,
            percent_temp: 10.0,
            applied_damage: 5,
        }
    );
}

#[test]
fn source_knockback_matches_ftcoll_80079ab0_normal_and_weight_set_paths() {
    let common = MeleeCommonData {
        knockback_weight_multiplier: 0.01,
        knockback_decay: 2.0,
        knockback_cap: 2500.0,
        knockback_damage_scale: 0.1,
        knockback_hit_count_scale: 0.05,
        knockback_weight_set_damage: 10.0,
        knockback_result_scale: 1.4,
        knockback_result_offset: 18.0,
        ..MeleeCommonData::provisional_mole()
    };
    let normal_hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 6,
        angle: 82,
        knockback_growth: 100,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let input = SourceKnockbackInput {
        victim_percent: 50.0,
        victim_percent_temp: 5.0,
        unk_count: 6,
        stage: 1.0,
        attack: 1.0,
        defense: 1.0,
        victim_weight: 104.0,
    };

    let normal = source_knockback(common, normal_hitbox, input);
    let weight_set = source_knockback(
        common,
        SourceHitboxAttributes {
            weight_set_knockback: 40,
            ..normal_hitbox
        },
        SourceKnockbackInput {
            victim_percent: 190.0,
            victim_percent_temp: 20.0,
            unk_count: 12,
            ..input
        },
    );
    let capped = source_knockback(
        MeleeCommonData {
            knockback_cap: 45.0,
            ..common
        },
        normal_hitbox,
        input,
    );

    assert!((normal - 48.19608).abs() < 0.0001);
    assert!((weight_set - 46.82353).abs() < 0.0001);
    assert_eq!(capped, 45.0);
}

#[test]
fn source_damage_result_selects_highest_knockback_entry_like_ftcoll_8007a06c() {
    let common = MeleeCommonData {
        knockback_weight_multiplier: 0.01,
        knockback_decay: 2.0,
        knockback_cap: 2500.0,
        knockback_damage_scale: 0.1,
        knockback_hit_count_scale: 0.05,
        knockback_weight_set_damage: 10.0,
        knockback_result_scale: 1.4,
        knockback_result_offset: 18.0,
        ..MeleeCommonData::provisional_mole()
    };
    let light_hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 3,
        angle: 72,
        knockback_growth: 50,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 1,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let heavy_hitbox = SourceHitboxAttributes {
        damage: 6,
        angle: 82,
        knockback_growth: 100,
        element: 2,
        ..light_hitbox
    };
    let hurt = SourceCollisionCapsule::new(
        1,
        10,
        Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75),
    )
    .with_owner_grounded(false);
    let confirms = source_hit_confirms(&SourceCollisionFrame {
        hits: vec![
            SourceCollisionCapsule::new(
                0,
                1,
                Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
            )
            .with_hitbox_attributes(light_hitbox)
            .with_owner_grounded(false),
            SourceCollisionCapsule::new(
                0,
                2,
                Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75),
            )
            .with_hitbox_attributes(heavy_hitbox)
            .with_owner_grounded(false),
        ],
        hurts: vec![hurt],
    });
    let stages = source_damage_stages_from_confirms(&confirms);

    let result = source_damage_result_for_victim(
        common,
        &stages,
        SourceDamageResultInput {
            victim_index: 1,
            victim_percent: 50.0,
            victim_percent_temp: 9.0,
            victim_weight: 104.0,
            stage: 1.0,
            attack: 1.0,
            defense: 1.0,
        },
    )
    .expect("same-victim damage result should be selected");

    assert_eq!(stages[0].unk_count, 3);
    assert_eq!(stages[1].unk_count, 6);
    assert_eq!(result.stage.hitbox_id, 2);
    assert_eq!(result.stage.damage, 6.0);
    assert_eq!(result.angle, 82);
    assert_eq!(result.element, 2);
    assert!((result.knockback - 50.39216).abs() < 0.0001);
}
