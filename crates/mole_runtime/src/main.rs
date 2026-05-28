use mole_core::{step_world, Frame, PlayerInput, World};

#[cfg(any(feature = "sdl", feature = "wup"))]
use mole_core::TICK_NANOS;

#[cfg(feature = "sdl")]
use mole_runtime::{
    configure_sdl_controller_hints, RenderColor, RenderRect, RenderScene, SdlInputSource,
};

#[cfg(feature = "sdl")]
use sdl3::{
    pixels::Color,
    render::{FRect, WindowCanvas},
};

#[cfg(feature = "wup")]
use mole_runtime::WupInputSource;

#[cfg(all(feature = "sdl", feature = "wup"))]
mod wup_monitor;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let frames = parse_frames(args.iter().cloned());

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
        if let Err(error) = run_wup_smoke(frames) {
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
        if let Err(error) = run_sdl_smoke(frames) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "sdl"))]
    if has_flag(&args, "--sdl") {
        eprintln!("The SDL3 runtime needs: cargo run -p mole_runtime --features sdl -- --sdl");
        std::process::exit(2);
    }

    run_headless(frames);
}

fn run_headless(frames: u32) {
    let mut world = World::for_two_players();
    let inputs = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..frames {
        step_world(&mut world, Frame(frame), &inputs);
    }

    println!(
        "final_frame={} checksum={}",
        world.frame().0,
        world.checksum()
    );
}

#[cfg(feature = "sdl")]
fn run_sdl_smoke(frames: u32) -> Result<(), String> {
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let video = sdl.video().map_err(|error| error.to_string())?;
    let window = video
        .window("Mole Rust SDL3 Runtime", 960, 540)
        .position_centered()
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window.into_canvas();

    let mut input_source = SdlInputSource::new(&sdl)?;
    let mut world = World::for_two_players();

    for frame in 0..frames {
        let inputs = mole_runtime::InputSource::poll_inputs(&mut input_source, Frame(frame));
        step_world(&mut world, Frame(frame), &inputs);
        let render_frame = mole_runtime::RenderFrame::from_world(&world);
        let (width, height) = canvas.output_size().map_err(|error| error.to_string())?;
        draw_sdl_scene(
            &mut canvas,
            &RenderScene::from_frame(&render_frame, width, height),
        )?;

        if input_source.quit_requested() {
            break;
        }

        std::thread::sleep(std::time::Duration::from_nanos(TICK_NANOS));
    }

    println!(
        "final_frame={} checksum={} gamepads={}",
        world.frame().0,
        world.checksum(),
        input_source.gamepad_count()
    );
    Ok(())
}

#[cfg(feature = "sdl")]
fn draw_sdl_scene(canvas: &mut WindowCanvas, scene: &RenderScene) -> Result<(), String> {
    canvas.set_draw_color(sdl_color(scene.background));
    canvas.clear();
    draw_sdl_rect(canvas, scene.stage)?;
    for player in scene.players {
        draw_sdl_rect(canvas, player)?;
    }
    if !canvas.present() {
        return Err("SDL present failed".to_string());
    }
    Ok(())
}

#[cfg(feature = "sdl")]
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

#[cfg(feature = "sdl")]
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
fn run_wup_smoke(frames: u32) -> Result<(), String> {
    let mut input_source = WupInputSource::open()?;
    let mut world = World::for_two_players();

    for frame in 0..frames {
        let inputs = mole_runtime::InputSource::poll_inputs(&mut input_source, Frame(frame));
        step_world(&mut world, Frame(frame), &inputs);
        std::thread::sleep(std::time::Duration::from_nanos(TICK_NANOS));
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

fn parse_frames(mut args: impl Iterator<Item = String>) -> u32 {
    while let Some(arg) = args.next() {
        if arg == "--frames" {
            return args
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(120);
        }
    }
    120
}
