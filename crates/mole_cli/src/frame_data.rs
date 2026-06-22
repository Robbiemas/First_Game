use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    base_report, frame_data_sampler, FrameDataBatchOptions, FrameDataCommand,
    FrameDataExportBatchOptions, FrameDataExportOptions, FrameDataOptions,
};
use mole_core::{
    melee_action_state_id_for_motion_state, motion_state_for_runtime_variant,
    runtime_motion_state_for_source_key, source_binding_for_motion_state, MotionStateSourceBinding,
    SourceSpecialActionBinding, CANONICAL_SOURCE_ONLY_ACTION_BINDINGS,
    FALCON_SOURCE_SPECIAL_ACTION_BINDINGS, RUST_MOTION_STATE_VARIANTS,
};
use mole_frame_data::{
    encode_runtime_source_frame_capsules, FrameDataSampleOptions, RuntimeFigatreeChunk,
    RuntimeSourceActionFrameSamples, RuntimeSourceExport, RuntimeSourceExportEvaluator,
};

pub(crate) fn frame_data_report(root: &Path, command: &FrameDataCommand) -> Value {
    match command {
        FrameDataCommand::Extract(options) => {
            artifact_report(root, "frame-data extract", options, true)
        }
        FrameDataCommand::ExtractAll(options) => artifact_batch_report(root, options),
        FrameDataCommand::ExportRuntime(options) => export_runtime_report(root, options),
        FrameDataCommand::ExportRuntimeAll(options) => export_runtime_batch_report(root, options),
        FrameDataCommand::Sample(options) => {
            frame_data_sampler::frame_data_sample_report(root, options)
        }
        FrameDataCommand::Show(options) => artifact_report(root, "frame-data show", options, false),
    }
}

fn artifact_report(
    root: &Path,
    command_name: &str,
    options: &FrameDataOptions,
    decode_source_script: bool,
) -> Value {
    let mut report = base_report(command_name, root);
    report["character"] = json!(options.character);
    report["source_character"] = json!(options.source_character);
    report["source_state"] = json!(options.source_state);
    report["state"] = json!(options.state);
    report["wrote_artifact"] = json!(false);
    report["created_artifact"] = json!(false);

    let path = root
        .join("resources")
        .join("melee")
        .join("frame_data")
        .join(&options.character)
        .join(format!("{}.json", options.state));
    report["artifact_path"] = json!(path.display().to_string());

    match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(mut artifact) if artifact.is_object() => {
                if decode_source_script {
                    if let Some(decoded) = decoded_action_script_artifact(root, options, &artifact)
                    {
                        apply_decoded_action_script(&mut artifact, decoded, root);
                    }
                }
                apply_melee_render_projection_metadata(&mut artifact);
                if decode_source_script && options.write {
                    match write_frame_data_artifact(&path, &artifact) {
                        Ok(()) => {
                            report["wrote_artifact"] = json!(true);
                            report["ok"] = json!(true);
                            report["artifact"] = artifact;
                            report["errors"] = json!([]);
                        }
                        Err(error) => {
                            report["ok"] = json!(false);
                            report["artifact"] = artifact;
                            report["errors"] = json!([error]);
                        }
                    }
                } else {
                    report["ok"] = json!(true);
                    report["artifact"] = artifact;
                    report["errors"] = json!([]);
                }
            }
            Ok(_) => {
                report["ok"] = json!(false);
                report["artifact"] = Value::Null;
                report["errors"] = json!([format!(
                    "move frame data artifact is not a JSON object: {}",
                    path.display()
                )]);
            }
            Err(error) => {
                report["ok"] = json!(false);
                report["artifact"] = Value::Null;
                report["errors"] = json!([format!(
                    "failed to parse move frame data artifact {}: {error}",
                    path.display()
                )]);
            }
        },
        Err(_) => {
            if decode_source_script && options.write {
                match source_action_table_frame_data_artifact(root, options) {
                    Ok(mut artifact) => {
                        report["created_artifact"] = json!(true);
                        if let Some(decoded) =
                            decoded_action_script_artifact(root, options, &artifact)
                        {
                            apply_decoded_action_script(&mut artifact, decoded, root);
                        }
                        apply_melee_render_projection_metadata(&mut artifact);
                        match write_frame_data_artifact(&path, &artifact) {
                            Ok(()) => {
                                report["wrote_artifact"] = json!(true);
                                report["ok"] = json!(true);
                                report["artifact"] = artifact;
                                report["errors"] = json!([]);
                            }
                            Err(error) => {
                                report["ok"] = json!(false);
                                report["artifact"] = artifact;
                                report["errors"] = json!([error]);
                            }
                        }
                    }
                    Err(create_error) => {
                        report["ok"] = json!(false);
                        report["artifact"] = Value::Null;
                        report["errors"] = json!([
                            format!("move frame data artifact not found: {}", path.display()),
                            create_error,
                        ]);
                    }
                }
            } else {
                report["ok"] = json!(false);
                report["artifact"] = Value::Null;
                report["errors"] = json!([format!(
                    "move frame data artifact not found: {}",
                    path.display()
                )]);
            }
        }
    }

    report
}

fn artifact_batch_report(root: &Path, options: &FrameDataBatchOptions) -> Value {
    let mut report = base_report("frame-data extract", root);
    report["all_states"] = json!(true);
    report["character"] = json!(options.character);
    report["source_character"] = json!(options.source_character);
    report["write"] = json!(options.write);
    report["created_artifacts"] = json!(0);
    report["wrote_artifacts"] = json!(0);
    report["created_manifest"] = json!(false);
    report["wrote_manifest"] = json!(false);

    let action_table_path = action_animation_table_path(root, &options.source_character);
    let manifest_path = frame_data_manifest_path(root, &options.character);
    report["source_action_table_path"] = json!(action_table_path.display().to_string());
    report["manifest_path"] = json!(manifest_path.display().to_string());

    let action_table = match read_json_object(&action_table_path) {
        Ok(action_table) => action_table,
        Err(error) => {
            report["ok"] = json!(false);
            report["mapped_state_count"] = json!(0);
            report["states"] = json!([]);
            report["skipped_source_actions"] = json!([]);
            report["skipped_motion_states"] = json!(RUST_MOTION_STATE_VARIANTS);
            report["manifest"] = Value::Null;
            report["errors"] = json!([error]);
            return report;
        }
    };
    let (source_actions, skipped_source_actions) = source_action_imports(&action_table);
    let mut states = Vec::new();
    let mut errors = Vec::new();
    let mut rust_parity_gaps = Vec::new();
    let source_raw = fs::read(raw_fighter_data_path(root, &options.source_character)).ok();
    let (rig, rig_errors) = compact_manifest_rig(root, &options.source_character);
    errors.extend(rig_errors);
    let mut actions = Vec::new();

    for import in &source_actions {
        states.push(json!({
            "state": import.state.as_str(),
            "runtime_motion_state": import.runtime_motion_state.as_deref(),
            "source_action_key": import.source_action_key.as_str(),
            "match_kind": import.match_kind.as_str(),
            "source_action_name": import.source_action_name.as_str(),
            "action_state_id": action_state_id(&import.action),
            "total_frames": action_total_frames(&import.action),
            "subaction_script_offset": action_subaction_script_offset(&import.action),
            "artifact_path": Value::Null,
            "created_artifact": false,
            "wrote_artifact": false,
            "canonical_storage": "compact_manifest",
            "ok": true,
            "errors": [],
        }));
        if import.runtime_motion_state.is_none() {
            rust_parity_gaps.push(json!({
                "state": import.state.as_str(),
                "source_action_key": import.source_action_key.as_str(),
                "source_action_name": import.source_action_name.as_str(),
                "action_state_id": action_state_id(&import.action),
                "reason": "source action imported but no current Rust MotionState binding exists",
            }));
        }
        let decoded = source_raw.as_deref().and_then(|raw| {
            decoded_action_script_from_action(
                root,
                &options.source_character,
                &import.action,
                Some(raw),
            )
        });
        actions.push(compact_manifest_action(
            root,
            &action_table_path,
            import,
            decoded.as_ref(),
        ));
    }

    let manifest = json!({
        "schema_version": 2,
        "artifact_kind": "source_character_frame_data_manifest",
        "target_character": options.character,
        "target_character_label": character_label(&options.character),
        "source_character": options.source_character,
        "source_character_label": character_label(&options.source_character),
        "source_space": "melee_xyz",
        "z_policy": "preserve_source_z_flatten_after_runtime_projection",
        "projection": {
            "render_transform": MELEE_RIGHT_FACING_RENDER_TRANSFORM,
            "flatten_after_render": MELEE_RIGHT_FACING_FLATTEN_POLICY,
        },
        "canonical_model": {
            "hitboxes": "decoded subaction procedures from ftAction_8007121C plus ftColl_8007AD18 runtime capsule update",
            "hurtboxes": "ftData.x30 hurtbox init table transformed through JObj/FigaTree at render/runtime sample time",
            "frame_capsules": "derived view; not canonical full-character storage",
        },
        "sources": [
            {
                "kind": "source_action_table",
                "path": path_for_artifact(root, &action_table_path),
                "purpose": "source action records and FigaTree metadata for imported states",
            },
            {
                "kind": "fighter_action_script_raw",
                "path": path_for_artifact(root, &raw_fighter_data_path(root, &options.source_character)),
                "purpose": "raw subaction command data decoded into manifest procedures when available",
                "available": source_raw.is_some(),
            },
        ],
        "rig": rig,
        "actions": actions,
        "rust_parity_gaps": rust_parity_gaps.clone(),
        "skipped_source_actions": skipped_source_actions.clone(),
    });

    let manifest_text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    let created_manifest = !manifest_path.exists();
    report["created_manifest"] = json!(created_manifest);
    report["manifest_bytes"] = json!(manifest_text.len() + 1);
    if options.write {
        if let Err(error) = write_frame_data_artifact(&manifest_path, &manifest) {
            errors.push(error);
        } else {
            report["wrote_manifest"] = json!(true);
        }
    }

    report["ok"] = json!(!source_actions.is_empty() && errors.is_empty());
    report["source_action_count"] = json!(source_actions.len());
    report["mapped_state_count"] = json!(source_actions.len());
    report["runtime_mapped_state_count"] = json!(source_actions
        .iter()
        .filter(|import| import.runtime_motion_state.is_some())
        .count());
    report["created_artifacts"] = json!(0);
    report["wrote_artifacts"] = json!(0);
    report["states"] = json!(states);
    report["skipped_source_actions"] = json!(skipped_source_actions);
    report["rust_parity_gaps"] = json!(rust_parity_gaps);
    report["skipped_motion_states"] = json!(skipped_motion_states(
        source_actions
            .iter()
            .filter_map(|import| import.runtime_motion_state.as_deref())
    ));
    report["errors"] = json!(errors);
    report
}

fn compact_manifest_action(
    root: &Path,
    action_table_path: &Path,
    import: &SourceActionImport,
    decoded: Option<&DecodedActionScript>,
) -> Value {
    json!({
        "state": import.state.as_str(),
        "runtime_motion_state": import.runtime_motion_state.as_deref(),
        "source_action_key": import.source_action_key.as_str(),
        "source_action_name": import.source_action_name.as_str(),
        "match_kind": import.match_kind.as_str(),
        "action_state_id": action_state_id(&import.action),
        "total_frames": action_total_frames(&import.action),
        "subaction_script_offset": action_subaction_script_offset(&import.action),
        "figatree_root": action_figatree_root(&import.action),
        "source_action_table": source_action_table_source_json(root, action_table_path, &import.action),
        "source_action": import.action.clone(),
        "decoded_action_script": decoded
            .map(|decoded| decoded_action_script_json(decoded, root))
            .unwrap_or(Value::Null),
        "derived_frame_capsules": {
            "materialized": false,
            "reason": "Melee stores compact action, hitbox command, hurtbox init, JObj, and FigaTree data; dev-tool/debug views may sample frames on demand, while player runtime preloads a baked in-memory action/frame cache from the compact export",
        },
    })
}

fn compact_manifest_rig(root: &Path, source_character: &str) -> (Value, Vec<String>) {
    let mut errors = Vec::new();
    let profile_path = character_profile_path(root, source_character);
    let hurtbox_inits_path = hurtbox_inits_path(root, source_character);
    let costume_skeleton_path = costume_skeleton_path(root, source_character);
    let common_parts_path = common_parts_path(root, source_character);
    let profile = optional_source_json(root, &profile_path, &mut errors);
    let hurtbox_inits = optional_source_json(root, &hurtbox_inits_path, &mut errors);
    let skeleton = optional_source_json(root, &costume_skeleton_path, &mut errors);
    let common_parts = optional_source_json(root, &common_parts_path, &mut errors);
    let topn_scale = profile
        .get("fields")
        .and_then(|fields| fields.get("model_scaling"))
        .and_then(|field| field.get("raw"))
        .and_then(Value::as_f64)
        .map(Value::from);
    if topn_scale.is_none() {
        errors.push(format!(
            "{} is missing fields.model_scaling.raw for source live JObj setup",
            profile_path.display()
        ));
    }

    (
        json!({
            "canonical_source": MELEE_HURTBOX_SOURCE,
            "live_pose_setup": {
                "source_order": "Fighter_ChangeMotionState seeds TopN rot_y; ftAnim mutates JObj/FObj state; Fighter_UpdateModelScale sets root scale before map/capture consumers read lb_8000B1CC",
                "topn_rot_y_source": "ftPartSetRotY(fp, FtPart_TopN, M_PI_2 * fp->facing_dir) for right-facing source samples",
                "topn_rot_y_radians": std::f64::consts::FRAC_PI_2,
                "topn_scale_source": "Fighter_UpdateModelScale -> ftCommon_GetModelScale(fp); unmodified Falcon uses x34_scale.y == 1.0, so model_scaling is the root scale",
                "topn_scale": topn_scale.unwrap_or(Value::Null),
            },
            "profile": {
                "source_table": "ftData.x0 co_attrs",
                "path": path_for_artifact(root, &profile_path),
                "status": if profile.is_null() { "missing" } else { "source_extracted" },
                "data": profile,
            },
            "common_parts": {
                "source_table": "ftData.x8",
                "path": path_for_artifact(root, &common_parts_path),
                "status": if common_parts.is_null() { "missing" } else { "source_extracted" },
                "source_capture_anchor": "ftCo_CatchPull_Anim -> fn_800DA1D8 stores fp->parts[fp->ft_data->x8->x11].joint in mv.co.capturedamage.x18",
                "source_capture_delta": "ftCo_CapturePulled* / CaptureWait* use lb_8000B1CC(capturedamage.x18) - lb_8000B1CC(FtPart_XRotN)",
                "data": common_parts,
            },
            "hurtbox_inits": {
                "source_table": "ftData.x30",
                "path": path_for_artifact(root, &hurtbox_inits_path),
                "status": if hurtbox_inits.is_null() { "missing" } else { "source_extracted" },
                "source_init_handler": MELEE_HURTBOX_INIT_HANDLER,
                "source_update_handler": MELEE_HURTBOX_UPDATE_HANDLER,
                "source_draw_handler": MELEE_HURTBOX_DRAW_HANDLER,
                "source_render_endpoints": MELEE_HURTBOX_RENDER_ENDPOINTS,
                "source_render_radius": MELEE_HURTBOX_RENDER_RADIUS,
                "source_color_table": MELEE_HURTBOX_COLOR_TABLE,
                "source_z_policy": MELEE_HURTBOX_Z_POLICY,
                "data": hurtbox_inits,
            },
            "skeleton": {
                "source_table": "JObj hierarchy",
                "path": path_for_artifact(root, &costume_skeleton_path),
                "status": if skeleton.is_null() { "missing" } else { "source_extracted" },
                "data": skeleton,
            },
            "derived_samples": {
                "canonical": false,
                "ecb_samples_path": path_for_artifact(root, &action_ecb_samples_path(root, source_character)),
                "reason": "sampled frame capsules are debug/dev-tool views derived on demand from compact ftData.x30, JObj, and FigaTree data; player runtime bakes equivalent compact frame capsules during preload, not during gameplay",
            },
        }),
        errors,
    )
}

fn optional_source_json(_root: &Path, path: &Path, errors: &mut Vec<String>) -> Value {
    match read_json_object(path) {
        Ok(value) => value,
        Err(error) => {
            if path.exists() {
                errors.push(error);
            }
            json!(null)
        }
    }
}

#[derive(Debug, Clone)]
struct SourceActionImport {
    state: String,
    runtime_motion_state: Option<String>,
    source_action_key: String,
    source_action_name: String,
    match_kind: String,
    action: Value,
}

fn source_action_imports(action_table: &Value) -> (Vec<SourceActionImport>, Vec<Value>) {
    let Some(actions) = action_table.get("actions").and_then(Value::as_array) else {
        return (
            Vec::new(),
            vec![json!({
                "reason": "source action table does not contain an actions array",
            })],
        );
    };
    let mut imports = Vec::new();
    let mut skipped = Vec::new();
    let mut used_states = BTreeSet::new();
    let mut used_runtime_states = BTreeSet::new();

    for action in actions {
        let Some(source_action_key) = source_action_key(action) else {
            skipped.push(json!({
                "action_state_id": action_state_id(action),
                "source_action_name": source_action_name(action),
                "reason": "source action record has no ACTION_* figatree key",
            }));
            continue;
        };
        let runtime_candidate = runtime_motion_state_for_source_key(&source_action_key);
        let runtime_motion_state = runtime_candidate.and_then(|candidate| {
            if used_runtime_states.contains(candidate) {
                None
            } else {
                used_runtime_states.insert(candidate.to_string());
                Some(candidate.to_string())
            }
        });
        let match_kind = if let Some(runtime_state) = runtime_motion_state.as_deref() {
            if runtime_state == source_action_key {
                "exact_motion_state"
            } else {
                "source_alias"
            }
        } else if runtime_candidate.is_some() {
            "duplicate_runtime_binding"
        } else {
            "source_only"
        };
        let state = source_import_state_key(
            runtime_motion_state
                .as_deref()
                .unwrap_or(&source_action_key),
            action,
            &mut used_states,
        );
        imports.push(SourceActionImport {
            state,
            runtime_motion_state,
            source_action_key,
            source_action_name: source_action_name(action),
            match_kind: match_kind.to_string(),
            action: action.clone(),
        });
    }

    (imports, skipped)
}

fn source_import_state_key(
    preferred_key: &str,
    action: &Value,
    used_states: &mut BTreeSet<String>,
) -> String {
    let sanitized = sanitize_state_key(preferred_key);
    if !sanitized.is_empty() && used_states.insert(sanitized.clone()) {
        return sanitized;
    }
    let action_id = action_state_id(action);
    let fallback = sanitize_state_key(&format!("{preferred_key}_Action{action_id}"));
    if !fallback.is_empty() && used_states.insert(fallback.clone()) {
        return fallback;
    }
    let mut index = 2usize;
    loop {
        let candidate = sanitize_state_key(&format!("{preferred_key}_Action{action_id}_{index}"));
        if used_states.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn source_action_key(action: &Value) -> Option<String> {
    [
        action.get("name").and_then(Value::as_str),
        action.get("figatree_root").and_then(Value::as_str),
        action
            .get("figatree")
            .and_then(|figatree| figatree.get("root"))
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .find_map(extract_action_key_from_symbol)
}

fn extract_action_key_from_symbol(symbol: &str) -> Option<String> {
    let (_, after_action) = symbol.split_once("_ACTION_")?;
    let key = after_action
        .strip_suffix("_figatree")
        .or_else(|| after_action.strip_suffix("_joint"))
        .unwrap_or(after_action);
    let sanitized = sanitize_state_key(key);
    (!sanitized.is_empty()).then_some(sanitized)
}

fn sanitize_state_key(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else if !output.ends_with('_') && !output.is_empty() {
            output.push('_');
        }
    }
    let output = output.trim_matches('_').to_string();
    if output.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("Action{output}")
    } else {
        output
    }
}

fn skipped_motion_states<'a>(
    mapped_states: impl IntoIterator<Item = &'a str>,
) -> Vec<&'static str> {
    let mapped = mapped_states.into_iter().collect::<BTreeSet<_>>();
    RUST_MOTION_STATE_VARIANTS
        .iter()
        .copied()
        .filter(|state| !mapped.contains(state))
        .collect()
}

fn write_frame_data_artifact(path: &Path, artifact: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create frame data artifact directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(artifact).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| {
        format!(
            "failed to write frame data artifact {}: {error}",
            path.display()
        )
    })
}

fn export_runtime_report(root: &Path, options: &FrameDataExportOptions) -> Value {
    let mut report = base_report("frame-data export-runtime", root);
    report["character"] = json!(options.character);
    report["state"] = json!(options.state);
    report["wrote_output"] = json!(false);

    let artifact_path = frame_data_artifact_path(root, &options.character, &options.state);
    let source_manifest_path = frame_data_manifest_path(root, &options.character);
    let output_path = frame_data_runtime_output_path(root, options.output.as_deref());
    report["source_artifact_path"] = json!(artifact_path.display().to_string());
    report["source_manifest_path"] = json!(source_manifest_path.display().to_string());
    report["output_path"] = json!(output_path.display().to_string());

    if source_manifest_path.exists() {
        match read_json_object(&source_manifest_path)
            .and_then(|manifest| {
                manifest_action_for_state(&manifest, &options.state)?;
                RuntimeSourceExportModule::from_manifest(
                    root,
                    &options.character,
                    &source_manifest_path,
                    &manifest,
                    Some(&options.state),
                )
            })
            .and_then(|module| {
                let generated = generate_runtime_source_export_module(&module)?;
                Ok((module, generated))
            }) {
            Ok((module, generated)) => {
                report["ok"] = json!(true);
                report["runtime_export_kind"] = json!("compact_source_export");
                report["state_count"] = json!(module.state_count());
                report["source_only_action_count"] = json!(module.source_only_action_count);
                report["figatree_chunk_count"] = json!(module.figatree_chunk_count());
                report["figatree_chunk_bytes"] = json!(module.figatree_chunk_bytes());
                report["source_frame_count"] = json!(module.source_frame_count());
                report["hitbox_frame_count"] = json!(module.hitbox_frame_count());
                report["hurtbox_frame_count"] = json!(module.hurtbox_frame_count());
                report["hitbox_count"] = json!(module.hitbox_count());
                report["hurtbox_count"] = json!(module.hurtbox_count());
                report["generated_bytes"] = json!(generated.total_bytes());
                report["generated_module_bytes"] = json!(generated.module_text.len());
                report["generated_sidecar_bytes"] = json!(generated.sidecar_bytes());
                report["errors"] = json!([]);
                if options.write {
                    match write_runtime_source_export_module(&output_path, &generated) {
                        Ok(()) => report["wrote_output"] = json!(true),
                        Err(error) => {
                            report["ok"] = json!(false);
                            report["errors"] = json!([error]);
                        }
                    }
                }
            }
            Err(error) => {
                report["ok"] = json!(false);
                report["runtime_export_kind"] = json!("compact_source_export");
                report["state_count"] = json!(0);
                report["source_only_action_count"] = json!(0);
                report["figatree_chunk_count"] = json!(0);
                report["figatree_chunk_bytes"] = json!(0);
                report["hitbox_frame_count"] = json!(0);
                report["hurtbox_frame_count"] = json!(0);
                report["hitbox_count"] = json!(0);
                report["hurtbox_count"] = json!(0);
                report["generated_bytes"] = json!(0);
                report["errors"] = json!([error]);
            }
        }
        return report;
    }

    let export_result = {
        read_json_object(&artifact_path).and_then(|artifact| {
            RuntimeFrameDataExport::from_artifact(&options.character, &options.state, &artifact)
        })
    };

    match export_result.and_then(|export| {
        let artifact_path = if source_manifest_path.exists() {
            source_manifest_path.clone()
        } else {
            artifact_path.clone()
        };
        let generated = generate_runtime_frame_data_module(
            root,
            &[RuntimeFrameDataModuleEntry {
                export: export.clone(),
                artifact_path,
            }],
        )?;
        Ok((export, generated))
    }) {
        Ok((export, generated)) => {
            report["ok"] = json!(true);
            report["runtime_export_kind"] = json!("legacy_baked_capsule_module");
            report["hitbox_frame_count"] = json!(export.hitbox_frame_count());
            report["hurtbox_frame_count"] = json!(export.hurtbox_frame_count());
            report["hitbox_count"] = json!(export.hitbox_count());
            report["hurtbox_count"] = json!(export.hurtbox_count());
            report["generated_bytes"] = json!(generated.len());
            report["errors"] = json!([]);
            if options.write {
                match write_runtime_frame_data_module(&output_path, &generated) {
                    Ok(()) => {
                        report["wrote_output"] = json!(true);
                    }
                    Err(error) => {
                        report["ok"] = json!(false);
                        report["errors"] = json!([error]);
                    }
                }
            }
        }
        Err(error) => {
            report["ok"] = json!(false);
            report["runtime_export_kind"] = json!("legacy_baked_capsule_module");
            report["hitbox_frame_count"] = json!(0);
            report["hurtbox_frame_count"] = json!(0);
            report["hitbox_count"] = json!(0);
            report["hurtbox_count"] = json!(0);
            report["source_frame_count"] = json!(0);
            report["generated_bytes"] = json!(0);
            report["errors"] = json!([error]);
        }
    }

    report
}

fn export_runtime_batch_report(root: &Path, options: &FrameDataExportBatchOptions) -> Value {
    let mut report = base_report("frame-data export-runtime", root);
    report["all_states"] = json!(true);
    report["character"] = json!(options.character);
    report["wrote_output"] = json!(false);
    report["compact_manifest_detected"] = json!(false);
    report["requires_sampler"] = json!(false);
    report["runtime_export_kind"] = json!("legacy_baked_capsule_module");

    let frame_data_dir = root
        .join("resources")
        .join("melee")
        .join("frame_data")
        .join(&options.character);
    let source_manifest_path = frame_data_manifest_path(root, &options.character);
    let output_path = frame_data_runtime_output_path(root, options.output.as_deref());
    report["source_artifact_dir"] = json!(frame_data_dir.display().to_string());
    report["source_manifest_path"] = json!(source_manifest_path.display().to_string());
    report["output_path"] = json!(output_path.display().to_string());

    if source_manifest_path.exists() {
        apply_compact_manifest_runtime_batch_export(
            root,
            options,
            &source_manifest_path,
            &output_path,
            &mut report,
        );
        return report;
    }

    let artifact_paths = match frame_data_artifact_paths(&frame_data_dir) {
        Ok(paths) => paths,
        Err(error) => {
            report["ok"] = json!(false);
            report["state_count"] = json!(0);
            report["source_artifact_paths"] = json!([]);
            report["rust_parity_gaps"] = json!([]);
            report["errors"] = json!([error]);
            return report;
        }
    };

    let mut entries = Vec::new();
    let mut source_artifact_paths = Vec::new();
    let mut rust_parity_gaps = Vec::new();
    let mut errors = Vec::new();
    let mut used_runtime_states = BTreeSet::new();

    for artifact_path in artifact_paths {
        source_artifact_paths.push(artifact_path.display().to_string());
        match read_json_object(&artifact_path) {
            Ok(artifact) => {
                let artifact_state = artifact_state_key(&artifact_path, &artifact);
                let Some(runtime_motion_state) =
                    artifact_runtime_motion_state(&artifact_state, &artifact)
                else {
                    rust_parity_gaps.push(json!({
                        "state": artifact_state,
                        "artifact_path": artifact_path.display().to_string(),
                        "source_action_key": artifact.get("source_action_key").cloned().unwrap_or(Value::Null),
                        "reason": "source artifact imported but no current Rust MotionState binding exists",
                    }));
                    continue;
                };
                if !used_runtime_states.insert(runtime_motion_state.clone()) {
                    rust_parity_gaps.push(json!({
                        "state": artifact_state,
                        "runtime_motion_state": runtime_motion_state,
                        "artifact_path": artifact_path.display().to_string(),
                        "reason": "runtime MotionState is already bound by an earlier source artifact",
                    }));
                    continue;
                }
                match RuntimeFrameDataExport::from_artifact(
                    &options.character,
                    &runtime_motion_state,
                    &artifact,
                ) {
                    Ok(export) => entries.push(RuntimeFrameDataModuleEntry {
                        export,
                        artifact_path,
                    }),
                    Err(error) => errors.push(format!("{}: {error}", artifact_path.display())),
                }
            }
            Err(error) => errors.push(error),
        }
    }

    match generate_runtime_frame_data_module(root, &entries) {
        Ok(generated) => {
            report["ok"] = json!(!entries.is_empty() && errors.is_empty());
            report["state_count"] = json!(entries.len());
            report["source_artifact_paths"] = json!(source_artifact_paths);
            report["rust_parity_gaps"] = json!(rust_parity_gaps);
            report["hitbox_frame_count"] = json!(entries
                .iter()
                .map(|entry| entry.export.hitbox_frame_count())
                .sum::<usize>());
            report["hurtbox_frame_count"] = json!(entries
                .iter()
                .map(|entry| entry.export.hurtbox_frame_count())
                .sum::<usize>());
            report["hitbox_count"] = json!(entries
                .iter()
                .map(|entry| entry.export.hitbox_count())
                .sum::<usize>());
            report["hurtbox_count"] = json!(entries
                .iter()
                .map(|entry| entry.export.hurtbox_count())
                .sum::<usize>());
            report["generated_bytes"] = json!(generated.len());
            report["errors"] = json!(errors);
            if options.write {
                match write_runtime_frame_data_module(&output_path, &generated) {
                    Ok(()) => report["wrote_output"] = json!(true),
                    Err(error) => {
                        report["ok"] = json!(false);
                        report["errors"] = json!([error]);
                    }
                }
            }
        }
        Err(error) => {
            report["ok"] = json!(false);
            report["state_count"] = json!(0);
            report["source_artifact_paths"] = json!(source_artifact_paths);
            report["rust_parity_gaps"] = json!(rust_parity_gaps);
            report["hitbox_frame_count"] = json!(0);
            report["hurtbox_frame_count"] = json!(0);
            report["hitbox_count"] = json!(0);
            report["hurtbox_count"] = json!(0);
            report["generated_bytes"] = json!(0);
            errors.push(error);
            report["errors"] = json!(errors);
        }
    }

    report
}

fn frame_data_artifact_paths(frame_data_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = fs::read_dir(frame_data_dir)
        .map_err(|error| format!("failed to read {}: {error}", frame_data_dir.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("failed to read {}: {error}", frame_data_dir.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"));
    paths.sort();
    Ok(paths)
}

fn apply_compact_manifest_runtime_batch_export(
    root: &Path,
    options: &FrameDataExportBatchOptions,
    source_manifest_path: &Path,
    output_path: &Path,
    report: &mut Value,
) {
    report["compact_manifest_detected"] = json!(true);
    report["requires_sampler"] = json!(false);
    report["requires_batched_sampler"] = json!(false);
    report["runtime_export_kind"] = json!("compact_source_export");
    report["companion_manifest_path"] = Value::Null;

    let manifest = match read_json_object(source_manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            report["ok"] = json!(false);
            report["state_count"] = json!(0);
            report["source_artifact_paths"] = json!([]);
            report["rust_parity_gaps"] = json!([]);
            report["hitbox_frame_count"] = json!(0);
            report["hurtbox_frame_count"] = json!(0);
            report["hitbox_count"] = json!(0);
            report["hurtbox_count"] = json!(0);
            report["figatree_chunk_count"] = json!(0);
            report["figatree_chunk_bytes"] = json!(0);
            report["generated_bytes"] = json!(0);
            report["errors"] = json!([error]);
            return;
        }
    };

    let Some(actions) = manifest.get("actions").and_then(Value::as_array) else {
        report["ok"] = json!(false);
        report["state_count"] = json!(0);
        report["source_artifact_paths"] = json!([source_manifest_path.display().to_string()]);
        report["rust_parity_gaps"] = json!([]);
        report["hitbox_frame_count"] = json!(0);
        report["hurtbox_frame_count"] = json!(0);
        report["hitbox_count"] = json!(0);
        report["hurtbox_count"] = json!(0);
        report["figatree_chunk_count"] = json!(0);
        report["figatree_chunk_bytes"] = json!(0);
        report["generated_bytes"] = json!(0);
        report["errors"] = json!(["compact source manifest requires actions array"]);
        return;
    };

    let mut consumed_source_action_table_ids = DERIVED_RUNTIME_SOURCE_BINDINGS
        .iter()
        .filter_map(|runtime_motion_state| {
            source_binding_for_runtime_motion_state_variant(runtime_motion_state)
                .ok()
                .flatten()
                .map(|binding| u64::from(binding.source_action_table_id))
        })
        .collect::<BTreeSet<_>>();
    consumed_source_action_table_ids.extend(
        CANONICAL_SOURCE_ONLY_ACTION_BINDINGS
            .iter()
            .map(|binding| u64::from(binding.source_action_table_id)),
    );
    let manifest_source_character = manifest.get("source_character").and_then(Value::as_str);
    consumed_source_action_table_ids.extend(
        source_special_action_bindings_for_character(manifest_source_character)
            .iter()
            .filter(|binding| binding.motion_state.is_none())
            .map(|binding| u64::from(binding.source_action_table_id)),
    );
    let mut rust_parity_gaps = Vec::new();
    let mut used_runtime_states = BTreeSet::new();

    for action in actions {
        let action_state = manifest_action_state_key(action);
        let action_table_id = action.get("action_state_id").and_then(Value::as_u64);
        if action_table_id.is_some_and(|id| consumed_source_action_table_ids.contains(&id)) {
            continue;
        }
        let Some(runtime_motion_state) = action
            .get("runtime_motion_state")
            .and_then(Value::as_str)
            .filter(|state| RUST_MOTION_STATE_VARIANTS.contains(state))
        else {
            rust_parity_gaps.push(json!({
                "state": action_state,
                "source_action_key": action.get("source_action_key").cloned().unwrap_or(Value::Null),
                "source_action_name": action.get("source_action_name").cloned().unwrap_or(Value::Null),
                "action_state_id": action.get("action_state_id").cloned().unwrap_or(Value::Null),
                "reason": "source action imported but no current Rust MotionState binding exists",
            }));
            continue;
        };
        match source_binding_for_runtime_motion_state_variant(runtime_motion_state) {
            Ok(Some(_)) => {}
            Ok(None) => continue,
            Err(error) => {
                rust_parity_gaps.push(json!({
                    "state": action_state,
                    "runtime_motion_state": runtime_motion_state,
                    "source_action_key": action.get("source_action_key").cloned().unwrap_or(Value::Null),
                    "reason": error,
                }));
                continue;
            }
        }
        if !used_runtime_states.insert(runtime_motion_state.to_string()) {
            rust_parity_gaps.push(json!({
                "state": action_state,
                "runtime_motion_state": runtime_motion_state,
                "source_action_key": action.get("source_action_key").cloned().unwrap_or(Value::Null),
                "reason": "runtime MotionState is already bound by an earlier source action",
            }));
            continue;
        }

        let _ = action_state;
    }

    match RuntimeSourceExportModule::from_manifest(
        root,
        &options.character,
        source_manifest_path,
        &manifest,
        None,
    )
    .and_then(|module| {
        let generated = generate_runtime_source_export_module(&module)?;
        Ok((module, generated))
    }) {
        Ok((module, generated)) => {
            report["ok"] = json!(module.state_count() > 0);
            report["state_count"] = json!(module.state_count());
            report["source_only_action_count"] = json!(rust_parity_gaps.len());
            report["source_artifact_paths"] = json!([source_manifest_path.display().to_string()]);
            report["rust_parity_gaps"] = json!(rust_parity_gaps);
            report["source_frame_count"] = json!(module.source_frame_count());
            report["hitbox_frame_count"] = json!(module.hitbox_frame_count());
            report["hurtbox_frame_count"] = json!(module.hurtbox_frame_count());
            report["hitbox_count"] = json!(module.hitbox_count());
            report["hurtbox_count"] = json!(module.hurtbox_count());
            report["figatree_chunk_count"] = json!(module.figatree_chunk_count());
            report["figatree_chunk_bytes"] = json!(module.figatree_chunk_bytes());
            report["generated_bytes"] = json!(generated.total_bytes());
            report["generated_module_bytes"] = json!(generated.module_text.len());
            report["generated_sidecar_bytes"] = json!(generated.sidecar_bytes());
            report["errors"] = json!([]);
            if options.write {
                match write_runtime_source_export_module(output_path, &generated) {
                    Ok(()) => report["wrote_output"] = json!(true),
                    Err(error) => {
                        report["ok"] = json!(false);
                        report["errors"] = json!([error]);
                    }
                }
            }
        }
        Err(error) => {
            report["ok"] = json!(false);
            report["state_count"] = json!(0);
            report["source_artifact_paths"] = json!([source_manifest_path.display().to_string()]);
            report["rust_parity_gaps"] = json!(rust_parity_gaps);
            report["hitbox_frame_count"] = json!(0);
            report["hurtbox_frame_count"] = json!(0);
            report["hitbox_count"] = json!(0);
            report["hurtbox_count"] = json!(0);
            report["source_frame_count"] = json!(0);
            report["figatree_chunk_count"] = json!(0);
            report["figatree_chunk_bytes"] = json!(0);
            report["generated_bytes"] = json!(0);
            report["errors"] = json!([error]);
        }
    }
}

fn manifest_action_state_key(action: &Value) -> String {
    action
        .get("state")
        .and_then(Value::as_str)
        .or_else(|| action.get("runtime_motion_state").and_then(Value::as_str))
        .or_else(|| action.get("source_action_key").and_then(Value::as_str))
        .unwrap_or("Unknown")
        .to_string()
}

fn manifest_action_for_state<'a>(manifest: &'a Value, state: &str) -> Result<&'a Value, String> {
    manifest
        .get("actions")
        .and_then(Value::as_array)
        .and_then(|actions| {
            actions.iter().find(|action| {
                action.get("state").and_then(Value::as_str) == Some(state)
                    || action.get("runtime_motion_state").and_then(Value::as_str) == Some(state)
                    || action.get("source_action_key").and_then(Value::as_str) == Some(state)
            })
        })
        .ok_or_else(|| format!("source manifest action `{state}` not found"))
}

fn manifest_action_for_action_state_id(
    manifest: &Value,
    action_state_id: u64,
) -> Result<&Value, String> {
    manifest
        .get("actions")
        .and_then(Value::as_array)
        .and_then(|actions| {
            actions.iter().find(|action| {
                action.get("action_state_id").and_then(Value::as_u64) == Some(action_state_id)
            })
        })
        .ok_or_else(|| format!("source manifest action id `{action_state_id}` not found"))
}

fn artifact_state_key(path: &Path, artifact: &Value) -> String {
    artifact
        .get("state")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Unknown".to_string())
}

fn artifact_runtime_motion_state(artifact_state: &str, artifact: &Value) -> Option<String> {
    artifact
        .get("runtime_motion_state")
        .and_then(Value::as_str)
        .filter(|state| RUST_MOTION_STATE_VARIANTS.contains(state))
        .map(str::to_string)
        .or_else(|| {
            RUST_MOTION_STATE_VARIANTS
                .contains(&artifact_state)
                .then(|| artifact_state.to_string())
        })
}

fn frame_data_artifact_path(root: &Path, character: &str, state: &str) -> PathBuf {
    root.join("resources")
        .join("melee")
        .join("frame_data")
        .join(character)
        .join(format!("{state}.json"))
}

fn frame_data_manifest_path(root: &Path, character: &str) -> PathBuf {
    root.join("resources")
        .join("melee")
        .join("frame_data")
        .join(character)
        .join("source_manifest.json")
}

fn frame_data_runtime_output_path(root: &Path, output: Option<&str>) -> PathBuf {
    output
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| {
            root.join("crates")
                .join("mole_runtime")
                .join("src")
                .join("generated")
                .join("source_frame_data.rs")
        })
}

fn write_runtime_frame_data_module(path: &Path, generated: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create runtime frame-data directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(path, generated).map_err(|error| {
        format!(
            "failed to write runtime frame-data module {}: {error}",
            path.display()
        )
    })
}

fn write_runtime_source_export_module(
    path: &Path,
    generated: &RuntimeSourceGeneratedModule,
) -> Result<(), String> {
    write_runtime_frame_data_module(path, &generated.module_text)?;
    let parent = path.parent().ok_or_else(|| {
        format!(
            "runtime source export path {} has no parent directory",
            path.display()
        )
    })?;
    let sidecar_dir = parent.join(RUNTIME_SOURCE_SIDECAR_DIR);
    if sidecar_dir.exists() {
        fs::remove_dir_all(&sidecar_dir).map_err(|error| {
            format!(
                "failed to clear runtime source sidecar directory {}: {error}",
                sidecar_dir.display()
            )
        })?;
    }
    if generated.sidecars.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(&sidecar_dir).map_err(|error| {
        format!(
            "failed to create runtime source sidecar directory {}: {error}",
            sidecar_dir.display()
        )
    })?;
    for sidecar in &generated.sidecars {
        let sidecar_path = parent.join(&sidecar.relative_path);
        if let Some(sidecar_parent) = sidecar_path.parent() {
            fs::create_dir_all(sidecar_parent).map_err(|error| {
                format!(
                    "failed to create runtime source sidecar directory {}: {error}",
                    sidecar_parent.display()
                )
            })?;
        }
        fs::write(&sidecar_path, &sidecar.bytes).map_err(|error| {
            format!(
                "failed to write runtime source sidecar {}: {error}",
                sidecar_path.display()
            )
        })?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct RuntimeFrameDataExport {
    character: String,
    state: String,
    hit_frames: Vec<RuntimeHitFrame>,
    hurt_frames: Vec<RuntimeHurtFrame>,
}

#[derive(Debug, Clone)]
struct RuntimeFrameDataModuleEntry {
    export: RuntimeFrameDataExport,
    artifact_path: PathBuf,
}

#[derive(Debug, Clone)]
struct RuntimeHitFrame {
    source_frame: u8,
    capsules: Vec<RuntimeHitCapsule>,
}

#[derive(Debug, Clone)]
struct RuntimeHurtFrame {
    source_frame: u8,
    capsules: Vec<RuntimeHurtCapsule>,
}

#[derive(Debug, Clone)]
struct RuntimeHitCapsule {
    id: u8,
    bone: u8,
    hit_group: u8,
    use_common_bone_ids: bool,
    damage: u16,
    angle: u16,
    kbg: u16,
    weight_set_kb: u16,
    bkb: u16,
    element: u16,
    shield_damage: i16,
    hit_grounded: bool,
    hit_aerial: bool,
    item_hit_interaction: bool,
    ignore_thrown_fighters: bool,
    ignore_fighter_scale: bool,
    clank: bool,
    rebound: bool,
    skip_if_thrown_hitbox_owner_absent: bool,
    hit_grabbed_victim_only: bool,
    source_handler: String,
    capsule: RuntimeCapsule,
}

#[derive(Debug, Clone)]
struct RuntimeHurtCapsule {
    id: u8,
    bone: u8,
    height: u8,
    is_grabbable: bool,
    state: String,
    capsule: RuntimeCapsule,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeCapsule {
    a: RuntimeSourcePoint,
    b: RuntimeSourcePoint,
    radius: f64,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeSourcePoint {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Clone)]
struct RuntimeSourceExportModule {
    character: String,
    source_character: Option<String>,
    manifest_text: String,
    figatree_chunks: Vec<RuntimeFigatreeChunkExport>,
    baked_source_actions: Vec<RuntimeSourceActionFrameSamples>,
    state_bindings: Vec<RuntimeStateBindingExport>,
    source_only_action_count: usize,
}

#[derive(Debug, Clone)]
struct RuntimeFigatreeChunkExport {
    source_action_key: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct RuntimeStateBindingExport {
    action_state_id: u64,
    runtime_motion_state: Option<String>,
    source_action_key: String,
}

const DERIVED_RUNTIME_SOURCE_BINDINGS: &[&str] = &[
    "RunDirect",
    "KneeBend",
    "GuardSetOff",
    "GuardReflect",
    "LandingFallSpecial",
    "EntryStart",
    "EntryEnd",
    "CliffCatch",
    "CliffWait",
];

#[derive(Debug, Clone)]
struct RuntimeSourceGeneratedModule {
    module_text: String,
    sidecars: Vec<RuntimeSourceSidecar>,
}

#[derive(Debug, Clone)]
struct RuntimeSourceSidecar {
    relative_path: PathBuf,
    bytes: Vec<u8>,
}

impl RuntimeSourceGeneratedModule {
    fn sidecar_bytes(&self) -> usize {
        self.sidecars
            .iter()
            .map(|sidecar| sidecar.bytes.len())
            .sum()
    }

    fn total_bytes(&self) -> usize {
        self.module_text.len() + self.sidecar_bytes()
    }
}

const RUNTIME_SOURCE_SIDECAR_DIR: &str = "source_frame_data";
const RUNTIME_SOURCE_FRAME_CAPSULES_FILE: &str = "source_frame_capsules.bin";
const RUNTIME_SOURCE_MANIFEST_FILE: &str = "source_manifest.json";

impl RuntimeSourceExportModule {
    fn from_manifest(
        root: &Path,
        character: &str,
        _manifest_path: &Path,
        manifest: &Value,
        state_filter: Option<&str>,
    ) -> Result<Self, String> {
        let source_character = manifest
            .get("source_character")
            .and_then(Value::as_str)
            .map(str::to_string);
        let actions = manifest
            .get("actions")
            .and_then(Value::as_array)
            .ok_or_else(|| "compact source manifest requires actions array".to_string())?;
        let action_table_path = source_action_table_path_from_manifest(root, manifest)?;
        let action_table = read_json_object(&action_table_path)?;
        let plcaaj_path = action_table_plcaaj_path(root, &action_table)?;
        let mut chunks_by_key = BTreeMap::<String, RuntimeFigatreeChunkExport>::new();
        let mut state_bindings = Vec::new();
        let mut used_runtime_states = BTreeSet::new();
        let mut source_only_action_count = 0usize;
        let mut runtime_actions = Vec::new();
        let mut runtime_action_keys = BTreeSet::new();

        for action in actions {
            if let Some(state_filter) = state_filter {
                let matches_filter = action.get("state").and_then(Value::as_str)
                    == Some(state_filter)
                    || action.get("runtime_motion_state").and_then(Value::as_str)
                        == Some(state_filter)
                    || action.get("source_action_key").and_then(Value::as_str)
                        == Some(state_filter);
                if !matches_filter {
                    continue;
                }
            }

            let runtime_motion_state = action
                .get("runtime_motion_state")
                .and_then(Value::as_str)
                .filter(|state| RUST_MOTION_STATE_VARIANTS.contains(state));
            match runtime_motion_state {
                Some(runtime_motion_state)
                    if used_runtime_states.insert(runtime_motion_state.to_string()) =>
                {
                    let Some(source_binding) =
                        source_binding_for_runtime_motion_state_variant(runtime_motion_state)?
                    else {
                        source_only_action_count += 1;
                        continue;
                    };
                    let source_action_key = source_binding.source_action_key.as_str().to_string();
                    if !chunks_by_key.contains_key(&source_action_key) {
                        chunks_by_key.insert(
                            source_action_key.clone(),
                            RuntimeFigatreeChunkExport {
                                source_action_key: source_action_key.clone(),
                                bytes: read_action_figatree_chunk(&plcaaj_path, action)?,
                            },
                        );
                    }
                    if runtime_action_keys.insert(source_action_key.clone()) {
                        runtime_actions.push(compact_runtime_manifest_action(action)?);
                    }
                    state_bindings.push(RuntimeStateBindingExport {
                        action_state_id: u64::from(source_binding.action_state_id.get()),
                        runtime_motion_state: Some(runtime_motion_state.to_string()),
                        source_action_key,
                    });
                }
                Some(_) | None => {
                    source_only_action_count += 1;
                }
            }
        }

        for runtime_motion_state in DERIVED_RUNTIME_SOURCE_BINDINGS {
            if state_filter.is_some_and(|state_filter| state_filter != *runtime_motion_state) {
                continue;
            }
            if !used_runtime_states.insert((*runtime_motion_state).to_string()) {
                continue;
            }
            let source_binding =
                source_binding_for_runtime_motion_state_variant(runtime_motion_state)?.ok_or_else(
                    || {
                        format!(
                            "derived runtime MotionState `{runtime_motion_state}` has no Melee source binding"
                        )
                    },
                )?;
            let source_action_table_id = u64::from(source_binding.source_action_table_id);
            let source_action_key = source_binding.source_action_key.as_str();
            let action = manifest_action_for_action_state_id(manifest, source_action_table_id)?;
            if !chunks_by_key.contains_key(source_action_key) {
                chunks_by_key.insert(
                    source_action_key.to_string(),
                    RuntimeFigatreeChunkExport {
                        source_action_key: source_action_key.to_string(),
                        bytes: read_action_figatree_chunk(&plcaaj_path, action)?,
                    },
                );
            }
            if runtime_action_keys.insert(source_action_key.to_string()) {
                runtime_actions.push(compact_runtime_manifest_action(action)?);
            }
            if action
                .get("runtime_motion_state")
                .and_then(Value::as_str)
                .filter(|state| RUST_MOTION_STATE_VARIANTS.contains(state))
                .is_none()
            {
                source_only_action_count = source_only_action_count.saturating_sub(1);
            }
            state_bindings.push(RuntimeStateBindingExport {
                action_state_id: u64::from(source_binding.action_state_id.get()),
                runtime_motion_state: Some((*runtime_motion_state).to_string()),
                source_action_key: source_action_key.to_string(),
            });
        }

        for source_binding in CANONICAL_SOURCE_ONLY_ACTION_BINDINGS {
            let source_action_key = source_binding.source_action_key.as_str();
            if state_filter.is_some_and(|state_filter| state_filter != source_action_key) {
                continue;
            }
            let source_action_table_id = u64::from(source_binding.source_action_table_id);
            let action = manifest_action_for_action_state_id(manifest, source_action_table_id)?;
            if !chunks_by_key.contains_key(source_action_key) {
                chunks_by_key.insert(
                    source_action_key.to_string(),
                    RuntimeFigatreeChunkExport {
                        source_action_key: source_action_key.to_string(),
                        bytes: read_action_figatree_chunk(&plcaaj_path, action)?,
                    },
                );
            }
            if runtime_action_keys.insert(source_action_key.to_string()) {
                runtime_actions.push(compact_runtime_manifest_action(action)?);
            }
            source_only_action_count = source_only_action_count.saturating_sub(1);
            state_bindings.push(RuntimeStateBindingExport {
                action_state_id: u64::from(source_binding.action_state_id.get()),
                runtime_motion_state: None,
                source_action_key: source_action_key.to_string(),
            });
        }

        for source_binding in
            source_special_action_bindings_for_character(source_character.as_deref())
                .iter()
                .filter(|binding| binding.motion_state.is_none())
        {
            let source_action_key = source_binding.source_action_key.as_str();
            if state_filter.is_some_and(|state_filter| state_filter != source_action_key) {
                continue;
            }
            let source_action_table_id = u64::from(source_binding.source_action_table_id);
            let action = manifest_action_for_action_state_id(manifest, source_action_table_id)?;
            if !chunks_by_key.contains_key(source_action_key) {
                chunks_by_key.insert(
                    source_action_key.to_string(),
                    RuntimeFigatreeChunkExport {
                        source_action_key: source_action_key.to_string(),
                        bytes: read_action_figatree_chunk(&plcaaj_path, action)?,
                    },
                );
            }
            if runtime_action_keys.insert(source_action_key.to_string()) {
                runtime_actions.push(compact_runtime_manifest_action(action)?);
            }
            source_only_action_count = source_only_action_count.saturating_sub(1);
            state_bindings.push(RuntimeStateBindingExport {
                action_state_id: u64::from(source_binding.action_state_id.get()),
                runtime_motion_state: None,
                source_action_key: source_action_key.to_string(),
            });
        }

        if state_filter.is_some() && state_bindings.is_empty() {
            return Err(format!(
                "compact source manifest action `{}` has no current Rust MotionState binding",
                state_filter.unwrap_or_default()
            ));
        }
        let manifest_text = compact_runtime_source_manifest(manifest, runtime_actions)?;
        let figatree_chunks = chunks_by_key.into_values().collect::<Vec<_>>();
        let baked_source_actions = bake_runtime_source_frame_capsules(
            character,
            source_character.as_deref(),
            &manifest_text,
            &figatree_chunks,
        )?;

        Ok(Self {
            character: character.to_string(),
            source_character,
            manifest_text,
            figatree_chunks,
            baked_source_actions,
            state_bindings,
            source_only_action_count,
        })
    }

    fn figatree_chunk_count(&self) -> usize {
        self.figatree_chunks.len()
    }

    fn figatree_chunk_bytes(&self) -> usize {
        self.figatree_chunks
            .iter()
            .map(|chunk| chunk.bytes.len())
            .sum()
    }

    fn state_count(&self) -> usize {
        self.state_bindings.len()
    }

    fn source_frame_count(&self) -> usize {
        self.baked_source_actions
            .iter()
            .map(|action| action.frames.len())
            .sum()
    }

    fn hitbox_count(&self) -> usize {
        self.baked_source_actions
            .iter()
            .flat_map(|action| &action.frames)
            .map(|frame| frame.hit_capsules.len())
            .sum()
    }

    fn hurtbox_count(&self) -> usize {
        self.baked_source_actions
            .iter()
            .flat_map(|action| &action.frames)
            .map(|frame| frame.hurt_capsules.len())
            .sum()
    }

    fn hitbox_frame_count(&self) -> usize {
        self.baked_source_actions
            .iter()
            .flat_map(|action| &action.frames)
            .filter(|frame| !frame.hit_capsules.is_empty())
            .count()
    }

    fn hurtbox_frame_count(&self) -> usize {
        self.baked_source_actions
            .iter()
            .flat_map(|action| &action.frames)
            .filter(|frame| !frame.hurt_capsules.is_empty())
            .count()
    }
}

fn bake_runtime_source_frame_capsules(
    character: &str,
    source_character: Option<&str>,
    manifest_text: &str,
    figatree_chunks: &[RuntimeFigatreeChunkExport],
) -> Result<Vec<RuntimeSourceActionFrameSamples>, String> {
    let runtime_chunks = figatree_chunks
        .iter()
        .map(|chunk| RuntimeFigatreeChunk {
            source_action_key: chunk.source_action_key.as_str(),
            bytes: chunk.bytes.as_slice(),
        })
        .collect::<Vec<_>>();
    let source_export = RuntimeSourceExport {
        character,
        source_character,
        manifest_json: manifest_text,
        figatree_chunks: &runtime_chunks,
    };
    let source_export = RuntimeSourceExportEvaluator::from_export(source_export)?;
    figatree_chunks
        .iter()
        .map(|chunk| {
            let evaluator = source_export.action_frame_evaluator(&FrameDataSampleOptions {
                character: character.to_string(),
                source_character: source_character.map(str::to_string),
                state: chunk.source_action_key.clone(),
                frame: 1,
            })?;
            let total_frames = evaluator.total_frames().min(u64::from(u8::MAX));
            let frames = (1..=total_frames)
                .map(|frame| evaluator.sample_frame_capsules(frame))
                .collect::<Result<Vec<_>, _>>()?;
            let script_events = evaluator.script_events()?;
            let cmd_var_events = evaluator.cmd_var_events()?;
            Ok(RuntimeSourceActionFrameSamples {
                source_action_key: chunk.source_action_key.clone(),
                total_frames: total_frames as u8,
                frames,
                cmd_var_events,
                script_events,
            })
        })
        .collect()
}

fn generate_runtime_source_export_module(
    module: &RuntimeSourceExportModule,
) -> Result<RuntimeSourceGeneratedModule, String> {
    let mut output = String::new();
    output.push_str(concat!(
        "// Generated compact Melee source frame-data export.\n",
        "// Runtime loads CLI-baked frame sidecars and compact source JObj/FigaTree artifacts.\n",
        "// Gameplay must not depend on raw ISO/decomp paths.\n\n",
        "// Canonical Melee action-state bindings may have no Rust MotionState alias.\n",
        "use mole_core::{MeleeActionStateId, MotionState};\n",
        "use mole_frame_data::{RuntimeFigatreeChunk, RuntimeSourceExport};\n",
        "pub(crate) const SOURCE_ARTIFACT_KIND: &str = \"runtime_source_frame_data\";\n",
    ));
    output.push_str(&format!(
        "pub(crate) const SOURCE_FRAME_CAPSULES_BYTES: &[u8] = include_bytes!(\"{RUNTIME_SOURCE_SIDECAR_DIR}/{RUNTIME_SOURCE_FRAME_CAPSULES_FILE}\");\n\n",
    ));
    output.push_str(&format!(
        "pub(crate) const SOURCE_MANIFEST_JSON: &str = include_str!(\"{RUNTIME_SOURCE_SIDECAR_DIR}/{RUNTIME_SOURCE_MANIFEST_FILE}\");\n"
    ));
    output.push_str(&format!(
        "pub(crate) const SOURCE_CHARACTER: Option<&str> = {};\n",
        option_rust_string_literal(module.source_character.as_deref())
    ));
    output.push_str(
        "pub(crate) const SOURCE_FIGATREE_CHUNKS: &[RuntimeFigatreeChunk<'static>] = &[\n",
    );
    for chunk in &module.figatree_chunks {
        let file_name = runtime_source_figatree_file_name(&chunk.source_action_key)?;
        output.push_str(&format!(
            "    RuntimeFigatreeChunk {{ source_action_key: {}, bytes: include_bytes!(\"{RUNTIME_SOURCE_SIDECAR_DIR}/{file_name}\") }},\n",
            rust_string_literal(&chunk.source_action_key)
        ));
    }
    output.push_str("];\n\n");
    output.push_str(&format!(
        "pub(crate) fn runtime_source_export() -> RuntimeSourceExport<'static> {{\n    RuntimeSourceExport {{\n        character: {},\n        source_character: SOURCE_CHARACTER,\n        manifest_json: SOURCE_MANIFEST_JSON,\n        figatree_chunks: SOURCE_FIGATREE_CHUNKS,\n    }}\n}}\n\n",
        rust_string_literal(&module.character)
    ));

    output.push_str(concat!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n",
        "pub(crate) struct RuntimeActionBinding {\n",
        "    pub(crate) action_state_id: MeleeActionStateId,\n",
        "    pub(crate) source_action_key: &'static str,\n",
        "    pub(crate) motion_state: Option<MotionState>,\n",
        "}\n\n",
        "pub(crate) const ACTION_BINDINGS: &[RuntimeActionBinding] = &[\n",
    ));
    for binding in &module.state_bindings {
        let motion_state = match binding.runtime_motion_state.as_deref() {
            Some(runtime_motion_state) => {
                let state_variant =
                    validate_current_rust_motion_state_variant(runtime_motion_state)?;
                format!("Some(MotionState::{state_variant})")
            }
            None => "None".to_string(),
        };
        output.push_str(&format!(
            "    RuntimeActionBinding {{ action_state_id: MeleeActionStateId::new({}), source_action_key: {}, motion_state: {motion_state} }},\n",
            binding.action_state_id,
            rust_string_literal(&binding.source_action_key)
        ));
    }
    output.push_str(concat!(
        "];\n\n",
        "pub(crate) fn action_binding_for_motion_state(state: MotionState) -> Option<&'static RuntimeActionBinding> {\n",
        "    ACTION_BINDINGS\n",
        "        .iter()\n",
        "        .find(|binding| binding.motion_state == Some(state))\n",
        "}\n\n",
        "pub(crate) fn source_action_key_for_action_state_id(\n",
        "    action_state_id: MeleeActionStateId,\n",
        ") -> Option<&'static str> {\n",
        "    ACTION_BINDINGS\n",
        "        .iter()\n",
        "        .find(|binding| binding.action_state_id == action_state_id)\n",
        "        .map(|binding| binding.source_action_key)\n",
        "}\n\n",
        "pub(crate) fn source_action_key_for_state(state: MotionState) -> Option<&'static str> {\n",
        "    action_binding_for_motion_state(state).map(|binding| binding.source_action_key)\n",
        "}\n",
    ));

    let mut sidecars = vec![
        RuntimeSourceSidecar {
            relative_path: PathBuf::from(RUNTIME_SOURCE_SIDECAR_DIR)
                .join(RUNTIME_SOURCE_FRAME_CAPSULES_FILE),
            bytes: encode_runtime_source_frame_capsules(&module.baked_source_actions)?,
        },
        RuntimeSourceSidecar {
            relative_path: PathBuf::from(RUNTIME_SOURCE_SIDECAR_DIR)
                .join(RUNTIME_SOURCE_MANIFEST_FILE),
            bytes: module.manifest_text.as_bytes().to_vec(),
        },
    ];
    for chunk in &module.figatree_chunks {
        sidecars.push(RuntimeSourceSidecar {
            relative_path: PathBuf::from(RUNTIME_SOURCE_SIDECAR_DIR)
                .join(runtime_source_figatree_file_name(&chunk.source_action_key)?),
            bytes: chunk.bytes.clone(),
        });
    }

    Ok(RuntimeSourceGeneratedModule {
        module_text: output,
        sidecars,
    })
}

fn runtime_source_figatree_file_name(source_action_key: &str) -> Result<String, String> {
    if !source_action_key
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(format!(
            "source action key `{source_action_key}` cannot be used as a runtime sidecar file name"
        ));
    }
    Ok(format!("{source_action_key}.figatree.bin"))
}

fn option_rust_string_literal(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("Some({})", rust_string_literal(value)),
        None => "None".to_string(),
    }
}

fn compact_runtime_source_manifest(
    manifest: &Value,
    actions: Vec<Value>,
) -> Result<String, String> {
    let skeleton_joints =
        required_manifest_array(manifest, &["rig", "skeleton", "data", "joints"])?
            .iter()
            .map(compact_runtime_skeleton_joint)
            .collect::<Result<Vec<_>, _>>()?;
    let hurtboxes =
        required_manifest_array(manifest, &["rig", "hurtbox_inits", "data", "hurtboxes"])?
            .iter()
            .map(compact_runtime_hurtbox_init)
            .collect::<Result<Vec<_>, _>>()?;
    let common_parts = compact_runtime_common_parts(required_manifest_value(
        manifest,
        &["rig", "common_parts", "data"],
    )?)?;
    let compact = json!({
        "schema_version": 2,
        "artifact_kind": "runtime_source_frame_data_manifest",
        "target_character": manifest.get("target_character").cloned().unwrap_or(Value::Null),
        "source_character": manifest.get("source_character").cloned().unwrap_or(Value::Null),
        "source_space": manifest.get("source_space").cloned().unwrap_or_else(|| json!("melee_xyz")),
        "projection": manifest.get("projection").cloned().unwrap_or_else(|| json!({})),
        "rig": {
            "live_pose_setup": compact_runtime_live_pose_setup(manifest)?,
            "common_parts": {
                "data": common_parts,
            },
            "skeleton": {
                "data": {
                    "joints": skeleton_joints,
                },
            },
            "hurtbox_inits": {
                "data": {
                    "hurtboxes": hurtboxes,
                },
            },
        },
        "actions": actions,
    });
    serde_json::to_string(&compact)
        .map_err(|error| format!("failed to serialize compact runtime source manifest: {error}"))
}

fn compact_runtime_live_pose_setup(manifest: &Value) -> Result<Value, String> {
    Ok(json!({
        "topn_rot_y_radians": required_manifest_value(
            manifest,
            &["rig", "live_pose_setup", "topn_rot_y_radians"],
        )?
        .clone(),
        "topn_scale": required_manifest_value(manifest, &["rig", "live_pose_setup", "topn_scale"])?
            .clone(),
    }))
}

fn compact_runtime_common_parts(common_parts: &Value) -> Result<Value, String> {
    Ok(json!({
        "capture_anchor_part": required_manifest_value(common_parts, &["capture_anchor_part"])?.clone(),
        "hipn_joint": required_manifest_value(common_parts, &["hipn_joint"])?.clone(),
        "hipn_part": required_manifest_value(common_parts, &["hipn_part"])?.clone(),
        "transn_joint": required_manifest_value(common_parts, &["transn_joint"])?.clone(),
        "transn_part": required_manifest_value(common_parts, &["transn_part"])?.clone(),
        "transn2_joint": required_manifest_value(common_parts, &["transn2_joint"])?.clone(),
        "transn2_part": required_manifest_value(common_parts, &["transn2_part"])?.clone(),
        "thrown_hitbox_joint": required_manifest_value(common_parts, &["thrown_hitbox_joint"])?.clone(),
        "thrown_hitbox_part": required_manifest_value(common_parts, &["thrown_hitbox_part"])?.clone(),
        "thrown_hitbox_scale": required_manifest_value(common_parts, &["thrown_hitbox_scale"])?.clone(),
        "xrotn_joint": required_manifest_value(common_parts, &["xrotn_joint"])?.clone(),
        "xrotn_part": required_manifest_value(common_parts, &["xrotn_part"])?.clone(),
    }))
}

fn compact_runtime_manifest_action(action: &Value) -> Result<Value, String> {
    let figatree = required_manifest_value(action, &["source_action", "figatree"])?;
    Ok(json!({
        "state": action.get("state").cloned().unwrap_or(Value::Null),
        "runtime_motion_state": action.get("runtime_motion_state").cloned().unwrap_or(Value::Null),
        "source_action_key": action.get("source_action_key").cloned().unwrap_or(Value::Null),
        "source_action_name": action.get("source_action_name").cloned().unwrap_or(Value::Null),
        "action_state_id": required_manifest_value(action, &["action_state_id"])?.clone(),
        "total_frames": required_manifest_value(action, &["total_frames"])?.clone(),
        "source_action": {
            "figatree": compact_runtime_figatree(figatree)?,
        },
        "decoded_action_script": {
            "procedures": compact_runtime_procedures(action)?,
        },
    }))
}

fn compact_runtime_figatree(figatree: &Value) -> Result<Value, String> {
    let tracks = required_manifest_array(figatree, &["tracks"])?
        .iter()
        .map(compact_runtime_figatree_track)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "root": figatree.get("root").cloned().unwrap_or(Value::Null),
        "track_counts_by_node": required_manifest_value(figatree, &["track_counts_by_node"])?.clone(),
        "tracks": tracks,
    }))
}

fn compact_runtime_figatree_track(track: &Value) -> Result<Value, String> {
    Ok(json!({
        "data_offset": required_manifest_value(track, &["data_offset"])?.clone(),
        "frac_slope_raw": required_manifest_value(track, &["frac_slope_raw"])?.clone(),
        "frac_value_raw": required_manifest_value(track, &["frac_value_raw"])?.clone(),
        "length": required_manifest_value(track, &["length"])?.clone(),
        "obj_type": required_manifest_value(track, &["obj_type"])?.clone(),
        "startframe": required_manifest_value(track, &["startframe"])?.clone(),
    }))
}

fn compact_runtime_skeleton_joint(joint: &Value) -> Result<Value, String> {
    Ok(json!({
        "flags_raw": required_manifest_value(joint, &["flags_raw"])?.clone(),
        "parent_index": joint.get("parent_index").cloned().unwrap_or(Value::Null),
        "position_raw": required_manifest_value(joint, &["position_raw"])?.clone(),
        "rotation_raw": required_manifest_value(joint, &["rotation_raw"])?.clone(),
        "scale_raw": required_manifest_value(joint, &["scale_raw"])?.clone(),
    }))
}

fn compact_runtime_hurtbox_init(hurtbox: &Value) -> Result<Value, String> {
    Ok(json!({
        "a_offset_raw": required_manifest_value(hurtbox, &["a_offset_raw"])?.clone(),
        "b_offset_raw": required_manifest_value(hurtbox, &["b_offset_raw"])?.clone(),
        "bone_idx": required_manifest_value(hurtbox, &["bone_idx"])?.clone(),
        "height": required_manifest_value(hurtbox, &["height"])?.clone(),
        "id": required_manifest_value(hurtbox, &["id"])?.clone(),
        "is_grabbable": required_manifest_value(hurtbox, &["is_grabbable"])?.clone(),
        "scale_raw": required_manifest_value(hurtbox, &["scale_raw"])?.clone(),
    }))
}

fn compact_runtime_procedures(action: &Value) -> Result<Vec<Value>, String> {
    Ok(
        required_manifest_array(action, &["decoded_action_script", "procedures"])?
            .iter()
            .filter_map(|procedure| {
                let name = procedure.get("procedure").and_then(Value::as_str)?;
                matches!(
                name,
                "fighter.spawn_hitbox"
                    | "fighter.clear_all_hitboxes"
                    | "fighter.set_cmd_var"
                    | "fighter.set_hurt_state"
                    | "fighter.set_throw_flag"
                    | "fighter.set_throw_hitbox"
                    | "fighter.set_jab_combo"
                    | "fighter.set_jab_rapid"
            )
            .then(|| {
                let mut compact = json!({
                    "procedure": name,
                    "frame": procedure.get("frame").cloned().unwrap_or(Value::Null),
                    "word_offset": procedure.get("word_offset").cloned().unwrap_or(Value::Null),
                    "raw_words": procedure.get("raw_words").cloned().unwrap_or_else(|| json!([])),
                });
                if let Some(disabled) = procedure.get("disabled") {
                    compact["disabled"] = disabled.clone();
                }
                if let Some(state) = procedure.get("state") {
                    compact["state"] = state.clone();
                }
                if let Some(hit_idx) = procedure.get("hit_idx") {
                    compact["hit_idx"] = hit_idx.clone();
                }
                if let Some(cmd_var) = procedure.get("cmd_var") {
                    compact["cmd_var"] = cmd_var.clone();
                }
                if let Some(value) = procedure.get("value") {
                    compact["value"] = value.clone();
                }
                if let Some(flag_bit) = procedure.get("flag_bit") {
                    compact["flag_bit"] = flag_bit.clone();
                }
                if let Some(hitbox_idx) = procedure.get("hitbox_idx") {
                    compact["hitbox_idx"] = hitbox_idx.clone();
                }
                if let Some(damage) = procedure.get("damage") {
                    compact["damage"] = damage.clone();
                }
                if let Some(angle) = procedure.get("angle") {
                    compact["angle"] = angle.clone();
                }
                if let Some(hit_x24) = procedure.get("hit_x24") {
                    compact["hit_x24"] = hit_x24.clone();
                }
                if let Some(hit_x28) = procedure.get("hit_x28") {
                    compact["hit_x28"] = hit_x28.clone();
                }
                if let Some(hit_x2c) = procedure.get("hit_x2c") {
                    compact["hit_x2c"] = hit_x2c.clone();
                }
                if let Some(element) = procedure.get("element") {
                    compact["element"] = element.clone();
                }
                if let Some(sfx_severity) = procedure.get("sfx_severity") {
                    compact["sfx_severity"] = sfx_severity.clone();
                }
                if let Some(sfx_kind) = procedure.get("sfx_kind") {
                    compact["sfx_kind"] = sfx_kind.clone();
                }
                compact
            })
            })
            .collect(),
    )
}

fn required_manifest_value<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    let mut current = value;
    for field in path {
        current = current.get(field).ok_or_else(|| {
            format!(
                "runtime source manifest is missing required field `{}`",
                path.join(".")
            )
        })?;
    }
    Ok(current)
}

fn required_manifest_array<'a>(value: &'a Value, path: &[&str]) -> Result<&'a [Value], String> {
    required_manifest_value(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| {
            format!(
                "runtime source manifest field `{}` must be an array",
                path.join(".")
            )
        })
}

fn source_action_table_path_from_manifest(
    root: &Path,
    manifest: &Value,
) -> Result<PathBuf, String> {
    let relative = manifest
        .get("sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources.iter().find_map(|source| {
                (source.get("kind").and_then(Value::as_str) == Some("source_action_table"))
                    .then(|| source.get("path").and_then(Value::as_str))
                    .flatten()
            })
        })
        .ok_or_else(|| "source manifest is missing source_action_table path".to_string())?;
    Ok(root.join(relative))
}

fn action_table_plcaaj_path(root: &Path, action_table: &Value) -> Result<PathBuf, String> {
    let relative = action_table
        .get("metadata")
        .and_then(|metadata| metadata.get("plcaaj_file"))
        .or_else(|| {
            action_table
                .get("source")
                .and_then(|source| source.get("plcaaj_file"))
        })
        .or_else(|| action_table.get("plcaaj_file"))
        .and_then(Value::as_str)
        .ok_or_else(|| "action animation table is missing plcaaj_file metadata".to_string())?;
    Ok(root.join(relative))
}

fn read_action_figatree_chunk(path: &Path, action: &Value) -> Result<Vec<u8>, String> {
    let source_action = action.get("source_action").ok_or_else(|| {
        format!(
            "manifest action {} is missing source_action metadata",
            manifest_action_state_key(action)
        )
    })?;
    let offset = source_action
        .get("figatree_archive_offset")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "manifest action {} is missing figatree_archive_offset",
                manifest_action_state_key(action)
            )
        })? as usize;
    let size = source_action
        .get("figatree_archive_size")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "manifest action {} is missing figatree_archive_size",
                manifest_action_state_key(action)
            )
        })? as usize;
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let end = offset.saturating_add(size);
    if end > bytes.len() {
        return Err(format!(
            "figatree archive slice {}..{} is outside {} bytes in {}",
            offset,
            end,
            bytes.len(),
            path.display()
        ));
    }
    Ok(bytes[offset..end].to_vec())
}

fn generate_runtime_frame_data_module(
    root: &Path,
    entries: &[RuntimeFrameDataModuleEntry],
) -> Result<String, String> {
    let mut output = String::new();
    let artifact_paths = entries
        .iter()
        .map(|entry| path_for_artifact(root, &entry.artifact_path))
        .collect::<Vec<_>>();
    output.push_str(&format!(
        "// Generated from {} frame-data artifact(s).\n",
        artifact_paths.len()
    ));
    for artifact_path in &artifact_paths {
        output.push_str(&format!("// - {artifact_path}\n"));
    }
    output.push_str(concat!(
        "// Source-space coordinates preserve Melee XYZ; runtime render helpers flatten for current 2D play.\n",
        "// Hit capsules use ftColl_8007AD18 previous/current centers from ftAction_8007121C data.\n",
        "// Hurt capsules use ftData.x30 samples transformed by the action FigaTree/JObj skeleton.\n\n",
        "use mole_core::MotionState;\n\n",
        "// Baked from source_action_frame_sample middleware samples.\n",
        "pub(crate) const SOURCE_ARTIFACT_KIND: &str = \"baked_frame_data_boxes\";\n",
        "\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub(crate) struct SourcePoint {\n",
        "    pub(crate) x: f64,\n",
        "    pub(crate) y: f64,\n",
        "    pub(crate) z: f64,\n",
        "}\n\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub(crate) struct SourceCapsule {\n",
        "    pub(crate) a: SourcePoint,\n",
        "    pub(crate) b: SourcePoint,\n",
        "    pub(crate) radius: f64,\n",
        "}\n\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub(crate) struct SourceHitCapsule {\n",
        "    pub(crate) id: u8,\n",
        "    pub(crate) bone: u8,\n",
        "    pub(crate) hit_group: u8,\n",
        "    pub(crate) use_common_bone_ids: bool,\n",
        "    pub(crate) damage: u16,\n",
        "    pub(crate) angle: u16,\n",
        "    pub(crate) kbg: u16,\n",
        "    pub(crate) weight_set_kb: u16,\n",
        "    pub(crate) bkb: u16,\n",
        "    pub(crate) element: u16,\n",
        "    pub(crate) shield_damage: i16,\n",
        "    pub(crate) hit_grounded: bool,\n",
        "    pub(crate) hit_aerial: bool,\n",
        "    pub(crate) item_hit_interaction: bool,\n",
        "    pub(crate) ignore_thrown_fighters: bool,\n",
        "    pub(crate) ignore_fighter_scale: bool,\n",
        "    pub(crate) clank: bool,\n",
        "    pub(crate) rebound: bool,\n",
        "    pub(crate) skip_if_thrown_hitbox_owner_absent: bool,\n",
        "    pub(crate) hit_grabbed_victim_only: bool,\n",
        "    pub(crate) source_handler: &'static str,\n",
        "    pub(crate) capsule: SourceCapsule,\n",
        "}\n\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub(crate) struct SourceHurtCapsule {\n",
        "    pub(crate) id: u8,\n",
        "    pub(crate) bone: u8,\n",
        "    pub(crate) height: u8,\n",
        "    pub(crate) is_grabbable: bool,\n",
        "    pub(crate) state: &'static str,\n",
        "    pub(crate) capsule: SourceCapsule,\n",
        "}\n\n",
        "const EMPTY_HIT_CAPSULES: &[SourceHitCapsule] = &[];\n",
        "const EMPTY_HURT_CAPSULES: &[SourceHurtCapsule] = &[];\n\n",
    ));

    for entry in entries {
        let export = &entry.export;
        let const_prefix = format!(
            "{}_{}",
            screaming_snake_identifier(&export.character),
            screaming_snake_identifier(&export.state)
        );
        for frame in &export.hit_frames {
            output.push_str(&format!(
                "const {const_prefix}_HIT_FRAME_{}: [SourceHitCapsule; {}] = [\n",
                frame.source_frame,
                frame.capsules.len()
            ));
            for capsule in &frame.capsules {
                output.push_str(&indent_multiline(&capsule.rust_literal(), 4));
                output.push_str(",\n");
            }
            output.push_str("];\n\n");
        }
        for frame in &export.hurt_frames {
            output.push_str(&format!(
                "const {const_prefix}_HURT_FRAME_{}: [SourceHurtCapsule; {}] = [\n",
                frame.source_frame,
                frame.capsules.len()
            ));
            for capsule in &frame.capsules {
                output.push_str(&indent_multiline(&capsule.rust_literal(), 4));
                output.push_str(",\n");
            }
            output.push_str("];\n\n");
        }
    }

    output.push_str(concat!(
        "pub(crate) fn hit_capsules(\n",
        "    character: &str,\n",
        "    state: MotionState,\n",
        "    source_frame: u8,\n",
        ") -> &'static [SourceHitCapsule] {\n",
        "    match (character, state, source_frame) {\n",
    ));
    for entry in entries {
        let export = &entry.export;
        let state_variant = validate_current_rust_motion_state_variant(&export.state)?;
        let const_prefix = format!(
            "{}_{}",
            screaming_snake_identifier(&export.character),
            screaming_snake_identifier(&export.state)
        );
        for frame in &export.hit_frames {
            output.push_str(&format!(
                "        ({}, MotionState::{state_variant}, {}) => &{const_prefix}_HIT_FRAME_{},\n",
                rust_string_literal(&export.character),
                frame.source_frame,
                frame.source_frame
            ));
        }
    }
    output.push_str("        _ => EMPTY_HIT_CAPSULES,\n    }\n}\n\n");

    output.push_str(concat!(
        "pub(crate) fn hurt_capsules(\n",
        "    character: &str,\n",
        "    state: MotionState,\n",
        "    source_frame: u8,\n",
        ") -> &'static [SourceHurtCapsule] {\n",
        "    match (character, state, source_frame) {\n",
    ));
    for entry in entries {
        let export = &entry.export;
        let state_variant = validate_current_rust_motion_state_variant(&export.state)?;
        let const_prefix = format!(
            "{}_{}",
            screaming_snake_identifier(&export.character),
            screaming_snake_identifier(&export.state)
        );
        for frame in &export.hurt_frames {
            output.push_str(&format!(
                "        ({}, MotionState::{state_variant}, {}) => &{const_prefix}_HURT_FRAME_{},\n",
                rust_string_literal(&export.character),
                frame.source_frame,
                frame.source_frame
            ));
        }
    }
    output.push_str("        _ => EMPTY_HURT_CAPSULES,\n    }\n}\n");

    Ok(output)
}

impl RuntimeFrameDataExport {
    fn from_artifact(character: &str, state: &str, artifact: &Value) -> Result<Self, String> {
        validate_current_rust_motion_state_variant(state)?;
        let keyframes = artifact
            .get("keyframes")
            .and_then(Value::as_array)
            .ok_or_else(|| "frame-data artifact requires keyframes array".to_string())?;
        let mut hit_frames = Vec::new();
        let mut hurt_frames = Vec::new();

        for keyframe in keyframes {
            let Some(source_frame) = keyframe
                .get("frame")
                .and_then(Value::as_u64)
                .and_then(|frame| u8::try_from(frame).ok())
            else {
                continue;
            };

            let hit_capsules = keyframe
                .get("hitboxes")
                .and_then(Value::as_array)
                .map(|hitboxes| {
                    hitboxes
                        .iter()
                        .map(RuntimeHitCapsule::from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            if !hit_capsules.is_empty() {
                hit_frames.push(RuntimeHitFrame {
                    source_frame,
                    capsules: hit_capsules,
                });
            }

            let hurt_capsules = keyframe
                .get("hurtboxes")
                .and_then(Value::as_array)
                .map(|hurtboxes| {
                    hurtboxes
                        .iter()
                        .map(RuntimeHurtCapsule::from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            if !hurt_capsules.is_empty() {
                hurt_frames.push(RuntimeHurtFrame {
                    source_frame,
                    capsules: hurt_capsules,
                });
            }
        }

        Ok(Self {
            character: character.to_string(),
            state: state.to_string(),
            hit_frames,
            hurt_frames,
        })
    }

    fn hitbox_frame_count(&self) -> usize {
        self.hit_frames.len()
    }

    fn hurtbox_frame_count(&self) -> usize {
        self.hurt_frames.len()
    }

    fn hitbox_count(&self) -> usize {
        self.hit_frames
            .iter()
            .map(|frame| frame.capsules.len())
            .sum()
    }

    fn hurtbox_count(&self) -> usize {
        self.hurt_frames
            .iter()
            .map(|frame| frame.capsules.len())
            .sum()
    }
}

impl RuntimeHitCapsule {
    fn from_value(value: &Value) -> Result<Self, String> {
        let source_center = source_point(value, &["source_center", "center"])
            .ok_or_else(|| "hitbox requires source_center or center".to_string())?;
        let source_previous_center =
            source_point(value, &["source_previous_center", "previous_center"])
                .unwrap_or(source_center);
        let radius = required_f64(value, "radius")?;
        Ok(Self {
            id: optional_u8(value, "id"),
            bone: optional_u8(value, "bone"),
            hit_group: optional_u8(value, "hit_group"),
            use_common_bone_ids: optional_bool(value, "use_common_bone_ids"),
            damage: optional_u16(value, "damage"),
            angle: optional_u16(value, "angle"),
            kbg: optional_u16(value, "kbg"),
            weight_set_kb: optional_u16(value, "weight_set_kb"),
            bkb: optional_u16(value, "bkb"),
            element: optional_u16(value, "element"),
            shield_damage: optional_i16(value, "shield_damage"),
            hit_grounded: optional_bool(value, "hit_grounded"),
            hit_aerial: optional_bool(value, "hit_aerial"),
            item_hit_interaction: optional_bool(value, "item_hit_interaction"),
            ignore_thrown_fighters: optional_bool(value, "ignore_thrown_fighters"),
            ignore_fighter_scale: optional_bool(value, "ignore_fighter_scale"),
            clank: optional_bool(value, "clank"),
            rebound: optional_bool(value, "rebound"),
            skip_if_thrown_hitbox_owner_absent: optional_bool(
                value,
                "skip_if_thrown_hitbox_owner_absent",
            ),
            hit_grabbed_victim_only: optional_bool(value, "hit_grabbed_victim_only"),
            source_handler: value
                .get("source_handler")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            capsule: RuntimeCapsule {
                a: source_previous_center,
                b: source_center,
                radius,
            },
        })
    }

    fn rust_literal(&self) -> String {
        let capsule = indent_after_first_line(&self.capsule.rust_literal(), 4);
        format!(
            "SourceHitCapsule {{\n    id: {},\n    bone: {},\n    hit_group: {},\n    use_common_bone_ids: {},\n    damage: {},\n    angle: {},\n    kbg: {},\n    weight_set_kb: {},\n    bkb: {},\n    element: {},\n    shield_damage: {},\n    hit_grounded: {},\n    hit_aerial: {},\n    item_hit_interaction: {},\n    ignore_thrown_fighters: {},\n    ignore_fighter_scale: {},\n    clank: {},\n    rebound: {},\n    skip_if_thrown_hitbox_owner_absent: {},\n    hit_grabbed_victim_only: {},\n    source_handler: {},\n    capsule: {},\n}}",
            self.id,
            self.bone,
            self.hit_group,
            self.use_common_bone_ids,
            self.damage,
            self.angle,
            self.kbg,
            self.weight_set_kb,
            self.bkb,
            self.element,
            self.shield_damage,
            self.hit_grounded,
            self.hit_aerial,
            self.item_hit_interaction,
            self.ignore_thrown_fighters,
            self.ignore_fighter_scale,
            self.clank,
            self.rebound,
            self.skip_if_thrown_hitbox_owner_absent,
            self.hit_grabbed_victim_only,
            rust_string_literal(&self.source_handler),
            capsule,
        )
    }
}

impl RuntimeHurtCapsule {
    fn from_value(value: &Value) -> Result<Self, String> {
        let source_a = source_point(value, &["source_a", "source_a_pos", "a", "a_pos"])
            .ok_or_else(|| "hurtbox requires source_a/a coordinates".to_string())?;
        let source_b = source_point(value, &["source_b", "source_b_pos", "b", "b_pos"])
            .ok_or_else(|| "hurtbox requires source_b/b coordinates".to_string())?;
        let radius = value
            .get("radius")
            .or_else(|| value.get("scale"))
            .and_then(Value::as_f64)
            .ok_or_else(|| "hurtbox requires radius or scale".to_string())?;
        Ok(Self {
            id: optional_u8(value, "id"),
            bone: optional_u8(value, "bone"),
            height: optional_u8(value, "height"),
            is_grabbable: value
                .get("is_grabbable")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            state: value
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            capsule: RuntimeCapsule {
                a: source_a,
                b: source_b,
                radius,
            },
        })
    }

    fn rust_literal(&self) -> String {
        let capsule = indent_after_first_line(&self.capsule.rust_literal(), 4);
        format!(
            "SourceHurtCapsule {{\n    id: {},\n    bone: {},\n    height: {},\n    is_grabbable: {},\n    state: {},\n    capsule: {},\n}}",
            self.id,
            self.bone,
            self.height,
            self.is_grabbable,
            rust_string_literal(&self.state),
            capsule,
        )
    }
}

impl RuntimeCapsule {
    fn rust_literal(&self) -> String {
        let a = indent_after_first_line(&self.a.rust_literal(), 4);
        let b = indent_after_first_line(&self.b.rust_literal(), 4);
        format!(
            "SourceCapsule {{\n    a: {},\n    b: {},\n    radius: {},\n}}",
            a,
            b,
            rust_float_literal(self.radius),
        )
    }
}

impl RuntimeSourcePoint {
    fn rust_literal(&self) -> String {
        format!(
            "SourcePoint {{\n    x: {},\n    y: {},\n    z: {},\n}}",
            rust_float_literal(self.x),
            rust_float_literal(self.y),
            rust_float_literal(self.z),
        )
    }
}

fn source_point(value: &Value, fields: &[&str]) -> Option<RuntimeSourcePoint> {
    fields
        .iter()
        .find_map(|field| value.get(*field))
        .and_then(|point| {
            Some(RuntimeSourcePoint {
                x: point.get("x")?.as_f64()?,
                y: point.get("y")?.as_f64()?,
                z: point.get("z").and_then(Value::as_f64).unwrap_or(0.0),
            })
        })
}

fn required_f64(value: &Value, field: &str) -> Result<f64, String> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("frame-data entry requires {field}"))
}

fn optional_u8(value: &Value, field: &str) -> u8 {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0)
}

fn optional_u16(value: &Value, field: &str) -> u16 {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(0)
}

fn optional_i16(value: &Value, field: &str) -> i16 {
    value
        .get(field)
        .and_then(Value::as_i64)
        .and_then(|value| i16::try_from(value).ok())
        .unwrap_or(0)
}

fn optional_bool(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn validate_rust_motion_state_variant(state: &str) -> Result<&str, String> {
    let mut chars = state.chars();
    if !chars.next().is_some_and(|ch| ch.is_ascii_uppercase()) {
        return Err(format!(
            "--state must be a Rust MotionState variant, got {state}"
        ));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(format!(
            "--state must be a Rust MotionState variant, got {state}"
        ));
    }
    Ok(state)
}

fn validate_current_rust_motion_state_variant(state: &str) -> Result<&str, String> {
    validate_rust_motion_state_variant(state)?;
    if RUST_MOTION_STATE_VARIANTS.contains(&state) {
        Ok(state)
    } else {
        Err(format!(
            "Rust MotionState does not contain {state}; import can keep the source artifact, but runtime export needs a state parity update first"
        ))
    }
}

fn screaming_snake_identifier(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_lower_or_digit = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && previous_was_lower_or_digit && !output.ends_with('_') {
                output.push('_');
            }
            output.push(ch.to_ascii_uppercase());
            previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            if !output.ends_with('_') && !output.is_empty() {
                output.push('_');
            }
            previous_was_lower_or_digit = false;
        }
    }
    output.trim_matches('_').to_string()
}

fn rust_float_literal(value: f64) -> String {
    if value == 0.0 {
        "0.0".to_string()
    } else {
        format!("{value:?}")
    }
}

fn rust_string_literal(value: &str) -> String {
    format!("{value:?}")
}

fn indent_multiline(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn indent_after_first_line(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut output = first.to_string();
    for line in lines {
        output.push('\n');
        output.push_str(&prefix);
        output.push_str(line);
    }
    output
}

fn source_action_table_frame_data_artifact(
    root: &Path,
    options: &FrameDataOptions,
) -> Result<Value, String> {
    let source_character = options
        .source_character
        .as_deref()
        .ok_or_else(|| "creating frame data requires --source-character <id>".to_string())?;
    let action_table_path = action_animation_table_path(root, source_character);
    let action_table = read_json_object(&action_table_path)?;
    let probe = json!({});
    let source_state = options.source_state.as_deref().unwrap_or(&options.state);
    let action = find_action_record(&action_table, &probe, source_state).ok_or_else(|| {
        format!(
            "could not find source action state {} in {}",
            source_state,
            action_table_path.display()
        )
    })?;
    source_action_frame_data_artifact(
        root,
        &options.character,
        source_character,
        &options.state,
        &action_table_path,
        action,
    )
}

fn source_action_frame_data_artifact(
    root: &Path,
    target_character: &str,
    source_character: &str,
    state: &str,
    action_table_path: &Path,
    action: &Value,
) -> Result<Value, String> {
    let total_frames = action_total_frames(action);
    let source_action_key = source_action_key(action).unwrap_or_else(|| state.to_string());
    let runtime_motion_state = runtime_motion_state_for_source_key(&source_action_key);

    Ok(json!({
        "schema_version": 1,
        "target_character": target_character,
        "target_character_label": character_label(target_character),
        "source_character": source_character,
        "source_character_label": character_label(source_character),
        "state": state,
        "label": state,
        "source_state": source_action_key,
        "source_action_key": source_action_key,
        "runtime_motion_state": runtime_motion_state,
        "projection": {
            "source_space": "melee_xyz",
            "default_view": "xy",
            "z_policy": "preserve_and_project",
        },
        "sources": [source_action_table_source_json(root, action_table_path, action)],
        "summary": {
            "total_frames": total_frames,
            "iasa_frame": "unknown",
            "active_hitbox_windows": [],
        },
        "keyframes": [],
        "gaps": [{"field": "iasa_frame", "reason": "not yet proven from source"}],
        "overrides": [],
    }))
}

fn source_action_table_source_json(root: &Path, action_table_path: &Path, action: &Value) -> Value {
    json!({
        "kind": "source_action_table",
        "path": path_for_artifact(root, action_table_path),
        "action_state_id": action_state_id(action),
        "subaction_script_offset": action_subaction_script_offset(action),
        "source_action_name": source_action_name(action),
        "source_action_key": source_action_key(action),
        "figatree_root": action_figatree_root(action),
        "purpose": "source action record used to initialize frame-data import artifact",
    })
}

fn action_state_id(action: &Value) -> u64 {
    action
        .get("action_state_id")
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn action_subaction_script_offset(action: &Value) -> u64 {
    action
        .get("subaction_script_offset")
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn action_total_frames(action: &Value) -> u64 {
    action
        .get("figatree")
        .and_then(|figatree| figatree.get("frames_ticks"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn source_action_name(action: &Value) -> String {
    action
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .or_else(|| action_figatree_root(action).filter(|root| !root.is_empty()))
        .unwrap_or("")
        .to_string()
}

fn action_figatree_root(action: &Value) -> Option<&str> {
    action
        .get("figatree_root")
        .and_then(Value::as_str)
        .or_else(|| {
            action
                .get("figatree")
                .and_then(|figatree| figatree.get("root"))
                .and_then(Value::as_str)
        })
}

fn character_label(character: &str) -> String {
    match character {
        "captain" | "captain_falcon" => "Captain Falcon".to_string(),
        "marth" | "mars" => "Marth".to_string(),
        "dolphin_mole" => "Dolphin Mole".to_string(),
        other => other
            .split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn apply_melee_render_projection_metadata(artifact: &mut Value) {
    if !artifact.get("projection").is_some_and(Value::is_object) {
        artifact["projection"] = json!({});
    }
    artifact["projection"]["render_transform"] = json!(MELEE_RIGHT_FACING_RENDER_TRANSFORM);
    artifact["projection"]["flatten_after_render"] = json!(MELEE_RIGHT_FACING_FLATTEN_POLICY);
}

#[derive(Debug, Clone)]
struct DecodedActionScript {
    source_character: String,
    action_state_id: u64,
    subaction_script_offset: u64,
    raw_path: PathBuf,
    procedures: Vec<DecodedProcedure>,
}

#[derive(Debug, Clone)]
enum DecodedProcedure {
    SpawnHitbox(DecodedHitbox),
    SetHurtState(DecodedHurtState),
    SetCmdVar(DecodedCmdVar),
    SetThrowFlag(DecodedThrowFlag),
    SetThrowHitbox(DecodedThrowHitbox),
    SetJabCombo(DecodedJabCombo),
    SetJabRapid(DecodedJabRapid),
    ClearAllHitboxes {
        frame: u64,
        word_offset: usize,
        raw_words: Vec<u32>,
    },
}

#[derive(Debug, Clone)]
struct DecodedHitbox {
    frame: u64,
    word_offset: usize,
    raw_words: [u32; 5],
    id: u64,
    hit_group: u64,
    bone: u64,
    use_common_bone_ids: bool,
    damage: u64,
    radius: f64,
    center_x: f64,
    center_y: f64,
    center_z: f64,
    angle: u64,
    kbg: u64,
    weight_set_kb: u64,
    bkb: u64,
    element: u64,
    shield_damage: i64,
    hit_grounded: bool,
    hit_aerial: bool,
    item_hit_interaction: bool,
    ignore_thrown_fighters: bool,
    ignore_fighter_scale: bool,
    clank: bool,
    rebound: bool,
    skip_if_thrown_hitbox_owner_absent: bool,
    hit_grabbed_victim_only: bool,
}

#[derive(Debug, Clone)]
struct DecodedHurtState {
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    bone_idx: u64,
    state: u64,
}

#[derive(Debug, Clone)]
struct DecodedCmdVar {
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    cmd_var: u64,
    value: u64,
}

#[derive(Debug, Clone)]
struct DecodedThrowFlag {
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    hit_idx: u64,
    flag_bit: Option<u8>,
}

#[derive(Debug, Clone)]
struct DecodedThrowHitbox {
    frame: u64,
    word_offset: usize,
    raw_words: [u32; 3],
    hitbox_idx: u64,
    damage: u64,
    angle: u64,
    hit_x24: u64,
    hit_x28: u64,
    hit_x2c: u64,
    element: u64,
    sfx_severity: u64,
    sfx_kind: u64,
}

#[derive(Debug, Clone)]
struct DecodedJabCombo {
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    disabled: bool,
}

#[derive(Debug, Clone)]
struct DecodedJabRapid {
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    state: bool,
}

const FIGHTER_CMD_LENGTHS: [usize; 49] = [
    5, 5, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 7, 4, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 3, 2, 1, 4,
];
const COMMON_CMD_LENGTHS: [usize; 10] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
const MELEE_RIGHT_FACING_RENDER_TRANSFORM: &str = "ftPartSetRotY(TopN, M_PI_2 * fp->facing_dir)";
const MELEE_RIGHT_FACING_FLATTEN_POLICY: &str = "right_facing_melee_xy";
const MELEE_HURTBOX_INIT_HANDLER: &str = "ftColl_8007B3A0/ftColl_8007B4E0 ftData.x30";
const MELEE_HURTBOX_UPDATE_HANDLER: &str = "lbColl_800083C4/lbColl_8000A244/lbColl_8000A584";
const MELEE_HURTBOX_DRAW_HANDLER: &str =
    "ftDrawCommon_800805C8 -> lbColl_8000A244/lbColl_8000A584 -> lbColl_DrawHitResult";
const MELEE_HURTBOX_RENDER_ENDPOINTS: &str = "HurtCapsule.a_pos -> HurtCapsule.b_pos";
const MELEE_HURTBOX_RENDER_RADIUS: &str = "HurtCapsule.scale";
const MELEE_HURTBOX_COLOR_TABLE: &str = "lbColl_803B9928[hurt->state]";
const MELEE_HURTBOX_Z_POLICY: &str =
    "preserve JObj-transformed z; debug render may force fighter->cur_pos.z when ftCommon_8007F804 returns non-null";
const MELEE_HURTBOX_SOURCE: &str = "ftData.x30 + PlCaAJ FigaTree + PlCaNr JObj skeleton";
fn source_binding_for_runtime_motion_state_variant(
    state: &str,
) -> Result<Option<MotionStateSourceBinding>, String> {
    validate_current_rust_motion_state_variant(state)?;
    let motion_state = motion_state_for_runtime_variant(state).ok_or_else(|| {
        format!(
            "Rust MotionState does not contain {state}; import can keep the source artifact, but runtime export needs a state parity update first"
        )
    })?;
    let expected_action_state_id = melee_action_state_id_for_motion_state(motion_state);
    let source_binding = source_binding_for_motion_state(motion_state);
    debug_assert_eq!(
        source_binding.map(|binding| binding.action_state_id),
        source_binding.map(|_| expected_action_state_id)
    );
    Ok(source_binding)
}

fn source_special_action_bindings_for_character(
    source_character: Option<&str>,
) -> &'static [SourceSpecialActionBinding] {
    match source_character {
        Some("captain" | "captain_falcon" | "falcon") => FALCON_SOURCE_SPECIAL_ACTION_BINDINGS,
        _ => &[],
    }
}

fn decoded_action_script_artifact(
    root: &Path,
    options: &FrameDataOptions,
    artifact: &Value,
) -> Option<DecodedActionScript> {
    let source_character = options
        .source_character
        .as_deref()
        .or_else(|| artifact.get("source_character").and_then(Value::as_str))?;
    let state = artifact
        .get("source_action_key")
        .and_then(Value::as_str)
        .or_else(|| artifact.get("source_state").and_then(Value::as_str))
        .or_else(|| artifact.get("state").and_then(Value::as_str))
        .unwrap_or(&options.state);
    let action_table_path = action_animation_table_path(root, source_character);
    let action_table = read_json_object(&action_table_path).ok()?;
    let action = find_action_record(&action_table, artifact, state)?;
    let action_state_id = action.get("action_state_id").and_then(Value::as_u64)?;
    let subaction_script_offset = action
        .get("subaction_script_offset")
        .and_then(Value::as_u64)?;
    let raw_path = raw_fighter_data_path(root, source_character);
    let raw = fs::read(&raw_path).ok()?;
    let procedures = decode_action_script(&raw, 0x20 + subaction_script_offset as usize)?;
    Some(DecodedActionScript {
        source_character: source_character.to_string(),
        action_state_id,
        subaction_script_offset,
        raw_path,
        procedures,
    })
}

fn decoded_action_script_from_action(
    root: &Path,
    source_character: &str,
    action: &Value,
    source_raw: Option<&[u8]>,
) -> Option<DecodedActionScript> {
    let action_state_id = action.get("action_state_id").and_then(Value::as_u64)?;
    let subaction_script_offset = action
        .get("subaction_script_offset")
        .and_then(Value::as_u64)?;
    let raw_path = raw_fighter_data_path(root, source_character);
    let raw_owned;
    let raw = if let Some(source_raw) = source_raw {
        source_raw
    } else {
        raw_owned = fs::read(&raw_path).ok()?;
        &raw_owned
    };
    let procedures = decode_action_script(raw, 0x20 + subaction_script_offset as usize)?;
    Some(DecodedActionScript {
        source_character: source_character.to_string(),
        action_state_id,
        subaction_script_offset,
        raw_path,
        procedures,
    })
}

fn read_json_object(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let value = serde_json::from_str::<Value>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(format!("{} is not a JSON object", path.display()))
    }
}

fn action_animation_table_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_action_animation_table.json".to_string(),
        "mars" => "marth_action_animation_table.json".to_string(),
        other => format!("{other}_action_animation_table.json"),
    };
    root.join("resources")
        .join("melee")
        .join("extracted")
        .join(filename)
}

fn raw_fighter_data_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "PlCa.dat".to_string(),
        "marth" | "mars" => "PlMs.dat".to_string(),
        other => format!("{other}.dat"),
    };
    root.join("resources")
        .join("melee")
        .join("raw")
        .join(filename)
}

fn action_hurtbox_samples_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_action_hurtbox_samples.json".to_string(),
        "mars" => "marth_action_hurtbox_samples.json".to_string(),
        other => format!("{other}_action_hurtbox_samples.json"),
    };
    root.join("resources")
        .join("melee")
        .join("extracted")
        .join(filename)
}

fn action_ecb_samples_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_action_ecb_samples.json".to_string(),
        "mars" => "marth_action_ecb_samples.json".to_string(),
        other => format!("{other}_action_ecb_samples.json"),
    };
    root.join("resources")
        .join("melee")
        .join("extracted")
        .join(filename)
}

fn character_profile_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_profile.json".to_string(),
        "mars" => "marth_profile.json".to_string(),
        other => format!("{other}_profile.json"),
    };
    root.join("resources")
        .join("melee")
        .join("extracted")
        .join(filename)
}

fn hurtbox_inits_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_hurtbox_inits.json".to_string(),
        "mars" => "marth_hurtbox_inits.json".to_string(),
        other => format!("{other}_hurtbox_inits.json"),
    };
    root.join("resources")
        .join("melee")
        .join("extracted")
        .join(filename)
}

fn common_parts_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_common_parts.json".to_string(),
        "mars" => "marth_common_parts.json".to_string(),
        other => format!("{other}_common_parts.json"),
    };
    root.join("resources")
        .join("melee")
        .join("extracted")
        .join(filename)
}

fn costume_skeleton_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_costume_skeleton.json".to_string(),
        "mars" => "marth_costume_skeleton.json".to_string(),
        other => format!("{other}_costume_skeleton.json"),
    };
    root.join("resources")
        .join("melee")
        .join("extracted")
        .join(filename)
}

fn source_hurtboxes_for_action(
    root: &Path,
    source_character: &str,
    action_state_id: u64,
) -> Option<Vec<Value>> {
    let samples = read_json_object(&action_hurtbox_samples_path(root, source_character)).ok()?;
    let metadata = samples
        .get("sample_metadata")
        .cloned()
        .unwrap_or_else(default_source_hurtbox_sample_metadata);
    let actions = samples.get("actions")?.as_array()?;
    let action = actions.iter().find(|action| {
        action.get("action_state_id").and_then(Value::as_u64) == Some(action_state_id)
    })?;
    let mut frames = action.get("frames")?.as_array()?.clone();
    for frame in &mut frames {
        let Some(hurtboxes) = frame.get_mut("hurtboxes").and_then(Value::as_array_mut) else {
            continue;
        };
        for hurtbox in hurtboxes {
            enrich_source_hurtbox(hurtbox, &metadata);
        }
    }
    Some(frames)
}

fn default_source_hurtbox_sample_metadata() -> Value {
    json!({
        "source_init_handler": MELEE_HURTBOX_INIT_HANDLER,
        "source_update_handler": MELEE_HURTBOX_UPDATE_HANDLER,
        "source_draw_handler": MELEE_HURTBOX_DRAW_HANDLER,
        "source_render_endpoints": MELEE_HURTBOX_RENDER_ENDPOINTS,
        "source_render_radius": MELEE_HURTBOX_RENDER_RADIUS,
        "source_render_transform": MELEE_RIGHT_FACING_RENDER_TRANSFORM,
        "flatten_after_render": MELEE_RIGHT_FACING_FLATTEN_POLICY,
        "source_color_table": MELEE_HURTBOX_COLOR_TABLE,
        "source_skip_update_pos_after_transform": true,
        "source_z_policy": MELEE_HURTBOX_Z_POLICY,
        "source": MELEE_HURTBOX_SOURCE,
        "confidence": "source_extracted",
    })
}

fn enrich_source_hurtbox(hurtbox: &mut Value, metadata: &Value) {
    let Some(object) = hurtbox.as_object_mut() else {
        return;
    };
    for (alias, source) in [
        ("a_pos", "a"),
        ("b_pos", "b"),
        ("source_a_pos", "source_a"),
        ("source_b_pos", "source_b"),
    ] {
        if !object.contains_key(alias) {
            if let Some(value) = object.get(source).cloned() {
                object.insert(alias.to_string(), value);
            }
        }
    }
    let Some(metadata_object) = metadata.as_object() else {
        return;
    };
    for (key, value) in metadata_object {
        object.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

fn source_ecb_frames_for_action(
    root: &Path,
    source_character: &str,
    action_state_id: u64,
) -> Option<Vec<Value>> {
    let samples = read_json_object(&action_ecb_samples_path(root, source_character)).ok()?;
    let actions = samples.get("actions")?.as_array()?;
    let action = actions.iter().find(|action| {
        action.get("action_state_id").and_then(Value::as_u64) == Some(action_state_id)
    })?;
    Some(action.get("frames")?.as_array()?.clone())
}

fn find_action_record<'a>(
    action_table: &'a Value,
    artifact: &Value,
    state: &str,
) -> Option<&'a Value> {
    let actions = action_table.get("actions")?.as_array()?;
    if let Some(action_state_id) =
        artifact
            .get("sources")
            .and_then(Value::as_array)
            .and_then(|sources| {
                sources
                    .iter()
                    .filter_map(|source| source.get("action_state_id").and_then(Value::as_u64))
                    .next()
            })
    {
        if let Some(action) = actions.iter().find(|action| {
            action.get("action_state_id").and_then(Value::as_u64) == Some(action_state_id)
        }) {
            return Some(action);
        }
    }
    if let Ok(action_state_id) = state.parse::<u64>() {
        if let Some(action) = actions.iter().find(|action| {
            action.get("action_state_id").and_then(Value::as_u64) == Some(action_state_id)
        }) {
            return Some(action);
        }
    }
    let wanted_key = sanitize_state_key(state);
    actions.iter().find(|action| {
        action
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name == state)
            || action_figatree_root(action).is_some_and(|root| root == state)
            || source_action_key(action).is_some_and(|key| key == wanted_key)
    })
}

fn decode_action_script(raw: &[u8], script_offset: usize) -> Option<Vec<DecodedProcedure>> {
    let mut procedures = Vec::new();
    let mut word_offset = 0usize;
    let mut current_frame = 0u64;

    while script_offset + word_offset * 4 + 4 <= raw.len() {
        let word = read_u32(raw, script_offset + word_offset * 4)?;
        let opcode = word >> 26;
        let value = word & 0x03ff_ffff;

        match opcode {
            0 => break,
            1 => {
                current_frame += command_frame_value(value);
                word_offset += 1;
            }
            2 => {
                current_frame = command_frame_value(value);
                word_offset += 1;
            }
            8 => {
                word_offset += 1;
            }
            3..=7 | 9 => {
                let length = *COMMON_CMD_LENGTHS.get(opcode as usize)?;
                if script_offset + (word_offset + length) * 4 > raw.len() {
                    return None;
                }
                word_offset += length;
            }
            19 => {
                procedures.push(DecodedProcedure::SetCmdVar(decode_set_cmd_var(
                    current_frame,
                    word_offset,
                    word,
                )));
                word_offset += 1;
            }
            10.. => {
                let fighter_index = (opcode - 10) as usize;
                let length = *FIGHTER_CMD_LENGTHS.get(fighter_index)?;
                if script_offset + (word_offset + length) * 4 > raw.len() {
                    return None;
                }
                match fighter_index {
                    1 => {
                        let raw_words = read_words(raw, script_offset, word_offset, 5)?;
                        procedures.push(DecodedProcedure::SpawnHitbox(decode_spawn_hitbox(
                            current_frame,
                            word_offset,
                            raw_words.try_into().ok()?,
                        )));
                    }
                    6 => {
                        procedures.push(DecodedProcedure::ClearAllHitboxes {
                            frame: current_frame,
                            word_offset,
                            raw_words: read_words(raw, script_offset, word_offset, length)?,
                        });
                    }
                    18 => {
                        procedures.push(DecodedProcedure::SetHurtState(decode_set_hurt_state(
                            current_frame,
                            word_offset,
                            word,
                        )));
                    }
                    10 => {
                        procedures.push(DecodedProcedure::SetThrowFlag(decode_set_throw_flag(
                            current_frame,
                            word_offset,
                            word,
                        )));
                    }
                    24 => {
                        let raw_words = read_words(raw, script_offset, word_offset, 3)?;
                        procedures.push(DecodedProcedure::SetThrowHitbox(decode_set_throw_hitbox(
                            current_frame,
                            word_offset,
                            raw_words.try_into().ok()?,
                        )));
                    }
                    19 => {
                        procedures.push(DecodedProcedure::SetJabCombo(decode_set_jab_combo(
                            current_frame,
                            word_offset,
                            word,
                        )));
                    }
                    20 => {
                        procedures.push(DecodedProcedure::SetJabRapid(decode_set_jab_rapid(
                            current_frame,
                            word_offset,
                            word,
                        )));
                    }
                    _ => {}
                }
                word_offset += length;
            }
        }
    }

    Some(procedures)
}

fn read_u32(raw: &[u8], offset: usize) -> Option<u32> {
    let bytes = raw.get(offset..offset + 4)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

fn read_words(
    raw: &[u8],
    script_offset: usize,
    word_offset: usize,
    length: usize,
) -> Option<Vec<u32>> {
    (0..length)
        .map(|index| read_u32(raw, script_offset + (word_offset + index) * 4))
        .collect()
}

fn command_frame_value(value: u32) -> u64 {
    if value >= 0x1_0000 && value & 0xffff == 0 {
        (value >> 16) as u64
    } else {
        value as u64
    }
}

fn decode_spawn_hitbox(frame: u64, word_offset: usize, raw_words: [u32; 5]) -> DecodedHitbox {
    let word0 = raw_words[0];
    let word1 = raw_words[1];
    let word2 = raw_words[2];
    let word3 = raw_words[3];
    let word4 = raw_words[4];
    let item_hit_interaction = bitfield(word3, 27, 1) != 0;
    let spawn_hitbox_skip_xf_b4 = bitfield(word3, 28, 1) != 0;
    let rebound = bitfield(word3, 31, 1) != 0;
    let hit_grabbed_victim_only = bitfield(word4, 12, 1) != 0;
    DecodedHitbox {
        frame,
        word_offset,
        raw_words,
        id: bitfield(word0, 6, 3) as u64,
        hit_group: bitfield(word0, 9, 3) as u64,
        bone: bitfield(word0, 13, 8) as u64,
        use_common_bone_ids: bitfield(word0, 21, 1) != 0,
        damage: bitfield(word0, 22, 10) as u64,
        radius: bitfield(word1, 0, 16) as f64 / 256.0,
        center_x: sign_extend(bitfield(word1, 16, 16), 16) as f64 / 256.0,
        center_y: sign_extend(bitfield(word2, 0, 16), 16) as f64 / 256.0,
        center_z: sign_extend(bitfield(word2, 16, 16), 16) as f64 / 256.0,
        angle: bitfield(word3, 0, 9) as u64,
        kbg: bitfield(word3, 9, 9) as u64,
        weight_set_kb: bitfield(word3, 18, 9) as u64,
        bkb: bitfield(word4, 0, 9) as u64,
        element: bitfield(word4, 9, 5) as u64,
        shield_damage: sign_extend(bitfield(word4, 14, 8), 8) as i64,
        hit_grounded: bitfield(word4, 30, 1) != 0,
        hit_aerial: bitfield(word4, 31, 1) != 0,
        item_hit_interaction,
        ignore_thrown_fighters: spawn_hitbox_skip_xf_b4,
        ignore_fighter_scale: bitfield(word3, 29, 1) != 0,
        clank: bitfield(word3, 30, 1) != 0,
        rebound,
        skip_if_thrown_hitbox_owner_absent: spawn_hitbox_skip_xf_b4,
        hit_grabbed_victim_only,
    }
}

fn decode_set_hurt_state(frame: u64, word_offset: usize, raw_word: u32) -> DecodedHurtState {
    DecodedHurtState {
        frame,
        word_offset,
        raw_word,
        bone_idx: bitfield(raw_word, 6, 8) as u64,
        state: bitfield(raw_word, 14, 18) as u64,
    }
}

fn decode_set_cmd_var(frame: u64, word_offset: usize, raw_word: u32) -> DecodedCmdVar {
    DecodedCmdVar {
        frame,
        word_offset,
        raw_word,
        cmd_var: ((raw_word >> 24) & 0x03) as u64,
        value: (raw_word & 0x00ff_ffff) as u64,
    }
}

fn decode_set_throw_flag(frame: u64, word_offset: usize, raw_word: u32) -> DecodedThrowFlag {
    let hit_idx = (raw_word & 0x03ff_ffff) as u64;
    DecodedThrowFlag {
        frame,
        word_offset,
        raw_word,
        hit_idx,
        flag_bit: match hit_idx {
            0 => Some(3),
            1 => Some(4),
            _ => None,
        },
    }
}

fn decode_set_throw_hitbox(
    frame: u64,
    word_offset: usize,
    raw_words: [u32; 3],
) -> DecodedThrowHitbox {
    let word0 = raw_words[0];
    let word1 = raw_words[1];
    let word2 = raw_words[2];
    DecodedThrowHitbox {
        frame,
        word_offset,
        raw_words,
        hitbox_idx: bitfield(word0, 6, 3) as u64,
        damage: bitfield(word0, 9, 23) as u64,
        angle: bitfield(word1, 0, 9) as u64,
        hit_x24: bitfield(word1, 9, 9) as u64,
        hit_x28: bitfield(word1, 18, 9) as u64,
        hit_x2c: bitfield(word2, 0, 9) as u64,
        element: bitfield(word2, 9, 4) as u64,
        sfx_severity: bitfield(word2, 13, 3) as u64,
        sfx_kind: bitfield(word2, 16, 4) as u64,
    }
}

fn decode_set_jab_combo(frame: u64, word_offset: usize, raw_word: u32) -> DecodedJabCombo {
    DecodedJabCombo {
        frame,
        word_offset,
        raw_word,
        disabled: (raw_word & 0x03ff_ffff) != 0,
    }
}

fn decode_set_jab_rapid(frame: u64, word_offset: usize, raw_word: u32) -> DecodedJabRapid {
    DecodedJabRapid {
        frame,
        word_offset,
        raw_word,
        state: (raw_word & 0x03ff_ffff) != 0,
    }
}

fn hurt_capsule_state_name(state: u64) -> &'static str {
    match state {
        0 => "HurtCapsule_Enabled",
        1 => "HurtCapsule_Disabled",
        2 => "Intangible",
        _ => "unknown",
    }
}

fn bitfield(word: u32, offset_from_msb: u32, width: u32) -> u32 {
    let shift = 32 - offset_from_msb - width;
    (word >> shift) & ((1u32 << width) - 1)
}

fn sign_extend(value: u32, width: u32) -> i32 {
    let sign_bit = 1u32 << (width - 1);
    if value & sign_bit != 0 {
        (value as i32) - (1i32 << width)
    } else {
        value as i32
    }
}

fn apply_decoded_action_script(artifact: &mut Value, decoded: DecodedActionScript, root: &Path) {
    artifact["decoded_action_script"] = decoded_action_script_json(&decoded, root);
    let hurtbox_frames =
        source_hurtboxes_for_action(root, &decoded.source_character, decoded.action_state_id);
    apply_decoded_hitboxes_to_keyframes(artifact, &decoded, hurtbox_frames.as_deref());
    apply_decoded_active_hitbox_windows(artifact, &decoded);
    let mut applied_hurtbox_samples = false;
    let mut applied_body_volume_samples = false;
    if let Some(hurtbox_frames) = &hurtbox_frames {
        apply_source_hurtboxes_to_keyframes(artifact, hurtbox_frames);
        apply_decoded_hurt_state_overlays(artifact, &decoded);
        apply_source_active_hurtbox_windows(artifact, hurtbox_frames);
        remove_gap(artifact, "hurtboxes");
        applied_hurtbox_samples = true;
    }
    let ecb_frames =
        source_ecb_frames_for_action(root, &decoded.source_character, decoded.action_state_id);
    if let Some(ecb_frames) = ecb_frames {
        apply_source_body_volumes_to_keyframes(artifact, &ecb_frames);
        apply_source_active_body_volume_windows(artifact, &ecb_frames);
        applied_body_volume_samples = true;
    }
    apply_extracted_source_citations(
        artifact,
        root,
        &decoded.source_character,
        applied_hurtbox_samples,
        applied_body_volume_samples,
    );
    remove_gap(artifact, "hitboxes.damage_angle_knockback");
}

fn decoded_action_script_json(decoded: &DecodedActionScript, root: &Path) -> Value {
    let procedures = decoded
        .procedures
        .iter()
        .map(|procedure| match procedure {
            DecodedProcedure::SpawnHitbox(hitbox) => json!({
                "procedure": "fighter.spawn_hitbox",
                "handler": "ftAction_8007121C",
                "frame": hitbox.frame,
                "word_offset": hitbox.word_offset,
                "raw_words": raw_words_json(&hitbox.raw_words),
                "hitbox_id": hitbox.id,
                "item_hit_interaction": hitbox.item_hit_interaction,
                "ignore_thrown_fighters": hitbox.ignore_thrown_fighters,
                "ignore_fighter_scale": hitbox.ignore_fighter_scale,
                "clank": hitbox.clank,
                "rebound": hitbox.rebound,
                "skip_if_thrown_hitbox_owner_absent": hitbox.skip_if_thrown_hitbox_owner_absent,
                "hit_grabbed_victim_only": hitbox.hit_grabbed_victim_only,
            }),
            DecodedProcedure::SetHurtState(hurt_state) => json!({
                "procedure": "fighter.set_hurt_state",
                "handler": "ftAction_80071A9C",
                "frame": hurt_state.frame,
                "word_offset": hurt_state.word_offset,
                "raw_words": raw_words_json(&[hurt_state.raw_word]),
                "bone_idx": hurt_state.bone_idx,
                "state": hurt_capsule_state_name(hurt_state.state),
                "state_raw": hurt_state.state,
            }),
            DecodedProcedure::SetCmdVar(cmd_var) => json!({
                "procedure": "fighter.set_cmd_var",
                "handler": "ftAction_80071820",
                "frame": cmd_var.frame,
                "word_offset": cmd_var.word_offset,
                "raw_words": raw_words_json(&[cmd_var.raw_word]),
                "cmd_var": cmd_var.cmd_var,
                "value": cmd_var.value,
            }),
            DecodedProcedure::SetThrowFlag(throw_flag) => json!({
                "procedure": "fighter.set_throw_flag",
                "handler": "ftAction_800718A4",
                "frame": throw_flag.frame,
                "word_offset": throw_flag.word_offset,
                "raw_words": raw_words_json(&[throw_flag.raw_word]),
                "hit_idx": throw_flag.hit_idx,
                "flag_bit": throw_flag.flag_bit,
            }),
            DecodedProcedure::SetThrowHitbox(throw_hitbox) => json!({
                "procedure": "fighter.set_throw_hitbox",
                "handler": "ftAction_80071E04",
                "frame": throw_hitbox.frame,
                "word_offset": throw_hitbox.word_offset,
                "raw_words": raw_words_json(&throw_hitbox.raw_words),
                "hitbox_idx": throw_hitbox.hitbox_idx,
                "damage": throw_hitbox.damage,
                "angle": throw_hitbox.angle,
                "hit_x24": throw_hitbox.hit_x24,
                "hit_x28": throw_hitbox.hit_x28,
                "hit_x2c": throw_hitbox.hit_x2c,
                "element": throw_hitbox.element,
                "sfx_severity": throw_hitbox.sfx_severity,
                "sfx_kind": throw_hitbox.sfx_kind,
            }),
            DecodedProcedure::SetJabCombo(jab_combo) => json!({
                "procedure": "fighter.set_jab_combo",
                "handler": "ftAction_80071AE8",
                "frame": jab_combo.frame,
                "word_offset": jab_combo.word_offset,
                "raw_words": raw_words_json(&[jab_combo.raw_word]),
                "disabled": jab_combo.disabled,
            }),
            DecodedProcedure::SetJabRapid(jab_rapid) => json!({
                "procedure": "fighter.set_jab_rapid",
                "handler": "ftAction_80071B28",
                "frame": jab_rapid.frame,
                "word_offset": jab_rapid.word_offset,
                "raw_words": raw_words_json(&[jab_rapid.raw_word]),
                "state": jab_rapid.state,
            }),
            DecodedProcedure::ClearAllHitboxes {
                frame,
                word_offset,
                raw_words,
            } => json!({
                "procedure": "fighter.clear_all_hitboxes",
                "handler": "ftAction_800717D8",
                "frame": frame,
                "word_offset": word_offset,
                "raw_words": raw_words_json(raw_words),
            }),
        })
        .collect::<Vec<_>>();
    json!({
        "source": {
            "kind": "fighter_action_script",
            "raw_path": path_for_artifact(root, &decoded.raw_path),
            "action_state_id": decoded.action_state_id,
            "subaction_script_offset": decoded.subaction_script_offset,
        },
        "procedures": procedures,
    })
}

fn apply_extracted_source_citations(
    artifact: &mut Value,
    root: &Path,
    source_character: &str,
    applied_hurtbox_samples: bool,
    applied_body_volume_samples: bool,
) {
    if !artifact.get("sources").is_some_and(Value::is_array) {
        artifact["sources"] = json!([]);
    }
    if applied_body_volume_samples {
        remove_source_kind(artifact, "generated_ecb");
    }
    if applied_hurtbox_samples {
        append_source_if_missing(
            artifact,
            "extracted_hurtbox_samples",
            json!({
                "kind": "extracted_hurtbox_samples",
                "path": path_for_artifact(root, &action_hurtbox_samples_path(root, source_character)),
                "purpose": "source-extracted ftData.x30 hurt capsule samples transformed by action FigaTree/JObj skeleton",
            }),
        );
    }
    if applied_body_volume_samples {
        append_source_if_missing(
            artifact,
            "extracted_body_volume_samples",
            json!({
                "kind": "extracted_body_volume_samples",
                "path": path_for_artifact(root, &action_ecb_samples_path(root, source_character)),
                "purpose": "source-extracted ftData.x44 ECB/body-volume samples transformed by action FigaTree/JObj skeleton",
            }),
        );
    }
}

fn remove_source_kind(artifact: &mut Value, kind: &str) {
    let Some(sources) = artifact.get_mut("sources").and_then(Value::as_array_mut) else {
        return;
    };
    sources.retain(|source| source.get("kind").and_then(Value::as_str) != Some(kind));
}

fn append_source_if_missing(artifact: &mut Value, kind: &str, source: Value) {
    let Some(sources) = artifact.get_mut("sources").and_then(Value::as_array_mut) else {
        return;
    };
    if sources
        .iter()
        .any(|source| source.get("kind").and_then(Value::as_str) == Some(kind))
    {
        return;
    }
    sources.push(source);
}

fn path_for_artifact(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn raw_words_json(words: &[u32]) -> Vec<String> {
    words.iter().map(|word| format!("0x{word:08x}")).collect()
}

fn apply_decoded_hitboxes_to_keyframes(
    artifact: &mut Value,
    decoded: &DecodedActionScript,
    source_frames: Option<&[Value]>,
) {
    let mut keyframes = artifact
        .get("keyframes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for keyframe in &mut keyframes {
        if let Some(hitboxes) = keyframe.get_mut("hitboxes").and_then(Value::as_array_mut) {
            hitboxes.retain(|hitbox| {
                !matches!(
                    hitbox.get("source").and_then(Value::as_str),
                    Some(
                        "provisional_viewer_scaffold_pending_hitbox_command_extraction"
                            | "decoded_action_script"
                    )
                )
            });
        }
    }
    let mut materialization_frames = BTreeSet::new();
    if let Some(source_frames) = source_frames {
        for source_frame in source_frames {
            if let Some(frame) = source_frame.get("frame").and_then(Value::as_u64) {
                materialization_frames.insert(frame);
            }
        }
    } else {
        materialization_frames =
            decoded_active_hitbox_frame_numbers(decoded, artifact_total_frames(artifact));
    }
    let frame_hitboxes = active_hitboxes_by_frame(
        decoded,
        source_frames,
        materialization_frames.iter().copied(),
    );
    for (frame, hitboxes) in frame_hitboxes {
        let hitbox_values = Value::Array(hitboxes);
        if hitbox_values.as_array().is_some_and(Vec::is_empty) {
            continue;
        }
        let pose = source_frames
            .and_then(|frames| {
                frames
                    .iter()
                    .find(|source_frame| {
                        source_frame.get("frame").and_then(Value::as_u64) == Some(frame)
                    })
                    .and_then(|source_frame| source_frame.get("pose"))
            })
            .cloned()
            .unwrap_or_else(|| json!([]));
        if let Some(existing) = keyframes
            .iter_mut()
            .find(|item| item.get("frame").and_then(Value::as_u64) == Some(frame))
        {
            if !pose.as_array().is_some_and(Vec::is_empty) {
                existing["pose"] = pose;
            }
            existing["hitboxes"] = hitbox_values;
        } else {
            keyframes.push(json!({
                "frame": frame,
                "interpolates_from_previous": true,
                "pose": pose,
                "hitboxes": hitbox_values,
                "hurtboxes": [],
            }));
        }
    }
    keyframes.sort_by_key(|item| item.get("frame").and_then(Value::as_u64).unwrap_or(0));
    artifact["keyframes"] = Value::Array(keyframes);
}

fn apply_source_hurtboxes_to_keyframes(artifact: &mut Value, hurtbox_frames: &[Value]) {
    let mut keyframes = artifact
        .get("keyframes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for keyframe in &mut keyframes {
        if let Some(hurtboxes) = keyframe.get_mut("hurtboxes").and_then(Value::as_array_mut) {
            hurtboxes.retain(|hurtbox| {
                hurtbox.get("source").and_then(Value::as_str) != Some("generated_ecb_preview")
            });
        }
    }

    for source_frame in hurtbox_frames {
        let Some(frame_number) = source_frame.get("frame").and_then(Value::as_u64) else {
            continue;
        };
        let hurtboxes = source_frame
            .get("hurtboxes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(existing) = keyframes
            .iter_mut()
            .find(|item| item.get("frame").and_then(Value::as_u64) == Some(frame_number))
        {
            if let Some(pose) = source_frame.get("pose") {
                existing["pose"] = pose.clone();
            }
            existing["hurtboxes"] = Value::Array(hurtboxes);
        } else {
            let pose = source_frame
                .get("pose")
                .cloned()
                .unwrap_or_else(|| json!([]));
            keyframes.push(json!({
                "frame": frame_number,
                "interpolates_from_previous": true,
                "pose": pose,
                "hitboxes": [],
                "hurtboxes": hurtboxes,
            }));
        }
    }

    keyframes.sort_by_key(|item| item.get("frame").and_then(Value::as_u64).unwrap_or(0));
    artifact["keyframes"] = Value::Array(keyframes);
}

fn apply_decoded_hurt_state_overlays(artifact: &mut Value, decoded: &DecodedActionScript) {
    let Some(keyframes) = artifact.get_mut("keyframes").and_then(Value::as_array_mut) else {
        return;
    };
    let hurt_states = decoded
        .procedures
        .iter()
        .filter_map(|procedure| match procedure {
            DecodedProcedure::SetHurtState(hurt_state) => Some(hurt_state),
            _ => None,
        })
        .collect::<Vec<_>>();
    if hurt_states.is_empty() {
        return;
    }

    for keyframe in keyframes {
        let Some(frame_number) = keyframe.get("frame").and_then(Value::as_u64) else {
            continue;
        };
        let Some(hurtboxes) = keyframe.get_mut("hurtboxes").and_then(Value::as_array_mut) else {
            continue;
        };
        for hurtbox in hurtboxes {
            let Some(bone) = hurtbox.get("bone").and_then(Value::as_u64) else {
                continue;
            };
            let Some(hurt_state) = hurt_states
                .iter()
                .filter(|hurt_state| {
                    hurt_state.bone_idx == bone && hurt_state.frame <= frame_number
                })
                .max_by_key(|hurt_state| (hurt_state.frame, hurt_state.word_offset))
            else {
                continue;
            };
            hurtbox["state"] = json!(hurt_capsule_state_name(hurt_state.state));
            hurtbox["state_raw"] = json!(hurt_state.state);
            hurtbox["state_source"] = json!("decoded_action_script");
            hurtbox["source_handler"] = json!("ftAction_80071A9C");
            hurtbox["source_word_offset"] = json!(hurt_state.word_offset);
        }
    }
}

fn apply_source_active_hurtbox_windows(artifact: &mut Value, hurtbox_frames: &[Value]) {
    let frames = hurtbox_frames
        .iter()
        .filter(|source_frame| {
            source_frame
                .get("hurtboxes")
                .and_then(Value::as_array)
                .is_some_and(|hurtboxes| !hurtboxes.is_empty())
        })
        .filter_map(|source_frame| source_frame.get("frame").and_then(Value::as_u64))
        .collect::<Vec<_>>();
    let Some(start) = frames.iter().min().copied() else {
        return;
    };
    let Some(end) = frames.iter().max().copied() else {
        return;
    };
    if !artifact.get("summary").is_some_and(Value::is_object) {
        artifact["summary"] = json!({});
    }
    bump_summary_total_frames(artifact, end);
    artifact["summary"]["active_hurtbox_windows"] = json!([{
        "start": start,
        "end": end,
        "source": "source_hurtbox_samples",
    }]);
}

fn apply_source_body_volumes_to_keyframes(artifact: &mut Value, ecb_frames: &[Value]) {
    let mut keyframes = artifact
        .get("keyframes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for source_frame in ecb_frames {
        let Some(source_frame_number) = source_frame.get("frame").and_then(Value::as_u64) else {
            continue;
        };
        let Some(body_volume) = body_volume_json(source_frame) else {
            continue;
        };
        let frame_number = source_frame_number + 1;
        if let Some(existing) = keyframes
            .iter_mut()
            .find(|item| item.get("frame").and_then(Value::as_u64) == Some(frame_number))
        {
            existing["body_volumes"] = Value::Array(vec![body_volume]);
        } else {
            keyframes.push(json!({
                "frame": frame_number,
                "interpolates_from_previous": true,
                "pose": [],
                "hitboxes": [],
                "hurtboxes": [],
                "body_volumes": [body_volume],
            }));
        }
    }

    keyframes.sort_by_key(|item| item.get("frame").and_then(Value::as_u64).unwrap_or(0));
    artifact["keyframes"] = Value::Array(keyframes);
}

fn body_volume_json(source_frame: &Value) -> Option<Value> {
    let top = ecb_point_json(source_frame, "top_raw")?;
    let bottom = ecb_point_json(source_frame, "bottom_raw")?;
    let left = ecb_point_json(source_frame, "left_raw")?;
    let right = ecb_point_json(source_frame, "right_raw")?;
    Some(json!({
        "id": "ecb",
        "kind": "diamond",
        "source": "ftData.x44 + PlCaAJ FigaTree + PlCaNr JObj skeleton",
        "top": top,
        "bottom": bottom,
        "left": left,
        "right": right,
        "source_top": top,
        "source_bottom": bottom,
        "source_left": left,
        "source_right": right,
        "source_points": source_frame
            .get("source_points")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "source_render_transform": source_frame
            .get("source_render_transform")
            .cloned()
            .unwrap_or_else(|| json!(MELEE_RIGHT_FACING_RENDER_TRANSFORM)),
        "flatten_after_render": source_frame
            .get("flatten_after_render")
            .cloned()
            .unwrap_or_else(|| json!(MELEE_RIGHT_FACING_FLATTEN_POLICY)),
        "source_joint_indices": source_frame
            .get("source_joint_indices")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "confidence": "source_extracted",
    }))
}

fn ecb_point_json(source_frame: &Value, field: &str) -> Option<Value> {
    let point = source_frame.get(field)?;
    Some(json!({
        "x": point.get("x").and_then(Value::as_f64)?,
        "y": point.get("y").and_then(Value::as_f64)?,
        "z": 0.0,
    }))
}

fn apply_source_active_body_volume_windows(artifact: &mut Value, ecb_frames: &[Value]) {
    let frames = ecb_frames
        .iter()
        .filter_map(|source_frame| source_frame.get("frame").and_then(Value::as_u64))
        .map(|frame| frame + 1)
        .collect::<Vec<_>>();
    let Some(start) = frames.iter().min().copied() else {
        return;
    };
    let Some(end) = frames.iter().max().copied() else {
        return;
    };
    if !artifact.get("summary").is_some_and(Value::is_object) {
        artifact["summary"] = json!({});
    }
    bump_summary_total_frames(artifact, end);
    artifact["summary"]["active_body_volume_windows"] = json!([{
        "start": start,
        "end": end,
        "source": "source_ecb_samples",
    }]);
}

fn bump_summary_total_frames(artifact: &mut Value, max_frame: u64) {
    if !artifact.get("summary").is_some_and(Value::is_object) {
        artifact["summary"] = json!({});
    }
    let current = artifact["summary"]
        .get("total_frames")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if max_frame > current {
        artifact["summary"]["total_frames"] = json!(max_frame);
    }
}

#[derive(Debug, Clone, Copy)]
enum SourceHitCapsuleState {
    Enabled,
    Unk2,
    Unk3,
}

#[derive(Debug, Clone)]
struct ActiveHitboxSlot {
    hitbox: DecodedHitbox,
    state: SourceHitCapsuleState,
    current_source_center: Option<[f64; 3]>,
}

fn active_hitboxes_by_frame<I>(
    decoded: &DecodedActionScript,
    source_frames: Option<&[Value]>,
    frame_numbers: I,
) -> BTreeMap<u64, Vec<Value>>
where
    I: IntoIterator<Item = u64>,
{
    let mut procedures = decoded.procedures.iter().collect::<Vec<_>>();
    procedures
        .sort_by_key(|procedure| (procedure_frame(procedure), procedure_word_offset(procedure)));
    let frame_numbers = frame_numbers.into_iter().collect::<BTreeSet<_>>();
    let mut result = BTreeMap::new();
    let mut active: BTreeMap<u64, ActiveHitboxSlot> = BTreeMap::new();
    let mut procedure_index = 0;
    for frame_number in frame_numbers {
        while let Some(procedure) = procedures.get(procedure_index) {
            if procedure_frame(procedure) > frame_number {
                break;
            }
            match procedure {
                DecodedProcedure::SpawnHitbox(hitbox) => {
                    active.insert(
                        hitbox.id,
                        ActiveHitboxSlot {
                            hitbox: hitbox.clone(),
                            state: SourceHitCapsuleState::Enabled,
                            current_source_center: None,
                        },
                    );
                }
                DecodedProcedure::ClearAllHitboxes { .. } => {
                    active.clear();
                }
                DecodedProcedure::SetHurtState(_)
                | DecodedProcedure::SetCmdVar(_)
                | DecodedProcedure::SetThrowFlag(_)
                | DecodedProcedure::SetThrowHitbox(_)
                | DecodedProcedure::SetJabCombo(_)
                | DecodedProcedure::SetJabRapid(_) => {}
            }
            procedure_index += 1;
        }
        if active.is_empty() {
            continue;
        }
        let mut hitboxes = Vec::new();
        for slot in active.values_mut() {
            let pose_matrix = source_frames.and_then(|frames| {
                source_pose_matrix_for_frame(frames, &slot.hitbox, frame_number)
            });
            let source_center = hitbox_source_center(&slot.hitbox, pose_matrix);
            let previous_source_center = match slot.state {
                SourceHitCapsuleState::Enabled => source_center,
                SourceHitCapsuleState::Unk2 | SourceHitCapsuleState::Unk3 => {
                    slot.current_source_center.unwrap_or(source_center)
                }
            };
            let next_state = match slot.state {
                SourceHitCapsuleState::Enabled => SourceHitCapsuleState::Unk2,
                SourceHitCapsuleState::Unk2 | SourceHitCapsuleState::Unk3 => {
                    SourceHitCapsuleState::Unk3
                }
            };
            hitboxes.push(hitbox_json(
                &slot.hitbox,
                pose_matrix,
                previous_source_center,
                next_state,
            ));
            slot.current_source_center = Some(source_center);
            slot.state = next_state;
        }
        result.insert(frame_number, hitboxes);
    }
    result
}

fn procedure_frame(procedure: &DecodedProcedure) -> u64 {
    match procedure {
        DecodedProcedure::SpawnHitbox(hitbox) => hitbox.frame,
        DecodedProcedure::SetHurtState(hurt_state) => hurt_state.frame,
        DecodedProcedure::SetCmdVar(cmd_var) => cmd_var.frame,
        DecodedProcedure::SetThrowFlag(throw_flag) => throw_flag.frame,
        DecodedProcedure::SetThrowHitbox(throw_hitbox) => throw_hitbox.frame,
        DecodedProcedure::SetJabCombo(jab_combo) => jab_combo.frame,
        DecodedProcedure::SetJabRapid(jab_rapid) => jab_rapid.frame,
        DecodedProcedure::ClearAllHitboxes { frame, .. } => *frame,
    }
}

fn procedure_word_offset(procedure: &DecodedProcedure) -> usize {
    match procedure {
        DecodedProcedure::SpawnHitbox(hitbox) => hitbox.word_offset,
        DecodedProcedure::SetHurtState(hurt_state) => hurt_state.word_offset,
        DecodedProcedure::SetCmdVar(cmd_var) => cmd_var.word_offset,
        DecodedProcedure::SetThrowFlag(throw_flag) => throw_flag.word_offset,
        DecodedProcedure::SetThrowHitbox(throw_hitbox) => throw_hitbox.word_offset,
        DecodedProcedure::SetJabCombo(jab_combo) => jab_combo.word_offset,
        DecodedProcedure::SetJabRapid(jab_rapid) => jab_rapid.word_offset,
        DecodedProcedure::ClearAllHitboxes { word_offset, .. } => *word_offset,
    }
}

fn hitbox_json(
    hitbox: &DecodedHitbox,
    pose_matrix: Option<[[f64; 4]; 3]>,
    previous_source_center: [f64; 3],
    source_hit_capsule_state: SourceHitCapsuleState,
) -> Value {
    let source_center = hitbox_source_center(hitbox, pose_matrix);
    let center = right_facing_render_flattened_point(source_center);
    let previous_center = right_facing_render_flattened_point(previous_source_center);
    let source_space = if pose_matrix.is_some() {
        "ftAction_8007121C JObj local offset transformed by sampled pose"
    } else {
        "ftAction_8007121C JObj local offset"
    };
    json!({
        "id": hitbox.id,
        "kind": "sphere",
        "bone": hitbox.bone,
        "hit_group": hitbox.hit_group,
        "use_common_bone_ids": hitbox.use_common_bone_ids,
        "center": {
            "x": center[0],
            "y": center[1],
            "z": center[2],
        },
        "previous_center": {
            "x": previous_center[0],
            "y": previous_center[1],
            "z": previous_center[2],
        },
        "source_center": {
            "x": source_center[0],
            "y": source_center[1],
            "z": source_center[2],
        },
        "source_previous_center": {
            "x": previous_source_center[0],
            "y": previous_source_center[1],
            "z": previous_source_center[2],
        },
        "source_offset": {
            "x": hitbox.center_x,
            "y": hitbox.center_y,
            "z": hitbox.center_z,
        },
        "source_pose_joint": hitbox.bone,
        "source_space": source_space,
        "source_render_transform": MELEE_RIGHT_FACING_RENDER_TRANSFORM,
        "flatten_after_render": MELEE_RIGHT_FACING_FLATTEN_POLICY,
        "source_hit_capsule_state": source_hit_capsule_state_name(source_hit_capsule_state),
        "source_sweep": "ftColl_8007AD18 x58(previous) -> x4C(current)",
        "radius": hitbox.radius,
        "damage": hitbox.damage,
        "angle": hitbox.angle,
        "kbg": hitbox.kbg,
        "weight_set_kb": hitbox.weight_set_kb,
        "bkb": hitbox.bkb,
        "element": hitbox.element,
        "shield_damage": hitbox.shield_damage,
        "hit_grounded": hitbox.hit_grounded,
        "hit_aerial": hitbox.hit_aerial,
        "item_hit_interaction": hitbox.item_hit_interaction,
        "ignore_thrown_fighters": hitbox.ignore_thrown_fighters,
        "ignore_fighter_scale": hitbox.ignore_fighter_scale,
        "clank": hitbox.clank,
        "rebound": hitbox.rebound,
        "skip_if_thrown_hitbox_owner_absent": hitbox.skip_if_thrown_hitbox_owner_absent,
        "hit_grabbed_victim_only": hitbox.hit_grabbed_victim_only,
        "source": "decoded_action_script",
        "source_handler": "ftAction_8007121C",
        "source_word_offset": hitbox.word_offset,
        "confidence": "source_extracted",
    })
}

fn hitbox_source_center(hitbox: &DecodedHitbox, pose_matrix: Option<[[f64; 4]; 3]>) -> [f64; 3] {
    let source_offset = [hitbox.center_x, hitbox.center_y, hitbox.center_z];
    pose_matrix
        .map(|matrix| matrix_transform_point(matrix, source_offset))
        .unwrap_or(source_offset)
}

fn right_facing_render_flattened_point(source: [f64; 3]) -> [f64; 3] {
    [source[0], source[1], 0.0]
}

fn source_hit_capsule_state_name(state: SourceHitCapsuleState) -> &'static str {
    match state {
        SourceHitCapsuleState::Enabled => "HitCapsule_Enabled",
        SourceHitCapsuleState::Unk2 => "HitCapsule_Unk2",
        SourceHitCapsuleState::Unk3 => "HitCapsule_Unk3",
    }
}

fn source_pose_matrix_for_frame(
    source_frames: &[Value],
    hitbox: &DecodedHitbox,
    frame_number: u64,
) -> Option<[[f64; 4]; 3]> {
    if hitbox.use_common_bone_ids {
        return None;
    }
    let frame = source_frames
        .iter()
        .find(|frame| frame.get("frame").and_then(Value::as_u64) == Some(frame_number))?;
    let joints = frame
        .get("pose")
        .and_then(|pose| pose.get("joints"))
        .and_then(Value::as_array)?;
    let joint = joints.iter().find(|joint| {
        joint
            .get("index")
            .and_then(Value::as_u64)
            .is_some_and(|index| index == hitbox.bone)
    })?;
    value_matrix_3x4(joint.get("world_matrix")?)
}

fn value_matrix_3x4(value: &Value) -> Option<[[f64; 4]; 3]> {
    let rows = value.as_array()?;
    if rows.len() != 3 {
        return None;
    }
    let mut matrix = [[0.0; 4]; 3];
    for (row_index, row) in rows.iter().enumerate() {
        let values = row.as_array()?;
        if values.len() != 4 {
            return None;
        }
        for (col_index, value) in values.iter().enumerate() {
            matrix[row_index][col_index] = value.as_f64()?;
        }
    }
    Some(matrix)
}

fn matrix_transform_point(matrix: [[f64; 4]; 3], point: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * point[0] + matrix[0][1] * point[1] + matrix[0][2] * point[2] + matrix[0][3],
        matrix[1][0] * point[0] + matrix[1][1] * point[1] + matrix[1][2] * point[2] + matrix[1][3],
        matrix[2][0] * point[0] + matrix[2][1] * point[1] + matrix[2][2] * point[2] + matrix[2][3],
    ]
}

fn apply_decoded_active_hitbox_windows(artifact: &mut Value, decoded: &DecodedActionScript) {
    let windows = decoded_active_hitbox_windows(decoded, artifact_total_frames(artifact))
        .into_iter()
        .map(|(start, end)| {
            json!({
                "start": start,
                "end": end,
                "source": "decoded_action_script",
            })
        })
        .collect::<Vec<_>>();
    if !windows.is_empty() {
        if !artifact.get("summary").is_some_and(Value::is_object) {
            artifact["summary"] = json!({});
        }
        artifact["summary"]["active_hitbox_windows"] = Value::Array(windows);
    }
}

fn decoded_active_hitbox_frame_numbers(
    decoded: &DecodedActionScript,
    total_frames: Option<u64>,
) -> BTreeSet<u64> {
    decoded_active_hitbox_windows(decoded, total_frames)
        .into_iter()
        .flat_map(|(start, end)| start..=end)
        .collect()
}

fn decoded_active_hitbox_windows(
    decoded: &DecodedActionScript,
    total_frames: Option<u64>,
) -> Vec<(u64, u64)> {
    let spawn_frames = decoded
        .procedures
        .iter()
        .filter_map(|procedure| match procedure {
            DecodedProcedure::SpawnHitbox(hitbox) => Some(hitbox.frame),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let clear_frames = decoded
        .procedures
        .iter()
        .filter_map(|procedure| match procedure {
            DecodedProcedure::ClearAllHitboxes { frame, .. } => Some(*frame),
            _ => None,
        })
        .collect::<Vec<_>>();
    spawn_frames
        .into_iter()
        .map(|start| {
            let end = clear_frames
                .iter()
                .copied()
                .find(|clear| *clear > start)
                .map(|clear| clear.saturating_sub(1))
                .or_else(|| total_frames.filter(|total| *total >= start))
                .unwrap_or(start);
            (start, end)
        })
        .collect()
}

fn artifact_total_frames(artifact: &Value) -> Option<u64> {
    artifact
        .get("summary")
        .and_then(|summary| summary.get("total_frames"))
        .and_then(Value::as_u64)
}

fn remove_gap(artifact: &mut Value, field: &str) {
    let Some(gaps) = artifact.get_mut("gaps").and_then(Value::as_array_mut) else {
        return;
    };
    gaps.retain(|gap| gap.get("field").and_then(Value::as_str) != Some(field));
}
