use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use mole_core::{source_units_to_milli, SourceCollEcbSnapshot, SourceVec2, Vec2};
use mole_runtime::{
    compare_slippi_export_from_match_start_with_core, compare_slippi_export_with_core,
    scan_slippi_export_from_match_start_with_core, scan_slippi_export_seeded_pre_frame_with_core,
    slippi_core_report_path, trace_slippi_export_from_match_start_with_core,
    trace_slippi_export_seeded_pre_frame_with_core, write_slippi_core_report, SlippiCoreComparison,
    SlippiCoreComparisonConfig, SlippiCoreComparisonMode, SlippiCoreDivergenceKind,
    SlippiCoreDivergenceScan, SlippiCoreDivergenceScanConfig, SlippiCoreDivergenceScenario,
    SlippiCoreFirstDivergence, SlippiCoreMismatch, SlippiCorePositionDrift, SlippiCoreTrace,
    SlippiCoreTraceConfig, SlippiCoreTraceRow,
};
use serde_json::{json, Value};

use crate::{
    base_report, ReplayCheckMode, ReplayCheckOptions, ReplayCommand, ReplayExplainOptions,
    ReplayScanOptions, ReplayTraceOptions, SCHEMA_VERSION,
};

pub(crate) fn replay_report(root: &Path, command: &ReplayCommand) -> Value {
    match command {
        ReplayCommand::Artifacts => replay_artifacts_report(root),
        ReplayCommand::Check(options) => replay_check_report(root, options),
        ReplayCommand::Explain(options) => replay_explain_report(root, options),
        ReplayCommand::Scan(options) => replay_scan_report(root, options),
        ReplayCommand::Trace(options) => replay_trace_report(root, options),
    }
}

fn replay_artifacts_report(root: &Path) -> Value {
    let input_dir = root.join("debug/slippi");
    let replay_dir = root.join("replays");
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "replay artifacts",
        "project_root": root.display().to_string(),
        "ok": true,
        "inputs_dir": project_relative_path(root, &input_dir),
        "replays_dir": project_relative_path(root, &replay_dir),
        "inputs": list_input_exports(root, &input_dir),
        "replays": list_replay_files(root, &replay_dir),
        "errors": [],
    })
}

fn replay_check_report(root: &Path, options: &ReplayCheckOptions) -> Value {
    match run_replay_check(root, options) {
        Ok(report) => report,
        Err(error) => replay_error_report(root, options, error),
    }
}

fn run_replay_check(root: &Path, options: &ReplayCheckOptions) -> Result<Value, String> {
    let export = match (&options.replay, &options.inputs) {
        (Some(replay), None) => export_slippi_replay(root, options, replay)?,
        (None, Some(inputs)) => ReplayExportPaths {
            replay_path: None,
            input_export_path: resolve_project_path(root, inputs),
            export_report_path: None,
        },
        _ => return Err("replay check requires exactly one input source".to_string()),
    };

    let text = fs::read_to_string(&export.input_export_path).map_err(|error| {
        format!(
            "failed to read Slippi input export {}: {error}",
            export.input_export_path.display()
        )
    })?;
    let comparison = compare_export(&text, options)?;
    let core_report_path = write_core_report(root, &export, &comparison)?;

    Ok(replay_success_report(
        root,
        options,
        export,
        comparison,
        core_report_path,
    ))
}

fn replay_scan_report(root: &Path, options: &ReplayScanOptions) -> Value {
    match run_replay_scan(root, options) {
        Ok(report) => report,
        Err(error) => {
            let mut report = base_report("replay scan", root);
            report["ok"] = json!(false);
            report["source"] = json!({
                "input_export_path": options.inputs,
                "frames_requested": options.frames,
                "mode": replay_mode_label(options.mode),
                "lookahead_frames": options.lookahead_frames,
            });
            report["errors"] = json!([error]);
            report
        }
    }
}

fn run_replay_scan(root: &Path, options: &ReplayScanOptions) -> Result<Value, String> {
    let input_export_path = resolve_project_path(root, &options.inputs);
    let text = fs::read_to_string(&input_export_path).map_err(|error| {
        format!(
            "failed to read Slippi input export {}: {error}",
            input_export_path.display()
        )
    })?;
    let config = SlippiCoreDivergenceScanConfig {
        compare_players: [true, true],
        max_frames: options.frames,
        lookahead_frames: options.lookahead_frames,
        max_scenarios: options.max_scenarios,
        position_tolerance_milli: options.position_tolerance_milli,
        velocity_tolerance_milli: options.velocity_tolerance_milli,
    };
    let scan = match options.mode {
        ReplayCheckMode::MatchStart => scan_slippi_export_from_match_start_with_core(&text, config),
        ReplayCheckMode::Seeded => scan_slippi_export_seeded_pre_frame_with_core(&text, config),
    }
    .map_err(|error| error.to_string())?;

    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "command": "replay scan",
        "project_root": root.display().to_string(),
        "ok": !scan_has_independent_engine_divergence(&scan),
        "source": {
            "input_export_path": input_export_path.display().to_string(),
            "frames_requested": options.frames,
            "mode": replay_mode_label(options.mode),
            "lookahead_frames": options.lookahead_frames,
            "max_scenarios": options.max_scenarios,
            "position_tolerance_milli": options.position_tolerance_milli,
            "velocity_tolerance_milli": options.velocity_tolerance_milli,
        },
        "scan": divergence_scan_json(&scan),
        "errors": [],
    }))
}

fn replay_trace_report(root: &Path, options: &ReplayTraceOptions) -> Value {
    match run_replay_trace(root, options) {
        Ok(report) => report,
        Err(error) => {
            let mut report = base_report("replay trace", root);
            report["ok"] = json!(false);
            report["source"] = json!({
                "input_export_path": options.inputs,
                "frames_requested": options.frames,
            });
            report["errors"] = json!([error]);
            report
        }
    }
}

fn replay_explain_report(root: &Path, options: &ReplayExplainOptions) -> Value {
    match run_replay_explain(root, options) {
        Ok(report) => report,
        Err(error) => {
            let mut report = base_report("replay explain", root);
            report["ok"] = json!(false);
            report["source"] = json!({
                "input_export_path": options.inputs,
                "frames_requested": options.frames,
                "target_source_frame": options.source_frame,
            });
            report["errors"] = json!([error]);
            report
        }
    }
}

fn run_replay_explain(root: &Path, options: &ReplayExplainOptions) -> Result<Value, String> {
    let input_export_path = resolve_project_path(root, &options.inputs);
    let text = fs::read_to_string(&input_export_path).map_err(|error| {
        format!(
            "failed to read Slippi input export {}: {error}",
            input_export_path.display()
        )
    })?;
    let source_frame_start = options.source_frame.saturating_sub(options.window);
    let source_frame_end = options.source_frame.saturating_add(options.window);
    let trace = trace_slippi_export_from_match_start_with_core(
        &text,
        SlippiCoreTraceConfig {
            player_index: options.player_index,
            source_frame_start,
            source_frame_end,
            max_frames: Some(options.frames),
        },
    )
    .map_err(|error| error.to_string())?;

    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "command": "replay explain",
        "project_root": root.display().to_string(),
        "ok": true,
        "source": {
            "input_export_path": input_export_path.display().to_string(),
            "frames_requested": options.frames,
        },
        "explanation": {
            "player": options.player_index + 1,
            "target_source_frame": options.source_frame,
            "trace_window": {
                "source_frame_start": source_frame_start,
                "source_frame_end": source_frame_end,
            },
            "primary_reference": "decomp",
            "secondary_reference": "Slippi replay export",
            "interpretation_rule": "Do not patch Rust to match a Slippi row until the decomp tick phase and source fields at that logical point are proven.",
            "decomp_phase_model": decomp_fighter_tick_phase_model(),
            "audit_checklist": decomp_parity_audit_checklist(),
            "trace": trace_json(&trace, ReplayCheckMode::MatchStart),
        },
        "errors": [],
    }))
}

fn run_replay_trace(root: &Path, options: &ReplayTraceOptions) -> Result<Value, String> {
    let input_export_path = resolve_project_path(root, &options.inputs);
    let text = fs::read_to_string(&input_export_path).map_err(|error| {
        format!(
            "failed to read Slippi input export {}: {error}",
            input_export_path.display()
        )
    })?;
    let trace_config = SlippiCoreTraceConfig {
        player_index: options.player_index,
        source_frame_start: options.source_frame_start,
        source_frame_end: options.source_frame_end,
        max_frames: Some(options.frames),
    };
    let trace = match options.mode {
        ReplayCheckMode::MatchStart => {
            trace_slippi_export_from_match_start_with_core(&text, trace_config)
        }
        ReplayCheckMode::Seeded => {
            trace_slippi_export_seeded_pre_frame_with_core(&text, trace_config)
        }
    }
    .map_err(|error| error.to_string())?;

    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "command": "replay trace",
        "project_root": root.display().to_string(),
        "ok": true,
        "source": {
            "input_export_path": input_export_path.display().to_string(),
            "frames_requested": options.frames,
        },
        "trace": trace_json(&trace, options.mode),
        "errors": [],
    }))
}

fn decomp_fighter_tick_phase_model() -> Vec<Value> {
    vec![
        json!({
            "phase": "action physics",
            "decomp_function": "ftCo_EscapeAir_Phys",
            "rust_anchor": "EscapeAir branch before collision",
            "parity_question": "Does Rust apply escapeair_decay only while cmd_skip_decay is false, and does it avoid grounded traction before collision callback?"
        }),
        json!({
            "phase": "airborne collision",
            "decomp_function": "ftCo_EscapeAir_Coll -> ft_80082C74 -> ft_80081D0C -> mpColl_800471F8",
            "rust_anchor": "source-collision floor sweep",
            "parity_question": "Does Rust use the previous/current ECB floor sweep and mpLineIntersectionH tolerances without replay-specific platform exceptions?"
        }),
        json!({
            "phase": "landing callback",
            "decomp_function": "ftCo_80099D70 -> ftCo_LandingFallSpecial_Enter",
            "rust_anchor": "enter_landing_fall_special",
            "parity_question": "Does Rust enter LandingFallSpecial immediately when the decomp collision callback fires?"
        }),
        json!({
            "phase": "ground entry primitive",
            "decomp_function": "ftCommon_8007D7FC -> ftCommon_8007D6A4",
            "rust_anchor": "source_ft_common_8007d6a4_ground_velocity_x",
            "parity_question": "Does Rust copy the correct pre-transition self_vel.x or TransN velocity into gr_vel without changing unrelated fields?"
        }),
        json!({
            "phase": "landing physics",
            "decomp_function": "ftCo_Landing_Phys -> ft_80084F3C",
            "rust_anchor": "LandingFallSpecial branch apply_ground_traction",
            "parity_question": "Does Rust apply gr_friction, high-speed multiplier x6C, floor friction multiplier, and staged ground accel in the same order?"
        }),
        json!({
            "phase": "fighter outer commit",
            "decomp_function": "fighter.c gr_vel += xE4 + xE8; self_vel += x74_anim_vel",
            "rust_anchor": "commit_ground_velocity and grounded_position_delta_x",
            "parity_question": "Does Rust keep gr_vel, staged accel, self_vel, and public/composed velocity distinct until the decomp commit point?"
        }),
    ]
}

fn decomp_parity_audit_checklist() -> Vec<Value> {
    vec![
        json!({
            "name": "Frame phase",
            "required_reference": "decomp fighter tick order",
            "done_when": "The divergence is assigned to a specific decomp phase before any sim change."
        }),
        json!({
            "name": "Collision callback timing",
            "required_reference": "ftCo_EscapeAir_Coll, ft_80082C74, ft_80081D0C, mpColl_800471F8",
            "done_when": "Rust enters landing on the same collision callback condition as the decomp, independent of Slippi row timing."
        }),
        json!({
            "name": "Field identity",
            "required_reference": "Fighter fields gr_vel, self_vel, x74_anim_vel, xE4/xE8 ground accels",
            "done_when": "Each Rust field used in the trace maps to exactly one decomp field or is explicitly marked as a public composed/export field."
        }),
        json!({
            "name": "Float preservation",
            "required_reference": "decomp f32 fields and extracted source artifacts",
            "done_when": "No f32 field is routed through milli-integer storage before being used by physics or collision."
        }),
        json!({
            "name": "Replay sampling phase",
            "required_reference": "Slippi post-frame export fields",
            "done_when": "Any Slippi mismatch is categorized as sim mismatch, expected mixed-phase export, or unsupported source behavior with evidence."
        }),
    ]
}

fn export_slippi_replay(
    root: &Path,
    options: &ReplayCheckOptions,
    replay: &str,
) -> Result<ReplayExportPaths, String> {
    let replay_path = resolve_project_path(root, replay);
    let mut command = Command::new("node");
    command
        .current_dir(root)
        .arg("tools/slippi_replay_to_inputs.cjs")
        .arg("--replay")
        .arg(&replay_path)
        .arg("--frames")
        .arg(options.frames.to_string());
    if options.include_negative_frames {
        command.arg("--include-negative-frames");
    }

    let output = command
        .output()
        .map_err(|error| format!("failed to run Slippi replay exporter with node: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(format!(
            "Slippi replay exporter failed with status {}: {}{}",
            output.status,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\nstdout: {}", stdout.trim())
            }
        ));
    }

    let input_export_path = parse_exporter_output_path(&stdout, "wrote_json")
        .ok_or_else(|| "Slippi replay exporter did not report wrote_json".to_string())?;
    let export_report_path = parse_exporter_output_path(&stdout, "wrote_report")
        .ok_or_else(|| "Slippi replay exporter did not report wrote_report".to_string())?;

    Ok(ReplayExportPaths {
        replay_path: Some(replay_path),
        input_export_path,
        export_report_path: Some(export_report_path),
    })
}

fn parse_exporter_output_path(stdout: &str, key: &str) -> Option<PathBuf> {
    stdout.lines().find_map(|line| {
        line.trim()
            .strip_prefix(key)
            .and_then(|value| value.strip_prefix('='))
            .map(PathBuf::from)
    })
}

fn compare_export(
    text: &str,
    options: &ReplayCheckOptions,
) -> Result<SlippiCoreComparison, String> {
    let config = SlippiCoreComparisonConfig {
        compare_players: [true, true],
        max_frames: Some(options.frames),
    };
    match options.mode {
        ReplayCheckMode::MatchStart => {
            compare_slippi_export_from_match_start_with_core(text, config)
        }
        ReplayCheckMode::Seeded => compare_slippi_export_with_core(text, config),
    }
    .map_err(|error| error.to_string())
}

fn write_core_report(
    root: &Path,
    export: &ReplayExportPaths,
    comparison: &SlippiCoreComparison,
) -> Result<PathBuf, String> {
    let report_source = export
        .replay_path
        .as_deref()
        .or_else(|| comparison.source_replay_path.as_deref().map(Path::new))
        .unwrap_or(&export.input_export_path);
    let core_report_path = root.join(slippi_core_report_path(report_source));
    write_slippi_core_report(&core_report_path, comparison).map_err(|error| error.to_string())?;
    Ok(core_report_path)
}

fn replay_success_report(
    root: &Path,
    options: &ReplayCheckOptions,
    export: ReplayExportPaths,
    comparison: SlippiCoreComparison,
    core_report_path: PathBuf,
) -> Value {
    let has_state_mismatch = comparison.first_state_mismatch.is_some();
    let has_position_drift = comparison.first_position_drift.is_some();
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "replay check",
        "project_root": root.display().to_string(),
        "ok": !has_state_mismatch && !has_position_drift,
        "source": {
            "replay_path": export.replay_path.as_ref().map(|path| path.display().to_string()),
            "input_export_path": export.input_export_path.display().to_string(),
            "export_report_path": export.export_report_path.as_ref().map(|path| path.display().to_string()),
            "core_report_path": core_report_path.display().to_string(),
            "frames_requested": options.frames,
            "include_negative_frames": options.include_negative_frames,
        },
        "comparison": comparison_json(&comparison),
        "errors": [],
    })
}

fn replay_error_report(root: &Path, options: &ReplayCheckOptions, error: String) -> Value {
    let mut report = base_report("replay check", root);
    report["ok"] = json!(false);
    report["source"] = json!({
        "replay_path": options.replay.as_deref(),
        "input_export_path": options.inputs.as_deref(),
        "frames_requested": options.frames,
        "include_negative_frames": options.include_negative_frames,
        "mode": replay_mode_label(options.mode),
    });
    report["errors"] = json!([error]);
    report
}

fn comparison_json(comparison: &SlippiCoreComparison) -> Value {
    json!({
        "mode": comparison_mode_label(comparison.mode),
        "source_replay_path": comparison.source_replay_path.as_deref(),
        "frames_compared": comparison.frames_compared,
        "player_frames_compared": comparison.player_frames_compared,
        "unsupported_state_count": comparison.unsupported_state_count,
        "unsupported_states": comparison.unsupported_states.iter().map(|(state, count)| {
            json!({"state_id": state, "count": count})
        }).collect::<Vec<_>>(),
        "state_mismatch_count": comparison.state_mismatch_count,
        "max_abs_ground_velocity_diff": comparison.max_abs_ground_velocity_diff,
        "ucf_players": comparison.ucf_players,
        "ucf_dashback_amendment_frames": comparison.ucf_dashback_amendment_frames,
        "first_position_drift": position_drift_json(comparison.first_position_drift),
        "first_position_drift_by_player": comparison.first_position_drift_by_player
            .iter()
            .copied()
            .map(position_drift_json)
            .collect::<Vec<_>>(),
        "first_state_mismatch": mismatch_json(comparison.first_state_mismatch),
        "first_classified_divergence": first_divergence_json(comparison.first_divergence.clone()),
    })
}

fn first_divergence_json(divergence: Option<SlippiCoreFirstDivergence>) -> Value {
    let Some(divergence) = divergence else {
        return Value::Null;
    };
    json!({
        "kind": divergence_kind_label(divergence.kind),
        "player": divergence.player_index + 1,
        "core_frame": divergence.core_frame.0,
        "source_frame": divergence.source_frame,
    })
}

fn mismatch_json(mismatch: Option<SlippiCoreMismatch>) -> Value {
    let Some(mismatch) = mismatch else {
        return Value::Null;
    };
    json!({
        "core_frame": mismatch.frame.0,
        "source_frame": mismatch.source_frame,
        "player": mismatch.player_index + 1,
        "expected_slippi_state_id": mismatch.expected_slippi_state_id,
        "expected_action_state_id": mismatch.expected_action_state_id.get(),
        "actual_action_state_id": mismatch.actual_action_state_id.map(|state| state.get()),
        "expected_motion_state": mismatch
            .expected_motion_state
            .map(|state| format!("{state:?}")),
        "actual_motion_state": format!("{:?}", mismatch.actual_motion_state),
        "expected_position": vec2_json(mismatch.expected_position),
        "actual_position": vec2_json(mismatch.actual_position),
        "position_delta": vec2_delta_json(mismatch.actual_position, mismatch.expected_position),
        "expected_ground_velocity_x": mismatch.expected_ground_velocity_x,
        "expected_air_velocity_x": mismatch.expected_air_velocity_x,
        "expected_velocity_y": mismatch.expected_velocity_y,
        "actual_velocity_x": mismatch.actual_velocity_x,
        "actual_velocity_y": mismatch.actual_velocity_y,
        "ground_velocity_x_delta": mismatch.actual_velocity_x - mismatch.expected_ground_velocity_x,
        "air_velocity_x_delta": mismatch.actual_velocity_x - mismatch.expected_air_velocity_x,
        "velocity_y_delta": mismatch.actual_velocity_y - mismatch.expected_velocity_y,
    })
}

fn position_drift_json(drift: Option<SlippiCorePositionDrift>) -> Value {
    let Some(drift) = drift else {
        return Value::Null;
    };
    json!({
        "core_frame": drift.frame.0,
        "source_frame": drift.source_frame,
        "player": drift.player_index + 1,
        "expected_slippi_state_id": drift.expected_slippi_state_id,
        "expected_action_state_id": drift.expected_action_state_id.get(),
        "actual_action_state_id": drift.actual_action_state_id.map(|state| state.get()),
        "expected_motion_state": drift
            .expected_motion_state
            .map(|state| format!("{state:?}")),
        "actual_motion_state": format!("{:?}", drift.actual_motion_state),
        "expected_position": vec2_json(drift.expected_position),
        "actual_position": vec2_json(drift.actual_position),
        "position_delta": vec2_delta_json(drift.actual_position, drift.expected_position),
        "expected_ground_velocity_x": drift.expected_ground_velocity_x,
        "expected_air_velocity_x": drift.expected_air_velocity_x,
        "expected_velocity_y": drift.expected_velocity_y,
        "actual_velocity_x": drift.actual_velocity_x,
        "actual_velocity_y": drift.actual_velocity_y,
    })
}

fn vec2_json(value: Vec2) -> Value {
    json!({
        "x_milli": value.x,
        "y_milli": value.y,
    })
}

fn source_vec2_json(value: SourceVec2) -> Value {
    json!({
        "x": value.x,
        "y": value.y,
    })
}

fn source_coll_ecb_json(value: SourceCollEcbSnapshot) -> Value {
    json!({
        "top": source_vec2_json(value.top),
        "right": source_vec2_json(value.right),
        "bottom": source_vec2_json(value.bottom),
        "left": source_vec2_json(value.left),
    })
}

fn action_identity_json(identity: mole_runtime::SlippiActionIdentity) -> Value {
    json!({
        "melee_motion_state_id": identity.melee_motion_state_id.map(|id| id.get()),
        "source_action_table_index": identity.source_action_table_index.map(|index| index.get()),
        "source_action_key": identity.source_action_key,
    })
}

fn vec2_delta_json(actual: Vec2, expected: Vec2) -> Value {
    json!({
        "x_milli": actual.x - expected.x,
        "y_milli": actual.y - expected.y,
    })
}

fn comparison_mode_label(mode: SlippiCoreComparisonMode) -> &'static str {
    match mode {
        SlippiCoreComparisonMode::SeededPreFrameDiagnostic => "seeded pre-frame diagnostic",
        SlippiCoreComparisonMode::SequentialMatchStart => "sequential Slippi match-start",
    }
}

fn replay_mode_label(mode: ReplayCheckMode) -> &'static str {
    match mode {
        ReplayCheckMode::MatchStart => "match-start",
        ReplayCheckMode::Seeded => "seeded",
    }
}

fn trace_json(trace: &SlippiCoreTrace, mode: ReplayCheckMode) -> Value {
    json!({
        "source_replay_path": trace.source_replay_path.as_deref(),
        "mode": replay_mode_label(mode),
        "player": trace.config.player_index + 1,
        "source_frame_start": trace.config.source_frame_start,
        "source_frame_end": trace.config.source_frame_end,
        "max_frames": trace.config.max_frames,
        "rows": trace.rows.iter().map(trace_row_json).collect::<Vec<_>>(),
    })
}

fn divergence_scan_json(scan: &SlippiCoreDivergenceScan) -> Value {
    json!({
        "source_replay_path": scan.source_replay_path.as_deref(),
        "frames_scanned": scan.frames_scanned,
        "scenario_count": scan.scenarios.len(),
        "independent_scenario_count": scan.scenarios
            .iter()
            .filter(|scenario| scenario.cascades_from_source_frame.is_none())
            .count(),
        "engine_root_scenario_count": independent_engine_divergence_count(scan),
        "first_engine_root_scenario": scan
            .scenarios
            .iter()
            .find(|scenario| is_independent_engine_divergence_scenario(scenario))
            .map(divergence_scenario_summary_json),
        "lookahead_frames": scan.config.lookahead_frames,
        "position_tolerance_milli": scan.config.position_tolerance_milli,
        "velocity_tolerance_milli": scan.config.velocity_tolerance_milli,
        "scenarios": scan.scenarios
            .iter()
            .map(divergence_scenario_json)
            .collect::<Vec<_>>(),
    })
}

fn scan_has_independent_engine_divergence(scan: &SlippiCoreDivergenceScan) -> bool {
    scan.scenarios
        .iter()
        .any(is_independent_engine_divergence_scenario)
}

fn independent_engine_divergence_count(scan: &SlippiCoreDivergenceScan) -> usize {
    scan.scenarios
        .iter()
        .filter(|scenario| is_independent_engine_divergence_scenario(scenario))
        .count()
}

fn is_independent_engine_divergence_scenario(scenario: &SlippiCoreDivergenceScenario) -> bool {
    scenario.cascades_from_source_frame.is_none()
}

fn divergence_scenario_summary_json(scenario: &SlippiCoreDivergenceScenario) -> Value {
    json!({
        "scenario_index": scenario.scenario_index,
        "kind": divergence_kind_label(scenario.kind),
        "player": scenario.player_index + 1,
        "core_frame": scenario.core_frame.0,
        "source_frame": scenario.source_frame,
        "end_source_frame": scenario.end_source_frame,
        "duration_frames": scenario.duration_frames,
    })
}

fn divergence_scenario_json(scenario: &SlippiCoreDivergenceScenario) -> Value {
    json!({
        "scenario_index": scenario.scenario_index,
        "kind": divergence_kind_label(scenario.kind),
        "player": scenario.player_index + 1,
        "core_frame": scenario.core_frame.0,
        "source_frame": scenario.source_frame,
        "end_core_frame": scenario.end_core_frame.0,
        "end_source_frame": scenario.end_source_frame,
        "duration_frames": scenario.duration_frames,
        "realigned_within_lookahead": scenario.realigned_within_lookahead,
        "realign_core_frame": scenario.realign_core_frame.map(|frame| frame.0),
        "realign_source_frame": scenario.realign_source_frame,
        "cascades_from_source_frame": scenario.cascades_from_source_frame,
        "rollback_replay_deterministic": scenario.rollback_replay_deterministic,
        "max_abs_position_delta_milli": scenario.max_abs_position_delta_milli,
        "max_abs_velocity_delta_milli": scenario.max_abs_velocity_delta_milli,
        "first_frame": trace_row_json(&scenario.first_frame),
    })
}

fn divergence_kind_label(kind: SlippiCoreDivergenceKind) -> &'static str {
    match kind {
        SlippiCoreDivergenceKind::UnsupportedState => "unsupported_state",
        SlippiCoreDivergenceKind::MixedPhaseWitness => "mixed_phase_witness",
        SlippiCoreDivergenceKind::StateMismatch => "state_mismatch",
        SlippiCoreDivergenceKind::PositionDrift => "position_drift",
        SlippiCoreDivergenceKind::VelocityDrift => "velocity_drift",
    }
}

fn trace_row_json(row: &SlippiCoreTraceRow) -> Value {
    json!({
        "core_frame": row.core_frame.0,
        "source_frame": row.source_frame,
        "player": row.player_index + 1,
        "input": {
            "stick_x": row.input_stick_x,
            "stick_y": row.input_stick_y,
            "button_bits": row.input_button_bits,
            "left_trigger": row.input_left_trigger,
            "right_trigger": row.input_right_trigger,
            "ucf_dashback_amendment": row.input_ucf_dashback_amendment,
            "ucf_shield_drop_amendment": row.input_ucf_shield_drop_amendment,
            "actual_jump_pressed": row.actual_input_jump_pressed,
            "actual_normal_jump_pressed": row.actual_input_normal_jump_pressed,
            "actual_shield_held": row.actual_input_shield_held,
            "actual_shield_pressed": row.actual_input_shield_pressed,
        },
        "expected_slippi_state_id": row.expected_slippi_state_id,
        "expected_action_state_id": row.expected_action_state_id.get(),
        "actual_action_state_id": row.actual_action_state_id.map(|state| state.get()),
        "expected_action_identity": action_identity_json(row.expected_action_identity),
        "actual_action_identity": action_identity_json(row.actual_action_identity),
        "expected_motion_state": row
            .expected_motion_state
            .map(|state| format!("{state:?}")),
        "actual_motion_state": format!("{:?}", row.actual_motion_state),
        "actual_grounded": row.actual_grounded,
        "actual_motion_frame": row.actual_motion_frame,
        "actual_motion_anim_frame_milli": row.actual_motion_anim_frame_milli,
        "actual_source_fall_anim_blend": row.actual_source_fall_anim_blend,
        "actual_source_fall_anim_pose": format!("{:?}", row.actual_source_fall_anim_pose),
        "expected_facing": row.expected_facing,
        "actual_facing": row.actual_facing,
        "expected_position": vec2_json(row.expected_position),
        "actual_position": vec2_json(row.actual_position),
        "expected_source_position": source_vec2_json(row.expected_source_position),
        "actual_source_position": source_vec2_json(row.actual_source_position),
        "position_delta": vec2_delta_json(row.actual_position, row.expected_position),
        "expected_ground_velocity_x": row.expected_ground_velocity_x,
        "expected_air_velocity_x": row.expected_air_velocity_x,
        "expected_velocity_y": row.expected_velocity_y,
        "expected_attack_velocity_x": row.expected_attack_velocity_x,
        "expected_attack_velocity_y": row.expected_attack_velocity_y,
        "expected_composed_velocity_x": row.expected_composed_velocity_x,
        "expected_composed_velocity_y": row.expected_composed_velocity_y,
        "expected_ground_velocity_x_source": row.expected_ground_velocity_x_source,
        "expected_air_velocity_x_source": row.expected_air_velocity_x_source,
        "expected_velocity_y_source": row.expected_velocity_y_source,
        "expected_attack_velocity_x_source": row.expected_attack_velocity_x_source,
        "expected_attack_velocity_y_source": row.expected_attack_velocity_y_source,
        "actual_velocity_x": row.actual_velocity_x,
        "actual_velocity_y": row.actual_velocity_y,
        "actual_source_self_velocity_x": row.actual_source_self_velocity_x,
        "actual_source_self_velocity_y": row.actual_source_self_velocity_y,
        "actual_source_knockback_velocity_x": row.actual_source_knockback_velocity_x,
        "actual_source_knockback_velocity_y": row.actual_source_knockback_velocity_y,
        "actual_source_ground_knockback_velocity": row.actual_source_ground_knockback_velocity,
        "actual_player_nudge_x": row.actual_player_nudge_x,
        "actual_player_nudge_z": row.actual_player_nudge_z,
        "actual_ground_velocity_x": row.actual_ground_velocity_x,
        "actual_ground_accel_x": row.actual_ground_accel_x,
        "actual_ground_accel_x2": row.actual_ground_accel_x2,
        "actual_dash_entry_velocity_delta": row.actual_dash_entry_velocity_delta,
        "actual_dash_x0": row.actual_dash_x0,
        "actual_source_coll_last_pos": source_vec2_json(row.actual_source_coll_last_pos),
        "actual_source_coll_cur_pos": source_vec2_json(row.actual_source_coll_cur_pos),
        "actual_source_coll_prev_pos": source_vec2_json(row.actual_source_coll_prev_pos),
        "actual_source_coll_ecb": source_coll_ecb_json(row.actual_source_coll_ecb),
        "actual_source_coll_prev_ecb": source_coll_ecb_json(row.actual_source_coll_prev_ecb),
        "actual_source_coll_desired_ecb": source_coll_ecb_json(row.actual_source_coll_desired_ecb),
        "actual_ecb_bottom_lock_timer": row.actual_ecb_bottom_lock_timer,
        "actual_source_coll_x130_locked": row.actual_source_coll_x130_locked,
        "actual_floor_skip_surface": row.actual_floor_skip_surface,
        "actual_source_floor_skip_line": row.actual_source_floor_skip_line,
        "actual_source_floor_surface": row.actual_source_floor_surface,
        "actual_source_floor_line": row.actual_source_floor_line,
        "actual_source_coll_env_flags": row.actual_source_coll_env_flags,
        "actual_source_coll_prev_env_flags": row.actual_source_coll_prev_env_flags,
        "actual_ground_velocity_x_milli": source_units_to_milli(row.actual_ground_velocity_x),
        "ground_velocity_x_delta": source_units_to_milli(row.actual_ground_velocity_x) - row.expected_ground_velocity_x,
        "air_velocity_x_delta": row.actual_velocity_x - row.expected_air_velocity_x,
        "velocity_y_delta": row.actual_velocity_y - row.expected_velocity_y,
        "attack_velocity_x_delta": source_units_to_milli(row.actual_source_knockback_velocity_x) - row.expected_attack_velocity_x,
        "attack_velocity_y_delta": source_units_to_milli(row.actual_source_knockback_velocity_y) - row.expected_attack_velocity_y,
        "composed_velocity_x_delta": row.actual_velocity_x - row.expected_composed_velocity_x,
        "composed_velocity_y_delta": row.actual_velocity_y - row.expected_composed_velocity_y,
    })
}

fn resolve_project_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn list_input_exports(root: &Path, dir: &Path) -> Vec<Value> {
    let mut entries = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".inputs.json"))
        })
        .map(|path| input_export_artifact_json(root, &path))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        entry
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    entries
}

fn list_replay_files(root: &Path, dir: &Path) -> Vec<Value> {
    let mut entries = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".slp") || name.ends_with(".slp.gz"))
        })
        .map(|path| {
            let size_bytes = fs::metadata(&path).ok().map(|metadata| metadata.len());
            json!({
                "path": project_relative_path(root, &path),
                "size_bytes": size_bytes,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        entry
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    entries
}

fn input_export_artifact_json(root: &Path, path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&text).unwrap_or(Value::Null);
    json!({
        "path": project_relative_path(root, path),
        "source_replay_path": parsed.pointer("/source/replay_path").and_then(Value::as_str),
        "frame_count": parsed.pointer("/export/frame_count").and_then(Value::as_u64),
        "first_frame": parsed.pointer("/export/first_frame").and_then(Value::as_i64),
        "last_frame": parsed.pointer("/export/last_frame").and_then(Value::as_i64),
        "requested_frame_limit": parsed.pointer("/export/requested_frame_limit").and_then(Value::as_u64),
        "included_negative_frames": parsed.pointer("/export/included_negative_frames").and_then(Value::as_bool),
        "stage_id": parsed.pointer("/settings/stage_id").and_then(Value::as_u64),
        "played_on": parsed.pointer("/metadata/played_on").and_then(Value::as_str),
        "start_at": parsed.pointer("/metadata/start_at").and_then(Value::as_str),
    })
}

fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug)]
struct ReplayExportPaths {
    replay_path: Option<PathBuf>,
    input_export_path: PathBuf,
    export_report_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mole_core::{
        Frame, MeleeActionStateId, MeleeMotionStateId, MotionState, SourceActionTableIndex,
        SourceVec2,
    };
    use mole_runtime::SlippiActionIdentity;

    #[test]
    fn trace_row_json_reports_ground_delta_from_actual_ground_velocity() {
        let row = SlippiCoreTraceRow {
            core_frame: Frame(0),
            source_frame: 525,
            player_index: 0,
            input_stick_x: 0,
            input_stick_y: 0,
            input_button_bits: 0,
            input_left_trigger: 0,
            input_right_trigger: 0,
            input_ucf_dashback_amendment: false,
            input_ucf_shield_drop_amendment: false,
            actual_input_jump_pressed: false,
            actual_input_normal_jump_pressed: false,
            actual_input_shield_held: false,
            actual_input_shield_pressed: false,
            expected_slippi_state_id: 26,
            expected_action_state_id: MeleeActionStateId::new(26),
            actual_action_state_id: Some(MeleeActionStateId::new(26)),
            expected_action_identity: SlippiActionIdentity {
                melee_motion_state_id: Some(MeleeMotionStateId::new(26)),
                source_action_table_index: Some(SourceActionTableIndex::new(17)),
                source_action_key: Some("JumpB"),
            },
            actual_action_identity: SlippiActionIdentity {
                melee_motion_state_id: Some(MeleeMotionStateId::new(26)),
                source_action_table_index: Some(SourceActionTableIndex::new(17)),
                source_action_key: Some("JumpB"),
            },
            expected_motion_state: Some(MotionState::JumpB),
            actual_motion_state: MotionState::JumpB,
            actual_grounded: false,
            actual_motion_frame: 22,
            actual_motion_anim_frame_milli: 22_000,
            actual_source_fall_anim_blend: 0.0,
            actual_source_fall_anim_pose: MotionState::Fall,
            expected_facing: -1,
            actual_facing: 1,
            expected_position: Vec2 { x: 0, y: 0 },
            actual_position: Vec2 { x: 0, y: 0 },
            expected_source_position: SourceVec2 { x: 1.25, y: -2.5 },
            actual_source_position: SourceVec2 { x: 1.5, y: -2.25 },
            expected_ground_velocity_x: 0,
            expected_air_velocity_x: 50,
            expected_velocity_y: -960,
            expected_attack_velocity_x: 1_250,
            expected_attack_velocity_y: 2_500,
            expected_composed_velocity_x: 1_300,
            expected_composed_velocity_y: 1_540,
            expected_ground_velocity_x_source: 0.0,
            expected_air_velocity_x_source: 0.0495,
            expected_velocity_y_source: -0.96,
            expected_attack_velocity_x_source: 1.25,
            expected_attack_velocity_y_source: 2.5,
            actual_velocity_x: 49,
            actual_velocity_y: -960,
            actual_source_self_velocity_x: 0.04949987,
            actual_source_self_velocity_y: -0.9599999,
            actual_source_knockback_velocity_x: 1.25,
            actual_source_knockback_velocity_y: 2.5,
            actual_source_ground_knockback_velocity: 3.75,
            actual_ground_velocity_x: 0.0,
            actual_ground_accel_x: 0.0,
            actual_ground_accel_x2: 0.0,
            actual_player_nudge_x: 0.0,
            actual_player_nudge_z: 0.0,
            actual_dash_entry_velocity_delta: 0.0,
            actual_dash_x0: 0.0,
            actual_source_coll_last_pos: SourceVec2 { x: 1.0, y: 2.0 },
            actual_source_coll_cur_pos: SourceVec2 { x: 3.0, y: 4.0 },
            actual_source_coll_prev_pos: SourceVec2 { x: 5.0, y: 6.0 },
            actual_source_coll_ecb: SourceCollEcbSnapshot::default(),
            actual_source_coll_prev_ecb: SourceCollEcbSnapshot::default(),
            actual_source_coll_desired_ecb: SourceCollEcbSnapshot::default(),
            actual_ecb_bottom_lock_timer: 0,
            actual_source_coll_x130_locked: false,
            actual_floor_skip_surface: None,
            actual_source_floor_skip_line: None,
            actual_source_floor_surface: Some(0),
            actual_source_floor_line: Some(12),
            actual_source_coll_env_flags: 0x20,
            actual_source_coll_prev_env_flags: 0x10,
        };

        let json = trace_row_json(&row);

        assert_eq!(json["ground_velocity_x_delta"], 0);
        assert_eq!(json["air_velocity_x_delta"], -1);
        assert_eq!(json["expected_facing"], -1);
        assert_eq!(json["actual_facing"], 1);
        assert_eq!(
            json["expected_action_identity"]["melee_motion_state_id"],
            26
        );
        assert_eq!(
            json["expected_action_identity"]["source_action_table_index"],
            17
        );
        assert_eq!(
            json["expected_action_identity"]["source_action_key"],
            "JumpB"
        );
        assert_eq!(json["actual_action_identity"]["melee_motion_state_id"], 26);
        assert_eq!(
            json["actual_action_identity"]["source_action_table_index"],
            17
        );
        assert_eq!(json["actual_action_identity"]["source_action_key"], "JumpB");
        assert_eq!(json["expected_source_position"]["x"], 1.25);
        assert_eq!(json["actual_source_position"]["y"], -2.25);
        assert_eq!(json["actual_motion_anim_frame_milli"], 22_000);
        assert_eq!(json["actual_grounded"], false);
        assert_eq!(json["actual_source_coll_cur_pos"]["x"], 3.0);
        assert_eq!(json["actual_source_coll_env_flags"], 0x20);
        assert_eq!(json["actual_source_floor_surface"], 0);
        assert_eq!(json["actual_source_floor_line"], 12);
        assert_eq!(json["actual_source_knockback_velocity_x"], 1.25);
        assert_eq!(json["actual_source_knockback_velocity_y"], 2.5);
        assert_eq!(json["actual_source_ground_knockback_velocity"], 3.75);
        assert_eq!(json["actual_ecb_bottom_lock_timer"], 0);
        assert_eq!(json["actual_source_coll_x130_locked"], false);
    }

    #[test]
    fn scan_summary_counts_mixed_phase_witness_as_parity_root() {
        let scan = test_divergence_scan(vec![
            test_divergence_scenario(SlippiCoreDivergenceKind::MixedPhaseWitness, 1, 142, None),
            test_divergence_scenario(SlippiCoreDivergenceKind::StateMismatch, 1, 143, Some(142)),
            test_divergence_scenario(SlippiCoreDivergenceKind::PositionDrift, 1, 189, Some(142)),
        ]);

        let json = divergence_scan_json(&scan);

        assert!(scan_has_independent_engine_divergence(&scan));
        assert_eq!(json["independent_scenario_count"], 1);
        assert_eq!(json["engine_root_scenario_count"], 1);
        assert_eq!(
            json["first_engine_root_scenario"]["kind"],
            "mixed_phase_witness"
        );
        assert_eq!(json["first_engine_root_scenario"]["source_frame"], 142);
    }

    #[test]
    fn scan_summary_reports_first_independent_root_even_when_mixed() {
        let scan = test_divergence_scan(vec![
            test_divergence_scenario(SlippiCoreDivergenceKind::MixedPhaseWitness, 1, 142, None),
            test_divergence_scenario(SlippiCoreDivergenceKind::StateMismatch, 1, 189, None),
        ]);

        let json = divergence_scan_json(&scan);

        assert!(scan_has_independent_engine_divergence(&scan));
        assert_eq!(json["independent_scenario_count"], 2);
        assert_eq!(json["engine_root_scenario_count"], 2);
        assert_eq!(
            json["first_engine_root_scenario"]["kind"],
            "mixed_phase_witness"
        );
        assert_eq!(json["first_engine_root_scenario"]["source_frame"], 142);
    }

    #[test]
    fn comparison_json_reports_first_classified_divergence() {
        let mut comparison = test_comparison(SlippiCoreComparisonMode::SequentialMatchStart);
        comparison.first_divergence = Some(mole_runtime::SlippiCoreFirstDivergence {
            kind: SlippiCoreDivergenceKind::MixedPhaseWitness,
            player_index: 1,
            core_frame: Frame(2748),
            source_frame: 2625,
        });

        let json = comparison_json(&comparison);

        assert_eq!(
            json["first_classified_divergence"]["kind"],
            "mixed_phase_witness"
        );
        assert_eq!(json["first_classified_divergence"]["player"], 2);
        assert_eq!(json["first_classified_divergence"]["core_frame"], 2748);
        assert_eq!(json["first_classified_divergence"]["source_frame"], 2625);
    }

    fn test_comparison(mode: SlippiCoreComparisonMode) -> SlippiCoreComparison {
        SlippiCoreComparison {
            mode,
            source_replay_path: None,
            ucf_players: [false; mole_core::PLAYER_COUNT],
            ucf_dashback_amendment_frames: [0; mole_core::PLAYER_COUNT],
            frames_compared: 0,
            player_frames_compared: [0; mole_core::PLAYER_COUNT],
            unsupported_state_count: 0,
            unsupported_states: Vec::new(),
            state_mismatch_count: 0,
            max_abs_ground_velocity_diff: [0; mole_core::PLAYER_COUNT],
            first_position_drift: None,
            first_position_drift_by_player: [None; mole_core::PLAYER_COUNT],
            first_state_mismatch: None,
            first_divergence: None,
        }
    }

    fn test_divergence_scan(
        scenarios: Vec<SlippiCoreDivergenceScenario>,
    ) -> SlippiCoreDivergenceScan {
        SlippiCoreDivergenceScan {
            source_replay_path: None,
            config: SlippiCoreDivergenceScanConfig::default(),
            frames_scanned: 0,
            scenarios,
        }
    }

    fn test_divergence_scenario(
        kind: SlippiCoreDivergenceKind,
        player_index: usize,
        source_frame: i32,
        cascades_from_source_frame: Option<i32>,
    ) -> SlippiCoreDivergenceScenario {
        SlippiCoreDivergenceScenario {
            scenario_index: 1,
            kind,
            player_index,
            core_frame: Frame(source_frame.max(0) as u32),
            source_frame,
            end_core_frame: Frame(source_frame.max(0) as u32),
            end_source_frame: source_frame,
            duration_frames: 1,
            realigned_within_lookahead: false,
            realign_core_frame: None,
            realign_source_frame: None,
            rollback_replay_deterministic: true,
            first_frame: test_trace_row(player_index, source_frame),
            max_abs_position_delta_milli: 0,
            max_abs_velocity_delta_milli: 0,
            cascades_from_source_frame,
        }
    }

    fn test_trace_row(player_index: usize, source_frame: i32) -> SlippiCoreTraceRow {
        SlippiCoreTraceRow {
            core_frame: Frame(source_frame.max(0) as u32),
            source_frame,
            player_index,
            input_stick_x: 0,
            input_stick_y: 0,
            input_button_bits: 0,
            input_left_trigger: 0,
            input_right_trigger: 0,
            input_ucf_dashback_amendment: false,
            input_ucf_shield_drop_amendment: false,
            actual_input_jump_pressed: false,
            actual_input_normal_jump_pressed: false,
            actual_input_shield_held: false,
            actual_input_shield_pressed: false,
            expected_slippi_state_id: 29,
            expected_action_state_id: MeleeActionStateId::new(29),
            actual_action_state_id: Some(MeleeActionStateId::new(29)),
            expected_action_identity: SlippiActionIdentity {
                melee_motion_state_id: Some(MeleeMotionStateId::new(29)),
                source_action_table_index: Some(SourceActionTableIndex::new(20)),
                source_action_key: Some("Fall"),
            },
            actual_action_identity: SlippiActionIdentity {
                melee_motion_state_id: Some(MeleeMotionStateId::new(29)),
                source_action_table_index: Some(SourceActionTableIndex::new(20)),
                source_action_key: Some("Fall"),
            },
            expected_motion_state: Some(MotionState::Fall),
            actual_motion_state: MotionState::Fall,
            actual_grounded: false,
            actual_motion_frame: 0,
            actual_motion_anim_frame_milli: 0,
            actual_source_fall_anim_blend: 0.0,
            actual_source_fall_anim_pose: MotionState::Fall,
            expected_facing: 1,
            actual_facing: 1,
            expected_position: Vec2 { x: 0, y: 0 },
            actual_position: Vec2 { x: 0, y: 0 },
            expected_source_position: SourceVec2 { x: 0.0, y: 0.0 },
            actual_source_position: SourceVec2 { x: 0.0, y: 0.0 },
            expected_ground_velocity_x: 0,
            expected_air_velocity_x: 0,
            expected_velocity_y: 0,
            expected_attack_velocity_x: 0,
            expected_attack_velocity_y: 0,
            expected_composed_velocity_x: 0,
            expected_composed_velocity_y: 0,
            expected_ground_velocity_x_source: 0.0,
            expected_air_velocity_x_source: 0.0,
            expected_velocity_y_source: 0.0,
            expected_attack_velocity_x_source: 0.0,
            expected_attack_velocity_y_source: 0.0,
            actual_velocity_x: 0,
            actual_velocity_y: 0,
            actual_source_self_velocity_x: 0.0,
            actual_source_self_velocity_y: 0.0,
            actual_source_knockback_velocity_x: 0.0,
            actual_source_knockback_velocity_y: 0.0,
            actual_source_ground_knockback_velocity: 0.0,
            actual_ground_velocity_x: 0.0,
            actual_ground_accel_x: 0.0,
            actual_ground_accel_x2: 0.0,
            actual_player_nudge_x: 0.0,
            actual_player_nudge_z: 0.0,
            actual_dash_entry_velocity_delta: 0.0,
            actual_dash_x0: 0.0,
            actual_source_coll_last_pos: SourceVec2 { x: 0.0, y: 0.0 },
            actual_source_coll_cur_pos: SourceVec2 { x: 0.0, y: 0.0 },
            actual_source_coll_prev_pos: SourceVec2 { x: 0.0, y: 0.0 },
            actual_source_coll_ecb: SourceCollEcbSnapshot::default(),
            actual_source_coll_prev_ecb: SourceCollEcbSnapshot::default(),
            actual_source_coll_desired_ecb: SourceCollEcbSnapshot::default(),
            actual_ecb_bottom_lock_timer: 0,
            actual_source_coll_x130_locked: false,
            actual_floor_skip_surface: None,
            actual_source_floor_skip_line: None,
            actual_source_floor_surface: None,
            actual_source_floor_line: None,
            actual_source_coll_env_flags: 0,
            actual_source_coll_prev_env_flags: 0,
        }
    }
}
