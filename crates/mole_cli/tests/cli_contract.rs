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
fn run_cli_defaults_to_json_for_ai_consumers() {
    let output = run_cli(&["help".to_string()]).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["ai_contract"]["read_only_by_default"], true);
    assert_eq!(parsed["ai_contract"]["no_interactive_prompts"], true);
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
        "parity snapshot",
        "snapshot",
        "agent brief",
        "graph missing",
        "graph next",
        "graph inspect",
        "verify changed",
        "generated check",
        "finish check",
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
    assert_eq!(parsed["global_flags"]["--root"], "Project root override.");
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
    write_json(&global_sheet, &json!({"rows": []}));
    write_json(&falcon_sheet, &json!({"rows": []}));

    thread::sleep(Duration::from_millis(50));

    write_json(
        &root.join("resources/melee/extracted/plco_common_data.json"),
        &json!({"newer": true}),
    );
    write_json(
        &root.join("resources/melee/extracted/captain_falcon_profile.json"),
        &json!({"newer": true}),
    );
    fs::create_dir_all(root.join("tools")).unwrap();
    fs::write(root.join("tools/generate_value_sheets.py"), "# generator\n").unwrap();

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

    assert_eq!(parsed["command"], "generated check");
    assert_eq!(parsed["mutated"], false);
    assert_eq!(value_sheets["stale"], true);
    assert_eq!(value_sheets["missing_outputs"].as_array().unwrap().len(), 0);
    assert!(value_sheets["newer_inputs"]
        .as_array()
        .unwrap()
        .contains(&json!("resources/melee/extracted/plco_common_data.json")));
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
    assert!(output.contains("tools\\generate_value_sheets.py"));
    assert!(!output.starts_with("# Mole CLI Handoff"));
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
fn snapshot_command_returns_compact_agent_parity_context() {
    let root = temp_project_root("snapshot");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("docs/state_graphs")).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
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
    assert!(parsed["generated_artifacts"].as_array().unwrap().len() >= 3);
    assert!(parsed["verification_commands"].as_array().unwrap().len() >= 3);
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

fn write_json(path: &Path, value: &serde_json::Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn write_message_board(root: &Path, text: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("MOLE_CLI_AGENT_MESSAGES.md"), text).unwrap();
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
