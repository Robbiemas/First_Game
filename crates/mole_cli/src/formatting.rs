use serde_json::Value;

pub(crate) fn format_text_report(report: &Value) -> String {
    let command = report
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("mole");
    let mut lines = vec![format!("Mole CLI {command}")];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("root: {root}"));
    }
    if let Some(git) = report.get("git") {
        if let Some(branch) = git.get("branch").and_then(Value::as_str) {
            lines.push(format!("branch: {branch}"));
        }
        if let Some(total) = git.get("total_changed").and_then(Value::as_u64) {
            lines.push(format!("changed files: {total}"));
        }
    }
    if let Some(parity) = report.get("parity") {
        if let Some(actionable) = parity.get("value_actionable").and_then(Value::as_u64) {
            lines.push(format!("value actionable rows: {actionable}"));
        }
        if let Some(mapped) = parity
            .get("ecb_mapped_motion_states")
            .and_then(Value::as_u64)
        {
            lines.push(format!("ecb mapped motion states: {mapped}"));
        }
    }
    if command == "replay check" || command == "replay trace" {
        let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
        lines.push(format!("ok: {ok}"));
        if let Some(source) = report.get("source") {
            if let Some(path) = source.get("input_export_path").and_then(Value::as_str) {
                lines.push(format!("inputs: {path}"));
            }
            if let Some(path) = source.get("core_report_path").and_then(Value::as_str) {
                lines.push(format!("core report: {path}"));
            }
        }
        if let Some(comparison) = report.get("comparison") {
            if let Some(frames) = comparison.get("frames_compared").and_then(Value::as_u64) {
                lines.push(format!("frames compared: {frames}"));
            }
            if let Some(mismatch) = comparison
                .get("first_state_mismatch")
                .filter(|value| !value.is_null())
            {
                lines.push(format!(
                    "first state mismatch: core frame {}, Slippi frame {}, P{} {} -> {}",
                    mismatch
                        .get("core_frame")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    mismatch
                        .get("source_frame")
                        .and_then(Value::as_i64)
                        .unwrap_or(0),
                    mismatch.get("player").and_then(Value::as_u64).unwrap_or(0),
                    mismatch
                        .get("expected_motion_state")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown"),
                    mismatch
                        .get("actual_motion_state")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ));
            }
        }
    }
    if command == "replay trace" {
        if let Some(trace) = report.get("trace") {
            if let Some(player) = trace.get("player").and_then(Value::as_u64) {
                lines.push(format!("player: {player}"));
            }
            if let Some(rows) = trace.get("rows").and_then(Value::as_array) {
                lines.push(format!("rows: {}", rows.len()));
            }
        }
    }
    if let Some(next) = report.get("recommended_next").and_then(Value::as_array) {
        lines.push("recommended next:".to_string());
        lines.extend(
            next.iter()
                .filter_map(Value::as_str)
                .map(|item| format!("- {item}")),
        );
    }
    lines.join("\n")
}

pub(crate) fn format_markdown_report(report: &Value) -> String {
    match report.get("command").and_then(Value::as_str) {
        Some("agent brief") => format_agent_brief_markdown(report),
        Some("graph next") => format_graph_next_markdown(report),
        Some("graph inspect") => format_graph_inspect_markdown(report),
        Some("verify changed") => format_verify_changed_markdown(report),
        Some("generated check") => format_generated_check_markdown(report),
        Some("finish check") => format_finish_check_markdown(report),
        Some("replay check") => format_replay_check_markdown(report),
        Some("replay trace") => format_replay_trace_markdown(report),
        Some("decomp search") | Some("decomp symbol") => format_decomp_search_markdown(report),
        Some("decomp show") => format_decomp_show_markdown(report),
        Some("frame-data extract") | Some("frame-data show") => format_frame_data_markdown(report),
        _ => format_handoff_markdown(report),
    }
}

fn format_agent_brief_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Agent Brief".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    if let Some(git) = report.get("git") {
        if let Some(branch) = git.get("branch").and_then(Value::as_str) {
            lines.push(format!("- Branch: `{branch}`"));
        }
        if let Some(remote) = git.get("remote_origin").and_then(Value::as_str) {
            lines.push(format!("- Remote: `{remote}`"));
        }
    }
    if let Some(plan) = report.get("macro_plan") {
        if let Some(path) = plan.get("path").and_then(Value::as_str) {
            lines.push(format!("- Macro plan: `{path}`"));
        }
    }
    lines.push(String::new());
    lines.push("## Compaction Anchor".to_string());
    if let Some(anchor) = report.get("compaction_anchor").and_then(Value::as_array) {
        for item in anchor.iter().filter_map(Value::as_str) {
            lines.push(format!("- {item}"));
        }
    }
    lines.push(String::new());
    lines.push("## Missing Graph".to_string());
    if let Some(missing) = report.get("missing_graph") {
        if let Some(count) = missing.get("missing_count").and_then(Value::as_u64) {
            lines.push(format!("- Missing entries: `{count}`"));
        }
        if let Some(next) = missing.get("recommended_next").and_then(Value::as_array) {
            for item in next.iter().filter_map(Value::as_str) {
                lines.push(format!("- {item}"));
            }
        }
    }
    if let Some(graph_next) = report.get("graph_next") {
        lines.push(String::new());
        lines.push("## Next Graph Targets".to_string());
        let ranked_count = graph_next
            .get("ranked_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let omitted_count = graph_next
            .get("omitted_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        lines.push(format!(
            "- Ranked entries: `{ranked_count}`; omitted from brief: `{omitted_count}`"
        ));
        if let Some(entries) = graph_next.get("ranked_entries").and_then(Value::as_array) {
            for entry in entries.iter().take(5) {
                let name = markdown_graph_entry_name(entry);
                let score = entry.get("score").and_then(Value::as_i64).unwrap_or(0);
                let status = entry
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                lines.push(format!("- `{name}`: {status}, score `{score}`"));
            }
        }
    }
    lines.push(String::new());
    lines.push("## Verification".to_string());
    if let Some(commands) = report
        .get("verification_commands")
        .and_then(Value::as_array)
    {
        for command in commands.iter().filter_map(Value::as_str) {
            lines.push(format!("- `{command}`"));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_graph_next_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Graph Next".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    let ranked_count = report
        .get("ranked_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let omitted_count = report
        .get("omitted_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    lines.push(format!("- Ranked entries: `{ranked_count}`"));
    lines.push(format!("- Omitted from output: `{omitted_count}`"));
    lines.push(String::new());
    lines.push("## Ranked Targets".to_string());
    if let Some(entries) = report.get("ranked_entries").and_then(Value::as_array) {
        for (index, entry) in entries.iter().enumerate() {
            let name = markdown_graph_entry_name(entry);
            let status = entry
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let score = entry.get("score").and_then(Value::as_i64).unwrap_or(0);
            lines.push(format!(
                "{}. `{name}` - {status}; Score: `{score}`",
                index + 1
            ));
            if let Some(reason) = entry.get("reason").and_then(Value::as_str) {
                lines.push(format!("   - {reason}"));
            }
            if let Some(score_reasons) = entry.get("score_reasons").and_then(Value::as_array) {
                let joined = score_reasons
                    .iter()
                    .filter_map(Value::as_str)
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("; ");
                if !joined.is_empty() {
                    lines.push(format!("   - Score reasons: {joined}"));
                }
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_verify_changed_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Verify Changed".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    lines.push(String::new());
    lines.push("## Changed Paths".to_string());
    if let Some(paths) = report.get("changed_paths").and_then(Value::as_array) {
        if paths.is_empty() {
            lines.push("- No changed paths reported.".to_string());
        } else {
            for path in paths.iter().filter_map(Value::as_str).take(30) {
                lines.push(format!("- `{path}`"));
            }
            if paths.len() > 30 {
                lines.push(format!("- ... `{}` more", paths.len() - 30));
            }
        }
    }
    lines.push(String::new());
    lines.push("## Commands".to_string());
    if let Some(commands) = report.get("commands").and_then(Value::as_array) {
        for command in commands.iter().filter_map(Value::as_str) {
            lines.push(format!("- `{command}`"));
            if let Some(reasons) = report
                .get("command_reasons")
                .and_then(|value| value.get(command))
                .and_then(Value::as_array)
            {
                for reason in reasons.iter().filter_map(Value::as_str) {
                    lines.push(format!("  - {reason}"));
                }
            }
        }
    }
    if let Some(errors) = report.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            lines.push(String::new());
            lines.push("## Errors".to_string());
            for error in errors.iter().filter_map(Value::as_str) {
                lines.push(format!("- {error}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_graph_inspect_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Graph Inspect".to_string(), String::new()];
    if let Some(target) = report.get("target").and_then(Value::as_str) {
        lines.push(format!("- Target: `{target}`"));
    }
    let found = report
        .get("found")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    lines.push(format!("- Found: `{found}`"));
    if let Some(entry) = report.get("entry").filter(|entry| !entry.is_null()) {
        lines.push(String::new());
        lines.push("## Entry".to_string());
        let name = markdown_graph_entry_name(entry);
        lines.push(format!("- Name: `{name}`"));
        if let Some(kind) = entry.get("kind").and_then(Value::as_str) {
            lines.push(format!("- Kind: `{kind}`"));
        }
        if let Some(status) = entry.get("status").and_then(Value::as_str) {
            lines.push(format!("- Status: `{status}`"));
        }
        if let Some(notes) = entry.get("notes").and_then(Value::as_str) {
            lines.push(format!("- Notes: {notes}"));
        }
    }
    if let Some(checklist) = report.get("audit_checklist").and_then(Value::as_array) {
        lines.push(String::new());
        lines.push("## Audit Checklist".to_string());
        for item in checklist.iter().filter_map(Value::as_str) {
            lines.push(format!("- {item}"));
        }
    }
    if let Some(suggestions) = report.get("suggestions").and_then(Value::as_array) {
        if !suggestions.is_empty() {
            lines.push(String::new());
            lines.push("## Suggestions".to_string());
            for item in suggestions.iter().filter_map(Value::as_str) {
                lines.push(format!("- `{item}`"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_finish_check_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Finish Check".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    if let Some(gate) = report.get("completion_gate") {
        let ok = gate.get("ok").and_then(Value::as_bool).unwrap_or(false);
        lines.push(format!("- Gate OK: `{ok}`"));
        lines.push(String::new());
        lines.push("## Completion Gate".to_string());
        if let Some(checks) = gate.get("checks").and_then(Value::as_array) {
            for check in checks {
                let name = check
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let ok = check.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let detail = check.get("detail").and_then(Value::as_str).unwrap_or("");
                lines.push(format!("- `{name}`: `{ok}` - {detail}"));
            }
        }
    }
    if let Some(help) = report.get("help_catalog") {
        lines.push(String::new());
        lines.push("## Help Catalog".to_string());
        let ok = help.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let count = help
            .get("documented_command_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        lines.push(format!("- OK: `{ok}`"));
        lines.push(format!("- Documented commands: `{count}`"));
    }
    if let Some(queue) = report.get("request_queue") {
        lines.push(String::new());
        lines.push("## Request Queue".to_string());
        let inbox = queue
            .get("inbox_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let completed = queue
            .get("completed_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        lines.push(format!("- Inbox: `{inbox}`"));
        lines.push(format!("- Completed: `{completed}`"));
    }
    if let Some(verification) = report.get("verification") {
        lines.push(String::new());
        lines.push("## Verification".to_string());
        if let Some(commands) = verification.get("commands").and_then(Value::as_array) {
            for command in commands.iter().filter_map(Value::as_str) {
                lines.push(format!("- `{command}`"));
                if let Some(reasons) = verification
                    .get("command_reasons")
                    .and_then(|value| value.get(command))
                    .and_then(Value::as_array)
                {
                    for reason in reasons.iter().filter_map(Value::as_str) {
                        lines.push(format!("  - {reason}"));
                    }
                }
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_generated_check_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Generated Check".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!("- OK: `{ok}`"));

    if let Some(summary) = report.get("summary") {
        lines.push(String::new());
        lines.push("## Summary".to_string());
        for field in [
            "total_groups",
            "ok_groups",
            "missing_input_groups",
            "missing_output_groups",
            "stale_groups",
            "dirty_output_groups",
        ] {
            let count = summary.get(field).and_then(Value::as_u64).unwrap_or(0);
            lines.push(format!("- `{field}`: `{count}`"));
        }
    }

    lines.push(String::new());
    lines.push("## Artifact Groups".to_string());
    if let Some(groups) = report.get("artifact_groups").and_then(Value::as_array) {
        for group in groups {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("unknown");
            let ok = group.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let stale = group.get("stale").and_then(Value::as_bool).unwrap_or(false);
            lines.push(format!("- `{id}`: ok `{ok}`, stale `{stale}`"));
            for field in [
                "missing_inputs",
                "missing_outputs",
                "dirty_outputs",
                "newer_inputs",
            ] {
                let values = string_array(group.get(field));
                if !values.is_empty() {
                    lines.push(format!("  - {field}: `{}`", values.join("`, `")));
                }
            }
            if let Some(command) = group.get("recommended_command").and_then(Value::as_str) {
                lines.push(format!("  - command: `{command}`"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_replay_check_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Replay Check".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!("- OK: `{ok}`"));
    if let Some(source) = report.get("source") {
        if let Some(path) = source.get("replay_path").and_then(Value::as_str) {
            lines.push(format!("- Replay: `{path}`"));
        }
        if let Some(path) = source.get("input_export_path").and_then(Value::as_str) {
            lines.push(format!("- Inputs: `{path}`"));
        }
        if let Some(path) = source.get("core_report_path").and_then(Value::as_str) {
            lines.push(format!("- Core report: `{path}`"));
        }
    }
    if let Some(comparison) = report.get("comparison") {
        lines.push(String::new());
        lines.push("## Comparison".to_string());
        if let Some(mode) = comparison.get("mode").and_then(Value::as_str) {
            lines.push(format!("- Mode: `{mode}`"));
        }
        if let Some(frames) = comparison.get("frames_compared").and_then(Value::as_u64) {
            lines.push(format!("- Frames compared: `{frames}`"));
        }
        if let Some(count) = comparison
            .get("state_mismatch_count")
            .and_then(Value::as_u64)
        {
            lines.push(format!("- State mismatches: `{count}`"));
        }
        if let Some(mismatch) = comparison
            .get("first_state_mismatch")
            .filter(|value| !value.is_null())
        {
            lines.push(String::new());
            lines.push("## First State Mismatch".to_string());
            lines.push(format!(
                "- Core frame: `{}`",
                mismatch
                    .get("core_frame")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            ));
            lines.push(format!(
                "- Slippi frame: `{}`",
                mismatch
                    .get("source_frame")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
            ));
            lines.push(format!(
                "- Player: `{}`",
                mismatch.get("player").and_then(Value::as_u64).unwrap_or(0)
            ));
            lines.push(format!(
                "- Expected: `{}`",
                mismatch
                    .get("expected_motion_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ));
            lines.push(format!(
                "- Actual: `{}`",
                mismatch
                    .get("actual_motion_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ));
        }
    }
    if let Some(errors) = report.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            lines.push(String::new());
            lines.push("## Errors".to_string());
            for error in errors.iter().filter_map(Value::as_str) {
                lines.push(format!("- {error}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_replay_trace_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Replay Trace".to_string(), String::new()];
    if let Some(source) = report.get("source") {
        if let Some(path) = source.get("input_export_path").and_then(Value::as_str) {
            lines.push(format!("- Inputs: `{path}`"));
        }
    }
    if let Some(trace) = report.get("trace") {
        if let Some(player) = trace.get("player").and_then(Value::as_u64) {
            lines.push(format!("- Player: `{player}`"));
        }
        let start = trace
            .get("source_frame_start")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let end = trace
            .get("source_frame_end")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        lines.push(format!("- Source frame window: `{start}..={end}`"));
        lines.push(String::new());
        lines.push("| Core | Source | Stick X | Stick Y | UCF DB | Expected | Actual | Actual Frame | dX | dY | Exp Gx | Act Vx | dVx |".to_string());
        lines.push("| ---: | ---: | ---: | ---: | :---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |".to_string());
        if let Some(rows) = trace.get("rows").and_then(Value::as_array) {
            for row in rows {
                let input = row.get("input").unwrap_or(&Value::Null);
                let delta = row.get("position_delta").unwrap_or(&Value::Null);
                lines.push(format!(
                    "| {} | {} | {} | {} | {} | {} ({}) | {} | {} | {} | {} | {} | {} | {} |",
                    row.get("core_frame").and_then(Value::as_u64).unwrap_or(0),
                    row.get("source_frame").and_then(Value::as_i64).unwrap_or(0),
                    input.get("stick_x").and_then(Value::as_i64).unwrap_or(0),
                    input.get("stick_y").and_then(Value::as_i64).unwrap_or(0),
                    if input
                        .get("ucf_dashback_amendment")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        "yes"
                    } else {
                        "no"
                    },
                    row.get("expected_motion_state")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown"),
                    row.get("expected_slippi_state_id")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    row.get("actual_motion_state")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown"),
                    row.get("actual_motion_frame")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    delta.get("x_milli").and_then(Value::as_i64).unwrap_or(0),
                    delta.get("y_milli").and_then(Value::as_i64).unwrap_or(0),
                    row.get("expected_ground_velocity_x")
                        .and_then(Value::as_i64)
                        .unwrap_or(0),
                    row.get("actual_velocity_x")
                        .and_then(Value::as_i64)
                        .unwrap_or(0),
                    row.get("ground_velocity_x_delta")
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                ));
            }
            if rows.is_empty() {
                lines.push(String::new());
                lines.push("No comparable rows were found in this trace window.".to_string());
            }
        }
    }
    lines.join("\n")
}

fn format_decomp_search_markdown(report: &Value) -> String {
    let title = match report.get("command").and_then(Value::as_str) {
        Some("decomp symbol") => "# Mole Decomp Symbol",
        _ => "# Mole Decomp Search",
    };
    let mut lines = vec![title.to_string(), String::new()];
    if let Some(root) = report.get("decomp_root").and_then(Value::as_str) {
        lines.push(format!("- Decomp root: `{root}`"));
    }
    if let Some(query) = report.get("query").and_then(Value::as_str) {
        lines.push(format!("- Query: `{query}`"));
    }
    let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!("- OK: `{ok}`"));
    lines.push(String::new());
    lines.push("## Matches".to_string());
    if let Some(matches) = report.get("matches").and_then(Value::as_array) {
        if matches.is_empty() {
            lines.push("- No matches.".to_string());
        } else {
            for item in matches.iter().take(20) {
                let path = item
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let line = item.get("line").and_then(Value::as_u64).unwrap_or(0);
                let preview = item.get("preview").and_then(Value::as_str).unwrap_or("");
                let rank = item
                    .get("rank_reason")
                    .and_then(Value::as_str)
                    .unwrap_or("match");
                lines.push(format!("- `{path}:{line}` `{rank}` - {preview}"));
                if let Some(command) = item.get("suggested_command").and_then(Value::as_str) {
                    lines.push(format!("  - `{command}`"));
                }
            }
        }
    }
    if let Some(errors) = report.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            lines.push(String::new());
            lines.push("## Errors".to_string());
            for error in errors.iter().filter_map(Value::as_str) {
                lines.push(format!("- {error}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_decomp_show_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Decomp Show".to_string(), String::new()];
    if let Some(root) = report.get("decomp_root").and_then(Value::as_str) {
        lines.push(format!("- Decomp root: `{root}`"));
    }
    let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!("- OK: `{ok}`"));
    if let Some(excerpt) = report.get("excerpt").filter(|value| !value.is_null()) {
        if let Some(path) = excerpt.get("path").and_then(Value::as_str) {
            let start = excerpt
                .get("start_line")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let end = excerpt.get("end_line").and_then(Value::as_u64).unwrap_or(0);
            lines.push(format!("- Excerpt: `{path}:{start}-{end}`"));
        }
        lines.push(String::new());
        lines.push("```c".to_string());
        if let Some(excerpt_lines) = excerpt.get("lines").and_then(Value::as_array) {
            for item in excerpt_lines {
                let line = item.get("line").and_then(Value::as_u64).unwrap_or(0);
                let text = item.get("text").and_then(Value::as_str).unwrap_or("");
                lines.push(format!("{line:>5}: {text}"));
            }
        }
        lines.push("```".to_string());
    }
    if let Some(errors) = report.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            lines.push(String::new());
            lines.push("## Errors".to_string());
            for error in errors.iter().filter_map(Value::as_str) {
                lines.push(format!("- {error}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_frame_data_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Frame Data".to_string(), String::new()];
    let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!("- OK: `{ok}`"));
    if let Some(path) = report.get("artifact_path").and_then(Value::as_str) {
        lines.push(format!("- Artifact: `{path}`"));
    }
    if let Some(artifact) = report.get("artifact").filter(|value| !value.is_null()) {
        lines.push(format!(
            "- Character: `{}`",
            artifact
                .get("target_character_label")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        let source_character = artifact
            .get("source_character_label")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                artifact
                    .get("source_character")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            });
        lines.push(format!("- Source character: `{source_character}`"));
        lines.push(format!(
            "- State: `{}` ({})",
            artifact
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            artifact
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        if let Some(projection) = artifact.get("projection") {
            lines.push(format!(
                "- Projection: `{}` / `{}`",
                projection
                    .get("default_view")
                    .and_then(Value::as_str)
                    .unwrap_or("xy"),
                projection
                    .get("z_policy")
                    .and_then(Value::as_str)
                    .unwrap_or("preserve_and_project")
            ));
        }
        if let Some(summary) = artifact.get("summary") {
            lines.push(String::new());
            lines.push("## Summary".to_string());
            if let Some(total) = summary.get("total_frames") {
                lines.push(format!("- Total frames: `{}`", display_json_scalar(total)));
            }
            if let Some(iasa) = summary.get("iasa_frame") {
                lines.push(format!("- IASA: `{}`", display_json_scalar(iasa)));
            }
            if let Some(windows) = summary
                .get("active_hitbox_windows")
                .and_then(Value::as_array)
            {
                for window in windows {
                    lines.push(format!(
                        "- Active hitboxes: `{}`-`{}`",
                        window.get("start").and_then(Value::as_i64).unwrap_or(0),
                        window.get("end").and_then(Value::as_i64).unwrap_or(0)
                    ));
                }
            }
            if let Some(windows) = summary
                .get("active_hurtbox_windows")
                .and_then(Value::as_array)
            {
                for window in windows {
                    lines.push(format!(
                        "- Active hurtboxes: `{}`-`{}`",
                        window.get("start").and_then(Value::as_i64).unwrap_or(0),
                        window.get("end").and_then(Value::as_i64).unwrap_or(0)
                    ));
                }
            }
            if let Some(windows) = summary
                .get("active_body_volume_windows")
                .and_then(Value::as_array)
            {
                for window in windows {
                    lines.push(format!(
                        "- Active body volumes: `{}`-`{}`",
                        window.get("start").and_then(Value::as_i64).unwrap_or(0),
                        window.get("end").and_then(Value::as_i64).unwrap_or(0)
                    ));
                }
            }
        }
        lines.push(String::new());
        lines.push("## Keyframes".to_string());
        if let Some(keyframes) = artifact.get("keyframes").and_then(Value::as_array) {
            for frame in keyframes.iter().take(12) {
                lines.push(format!(
                    "- Frame {}: {} hitbox(es), {} hurtbox(es), {} body volume(s)",
                    frame.get("frame").and_then(Value::as_i64).unwrap_or(0),
                    frame
                        .get("hitboxes")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0),
                    frame
                        .get("hurtboxes")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0),
                    frame
                        .get("body_volumes")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0)
                ));
            }
        }
        lines.push(String::new());
        lines.push("## Sources".to_string());
        if let Some(sources) = artifact.get("sources").and_then(Value::as_array) {
            for source in sources {
                let path = source
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let suffix = source
                    .get("line")
                    .and_then(Value::as_i64)
                    .map(|line| format!(":{line}"))
                    .unwrap_or_default();
                lines.push(format!(
                    "- `{}` `{}{}` - {}",
                    source
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("source"),
                    path,
                    suffix,
                    source.get("purpose").and_then(Value::as_str).unwrap_or("")
                ));
            }
        }
        if let Some(gaps) = artifact.get("gaps").and_then(Value::as_array) {
            if !gaps.is_empty() {
                lines.push(String::new());
                lines.push("## Gaps".to_string());
                for gap in gaps {
                    lines.push(format!(
                        "- `{}` - {}",
                        gap.get("field")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown"),
                        gap.get("reason").and_then(Value::as_str).unwrap_or("")
                    ));
                }
            }
        }
    }
    if let Some(errors) = report.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            lines.push(String::new());
            lines.push("## Errors".to_string());
            for error in errors.iter().filter_map(Value::as_str) {
                lines.push(format!("- {error}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn display_json_scalar(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn string_array(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn markdown_graph_entry_name(entry: &Value) -> String {
    if let Some(id) = entry.get("id").and_then(Value::as_str) {
        id.to_string()
    } else {
        format!(
            "{} -> {}",
            entry
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            entry.get("to").and_then(Value::as_str).unwrap_or("unknown")
        )
    }
}

fn format_handoff_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole CLI Handoff".to_string(), String::new()];
    if let Some(root) = report.get("project_root").and_then(Value::as_str) {
        lines.push(format!("- Project root: `{root}`"));
    }
    if let Some(git) = report.get("git") {
        lines.push(format!(
            "- Branch: `{}`",
            git.get("branch")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        lines.push(format!(
            "- Remote: `{}`",
            git.get("remote_origin")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        lines.push(format!(
            "- Dirty files: {}",
            git.get("total_changed")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
    }
    if let Some(parity) = report.get("parity") {
        lines.push(format!(
            "- Value rows: {} total, {} actionable, {} derived",
            parity
                .get("value_total_rows")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            parity
                .get("value_actionable")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            parity
                .get("value_derived")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
        lines.push(format!(
            "- ECB mapped states: {}",
            parity
                .get("ecb_mapped_motion_states")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
        if let Some(states) = parity
            .get("ecb_unmapped_derived_states")
            .and_then(Value::as_array)
        {
            let names = states.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            lines.push(format!("- Unmapped derived states: {}", names.join(", ")));
        }
    }
    if let Some(next) = report.get("recommended_next").and_then(Value::as_array) {
        lines.extend([String::new(), "## Recommended Next".to_string()]);
        lines.extend(
            next.iter()
                .filter_map(Value::as_str)
                .map(|item| format!("- {item}")),
        );
    }
    if let Some(commands) = report
        .get("verification_commands")
        .and_then(Value::as_array)
    {
        lines.extend([String::new(), "## Verification".to_string()]);
        lines.extend(
            commands
                .iter()
                .filter_map(Value::as_str)
                .map(|command| format!("- `{command}`")),
        );
    }
    lines.push(String::new());
    lines.join("\n")
}
