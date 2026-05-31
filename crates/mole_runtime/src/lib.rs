use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::{fs, io};

use mole_core::{
    step_world, EcbDiamond, Frame, GameCubePadStatus, MeleeInputFacts, MotionState, PlayerInput,
    StageProfile, StageSurface, StageSurfaceKind, Vec2, World, WorldSnapshot, TICK_NANOS,
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
    slippi_core_report_path, write_slippi_core_report, SlippiCoreComparison,
    SlippiCoreComparisonConfig, SlippiCoreComparisonMode, SlippiCoreDiagnosticError,
    SlippiCoreMismatch,
};

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
    pub player_velocities: [Vec2; 2],
    pub player_ecbs: [EcbDiamond; 2],
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
            player_velocities: [snapshot.players[0].velocity, snapshot.players[1].velocity],
            player_ecbs: [
                snapshot.players[0].active_ecb,
                snapshot.players[1].active_ecb,
            ],
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

impl RenderTransform {
    pub fn battlefield_camera(viewport_width: u32, viewport_height: u32) -> Self {
        let stage = StageProfile::battlefield_test();
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
        let numerator = value as i64 * self.pixels_per_core_unit_milli as i64;
        if numerator >= 0 {
            ((numerator + CORE_TO_SCREEN_SCALE_DENOMINATOR / 2) / CORE_TO_SCREEN_SCALE_DENOMINATOR)
                as i32
        } else {
            ((numerator - CORE_TO_SCREEN_SCALE_DENOMINATOR / 2) / CORE_TO_SCREEN_SCALE_DENOMINATOR)
                as i32
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderScene {
    pub background: RenderColor,
    pub background_image: RenderImage,
    pub transform: RenderTransform,
    pub stage: RenderRect,
    pub stage_surfaces: Vec<RenderRect>,
    pub players: [RenderRect; 2],
    /// Screen-space projection of the fighter's simulation/root position.
    /// Visual assets stay anchored here while the active ECB polygon is drawn
    /// from core-owned collision data.
    pub player_contact_points: [RenderPoint; 2],
    pub player_ecbs: [RenderPolygon; 2],
    pub player_sprites: [LegacySpriteCue; 2],
    pub player_shields: [Option<RenderCircle>; 2],
}

impl RenderScene {
    pub fn from_frame(frame: &RenderFrame, viewport_width: u32, viewport_height: u32) -> Self {
        let transform = RenderTransform::battlefield_camera(viewport_width, viewport_height);
        let player_colors = [RenderColor::PLAYER_ONE, RenderColor::PLAYER_TWO];
        let stage_profile = StageProfile::battlefield_test();
        let stage_surfaces = render_stage_surfaces(&stage_profile, transform);
        let player_sprites = [
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
        ];
        let visual = DolphinMoleVisualProfile::default();
        let player_contact_points = [
            transform.world_to_screen(frame.player_positions[0]),
            transform.world_to_screen(frame.player_positions[1]),
        ];
        let players = [
            player_rect(
                player_contact_points[0],
                transform,
                player_sprites[0],
                visual,
                player_colors[0],
            ),
            player_rect(
                player_contact_points[1],
                transform,
                player_sprites[1],
                visual,
                player_colors[1],
            ),
        ];

        Self {
            background: RenderColor::BACKGROUND,
            background_image: RenderImage {
                relative_path: "background.png",
                rect: RenderRect {
                    x: 0,
                    y: 0,
                    width: viewport_width,
                    height: viewport_height,
                    color: RenderColor::BACKGROUND,
                },
            },
            transform,
            stage: stage_surfaces[0],
            stage_surfaces,
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
        }
    }
}

pub fn project_asset_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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

fn player_state_overlay_line(frame: &RenderFrame, index: usize) -> String {
    format!(
        "P{} {} F{}",
        index + 1,
        format!("{:?}", frame.player_motion_states[index]).to_ascii_uppercase(),
        frame.player_state_frames[index]
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

        Self {
            json_line: format!(
                "{{\"frame\":{},\"checksum\":{},\"p1_bits\":{},\"p2_bits\":{},\"players\":[{},{}],\"render_transform\":{{\"center_x\":{},\"ground_y\":{},\"pixels_per_core_unit_milli\":{}}}}}",
                frame.frame.0,
                frame.checksum,
                inputs[0].bits(),
                inputs[1].bits(),
                player_logs[0],
                player_logs[1],
                scene.transform.center_x,
                scene.transform.ground_y,
                scene.transform.pixels_per_core_unit_milli
            ),
        }
    }

    pub fn to_json_line(&self) -> String {
        self.json_line.clone()
    }
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
        "{{\"index\":{},\"bits\":{},\"motion_state\":\"{:?}\",\"state_frame\":{},\"position_x\":{},\"position_y\":{},\"velocity_x\":{},\"velocity_y\":{},\"ecb\":[{}]}}",
        index,
        input.bits(),
        frame.player_motion_states[index],
        frame.player_state_frames[index],
        frame.player_positions[index].x,
        frame.player_positions[index].y,
        frame.player_velocities[index].x,
        frame.player_velocities[index].y,
        ecb
    )
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

fn player_shield(motion_state: MotionState, player: RenderRect) -> Option<RenderCircle> {
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
            .map(|surface| render_stage_surface(surface, transform)),
    );
    surfaces
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
