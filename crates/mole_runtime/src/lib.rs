use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::{fs, io};

use mole_core::{
    step_world, Frame, GameCubePadStatus, MeleeInputFacts, MotionState, PlayerInput, Vec2, World,
    WorldSnapshot, TICK_NANOS,
};
use mole_replay::{ReplayFrame, ReplayLog};
use mole_transport::{InputPacket, PacketAcceptResult};

pub mod assets;
pub use assets::{
    legacy_animation_for_motion_state, legacy_animation_spec, LegacyAnimationKey,
    LegacyAnimationSpec, LegacySpriteCue, LEGACY_DOLPHIN_MOLE_ANIMATIONS,
};

#[cfg(feature = "sdl")]
pub mod sdl_input;

#[cfg(feature = "sdl")]
pub use sdl_input::{configure_sdl_controller_hints, SdlInputSource};

pub mod readout;
pub use readout::{ButtonReadout, InputReadout, MeleeReadout, PlayerReadout};

pub mod wup_input;
pub use wup_input::{map_wup_ports_to_player_inputs, parse_wup_report, WupInputMapper, WupPort};

#[cfg(feature = "wup")]
pub use wup_input::WupInputSource;

const DEFAULT_MAX_TICKS_PER_UPDATE: u32 = 5;
const AXIS_DEADZONE: i16 = 8_000;
const PLAYER_RENDER_WIDTH: u32 = 48;
const PLAYER_RENDER_HEIGHT: u32 = 72;
const WORLD_TO_SCREEN_SCALE: i32 = 10;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderFrame {
    pub frame: Frame,
    pub player_positions: [Vec2; 2],
    pub player_facings: [i8; 2],
    pub player_motion_states: [MotionState; 2],
    pub player_state_frames: [u8; 2],
    pub player_animation_frames: [u8; 2],
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
            player_positions: [snapshot.players[0].position, snapshot.players[1].position],
            player_facings: [snapshot.players[0].facing, snapshot.players[1].facing],
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
            player_debug_input_facts: [
                snapshot.players[0].debug_input_facts,
                snapshot.players[1].debug_input_facts,
            ],
            checksum: snapshot.checksum,
        }
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
    pub const BACKGROUND: Self = Self {
        r: 17,
        g: 19,
        b: 24,
        a: 255,
    };
    pub const STAGE: Self = Self {
        r: 180,
        g: 187,
        b: 196,
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
pub struct RenderScene {
    pub background: RenderColor,
    pub stage: RenderRect,
    pub players: [RenderRect; 2],
    pub player_sprites: [LegacySpriteCue; 2],
}

impl RenderScene {
    pub fn from_frame(frame: &RenderFrame, viewport_width: u32, viewport_height: u32) -> Self {
        let center_x = viewport_width as i32 / 2;
        let ground_y = viewport_height as i32 * 3 / 4;
        let player_colors = [RenderColor::PLAYER_ONE, RenderColor::PLAYER_TWO];

        Self {
            background: RenderColor::BACKGROUND,
            stage: RenderRect {
                x: viewport_width as i32 / 8,
                y: ground_y,
                width: viewport_width * 3 / 4,
                height: 8,
                color: RenderColor::STAGE,
            },
            players: [
                player_rect(frame, 0, center_x, ground_y, player_colors[0]),
                player_rect(frame, 1, center_x, ground_y, player_colors[1]),
            ],
            player_sprites: [
                LegacySpriteCue::for_player(
                    frame.player_motion_states[0],
                    frame.player_state_frames[0],
                    frame.player_facings[0],
                ),
                LegacySpriteCue::for_player(
                    frame.player_motion_states[1],
                    frame.player_state_frames[1],
                    frame.player_facings[1],
                ),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugOverlay {
    pub lines: Vec<String>,
}

impl DebugOverlay {
    pub fn from_frame(frame: &RenderFrame) -> Self {
        Self {
            lines: vec![
                format!("FRAME {}", frame.frame.0),
                format!("CHECKSUM {}", frame.checksum),
            ],
        }
    }

    pub fn from_frame_with_udp_stats(frame: &RenderFrame, stats: &UdpRuntimeStats) -> Self {
        let mut overlay = Self::from_frame(frame);
        overlay.lines.push(format!(
            "UDP TX {} RX {} DUP {} MISS {}",
            stats.sent_packets,
            stats.received_packets,
            stats.duplicate_packets,
            stats.missing_remote_frames
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

fn player_rect(
    frame: &RenderFrame,
    index: usize,
    center_x: i32,
    ground_y: i32,
    color: RenderColor,
) -> RenderRect {
    let position = frame.player_positions[index];

    RenderRect {
        x: center_x + position.x / WORLD_TO_SCREEN_SCALE - PLAYER_RENDER_WIDTH as i32 / 2,
        y: ground_y - position.y / WORLD_TO_SCREEN_SCALE - PLAYER_RENDER_HEIGHT as i32,
        width: PLAYER_RENDER_WIDTH,
        height: PLAYER_RENDER_HEIGHT,
        color,
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
                self.last_remote_frame = Some(packet.frame);
                self.last_remote_checksum = Some(packet.checksum);
                self.last_remote_sequence = Some(packet.sequence);
                self.last_acked_sequence = Some(packet.ack_sequence);
                self.last_rtt_frames = Some(local_frame.0.saturating_sub(packet.ack_sequence));
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
}

fn required_arg(args: &[String], flag: &'static str) -> Result<String, String> {
    optional_arg(args, flag).ok_or_else(|| format!("{flag} is required"))
}

fn optional_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}
