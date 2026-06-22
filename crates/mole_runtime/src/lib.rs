use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{fs, io};

use serde::Serialize;

use mole_frame_data::{
    decode_runtime_source_frame_capsules, FrameDataSampleOptions, RuntimeActionFrameEvaluator,
    RuntimeSourceCapsuleSample, RuntimeSourceExportEvaluator, RuntimeSourceFrameSample,
    RuntimeSourceLivePoseSample, RuntimeSourcePoint as FrameDataRuntimeSourcePoint,
    RuntimeSourceScriptEvent as FrameDataRuntimeSourceScriptEvent,
};

use mole_core::collision::{
    source_collision_hits, source_damage_accumulator_after_stages, source_damage_result_for_victim,
    source_damage_stages_from_confirms, source_hit_confirms, Capsule3, SourceCollisionCapsule,
    SourceCollisionFrame, SourceCollisionHit, SourceDamageAccumulator, SourceDamageResult,
    SourceDamageResultInput, SourceDamageStage, SourceHitConfirm, SourceHitboxAttributes,
    SourceHitboxFlags, SourceHitboxLifecycleId, Vec3, SOURCE_HIT_ELEMENT_CATCH,
};
use mole_core::{
    canonical_source_action_binding_for_runtime_id, source_root_motion_position,
    source_special_action_binding_for_runtime_id, source_units_to_milli, step_world,
    step_world_with_source_runtime_data, EcbDiamond, FighterCameraBox, FighterEntryPlatformProfile,
    Frame, GameCubePadStatus, MeleeActionStateId, MeleeCommonData, MeleeInputFacts, MotionState,
    PlayerInput, PlayerState, SourceActionKey, SourceActionPoseMetadata, SourceActionScriptEvent,
    SourceActionScriptEvents, SourceCapturePose, SourceCollisionStep, SourceDownBoundPose,
    SourcePosePoint, SourceVec2, StageProfile, StageSurface, StageSurfaceKind, Vec2, World,
    WorldSnapshot, SOURCE_COLLISION_STATE_NORMAL, TICK_NANOS,
};
use mole_replay::{ReplayFrame, ReplayLog};
use mole_transport::{InputPacket, PacketAcceptResult};

pub mod assets;
pub use assets::{
    legacy_animation_for_motion_state, legacy_animation_spec, legacy_sprite_source_size,
    DolphinMoleVisualProfile, LegacyAnimationKey, LegacyAnimationSpec, LegacySpriteCue,
    SpriteSizeUnits, SpriteSourceSize, LEGACY_DOLPHIN_MOLE_ANIMATIONS,
};

#[cfg(feature = "sdl")]
pub mod sdl_input;

#[cfg(feature = "sdl")]
pub use sdl_input::{configure_sdl_controller_hints, SdlInputSource};

pub mod readout;
pub use readout::{
    ButtonReadout, ControllerInputTraceLog, InputReadout, InputTraceWriter, MeleeReadout,
    PlayerReadout,
};

mod slippi_diagnostic;
pub use slippi_diagnostic::{
    compare_slippi_export_from_match_start_with_core, compare_slippi_export_with_core,
    first_slippi_divergence_from_match_start_with_core,
    scan_slippi_export_from_match_start_with_core, slippi_core_report_path,
    slippi_visual_replay_divergence_for_frame, slippi_visual_replay_inputs_from_match_start,
    trace_slippi_export_from_match_start_with_core, write_slippi_core_report,
    write_slippi_core_trace_report, SlippiCoreComparison, SlippiCoreComparisonConfig,
    SlippiCoreComparisonMode, SlippiCoreDiagnosticError, SlippiCoreDivergenceKind,
    SlippiCoreDivergenceScan, SlippiCoreDivergenceScanConfig, SlippiCoreDivergenceScenario,
    SlippiCoreFirstDivergence, SlippiCoreMismatch, SlippiCorePositionDrift, SlippiCoreTrace,
    SlippiCoreTraceConfig, SlippiCoreTraceRow, SlippiVisualReplayDivergence,
    SlippiVisualReplayDivergenceGate, SlippiVisualReplayFrame,
};

#[rustfmt::skip]
#[path = "generated/source_frame_data.rs"]
mod source_frame_data;

pub mod wup_input;
pub use wup_input::{
    map_wup_ports_to_player_inputs, parse_wup_report, WupInputConfig, WupInputMapper,
    WupInputTrace, WupPlayerInputTrace, WupPort,
};

#[cfg(feature = "wup")]
pub use wup_input::WupInputSource;

const DEFAULT_MAX_TICKS_PER_UPDATE: u32 = 5;
const AXIS_DEADZONE: i16 = 8_000;
const CORE_TO_SCREEN_SCALE_DENOMINATOR: i64 = 1_000_000;
const MELEE_GAMEPLAY_CAMERA_FOV_DEGREES: f32 = 38.0;
const MELEE_GAMEPLAY_CAMERA_FOV_SMOOTH: f32 = 0.1;
const SOURCE_SPACE_MELEE_XYZ: &str = "melee_xyz";
const PROJECTED_VIEW_DERIVED_DEBUG: &str = "derived_debug_view";

pub fn default_play_world() -> World {
    World::for_slippi_battlefield_singles_match_start()
}

pub fn visual_platform_ledge_probe_world() -> World {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let platform = world.stage().soft_platforms[0];
    let ledge = world.stage().ledges[0];
    let mut platform_probe = world.players()[0];
    platform_probe.set_motion_state_alias(MotionState::Fall);
    platform_probe.grounded = false;
    platform_probe.facing = 1;
    platform_probe.position = Vec2 {
        x: platform.left_x,
        y: platform.y - 10_000,
    };
    platform_probe.source_position = mole_core::SourceVec2::from_milli(platform_probe.position);
    platform_probe.velocity.y = -1_000;
    platform_probe.source_self_velocity_y = -1.0;
    assert!(world.set_player_state_for_diagnostic(0, platform_probe));

    let mut stage_ledge_probe = world.players()[1];
    stage_ledge_probe.set_motion_state_alias(MotionState::Fall);
    stage_ledge_probe.grounded = false;
    stage_ledge_probe.facing = 1;
    stage_ledge_probe.position = Vec2 {
        x: ledge.x_milli - 1_500,
        y: ledge.y_milli - 12_000,
    };
    stage_ledge_probe.source_position =
        mole_core::SourceVec2::from_milli(stage_ledge_probe.position);
    stage_ledge_probe.velocity.y = -1_000;
    stage_ledge_probe.source_self_velocity_y = -1.0;
    assert!(world.set_player_state_for_diagnostic(1, stage_ledge_probe));

    step_world_with_source_collisions(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);
    world
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedStepClock {
    accumulator_nanos: u64,
    max_ticks_per_update: u32,
}

impl Default for FixedStepClock {
    fn default() -> Self {
        Self {
            accumulator_nanos: 0,
            max_ticks_per_update: DEFAULT_MAX_TICKS_PER_UPDATE,
        }
    }
}

impl FixedStepClock {
    pub fn add_elapsed_nanos(&mut self, elapsed_nanos: u64) -> u32 {
        self.accumulator_nanos = self.accumulator_nanos.saturating_add(elapsed_nanos);
        let available_ticks = self.accumulator_nanos / TICK_NANOS;
        let emitted_ticks = available_ticks.min(self.max_ticks_per_update as u64) as u32;

        if available_ticks > self.max_ticks_per_update as u64 {
            self.accumulator_nanos = 0;
        } else {
            self.accumulator_nanos -= emitted_ticks as u64 * TICK_NANOS;
        }

        emitted_ticks
    }
}

pub fn frame_pacing_sleep_nanos(elapsed_nanos: u64) -> u64 {
    TICK_NANOS.saturating_sub(elapsed_nanos)
}

pub fn frame_pacing_coarse_sleep_nanos(_remaining_nanos: u64) -> u64 {
    0
}

pub trait InputSource {
    fn poll_inputs(&mut self, frame: Frame) -> [PlayerInput; 2];
}

pub fn step_world_from_input_source<S: InputSource + ?Sized>(
    world: &mut World,
    input_source: &mut S,
    frame: Frame,
) -> [PlayerInput; 2] {
    let inputs = input_source.poll_inputs(frame);
    step_world(world, frame, &inputs);
    inputs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetplayLogRole {
    VisibleHost,
    HeadlessPeer,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetplayLogEvent<'a> {
    role: NetplayLogRole,
    event: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    peer_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sent_packets: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    received_packets: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duplicate_packets: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unsupported_packets: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_remote_frames: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rollback_corrections: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_remote_frame: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_remote_checksum: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_remote_sequence: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_acked_sequence: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_rtt_frames: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    world_checksum: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packet_bundle_len: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_ppm: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    advance_online_frame: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_online_frame: Option<bool>,
}

impl<'a> NetplayLogEvent<'a> {
    pub const fn new(role: NetplayLogRole, event: &'a str) -> Self {
        Self {
            role,
            event,
            room_code: None,
            peer_id: None,
            frame: None,
            message: None,
            sent_packets: None,
            received_packets: None,
            duplicate_packets: None,
            unsupported_packets: None,
            missing_remote_frames: None,
            rollback_corrections: None,
            last_remote_frame: None,
            last_remote_checksum: None,
            last_remote_sequence: None,
            last_acked_sequence: None,
            last_rtt_frames: None,
            world_checksum: None,
            packet_bundle_len: None,
            speed_ppm: None,
            advance_online_frame: None,
            skip_online_frame: None,
        }
    }

    pub const fn with_room_code(mut self, room_code: &'a str) -> Self {
        self.room_code = Some(room_code);
        self
    }

    pub const fn with_peer_id(mut self, peer_id: &'a str) -> Self {
        self.peer_id = Some(peer_id);
        self
    }

    pub const fn with_frame(mut self, frame: Frame) -> Self {
        self.frame = Some(frame.0);
        self
    }

    pub const fn with_message(mut self, message: &'a str) -> Self {
        self.message = Some(message);
        self
    }

    pub fn with_netplay_stats(mut self, stats: &UdpRuntimeStats) -> Self {
        self.sent_packets = nonzero_u32(stats.sent_packets);
        self.received_packets = nonzero_u32(stats.received_packets);
        self.duplicate_packets = nonzero_u32(stats.duplicate_packets);
        self.unsupported_packets = nonzero_u32(stats.unsupported_packets);
        self.missing_remote_frames = nonzero_u32(stats.missing_remote_frames);
        self.rollback_corrections = nonzero_u32(stats.rollback_corrections);
        self.last_remote_frame = stats.last_remote_frame.map(|frame| frame.0);
        self.last_remote_checksum = stats.last_remote_checksum;
        self.last_remote_sequence = stats.last_remote_sequence;
        self.last_acked_sequence = stats.last_acked_sequence;
        self.last_rtt_frames = stats.last_rtt_frames;
        self
    }

    pub const fn with_world_checksum(mut self, checksum: u64) -> Self {
        self.world_checksum = Some(checksum);
        self
    }

    pub const fn with_packet_bundle_len(mut self, len: u32) -> Self {
        self.packet_bundle_len = Some(len);
        self
    }

    pub const fn with_pacing(
        mut self,
        speed_ppm: u32,
        advance_online_frame: bool,
        skip_online_frame: bool,
    ) -> Self {
        self.speed_ppm = Some(speed_ppm);
        self.advance_online_frame = Some(advance_online_frame);
        self.skip_online_frame = Some(skip_online_frame);
        self
    }
}

const fn nonzero_u32(value: u32) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug)]
pub struct BoundedNetplayLogger<W> {
    writer: W,
    max_bytes: usize,
    written_bytes: usize,
    cap_logged: bool,
    disabled: bool,
}

impl<W: Write> BoundedNetplayLogger<W> {
    pub const fn new(writer: W, max_bytes: usize) -> Self {
        Self {
            writer,
            max_bytes,
            written_bytes: 0,
            cap_logged: false,
            disabled: false,
        }
    }

    pub fn write_event(&mut self, event: NetplayLogEvent<'_>) -> io::Result<bool> {
        let mut line = serde_json::to_vec(&event)?;
        line.push(b'\n');
        if self.disabled || !self.write_line(&line)? {
            self.write_cap_event(event.role)?;
            return Ok(false);
        }
        Ok(true)
    }

    fn write_line(&mut self, line: &[u8]) -> io::Result<bool> {
        if self.disabled {
            return Ok(false);
        }
        if self.written_bytes.saturating_add(line.len()) > self.max_bytes {
            return Ok(false);
        }
        self.writer.write_all(line)?;
        self.written_bytes = self.written_bytes.saturating_add(line.len());
        Ok(true)
    }

    fn write_cap_event(&mut self, role: NetplayLogRole) -> io::Result<()> {
        if self.cap_logged {
            self.disabled = true;
            return Ok(());
        }
        self.cap_logged = true;
        let mut line = serde_json::to_vec(&NetplayLogEvent::new(role, "log_cap_reached"))?;
        line.push(b'\n');
        let _ = self.write_line(&line)?;
        self.disabled = true;
        Ok(())
    }
}

pub fn apply_source_collisions_for_world(world: &mut World) -> SourceCollisionStep {
    let frame = RenderFrame::from_world(world);
    source_collision_step_from_frame(world, &frame)
}

pub fn step_world_with_source_collisions(
    world: &mut World,
    frame: Frame,
    inputs: &[PlayerInput; 2],
) {
    let _ = step_world_with_source_collision_step(world, frame, inputs);
}

pub fn step_world_with_source_collision_step(
    world: &mut World,
    frame: Frame,
    inputs: &[PlayerInput; 2],
) -> SourceCollisionStep {
    step_world_with_source_runtime_data(
        world,
        frame,
        inputs,
        runtime_source_pose_metadata_for_player,
        runtime_source_action_total_frames_for_action_state_id,
    );
    let source_step = apply_source_collisions_for_world(world);
    world.apply_source_damage_results_with_action_total_frames(
        &source_step.results,
        runtime_source_action_total_frames_for_action_state_id,
    );
    world.commit_staged_source_damage();
    source_step
}

pub fn preload_runtime_source_frame_data() -> Result<usize, String> {
    let actions = runtime_source_actions()?;
    Ok(actions.len())
}

pub fn runtime_source_frame_data_is_preloaded() -> bool {
    RUNTIME_SOURCE_ACTION_CACHE.get().is_some()
}

pub fn source_collision_frame_from_frame(frame: &RenderFrame) -> SourceCollisionFrame {
    let mut hits = Vec::new();
    let mut hurts = Vec::new();
    for player_index in 0..frame.player_positions.len() {
        if !frame.player_participates(player_index) {
            continue;
        }
        let source_action_key = source_action_key_for_player(frame, player_index);
        let action_state_id = frame.player_source_pose_action_state_ids[player_index];
        let damage_hit_source_frame =
            source_collision_capsule_frame_for_player(frame, player_index, source_action_key);
        let catch_hit_source_frame = damage_hit_source_frame;
        extend_source_hits_for_player(
            &mut hits,
            frame,
            player_index,
            source_action_key,
            action_state_id,
            catch_hit_source_frame,
            damage_hit_source_frame,
            SourceHitElementFilter::NonCatch,
        );
        extend_source_hits_for_player(
            &mut hits,
            frame,
            player_index,
            source_action_key,
            action_state_id,
            catch_hit_source_frame,
            catch_hit_source_frame,
            SourceHitElementFilter::Catch,
        );
        if frame.player_source_collision_states[player_index] == SOURCE_COLLISION_STATE_NORMAL {
            let hurt_source_action_key = source_hurt_action_key_for_player(frame, player_index);
            let hurt_source_frame = source_collision_hurt_capsule_frame_for_player(
                frame,
                player_index,
                hurt_source_action_key,
            );
            let hurt_source_root = source_render_root_position(
                frame,
                player_index,
                hurt_source_action_key,
                hurt_source_frame,
            );
            if let Some(source_capsules) =
                runtime_source_frame_capsules_ref(hurt_source_action_key, hurt_source_frame)
            {
                let thrown_constraint = source_thrown_hurt_constraint(frame, player_index);
                hurts.extend(
                    source_capsules
                        .hurt_capsules
                        .iter()
                        .copied()
                        .map(|capsule| {
                            let current_capsule = if let Some(constraint) = thrown_constraint {
                                source_capsule_to_constrained_xrotn_world_3d(
                                    capsule,
                                    constraint.current_anchor,
                                    frame.player_model_facing(player_index),
                                    constraint.victim_xrotn,
                                )
                            } else {
                                source_capsule_to_world_3d(
                                    capsule,
                                    frame.player_source_positions[player_index],
                                    frame.player_model_facing(player_index),
                                    hurt_source_root,
                                )
                            };
                            let previous_capsule = if let Some(constraint) = thrown_constraint {
                                source_capsule_to_constrained_xrotn_world_3d(
                                    capsule,
                                    constraint.previous_anchor,
                                    frame.player_model_facing(player_index),
                                    constraint.victim_xrotn,
                                )
                            } else {
                                source_capsule_to_world_3d(
                                    capsule,
                                    frame.player_source_previous_positions[player_index],
                                    frame.player_model_facing(player_index),
                                    hurt_source_root,
                                )
                            };
                            SourceCollisionCapsule::new(player_index, capsule.id, current_capsule)
                                .with_previous_capsule(previous_capsule)
                                .with_owner_grounded(frame.player_grounded[player_index])
                                .with_source_pose(
                                    action_state_id,
                                    source_action_key,
                                    hurt_source_frame,
                                )
                                .with_hurt_height(capsule.hurt_height)
                        }),
                );
            }
        }
    }
    SourceCollisionFrame { hits, hurts }
}

#[derive(Clone, Copy)]
enum SourceHitElementFilter {
    Catch,
    NonCatch,
}

impl SourceHitElementFilter {
    fn accepts(self, capsule: RuntimeSourceCapsule) -> bool {
        let is_catch = capsule
            .hitbox
            .is_some_and(|hitbox| hitbox.element == SOURCE_HIT_ELEMENT_CATCH);
        match self {
            SourceHitElementFilter::Catch => is_catch,
            SourceHitElementFilter::NonCatch => !is_catch,
        }
    }
}

fn extend_source_hits_for_player(
    hits: &mut Vec<SourceCollisionCapsule>,
    frame: &RenderFrame,
    player_index: usize,
    source_action_key: Option<SourceActionKey>,
    action_state_id: Option<MeleeActionStateId>,
    active_hit_source_frame: u8,
    geometry_hit_source_frame: u8,
    filter: SourceHitElementFilter,
) {
    let previous_hit_source_frame = geometry_hit_source_frame.saturating_sub(1).max(1);
    let hit_source_root = source_render_root_position(
        frame,
        player_index,
        source_action_key,
        geometry_hit_source_frame,
    );
    let previous_source_root = source_render_root_position(
        frame,
        player_index,
        source_action_key,
        previous_hit_source_frame,
    );
    if let Some(active_source_capsules) =
        runtime_source_frame_capsules_ref(source_action_key, active_hit_source_frame)
    {
        let geometry_source_capsules =
            runtime_source_frame_capsules_ref(source_action_key, geometry_hit_source_frame);
        hits.extend(
            active_source_capsules
                .hit_capsules
                .iter()
                .copied()
                .filter(|capsule| filter.accepts(*capsule))
                .filter(|capsule| {
                    !capsule.hitbox_flags.skip_if_thrown_hitbox_owner_absent()
                        || frame.player_source_thrown_hitbox_owner_indexes[player_index].is_some()
                })
                .map(|capsule| {
                    let geometry_capsule = geometry_source_capsules
                        .and_then(|geometry_source_capsules| {
                            source_hit_geometry_capsule_for_active(
                                capsule,
                                geometry_source_capsules.hit_capsules.as_slice(),
                                filter,
                            )
                        })
                        .unwrap_or(capsule);
                    let collision_capsule = RuntimeSourceCapsule {
                        a: geometry_capsule.a,
                        b: geometry_capsule.b,
                        radius: capsule.radius,
                        ..capsule
                    };
                    let uses_previous_root = source_hit_capsule_uses_previous_root(
                        collision_capsule,
                        geometry_hit_source_frame,
                    );
                    let a_root_position = if uses_previous_root {
                        frame.player_source_previous_positions[player_index]
                    } else {
                        frame.player_source_positions[player_index]
                    };
                    let a_source_root = if uses_previous_root {
                        previous_source_root
                    } else {
                        hit_source_root
                    };
                    SourceCollisionCapsule::new(
                        player_index,
                        capsule.id,
                        source_hit_capsule_to_world_3d(
                            collision_capsule,
                            a_root_position,
                            frame.player_source_positions[player_index],
                            frame.player_model_facing(player_index),
                            a_source_root,
                            hit_source_root,
                        ),
                    )
                    .with_owner_grounded(frame.player_grounded[player_index])
                    .with_source_pose(
                        action_state_id,
                        source_action_key,
                        geometry_hit_source_frame,
                    )
                    .with_optional_hitbox_lifecycle(capsule.hitbox_lifecycle_id)
                    .with_optional_hitbox_attributes(capsule.hitbox)
                    .with_hitbox_flags(capsule.hitbox_flags)
                }),
        );
    }
}

fn source_hit_geometry_capsule_for_active(
    active: RuntimeSourceCapsule,
    geometry_capsules: &[RuntimeSourceCapsule],
    filter: SourceHitElementFilter,
) -> Option<RuntimeSourceCapsule> {
    geometry_capsules
        .iter()
        .copied()
        .find(|candidate| {
            filter.accepts(*candidate)
                && candidate.id == active.id
                && active.hitbox_lifecycle_id.is_some()
                && candidate.hitbox_lifecycle_id == active.hitbox_lifecycle_id
        })
        .or_else(|| {
            geometry_capsules.iter().copied().find(|candidate| {
                filter.accepts(*candidate)
                    && candidate.id == active.id
                    && source_hitbox_group(*candidate) == source_hitbox_group(active)
            })
        })
}

fn source_hitbox_group(capsule: RuntimeSourceCapsule) -> Option<u8> {
    capsule.hitbox.map(|hitbox| hitbox.hit_group)
}

#[derive(Clone, Copy)]
struct SourceThrownHurtConstraint {
    current_anchor: SourcePosePoint,
    previous_anchor: SourcePosePoint,
    victim_xrotn: SourcePosePoint,
}

pub fn source_collision_hits_from_frame(frame: &RenderFrame) -> Vec<SourceCollisionHit> {
    source_collision_hits(&source_collision_frame_from_frame(frame))
}

pub fn source_collision_step_from_frame(
    world: &mut World,
    frame: &RenderFrame,
) -> SourceCollisionStep {
    let collision_frame = source_collision_frame_from_frame(frame);
    world.apply_source_collision_frame_with_action_total_frames(
        &collision_frame,
        runtime_source_action_total_frames_for_action_state_id,
    )
}

pub fn source_hit_confirms_from_frame(frame: &RenderFrame) -> Vec<SourceHitConfirm> {
    source_hit_confirms(&source_collision_frame_from_frame(frame))
}

pub fn source_damage_stages_from_frame(frame: &RenderFrame) -> Vec<SourceDamageStage> {
    source_damage_stages_from_confirms(&source_hit_confirms_from_frame(frame))
}

pub fn source_damage_results_from_frame(frame: &RenderFrame) -> Vec<SourceDamageResult> {
    let stages = source_damage_stages_from_frame(frame);
    source_damage_results_from_stages_for_frame(frame, &stages)
}

pub fn source_damage_results_from_stages_for_frame(
    frame: &RenderFrame,
    stages: &[SourceDamageStage],
) -> Vec<SourceDamageResult> {
    let mut results = Vec::new();
    for victim_index in 0..frame.player_positions.len() {
        let accumulator = source_damage_accumulator_after_stages(
            SourceDamageAccumulator {
                victim_index,
                percent_temp: frame.player_damage_percent_temps[victim_index],
                applied_damage: frame.player_damage_applied[victim_index],
            },
            stages,
        );
        if let Some(result) = source_damage_result_for_victim(
            frame.common_data,
            stages,
            SourceDamageResultInput {
                victim_index,
                victim_percent: frame.player_damage_percents[victim_index],
                victim_percent_temp: accumulator.percent_temp,
                victim_weight: frame.player_profile_weights[victim_index],
                stage: 1.0,
                attack: 1.0,
                defense: 1.0,
            },
        ) {
            results.push(result);
        }
    }
    results
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalInput {
    pub left_x: i16,
    pub left_y: i16,
    pub c_x: i16,
    pub c_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub attack: bool,
    pub special: bool,
    pub jump_primary: bool,
    pub jump_secondary: bool,
    pub shield: bool,
    pub grab: bool,
    pub left_trigger_pressed: bool,
    pub right_trigger_pressed: bool,
    pub start: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
}

pub fn map_physical_input(input: PhysicalInput) -> PlayerInput {
    PlayerInput::neutral()
        .with_left_stick(axis_to_i8(input.left_x), axis_to_i8(input.left_y))
        .with_c_stick(axis_to_i8(input.c_x), axis_to_i8(input.c_y))
        .with_left_trigger_analog(input.left_trigger)
        .with_right_trigger_analog(input.right_trigger)
        .with_left_trigger_digital(input.left_trigger_pressed)
        .with_right_trigger_digital(input.right_trigger_pressed)
        .with_attack(input.attack)
        .with_special(input.special)
        .with_jump_primary(input.jump_primary)
        .with_jump_secondary(input.jump_secondary)
        .with_shield(input.shield)
        .with_grab(input.grab)
        .with_start(input.start)
        .with_dpad_up(input.dpad_up)
        .with_dpad_down(input.dpad_down)
        .with_dpad_left(input.dpad_left)
        .with_dpad_right(input.dpad_right)
}

pub fn physical_input_from_gamecube_pad(pad: GameCubePadStatus) -> PhysicalInput {
    let (left_x, left_y) = pad.main_stick_i16();
    let (c_x, c_y) = pad.c_stick_i16();

    PhysicalInput {
        left_x,
        left_y,
        c_x,
        c_y,
        left_trigger: pad.left_trigger,
        right_trigger: pad.right_trigger,
        attack: pad.buttons.a(),
        special: pad.buttons.b(),
        jump_primary: pad.buttons.x(),
        jump_secondary: pad.buttons.y(),
        shield: false,
        grab: pad.buttons.z(),
        left_trigger_pressed: pad.buttons.l(),
        right_trigger_pressed: pad.buttons.r(),
        start: pad.buttons.start(),
        dpad_up: pad.buttons.dpad_up(),
        dpad_down: pad.buttons.dpad_down(),
        dpad_left: pad.buttons.dpad_left(),
        dpad_right: pad.buttons.dpad_right(),
    }
}

pub fn map_gamecube_pad_to_player_input(pad: GameCubePadStatus) -> PlayerInput {
    let (stick_x, stick_y) = pad.main_stick_i8();
    let (c_stick_x, c_stick_y) = pad.c_stick_i8();

    PlayerInput::neutral()
        .with_left_stick(stick_x, stick_y)
        .with_c_stick(c_stick_x, c_stick_y)
        .with_left_trigger_analog(pad.left_trigger)
        .with_right_trigger_analog(pad.right_trigger)
        .with_left_trigger_digital(pad.buttons.l())
        .with_right_trigger_digital(pad.buttons.r())
        .with_attack(pad.buttons.a())
        .with_special(pad.buttons.b())
        .with_jump_primary(pad.buttons.x())
        .with_jump_secondary(pad.buttons.y())
        .with_grab(pad.buttons.z())
        .with_start(pad.buttons.start())
        .with_dpad_up(pad.buttons.dpad_up())
        .with_dpad_down(pad.buttons.dpad_down())
        .with_dpad_left(pad.buttons.dpad_left())
        .with_dpad_right(pad.buttons.dpad_right())
}

fn axis_to_i8(value: i16) -> i8 {
    let wide = value as i32;
    if wide.abs() < AXIS_DEADZONE as i32 {
        return 0;
    }

    let scaled = wide * 127 / 32_767;
    scaled.clamp(-127, 127) as i8
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderFrame {
    pub frame: Frame,
    pub stage: StageProfile,
    pub common_data: MeleeCommonData,
    pub match_phase: mole_core::MatchPhase,
    pub match_phase_timer: u16,
    pub player_states: [u8; 2],
    pub player_stocks: [i8; 2],
    pub player_positions: [Vec2; 2],
    pub player_source_positions: [SourceVec2; 2],
    pub player_source_previous_positions: [SourceVec2; 2],
    pub player_velocities: [Vec2; 2],
    pub player_camera_boxes: [FighterCameraBox; 2],
    pub player_ecbs: [EcbDiamond; 2],
    pub player_grounded: [bool; 2],
    pub player_facings: [i8; 2],
    pub player_damage_percents: [f32; 2],
    pub player_damage_percent_temps: [f32; 2],
    pub player_damage_applied: [u16; 2],
    pub player_damage_knockbacks: [f32; 2],
    pub player_damage_angles: [u16; 2],
    pub player_damage_elements: [u8; 2],
    pub player_hitlag_frames: [u8; 2],
    pub player_damage_hitstun_frames: [u16; 2],
    pub player_source_collision_states: [u8; 2],
    pub player_profile_weights: [f32; 2],
    pub player_action_state_ids: [Option<MeleeActionStateId>; 2],
    pub player_source_action_keys: [Option<SourceActionKey>; 2],
    pub player_source_thrown_hitbox_owner_indexes: [Option<u8>; 2],
    pub player_source_thrown_hitbox_team_unks: [u8; 2],
    pub player_source_thrown_hitbox_grabber_player_ids: [Option<u8>; 2],
    pub player_motion_state_aliases: [Option<MotionState>; 2],
    pub player_motion_states: [MotionState; 2],
    pub player_state_frames: [u8; 2],
    pub player_animation_frames: [u8; 2],
    pub player_source_motion_anim_frames: [f32; 2],
    pub player_source_pose_action_state_ids: [Option<MeleeActionStateId>; 2],
    pub player_source_pose_action_keys: [Option<SourceActionKey>; 2],
    pub player_source_pose_motion_states: [MotionState; 2],
    pub player_source_pose_frames: [u8; 2],
    pub player_source_pose_model_facings: [i8; 2],
    pub player_source_victim_indexes: [Option<u8>; 2],
    pub player_source_x1a5c_indexes: [Option<u8>; 2],
    pub player_source_x2226_b2: [bool; 2],
    pub player_ground_velocity_x: [f32; 2],
    pub player_ground_accel_x: [f32; 2],
    pub player_ground_accel_x2: [f32; 2],
    pub player_dash_entry_velocity_delta: [f32; 2],
    pub player_dash_x0: [f32; 2],
    pub player_walk_anim_velocity_x: [f32; 2],
    pub player_walk_accel_mul_milli: [i32; 2],
    pub player_turn_facing_after: [i8; 2],
    pub player_turn_has_turned: [bool; 2],
    pub player_turn_just_turned: [bool; 2],
    pub player_turn_frames_to_turn: [u8; 2],
    pub player_turn_dash_after_direction: [i8; 2],
    pub player_turn_latched_buttons: [u8; 2],
    pub player_run_no_interrupt_frames: [u8; 2],
    pub player_motion_cmd_var0: [u32; 2],
    pub player_motion_cmd_var1: [u32; 2],
    pub player_run_brake_x0: [bool; 2],
    pub player_run_brake_frames_remaining: [u8; 2],
    pub player_turn_run_accel_mul: [i8; 2],
    pub player_turn_run_x14: [bool; 2],
    pub player_motion_anim_rate_milli: [i32; 2],
    pub player_entry_base_y: [i32; 2],
    pub player_entry_platforms: [FighterEntryPlatformProfile; 2],
    pub player_entry_platform_offset_y: [i32; 2],
    pub player_entry_timers: [u8; 2],
    pub player_debug_input_facts: [MeleeInputFacts; 2],
    pub checksum: u64,
}

impl RenderFrame {
    pub fn from_world(world: &World) -> Self {
        Self::from_snapshot(world.snapshot())
    }

    pub fn from_snapshot(snapshot: WorldSnapshot) -> Self {
        Self {
            frame: snapshot.frame,
            stage: snapshot.stage,
            common_data: snapshot.common_data,
            match_phase: snapshot.match_phase,
            match_phase_timer: snapshot.match_phase_timer,
            player_states: [
                snapshot.players[0].player_state,
                snapshot.players[1].player_state,
            ],
            player_stocks: [snapshot.players[0].stocks, snapshot.players[1].stocks],
            player_positions: [snapshot.players[0].position, snapshot.players[1].position],
            player_source_positions: [
                snapshot.players[0].source_position,
                snapshot.players[1].source_position,
            ],
            player_source_previous_positions: [
                snapshot.players[0].source_coll_prev_pos,
                snapshot.players[1].source_coll_prev_pos,
            ],
            player_velocities: [snapshot.players[0].velocity, snapshot.players[1].velocity],
            player_camera_boxes: [
                snapshot.players[0].camera_box,
                snapshot.players[1].camera_box,
            ],
            player_ecbs: [
                snapshot.players[0].active_ecb,
                snapshot.players[1].active_ecb,
            ],
            player_grounded: [snapshot.players[0].grounded, snapshot.players[1].grounded],
            player_facings: [snapshot.players[0].facing, snapshot.players[1].facing],
            player_damage_percents: [
                snapshot.players[0].damage_percent,
                snapshot.players[1].damage_percent,
            ],
            player_damage_percent_temps: [
                snapshot.players[0].damage_percent_temp,
                snapshot.players[1].damage_percent_temp,
            ],
            player_damage_applied: [
                snapshot.players[0].damage_applied,
                snapshot.players[1].damage_applied,
            ],
            player_damage_knockbacks: [
                snapshot.players[0].damage_knockback,
                snapshot.players[1].damage_knockback,
            ],
            player_damage_angles: [
                snapshot.players[0].damage_angle,
                snapshot.players[1].damage_angle,
            ],
            player_damage_elements: [
                snapshot.players[0].damage_element,
                snapshot.players[1].damage_element,
            ],
            player_hitlag_frames: [
                snapshot.players[0].hitlag_frames,
                snapshot.players[1].hitlag_frames,
            ],
            player_damage_hitstun_frames: [
                snapshot.players[0].damage_hitstun_frames,
                snapshot.players[1].damage_hitstun_frames,
            ],
            player_source_collision_states: [
                snapshot.players[0].source_collision_state,
                snapshot.players[1].source_collision_state,
            ],
            player_profile_weights: [
                snapshot.players[0].profile_weight,
                snapshot.players[1].profile_weight,
            ],
            player_action_state_ids: [
                snapshot.players[0].melee_action_state_id,
                snapshot.players[1].melee_action_state_id,
            ],
            player_source_action_keys: [
                snapshot.players[0].source_action_key,
                snapshot.players[1].source_action_key,
            ],
            player_source_thrown_hitbox_owner_indexes: [
                snapshot.players[0].source_thrown_hitbox_owner_index,
                snapshot.players[1].source_thrown_hitbox_owner_index,
            ],
            player_source_thrown_hitbox_team_unks: [
                snapshot.players[0].source_thrown_hitbox_team_unk,
                snapshot.players[1].source_thrown_hitbox_team_unk,
            ],
            player_source_thrown_hitbox_grabber_player_ids: [
                snapshot.players[0].source_thrown_hitbox_grabber_player_id,
                snapshot.players[1].source_thrown_hitbox_grabber_player_id,
            ],
            player_motion_state_aliases: [
                snapshot.players[0].motion_state_alias,
                snapshot.players[1].motion_state_alias,
            ],
            player_motion_states: [
                snapshot.players[0].motion_state,
                snapshot.players[1].motion_state,
            ],
            player_state_frames: [
                snapshot.players[0].state_frame,
                snapshot.players[1].state_frame,
            ],
            player_animation_frames: [
                snapshot.players[0].animation_frame,
                snapshot.players[1].animation_frame,
            ],
            player_source_motion_anim_frames: [
                snapshot.players[0].source_motion_anim_frame,
                snapshot.players[1].source_motion_anim_frame,
            ],
            player_source_pose_action_state_ids: [
                snapshot.players[0].source_pose_action_state_id,
                snapshot.players[1].source_pose_action_state_id,
            ],
            player_source_pose_action_keys: [
                snapshot.players[0].source_pose_action_key,
                snapshot.players[1].source_pose_action_key,
            ],
            player_source_pose_motion_states: [
                snapshot.players[0].source_pose_motion_state,
                snapshot.players[1].source_pose_motion_state,
            ],
            player_source_pose_frames: [
                snapshot.players[0].source_pose_frame,
                snapshot.players[1].source_pose_frame,
            ],
            player_source_pose_model_facings: [
                snapshot.players[0].source_pose_model_facing,
                snapshot.players[1].source_pose_model_facing,
            ],
            player_source_victim_indexes: [
                snapshot.players[0].source_victim_index,
                snapshot.players[1].source_victim_index,
            ],
            player_source_x1a5c_indexes: [
                snapshot.players[0].source_x1a5c_index,
                snapshot.players[1].source_x1a5c_index,
            ],
            player_source_x2226_b2: [
                snapshot.players[0].source_x2226_b2,
                snapshot.players[1].source_x2226_b2,
            ],
            player_ground_velocity_x: [
                snapshot.players[0].ground_velocity_x,
                snapshot.players[1].ground_velocity_x,
            ],
            player_ground_accel_x: [
                snapshot.players[0].ground_accel_x,
                snapshot.players[1].ground_accel_x,
            ],
            player_ground_accel_x2: [
                snapshot.players[0].ground_accel_x2,
                snapshot.players[1].ground_accel_x2,
            ],
            player_dash_entry_velocity_delta: [
                snapshot.players[0].dash_entry_velocity_delta,
                snapshot.players[1].dash_entry_velocity_delta,
            ],
            player_dash_x0: [snapshot.players[0].dash_x0, snapshot.players[1].dash_x0],
            player_walk_anim_velocity_x: [
                snapshot.players[0].walk_anim_velocity_x,
                snapshot.players[1].walk_anim_velocity_x,
            ],
            player_walk_accel_mul_milli: [
                snapshot.players[0].walk_accel_mul_milli,
                snapshot.players[1].walk_accel_mul_milli,
            ],
            player_turn_facing_after: [
                snapshot.players[0].turn_facing_after,
                snapshot.players[1].turn_facing_after,
            ],
            player_turn_has_turned: [
                snapshot.players[0].turn_has_turned,
                snapshot.players[1].turn_has_turned,
            ],
            player_turn_just_turned: [
                snapshot.players[0].turn_just_turned,
                snapshot.players[1].turn_just_turned,
            ],
            player_turn_frames_to_turn: [
                snapshot.players[0].turn_frames_to_turn,
                snapshot.players[1].turn_frames_to_turn,
            ],
            player_turn_dash_after_direction: [
                snapshot.players[0].turn_dash_after_direction,
                snapshot.players[1].turn_dash_after_direction,
            ],
            player_turn_latched_buttons: [
                snapshot.players[0].turn_latched_buttons,
                snapshot.players[1].turn_latched_buttons,
            ],
            player_run_no_interrupt_frames: [
                snapshot.players[0].run_no_interrupt_frames,
                snapshot.players[1].run_no_interrupt_frames,
            ],
            player_motion_cmd_var0: [
                snapshot.players[0].motion_cmd_var0,
                snapshot.players[1].motion_cmd_var0,
            ],
            player_motion_cmd_var1: [
                snapshot.players[0].motion_cmd_var1,
                snapshot.players[1].motion_cmd_var1,
            ],
            player_run_brake_x0: [
                snapshot.players[0].run_brake_x0,
                snapshot.players[1].run_brake_x0,
            ],
            player_run_brake_frames_remaining: [
                snapshot.players[0].run_brake_frames_remaining,
                snapshot.players[1].run_brake_frames_remaining,
            ],
            player_turn_run_accel_mul: [
                snapshot.players[0].turn_run_accel_mul,
                snapshot.players[1].turn_run_accel_mul,
            ],
            player_turn_run_x14: [
                snapshot.players[0].turn_run_x14,
                snapshot.players[1].turn_run_x14,
            ],
            player_motion_anim_rate_milli: [
                snapshot.players[0].motion_anim_rate_milli,
                snapshot.players[1].motion_anim_rate_milli,
            ],
            player_entry_base_y: [
                snapshot.players[0].entry_base_y,
                snapshot.players[1].entry_base_y,
            ],
            player_entry_platforms: [
                snapshot.players[0].entry_platform,
                snapshot.players[1].entry_platform,
            ],
            player_entry_platform_offset_y: [
                snapshot.players[0].entry_platform_offset_y,
                snapshot.players[1].entry_platform_offset_y,
            ],
            player_entry_timers: [
                snapshot.players[0].entry_timer,
                snapshot.players[1].entry_timer,
            ],
            player_debug_input_facts: [
                snapshot.players[0].debug_input_facts,
                snapshot.players[1].debug_input_facts,
            ],
            checksum: snapshot.checksum,
        }
    }

    fn player_model_facing(&self, index: usize) -> i8 {
        self.player_source_pose_model_facings[index]
    }

    fn player_participates(&self, index: usize) -> bool {
        self.player_states[index] == mole_core::PLAYER_STATE_IN_GAME
            && self.player_stocks[index] > 0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderTransform {
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub center_x: i32,
    pub ground_y: i32,
    pub pixels_per_core_unit_milli: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCameraTransformState {
    pub interest_x: f32,
    pub interest_y: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub target_interest_x: f32,
    pub target_interest_y: f32,
    pub target_position_x: f32,
    pub target_position_y: f32,
    pub target_position_z: f32,
    pub fov_degrees: f32,
    pub target_fov_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCameraSubjectBounds {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
    pub total_subjects: u8,
    pub z_pos: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCameraState {
    pub transform: RenderCameraTransformState,
}

impl RenderCameraState {
    pub fn new_for_stage(stage: &StageProfile) -> Self {
        let transform = initial_camera_transform(stage);
        Self { transform }
    }

    pub fn battlefield() -> Self {
        let stage = StageProfile::battlefield();
        Self::new_for_stage(&stage)
    }

    pub fn update_from_frame(
        &mut self,
        frame: &RenderFrame,
        viewport_width: u32,
        viewport_height: u32,
    ) -> RenderTransform {
        if frame.stage.melee_stage_profile().is_some() {
            self.step_melee_camera(frame, viewport_width, viewport_height)
        } else {
            RenderTransform::stage_camera(&frame.stage, viewport_width, viewport_height)
        }
    }

    fn step_melee_camera(
        &mut self,
        frame: &RenderFrame,
        viewport_width: u32,
        viewport_height: u32,
    ) -> RenderTransform {
        if frame.stage.melee_stage_profile().is_none() {
            return RenderTransform::stage_camera(&frame.stage, viewport_width, viewport_height);
        }
        let mut bounds = melee_camera_subject_bounds(frame, self.transform.position_z);
        self.transform.target_fov_degrees = MELEE_GAMEPLAY_CAMERA_FOV_DEGREES;
        self.transform.fov_degrees += (self.transform.target_fov_degrees
            - self.transform.fov_degrees)
            * MELEE_GAMEPLAY_CAMERA_FOV_SMOOTH;
        melee_camera_target_from_bounds(
            &frame.stage,
            &mut bounds,
            &mut self.transform,
            viewport_width,
            viewport_height,
        );
        melee_camera_smooth_interest(&bounds, &mut self.transform);
        melee_camera_smooth_position(&mut self.transform);
        RenderTransform::from_camera_transform(
            &frame.stage,
            self.transform,
            viewport_width,
            viewport_height,
        )
    }
}

impl RenderTransform {
    pub fn battlefield_camera(viewport_width: u32, viewport_height: u32) -> Self {
        let stage = StageProfile::battlefield();
        Self::stage_camera(&stage, viewport_width, viewport_height)
    }

    pub fn stage_camera(stage: &StageProfile, viewport_width: u32, viewport_height: u32) -> Self {
        if let Some(melee_stage) = stage.melee_stage_profile() {
            let bounds = melee_stage.camera.cam_bounds;
            let left = source_units_to_milli(bounds.left);
            let right = source_units_to_milli(bounds.right);
            let top = source_units_to_milli(bounds.top);
            let bottom = source_units_to_milli(bounds.bottom);
            let width = (right - left).abs().max(1) as i64;
            let height = (top - bottom).abs().max(1) as i64;
            let fit_width = viewport_width as i64 * CORE_TO_SCREEN_SCALE_DENOMINATOR * 9 / 10;
            let fit_height = viewport_height as i64 * CORE_TO_SCREEN_SCALE_DENOMINATOR * 9 / 10;
            let pixels_per_core_unit_milli =
                (fit_width / width).min(fit_height / height).max(1) as i32;
            let center_x_milli = (left + right) / 2;
            let center_y_milli = (top + bottom) / 2;
            let center_x = viewport_width as i32 / 2
                - scale_core_delta_with(pixels_per_core_unit_milli, center_x_milli);
            let ground_y = viewport_height as i32 / 2
                + scale_core_delta_with(pixels_per_core_unit_milli, center_y_milli);
            return Self {
                viewport_width,
                viewport_height,
                center_x,
                ground_y,
                pixels_per_core_unit_milli,
            };
        }

        let stage_width_units = stage.main_floor.right_x - stage.main_floor.left_x;
        let target_stage_width = viewport_width as i32 * 3 / 4;
        let pixels_per_core_unit_milli = (target_stage_width as i64
            * CORE_TO_SCREEN_SCALE_DENOMINATOR
            / stage_width_units as i64) as i32;

        Self {
            viewport_width,
            viewport_height,
            center_x: viewport_width as i32 / 2,
            ground_y: viewport_height as i32 * 3 / 4,
            pixels_per_core_unit_milli,
        }
    }

    pub fn from_camera_state(
        camera: &RenderCameraState,
        stage: &StageProfile,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        Self::from_camera_transform(stage, camera.transform, viewport_width, viewport_height)
    }

    fn from_camera_transform(
        stage: &StageProfile,
        camera: RenderCameraTransformState,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let Some(melee_stage) = stage.melee_stage_profile() else {
            return Self::stage_camera(stage, viewport_width, viewport_height);
        };
        let aspect = viewport_width as f32 / viewport_height.max(1) as f32;
        let half_fov = (camera.fov_degrees.max(1.0) * 0.5).to_radians();
        let half_height = (camera
            .position_z
            .abs()
            .max(melee_stage.camera.cam_zoom_rate)
            * half_fov.tan())
        .max(1.0);
        let half_width = (half_height * aspect).max(1.0);
        let scale_x = viewport_width as f32 / (half_width * 2.0);
        let scale_y = viewport_height as f32 / (half_height * 2.0);
        let pixels_per_core_unit_milli = ((scale_x.min(scale_y) * 1000.0).round() as i32).max(1);
        let center_x = viewport_width as i32 / 2
            - scale_core_delta_with(
                pixels_per_core_unit_milli,
                source_units_to_milli(camera.interest_x),
            );
        let ground_y = viewport_height as i32 / 2
            + scale_core_delta_with(
                pixels_per_core_unit_milli,
                source_units_to_milli(camera.interest_y),
            );

        Self {
            viewport_width,
            viewport_height,
            center_x,
            ground_y,
            pixels_per_core_unit_milli,
        }
    }

    pub fn world_to_screen(self, point: Vec2) -> RenderPoint {
        RenderPoint {
            x: self.center_x + self.scale_core_delta(point.x),
            y: self.ground_y - self.scale_core_delta(point.y),
        }
    }

    pub fn core_length_to_screen(self, length: i32) -> u32 {
        self.scale_core_delta(length).unsigned_abs().max(1)
    }

    fn scale_core_delta(self, value: i32) -> i32 {
        scale_core_delta_with(self.pixels_per_core_unit_milli, value)
    }
}

fn initial_camera_transform(stage: &StageProfile) -> RenderCameraTransformState {
    if let Some(melee_stage) = stage.melee_stage_profile() {
        let bounds = melee_stage.camera.cam_bounds;
        let interest_x = (bounds.left + bounds.right) * 0.5 + melee_stage.camera.cam_x_offset;
        let interest_y = (bounds.top + bounds.bottom) * 0.5 + melee_stage.camera.cam_y_offset;
        let fov = melee_stage.camera.fixed_cam_fov.max(1.0);
        let position_z = melee_stage
            .camera
            .fixed_cam_pos
            .z
            .abs()
            .max(melee_stage.camera.cam_zoom_rate);
        return RenderCameraTransformState {
            interest_x,
            interest_y,
            position_x: interest_x,
            position_y: interest_y,
            position_z,
            target_interest_x: interest_x,
            target_interest_y: interest_y,
            target_position_x: interest_x,
            target_position_y: interest_y,
            target_position_z: position_z,
            fov_degrees: fov,
            target_fov_degrees: fov,
        };
    }

    RenderCameraTransformState {
        interest_x: 0.0,
        interest_y: 0.0,
        position_x: 0.0,
        position_y: 0.0,
        position_z: 300.0,
        target_interest_x: 0.0,
        target_interest_y: 0.0,
        target_position_x: 0.0,
        target_position_y: 0.0,
        target_position_z: 300.0,
        fov_degrees: 30.0,
        target_fov_degrees: 30.0,
    }
}

fn melee_camera_subject_bounds(frame: &RenderFrame, current_z: f32) -> RenderCameraSubjectBounds {
    let stage = frame
        .stage
        .melee_stage_profile()
        .expect("melee_camera_subject_bounds requires a Melee stage profile");
    let cam = stage.camera;
    let subject_weight = [0.0_f32, 1.5, 1.32, 1.16, 1.0][2];
    let tracking_multiplier = subject_weight * cam.cam_track_ratio;
    let mut x_min = f32::MAX;
    let mut y_min = f32::MAX;
    let mut x_max = -f32::MAX;
    let mut y_max = -f32::MAX;
    let mut total_subjects = 0_u8;

    for index in 0..frame.player_positions.len() {
        if !frame.player_participates(index) {
            continue;
        }
        let root = frame.player_positions[index];
        let camera_box = frame.player_camera_boxes[index];
        let root_x = milli_to_source_for_camera(root.x);
        let root_y = milli_to_source_for_camera(root.y);
        let facing_right = frame.player_facings[index] == 1;
        let (subject_x2c_x, subject_x2c_y) = if facing_right {
            (camera_box.x0.z, camera_box.x0.y * cam.cam_fixed_zoom)
        } else {
            (-camera_box.x0.y * cam.cam_fixed_zoom, -camera_box.x0.z)
        };
        let subject_x10_x = root_x;
        let subject_x10_y = root_y + camera_box.x0.x;
        let x_a = (subject_x10_x + subject_x2c_x * tracking_multiplier)
            .clamp(cam.cam_bounds.left, cam.cam_bounds.right);
        let x_b = (subject_x10_x + subject_x2c_y * tracking_multiplier)
            .clamp(cam.cam_bounds.left, cam.cam_bounds.right);
        let y_a = (subject_x10_y + camera_box.xc.y * tracking_multiplier)
            .clamp(cam.cam_bounds.bottom, cam.cam_bounds.top);
        let y_b = (subject_x10_y + camera_box.xc.x * tracking_multiplier)
            .clamp(cam.cam_bounds.bottom, cam.cam_bounds.top);
        x_min = x_min.min(x_a).min(x_b);
        x_max = x_max.max(x_a).max(x_b);
        y_min = y_min.min(y_a).min(y_b);
        y_max = y_max.max(y_a).max(y_b);
        total_subjects += 1;
    }

    if total_subjects == 0 {
        return RenderCameraSubjectBounds {
            x_min: cam.cam_bounds.left,
            y_min: cam.cam_bounds.bottom,
            x_max: cam.cam_bounds.right,
            y_max: cam.cam_bounds.top,
            total_subjects,
            z_pos: current_z.abs(),
        };
    }

    let z_pos = current_z.abs();
    let z_factor = if z_pos < 80.0 {
        0.0
    } else if z_pos > 5000.0 {
        1.0
    } else {
        (z_pos - 80.0) / 4920.0
    };
    y_min -= (390.0 * z_factor) + 10.0;
    y_min = y_min.max(cam.cam_bounds.bottom);

    RenderCameraSubjectBounds {
        x_min,
        y_min,
        x_max,
        y_max,
        total_subjects,
        z_pos,
    }
}

fn melee_camera_target_from_bounds(
    stage: &StageProfile,
    bounds: &mut RenderCameraSubjectBounds,
    transform: &mut RenderCameraTransformState,
    viewport_width: u32,
    viewport_height: u32,
) {
    let melee_stage = stage
        .melee_stage_profile()
        .expect("Melee camera target requires stage metadata");
    let cam = melee_stage.camera;
    let aspect = viewport_width as f32 / viewport_height.max(1) as f32;
    let half_fov = (transform.target_fov_degrees.max(1.0) * 0.5).to_radians();
    let vertical_dist = (bounds.y_max - bounds.y_min).max(1.0) / (2.0 * half_fov.tan());
    let horizontal_dist =
        (bounds.x_max - bounds.x_min).max(1.0) / (2.0 * aspect.max(0.001) * half_fov.tan());
    let target_z = vertical_dist
        .max(horizontal_dist)
        .max(cam.cam_zoom_rate)
        .min(cam.cam_max_depth);
    let visible_half_height = target_z * half_fov.tan();
    let visible_half_width = visible_half_height * aspect;
    let mut target_x = (bounds.x_min + bounds.x_max) * 0.5 + cam.cam_x_offset;
    let mut target_y = (bounds.y_min + bounds.y_max) * 0.5 + cam.cam_y_offset;

    target_x = clamp_visible_center(
        target_x,
        visible_half_width,
        cam.cam_bounds.left,
        cam.cam_bounds.right,
    );
    target_y = clamp_visible_center(
        target_y,
        visible_half_height,
        cam.cam_bounds.bottom,
        cam.cam_bounds.top,
    );

    transform.target_interest_x = target_x;
    transform.target_interest_y = target_y;
    transform.target_position_x = target_x;
    transform.target_position_y = target_y;
    transform.target_position_z = target_z;
    bounds.z_pos = target_z;
}

fn melee_camera_smooth_interest(
    bounds: &RenderCameraSubjectBounds,
    transform: &mut RenderCameraTransformState,
) {
    let spread = (bounds.x_max - bounds.x_min).max(bounds.y_max - bounds.y_min);
    let follow_speed = if spread > 900.0 {
        0.1
    } else if spread < 120.0 {
        0.05
    } else {
        ((spread - 120.0) / (900.0 - 120.0)) * (0.1 - 0.05) + 0.05
    };
    let lerp = follow_speed.clamp(0.0001, 1.0);
    transform.interest_x += (transform.target_interest_x - transform.interest_x) * lerp;
    transform.interest_y += (transform.target_interest_y - transform.interest_y) * lerp;
}

fn melee_camera_smooth_position(transform: &mut RenderCameraTransformState) {
    let lerp = 0.15_f32;
    transform.position_x += (transform.target_position_x - transform.position_x) * lerp;
    transform.position_y += (transform.target_position_y - transform.position_y) * lerp;
    transform.position_z += (transform.target_position_z - transform.position_z) * lerp;
}

fn clamp_visible_center(center: f32, half_extent: f32, min: f32, max: f32) -> f32 {
    if (max - min) <= half_extent * 2.0 {
        return (min + max) * 0.5;
    }
    center.clamp(min + half_extent, max - half_extent)
}

fn milli_to_source_for_camera(value: i32) -> f32 {
    value as f32 / 1000.0
}

fn scale_core_delta_with(pixels_per_core_unit_milli: i32, value: i32) -> i32 {
    let numerator = value as i64 * pixels_per_core_unit_milli as i64;
    if numerator >= 0 {
        ((numerator + CORE_TO_SCREEN_SCALE_DENOMINATOR / 2) / CORE_TO_SCREEN_SCALE_DENOMINATOR)
            as i32
    } else {
        ((numerator - CORE_TO_SCREEN_SCALE_DENOMINATOR / 2) / CORE_TO_SCREEN_SCALE_DENOMINATOR)
            as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RenderColor {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BACKGROUND: Self = Self {
        r: 17,
        g: 19,
        b: 24,
        a: 255,
    };
    pub const DEV_BACKGROUND: Self = Self {
        r: 247,
        g: 250,
        b: 252,
        a: 255,
    };
    pub const STAGE: Self = Self {
        r: 180,
        g: 187,
        b: 196,
        a: 255,
    };
    pub const SOFT_PLATFORM: Self = Self {
        r: 134,
        g: 203,
        b: 190,
        a: 255,
    };
    pub const ECB: Self = Self {
        r: 87,
        g: 237,
        b: 133,
        a: 255,
    };
    pub const PLAYER_ONE: Self = Self {
        r: 74,
        g: 138,
        b: 255,
        a: 255,
    };
    pub const PLAYER_TWO: Self = Self {
        r: 255,
        g: 198,
        b: 87,
        a: 255,
    };
    pub const SHIELD_BUBBLE: Self = Self {
        r: 94,
        g: 192,
        b: 255,
        a: 96,
    };
    pub const HITBOX_PILL: Self = Self {
        r: 239,
        g: 68,
        b: 68,
        a: 112,
    };
    pub const HURTBOX_PILL: Self = Self {
        r: 246,
        g: 197,
        b: 83,
        a: 96,
    };
    pub const ENTRY_PLATFORM: Self = Self {
        r: 120,
        g: 206,
        b: 255,
        a: 112,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub color: RenderColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCircle {
    pub center: RenderPoint,
    pub radius: u32,
    pub color: RenderColor,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceRenderPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl PartialEq for SourceRenderPoint {
    fn eq(&self, other: &Self) -> bool {
        self.x.to_bits() == other.x.to_bits()
            && self.y.to_bits() == other.y.to_bits()
            && self.z.to_bits() == other.z.to_bits()
    }
}

impl Eq for SourceRenderPoint {}

#[derive(Debug, Clone, Copy)]
pub struct SourceRenderCapsule {
    pub a: SourceRenderPoint,
    pub b: SourceRenderPoint,
    pub radius: f64,
}

impl PartialEq for SourceRenderCapsule {
    fn eq(&self, other: &Self) -> bool {
        self.a == other.a && self.b == other.b && self.radius.to_bits() == other.radius.to_bits()
    }
}

impl Eq for SourceRenderCapsule {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCapsule {
    pub a: RenderPoint,
    pub b: RenderPoint,
    pub radius: u32,
    pub color: RenderColor,
    pub source: SourceRenderCapsule,
    pub source_space: &'static str,
    pub projected_view_kind: &'static str,
    pub source_artifact_kind: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderImage {
    pub relative_path: &'static str,
    pub rect: RenderRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderPolygon {
    pub points: [RenderPoint; 4],
    pub color: RenderColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLine {
    pub start: RenderPoint,
    pub end: RenderPoint,
    pub color: RenderColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderScene {
    pub background: RenderColor,
    pub background_image: Option<RenderImage>,
    pub transform: RenderTransform,
    pub stage: RenderRect,
    pub stage_surfaces: Vec<RenderRect>,
    pub stage_collision_lines: Vec<RenderLine>,
    pub players: [RenderRect; 2],
    /// Screen-space projection of the fighter's simulation/root position.
    /// Visual assets stay anchored here while the active ECB polygon is drawn
    /// from core-owned collision data.
    pub player_contact_points: [RenderPoint; 2],
    pub player_ecbs: [RenderPolygon; 2],
    pub player_sprites: [LegacySpriteCue; 2],
    pub player_shields: [Option<RenderCircle>; 2],
    pub player_hitbox_pills: [Vec<RenderCapsule>; 2],
    pub player_hurtbox_pills: [Vec<RenderCapsule>; 2],
    pub entry_platforms: [Option<RenderRect>; 2],
    pub entry_platform_wireframes: [Vec<RenderLine>; 2],
    pub match_intro_label: Option<&'static str>,
}

impl RenderScene {
    pub fn from_frame(frame: &RenderFrame, viewport_width: u32, viewport_height: u32) -> Self {
        Self::from_frame_on_stage(frame, &frame.stage, viewport_width, viewport_height)
    }

    pub fn from_frame_with_camera(
        frame: &RenderFrame,
        viewport_width: u32,
        viewport_height: u32,
        camera: &mut RenderCameraState,
    ) -> Self {
        let transform = camera.update_from_frame(frame, viewport_width, viewport_height);
        Self::from_frame_on_stage_with_transform(
            frame,
            &frame.stage,
            viewport_width,
            viewport_height,
            transform,
        )
    }

    pub fn from_frame_on_stage(
        frame: &RenderFrame,
        stage_profile: &StageProfile,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let transform =
            RenderTransform::stage_camera(stage_profile, viewport_width, viewport_height);
        Self::from_frame_on_stage_with_transform(
            frame,
            stage_profile,
            viewport_width,
            viewport_height,
            transform,
        )
    }

    fn from_frame_on_stage_with_transform(
        frame: &RenderFrame,
        stage_profile: &StageProfile,
        viewport_width: u32,
        viewport_height: u32,
        transform: RenderTransform,
    ) -> Self {
        let player_colors = [RenderColor::PLAYER_ONE, RenderColor::PLAYER_TWO];
        let stage_surfaces = render_stage_surfaces(stage_profile, transform);
        let stage_collision_lines = render_stage_collision_lines(stage_profile, transform);
        let background = if stage_profile.melee_stage_profile().is_some()
            || stage_profile.name == "dev_flat_test"
        {
            RenderColor::DEV_BACKGROUND
        } else {
            RenderColor::BACKGROUND
        };
        let background_image = if stage_profile.melee_stage_profile().is_some() {
            None
        } else {
            Some(RenderImage {
                relative_path: "background.png",
                rect: RenderRect {
                    x: 0,
                    y: 0,
                    width: viewport_width,
                    height: viewport_height,
                    color: background,
                },
            })
        };
        let player_sprites = [
            LegacySpriteCue::for_player(
                frame.player_motion_states[0],
                frame.player_animation_frames[0],
                frame.player_model_facing(0),
            ),
            LegacySpriteCue::for_player(
                frame.player_motion_states[1],
                frame.player_animation_frames[1],
                frame.player_model_facing(1),
            ),
        ];
        let visual = DolphinMoleVisualProfile::default();
        let player_contact_points = [
            transform.world_to_screen(frame.player_positions[0]),
            transform.world_to_screen(frame.player_positions[1]),
        ];
        let players = [
            active_player_rect(
                frame,
                0,
                player_contact_points[0],
                transform,
                player_sprites[0],
                visual,
                player_colors[0],
            ),
            active_player_rect(
                frame,
                1,
                player_contact_points[1],
                transform,
                player_sprites[1],
                visual,
                player_colors[1],
            ),
        ];

        let entry_platforms = [
            entry_platform(frame, 0, transform, players[0]),
            entry_platform(frame, 1, transform, players[1]),
        ];
        let entry_platform_wireframes = [
            entry_platform_wireframe(entry_platforms[0]),
            entry_platform_wireframe(entry_platforms[1]),
        ];

        Self {
            background,
            background_image,
            transform,
            stage: stage_surfaces[0],
            stage_surfaces,
            stage_collision_lines,
            players,
            player_contact_points,
            player_ecbs: [
                player_ecb(frame, 0, transform),
                player_ecb(frame, 1, transform),
            ],
            player_sprites,
            player_shields: [
                player_shield(frame.player_motion_states[0], players[0]),
                player_shield(frame.player_motion_states[1], players[1]),
            ],
            player_hitbox_pills: [
                player_hitbox_pills(frame, 0, transform),
                player_hitbox_pills(frame, 1, transform),
            ],
            player_hurtbox_pills: [
                player_hurtbox_pills(frame, 0, transform),
                player_hurtbox_pills(frame, 1, transform),
            ],
            entry_platforms,
            entry_platform_wireframes,
            match_intro_label: match_intro_label(frame),
        }
    }
}

pub fn project_asset_root() -> PathBuf {
    if let Some(root) = std::env::var_os("MOLE_ASSET_ROOT").map(PathBuf::from) {
        if !root.as_os_str().is_empty() {
            return root;
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(root) = packaged_asset_root_for_exe(&exe_path) {
            return root;
        }
    }

    source_asset_root()
}

pub fn packaged_asset_root_for_exe(exe_path: &Path) -> Option<PathBuf> {
    let root = exe_path.parent()?;
    has_runtime_assets(root).then(|| root.to_path_buf())
}

fn source_asset_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn has_runtime_assets(root: &Path) -> bool {
    root.join("DolphinMole").is_dir()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugOverlay {
    pub lines: Vec<String>,
    pub player_state_lines: [String; 2],
}

impl DebugOverlay {
    pub fn from_frame(frame: &RenderFrame) -> Self {
        Self {
            lines: vec![
                format!("FRAME {}", frame.frame.0),
                format!("CHECKSUM {}", frame.checksum),
            ],
            player_state_lines: [
                player_state_overlay_line(frame, 0),
                player_state_overlay_line(frame, 1),
            ],
        }
    }

    pub fn from_frame_with_udp_stats(frame: &RenderFrame, stats: &UdpRuntimeStats) -> Self {
        let mut overlay = Self::from_frame(frame);
        overlay.lines.push(format!(
            "UDP TX {} RX {} DUP {} MISS {} RB {}",
            stats.sent_packets,
            stats.received_packets,
            stats.duplicate_packets,
            stats.missing_remote_frames,
            stats.rollback_corrections
        ));
        overlay.lines.push(format!(
            "REMOTE FRAME {} CHECKSUM {}",
            stats
                .last_remote_frame
                .map(|frame| frame.0.to_string())
                .unwrap_or_else(|| "NONE".to_string()),
            stats
                .last_remote_checksum
                .map(|checksum| checksum.to_string())
                .unwrap_or_else(|| "NONE".to_string())
        ));
        overlay.lines.push(format!(
            "UDP RTT {}",
            stats
                .last_rtt_frames
                .map(|frames| format!("{frames}F"))
                .unwrap_or_else(|| "NONE".to_string())
        ));
        overlay
    }
}

fn player_state_overlay_line(frame: &RenderFrame, index: usize) -> String {
    let action = frame.player_action_state_ids[index]
        .map(|action| action.get().to_string())
        .unwrap_or_else(|| "NONE".to_string());
    format!(
        "P{} A{} {} F{} D{} K{:.1} H{} S{}",
        index + 1,
        action,
        format!("{:?}", frame.player_motion_states[index]).to_ascii_uppercase(),
        frame.player_state_frames[index],
        frame.player_damage_percents[index] as u16,
        frame.player_damage_knockbacks[index],
        frame.player_hitlag_frames[index],
        frame.player_damage_hitstun_frames[index]
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDebugLog {
    json_line: String,
}

impl FrameDebugLog {
    pub fn from_frame_and_scene(
        frame: &RenderFrame,
        scene: &RenderScene,
        inputs: [PlayerInput; 2],
    ) -> Self {
        let player_logs = [
            player_debug_json(frame, scene, inputs[0], 0),
            player_debug_json(frame, scene, inputs[1], 1),
        ];
        let entry_platforms = scene
            .entry_platforms
            .iter()
            .map(|platform| match platform {
                Some(rect) => render_rect_json(*rect),
                None => "null".to_string(),
            })
            .collect::<Vec<_>>()
            .join(",");
        let source_hit_confirms = source_hit_confirms_from_frame(frame)
            .iter()
            .map(source_hit_confirm_json)
            .collect::<Vec<_>>()
            .join(",");
        let source_damage_stages = source_damage_stages_from_frame(frame)
            .iter()
            .map(source_damage_stage_json)
            .collect::<Vec<_>>()
            .join(",");
        let source_damage_results = source_damage_results_from_frame(frame)
            .iter()
            .map(source_damage_result_json)
            .collect::<Vec<_>>()
            .join(",");

        Self {
            json_line: format!(
                "{{\"frame\":{},\"checksum\":{},\"p1_bits\":{},\"p2_bits\":{},\"players\":[{},{}],\"render_transform\":{{\"center_x\":{},\"ground_y\":{},\"pixels_per_core_unit_milli\":{}}},\"match_intro_label\":{},\"entry_platforms\":[{}],\"source_hit_confirms\":[{}],\"source_damage_stages\":[{}],\"source_damage_results\":[{}]}}",
                frame.frame.0,
                frame.checksum,
                inputs[0].bits(),
                inputs[1].bits(),
                player_logs[0],
                player_logs[1],
                scene.transform.center_x,
                scene.transform.ground_y,
                scene.transform.pixels_per_core_unit_milli,
                match scene.match_intro_label {
                    Some(label) => format!("\"{label}\""),
                    None => "null".to_string(),
                },
                entry_platforms,
                source_hit_confirms,
                source_damage_stages,
                source_damage_results
            ),
        }
    }

    pub fn to_json_line(&self) -> String {
        self.json_line.clone()
    }
}

fn source_hit_confirm_json(confirm: &SourceHitConfirm) -> String {
    format!(
        "{{\"attacker_index\":{},\"victim_index\":{},\"hitbox_id\":{},\"hurtbox_id\":{},\"action_state_id\":{},\"source_action_key\":{},\"source_frame\":{},\"damaged_hurt_height\":{},\"hitbox\":{}}}",
        confirm.attacker_index,
        confirm.victim_index,
        confirm.hitbox_id,
        confirm.hurtbox_id,
        optional_action_state_id_json(confirm.action_state_id),
        optional_source_action_key_json(confirm.source_action_key),
        optional_source_frame_json(confirm.source_frame),
        confirm.damaged_hurt_height,
        source_hitbox_attributes_json(confirm.hitbox)
    )
}

fn source_damage_stage_json(stage: &SourceDamageStage) -> String {
    format!(
        "{{\"attacker_index\":{},\"victim_index\":{},\"hitbox_id\":{},\"hurtbox_id\":{},\"action_state_id\":{},\"source_action_key\":{},\"source_frame\":{},\"damaged_hurt_height\":{},\"damage\":{},\"env_damage\":{},\"unk_count\":{},\"hitbox\":{}}}",
        stage.attacker_index,
        stage.victim_index,
        stage.hitbox_id,
        stage.hurtbox_id,
        optional_action_state_id_json(stage.action_state_id),
        optional_source_action_key_json(stage.source_action_key),
        optional_source_frame_json(stage.source_frame),
        stage.damaged_hurt_height,
        stage.damage,
        stage.env_damage,
        stage.unk_count,
        source_hitbox_attributes_json(stage.hitbox)
    )
}

fn source_damage_result_json(result: &SourceDamageResult) -> String {
    let stage = result.stage;
    format!(
        "{{\"attacker_index\":{},\"victim_index\":{},\"hitbox_id\":{},\"hurtbox_id\":{},\"action_state_id\":{},\"source_action_key\":{},\"source_frame\":{},\"damaged_hurt_height\":{},\"damage\":{},\"env_damage\":{},\"unk_count\":{},\"knockback\":{},\"angle\":{},\"element\":{},\"hitbox\":{}}}",
        stage.attacker_index,
        stage.victim_index,
        stage.hitbox_id,
        stage.hurtbox_id,
        optional_action_state_id_json(stage.action_state_id),
        optional_source_action_key_json(stage.source_action_key),
        optional_source_frame_json(stage.source_frame),
        stage.damaged_hurt_height,
        stage.damage,
        stage.env_damage,
        stage.unk_count,
        result.knockback,
        result.angle,
        result.element,
        source_hitbox_attributes_json(stage.hitbox)
    )
}

fn optional_action_state_id_json(value: Option<MeleeActionStateId>) -> String {
    value
        .map(|id| id.get().to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_source_action_key_json(value: Option<SourceActionKey>) -> String {
    value
        .map(|key| format!("\"{}\"", key.as_str()))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_source_frame_json(value: Option<u8>) -> String {
    value
        .map(|frame| frame.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn source_hitbox_attributes_json(hitbox: SourceHitboxAttributes) -> String {
    format!(
        "{{\"bone\":{},\"hit_group\":{},\"damage\":{},\"angle\":{},\"knockback_growth\":{},\"weight_set_knockback\":{},\"base_knockback\":{},\"element\":{},\"shield_damage\":{},\"hit_grounded\":{},\"hit_aerial\":{}}}",
        hitbox.bone,
        hitbox.hit_group,
        hitbox.damage,
        hitbox.angle,
        hitbox.knockback_growth,
        hitbox.weight_set_knockback,
        hitbox.base_knockback,
        hitbox.element,
        hitbox.shield_damage,
        hitbox.hit_grounded,
        hitbox.hit_aerial
    )
}

fn render_rect_json(rect: RenderRect) -> String {
    format!(
        "{{\"x\":{},\"y\":{},\"width\":{},\"height\":{},\"color\":{{\"r\":{},\"g\":{},\"b\":{},\"a\":{}}}}}",
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.color.r,
        rect.color.g,
        rect.color.b,
        rect.color.a
    )
}

fn player_debug_json(
    frame: &RenderFrame,
    scene: &RenderScene,
    input: PlayerInput,
    index: usize,
) -> String {
    let ecb = scene.player_ecbs[index]
        .points
        .iter()
        .map(|point| format!("{{\"x\":{},\"y\":{}}}", point.x, point.y))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"index\":{},\"bits\":{},\"action_state_id\":{},\"source_action_key\":{},\"motion_state\":\"{:?}\",\"motion_state_alias\":{},\"state_frame\":{},\"position_x\":{},\"position_y\":{},\"velocity_x\":{},\"velocity_y\":{},\"damage_percent\":{},\"damage_percent_temp\":{},\"damage_applied\":{},\"damage_knockback\":{},\"damage_angle\":{},\"damage_element\":{},\"hitlag_frames\":{},\"damage_hitstun_frames\":{},\"profile_weight\":{},\"ecb\":[{}]}}",
        index,
        input.bits(),
        optional_action_state_id_json(frame.player_action_state_ids[index]),
        optional_source_action_key_json(frame.player_source_action_keys[index]),
        frame.player_motion_states[index],
        optional_motion_state_json(frame.player_motion_state_aliases[index]),
        frame.player_state_frames[index],
        frame.player_positions[index].x,
        frame.player_positions[index].y,
        frame.player_velocities[index].x,
        frame.player_velocities[index].y,
        frame.player_damage_percents[index],
        frame.player_damage_percent_temps[index],
        frame.player_damage_applied[index],
        frame.player_damage_knockbacks[index],
        frame.player_damage_angles[index],
        frame.player_damage_elements[index],
        frame.player_hitlag_frames[index],
        frame.player_damage_hitstun_frames[index],
        frame.player_profile_weights[index],
        ecb
    )
}

fn optional_motion_state_json(value: Option<MotionState>) -> String {
    value
        .map(|state| format!("\"{state:?}\""))
        .unwrap_or_else(|| "null".to_string())
}

fn player_rect(
    bottom_center: RenderPoint,
    transform: RenderTransform,
    sprite: LegacySpriteCue,
    visual: DolphinMoleVisualProfile,
    color: RenderColor,
) -> RenderRect {
    let source_size = sprite.source_size_px();
    let size = visual.scaled_size_units(source_size.width, source_size.height);
    let width = transform.core_length_to_screen(size.width);
    let height = transform.core_length_to_screen(size.height);

    RenderRect {
        x: bottom_center.x - width as i32 / 2,
        y: bottom_center.y - height as i32,
        width,
        height,
        color,
    }
}

fn active_player_rect(
    frame: &RenderFrame,
    index: usize,
    bottom_center: RenderPoint,
    transform: RenderTransform,
    sprite: LegacySpriteCue,
    visual: DolphinMoleVisualProfile,
    color: RenderColor,
) -> RenderRect {
    if !frame.player_participates(index) {
        return RenderRect {
            x: bottom_center.x,
            y: bottom_center.y,
            width: 0,
            height: 0,
            color: RenderColor::TRANSPARENT,
        };
    }

    player_rect(bottom_center, transform, sprite, visual, color)
}

fn player_shield(motion_state: MotionState, player: RenderRect) -> Option<RenderCircle> {
    if player.width == 0 || player.height == 0 {
        return None;
    }
    if !matches!(
        motion_state,
        MotionState::GuardOn
            | MotionState::Guard
            | MotionState::GuardOff
            | MotionState::GuardReflect
    ) {
        return None;
    }

    Some(RenderCircle {
        center: RenderPoint {
            x: player.x + player.width as i32 / 2,
            y: player.y + player.height as i32 / 2,
        },
        radius: player.width.max(player.height) * 58 / 100,
        color: RenderColor::SHIELD_BUBBLE,
    })
}

fn player_hitbox_pills(
    frame: &RenderFrame,
    index: usize,
    transform: RenderTransform,
) -> Vec<RenderCapsule> {
    if !frame.player_participates(index) {
        return Vec::new();
    }
    let source_action_key = source_action_key_for_player(frame, index);
    let source_frame = source_collision_capsule_frame_for_player(frame, index, source_action_key);
    let source_root = source_render_root_position(frame, index, source_action_key, source_frame);
    runtime_source_frame_capsules_ref(source_action_key, source_frame)
        .map(|source_capsules| source_capsules.hit_capsules.as_slice())
        .unwrap_or(&[])
        .iter()
        .copied()
        .map(|hitbox| {
            let visual_hitbox = RuntimeSourceCapsule {
                a: hitbox.b,
                ..hitbox
            };
            render_source_capsule(
                visual_hitbox,
                frame.player_source_positions[index].to_milli(),
                frame.player_model_facing(index),
                source_root,
                transform,
                RenderColor::HITBOX_PILL,
                source_frame_data::SOURCE_ARTIFACT_KIND,
            )
        })
        .collect()
}

fn player_hurtbox_pills(
    frame: &RenderFrame,
    index: usize,
    transform: RenderTransform,
) -> Vec<RenderCapsule> {
    if !frame.player_participates(index) {
        return Vec::new();
    }
    let source_action_key = source_hurt_action_key_for_player(frame, index);
    let source_frame =
        source_collision_hurt_capsule_frame_for_player(frame, index, source_action_key);
    let source_root = source_render_root_position(frame, index, source_action_key, source_frame);
    runtime_source_frame_capsules_ref(source_action_key, source_frame)
        .map(|source_capsules| source_capsules.hurt_capsules.as_slice())
        .unwrap_or(&[])
        .iter()
        .copied()
        .map(|hurtbox| {
            render_source_capsule(
                hurtbox,
                frame.player_source_positions[index].to_milli(),
                frame.player_model_facing(index),
                source_root,
                transform,
                RenderColor::HURTBOX_PILL,
                source_frame_data::SOURCE_ARTIFACT_KIND,
            )
        })
        .collect()
}

fn source_capsule_frame_for_player(
    frame: &RenderFrame,
    index: usize,
    source_action_key: Option<SourceActionKey>,
) -> u8 {
    let source_frame = frame.player_source_pose_frames[index].max(1);
    normalize_runtime_source_frame(source_action_key, source_frame)
}

fn source_collision_capsule_frame_for_player(
    frame: &RenderFrame,
    index: usize,
    source_action_key: Option<SourceActionKey>,
) -> u8 {
    let live_frame = frame.player_source_motion_anim_frames[index];
    let source_frame = if live_frame > 0.0 && live_frame.is_finite() {
        (live_frame.floor().clamp(0.0, f32::from(u8::MAX)) as u8).saturating_add(1)
    } else {
        frame.player_source_pose_frames[index].max(1)
    };
    normalize_runtime_source_frame(source_action_key, source_frame)
}

fn source_collision_hurt_capsule_frame_for_player(
    frame: &RenderFrame,
    index: usize,
    source_action_key: Option<SourceActionKey>,
) -> u8 {
    source_collision_capsule_frame_for_player(frame, index, source_action_key)
}

fn source_action_key_for_player(frame: &RenderFrame, index: usize) -> Option<SourceActionKey> {
    if let Some(action_state_id) = frame.player_source_pose_action_state_ids[index] {
        if let Some(key) = source_frame_data::source_action_key_for_action_state_id(action_state_id)
        {
            return Some(SourceActionKey::new(key));
        }
        return frame.player_source_pose_action_keys[index]
            .or(frame.player_source_action_keys[index]);
    }
    if let Some(key) = frame.player_source_pose_action_keys[index] {
        return Some(key);
    }
    if let Some(key) = frame.player_source_action_keys[index] {
        return Some(key);
    }
    if let Some(action_state_id) = frame.player_action_state_ids[index] {
        return source_frame_data::source_action_key_for_action_state_id(action_state_id)
            .map(SourceActionKey::new);
    }
    source_frame_data::source_action_key_for_state(frame.player_source_pose_motion_states[index])
        .map(SourceActionKey::new)
}

fn source_hurt_action_key_for_player(frame: &RenderFrame, index: usize) -> Option<SourceActionKey> {
    source_common_pose_action_key_for_player(frame, index)
        .or_else(|| source_action_key_for_player(frame, index))
}

fn source_common_pose_action_key_for_player(
    frame: &RenderFrame,
    index: usize,
) -> Option<SourceActionKey> {
    let pose_key = frame.player_source_pose_action_keys[index]?;
    let pose_state_key = source_frame_data::source_action_key_for_state(
        frame.player_source_pose_motion_states[index],
    )?;
    if pose_key.as_str() != pose_state_key {
        return None;
    }
    if frame.player_source_pose_action_state_ids[index]
        .is_some_and(source_action_state_id_is_source_only)
    {
        return None;
    }
    Some(pose_key)
}

fn source_action_state_id_is_source_only(action_state_id: MeleeActionStateId) -> bool {
    canonical_source_action_binding_for_runtime_id(action_state_id).is_some()
        || source_special_action_binding_for_runtime_id(action_state_id)
            .is_some_and(|binding| binding.motion_state.is_none())
}

fn source_render_root_position(
    frame: &RenderFrame,
    index: usize,
    source_action_key: Option<SourceActionKey>,
    source_frame: u8,
) -> Vec3 {
    runtime_source_frame_capsules_ref(source_action_key, source_frame)
        .map(|capsules| capsules.source_root_position)
        .or_else(|| {
            source_root_motion_position(frame.player_source_pose_motion_states[index], source_frame)
        })
        .unwrap_or(Vec3::new(0.0, 0.0, 0.0))
}

fn source_thrown_hurt_constraint(
    frame: &RenderFrame,
    player_index: usize,
) -> Option<SourceThrownHurtConstraint> {
    if !frame.player_source_x2226_b2[player_index] {
        return None;
    }
    let thrower_index = usize::from(frame.player_source_victim_indexes[player_index]?);
    if thrower_index >= frame.player_positions.len() {
        return None;
    }

    let victim_pose = source_live_capture_pose_for_player(frame, player_index)?;
    let thrower_pose = source_live_capture_pose_for_player(frame, thrower_index)?;
    Some(SourceThrownHurtConstraint {
        current_anchor: source_pose_point_add_root(
            frame.player_source_positions[thrower_index],
            thrower_pose.transn2,
        ),
        previous_anchor: source_pose_point_add_root(
            frame.player_source_previous_positions[thrower_index],
            thrower_pose.transn2,
        ),
        victim_xrotn: victim_pose.xrotn,
    })
}

fn source_live_capture_pose_for_player(
    frame: &RenderFrame,
    index: usize,
) -> Option<SourceCapturePose> {
    let source_action_key = source_action_key_for_player(frame, index);
    runtime_source_live_pose(
        source_action_key,
        frame.player_source_motion_anim_frames[index],
    )
    .map(runtime_source_capture_pose_from_live_sample)
    .or_else(|| {
        let source_frame = source_capsule_frame_for_player(frame, index, source_action_key);
        runtime_source_frame_capsules_ref(source_action_key, source_frame)
            .map(|capsules| capsules.capture_pose)
    })
}

fn source_pose_point_add_root(
    root_position: SourceVec2,
    point: SourcePosePoint,
) -> SourcePosePoint {
    SourcePosePoint {
        x: root_position.x + point.x,
        y: root_position.y + point.y,
        z: point.z,
    }
}

fn render_source_capsule(
    capsule: RuntimeSourceCapsule,
    root_position: Vec2,
    facing: i8,
    source_root: Vec3,
    transform: RenderTransform,
    color: RenderColor,
    source_artifact_kind: &'static str,
) -> RenderCapsule {
    let a_world = source_point_to_world_flattened(capsule.a, root_position, facing, source_root);
    let b_world = source_point_to_world_flattened(capsule.b, root_position, facing, source_root);
    RenderCapsule {
        a: transform.world_to_screen(a_world),
        b: transform.world_to_screen(b_world),
        radius: transform.core_length_to_screen(source_units_to_core_units(capsule.radius)),
        color,
        source: SourceRenderCapsule {
            a: SourceRenderPoint {
                x: f64::from(capsule.a.x),
                y: f64::from(capsule.a.y),
                z: f64::from(capsule.a.z),
            },
            b: SourceRenderPoint {
                x: f64::from(capsule.b.x),
                y: f64::from(capsule.b.y),
                z: f64::from(capsule.b.z),
            },
            radius: f64::from(capsule.radius),
        },
        source_space: SOURCE_SPACE_MELEE_XYZ,
        projected_view_kind: PROJECTED_VIEW_DERIVED_DEBUG,
        source_artifact_kind,
    }
}

fn source_point_to_world_flattened(
    point: RuntimeSourcePoint,
    root_position: Vec2,
    facing: i8,
    source_root: Vec3,
) -> Vec2 {
    let facing_sign = if facing < 0 { -1 } else { 1 };
    Vec2 {
        x: root_position.x + source_units_to_core_units(point.x - source_root.z) * facing_sign,
        y: root_position.y + source_units_to_core_units(point.y - source_root.y),
    }
}

fn source_hit_capsule_to_world_3d(
    capsule: RuntimeSourceCapsule,
    previous_root_position: SourceVec2,
    current_root_position: SourceVec2,
    facing: i8,
    previous_source_root: Vec3,
    current_source_root: Vec3,
) -> Capsule3 {
    Capsule3::new(
        source_point_to_world_3d(
            capsule.a,
            previous_root_position,
            facing,
            previous_source_root,
        ),
        source_point_to_world_3d(
            capsule.b,
            current_root_position,
            facing,
            current_source_root,
        ),
        source_units_to_core_units_f32(capsule.radius),
    )
}

fn source_hit_capsule_uses_previous_root(capsule: RuntimeSourceCapsule, source_frame: u8) -> bool {
    capsule
        .hitbox_lifecycle_id
        .map(|lifecycle_id| (lifecycle_id.get() >> 32) != u64::from(source_frame))
        .unwrap_or(capsule.a != capsule.b)
}

fn source_capsule_to_world_3d(
    capsule: RuntimeSourceCapsule,
    root_position: SourceVec2,
    facing: i8,
    source_root: Vec3,
) -> Capsule3 {
    Capsule3::new(
        source_point_to_world_3d(capsule.a, root_position, facing, source_root),
        source_point_to_world_3d(capsule.b, root_position, facing, source_root),
        source_units_to_core_units_f32(capsule.radius),
    )
}

fn source_capsule_to_constrained_xrotn_world_3d(
    capsule: RuntimeSourceCapsule,
    constrained_xrotn: SourcePosePoint,
    facing: i8,
    source_xrotn: SourcePosePoint,
) -> Capsule3 {
    Capsule3::new(
        source_point_to_constrained_xrotn_world_3d(
            capsule.a,
            constrained_xrotn,
            facing,
            source_xrotn,
        ),
        source_point_to_constrained_xrotn_world_3d(
            capsule.b,
            constrained_xrotn,
            facing,
            source_xrotn,
        ),
        source_units_to_core_units_f32(capsule.radius),
    )
}

fn source_point_to_constrained_xrotn_world_3d(
    point: RuntimeSourcePoint,
    constrained_xrotn: SourcePosePoint,
    facing: i8,
    source_xrotn: SourcePosePoint,
) -> Vec3 {
    let facing_sign = if facing < 0 { -1.0_f32 } else { 1.0_f32 };
    Vec3::new(
        source_units_to_core_units_f32(
            constrained_xrotn.x + (point.x - source_xrotn.x) * facing_sign,
        ),
        source_units_to_core_units_f32(constrained_xrotn.y + point.y - source_xrotn.y),
        source_units_to_core_units_f32(
            (constrained_xrotn.z + point.z - source_xrotn.z) * facing_sign,
        ),
    )
}

fn source_point_to_world_3d(
    point: RuntimeSourcePoint,
    root_position: SourceVec2,
    facing: i8,
    source_root: Vec3,
) -> Vec3 {
    let facing_sign = if facing < 0 { -1.0_f32 } else { 1.0_f32 };
    Vec3::new(
        source_units_to_core_units_f32(root_position.x + (point.x - source_root.z) * facing_sign),
        source_units_to_core_units_f32(root_position.y + point.y - source_root.y),
        source_units_to_core_units_f32((point.z - source_root.x) * facing_sign),
    )
}

fn source_units_to_core_units(value: f32) -> i32 {
    (value * 1_000.0).round() as i32
}

fn source_units_to_core_units_f32(value: f32) -> f32 {
    (value * 1_000.0) as f32
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RuntimeSourcePoint {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RuntimeSourceCapsule {
    id: u64,
    a: RuntimeSourcePoint,
    b: RuntimeSourcePoint,
    radius: f32,
    hurt_height: u8,
    hitbox_lifecycle_id: Option<SourceHitboxLifecycleId>,
    hitbox: Option<SourceHitboxAttributes>,
    hitbox_flags: SourceHitboxFlags,
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimeSourceFrameCapsules {
    source_frame: u8,
    source_root_position: Vec3,
    down_bound_pose: SourceDownBoundPose,
    capture_pose: SourceCapturePose,
    hit_capsules: Vec<RuntimeSourceCapsule>,
    hurt_capsules: Vec<RuntimeSourceCapsule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSourceActionScriptEvent {
    source_frame: u8,
    event: SourceActionScriptEvent,
}

#[derive(Clone)]
struct RuntimeSourceAction {
    source_action_key: SourceActionKey,
    total_frames: u8,
    loops: bool,
    frames: Vec<RuntimeSourceFrameCapsules>,
    live_pose_evaluator: RuntimeActionFrameEvaluator,
    script_events: Vec<RuntimeSourceActionScriptEvent>,
}

static RUNTIME_SOURCE_ACTION_CACHE: OnceLock<Result<Vec<RuntimeSourceAction>, String>> =
    OnceLock::new();

fn runtime_source_frame_capsules_ref(
    source_action_key: Option<SourceActionKey>,
    source_frame: u8,
) -> Option<&'static RuntimeSourceFrameCapsules> {
    let Some(source_action_key) = source_action_key else {
        return None;
    };
    let Ok(cache) = runtime_source_actions() else {
        return None;
    };
    cache
        .iter()
        .find(|action| action.source_action_key == source_action_key)
        .and_then(|action| action.frame(source_frame))
}

fn runtime_source_action_total_frames_for_action_state_id(
    action_state_id: MeleeActionStateId,
) -> Option<u8> {
    source_frame_data::source_action_key_for_action_state_id(action_state_id)
        .map(SourceActionKey::new)
        .and_then(runtime_source_action_total_frames)
}

fn runtime_source_pose_metadata_for_player(
    player: &PlayerState,
) -> Option<SourceActionPoseMetadata> {
    let source_action_key = source_action_key_for_player_state(player);
    let source_frame = source_collision_frame_for_player_state(player);
    let script_frame = source_frame;
    let live_pose = runtime_source_live_pose(source_action_key, player.source_motion_anim_frame);
    Some(SourceActionPoseMetadata {
        down_bound_pose: live_pose.map(runtime_source_down_bound_pose_from_live_sample),
        capture_pose: live_pose.map(runtime_source_capture_pose_from_live_sample),
        script_events: runtime_source_script_events(
            source_action_key,
            script_frame,
            player.motion_frame == 0,
        ),
        primary_hitbox: runtime_source_primary_hitbox(source_action_key, source_frame),
    })
}

fn runtime_source_primary_hitbox(
    source_action_key: Option<SourceActionKey>,
    source_frame: u8,
) -> Option<SourceHitboxAttributes> {
    let capsules = runtime_source_frame_capsules_ref(source_action_key, source_frame)?;
    capsules
        .hit_capsules
        .iter()
        .filter_map(|capsule| capsule.hitbox)
        .find(|hitbox| hitbox.hit_group == 0)
        .or_else(|| {
            capsules
                .hit_capsules
                .iter()
                .filter_map(|capsule| capsule.hitbox)
                .next()
        })
}

fn source_action_key_for_player_state(player: &PlayerState) -> Option<SourceActionKey> {
    if let Some(action_state_id) = player.melee_action_state_id {
        if let Some(key) = source_frame_data::source_action_key_for_action_state_id(action_state_id)
        {
            return Some(SourceActionKey::new(key));
        }
    }
    player.source_action_key
}

fn source_collision_frame_for_player_state(player: &PlayerState) -> u8 {
    let anim_frame = if player.source_motion_anim_frame.is_finite() {
        player
            .source_motion_anim_frame
            .floor()
            .clamp(0.0, f32::from(u8::MAX)) as u8
    } else {
        0
    };
    let frame = anim_frame.saturating_add(1);
    let source_action_key = source_action_key_for_player_state(player);
    normalize_runtime_source_frame(source_action_key, frame)
}

fn normalize_runtime_source_frame(
    source_action_key: Option<SourceActionKey>,
    source_frame: u8,
) -> u8 {
    let Some(source_action_key) = source_action_key else {
        return source_frame.max(1);
    };
    let Some(total_frames) = runtime_source_action_total_frames(source_action_key) else {
        return source_frame.max(1);
    };
    normalize_source_frame(
        source_frame,
        total_frames,
        runtime_source_action_loops(source_action_key),
    )
}

fn normalize_source_frame(source_frame: u8, total_frames: u8, loops: bool) -> u8 {
    if total_frames == 0 {
        return source_frame.max(1);
    }
    let one_based = source_frame.max(1);
    if loops {
        one_based
            .saturating_sub(1)
            .checked_rem(total_frames)
            .unwrap_or(0)
            .saturating_add(1)
    } else {
        one_based.min(total_frames)
    }
}

fn runtime_source_action_loops(source_action_key: SourceActionKey) -> bool {
    runtime_source_actions()
        .ok()
        .and_then(|actions| {
            actions
                .iter()
                .find(|action| action.source_action_key == source_action_key)
                .map(|action| action.loops)
        })
        .unwrap_or(false)
}

fn runtime_source_live_pose(
    source_action_key: Option<SourceActionKey>,
    anim_frame: f32,
) -> Option<RuntimeSourceLivePoseSample> {
    let source_action_key = source_action_key?;
    let cache = runtime_source_actions().ok()?;
    cache
        .iter()
        .find(|action| action.source_action_key == source_action_key)?
        .sample_live_pose(anim_frame)
}

fn runtime_source_down_bound_pose_from_live_sample(
    sample: RuntimeSourceLivePoseSample,
) -> SourceDownBoundPose {
    SourceDownBoundPose {
        hip_mtx_0_1: sample.down_bound_pose.hip_mtx_0_1,
        hip_mtx_0_2: sample.down_bound_pose.hip_mtx_0_2,
        hip_mtx_1_1: sample.down_bound_pose.hip_mtx_1_1,
        hip_mtx_1_2: sample.down_bound_pose.hip_mtx_1_2,
    }
}

fn runtime_source_capture_pose_from_live_sample(
    sample: RuntimeSourceLivePoseSample,
) -> SourceCapturePose {
    SourceCapturePose {
        capture_anchor: runtime_source_pose_point_from_sample(sample.capture_pose.capture_anchor),
        xrotn: runtime_source_pose_point_from_sample(sample.capture_pose.xrotn),
        transn2: runtime_source_pose_point_from_sample(sample.capture_pose.transn2),
        x1a70: runtime_source_pose_point_from_sample(sample.capture_pose.x1a70),
        thrown_hitbox: runtime_source_pose_point_from_sample(sample.capture_pose.thrown_hitbox),
        thrown_hitbox_scale: sample.capture_pose.thrown_hitbox_scale,
    }
}

fn runtime_source_script_events(
    source_action_key: Option<SourceActionKey>,
    source_frame: u8,
    include_frame_zero: bool,
) -> SourceActionScriptEvents {
    let Some(source_action_key) = source_action_key else {
        return SourceActionScriptEvents::empty();
    };
    let Ok(cache) = runtime_source_actions() else {
        return SourceActionScriptEvents::empty();
    };
    let Some(action) = cache
        .iter()
        .find(|action| action.source_action_key == source_action_key)
    else {
        return SourceActionScriptEvents::empty();
    };
    let mut events = SourceActionScriptEvents::empty();
    for event in action.script_events.iter().filter(|event| {
        event.source_frame == source_frame || (include_frame_zero && event.source_frame == 0)
    }) {
        let _ = events.push(event.event);
    }
    events
}

fn runtime_source_action_total_frames(source_action_key: SourceActionKey) -> Option<u8> {
    let cache = runtime_source_actions().ok()?;
    cache
        .iter()
        .find(|action| action.source_action_key == source_action_key)
        .map(|action| action.total_frames)
}

fn runtime_source_actions() -> Result<&'static Vec<RuntimeSourceAction>, String> {
    match RUNTIME_SOURCE_ACTION_CACHE.get_or_init(load_all_runtime_source_actions) {
        Ok(actions) => Ok(actions),
        Err(error) => Err(error.clone()),
    }
}

fn load_all_runtime_source_actions() -> Result<Vec<RuntimeSourceAction>, String> {
    let source_export = source_frame_data::runtime_source_export();
    let character = source_export.character.to_string();
    let source_character = source_export.source_character.map(str::to_string);
    let source_export_evaluator = RuntimeSourceExportEvaluator::from_export(source_export)?;
    decode_runtime_source_frame_capsules(source_frame_data::SOURCE_FRAME_CAPSULES_BYTES)?
        .into_iter()
        .map(|action| {
            let source_action_key_text = action.source_action_key;
            let live_pose_evaluator =
                source_export_evaluator.action_frame_evaluator(&FrameDataSampleOptions {
                    character: character.clone(),
                    source_character: source_character.clone(),
                    state: source_action_key_text.clone(),
                    frame: 1,
                })?;
            let source_action_key =
                SourceActionKey::new(leak_runtime_source_action_key(source_action_key_text));
            let frames = action
                .frames
                .into_iter()
                .map(runtime_source_frame_from_sample)
                .collect();
            let script_events = action
                .script_events
                .into_iter()
                .filter_map(runtime_source_script_event_from_sample)
                .collect();
            Ok(RuntimeSourceAction {
                source_action_key,
                total_frames: action.total_frames,
                loops: action.loops,
                frames,
                live_pose_evaluator,
                script_events,
            })
        })
        .collect()
}

fn leak_runtime_source_action_key(key: String) -> &'static str {
    Box::leak(key.into_boxed_str())
}

fn runtime_source_frame_from_sample(
    sample: RuntimeSourceFrameSample,
) -> RuntimeSourceFrameCapsules {
    RuntimeSourceFrameCapsules {
        source_frame: sample.source_frame,
        source_root_position: runtime_source_vec3_from_sample(sample.source_root_position),
        down_bound_pose: SourceDownBoundPose {
            hip_mtx_0_1: sample.down_bound_pose.hip_mtx_0_1,
            hip_mtx_0_2: sample.down_bound_pose.hip_mtx_0_2,
            hip_mtx_1_1: sample.down_bound_pose.hip_mtx_1_1,
            hip_mtx_1_2: sample.down_bound_pose.hip_mtx_1_2,
        },
        capture_pose: SourceCapturePose {
            capture_anchor: runtime_source_pose_point_from_sample(
                sample.capture_pose.capture_anchor,
            ),
            xrotn: runtime_source_pose_point_from_sample(sample.capture_pose.xrotn),
            transn2: runtime_source_pose_point_from_sample(sample.capture_pose.transn2),
            x1a70: runtime_source_pose_point_from_sample(sample.capture_pose.x1a70),
            thrown_hitbox: runtime_source_pose_point_from_sample(sample.capture_pose.thrown_hitbox),
            thrown_hitbox_scale: sample.capture_pose.thrown_hitbox_scale,
        },
        hit_capsules: sample
            .hit_capsules
            .into_iter()
            .map(runtime_source_capsule_from_sample)
            .collect(),
        hurt_capsules: sample
            .hurt_capsules
            .into_iter()
            .map(runtime_source_capsule_from_sample)
            .collect(),
    }
}

fn runtime_source_script_event_from_sample(
    event: FrameDataRuntimeSourceScriptEvent,
) -> Option<RuntimeSourceActionScriptEvent> {
    match event {
        FrameDataRuntimeSourceScriptEvent::SetCmdVar(event) => {
            Some(RuntimeSourceActionScriptEvent {
                source_frame: event.source_frame,
                event: SourceActionScriptEvent::SetCmdVar {
                    cmd_var: event.cmd_var,
                    value: event.value,
                },
            })
        }
        FrameDataRuntimeSourceScriptEvent::SetJabCombo(event) => {
            Some(RuntimeSourceActionScriptEvent {
                source_frame: event.source_frame,
                event: SourceActionScriptEvent::SetJabCombo {
                    disabled: event.disabled,
                },
            })
        }
        FrameDataRuntimeSourceScriptEvent::SetJabRapid(event) => {
            Some(RuntimeSourceActionScriptEvent {
                source_frame: event.source_frame,
                event: SourceActionScriptEvent::SetJabRapid { state: event.state },
            })
        }
        FrameDataRuntimeSourceScriptEvent::SetThrowFlag(event) => {
            Some(RuntimeSourceActionScriptEvent {
                source_frame: event.source_frame,
                event: SourceActionScriptEvent::SetThrowFlag {
                    hit_idx: event.hit_idx,
                    flag_bit: event.flag_bit,
                },
            })
        }
        FrameDataRuntimeSourceScriptEvent::SetThrowHitbox(event) => {
            Some(RuntimeSourceActionScriptEvent {
                source_frame: event.source_frame,
                event: SourceActionScriptEvent::SetThrowHitbox(event.hitbox),
            })
        }
    }
}

fn runtime_source_pose_point_from_sample(sample: FrameDataRuntimeSourcePoint) -> SourcePosePoint {
    SourcePosePoint {
        x: sample.x,
        y: sample.y,
        z: sample.z,
    }
}

fn runtime_source_vec3_from_sample(sample: FrameDataRuntimeSourcePoint) -> Vec3 {
    Vec3::new(sample.x, sample.y, sample.z)
}

fn runtime_source_capsule_from_sample(sample: RuntimeSourceCapsuleSample) -> RuntimeSourceCapsule {
    RuntimeSourceCapsule {
        id: sample.id,
        a: RuntimeSourcePoint {
            x: sample.a.x,
            y: sample.a.y,
            z: sample.a.z,
        },
        b: RuntimeSourcePoint {
            x: sample.b.x,
            y: sample.b.y,
            z: sample.b.z,
        },
        radius: sample.radius,
        hurt_height: sample.hurt_height,
        hitbox_lifecycle_id: sample.hitbox_lifecycle_id,
        hitbox: sample.hitbox,
        hitbox_flags: sample.hitbox_flags,
    }
}

impl RuntimeSourceAction {
    fn frame(&self, source_frame: u8) -> Option<&RuntimeSourceFrameCapsules> {
        let index = usize::from(source_frame.checked_sub(1)?);
        self.frames.get(index)
    }

    fn sample_live_pose(&self, anim_frame: f32) -> Option<RuntimeSourceLivePoseSample> {
        self.live_pose_evaluator.sample_live_pose(anim_frame).ok()
    }
}

fn match_intro_label(frame: &RenderFrame) -> Option<&'static str> {
    match frame.match_phase {
        mole_core::MatchPhase::Ready => Some("READY"),
        mole_core::MatchPhase::Go => Some("GO"),
        _ => None,
    }
}

fn entry_platform(
    frame: &RenderFrame,
    index: usize,
    transform: RenderTransform,
    _player: RenderRect,
) -> Option<RenderRect> {
    if !frame.player_participates(index) {
        return None;
    }
    if !matches!(
        frame.player_motion_states[index],
        MotionState::EntryStart
            | MotionState::EntryEnd
            | MotionState::Rebirth
            | MotionState::RebirthWait
    ) {
        return None;
    }

    let source_platform = frame.player_entry_platforms[index];
    let base_y = if matches!(
        frame.player_motion_states[index],
        MotionState::Rebirth | MotionState::RebirthWait
    ) {
        frame.player_positions[index].y
    } else {
        frame.player_entry_base_y[index]
    };
    let contact = transform.world_to_screen(Vec2 {
        x: frame.player_positions[index].x,
        y: base_y,
    });
    let width = transform
        .core_length_to_screen(source_units_to_milli(
            source_platform.accessory.mesh_bounds.width_x(),
        ))
        .max(1);
    let height = transform
        .core_length_to_screen(source_units_to_milli(
            source_platform.accessory.mesh_bounds.height_y().abs(),
        ))
        .max(4);

    Some(RenderRect {
        x: contact.x - width as i32 / 2,
        y: contact.y - height as i32,
        width,
        height,
        color: RenderColor::ENTRY_PLATFORM,
    })
}

fn entry_platform_wireframe(platform: Option<RenderRect>) -> Vec<RenderLine> {
    let Some(platform) = platform else {
        return Vec::new();
    };
    let left = platform.x;
    let right = platform.x + platform.width as i32;
    let top = platform.y;
    let bottom = platform.y + platform.height as i32;
    let color = platform.color;
    vec![
        RenderLine {
            start: RenderPoint { x: left, y: top },
            end: RenderPoint { x: right, y: top },
            color,
        },
        RenderLine {
            start: RenderPoint { x: right, y: top },
            end: RenderPoint {
                x: right,
                y: bottom,
            },
            color,
        },
        RenderLine {
            start: RenderPoint {
                x: right,
                y: bottom,
            },
            end: RenderPoint { x: left, y: bottom },
            color,
        },
        RenderLine {
            start: RenderPoint { x: left, y: bottom },
            end: RenderPoint { x: left, y: top },
            color,
        },
    ]
}

fn render_stage_surfaces(
    stage_profile: &StageProfile,
    transform: RenderTransform,
) -> Vec<RenderRect> {
    let mut surfaces = Vec::with_capacity(1 + stage_profile.soft_platforms.len());
    surfaces.push(render_stage_surface(&stage_profile.main_floor, transform));
    surfaces.extend(
        stage_profile
            .soft_platforms
            .iter()
            .filter(|surface| surface.left_x != surface.right_x)
            .map(|surface| render_stage_surface(surface, transform)),
    );
    surfaces
}

fn render_stage_collision_lines(
    stage_profile: &StageProfile,
    transform: RenderTransform,
) -> Vec<RenderLine> {
    let Some(stage) = stage_profile.melee_stage_profile() else {
        return Vec::new();
    };

    (0..stage.collision.lines.len())
        .filter_map(|index| {
            let line = stage.collision.scaled_line(index)?;
            Some(RenderLine {
                start: transform.world_to_screen(Vec2 {
                    x: line.x0_milli,
                    y: line.y0_milli,
                }),
                end: transform.world_to_screen(Vec2 {
                    x: line.x1_milli,
                    y: line.y1_milli,
                }),
                color: match line.kind {
                    mole_core::StageCollisionLineKind::Floor => RenderColor::STAGE,
                    mole_core::StageCollisionLineKind::SoftFloor => RenderColor::SOFT_PLATFORM,
                    mole_core::StageCollisionLineKind::Ceiling => RenderColor {
                        r: 132,
                        g: 145,
                        b: 168,
                        a: 255,
                    },
                    mole_core::StageCollisionLineKind::RightWall
                    | mole_core::StageCollisionLineKind::LeftWall => RenderColor {
                        r: 156,
                        g: 168,
                        b: 184,
                        a: 255,
                    },
                    mole_core::StageCollisionLineKind::Dynamic => RenderColor {
                        r: 208,
                        g: 130,
                        b: 130,
                        a: 255,
                    },
                },
            })
        })
        .collect()
}

fn render_stage_surface(surface: &StageSurface, transform: RenderTransform) -> RenderRect {
    let left = transform.world_to_screen(Vec2 {
        x: surface.left_x,
        y: surface.y,
    });
    let right = transform.world_to_screen(Vec2 {
        x: surface.right_x,
        y: surface.y,
    });

    RenderRect {
        x: left.x.min(right.x),
        y: left.y,
        width: (right.x - left.x).unsigned_abs().max(1),
        height: match surface.kind {
            StageSurfaceKind::Solid => 8,
            StageSurfaceKind::Soft => 6,
        },
        color: match surface.kind {
            StageSurfaceKind::Solid => RenderColor::STAGE,
            StageSurfaceKind::Soft => RenderColor::SOFT_PLATFORM,
        },
    }
}

fn player_ecb(frame: &RenderFrame, index: usize, transform: RenderTransform) -> RenderPolygon {
    if !frame.player_participates(index) {
        let point = transform.world_to_screen(frame.player_positions[index]);
        return RenderPolygon {
            points: [point; 4],
            color: RenderColor::TRANSPARENT,
        };
    }

    RenderPolygon {
        points: frame.player_ecbs[index]
            .points()
            .map(|point| transform.world_to_screen(point)),
        color: RenderColor::ECB,
    }
}

#[derive(Debug, Clone)]
pub struct ReplayCapture {
    log: ReplayLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayCaptureParseError {
    pub line: usize,
    pub message: &'static str,
}

impl ReplayCapture {
    pub fn new(initial: World) -> Self {
        Self {
            log: ReplayLog::new(initial),
        }
    }

    pub fn record_frame(&mut self, frame: Frame, inputs: [PlayerInput; 2], checksum: u64) {
        self.log.push(ReplayFrame {
            frame,
            inputs,
            checksum,
        });
    }

    pub const fn log(&self) -> &ReplayLog {
        &self.log
    }

    pub fn to_text(&self) -> String {
        let mut text = format!(
            "mole_replay_v1\ninitial_checksum={}\n",
            self.log.initial().checksum()
        );
        for frame in self.log.frames() {
            text.push_str(&format!(
                "frame={} p1_bits={} p2_bits={} checksum={}\n",
                frame.frame.0,
                frame.inputs[0].bits(),
                frame.inputs[1].bits(),
                frame.checksum
            ));
        }
        text
    }

    pub fn from_text(initial: World, text: &str) -> Result<ReplayLog, ReplayCaptureParseError> {
        let mut lines = text.lines().enumerate();
        match lines.next() {
            Some((_, "mole_replay_v1")) => {}
            _ => {
                return Err(ReplayCaptureParseError {
                    line: 1,
                    message: "missing replay header",
                });
            }
        }

        let Some((line_index, initial_line)) = lines.next() else {
            return Err(ReplayCaptureParseError {
                line: 2,
                message: "missing initial checksum",
            });
        };
        let initial_checksum = parse_prefixed_u64(
            line_index + 1,
            initial_line,
            "initial_checksum=",
            "invalid initial checksum",
        )?;
        if initial_checksum != initial.checksum() {
            return Err(ReplayCaptureParseError {
                line: line_index + 1,
                message: "initial checksum mismatch",
            });
        }

        let mut log = ReplayLog::new(initial);
        for (line_index, line) in lines {
            if line.trim().is_empty() {
                continue;
            }
            log.push(parse_replay_frame(line_index + 1, line)?);
        }
        Ok(log)
    }
}

pub fn native_replay_path(frames: u32) -> PathBuf {
    PathBuf::from("debug")
        .join("replays")
        .join(format!("native-replay-{frames}-frames.mrep"))
}

pub fn write_replay_capture(path: impl AsRef<Path>, capture: &ReplayCapture) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, capture.to_text())
}

fn parse_replay_frame(
    line_number: usize,
    line: &str,
) -> Result<ReplayFrame, ReplayCaptureParseError> {
    let mut frame = None;
    let mut p1_bits = None;
    let mut p2_bits = None;
    let mut checksum = None;

    for token in line.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            return Err(ReplayCaptureParseError {
                line: line_number,
                message: "invalid replay token",
            });
        };
        match key {
            "frame" => frame = Some(parse_u32(line_number, value, "invalid frame")?),
            "p1_bits" => p1_bits = Some(parse_u64(line_number, value, "invalid p1 bits")?),
            "p2_bits" => p2_bits = Some(parse_u64(line_number, value, "invalid p2 bits")?),
            "checksum" => checksum = Some(parse_u64(line_number, value, "invalid checksum")?),
            _ => {
                return Err(ReplayCaptureParseError {
                    line: line_number,
                    message: "unknown replay field",
                });
            }
        }
    }

    Ok(ReplayFrame {
        frame: Frame(frame.ok_or(ReplayCaptureParseError {
            line: line_number,
            message: "missing frame",
        })?),
        inputs: [
            PlayerInput::from_bits(p1_bits.ok_or(ReplayCaptureParseError {
                line: line_number,
                message: "missing p1 bits",
            })?),
            PlayerInput::from_bits(p2_bits.ok_or(ReplayCaptureParseError {
                line: line_number,
                message: "missing p2 bits",
            })?),
        ],
        checksum: checksum.ok_or(ReplayCaptureParseError {
            line: line_number,
            message: "missing checksum",
        })?,
    })
}

fn parse_prefixed_u64(
    line_number: usize,
    line: &str,
    prefix: &'static str,
    message: &'static str,
) -> Result<u64, ReplayCaptureParseError> {
    let Some(value) = line.strip_prefix(prefix) else {
        return Err(ReplayCaptureParseError {
            line: line_number,
            message,
        });
    };
    parse_u64(line_number, value, message)
}

fn parse_u32(
    line_number: usize,
    value: &str,
    message: &'static str,
) -> Result<u32, ReplayCaptureParseError> {
    value.parse().map_err(|_| ReplayCaptureParseError {
        line: line_number,
        message,
    })
}

fn parse_u64(
    line_number: usize,
    value: &str,
    message: &'static str,
) -> Result<u64, ReplayCaptureParseError> {
    value.parse().map_err(|_| ReplayCaptureParseError {
        line: line_number,
        message,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpRuntimeConfig {
    pub local_addr: SocketAddr,
    pub peer_addr: SocketAddr,
    pub player_index: u8,
}

impl UdpRuntimeConfig {
    pub fn from_args(args: &[String]) -> Result<Self, String> {
        let local_addr = required_arg(args, "--local-addr")?
            .parse()
            .map_err(|error| format!("invalid --local-addr: {error}"))?;
        let peer_addr = required_arg(args, "--peer-addr")?
            .parse()
            .map_err(|error| format!("invalid --peer-addr: {error}"))?;
        let player_index = optional_arg(args, "--player-index")
            .map(|value| {
                value
                    .parse::<u8>()
                    .map_err(|error| format!("invalid --player-index: {error}"))
            })
            .transpose()?
            .unwrap_or(0);

        if player_index > 1 {
            return Err("--player-index must be 0 or 1".to_string());
        }

        Ok(Self {
            local_addr,
            peer_addr,
            player_index,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UdpRuntimeStats {
    pub sent_packets: u32,
    pub received_packets: u32,
    pub duplicate_packets: u32,
    pub unsupported_packets: u32,
    pub missing_remote_frames: u32,
    pub rollback_corrections: u32,
    pub last_remote_frame: Option<Frame>,
    pub last_remote_checksum: Option<u64>,
    pub last_remote_sequence: Option<u32>,
    pub last_acked_sequence: Option<u32>,
    pub last_rtt_frames: Option<u32>,
}

impl UdpRuntimeStats {
    pub fn record_sent(&mut self) {
        self.sent_packets = self.sent_packets.saturating_add(1);
    }

    pub fn record_accept(&mut self, result: PacketAcceptResult, packet: InputPacket) {
        self.record_accept_at(packet.frame, result, packet);
    }

    pub fn record_accept_at(
        &mut self,
        local_frame: Frame,
        result: PacketAcceptResult,
        packet: InputPacket,
    ) {
        match result {
            PacketAcceptResult::Accepted => {
                self.received_packets = self.received_packets.saturating_add(1);
                self.last_remote_frame = Some(
                    self.last_remote_frame
                        .map(|frame| Frame(frame.0.max(packet.frame.0)))
                        .unwrap_or(packet.frame),
                );
                self.last_remote_checksum = Some(packet.checksum);
                self.last_remote_sequence = Some(
                    self.last_remote_sequence
                        .map(|sequence| sequence.max(packet.sequence))
                        .unwrap_or(packet.sequence),
                );
                let newest_acked_sequence = self
                    .last_acked_sequence
                    .map(|sequence| sequence.max(packet.ack_sequence))
                    .unwrap_or(packet.ack_sequence);
                self.last_acked_sequence = Some(newest_acked_sequence);
                self.last_rtt_frames = Some(local_frame.0.saturating_sub(newest_acked_sequence));
            }
            PacketAcceptResult::Duplicate => {
                self.duplicate_packets = self.duplicate_packets.saturating_add(1);
            }
            PacketAcceptResult::UnsupportedVersion => {
                self.unsupported_packets = self.unsupported_packets.saturating_add(1);
            }
        }
    }

    pub fn record_missing_remote_frame(&mut self) {
        self.missing_remote_frames = self.missing_remote_frames.saturating_add(1);
    }

    pub fn record_rollback_correction(&mut self) {
        self.rollback_corrections = self.rollback_corrections.saturating_add(1);
    }
}

fn required_arg(args: &[String], flag: &'static str) -> Result<String, String> {
    optional_arg(args, flag).ok_or_else(|| format!("{flag} is required"))
}

fn optional_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}
