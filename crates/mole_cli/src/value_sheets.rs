use mole_core::{StageProfile, StageSurfaceKind};
use serde::Serialize;
use serde_json::{json, Value};
use std::{fs, path::Path};

use crate::{read_json, SCHEMA_VERSION};

pub(crate) const VALUE_SHEETS_GENERATOR: &str = "crates/mole_cli/src/value_sheets.rs";
pub(crate) const VALUE_SHEETS_COMMAND: &str =
    "cargo run -p mole_cli -- generated write-value-sheets --write --json";
pub(crate) const VALUE_SHEETS_INPUTS: &[&str] = &[
    "resources/melee/extracted/plco_common_data.json",
    "resources/melee/extracted/captain_falcon_profile.json",
    "crates/mole_core/src/stage.rs",
];
pub(crate) const VALUE_SHEET_OUTPUTS: &[&str] = &[
    "docs/state_graphs/value_sheets/global_common_values.json",
    "docs/state_graphs/value_sheets/captain_falcon_values.json",
    "docs/state_graphs/value_sheets/physics_engine_values.json",
    "docs/state_graphs/value_sheets/global_combat_values.json",
    "docs/state_graphs/value_sheets/captain_falcon_combat_values.json",
    "docs/state_graphs/value_sheets/battlefield_stage_values.json",
];

const SOURCE_FLOAT_COMPARISON_FIELDS: &[&str] = &[
    "dash_run_acceleration_a",
    "dash_run_acceleration_b",
    "dash_run_terminal_velocity",
];

const GLOBAL_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "input",
        &[
            "main_stick_deadzone_x",
            "main_stick_deadzone_y",
            "c_stick_deadzone_x",
            "c_stick_deadzone_y",
            "tap_x_threshold",
            "tap_y_threshold",
            "trigger_deadzone",
            "trigger_timer_threshold",
            "z_shield_analog",
        ],
    ),
    (
        "grounded_locomotion",
        &[
            "walk_x",
            "walk_middle_velocity_ratio",
            "walk_fast_velocity_ratio",
            "walk_accel_taper",
            "turn_x",
            "turn_run_x",
            "dash_x",
            "dash_tap_window",
            "dash_early_action_window",
            "dash_defensive_action_window",
            "dash_late_action_window",
            "dash_velocity_decay",
            "run_x",
            "run_accel_taper",
            "run_ground_friction_multiplier",
            "high_speed_ground_friction_multiplier",
            "run_turn_run_no_interrupt_frames",
            "run_brake_animation_pause_velocity",
            "animation_velocity_scale",
        ],
    ),
    (
        "jump_and_air",
        &[
            "tap_jump_y",
            "tap_jump_window",
            "air_jump_backward_x",
            "tap_jump_release_y",
            "fast_fall_y",
            "fast_fall_window",
            "aerial_neutral_x",
            "aerial_neutral_y",
            "aerial_vertical_angle_tan_milli",
            "fall_animation_drift_threshold",
            "fall_animation_blend",
        ],
    ),
    (
        "collision_damage",
        &[
            "knockback_weight_multiplier",
            "knockback_decay",
            "knockback_cap",
            "knockback_damage_scale",
            "knockback_hit_count_scale",
            "knockback_weight_set_damage",
            "knockback_result_scale",
            "knockback_result_offset",
        ],
    ),
    (
        "defense_and_platforms",
        &[
            "crouch_y",
            "crouch_release_y",
            "tilt_x",
            "tilt_y",
            "escape_x",
            "escape_x_tap_window",
            "escape_y",
            "escape_y_tap_window",
            "fallspecial_platform_landing_y",
            "platform_pass_y",
            "platform_pass_y_tap_window",
            "pass_initial_y_velocity",
            "platform_drop_delay_ticks",
            "guard_on_catch_dash_window",
            "guard_reflect_input_window",
        ],
    ),
    (
        "escape_air",
        &[
            "escapeair_deadzone_x",
            "escapeair_deadzone_y",
            "escapeair_iasa_timer_ticks",
            "escapeair_force",
            "escapeair_decay",
            "escapeair_landing_lag_ticks",
        ],
    ),
    (
        "entry_and_respawn",
        &[
            "entry_start_ticks",
            "entry_end_ticks",
            "entry_initial_scale_y",
            "entry_collision_landing_lag_ticks",
        ],
    ),
];

const CHARACTER_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "walk_and_run",
        &[
            "walk_initial_velocity",
            "walk_accel",
            "walk_max_vel",
            "slow_walk_max_velocity",
            "mid_walk_threshold",
            "fast_walk_threshold",
            "ground_friction",
            "dash_run_terminal_velocity",
            "run_animation_scaling",
            "max_run_brake_frames",
            "ground_max_horizontal_velocity",
        ],
    ),
    (
        "dash_and_turn",
        &[
            "dash_initial_velocity",
            "dash_run_acceleration_a",
            "dash_run_acceleration_b",
            "frames_to_change_direction_on_standing_turn",
        ],
    ),
    (
        "jump_and_air",
        &[
            "jump_startup_time",
            "jump_h_initial_velocity",
            "jump_v_initial_velocity",
            "ground_to_air_jump_momentum_multiplier",
            "jump_h_max_velocity",
            "hop_v_initial_velocity",
            "air_jump_v_multiplier",
            "air_jump_h_multiplier",
            "max_jumps",
            "air_drift_stick_mul",
            "aerial_drift_base",
            "air_drift_max",
            "aerial_friction",
            "air_max_horizontal_velocity",
        ],
    ),
    (
        "gravity_and_landing",
        &[
            "grav",
            "terminal_vel",
            "fast_fall_velocity",
            "normal_landing_lag",
            "landingairn_lag",
            "landingairf_lag",
            "landingairb_lag",
            "landingairhi_lag",
            "landingairlw_lag",
        ],
    ),
    (
        "entry_and_collision",
        &["weight", "entry_platform_offset_y"],
    ),
];

#[derive(Clone, Copy)]
enum FieldOwner {
    Global,
    Falcon,
}

#[derive(Clone, Copy)]
struct FieldSpec {
    owner: FieldOwner,
    rust_name: &'static str,
}

const PHYSICS_ENGINE_CATEGORIES: &[(&str, &[FieldSpec])] = &[
    (
        "grounded_movement",
        &[
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "run_accel_taper",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "run_ground_friction_multiplier",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "high_speed_ground_friction_multiplier",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "animation_velocity_scale",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "ground_friction",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "walk_initial_velocity",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "walk_accel",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "walk_max_vel",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "dash_run_terminal_velocity",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "dash_run_acceleration_a",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "dash_run_acceleration_b",
            },
        ],
    ),
    (
        "jump_and_air",
        &[
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "tap_jump_y",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "tap_jump_window",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "fast_fall_y",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "fall_animation_drift_threshold",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "fall_animation_blend",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "aerial_neutral_x",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "aerial_neutral_y",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "grav",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "terminal_vel",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "fast_fall_velocity",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "jump_h_initial_velocity",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "jump_v_initial_velocity",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "ground_to_air_jump_momentum_multiplier",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "jump_h_max_velocity",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "air_drift_stick_mul",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "aerial_drift_base",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "air_drift_max",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "aerial_friction",
            },
            FieldSpec {
                owner: FieldOwner::Falcon,
                rust_name: "air_max_horizontal_velocity",
            },
        ],
    ),
    (
        "platforms_and_escapes",
        &[
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "platform_pass_y",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "pass_initial_y_velocity",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "platform_drop_delay_ticks",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "escapeair_force",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "escapeair_decay",
            },
            FieldSpec {
                owner: FieldOwner::Global,
                rust_name: "escapeair_landing_lag_ticks",
            },
        ],
    ),
];

const GLOBAL_COMBAT_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "knockback",
        &[
            "knockback_weight_multiplier",
            "knockback_decay",
            "knockback_cap",
            "knockback_damage_scale",
            "knockback_hit_count_scale",
            "knockback_weight_set_damage",
            "knockback_result_scale",
            "knockback_result_offset",
            "damage_knockback_velocity_scale",
        ],
    ),
    (
        "damage_motion_thresholds",
        &[
            "damage_landing_basic_knockback_threshold",
            "damage_landing_down_bound_knockback_threshold",
            "damage_motion_tier_1_threshold",
            "damage_motion_tier_2_threshold",
            "damage_motion_tier_3_threshold",
            "down_wait_timer",
        ],
    ),
    (
        "damage_angles",
        &[
            "damage_sakurai_air_angle_radians",
            "damage_sakurai_ground_angle_degrees",
            "damage_sakurai_ground_max_knockback",
            "damage_sakurai_ground_min_knockback",
        ],
    ),
    (
        "hitlag_and_lcancel",
        &[
            "hitlag_base_frames",
            "hitlag_crouch_multiplier",
            "hitlag_damage_scale",
            "hitlag_max_frames",
            "lcancel_divisor",
            "lcancel_window",
        ],
    ),
    (
        "passive_and_recovery",
        &[
            "down_stand_stick_y",
            "passive_input_age_threshold",
            "passive_stand_stick_x",
            "passive_window_max",
        ],
    ),
    (
        "shield_and_defense",
        &[
            "shield_start_health",
            "shield_release_lockout_frames",
            "shield_hold_drain",
            "shield_regen",
            "shield_break_reset_health",
            "shield_hit_drain_damage_scale",
            "shield_hit_drain_base",
            "shield_hit_lightshield_min",
            "shield_hit_lightshield_max",
            "shield_hold_lightshield_min",
            "shield_hold_lightshield_max",
        ],
    ),
    ("damage_response", &["damage_duration_scale"]),
];

const FALCON_COMBAT_CATEGORIES: &[(&str, &[&str])] = &[("combat_attributes", &["weight"])];
const BATTLEFIELD_STAGE_PROVENANCE: &str = "melee_stage_dat_core_projection";

#[derive(Serialize)]
struct ValueSheet {
    id: String,
    title: String,
    scope: String,
    engine_boundary: String,
    source: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    character_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_stage: Option<String>,
    categories: Vec<ValueCategory>,
}

#[derive(Serialize)]
struct ValueCategory {
    id: String,
    label: String,
    fields: Vec<ValueRow>,
}

#[derive(Serialize)]
struct ValueRow {
    rust_name: String,
    source_name: String,
    offset: Option<i64>,
    offset_hex: Option<String>,
    kind: String,
    raw: Value,
    converted_value: Value,
    comparison_value_kind: String,
    provenance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

struct SheetOutput {
    relative_path: &'static str,
    sheet: ValueSheet,
}

struct ExtractedSheetSource<'a> {
    owner_scope: &'static str,
    owner_id: Option<&'static str>,
    source: &'a Value,
    fields: &'a serde_json::Map<String, Value>,
}

pub(crate) fn write_value_sheets_report(root: &Path, write: bool) -> Value {
    match build_sheet_outputs(root) {
        Ok(outputs) => {
            if write {
                if let Err(error) = write_outputs(root, &outputs) {
                    return json!({
                        "schema_version": SCHEMA_VERSION,
                        "command": "generated write-value-sheets",
                        "project_root": root.display().to_string(),
                        "mutated": false,
                        "ok": false,
                        "error": error,
                    });
                }
            }

            let sheets = outputs
                .iter()
                .map(|output| {
                    let field_count = output
                        .sheet
                        .categories
                        .iter()
                        .map(|category| category.fields.len())
                        .sum::<usize>();
                    json!({
                        "path": output.relative_path,
                        "title": output.sheet.title,
                        "scope": output.sheet.scope,
                        "category_count": output.sheet.categories.len(),
                        "field_count": field_count,
                    })
                })
                .collect::<Vec<_>>();
            let written_paths = if write {
                VALUE_SHEET_OUTPUTS
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            json!({
                "schema_version": SCHEMA_VERSION,
                "command": "generated write-value-sheets",
                "project_root": root.display().to_string(),
                "mutated": write,
                "ok": true,
                "write": write,
                "generator": VALUE_SHEETS_GENERATOR,
                "recommended_command": VALUE_SHEETS_COMMAND,
                "inputs": VALUE_SHEETS_INPUTS,
                "outputs": VALUE_SHEET_OUTPUTS,
                "written_paths": written_paths,
                "pending_paths": if write { Vec::<String>::new() } else {
                    VALUE_SHEET_OUTPUTS.iter().map(|path| path.to_string()).collect::<Vec<_>>()
                },
                "sheets": sheets,
            })
        }
        Err(error) => json!({
            "schema_version": SCHEMA_VERSION,
            "command": "generated write-value-sheets",
            "project_root": root.display().to_string(),
            "mutated": false,
            "ok": false,
            "error": error,
        }),
    }
}

fn build_sheet_outputs(root: &Path) -> Result<Vec<SheetOutput>, String> {
    let common_payload = read_json(root.join(VALUE_SHEETS_INPUTS[0])).map_err(|error| {
        format!(
            "failed to read {}: {error}",
            root.join(VALUE_SHEETS_INPUTS[0]).display()
        )
    })?;
    let falcon_payload = read_json(root.join(VALUE_SHEETS_INPUTS[1])).map_err(|error| {
        format!(
            "failed to read {}: {error}",
            root.join(VALUE_SHEETS_INPUTS[1]).display()
        )
    })?;

    let global_source = extracted_source(&common_payload, "global", None)?;
    let falcon_source = extracted_source(&falcon_payload, "character", Some("captain_falcon"))?;

    Ok(vec![
        SheetOutput {
            relative_path: VALUE_SHEET_OUTPUTS[0],
            sheet: build_extracted_sheet(
                "global_common_values",
                "Global Common Values",
                "global",
                global_source.source.clone(),
                None,
                None,
                GLOBAL_CATEGORIES,
                &global_source,
            )?,
        },
        SheetOutput {
            relative_path: VALUE_SHEET_OUTPUTS[1],
            sheet: build_extracted_sheet(
                "captain_falcon_values",
                "Captain Falcon Character Values",
                "character",
                falcon_source.source.clone(),
                Some("captain_falcon"),
                None,
                CHARACTER_CATEGORIES,
                &falcon_source,
            )?,
        },
        SheetOutput {
            relative_path: VALUE_SHEET_OUTPUTS[2],
            sheet: build_mixed_sheet(
                "physics_engine_values",
                "Physics Engine Values",
                "engine_physics",
                json!({
                    "global": global_source.source.clone(),
                    "captain_falcon": falcon_source.source.clone(),
                }),
                None,
                None,
                PHYSICS_ENGINE_CATEGORIES,
                &global_source,
                &falcon_source,
            )?,
        },
        SheetOutput {
            relative_path: VALUE_SHEET_OUTPUTS[3],
            sheet: build_extracted_sheet(
                "global_combat_values",
                "Global Combat Values",
                "global_combat",
                global_source.source.clone(),
                None,
                None,
                GLOBAL_COMBAT_CATEGORIES,
                &global_source,
            )?,
        },
        SheetOutput {
            relative_path: VALUE_SHEET_OUTPUTS[4],
            sheet: build_extracted_sheet(
                "captain_falcon_combat_values",
                "Captain Falcon Combat Values",
                "character_combat",
                falcon_source.source.clone(),
                Some("captain_falcon"),
                None,
                FALCON_COMBAT_CATEGORIES,
                &falcon_source,
            )?,
        },
        SheetOutput {
            relative_path: VALUE_SHEET_OUTPUTS[5],
            sheet: build_battlefield_stage_sheet(),
        },
    ])
}

fn write_outputs(root: &Path, outputs: &[SheetOutput]) -> Result<(), String> {
    for output in outputs {
        let path = root.join(output.relative_path);
        let Some(parent) = path.parent() else {
            return Err(format!("missing parent directory for {}", path.display()));
        };
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        let mut text = serde_json::to_string_pretty(&output.sheet)
            .map_err(|error| format!("failed to serialize {}: {error}", output.relative_path))?;
        text.push('\n');
        fs::write(&path, text)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn build_extracted_sheet(
    id: &str,
    title: &str,
    scope: &str,
    source: Value,
    character_id: Option<&str>,
    reference_stage: Option<&str>,
    categories: &[(&str, &[&str])],
    extracted: &ExtractedSheetSource<'_>,
) -> Result<ValueSheet, String> {
    let categories = categories
        .iter()
        .map(|(category_id, fields)| {
            let fields = fields
                .iter()
                .map(|rust_name| extracted_row(rust_name, extracted))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ValueCategory {
                id: (*category_id).to_string(),
                label: title_case(category_id),
                fields,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(ValueSheet {
        id: id.to_string(),
        title: title.to_string(),
        scope: scope.to_string(),
        engine_boundary: "rust_core_authority".to_string(),
        source,
        character_id: character_id.map(str::to_string),
        stage_id: None,
        reference_stage: reference_stage.map(str::to_string),
        categories,
    })
}

fn build_mixed_sheet(
    id: &str,
    title: &str,
    scope: &str,
    source: Value,
    character_id: Option<&str>,
    reference_stage: Option<&str>,
    categories: &[(&str, &[FieldSpec])],
    global: &ExtractedSheetSource<'_>,
    falcon: &ExtractedSheetSource<'_>,
) -> Result<ValueSheet, String> {
    let categories = categories
        .iter()
        .map(|(category_id, fields)| {
            let fields = fields
                .iter()
                .map(|field| match field.owner {
                    FieldOwner::Global => extracted_row(field.rust_name, global),
                    FieldOwner::Falcon => extracted_row(field.rust_name, falcon),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ValueCategory {
                id: (*category_id).to_string(),
                label: title_case(category_id),
                fields,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(ValueSheet {
        id: id.to_string(),
        title: title.to_string(),
        scope: scope.to_string(),
        engine_boundary: "rust_core_authority".to_string(),
        source,
        character_id: character_id.map(str::to_string),
        stage_id: None,
        reference_stage: reference_stage.map(str::to_string),
        categories,
    })
}

fn build_battlefield_stage_sheet() -> ValueSheet {
    let stage = StageProfile::battlefield();
    let mut surface_fields = Vec::new();
    for surface in stage.collision_surfaces() {
        surface_fields.push(stage_int_row(
            &format!("{}.left_x", surface.name),
            &format!("{}.left_x", surface.name),
            "stage_coord_milli",
            surface.left_x,
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
        surface_fields.push(stage_int_row(
            &format!("{}.right_x", surface.name),
            &format!("{}.right_x", surface.name),
            "stage_coord_milli",
            surface.right_x,
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
        surface_fields.push(stage_int_row(
            &format!("{}.y", surface.name),
            &format!("{}.y", surface.name),
            "stage_coord_milli",
            surface.y,
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
        surface_fields.push(stage_enum_row(
            &format!("{}.kind", surface.name),
            &format!("{}.kind", surface.name),
            "stage_surface_kind",
            surface_kind_label(surface.kind),
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
        surface_fields.push(stage_float_row(
            &format!("{}.friction_multiplier", surface.name),
            &format!("{}.friction_multiplier", surface.name),
            "source_f32",
            surface.friction_multiplier,
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
    }

    let blast_zone_fields = vec![
        stage_int_row(
            "blast_zones.left_x",
            "blast_zones.left_x",
            "stage_coord_milli",
            stage.blast_zones.left_x,
            BATTLEFIELD_STAGE_PROVENANCE,
        ),
        stage_int_row(
            "blast_zones.right_x",
            "blast_zones.right_x",
            "stage_coord_milli",
            stage.blast_zones.right_x,
            BATTLEFIELD_STAGE_PROVENANCE,
        ),
        stage_int_row(
            "blast_zones.top_y",
            "blast_zones.top_y",
            "stage_coord_milli",
            stage.blast_zones.top_y,
            BATTLEFIELD_STAGE_PROVENANCE,
        ),
        stage_int_row(
            "blast_zones.bottom_y",
            "blast_zones.bottom_y",
            "stage_coord_milli",
            stage.blast_zones.bottom_y,
            BATTLEFIELD_STAGE_PROVENANCE,
        ),
    ];

    let mut spawn_point_fields = Vec::new();
    for (index, spawn_point) in stage.spawn_points.iter().enumerate() {
        let label = format!("spawn_points.p{}", index + 1);
        spawn_point_fields.push(stage_int_row(
            &format!("{label}.x"),
            &format!("{label}.x"),
            "stage_coord_milli",
            spawn_point.x,
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
        spawn_point_fields.push(stage_int_row(
            &format!("{label}.y"),
            &format!("{label}.y"),
            "stage_coord_milli",
            spawn_point.y,
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
        spawn_point_fields.push(stage_int_row(
            &format!("{label}.facing"),
            &format!("{label}.facing"),
            "stage_facing",
            i32::from(spawn_point.facing),
            BATTLEFIELD_STAGE_PROVENANCE,
        ));
    }

    ValueSheet {
        id: "battlefield_stage_values".to_string(),
        title: "Battlefield Stage Values".to_string(),
        scope: "stage".to_string(),
        engine_boundary: "rust_core_authority".to_string(),
        source: json!({
            "stage_profile_name": stage.name,
            "reference_stage": "Battlefield",
            "provenance": BATTLEFIELD_STAGE_PROVENANCE,
            "required_raw_dat": "resources/melee/raw/GrNBa.dat",
            "decomp_refs": [
                ".research/doldecomp-melee/src/melee/gr/grbattle.c",
                ".research/doldecomp-melee/src/melee/gr/grdatfiles.c",
                ".research/doldecomp-melee/src/melee/gr/ground.c",
                ".research/doldecomp-melee/src/melee/mp/types.h"
            ],
            "required_public_symbols": [
                "map_head",
                "coll_data",
                "grGroundParam"
            ],
            "notes": "Current values are the Rust compatibility surface projection from the extracted Battlefield MapCollData path. The full lossless collision wireframe lives in resources/melee/extracted/stages/battlefield_stage.json and mole_core::MeleeStageProfile::battlefield()."
        }),
        character_id: None,
        stage_id: Some("battlefield".to_string()),
        reference_stage: Some("Battlefield".to_string()),
        categories: vec![
            ValueCategory {
                id: "collision_surfaces".to_string(),
                label: "Collision Surfaces".to_string(),
                fields: surface_fields,
            },
            ValueCategory {
                id: "blast_zones".to_string(),
                label: "Blast Zones".to_string(),
                fields: blast_zone_fields,
            },
            ValueCategory {
                id: "spawn_points".to_string(),
                label: "Spawn Points".to_string(),
                fields: spawn_point_fields,
            },
        ],
    }
}

fn extracted_source<'a>(
    payload: &'a Value,
    owner_scope: &'static str,
    owner_id: Option<&'static str>,
) -> Result<ExtractedSheetSource<'a>, String> {
    let source = payload
        .get("source")
        .ok_or_else(|| format!("missing source block for {owner_scope} payload"))?;
    let fields = payload
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing fields block for {owner_scope} payload"))?;
    Ok(ExtractedSheetSource {
        owner_scope,
        owner_id,
        source,
        fields,
    })
}

fn extracted_row(
    rust_name: &str,
    extracted: &ExtractedSheetSource<'_>,
) -> Result<ValueRow, String> {
    let field = extracted.fields.get(rust_name).ok_or_else(|| {
        format!(
            "missing `{rust_name}` in {} extracted field set",
            extracted.owner_scope
        )
    })?;
    let kind = field
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing kind for `{rust_name}`"))?;
    let source_name = field
        .get("source_name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing source_name for `{rust_name}`"))?;
    let offset = field.get("offset").and_then(Value::as_i64);
    let raw = field.get("raw").cloned().unwrap_or(Value::Null);
    let comparison_value_kind = comparison_value_kind(rust_name, kind, field);
    let converted_value = converted_value(&comparison_value_kind, raw.clone(), field);

    Ok(ValueRow {
        rust_name: rust_name.to_string(),
        source_name: source_name.to_string(),
        offset,
        offset_hex: offset.map(|offset| format!("0x{offset:x}")),
        kind: kind.to_string(),
        raw,
        converted_value,
        comparison_value_kind,
        provenance: "extracted_melee_dat".to_string(),
        owner_scope: Some(extracted.owner_scope.to_string()),
        owner_id: extracted.owner_id.map(str::to_string),
        notes: None,
    })
}

fn converted_value(comparison_value_kind: &str, raw: Value, field: &Value) -> Value {
    if comparison_value_kind == "source_f32" {
        return raw;
    }
    for key in ["stick_byte", "trigger_byte", "ticks", "milli"] {
        if let Some(value) = field.get(key) {
            return value.clone();
        }
    }
    raw
}

fn comparison_value_kind(rust_name: &str, kind: &str, field: &Value) -> String {
    if SOURCE_FLOAT_COMPARISON_FIELDS.contains(&rust_name) || kind == "source_f32" {
        return "source_f32".to_string();
    }
    for key in ["stick_byte", "trigger_byte", "ticks", "milli"] {
        if field.get(key).is_some() {
            return key.to_string();
        }
    }
    "raw".to_string()
}

fn stage_int_row(
    rust_name: &str,
    source_name: &str,
    kind: &str,
    value: i32,
    provenance: &str,
) -> ValueRow {
    ValueRow {
        rust_name: rust_name.to_string(),
        source_name: source_name.to_string(),
        offset: None,
        offset_hex: None,
        kind: kind.to_string(),
        raw: json!(value),
        converted_value: json!(value),
        comparison_value_kind: if kind == "stage_facing" {
            "raw".to_string()
        } else {
            "milli".to_string()
        },
        provenance: provenance.to_string(),
        owner_scope: Some("stage".to_string()),
        owner_id: Some("battlefield".to_string()),
        notes: None,
    }
}

fn stage_float_row(
    rust_name: &str,
    source_name: &str,
    kind: &str,
    value: f32,
    provenance: &str,
) -> ValueRow {
    ValueRow {
        rust_name: rust_name.to_string(),
        source_name: source_name.to_string(),
        offset: None,
        offset_hex: None,
        kind: kind.to_string(),
        raw: json!(value),
        converted_value: json!(value),
        comparison_value_kind: "source_f32".to_string(),
        provenance: provenance.to_string(),
        owner_scope: Some("stage".to_string()),
        owner_id: Some("battlefield".to_string()),
        notes: None,
    }
}

fn stage_enum_row(
    rust_name: &str,
    source_name: &str,
    kind: &str,
    value: &str,
    provenance: &str,
) -> ValueRow {
    ValueRow {
        rust_name: rust_name.to_string(),
        source_name: source_name.to_string(),
        offset: None,
        offset_hex: None,
        kind: kind.to_string(),
        raw: json!(value),
        converted_value: json!(value),
        comparison_value_kind: "raw".to_string(),
        provenance: provenance.to_string(),
        owner_scope: Some("stage".to_string()),
        owner_id: Some("battlefield".to_string()),
        notes: None,
    }
}

fn surface_kind_label(kind: StageSurfaceKind) -> &'static str {
    match kind {
        StageSurfaceKind::Solid => "solid",
        StageSurfaceKind::Soft => "soft",
    }
}

fn title_case(value: &str) -> String {
    value
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
