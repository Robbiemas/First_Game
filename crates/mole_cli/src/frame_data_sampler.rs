use mole_core::collision::{Mat3x4, Vec3};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{base_report, FrameDataSampleOptions};

const HSD_A_OP_CON: u8 = 1;
const HSD_A_OP_LIN: u8 = 2;
const HSD_A_OP_SPL0: u8 = 3;
const HSD_A_OP_SPL: u8 = 4;
const HSD_A_OP_SLP: u8 = 5;
const HSD_A_OP_KEY: u8 = 6;

const HSD_A_FRAC_FLOAT: u8 = 0 << 5;
const HSD_A_FRAC_S16: u8 = 1 << 5;
const HSD_A_FRAC_U16: u8 = 2 << 5;
const HSD_A_FRAC_S8: u8 = 3 << 5;
const HSD_A_FRAC_U8: u8 = 4 << 5;

const FOBJ_LOAD_DATA0: u8 = 1;
const FOBJ_LOAD_DATA: u8 = 2;
const FOBJ_LOAD_WAIT: u8 = 3;

pub(crate) fn frame_data_sample_report(root: &Path, options: &FrameDataSampleOptions) -> Value {
    let mut report = base_report("frame-data sample", root);
    report["character"] = json!(options.character);
    report["source_character"] = json!(options.source_character);
    report["state"] = json!(options.state);
    report["frame"] = json!(options.frame);
    report["wrote_artifact"] = json!(false);
    report["source_space"] = json!("melee_xyz");

    match sample_state_frame(root, options) {
        Ok(sample) => {
            report["ok"] = json!(true);
            report["sample"] = sample;
            report["errors"] = json!([]);
        }
        Err(error) => {
            report["ok"] = json!(false);
            report["sample"] = Value::Null;
            report["errors"] = json!([error]);
        }
    }

    report
}

pub(crate) fn sample_state_frame(
    root: &Path,
    options: &FrameDataSampleOptions,
) -> Result<Value, String> {
    sample_action_keyframes(root, options)?
        .into_iter()
        .find(|sample| sample.get("frame").and_then(Value::as_u64) == Some(options.frame))
        .ok_or_else(|| {
            format!(
                "source manifest action `{}` has no sampled frame {}",
                options.state, options.frame
            )
        })
}

pub(crate) fn sample_action_keyframes(
    root: &Path,
    options: &FrameDataSampleOptions,
) -> Result<Vec<Value>, String> {
    let source = load_action_sample_source(root, options)?;
    let mut poses = Vec::new();
    for frame in 1..=source.total_frames {
        poses.push((
            frame,
            sample_pose(
                &source.figatree_chunk,
                &source.figatree,
                &source.skeleton,
                frame,
            )?,
        ));
    }
    let hitboxes_by_frame = sample_hitboxes_by_frame(&poses, &source.procedures);
    let mut keyframes = Vec::new();
    let mut previous_transn = Vec3::new(0.0, 0.0, 0.0);
    for (frame, pose) in &poses {
        let hitboxes = hitboxes_by_frame.get(frame).cloned().unwrap_or_default();
        let hurtboxes =
            sample_hurtboxes_for_frame(pose, &source.hurtbox_inits, &source.procedures, *frame)?;
        let source_root_motion = source_root_motion_json(pose, previous_transn)?;
        previous_transn = transn_translation(pose)?;
        keyframes.push(source.sample_json(
            *frame,
            pose.len(),
            hitboxes,
            hurtboxes,
            source_root_motion,
        ));
    }
    Ok(keyframes)
}

struct ActionSampleSource {
    character: String,
    manifest_source_character: String,
    state: String,
    source_action_key: String,
    source_action_name: String,
    action_state_id: u64,
    total_frames: u64,
    projection: Value,
    figatree_chunk: Vec<u8>,
    figatree: Value,
    skeleton: Vec<Value>,
    hurtbox_inits: Vec<Value>,
    procedures: Vec<Procedure>,
}

impl ActionSampleSource {
    fn sample_json(
        &self,
        frame: u64,
        pose_joint_count: usize,
        hitboxes: Vec<Value>,
        hurtboxes: Vec<Value>,
        source_root_motion: Value,
    ) -> Value {
        json!({
            "artifact_kind": "source_action_frame_sample",
            "character": self.character,
            "source_character": self.manifest_source_character,
            "state": self.state,
            "source_action_key": self.source_action_key,
            "source_action_name": self.source_action_name,
            "action_state_id": self.action_state_id,
            "frame": frame,
            "source_space": "melee_xyz",
            "projected_view_kind": "derived_debug_view",
            "projection": self.projection,
            "pose_joint_count": pose_joint_count,
            "source_root_motion": source_root_motion,
            "hit_capsules": hitboxes,
            "hurt_capsules": hurtboxes,
        })
    }
}

fn load_action_sample_source(
    root: &Path,
    options: &FrameDataSampleOptions,
) -> Result<ActionSampleSource, String> {
    let manifest_path = root
        .join("resources")
        .join("melee")
        .join("frame_data")
        .join(&options.character)
        .join("source_manifest.json");
    let manifest = read_json(&manifest_path)?;
    let manifest_source_character =
        optional_str(&manifest, &["source_character"]).unwrap_or_default();
    if let Some(requested_source_character) = &options.source_character {
        if requested_source_character != &manifest_source_character {
            return Err(format!(
                "source manifest source character mismatch: requested `{requested_source_character}`, manifest has `{manifest_source_character}`"
            ));
        }
    }
    let action = manifest_action(&manifest, &options.state)?;
    let projection = manifest
        .get("projection")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let action_state_id = required_u64(&action, &["action_state_id"])?;
    let total_frames = required_u64(action, &["total_frames"])?;
    let figatree_offset =
        required_u64(&action, &["source_action", "figatree_archive_offset"])? as usize;
    let figatree_size =
        required_u64(&action, &["source_action", "figatree_archive_size"])? as usize;
    let figatree = required_value(&action, &["source_action", "figatree"])?;
    let skeleton = required_array(&manifest, &["rig", "skeleton", "data", "joints"])?;
    let hurtbox_inits = required_array(&manifest, &["rig", "hurtbox_inits", "data", "hurtboxes"])?;
    let action_table_path = source_action_table_path(root, &manifest)?;
    let action_table = read_json(&action_table_path)?;
    let plcaaj_path = action_table_plcaaj_path(root, &action_table)?;
    let figatree_chunk = read_figatree_chunk(&plcaaj_path, figatree_offset, figatree_size)?;
    let procedures = decode_procedures(&action)?;

    Ok(ActionSampleSource {
        character: options.character.clone(),
        manifest_source_character,
        state: optional_str(&action, &["state"]).unwrap_or_else(|| options.state.clone()),
        source_action_key: optional_str(&action, &["source_action_key"])
            .unwrap_or_else(|| options.state.clone()),
        source_action_name: optional_str(&action, &["source_action_name"]).unwrap_or_default(),
        action_state_id,
        total_frames,
        projection,
        figatree_chunk,
        figatree: figatree.clone(),
        skeleton: skeleton.to_vec(),
        hurtbox_inits: hurtbox_inits.to_vec(),
        procedures,
    })
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn manifest_action<'a>(manifest: &'a Value, state: &str) -> Result<&'a Value, String> {
    let actions = required_array(manifest, &["actions"])?;
    actions
        .iter()
        .find(|action| {
            optional_str(action, &["state"]).as_deref() == Some(state)
                || optional_str(action, &["source_action_key"]).as_deref() == Some(state)
        })
        .ok_or_else(|| format!("source manifest action `{state}` not found"))
}

fn source_action_table_path(root: &Path, manifest: &Value) -> Result<PathBuf, String> {
    let sources = required_array(manifest, &["sources"])?;
    let relative = sources
        .iter()
        .find(|source| optional_str(source, &["kind"]).as_deref() == Some("source_action_table"))
        .and_then(|source| optional_str(source, &["path"]))
        .ok_or_else(|| "source manifest is missing source_action_table path".to_string())?;
    Ok(root.join(relative))
}

fn action_table_plcaaj_path(root: &Path, action_table: &Value) -> Result<PathBuf, String> {
    let relative = optional_str(action_table, &["metadata", "plcaaj_file"])
        .or_else(|| optional_str(action_table, &["source", "plcaaj_file"]))
        .or_else(|| optional_str(action_table, &["plcaaj_file"]))
        .ok_or_else(|| "action animation table is missing plcaaj_file metadata".to_string())?;
    Ok(root.join(relative))
}

fn read_figatree_chunk(path: &Path, offset: usize, size: usize) -> Result<Vec<u8>, String> {
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

#[derive(Clone, Copy)]
struct JointPose {
    world_matrix: Mat3x4,
    local_translation: Vec3,
}

fn sample_pose(
    figatree_chunk: &[u8],
    figatree: &Value,
    skeleton: &[Value],
    frame_number: u64,
) -> Result<Vec<JointPose>, String> {
    let frame = frame_number.saturating_sub(1) as f32;
    let track_counts = required_array(figatree, &["track_counts_by_node"])?;
    let tracks = required_array(figatree, &["tracks"])?;

    let mut matrices = Vec::with_capacity(skeleton.len());
    let mut local_translations = Vec::with_capacity(skeleton.len());
    let mut global_scales: Vec<Option<Vec3>> = Vec::with_capacity(skeleton.len());

    for (index, joint) in skeleton.iter().enumerate() {
        let mut rotation = vec3_from_value(required_value(joint, &["rotation_raw"])?);
        let mut scale = vec3_from_value(required_value(joint, &["scale_raw"])?);
        let mut translation = vec3_from_value(required_value(joint, &["position_raw"])?);

        if index < track_counts.len() {
            let sampled = sample_figatree_node_tracks(
                figatree_chunk,
                figatree,
                tracks,
                track_counts,
                index,
                frame,
            )?;
            if let Some(value) = sampled.rotation.get("x") {
                rotation.x = *value;
            }
            if let Some(value) = sampled.rotation.get("y") {
                rotation.y = *value;
            }
            if let Some(value) = sampled.rotation.get("z") {
                rotation.z = *value;
            }
            if let Some(value) = sampled.translation.get("x") {
                translation.x = *value;
            }
            if let Some(value) = sampled.translation.get("y") {
                translation.y = *value;
            }
            if let Some(value) = sampled.translation.get("z") {
                translation.z = *value;
            }
            if let Some(value) = sampled.scale.get("x") {
                scale.x = *value;
            }
            if let Some(value) = sampled.scale.get("y") {
                scale.y = *value;
            }
            if let Some(value) = sampled.scale.get("z") {
                scale.z = *value;
            }
        }

        let parent_index = joint
            .get("parent_index")
            .and_then(Value::as_u64)
            .map(|value| value as usize);
        let parent_matrix = parent_index.and_then(|parent| matrices.get(parent).copied());
        let parent_scale =
            parent_index.and_then(|parent| global_scales.get(parent).cloned().flatten());
        let flags = parse_hex_u32(
            optional_str(joint, &["flags_raw"])
                .ok_or_else(|| "skeleton joint is missing flags_raw".to_string())?
                .as_str(),
        )?;

        let global_scale = if flags & 8 != 0 {
            parent_scale
        } else if let Some(parent_scale) = parent_scale {
            Some(Vec3::new(
                scale.x * parent_scale.x,
                scale.y * parent_scale.y,
                scale.z * parent_scale.z,
            ))
        } else {
            Some(scale)
        };

        let local = matrix_srt(scale, rotation, translation, parent_scale);
        let world = if let Some(parent_matrix) = parent_matrix {
            matrix_concat(parent_matrix, local)
        } else {
            local
        };
        matrices.push(world);
        global_scales.push(global_scale);
        local_translations.push(translation);
    }

    Ok(matrices
        .into_iter()
        .zip(local_translations)
        .map(|(world_matrix, local_translation)| JointPose {
            world_matrix,
            local_translation,
        })
        .collect())
}

fn source_root_motion_json(pose: &[JointPose], previous_transn: Vec3) -> Result<Value, String> {
    let transn = transn_translation(pose)?;
    let offset = Vec3::new(
        transn.x - previous_transn.x,
        transn.y - previous_transn.y,
        transn.z - previous_transn.z,
    );
    Ok(json!({
        "source": "ftAnim.c x68C_transNPos/x6A4_transNOffset sampled from FtPart_TransN",
        "source_part": "FtPart_TransN",
        "source_node_index": 1,
        "transn_position": source_vec3_json(transn),
        "transn_offset": source_vec3_json(offset),
    }))
}

fn transn_translation(pose: &[JointPose]) -> Result<Vec3, String> {
    pose.get(1)
        .map(|joint| joint.local_translation)
        .ok_or_else(|| "sampled pose is missing FtPart_TransN node 1".to_string())
}

fn source_vec3_json(value: Vec3) -> Value {
    json!({
        "x": value.x as f64,
        "y": value.y as f64,
        "z": value.z as f64,
    })
}

#[derive(Default)]
struct SampledNodeTracks {
    rotation: BTreeMap<&'static str, f32>,
    translation: BTreeMap<&'static str, f32>,
    scale: BTreeMap<&'static str, f32>,
}

fn sample_figatree_node_tracks(
    figatree_chunk: &[u8],
    figatree: &Value,
    tracks: &[Value],
    track_counts: &[Value],
    node_index: usize,
    frame: f32,
) -> Result<SampledNodeTracks, String> {
    let track_start = track_counts[..node_index]
        .iter()
        .map(|count| count.as_u64().unwrap_or(0) as usize)
        .sum::<usize>();
    let track_count = track_counts
        .get(node_index)
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let track_end = track_start + track_count;
    let action_name =
        optional_str(figatree, &["root"]).unwrap_or_else(|| "unknown_figatree".to_string());

    let mut sampled = SampledNodeTracks::default();
    for (local_index, track) in tracks[track_start..track_end].iter().enumerate() {
        let obj_type = required_u64(track, &["obj_type"])? as u8;
        let channel = jobj_anim_channel(obj_type).ok_or_else(|| {
            format!(
                "unsupported FigaTree channel for action `{action_name}`, node {node_index}, track {}: obj_type {}",
                local_index,
                obj_type
            )
        })?;
        let frac_value = parse_hex_u8(
            optional_str(track, &["frac_value_raw"])
                .ok_or_else(|| format!("missing frac_value_raw for action `{action_name}`"))?
                .as_str(),
        )?;
        let frac_slope = parse_hex_u8(
            optional_str(track, &["frac_slope_raw"])
                .ok_or_else(|| format!("missing frac_slope_raw for action `{action_name}`"))?
                .as_str(),
        )?;
        let data_offset = required_u64(track, &["data_offset"])? as usize;
        let length = required_u64(track, &["length"])? as usize;
        let startframe = required_u64(track, &["startframe"])? as i32;
        let start = 0x20usize.saturating_add(data_offset);
        let end = start.saturating_add(length);
        if end > figatree_chunk.len() {
            return Err(format!(
                "FigaTree data slice is out of bounds for action `{action_name}`, node {node_index}, track {}",
                local_index
            ));
        }
        let value = sample_fobj_value(
            &figatree_chunk[start..end],
            length,
            startframe,
            frac_value,
            frac_slope,
            frame,
        )
        .map_err(|error| {
            format!(
                "failed to sample FigaTree action `{action_name}`, node {node_index}, track {}, op/channel {}: {error}",
                local_index,
                obj_type
            )
        })?;
        let (bucket, axis) = channel;
        match bucket {
            "rotation" => {
                sampled.rotation.insert(axis, value);
            }
            "translation" => {
                sampled.translation.insert(axis, value);
            }
            "scale" => {
                sampled.scale.insert(axis, value);
            }
            _ => {}
        }
    }
    Ok(sampled)
}

fn sample_fobj_value(
    animation_data: &[u8],
    length: usize,
    startframe: i32,
    frac_value: u8,
    frac_slope: u8,
    frame: f32,
) -> Result<f32, String> {
    let data = &animation_data[..length.min(animation_data.len())];
    let mut pos = 0usize;
    let mut time = startframe as f32 + frame;
    let mut flags = 0u32;
    let mut state = FOBJ_LOAD_DATA0;
    let mut op = 0u8;
    let mut op_intrp = 0u8;
    let mut nb_pack = 0usize;
    let mut fterm = 0f32;
    let mut p0 = 0f32;
    let mut p1 = 0f32;
    let mut d0 = 0f32;
    let mut d1 = 0f32;
    let mut carried_fterm = 0f32;
    let mut last_value: Option<f32> = None;

    loop {
        if matches!(state, FOBJ_LOAD_DATA0 | FOBJ_LOAD_DATA) {
            if pos >= data.len() {
                state = 6;
                continue;
            }
            let load_state = state;
            op_intrp = op;
            if nb_pack == 0 {
                op = data[pos] & 0xF;
                let (next_nb_pack, next_pos) = parse_fobj_pack_info(data, pos)?;
                nb_pack = next_nb_pack;
                pos = next_pos;
            }
            nb_pack = nb_pack.saturating_sub(1);
            match op {
                HSD_A_OP_CON => {
                    p0 = p1;
                    let (value, next_pos) = parse_fobj_float(data, pos, frac_value)?;
                    p1 = value;
                    pos = next_pos;
                    if op_intrp != HSD_A_OP_SLP {
                        d0 = d1;
                        d1 = 0.0;
                    }
                }
                HSD_A_OP_LIN => {
                    p0 = p1;
                    let (value, next_pos) = parse_fobj_float(data, pos, frac_value)?;
                    p1 = value;
                    pos = next_pos;
                    if op_intrp != HSD_A_OP_SLP {
                        d0 = d1;
                        d1 = 0.0;
                    }
                }
                HSD_A_OP_SPL0 => {
                    p0 = p1;
                    d0 = d1;
                    let (value, next_pos) = parse_fobj_float(data, pos, frac_value)?;
                    p1 = value;
                    pos = next_pos;
                    d1 = 0.0;
                }
                HSD_A_OP_SPL => {
                    p0 = p1;
                    let (value, next_pos) = parse_fobj_float(data, pos, frac_value)?;
                    p1 = value;
                    pos = next_pos;
                    d0 = d1;
                    let (slope, next_pos) = parse_fobj_float(data, pos, frac_slope)?;
                    d1 = slope;
                    pos = next_pos;
                }
                HSD_A_OP_SLP => {
                    d0 = d1;
                    let (slope, next_pos) = parse_fobj_float(data, pos, frac_slope)?;
                    d1 = slope;
                    pos = next_pos;
                }
                HSD_A_OP_KEY => {
                    if flags & 0x40 != 0 {
                        op_intrp = op;
                        flags &= !0x40;
                        flags |= 0x80;
                        p0 = p1;
                    }
                    let (value, next_pos) = parse_fobj_float(data, pos, frac_value)?;
                    p1 = value;
                    pos = next_pos;
                    flags |= 0x40;
                }
                other => return Err(format!("unsupported FObj op {other}")),
            }
            state = if op == HSD_A_OP_SLP {
                load_state
            } else if load_state == FOBJ_LOAD_DATA0 {
                FOBJ_LOAD_WAIT
            } else {
                4
            };
            continue;
        }

        if state == FOBJ_LOAD_WAIT {
            if flags & 0x80 != 0 {
                if let Some(updated) =
                    update_anim(op_intrp, &mut flags, time, fterm, &mut p0, p1, &mut d0, d1)
                {
                    last_value = Some(updated);
                }
            }
            if pos >= data.len() {
                state = 6;
            } else {
                let (wait, next_pos) = parse_fobj_wait(data, pos)?;
                fterm = wait as f32;
                pos = next_pos;
                flags |= 0x20;
                state = FOBJ_LOAD_DATA;
            }
            continue;
        }

        if state == 4 {
            if fterm <= time {
                carried_fterm = fterm;
                time -= fterm;
                state = FOBJ_LOAD_WAIT;
                continue;
            }
            if let Some(updated) =
                update_anim(op_intrp, &mut flags, time, fterm, &mut p0, p1, &mut d0, d1)
            {
                return Ok(updated);
            }
            state = 5;
            continue;
        }

        if state == 5 {
            state = 4;
            continue;
        }

        if state == 6 {
            time += carried_fterm;
            if flags & 0x40 != 0 {
                op_intrp = op;
                flags &= !0x40;
                flags |= 0x80;
                p0 = p1;
            }
            if let Some(updated) =
                update_anim(op_intrp, &mut flags, time, fterm, &mut p0, p1, &mut d0, d1)
            {
                return Ok(updated);
            }
            if let Some(last_value) = last_value {
                return Ok(last_value);
            }
            return Err("FObj track produced no sampled value".to_string());
        }
    }
}

fn update_anim(
    op_intrp: u8,
    flags: &mut u32,
    time: f32,
    fterm: f32,
    p0: &mut f32,
    p1: f32,
    d0: &mut f32,
    d1: f32,
) -> Option<f32> {
    match op_intrp {
        HSD_A_OP_KEY => {
            if *flags & 0x80 != 0 {
                *flags &= !0x80;
                Some(*p0)
            } else {
                None
            }
        }
        HSD_A_OP_CON => Some(if time >= fterm { p1 } else { *p0 }),
        HSD_A_OP_LIN => {
            if *flags & 0x20 != 0 {
                *flags &= !0x20;
                if fterm != 0.0 {
                    *d0 = (p1 - *p0) / fterm;
                } else {
                    *d0 = 0.0;
                    *p0 = p1;
                }
            }
            Some(*d0 * time + *p0)
        }
        HSD_A_OP_SPL0 | HSD_A_OP_SPL | HSD_A_OP_SLP => {
            if fterm != 0.0 {
                Some(spl_get_hermite(1.0 / fterm, time, *p0, p1, *d0, d1))
            } else {
                Some(p1)
            }
        }
        _ => None,
    }
}

fn parse_fobj_float(data: &[u8], pos: usize, frac: u8) -> Result<(f32, usize), String> {
    if frac == HSD_A_FRAC_FLOAT {
        require_fobj_bytes(data, pos, 4)?;
        let bytes: [u8; 4] = data[pos..pos + 4]
            .try_into()
            .map_err(|_| "invalid f32 bytes".to_string())?;
        return Ok((f32::from_le_bytes(bytes), pos + 4));
    }
    let denom = (1u32 << (frac & 0x1F)) as f32;
    let frac_kind = frac & 0xE0;
    match frac_kind {
        HSD_A_FRAC_S8 => {
            require_fobj_bytes(data, pos, 1)?;
            Ok(((data[pos] as i8) as f32 / denom, pos + 1))
        }
        HSD_A_FRAC_U8 => {
            require_fobj_bytes(data, pos, 1)?;
            Ok((data[pos] as f32 / denom, pos + 1))
        }
        HSD_A_FRAC_S16 => {
            require_fobj_bytes(data, pos, 2)?;
            let bytes: [u8; 2] = data[pos..pos + 2]
                .try_into()
                .map_err(|_| "invalid s16 bytes".to_string())?;
            Ok((i16::from_le_bytes(bytes) as f32 / denom, pos + 2))
        }
        HSD_A_FRAC_U16 => {
            require_fobj_bytes(data, pos, 2)?;
            let bytes: [u8; 2] = data[pos..pos + 2]
                .try_into()
                .map_err(|_| "invalid u16 bytes".to_string())?;
            Ok((u16::from_le_bytes(bytes) as f32 / denom, pos + 2))
        }
        _ => Err(format!("unsupported FObj fraction byte 0x{frac:02x}")),
    }
}

fn parse_fobj_pack_info(data: &[u8], mut pos: usize) -> Result<(usize, usize), String> {
    require_fobj_bytes(data, pos, 1)?;
    let mut d = data[pos];
    pos += 1;
    let mut nb_pack = (((d >> 4) & 7) + 1) as usize;
    let mut shift = 3usize;
    while d & 0x80 != 0 {
        require_fobj_bytes(data, pos, 1)?;
        d = data[pos];
        pos += 1;
        nb_pack += ((d & 0x7F) as usize) << shift;
        shift += 7;
    }
    Ok((nb_pack, pos))
}

fn parse_fobj_wait(data: &[u8], mut pos: usize) -> Result<(u32, usize), String> {
    let mut wait = 0u32;
    let mut shift = 0u32;
    loop {
        require_fobj_bytes(data, pos, 1)?;
        let d = data[pos];
        pos += 1;
        wait |= ((d & 0x7F) as u32) << shift;
        shift += 7;
        if d & 0x80 == 0 {
            return Ok((wait, pos));
        }
    }
}

fn require_fobj_bytes(data: &[u8], pos: usize, count: usize) -> Result<(), String> {
    if pos.saturating_add(count) > data.len() {
        return Err(format!(
            "FObj stream ended early at byte {} while reading {} byte(s)",
            pos, count
        ));
    }
    Ok(())
}

fn spl_get_hermite(inv_fterm: f32, time: f32, p0: f32, p1: f32, d0: f32, d1: f32) -> f32 {
    let time_sq = time * time;
    let inv_sq = inv_fterm * inv_fterm;
    let time_sq_inv = time_sq * inv_fterm;
    let inv_sq_time_cu = inv_sq * (time_sq * time);
    let two_time_cu_inv_cu = 2.0 * inv_sq_time_cu * inv_fterm;
    let three_time_sq_inv_sq = 3.0 * time_sq * inv_sq;
    d1 * (inv_sq_time_cu - time_sq_inv)
        + d0 * (time + ((inv_sq_time_cu - time_sq_inv) - time_sq_inv))
        + p0 * (1.0 + (two_time_cu_inv_cu - three_time_sq_inv_sq))
        + p1 * (-two_time_cu_inv_cu + three_time_sq_inv_sq)
}

fn jobj_anim_channel(obj_type: u8) -> Option<(&'static str, &'static str)> {
    match obj_type {
        1 => Some(("rotation", "x")),
        2 => Some(("rotation", "y")),
        3 => Some(("rotation", "z")),
        5 => Some(("translation", "x")),
        6 => Some(("translation", "y")),
        7 => Some(("translation", "z")),
        8 => Some(("scale", "x")),
        9 => Some(("scale", "y")),
        10 => Some(("scale", "z")),
        _ => None,
    }
}

fn matrix_srt(
    scale: Vec3,
    rotation: Vec3,
    translation: Vec3,
    parent_scale: Option<Vec3>,
) -> Mat3x4 {
    let sin_x = rotation.x.sin();
    let cos_x = rotation.x.cos();
    let sin_y = rotation.y.sin();
    let cos_y = rotation.y.cos();
    let sin_z = rotation.z.sin();
    let cos_z = rotation.z.cos();

    let scale_x2 = scale.x;
    let mut scale_x1 = scale.x;
    let mut scale_x = scale.x;
    let mut scale_y2 = scale.y;
    let scale_y1 = scale.y;
    let mut scale_y = scale.y;
    let mut scale_z2 = scale.z;
    let mut scale_z1 = scale.z;
    let scale_z = scale.z;

    if let Some(parent_scale) = parent_scale {
        scale_y2 *= parent_scale.y / parent_scale.x;
        scale_z2 *= parent_scale.z / parent_scale.x;
        scale_x1 *= parent_scale.x / parent_scale.y;
        scale_z1 *= parent_scale.z / parent_scale.y;
        scale_x *= parent_scale.x / parent_scale.z;
        scale_y *= parent_scale.y / parent_scale.z;
    }

    Mat3x4::from_rows([
        [
            cos_z * (scale_x2 * cos_y),
            scale_y2 * ((cos_z * (sin_x * sin_y)) - (cos_x * sin_z)),
            scale_z2 * ((cos_z * (cos_x * sin_y)) + (sin_x * sin_z)),
            translation.x,
        ],
        [
            sin_z * (scale_x1 * cos_y),
            scale_y1 * ((sin_z * (sin_x * sin_y)) + (cos_x * cos_z)),
            scale_z1 * ((sin_z * (cos_x * sin_y)) - (sin_x * cos_z)),
            translation.y,
        ],
        [
            -scale_x * sin_y,
            cos_y * (scale_y * sin_x),
            cos_y * (scale_z * cos_x),
            translation.z,
        ],
    ])
}

fn matrix_concat(parent: Mat3x4, child: Mat3x4) -> Mat3x4 {
    let mut rows = [[0.0f32; 4]; 3];
    for row in 0..3 {
        for col in 0..4 {
            let mut value = parent.rows[row][0] * child.rows[0][col]
                + parent.rows[row][1] * child.rows[1][col]
                + parent.rows[row][2] * child.rows[2][col];
            if col == 3 {
                value += parent.rows[row][3];
            }
            rows[row][col] = value;
        }
    }
    Mat3x4::from_rows(rows)
}

#[derive(Clone)]
struct DecodedHitbox {
    frame: u64,
    word_offset: usize,
    id: u64,
    hit_group: u64,
    bone: u64,
    use_common_bone_ids: bool,
    damage: u64,
    radius: f32,
    center_x: f32,
    center_y: f32,
    center_z: f32,
    angle: u64,
    kbg: u64,
    weight_set_kb: u64,
    bkb: u64,
    element: u64,
    shield_damage: i64,
    hit_grounded: bool,
    hit_aerial: bool,
}

#[derive(Clone)]
enum Procedure {
    SpawnHitbox(DecodedHitbox),
    ClearAllHitboxes {
        frame: u64,
        word_offset: usize,
    },
    SetHurtState {
        frame: u64,
        bone_idx: u64,
        state: u64,
        word_offset: usize,
    },
}

fn decode_procedures(action: &Value) -> Result<Vec<Procedure>, String> {
    let procedures = required_array(action, &["decoded_action_script", "procedures"])?;
    let mut decoded = Vec::new();
    for procedure in procedures {
        let name = optional_str(procedure, &["procedure"]).unwrap_or_default();
        match name.as_str() {
            "fighter.spawn_hitbox" => {
                let raw_words = required_array(procedure, &["raw_words"])?;
                if raw_words.len() != 5 {
                    return Err("fighter.spawn_hitbox raw_words must have 5 entries".to_string());
                }
                let mut words = [0u32; 5];
                for (index, raw_word) in raw_words.iter().enumerate() {
                    words[index] =
                        parse_hex_u32(raw_word.as_str().ok_or_else(|| {
                            "spawn_hitbox raw word must be a string".to_string()
                        })?)?;
                }
                let frame = required_u64(procedure, &["frame"])?;
                let word_offset = required_u64(procedure, &["word_offset"])? as usize;
                decoded.push(Procedure::SpawnHitbox(decode_spawn_hitbox(
                    frame,
                    word_offset,
                    words,
                )));
            }
            "fighter.clear_all_hitboxes" => {
                decoded.push(Procedure::ClearAllHitboxes {
                    frame: required_u64(procedure, &["frame"])?,
                    word_offset: required_u64(procedure, &["word_offset"])? as usize,
                });
            }
            "fighter.set_hurt_state" => {
                let raw_words = required_array(procedure, &["raw_words"])?;
                let raw_word = raw_words
                    .first()
                    .and_then(Value::as_str)
                    .ok_or_else(|| "set_hurt_state raw_words[0] must be a string".to_string())?;
                let raw_word = parse_hex_u32(raw_word)?;
                decoded.push(decode_set_hurt_state(
                    required_u64(procedure, &["frame"])?,
                    required_u64(procedure, &["word_offset"])? as usize,
                    raw_word,
                ));
            }
            _ => {}
        }
    }
    decoded.sort_by_key(|procedure| match procedure {
        Procedure::SpawnHitbox(hitbox) => (hitbox.frame, hitbox.word_offset),
        Procedure::ClearAllHitboxes { frame, word_offset } => (*frame, *word_offset),
        Procedure::SetHurtState {
            frame, word_offset, ..
        } => (*frame, *word_offset),
    });
    Ok(decoded)
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
        id: bitfield(word0, 6, 3) as u64,
        hit_group: bitfield(word0, 9, 3) as u64,
        bone: bitfield(word0, 13, 8) as u64,
        use_common_bone_ids: bitfield(word0, 21, 1) != 0,
        damage: bitfield(word0, 22, 10) as u64,
        radius: bitfield(word1, 0, 16) as f32 / 256.0,
        center_x: sign_extend(bitfield(word1, 16, 16), 16) as f32 / 256.0,
        center_y: sign_extend(bitfield(word2, 0, 16), 16) as f32 / 256.0,
        center_z: sign_extend(bitfield(word2, 16, 16), 16) as f32 / 256.0,
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

fn decode_set_hurt_state(frame: u64, word_offset: usize, raw_word: u32) -> Procedure {
    Procedure::SetHurtState {
        frame,
        word_offset,
        bone_idx: bitfield(raw_word, 6, 8) as u64,
        state: bitfield(raw_word, 14, 18) as u64,
    }
}

#[derive(Clone, Copy)]
enum SourceHitCapsuleState {
    Enabled,
    Unk2,
    Unk3,
}

fn sample_hitboxes_by_frame(
    poses: &[(u64, Vec<JointPose>)],
    procedures: &[Procedure],
) -> BTreeMap<u64, Vec<Value>> {
    let mut active: BTreeMap<u64, (DecodedHitbox, SourceHitCapsuleState, Option<Vec3>)> =
        BTreeMap::new();
    let mut output = BTreeMap::new();
    for (frame, pose) in poses {
        for procedure in procedures
            .iter()
            .filter(|procedure| procedure_frame(procedure) == *frame)
        {
            match procedure {
                Procedure::SpawnHitbox(hitbox) => {
                    active.insert(
                        hitbox.id,
                        (hitbox.clone(), SourceHitCapsuleState::Enabled, None),
                    );
                }
                Procedure::ClearAllHitboxes { .. } => active.clear(),
                Procedure::SetHurtState { .. } => {}
            }
        }
        advance_hitbox_states(pose, &mut active);
        let frame_hitboxes = active
            .values()
            .map(|(hitbox, state, previous)| hitbox_sample_json(hitbox, *state, *previous, pose))
            .collect::<Vec<_>>();
        if !frame_hitboxes.is_empty() {
            output.insert(*frame, frame_hitboxes);
        }
    }
    output
}

fn hitbox_sample_json(
    hitbox: &DecodedHitbox,
    state: SourceHitCapsuleState,
    previous: Option<Vec3>,
    pose: &[JointPose],
) -> Value {
    let current_center = hitbox_source_center(hitbox, pose);
    let previous_center = match state {
        SourceHitCapsuleState::Enabled => current_center,
        SourceHitCapsuleState::Unk2 | SourceHitCapsuleState::Unk3 => {
            previous.unwrap_or(current_center)
        }
    };
    json!({
        "id": hitbox.id,
        "bone": hitbox.bone,
        "hit_group": hitbox.hit_group,
        "use_common_bone_ids": hitbox.use_common_bone_ids,
        "damage": hitbox.damage,
        "angle": hitbox.angle,
        "kbg": hitbox.kbg,
        "weight_set_kb": hitbox.weight_set_kb,
        "bkb": hitbox.bkb,
        "element": hitbox.element,
        "shield_damage": hitbox.shield_damage,
        "hit_grounded": hitbox.hit_grounded,
        "hit_aerial": hitbox.hit_aerial,
        "radius": hitbox.radius,
        "color": "red",
        "source_capsule_kind": "swept_sphere_capsule",
        "source_hit_capsule_state": source_hit_capsule_state_name(state),
        "source_handler": "ftAction_8007121C",
        "source_sweep": "ftColl_8007AD18 x58(previous) -> x4C(current)",
        "source_offset": point_json(Vec3::new(hitbox.center_x, hitbox.center_y, hitbox.center_z)),
        "source_previous_center": point_json(previous_center),
        "source_center": point_json(current_center),
        "source_a": point_json(previous_center),
        "source_b": point_json(current_center),
        "previous_center": point_json(flatten_right_facing(previous_center)),
        "center": point_json(flatten_right_facing(current_center)),
        "a": point_json(flatten_right_facing(previous_center)),
        "b": point_json(flatten_right_facing(current_center)),
        "projected_view_kind": "derived_debug_view",
    })
}

fn advance_hitbox_states(
    pose: &[JointPose],
    active: &mut BTreeMap<u64, (DecodedHitbox, SourceHitCapsuleState, Option<Vec3>)>,
) {
    for (hitbox, state, previous) in active.values_mut() {
        let current_center = hitbox_source_center(hitbox, pose);
        *previous = Some(current_center);
        *state = match state {
            SourceHitCapsuleState::Enabled => SourceHitCapsuleState::Unk2,
            SourceHitCapsuleState::Unk2 | SourceHitCapsuleState::Unk3 => {
                SourceHitCapsuleState::Unk3
            }
        };
    }
}

fn hitbox_source_center(hitbox: &DecodedHitbox, pose: &[JointPose]) -> Vec3 {
    let offset = Vec3::new(hitbox.center_x, hitbox.center_y, hitbox.center_z);
    if hitbox.use_common_bone_ids {
        return offset;
    }
    pose.get(hitbox.bone as usize)
        .map(|joint| joint.world_matrix.transform_point(offset))
        .unwrap_or(offset)
}

fn sample_hurtboxes_for_frame(
    pose: &[JointPose],
    hurtbox_inits: &[Value],
    procedures: &[Procedure],
    target_frame: u64,
) -> Result<Vec<Value>, String> {
    let mut hurt_states = BTreeMap::new();
    for procedure in procedures {
        if let Procedure::SetHurtState {
            frame,
            bone_idx,
            state,
            ..
        } = procedure
        {
            if *frame <= target_frame {
                hurt_states.insert(*bone_idx, *state);
            }
        }
    }

    let mut sampled = Vec::new();
    for hurtbox in hurtbox_inits {
        let bone_idx = required_u64(hurtbox, &["bone_idx"])?;
        let matrix = pose
            .get(bone_idx as usize)
            .ok_or_else(|| format!("pose joint {} is missing for hurtbox sampling", bone_idx))?
            .world_matrix;
        let source_a =
            matrix.transform_point(vec3_from_value(required_value(hurtbox, &["a_offset_raw"])?));
        let source_b =
            matrix.transform_point(vec3_from_value(required_value(hurtbox, &["b_offset_raw"])?));
        let projected_a = flatten_right_facing(source_a);
        let projected_b = flatten_right_facing(source_b);
        let state_raw = *hurt_states.get(&bone_idx).unwrap_or(&0);
        sampled.push(json!({
            "id": required_u64(hurtbox, &["id"])?,
            "bone": bone_idx,
            "height": required_u64(hurtbox, &["height"])?,
            "is_grabbable": required_bool(hurtbox, &["is_grabbable"])?,
            "radius": required_f32(hurtbox, &["scale_raw"])?,
            "scale": required_f32(hurtbox, &["scale_raw"])?,
            "state": hurt_capsule_state_name(state_raw),
            "state_raw": state_raw,
            "color": "yellow",
            "source_capsule_kind": "segment_capsule",
            "source_a": point_json(source_a),
            "source_b": point_json(source_b),
            "source_a_pos": point_json(source_a),
            "source_b_pos": point_json(source_b),
            "a_pos": point_json(projected_a),
            "b_pos": point_json(projected_b),
            "a": point_json(projected_a),
            "b": point_json(projected_b),
            "a_offset": required_value(hurtbox, &["a_offset_raw"])?,
            "b_offset": required_value(hurtbox, &["b_offset_raw"])?,
            "projected_view_kind": "derived_debug_view",
            "source_render_endpoints": "HurtCapsule.a_pos -> HurtCapsule.b_pos",
            "source_render_radius": "HurtCapsule.scale",
            "source_update_handler": "lbColl_800083C4/lbColl_8000A244/lbColl_8000A584",
        }));
    }
    Ok(sampled)
}

fn procedure_frame(procedure: &Procedure) -> u64 {
    match procedure {
        Procedure::SpawnHitbox(hitbox) => hitbox.frame,
        Procedure::ClearAllHitboxes { frame, .. } => *frame,
        Procedure::SetHurtState { frame, .. } => *frame,
    }
}

fn source_hit_capsule_state_name(state: SourceHitCapsuleState) -> &'static str {
    match state {
        SourceHitCapsuleState::Enabled => "HitCapsule_Enabled",
        SourceHitCapsuleState::Unk2 => "HitCapsule_Unk2",
        SourceHitCapsuleState::Unk3 => "HitCapsule_Unk3",
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

fn point_json(point: Vec3) -> Value {
    json!({
        "x": point.x,
        "y": point.y,
        "z": point.z,
    })
}

fn flatten_right_facing(point: Vec3) -> Vec3 {
    Vec3::new(point.z, point.y, 0.0)
}

fn vec3_from_value(value: &Value) -> Vec3 {
    Vec3::new(
        value.get("x").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        value.get("y").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        value.get("z").and_then(Value::as_f64).unwrap_or(0.0) as f32,
    )
}

fn required_value<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| format!("missing JSON field {}", path.join(".")))?;
    }
    Ok(current)
}

fn required_array<'a>(value: &'a Value, path: &[&str]) -> Result<&'a [Value], String> {
    required_value(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("JSON field {} is not an array", path.join(".")))
}

fn optional_str(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_string)
}

fn required_u64(value: &Value, path: &[&str]) -> Result<u64, String> {
    required_value(value, path)?
        .as_u64()
        .ok_or_else(|| format!("JSON field {} is not a u64", path.join(".")))
}

fn required_f32(value: &Value, path: &[&str]) -> Result<f32, String> {
    required_value(value, path)?
        .as_f64()
        .map(|value| value as f32)
        .ok_or_else(|| format!("JSON field {} is not a number", path.join(".")))
}

fn required_bool(value: &Value, path: &[&str]) -> Result<bool, String> {
    required_value(value, path)?
        .as_bool()
        .ok_or_else(|| format!("JSON field {} is not a bool", path.join(".")))
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

fn parse_hex_u8(text: &str) -> Result<u8, String> {
    u8::from_str_radix(text.trim_start_matches("0x"), 16)
        .map_err(|error| format!("failed to parse hex byte `{text}`: {error}"))
}

fn parse_hex_u32(text: &str) -> Result<u32, String> {
    u32::from_str_radix(text.trim_start_matches("0x"), 16)
        .map_err(|error| format!("failed to parse hex word `{text}`: {error}"))
}
