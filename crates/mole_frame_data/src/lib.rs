use mole_core::collision::{
    Mat3x4, SourceHitboxAttributes, SourceHitboxFlags, SourceHitboxLifecycleId,
    SourceThrowHitboxAttributes, Vec3, SOURCE_HURT_HEIGHT_MID,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDataSampleOptions {
    pub character: String,
    pub source_character: Option<String>,
    pub state: String,
    pub frame: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeFigatreeChunk<'a> {
    pub source_action_key: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeSourceExport<'a> {
    pub character: &'a str,
    pub source_character: Option<&'a str>,
    pub manifest_json: &'a str,
    pub figatree_chunks: &'a [RuntimeFigatreeChunk<'a>],
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RuntimeSourcePoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeSourceCapsuleSample {
    pub id: u64,
    pub a: RuntimeSourcePoint,
    pub b: RuntimeSourcePoint,
    pub radius: f32,
    pub hurt_height: u8,
    pub hitbox_lifecycle_id: Option<SourceHitboxLifecycleId>,
    pub hitbox: Option<SourceHitboxAttributes>,
    pub hitbox_flags: SourceHitboxFlags,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeSourceDownBoundPoseSample {
    pub hip_mtx_0_1: f32,
    pub hip_mtx_0_2: f32,
    pub hip_mtx_1_1: f32,
    pub hip_mtx_1_2: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RuntimeSourceCapturePoseSample {
    pub capture_anchor: RuntimeSourcePoint,
    pub xrotn: RuntimeSourcePoint,
    pub transn2: RuntimeSourcePoint,
    pub x1a70: RuntimeSourcePoint,
    pub thrown_hitbox: RuntimeSourcePoint,
    pub thrown_hitbox_scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSourceFrameSample {
    pub source_frame: u8,
    pub source_root_position: RuntimeSourcePoint,
    pub down_bound_pose: RuntimeSourceDownBoundPoseSample,
    pub capture_pose: RuntimeSourceCapturePoseSample,
    pub hit_capsules: Vec<RuntimeSourceCapsuleSample>,
    pub hurt_capsules: Vec<RuntimeSourceCapsuleSample>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeSourceLivePoseSample {
    pub source_root_position: RuntimeSourcePoint,
    pub down_bound_pose: RuntimeSourceDownBoundPoseSample,
    pub capture_pose: RuntimeSourceCapturePoseSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSourceCmdVarEvent {
    pub source_frame: u8,
    pub cmd_var: u8,
    pub value: u32,
    pub word_offset: u16,
    pub raw_word: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSourceJabComboEvent {
    pub source_frame: u8,
    pub disabled: bool,
    pub word_offset: u16,
    pub raw_word: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSourceJabRapidEvent {
    pub source_frame: u8,
    pub state: bool,
    pub word_offset: u16,
    pub raw_word: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSourceThrowFlagEvent {
    pub source_frame: u8,
    pub hit_idx: u32,
    pub flag_bit: Option<u8>,
    pub word_offset: u16,
    pub raw_word: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSourceThrowHitboxEvent {
    pub source_frame: u8,
    pub hitbox: SourceThrowHitboxAttributes,
    pub word_offset: u16,
    pub raw_words: [u32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSourceScriptEvent {
    SetCmdVar(RuntimeSourceCmdVarEvent),
    SetJabCombo(RuntimeSourceJabComboEvent),
    SetJabRapid(RuntimeSourceJabRapidEvent),
    SetThrowFlag(RuntimeSourceThrowFlagEvent),
    SetThrowHitbox(RuntimeSourceThrowHitboxEvent),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSourceActionFrameSamples {
    pub source_action_key: String,
    pub total_frames: u8,
    pub loops: bool,
    pub frames: Vec<RuntimeSourceFrameSample>,
    pub cmd_var_events: Vec<RuntimeSourceCmdVarEvent>,
    pub script_events: Vec<RuntimeSourceScriptEvent>,
}

const RUNTIME_SOURCE_FRAME_CAPSULES_MAGIC: &[u8; 8] = b"MSFC0013";
const RUNTIME_SOURCE_FRAME_CAPSULES_NO_PLAYBACK_MAGIC: &[u8; 8] = b"MSFC0012";
const RUNTIME_SOURCE_FRAME_CAPSULES_NO_THROW_HITBOX_EVENTS_MAGIC: &[u8; 8] = b"MSFC0011";
const RUNTIME_SOURCE_FRAME_CAPSULES_NO_HITBOX_FLAGS_MAGIC: &[u8; 8] = b"MSFC0010";
const RUNTIME_SOURCE_FRAME_CAPSULES_NO_THROWN_HITBOX_MAGIC: &[u8; 8] = b"MSFC0009";
const RUNTIME_SOURCE_FRAME_CAPSULES_ROOT_POSE_MAGIC: &[u8; 8] = b"MSFC0008";
const RUNTIME_SOURCE_FRAME_CAPSULES_CAPTURE_POSE_V1_MAGIC: &[u8; 8] = b"MSFC0007";
const RUNTIME_SOURCE_FRAME_CAPSULES_NO_CAPTURE_POSE_MAGIC: &[u8; 8] = b"MSFC0006";
const RUNTIME_SOURCE_FRAME_CAPSULES_CMD_VAR_EVENTS_MAGIC: &[u8; 8] = b"MSFC0005";
const RUNTIME_SOURCE_FRAME_CAPSULES_NO_HURT_HEIGHT_MAGIC: &[u8; 8] = b"MSFC0004";
const RUNTIME_SOURCE_FRAME_CAPSULES_F32_NO_EVENTS_MAGIC: &[u8; 8] = b"MSFC0003";
const RUNTIME_SOURCE_FRAME_CAPSULES_F64_MAGIC: &[u8; 8] = b"MSFC0002";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeSourceFloatWidth {
    F32,
    F64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSourceFrameCapsuleFormat {
    float_width: RuntimeSourceFloatWidth,
    script_event_format: RuntimeSourceScriptEventFormat,
    has_hurt_height: bool,
    has_capture_pose: bool,
    has_extended_capture_pose: bool,
    has_source_root_position: bool,
    has_thrown_hitbox_pose: bool,
    has_hitbox_flags: bool,
    has_action_playback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeSourceScriptEventFormat {
    None,
    CmdVarsOnly,
    Generic,
}

pub fn encode_runtime_source_frame_capsules(
    actions: &[RuntimeSourceActionFrameSamples],
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(RUNTIME_SOURCE_FRAME_CAPSULES_MAGIC);
    push_u32(
        &mut bytes,
        checked_u32(actions.len(), "runtime source action count")?,
    );
    for action in actions {
        push_string(&mut bytes, &action.source_action_key)?;
        push_u8(&mut bytes, action.total_frames);
        push_bool(&mut bytes, action.loops);
        push_u16(
            &mut bytes,
            checked_u16(action.frames.len(), "runtime source frame count")?,
        );
        for frame in &action.frames {
            push_u8(&mut bytes, frame.source_frame);
            push_point(&mut bytes, frame.source_root_position);
            push_down_bound_pose(&mut bytes, frame.down_bound_pose);
            push_capture_pose(&mut bytes, frame.capture_pose);
            push_capsules(&mut bytes, &frame.hit_capsules)?;
            push_capsules(&mut bytes, &frame.hurt_capsules)?;
        }
        push_script_events(&mut bytes, &action.script_events)?;
    }
    Ok(bytes)
}

pub fn decode_runtime_source_frame_capsules(
    bytes: &[u8],
) -> Result<Vec<RuntimeSourceActionFrameSamples>, String> {
    let mut reader = RuntimeSourceFrameCapsuleReader::new(bytes);
    let format = reader.read_magic()?;
    let action_count = reader.read_u32()? as usize;
    let mut actions = Vec::with_capacity(action_count);
    for _ in 0..action_count {
        let source_action_key = reader.read_string()?;
        let total_frames = reader.read_u8()?;
        let loops = if format.has_action_playback {
            reader.read_bool()?
        } else {
            false
        };
        let frame_count = reader.read_u16()? as usize;
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let source_frame = reader.read_u8()?;
            let source_root_position = if format.has_source_root_position {
                reader.read_point(format.float_width)?
            } else {
                RuntimeSourcePoint::default()
            };
            let down_bound_pose = reader.read_down_bound_pose(format.float_width)?;
            let capture_pose = if format.has_capture_pose {
                reader.read_capture_pose(
                    format.float_width,
                    format.has_extended_capture_pose,
                    format.has_thrown_hitbox_pose,
                )?
            } else {
                RuntimeSourceCapturePoseSample::default()
            };
            let hit_capsules = reader.read_capsules(
                format.float_width,
                format.has_hurt_height,
                format.has_hitbox_flags,
            )?;
            let hurt_capsules = reader.read_capsules(
                format.float_width,
                format.has_hurt_height,
                format.has_hitbox_flags,
            )?;
            frames.push(RuntimeSourceFrameSample {
                source_frame,
                source_root_position,
                down_bound_pose,
                capture_pose,
                hit_capsules,
                hurt_capsules,
            });
        }
        let (cmd_var_events, script_events) = match format.script_event_format {
            RuntimeSourceScriptEventFormat::Generic => {
                let script_events = reader.read_script_events()?;
                let cmd_var_events = cmd_var_events_from_script_events(&script_events);
                (cmd_var_events, script_events)
            }
            RuntimeSourceScriptEventFormat::CmdVarsOnly => {
                let cmd_var_events = reader.read_cmd_var_events()?;
                let script_events = cmd_var_events
                    .iter()
                    .copied()
                    .map(RuntimeSourceScriptEvent::SetCmdVar)
                    .collect();
                (cmd_var_events, script_events)
            }
            RuntimeSourceScriptEventFormat::None => (Vec::new(), Vec::new()),
        };
        actions.push(RuntimeSourceActionFrameSamples {
            source_action_key,
            total_frames,
            loops,
            frames,
            cmd_var_events,
            script_events,
        });
    }
    reader.finish()?;
    Ok(actions)
}

fn checked_u16(value: usize, field: &str) -> Result<u16, String> {
    u16::try_from(value).map_err(|_| format!("{field} {value} does not fit u16"))
}

fn checked_u32(value: usize, field: &str) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| format!("{field} {value} does not fit u32"))
}

fn push_u8(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

fn push_bool(bytes: &mut Vec<u8>, value: bool) {
    bytes.push(u8::from(value));
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), String> {
    push_u16(
        bytes,
        checked_u16(value.len(), "runtime source action key length")?,
    );
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_point(bytes: &mut Vec<u8>, point: RuntimeSourcePoint) {
    push_f32(bytes, point.x);
    push_f32(bytes, point.y);
    push_f32(bytes, point.z);
}

fn push_down_bound_pose(bytes: &mut Vec<u8>, pose: RuntimeSourceDownBoundPoseSample) {
    push_f32(bytes, pose.hip_mtx_0_1);
    push_f32(bytes, pose.hip_mtx_0_2);
    push_f32(bytes, pose.hip_mtx_1_1);
    push_f32(bytes, pose.hip_mtx_1_2);
}

fn push_capture_pose(bytes: &mut Vec<u8>, pose: RuntimeSourceCapturePoseSample) {
    push_point(bytes, pose.capture_anchor);
    push_point(bytes, pose.xrotn);
    push_point(bytes, pose.transn2);
    push_point(bytes, pose.x1a70);
    push_point(bytes, pose.thrown_hitbox);
    push_f32(bytes, pose.thrown_hitbox_scale);
}

fn push_capsules(
    bytes: &mut Vec<u8>,
    capsules: &[RuntimeSourceCapsuleSample],
) -> Result<(), String> {
    push_u16(
        bytes,
        checked_u16(capsules.len(), "runtime source capsule count")?,
    );
    for capsule in capsules {
        push_u64(bytes, capsule.id);
        push_point(bytes, capsule.a);
        push_point(bytes, capsule.b);
        push_f32(bytes, capsule.radius);
        push_u8(bytes, capsule.hurt_height);
        match capsule.hitbox_lifecycle_id {
            Some(lifecycle_id) => {
                push_u8(bytes, 1);
                push_u64(bytes, lifecycle_id.get());
            }
            None => push_u8(bytes, 0),
        }
        match capsule.hitbox {
            Some(hitbox) => {
                push_u8(bytes, 1);
                push_u16(bytes, hitbox.bone);
                push_u8(bytes, hitbox.hit_group);
                push_u16(bytes, hitbox.damage);
                push_u16(bytes, hitbox.angle);
                push_u16(bytes, hitbox.knockback_growth);
                push_u16(bytes, hitbox.weight_set_knockback);
                push_u16(bytes, hitbox.base_knockback);
                push_u8(bytes, hitbox.element);
                push_i16(bytes, hitbox.shield_damage);
                push_bool(bytes, hitbox.hit_grounded);
                push_bool(bytes, hitbox.hit_aerial);
            }
            None => push_u8(bytes, 0),
        }
        push_u8(bytes, capsule.hitbox_flags.bits());
    }
    Ok(())
}

fn push_script_events(
    bytes: &mut Vec<u8>,
    events: &[RuntimeSourceScriptEvent],
) -> Result<(), String> {
    push_u16(
        bytes,
        checked_u16(events.len(), "runtime source script event count")?,
    );
    for event in events {
        match event {
            RuntimeSourceScriptEvent::SetCmdVar(event) => {
                push_u8(bytes, 1);
                push_u8(bytes, event.source_frame);
                push_u8(bytes, event.cmd_var);
                push_u32(bytes, event.value);
                push_u16(bytes, event.word_offset);
                push_u32(bytes, event.raw_word);
            }
            RuntimeSourceScriptEvent::SetJabCombo(event) => {
                push_u8(bytes, 2);
                push_u8(bytes, event.source_frame);
                push_bool(bytes, event.disabled);
                push_u16(bytes, event.word_offset);
                push_u32(bytes, event.raw_word);
            }
            RuntimeSourceScriptEvent::SetJabRapid(event) => {
                push_u8(bytes, 3);
                push_u8(bytes, event.source_frame);
                push_bool(bytes, event.state);
                push_u16(bytes, event.word_offset);
                push_u32(bytes, event.raw_word);
            }
            RuntimeSourceScriptEvent::SetThrowFlag(event) => {
                push_u8(bytes, 4);
                push_u8(bytes, event.source_frame);
                push_u32(bytes, event.hit_idx);
                push_u8(bytes, event.flag_bit.unwrap_or(u8::MAX));
                push_u16(bytes, event.word_offset);
                push_u32(bytes, event.raw_word);
            }
            RuntimeSourceScriptEvent::SetThrowHitbox(event) => {
                push_u8(bytes, 5);
                push_u8(bytes, event.source_frame);
                push_u8(bytes, event.hitbox.hitbox_idx);
                push_u32(bytes, event.hitbox.damage);
                push_u16(bytes, event.hitbox.angle);
                push_u16(bytes, event.hitbox.hit_x24);
                push_u16(bytes, event.hitbox.hit_x28);
                push_u16(bytes, event.hitbox.hit_x2c);
                push_u8(bytes, event.hitbox.element);
                push_u8(bytes, event.hitbox.sfx_severity);
                push_u8(bytes, event.hitbox.sfx_kind);
                push_u16(bytes, event.word_offset);
                for raw_word in event.raw_words {
                    push_u32(bytes, raw_word);
                }
            }
        }
    }
    Ok(())
}

fn cmd_var_events_from_script_events(
    events: &[RuntimeSourceScriptEvent],
) -> Vec<RuntimeSourceCmdVarEvent> {
    events
        .iter()
        .filter_map(|event| match event {
            RuntimeSourceScriptEvent::SetCmdVar(event) => Some(*event),
            _ => None,
        })
        .collect()
}

struct RuntimeSourceFrameCapsuleReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> RuntimeSourceFrameCapsuleReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_magic(&mut self) -> Result<RuntimeSourceFrameCapsuleFormat, String> {
        let magic = self.read_exact(RUNTIME_SOURCE_FRAME_CAPSULES_MAGIC.len())?;
        if magic == RUNTIME_SOURCE_FRAME_CAPSULES_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: true,
                has_extended_capture_pose: true,
                has_source_root_position: true,
                has_thrown_hitbox_pose: true,
                has_hitbox_flags: true,
                has_action_playback: true,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_NO_PLAYBACK_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: true,
                has_extended_capture_pose: true,
                has_source_root_position: true,
                has_thrown_hitbox_pose: true,
                has_hitbox_flags: true,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_NO_THROW_HITBOX_EVENTS_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: true,
                has_extended_capture_pose: true,
                has_source_root_position: true,
                has_thrown_hitbox_pose: true,
                has_hitbox_flags: true,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_NO_HITBOX_FLAGS_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: true,
                has_extended_capture_pose: true,
                has_source_root_position: true,
                has_thrown_hitbox_pose: true,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_NO_THROWN_HITBOX_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: true,
                has_extended_capture_pose: true,
                has_source_root_position: true,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_ROOT_POSE_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: true,
                has_extended_capture_pose: true,
                has_source_root_position: false,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_CAPTURE_POSE_V1_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: true,
                has_extended_capture_pose: false,
                has_source_root_position: false,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_NO_CAPTURE_POSE_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::Generic,
                has_hurt_height: true,
                has_capture_pose: false,
                has_extended_capture_pose: false,
                has_source_root_position: false,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_CMD_VAR_EVENTS_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::CmdVarsOnly,
                has_hurt_height: true,
                has_capture_pose: false,
                has_extended_capture_pose: false,
                has_source_root_position: false,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_NO_HURT_HEIGHT_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::CmdVarsOnly,
                has_hurt_height: false,
                has_capture_pose: false,
                has_extended_capture_pose: false,
                has_source_root_position: false,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_F32_NO_EVENTS_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F32,
                script_event_format: RuntimeSourceScriptEventFormat::None,
                has_hurt_height: false,
                has_capture_pose: false,
                has_extended_capture_pose: false,
                has_source_root_position: false,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else if magic == RUNTIME_SOURCE_FRAME_CAPSULES_F64_MAGIC {
            Ok(RuntimeSourceFrameCapsuleFormat {
                float_width: RuntimeSourceFloatWidth::F64,
                script_event_format: RuntimeSourceScriptEventFormat::None,
                has_hurt_height: false,
                has_capture_pose: false,
                has_extended_capture_pose: false,
                has_source_root_position: false,
                has_thrown_hitbox_pose: false,
                has_hitbox_flags: false,
                has_action_playback: false,
            })
        } else {
            Err("runtime source frame capsule sidecar has an unsupported format".to_string())
        }
    }

    fn finish(&self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "runtime source frame capsule sidecar has {} trailing bytes",
                self.bytes.len() - self.offset
            ))
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "runtime source frame capsule offset overflow".to_string())?;
        let Some(slice) = self.bytes.get(self.offset..end) else {
            return Err("runtime source frame capsule sidecar ended early".to_string());
        };
        self.offset = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_bool(&mut self) -> Result<bool, String> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!(
                "runtime source frame capsule sidecar bool value {value} is invalid"
            )),
        }
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(self.read_exact(2)?);
        Ok(u16::from_le_bytes(buf))
    }

    fn read_i16(&mut self) -> Result<i16, String> {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(self.read_exact(2)?);
        Ok(i16::from_le_bytes(buf))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(self.read_exact(4)?);
        Ok(u32::from_le_bytes(buf))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(self.read_exact(8)?);
        Ok(u64::from_le_bytes(buf))
    }

    fn read_f32(&mut self) -> Result<f32, String> {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(self.read_exact(4)?);
        Ok(f32::from_le_bytes(buf))
    }

    fn read_string(&mut self) -> Result<String, String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_exact(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_string)
            .map_err(|error| format!("runtime source action key is not UTF-8: {error}"))
    }

    fn read_source_float(&mut self, float_width: RuntimeSourceFloatWidth) -> Result<f32, String> {
        match float_width {
            RuntimeSourceFloatWidth::F32 => self.read_f32(),
            RuntimeSourceFloatWidth::F64 => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(self.read_exact(8)?);
                Ok(f64::from_le_bytes(buf) as f32)
            }
        }
    }

    fn read_point(
        &mut self,
        float_width: RuntimeSourceFloatWidth,
    ) -> Result<RuntimeSourcePoint, String> {
        Ok(RuntimeSourcePoint {
            x: self.read_source_float(float_width)?,
            y: self.read_source_float(float_width)?,
            z: self.read_source_float(float_width)?,
        })
    }

    fn read_down_bound_pose(
        &mut self,
        float_width: RuntimeSourceFloatWidth,
    ) -> Result<RuntimeSourceDownBoundPoseSample, String> {
        Ok(RuntimeSourceDownBoundPoseSample {
            hip_mtx_0_1: self.read_source_float(float_width)?,
            hip_mtx_0_2: self.read_source_float(float_width)?,
            hip_mtx_1_1: self.read_source_float(float_width)?,
            hip_mtx_1_2: self.read_source_float(float_width)?,
        })
    }

    fn read_capture_pose(
        &mut self,
        float_width: RuntimeSourceFloatWidth,
        has_extended_capture_pose: bool,
        has_thrown_hitbox_pose: bool,
    ) -> Result<RuntimeSourceCapturePoseSample, String> {
        let capture_anchor = self.read_point(float_width)?;
        let xrotn = self.read_point(float_width)?;
        let (transn2, x1a70) = if has_extended_capture_pose {
            (self.read_point(float_width)?, self.read_point(float_width)?)
        } else {
            (RuntimeSourcePoint::default(), RuntimeSourcePoint::default())
        };
        let (thrown_hitbox, thrown_hitbox_scale) = if has_thrown_hitbox_pose {
            (
                self.read_point(float_width)?,
                self.read_source_float(float_width)?,
            )
        } else {
            (RuntimeSourcePoint::default(), 0.0)
        };
        Ok(RuntimeSourceCapturePoseSample {
            capture_anchor,
            xrotn,
            transn2,
            x1a70,
            thrown_hitbox,
            thrown_hitbox_scale,
        })
    }

    fn read_capsules(
        &mut self,
        float_width: RuntimeSourceFloatWidth,
        has_hurt_height: bool,
        has_hitbox_flags: bool,
    ) -> Result<Vec<RuntimeSourceCapsuleSample>, String> {
        let capsule_count = self.read_u16()? as usize;
        let mut capsules = Vec::with_capacity(capsule_count);
        for _ in 0..capsule_count {
            let id = self.read_u64()?;
            let a = self.read_point(float_width)?;
            let b = self.read_point(float_width)?;
            let radius = self.read_source_float(float_width)?;
            let hurt_height = if has_hurt_height {
                self.read_u8()?
            } else {
                SOURCE_HURT_HEIGHT_MID
            };
            let hitbox_lifecycle_id = match self.read_u8()? {
                0 => None,
                1 => Some(SourceHitboxLifecycleId::new(self.read_u64()?)),
                value => {
                    return Err(format!(
                        "runtime source frame capsule lifecycle tag {value} is invalid"
                    ))
                }
            };
            let hitbox = match self.read_u8()? {
                0 => None,
                1 => Some(SourceHitboxAttributes {
                    bone: self.read_u16()?,
                    hit_group: self.read_u8()?,
                    damage: self.read_u16()?,
                    angle: self.read_u16()?,
                    knockback_growth: self.read_u16()?,
                    weight_set_knockback: self.read_u16()?,
                    base_knockback: self.read_u16()?,
                    element: self.read_u8()?,
                    shield_damage: self.read_i16()?,
                    hit_grounded: self.read_bool()?,
                    hit_aerial: self.read_bool()?,
                }),
                value => {
                    return Err(format!(
                        "runtime source frame capsule hitbox tag {value} is invalid"
                    ))
                }
            };
            let hitbox_flags = if has_hitbox_flags {
                SourceHitboxFlags::from_bits_truncate(self.read_u8()?)
            } else {
                SourceHitboxFlags::none()
            };
            capsules.push(RuntimeSourceCapsuleSample {
                id,
                a,
                b,
                radius,
                hurt_height,
                hitbox_lifecycle_id,
                hitbox,
                hitbox_flags,
            });
        }
        Ok(capsules)
    }

    fn read_cmd_var_events(&mut self) -> Result<Vec<RuntimeSourceCmdVarEvent>, String> {
        let event_count = self.read_u16()? as usize;
        let mut events = Vec::with_capacity(event_count);
        for _ in 0..event_count {
            events.push(RuntimeSourceCmdVarEvent {
                source_frame: self.read_u8()?,
                cmd_var: self.read_u8()?,
                value: self.read_u32()?,
                word_offset: self.read_u16()?,
                raw_word: self.read_u32()?,
            });
        }
        Ok(events)
    }

    fn read_script_events(&mut self) -> Result<Vec<RuntimeSourceScriptEvent>, String> {
        let event_count = self.read_u16()? as usize;
        let mut events = Vec::with_capacity(event_count);
        for _ in 0..event_count {
            let tag = self.read_u8()?;
            let event = match tag {
                1 => RuntimeSourceScriptEvent::SetCmdVar(RuntimeSourceCmdVarEvent {
                    source_frame: self.read_u8()?,
                    cmd_var: self.read_u8()?,
                    value: self.read_u32()?,
                    word_offset: self.read_u16()?,
                    raw_word: self.read_u32()?,
                }),
                2 => RuntimeSourceScriptEvent::SetJabCombo(RuntimeSourceJabComboEvent {
                    source_frame: self.read_u8()?,
                    disabled: self.read_bool()?,
                    word_offset: self.read_u16()?,
                    raw_word: self.read_u32()?,
                }),
                3 => RuntimeSourceScriptEvent::SetJabRapid(RuntimeSourceJabRapidEvent {
                    source_frame: self.read_u8()?,
                    state: self.read_bool()?,
                    word_offset: self.read_u16()?,
                    raw_word: self.read_u32()?,
                }),
                4 => {
                    let source_frame = self.read_u8()?;
                    let hit_idx = self.read_u32()?;
                    let flag_bit = match self.read_u8()? {
                        u8::MAX => None,
                        value => Some(value),
                    };
                    RuntimeSourceScriptEvent::SetThrowFlag(RuntimeSourceThrowFlagEvent {
                        source_frame,
                        hit_idx,
                        flag_bit,
                        word_offset: self.read_u16()?,
                        raw_word: self.read_u32()?,
                    })
                }
                5 => {
                    let source_frame = self.read_u8()?;
                    let hitbox_idx = self.read_u8()?;
                    let damage = self.read_u32()?;
                    let angle = self.read_u16()?;
                    let hit_x24 = self.read_u16()?;
                    let hit_x28 = self.read_u16()?;
                    let hit_x2c = self.read_u16()?;
                    let element = self.read_u8()?;
                    let sfx_severity = self.read_u8()?;
                    let sfx_kind = self.read_u8()?;
                    let word_offset = self.read_u16()?;
                    RuntimeSourceScriptEvent::SetThrowHitbox(RuntimeSourceThrowHitboxEvent {
                        source_frame,
                        hitbox: SourceThrowHitboxAttributes {
                            hitbox_idx,
                            damage,
                            angle,
                            hit_x24,
                            hit_x28,
                            hit_x2c,
                            element,
                            sfx_severity,
                            sfx_kind,
                        },
                        word_offset,
                        raw_words: [self.read_u32()?, self.read_u32()?, self.read_u32()?],
                    })
                }
                value => {
                    return Err(format!(
                        "runtime source frame capsule script event tag {value} is invalid"
                    ))
                }
            };
            events.push(event);
        }
        Ok(events)
    }
}

pub struct RuntimeSourceExportEvaluator<'a> {
    export: RuntimeSourceExport<'a>,
    manifest: Value,
}

impl<'a> RuntimeSourceExportEvaluator<'a> {
    pub fn from_export(export: RuntimeSourceExport<'a>) -> Result<Self, String> {
        let manifest: Value = serde_json::from_str(export.manifest_json)
            .map_err(|error| format!("failed to parse runtime source export manifest: {error}"))?;
        Ok(Self { export, manifest })
    }

    pub fn action_frame_evaluator(
        &self,
        options: &FrameDataSampleOptions,
    ) -> Result<RuntimeActionFrameEvaluator, String> {
        Ok(RuntimeActionFrameEvaluator {
            source: load_action_sample_source_from_export_manifest(
                &self.export,
                &self.manifest,
                options,
            )?,
        })
    }
}

#[derive(Clone)]
pub struct RuntimeActionFrameEvaluator {
    source: ActionSampleSource,
}

impl RuntimeActionFrameEvaluator {
    pub fn from_export(
        export: &RuntimeSourceExport<'_>,
        options: &FrameDataSampleOptions,
    ) -> Result<Self, String> {
        Ok(Self {
            source: load_action_sample_source_from_export(export, options)?,
        })
    }

    pub fn source_action_key(&self) -> &str {
        &self.source.source_action_key
    }

    pub fn total_frames(&self) -> u64 {
        self.source.total_frames
    }

    pub fn sample_frame(&self, frame: u64) -> Result<Value, String> {
        self.source.sample_frame_json(frame)
    }

    pub fn sample_frame_capsules(&self, frame: u64) -> Result<RuntimeSourceFrameSample, String> {
        self.source.sample_frame_capsules(frame)
    }

    pub fn sample_live_pose(&self, anim_frame: f32) -> Result<RuntimeSourceLivePoseSample, String> {
        self.source.sample_live_pose(anim_frame)
    }

    pub fn cmd_var_events(&self) -> Result<Vec<RuntimeSourceCmdVarEvent>, String> {
        self.source.cmd_var_events()
    }

    pub fn script_events(&self) -> Result<Vec<RuntimeSourceScriptEvent>, String> {
        self.source.script_events()
    }
}

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

pub fn sample_state_frame(root: &Path, options: &FrameDataSampleOptions) -> Result<Value, String> {
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

pub fn sample_action_keyframes(
    root: &Path,
    options: &FrameDataSampleOptions,
) -> Result<Vec<Value>, String> {
    let source = load_action_sample_source(root, options)?;
    let mut poses = Vec::new();
    for frame in 1..=source.total_frames {
        poses.push((frame, source.sample_pose(frame)?));
    }
    let hitboxes_by_frame = sample_hitboxes_by_frame(&poses, &source.procedures);
    let mut keyframes = Vec::new();
    let mut previous_transn = Vec3::new(0.0, 0.0, 0.0);
    for (frame, pose) in &poses {
        let hitboxes = hitboxes_by_frame.get(frame).cloned().unwrap_or_default();
        let hurtboxes =
            sample_hurtboxes_for_frame(pose, &source.hurtbox_inits, &source.procedures, *frame)?;
        let source_root_motion = source_root_motion_json(pose, previous_transn)?;
        let source_capture_pose = source_capture_pose_json(pose, source.common_parts)?;
        previous_transn = transn_translation(pose)?;
        keyframes.push(source.sample_json(
            *frame,
            pose.len(),
            hitboxes,
            hurtboxes,
            source_root_motion,
            source_capture_pose,
        ));
    }
    Ok(keyframes)
}

pub fn sample_state_frame_from_export(
    export: &RuntimeSourceExport<'_>,
    options: &FrameDataSampleOptions,
) -> Result<Value, String> {
    RuntimeActionFrameEvaluator::from_export(export, options)?.sample_frame(options.frame)
}

pub fn sample_action_keyframes_from_export(
    export: &RuntimeSourceExport<'_>,
    options: &FrameDataSampleOptions,
) -> Result<Vec<Value>, String> {
    let source = load_action_sample_source_from_export(export, options)?;
    let mut poses = Vec::new();
    for frame in 1..=source.total_frames {
        poses.push((frame, source.sample_pose(frame)?));
    }
    let hitboxes_by_frame = sample_hitboxes_by_frame(&poses, &source.procedures);
    let mut keyframes = Vec::new();
    let mut previous_transn = Vec3::new(0.0, 0.0, 0.0);
    for (frame, pose) in &poses {
        let hitboxes = hitboxes_by_frame.get(frame).cloned().unwrap_or_default();
        let hurtboxes =
            sample_hurtboxes_for_frame(pose, &source.hurtbox_inits, &source.procedures, *frame)?;
        let source_root_motion = source_root_motion_json(pose, previous_transn)?;
        let source_capture_pose = source_capture_pose_json(pose, source.common_parts)?;
        previous_transn = transn_translation(pose)?;
        keyframes.push(source.sample_json(
            *frame,
            pose.len(),
            hitboxes,
            hurtboxes,
            source_root_motion,
            source_capture_pose,
        ));
    }
    Ok(keyframes)
}

#[derive(Clone)]
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
    common_parts: SourceCommonParts,
    pose_setup: SourcePoseSetup,
    procedures: Vec<Procedure>,
}

impl ActionSampleSource {
    fn sample_pose(&self, frame: u64) -> Result<Vec<JointPose>, String> {
        sample_pose(
            &self.figatree_chunk,
            &self.figatree,
            &self.skeleton,
            frame,
            self.pose_setup,
        )
    }

    fn sample_pose_at_anim_frame(&self, anim_frame: f32) -> Result<Vec<JointPose>, String> {
        sample_pose_at_anim_frame(
            &self.figatree_chunk,
            &self.figatree,
            &self.skeleton,
            anim_frame,
            self.pose_setup,
        )
    }

    fn sample_json(
        &self,
        frame: u64,
        pose_joint_count: usize,
        hitboxes: Vec<Value>,
        hurtboxes: Vec<Value>,
        source_root_motion: Value,
        source_capture_pose: Value,
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
            "source_capture_pose": source_capture_pose,
            "hit_capsules": hitboxes,
            "hurt_capsules": hurtboxes,
        })
    }

    fn sample_frame_json(&self, frame: u64) -> Result<Value, String> {
        if frame == 0 || frame > self.total_frames {
            return Err(format!(
                "runtime source export action `{}` has no sampled frame {}",
                self.source_action_key, frame
            ));
        }
        let pose = self.sample_pose(frame)?;
        let previous_pose = if frame > 1 {
            Some(self.sample_pose(frame - 1)?)
        } else {
            None
        };
        let previous_transn = previous_pose
            .as_deref()
            .map(transn_translation)
            .transpose()?
            .unwrap_or(Vec3::new(0.0, 0.0, 0.0));
        let hitboxes =
            sample_hitboxes_for_frame(&pose, previous_pose.as_deref(), &self.procedures, frame);
        let hurtboxes =
            sample_hurtboxes_for_frame(&pose, &self.hurtbox_inits, &self.procedures, frame)?;
        let source_root_motion = source_root_motion_json(&pose, previous_transn)?;
        let source_capture_pose = source_capture_pose_json(&pose, self.common_parts)?;

        Ok(self.sample_json(
            frame,
            pose.len(),
            hitboxes,
            hurtboxes,
            source_root_motion,
            source_capture_pose,
        ))
    }

    fn sample_frame_capsules(&self, frame: u64) -> Result<RuntimeSourceFrameSample, String> {
        if frame == 0 || frame > self.total_frames {
            return Err(format!(
                "runtime source export action `{}` has no sampled frame {}",
                self.source_action_key, frame
            ));
        }
        let pose = self.sample_pose(frame)?;
        let previous_pose = if frame > 1 {
            Some(self.sample_pose(frame - 1)?)
        } else {
            None
        };
        let source_frame = u8::try_from(frame).map_err(|_| {
            format!(
                "runtime source export action `{}` frame {} does not fit u8",
                self.source_action_key, frame
            )
        })?;
        Ok(RuntimeSourceFrameSample {
            source_frame,
            source_root_position: source_root_position(&pose)?,
            down_bound_pose: source_down_bound_pose(&pose, self.common_parts)?,
            capture_pose: source_capture_pose(&pose, self.common_parts)?,
            hit_capsules: sample_hitbox_capsules_for_frame(
                &pose,
                previous_pose.as_deref(),
                &self.procedures,
                frame,
            ),
            hurt_capsules: sample_hurtbox_capsules_for_frame(
                &pose,
                &self.hurtbox_inits,
                &self.procedures,
                frame,
            )?,
        })
    }

    fn sample_live_pose(&self, anim_frame: f32) -> Result<RuntimeSourceLivePoseSample, String> {
        let pose = self.sample_pose_at_anim_frame(anim_frame)?;
        Ok(RuntimeSourceLivePoseSample {
            source_root_position: source_root_position(&pose)?,
            down_bound_pose: source_down_bound_pose(&pose, self.common_parts)?,
            capture_pose: source_capture_pose(&pose, self.common_parts)?,
        })
    }

    fn cmd_var_events(&self) -> Result<Vec<RuntimeSourceCmdVarEvent>, String> {
        Ok(cmd_var_events_from_script_events(&self.script_events()?))
    }

    fn script_events(&self) -> Result<Vec<RuntimeSourceScriptEvent>, String> {
        self.procedures
            .iter()
            .filter_map(|procedure| match procedure {
                Procedure::SetCmdVar {
                    frame,
                    cmd_var,
                    value,
                    word_offset,
                    raw_word,
                } => Some(source_script_event_from_parts(
                    self.source_action_key.as_str(),
                    *frame,
                    *word_offset,
                    *raw_word,
                    SourceScriptEventParts::CmdVar {
                        cmd_var: *cmd_var,
                        value: *value,
                    },
                )),
                Procedure::SetJabCombo {
                    frame,
                    disabled,
                    word_offset,
                    raw_word,
                } => Some(source_script_event_from_parts(
                    self.source_action_key.as_str(),
                    *frame,
                    *word_offset,
                    *raw_word,
                    SourceScriptEventParts::JabCombo {
                        disabled: *disabled,
                    },
                )),
                Procedure::SetJabRapid {
                    frame,
                    state,
                    word_offset,
                    raw_word,
                } => Some(source_script_event_from_parts(
                    self.source_action_key.as_str(),
                    *frame,
                    *word_offset,
                    *raw_word,
                    SourceScriptEventParts::JabRapid { state: *state },
                )),
                Procedure::SetThrowFlag {
                    frame,
                    hit_idx,
                    flag_bit,
                    word_offset,
                    raw_word,
                } => Some(source_script_event_from_parts(
                    self.source_action_key.as_str(),
                    *frame,
                    *word_offset,
                    *raw_word,
                    SourceScriptEventParts::ThrowFlag {
                        hit_idx: *hit_idx,
                        flag_bit: *flag_bit,
                    },
                )),
                Procedure::SetThrowHitbox(throw_hitbox) => Some(source_script_event_from_parts(
                    self.source_action_key.as_str(),
                    throw_hitbox.frame,
                    throw_hitbox.word_offset,
                    throw_hitbox.raw_words[0],
                    SourceScriptEventParts::ThrowHitbox {
                        hitbox: throw_hitbox.hitbox,
                        raw_words: throw_hitbox.raw_words,
                    },
                )),
                _ => None,
            })
            .collect()
    }
}

enum SourceScriptEventParts {
    CmdVar {
        cmd_var: u8,
        value: u32,
    },
    JabCombo {
        disabled: bool,
    },
    JabRapid {
        state: bool,
    },
    ThrowFlag {
        hit_idx: u32,
        flag_bit: Option<u8>,
    },
    ThrowHitbox {
        hitbox: SourceThrowHitboxAttributes,
        raw_words: [u32; 3],
    },
}

fn source_script_event_from_parts(
    source_action_key: &str,
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    parts: SourceScriptEventParts,
) -> Result<RuntimeSourceScriptEvent, String> {
    let source_frame = u8::try_from(frame).map_err(|_| {
        format!("runtime source export action `{source_action_key}` script frame {frame} does not fit u8")
    })?;
    let word_offset = u16::try_from(word_offset).map_err(|_| {
        format!(
            "runtime source export action `{source_action_key}` script word offset {word_offset} does not fit u16"
        )
    })?;
    Ok(match parts {
        SourceScriptEventParts::CmdVar { cmd_var, value } => {
            RuntimeSourceScriptEvent::SetCmdVar(RuntimeSourceCmdVarEvent {
                source_frame,
                cmd_var,
                value,
                word_offset,
                raw_word,
            })
        }
        SourceScriptEventParts::JabCombo { disabled } => {
            RuntimeSourceScriptEvent::SetJabCombo(RuntimeSourceJabComboEvent {
                source_frame,
                disabled,
                word_offset,
                raw_word,
            })
        }
        SourceScriptEventParts::JabRapid { state } => {
            RuntimeSourceScriptEvent::SetJabRapid(RuntimeSourceJabRapidEvent {
                source_frame,
                state,
                word_offset,
                raw_word,
            })
        }
        SourceScriptEventParts::ThrowFlag { hit_idx, flag_bit } => {
            RuntimeSourceScriptEvent::SetThrowFlag(RuntimeSourceThrowFlagEvent {
                source_frame,
                hit_idx,
                flag_bit,
                word_offset,
                raw_word,
            })
        }
        SourceScriptEventParts::ThrowHitbox { hitbox, raw_words } => {
            RuntimeSourceScriptEvent::SetThrowHitbox(RuntimeSourceThrowHitboxEvent {
                source_frame,
                hitbox,
                word_offset,
                raw_words,
            })
        }
    })
}

#[derive(Clone, Copy)]
struct SourceCommonParts {
    transn_joint: usize,
    xrotn_joint: usize,
    hipn_joint: usize,
    capture_anchor_part: usize,
    transn2_joint: usize,
    thrown_hitbox_joint: usize,
    thrown_hitbox_scale: f32,
}

#[derive(Clone, Copy)]
struct SourcePoseSetup {
    topn_rot_y: f32,
    topn_scale: f32,
}

impl SourcePoseSetup {
    #[cfg(test)]
    fn raw_joint_data() -> Self {
        Self {
            topn_rot_y: 0.0,
            topn_scale: 1.0,
        }
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
    let common_parts = source_common_parts_from_manifest(&manifest)?;
    let pose_setup = source_pose_setup_from_manifest(&manifest)?;
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
        common_parts,
        pose_setup,
        procedures,
    })
}

fn load_action_sample_source_from_export(
    export: &RuntimeSourceExport<'_>,
    options: &FrameDataSampleOptions,
) -> Result<ActionSampleSource, String> {
    let manifest: Value = serde_json::from_str(export.manifest_json)
        .map_err(|error| format!("failed to parse runtime source export manifest: {error}"))?;
    load_action_sample_source_from_export_manifest(export, &manifest, options)
}

fn load_action_sample_source_from_export_manifest(
    export: &RuntimeSourceExport<'_>,
    manifest: &Value,
    options: &FrameDataSampleOptions,
) -> Result<ActionSampleSource, String> {
    if options.character != export.character {
        return Err(format!(
            "runtime source export character mismatch: requested `{}`, export has `{}`",
            options.character, export.character
        ));
    }

    let manifest_source_character = optional_str(&manifest, &["source_character"])
        .or_else(|| export.source_character.map(str::to_string))
        .unwrap_or_default();
    if let Some(requested_source_character) = &options.source_character {
        if requested_source_character != &manifest_source_character {
            return Err(format!(
                "runtime source export source character mismatch: requested `{requested_source_character}`, export has `{manifest_source_character}`"
            ));
        }
    }

    let action = manifest_action(manifest, &options.state)?;
    let projection = manifest
        .get("projection")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let action_state_id = required_u64(&action, &["action_state_id"])?;
    let total_frames = required_u64(action, &["total_frames"])?;
    let figatree = required_value(&action, &["source_action", "figatree"])?;
    let skeleton = required_array(manifest, &["rig", "skeleton", "data", "joints"])?;
    let hurtbox_inits = required_array(manifest, &["rig", "hurtbox_inits", "data", "hurtboxes"])?;
    let common_parts = source_common_parts_from_manifest(manifest)?;
    let pose_setup = source_pose_setup_from_manifest(manifest)?;
    let source_action_key =
        optional_str(&action, &["source_action_key"]).unwrap_or_else(|| options.state.clone());
    let figatree_chunk = export
        .figatree_chunks
        .iter()
        .find(|chunk| chunk.source_action_key == source_action_key)
        .map(|chunk| chunk.bytes.to_vec())
        .ok_or_else(|| {
            format!(
                "runtime source export action `{source_action_key}` is missing its baked FigaTree chunk"
            )
        })?;
    let procedures = decode_procedures(&action)?;

    Ok(ActionSampleSource {
        character: options.character.clone(),
        manifest_source_character,
        state: optional_str(&action, &["state"]).unwrap_or_else(|| options.state.clone()),
        source_action_key,
        source_action_name: optional_str(&action, &["source_action_name"]).unwrap_or_default(),
        action_state_id,
        total_frames,
        projection,
        figatree_chunk,
        figatree: figatree.clone(),
        skeleton: skeleton.to_vec(),
        hurtbox_inits: hurtbox_inits.to_vec(),
        common_parts,
        pose_setup,
        procedures,
    })
}

fn source_common_parts_from_manifest(manifest: &Value) -> Result<SourceCommonParts, String> {
    Ok(SourceCommonParts {
        transn_joint: required_u64(manifest, &["rig", "common_parts", "data", "transn_joint"])?
            as usize,
        xrotn_joint: required_u64(manifest, &["rig", "common_parts", "data", "xrotn_joint"])?
            as usize,
        hipn_joint: required_u64(manifest, &["rig", "common_parts", "data", "hipn_joint"])?
            as usize,
        capture_anchor_part: required_u64(
            manifest,
            &["rig", "common_parts", "data", "capture_anchor_part"],
        )? as usize,
        transn2_joint: required_u64(manifest, &["rig", "common_parts", "data", "transn2_joint"])?
            as usize,
        thrown_hitbox_joint: required_u64(
            manifest,
            &["rig", "common_parts", "data", "thrown_hitbox_joint"],
        )? as usize,
        thrown_hitbox_scale: required_f32(
            manifest,
            &["rig", "common_parts", "data", "thrown_hitbox_scale"],
        )?,
    })
}

fn source_pose_setup_from_manifest(manifest: &Value) -> Result<SourcePoseSetup, String> {
    Ok(SourcePoseSetup {
        topn_rot_y: required_f32(manifest, &["rig", "live_pose_setup", "topn_rot_y_radians"])?,
        topn_scale: required_f32(manifest, &["rig", "live_pose_setup", "topn_scale"])?,
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
    pose_setup: SourcePoseSetup,
) -> Result<Vec<JointPose>, String> {
    let frame = frame_number.saturating_sub(1) as f32;
    sample_pose_at_anim_frame(figatree_chunk, figatree, skeleton, frame, pose_setup)
}

fn sample_pose_at_anim_frame(
    figatree_chunk: &[u8],
    figatree: &Value,
    skeleton: &[Value],
    frame: f32,
    pose_setup: SourcePoseSetup,
) -> Result<Vec<JointPose>, String> {
    if !frame.is_finite() {
        return Err(format!("source animation frame {frame} is not finite"));
    }
    let track_counts = required_array(figatree, &["track_counts_by_node"])?;
    let tracks = required_array(figatree, &["tracks"])?;

    let mut matrices = Vec::with_capacity(skeleton.len());
    let mut local_translations = Vec::with_capacity(skeleton.len());
    let mut global_scales: Vec<Option<Vec3>> = Vec::with_capacity(skeleton.len());

    for (index, joint) in skeleton.iter().enumerate() {
        let mut rotation = vec3_from_value(required_value(joint, &["rotation_raw"])?);
        let mut scale = vec3_from_value(required_value(joint, &["scale_raw"])?);
        let mut translation = vec3_from_value(required_value(joint, &["position_raw"])?);
        if index == 0 {
            rotation.y = pose_setup.topn_rot_y;
            scale.x = pose_setup.topn_scale;
            scale.y = pose_setup.topn_scale;
            scale.z = pose_setup.topn_scale;
        }

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

fn source_down_bound_pose(
    pose: &[JointPose],
    common_parts: SourceCommonParts,
) -> Result<RuntimeSourceDownBoundPoseSample, String> {
    // ftCo_80097570 samples fp->parts[ftParts_GetBoneIndex(fp, FtPart_HipN)].
    let hip = pose.get(common_parts.hipn_joint).ok_or_else(|| {
        format!(
            "sampled pose is missing FtPart_HipN joint {}",
            common_parts.hipn_joint
        )
    })?;
    Ok(RuntimeSourceDownBoundPoseSample {
        hip_mtx_0_1: hip.world_matrix.rows[0][1],
        hip_mtx_0_2: hip.world_matrix.rows[0][2],
        hip_mtx_1_1: hip.world_matrix.rows[1][1],
        hip_mtx_1_2: hip.world_matrix.rows[1][2],
    })
}

fn source_capture_pose(
    pose: &[JointPose],
    common_parts: SourceCommonParts,
) -> Result<RuntimeSourceCapturePoseSample, String> {
    Ok(RuntimeSourceCapturePoseSample {
        capture_anchor: source_capture_world_point(
            pose,
            common_parts.capture_anchor_part,
            "ftData.x8.x11 capture anchor",
        )?,
        xrotn: source_capture_world_point(pose, common_parts.xrotn_joint, "FtPart_XRotN")?,
        transn2: source_capture_world_point(pose, common_parts.transn2_joint, "FtPart_TransN2")?,
        x1a70: source_x1a70_point(pose, common_parts)?,
        thrown_hitbox: source_capture_world_point(
            pose,
            common_parts.thrown_hitbox_joint,
            "ftData.x34 thrown hitbox",
        )?,
        thrown_hitbox_scale: common_parts.thrown_hitbox_scale,
    })
}

fn source_capture_pose_json(
    pose: &[JointPose],
    common_parts: SourceCommonParts,
) -> Result<Value, String> {
    let pose = source_capture_pose(pose, common_parts)?;
    Ok(json!({
        "source": "live lb_8000B1CC joint world samples after HSD_JObjSetupMatrix",
        "capture_anchor": runtime_source_point_json(pose.capture_anchor),
        "xrotn": runtime_source_point_json(pose.xrotn),
        "transn2": runtime_source_point_json(pose.transn2),
        "x1a70": runtime_source_point_json(pose.x1a70),
        "thrown_hitbox": runtime_source_point_json(pose.thrown_hitbox),
        "thrown_hitbox_scale": pose.thrown_hitbox_scale,
    }))
}

fn runtime_source_point_json(point: RuntimeSourcePoint) -> Value {
    json!({
        "x": point.x,
        "y": point.y,
        "z": point.z,
    })
}

fn source_capture_world_point(
    pose: &[JointPose],
    part_index: usize,
    source_name: &str,
) -> Result<RuntimeSourcePoint, String> {
    let joint = pose
        .get(part_index)
        .ok_or_else(|| format!("sampled pose is missing {source_name} joint {part_index}"))?;
    Ok(runtime_source_point(Vec3::new(
        joint.world_matrix.rows[0][3],
        joint.world_matrix.rows[1][3],
        joint.world_matrix.rows[2][3],
    )))
}

fn source_x1a70_point(
    pose: &[JointPose],
    common_parts: SourceCommonParts,
) -> Result<RuntimeSourcePoint, String> {
    let transn = pose.get(common_parts.transn_joint).ok_or_else(|| {
        format!(
            "sampled pose is missing FtPart_TransN joint {}",
            common_parts.transn_joint
        )
    })?;
    let xrotn = pose.get(common_parts.xrotn_joint).ok_or_else(|| {
        format!(
            "sampled pose is missing FtPart_XRotN joint {}",
            common_parts.xrotn_joint
        )
    })?;
    Ok(runtime_source_point(Vec3::new(
        transn.world_matrix.rows[0][3] - xrotn.world_matrix.rows[0][3],
        transn.world_matrix.rows[1][3] - xrotn.world_matrix.rows[1][3],
        transn.world_matrix.rows[2][3] - xrotn.world_matrix.rows[2][3],
    )))
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

fn source_root_position(pose: &[JointPose]) -> Result<RuntimeSourcePoint, String> {
    Ok(runtime_source_point(transn_translation(pose)?))
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
        let declared_end = start.saturating_add(length);
        if declared_end > figatree_chunk.len() {
            return Err(format!(
                "FigaTree data slice is out of bounds for action `{action_name}`, node {node_index}, track {}",
                local_index
            ));
        }
        let value = sample_fobj_value(
            &figatree_chunk[start..],
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
    let data = animation_data;
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
            if pos >= length {
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
            if pos >= length {
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
    hitbox_flags: SourceHitboxFlags,
}

#[derive(Clone)]
struct DecodedThrowHitbox {
    frame: u64,
    word_offset: usize,
    raw_words: [u32; 3],
    hitbox: SourceThrowHitboxAttributes,
}

#[derive(Clone)]
enum Procedure {
    SpawnHitbox(DecodedHitbox),
    ClearAllHitboxes {
        frame: u64,
        word_offset: usize,
    },
    SetCmdVar {
        frame: u64,
        cmd_var: u8,
        value: u32,
        word_offset: usize,
        raw_word: u32,
    },
    SetJabCombo {
        frame: u64,
        disabled: bool,
        word_offset: usize,
        raw_word: u32,
    },
    SetJabRapid {
        frame: u64,
        state: bool,
        word_offset: usize,
        raw_word: u32,
    },
    SetThrowFlag {
        frame: u64,
        hit_idx: u32,
        flag_bit: Option<u8>,
        word_offset: usize,
        raw_word: u32,
    },
    SetThrowHitbox(DecodedThrowHitbox),
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
            "fighter.set_cmd_var" => {
                let raw_words = required_array(procedure, &["raw_words"])?;
                let raw_word = raw_words
                    .first()
                    .and_then(Value::as_str)
                    .ok_or_else(|| "set_cmd_var raw_words[0] must be a string".to_string())?;
                let cmd_var = required_u64(procedure, &["cmd_var"])?;
                let value = required_u64(procedure, &["value"])?;
                decoded.push(Procedure::SetCmdVar {
                    frame: required_u64(procedure, &["frame"])?,
                    cmd_var: u8::try_from(cmd_var)
                        .map_err(|_| format!("set_cmd_var cmd_var {cmd_var} does not fit u8"))?,
                    value: u32::try_from(value)
                        .map_err(|_| format!("set_cmd_var value {value} does not fit u32"))?,
                    word_offset: required_u64(procedure, &["word_offset"])? as usize,
                    raw_word: parse_hex_u32(raw_word)?,
                });
            }
            "fighter.set_jab_combo" => {
                let raw_word = first_raw_word(procedure, "set_jab_combo")?;
                decoded.push(Procedure::SetJabCombo {
                    frame: required_u64(procedure, &["frame"])?,
                    disabled: required_bool(procedure, &["disabled"])?,
                    word_offset: required_u64(procedure, &["word_offset"])? as usize,
                    raw_word,
                });
            }
            "fighter.set_jab_rapid" => {
                let raw_word = first_raw_word(procedure, "set_jab_rapid")?;
                decoded.push(Procedure::SetJabRapid {
                    frame: required_u64(procedure, &["frame"])?,
                    state: required_bool(procedure, &["state"])?,
                    word_offset: required_u64(procedure, &["word_offset"])? as usize,
                    raw_word,
                });
            }
            "fighter.set_throw_flag" => {
                let raw_word = first_raw_word(procedure, "set_throw_flag")?;
                let hit_idx = required_u64(procedure, &["hit_idx"])?;
                let flag_bit = optional_u64(procedure, &["flag_bit"])?
                    .map(|value| {
                        u8::try_from(value)
                            .map_err(|_| format!("set_throw_flag flag_bit {value} does not fit u8"))
                    })
                    .transpose()?;
                decoded.push(Procedure::SetThrowFlag {
                    frame: required_u64(procedure, &["frame"])?,
                    hit_idx: u32::try_from(hit_idx).map_err(|_| {
                        format!("set_throw_flag hit_idx {hit_idx} does not fit u32")
                    })?,
                    flag_bit,
                    word_offset: required_u64(procedure, &["word_offset"])? as usize,
                    raw_word,
                });
            }
            "fighter.set_throw_hitbox" => {
                let raw_words = required_array(procedure, &["raw_words"])?;
                if raw_words.len() != 3 {
                    return Err(
                        "fighter.set_throw_hitbox raw_words must have 3 entries".to_string()
                    );
                }
                let mut words = [0u32; 3];
                for (index, raw_word) in raw_words.iter().enumerate() {
                    words[index] = parse_hex_u32(raw_word.as_str().ok_or_else(|| {
                        "set_throw_hitbox raw word must be a string".to_string()
                    })?)?;
                }
                let hitbox_idx = required_u64(procedure, &["hitbox_idx"])?;
                let damage = required_u64(procedure, &["damage"])?;
                let angle = required_u64(procedure, &["angle"])?;
                let hit_x24 = required_u64(procedure, &["hit_x24"])?;
                let hit_x28 = required_u64(procedure, &["hit_x28"])?;
                let hit_x2c = required_u64(procedure, &["hit_x2c"])?;
                let element = required_u64(procedure, &["element"])?;
                let sfx_severity = required_u64(procedure, &["sfx_severity"])?;
                let sfx_kind = required_u64(procedure, &["sfx_kind"])?;
                decoded.push(Procedure::SetThrowHitbox(DecodedThrowHitbox {
                    frame: required_u64(procedure, &["frame"])?,
                    word_offset: required_u64(procedure, &["word_offset"])? as usize,
                    raw_words: words,
                    hitbox: SourceThrowHitboxAttributes {
                        hitbox_idx: u8::try_from(hitbox_idx).map_err(|_| {
                            format!("set_throw_hitbox hitbox_idx {hitbox_idx} does not fit u8")
                        })?,
                        damage: u32::try_from(damage).map_err(|_| {
                            format!("set_throw_hitbox damage {damage} does not fit u32")
                        })?,
                        angle: u16::try_from(angle).map_err(|_| {
                            format!("set_throw_hitbox angle {angle} does not fit u16")
                        })?,
                        hit_x24: u16::try_from(hit_x24).map_err(|_| {
                            format!("set_throw_hitbox hit_x24 {hit_x24} does not fit u16")
                        })?,
                        hit_x28: u16::try_from(hit_x28).map_err(|_| {
                            format!("set_throw_hitbox hit_x28 {hit_x28} does not fit u16")
                        })?,
                        hit_x2c: u16::try_from(hit_x2c).map_err(|_| {
                            format!("set_throw_hitbox hit_x2c {hit_x2c} does not fit u16")
                        })?,
                        element: u8::try_from(element).map_err(|_| {
                            format!("set_throw_hitbox element {element} does not fit u8")
                        })?,
                        sfx_severity: u8::try_from(sfx_severity).map_err(|_| {
                            format!("set_throw_hitbox sfx_severity {sfx_severity} does not fit u8")
                        })?,
                        sfx_kind: u8::try_from(sfx_kind).map_err(|_| {
                            format!("set_throw_hitbox sfx_kind {sfx_kind} does not fit u8")
                        })?,
                    },
                }));
            }
            _ => {}
        }
    }
    decoded.sort_by_key(|procedure| match procedure {
        Procedure::SpawnHitbox(hitbox) => (hitbox.frame, hitbox.word_offset),
        Procedure::ClearAllHitboxes { frame, word_offset } => (*frame, *word_offset),
        Procedure::SetCmdVar {
            frame, word_offset, ..
        } => (*frame, *word_offset),
        Procedure::SetJabCombo {
            frame, word_offset, ..
        } => (*frame, *word_offset),
        Procedure::SetJabRapid {
            frame, word_offset, ..
        } => (*frame, *word_offset),
        Procedure::SetThrowFlag {
            frame, word_offset, ..
        } => (*frame, *word_offset),
        Procedure::SetThrowHitbox(throw_hitbox) => (throw_hitbox.frame, throw_hitbox.word_offset),
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
    let item_hit_interaction = bitfield(word3, 27, 1) != 0;
    let spawn_hitbox_skip_xf_b4 = bitfield(word3, 28, 1) != 0;
    let rebound = bitfield(word3, 31, 1) != 0;
    let hit_grabbed_victim_only = bitfield(word4, 12, 1) != 0;
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
        hitbox_flags: SourceHitboxFlags::from_decomp_spawn_hitbox_3(
            item_hit_interaction,
            spawn_hitbox_skip_xf_b4,
            bitfield(word3, 29, 1) != 0,
            bitfield(word3, 30, 1) != 0,
            rebound,
        )
        .with_skip_if_thrown_hitbox_owner_absent(spawn_hitbox_skip_xf_b4)
        .with_hit_grabbed_victim_only(hit_grabbed_victim_only),
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

#[derive(Clone)]
struct ActiveHitbox {
    hitbox: DecodedHitbox,
    spawn_frame: u64,
    lifecycle_id: SourceHitboxLifecycleId,
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
                Procedure::SetCmdVar { .. } => {}
                Procedure::SetJabCombo { .. } => {}
                Procedure::SetJabRapid { .. } => {}
                Procedure::SetThrowFlag { .. } => {}
                Procedure::SetThrowHitbox(_) => {}
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

fn sample_hitboxes_for_frame(
    pose: &[JointPose],
    previous_pose: Option<&[JointPose]>,
    procedures: &[Procedure],
    target_frame: u64,
) -> Vec<Value> {
    let mut active: BTreeMap<u64, (DecodedHitbox, u64)> = BTreeMap::new();
    for procedure in procedures
        .iter()
        .filter(|procedure| procedure_frame(procedure) <= target_frame)
    {
        match procedure {
            Procedure::SpawnHitbox(hitbox) => {
                active.insert(hitbox.id, (hitbox.clone(), hitbox.frame));
            }
            Procedure::ClearAllHitboxes { .. } => active.clear(),
            Procedure::SetCmdVar { .. } => {}
            Procedure::SetJabCombo { .. } => {}
            Procedure::SetJabRapid { .. } => {}
            Procedure::SetThrowFlag { .. } => {}
            Procedure::SetThrowHitbox(_) => {}
            Procedure::SetHurtState { .. } => {}
        }
    }

    active
        .values()
        .map(|(hitbox, spawn_frame)| {
            let current_center = hitbox_source_center(hitbox, pose);
            let (state, previous_center) = if *spawn_frame == target_frame {
                (SourceHitCapsuleState::Unk2, current_center)
            } else {
                (
                    SourceHitCapsuleState::Unk3,
                    previous_pose
                        .map(|previous_pose| hitbox_source_center(hitbox, previous_pose))
                        .unwrap_or(current_center),
                )
            };
            hitbox_sample_json_with_centers(hitbox, state, previous_center, current_center)
        })
        .collect()
}

fn sample_hitbox_capsules_for_frame(
    pose: &[JointPose],
    previous_pose: Option<&[JointPose]>,
    procedures: &[Procedure],
    target_frame: u64,
) -> Vec<RuntimeSourceCapsuleSample> {
    let mut active: BTreeMap<u64, ActiveHitbox> = BTreeMap::new();
    for procedure in procedures
        .iter()
        .filter(|procedure| procedure_frame(procedure) <= target_frame)
    {
        match procedure {
            Procedure::SpawnHitbox(hitbox) => {
                let lifecycle_id = source_hitbox_lifecycle_id_for_spawn(&active, hitbox);
                active.insert(
                    hitbox.id,
                    ActiveHitbox {
                        hitbox: hitbox.clone(),
                        spawn_frame: hitbox.frame,
                        lifecycle_id,
                    },
                );
            }
            Procedure::ClearAllHitboxes { .. } => active.clear(),
            Procedure::SetCmdVar { .. } => {}
            Procedure::SetJabCombo { .. } => {}
            Procedure::SetJabRapid { .. } => {}
            Procedure::SetThrowFlag { .. } => {}
            Procedure::SetThrowHitbox(_) => {}
            Procedure::SetHurtState { .. } => {}
        }
    }

    active
        .values()
        .map(|active_hitbox| {
            let hitbox = &active_hitbox.hitbox;
            let current_center = hitbox_source_center(hitbox, pose);
            let previous_center = if active_hitbox.spawn_frame == target_frame {
                current_center
            } else {
                previous_pose
                    .map(|previous_pose| hitbox_source_center(hitbox, previous_pose))
                    .unwrap_or(current_center)
            };
            RuntimeSourceCapsuleSample {
                id: hitbox.id,
                a: runtime_source_point(previous_center),
                b: runtime_source_point(current_center),
                radius: hitbox.radius,
                hurt_height: SOURCE_HURT_HEIGHT_MID,
                hitbox_lifecycle_id: Some(active_hitbox.lifecycle_id),
                hitbox: Some(SourceHitboxAttributes {
                    bone: hitbox.bone as u16,
                    hit_group: hitbox.hit_group as u8,
                    damage: hitbox.damage as u16,
                    angle: hitbox.angle as u16,
                    knockback_growth: hitbox.kbg as u16,
                    weight_set_knockback: hitbox.weight_set_kb as u16,
                    base_knockback: hitbox.bkb as u16,
                    element: hitbox.element as u8,
                    shield_damage: hitbox.shield_damage as i16,
                    hit_grounded: hitbox.hit_grounded,
                    hit_aerial: hitbox.hit_aerial,
                }),
                hitbox_flags: hitbox.hitbox_flags,
            }
        })
        .collect()
}

fn source_hitbox_lifecycle_id_for_spawn(
    active: &BTreeMap<u64, ActiveHitbox>,
    hitbox: &DecodedHitbox,
) -> SourceHitboxLifecycleId {
    if let Some(current) = active.get(&hitbox.id) {
        if current.hitbox.hit_group == hitbox.hit_group {
            return current.lifecycle_id;
        }
    }
    active
        .values()
        .find(|current| current.hitbox.hit_group == hitbox.hit_group)
        .map(|current| current.lifecycle_id)
        .unwrap_or_else(|| {
            SourceHitboxLifecycleId::new((hitbox.frame << 32) | hitbox.word_offset as u64)
        })
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
        "item_hit_interaction": hitbox.hitbox_flags.item_hit_interaction(),
        "ignore_thrown_fighters": hitbox.hitbox_flags.ignore_thrown_fighters(),
        "ignore_fighter_scale": hitbox.hitbox_flags.ignore_fighter_scale(),
        "clank": hitbox.hitbox_flags.clank(),
        "rebound": hitbox.hitbox_flags.rebound(),
        "skip_if_thrown_hitbox_owner_absent": hitbox.hitbox_flags.skip_if_thrown_hitbox_owner_absent(),
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

fn hitbox_sample_json_with_centers(
    hitbox: &DecodedHitbox,
    state: SourceHitCapsuleState,
    previous_center: Vec3,
    current_center: Vec3,
) -> Value {
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

fn sample_hurtbox_capsules_for_frame(
    pose: &[JointPose],
    hurtbox_inits: &[Value],
    procedures: &[Procedure],
    target_frame: u64,
) -> Result<Vec<RuntimeSourceCapsuleSample>, String> {
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
        let _state_raw = *hurt_states.get(&bone_idx).unwrap_or(&0);
        sampled.push(RuntimeSourceCapsuleSample {
            id: required_u64(hurtbox, &["id"])?,
            a: runtime_source_point(source_a),
            b: runtime_source_point(source_b),
            radius: required_f32(hurtbox, &["scale_raw"])?,
            hurt_height: required_hurt_height(hurtbox, &["height"])?,
            hitbox_lifecycle_id: None,
            hitbox: None,
            hitbox_flags: SourceHitboxFlags::none(),
        });
    }
    Ok(sampled)
}

fn runtime_source_point(point: Vec3) -> RuntimeSourcePoint {
    RuntimeSourcePoint {
        x: point.x,
        y: point.y,
        z: point.z,
    }
}

fn procedure_frame(procedure: &Procedure) -> u64 {
    match procedure {
        Procedure::SpawnHitbox(hitbox) => hitbox.frame,
        Procedure::ClearAllHitboxes { frame, .. } => *frame,
        Procedure::SetCmdVar { frame, .. } => *frame,
        Procedure::SetJabCombo { frame, .. } => *frame,
        Procedure::SetJabRapid { frame, .. } => *frame,
        Procedure::SetThrowFlag { frame, .. } => *frame,
        Procedure::SetThrowHitbox(throw_hitbox) => throw_hitbox.frame,
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
    Vec3::new(point.x, point.y, 0.0)
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

fn optional_u64(value: &Value, path: &[&str]) -> Result<Option<u64>, String> {
    let mut current = value;
    for segment in path {
        let Some(next) = current.get(*segment) else {
            return Ok(None);
        };
        current = next;
    }
    if current.is_null() {
        return Ok(None);
    }
    current
        .as_u64()
        .map(Some)
        .ok_or_else(|| format!("JSON field {} is not a u64", path.join(".")))
}

fn required_hurt_height(value: &Value, path: &[&str]) -> Result<u8, String> {
    let raw = required_u64(value, path)?;
    u8::try_from(raw)
        .ok()
        .filter(|height| *height <= 2)
        .ok_or_else(|| format!("JSON field {} is not a HurtHeight 0..=2", path.join(".")))
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

fn first_raw_word(procedure: &Value, procedure_name: &str) -> Result<u32, String> {
    let raw_words = required_array(procedure, &["raw_words"])?;
    let raw_word = raw_words
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{procedure_name} raw_words[0] must be a string"))?;
    parse_hex_u32(raw_word)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crate should live under workspace/crates")
            .to_path_buf()
    }

    fn sample_source(state: &str) -> ActionSampleSource {
        load_action_sample_source(
            &workspace_root(),
            &FrameDataSampleOptions {
                character: "dolphin_mole".to_string(),
                source_character: None,
                state: state.to_string(),
                frame: 1,
            },
        )
        .expect("source action sample should load")
    }

    fn joint_world_point(pose: &[JointPose], part_index: usize) -> RuntimeSourcePoint {
        let joint = pose
            .get(part_index)
            .expect("common part should exist in sampled pose");
        RuntimeSourcePoint {
            x: joint.world_matrix.rows[0][3],
            y: joint.world_matrix.rows[1][3],
            z: joint.world_matrix.rows[2][3],
        }
    }

    fn set_test_bitfield(word: &mut u32, offset_from_msb: u32, width: u32, value: u32) {
        let shift = 32 - offset_from_msb - width;
        let mask = ((1u32 << width) - 1) << shift;
        *word = (*word & !mask) | ((value << shift) & mask);
    }

    #[test]
    fn decode_spawn_hitbox_preserves_ftaction_decomp_flags() {
        let mut word3 = 0u32;
        let mut word4 = 0u32;
        set_test_bitfield(&mut word3, 27, 1, 1);
        set_test_bitfield(&mut word3, 28, 1, 1);
        set_test_bitfield(&mut word3, 29, 1, 1);
        set_test_bitfield(&mut word3, 30, 1, 1);
        set_test_bitfield(&mut word3, 31, 1, 1);
        set_test_bitfield(&mut word4, 12, 1, 1);

        let hitbox = decode_spawn_hitbox(11, 8, [0, 0, 0, word3, word4]);

        assert!(
            hitbox.hitbox_flags.item_hit_interaction(),
            "ftAction_8007121C assigns create_hitbox_3.item_hit_interaction into HitCapsule.x43_b0"
        );
        assert!(
            hitbox.hitbox_flags.ignore_thrown_fighters(),
            "struct spawn_hitbox_3 includes ignore_thrown_fighters and the decoded source command must preserve it"
        );
        assert!(
            hitbox.hitbox_flags.ignore_fighter_scale(),
            "ftAction_8007121C assigns create_hitbox_3.ignore_fighter_scale into HitCapsule.x43_b1"
        );
        assert!(
            hitbox.hitbox_flags.clank(),
            "ftAction_8007121C assigns create_hitbox_3.clank into HitCapsule.x40_b0"
        );
        assert!(
            hitbox.hitbox_flags.rebound(),
            "ftAction_8007121C assigns create_hitbox_3.rebound into HitCapsule.x40_b1"
        );
        assert!(
            hitbox.hitbox_flags.skip_if_thrown_hitbox_owner_absent(),
            "ftAction_8007121C checks spawn_hitbox_skip.xF_b4 before spawning the hit capsule"
        );
        assert!(
            hitbox.hitbox_flags.hit_grabbed_victim_only(),
            "ftAction_8007121C assigns create_hitbox_5.x1_b4 into HitCapsule.hit_grabbed_victim_only"
        );
    }

    #[test]
    fn decode_spawn_hitbox_does_not_alias_rebound_to_throw_owner_skip() {
        let mut word3 = 0u32;
        set_test_bitfield(&mut word3, 31, 1, 1);

        let hitbox = decode_spawn_hitbox(6, 8, [0, 0, 0, word3, 0]);

        assert!(
            hitbox.hitbox_flags.rebound(),
            "ftAction_8007121C assigns create_hitbox_3.rebound into HitCapsule.x40_b1"
        );
        assert!(
            !hitbox.hitbox_flags.skip_if_thrown_hitbox_owner_absent(),
            "spawn_hitbox_skip.xF_b4 is byte 0xF bit 4 in the decomp layout, not create_hitbox_3.rebound"
        );
    }

    #[test]
    fn runtime_source_capsule_sidecar_round_trips_hitbox_flags() {
        let flags = SourceHitboxFlags::from_decomp_spawn_hitbox_3(true, true, true, true, true);
        let actions = vec![RuntimeSourceActionFrameSamples {
            source_action_key: "Attack11".to_string(),
            total_frames: 1,
            loops: false,
            frames: vec![RuntimeSourceFrameSample {
                source_frame: 1,
                source_root_position: RuntimeSourcePoint::default(),
                down_bound_pose: RuntimeSourceDownBoundPoseSample {
                    hip_mtx_0_1: 0.0,
                    hip_mtx_0_2: 0.0,
                    hip_mtx_1_1: 0.0,
                    hip_mtx_1_2: 0.0,
                },
                capture_pose: RuntimeSourceCapturePoseSample::default(),
                hit_capsules: vec![RuntimeSourceCapsuleSample {
                    id: 0,
                    a: RuntimeSourcePoint::default(),
                    b: RuntimeSourcePoint::default(),
                    radius: 1.0,
                    hurt_height: SOURCE_HURT_HEIGHT_MID,
                    hitbox_lifecycle_id: Some(SourceHitboxLifecycleId::new(1)),
                    hitbox: Some(SourceHitboxAttributes {
                        bone: 0,
                        hit_group: 0,
                        damage: 1,
                        angle: 361,
                        knockback_growth: 100,
                        weight_set_knockback: 0,
                        base_knockback: 10,
                        element: 0,
                        shield_damage: 0,
                        hit_grounded: true,
                        hit_aerial: true,
                    }),
                    hitbox_flags: flags,
                }],
                hurt_capsules: Vec::new(),
            }],
            cmd_var_events: Vec::new(),
            script_events: Vec::new(),
        }];

        let encoded =
            encode_runtime_source_frame_capsules(&actions).expect("sidecar should encode");
        let decoded =
            decode_runtime_source_frame_capsules(&encoded).expect("sidecar should decode");

        assert_eq!(
            decoded[0].frames[0].hit_capsules[0].hitbox_flags, flags,
            "compact runtime artifacts must preserve ftAction_8007121C hitbox flags instead of dropping them before runtime collision"
        );
        assert!(
            !decoded[0].loops,
            "finite action playback metadata should round-trip through compact runtime sidecars"
        );
    }

    #[test]
    fn runtime_source_capsule_sidecar_round_trips_throw_hitbox_events() {
        let hitbox = SourceThrowHitboxAttributes {
            hitbox_idx: 1,
            damage: 13,
            angle: 90,
            hit_x24: 100,
            hit_x28: 30,
            hit_x2c: 45,
            element: 3,
            sfx_severity: 5,
            sfx_kind: 7,
        };
        let event = RuntimeSourceScriptEvent::SetThrowHitbox(RuntimeSourceThrowHitboxEvent {
            source_frame: 11,
            hitbox,
            word_offset: 8,
            raw_words: [0x8880000d, 0x2d1903c0, 0x169d7000],
        });
        let actions = vec![RuntimeSourceActionFrameSamples {
            source_action_key: "ThrowHi".to_string(),
            total_frames: 45,
            loops: true,
            frames: Vec::new(),
            cmd_var_events: Vec::new(),
            script_events: vec![event],
        }];

        let encoded =
            encode_runtime_source_frame_capsules(&actions).expect("sidecar should encode");
        assert_eq!(&encoded[..8], b"MSFC0013");
        let decoded =
            decode_runtime_source_frame_capsules(&encoded).expect("sidecar should decode");

        assert_eq!(
            decoded[0].script_events,
            vec![event],
            "ftAction_80071E04 throw-hitbox commands must survive compact runtime export as xDF4 state events"
        );
        assert!(
            decoded[0].loops,
            "looping action playback metadata should round-trip through compact runtime sidecars"
        );
    }

    #[test]
    fn source_common_parts_from_manifest_uses_resolved_jobj_joints() {
        let manifest = json!({
            "rig": {
                "common_parts": {
                    "data": {
                        "capture_anchor_part": 61,
                        "transn_part": 1,
                        "xrotn_part": 2,
                        "hipn_part": 4,
                        "transn2_part": 52,
                        "thrown_hitbox_part": 14,
                        "transn_joint": 1,
                        "xrotn_joint": 2,
                        "hipn_joint": 4,
                        "transn2_joint": 61,
                        "thrown_hitbox_joint": 27,
                        "thrown_hitbox_scale": 4.25
                    }
                }
            }
        });

        let parts =
            source_common_parts_from_manifest(&manifest).expect("source common parts should load");

        assert_eq!(
            parts.transn2_joint, 61,
            "runtime source pose sampling must use ftParts_GetBoneIndex(FtPart_TransN2), not the source enum value"
        );
        assert_eq!(
            parts.thrown_hitbox_joint, 27,
            "ft_8007C17C initializes x1064_thrownHitbox from fp->parts[ftData.x34->x0].joint, so runtime source pose sampling must use the resolved JObj joint"
        );
        assert_eq!(
            parts.thrown_hitbox_scale, 4.25,
            "ft_8007C17C copies ftData.x34->scale into x1064_thrownHitbox.scale"
        );
    }

    #[test]
    fn source_capture_pose_bakes_lb_8000b1cc_world_xyz_not_projected_view() {
        let source = sample_source("Catch");
        let pose = sample_pose_at_anim_frame(
            &source.figatree_chunk,
            &source.figatree,
            &source.skeleton,
            7.0,
            SourcePoseSetup::raw_joint_data(),
        )
        .expect("source pose should sample");

        let capture_pose =
            source_capture_pose(&pose, source.common_parts).expect("capture pose should sample");
        let expected_capture_anchor =
            joint_world_point(&pose, source.common_parts.capture_anchor_part);
        let expected_xrotn = joint_world_point(&pose, source.common_parts.xrotn_joint);
        let expected_transn2 = joint_world_point(&pose, source.common_parts.transn2_joint);
        let expected_thrown_hitbox =
            joint_world_point(&pose, source.common_parts.thrown_hitbox_joint);

        assert_eq!(
            capture_pose.capture_anchor, expected_capture_anchor,
            "ftCo_CapturePulled* reads lb_8000B1CC(capturedamage.x18), so runtime capture anchor must preserve live JObj world XYZ instead of the derived render projection"
        );
        assert_eq!(
            capture_pose.xrotn, expected_xrotn,
            "ftCo_CapturePulled* subtracts lb_8000B1CC(FtPart_XRotN) directly"
        );
        assert_eq!(
            capture_pose.transn2, expected_transn2,
            "throw placement reads lb_8000B1CC(FtPart_TransN2) directly"
        );
        assert_eq!(
            capture_pose.thrown_hitbox, expected_thrown_hitbox,
            "ft_8007C224 updates x1064_thrownHitbox from lb_8000B1CC(ftData.x34->x0 joint) every fighter update"
        );
    }

    #[test]
    fn source_live_pose_sampling_applies_decomp_topn_setup_before_capture_pose() {
        let source = sample_source("Catch");

        let live_pose = source
            .sample_live_pose(7.0)
            .expect("source live pose should sample");

        assert!(
            live_pose.capture_pose.capture_anchor.x > 8.0,
            "Fighter_ChangeMotionState sets TopN rot_y to M_PI_2 * facing_dir before capture reads lb_8000B1CC; the live source capture anchor should already be in gameplay X"
        );
        assert!(
            live_pose.capture_pose.capture_anchor.z.abs() < 0.5,
            "live capture pose should preserve lb_8000B1CC world XYZ after TopN setup, not leave the right-facing gameplay axis in source Z"
        );
    }
}
