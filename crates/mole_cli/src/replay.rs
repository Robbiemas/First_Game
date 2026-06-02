use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use mole_core::Vec2;
use mole_runtime::{
    compare_slippi_export_from_match_start_with_core, compare_slippi_export_with_core,
    slippi_core_report_path, trace_slippi_export_from_match_start_with_core,
    write_slippi_core_report, SlippiCoreComparison, SlippiCoreComparisonConfig,
    SlippiCoreComparisonMode, SlippiCoreMismatch, SlippiCorePositionDrift, SlippiCoreTrace,
    SlippiCoreTraceConfig, SlippiCoreTraceRow,
};
use serde_json::{json, Value};

use crate::{
    base_report, ReplayCheckMode, ReplayCheckOptions, ReplayCommand, ReplayTraceOptions,
    SCHEMA_VERSION,
};

pub(crate) fn replay_report(root: &Path, command: &ReplayCommand) -> Value {
    match command {
        ReplayCommand::Check(options) => replay_check_report(root, options),
        ReplayCommand::Trace(options) => replay_trace_report(root, options),
    }
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

fn run_replay_trace(root: &Path, options: &ReplayTraceOptions) -> Result<Value, String> {
    let input_export_path = resolve_project_path(root, &options.inputs);
    let text = fs::read_to_string(&input_export_path).map_err(|error| {
        format!(
            "failed to read Slippi input export {}: {error}",
            input_export_path.display()
        )
    })?;
    let trace = trace_slippi_export_from_match_start_with_core(
        &text,
        SlippiCoreTraceConfig {
            player_index: options.player_index,
            source_frame_start: options.source_frame_start,
            source_frame_end: options.source_frame_end,
            max_frames: Some(options.frames),
        },
    )
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
        "trace": trace_json(&trace),
        "errors": [],
    }))
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
        "expected_motion_state": format!("{:?}", mismatch.expected_motion_state),
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
        "expected_motion_state": format!("{:?}", drift.expected_motion_state),
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

fn trace_json(trace: &SlippiCoreTrace) -> Value {
    json!({
        "source_replay_path": trace.source_replay_path.as_deref(),
        "player": trace.config.player_index + 1,
        "source_frame_start": trace.config.source_frame_start,
        "source_frame_end": trace.config.source_frame_end,
        "max_frames": trace.config.max_frames,
        "rows": trace.rows.iter().map(trace_row_json).collect::<Vec<_>>(),
    })
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
        },
        "expected_slippi_state_id": row.expected_slippi_state_id,
        "expected_motion_state": row
            .expected_motion_state
            .map(|state| format!("{state:?}")),
        "actual_motion_state": format!("{:?}", row.actual_motion_state),
        "actual_motion_frame": row.actual_motion_frame,
        "expected_position": vec2_json(row.expected_position),
        "actual_position": vec2_json(row.actual_position),
        "position_delta": vec2_delta_json(row.actual_position, row.expected_position),
        "expected_ground_velocity_x": row.expected_ground_velocity_x,
        "expected_air_velocity_x": row.expected_air_velocity_x,
        "expected_velocity_y": row.expected_velocity_y,
        "actual_velocity_x": row.actual_velocity_x,
        "actual_velocity_y": row.actual_velocity_y,
        "ground_velocity_x_delta": row.actual_velocity_x - row.expected_ground_velocity_x,
        "air_velocity_x_delta": row.actual_velocity_x - row.expected_air_velocity_x,
        "velocity_y_delta": row.actual_velocity_y - row.expected_velocity_y,
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

#[derive(Debug)]
struct ReplayExportPaths {
    replay_path: Option<PathBuf>,
    input_export_path: PathBuf,
    export_report_path: Option<PathBuf>,
}
