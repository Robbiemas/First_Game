use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{base_report, FrameDataCommand, FrameDataOptions};

pub(crate) fn frame_data_report(root: &Path, command: &FrameDataCommand) -> Value {
    match command {
        FrameDataCommand::Extract(options) => {
            artifact_report(root, "frame-data extract", options, true)
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
    let action = find_action_record(&action_table, &probe, &options.state).ok_or_else(|| {
        format!(
            "could not find source action state {} in {}",
            options.state,
            action_table_path.display()
        )
    })?;
    let action_state_id = action
        .get("action_state_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "source action record does not contain action_state_id".to_string())?;
    let subaction_script_offset = action
        .get("subaction_script_offset")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_frames = action
        .get("figatree")
        .and_then(|figatree| figatree.get("frames_ticks"))
        .and_then(Value::as_u64)
        .unwrap_or(0);

    Ok(json!({
        "schema_version": 1,
        "target_character": options.character,
        "target_character_label": character_label(&options.character),
        "source_character": source_character,
        "source_character_label": character_label(source_character),
        "state": options.state,
        "label": options.state,
        "projection": {
            "source_space": "melee_xyz",
            "default_view": "xy",
            "z_policy": "preserve_and_project",
        },
        "sources": [{
            "kind": "source_action_table",
            "path": path_for_artifact(root, &action_table_path),
            "action_state_id": action_state_id,
            "subaction_script_offset": subaction_script_offset,
            "source_action_name": action.get("name").and_then(Value::as_str).unwrap_or(""),
            "figatree_root": action.get("figatree_root").and_then(Value::as_str).unwrap_or(""),
            "purpose": "source action record used to initialize frame-data import artifact",
        }],
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

fn character_label(character: &str) -> String {
    match character {
        "captain" | "captain_falcon" => "Captain Falcon".to_string(),
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
}

#[derive(Debug, Clone)]
struct DecodedHurtState {
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    bone_idx: u64,
    state: u64,
}

const FIGHTER_CMD_LENGTHS: [usize; 49] = [
    5, 5, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 7, 4, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 3, 2, 1, 4,
];
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
        .get("state")
        .and_then(Value::as_str)
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
        other => format!("{other}_action_ecb_samples.json"),
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
    actions.iter().find(|action| {
        action
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.contains(state))
            || action
                .get("figatree_root")
                .and_then(Value::as_str)
                .is_some_and(|name| name.contains(state))
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
            8 | 19 => {
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
                    _ => {}
                }
                word_offset += length;
            }
            _ => return None,
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
    if let Some(ecb_frames) =
        source_ecb_frames_for_action(root, &decoded.source_character, decoded.action_state_id)
    {
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
                DecodedProcedure::SetHurtState(_) => {}
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
        DecodedProcedure::ClearAllHitboxes { frame, .. } => *frame,
    }
}

fn procedure_word_offset(procedure: &DecodedProcedure) -> usize {
    match procedure {
        DecodedProcedure::SpawnHitbox(hitbox) => hitbox.word_offset,
        DecodedProcedure::SetHurtState(hurt_state) => hurt_state.word_offset,
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
    [source[2], source[1], 0.0]
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
