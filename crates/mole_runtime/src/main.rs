#[cfg(all(feature = "sdl", feature = "wup"))]
use std::collections::{HashMap, VecDeque};
use std::fs;
#[cfg(all(feature = "sdl", feature = "wup"))]
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::path::{Path, PathBuf};
#[cfg(all(feature = "sdl", feature = "wup"))]
use std::sync::mpsc;

#[cfg(feature = "wup")]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mole_core::{Frame, PlayerInput};
#[cfg(all(feature = "sdl", feature = "wup"))]
use mole_rollback::{RollbackSession, SlippiDelayedInput, SlippiInputDelayBuffer};
#[cfg(all(feature = "sdl", feature = "wup"))]
use mole_transport::InputPacketDatagram;
use mole_transport::{InputPacket, InputPacketInbox, UdpTransport};

#[cfg(feature = "sdl")]
use mole_runtime::configure_sdl_controller_hints;

#[cfg(all(feature = "sdl", feature = "wup"))]
use mole_signaling::{
    DirectEndpoint, FriendConnectPair, RemoteDirectEndpoint, SupabaseRealtimeConfig,
};

#[cfg(all(feature = "sdl", feature = "wup"))]
use mole_transport::discover_public_udp_endpoint;

#[cfg(all(feature = "sdl", feature = "wup"))]
use mole_runtime::{
    BoundedNetplayLogger, DebugOverlay, NetplayLogEvent, NetplayLogRole, RenderCapsule,
    RenderColor, RenderPolygon, RenderRect, RenderScene, SdlInputSource,
};

#[cfg(all(feature = "sdl", feature = "wup"))]
use sdl3::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    pixels::PixelFormat,
    rect::Point,
    render::{BlendMode, FRect, Texture, TextureCreator, WindowCanvas},
    video::WindowContext,
};

#[cfg(feature = "wup")]
use mole_runtime::WupInputSource;

#[cfg(all(feature = "sdl", feature = "wup"))]
mod wup_monitor;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let frames = parse_frames(&args);
    let replay_path = parse_replay_path(&args, frames);
    #[cfg(feature = "sdl")]
    let frame_log = has_flag(&args, "--frame-log");
    #[cfg(feature = "sdl")]
    let input_trace = has_flag(&args, "--input-trace");
    #[cfg(feature = "sdl")]
    let timing = has_flag(&args, "--timing");
    #[cfg(feature = "sdl")]
    let frame_cap_enabled = !has_flag(&args, "--no-frame-cap");
    #[cfg(feature = "wup")]
    let ucf_enabled = parse_ucf_enabled(&args);
    #[cfg(all(feature = "sdl", feature = "wup"))]
    let netplay_delay_frames = parse_netplay_delay_frames(&args);
    #[cfg(all(feature = "sdl", feature = "wup"))]
    let friend_code_override = parse_friend_code_override(&args);
    #[cfg(all(feature = "sdl", feature = "wup"))]
    let visual_connect_code = parse_friend_connect_visual_join_code(&args);
    #[cfg(all(feature = "sdl", feature = "wup"))]
    let auto_start_friend_connect = has_flag(&args, "--auto-start");
    #[cfg(all(feature = "sdl", feature = "wup"))]
    let friend_connect_window_offset = parse_friend_connect_window_offset(&args);
    #[cfg(all(feature = "sdl", feature = "wup"))]
    let friend_connect_local_udp_addr = match parse_friend_connect_local_udp_addr(&args) {
        Ok(addr) => addr,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    #[cfg(all(feature = "sdl", feature = "wup"))]
    match parse_headless_friend_peer_config(&args) {
        Ok(Some(config)) => {
            if let Err(error) = run_friend_connect_headless_peer(config) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    if has_flag(&args, "--friend-connect") {
        if let Err(error) = run_friend_connect_sdl(
            frames,
            replay_path.as_deref(),
            ucf_enabled,
            netplay_delay_frames,
            friend_code_override.as_deref(),
            visual_connect_code.as_deref(),
            auto_start_friend_connect,
            friend_connect_window_offset,
            friend_connect_local_udp_addr,
        ) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(all(feature = "sdl", feature = "wup")))]
    if has_flag(&args, "--friend-connect") || has_flag(&args, "--friend-connect-headless-peer") {
        eprintln!(
            "Friend Connect needs: cargo run -p mole_runtime --features \"sdl wup\" -- --friend-connect"
        );
        std::process::exit(2);
    }

    if let Some(path) = value_after(&args, "--compare-slippi") {
        let frame_limit = has_flag(&args, "--frames").then_some(frames as usize);
        let report_path = value_after(&args, "--slippi-core-report").map(PathBuf::from);
        if let Some(trace_report_path) = value_after(&args, "--slippi-core-trace-report") {
            match run_slippi_core_trace(
                Path::new(&path),
                Path::new(&trace_report_path),
                frame_limit,
                parse_slippi_trace_player(&args),
                parse_i32_after(&args, "--slippi-trace-start").unwrap_or_default(),
                parse_i32_after(&args, "--slippi-trace-end").unwrap_or_default(),
            ) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        let comparison_mode = parse_slippi_compare_mode(&args);
        match run_slippi_core_compare(
            Path::new(&path),
            report_path.as_deref(),
            frame_limit,
            comparison_mode,
        ) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }

    if has_flag(&args, "--udp") {
        #[cfg(feature = "sdl")]
        if has_flag(&args, "--sdl") {
            match mole_runtime::UdpRuntimeConfig::from_args(&args).and_then(|config| {
                run_udp_sdl(
                    frames,
                    config,
                    replay_path.as_deref(),
                    frame_log,
                    input_trace,
                    ucf_enabled,
                )
            }) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }

        #[cfg(not(feature = "sdl"))]
        if has_flag(&args, "--sdl") {
            eprintln!(
                "UDP SDL runtime needs native WUP gameplay input: cargo run -p mole_runtime --features \"sdl wup\" -- --udp --sdl --local-addr <addr> --peer-addr <addr>"
            );
            std::process::exit(2);
        }

        match mole_runtime::UdpRuntimeConfig::from_args(&args)
            .and_then(|config| run_udp_headless(frames, config, replay_path.as_deref()))
        {
            Ok(()) => {}
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }

    #[cfg(feature = "sdl")]
    if has_flag(&args, "--list-inputs") {
        if let Err(error) = list_sdl_inputs() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "sdl"))]
    if has_flag(&args, "--list-inputs") {
        eprintln!(
            "SDL3 input listing needs: cargo run -p mole_runtime --features sdl -- --list-inputs"
        );
        std::process::exit(2);
    }

    #[cfg(feature = "wup")]
    if has_flag(&args, "--check-wup") {
        if let Err(error) = check_wup_native() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "wup"))]
    if has_flag(&args, "--check-wup") {
        eprintln!(
            "Native WUP input check needs: cargo run -p mole_runtime --features wup -- --check-wup"
        );
        std::process::exit(2);
    }

    #[cfg(feature = "wup")]
    if has_flag(&args, "--stream-wup") {
        let stream_frames = if has_flag(&args, "--frames") {
            frames
        } else {
            u32::MAX
        };
        if let Err(error) = stream_wup_native(stream_frames) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "wup"))]
    if has_flag(&args, "--stream-wup") {
        eprintln!(
            "Native WUP stream needs: cargo run -p mole_runtime --features wup -- --stream-wup"
        );
        std::process::exit(2);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    if has_flag(&args, "--monitor-wup") {
        let monitor_frames = if has_flag(&args, "--frames") {
            frames
        } else {
            u32::MAX
        };
        if let Err(error) = wup_monitor::run_wup_monitor(monitor_frames) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(all(feature = "sdl", feature = "wup")))]
    if has_flag(&args, "--monitor-wup") {
        eprintln!(
            "Native WUP monitor needs: cargo run -p mole_runtime --features \"sdl wup\" -- --monitor-wup"
        );
        std::process::exit(2);
    }

    #[cfg(feature = "wup")]
    if has_flag(&args, "--wup") {
        if let Err(error) = run_wup_smoke(frames, replay_path.as_deref(), ucf_enabled) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "wup"))]
    if has_flag(&args, "--wup") {
        eprintln!("Native WUP runtime needs: cargo run -p mole_runtime --features wup -- --wup");
        std::process::exit(2);
    }

    #[cfg(feature = "sdl")]
    if has_flag(&args, "--sdl") {
        if let Err(error) = run_sdl_smoke(
            frames,
            replay_path.as_deref(),
            frame_log,
            input_trace,
            timing,
            frame_cap_enabled,
            #[cfg(feature = "wup")]
            ucf_enabled,
        ) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "sdl"))]
    if has_flag(&args, "--sdl") {
        eprintln!(
            "The SDL3 native WUP runtime needs: cargo run -p mole_runtime --features \"sdl wup\" -- --sdl"
        );
        std::process::exit(2);
    }

    if let Err(error) = run_headless(frames, replay_path.as_deref()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_slippi_core_compare(
    path: &Path,
    report_path: Option<&Path>,
    frame_limit: Option<usize>,
    comparison_mode: mole_runtime::SlippiCoreComparisonMode,
) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let config = mole_runtime::SlippiCoreComparisonConfig {
        max_frames: frame_limit,
        ..Default::default()
    };
    let comparison = match comparison_mode {
        mole_runtime::SlippiCoreComparisonMode::SeededPreFrameDiagnostic => {
            mole_runtime::compare_slippi_export_with_core(&text, config)
        }
        mole_runtime::SlippiCoreComparisonMode::SequentialMatchStart => {
            mole_runtime::compare_slippi_export_from_match_start_with_core(&text, config)
        }
    }
    .map_err(|error| error.to_string())?;
    let output_path = report_path.map(PathBuf::from).unwrap_or_else(|| {
        comparison
            .source_replay_path
            .as_deref()
            .map(mole_runtime::slippi_core_report_path)
            .unwrap_or_else(|| mole_runtime::slippi_core_report_path(path))
    });
    mole_runtime::write_slippi_core_report(&output_path, &comparison)
        .map_err(|error| error.to_string())?;
    println!("slippi_core_report={}", output_path.display());
    println!(
        "frames_compared={} state_mismatches={} unsupported_states={}",
        comparison.frames_compared,
        comparison.state_mismatch_count,
        comparison.unsupported_state_count
    );
    Ok(())
}

fn run_slippi_core_trace(
    path: &Path,
    report_path: &Path,
    frame_limit: Option<usize>,
    player_index: usize,
    source_frame_start: i32,
    source_frame_end: i32,
) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let trace = mole_runtime::trace_slippi_export_from_match_start_with_core(
        &text,
        mole_runtime::SlippiCoreTraceConfig {
            player_index,
            source_frame_start,
            source_frame_end,
            max_frames: frame_limit,
        },
    )
    .map_err(|error| error.to_string())?;
    mole_runtime::write_slippi_core_trace_report(report_path, &trace)
        .map_err(|error| error.to_string())?;
    println!("slippi_core_trace_report={}", report_path.display());
    println!("trace_rows={}", trace.rows.len());
    Ok(())
}

fn run_headless(frames: u32, replay_path: Option<&Path>) -> Result<(), String> {
    mole_runtime::preload_runtime_source_frame_data()?;
    let initial = mole_runtime::default_play_world();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));
    let inputs = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..frames {
        mole_runtime::step_world_with_source_collisions(&mut world, Frame(frame), &inputs);
        if let Some(capture) = replay_capture.as_mut() {
            capture.record_frame(Frame(frame), inputs, world.checksum());
        }
    }

    if let (Some(path), Some(capture)) = (replay_path, replay_capture.as_ref()) {
        mole_runtime::write_replay_capture(path, capture).map_err(|error| error.to_string())?;
        println!("replay_path={}", path.display());
    }

    println!(
        "final_frame={} checksum={}",
        world.frame().0,
        world.checksum()
    );
    Ok(())
}

fn run_udp_headless(
    frames: u32,
    config: mole_runtime::UdpRuntimeConfig,
    replay_path: Option<&Path>,
) -> Result<(), String> {
    mole_runtime::preload_runtime_source_frame_data()?;
    let transport = UdpTransport::bind(config.local_addr, config.peer_addr)
        .map_err(|error| error.to_string())?;
    let initial = mole_runtime::default_play_world();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));
    let mut inbox = InputPacketInbox::default();
    let mut stats = mole_runtime::UdpRuntimeStats::default();
    let remote_player = 1 - config.player_index;

    for frame_number in 0..frames {
        let frame = Frame(frame_number);
        drain_udp_packets(&transport, &mut inbox, &mut stats, frame)?;
        let mut inputs = [PlayerInput::neutral(), PlayerInput::neutral()];
        if let Some(remote_input) = inbox.input(frame, remote_player) {
            inputs[remote_player as usize] = remote_input;
        } else {
            stats.record_missing_remote_frame();
        }

        mole_runtime::step_world_with_source_collisions(&mut world, frame, &inputs);

        let local_packet = InputPacket::new(
            frame,
            config.player_index,
            inputs[config.player_index as usize],
            world.checksum(),
        )
        .with_timing_probe(frame.0, stats.last_remote_sequence.unwrap_or(0));
        transport
            .send_packet(local_packet)
            .map_err(|error| error.to_string())?;
        stats.record_sent();

        if let Some(capture) = replay_capture.as_mut() {
            capture.record_frame(frame, inputs, world.checksum());
        }
    }
    drain_udp_packets(&transport, &mut inbox, &mut stats, world.frame())?;

    if let (Some(path), Some(capture)) = (replay_path, replay_capture.as_ref()) {
        mole_runtime::write_replay_capture(path, capture).map_err(|error| error.to_string())?;
        println!("replay_path={}", path.display());
    }

    println!(
        "final_frame={} checksum={} udp_sent={} udp_recv={} udp_dup={} udp_unsupported={} udp_missing={} udp_last_remote_frame={:?} udp_last_remote_checksum={:?} udp_rtt_frames={:?}",
        world.frame().0,
        world.checksum(),
        stats.sent_packets,
        stats.received_packets,
        stats.duplicate_packets,
        stats.unsupported_packets,
        stats.missing_remote_frames,
        stats.last_remote_frame.map(|frame| frame.0),
        stats.last_remote_checksum,
        stats.last_rtt_frames
    );
    Ok(())
}

fn drain_udp_packets(
    transport: &UdpTransport,
    inbox: &mut InputPacketInbox,
    stats: &mut mole_runtime::UdpRuntimeStats,
    local_frame: Frame,
) -> Result<(), String> {
    while let Some(packet) = transport
        .try_recv_packet()
        .map_err(|error| error.to_string())?
    {
        let result = inbox.accept(packet);
        stats.record_accept_at(local_frame, result, packet);
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SUPABASE_URL: &str = "https://rhxagobpcbmodzgymrgo.supabase.co";
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SUPABASE_PUBLISHABLE_KEY: &str =
    "sb_publishable_QU6ckldfsBXPBcw8bT65Qg_dFZVXSNG";
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_STUN_SERVER: &str = "stun.l.google.com:19302";
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_MATCH_START_BROADCASTS: usize = 30;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_MATCH_START_INTERVAL: Duration = Duration::from_millis(200);
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_DEFAULT_INPUT_DELAY_FRAMES: u32 = 2;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_ROLLBACK_MAX_FRAMES: u32 = 7;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_LOCKSTEP_INTERVAL: u32 = 30;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_FRAME_TIME_US: i64 = 16_683;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_START_SYNC_THRESHOLD_US: i32 = 10_000;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_START_SYNC_MAX_FRAME: u32 = 120;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_START_SYNC_MAX_SKIP_FRAMES: u32 = 5;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION: u32 = 1_000_000;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_MAX_SLOWDOWN_PPM: i32 = 5_000;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_MAX_SPEEDUP_PPM: i32 = 10_000;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_SPEED_WINDOW_FRAMES: i32 = 3;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_ADVANCE_SPACING_FRAMES: u32 = 5;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_SLIPPI_POST_START_MAX_ADVANCE_FRAMES: u32 = 3;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_RECENT_INPUT_RETRANSMIT_FRAMES: usize = 8;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_RECENT_INPUT_RETAIN_FRAMES: u32 = 128;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_NETPLAY_LOG_MAX_BYTES: usize = 5 * 1024 * 1024;
#[cfg(all(feature = "sdl", feature = "wup"))]
const FRIEND_CONNECT_NETPLAY_SUMMARY_INTERVAL: u32 = 60;

#[cfg(all(feature = "sdl", feature = "wup"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct HeadlessFriendPeerConfig {
    host_code: String,
    frames: u32,
    netplay_delay_frames: u32,
    player_index: u8,
    remote_player: u8,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
struct PendingFriendConnect {
    pair: Option<FriendConnectPair>,
    lobby_room_code: String,
    lobby_owner: bool,
    socket: UdpSocket,
    local_endpoint: SocketAddr,
    receiver: mpsc::Receiver<Result<RemoteDirectEndpoint, String>>,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
struct ConnectedFriendGame {
    pair: FriendConnectPair,
    transport: UdpTransport,
    inbox: InputPacketInbox,
    stats: mole_runtime::UdpRuntimeStats,
    session: RollbackSession,
    match_frame: Frame,
    local_input_delay: SlippiInputDelayBuffer,
    time_sync: FriendConnectTimeSync,
    seeded_initial_delay_pads: bool,
    recent_local_packets: VecDeque<InputPacket>,
    player_index: u8,
    remote_player: u8,
    lobby_room_code: String,
    lobby_owner: bool,
    active_controller_port: Option<usize>,
    started: bool,
    start_receiver: mpsc::Receiver<Result<String, String>>,
    start_broadcast_receiver: Option<mpsc::Receiver<Result<(), String>>>,
    start_error: Option<String>,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
enum FriendConnectNetwork {
    Editing,
    Pending(PendingFriendConnect),
    Connected(ConnectedFriendGame),
    Failed,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
struct FriendConnectPanel {
    local_peer_id: String,
    remote_peer_id: String,
    status: String,
    input_status: String,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
type FriendNetplayLogger = BoundedNetplayLogger<fs::File>;

#[cfg(all(feature = "sdl", feature = "wup"))]
#[derive(Debug, Clone)]
struct FriendConnectTimeSync {
    has_game_started: bool,
    ping_us: i64,
    last_local_send_frame: Frame,
    last_local_send_time: Instant,
    ack_timers: VecDeque<(u32, Instant)>,
    offset_samples: VecDeque<i32>,
    frames_to_skip: u32,
    is_currently_skipping: bool,
    frames_to_advance: u32,
    is_currently_advancing: bool,
    fall_behind_counter: u32,
    fall_far_behind_counter: u32,
    speed_parts_per_million: u32,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
impl FriendConnectTimeSync {
    fn new(now: Instant) -> Self {
        Self {
            has_game_started: false,
            ping_us: 0,
            last_local_send_frame: Frame(0),
            last_local_send_time: now,
            ack_timers: VecDeque::new(),
            offset_samples: VecDeque::new(),
            frames_to_skip: 0,
            is_currently_skipping: false,
            frames_to_advance: 0,
            is_currently_advancing: false,
            fall_behind_counter: 0,
            fall_far_behind_counter: 0,
            speed_parts_per_million: FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION,
        }
    }

    fn record_local_send(&mut self, frame: Frame, now: Instant) {
        self.has_game_started = true;
        self.last_local_send_frame = frame;
        self.last_local_send_time = now;
        self.ack_timers.push_back((frame.0, now));
        while self.ack_timers.len() > FRIEND_CONNECT_RECENT_INPUT_RETAIN_FRAMES as usize {
            self.ack_timers.pop_front();
        }
    }

    fn record_remote_packet(&mut self, packet: InputPacket, now: Instant) {
        let (local_frame, local_time) = if self.has_game_started {
            (self.last_local_send_frame, self.last_local_send_time)
        } else {
            (Frame(0), now)
        };
        let receive_delta_us = instant_delta_us(now, local_time);
        let opponent_send_delta_us = receive_delta_us - (self.ping_us / 2);
        let frame_delta = i64::from(local_frame.0).saturating_sub(i64::from(packet.frame.0));
        let frame_delta_us = frame_delta.saturating_mul(FRIEND_CONNECT_SLIPPI_FRAME_TIME_US);
        let offset = opponent_send_delta_us.saturating_add(frame_delta_us);
        self.push_offset_sample(clamp_i64_to_i32(offset));
        self.record_remote_ack(packet.ack_sequence, now);
    }

    fn should_skip_online_frame(
        &mut self,
        frame: Frame,
        latest_remote_frame: Option<Frame>,
    ) -> bool {
        if friend_connect_should_skip_online_frame(frame, latest_remote_frame) {
            return true;
        }

        let is_time_sync_frame = frame.0 % FRIEND_CONNECT_SLIPPI_LOCKSTEP_INTERVAL == 0;
        if is_time_sync_frame
            && !self.is_currently_skipping
            && frame.0 <= FRIEND_CONNECT_SLIPPI_START_SYNC_MAX_FRAME
        {
            let offset_us = self.calc_time_offset_us();
            if offset_us > FRIEND_CONNECT_SLIPPI_START_SYNC_THRESHOLD_US {
                self.is_currently_skipping = true;
                let excess = offset_us - FRIEND_CONNECT_SLIPPI_START_SYNC_THRESHOLD_US;
                let skip_frames =
                    (excess as u32 / FRIEND_CONNECT_SLIPPI_FRAME_TIME_US as u32).saturating_add(1);
                self.frames_to_skip =
                    skip_frames.min(FRIEND_CONNECT_SLIPPI_START_SYNC_MAX_SKIP_FRAMES);
            }
        }

        if self.frames_to_skip > 0 {
            self.frames_to_skip = self.frames_to_skip.saturating_sub(1);
            return true;
        }

        self.is_currently_skipping = false;
        false
    }

    fn advance_pacing_for_frame(&mut self, frame: Frame) -> FriendConnectPacingDecision {
        let is_time_sync_frame = frame.0 % FRIEND_CONNECT_SLIPPI_LOCKSTEP_INTERVAL == 0;
        if is_time_sync_frame {
            let offset_us = self.calc_time_offset_us();
            self.speed_parts_per_million = slippi_dynamic_speed_parts_per_million(offset_us);

            if offset_us < -FRIEND_CONNECT_SLIPPI_START_SYNC_THRESHOLD_US {
                self.fall_behind_counter = self.fall_behind_counter.saturating_add(1);
            }
            if offset_us
                < -(FRIEND_CONNECT_SLIPPI_FRAME_TIME_US as i32
                    + FRIEND_CONNECT_SLIPPI_START_SYNC_THRESHOLD_US)
            {
                self.fall_far_behind_counter = self.fall_far_behind_counter.saturating_add(1);
            }

            let advance_threshold = FRIEND_CONNECT_SLIPPI_FRAME_TIME_US as i32
                + FRIEND_CONNECT_SLIPPI_START_SYNC_THRESHOLD_US;
            if offset_us < -advance_threshold && !self.is_currently_advancing {
                self.is_currently_advancing = true;
                let max_advance_frames = if frame.0 > FRIEND_CONNECT_SLIPPI_START_SYNC_MAX_FRAME {
                    FRIEND_CONNECT_SLIPPI_POST_START_MAX_ADVANCE_FRAMES
                } else {
                    0
                };
                let advance_frames = ((-offset_us - FRIEND_CONNECT_SLIPPI_START_SYNC_THRESHOLD_US)
                    as u32
                    / FRIEND_CONNECT_SLIPPI_FRAME_TIME_US as u32)
                    .saturating_add(1);
                self.frames_to_advance = advance_frames.min(max_advance_frames);
            }
        }

        let mut decision = FriendConnectPacingDecision {
            speed_parts_per_million: self.speed_parts_per_million,
            advance_online_frame: false,
        };
        if self.frames_to_advance > 0 {
            if frame.0 % FRIEND_CONNECT_SLIPPI_ADVANCE_SPACING_FRAMES != 0 {
                return decision;
            }
            self.frames_to_advance = self.frames_to_advance.saturating_sub(1);
            decision.advance_online_frame = true;
            return decision;
        }

        self.is_currently_advancing = false;
        decision
    }

    fn record_remote_ack(&mut self, ack_sequence: u32, now: Instant) {
        if ack_sequence == 0 {
            return;
        }
        while self
            .ack_timers
            .front()
            .map(|(frame, _)| *frame < ack_sequence)
            .unwrap_or(false)
        {
            self.ack_timers.pop_front();
        }
        let Some((frame, sent_at)) = self.ack_timers.front().copied() else {
            return;
        };
        if frame != ack_sequence {
            return;
        }
        self.ping_us = instant_delta_us(now, sent_at).max(0);
        self.ack_timers.pop_front();
    }

    fn push_offset_sample(&mut self, offset: i32) {
        if self.offset_samples.len() >= FRIEND_CONNECT_SLIPPI_LOCKSTEP_INTERVAL as usize {
            self.offset_samples.pop_front();
        }
        self.offset_samples.push_back(offset);
    }

    fn calc_time_offset_us(&self) -> i32 {
        if self.offset_samples.is_empty() {
            return 0;
        }
        let mut samples = self.offset_samples.iter().copied().collect::<Vec<_>>();
        samples.sort_unstable();
        let offset = samples.len() / 3;
        let end = samples.len().saturating_sub(offset);
        if end <= offset {
            return 0;
        }
        let sum: i64 = samples[offset..end]
            .iter()
            .map(|sample| i64::from(*sample))
            .sum();
        (sum / (end - offset) as i64) as i32
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FriendConnectPacingDecision {
    speed_parts_per_million: u32,
    advance_online_frame: bool,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
impl Default for FriendConnectPacingDecision {
    fn default() -> Self {
        Self {
            speed_parts_per_million: FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION,
            advance_online_frame: false,
        }
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn slippi_dynamic_speed_parts_per_million(offset_us: i32) -> u32 {
    if offset_us > -250 && offset_us < 8_000 {
        return FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION;
    }
    let window_us =
        FRIEND_CONNECT_SLIPPI_SPEED_WINDOW_FRAMES * FRIEND_CONNECT_SLIPPI_FRAME_TIME_US as i32;
    if offset_us < 0 {
        let deviation = ((-offset_us).saturating_mul(FRIEND_CONNECT_SLIPPI_MAX_SPEEDUP_PPM)
            / window_us)
            .min(FRIEND_CONNECT_SLIPPI_MAX_SPEEDUP_PPM);
        return (FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION as i32 + deviation) as u32;
    }
    let deviation = (offset_us.saturating_mul(FRIEND_CONNECT_SLIPPI_MAX_SLOWDOWN_PPM) / window_us)
        .min(FRIEND_CONNECT_SLIPPI_MAX_SLOWDOWN_PPM);
    (FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION as i32 - deviation) as u32
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_frame_budget_for_speed(
    base_budget: Duration,
    speed_parts_per_million: u32,
) -> Duration {
    if speed_parts_per_million == 0
        || speed_parts_per_million == FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION
    {
        return base_budget;
    }
    let nanos = base_budget.as_nanos();
    let adjusted = nanos.saturating_mul(u128::from(FRIEND_CONNECT_SLIPPI_SPEED_PARTS_PER_MILLION))
        / u128::from(speed_parts_per_million);
    Duration::from_nanos(adjusted.min(u128::from(u64::MAX)) as u64)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn instant_delta_us(later: Instant, earlier: Instant) -> i64 {
    if later >= earlier {
        later
            .duration_since(earlier)
            .as_micros()
            .min(i64::MAX as u128) as i64
    } else {
        -(earlier
            .duration_since(later)
            .as_micros()
            .min(i64::MAX as u128) as i64)
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn create_friend_netplay_logger(
    role: NetplayLogRole,
    room_code: &str,
    peer_id: &str,
) -> Option<FriendNetplayLogger> {
    let log_dir = mole_runtime::project_asset_root()
        .join("logs")
        .join("netplay");
    fs::create_dir_all(&log_dir).ok()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let file_name = format!(
        "{}-{}-{}-{}.jsonl",
        friend_netplay_log_role_name(role),
        sanitize_netplay_log_name(room_code),
        sanitize_netplay_log_name(peer_id),
        timestamp
    );
    let file = fs::File::create(log_dir.join(file_name)).ok()?;
    Some(BoundedNetplayLogger::new(
        file,
        FRIEND_CONNECT_NETPLAY_LOG_MAX_BYTES,
    ))
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_netplay_log_role_name(role: NetplayLogRole) -> &'static str {
    match role {
        NetplayLogRole::VisibleHost => "visible_host",
        NetplayLogRole::HeadlessPeer => "headless_peer",
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn sanitize_netplay_log_name(value: &str) -> String {
    let cleaned = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>();
    if cleaned.is_empty() {
        "UNKNOWN".to_string()
    } else {
        cleaned
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn write_friend_netplay_log(logger: &mut Option<FriendNetplayLogger>, event: NetplayLogEvent<'_>) {
    if let Some(logger) = logger.as_mut() {
        let _ = logger.write_event(event);
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn run_friend_connect_headless_peer(config: HeadlessFriendPeerConfig) -> Result<(), String> {
    mole_runtime::preload_runtime_source_frame_data()?;
    let local_peer_id = generate_friend_peer_id();
    let mut logger = create_friend_netplay_logger(
        NetplayLogRole::HeadlessPeer,
        &config.host_code,
        &local_peer_id,
    );
    write_friend_netplay_log(
        &mut logger,
        NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "session_start")
            .with_room_code(&config.host_code)
            .with_peer_id(&local_peer_id)
            .with_message("headless peer starting as P2"),
    );

    let mut panel = FriendConnectPanel {
        local_peer_id: local_peer_id.clone(),
        remote_peer_id: config.host_code.clone(),
        status: "HEADLESS OPENING UDP".to_string(),
        input_status: "HEADLESS INPUT NEUTRAL".to_string(),
    };
    let (socket, local_endpoint) = create_friend_udp_socket_and_endpoint(&mut panel, None)
        .ok_or_else(|| format!("headless UDP setup failed: {}", panel.status))?;
    write_friend_netplay_log(
        &mut logger,
        NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "local_endpoint")
            .with_room_code(&config.host_code)
            .with_peer_id(&local_peer_id)
            .with_message(&local_endpoint.to_string()),
    );

    let signaling = friend_connect_supabase_config()?;
    println!(
        "{}",
        headless_friend_peer_status(
            "JOINING",
            &config.host_code,
            Frame(0),
            config.netplay_delay_frames
        )
    );
    let remote_endpoint = signaling.join_direct_endpoint(
        &config.host_code,
        &local_peer_id,
        DirectEndpoint::udp(local_endpoint.to_string()),
    )?;
    write_friend_netplay_log(
        &mut logger,
        NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "remote_endpoint")
            .with_room_code(&config.host_code)
            .with_peer_id(&local_peer_id)
            .with_message(&remote_endpoint.endpoint.udp_addr),
    );

    let peer_addr = remote_endpoint
        .endpoint
        .udp_addr
        .parse::<SocketAddr>()
        .map_err(|error| format!("headless peer UDP endpoint parse failed: {error}"))?;
    let transport =
        UdpTransport::from_socket(socket, peer_addr).map_err(|error| error.to_string())?;
    println!(
        "{}",
        headless_friend_peer_status(
            "WAIT START",
            &config.host_code,
            Frame(0),
            config.netplay_delay_frames
        )
    );
    signaling.wait_for_match_start_in_room(&config.host_code, &local_peer_id, &config.host_code)?;
    write_friend_netplay_log(
        &mut logger,
        NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "match_start")
            .with_room_code(&config.host_code)
            .with_peer_id(&local_peer_id),
    );

    let mut inbox = InputPacketInbox::default();
    let mut stats = mole_runtime::UdpRuntimeStats::default();
    let mut session = RollbackSession::new(mole_runtime::default_play_world(), 32);
    let mut local_input_delay = SlippiInputDelayBuffer::new(config.netplay_delay_frames);
    let mut time_sync = FriendConnectTimeSync::new(Instant::now());
    let mut seeded_initial_delay_pads = false;
    let mut recent_local_packets = VecDeque::new();
    let frame_budget = frame_pacing_budget_duration();
    let mut next_frame_deadline = Instant::now() + frame_budget;
    let mut match_frame = Frame(0);

    for frame_tick in 0..config.frames {
        drain_friend_udp_packets_from(
            &transport,
            &mut inbox,
            &mut stats,
            &mut session,
            &mut time_sync,
            config.remote_player,
            match_frame,
        )?;
        if time_sync.should_skip_online_frame(match_frame, stats.last_remote_frame) {
            resend_friend_recent_input_packets_to(
                &transport,
                &mut stats,
                &mut time_sync,
                &recent_local_packets,
            )?;
            if frame_tick % FRIEND_CONNECT_NETPLAY_SUMMARY_INTERVAL == 0 {
                let latest_remote = stats
                    .last_remote_frame
                    .map(|frame| frame.0.to_string())
                    .unwrap_or_else(|| "NONE".to_string());
                let status = headless_friend_peer_status(
                    "WAIT REMOTE",
                    &config.host_code,
                    match_frame,
                    config.netplay_delay_frames,
                );
                println!(
                    "{status} latest_remote={} sent={} recv={} rb={}",
                    latest_remote,
                    stats.sent_packets,
                    stats.received_packets,
                    stats.rollback_corrections
                );
                write_friend_netplay_log(
                    &mut logger,
                    NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "frame_summary")
                        .with_room_code(&config.host_code)
                        .with_peer_id(&local_peer_id)
                        .with_frame(match_frame)
                        .with_netplay_stats(&stats)
                        .with_world_checksum(session.world().checksum())
                        .with_packet_bundle_len(
                            friend_connect_recent_input_retransmit_packets(&recent_local_packets)
                                .len() as u32,
                        )
                        .with_pacing(time_sync.speed_parts_per_million, false, true),
                );
            }
            wait_until_frame_deadline(next_frame_deadline);
            next_frame_deadline += friend_connect_frame_budget_for_speed(
                frame_budget,
                time_sync.speed_parts_per_million,
            );
            continue;
        }
        let local_plan = friend_connect_stage_local_input(
            &mut local_input_delay,
            match_frame,
            headless_friend_peer_local_input(match_frame),
        );
        let local_input = local_plan.current_frame_input;
        let remote_input = inbox.input(match_frame, config.remote_player);
        if remote_input.is_none() {
            stats.record_missing_remote_frame();
        }
        let mut frame_inputs = [None, None];
        frame_inputs[config.player_index as usize] = Some(local_input);
        frame_inputs[config.remote_player as usize] = remote_input;
        session.advance_with_prediction(match_frame, frame_inputs);
        let checksum = session.world().checksum();
        let local_packet = friend_connect_local_input_packet(
            local_plan,
            config.player_index,
            checksum,
            stats.last_remote_sequence.unwrap_or(0),
        );
        send_friend_recent_input_packets_to(
            &transport,
            &mut stats,
            &mut time_sync,
            &mut seeded_initial_delay_pads,
            &mut recent_local_packets,
            config.netplay_delay_frames,
            local_packet,
        )?;

        let pacing_decision = time_sync.advance_pacing_for_frame(match_frame);
        if match_frame.0 % FRIEND_CONNECT_NETPLAY_SUMMARY_INTERVAL == 0 {
            let status = headless_friend_peer_status(
                "RUNNING",
                &config.host_code,
                match_frame,
                config.netplay_delay_frames,
            );
            println!(
                "{status} sent={} recv={} miss={} rb={} rtt={}",
                stats.sent_packets,
                stats.received_packets,
                stats.missing_remote_frames,
                stats.rollback_corrections,
                stats
                    .last_rtt_frames
                    .map(|rtt| format!("{rtt}F"))
                    .unwrap_or_else(|| "NONE".to_string())
            );
            write_friend_netplay_log(
                &mut logger,
                NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "frame_summary")
                    .with_room_code(&config.host_code)
                    .with_peer_id(&local_peer_id)
                    .with_frame(match_frame)
                    .with_netplay_stats(&stats)
                    .with_world_checksum(checksum)
                    .with_packet_bundle_len(
                        friend_connect_recent_input_retransmit_packets(&recent_local_packets).len()
                            as u32,
                    )
                    .with_pacing(
                        pacing_decision.speed_parts_per_million,
                        pacing_decision.advance_online_frame,
                        false,
                    )
                    .with_message(&format!(
                        "sent={} recv={} dup={} miss={} rb={} checksum={}",
                        stats.sent_packets,
                        stats.received_packets,
                        stats.duplicate_packets,
                        stats.missing_remote_frames,
                        stats.rollback_corrections,
                        checksum
                    )),
            );
        }

        match_frame = Frame(match_frame.0.saturating_add(1));
        if pacing_decision.advance_online_frame {
            next_frame_deadline = Instant::now();
        }
        wait_until_frame_deadline(next_frame_deadline);
        next_frame_deadline += friend_connect_frame_budget_for_speed(
            frame_budget,
            pacing_decision.speed_parts_per_million,
        );
    }

    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn headless_friend_peer_local_input(_frame: Frame) -> PlayerInput {
    PlayerInput::neutral()
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn headless_friend_peer_status(
    phase: &str,
    room_code: &str,
    frame: Frame,
    netplay_delay_frames: u32,
) -> String {
    format!(
        "HEADLESS P2 {} {} F{} D{}",
        phase, room_code, frame.0, netplay_delay_frames
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_stage_local_input(
    delay: &mut SlippiInputDelayBuffer,
    match_frame: Frame,
    raw_local_input: PlayerInput,
) -> SlippiDelayedInput {
    delay.push_physical_input(match_frame, raw_local_input)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_local_input_packet(
    delayed: SlippiDelayedInput,
    player_index: u8,
    checksum: u64,
    ack_sequence: u32,
) -> InputPacket {
    InputPacket::new(
        delayed.scheduled_frame,
        player_index,
        delayed.scheduled_input,
        checksum,
    )
    .with_timing_probe(delayed.scheduled_frame.0, ack_sequence)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_should_skip_online_frame(
    frame: Frame,
    latest_remote_frame: Option<Frame>,
) -> bool {
    let minimum_remote_frame = frame.0.saturating_sub(FRIEND_CONNECT_ROLLBACK_MAX_FRAMES);
    match latest_remote_frame {
        Some(latest_remote_frame) => latest_remote_frame.0 < minimum_remote_frame,
        None => frame.0 > FRIEND_CONNECT_ROLLBACK_MAX_FRAMES,
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn run_friend_connect_sdl(
    frames: u32,
    _replay_path: Option<&Path>,
    ucf_enabled: bool,
    netplay_delay_frames: u32,
    friend_code_override: Option<&str>,
    visual_connect_code: Option<&str>,
    auto_start: bool,
    window_offset: (i32, i32),
    local_udp_addr: Option<SocketAddr>,
) -> Result<(), String> {
    mole_runtime::preload_runtime_source_frame_data()?;
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let video = sdl.video().map_err(|error| error.to_string())?;
    let window = video
        .window("Mole Rust Friend Connect", 960, 540)
        .position(80 + window_offset.0, 120 + window_offset.1)
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window.into_canvas();
    disable_sdl_renderer_vsync(&canvas)?;
    let connect_window = video
        .window("Mole Friend Connect", 520, 260)
        .position(1060 + window_offset.0, 120 + window_offset.1)
        .build()
        .map_err(|error| error.to_string())?;
    let mut connect_canvas = connect_window.into_canvas();
    disable_sdl_renderer_vsync(&connect_canvas)?;
    let texture_creator = canvas.texture_creator();
    let mut texture_cache =
        SdlTextureCache::new(&texture_creator, mole_runtime::project_asset_root());
    let mut event_pump = sdl.event_pump().map_err(|error| error.to_string())?;
    let mut local_input_source =
        match WupInputSource::open_with_config(mole_runtime::WupInputConfig { ucf_enabled }) {
            Ok(source) => Some(source),
            Err(error) => {
                eprintln!("Friend Connect WUP input is not ready yet: {error}");
                None
            }
        };
    let initial = mole_runtime::default_play_world();
    let preview_world = initial.clone();
    let mut panel = FriendConnectPanel {
        local_peer_id: friend_code_override
            .map(str::to_string)
            .unwrap_or_else(generate_friend_peer_id),
        remote_peer_id: String::new(),
        status: "OPENING LOBBY".to_string(),
        input_status: if local_input_source.is_some() {
            "INPUT WUP READY".to_string()
        } else {
            "INPUT WUP NOT READY".to_string()
        },
    };
    let mut netplay_logger = create_friend_netplay_logger(
        NetplayLogRole::VisibleHost,
        &panel.local_peer_id,
        &panel.local_peer_id,
    );
    write_friend_netplay_log(
        &mut netplay_logger,
        NetplayLogEvent::new(NetplayLogRole::VisibleHost, "session_start")
            .with_room_code(&panel.local_peer_id)
            .with_peer_id(&panel.local_peer_id)
            .with_message("visible host starting as P1"),
    );
    let mut network = FriendConnectNetwork::Editing;
    if let Some(connect_code) = visual_connect_code {
        panel.remote_peer_id = connect_code.to_string();
        start_friend_connect(&mut panel, &mut network, local_udp_addr)?;
    } else {
        open_friend_lobby(&mut panel, &mut network, local_udp_addr)?;
    }
    let _timer_resolution = request_high_resolution_frame_timer();
    let frame_budget = frame_pacing_budget_duration();
    let mut next_frame_deadline = Instant::now() + frame_budget;

    for frame_number in 0..frames {
        let frame_start = Instant::now();
        let frame = Frame(frame_number);
        for event in event_pump.poll_iter() {
            if handle_friend_connect_event(event, &mut panel, &mut network, local_udp_addr)? {
                return Ok(());
            }
        }

        poll_pending_friend_connect(&mut network, &mut panel, netplay_delay_frames)?;
        if auto_start {
            auto_start_friend_lobby(&mut panel, &mut network)?;
        }

        let mut friend_pacing_decision = FriendConnectPacingDecision::default();
        let render_frame = match &mut network {
            FriendConnectNetwork::Connected(game) => {
                friend_pacing_decision.speed_parts_per_million =
                    game.time_sync.speed_parts_per_million;
                poll_friend_lobby_start(game, &mut panel);
                if !game.started {
                    panel.status = friend_lobby_status(game);
                    mole_runtime::RenderFrame::from_world(game.session.world())
                } else {
                    let match_frame = game.match_frame;
                    drain_friend_udp_packets(game, match_frame)?;
                    if game
                        .time_sync
                        .should_skip_online_frame(match_frame, game.stats.last_remote_frame)
                    {
                        resend_friend_recent_input_packets(game)?;
                        let cpu_headroom_hz = friend_connect_cpu_headroom_hz(frame_start.elapsed());
                        let latest_remote = game
                            .stats
                            .last_remote_frame
                            .map(|frame| frame.0.to_string())
                            .unwrap_or_else(|| "NONE".to_string());
                        panel.status = format!(
                            "CONNECTED P{} F{} D{} CPU {}HZ RTT {} RB {} WAIT REMOTE {}",
                            game.player_index + 1,
                            match_frame.0,
                            game.local_input_delay.delay_frames(),
                            cpu_headroom_hz,
                            game.stats
                                .last_rtt_frames
                                .map(|rtt| format!("{rtt}F"))
                                .unwrap_or_else(|| "NONE".to_string()),
                            game.stats.rollback_corrections,
                            latest_remote
                        );
                        if match_frame.0 % FRIEND_CONNECT_NETPLAY_SUMMARY_INTERVAL == 0 {
                            write_friend_netplay_log(
                                &mut netplay_logger,
                                NetplayLogEvent::new(NetplayLogRole::VisibleHost, "frame_summary")
                                    .with_room_code(&game.lobby_room_code)
                                    .with_peer_id(game.pair.local_peer_id())
                                    .with_frame(match_frame)
                                    .with_netplay_stats(&game.stats)
                                    .with_world_checksum(game.session.world().checksum())
                                    .with_packet_bundle_len(
                                        friend_connect_recent_input_retransmit_packets(
                                            &game.recent_local_packets,
                                        )
                                        .len() as u32,
                                    )
                                    .with_pacing(
                                        game.time_sync.speed_parts_per_million,
                                        false,
                                        true,
                                    ),
                            );
                        }
                    } else {
                        let raw_local_input = poll_friend_connect_local_input(
                            &mut local_input_source,
                            &mut game.active_controller_port,
                            ucf_enabled,
                            &mut panel,
                            frame,
                        );
                        let local_plan = friend_connect_stage_local_input(
                            &mut game.local_input_delay,
                            match_frame,
                            raw_local_input,
                        );
                        let local_input = local_plan.current_frame_input;
                        let remote_input = game.inbox.input(match_frame, game.remote_player);
                        if remote_input.is_none() {
                            game.stats.record_missing_remote_frame();
                        }
                        let mut frame_inputs = [None, None];
                        frame_inputs[game.player_index as usize] = Some(local_input);
                        frame_inputs[game.remote_player as usize] = remote_input;
                        game.session
                            .advance_with_prediction(match_frame, frame_inputs);
                        let checksum = game.session.world().checksum();
                        let cpu_headroom_hz = friend_connect_cpu_headroom_hz(frame_start.elapsed());
                        let local_packet = friend_connect_local_input_packet(
                            local_plan,
                            game.player_index,
                            checksum,
                            game.stats.last_remote_sequence.unwrap_or(0),
                        );
                        send_friend_recent_input_packets(game, local_packet)?;
                        game.match_frame = Frame(match_frame.0.saturating_add(1));
                        friend_pacing_decision =
                            game.time_sync.advance_pacing_for_frame(match_frame);
                        panel.status = format!(
                            "CONNECTED P{} F{} D{} CPU {}HZ RTT {} RB {}",
                            game.player_index + 1,
                            match_frame.0,
                            game.local_input_delay.delay_frames(),
                            cpu_headroom_hz,
                            game.stats
                                .last_rtt_frames
                                .map(|rtt| format!("{rtt}F"))
                                .unwrap_or_else(|| "NONE".to_string()),
                            game.stats.rollback_corrections
                        );
                        if match_frame.0 % FRIEND_CONNECT_NETPLAY_SUMMARY_INTERVAL == 0 {
                            write_friend_netplay_log(
                                &mut netplay_logger,
                                NetplayLogEvent::new(NetplayLogRole::VisibleHost, "frame_summary")
                                    .with_room_code(&game.lobby_room_code)
                                    .with_peer_id(game.pair.local_peer_id())
                                    .with_frame(match_frame)
                                    .with_netplay_stats(&game.stats)
                                    .with_world_checksum(checksum)
                                    .with_packet_bundle_len(
                                        friend_connect_recent_input_retransmit_packets(
                                            &game.recent_local_packets,
                                        )
                                        .len() as u32,
                                    )
                                    .with_pacing(
                                        friend_pacing_decision.speed_parts_per_million,
                                        friend_pacing_decision.advance_online_frame,
                                        false,
                                    )
                                    .with_message(&format!(
                                        "sent={} recv={} dup={} miss={} rb={} checksum={}",
                                        game.stats.sent_packets,
                                        game.stats.received_packets,
                                        game.stats.duplicate_packets,
                                        game.stats.missing_remote_frames,
                                        game.stats.rollback_corrections,
                                        checksum
                                    )),
                            );
                        }
                    }
                    mole_runtime::RenderFrame::from_world(game.session.world())
                }
            }
            FriendConnectNetwork::Editing
            | FriendConnectNetwork::Pending(_)
            | FriendConnectNetwork::Failed => mole_runtime::RenderFrame::from_world(&preview_world),
        };

        let overlay = match &network {
            FriendConnectNetwork::Connected(game) => {
                DebugOverlay::from_frame_with_udp_stats(&render_frame, &game.stats)
            }
            _ => DebugOverlay::from_frame(&render_frame),
        };
        let (width, height) = canvas.output_size().map_err(|error| error.to_string())?;
        let scene = RenderScene::from_frame(&render_frame, width, height);
        draw_sdl_scene(
            &mut canvas,
            &scene,
            Some(&overlay),
            Some(&mut texture_cache),
        )?;
        draw_friend_connect_panel(&mut connect_canvas, &panel, &network)?;

        if friend_pacing_decision.advance_online_frame {
            next_frame_deadline = Instant::now();
        }
        wait_until_frame_deadline(next_frame_deadline);
        next_frame_deadline += friend_connect_frame_budget_for_speed(
            frame_budget,
            friend_pacing_decision.speed_parts_per_million,
        );
    }

    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn handle_friend_connect_event(
    event: Event,
    panel: &mut FriendConnectPanel,
    network: &mut FriendConnectNetwork,
    local_udp_addr: Option<SocketAddr>,
) -> Result<bool, String> {
    match event {
        Event::Quit { .. }
        | Event::KeyDown {
            keycode: Some(Keycode::Escape),
            ..
        } => return Ok(true),
        Event::MouseButtonDown { x, y, .. } => {
            if start_button_contains(x, y) {
                start_friend_lobby(panel, network)?;
            }
        }
        Event::KeyDown {
            keycode: Some(Keycode::S | Keycode::Space),
            ..
        } => {
            start_friend_lobby(panel, network)?;
        }
        Event::KeyDown {
            keycode: Some(Keycode::Backspace),
            ..
        } => {
            if friend_connect_accepts_peer_code_input(network) {
                panel.remote_peer_id.pop();
            }
        }
        Event::KeyDown {
            keycode: Some(Keycode::Return | Keycode::KpEnter),
            ..
        } => {
            if friend_connect_accepts_peer_code_input(network) {
                start_friend_connect(panel, network, local_udp_addr)?;
            }
        }
        Event::KeyDown {
            keycode: Some(keycode),
            ..
        } => {
            if friend_connect_accepts_peer_code_input(network) {
                if let Some(character) = friend_code_character(keycode) {
                    if panel.remote_peer_id.len() < 12 {
                        panel.remote_peer_id.push(character);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_accepts_peer_code_input(network: &FriendConnectNetwork) -> bool {
    match network {
        FriendConnectNetwork::Editing | FriendConnectNetwork::Failed => true,
        FriendConnectNetwork::Pending(pending) => pending.lobby_owner,
        FriendConnectNetwork::Connected(_) => false,
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn open_friend_lobby(
    panel: &mut FriendConnectPanel,
    network: &mut FriendConnectNetwork,
    local_udp_addr: Option<SocketAddr>,
) -> Result<(), String> {
    let (socket, local_endpoint) =
        match create_friend_udp_socket_and_endpoint(panel, local_udp_addr) {
            Some(parts) => parts,
            None => {
                *network = FriendConnectNetwork::Failed;
                return Ok(());
            }
        };
    let config = match friend_connect_supabase_config() {
        Ok(config) => config,
        Err(error) => {
            panel.status = format!("SIGNAL CONFIG {}", trim_status(&error));
            *network = FriendConnectNetwork::Failed;
            return Ok(());
        }
    };
    let room_code = panel.local_peer_id.clone();
    let local_peer_id = panel.local_peer_id.clone();
    let local_endpoint_text = local_endpoint.to_string();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = config.host_direct_endpoint(
            &room_code,
            &local_peer_id,
            DirectEndpoint::udp(local_endpoint_text),
        );
        let _ = sender.send(result);
    });
    panel.status = format!("LOBBY OPEN SHARE {}", panel.local_peer_id);
    *network = FriendConnectNetwork::Pending(PendingFriendConnect {
        pair: None,
        lobby_room_code: panel.local_peer_id.clone(),
        lobby_owner: true,
        socket,
        local_endpoint,
        receiver,
    });
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn start_friend_connect(
    panel: &mut FriendConnectPanel,
    network: &mut FriendConnectNetwork,
    local_udp_addr: Option<SocketAddr>,
) -> Result<(), String> {
    let pair = match FriendConnectPair::new(&panel.local_peer_id, &panel.remote_peer_id) {
        Ok(pair) => pair,
        Err(error) => {
            panel.status = error.to_ascii_uppercase();
            *network = FriendConnectNetwork::Failed;
            return Ok(());
        }
    };
    let (socket, local_endpoint) =
        match create_friend_udp_socket_and_endpoint(panel, local_udp_addr) {
            Some(parts) => parts,
            None => {
                *network = FriendConnectNetwork::Failed;
                return Ok(());
            }
        };
    let config = match friend_connect_supabase_config() {
        Ok(config) => config,
        Err(error) => {
            panel.status = format!("SIGNAL CONFIG {}", trim_status(&error));
            *network = FriendConnectNetwork::Failed;
            return Ok(());
        }
    };
    let thread_pair = pair.clone();
    let lobby_room_code = panel.remote_peer_id.clone();
    let local_peer_id = panel.local_peer_id.clone();
    let local_endpoint_text = local_endpoint.to_string();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = config.join_direct_endpoint(
            &lobby_room_code,
            &local_peer_id,
            DirectEndpoint::udp(local_endpoint_text),
        );
        let _ = sender.send(result);
    });
    panel.status = format!("JOINING {}", panel.remote_peer_id);
    *network = FriendConnectNetwork::Pending(PendingFriendConnect {
        pair: Some(thread_pair),
        lobby_room_code: panel.remote_peer_id.clone(),
        lobby_owner: false,
        socket,
        local_endpoint,
        receiver,
    });
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn create_friend_udp_socket_and_endpoint(
    panel: &mut FriendConnectPanel,
    local_udp_addr: Option<SocketAddr>,
) -> Option<(UdpSocket, SocketAddr)> {
    if let Some(local_udp_addr) = local_udp_addr {
        let socket = match UdpSocket::bind(local_udp_addr) {
            Ok(socket) => socket,
            Err(error) => {
                panel.status = format!("UDP BIND ERROR {}", trim_status(&error.to_string()));
                return None;
            }
        };
        return Some((socket, local_udp_addr));
    }

    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => socket,
        Err(error) => {
            panel.status = format!("UDP BIND ERROR {}", trim_status(&error.to_string()));
            return None;
        }
    };
    let local_endpoint = match resolve_friend_connect_stun_server().and_then(|stun_server| {
        discover_public_udp_endpoint(&socket, stun_server, Duration::from_secs(3))
            .map_err(|error| error.to_string())
    }) {
        Ok(endpoint) => endpoint,
        Err(error) => match socket.local_addr() {
            Ok(fallback) => {
                panel.status = format!("STUN FAILED USING LOCAL {}", fallback.port());
                eprintln!("Friend Connect STUN failed, using local endpoint {fallback}: {error}");
                fallback
            }
            Err(local_error) => {
                panel.status = format!("LOCAL UDP ERROR {}", trim_status(&local_error.to_string()));
                return None;
            }
        },
    };
    Some((socket, local_endpoint))
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn poll_pending_friend_connect(
    network: &mut FriendConnectNetwork,
    panel: &mut FriendConnectPanel,
    netplay_delay_frames: u32,
) -> Result<(), String> {
    let FriendConnectNetwork::Pending(pending) = network else {
        return Ok(());
    };
    match pending.receiver.try_recv() {
        Ok(Ok(remote_endpoint)) => {
            let peer_addr = match remote_endpoint.endpoint.udp_addr.parse::<SocketAddr>() {
                Ok(peer_addr) => peer_addr,
                Err(error) => {
                    panel.status = format!("PEER UDP ERROR {}", trim_status(&error.to_string()));
                    *network = FriendConnectNetwork::Failed;
                    return Ok(());
                }
            };
            let socket = match pending.socket.try_clone() {
                Ok(socket) => socket,
                Err(error) => {
                    panel.status = format!("UDP CLONE ERROR {}", trim_status(&error.to_string()));
                    *network = FriendConnectNetwork::Failed;
                    return Ok(());
                }
            };
            let transport = match UdpTransport::from_socket(socket, peer_addr) {
                Ok(transport) => transport,
                Err(error) => {
                    panel.status = format!("UDP SETUP ERROR {}", trim_status(&error.to_string()));
                    *network = FriendConnectNetwork::Failed;
                    return Ok(());
                }
            };
            let pair = match pending.pair.as_ref() {
                Some(pair) => pair.clone(),
                None => {
                    match FriendConnectPair::new(&panel.local_peer_id, &remote_endpoint.peer_id) {
                        Ok(pair) => pair,
                        Err(error) => {
                            panel.status = error.to_ascii_uppercase();
                            *network = FriendConnectNetwork::Failed;
                            return Ok(());
                        }
                    }
                }
            };
            let (player_index, remote_player) = friend_connect_player_indices(pending.lobby_owner);
            panel.remote_peer_id = remote_endpoint.peer_id;
            panel.status = format!("CONNECTED P{} UDP {}", player_index + 1, peer_addr);
            *network = FriendConnectNetwork::Connected(ConnectedFriendGame {
                pair,
                transport,
                inbox: InputPacketInbox::default(),
                stats: mole_runtime::UdpRuntimeStats::default(),
                session: RollbackSession::new(mole_runtime::default_play_world(), 32),
                match_frame: Frame(0),
                local_input_delay: SlippiInputDelayBuffer::new(netplay_delay_frames),
                time_sync: FriendConnectTimeSync::new(Instant::now()),
                seeded_initial_delay_pads: false,
                recent_local_packets: VecDeque::new(),
                player_index,
                remote_player,
                lobby_room_code: pending.lobby_room_code.clone(),
                lobby_owner: pending.lobby_owner,
                active_controller_port: None,
                started: false,
                start_receiver: spawn_friend_match_start_listener(
                    pending.lobby_room_code.clone(),
                    panel.local_peer_id.clone(),
                    panel.remote_peer_id.clone(),
                ),
                start_broadcast_receiver: None,
                start_error: None,
            });
        }
        Ok(Err(error)) => {
            panel.status = format!("SIGNAL ERROR {}", trim_status(&error));
            *network = FriendConnectNetwork::Failed;
        }
        Err(mpsc::TryRecvError::Empty) => {
            panel.status = friend_pending_status(pending);
        }
        Err(mpsc::TryRecvError::Disconnected) => {
            panel.status = "SIGNAL THREAD LOST".to_string();
            *network = FriendConnectNetwork::Failed;
        }
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_player_indices(lobby_owner: bool) -> (u8, u8) {
    if lobby_owner {
        (0, 1)
    } else {
        (1, 0)
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_pending_status(pending: &PendingFriendConnect) -> String {
    if pending.lobby_owner {
        format!(
            "LOBBY OPEN {} UDP {}",
            pending.lobby_room_code, pending.local_endpoint
        )
    } else {
        format!(
            "JOINING {} UDP {}",
            pending.lobby_room_code, pending.local_endpoint
        )
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_lobby_status(game: &ConnectedFriendGame) -> String {
    if let Some(error) = game.start_error.as_ref() {
        return format!("START SIGNAL {}", trim_status(error));
    }
    friend_lobby_status_for(game.lobby_owner, None)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_lobby_status_for(lobby_owner: bool, start_error: Option<&str>) -> String {
    if let Some(error) = start_error {
        return format!("START SIGNAL {}", trim_status(error));
    }
    if lobby_owner {
        "LOBBY PEER JOINED CLICK START".to_string()
    } else {
        "LOBBY PEER JOINED WAIT START".to_string()
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn start_friend_lobby(
    panel: &mut FriendConnectPanel,
    network: &mut FriendConnectNetwork,
) -> Result<(), String> {
    let FriendConnectNetwork::Connected(game) = network else {
        return Ok(());
    };
    if game.started {
        return Ok(());
    }
    if !game.lobby_owner {
        panel.status = "WAITING FOR LOBBY START".to_string();
        return Ok(());
    }

    game.started = true;
    panel.status = "STARTING MATCH".to_string();
    let lobby_room_code = game.lobby_room_code.clone();
    let local_peer_id = game.pair.local_peer_id().to_string();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = friend_connect_supabase_config().and_then(|config| {
            config.broadcast_match_start_repeated_in_room(
                &lobby_room_code,
                &local_peer_id,
                FRIEND_CONNECT_MATCH_START_BROADCASTS,
                FRIEND_CONNECT_MATCH_START_INTERVAL,
            )
        });
        let _ = sender.send(result);
    });
    game.start_broadcast_receiver = Some(receiver);
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn auto_start_friend_lobby(
    panel: &mut FriendConnectPanel,
    network: &mut FriendConnectNetwork,
) -> Result<(), String> {
    let should_start = match network {
        FriendConnectNetwork::Connected(game) => {
            game.lobby_owner && !game.started && game.start_broadcast_receiver.is_none()
        }
        _ => false,
    };
    if should_start {
        start_friend_lobby(panel, network)?;
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn poll_friend_lobby_start(game: &mut ConnectedFriendGame, panel: &mut FriendConnectPanel) {
    if let Some(receiver) = game.start_broadcast_receiver.take() {
        match receiver.try_recv() {
            Ok(Ok(())) => {
                game.start_broadcast_receiver = None;
            }
            Ok(Err(error)) => {
                game.start_error = Some(error);
                game.start_broadcast_receiver = None;
            }
            Err(mpsc::TryRecvError::Empty) => {
                game.start_broadcast_receiver = Some(receiver);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                game.start_error = Some("start broadcast thread lost".to_string());
                game.start_broadcast_receiver = None;
            }
        }
    }
    if game.started {
        return;
    }
    match game.start_receiver.try_recv() {
        Ok(Ok(peer_id)) => {
            game.started = true;
            panel.status = format!("STARTED BY {}", peer_id);
        }
        Ok(Err(error)) => {
            game.start_error = Some(error);
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            game.start_error = Some("start listener lost".to_string());
        }
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn spawn_friend_match_start_listener(
    lobby_room_code: String,
    local_peer_id: String,
    remote_peer_id: String,
) -> mpsc::Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = friend_connect_supabase_config().and_then(|config| {
            config.wait_for_match_start_in_room(&lobby_room_code, &local_peer_id, &remote_peer_id)
        });
        let _ = sender.send(result);
    });
    receiver
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn resolve_friend_connect_stun_server() -> Result<SocketAddr, String> {
    select_friend_connect_stun_addr(
        FRIEND_CONNECT_STUN_SERVER
            .to_socket_addrs()
            .map_err(|error| format!("STUN DNS failed: {error}"))?,
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn select_friend_connect_stun_addr(
    addrs: impl IntoIterator<Item = SocketAddr>,
) -> Result<SocketAddr, String> {
    let addrs = addrs.into_iter().collect::<Vec<_>>();
    addrs
        .iter()
        .copied()
        .find(SocketAddr::is_ipv4)
        .or_else(|| addrs.first().copied())
        .ok_or_else(|| "STUN DNS returned no socket addresses".to_string())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn poll_friend_connect_local_input(
    input_source: &mut Option<WupInputSource>,
    active_controller_port: &mut Option<usize>,
    ucf_enabled: bool,
    panel: &mut FriendConnectPanel,
    frame: Frame,
) -> PlayerInput {
    if input_source.is_none() && frame.0 % 60 == 0 {
        match WupInputSource::open_with_config(mole_runtime::WupInputConfig { ucf_enabled }) {
            Ok(source) => {
                *input_source = Some(source);
                panel.input_status = "INPUT WUP READY WAIT ACTIVE".to_string();
            }
            Err(error) => {
                panel.input_status = format!("INPUT WUP WAIT {}", trim_status(&error));
            }
        }
    }

    let Some(source) = input_source.as_mut() else {
        return PlayerInput::neutral();
    };

    match source.poll_local_player_input(active_controller_port) {
        Ok(input) => {
            panel.input_status = active_controller_port
                .map(|port| format!("INPUT PORT {} ACTIVE", port + 1))
                .unwrap_or_else(|| "INPUT WUP READY WAIT ACTIVE".to_string());
            input
        }
        Err(rusb::Error::Timeout) => source.latest_inputs()[0],
        Err(error) => {
            panel.input_status = format!("INPUT WUP WAIT {}", trim_status(&error.to_string()));
            source.latest_inputs()[0]
        }
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn send_friend_recent_input_packets(
    game: &mut ConnectedFriendGame,
    packet: InputPacket,
) -> Result<(), String> {
    send_friend_recent_input_packets_to(
        &game.transport,
        &mut game.stats,
        &mut game.time_sync,
        &mut game.seeded_initial_delay_pads,
        &mut game.recent_local_packets,
        game.local_input_delay.delay_frames(),
        packet,
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn send_friend_recent_input_packets_to(
    transport: &UdpTransport,
    stats: &mut mole_runtime::UdpRuntimeStats,
    time_sync: &mut FriendConnectTimeSync,
    seeded_initial_delay_pads: &mut bool,
    recent_local_packets: &mut VecDeque<InputPacket>,
    delay_frames: u32,
    packet: InputPacket,
) -> Result<(), String> {
    push_friend_local_input_packet(
        recent_local_packets,
        stats.last_acked_sequence,
        seeded_initial_delay_pads,
        delay_frames,
        packet,
    );
    resend_friend_recent_input_packets_to(transport, stats, time_sync, recent_local_packets)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn resend_friend_recent_input_packets(game: &mut ConnectedFriendGame) -> Result<(), String> {
    resend_friend_recent_input_packets_to(
        &game.transport,
        &mut game.stats,
        &mut game.time_sync,
        &game.recent_local_packets,
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn resend_friend_recent_input_packets_to(
    transport: &UdpTransport,
    stats: &mut mole_runtime::UdpRuntimeStats,
    time_sync: &mut FriendConnectTimeSync,
    recent_local_packets: &VecDeque<InputPacket>,
) -> Result<(), String> {
    if let Some(datagram) = friend_connect_recent_input_retransmit_datagram(recent_local_packets) {
        transport
            .send_packet_datagram(&datagram)
            .map_err(|error| error.to_string())?;
        stats.record_sent();
    }
    if let Some(packet) = recent_local_packets.front() {
        time_sync.record_local_send(packet.frame, Instant::now());
    }

    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_recent_input_retransmit_packets(
    recent_local_packets: &VecDeque<InputPacket>,
) -> Vec<InputPacket> {
    recent_local_packets
        .iter()
        .take(FRIEND_CONNECT_RECENT_INPUT_RETRANSMIT_FRAMES)
        .copied()
        .collect()
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_recent_input_retransmit_datagram(
    recent_local_packets: &VecDeque<InputPacket>,
) -> Option<InputPacketDatagram> {
    let packets = friend_connect_recent_input_retransmit_packets(recent_local_packets);
    InputPacketDatagram::from_packets(packets).ok()
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn push_friend_recent_input_packet(
    packets: &mut VecDeque<InputPacket>,
    last_acked_sequence: Option<u32>,
    packet: InputPacket,
) {
    packets.push_front(packet);
    if let Some(last_acked_sequence) = last_acked_sequence {
        packets.retain(|packet| packet.sequence >= last_acked_sequence);
    }
    let newest = packet.sequence;
    if newest > FRIEND_CONNECT_RECENT_INPUT_RETAIN_FRAMES {
        let minimum_sequence = newest - FRIEND_CONNECT_RECENT_INPUT_RETAIN_FRAMES;
        packets.retain(|packet| packet.sequence > minimum_sequence);
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn push_friend_local_input_packet(
    packets: &mut VecDeque<InputPacket>,
    last_acked_sequence: Option<u32>,
    seeded_initial_delay_pads: &mut bool,
    delay_frames: u32,
    packet: InputPacket,
) {
    if !*seeded_initial_delay_pads {
        let first_delay_frame = packet.frame.0.saturating_sub(delay_frames);
        for frame in first_delay_frame..packet.frame.0 {
            let neutral_packet = InputPacket::new(
                Frame(frame),
                packet.player_index,
                PlayerInput::neutral(),
                packet.checksum,
            )
            .with_timing_probe(frame, packet.ack_sequence);
            push_friend_recent_input_packet(packets, last_acked_sequence, neutral_packet);
        }
        *seeded_initial_delay_pads = true;
    }
    push_friend_recent_input_packet(packets, last_acked_sequence, packet);
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn drain_friend_udp_packets(
    game: &mut ConnectedFriendGame,
    local_frame: Frame,
) -> Result<(), String> {
    drain_friend_udp_packets_from(
        &game.transport,
        &mut game.inbox,
        &mut game.stats,
        &mut game.session,
        &mut game.time_sync,
        game.remote_player,
        local_frame,
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn drain_friend_udp_packets_from(
    transport: &UdpTransport,
    inbox: &mut InputPacketInbox,
    stats: &mut mole_runtime::UdpRuntimeStats,
    session: &mut RollbackSession,
    time_sync: &mut FriendConnectTimeSync,
    remote_player: u8,
    local_frame: Frame,
) -> Result<(), String> {
    while let Some(datagram) = transport
        .try_recv_packet_datagram()
        .map_err(|error| error.to_string())?
    {
        let Some(newest_packet) = datagram.packets().first().copied() else {
            continue;
        };
        time_sync.record_remote_packet(newest_packet, Instant::now());
        let previous_remote_head = stats.last_remote_frame;
        if previous_remote_head
            .map(|head| newest_packet.frame.0 <= head.0)
            .unwrap_or(false)
        {
            stats.record_accept_at(
                local_frame,
                mole_transport::PacketAcceptResult::Duplicate,
                newest_packet,
            );
            continue;
        }

        for packet in datagram.packets().iter().copied() {
            if previous_remote_head
                .map(|head| packet.frame.0 <= head.0)
                .unwrap_or(false)
            {
                break;
            }
            let result = inbox.accept(packet);
            stats.record_accept_at(local_frame, result, packet);
            if matches!(result, mole_transport::PacketAcceptResult::Accepted)
                && packet.player_index == remote_player
                && packet.frame.0 < local_frame.0
                && session.confirm_input(
                    packet.frame,
                    packet.player_index as usize,
                    packet.input,
                    local_frame,
                )
            {
                stats.record_rollback_correction();
            }
        }
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_friend_connect_panel(
    canvas: &mut WindowCanvas,
    panel: &FriendConnectPanel,
    network: &FriendConnectNetwork,
) -> Result<(), String> {
    canvas.set_draw_color(Color::RGBA(13, 17, 23, 255));
    canvas.clear();
    draw_sdl_label(
        canvas,
        "FRIEND CONNECT",
        22,
        22,
        5,
        Color::RGBA(245, 248, 255, 255),
    )?;
    draw_sdl_label(
        canvas,
        &format!("YOUR CODE {}", panel.local_peer_id),
        22,
        62,
        4,
        Color::RGBA(145, 213, 255, 255),
    )?;
    draw_sdl_label(
        canvas,
        &format!(
            "PEER CODE {}",
            empty_code_placeholder(&panel.remote_peer_id)
        ),
        22,
        94,
        4,
        Color::RGBA(238, 242, 248, 255),
    )?;
    draw_sdl_label(
        canvas,
        &format!("STATUS {}", trim_status(&panel.status)),
        22,
        136,
        3,
        status_color(network),
    )?;
    draw_sdl_label(
        canvas,
        &trim_status(&panel.input_status),
        22,
        166,
        3,
        Color::RGBA(160, 170, 184, 255),
    )?;
    draw_sdl_label(
        canvas,
        "ENTER CONNECTS  ESC QUITS  S STARTS",
        22,
        226,
        3,
        Color::RGBA(160, 170, 184, 255),
    )?;
    if let FriendConnectNetwork::Connected(game) = network {
        if !game.started && game.lobby_owner {
            draw_sdl_rect(
                canvas,
                RenderRect {
                    x: 382,
                    y: 188,
                    width: 106,
                    height: 28,
                    color: RenderColor {
                        r: 55,
                        g: 150,
                        b: 96,
                        a: 255,
                    },
                },
            )?;
            draw_sdl_label(
                canvas,
                "START",
                404,
                196,
                3,
                Color::RGBA(245, 248, 255, 255),
            )?;
        }
    }
    if !canvas.present() {
        return Err("SDL friend-connect present failed".to_string());
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_supabase_config() -> Result<SupabaseRealtimeConfig, String> {
    let url = std::env::var("MOLE_SUPABASE_URL")
        .unwrap_or_else(|_| FRIEND_CONNECT_SUPABASE_URL.to_string());
    let key = std::env::var("MOLE_SUPABASE_PUBLISHABLE_KEY")
        .unwrap_or_else(|_| FRIEND_CONNECT_SUPABASE_PUBLISHABLE_KEY.to_string());
    SupabaseRealtimeConfig::new(url, key)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn generate_friend_peer_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    let mixed = now ^ (u64::from(std::process::id()) << 17);
    format!("M{:05X}", mixed & 0x000f_ffff)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_code_character(keycode: Keycode) -> Option<char> {
    match keycode {
        Keycode::A => Some('A'),
        Keycode::B => Some('B'),
        Keycode::C => Some('C'),
        Keycode::D => Some('D'),
        Keycode::E => Some('E'),
        Keycode::F => Some('F'),
        Keycode::G => Some('G'),
        Keycode::H => Some('H'),
        Keycode::I => Some('I'),
        Keycode::J => Some('J'),
        Keycode::K => Some('K'),
        Keycode::L => Some('L'),
        Keycode::M => Some('M'),
        Keycode::N => Some('N'),
        Keycode::O => Some('O'),
        Keycode::P => Some('P'),
        Keycode::Q => Some('Q'),
        Keycode::R => Some('R'),
        Keycode::S => Some('S'),
        Keycode::T => Some('T'),
        Keycode::U => Some('U'),
        Keycode::V => Some('V'),
        Keycode::W => Some('W'),
        Keycode::X => Some('X'),
        Keycode::Y => Some('Y'),
        Keycode::Z => Some('Z'),
        Keycode::_0 | Keycode::Kp0 => Some('0'),
        Keycode::_1 | Keycode::Kp1 => Some('1'),
        Keycode::_2 | Keycode::Kp2 => Some('2'),
        Keycode::_3 | Keycode::Kp3 => Some('3'),
        Keycode::_4 | Keycode::Kp4 => Some('4'),
        Keycode::_5 | Keycode::Kp5 => Some('5'),
        Keycode::_6 | Keycode::Kp6 => Some('6'),
        Keycode::_7 | Keycode::Kp7 => Some('7'),
        Keycode::_8 | Keycode::Kp8 => Some('8'),
        Keycode::_9 | Keycode::Kp9 => Some('9'),
        _ => None,
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn empty_code_placeholder(code: &str) -> &str {
    if code.is_empty() {
        "------"
    } else {
        code
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn trim_status(status: &str) -> String {
    const MAX_CHARS: usize = 42;
    let mut trimmed = status
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '.') {
                character.to_ascii_uppercase()
            } else {
                ' '
            }
        })
        .take(MAX_CHARS)
        .collect::<String>();
    while trimmed.ends_with(' ') {
        trimmed.pop();
    }
    trimmed
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn status_color(network: &FriendConnectNetwork) -> Color {
    match network {
        FriendConnectNetwork::Connected(_) => Color::RGBA(116, 255, 178, 255),
        FriendConnectNetwork::Failed => Color::RGBA(255, 124, 124, 255),
        FriendConnectNetwork::Pending(_) => Color::RGBA(255, 216, 120, 255),
        FriendConnectNetwork::Editing => Color::RGBA(238, 242, 248, 255),
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn start_button_contains(x: f32, y: f32) -> bool {
    (382.0..=488.0).contains(&x) && (188.0..=216.0).contains(&y)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn run_udp_sdl(
    frames: u32,
    config: mole_runtime::UdpRuntimeConfig,
    replay_path: Option<&Path>,
    frame_log: bool,
    input_trace: bool,
    ucf_enabled: bool,
) -> Result<(), String> {
    mole_runtime::preload_runtime_source_frame_data()?;
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let video = sdl.video().map_err(|error| error.to_string())?;
    let window = video
        .window("Mole Rust UDP SDL Runtime", 960, 540)
        .position_centered()
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window.into_canvas();
    disable_sdl_renderer_vsync(&canvas)?;
    let texture_creator = canvas.texture_creator();
    let mut texture_cache =
        SdlTextureCache::new(&texture_creator, mole_runtime::project_asset_root());
    let mut sdl_shell_input = SdlInputSource::new(&sdl)?;
    let mut local_input_source =
        WupInputSource::open_with_config(mole_runtime::WupInputConfig { ucf_enabled })?;
    let mut input_trace_writer = create_input_trace_writer(input_trace)?;
    let transport = UdpTransport::bind(config.local_addr, config.peer_addr)
        .map_err(|error| error.to_string())?;
    let initial = mole_runtime::default_play_world();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));
    let mut inbox = InputPacketInbox::default();
    let mut stats = mole_runtime::UdpRuntimeStats::default();
    let remote_player = 1 - config.player_index;
    let _timer_resolution = request_high_resolution_frame_timer();
    let frame_budget = frame_pacing_budget_duration();
    let mut next_frame_deadline = Instant::now() + frame_budget;

    for frame_number in 0..frames {
        let frame = Frame(frame_number);
        let _ = mole_runtime::InputSource::poll_inputs(&mut sdl_shell_input, frame);
        let before_trace_frame = input_trace_writer
            .as_ref()
            .map(|_| mole_runtime::RenderFrame::from_world(&world));
        let (polled_inputs, local_trace) = if input_trace_writer.is_some() {
            poll_traced_wup_inputs(&mut local_input_source)
        } else {
            (
                mole_runtime::InputSource::poll_inputs(&mut local_input_source, frame),
                None,
            )
        };
        drain_udp_packets(&transport, &mut inbox, &mut stats, frame)?;

        let mut inputs = [PlayerInput::neutral(), PlayerInput::neutral()];
        inputs[config.player_index as usize] = polled_inputs[config.player_index as usize];
        if let Some(remote_input) = inbox.input(frame, remote_player) {
            inputs[remote_player as usize] = remote_input;
        } else {
            stats.record_missing_remote_frame();
        }

        mole_runtime::step_world_with_source_collisions(&mut world, frame, &inputs);

        let local_packet = InputPacket::new(
            frame,
            config.player_index,
            inputs[config.player_index as usize],
            world.checksum(),
        )
        .with_timing_probe(frame.0, stats.last_remote_sequence.unwrap_or(0));
        transport
            .send_packet(local_packet)
            .map_err(|error| error.to_string())?;
        stats.record_sent();

        if let Some(capture) = replay_capture.as_mut() {
            capture.record_frame(frame, inputs, world.checksum());
        }

        let render_frame = mole_runtime::RenderFrame::from_world(&world);
        if let (Some(writer), Some(trace), Some(before)) = (
            input_trace_writer.as_mut(),
            local_trace.as_ref(),
            before_trace_frame.as_ref(),
        ) {
            writer
                .write_line(&mole_runtime::ControllerInputTraceLog::from_wup_trace(
                    frame,
                    trace,
                    before,
                    &render_frame,
                ))
                .map_err(|error| error.to_string())?;
        }
        let overlay = DebugOverlay::from_frame_with_udp_stats(&render_frame, &stats);
        let (width, height) = canvas.output_size().map_err(|error| error.to_string())?;
        let scene = RenderScene::from_frame(&render_frame, width, height);
        if frame_log {
            println!(
                "{}",
                mole_runtime::FrameDebugLog::from_frame_and_scene(&render_frame, &scene, inputs)
                    .to_json_line()
            );
        }
        draw_sdl_scene(
            &mut canvas,
            &scene,
            Some(&overlay),
            Some(&mut texture_cache),
        )?;

        if sdl_shell_input.quit_requested() {
            break;
        }

        wait_until_frame_deadline(next_frame_deadline);
        next_frame_deadline += frame_budget;
    }
    drain_udp_packets(&transport, &mut inbox, &mut stats, world.frame())?;

    if let (Some(path), Some(capture)) = (replay_path, replay_capture.as_ref()) {
        mole_runtime::write_replay_capture(path, capture).map_err(|error| error.to_string())?;
        println!("replay_path={}", path.display());
    }

    println!(
        "final_frame={} checksum={} input_backend=wup sdl_gamepads={} udp_sent={} udp_recv={} udp_dup={} udp_unsupported={} udp_missing={} udp_last_remote_frame={:?} udp_last_remote_checksum={:?} udp_rtt_frames={:?}",
        world.frame().0,
        world.checksum(),
        sdl_shell_input.gamepad_count(),
        stats.sent_packets,
        stats.received_packets,
        stats.duplicate_packets,
        stats.unsupported_packets,
        stats.missing_remote_frames,
        stats.last_remote_frame.map(|frame| frame.0),
        stats.last_remote_checksum,
        stats.last_rtt_frames
    );
    Ok(())
}

#[cfg(all(feature = "sdl", not(feature = "wup")))]
fn run_udp_sdl(
    _frames: u32,
    _config: mole_runtime::UdpRuntimeConfig,
    _replay_path: Option<&Path>,
    _frame_log: bool,
    _input_trace: bool,
    _timing: bool,
    #[cfg(feature = "wup")] _ucf_enabled: bool,
) -> Result<(), String> {
    Err(
        "UDP SDL runtime gameplay input requires native WUP: cargo run -p mole_runtime --features \"sdl wup\" -- --udp --sdl --local-addr <addr> --peer-addr <addr>"
            .to_string(),
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn run_sdl_smoke(
    frames: u32,
    replay_path: Option<&Path>,
    frame_log: bool,
    input_trace: bool,
    timing: bool,
    frame_cap_enabled: bool,
    ucf_enabled: bool,
) -> Result<(), String> {
    mole_runtime::preload_runtime_source_frame_data()?;
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let video = sdl.video().map_err(|error| error.to_string())?;
    let window = video
        .window("Mole Rust SDL3 Runtime", 960, 540)
        .position_centered()
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window.into_canvas();
    disable_sdl_renderer_vsync(&canvas)?;
    let texture_creator = canvas.texture_creator();
    let mut texture_cache =
        SdlTextureCache::new(&texture_creator, mole_runtime::project_asset_root());

    let mut sdl_shell_input = SdlInputSource::new(&sdl)?;
    let mut gameplay_input_source =
        WupInputSource::open_with_config(mole_runtime::WupInputConfig { ucf_enabled })?;
    let mut input_trace_writer = create_input_trace_writer(input_trace)?;
    let initial = mole_runtime::default_play_world();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));
    let mut timing_stats = SdlFrameTimingStats::default();
    let _timer_resolution = request_high_resolution_frame_timer();
    let frame_budget = frame_pacing_budget_duration();
    let mut next_frame_deadline = Instant::now() + frame_budget;

    for frame in 0..frames {
        let frame_started = Instant::now();
        let frame = Frame(frame);
        let input_started = Instant::now();
        let _ = mole_runtime::InputSource::poll_inputs(&mut sdl_shell_input, frame);
        let before_trace_frame = input_trace_writer
            .as_ref()
            .map(|_| mole_runtime::RenderFrame::from_world(&world));
        let (inputs, wup_trace) = if input_trace_writer.is_some() {
            poll_traced_wup_inputs(&mut gameplay_input_source)
        } else {
            (
                mole_runtime::InputSource::poll_inputs(&mut gameplay_input_source, frame),
                None,
            )
        };
        let input_elapsed = input_started.elapsed();
        let sim_started = Instant::now();
        mole_runtime::step_world_with_source_collisions(&mut world, frame, &inputs);
        let sim_elapsed = sim_started.elapsed();
        if let Some(capture) = replay_capture.as_mut() {
            capture.record_frame(frame, inputs, world.checksum());
        }
        let scene_started = Instant::now();
        let render_frame = mole_runtime::RenderFrame::from_world(&world);
        if let (Some(writer), Some(trace), Some(before)) = (
            input_trace_writer.as_mut(),
            wup_trace.as_ref(),
            before_trace_frame.as_ref(),
        ) {
            writer
                .write_line(&mole_runtime::ControllerInputTraceLog::from_wup_trace(
                    frame,
                    trace,
                    before,
                    &render_frame,
                ))
                .map_err(|error| error.to_string())?;
        }
        let overlay = DebugOverlay::from_frame(&render_frame);
        let (width, height) = canvas.output_size().map_err(|error| error.to_string())?;
        let scene = RenderScene::from_frame(&render_frame, width, height);
        let scene_elapsed = scene_started.elapsed();
        if frame_log {
            println!(
                "{}",
                mole_runtime::FrameDebugLog::from_frame_and_scene(&render_frame, &scene, inputs)
                    .to_json_line()
            );
        }
        let draw_started = Instant::now();
        draw_sdl_scene(
            &mut canvas,
            &scene,
            Some(&overlay),
            Some(&mut texture_cache),
        )?;
        let draw_elapsed = draw_started.elapsed();

        if sdl_shell_input.quit_requested() {
            break;
        }

        let sleep_started = Instant::now();
        if frame_cap_enabled {
            wait_until_frame_deadline(next_frame_deadline);
            next_frame_deadline += frame_budget;
        }
        timing_stats.record(
            input_elapsed,
            sim_elapsed,
            scene_elapsed,
            draw_elapsed,
            sleep_started.elapsed(),
            frame_started.elapsed(),
        );
    }

    if let (Some(path), Some(capture)) = (replay_path, replay_capture.as_ref()) {
        mole_runtime::write_replay_capture(path, capture).map_err(|error| error.to_string())?;
        println!("replay_path={}", path.display());
    }

    println!(
        "final_frame={} checksum={} input_backend=wup sdl_gamepads={}",
        world.frame().0,
        world.checksum(),
        sdl_shell_input.gamepad_count()
    );
    if timing {
        println!("{}", timing_stats.summary_json(frame_cap_enabled));
    }
    Ok(())
}

#[cfg(all(feature = "sdl", not(feature = "wup")))]
fn run_sdl_smoke(
    _frames: u32,
    _replay_path: Option<&Path>,
    _frame_log: bool,
    _input_trace: bool,
    _timing: bool,
    _frame_cap_enabled: bool,
    #[cfg(feature = "wup")] _ucf_enabled: bool,
) -> Result<(), String> {
    Err(
        "SDL3 runtime gameplay input requires native WUP: cargo run -p mole_runtime --features \"sdl wup\" -- --sdl"
            .to_string(),
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn create_input_trace_writer(
    enabled: bool,
) -> Result<Option<mole_runtime::InputTraceWriter>, String> {
    if !enabled {
        return Ok(None);
    }

    let writer = mole_runtime::InputTraceWriter::create_default()
        .map_err(|error| format!("failed to create controller input trace log: {error}"))?;
    println!("input_trace_path={}", writer.path().display());
    Ok(Some(writer))
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn poll_traced_wup_inputs(
    input_source: &mut WupInputSource,
) -> ([PlayerInput; 2], Option<mole_runtime::WupInputTrace>) {
    match input_source.poll_traced_adapter() {
        Ok(trace) => (trace.inputs, Some(trace)),
        Err(rusb::Error::Timeout) => (input_source.latest_inputs(), input_source.latest_trace()),
        Err(error) => {
            eprintln!("WUP read failed; neutralizing gameplay input for this frame: {error}");
            ([PlayerInput::neutral(), PlayerInput::neutral()], None)
        }
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_scene(
    canvas: &mut WindowCanvas,
    scene: &RenderScene,
    overlay: Option<&DebugOverlay>,
    mut texture_cache: Option<&mut SdlTextureCache<'_>>,
) -> Result<(), String> {
    const DRAW_LEGACY_PLAYER_SPRITES: bool = true;

    canvas.set_draw_color(sdl_color(scene.background));
    canvas.clear();
    if let Some(cache) = texture_cache.as_mut() {
        draw_sdl_image(
            canvas,
            cache,
            scene.background_image.relative_path,
            scene.background_image.rect,
            false,
        )?;
    }
    for surface in &scene.stage_surfaces {
        draw_sdl_rect(canvas, *surface)?;
    }
    for platform in scene.entry_platforms.into_iter().flatten() {
        draw_sdl_rect(canvas, platform)?;
    }
    if DRAW_LEGACY_PLAYER_SPRITES {
        if let Some(cache) = texture_cache.as_mut() {
            for (index, player) in scene.players.iter().copied().enumerate() {
                draw_sdl_image(
                    canvas,
                    cache,
                    &scene.player_sprites[index].relative_path(),
                    player,
                    false,
                )?;
            }
        } else {
            for player in scene.players {
                draw_sdl_rect(canvas, player)?;
            }
        }
    }
    for hurtbox in scene.player_hurtbox_pills.iter().flatten().copied() {
        draw_sdl_capsule(canvas, hurtbox)?;
    }
    for hitbox in scene.player_hitbox_pills.iter().flatten().copied() {
        draw_sdl_capsule(canvas, hitbox)?;
    }
    for shield in scene.player_shields.into_iter().flatten() {
        draw_sdl_circle(canvas, shield)?;
    }
    for ecb in scene.player_ecbs {
        draw_sdl_polygon(canvas, ecb)?;
    }
    if let Some(label) = scene.match_intro_label {
        draw_match_intro_label(canvas, label)?;
    }
    if let Some(overlay) = overlay {
        draw_debug_overlay(canvas, overlay)?;
    }
    if !canvas.present() {
        return Err("SDL present failed".to_string());
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
const UNCAPPED_WORK_BUDGET_TARGET_FPS: f64 = 240.0;
#[cfg(all(feature = "sdl", feature = "wup"))]
const CAPPED_RUNTIME_TARGET_FPS: f64 = 60.0;

#[cfg(all(feature = "sdl", feature = "wup"))]
#[derive(Default)]
struct SdlFrameTimingStats {
    frames: u32,
    input: Duration,
    sim: Duration,
    scene: Duration,
    draw: Duration,
    sleep: Duration,
    total: Duration,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
impl SdlFrameTimingStats {
    fn record(
        &mut self,
        input: Duration,
        sim: Duration,
        scene: Duration,
        draw: Duration,
        sleep: Duration,
        total: Duration,
    ) {
        self.frames += 1;
        self.input += input;
        self.sim += sim;
        self.scene += scene;
        self.draw += draw;
        self.sleep += sleep;
        self.total += total;
    }

    fn summary_json(&self, frame_cap_enabled: bool) -> String {
        let work = self.input + self.sim + self.scene + self.draw;
        let avg_work_ms = avg_ms(work, self.frames);
        let uncapped_work_fps = fps_from_ms(avg_work_ms);
        format!(
            "{{\"timing\":\"sdl\",\"frames\":{},\"frame_cap_enabled\":{},\"cap_target_fps\":{:.1},\"uncapped_budget_target_fps\":{:.1},\"uncapped_budget_ms\":{:.3},\"avg_input_ms\":{:.3},\"avg_sim_ms\":{:.3},\"avg_scene_ms\":{:.3},\"avg_draw_present_ms\":{:.3},\"avg_work_ms\":{:.3},\"uncapped_work_fps\":{:.1},\"avg_sleep_ms\":{:.3},\"avg_total_ms\":{:.3}}}",
            self.frames,
            frame_cap_enabled,
            CAPPED_RUNTIME_TARGET_FPS,
            UNCAPPED_WORK_BUDGET_TARGET_FPS,
            1000.0 / UNCAPPED_WORK_BUDGET_TARGET_FPS,
            avg_ms(self.input, self.frames),
            avg_ms(self.sim, self.frames),
            avg_ms(self.scene, self.frames),
            avg_ms(self.draw, self.frames),
            avg_work_ms,
            uncapped_work_fps,
            avg_ms(self.sleep, self.frames),
            avg_ms(self.total, self.frames),
        )
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn avg_ms(duration: Duration, frames: u32) -> f64 {
    if frames == 0 {
        return 0.0;
    }
    duration.as_secs_f64() * 1000.0 / f64::from(frames)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn fps_from_ms(avg_ms: f64) -> f64 {
    if avg_ms <= f64::EPSILON {
        return 0.0;
    }
    1000.0 / avg_ms
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn disable_sdl_renderer_vsync(canvas: &WindowCanvas) -> Result<(), String> {
    let disabled = unsafe {
        sdl3::sys::render::SDL_SetRenderVSync(
            canvas.raw(),
            sdl3::sys::render::SDL_RENDERER_VSYNC_DISABLED,
        )
    };
    if disabled {
        Ok(())
    } else {
        Err(format!(
            "failed to disable SDL renderer vsync: {}",
            sdl3::get_error()
        ))
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_match_intro_label(canvas: &mut WindowCanvas, label: &str) -> Result<(), String> {
    let x = centered_debug_label_x(canvas, label, 8)?;
    draw_sdl_label(canvas, label, x, 72, 8, Color::RGBA(245, 248, 255, 255))
}

#[cfg(all(feature = "sdl", feature = "wup"))]
struct SdlTextureCache<'a> {
    asset_root: PathBuf,
    texture_creator: &'a TextureCreator<WindowContext>,
    textures: HashMap<String, Texture<'a>>,
}

#[cfg(all(feature = "sdl", feature = "wup"))]
impl<'a> SdlTextureCache<'a> {
    fn new(texture_creator: &'a TextureCreator<WindowContext>, asset_root: PathBuf) -> Self {
        Self {
            asset_root,
            texture_creator,
            textures: HashMap::new(),
        }
    }

    fn texture(&mut self, relative_path: &str) -> Result<&Texture<'a>, String> {
        if !self.textures.contains_key(relative_path) {
            let texture = self.load_texture(relative_path)?;
            self.textures.insert(relative_path.to_string(), texture);
        }

        self.textures
            .get(relative_path)
            .ok_or_else(|| format!("texture cache missed {relative_path}"))
    }

    fn load_texture(&self, relative_path: &str) -> Result<Texture<'a>, String> {
        let path = self.asset_root.join(relative_path);
        let image = image::ImageReader::open(&path)
            .map_err(|error| format!("failed to open {}: {error}", path.display()))?
            .decode()
            .map_err(|error| format!("failed to decode {}: {error}", path.display()))?
            .to_rgba8();
        let (width, height) = image.dimensions();
        let mut texture = self
            .texture_creator
            .create_texture_static(PixelFormat::RGBA32, width, height)
            .map_err(|error| error.to_string())?;

        texture
            .update(None, image.as_raw(), width as usize * 4)
            .map_err(|error| error.to_string())?;
        texture.set_blend_mode(BlendMode::Blend);

        Ok(texture)
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_image(
    canvas: &mut WindowCanvas,
    texture_cache: &mut SdlTextureCache<'_>,
    relative_path: &str,
    rect: RenderRect,
    flip_x: bool,
) -> Result<(), String> {
    let texture = texture_cache.texture(relative_path)?;
    let dst = FRect::new(
        rect.x as f32,
        rect.y as f32,
        rect.width as f32,
        rect.height as f32,
    );

    if flip_x {
        canvas
            .copy_ex(texture, None, Some(dst), 0.0, None, true, false)
            .map_err(|error| error.to_string())
    } else {
        canvas
            .copy(texture, None, Some(dst))
            .map_err(|error| error.to_string())
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_polygon(canvas: &mut WindowCanvas, polygon: RenderPolygon) -> Result<(), String> {
    let points = polygon.points;
    for index in 0..points.len() {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        draw_sdl_line(canvas, start.x, start.y, end.x, end.y, polygon.color)?;
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_circle(
    canvas: &mut WindowCanvas,
    circle: mole_runtime::RenderCircle,
) -> Result<(), String> {
    canvas.set_blend_mode(BlendMode::Blend);
    canvas.set_draw_color(sdl_color(circle.color));
    let radius = circle.radius as i32;
    let radius_squared = radius * radius;

    for dy in -radius..=radius {
        let half_width = ((radius_squared - dy * dy) as f32).sqrt().round() as i32;
        canvas
            .draw_line(
                Point::new(circle.center.x - half_width, circle.center.y + dy),
                Point::new(circle.center.x + half_width, circle.center.y + dy),
            )
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_capsule(canvas: &mut WindowCanvas, capsule: RenderCapsule) -> Result<(), String> {
    canvas.set_blend_mode(BlendMode::Blend);
    canvas.set_draw_color(sdl_color(capsule.color));
    let radius = capsule.radius as i32;
    if capsule.a == capsule.b {
        return draw_sdl_circle(
            canvas,
            mole_runtime::RenderCircle {
                center: capsule.a,
                radius: capsule.radius,
                color: capsule.color,
            },
        );
    }

    let dx = (capsule.b.x - capsule.a.x) as f32;
    let dy = (capsule.b.y - capsule.a.y) as f32;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= f32::EPSILON {
        return Ok(());
    }
    let ux = dx / length;
    let uy = dy / length;
    let nx = -uy;
    let ny = ux;
    let radius_squared = radius * radius;

    for offset in -radius..=radius {
        let cap_extension = ((radius_squared - offset * offset) as f32).sqrt();
        let offset = offset as f32;
        let start_x = capsule.a.x as f32 + nx * offset - ux * cap_extension;
        let start_y = capsule.a.y as f32 + ny * offset - uy * cap_extension;
        let end_x = capsule.b.x as f32 + nx * offset + ux * cap_extension;
        let end_y = capsule.b.y as f32 + ny * offset + uy * cap_extension;
        canvas
            .draw_line(
                Point::new(start_x.round() as i32, start_y.round() as i32),
                Point::new(end_x.round() as i32, end_y.round() as i32),
            )
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_line(
    canvas: &mut WindowCanvas,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: RenderColor,
) -> Result<(), String> {
    canvas.set_draw_color(sdl_color(color));
    canvas
        .draw_line(Point::new(x1, y1), Point::new(x2, y2))
        .map_err(|error| error.to_string())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_rect(canvas: &mut WindowCanvas, rect: RenderRect) -> Result<(), String> {
    canvas.set_draw_color(sdl_color(rect.color));
    canvas
        .fill_rect(FRect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ))
        .map_err(|error| error.to_string())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_debug_overlay(canvas: &mut WindowCanvas, overlay: &DebugOverlay) -> Result<(), String> {
    let state_color = Color::RGBA(0, 0, 0, 255);
    for (index, line) in overlay.player_state_lines.iter().enumerate() {
        let x = centered_debug_label_x(canvas, line, 3)?;
        draw_sdl_label(canvas, line, x, 18 + index as i32 * 18, 3, state_color)?;
    }

    let color = Color::RGBA(235, 240, 248, 255);
    for (index, line) in overlay.lines.iter().enumerate() {
        draw_sdl_label(canvas, line, 12, 12 + index as i32 * 18, 3, color)?;
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn centered_debug_label_x(canvas: &WindowCanvas, text: &str, scale: i32) -> Result<i32, String> {
    let (width, _height) = canvas.output_size().map_err(|error| error.to_string())?;
    let label_width = debug_label_width(text, scale);
    Ok(((width as i32 - label_width) / 2).max(0))
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn debug_label_width(text: &str, scale: i32) -> i32 {
    text.chars().count() as i32 * scale * 4
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_label(
    canvas: &mut WindowCanvas,
    text: &str,
    x: i32,
    y: i32,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    canvas.set_draw_color(color);
    let mut cursor_x = x;
    for character in text.chars() {
        if character == ' ' {
            cursor_x += scale * 4;
            continue;
        }

        for (row, pattern) in debug_glyph(character).iter().enumerate() {
            for (column, pixel) in pattern.chars().enumerate() {
                if pixel == '1' {
                    canvas
                        .fill_rect(FRect::new(
                            (cursor_x + column as i32 * scale) as f32,
                            (y + row as i32 * scale) as f32,
                            scale as f32,
                            scale as f32,
                        ))
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        cursor_x += scale * 4;
    }
    Ok(())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn debug_glyph(character: char) -> [&'static str; 5] {
    match character {
        '0' => ["111", "101", "101", "101", "111"],
        '1' => ["010", "110", "010", "010", "111"],
        '2' => ["111", "001", "111", "100", "111"],
        '3' => ["111", "001", "111", "001", "111"],
        '4' => ["101", "101", "111", "001", "001"],
        '5' => ["111", "100", "111", "001", "111"],
        '6' => ["111", "100", "111", "101", "111"],
        '7' => ["111", "001", "010", "010", "010"],
        '8' => ["111", "101", "111", "101", "111"],
        '9' => ["111", "101", "111", "001", "111"],
        'A' => ["010", "101", "111", "101", "101"],
        'B' => ["110", "101", "110", "101", "110"],
        'C' => ["111", "100", "100", "100", "111"],
        'D' => ["110", "101", "101", "101", "110"],
        'E' => ["111", "100", "110", "100", "111"],
        'F' => ["111", "100", "110", "100", "100"],
        'G' => ["111", "100", "101", "101", "111"],
        'H' => ["101", "101", "111", "101", "101"],
        'I' => ["111", "010", "010", "010", "111"],
        'J' => ["001", "001", "001", "101", "111"],
        'K' => ["101", "101", "110", "101", "101"],
        'L' => ["100", "100", "100", "100", "111"],
        'M' => ["101", "111", "111", "101", "101"],
        'N' => ["101", "111", "111", "111", "101"],
        'O' => ["111", "101", "101", "101", "111"],
        'P' => ["110", "101", "110", "100", "100"],
        'Q' => ["111", "101", "101", "111", "001"],
        'R' => ["110", "101", "110", "101", "101"],
        'S' => ["111", "100", "111", "001", "111"],
        'T' => ["111", "010", "010", "010", "010"],
        'U' => ["101", "101", "101", "101", "111"],
        'V' => ["101", "101", "101", "101", "010"],
        'W' => ["101", "101", "111", "111", "101"],
        'X' => ["101", "101", "010", "101", "101"],
        'Y' => ["101", "101", "010", "010", "010"],
        'Z' => ["111", "001", "010", "100", "111"],
        '-' => ["000", "000", "111", "000", "000"],
        '.' => ["000", "000", "000", "000", "010"],
        ':' => ["000", "010", "000", "010", "000"],
        _ => ["111", "001", "011", "000", "010"],
    }
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn sdl_color(color: RenderColor) -> Color {
    Color::RGBA(color.r, color.g, color.b, color.a)
}

#[cfg(feature = "sdl")]
fn list_sdl_inputs() -> Result<(), String> {
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let gamepad = sdl.gamepad().map_err(|error| error.to_string())?;
    let joystick = sdl.joystick().map_err(|error| error.to_string())?;

    let gamepads = gamepad.gamepads().map_err(|error| error.to_string())?;
    println!("gamepads={}", gamepads.len());
    for id in gamepads {
        println!("gamepad id={}", u32::from(id));
        println!(
            "  name={}",
            gamepad.name_for_id(id).map_err(|error| error.to_string())?
        );
        println!("  vendor={:?}", gamepad.vendor_for_id(id));
        println!("  product={:?}", gamepad.product_for_id(id));
        println!("  type={:?}", gamepad.type_for_id(id));
        println!("  real_type={:?}", gamepad.real_type_for_id(id));
        println!("  path={:?}", gamepad.path_for_id(id).ok());
        println!("  mapping={:?}", gamepad.mapping_for_id(id));
    }

    let joysticks = joystick.joysticks().map_err(|error| error.to_string())?;
    println!("joysticks={}", joysticks.len());
    for id in joysticks {
        println!("joystick id={}", u32::from(id));
        if let Ok(opened) = joystick.open(id) {
            println!("  name={}", opened.name());
            println!("  axes={}", opened.num_axes());
            println!("  buttons={}", opened.num_buttons());
            println!("  connected={}", opened.connected());
        }
    }

    Ok(())
}

#[cfg(feature = "wup")]
fn check_wup_native() -> Result<(), String> {
    let mut input_source = WupInputSource::open()?;
    let ports = input_source
        .poll_ports()
        .map_err(|error| error.to_string())?;
    for (index, port) in ports.iter().enumerate() {
        let input = mole_runtime::map_gamecube_pad_to_player_input(port.pad);
        let (c_x, c_y) = port.pad.c_stick_i16();
        println!(
            "port={} connected={} bits={} raw_main=({}, {}) raw_c=({}, {}) main=({}, {}) c=({}, {}) triggers=({}, {}) digital_l={} digital_r={} dpad=({}, {}, {}, {})",
            index + 1,
            port.connected,
            input.bits(),
            port.pad.stick_x,
            port.pad.stick_y,
            port.pad.c_stick_x,
            port.pad.c_stick_y,
            input.stick_x(),
            input.stick_y(),
            c_x,
            c_y,
            port.pad.left_trigger,
            port.pad.right_trigger,
            port.pad.buttons.l(),
            port.pad.buttons.r(),
            port.pad.buttons.dpad_up(),
            port.pad.buttons.dpad_down(),
            port.pad.buttons.dpad_left(),
            port.pad.buttons.dpad_right()
        );
    }
    Ok(())
}

#[cfg(feature = "wup")]
fn stream_wup_native(frames: u32) -> Result<(), String> {
    let mut input_source = WupInputSource::open()?;
    let mut mapper = mole_runtime::WupInputMapper::default();
    let mut ports = [mole_runtime::WupPort::default(); 4];
    let _timer_resolution = request_high_resolution_frame_timer();
    let frame_budget = frame_pacing_budget_duration();
    let mut next_frame_deadline = Instant::now() + frame_budget;

    for _frame in 0..frames {
        match input_source.poll_ports() {
            Ok(next_ports) => ports = next_ports,
            Err(rusb::Error::Timeout) => {}
            Err(error) => {
                eprintln!("WUP read failed; keeping last native sample: {error}");
            }
        }
        let melee = mapper.map_ports_to_melee_snapshots(ports);
        println!(
            "{}",
            mole_runtime::InputReadout::from_wup_ports_with_melee_snapshots(ports, melee)
                .to_json_line()
        );
        wait_until_frame_deadline(next_frame_deadline);
        next_frame_deadline += frame_budget;
    }

    Ok(())
}

#[cfg(feature = "wup")]
fn run_wup_smoke(frames: u32, replay_path: Option<&Path>, ucf_enabled: bool) -> Result<(), String> {
    let mut input_source =
        WupInputSource::open_with_config(mole_runtime::WupInputConfig { ucf_enabled })?;
    let initial = mole_runtime::default_play_world();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));
    let _timer_resolution = request_high_resolution_frame_timer();
    let frame_budget = frame_pacing_budget_duration();
    let mut next_frame_deadline = Instant::now() + frame_budget;

    for frame in 0..frames {
        let frame = Frame(frame);
        let inputs =
            mole_runtime::step_world_from_input_source(&mut world, &mut input_source, frame);
        if let Some(capture) = replay_capture.as_mut() {
            capture.record_frame(frame, inputs, world.checksum());
        }
        wait_until_frame_deadline(next_frame_deadline);
        next_frame_deadline += frame_budget;
    }

    if let (Some(path), Some(capture)) = (replay_path, replay_capture.as_ref()) {
        mole_runtime::write_replay_capture(path, capture).map_err(|error| error.to_string())?;
        println!("replay_path={}", path.display());
    }

    println!(
        "final_frame={} checksum={} input_backend=wup",
        world.frame().0,
        world.checksum()
    );
    Ok(())
}

#[cfg(all(feature = "wup", target_os = "windows"))]
#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period_ms: u32) -> u32;
    fn timeEndPeriod(period_ms: u32) -> u32;
}

#[cfg(all(feature = "wup", target_os = "windows"))]
struct HighResolutionFrameTimer {
    period_ms: u32,
}

#[cfg(all(feature = "wup", target_os = "windows"))]
impl Drop for HighResolutionFrameTimer {
    fn drop(&mut self) {
        unsafe {
            let _ = timeEndPeriod(self.period_ms);
        }
    }
}

#[cfg(all(feature = "wup", target_os = "windows"))]
fn request_high_resolution_frame_timer() -> Option<HighResolutionFrameTimer> {
    let period_ms = 1;
    let result = unsafe { timeBeginPeriod(period_ms) };
    (result == 0).then_some(HighResolutionFrameTimer { period_ms })
}

#[cfg(all(feature = "wup", not(target_os = "windows")))]
fn request_high_resolution_frame_timer() -> Option<()> {
    None
}

#[cfg(feature = "wup")]
fn frame_pacing_budget_duration() -> Duration {
    Duration::from_nanos(mole_runtime::frame_pacing_sleep_nanos(0))
}

#[cfg(feature = "wup")]
fn wait_until_frame_deadline(deadline: Instant) {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }

        let remaining_nanos = remaining.as_nanos().min(u128::from(u64::MAX)) as u64;
        let coarse_sleep_nanos = mole_runtime::frame_pacing_coarse_sleep_nanos(remaining_nanos);
        if coarse_sleep_nanos > 0 {
            std::thread::sleep(Duration::from_nanos(coarse_sleep_nanos));
        } else {
            std::hint::spin_loop();
        }
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn parse_frames(args: &[String]) -> u32 {
    if let Some(value) = value_after(args, "--frames") {
        return value.parse::<u32>().unwrap_or(120);
    }
    if has_flag(args, "--play") || has_flag(args, "--friend-connect") {
        return u32::MAX;
    }

    120
}

#[cfg_attr(not(feature = "wup"), allow(dead_code))]
fn parse_ucf_enabled(args: &[String]) -> bool {
    !has_flag(args, "--no-ucf")
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn parse_netplay_delay_frames(args: &[String]) -> u32 {
    value_after(args, "--netplay-delay")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(FRIEND_CONNECT_DEFAULT_INPUT_DELAY_FRAMES)
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn parse_friend_code_override(args: &[String]) -> Option<String> {
    value_after(args, "--friend-code").map(|value| value.trim().to_ascii_uppercase())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn parse_friend_connect_visual_join_code(args: &[String]) -> Option<String> {
    if has_flag(args, "--friend-connect-headless-peer") {
        return None;
    }
    value_after(args, "--connect-code").map(|value| value.trim().to_ascii_uppercase())
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn parse_friend_connect_window_offset(args: &[String]) -> (i32, i32) {
    (
        parse_i32_after(args, "--window-offset-x").unwrap_or_default(),
        parse_i32_after(args, "--window-offset-y").unwrap_or_default(),
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn parse_friend_connect_local_udp_addr(args: &[String]) -> Result<Option<SocketAddr>, String> {
    let Some(value) = value_after(args, "--friend-local-udp") else {
        return Ok(None);
    };
    value
        .parse::<SocketAddr>()
        .map(Some)
        .map_err(|error| format!("invalid --friend-local-udp socket address: {error}"))
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn friend_connect_cpu_headroom_hz(work_duration: Duration) -> u32 {
    let nanos = work_duration.as_nanos();
    if nanos == 0 {
        return u32::MAX;
    }
    let hz = 1_000_000_000u128 / nanos;
    hz.min(u128::from(u32::MAX)) as u32
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn parse_headless_friend_peer_config(
    args: &[String],
) -> Result<Option<HeadlessFriendPeerConfig>, String> {
    if !has_flag(args, "--friend-connect-headless-peer") {
        return Ok(None);
    }
    let host_code = value_after(args, "--connect-code").ok_or_else(|| {
        "--connect-code is required for --friend-connect-headless-peer".to_string()
    })?;
    let frames = value_after(args, "--frames")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(u32::MAX);

    Ok(Some(HeadlessFriendPeerConfig {
        host_code,
        frames,
        netplay_delay_frames: parse_netplay_delay_frames(args),
        player_index: 1,
        remote_player: 0,
    }))
}

fn parse_slippi_compare_mode(args: &[String]) -> mole_runtime::SlippiCoreComparisonMode {
    if has_flag(args, "--slippi-match-start") {
        mole_runtime::SlippiCoreComparisonMode::SequentialMatchStart
    } else {
        mole_runtime::SlippiCoreComparisonMode::SeededPreFrameDiagnostic
    }
}

fn parse_slippi_trace_player(args: &[String]) -> usize {
    value_after(args, "--slippi-trace-player")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|one_based| one_based.checked_sub(1))
        .unwrap_or_default()
}

fn parse_i32_after(args: &[String], flag: &str) -> Option<i32> {
    value_after(args, flag).and_then(|value| value.parse::<i32>().ok())
}

fn parse_replay_path(args: &[String], frames: u32) -> Option<PathBuf> {
    value_after(args, "--replay-path")
        .map(PathBuf::from)
        .or_else(|| {
            has_flag(args, "--record-replay").then(|| mole_runtime::native_replay_path(frames))
        })
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_frames, parse_i32_after, parse_slippi_compare_mode, parse_slippi_trace_player,
        parse_ucf_enabled,
    };
    #[cfg(all(feature = "sdl", feature = "wup"))]
    use std::collections::VecDeque;

    #[cfg(all(feature = "sdl", feature = "wup"))]
    use mole_core::{Frame, PlayerInput};
    #[cfg(all(feature = "sdl", feature = "wup"))]
    use mole_rollback::RollbackSession;
    use mole_runtime::SlippiCoreComparisonMode;
    #[cfg(all(feature = "sdl", feature = "wup"))]
    use mole_transport::{InputPacket, InputPacketDatagram, InputPacketInbox, UdpTransport};

    #[test]
    fn parse_frames_defaults_to_smoke_length() {
        assert_eq!(parse_frames(&[]), 120);
    }

    #[test]
    fn parse_frames_supports_play_until_quit_mode() {
        assert_eq!(parse_frames(&["--play".to_string()]), u32::MAX);
    }

    #[test]
    fn parse_frames_treats_friend_connect_as_play_until_quit_mode() {
        assert_eq!(parse_frames(&["--friend-connect".to_string()]), u32::MAX);
    }

    #[test]
    fn parse_frames_keeps_explicit_frame_counts_for_smoke_tests() {
        assert_eq!(
            parse_frames(&[
                "--play".to_string(),
                "--frames".to_string(),
                "2".to_string()
            ]),
            2
        );
    }

    #[test]
    fn parse_frames_keeps_explicit_friend_connect_frame_counts_for_smoke_tests() {
        assert_eq!(
            parse_frames(&[
                "--friend-connect".to_string(),
                "--frames".to_string(),
                "2".to_string()
            ]),
            2
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_netplay_delay_defaults_to_slippi_online_delay_frames() {
        assert_eq!(
            super::parse_netplay_delay_frames(&["--friend-connect".to_string()]),
            super::FRIEND_CONNECT_DEFAULT_INPUT_DELAY_FRAMES
        );
        assert_eq!(super::FRIEND_CONNECT_DEFAULT_INPUT_DELAY_FRAMES, 2);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_netplay_delay_supports_manual_override() {
        assert_eq!(
            super::parse_netplay_delay_frames(&[
                "--friend-connect".to_string(),
                "--netplay-delay".to_string(),
                "4".to_string()
            ]),
            4
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_friend_connect_local_udp_addr_supports_same_machine_visual_pair() {
        assert_eq!(
            super::parse_friend_connect_local_udp_addr(&[
                "--friend-local-udp".to_string(),
                "127.0.0.1:41001".to_string()
            ])
            .unwrap()
            .unwrap()
            .to_string(),
            "127.0.0.1:41001"
        );
        assert_eq!(
            super::parse_friend_connect_local_udp_addr(&[]).unwrap(),
            None
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_friend_connect_local_udp_addr_rejects_invalid_socket_address() {
        let error = super::parse_friend_connect_local_udp_addr(&[
            "--friend-local-udp".to_string(),
            "not-an-address".to_string(),
        ])
        .expect_err("invalid same-machine UDP address should be rejected");

        assert!(error.contains("--friend-local-udp"));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_friend_connect_visual_join_code_uses_connect_code_without_headless_mode() {
        assert_eq!(
            super::parse_friend_connect_visual_join_code(&[
                "--friend-connect".to_string(),
                "--connect-code".to_string(),
                "HOST42".to_string()
            ])
            .as_deref(),
            Some("HOST42")
        );
        assert_eq!(
            super::parse_friend_connect_visual_join_code(&[
                "--friend-connect-headless-peer".to_string(),
                "--connect-code".to_string(),
                "HOST42".to_string()
            ]),
            None
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_friend_connect_window_offset_supports_local_two_visual_clients() {
        assert_eq!(
            super::parse_friend_connect_window_offset(&[
                "--window-offset-x".to_string(),
                "440".to_string(),
                "--window-offset-y".to_string(),
                "260".to_string()
            ]),
            (440, 260)
        );
        assert_eq!(super::parse_friend_connect_window_offset(&[]), (0, 0));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_cpu_headroom_hz_reports_uncapped_work_rate() {
        assert_eq!(
            super::friend_connect_cpu_headroom_hz(std::time::Duration::from_millis(2)),
            500
        );
        assert_eq!(
            super::friend_connect_cpu_headroom_hz(std::time::Duration::from_millis(20)),
            50
        );
        assert_eq!(
            super::friend_connect_cpu_headroom_hz(std::time::Duration::ZERO),
            u32::MAX
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_friend_code_override_accepts_packaged_local_internet_code() {
        assert_eq!(
            super::parse_friend_code_override(&[
                "--friend-connect".to_string(),
                "--friend-code".to_string(),
                "LOCAL1".to_string()
            ])
            .as_deref(),
            Some("LOCAL1")
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_headless_friend_peer_config_defaults_to_joiner_slot_and_slippi_delay() {
        let config = super::parse_headless_friend_peer_config(&[
            "--friend-connect-headless-peer".to_string(),
            "--connect-code".to_string(),
            "ABCD12".to_string(),
        ])
        .expect("headless peer args should parse")
        .expect("headless peer mode should be enabled");

        assert_eq!(config.host_code, "ABCD12");
        assert_eq!(
            config.netplay_delay_frames,
            super::FRIEND_CONNECT_DEFAULT_INPUT_DELAY_FRAMES
        );
        assert_eq!(config.frames, u32::MAX);
        assert_eq!(config.player_index, 1);
        assert_eq!(config.remote_player, 0);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn parse_headless_friend_peer_config_requires_connect_code() {
        let error = super::parse_headless_friend_peer_config(&[
            "--friend-connect-headless-peer".to_string()
        ])
        .expect_err("headless peer mode should require a host code");

        assert!(error.contains("--connect-code"));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn headless_friend_peer_uses_deterministic_neutral_input_by_default() {
        assert_eq!(
            super::headless_friend_peer_local_input(Frame(0)),
            PlayerInput::neutral()
        );
        assert_eq!(
            super::headless_friend_peer_local_input(Frame(999)),
            PlayerInput::neutral()
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn headless_friend_peer_status_names_joiner_role_and_room() {
        assert_eq!(
            super::headless_friend_peer_status("JOINING", "ABCD12", Frame(7), 2),
            "HEADLESS P2 JOINING ABCD12 F7 D2"
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_local_packet_uses_slippi_future_frame_delay() {
        let mut delay = mole_rollback::SlippiInputDelayBuffer::new(2);
        let raw = PlayerInput::neutral().with_left_stick(127, 0);

        let delayed = super::friend_connect_stage_local_input(&mut delay, Frame(10), raw);
        let packet = super::friend_connect_local_input_packet(delayed, 0, 0xCAFE, 7);

        assert_eq!(delayed.current_frame_input, PlayerInput::neutral());
        assert_eq!(packet.frame, Frame(12));
        assert_eq!(packet.input, raw);
        assert_eq!(packet.sequence, 12);
        assert_eq!(packet.ack_sequence, 7);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_should_skip_online_frame_matches_slippi_rollback_lookahead() {
        assert!(!super::friend_connect_should_skip_online_frame(
            Frame(0),
            None
        ));
        assert!(!super::friend_connect_should_skip_online_frame(
            Frame(7),
            None
        ));
        assert!(super::friend_connect_should_skip_online_frame(
            Frame(8),
            None
        ));
        assert!(!super::friend_connect_should_skip_online_frame(
            Frame(17),
            Some(Frame(10))
        ));
        assert!(super::friend_connect_should_skip_online_frame(
            Frame(18),
            Some(Frame(10))
        ));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_time_sync_skips_early_lockstep_frame_when_local_is_ahead() {
        let start = std::time::Instant::now();
        let mut time_sync = super::FriendConnectTimeSync::new(start);
        time_sync.record_local_send(Frame(30), start);
        time_sync.record_remote_packet(
            InputPacket::new(Frame(30), 1, PlayerInput::neutral(), 0).with_timing_probe(30, 0),
            start + std::time::Duration::from_micros(60_000),
        );

        assert!(time_sync.should_skip_online_frame(Frame(30), Some(Frame(30))));
        assert!(time_sync.should_skip_online_frame(Frame(30), Some(Frame(30))));
        assert!(time_sync.should_skip_online_frame(Frame(30), Some(Frame(30))));
        assert!(!time_sync.should_skip_online_frame(Frame(30), Some(Frame(30))));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_time_sync_does_not_skip_inside_slippi_start_threshold() {
        let start = std::time::Instant::now();
        let mut time_sync = super::FriendConnectTimeSync::new(start);
        time_sync.record_local_send(Frame(30), start);
        time_sync.record_remote_packet(
            InputPacket::new(Frame(30), 1, PlayerInput::neutral(), 0).with_timing_probe(30, 0),
            start + std::time::Duration::from_micros(9_000),
        );

        assert!(!time_sync.should_skip_online_frame(Frame(30), Some(Frame(30))));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_time_sync_translates_slippi_dynamic_emulation_speed() {
        let start = std::time::Instant::now();
        let mut ahead = super::FriendConnectTimeSync::new(start);
        ahead.record_local_send(Frame(150), start);
        ahead.record_remote_packet(
            InputPacket::new(Frame(150), 1, PlayerInput::neutral(), 0).with_timing_probe(150, 0),
            start + std::time::Duration::from_micros(60_000),
        );

        let ahead_decision = ahead.advance_pacing_for_frame(Frame(150));
        assert_eq!(ahead_decision.speed_parts_per_million, 995_000);
        assert!(!ahead_decision.advance_online_frame);

        let mut behind = super::FriendConnectTimeSync::new(start);
        behind.record_local_send(Frame(150), start + std::time::Duration::from_micros(60_000));
        behind.record_remote_packet(
            InputPacket::new(Frame(150), 1, PlayerInput::neutral(), 0).with_timing_probe(150, 0),
            start,
        );

        let behind_decision = behind.advance_pacing_for_frame(Frame(150));
        assert_eq!(behind_decision.speed_parts_per_million, 1_010_000);
        assert!(behind_decision.advance_online_frame);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_time_sync_spaces_slippi_advance_frames_every_five_frames() {
        let start = std::time::Instant::now();
        let mut time_sync = super::FriendConnectTimeSync::new(start);
        time_sync.record_local_send(Frame(150), start + std::time::Duration::from_micros(60_000));
        time_sync.record_remote_packet(
            InputPacket::new(Frame(150), 1, PlayerInput::neutral(), 0).with_timing_probe(150, 0),
            start,
        );

        assert!(
            time_sync
                .advance_pacing_for_frame(Frame(150))
                .advance_online_frame
        );
        assert!(
            !time_sync
                .advance_pacing_for_frame(Frame(151))
                .advance_online_frame
        );
        assert!(
            time_sync
                .advance_pacing_for_frame(Frame(155))
                .advance_online_frame
        );
        assert!(
            time_sync
                .advance_pacing_for_frame(Frame(160))
                .advance_online_frame
        );
        assert!(
            !time_sync
                .advance_pacing_for_frame(Frame(165))
                .advance_online_frame
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_frame_budget_scales_with_slippi_emulation_speed() {
        let base = std::time::Duration::from_micros(16_683);

        assert_eq!(
            super::friend_connect_frame_budget_for_speed(base, 1_000_000),
            base
        );
        assert!(super::friend_connect_frame_budget_for_speed(base, 1_010_000) < base);
        assert!(super::friend_connect_frame_budget_for_speed(base, 995_000) > base);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_recent_input_queue_drops_acked_and_retains_repair_window() {
        let mut packets = VecDeque::new();
        for frame in 0..12 {
            super::push_friend_recent_input_packet(
                &mut packets,
                None,
                InputPacket::new(Frame(frame), 0, PlayerInput::neutral(), frame as u64)
                    .with_timing_probe(frame, 0),
            );
        }

        assert_eq!(packets.front().map(|packet| packet.sequence), Some(11));
        assert_eq!(
            packets
                .iter()
                .take(super::FRIEND_CONNECT_RECENT_INPUT_RETRANSMIT_FRAMES)
                .map(|packet| packet.sequence)
                .collect::<Vec<_>>(),
            vec![11, 10, 9, 8, 7, 6, 5, 4]
        );

        super::push_friend_recent_input_packet(
            &mut packets,
            Some(8),
            InputPacket::new(Frame(12), 0, PlayerInput::neutral(), 12).with_timing_probe(12, 0),
        );

        assert!(packets.iter().all(|packet| packet.sequence >= 8));
        assert_eq!(packets.back().map(|packet| packet.sequence), Some(8));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_first_send_queues_slippi_delay_neutral_pads() {
        let mut packets = VecDeque::new();
        let mut seeded_delay_pads = false;
        let raw = PlayerInput::neutral().with_left_stick(127, 0);
        let first = InputPacket::new(Frame(2), 0, raw, 0xCAFE).with_timing_probe(2, 0);

        super::push_friend_local_input_packet(&mut packets, None, &mut seeded_delay_pads, 2, first);

        assert!(seeded_delay_pads);
        assert_eq!(
            packets
                .iter()
                .map(|packet| (packet.frame.0, packet.input))
                .collect::<Vec<_>>(),
            vec![
                (2, raw),
                (1, PlayerInput::neutral()),
                (0, PlayerInput::neutral())
            ]
        );

        super::push_friend_local_input_packet(
            &mut packets,
            None,
            &mut seeded_delay_pads,
            2,
            InputPacket::new(Frame(3), 0, raw, 0xCAFE).with_timing_probe(3, 0),
        );

        assert_eq!(
            packets
                .iter()
                .map(|packet| packet.frame.0)
                .collect::<Vec<_>>(),
            vec![3, 2, 1, 0]
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_retransmit_window_reuses_existing_packets_without_staging_new_input() {
        let mut packets = VecDeque::new();
        for frame in 0..10 {
            packets.push_front(
                InputPacket::new(Frame(frame), 0, PlayerInput::neutral(), frame as u64)
                    .with_timing_probe(frame, 0),
            );
        }

        let window = super::friend_connect_recent_input_retransmit_packets(&packets);

        assert_eq!(
            window
                .iter()
                .map(|packet| packet.sequence)
                .collect::<Vec<_>>(),
            vec![9, 8, 7, 6, 5, 4, 3, 2]
        );
        assert_eq!(packets.len(), 10);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_retransmit_window_is_encoded_as_one_slippi_style_bundle() {
        let mut packets = VecDeque::new();
        for frame in 0..10 {
            packets.push_front(
                InputPacket::new(Frame(frame), 0, PlayerInput::neutral(), frame as u64)
                    .with_timing_probe(frame, 0),
            );
        }

        let bundle = super::friend_connect_recent_input_retransmit_datagram(&packets)
            .expect("recent input window should encode as one datagram");

        assert_eq!(bundle.packets().len(), 8);
        assert_eq!(
            bundle
                .packets()
                .iter()
                .map(|packet| packet.sequence)
                .collect::<Vec<_>>(),
            vec![9, 8, 7, 6, 5, 4, 3, 2]
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_drains_slippi_style_bundle_and_resimulates_late_inputs() {
        let receiver_addr = reserve_local_udp_addr();
        let sender_addr = reserve_local_udp_addr();
        let receiver =
            UdpTransport::bind(receiver_addr, sender_addr).expect("receiver bind should work");
        let sender =
            UdpTransport::bind(sender_addr, receiver_addr).expect("sender bind should work");
        let mut inbox = InputPacketInbox::default();
        let mut stats = mole_runtime::UdpRuntimeStats::default();
        let mut session = RollbackSession::new(mole_runtime::default_play_world(), 32);
        let start = std::time::Instant::now();
        let mut time_sync = super::FriendConnectTimeSync::new(start);
        let local = PlayerInput::neutral();
        for frame in 0..5 {
            session.advance_with_prediction(Frame(frame), [Some(local), None]);
        }
        let predicted_checksum = session.world().checksum();
        let actual_remote_two = PlayerInput::neutral().with_attack(true);
        let actual_remote_three = PlayerInput::neutral().with_special(true);
        let datagram = InputPacketDatagram::from_packets(vec![
            InputPacket::new(Frame(3), 1, actual_remote_three, 0x303).with_timing_probe(3, 4),
            InputPacket::new(Frame(2), 1, actual_remote_two, 0x202).with_timing_probe(2, 4),
        ])
        .unwrap();

        sender
            .send_packet_datagram(&datagram)
            .expect("datagram send should work");
        for _ in 0..20 {
            super::drain_friend_udp_packets_from(
                &receiver,
                &mut inbox,
                &mut stats,
                &mut session,
                &mut time_sync,
                1,
                Frame(5),
            )
            .expect("drain should not fail");
            if stats.received_packets == 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert_eq!(stats.received_packets, 2);
        assert_eq!(stats.rollback_corrections, 2);
        assert_eq!(time_sync.offset_samples.len(), 1);
        assert_eq!(inbox.input(Frame(2), 1), Some(actual_remote_two));
        assert_eq!(inbox.input(Frame(3), 1), Some(actual_remote_three));
        assert_ne!(session.world().checksum(), predicted_checksum);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_duplicate_remote_frame_does_not_overwrite_rollback_history() {
        let receiver_addr = reserve_local_udp_addr();
        let sender_addr = reserve_local_udp_addr();
        let receiver =
            UdpTransport::bind(receiver_addr, sender_addr).expect("receiver bind should work");
        let sender =
            UdpTransport::bind(sender_addr, receiver_addr).expect("sender bind should work");
        let mut inbox = InputPacketInbox::default();
        let mut stats = mole_runtime::UdpRuntimeStats::default();
        let mut session = RollbackSession::new(mole_runtime::default_play_world(), 32);
        let mut time_sync = super::FriendConnectTimeSync::new(std::time::Instant::now());
        for frame in 0..5 {
            session.advance_with_prediction(Frame(frame), [Some(PlayerInput::neutral()), None]);
        }
        let accepted_input = PlayerInput::neutral().with_attack(true);
        let duplicate_conflict = PlayerInput::neutral().with_special(true);

        sender
            .send_packet_datagram(
                &InputPacketDatagram::from_packets(vec![InputPacket::new(
                    Frame(2),
                    1,
                    accepted_input,
                    0x202,
                )
                .with_timing_probe(2, 4)])
                .unwrap(),
            )
            .expect("first send should work");
        drain_until_received(
            &receiver,
            &mut inbox,
            &mut stats,
            &mut session,
            &mut time_sync,
            1,
            Frame(5),
            1,
            0,
        );
        let checksum_after_first = session.world().checksum();

        sender
            .send_packet_datagram(
                &InputPacketDatagram::from_packets(vec![InputPacket::new(
                    Frame(2),
                    1,
                    duplicate_conflict,
                    0xBAD,
                )
                .with_timing_probe(2, 4)])
                .unwrap(),
            )
            .expect("duplicate send should work");
        drain_until_received(
            &receiver,
            &mut inbox,
            &mut stats,
            &mut session,
            &mut time_sync,
            1,
            Frame(5),
            1,
            1,
        );

        assert_eq!(stats.received_packets, 1);
        assert_eq!(stats.duplicate_packets, 1);
        assert_eq!(stats.rollback_corrections, 1);
        assert_eq!(inbox.input(Frame(2), 1), Some(accepted_input));
        assert_eq!(session.world().checksum(), checksum_after_first);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_overlapping_old_bundle_does_not_backfill_before_remote_head() {
        let receiver_addr = reserve_local_udp_addr();
        let sender_addr = reserve_local_udp_addr();
        let receiver =
            UdpTransport::bind(receiver_addr, sender_addr).expect("receiver bind should work");
        let sender =
            UdpTransport::bind(sender_addr, receiver_addr).expect("sender bind should work");
        let mut inbox = InputPacketInbox::default();
        let mut stats = mole_runtime::UdpRuntimeStats::default();
        let mut session = RollbackSession::new(mole_runtime::default_play_world(), 32);
        let mut time_sync = super::FriendConnectTimeSync::new(std::time::Instant::now());
        for frame in 0..7 {
            session.advance_with_prediction(Frame(frame), [Some(PlayerInput::neutral()), None]);
        }
        sender
            .send_packet_datagram(
                &InputPacketDatagram::from_packets(vec![InputPacket::new(
                    Frame(5),
                    1,
                    PlayerInput::neutral(),
                    0x505,
                )
                .with_timing_probe(5, 6)])
                .unwrap(),
            )
            .expect("head packet send should work");
        drain_until_received(
            &receiver,
            &mut inbox,
            &mut stats,
            &mut session,
            &mut time_sync,
            1,
            Frame(7),
            1,
            0,
        );
        let checksum_after_head = session.world().checksum();

        sender
            .send_packet_datagram(
                &InputPacketDatagram::from_packets(vec![
                    InputPacket::new(Frame(5), 1, PlayerInput::neutral(), 0x505)
                        .with_timing_probe(5, 6),
                    InputPacket::new(Frame(4), 1, PlayerInput::neutral().with_attack(true), 0x404)
                        .with_timing_probe(4, 6),
                    InputPacket::new(
                        Frame(3),
                        1,
                        PlayerInput::neutral().with_special(true),
                        0x303,
                    )
                    .with_timing_probe(3, 6),
                ])
                .unwrap(),
            )
            .expect("overlap packet send should work");
        drain_until_received(
            &receiver,
            &mut inbox,
            &mut stats,
            &mut session,
            &mut time_sync,
            1,
            Frame(7),
            1,
            1,
        );

        assert_eq!(stats.received_packets, 1);
        assert_eq!(stats.duplicate_packets, 1);
        assert_eq!(stats.rollback_corrections, 0);
        assert_eq!(inbox.input(Frame(5), 1), Some(PlayerInput::neutral()));
        assert_eq!(inbox.input(Frame(4), 1), None);
        assert_eq!(inbox.input(Frame(3), 1), None);
        assert_eq!(session.world().checksum(), checksum_after_head);
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_stun_selection_prefers_ipv4_dns_result() {
        let selected = super::select_friend_connect_stun_addr([
            "[::1]:19302".parse().unwrap(),
            "127.0.0.1:19302".parse().unwrap(),
        ])
        .expect("resolved STUN addresses should select a socket address");

        assert_eq!(selected, "127.0.0.1:19302".parse().unwrap());
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    fn reserve_local_udp_addr() -> std::net::SocketAddr {
        std::net::UdpSocket::bind("127.0.0.1:0")
            .expect("reserve UDP socket")
            .local_addr()
            .expect("local addr")
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    fn drain_until_received(
        transport: &UdpTransport,
        inbox: &mut InputPacketInbox,
        stats: &mut mole_runtime::UdpRuntimeStats,
        session: &mut RollbackSession,
        time_sync: &mut super::FriendConnectTimeSync,
        remote_player: u8,
        local_frame: Frame,
        expected_received_packets: u32,
        expected_duplicate_packets: u32,
    ) {
        for _ in 0..20 {
            super::drain_friend_udp_packets_from(
                transport,
                inbox,
                stats,
                session,
                time_sync,
                remote_player,
                local_frame,
            )
            .expect("drain should not fail");
            if stats.received_packets >= expected_received_packets
                && stats.duplicate_packets >= expected_duplicate_packets
            {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_stun_selection_reports_empty_dns_results() {
        assert_eq!(
            super::select_friend_connect_stun_addr([]),
            Err("STUN DNS returned no socket addresses".to_string())
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_start_button_has_stable_panel_bounds() {
        assert!(super::start_button_contains(382.0, 188.0));
        assert!(super::start_button_contains(488.0, 216.0));
        assert!(!super::start_button_contains(381.0, 188.0));
        assert!(!super::start_button_contains(488.0, 217.0));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_lobby_status_ties_start_to_shared_code_owner() {
        assert_eq!(
            super::friend_lobby_status_for(true, None),
            "LOBBY PEER JOINED CLICK START"
        );
        assert_eq!(
            super::friend_lobby_status_for(false, None),
            "LOBBY PEER JOINED WAIT START"
        );
        assert!(
            super::friend_lobby_status_for(false, Some("network retry")).contains("START SIGNAL")
        );
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_player_slots_are_owned_by_lobby_role_not_code_sort() {
        assert_eq!(super::friend_connect_player_indices(true), (0, 1));
        assert_eq!(super::friend_connect_player_indices(false), (1, 0));
    }

    #[cfg(all(feature = "sdl", feature = "wup"))]
    #[test]
    fn friend_connect_match_start_rebroadcast_covers_listener_race_window() {
        let window = super::FRIEND_CONNECT_MATCH_START_INTERVAL
            .saturating_mul(super::FRIEND_CONNECT_MATCH_START_BROADCASTS as u32);

        assert!(super::FRIEND_CONNECT_MATCH_START_BROADCASTS >= 30);
        assert!(window >= std::time::Duration::from_secs(6));
    }

    #[test]
    fn parse_ucf_enabled_defaults_to_on() {
        assert!(parse_ucf_enabled(&[]));
    }

    #[test]
    fn parse_ucf_enabled_supports_vanilla_input_mode() {
        assert!(!parse_ucf_enabled(&["--no-ucf".to_string()]));
    }

    #[test]
    fn parse_slippi_compare_mode_defaults_to_seeded_diagnostic() {
        assert_eq!(
            parse_slippi_compare_mode(&[]),
            SlippiCoreComparisonMode::SeededPreFrameDiagnostic
        );
    }

    #[test]
    fn parse_slippi_compare_mode_supports_match_start_oracle() {
        assert_eq!(
            parse_slippi_compare_mode(&["--slippi-match-start".to_string()]),
            SlippiCoreComparisonMode::SequentialMatchStart
        );
    }

    #[test]
    fn parse_slippi_trace_player_uses_one_based_cli_values() {
        assert_eq!(
            parse_slippi_trace_player(&["--slippi-trace-player".to_string(), "2".to_string()]),
            1
        );
    }

    #[test]
    fn parse_i32_after_supports_negative_slippi_frames() {
        assert_eq!(
            parse_i32_after(
                &["--slippi-trace-start".to_string(), "-20".to_string()],
                "--slippi-trace-start"
            ),
            Some(-20)
        );
    }
}
