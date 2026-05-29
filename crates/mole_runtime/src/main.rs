#[cfg(all(feature = "sdl", feature = "wup"))]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mole_core::{step_world, Frame, PlayerInput, World};
use mole_transport::{InputPacket, InputPacketInbox, UdpTransport};

#[cfg(feature = "wup")]
use mole_core::TICK_NANOS;

#[cfg(feature = "sdl")]
use mole_runtime::configure_sdl_controller_hints;

#[cfg(all(feature = "sdl", feature = "wup"))]
use mole_runtime::{
    DebugOverlay, RenderColor, RenderPolygon, RenderRect, RenderScene, SdlInputSource,
};

#[cfg(all(feature = "sdl", feature = "wup"))]
use sdl3::{
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

    if has_flag(&args, "--udp") {
        #[cfg(feature = "sdl")]
        if has_flag(&args, "--sdl") {
            match mole_runtime::UdpRuntimeConfig::from_args(&args)
                .and_then(|config| run_udp_sdl(frames, config, replay_path.as_deref(), frame_log))
            {
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
        if let Err(error) = run_wup_smoke(frames, replay_path.as_deref()) {
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
        if let Err(error) = run_sdl_smoke(frames, replay_path.as_deref(), frame_log) {
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

fn run_headless(frames: u32, replay_path: Option<&Path>) -> Result<(), String> {
    let initial = World::for_two_players();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));
    let inputs = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..frames {
        step_world(&mut world, Frame(frame), &inputs);
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
    let transport = UdpTransport::bind(config.local_addr, config.peer_addr)
        .map_err(|error| error.to_string())?;
    let initial = World::for_two_players();
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

        step_world(&mut world, frame, &inputs);

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
fn run_udp_sdl(
    frames: u32,
    config: mole_runtime::UdpRuntimeConfig,
    replay_path: Option<&Path>,
    frame_log: bool,
) -> Result<(), String> {
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let video = sdl.video().map_err(|error| error.to_string())?;
    let window = video
        .window("Mole Rust UDP SDL Runtime", 960, 540)
        .position_centered()
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let mut texture_cache =
        SdlTextureCache::new(&texture_creator, mole_runtime::project_asset_root());
    let mut sdl_shell_input = SdlInputSource::new(&sdl)?;
    let mut local_input_source = WupInputSource::open()?;
    let transport = UdpTransport::bind(config.local_addr, config.peer_addr)
        .map_err(|error| error.to_string())?;
    let initial = World::for_two_players();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));
    let mut inbox = InputPacketInbox::default();
    let mut stats = mole_runtime::UdpRuntimeStats::default();
    let remote_player = 1 - config.player_index;

    for frame_number in 0..frames {
        let frame = Frame(frame_number);
        let _ = mole_runtime::InputSource::poll_inputs(&mut sdl_shell_input, frame);
        let polled_inputs = mole_runtime::InputSource::poll_inputs(&mut local_input_source, frame);
        drain_udp_packets(&transport, &mut inbox, &mut stats, frame)?;

        let mut inputs = [PlayerInput::neutral(), PlayerInput::neutral()];
        inputs[config.player_index as usize] = polled_inputs[config.player_index as usize];
        if let Some(remote_input) = inbox.input(frame, remote_player) {
            inputs[remote_player as usize] = remote_input;
        } else {
            stats.record_missing_remote_frame();
        }

        step_world(&mut world, frame, &inputs);

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

        std::thread::sleep(std::time::Duration::from_nanos(TICK_NANOS));
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
) -> Result<(), String> {
    Err(
        "UDP SDL runtime gameplay input requires native WUP: cargo run -p mole_runtime --features \"sdl wup\" -- --udp --sdl --local-addr <addr> --peer-addr <addr>"
            .to_string(),
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn run_sdl_smoke(frames: u32, replay_path: Option<&Path>, frame_log: bool) -> Result<(), String> {
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let video = sdl.video().map_err(|error| error.to_string())?;
    let window = video
        .window("Mole Rust SDL3 Runtime", 960, 540)
        .position_centered()
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let mut texture_cache =
        SdlTextureCache::new(&texture_creator, mole_runtime::project_asset_root());

    let mut sdl_shell_input = SdlInputSource::new(&sdl)?;
    let mut gameplay_input_source = WupInputSource::open()?;
    let initial = World::for_two_players();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));

    for frame in 0..frames {
        let frame = Frame(frame);
        let _ = mole_runtime::InputSource::poll_inputs(&mut sdl_shell_input, frame);
        let inputs = mole_runtime::step_world_from_input_source(
            &mut world,
            &mut gameplay_input_source,
            frame,
        );
        if let Some(capture) = replay_capture.as_mut() {
            capture.record_frame(frame, inputs, world.checksum());
        }
        let render_frame = mole_runtime::RenderFrame::from_world(&world);
        let overlay = DebugOverlay::from_frame(&render_frame);
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

        std::thread::sleep(std::time::Duration::from_nanos(TICK_NANOS));
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
    Ok(())
}

#[cfg(all(feature = "sdl", not(feature = "wup")))]
fn run_sdl_smoke(
    _frames: u32,
    _replay_path: Option<&Path>,
    _frame_log: bool,
) -> Result<(), String> {
    Err(
        "SDL3 runtime gameplay input requires native WUP: cargo run -p mole_runtime --features \"sdl wup\" -- --sdl"
            .to_string(),
    )
}

#[cfg(all(feature = "sdl", feature = "wup"))]
fn draw_sdl_scene(
    canvas: &mut WindowCanvas,
    scene: &RenderScene,
    overlay: Option<&DebugOverlay>,
    mut texture_cache: Option<&mut SdlTextureCache<'_>>,
) -> Result<(), String> {
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
    let mut drew_player_textures = false;
    if let Some(cache) = texture_cache.as_mut() {
        for (index, player) in scene.players.iter().copied().enumerate() {
            draw_sdl_image(
                canvas,
                cache,
                &scene.player_sprites[index].relative_path(),
                player,
                scene.player_sprites[index].flip_x,
            )?;
        }
        drew_player_textures = true;
    }
    if !drew_player_textures {
        for player in scene.players {
            draw_sdl_rect(canvas, player)?;
        }
    }
    for shield in scene.player_shields.into_iter().flatten() {
        draw_sdl_circle(canvas, shield)?;
    }
    for ecb in scene.player_ecbs {
        draw_sdl_polygon(canvas, ecb)?;
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
    let color = Color::RGBA(235, 240, 248, 255);
    for (index, line) in overlay.lines.iter().enumerate() {
        draw_sdl_label(canvas, line, 12, 12 + index as i32 * 18, 3, color)?;
    }
    Ok(())
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
        std::thread::sleep(std::time::Duration::from_nanos(TICK_NANOS));
    }

    Ok(())
}

#[cfg(feature = "wup")]
fn run_wup_smoke(frames: u32, replay_path: Option<&Path>) -> Result<(), String> {
    let mut input_source = WupInputSource::open()?;
    let initial = World::for_two_players();
    let mut world = initial.clone();
    let mut replay_capture = replay_path.map(|_| mole_runtime::ReplayCapture::new(initial));

    for frame in 0..frames {
        let frame = Frame(frame);
        let inputs =
            mole_runtime::step_world_from_input_source(&mut world, &mut input_source, frame);
        if let Some(capture) = replay_capture.as_mut() {
            capture.record_frame(frame, inputs, world.checksum());
        }
        std::thread::sleep(std::time::Duration::from_nanos(TICK_NANOS));
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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn parse_frames(args: &[String]) -> u32 {
    if let Some(value) = value_after(args, "--frames") {
        return value.parse::<u32>().unwrap_or(120);
    }
    if has_flag(args, "--play") {
        return u32::MAX;
    }

    120
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
    use super::parse_frames;

    #[test]
    fn parse_frames_defaults_to_smoke_length() {
        assert_eq!(parse_frames(&[]), 120);
    }

    #[test]
    fn parse_frames_supports_play_until_quit_mode() {
        assert_eq!(parse_frames(&["--play".to_string()]), u32::MAX);
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
}
