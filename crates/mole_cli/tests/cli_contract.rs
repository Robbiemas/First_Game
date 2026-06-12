use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use mole_cli::{
    count_graph_statuses, find_project_root, parity_summary, parse_git_status, run_cli,
    verification_plan_for_changed_paths,
};
use mole_frame_data::decode_runtime_source_frame_capsules;
use mole_ledger::{LedgerMap, LedgerRegistry};
use serde_json::json;

#[test]
fn parse_git_status_summarizes_branch_remote_and_dirty_counts_for_agents() {
    let status = "\
## handoff/rust-rollback-architecture...origin/handoff/rust-rollback-architecture
 M crates/mole_core/src/sim.rs
 D execs/Old.cmd
?? crates/mole_cli/Cargo.toml
R  old -> new
";

    let summary = parse_git_status(
        status,
        Some("https://github.com/Robbiemas/First_Game.git".to_string()),
    );

    assert_eq!(
        summary.branch.as_deref(),
        Some("handoff/rust-rollback-architecture")
    );
    assert!(summary.branch_ok);
    assert!(summary.remote_ok);
    assert!(summary.dirty);
    assert_eq!(summary.modified, 1);
    assert_eq!(summary.deleted, 1);
    assert_eq!(summary.untracked, 1);
    assert_eq!(summary.renamed, 1);
    assert_eq!(summary.total_changed, 4);
}

#[test]
fn graph_status_counter_counts_nodes_and_edges_together() {
    let graph = json!({
        "nodes": [
            {"id": "Wait", "status": "aligned"},
            {"id": "Dash", "status": "partial"}
        ],
        "edges": [
            {"from": "Wait", "to": "Dash", "status": "aligned"},
            {"from": "Dash", "to": "Run", "status": "missing"}
        ]
    });

    let counts = count_graph_statuses(&graph);

    assert_eq!(counts.get("aligned"), Some(&2));
    assert_eq!(counts.get("partial"), Some(&1));
    assert_eq!(counts.get("missing"), Some(&1));
}

#[test]
fn parity_summary_reads_agent_facing_ledgers_without_running_generators() {
    let root = temp_project_root("parity");
    write_json(
        &root.join("docs/state_graphs/parity_reports/value_diffs.json"),
        &json!({
            "sections": {
                "global_values": {
                    "rows": [
                        {"status": "match"},
                        {"status": "diff"}
                    ],
                    "actionable_rows": [{"field": "gravity"}],
                    "derived_rows": []
                },
                "test_character_values": {
                    "rows": [{"status": "match"}],
                    "actionable_rows": [],
                    "derived_rows": [{"field": "max_jumps"}]
                }
            }
        }),
    );
    write_json(
        &root.join("docs/state_graphs/parity_reports/falcon_ecb_coverage.json"),
        &json!({
            "mapped_motion_state_count": 55,
            "missing_sampled_mappings": [],
            "unmapped_derived_motion_states": ["KneeBend", "GuardReflect"]
        }),
    );
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [{"status": "partial"}],
            "edges": [{"status": "missing"}]
        }),
    );

    let summary = parity_summary(&root);

    assert_eq!(summary.value_total_rows, 3);
    assert_eq!(summary.value_matches, 2);
    assert_eq!(summary.value_actionable, 1);
    assert_eq!(summary.value_derived, 1);
    assert_eq!(summary.ecb_mapped_motion_states, 55);
    assert_eq!(
        summary.ecb_unmapped_derived_states,
        vec!["KneeBend".to_string(), "GuardReflect".to_string()]
    );
    assert_eq!(summary.graph_status_counts.get("missing"), Some(&1));
}

#[test]
fn project_root_detection_handles_parent_workspace_directory() {
    let parent = temp_project_root("parent");
    let first_game = parent.join("First_Game");
    fs::create_dir_all(first_game.join(".git")).unwrap();
    fs::create_dir_all(first_game.join("docs/state_graphs")).unwrap();
    fs::write(first_game.join("Cargo.toml"), "[workspace]\n").unwrap();

    assert_eq!(find_project_root(&parent), Some(first_game));
}

#[test]
fn decomp_search_returns_agent_sized_matches_with_followup_commands() {
    let root = temp_project_root("decomp_search");
    let decomp = root.join(".research/doldecomp-melee");
    fs::create_dir_all(decomp.join("src/melee/ft/chara/ftCommon")).unwrap();
    fs::write(
        decomp.join("src/melee/ft/chara/ftCommon/ftCo_Turn.c"),
        "void ftCo_Turn_Anim(Fighter_GObj* gobj)\n{\n    ftCo_Turn_Anim_Inner(gobj);\n}\n",
    )
    .unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "decomp".to_string(),
        "search".to_string(),
        "ftCo_Turn_Anim".to_string(),
        "--decomp-root".to_string(),
        decomp.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "decomp search");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["query"], "ftCo_Turn_Anim");
    assert_eq!(
        parsed["matches"][0]["path"],
        "src/melee/ft/chara/ftCommon/ftCo_Turn.c"
    );
    assert_eq!(parsed["matches"][0]["line"], 1);
    assert!(parsed["matches"][0]["preview"]
        .as_str()
        .unwrap()
        .contains("ftCo_Turn_Anim"));
    assert!(parsed["matches"][0]["suggested_command"]
        .as_str()
        .unwrap()
        .contains("decomp show"));
}

#[test]
fn decomp_show_returns_bounded_numbered_excerpt_for_reference() {
    let root = temp_project_root("decomp_show");
    let decomp = root.join(".research/doldecomp-melee");
    fs::create_dir_all(decomp.join("src/melee/ft/chara/ftCommon")).unwrap();
    fs::write(
        decomp.join("src/melee/ft/chara/ftCommon/ftCo_Landing.c"),
        "one\ntwo\nvoid ftCo_LandingFallSpecial_Enter(void) {}\nfour\nfive\n",
    )
    .unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "decomp".to_string(),
        "show".to_string(),
        "src/melee/ft/chara/ftCommon/ftCo_Landing.c".to_string(),
        "--line".to_string(),
        "3".to_string(),
        "--context".to_string(),
        "1".to_string(),
        "--decomp-root".to_string(),
        decomp.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "decomp show");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["excerpt"]["start_line"], 2);
    assert_eq!(parsed["excerpt"]["end_line"], 4);
    assert_eq!(parsed["excerpt"]["lines"].as_array().unwrap().len(), 3);
    assert_eq!(
        parsed["excerpt"]["lines"][1]["text"],
        "void ftCo_LandingFallSpecial_Enter(void) {}"
    );
}

#[test]
fn decomp_symbol_prefers_exact_function_definition_matches() {
    let root = temp_project_root("decomp_symbol");
    let decomp = root.join(".research/doldecomp-melee");
    fs::create_dir_all(decomp.join("src/melee/ft/chara/ftCommon")).unwrap();
    fs::write(
        decomp.join("src/melee/ft/chara/ftCommon/ftCo_Guard.c"),
        "void helper(void) { ftCo_GuardOn_Phys(0); }\nftCo_GuardOn_Phys(gobj, false,\nvoid ftCo_GuardOn_Phys(Fighter_GObj* gobj)\n{\n    ft_80084F3C(gobj);\n}\n",
    )
    .unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "decomp".to_string(),
        "symbol".to_string(),
        "ftCo_GuardOn_Phys".to_string(),
        "--decomp-root".to_string(),
        decomp.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "decomp symbol");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["matches"][0]["line"], 3);
    assert_eq!(parsed["matches"][0]["rank_reason"], "exact_definition");
}

#[test]
fn decomp_commands_report_missing_decomp_root_without_mutation() {
    let root = temp_project_root("decomp_missing");
    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "decomp".to_string(),
        "search".to_string(),
        "ftCo_Turn".to_string(),
        "--decomp-root".to_string(),
        root.join("missing-decomp").display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "decomp search");
    assert_eq!(parsed["ok"], false);
    assert!(parsed["errors"][0]
        .as_str()
        .unwrap()
        .contains("decompiled Melee root not found"));
    assert_eq!(parsed["matches"].as_array().unwrap().len(), 0);
}

#[test]
fn friend_connect_diagnostics_summarizes_bounded_netplay_log() {
    let root = temp_project_root("friend_connect_diagnostics");
    let log_path = root.join("logs/netplay/visible_host-test.jsonl");
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    fs::write(
        &log_path,
        concat!(
            "{\"role\":\"visible_host\",\"event\":\"frame_summary\",\"frame\":60,\"received_packets\":5,\"missing_remote_frames\":2,\"rollback_corrections\":1,\"last_remote_frame\":59,\"last_remote_checksum\":100,\"last_rtt_frames\":2,\"world_checksum\":200,\"packet_bundle_len\":8,\"speed_ppm\":1000000,\"advance_online_frame\":false,\"skip_online_frame\":false}\n",
            "{\"role\":\"visible_host\",\"event\":\"frame_summary\",\"frame\":120,\"received_packets\":9,\"missing_remote_frames\":3,\"rollback_corrections\":2,\"last_remote_frame\":120,\"last_remote_checksum\":101,\"last_rtt_frames\":1,\"world_checksum\":201,\"packet_bundle_len\":8,\"speed_ppm\":1010000,\"advance_online_frame\":true,\"skip_online_frame\":false}\n",
            "{\"role\":\"visible_host\",\"event\":\"frame_summary\",\"frame\":121,\"received_packets\":9,\"missing_remote_frames\":3,\"rollback_corrections\":2,\"last_remote_frame\":120,\"last_remote_checksum\":101,\"last_rtt_frames\":1,\"world_checksum\":201,\"packet_bundle_len\":8,\"speed_ppm\":1010000,\"advance_online_frame\":false,\"skip_online_frame\":true}\n",
        ),
    )
    .unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "friend-connect".to_string(),
        "diagnostics".to_string(),
        "--log".to_string(),
        log_path.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "friend-connect diagnostics");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["events"], 3);
    assert_eq!(parsed["frame_summaries"], 3);
    assert_eq!(parsed["last_frame"], 121);
    assert_eq!(parsed["max_rollback_corrections"], 2);
    assert_eq!(parsed["max_missing_remote_frames"], 3);
    assert_eq!(parsed["max_packet_bundle_len"], 8);
    assert_eq!(parsed["advance_events"], 1);
    assert_eq!(parsed["skip_events"], 1);
    assert_eq!(parsed["speed_ppm_min"], 1_000_000);
    assert_eq!(parsed["speed_ppm_max"], 1_010_000);
    assert_eq!(parsed["last_world_checksum"], 201);
}

#[test]
fn frame_data_extract_returns_dolphin_mole_scoped_attack_air_n_artifact() {
    let root = temp_project_root("frame_data_extract");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {
                "source_space": "melee_xyz",
                "default_view": "xy",
                "z_policy": "preserve_and_project"
            },
            "sources": [{"kind": "decomp", "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c", "line": 1}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [{
                "frame": 6,
                "interpolates_from_previous": true,
                "hitboxes": [{"id": 0, "center": {"x": 3.5, "y": 8.0, "z": -1.25}}],
                "hurtboxes": []
            }],
            "gaps": [{"field": "iasa_frame", "reason": "not yet proven"}],
            "overrides": []
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data extract");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["artifact"]["target_character"], "dolphin_mole");
    assert_eq!(parsed["artifact"]["target_character_label"], "Dolphin Mole");
    assert_eq!(parsed["artifact"]["source_character"], "captain");
    assert_eq!(
        parsed["artifact"]["projection"]["z_policy"],
        "preserve_and_project"
    );
    assert_eq!(
        parsed["artifact"]["keyframes"][0]["hitboxes"][0]["center"]["z"],
        -1.25
    );
    assert_eq!(parsed["artifact"]["gaps"][0]["field"], "iasa_frame");
}

#[test]
fn frame_data_extract_can_map_source_state_from_another_character() {
    let root = temp_project_root("frame_data_extract_cross_character_source_state");
    write_json(
        &root.join("resources/melee/extracted/marth_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 56,
                "name": "PlyMars5K_Share_ACTION_AttackLw3_figatree",
                "figatree_root": "PlyMars5K_Share_ACTION_AttackLw3_figatree",
                "figatree": {"frames_ticks": 39},
                "subaction_script_offset": 1234
            }]
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "marth".to_string(),
        "--source-state".to_string(),
        "AttackLw3".to_string(),
        "--state".to_string(),
        "AttackLw3".to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data extract");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["source_character"], "marth");
    assert_eq!(parsed["source_state"], "AttackLw3");
    assert_eq!(parsed["state"], "AttackLw3");
    assert_eq!(parsed["artifact"]["target_character"], "dolphin_mole");
    assert_eq!(parsed["artifact"]["source_character"], "marth");
    assert_eq!(parsed["artifact"]["state"], "AttackLw3");
    assert_eq!(parsed["artifact"]["source_action_key"], "AttackLw3");
    assert_eq!(parsed["artifact"]["runtime_motion_state"], "AttackLw3");
    assert_eq!(
        parsed["artifact"]["sources"][0]["source_action_name"],
        "PlyMars5K_Share_ACTION_AttackLw3_figatree"
    );
}

#[test]
fn frame_data_export_runtime_writes_native_capsule_module_from_artifact() {
    let root = temp_project_root("frame_data_export_runtime");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "source_character": "captain",
            "state": "AttackAirN",
            "projection": {
                "source_space": "melee_xyz",
                "default_view": "xy",
                "z_policy": "preserve_and_project"
            },
            "sources": [{"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 290}],
            "keyframes": [
                {
                    "frame": 7,
                    "hitboxes": [{
                        "id": 0,
                        "bone": 14,
                        "hit_group": 0,
                        "use_common_bone_ids": false,
                        "source_center": {"x": 3.5, "y": 8.0, "z": -1.25},
                        "source_previous_center": {"x": 2.5, "y": 7.0, "z": -0.75},
                        "center": {"x": -1.25, "y": 8.0, "z": 0.0},
                        "previous_center": {"x": -0.75, "y": 7.0, "z": 0.0},
                        "radius": 4.296875,
                        "damage": 6,
                        "angle": 82,
                        "kbg": 100,
                        "weight_set_kb": 0,
                        "bkb": 0,
                        "element": 0,
                        "shield_damage": 0,
                        "hit_grounded": true,
                        "hit_aerial": true,
                        "source_handler": "ftAction_8007121C"
                    }],
                    "hurtboxes": [{
                        "id": 4,
                        "bone": 23,
                        "height": 2,
                        "is_grabbable": true,
                        "a": {"x": 1.5, "y": 2.25, "z": 0.0},
                        "b": {"x": -2.0, "y": 3.75, "z": 0.0},
                        "source_a": {"x": 9.0, "y": 2.25, "z": 1.5},
                        "source_b": {"x": 7.0, "y": 3.75, "z": -2.0},
                        "radius": 1.25,
                        "state": "HurtCapsule_Enabled"
                    }]
                },
                {
                    "frame": 13,
                    "hitboxes": [],
                    "hurtboxes": [{
                        "id": 4,
                        "bone": 23,
                        "a": {"x": 1.0, "y": 2.0, "z": 0.0},
                        "b": {"x": -1.0, "y": 3.0, "z": 0.0},
                        "source_a": {"x": 9.0, "y": 2.0, "z": 1.0},
                        "source_b": {"x": 7.0, "y": 3.0, "z": -1.0},
                        "radius": 1.25
                    }]
                }
            ],
            "gaps": [],
            "overrides": []
        }),
    );
    let output_path = root.join("crates/mole_runtime/src/generated/source_frame_data.rs");

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "export-runtime".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
        "--output".to_string(),
        output_path.display().to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let generated = fs::read_to_string(&output_path).unwrap();

    assert_eq!(parsed["command"], "frame-data export-runtime");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["wrote_output"], true);
    assert_eq!(parsed["hitbox_frame_count"], 1);
    assert_eq!(parsed["hurtbox_frame_count"], 2);
    assert_eq!(
        PathBuf::from(parsed["source_artifact_path"].as_str().unwrap()),
        root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json")
    );
    assert_eq!(
        PathBuf::from(parsed["output_path"].as_str().unwrap()),
        output_path
    );
    assert!(generated.contains("MotionState::AttackAirN"));
    assert!(generated.contains("\"dolphin_mole\""));
    assert!(generated.contains("x: 3.5"));
    assert!(generated.contains("y: 8.0"));
    assert!(generated.contains("z: -1.25"));
    assert!(generated.contains("x: 9.0"));
    assert!(generated.contains("y: 2.25"));
    assert!(generated.contains("z: 1.5"));
    assert!(generated.contains("damage: 6"));
    assert!(generated.contains("radius: 4.296875"));
    assert!(generated.contains("ftAction_8007121C"));
}

#[test]
fn frame_data_extract_all_states_creates_compact_source_manifest() {
    let root = temp_project_root("frame_data_extract_all_states");
    write_json(
        &root.join("resources/melee/extracted/test_source_action_animation_table.json"),
        &json!({
            "actions": [
                {
                    "action_state_id": 2,
                    "name": "TestSource_ACTION_Wait1_figatree",
                    "figatree_root": "TestSource_ACTION_Wait1_figatree",
                    "figatree": {"frames_ticks": 60},
                    "subaction_script_offset": 0
                },
                {
                    "action_state_id": 68,
                    "name": "TestSource_ACTION_AttackAirN_figatree",
                    "figatree_root": "TestSource_ACTION_AttackAirN_figatree",
                    "figatree": {"frames_ticks": 45},
                    "subaction_script_offset": 16
                },
                {
                    "action_state_id": 62,
                    "name": "TestSource_ACTION_AttackS4S_figatree",
                    "figatree_root": "TestSource_ACTION_AttackS4S_figatree",
                    "figatree": {"frames_ticks": 54},
                    "subaction_script_offset": 32
                },
                {
                    "action_state_id": 999,
                    "name": "TestSource_ACTION_SourceOnlyState_figatree",
                    "figatree_root": "TestSource_ACTION_SourceOnlyState_figatree",
                    "figatree": {"frames_ticks": 12},
                    "subaction_script_offset": 48
                }
            ]
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/test_source_hurtbox_inits.json"),
        &json!({
            "source": "ftData.x30",
            "inits": [{
                "bone_idx": 14,
                "height": 2,
                "is_grabbable": true,
                "a_offset": {"x": 0.0, "y": 1.25, "z": -2.5},
                "b_offset": {"x": 3.75, "y": 4.5, "z": 6.25},
                "scale": 1.125
            }]
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/test_source_costume_skeleton.json"),
        &json!({
            "source": "JObj",
            "joints": [{"index": 14, "name": "TransN", "parent": null}]
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--all-states".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "test_source".to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data extract");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["all_states"], true);
    assert_eq!(parsed["character"], "dolphin_mole");
    assert_eq!(parsed["source_character"], "test_source");
    assert_eq!(parsed["source_action_count"], 4);
    assert_eq!(parsed["runtime_mapped_state_count"], 3);
    assert_eq!(parsed["created_artifacts"], 0);
    assert_eq!(parsed["wrote_artifacts"], 0);
    assert_eq!(parsed["created_manifest"], true);
    assert_eq!(parsed["wrote_manifest"], true);
    let manifest_path = root.join("resources/melee/frame_data/dolphin_mole/source_manifest.json");
    assert_eq!(
        PathBuf::from(parsed["manifest_path"].as_str().unwrap()),
        manifest_path
    );
    assert!(manifest_path.exists());
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(
        manifest["artifact_kind"],
        "source_character_frame_data_manifest"
    );
    assert_eq!(manifest["source_space"], "melee_xyz");
    assert_eq!(
        manifest["z_policy"],
        "preserve_source_z_flatten_after_runtime_projection"
    );
    assert_eq!(
        manifest["rig"]["hurtbox_inits"]["source_table"],
        "ftData.x30"
    );
    assert_eq!(
        manifest["rig"]["hurtbox_inits"]["data"]["inits"][0]["a_offset"]["z"],
        -2.5
    );
    assert!(
        manifest["rig"]["derived_samples"]
            .get("hurtbox_samples_path")
            .is_none(),
        "compact manifests must derive hurt capsules from ftData.x30 + JObj/FigaTree, not a cached per-frame sample file"
    );
    assert_eq!(
        manifest["rig"]["skeleton"]["data"]["joints"][0]["name"],
        "TransN"
    );
    let state_names: Vec<_> = manifest["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|state| state["state"].as_str().unwrap())
        .collect();
    assert_eq!(
        state_names,
        vec!["Wait", "AttackAirN", "AttackS4", "SourceOnlyState"]
    );
    assert_eq!(
        manifest["actions"][0]["source_action_name"],
        "TestSource_ACTION_Wait1_figatree"
    );
    assert_eq!(
        manifest["actions"][2]["source_action_name"],
        "TestSource_ACTION_AttackS4S_figatree"
    );
    assert_eq!(manifest["actions"][1]["subaction_script_offset"], 16);
    assert_eq!(manifest["actions"][2]["runtime_motion_state"], "AttackS4");
    assert_eq!(
        manifest["actions"][3]["runtime_motion_state"],
        serde_json::Value::Null
    );
    assert!(manifest["actions"][1].get("keyframes").is_none());
    assert!(!root
        .join("resources/melee/frame_data/dolphin_mole/Wait.json")
        .exists());
    assert!(!root
        .join("resources/melee/frame_data/dolphin_mole/AttackAirN.json")
        .exists());
    assert!(!root
        .join("resources/melee/frame_data/dolphin_mole/AttackS4.json")
        .exists());
    assert!(!root
        .join("resources/melee/frame_data/dolphin_mole/SourceOnlyState.json")
        .exists());
    assert_eq!(
        manifest["actions"][3]["runtime_motion_state"],
        serde_json::Value::Null
    );
    assert_eq!(parsed["rust_parity_gaps"][0]["state"], "SourceOnlyState");
    assert_eq!(
        parsed["skipped_source_actions"].as_array().unwrap().len(),
        0
    );
}

#[test]
fn frame_data_sample_attack_air_n_uses_compact_manifest() {
    let root = workspace_root();
    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "sample".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
        "--frame".to_string(),
        "7".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data sample");
    assert_eq!(parsed["character"], "dolphin_mole");
    assert_eq!(parsed["state"], "AttackAirN");
    assert_eq!(parsed["source_space"], "melee_xyz");
    assert_eq!(parsed["ok"], true);
    assert_eq!(
        parsed["sample"]["projected_view_kind"],
        "derived_debug_view"
    );
    assert!(!parsed["sample"]["hit_capsules"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!parsed["sample"]["hurt_capsules"]
        .as_array()
        .unwrap()
        .is_empty());

    let source_hit = &parsed["sample"]["hit_capsules"][0]["source_b"];
    let projected_hit = &parsed["sample"]["hit_capsules"][0]["b"];
    let source_hurt = &parsed["sample"]["hurt_capsules"][0]["source_a"];
    let projected_hurt = &parsed["sample"]["hurt_capsules"][0]["a"];

    assert!(source_hit["z"].as_f64().unwrap().abs() > 0.01);
    assert_eq!(projected_hit["z"], json!(0.0));
    assert!(source_hurt["z"].as_f64().unwrap().abs() > 0.01);
    assert_eq!(projected_hurt["z"], json!(0.0));

    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            root.join("resources/melee/frame_data/dolphin_mole/source_manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let attack_air_n = manifest["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["state"] == json!("AttackAirN"))
        .unwrap();
    assert_eq!(
        attack_air_n["derived_frame_capsules"]["materialized"],
        json!(false)
    );
    assert_eq!(attack_air_n["source_action_key"], json!("AttackAirN"));
    assert_eq!(attack_air_n["source_action"]["action_state_id"], json!(68));
}

#[test]
fn frame_data_sample_exposes_transn_root_motion_for_roll_physics() {
    let root = workspace_root();
    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "sample".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "EscapeF".to_string(),
        "--frame".to_string(),
        "2".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data sample");
    assert_eq!(parsed["ok"], true);
    assert_eq!(
        parsed["sample"]["source_root_motion"]["source_part"],
        "FtPart_TransN"
    );
    assert_eq!(
        parsed["sample"]["source_root_motion"]["source_node_index"],
        1
    );
    assert!(
        parsed["sample"]["source_root_motion"]["transn_offset"]["z"]
            .as_f64()
            .unwrap()
            > 0.0
    );
}

#[test]
fn frame_data_export_runtime_all_states_writes_combined_capsule_module() {
    let root = temp_project_root("frame_data_export_runtime_all_states");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "source_character": "test_source",
            "state": "AttackAirN",
            "keyframes": [{
                "frame": 7,
                "hitboxes": [{
                    "id": 0,
                    "bone": 14,
                    "source_center": {"x": 3.5, "y": 8.0, "z": -1.25},
                    "source_previous_center": {"x": 2.5, "y": 7.0, "z": -0.75},
                    "radius": 4.296875,
                    "damage": 6,
                    "angle": 82,
                    "kbg": 100,
                    "bkb": 0,
                    "hit_grounded": true,
                    "hit_aerial": true,
                    "source_handler": "ftAction_8007121C"
                }],
                "hurtboxes": []
            }]
        }),
    );
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/Wait.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "source_character": "test_source",
            "state": "Wait",
            "keyframes": [{
                "frame": 1,
                "hitboxes": [],
                "hurtboxes": [{
                    "id": 4,
                    "bone": 23,
                    "height": 2,
                    "is_grabbable": true,
                    "source_a": {"x": 9.0, "y": 2.25, "z": 1.5},
                    "source_b": {"x": 7.0, "y": 3.75, "z": -2.0},
                    "radius": 1.25,
                    "state": "HurtCapsule_Enabled"
                }]
            }]
        }),
    );
    let output_path = root.join("legacy_frame_data_boxes.rs");

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "export-runtime".to_string(),
        "--all-states".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--output".to_string(),
        output_path.display().to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let generated = fs::read_to_string(&output_path).unwrap();

    assert_eq!(parsed["command"], "frame-data export-runtime");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["all_states"], true);
    assert_eq!(parsed["state_count"], 2);
    assert_eq!(parsed["hitbox_frame_count"], 1);
    assert_eq!(parsed["hurtbox_frame_count"], 1);
    assert_eq!(parsed["source_artifact_paths"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["runtime_export_kind"], "legacy_baked_capsule_module");
    assert!(generated.contains("SourceHitCapsule"));
    assert!(generated.contains("SourceHurtCapsule"));
    assert!(generated
        .contains("pub(crate) const SOURCE_ARTIFACT_KIND: &str = \"baked_frame_data_boxes\""));
    assert!(generated.contains("MotionState::AttackAirN"));
    assert!(generated.contains("MotionState::Wait"));
    assert!(generated.contains("_HIT_FRAME_"));
    assert!(generated.contains("_HURT_FRAME_"));
}

#[test]
fn frame_data_export_runtime_all_states_compact_manifest_writes_compact_source_export() {
    let root = workspace_root();
    let temp_root = temp_project_root("frame_data_export_runtime_compact_source_export");
    let output_path = temp_root.join("source_frame_data.rs");

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "export-runtime".to_string(),
        "--all-states".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--output".to_string(),
        output_path.display().to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let generated = fs::read_to_string(&output_path).unwrap();

    assert_eq!(parsed["command"], "frame-data export-runtime");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["compact_manifest_detected"], true);
    assert_eq!(parsed["runtime_export_kind"], "compact_source_export");
    assert_eq!(parsed["state_count"], 105);
    let gap_action_ids = parsed["rust_parity_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|gap| gap["action_state_id"].as_u64())
        .collect::<Vec<_>>();
    assert!(
        !gap_action_ids.contains(&40),
        "GuardDamage is consumed by the GuardSetOff runtime binding"
    );
    assert!(
        !gap_action_ids.contains(&36),
        "Landing action-table id 36 is consumed by LandingFallSpecial"
    );
    assert!(
        !gap_action_ids.contains(&238),
        "Entry source action data is shared by the Entry, EntryStart, and EntryEnd runtime aliases"
    );
    assert_eq!(parsed["wrote_output"], true);
    assert!(parsed["figatree_chunk_count"].as_u64().unwrap() >= 65);
    assert!(parsed["figatree_chunk_bytes"].as_u64().unwrap() > 0);
    assert!(!generated.contains("RuntimeSourceExport"));
    assert!(!generated.contains("RuntimeFigatreeChunk"));
    assert!(generated.contains("RuntimeActionBinding"));
    assert!(generated.contains("MeleeActionStateId::new(65)"));
    assert!(generated.contains("MeleeActionStateId::new(75)"));
    assert!(generated.contains("MeleeActionStateId::new(45)"));
    assert!(generated.contains("MeleeActionStateId::new(46)"));
    assert!(generated.contains("MeleeActionStateId::new(47)"));
    assert!(generated.contains("MeleeActionStateId::new(48)"));
    assert!(generated.contains("MeleeActionStateId::new(49)"));
    assert!(generated.contains("MeleeActionStateId::new(91)"));
    assert!(generated.contains("MeleeActionStateId::new(183)"));
    assert!(generated.contains("MeleeActionStateId::new(184)"));
    assert!(generated.contains("MeleeActionStateId::new(186)"));
    assert!(generated.contains("MeleeActionStateId::new(187)"));
    assert!(generated.contains("MeleeActionStateId::new(191)"));
    assert!(generated.contains("MeleeActionStateId::new(192)"));
    assert!(generated.contains("MeleeActionStateId::new(194)"));
    assert!(generated.contains("MeleeActionStateId::new(195)"));
    assert!(generated.contains("MeleeActionStateId::new(199)"));
    assert!(generated.contains("MeleeActionStateId::new(200)"));
    assert!(generated.contains("MeleeActionStateId::new(201)"));
    assert!(generated.contains("source_action_key: \"DamageHi1\""));
    assert!(generated.contains("source_action_key: \"Attack12\""));
    assert!(generated.contains("source_action_key: \"Attack13\""));
    assert!(generated.contains("source_action_key: \"Attack100Start\""));
    assert!(generated.contains("source_action_key: \"Attack100Loop\""));
    assert!(generated.contains("source_action_key: \"Attack100End\""));
    assert!(generated.contains("source_action_key: \"DamageFlyRoll\""));
    assert!(generated.contains("source_action_key: \"DownBoundU\""));
    assert!(generated.contains("source_action_key: \"DownWaitU\""));
    assert!(generated.contains("source_action_key: \"DownStandU\""));
    assert!(generated.contains("source_action_key: \"DownAttackU\""));
    assert!(generated.contains("source_action_key: \"DownBoundD\""));
    assert!(generated.contains("source_action_key: \"DownWaitD\""));
    assert!(generated.contains("source_action_key: \"DownStandD\""));
    assert!(generated.contains("source_action_key: \"DownAttackD\""));
    assert!(generated.contains("source_action_key: \"Passive\""));
    assert!(generated.contains("source_action_key: \"PassiveStandF\""));
    assert!(generated.contains("source_action_key: \"PassiveStandB\""));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(45), source_action_key: \"Attack12\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(46), source_action_key: \"Attack13\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(47), source_action_key: \"Attack100Start\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(48), source_action_key: \"Attack100Loop\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(49), source_action_key: \"Attack100End\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(75), source_action_key: \"DamageHi1\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(91), source_action_key: \"DamageFlyRoll\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(183), source_action_key: \"DownBoundU\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(184), source_action_key: \"DownWaitU\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(186), source_action_key: \"DownStandU\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(187), source_action_key: \"DownAttackU\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(191), source_action_key: \"DownBoundD\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(192), source_action_key: \"DownWaitD\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(194), source_action_key: \"DownStandD\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(195), source_action_key: \"DownAttackD\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(199), source_action_key: \"Passive\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(200), source_action_key: \"PassiveStandF\", motion_state: None }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(201), source_action_key: \"PassiveStandB\", motion_state: None }"
    ));
    assert!(generated.contains("MeleeActionStateId::new(322)"));
    assert!(generated.contains("MeleeActionStateId::new(323)"));
    assert!(generated.contains("MeleeActionStateId::new(324)"));
    assert!(generated.contains("source_action_key: \"Entry\""));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(322), source_action_key: \"Entry\", motion_state: Some(MotionState::Entry) }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(323), source_action_key: \"Entry\", motion_state: Some(MotionState::EntryStart) }"
    ));
    assert!(generated.contains(
        "RuntimeActionBinding { action_state_id: MeleeActionStateId::new(324), source_action_key: \"Entry\", motion_state: Some(MotionState::EntryEnd) }"
    ));
    assert!(generated.contains("motion_state: Some(MotionState::AttackAirN)"));
    assert!(generated.contains("SOURCE_FRAME_CAPSULES_BYTES"));
    assert!(generated.contains("include_bytes!(\"source_frame_data/source_frame_capsules.bin\")"));
    assert!(!generated.contains("SOURCE_MANIFEST_JSON"));
    assert!(!generated.contains("SOURCE_EXPORT"));
    assert!(!generated.contains("include_str!(\"source_frame_data/source_manifest.json\")"));
    assert!(!generated.contains(".figatree.bin"));
    assert!(generated.contains("MotionState::AttackAirN"));
    assert!(generated.contains("AttackAirN"));
    assert!(!generated.contains("SourceHitCapsule"));
    assert!(!generated.contains("SourceHurtCapsule"));
    assert!(!generated.contains("_HIT_FRAME_"));
    assert!(!generated.contains("_HURT_FRAME_"));
    assert!(generated.len() < 200_000);
    assert!(output_path
        .parent()
        .unwrap()
        .join("source_frame_data/source_frame_capsules.bin")
        .exists());
    let source_frame_capsules = fs::read(
        output_path
            .parent()
            .unwrap()
            .join("source_frame_data/source_frame_capsules.bin"),
    )
    .unwrap();
    let decoded_capsules = decode_runtime_source_frame_capsules(&source_frame_capsules).unwrap();
    let damage_air1 = decoded_capsules
        .iter()
        .find(|action| action.source_action_key == "DamageAir1")
        .expect("all-states runtime export should bake DamageAir1");
    let down_wait_u = decoded_capsules
        .iter()
        .find(|action| action.source_action_key == "DownWaitU")
        .expect("all-states runtime export should bake DownWaitU");
    assert_eq!(down_wait_u.frames.len(), 70);
    let down_stand_u = decoded_capsules
        .iter()
        .find(|action| action.source_action_key == "DownStandU")
        .expect("all-states runtime export should bake DownStandU");
    assert_eq!(down_stand_u.frames.len(), 30);
    let down_attack_u = decoded_capsules
        .iter()
        .find(|action| action.source_action_key == "DownAttackU")
        .expect("all-states runtime export should bake DownAttackU");
    assert_eq!(down_attack_u.frames.len(), 50);
    let passive = decoded_capsules
        .iter()
        .find(|action| action.source_action_key == "Passive")
        .expect("all-states runtime export should bake Passive");
    assert_eq!(passive.frames.len(), 26);
    let passive_stand_f = decoded_capsules
        .iter()
        .find(|action| action.source_action_key == "PassiveStandF")
        .expect("all-states runtime export should bake PassiveStandF");
    assert_eq!(passive_stand_f.frames.len(), 40);
    let passive_stand_b = decoded_capsules
        .iter()
        .find(|action| action.source_action_key == "PassiveStandB")
        .expect("all-states runtime export should bake PassiveStandB");
    assert_eq!(passive_stand_b.frames.len(), 40);
    let down_bound_pose = damage_air1
        .frames
        .get(1)
        .expect("DamageAir1 should have a second source frame")
        .down_bound_pose;
    assert!(down_bound_pose.hip_mtx_1_1.is_finite());
    assert!(down_bound_pose.hip_mtx_1_1.abs() > 0.001);
    assert_eq!(parsed["companion_manifest_path"], serde_json::Value::Null);
}

#[test]
fn frame_data_export_runtime_state_samples_compact_manifest() {
    let root = workspace_root();
    let temp_root = temp_project_root("frame_data_export_runtime_state_compact_manifest");
    let manifest_path = root.join("resources/melee/frame_data/dolphin_mole/source_manifest.json");
    let output_path = temp_root.join("source_frame_data.rs");

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "export-runtime".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
        "--output".to_string(),
        output_path.display().to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let generated = fs::read_to_string(&output_path).unwrap();

    assert_eq!(parsed["command"], "frame-data export-runtime");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["wrote_output"], true);
    assert_eq!(parsed["runtime_export_kind"], "compact_source_export");
    assert!(parsed["figatree_chunk_count"].as_u64().unwrap() >= 1);
    assert!(parsed["figatree_chunk_bytes"].as_u64().unwrap() > 0);
    assert_eq!(
        PathBuf::from(parsed["source_manifest_path"].as_str().unwrap()),
        manifest_path
    );
    assert!(!generated.contains("RuntimeSourceExport"));
    assert!(!generated.contains("RuntimeFigatreeChunk"));
    assert!(generated.contains("RuntimeActionBinding"));
    assert!(generated.contains("MeleeActionStateId::new(65)"));
    assert!(generated.contains("source_action_key: \"AttackAirN\""));
    assert!(generated.contains("motion_state: Some(MotionState::AttackAirN)"));
    assert!(generated.contains("SOURCE_FRAME_CAPSULES_BYTES"));
    assert!(generated.contains("include_bytes!(\"source_frame_data/source_frame_capsules.bin\")"));
    assert!(!generated.contains("include_str!(\"source_frame_data/source_manifest.json\")"));
    assert!(!generated.contains(".figatree.bin"));
    assert!(generated.contains("MotionState::AttackAirN"));
    assert!(generated.contains("AttackAirN"));
    assert!(!generated.contains("SourceHitCapsule"));
    assert!(!generated.contains("SourceHurtCapsule"));
    assert!(output_path
        .parent()
        .unwrap()
        .join("source_frame_data/source_frame_capsules.bin")
        .exists());
}

#[test]
fn frame_data_extract_decodes_source_action_script_hitbox_procedures() {
    let root = temp_project_root("frame_data_source_hitboxes");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 290}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [
                {"frame": 1, "hitboxes": [], "hurtboxes": []},
                {
                    "frame": 6,
                    "hitboxes": [{
                        "id": 0,
                        "source": "provisional_viewer_scaffold_pending_hitbox_command_extraction"
                    }],
                    "hurtboxes": []
                }
            ],
            "gaps": [{"field": "hitboxes.damage_angle_knockback", "reason": "pending source extraction"}],
            "overrides": []
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "subaction_script_offset": 19908
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x08000004u32,
        0x4c000001,
        0x08000007,
        0x2c007006,
        0x044c05dc,
        0x00000000,
        0x29190513,
        0x0000008b,
        0x2c806805,
        0x05780320,
        0x00000000,
        0x27190513,
        0x0000008b,
        0x04000006,
        0x40000000,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let artifact = &parsed["artifact"];

    assert_eq!(parsed["ok"], true);
    assert_eq!(
        artifact["decoded_action_script"]["source"]["subaction_script_offset"],
        19908
    );
    let spawn_hitbox_procedure = artifact["decoded_action_script"]["procedures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|procedure| procedure["procedure"] == "fighter.spawn_hitbox")
        .unwrap();
    assert_eq!(spawn_hitbox_procedure["handler"], "ftAction_8007121C");
    assert_eq!(
        artifact["summary"]["active_hitbox_windows"][0],
        json!({"start": 7, "end": 12, "source": "decoded_action_script"})
    );
    let frame_7 = artifact["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["frame"] == 7)
        .unwrap();
    assert_eq!(frame_7["hitboxes"].as_array().unwrap().len(), 2);
    assert_eq!(frame_7["hitboxes"][0]["id"], 0);
    assert_eq!(frame_7["hitboxes"][0]["bone"], 14);
    assert_eq!(frame_7["hitboxes"][0]["damage"], 6);
    assert_eq!(frame_7["hitboxes"][0]["angle"], 82);
    assert_eq!(frame_7["hitboxes"][0]["kbg"], 100);
    assert_eq!(frame_7["hitboxes"][0]["bkb"], 0);
    assert_eq!(frame_7["hitboxes"][0]["radius"], 4.296875);
    assert_eq!(frame_7["hitboxes"][0]["center"]["x"], 0.0);
    assert_eq!(frame_7["hitboxes"][0]["center"]["y"], 0.0);
    assert_eq!(frame_7["hitboxes"][0]["center"]["z"], 0.0);
    assert_eq!(frame_7["hitboxes"][0]["source_center"]["x"], 5.859375);
    assert_eq!(frame_7["hitboxes"][0]["source_center"]["z"], 0.0);
    assert!(artifact["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|gap| gap["field"] != "hitboxes.damage_angle_knockback"));
    assert!(artifact["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|frame| frame["hitboxes"].as_array().unwrap())
        .all(|hitbox| hitbox["source"]
            != "provisional_viewer_scaffold_pending_hitbox_command_extraction"));
}

#[test]
fn frame_data_extract_write_updates_project_frame_data_artifact() {
    let root = temp_project_root("frame_data_extract_write");
    let artifact_path = root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json");
    write_json(
        &artifact_path,
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 290}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [{
                "frame": 7,
                "hitboxes": [{
                    "id": 0,
                    "source": "provisional_viewer_scaffold_pending_hitbox_command_extraction"
                }],
                "hurtboxes": []
            }],
            "gaps": [{"field": "hitboxes.damage_angle_knockback", "reason": "pending source extraction"}],
            "overrides": []
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "subaction_script_offset": 19908
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x04000007u32,
        0x2c007006,
        0x044c05dc,
        0x00000000,
        0x29190513,
        0x0000008b,
        0x04000006,
        0x40000000,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&artifact_path).unwrap()).unwrap();

    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["wrote_artifact"], true);
    assert_eq!(
        PathBuf::from(parsed["artifact_path"].as_str().unwrap()),
        artifact_path
    );
    assert_eq!(written, parsed["artifact"]);
    assert_eq!(
        written["keyframes"][0]["hitboxes"][0]["source"],
        "decoded_action_script"
    );
    assert_eq!(written["keyframes"][0]["hitboxes"][0]["damage"], 6);
    assert!(written["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|gap| gap["field"] != "hitboxes.damage_angle_knockback"));
}

#[test]
fn frame_data_extract_write_creates_missing_artifact_from_source_action_table() {
    let root = temp_project_root("frame_data_extract_create");
    let artifact_path = root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json");
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "figatree_root": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "figatree": {"frames_ticks": 45},
                "subaction_script_offset": 19908
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x04000007u32,
        0x2c007006,
        0x044c05dc,
        0x00000000,
        0x29190513,
        0x0000008b,
        0x04000006,
        0x40000000,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
        "--write".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&artifact_path).unwrap()).unwrap();

    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["created_artifact"], true);
    assert_eq!(parsed["wrote_artifact"], true);
    assert_eq!(written, parsed["artifact"]);
    assert_eq!(written["summary"]["total_frames"], 45);
    assert_eq!(
        written["summary"]["active_hitbox_windows"][0],
        json!({"start": 7, "end": 12, "source": "decoded_action_script"})
    );
    assert_eq!(written["sources"][0]["action_state_id"], 68);
    assert_eq!(written["keyframes"][0]["frame"], 7);
    assert_eq!(written["keyframes"][0]["hitboxes"][0]["damage"], 6);
    let active_frame = written["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["frame"] == 12)
        .unwrap();
    assert_eq!(
        active_frame["hitboxes"][0]["source"],
        "decoded_action_script"
    );
    assert_eq!(
        active_frame["hitboxes"][0]["source_hit_capsule_state"],
        "HitCapsule_Unk3"
    );
    assert_eq!(
        written["decoded_action_script"]["source"]["subaction_script_offset"],
        19908
    );
}

#[test]
fn frame_data_extract_transforms_hitbox_offsets_through_sampled_jobj_pose() {
    let root = temp_project_root("frame_data_source_hitbox_pose");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 290}],
            "summary": {"total_frames": 45, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [{"frame": 7, "hitboxes": [], "hurtboxes": []}],
            "gaps": [],
            "overrides": []
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "subaction_script_offset": 19908
            }]
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_hurtbox_samples.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "frames": [
                    {
                        "frame": 7,
                        "pose": {
                            "source": "HSD_JObj FigaTree sampled pose",
                            "joints": [{
                                "index": 14,
                                "parent_index": 4,
                                "world_matrix": [
                                    [1.0, 0.0, 0.0, 10.0],
                                    [0.0, 1.0, 0.0, 20.0],
                                    [0.0, 0.0, 1.0, 30.0]
                                ],
                                "world_position_raw": {"x": 10.0, "y": 20.0, "z": 30.0},
                                "world_position_milli": {"x": 10000, "y": 20000, "z": 30000}
                            }]
                        },
                        "hurtboxes": []
                    },
                    {
                        "frame": 8,
                        "pose": {
                            "source": "HSD_JObj FigaTree sampled pose",
                            "joints": [{
                                "index": 14,
                                "parent_index": 4,
                                "world_matrix": [
                                    [1.0, 0.0, 0.0, 40.0],
                                    [0.0, 1.0, 0.0, 50.0],
                                    [0.0, 0.0, 1.0, 60.0]
                                ],
                                "world_position_raw": {"x": 40.0, "y": 50.0, "z": 60.0},
                                "world_position_milli": {"x": 40000, "y": 50000, "z": 60000}
                            }]
                        },
                        "hurtboxes": []
                    }
                ]
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x04000007u32,
        0x2c007006,
        0x044c05dc,
        0x00000000,
        0x29190513,
        0x0000008b,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let frame_7 = parsed["artifact"]["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["frame"] == 7)
        .unwrap();
    let hitbox = &frame_7["hitboxes"][0];

    assert_eq!(
        hitbox["source_offset"],
        json!({"x": 5.859375, "y": 0.0, "z": 0.0})
    );
    assert_eq!(
        hitbox["source_center"],
        json!({"x": 15.859375, "y": 20.0, "z": 30.0})
    );
    assert_eq!(
        hitbox["source_previous_center"],
        json!({"x": 15.859375, "y": 20.0, "z": 30.0})
    );
    assert_eq!(hitbox["center"], json!({"x": 30.0, "y": 20.0, "z": 0.0}));
    assert_eq!(
        hitbox["previous_center"],
        json!({"x": 30.0, "y": 20.0, "z": 0.0})
    );
    assert!(hitbox["source_center"]["x"].as_f64().is_some());
    assert!(hitbox["source_offset"]["x"].as_f64().is_some());
    assert_eq!(hitbox["source_pose_joint"], 14);
    assert_eq!(hitbox["source_hit_capsule_state"], "HitCapsule_Unk2");
    assert_eq!(
        hitbox["source_sweep"],
        "ftColl_8007AD18 x58(previous) -> x4C(current)"
    );
    assert_eq!(
        hitbox["source_space"],
        "ftAction_8007121C JObj local offset transformed by sampled pose"
    );
    let frame_8 = parsed["artifact"]["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["frame"] == 8)
        .unwrap();
    let active_hitbox = &frame_8["hitboxes"][0];
    assert_eq!(
        active_hitbox["source_offset"],
        json!({"x": 5.859375, "y": 0.0, "z": 0.0})
    );
    assert_eq!(
        active_hitbox["source_center"],
        json!({"x": 45.859375, "y": 50.0, "z": 60.0})
    );
    assert_eq!(
        active_hitbox["source_previous_center"],
        json!({"x": 15.859375, "y": 20.0, "z": 30.0})
    );
    assert_eq!(
        active_hitbox["center"],
        json!({"x": 60.0, "y": 50.0, "z": 0.0})
    );
    assert_eq!(
        active_hitbox["previous_center"],
        json!({"x": 30.0, "y": 20.0, "z": 0.0})
    );
    assert_eq!(active_hitbox["source_hit_capsule_state"], "HitCapsule_Unk3");
}

#[test]
fn frame_data_extract_decodes_source_action_script_cmd_var_procedures() {
    let root = temp_project_root("frame_data_source_cmd_vars");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 290}],
            "summary": {"total_frames": 45, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [{"frame": 1, "hitboxes": [], "hurtboxes": []}],
            "gaps": [],
            "overrides": []
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "subaction_script_offset": 19908
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x08000004u32,
        0x4c000001,
        0x1c000000,
        0x08000022,
        0x4c000000,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let decoded = parsed["artifact"]["decoded_action_script"]
        .as_object()
        .expect("source action script should decode through common Command_Execute opcodes");
    let procedures = decoded["procedures"].as_array().unwrap();

    assert!(procedures.iter().any(|procedure| {
        procedure["procedure"] == "fighter.set_cmd_var"
            && procedure["handler"] == "ftAction_80071820"
            && procedure["frame"] == 4
            && procedure["cmd_var"] == 0
            && procedure["value"] == 1
    }));
    assert!(procedures.iter().any(|procedure| {
        procedure["procedure"] == "fighter.set_cmd_var"
            && procedure["handler"] == "ftAction_80071820"
            && procedure["frame"] == 34
            && procedure["cmd_var"] == 0
            && procedure["value"] == 0
    }));
}

#[test]
fn frame_data_extract_decodes_source_jab_script_procedures() {
    let root = temp_project_root("frame_data_source_jab_procedures");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 290}],
            "summary": {"total_frames": 45, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [{"frame": 1, "hitboxes": [], "hurtboxes": []}],
            "gaps": [],
            "overrides": []
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "subaction_script_offset": 19908
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x08000005u32,
        0x74000001,
        0x08000009,
        0x74000000,
        0x0800000a,
        0x78000001,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let decoded = parsed["artifact"]["decoded_action_script"]
        .as_object()
        .expect("source action script should decode jab combo/rapid fighter commands");
    let procedures = decoded["procedures"].as_array().unwrap();

    assert!(procedures.iter().any(|procedure| {
        procedure["procedure"] == "fighter.set_jab_combo"
            && procedure["handler"] == "ftAction_80071AE8"
            && procedure["frame"] == 5
            && procedure["raw_words"] == json!(["0x74000001"])
            && procedure["disabled"] == true
    }));
    assert!(procedures.iter().any(|procedure| {
        procedure["procedure"] == "fighter.set_jab_combo"
            && procedure["handler"] == "ftAction_80071AE8"
            && procedure["frame"] == 9
            && procedure["raw_words"] == json!(["0x74000000"])
            && procedure["disabled"] == false
    }));
    assert!(procedures.iter().any(|procedure| {
        procedure["procedure"] == "fighter.set_jab_rapid"
            && procedure["handler"] == "ftAction_80071B28"
            && procedure["frame"] == 10
            && procedure["raw_words"] == json!(["0x78000001"])
            && procedure["state"] == true
    }));
}

#[test]
fn frame_data_extract_decodes_source_hurt_state_procedures() {
    let root = temp_project_root("frame_data_source_hurt_state");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [
                {"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 551},
                {
                    "kind": "generated_ecb",
                    "path": "crates/mole_core/src/generated/falcon_ecb.rs",
                    "purpose": "sampled Falcon ECB reference frames for projected hurt volume preview"
                }
            ],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [{"frame": 1, "hitboxes": [], "hurtboxes": []}],
            "gaps": [],
            "overrides": []
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "subaction_script_offset": 19908
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x04000007u32,
        (28u32 << 26) | (14u32 << 18) | 2u32,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let procedures = parsed["artifact"]["decoded_action_script"]["procedures"]
        .as_array()
        .unwrap();

    assert!(procedures.iter().any(|procedure| {
        procedure["procedure"] == "fighter.set_hurt_state"
            && procedure["handler"] == "ftAction_80071A9C"
            && procedure["bone_idx"] == 14
            && procedure["state"] == "Intangible"
            && procedure["state_raw"] == 2
    }));
}

#[test]
fn frame_data_extract_uses_source_hurtbox_samples_instead_of_preview() {
    let root = temp_project_root("frame_data_source_hurtboxes");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/ftaction.c", "line": 551}],
            "summary": {
                "total_frames": 35,
                "iasa_frame": "unknown",
                "active_hurtbox_windows": [{"start": 1, "end": 6, "source": "generated_ecb_preview"}]
            },
            "keyframes": [
                {
                    "frame": 1,
                    "hitboxes": [],
                    "hurtboxes": [{"id": "preview", "source": "generated_ecb_preview"}]
                },
                {
                    "frame": 6,
                    "hitboxes": [],
                    "hurtboxes": [{"id": "preview", "source": "generated_ecb_preview"}]
                },
                {"frame": 7, "hitboxes": [], "hurtboxes": []}
            ],
            "gaps": [{"field": "hurtboxes", "reason": "pending source extraction"}],
            "overrides": []
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_animation_table.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "subaction_script_offset": 19908
            }]
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_hurtbox_samples.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "name": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "frames": [
                    {"frame": 1, "hurtboxes": [source_hurtbox_fixture(4, "HurtCapsule_Enabled")]},
                    {"frame": 6, "hurtboxes": [source_hurtbox_fixture(4, "HurtCapsule_Enabled")]},
                    {"frame": 7, "hurtboxes": [source_hurtbox_fixture(14, "HurtCapsule_Enabled")]},
                    {"frame": 45, "hurtboxes": [source_hurtbox_fixture(4, "HurtCapsule_Enabled")]}
                ]
            }]
        }),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_action_ecb_samples.json"),
        &json!({
            "actions": [{
                "action_state_id": 68,
                "figatree_root": "PlyCaptain5K_Share_ACTION_AttackAirN_figatree",
                "frames": [
                    source_ecb_frame_fixture(0),
                    source_ecb_frame_fixture(5),
                    source_ecb_frame_fixture(6),
                    source_ecb_frame_fixture(44)
                ]
            }]
        }),
    );
    let script_start = 0x20 + 19908;
    let script_words = [
        0x04000007u32,
        (28u32 << 26) | (14u32 << 18) | 2u32,
        0x00000000,
    ];
    let mut plca = vec![0u8; script_start + script_words.len() * 4];
    for (index, word) in script_words.iter().enumerate() {
        plca[script_start + index * 4..script_start + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    let raw_path = root.join("resources/melee/raw/PlCa.dat");
    fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    fs::write(&raw_path, plca).unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let artifact = &parsed["artifact"];
    let frame_7 = artifact["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["frame"] == 7)
        .unwrap();
    let hurtboxes = frame_7["hurtboxes"].as_array().unwrap();

    assert_eq!(parsed["ok"], true);
    assert!(!hurtboxes.is_empty());
    assert!(artifact["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|frame| frame["hurtboxes"].as_array().unwrap())
        .all(|hurtbox| hurtbox["source"] != "generated_ecb_preview"));
    assert_eq!(hurtboxes[0]["confidence"], "source_extracted");
    assert_eq!(hurtboxes[0]["a"]["x"], 3.25);
    assert_eq!(hurtboxes[0]["b"]["x"], -1.5);
    assert_eq!(hurtboxes[0]["a_pos"]["x"], 3.25);
    assert_eq!(hurtboxes[0]["b_pos"]["x"], -1.5);
    assert!(hurtboxes[0]["source_a"].is_object());
    assert!(hurtboxes[0]["source_b"].is_object());
    assert_eq!(hurtboxes[0]["a"]["z"], 0.0);
    assert_eq!(hurtboxes[0]["b"]["z"], 0.0);
    assert_eq!(hurtboxes[0]["source_a"]["z"], 3.25);
    assert_eq!(hurtboxes[0]["source_b"]["z"], -1.5);
    assert_eq!(hurtboxes[0]["state"], "Intangible");
    assert_eq!(hurtboxes[0]["source_handler"], "ftAction_80071A9C");
    assert_eq!(hurtboxes[0]["source_word_offset"], 1);
    assert!(artifact["keyframes"][0]["hurtboxes"].is_array());
    assert!(artifact["keyframes"][0]["body_volumes"].is_array());
    assert_eq!(
        frame_7["body_volumes"][0]["source"],
        "ftData.x44 + PlCaAJ FigaTree + PlCaNr JObj skeleton"
    );
    assert_eq!(frame_7["body_volumes"][0]["top"]["z"], 0.0);
    assert_eq!(frame_7["body_volumes"][0]["source_top"]["z"], 0.0);
    assert_eq!(
        frame_7["body_volumes"][0]["source_render_transform"],
        "ftPartSetRotY(TopN, M_PI_2 * fp->facing_dir)"
    );
    assert!(frame_7["body_volumes"][0].get("render_points").is_none());
    assert_eq!(
        frame_7["body_volumes"][0]["source_joint_indices"],
        json!([39, 47, 25, 14, 8, 4])
    );
    let sources = artifact["sources"].as_array().unwrap();
    assert!(sources
        .iter()
        .all(|source| source["kind"] != "generated_ecb"));
    assert!(sources
        .iter()
        .any(|source| source["kind"] == "extracted_hurtbox_samples"));
    assert!(sources
        .iter()
        .any(|source| source["kind"] == "extracted_body_volume_samples"));
    assert_eq!(
        artifact["summary"]["active_hurtbox_windows"][0],
        json!({"start": 1, "end": 45, "source": "source_hurtbox_samples"})
    );
    assert_eq!(artifact["summary"]["total_frames"], 45);
    assert!(artifact["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|gap| gap["field"] != "hurtboxes"));
}

#[test]
fn frame_data_show_reads_existing_artifact_without_source_character_flag() {
    let root = temp_project_root("frame_data_show");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c", "line": 1}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [],
            "gaps": [],
            "overrides": []
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "show".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data show");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["artifact"]["label"], "Neutral Air");
}

#[test]
fn frame_data_extract_reports_missing_character_state_without_guessing() {
    let root = temp_project_root("frame_data_missing");
    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "test_character_2".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data extract");
    assert_eq!(parsed["ok"], false);
    assert!(parsed["errors"][0]
        .as_str()
        .unwrap()
        .contains("move frame data artifact not found"));
    assert!(parsed["artifact"].is_null());
}

#[test]
fn frame_data_show_markdown_summarizes_sources_keyframes_and_gaps() {
    let root = temp_project_root("frame_data_markdown");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c", "line": 1}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": [{"start": 6, "end": 9}]},
            "keyframes": [{
                "frame": 6,
                "hitboxes": [{"id": 0, "center": {"x": 3.5, "y": 8.0, "z": -1.25}}],
                "hurtboxes": [],
                "body_volumes": [{"id": "ecb", "source": "ftData.x44 + PlCaAJ FigaTree + PlCaNr JObj skeleton"}]
            }],
            "gaps": [{"field": "iasa_frame", "reason": "not yet proven"}],
            "overrides": []
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "show".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
    ])
    .unwrap();

    assert!(output.starts_with("# Mole Frame Data"));
    assert!(output.contains("Dolphin Mole"));
    assert!(output.contains("Captain Falcon"));
    assert!(output.contains("Neutral Air"));
    assert!(output.contains("preserve_and_project"));
    assert!(output.contains("Frame 6"));
    assert!(output.contains("1 body volume(s)"));
    assert!(output.contains("src/melee/ft/chara/ftCommon/ftCo_AttackAir.c:1"));
    assert!(output.contains("iasa_frame"));
}

#[test]
fn run_cli_defaults_to_json_for_ai_consumers() {
    let output = run_cli(&["help".to_string()]).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["ai_contract"]["read_only_by_default"], true);
    assert_eq!(parsed["ai_contract"]["no_interactive_prompts"], true);
}

#[test]
fn package_friend_playtest_dry_run_reports_closed_handoff_artifacts() {
    let root = temp_project_root("package_friend_playtest_dry_run");
    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "package".to_string(),
        "friend-playtest".to_string(),
        "--dry-run".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "package friend-playtest");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["dry_run"], true);
    assert_eq!(parsed["verify"], true);
    assert!(parsed["build_command"]
        .as_array()
        .unwrap()
        .contains(&json!("tools/package_friend_playtest.ps1")));
    assert!(parsed["artifacts"]["playtest_exe"]
        .as_str()
        .unwrap()
        .ends_with("playtest\\MoleGame-FriendPlaytest.exe"));
}

#[test]
fn package_friend_playtest_default_launchers_are_lean_and_trace_is_explicit() {
    let script = fs::read_to_string(workspace_root().join("tools/package_friend_playtest.ps1"))
        .expect("friend playtest package script should be readable");
    fn launcher_block<'a>(script: &'a str, launcher: &str) -> &'a str {
        let marker =
            format!("'@ | Set-Content -LiteralPath (Join-Path $packageRoot \"{launcher}\")");
        let end = script
            .find(&marker)
            .unwrap_or_else(|| panic!("missing launcher block for {launcher}"));
        let start = script[..end]
            .rfind("@'")
            .unwrap_or_else(|| panic!("missing here-string start for {launcher}"));
        &script[start..end]
    }

    assert!(!launcher_block(&script, "Run Mole Game.cmd").contains("--input-trace"));
    assert!(!launcher_block(&script, "Run Local Practice.cmd").contains("--input-trace"));
    assert!(launcher_block(&script, "Run Mole Game Trace.cmd").contains("--input-trace"));
    assert!(launcher_block(&script, "Run Local Practice Trace.cmd").contains("--input-trace"));
}

#[test]
fn package_local_internet_playtest_dry_run_reports_secondary_launcher_artifacts() {
    let root = temp_project_root("package_local_internet_playtest_dry_run");
    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "package".to_string(),
        "local-internet-playtest".to_string(),
        "--dry-run".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "package local-internet-playtest");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["dry_run"], true);
    assert!(parsed["artifacts"]["playtest_exe"]
        .as_str()
        .unwrap()
        .ends_with("playtest\\MoleGame-LocalInternetPlaytest.exe"));
    assert!(parsed["build_command"]
        .as_array()
        .unwrap()
        .contains(&json!("tools/package_friend_playtest.ps1")));
}

#[test]
fn friend_connect_status_reports_role_controller_and_package_contract() {
    let root = temp_project_root("friend_connect_status");
    fs::create_dir_all(root.join("playtest")).unwrap();
    fs::write(
        root.join("playtest/MoleGame-FriendPlaytest.exe"),
        b"placeholder",
    )
    .unwrap();
    fs::write(
        root.join("playtest/MoleGame-LocalInternetPlaytest.exe"),
        b"placeholder",
    )
    .unwrap();

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "friend-connect".to_string(),
        "status".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "friend-connect status");
    assert_eq!(parsed["mutated"], false);
    assert_eq!(parsed["package_ready"], true);
    assert_eq!(parsed["role_contract"]["host"]["player_slot"], 1);
    assert_eq!(parsed["role_contract"]["host"]["network_index"], 0);
    assert_eq!(parsed["role_contract"]["host"]["can_start_match"], true);
    assert_eq!(parsed["role_contract"]["joiner"]["player_slot"], 2);
    assert_eq!(parsed["role_contract"]["joiner"]["network_index"], 1);
    assert_eq!(parsed["role_contract"]["joiner"]["can_start_match"], false);
    assert_eq!(parsed["controller_contract"]["active_at_launch"], false);
    assert!(parsed["controller_contract"]["activation_rule"]
        .as_str()
        .unwrap()
        .contains("non-neutral gameplay input"));
    assert!(parsed["controller_contract"]["extra_local_controllers"]
        .as_str()
        .unwrap()
        .contains("future local doubles"));
    assert_eq!(parsed["netplay_contract"]["default_input_delay_frames"], 2);
    assert_eq!(
        parsed["netplay_contract"]["manual_delay_override"],
        "--netplay-delay N"
    );
    assert!(parsed["netplay_contract"]["slippi_delay_model"]
        .as_str()
        .unwrap()
        .contains("F + delay"));
    assert!(parsed["netplay_contract"]["initial_delay_pads"]
        .as_str()
        .unwrap()
        .contains("neutral"));
    assert_eq!(parsed["netplay_contract"]["rollback_window_frames"], 7);
    assert_eq!(
        parsed["netplay_contract"]["recent_input_retransmit_frames"],
        8
    );
    assert!(parsed["netplay_contract"]["recent_input_datagram"]
        .as_str()
        .unwrap()
        .contains("one bundled datagram"));
    assert!(parsed["netplay_contract"]["ack_pruning"]
        .as_str()
        .unwrap()
        .contains("frame < minAckFrame"));
    assert!(parsed["netplay_contract"]["remote_receive_head"]
        .as_str()
        .unwrap()
        .contains("packetNewestFrame - headFrame"));
    assert!(parsed["netplay_contract"]["time_offset_sampling"]
        .as_str()
        .unwrap()
        .contains("one CalcTimeOffsetUs-style timing sample"));
    assert!(parsed["netplay_contract"]["remote_lookahead_stall"]
        .as_str()
        .unwrap()
        .contains("does not advance"));
    assert!(parsed["netplay_contract"]["slippi_time_sync"]
        .as_str()
        .unwrap()
        .contains("CalcTimeOffsetUs"));
    assert!(parsed["netplay_contract"]["slippi_dynamic_pacing"]
        .as_str()
        .unwrap()
        .contains("m_EmulationSpeed"));
    assert!(parsed["netplay_contract"]["match_frame_epoch"]
        .as_str()
        .unwrap()
        .contains("match frame 0"));
    assert!(parsed["netplay_contract"]["late_remote_input"]
        .as_str()
        .unwrap()
        .contains("rollback confirmation"));
    assert_eq!(
        parsed["solo_internet_test_contract"]["mode"],
        "visible-host-plus-visible-peer"
    );
    assert!(
        parsed["solo_internet_test_contract"]["visible_peer_command"]
            .as_str()
            .unwrap()
            .contains("--friend-connect --play")
    );
    assert!(
        parsed["solo_internet_test_contract"]["visible_peer_command"]
            .as_str()
            .unwrap()
            .contains("--connect-code")
    );
    assert!(
        parsed["solo_internet_test_contract"]["visible_peer_command"]
            .as_str()
            .unwrap()
            .contains("--friend-local-udp 127.0.0.1:41002")
    );
    assert!(
        !parsed["solo_internet_test_contract"]["visible_host_command"]
            .as_str()
            .unwrap()
            .contains("--input-trace")
    );
    assert!(
        !parsed["solo_internet_test_contract"]["visible_peer_command"]
            .as_str()
            .unwrap()
            .contains("--input-trace")
    );
    assert_eq!(
        parsed["solo_internet_test_contract"]["gameplay_transport"],
        "direct UDP input packets over explicit loopback ports for same-machine visual testing"
    );
    assert!(parsed["solo_internet_test_contract"]["performance_readout"]
        .as_str()
        .unwrap()
        .contains("CPU"));
    assert!(parsed["solo_internet_test_contract"]["log_policy"]
        .as_str()
        .unwrap()
        .contains("5 MB"));
    assert_eq!(parsed["artifacts"]["playtest_exe"]["exists"], true);
    assert_eq!(
        parsed["artifacts"]["local_internet_playtest_exe"]["exists"],
        true
    );
    assert!(parsed["build_command"]
        .as_array()
        .unwrap()
        .contains(&json!("friend-playtest")));
}

#[test]
fn help_command_exposes_full_agent_command_catalog() {
    let output = run_cli(&["help".to_string()]).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let commands = parsed["commands"].as_array().unwrap();
    let command_names = commands
        .iter()
        .filter_map(|command| command["name"].as_str())
        .collect::<Vec<_>>();

    for expected in [
        "status",
        "parity",
        "parity gaps",
        "parity snapshot",
        "snapshot",
        "agent brief",
        "graph missing",
        "graph next",
        "graph inspect",
        "verify changed",
        "generated check",
        "stage inspect",
        "finish check",
        "replay check",
        "frame-data extract",
        "frame-data export-runtime",
        "frame-data show",
        "package friend-playtest",
        "package local-internet-playtest",
        "friend-connect status",
        "decomp search",
        "decomp show",
        "decomp symbol",
        "doctor",
        "tests",
        "handoff",
        "recommend-next",
        "request list",
        "request next",
        "request add",
        "request done",
        "help",
    ] {
        assert!(
            command_names.contains(&expected),
            "help output should document {expected}"
        );
    }

    let request_add = commands
        .iter()
        .find(|command| command["name"] == "request add")
        .unwrap();
    assert_eq!(request_add["mutates_workspace"], true);
    assert_eq!(request_add["writes"][0], "MOLE_CLI_AGENT_MESSAGES.md");
    assert!(request_add["required_flags"]
        .as_array()
        .unwrap()
        .contains(&json!("--title")));

    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example.as_str().unwrap().contains("request next --json")));
    let replay_check = commands
        .iter()
        .find(|command| command["name"] == "replay check")
        .unwrap();
    assert_eq!(replay_check["mutates_workspace"], true);
    assert!(replay_check["writes"]
        .as_array()
        .unwrap()
        .contains(&json!("debug/slippi/*.core.report.md")));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example.as_str().unwrap().contains("replay check")));
    let replay_trace = commands
        .iter()
        .find(|command| command["name"] == "replay trace")
        .unwrap();
    assert_eq!(replay_trace["mutates_workspace"], false);
    assert!(replay_trace["purpose"]
        .as_str()
        .unwrap()
        .contains("trace window"));
    assert!(replay_trace["optional_flags"]
        .as_array()
        .unwrap()
        .contains(&json!("--player")));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example.as_str().unwrap().contains("replay trace")));
    let decomp_search = commands
        .iter()
        .find(|command| command["name"] == "decomp search")
        .unwrap();
    assert_eq!(decomp_search["mutates_workspace"], false);
    assert!(decomp_search["purpose"]
        .as_str()
        .unwrap()
        .contains("decompiled Melee"));
    assert!(decomp_search["agent_notes"]
        .as_str()
        .unwrap()
        .contains("replay"));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example.as_str().unwrap().contains("decomp search")));
    let frame_data_extract = commands
        .iter()
        .find(|command| command["name"] == "frame-data extract")
        .unwrap();
    assert_eq!(frame_data_extract["mutates_workspace"], true);
    assert!(frame_data_extract["purpose"]
        .as_str()
        .unwrap()
        .contains("move frame data"));
    assert!(frame_data_extract["optional_flags"]
        .as_array()
        .unwrap()
        .contains(&json!("--write")));
    assert!(frame_data_extract["writes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|path| path
            .as_str()
            .unwrap()
            .contains("resources/melee/frame_data")));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example.as_str().unwrap().contains("frame-data extract")));
    let frame_data_export_runtime = commands
        .iter()
        .find(|command| command["name"] == "frame-data export-runtime")
        .unwrap();
    assert_eq!(frame_data_export_runtime["mutates_workspace"], true);
    assert!(frame_data_export_runtime["purpose"]
        .as_str()
        .unwrap()
        .contains(
            "source_frame_capsules.bin action/frame capsule and DownBound hip-pose sidecar consumed by player runtime"
        ));
    assert!(frame_data_export_runtime["writes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|path| path.as_str().unwrap().contains("source_frame_data")));
    assert!(frame_data_export_runtime["optional_flags"]
        .as_array()
        .unwrap()
        .contains(&json!("--output")));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example
            .as_str()
            .unwrap()
            .contains("frame-data export-runtime")));
    let graph_next = commands
        .iter()
        .find(|command| command["name"] == "graph next")
        .unwrap();
    assert!(graph_next["output_modes"]
        .as_array()
        .unwrap()
        .contains(&json!("markdown")));
    assert!(graph_next["purpose"]
        .as_str()
        .unwrap()
        .contains("score reasons"));
    let graph_inspect = commands
        .iter()
        .find(|command| command["name"] == "graph inspect")
        .unwrap();
    assert!(graph_inspect["output_modes"]
        .as_array()
        .unwrap()
        .contains(&json!("markdown")));
    assert!(graph_inspect["purpose"]
        .as_str()
        .unwrap()
        .contains("audit checklist"));
    let verify_changed = commands
        .iter()
        .find(|command| command["name"] == "verify changed")
        .unwrap();
    assert!(verify_changed["output_modes"]
        .as_array()
        .unwrap()
        .contains(&json!("markdown")));
    assert!(verify_changed["purpose"].as_str().unwrap().contains("why"));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| {
            example
                .as_str()
                .unwrap()
                .contains("verify changed --format markdown")
        }));
    let generated_check = commands
        .iter()
        .find(|command| command["name"] == "generated check")
        .unwrap();
    assert!(generated_check["output_modes"]
        .as_array()
        .unwrap()
        .contains(&json!("markdown")));
    assert!(generated_check["purpose"]
        .as_str()
        .unwrap()
        .contains("generated artifacts"));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| { example.as_str().unwrap().contains("generated check --json") }));
    let package_friend_playtest = commands
        .iter()
        .find(|command| command["name"] == "package friend-playtest")
        .unwrap();
    assert_eq!(package_friend_playtest["mutates_workspace"], true);
    assert!(package_friend_playtest["purpose"]
        .as_str()
        .unwrap()
        .contains("one-file Windows Friend Connect playtest"));
    assert!(package_friend_playtest["writes"]
        .as_array()
        .unwrap()
        .contains(&json!("playtest/MoleGame-FriendPlaytest.exe")));
    assert!(package_friend_playtest["optional_flags"]
        .as_array()
        .unwrap()
        .contains(&json!("--dry-run")));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example
            .as_str()
            .unwrap()
            .contains("package friend-playtest")));
    let package_local_internet_playtest = commands
        .iter()
        .find(|command| command["name"] == "package local-internet-playtest")
        .unwrap();
    assert_eq!(package_local_internet_playtest["mutates_workspace"], true);
    assert!(package_local_internet_playtest["purpose"]
        .as_str()
        .unwrap()
        .contains("secondary one-file Windows launcher"));
    assert!(package_local_internet_playtest["writes"]
        .as_array()
        .unwrap()
        .contains(&json!("playtest/MoleGame-LocalInternetPlaytest.exe")));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| {
            example
                .as_str()
                .unwrap()
                .contains("package local-internet-playtest")
        }));
    let friend_connect_status = commands
        .iter()
        .find(|command| command["name"] == "friend-connect status")
        .unwrap();
    assert_eq!(friend_connect_status["mutates_workspace"], false);
    assert!(friend_connect_status["purpose"]
        .as_str()
        .unwrap()
        .contains("controller activation"));
    assert!(friend_connect_status["agent_notes"]
        .as_str()
        .unwrap()
        .contains("host/code owner is P1"));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| example.as_str().unwrap().contains("friend-connect status")));
    let finish_check = commands
        .iter()
        .find(|command| command["name"] == "finish check")
        .unwrap();
    assert!(finish_check["output_modes"]
        .as_array()
        .unwrap()
        .contains(&json!("markdown")));
    assert!(finish_check["purpose"]
        .as_str()
        .unwrap()
        .contains("completion gate"));
    assert!(parsed["examples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|example| { example.as_str().unwrap().contains("finish check --json") }));
    let tests_command = commands
        .iter()
        .find(|command| command["name"] == "tests")
        .unwrap();
    assert!(tests_command["agent_notes"]
        .as_str()
        .unwrap()
        .contains("one test-name filter"));
    assert!(tests_command["usage"]
        .as_str()
        .unwrap()
        .contains("tests run"));
    assert_eq!(parsed["global_flags"]["--root"], "Project root override.");
}

#[test]
fn tests_command_notes_cargo_filter_limit() {
    let output = run_cli(&["tests".to_string()]).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert!(parsed["notes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|note| note.as_str().unwrap().contains("one test-name filter")));
}

#[test]
fn tests_run_dry_run_expands_multiple_filters_into_separate_cargo_invocations() {
    let output = run_cli(&[
        "tests".to_string(),
        "run".to_string(),
        "-p".to_string(),
        "mole_core".to_string(),
        "--dry-run".to_string(),
        "stage_blast_zone_ko_loses_stock_and_respawns_from_stage_spawn".to_string(),
        "final_blast_zone_ko_clears_player_state_and_restores_through_rollback".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "tests run");
    assert_eq!(parsed["dry_run"], true);
    assert_eq!(parsed["ok"], true);
    assert_eq!(
        parsed["commands"][0],
        "cargo test --target-dir target/mole-cli-test-runner -p mole_core stage_blast_zone_ko_loses_stock_and_respawns_from_stage_spawn"
    );
    assert_eq!(
        parsed["commands"][1],
        "cargo test --target-dir target/mole-cli-test-runner -p mole_core final_blast_zone_ko_clears_player_state_and_restores_through_rollback"
    );
    assert!(parsed["notes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|note| note.as_str().unwrap().contains("one test-name filter")));
}

#[test]
fn finish_check_returns_read_only_completion_gate_packet() {
    let root = temp_project_root("finish_check");
    fs::create_dir_all(root.join("docs/state_graphs")).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    fs::write(root.join(".gitignore"), "target/\n").unwrap();
    write_message_board(
        &root,
        "## Inbox\n\nAdd new messages here, newest at the top.\n\n## Completed Notes\n\nDone.\n",
    );
    run_git_test_command(&root, &["init"]);
    run_git_test_command(&root, &["add", ".gitignore", "Cargo.toml"]);
    run_git_test_command(
        &root,
        &[
            "-c",
            "user.name=Mole CLI",
            "-c",
            "user.email=mole@example.invalid",
            "commit",
            "-m",
            "init",
        ],
    );
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    write_json(
        &root.join("docs/state_graphs/parity_ledger_map.json"),
        &serde_json::to_value(LedgerMap::from_registry(&LedgerRegistry::roadmap())).unwrap(),
    );

    let output = run_cli(&[
        "finish".to_string(),
        "check".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "finish check");
    assert_eq!(parsed["mutated"], false);
    assert!(parsed["completion_gate"]["checks"].is_array());
    assert!(parsed["completion_gate"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "ledger map"));
    assert!(parsed["help_catalog"]["ok"].as_bool().unwrap());
    assert!(parsed["help_catalog"]["documented_commands"]
        .as_array()
        .unwrap()
        .contains(&json!("finish check")));
    assert_eq!(parsed["request_queue"]["inbox_count"], 0);
    assert!(parsed["verification"]["changed_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("Cargo.toml")));
    assert!(parsed["verification"]["commands"]
        .as_array()
        .unwrap()
        .contains(&json!("cargo test --workspace")));
    assert!(
        parsed["verification"]["command_reasons"]["cargo test --workspace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason.as_str().unwrap().contains("Cargo.toml"))
    );
}

#[test]
fn finish_check_markdown_summarizes_completion_gate() {
    let root = temp_project_root("finish_check_markdown");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    write_message_board(
        &root,
        "## Inbox\n\nAdd new messages here, newest at the top.\n\n## Completed Notes\n\nDone.\n",
    );
    run_git_test_command(&root, &["init"]);

    let output = run_cli(&[
        "finish".to_string(),
        "check".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();

    assert!(output.starts_with("# Mole Finish Check"));
    assert!(output.contains("## Completion Gate"));
    assert!(output.contains("## Verification"));
    assert!(!output.starts_with("# Mole CLI Handoff"));
}

#[test]
fn generated_check_reports_missing_and_stale_artifact_groups() {
    let root = temp_project_root("generated_check");
    let global_sheet = root.join("docs/state_graphs/value_sheets/global_common_values.json");
    let falcon_sheet = root.join("docs/state_graphs/value_sheets/captain_falcon_values.json");
    let physics_sheet = root.join("docs/state_graphs/value_sheets/physics_engine_values.json");
    let global_combat_sheet = root.join("docs/state_graphs/value_sheets/global_combat_values.json");
    let falcon_combat_sheet =
        root.join("docs/state_graphs/value_sheets/captain_falcon_combat_values.json");
    let battlefield_sheet =
        root.join("docs/state_graphs/value_sheets/battlefield_stage_values.json");
    write_json(&global_sheet, &json!({"rows": []}));
    write_json(&falcon_sheet, &json!({"rows": []}));
    write_json(&physics_sheet, &json!({"rows": []}));
    write_json(&global_combat_sheet, &json!({"rows": []}));
    write_json(&falcon_combat_sheet, &json!({"rows": []}));
    write_json(&battlefield_sheet, &json!({"rows": []}));

    thread::sleep(Duration::from_millis(50));

    write_json(
        &root.join("resources/melee/extracted/plco_common_data.json"),
        &json!({"newer": true}),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_profile.json"),
        &json!({"newer": true}),
    );
    fs::create_dir_all(root.join("crates/mole_cli/src")).unwrap();
    fs::create_dir_all(root.join("crates/mole_core/src")).unwrap();
    fs::write(
        root.join("crates/mole_cli/src/value_sheets.rs"),
        "// generator\n",
    )
    .unwrap();
    fs::write(root.join("crates/mole_core/src/stage.rs"), "// stage\n").unwrap();

    let output = run_cli(&[
        "generated".to_string(),
        "check".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let groups = parsed["artifact_groups"].as_array().unwrap();
    let value_sheets = groups
        .iter()
        .find(|group| group["id"] == "value_sheets")
        .unwrap();
    let runtime_source_frame_data = groups
        .iter()
        .find(|group| group["id"] == "runtime_source_frame_data")
        .unwrap();

    assert_eq!(parsed["command"], "generated check");
    assert_eq!(parsed["mutated"], false);
    assert_eq!(value_sheets["stale"], true);
    assert_eq!(value_sheets["missing_outputs"].as_array().unwrap().len(), 0);
    assert!(value_sheets["newer_inputs"]
        .as_array()
        .unwrap()
        .contains(&json!("resources/melee/extracted/plco_common_data.json")));
    assert!(value_sheets["outputs"].as_array().unwrap().contains(&json!(
        "docs/state_graphs/value_sheets/battlefield_stage_values.json"
    )));
    assert!(runtime_source_frame_data["recommended_command"]
        .as_str()
        .unwrap()
        .contains("source_frame_data.rs"));
    assert!(parsed["summary"]["stale_groups"].as_u64().unwrap() >= 1);
    assert!(parsed["summary"]["missing_output_groups"].as_u64().unwrap() >= 1);
}

#[test]
fn generated_check_markdown_summarizes_groups_and_commands() {
    let root = temp_project_root("generated_check_markdown");

    let output = run_cli(&[
        "generated".to_string(),
        "check".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();

    assert!(output.starts_with("# Mole Generated Check"));
    assert!(output.contains("## Artifact Groups"));
    assert!(output.contains("`value_sheets`"));
    assert!(output.contains("crates/mole_cli/src/value_sheets.rs"));
    assert!(output.contains("crates/mole_cli/src/stage_assets.rs"));
    assert!(output.contains("resources/melee/extracted/stages/battlefield_stage.json"));
    assert!(output.contains("docs/state_graphs/parity_ledger_map.json"));
    assert!(!output.starts_with("# Mole CLI Handoff"));
}

#[test]
fn generated_write_stage_asset_emits_battlefield_stage_blob() {
    let root = temp_project_root("generated_write_stage_asset");

    let output = run_cli(&[
        "generated".to_string(),
        "write-stage-asset".to_string(),
        "--stage".to_string(),
        "battlefield".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let stage_asset_path = root.join("resources/melee/extracted/stages/battlefield_stage.json");
    let stage_asset = fs::read_to_string(&stage_asset_path).unwrap();

    assert_eq!(parsed["command"], "generated write-stage-asset");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["mutated"], true);
    assert!(parsed["written_paths"].as_array().unwrap().contains(&json!(
        "resources/melee/extracted/stages/battlefield_stage.json"
    )));
    assert!(stage_asset.contains("\"stage_id\": \"battlefield\""));
    assert!(stage_asset.contains("\"stage_name\": \"Battlefield\""));
    assert!(stage_asset.contains("\"soft_platforms\""));
    assert!(stage_asset.contains("\"kind\": \"rust_baked_stage_asset_pending_grnba_dat_extract\""));
    assert!(stage_asset.contains("\"required_raw_dat\": \"resources/melee/raw/GrNBa.dat\""));
    assert!(stage_asset.contains(".research/doldecomp-melee/src/melee/gr/grbattle.c"));
    assert!(stage_asset.contains(".research/doldecomp-melee/src/melee/mp/types.h"));
}

#[test]
fn generated_write_stage_asset_prefers_raw_dat_extraction_when_available() {
    let root = temp_project_root("generated_write_stage_asset_from_raw");
    let raw_dir = root.join("resources/melee/raw");
    fs::create_dir_all(&raw_dir).unwrap();
    fs::write(raw_dir.join("GrNBa.dat"), make_stage_dat_fixture(0.5)).unwrap();

    let output = run_cli(&[
        "generated".to_string(),
        "write-stage-asset".to_string(),
        "--stage".to_string(),
        "battlefield".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let stage_asset_path = root.join("resources/melee/extracted/stages/battlefield_stage.json");
    let stage_asset: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&stage_asset_path).unwrap()).unwrap();

    assert_eq!(parsed["command"], "generated write-stage-asset");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["mutated"], true);
    assert_eq!(stage_asset["source"]["kind"], "melee_stage_dat");
    assert_eq!(stage_asset["collision"]["scale"], 0.5);
    assert_eq!(stage_asset["collision"]["line_count"], 2);
    assert_eq!(stage_asset["ledges"].as_array().unwrap().len(), 2);
    assert_eq!(stage_asset["ledges"][0]["line_index"], 0);
    assert_eq!(stage_asset["ledges"][0]["side"], "left");
    assert_eq!(stage_asset["ledges"][1]["side"], "right");
    assert_eq!(stage_asset["dynamic_collision"]["line_count"], 0);
    assert_eq!(stage_asset["camera"]["cam_bounds"]["left"], -170.0);
    assert_eq!(stage_asset["camera"]["cam_zoom_rate"], 0.0);
    assert_eq!(stage_asset["blast_zones"]["source"], "stage_info_default");
    assert_eq!(stage_asset["callbacks"]["stage_data_symbol"], "");
    assert_eq!(stage_asset["callbacks"]["callback_table_symbol"], "");
    assert_eq!(
        stage_asset["callbacks"]["struct_refs"][0],
        ".research/doldecomp-melee/src/melee/gr/types.h::StageCallbacks"
    );
    assert_eq!(
        stage_asset["map_head_object_tree"]["source_root"],
        "map_head"
    );
    assert_eq!(stage_asset["map_head_object_tree"]["entry_count"], 1);
    assert_eq!(
        stage_asset["map_head_object_tree"]["entry_count_decoded"],
        1
    );
    assert_eq!(
        stage_asset["map_head_object_tree"]["joint_count_decoded"],
        2
    );
    assert_eq!(
        stage_asset["map_head_object_tree"]["entries"][0]["index"],
        0
    );
    assert_eq!(
        stage_asset["map_head_object_tree"]["entries"][0]["joint_root_index"],
        0
    );
    assert_eq!(
        stage_asset["map_head_object_tree"]["joints"][0]["position"]["x"],
        10.0
    );
    assert_eq!(
        stage_asset["map_head_object_tree"]["joints"][1]["position"]["x"],
        4.0
    );
    assert!(!stage_asset["pending_stage_layers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry == "map_head_object_tree"));
}

#[test]
fn stage_inspect_reports_battlefield_decomp_source_gap() {
    let root = temp_project_root("stage_inspect_battlefield");
    run_cli(&[
        "generated".to_string(),
        "write-stage-asset".to_string(),
        "--stage".to_string(),
        "battlefield".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();

    let output = run_cli(&[
        "stage".to_string(),
        "inspect".to_string(),
        "--stage".to_string(),
        "battlefield".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "stage inspect");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["stage_id"], "battlefield");
    assert_eq!(parsed["decomp_parity_ready"], false);
    assert_eq!(parsed["status"], "raw_stage_dat_missing");
    assert_eq!(parsed["raw_dat"]["present"], false);
    assert_eq!(
        parsed["asset"]["source_kind"],
        "rust_baked_stage_asset_pending_grnba_dat_extract"
    );
    assert_eq!(parsed["current_surfaces"]["surface_count"], 4);
    assert!(parsed["blocking_notes"][0]
        .as_str()
        .unwrap()
        .contains("GrNBa.dat"));
    assert!(parsed["recommended_next"][0]
        .as_str()
        .unwrap()
        .contains("mole stage extract --stage battlefield --write"));
}

#[test]
fn stage_extract_emits_map_coll_data_stage_blob_from_registered_raw_dat() {
    let root = temp_project_root("stage_extract_registered");
    let raw_dir = root.join("resources/melee/raw");
    fs::create_dir_all(&raw_dir).unwrap();
    fs::write(raw_dir.join("GrNBa.dat"), make_stage_dat_fixture(0.5)).unwrap();

    let output = run_cli(&[
        "stage".to_string(),
        "extract".to_string(),
        "--stage".to_string(),
        "battlefield".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let asset_path = root.join("resources/melee/extracted/stages/battlefield_stage.json");
    let asset: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(asset_path).unwrap()).unwrap();
    let engine_stage_path = root.join("crates/mole_core/src/generated/stages.rs");
    let engine_stage_blob = fs::read_to_string(&engine_stage_path).unwrap();

    assert_eq!(parsed["command"], "stage extract");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["mutated"], true);
    assert_eq!(parsed["stage_id"], "battlefield");
    assert_eq!(parsed["raw_dat"]["path"], "resources/melee/raw/GrNBa.dat");
    assert!(parsed["written_paths"].as_array().unwrap().contains(&json!(
        "resources/melee/extracted/stages/battlefield_stage.json"
    )));
    assert!(parsed["written_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("crates/mole_core/src/generated/stages.rs")));
    assert_eq!(
        parsed["engine_blob"]["path"],
        "crates/mole_core/src/generated/stages.rs"
    );

    assert_eq!(asset["source"]["kind"], "melee_stage_dat");
    assert_eq!(asset["source"]["raw_dat"], "resources/melee/raw/GrNBa.dat");
    assert_eq!(asset["collision"]["scale"], 0.5);
    assert_eq!(asset["collision"]["vertex_count"], 4);
    assert_eq!(asset["collision"]["line_count"], 2);
    assert_eq!(asset["collision"]["joint_count"], 1);
    assert_eq!(asset["collision"]["vertices"][0]["source_x"], -10.0);
    assert_eq!(asset["collision"]["vertices"][0]["x"], -5000);
    assert_eq!(asset["collision"]["lines"][1]["kind"], "soft_floor");
    assert_eq!(asset["map_head_object_tree"]["entry_count_decoded"], 1);
    assert_eq!(asset["main_floor"]["left_x"], -5000);
    assert_eq!(asset["main_floor"]["right_x"], 5000);
    assert_eq!(asset["soft_platforms"][0]["left_x"], -2500);
    assert_eq!(asset["soft_platforms"][0]["right_x"], 2500);
    assert_eq!(asset["soft_platforms"][0]["y"], 10000);
    assert!(engine_stage_blob.contains("@generated by mole_cli stage extract"));
    assert!(engine_stage_blob.contains("BATTLEFIELD_COLLISION_SCALE: f32 = 0.5_f32;"));
    assert!(engine_stage_blob.contains("BATTLEFIELD_COLLISION_VERTICES: [StageCollisionVertex; 4]"));
    assert!(engine_stage_blob.contains("StageCollisionLineKind::SoftFloor"));
    assert!(!engine_stage_blob.contains("serde_json"));
}

#[test]
fn fighter_common_extract_writes_slot_backed_accessory_profile() {
    let root = temp_project_root("fighter_common_extract");
    let raw_dir = root.join("resources/melee/raw");
    fs::create_dir_all(&raw_dir).unwrap();
    fs::write(raw_dir.join("PlCo.dat"), make_fighter_common_dat_fixture()).unwrap();

    let output = run_cli(&[
        "fighter-common".to_string(),
        "extract".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let asset_path = root.join("resources/melee/extracted/fighter_common_accessories.json");
    let asset: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(asset_path).unwrap()).unwrap();
    let engine_path = root.join("crates/mole_core/src/generated/fighter_common.rs");
    let engine_blob = fs::read_to_string(&engine_path).unwrap();

    assert_eq!(parsed["command"], "fighter-common extract");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["mutated"], true);
    assert_eq!(parsed["source"]["root_symbol"], "ftLoadCommonData");
    assert_eq!(parsed["source"]["pointer_table_slot"], 16);
    assert_eq!(parsed["accessory"]["symbol"], "Fighter_804D6514");
    assert_eq!(
        parsed["accessory"]["joint_root_data_offset_hex"],
        "0x00015528"
    );
    assert!(parsed["written_paths"].as_array().unwrap().contains(&json!(
        "resources/melee/extracted/fighter_common_accessories.json"
    )));
    assert!(parsed["written_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("crates/mole_core/src/generated/fighter_common.rs")));

    assert_eq!(asset["source"]["raw_dat"], "resources/melee/raw/PlCo.dat");
    assert_eq!(asset["accessory"]["joint_count"], 1);
    assert_eq!(asset["accessory"]["mesh"]["primitive_count"], 1);
    assert_eq!(asset["accessory"]["mesh"]["vertex_emit_count"], 3);
    assert_eq!(asset["accessory"]["mesh"]["bounds"]["min"]["x"], -1.0);
    assert_eq!(asset["accessory"]["mesh"]["bounds"]["max"]["x"], 2.0);
    assert!(engine_blob.contains("@generated by mole_cli fighter-common extract"));
    assert!(engine_blob.contains("symbol: \"Fighter_804D6514\""));
    assert!(engine_blob.contains("joint_root_data_offset: 0x15528"));
    assert!(engine_blob.contains("x: -1.0_f32"));
    assert!(engine_blob.contains("x: 2.0_f32"));
}

#[test]
fn stage_extract_accepts_arbitrary_dat_override_for_unregistered_stage() {
    let root = temp_project_root("stage_extract_custom");
    let raw_dir = root.join("resources/melee/raw");
    fs::create_dir_all(&raw_dir).unwrap();
    fs::write(raw_dir.join("CustomStage.dat"), make_stage_dat_fixture(1.0)).unwrap();

    let output = run_cli(&[
        "stage".to_string(),
        "extract".to_string(),
        "--stage".to_string(),
        "custom-dev-stage".to_string(),
        "--stage-name".to_string(),
        "Custom Dev Stage".to_string(),
        "--dat".to_string(),
        "resources/melee/raw/CustomStage.dat".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let asset_path = root.join("resources/melee/extracted/stages/custom-dev-stage_stage.json");
    let asset: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(asset_path).unwrap()).unwrap();

    assert_eq!(parsed["command"], "stage extract");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["stage_id"], "custom-dev-stage");
    assert_eq!(asset["stage_name"], "Custom Dev Stage");
    assert_eq!(
        asset["source"]["raw_dat"],
        "resources/melee/raw/CustomStage.dat"
    );
    assert_eq!(asset["collision"]["vertices"][1]["x"], 10000);
}

#[test]
fn stage_extract_iso_competitive_writes_game_stage_assets_not_research_outputs() {
    let root = temp_project_root("stage_extract_iso_competitive");
    let iso_path = root.join("melee_fixture.iso");
    fs::write(
        &iso_path,
        make_gcm_iso_fixture(&[
            ("GrNBa.dat", make_stage_dat_fixture(0.5)),
            ("GrNLa.dat", make_stage_dat_fixture(1.0)),
            ("GrSt.dat", make_stage_dat_fixture(1.0)),
            ("GrIz.dat", make_stage_dat_fixture(1.0)),
            ("GrOp.dat", make_stage_dat_fixture(1.0)),
            ("GrPs.dat", make_stage_dat_fixture(1.0)),
            ("GrPs1.dat", make_stage_dat_fixture(1.0)),
            ("GrPs2.dat", make_stage_dat_fixture(1.0)),
            ("GrPs3.dat", make_stage_dat_fixture(1.0)),
            ("GrPs4.dat", make_stage_dat_fixture(1.0)),
        ]),
    )
    .unwrap();

    let output = run_cli(&[
        "stage".to_string(),
        "extract-iso".to_string(),
        "--iso".to_string(),
        iso_path.display().to_string(),
        "--competitive".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let battlefield_asset: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("resources/melee/extracted/stages/battlefield_stage.json"))
            .unwrap(),
    )
    .unwrap();

    assert_eq!(parsed["command"], "stage extract-iso");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["mutated"], true);
    assert_eq!(parsed["selected_stage_count"], 6);
    assert_eq!(parsed["stages"][0]["stage_id"], "battlefield");
    assert_eq!(parsed["stages"][5]["stage_id"], "pokemon-stadium");
    assert_eq!(
        parsed["stages"][5]["related_raw_dats"][3]["dat_file"],
        "GrPs4.dat"
    );
    assert!(root.join("resources/melee/raw/GrIz.dat").exists());
    assert!(!root.join(".research/comp-melee/extracted/stages").exists());
    assert!(root
        .join("resources/melee/extracted/stages/pokemon-stadium_stage.json")
        .exists());
    assert!(root
        .join("crates/mole_core/src/generated/stages.rs")
        .exists());
    assert!(parsed["written_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("crates/mole_core/src/generated/stages.rs")));
    assert!(parsed["written_paths"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_str)
        .all(|path| !path.starts_with(".research/")));
    assert!(parsed.get("research_manifest").is_none());
    assert_eq!(battlefield_asset["source"]["kind"], "melee_stage_dat");
    assert_eq!(battlefield_asset["collision"]["scale"], 0.5);
}

#[test]
fn generated_write_ledger_map_emits_the_dual_surface_registry() {
    let root = temp_project_root("generated_write_ledger_map");

    let output = run_cli(&[
        "generated".to_string(),
        "write-ledger-map".to_string(),
        "--write".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let ledger_map_path = root.join("docs/state_graphs/parity_ledger_map.json");
    let ledger_map = fs::read_to_string(&ledger_map_path).unwrap();

    assert_eq!(parsed["command"], "generated write-ledger-map");
    assert_eq!(parsed["ok"], true);
    assert!(parsed["mutated"].as_bool().unwrap());
    assert!(parsed["written_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("docs/state_graphs/parity_ledger_map.json")));
    let consumed_map = LedgerMap::load(&ledger_map_path).unwrap();
    assert_eq!(consumed_map.registry.tab_count, 11);
    assert!(consumed_map.is_dual_surface());
    assert!(ledger_map.contains("\"global_values\""));
    assert!(ledger_map.contains("\"cli_surface\""));
    assert!(ledger_map.contains("\"gui_surface\""));
    assert!(ledger_map.contains("\"action_motion_tables\""));
}

#[test]
fn devtool_ledger_returns_gui_ready_view_model() {
    let root = temp_project_root("devtool_ledger");
    write_json(
        &root.join("docs/state_graphs/parity_ledger_map.json"),
        &serde_json::to_value(LedgerMap::from_registry(&LedgerRegistry::roadmap())).unwrap(),
    );

    let output = run_cli(&[
        "devtool".to_string(),
        "ledger".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "devtool ledger");
    assert_eq!(parsed["ledger"]["registry"]["tab_count"], 11);
    assert!(parsed["ledger"]["registry"]["dual_surface"]
        .as_bool()
        .unwrap());
    assert_eq!(parsed["ledger"]["tabs"][0]["id"], "global_values");
    assert_eq!(parsed["ledger"]["tabs"][5]["id"], "stage_values");
}

#[test]
fn default_subcommand_aliases_do_not_panic() {
    let root = temp_project_root("default_aliases");
    write_message_board(
        &root,
        "## Inbox\n\nAdd new messages here, newest at the top.\n\n## Completed Notes\n\nDone.\n",
    );

    for (alias, expected_command) in [
        ("agent", "agent brief"),
        ("graph", "graph missing"),
        ("verify", "verify changed"),
        ("generated", "generated check"),
        ("finish", "finish check"),
        ("request", "request list"),
    ] {
        let output = run_cli(&[
            alias.to_string(),
            "--root".to_string(),
            root.display().to_string(),
        ])
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["command"], expected_command);
    }
}

#[test]
fn agent_brief_returns_compaction_anchor_parity_missing_graph_and_request_context() {
    let root = temp_project_root("agent_brief");
    fs::create_dir_all(root.join("docs/superpowers/plans")).unwrap();
    fs::write(
        root.join("docs/superpowers/plans/2026-05-31-human-noticeable-melee-parity-macro-plan.md"),
        "# Human-Noticeable Melee Parity Macro Plan\n\n## Compaction Anchor\n\nIf context is compacted, resume from this anchor before touching code:\n\n1. The target is human-noticeable Falcon-like movement parity on Battlefield.\n2. Do not guess mechanics.\n\n## Human-Noticeable Scope\n",
    )
    .unwrap();
    write_json(
        &root.join("docs/state_graphs/parity_reports/value_diffs.json"),
        &json!({
            "sections": {
                "global_values": {
                    "rows": [{"status": "match"}],
                    "actionable_rows": [],
                    "derived_rows": []
                },
                "test_character_values": {
                    "rows": [{"status": "match"}],
                    "actionable_rows": [],
                    "derived_rows": []
                }
            }
        }),
    );
    write_json(
        &root.join("docs/state_graphs/parity_reports/falcon_ecb_coverage.json"),
        &json!({
            "mapped_motion_state_count": 70,
            "missing_sampled_mappings": [],
            "unmapped_derived_motion_states": []
        }),
    );
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [{"id": "Dash", "status": "partial"}],
            "edges": [{"from": "RunDirect", "to": "Run", "status": "missing"}]
        }),
    );
    write_json(
        &root.join("docs/state_graphs/parity_ledger_map.json"),
        &serde_json::to_value(LedgerMap::from_registry(&LedgerRegistry::roadmap())).unwrap(),
    );
    write_message_board(
        &root,
        "## Inbox\n\n### trace-summary - Trace Summary\n\nRequest:\nAdd trace summary.\n\nContext:\nAgents need shorter logs.\n\nExpected output:\nJSON.\n\n## Completed Notes\n\nDone.\n",
    );

    let output = run_cli(&[
        "agent".to_string(),
        "brief".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "agent brief");
    assert_eq!(
        parsed["macro_plan"]["path"],
        "docs/superpowers/plans/2026-05-31-human-noticeable-melee-parity-macro-plan.md"
    );
    assert_eq!(parsed["compaction_anchor"].as_array().unwrap().len(), 2);
    assert!(parsed["compaction_anchor"][0]
        .as_str()
        .unwrap()
        .contains("human-noticeable Falcon-like movement parity"));
    assert_eq!(parsed["parity"]["value_total_rows"], 2);
    assert_eq!(parsed["ledger_map"]["registry"]["tab_count"], 11);
    assert_eq!(parsed["missing_graph"]["missing_count"], 1);
    assert_eq!(parsed["graph_next"]["ranked_count"], 2);
    assert_eq!(
        parsed["graph_next"]["ranked_entries"][0]["status"],
        "missing"
    );
    assert_eq!(parsed["request_queue"]["inbox_count"], 1);
    assert_eq!(parsed["request_queue"]["next"]["id"], "trace-summary");
    assert!(parsed["verification_commands"].as_array().unwrap().len() >= 3);
}

#[test]
fn graph_missing_lists_missing_nodes_and_edges_with_refs_and_recommendations() {
    let root = temp_project_root("graph_missing");
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [
                {
                    "id": "RunDirect",
                    "label": "RunDirect",
                    "status": "partial",
                    "notes": "State identity exists."
                },
                {
                    "id": "CliffCatch",
                    "label": "CliffCatch",
                    "status": "missing",
                    "notes": "Ledge grab source audit has not been implemented.",
                    "source_refs": [
                        {
                            "label": "Melee cliff source",
                            "path": ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_CliffCatch.c",
                            "function": "ftCo_CliffCatch_Enter"
                        }
                    ],
                    "rust_refs": [
                        {
                            "label": "Rust stage collision",
                            "path": "crates/mole_core/src/stage.rs",
                            "function": "landing_contact_for_bottom_with_floor_skip"
                        }
                    ]
                }
            ],
            "edges": [
                {
                    "from": "RunDirect",
                    "to": "Run",
                    "status": "missing",
                    "notes": "No safe source entry trace yet.",
                    "source_refs": [
                        {
                            "label": "Melee RunDirect source",
                            "path": ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Run.c",
                            "function": "ftCo_RunDirect_IASA"
                        }
                    ],
                    "rust_refs": []
                },
                {
                    "from": "Dash",
                    "to": "Run",
                    "status": "aligned",
                    "notes": "Covered."
                }
            ]
        }),
    );

    let output = run_cli(&[
        "graph".to_string(),
        "missing".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "graph missing");
    assert_eq!(parsed["mutated"], false);
    assert_eq!(parsed["missing_count"], 2);
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["edges"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["nodes"][0]["id"], "CliffCatch");
    assert_eq!(
        parsed["nodes"][0]["source_refs"][0]["function"],
        "ftCo_CliffCatch_Enter"
    );
    assert_eq!(
        parsed["nodes"][0]["rust_refs"][0]["path"],
        "crates/mole_core/src/stage.rs"
    );
    assert_eq!(parsed["edges"][0]["from"], "RunDirect");
    assert_eq!(parsed["edges"][0]["to"], "Run");
    assert!(parsed["recommended_next"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("RunDirect -> Run")));
}

#[test]
fn graph_next_ranks_missing_entries_before_high_priority_partial_entries() {
    let root = temp_project_root("graph_next");
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [
                {
                    "id": "Dash",
                    "label": "Dash",
                    "status": "partial",
                    "notes": "Moonwalk source-order work remains.",
                    "known_gaps": ["Audit gr_vel update order."]
                },
                {
                    "id": "Attack1",
                    "label": "Attack1",
                    "status": "partial",
                    "notes": "Combat placeholder."
                }
            ],
            "edges": [
                {
                    "from": "RunDirect",
                    "to": "Run",
                    "status": "missing",
                    "notes": "No source-backed handoff yet.",
                    "source_refs": [{"path": ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Run.c"}]
                },
                {
                    "from": "Dash",
                    "to": "Turn",
                    "status": "partial",
                    "notes": "Dash dance timing source-order audit remains."
                }
            ]
        }),
    );

    let output = run_cli(&[
        "graph".to_string(),
        "next".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let entries = parsed["ranked_entries"].as_array().unwrap();

    assert_eq!(parsed["command"], "graph next");
    assert_eq!(parsed["ranked_count"], 4);
    assert_eq!(parsed["omitted_count"], 0);
    assert_eq!(entries[0]["status"], "missing");
    assert_eq!(entries[0]["from"], "RunDirect");
    assert_eq!(entries[0]["to"], "Run");
    assert!(entries
        .iter()
        .any(|entry| entry["id"] == "Dash" && entry["status"] == "partial"));
    assert!(entries[0]["recommended_next"]
        .as_str()
        .unwrap()
        .contains("Source-audit"));
    assert!(entries[0]["score_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains("missing")));
}

#[test]
fn graph_next_markdown_reports_ranked_targets_instead_of_generic_handoff() {
    let root = temp_project_root("graph_next_markdown");
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [
                {
                    "id": "Dash",
                    "label": "Dash",
                    "status": "partial",
                    "notes": "Moonwalk source-order work remains."
                }
            ],
            "edges": [
                {
                    "from": "RunDirect",
                    "to": "Run",
                    "status": "missing",
                    "notes": "No source-backed handoff yet."
                }
            ]
        }),
    );

    let output = run_cli(&[
        "graph".to_string(),
        "next".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();

    assert!(output.starts_with("# Mole Graph Next"));
    assert!(output.contains("`RunDirect -> Run`"));
    assert!(output.contains("Score:"));
    assert!(!output.starts_with("# Mole CLI Handoff"));
}

#[test]
fn graph_inspect_returns_node_and_edge_details_for_targeted_agent_context() {
    let root = temp_project_root("graph_inspect");
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [
                {
                    "id": "Dash",
                    "label": "Dash",
                    "status": "partial",
                    "notes": "Dash source-order work remains.",
                    "source_refs": [{"path": ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Dash.c"}],
                    "rust_refs": [{"path": "crates/mole_core/src/sim.rs"}],
                    "value_refs": ["global_common_values.grounded_locomotion.dash_x"],
                    "known_gaps": ["Audit gr_vel update order."]
                }
            ],
            "edges": [
                {
                    "from": "Dash",
                    "to": "Run",
                    "status": "missing",
                    "notes": "No source-backed handoff yet."
                }
            ]
        }),
    );

    let node_output = run_cli(&[
        "graph".to_string(),
        "inspect".to_string(),
        "Dash".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let node: serde_json::Value = serde_json::from_str(&node_output).unwrap();

    assert_eq!(node["command"], "graph inspect");
    assert_eq!(node["found"], true);
    assert_eq!(node["target"], "Dash");
    assert_eq!(node["entry"]["kind"], "node");
    assert_eq!(node["entry"]["id"], "Dash");
    assert_eq!(
        node["entry"]["source_refs"][0]["path"],
        ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Dash.c"
    );
    assert!(node["audit_checklist"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("Compare source refs")));

    let edge_output = run_cli(&[
        "graph".to_string(),
        "inspect".to_string(),
        "Dash->Run".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let edge: serde_json::Value = serde_json::from_str(&edge_output).unwrap();

    assert_eq!(edge["found"], true);
    assert_eq!(edge["entry"]["kind"], "edge");
    assert_eq!(edge["entry"]["from"], "Dash");
    assert_eq!(edge["entry"]["to"], "Run");
}

#[test]
fn graph_inspect_markdown_reports_focused_entry_without_generic_handoff() {
    let root = temp_project_root("graph_inspect_markdown");
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [
                {
                    "id": "Dash",
                    "label": "Dash",
                    "status": "partial",
                    "notes": "Dash source-order work remains."
                }
            ],
            "edges": []
        }),
    );

    let output = run_cli(&[
        "graph".to_string(),
        "inspect".to_string(),
        "Dash".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();

    assert!(output.starts_with("# Mole Graph Inspect"));
    assert!(output.contains("`Dash`"));
    assert!(output.contains("Dash source-order work remains."));
    assert!(!output.starts_with("# Mole CLI Handoff"));
}

#[test]
fn graph_layout_reports_two_pane_canvas_contract_and_saved_positions() {
    let root = temp_project_root("graph_layout");
    let graphs_dir = root.join("docs/state_graphs");
    let config_dir = root.join("config");
    fs::create_dir_all(&graphs_dir).unwrap();
    fs::create_dir_all(&config_dir).unwrap();
    let graph = |id: &str, title: &str| {
        json!({
            "id": id,
            "title": title,
            "root": "Wait",
            "nodes": [
                {"id": "Wait", "label": "Wait", "pos": [0.0, 0.0], "status": "reference"},
                {"id": "Dash", "label": "Dash", "pos": [1.0, 0.0], "status": "partial"}
            ],
            "edges": [
                {"from": "Wait", "to": "Dash", "input": "tap", "frames": "1", "status": "partial"}
            ]
        })
    };
    write_json(
        &graphs_dir.join("melee_reference_graph.json"),
        &graph("melee_reference", "Melee Reference"),
    );
    write_json(
        &graphs_dir.join("mole_current_graph.json"),
        &graph("mole_current", "Mole Current"),
    );
    write_json(
        &config_dir.join("state_graph_layout.json"),
        &json!({
            "version": 1,
            "graphs": {
                "melee_reference": {"zoom": 0.5, "nodes": {"Wait": [3.0, 4.0]}},
                "mole_current": {"zoom": 0.75, "nodes": {"Wait": [5.0, 6.0]}}
            }
        }),
    );

    let output = run_cli(&[
        "graph".to_string(),
        "layout".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "graph layout");
    assert_eq!(parsed["mutated"], false);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["graph_count"], 2);
    assert_eq!(parsed["graphs"][0]["id"], "melee_reference");
    assert_eq!(parsed["graphs"][0]["zoom"], 0.5);
    assert_eq!(parsed["graphs"][1]["id"], "mole_current");
    assert_eq!(parsed["graphs"][1]["zoom"], 0.75);
    assert_eq!(parsed["graphs"][1]["node_count"], 2);
}

#[test]
fn graph_layout_save_check_validates_without_mutating_layout_file() {
    let root = temp_project_root("graph_layout_save_check");
    let graphs_dir = root.join("docs/state_graphs");
    let config_dir = root.join("config");
    fs::create_dir_all(&graphs_dir).unwrap();
    fs::create_dir_all(&config_dir).unwrap();
    let graph = |id: &str, title: &str| {
        json!({
            "id": id,
            "title": title,
            "root": "Wait",
            "nodes": [
                {"id": "Wait", "label": "Wait", "pos": [0.0, 0.0], "status": "reference"},
                {"id": "Dash", "label": "Dash", "pos": [1.0, 0.0], "status": "partial"}
            ],
            "edges": [
                {"from": "Wait", "to": "Dash", "input": "tap", "frames": "1", "status": "partial"}
            ]
        })
    };
    write_json(
        &graphs_dir.join("melee_reference_graph.json"),
        &graph("melee_reference", "Melee Reference"),
    );
    write_json(
        &graphs_dir.join("mole_current_graph.json"),
        &graph("mole_current", "Mole Current"),
    );
    let layout_path = config_dir.join("state_graph_layout.json");
    write_json(
        &layout_path,
        &json!({
            "version": 1,
            "graphs": {
                "melee_reference": {"zoom": 0.5, "nodes": {"Wait": [3.0, 4.0], "Dash": [4.0, 4.0]}},
                "mole_current": {"zoom": 0.75, "nodes": {"Wait": [5.0, 6.0], "Dash": [6.0, 6.0]}}
            }
        }),
    );
    let before = fs::read_to_string(&layout_path).unwrap();

    let output = run_cli(&[
        "graph".to_string(),
        "layout".to_string(),
        "save".to_string(),
        "--check".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "graph layout save");
    assert_eq!(parsed["mutated"], false);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["layout_path"], "config/state_graph_layout.json");
    assert_eq!(parsed["graph_count"], 2);
    assert!(parsed["errors"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(&layout_path).unwrap(), before);
}

#[test]
fn verify_changed_plans_small_safe_command_set_from_changed_paths() {
    let commands = verification_plan_for_changed_paths(&[
        "crates/mole_core/src/sim.rs".to_string(),
        "crates/mole_cli/src/lib.rs".to_string(),
        "tools/state_graph_viewer.py".to_string(),
        "docs/state_graphs/mole_current_graph.json".to_string(),
        "README.md".to_string(),
    ]);

    assert!(commands.contains(&"cargo test -p mole_core".to_string()));
    assert!(commands.contains(&"cargo test -p mole_cli".to_string()));
    assert!(commands
        .iter()
        .any(|command| command.contains("test_state_graph_viewer.py")));
    assert!(commands.contains(&"git diff --check".to_string()));
    assert!(!commands.contains(&"cargo test --workspace".to_string()));
}

#[test]
fn verify_changed_uses_workspace_verification_for_manifest_changes() {
    let commands = verification_plan_for_changed_paths(&[
        "Cargo.toml".to_string(),
        "Cargo.lock".to_string(),
        "crates/mole_cli/src/lib.rs".to_string(),
    ]);

    assert!(commands.contains(&"cargo test --workspace".to_string()));
    assert!(!commands.contains(&"cargo test -p mole_core".to_string()));
    assert!(!commands.contains(&"cargo test -p mole_cli".to_string()));
    assert!(commands.contains(&"git diff --check".to_string()));
}

#[test]
fn verify_changed_reports_git_files_and_verification_commands() {
    let root = temp_project_root("verify_changed");

    let output = run_cli(&[
        "verify".to_string(),
        "changed".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "verify changed");
    assert_eq!(parsed["mutated"], false);
    assert!(parsed["commands"].as_array().unwrap().len() >= 1);
    assert!(parsed["command_reasons"].is_object());
    assert!(parsed["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command.as_str().unwrap() == "git diff --check"));
}

#[test]
fn verify_changed_preserves_leading_dotfile_paths_from_git_status() {
    let root = temp_project_root("verify_dotfile");
    fs::write(root.join(".gitignore"), "target/\n").unwrap();
    run_git_test_command(&root, &["init"]);
    run_git_test_command(&root, &["add", ".gitignore"]);
    run_git_test_command(
        &root,
        &[
            "-c",
            "user.name=Mole CLI",
            "-c",
            "user.email=mole@example.invalid",
            "commit",
            "-m",
            "init",
        ],
    );
    fs::write(root.join(".gitignore"), "target/\n*.tmp\n").unwrap();

    let output = run_cli(&[
        "verify".to_string(),
        "changed".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert!(parsed["changed_paths"]
        .as_array()
        .unwrap()
        .contains(&json!(".gitignore")));
    assert!(!parsed["changed_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("gitignore")));
}

#[test]
fn verify_changed_explains_selected_commands_for_manifest_changes() {
    let root = temp_project_root("verify_reasons");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    run_git_test_command(&root, &["init"]);
    run_git_test_command(&root, &["add", "Cargo.toml"]);
    run_git_test_command(
        &root,
        &[
            "-c",
            "user.name=Mole CLI",
            "-c",
            "user.email=mole@example.invalid",
            "commit",
            "-m",
            "init",
        ],
    );
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/mole_cli\"]\n",
    )
    .unwrap();

    let output = run_cli(&[
        "verify".to_string(),
        "changed".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert!(parsed["commands"]
        .as_array()
        .unwrap()
        .contains(&json!("cargo test --workspace")));
    assert!(parsed["command_reasons"]["cargo test --workspace"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains("Cargo.toml")));
}

#[test]
fn verify_changed_markdown_reports_paths_commands_and_reasons() {
    let root = temp_project_root("verify_markdown");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    run_git_test_command(&root, &["init"]);
    run_git_test_command(&root, &["add", "Cargo.toml"]);
    run_git_test_command(
        &root,
        &[
            "-c",
            "user.name=Mole CLI",
            "-c",
            "user.email=mole@example.invalid",
            "commit",
            "-m",
            "init",
        ],
    );
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/mole_cli\"]\n",
    )
    .unwrap();

    let output = run_cli(&[
        "verify".to_string(),
        "changed".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();

    assert!(output.starts_with("# Mole Verify Changed"));
    assert!(output.contains("`Cargo.toml`"));
    assert!(output.contains("`cargo test --workspace`"));
    assert!(output.contains("Workspace Cargo manifest"));
    assert!(!output.starts_with("# Mole CLI Handoff"));
}

#[test]
fn replay_check_reads_exported_slippi_inputs_and_reports_first_mismatch() {
    let root = temp_project_root("replay_check");
    let export_path = root.join("debug/slippi/fixture.inputs.json");
    write_json(
        &export_path,
        &json!({
            "schema_version": 1,
            "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
            "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
            "metadata": {"last_frame": 0},
            "export": {
                "first_frame": 0,
                "last_frame": 0,
                "frame_count": 1,
                "included_negative_frames": false
            },
            "frames": [
                {"frame": 0, "players": {"0": {
                    "pre": {
                        "action_state_id": 14,
                        "position": [0.0, 0.0],
                        "facing": 1.0,
                        "rust_player_input": {
                            "stick_x": 0,
                            "stick_y": 0,
                            "c_stick_x": 0,
                            "c_stick_y": 0,
                            "left_trigger": 0,
                            "right_trigger": 0,
                            "physical_button_bits": 0,
                            "ucf_dashback_amendment": false
                        }
                    },
                    "post": {
                        "action_state_id": 20,
                        "position": [0.0, 0.0],
                        "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
                    }
                }}}
            ]
        }),
    );

    let output = run_cli(&[
        "replay".to_string(),
        "check".to_string(),
        "--inputs".to_string(),
        export_path.display().to_string(),
        "--mode".to_string(),
        "seeded".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "replay check");
    assert_eq!(parsed["ok"], false);
    assert_eq!(
        parsed["source"]["input_export_path"],
        export_path.display().to_string()
    );
    assert_eq!(parsed["comparison"]["mode"], "seeded pre-frame diagnostic");
    assert_eq!(parsed["comparison"]["frames_compared"], 1);
    assert_eq!(
        parsed["comparison"]["first_state_mismatch"]["core_frame"],
        0
    );
    assert_eq!(
        parsed["comparison"]["first_state_mismatch"]["source_frame"],
        0
    );
    assert_eq!(parsed["comparison"]["first_state_mismatch"]["player"], 1);
    assert_eq!(
        parsed["comparison"]["first_state_mismatch"]["expected_motion_state"],
        "Dash"
    );
    assert_eq!(
        parsed["comparison"]["first_state_mismatch"]["actual_motion_state"],
        "Wait"
    );
    assert!(root.join("debug/slippi/fixture.core.report.md").exists());
}

#[test]
fn replay_trace_returns_match_start_trace_rows() {
    let root = temp_project_root("replay_trace");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("docs/state_graphs")).unwrap();
    fs::create_dir_all(root.join("debug/slippi")).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    let export_path = root.join("debug/slippi/fixture.inputs.json");
    write_json(
        &export_path,
        &json!({
            "source": {"replay_path": "fixture.slp"},
            "settings": {
                "players": {
                    "0": {"controller_fix": "UCF"},
                    "1": {"controller_fix": "UCF"}
                }
            },
            "frames": [
                {"frame": 0, "players": {"0": {
                    "pre": {
                        "action_state_id": 14,
                        "position": [-32.0, 0.0],
                        "facing": 1,
                        "rust_player_input": {
                            "stick_x": 102,
                            "stick_y": 0,
                            "c_stick_x": 0,
                            "c_stick_y": 0,
                            "left_trigger": 0,
                            "right_trigger": 0,
                            "physical_button_bits": 0,
                            "ucf_dashback_amendment": true
                        }
                    },
                    "post": {
                        "action_state_id": 20,
                        "position": [-30.0, 0.0],
                        "self_induced_speeds": {"ground_x": 2.0, "air_x": 2.0, "y": 0.0}
                    }
                }}}
            ]
        }),
    );

    let output = run_cli(&[
        "replay".to_string(),
        "trace".to_string(),
        "--inputs".to_string(),
        export_path.display().to_string(),
        "--player".to_string(),
        "1".to_string(),
        "--start".to_string(),
        "0".to_string(),
        "--end".to_string(),
        "0".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "replay trace");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["trace"]["player"], 1);
    assert_eq!(parsed["trace"]["rows"][0]["source_frame"], 0);
    assert_eq!(parsed["trace"]["rows"][0]["input"]["stick_x"], 102);
    assert_eq!(
        parsed["trace"]["rows"][0]["input"]["ucf_dashback_amendment"],
        true
    );
    assert_eq!(parsed["trace"]["rows"][0]["expected_motion_state"], "Dash");
    assert!(parsed["trace"]["rows"][0]["actual_motion_frame"].is_u64());
}

#[test]
fn replay_artifacts_lists_input_exports_and_replay_files() {
    let root = temp_project_root("replay_artifacts");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("docs/state_graphs")).unwrap();
    fs::create_dir_all(root.join("debug/slippi")).unwrap();
    fs::create_dir_all(root.join("replays")).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    write_json(
        &root.join("debug/slippi/fixture.inputs.json"),
        &json!({
            "source": {"replay_path": "replays/fixture.slp"},
            "export": {
                "first_frame": -123,
                "last_frame": 42,
                "frame_count": 166,
                "requested_frame_limit": 1800,
                "included_negative_frames": true
            },
            "metadata": {
                "played_on": "dolphin",
                "start_at": "2026-06-10T00:00:00Z",
                "last_frame": 42
            },
            "settings": {"stage_id": 31},
            "frames": []
        }),
    );
    fs::write(root.join("replays/fixture.slp"), b"slp").unwrap();

    let output = run_cli(&[
        "replay".to_string(),
        "artifacts".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "replay artifacts");
    assert_eq!(parsed["ok"], true);
    assert_eq!(
        parsed["inputs"][0]["path"],
        "debug/slippi/fixture.inputs.json"
    );
    assert_eq!(
        parsed["inputs"][0]["source_replay_path"],
        "replays/fixture.slp"
    );
    assert_eq!(parsed["inputs"][0]["frame_count"], 166);
    assert_eq!(parsed["inputs"][0]["stage_id"], 31);
    assert_eq!(parsed["replays"][0]["path"], "replays/fixture.slp");
}

#[test]
fn snapshot_command_returns_compact_agent_parity_context() {
    let root = temp_project_root("snapshot");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("docs/state_graphs")).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    write_json(
        &root.join("docs/state_graphs/parity_ledger_map.json"),
        &serde_json::to_value(LedgerMap::from_registry(&LedgerRegistry::roadmap())).unwrap(),
    );
    write_json(
        &root.join("docs/state_graphs/parity_reports/value_diffs.json"),
        &json!({
            "sections": {
                "global_values": {
                    "rows": [{"status": "match"}, {"status": "diff"}],
                    "actionable_rows": [{"field": "dash_x"}],
                    "derived_rows": []
                },
                "test_character_values": {
                    "rows": [{"status": "match"}],
                    "actionable_rows": [],
                    "derived_rows": [{"field": "max_jumps"}]
                }
            }
        }),
    );
    write_json(
        &root.join("docs/state_graphs/parity_reports/falcon_ecb_coverage.json"),
        &json!({
            "mapped_motion_state_count": 56,
            "missing_sampled_mappings": [],
            "unmapped_derived_motion_states": ["KneeBend"]
        }),
    );
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [{"status": "partial"}],
            "edges": [{"status": "missing"}]
        }),
    );

    let output = run_cli(&[
        "parity".to_string(),
        "snapshot".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "parity snapshot");
    assert_eq!(parsed["value_counts"]["total_rows"], 3);
    assert_eq!(parsed["value_counts"]["actionable"], 1);
    assert_eq!(parsed["value_counts"]["derived"], 1);
    assert_eq!(parsed["falcon_ecb"]["mapped_motion_states"], 56);
    assert_eq!(
        parsed["falcon_ecb"]["unmapped_derived_states"][0],
        "KneeBend"
    );
    assert_eq!(parsed["ledger_map"]["registry"]["tab_count"], 11);
    assert!(parsed["ledger_map"]["registry"]["dual_surface"]
        .as_bool()
        .unwrap());
    assert!(parsed["generated_artifacts"].as_array().unwrap().len() >= 3);
    assert!(parsed["verification_commands"].as_array().unwrap().len() >= 3);
}

#[test]
fn parity_report_includes_owned_ledger_map_summary() {
    let root = temp_project_root("parity_ledger_map");
    write_json(
        &root.join("docs/state_graphs/parity_ledger_map.json"),
        &serde_json::to_value(LedgerMap::from_registry(&LedgerRegistry::roadmap())).unwrap(),
    );

    let output = run_cli(&[
        "parity".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "parity");
    assert_eq!(parsed["ledger_map"]["registry"]["tab_count"], 11);
    assert_eq!(parsed["ledger_map"]["registry"]["active_tab_count"], 6);
    assert_eq!(parsed["ledger_map"]["registry"]["planned_tab_count"], 5);
    assert!(parsed["ledger_map"]["registry"]["dual_surface"]
        .as_bool()
        .unwrap());
}

#[test]
fn parity_gaps_reports_planned_surfaces_and_partial_graph_entries() {
    let root = temp_project_root("parity_gaps");
    write_json(
        &root.join("docs/state_graphs/parity_ledger_map.json"),
        &serde_json::to_value(LedgerMap::from_registry(&LedgerRegistry::roadmap())).unwrap(),
    );
    write_json(
        &root.join("docs/state_graphs/mole_current_graph.json"),
        &json!({
            "nodes": [
                {
                    "id": "GuardReflect",
                    "label": "GuardReflect",
                    "status": "partial",
                    "notes": "Shield-hit reflect behavior is still missing.",
                    "known_gaps": ["Shield-hit reflect behavior is still missing."]
                },
                {"id": "Wait", "label": "Wait", "status": "aligned"}
            ],
            "edges": [
                {
                    "from": "GuardReflect",
                    "to": "Guard",
                    "status": "partial",
                    "notes": "GuardReflect exit remains partial."
                }
            ]
        }),
    );

    let output = run_cli(&[
        "parity".to_string(),
        "gaps".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "parity gaps");
    assert_eq!(parsed["mutated"], false);
    assert_eq!(parsed["summary"]["planned_surface_count"], 5);
    assert_eq!(parsed["summary"]["partial_graph_count"], 2);
    assert_eq!(parsed["planned_surfaces"][0]["id"], "action_motion_tables");
    assert_eq!(parsed["partial_graph_entries"][0]["id"], "GuardReflect");
    assert_eq!(
        parsed["partial_graph_entries"][1]["id"],
        "GuardReflect -> Guard"
    );
    assert!(parsed["recommended_next"][0]
        .as_str()
        .unwrap()
        .contains("action_motion_tables"));
}

#[test]
fn request_add_writes_feature_request_to_message_board() {
    let root = temp_project_root("request_add");
    write_message_board(
        &root,
        "## Inbox\n\nAdd new messages here, newest at the top.\n\n## Completed Notes\n\nDone.\n",
    );

    let output = run_cli(&[
        "request".to_string(),
        "add".to_string(),
        "--title".to_string(),
        "Trace Summary".to_string(),
        "--request".to_string(),
        "Add a compact trace summary command.".to_string(),
        "--context".to_string(),
        "Agents need shorter input logs.".to_string(),
        "--expected".to_string(),
        "JSON summary.".to_string(),
        "--id".to_string(),
        "trace-summary".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let board = fs::read_to_string(root.join("MOLE_CLI_AGENT_MESSAGES.md")).unwrap();

    assert_eq!(parsed["command"], "request add");
    assert_eq!(parsed["request"]["id"], "trace-summary");
    assert!(parsed["mutated"].as_bool().unwrap());
    assert!(board.contains("### trace-summary - Trace Summary"));
    assert!(board.contains("Add a compact trace summary command."));
}

#[test]
fn request_next_returns_newest_inbox_item_as_json() {
    let root = temp_project_root("request_next");
    write_message_board(
        &root,
        "## Inbox\n\nAdd new messages here, newest at the top.\n\n### newest - Newest Feature\n\nRequest:\nBuild newest.\n\nContext:\nCurrent context.\n\nExpected output:\nJSON.\n\n### older - Older Feature\n\nRequest:\nBuild older.\n\n## Completed Notes\n\nDone.\n",
    );

    let output = run_cli(&[
        "request".to_string(),
        "next".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "request next");
    assert_eq!(parsed["request"]["id"], "newest");
    assert_eq!(parsed["request"]["title"], "Newest Feature");
    assert_eq!(parsed["request"]["request"], "Build newest.");
}

#[test]
fn request_done_moves_inbox_item_to_completed_notes() {
    let root = temp_project_root("request_done");
    write_message_board(
        &root,
        "## Inbox\n\nAdd new messages here, newest at the top.\n\n### trace-summary - Trace Summary\n\nRequest:\nAdd summary.\n\nContext:\nAgent context.\n\nExpected output:\nJSON.\n\n## Completed Notes\n\nDone.\n",
    );

    let output = run_cli(&[
        "request".to_string(),
        "done".to_string(),
        "--id".to_string(),
        "trace-summary".to_string(),
        "--result".to_string(),
        "Implemented request workflow.".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let board = fs::read_to_string(root.join("MOLE_CLI_AGENT_MESSAGES.md")).unwrap();
    let inbox = board.split("## Completed Notes").next().unwrap();

    assert_eq!(parsed["command"], "request done");
    assert_eq!(parsed["request"]["id"], "trace-summary");
    assert!(parsed["mutated"].as_bool().unwrap());
    assert!(!inbox.contains("trace-summary - Trace Summary"));
    assert!(board.contains("Result:\nImplemented request workflow."));
}

#[test]
fn request_workflow_preserves_inbox_template_code_fence() {
    let root = temp_project_root("request_template");
    write_message_board(
        &root,
        "## Inbox\n\nAdd new messages here, newest at the top.\n\n```markdown\n### YYYY-MM-DD - Short Title\n\nRequest:\n\nContext:\n\nExpected output:\n```\n\n## Completed Notes\n\nDone.\n",
    );

    run_cli(&[
        "request".to_string(),
        "add".to_string(),
        "--id".to_string(),
        "template-safe".to_string(),
        "--title".to_string(),
        "Template Safe".to_string(),
        "--request".to_string(),
        "Keep the template intact.".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();
    let board_after_add = fs::read_to_string(root.join("MOLE_CLI_AGENT_MESSAGES.md")).unwrap();
    assert!(
        board_after_add
            .find("### template-safe - Template Safe")
            .unwrap()
            < board_after_add
                .find("```markdown\n### YYYY-MM-DD - Short Title")
                .unwrap()
    );

    run_cli(&[
        "request".to_string(),
        "done".to_string(),
        "--id".to_string(),
        "template-safe".to_string(),
        "--result".to_string(),
        "Done.".to_string(),
        "--root".to_string(),
        root.display().to_string(),
    ])
    .unwrap();

    let board = fs::read_to_string(root.join("MOLE_CLI_AGENT_MESSAGES.md")).unwrap();
    let inbox = board.split("## Completed Notes").next().unwrap();

    assert!(inbox.contains("```markdown\n### YYYY-MM-DD - Short Title"));
    assert!(!board.contains("Expected output:\n\n```markdown"));
    assert!(board.contains("Result:\nDone."));
}

fn temp_project_root(label: &str) -> PathBuf {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("mole_cli_{label}_{id}"));
    fs::create_dir_all(root.join("docs/state_graphs/parity_reports")).unwrap();
    root
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn write_json(path: &Path, value: &serde_json::Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn write_message_board(root: &Path, text: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("MOLE_CLI_AGENT_MESSAGES.md"), text).unwrap();
}

fn make_stage_dat_fixture(scale: f32) -> Vec<u8> {
    let mut data_block = vec![0_u8; 0x1C0];
    let coll_offset = 0x00;
    let vertices_offset = 0x40;
    let lines_offset = 0x60;
    let joints_offset = 0x90;
    let ground_param_offset = 0xC0;
    let map_head_offset = 0xD0;
    let map_head_models_offset = 0xE0;
    let map_head_root_node_offset = 0x110;
    let map_head_child_node_offset = 0x150;

    put_u32_be(&mut data_block, coll_offset, vertices_offset as u32);
    put_u32_be(&mut data_block, coll_offset + 0x04, 4);
    put_u32_be(&mut data_block, coll_offset + 0x08, lines_offset as u32);
    put_u32_be(&mut data_block, coll_offset + 0x0C, 2);
    put_i16_be(&mut data_block, coll_offset + 0x10, 0);
    put_i16_be(&mut data_block, coll_offset + 0x12, 2);
    put_u32_be(&mut data_block, coll_offset + 0x24, joints_offset as u32);
    put_u32_be(&mut data_block, coll_offset + 0x28, 1);

    put_vec2_be(&mut data_block, vertices_offset, -10.0, 0.0);
    put_vec2_be(&mut data_block, vertices_offset + 0x08, 10.0, 0.0);
    put_vec2_be(&mut data_block, vertices_offset + 0x10, -5.0, 20.0);
    put_vec2_be(&mut data_block, vertices_offset + 0x18, 5.0, 20.0);

    put_map_line_be(&mut data_block, lines_offset, 0, 1, 1, 0);
    put_map_line_be(&mut data_block, lines_offset + 0x10, 2, 3, 1, 0x100);

    put_i16_be(&mut data_block, joints_offset, 0);
    put_i16_be(&mut data_block, joints_offset + 0x02, 2);
    put_f32_be(&mut data_block, joints_offset + 0x14, -10.0);
    put_f32_be(&mut data_block, joints_offset + 0x18, 0.0);
    put_f32_be(&mut data_block, joints_offset + 0x1C, 10.0);
    put_f32_be(&mut data_block, joints_offset + 0x20, 20.0);
    put_i16_be(&mut data_block, joints_offset + 0x24, 0);
    put_i16_be(&mut data_block, joints_offset + 0x26, 4);

    put_f32_be(&mut data_block, ground_param_offset, scale);
    put_u32_be(
        &mut data_block,
        map_head_offset,
        map_head_models_offset as u32,
    );
    put_u32_be(
        &mut data_block,
        map_head_offset + 0x08,
        map_head_models_offset as u32,
    );
    put_u32_be(&mut data_block, map_head_offset + 0x0C, 1);
    put_u32_be(
        &mut data_block,
        map_head_models_offset,
        map_head_root_node_offset as u32,
    );

    put_u32_be(&mut data_block, map_head_root_node_offset + 0x04, 1);
    put_u32_be(
        &mut data_block,
        map_head_root_node_offset + 0x08,
        map_head_child_node_offset as u32,
    );
    put_f32_be(&mut data_block, map_head_root_node_offset + 0x20, 1.0);
    put_f32_be(&mut data_block, map_head_root_node_offset + 0x24, 1.0);
    put_f32_be(&mut data_block, map_head_root_node_offset + 0x28, 1.0);
    put_f32_be(&mut data_block, map_head_root_node_offset + 0x2C, 10.0);
    put_f32_be(&mut data_block, map_head_root_node_offset + 0x30, 20.0);

    put_u32_be(&mut data_block, map_head_child_node_offset + 0x04, 2);
    put_f32_be(&mut data_block, map_head_child_node_offset + 0x20, 1.0);
    put_f32_be(&mut data_block, map_head_child_node_offset + 0x24, 1.0);
    put_f32_be(&mut data_block, map_head_child_node_offset + 0x28, 1.0);
    put_f32_be(&mut data_block, map_head_child_node_offset + 0x2C, 4.0);
    put_f32_be(&mut data_block, map_head_child_node_offset + 0x30, 8.0);

    make_stage_dat_with_roots(
        &[
            ("coll_data", coll_offset as u32),
            ("grGroundParam", ground_param_offset as u32),
            ("map_head", map_head_offset as u32),
        ],
        data_block,
    )
}

fn make_fighter_common_dat_fixture() -> Vec<u8> {
    let mut data_block = vec![0_u8; 0x15600];
    let root_offset = 0xECD8;
    let vertices_offset = 0x11700;
    let vtx_desc_offset = 0x14590;
    let display_offset = 0x14600;
    let pobj_offset = 0x15500;
    let dobj_offset = 0x15518;
    let joint_offset = 0x15528;

    put_u32_be(&mut data_block, root_offset + 16 * 4, joint_offset as u32);

    put_i16_be(&mut data_block, vertices_offset, -4096);
    put_i16_be(&mut data_block, vertices_offset + 2, -8192);
    put_i16_be(&mut data_block, vertices_offset + 4, 0);
    put_i16_be(&mut data_block, vertices_offset + 6, 0);
    put_i16_be(&mut data_block, vertices_offset + 8, 0);
    put_i16_be(&mut data_block, vertices_offset + 10, 0);
    put_i16_be(&mut data_block, vertices_offset + 12, 8192);
    put_i16_be(&mut data_block, vertices_offset + 14, 4096);
    put_i16_be(&mut data_block, vertices_offset + 16, 4096);

    put_u32_be(&mut data_block, vtx_desc_offset, 9);
    put_u32_be(&mut data_block, vtx_desc_offset + 0x04, 2);
    put_u32_be(&mut data_block, vtx_desc_offset + 0x08, 1);
    put_u32_be(&mut data_block, vtx_desc_offset + 0x0C, 3);
    data_block[vtx_desc_offset + 0x10] = 12;
    put_u16_be(&mut data_block, vtx_desc_offset + 0x12, 6);
    put_u32_be(
        &mut data_block,
        vtx_desc_offset + 0x14,
        vertices_offset as u32,
    );
    put_u32_be(&mut data_block, vtx_desc_offset + 0x18, 0xFF);

    data_block[display_offset] = 0x90;
    put_u16_be(&mut data_block, display_offset + 1, 3);
    data_block[display_offset + 3] = 0;
    data_block[display_offset + 4] = 1;
    data_block[display_offset + 5] = 2;

    put_u32_be(&mut data_block, pobj_offset + 0x08, vtx_desc_offset as u32);
    put_u16_be(&mut data_block, pobj_offset + 0x0C, 0x8000);
    put_u16_be(&mut data_block, pobj_offset + 0x0E, 1);
    put_u32_be(&mut data_block, pobj_offset + 0x10, display_offset as u32);

    put_u32_be(&mut data_block, dobj_offset + 0x0C, pobj_offset as u32);

    put_u32_be(&mut data_block, joint_offset + 0x04, 0x10050188);
    put_u32_be(&mut data_block, joint_offset + 0x10, dobj_offset as u32);
    put_f32_be(&mut data_block, joint_offset + 0x20, 1.0);
    put_f32_be(&mut data_block, joint_offset + 0x24, 1.0);
    put_f32_be(&mut data_block, joint_offset + 0x28, 1.0);
    put_f32_be(&mut data_block, joint_offset + 0x30, 1.515542984008789);

    make_stage_dat_with_roots(&[("ftLoadCommonData", root_offset as u32)], data_block)
}

fn make_stage_dat_with_roots(roots: &[(&str, u32)], data_block: Vec<u8>) -> Vec<u8> {
    let mut dat = vec![0_u8; 0x20];
    put_u32_be(&mut dat, 0x04, data_block.len() as u32);
    put_u32_be(&mut dat, 0x0C, roots.len() as u32);
    dat.extend_from_slice(&data_block);

    let root_table_offset = dat.len();
    let mut string_table = Vec::new();
    for (name, offset) in roots {
        put_u32_be_at_end(&mut dat, *offset);
        put_u32_be_at_end(&mut dat, string_table.len() as u32);
        string_table.extend_from_slice(name.as_bytes());
        string_table.push(0);
    }
    debug_assert_eq!(root_table_offset, 0x20 + data_block.len());
    dat.extend_from_slice(&string_table);
    dat
}

fn make_gcm_iso_fixture(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let fst_offset = 0x500_usize;
    let entry_count = files.len() + 1;
    let entry_bytes = entry_count * 12;
    let mut name_offsets = Vec::with_capacity(files.len());
    let mut string_table = Vec::new();
    for (name, _) in files {
        name_offsets.push(string_table.len());
        string_table.extend_from_slice(name.as_bytes());
        string_table.push(0);
    }
    let fst_size = entry_bytes + string_table.len();
    let mut entries = vec![0_u8; entry_bytes];
    put_u32_be(&mut entries, 0, 0x0100_0000);
    put_u32_be(&mut entries, 0x04, 0);
    put_u32_be(&mut entries, 0x08, entry_count as u32);

    let mut data_offset = align_up(fst_offset + fst_size, 0x20);
    let mut payload_offsets = Vec::with_capacity(files.len());
    for (_, payload) in files {
        payload_offsets.push(data_offset);
        data_offset = align_up(data_offset + payload.len(), 0x20);
    }

    for (index, ((_, payload), name_offset)) in files.iter().zip(name_offsets.iter()).enumerate() {
        let entry_offset = (index + 1) * 12;
        put_u32_be(&mut entries, entry_offset, *name_offset as u32);
        put_u32_be(
            &mut entries,
            entry_offset + 0x04,
            payload_offsets[index] as u32,
        );
        put_u32_be(&mut entries, entry_offset + 0x08, payload.len() as u32);
    }

    let mut iso = vec![0_u8; data_offset];
    put_u32_be(&mut iso, 0x424, fst_offset as u32);
    put_u32_be(&mut iso, 0x428, fst_size as u32);
    iso[fst_offset..fst_offset + entry_bytes].copy_from_slice(&entries);
    iso[fst_offset + entry_bytes..fst_offset + fst_size].copy_from_slice(&string_table);
    for ((_, payload), offset) in files.iter().zip(payload_offsets.iter()) {
        iso[*offset..*offset + payload.len()].copy_from_slice(payload);
    }
    iso
}

fn align_up(value: usize, align: usize) -> usize {
    ((value + align - 1) / align) * align
}

fn put_map_line_be(
    data: &mut [u8],
    offset: usize,
    v0_idx: u16,
    v1_idx: u16,
    hi_flags: u16,
    lo_flags: u16,
) {
    put_u16_be(data, offset, v0_idx);
    put_u16_be(data, offset + 0x02, v1_idx);
    put_i16_be(data, offset + 0x04, -1);
    put_i16_be(data, offset + 0x06, -1);
    put_i16_be(data, offset + 0x08, -1);
    put_i16_be(data, offset + 0x0A, -1);
    put_u16_be(data, offset + 0x0C, hi_flags);
    put_u16_be(data, offset + 0x0E, lo_flags);
}

fn put_vec2_be(data: &mut [u8], offset: usize, x: f32, y: f32) {
    put_f32_be(data, offset, x);
    put_f32_be(data, offset + 0x04, y);
}

fn put_u16_be(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_i16_be(data: &mut [u8], offset: usize, value: i16) {
    data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_u32_be(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn put_u32_be_at_end(data: &mut Vec<u8>, value: u32) {
    data.extend_from_slice(&value.to_be_bytes());
}

fn put_f32_be(data: &mut [u8], offset: usize, value: f32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn run_git_test_command(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn source_hurtbox_fixture(bone: u64, state: &str) -> serde_json::Value {
    json!({
        "id": bone,
        "kind": "capsule",
        "bone": bone,
        "height": 2,
        "is_grabbable": true,
        "a": {"x": 3.25, "y": 2.0, "z": 0.0},
        "b": {"x": -1.5, "y": 5.0, "z": 0.0},
        "source_a": {"x": 1.0, "y": 2.0, "z": 3.25},
        "source_b": {"x": 4.0, "y": 5.0, "z": -1.5},
        "a_offset": {"x": 0.25, "y": 0.5, "z": 0.75},
        "b_offset": {"x": -0.25, "y": -0.5, "z": -0.75},
        "radius": 2.5,
        "scale": 2.5,
        "state": state,
    })
}

fn source_ecb_frame_fixture(frame: u64) -> serde_json::Value {
    json!({
        "frame": frame,
        "top_raw": {"x": 0.0, "y": 16.0},
        "bottom_raw": {"x": 0.0, "y": 6.0},
        "left_raw": {"x": -2.0, "y": 11.0},
        "right_raw": {"x": 2.0, "y": 11.0},
        "source_joint_indices": [39, 47, 25, 14, 8, 4],
    })
}
