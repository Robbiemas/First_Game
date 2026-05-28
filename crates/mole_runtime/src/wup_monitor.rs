use mole_core::TICK_NANOS;
use mole_runtime::{
    configure_sdl_controller_hints, InputReadout, PlayerReadout, WupInputMapper, WupInputSource,
    WupPort,
};
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use sdl3::rect::{Point, Rect};
use sdl3::render::WindowCanvas;
use std::time::Duration;

const WINDOW_WIDTH: u32 = 960;
const WINDOW_HEIGHT: u32 = 540;
const PANEL_WIDTH: u32 = 390;
const PANEL_HEIGHT: u32 = 370;
const STICK_BOX: u32 = 148;
const C_STICK_BOX: u32 = 88;

pub fn run_wup_monitor(frames: u32) -> Result<(), String> {
    configure_sdl_controller_hints();
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let video = sdl.video().map_err(|error| error.to_string())?;
    let window = video
        .window(
            "Mole WUP-028 Native Input Monitor",
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
        )
        .position_centered()
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window.into_canvas();
    let mut event_pump = sdl.event_pump().map_err(|error| error.to_string())?;
    let mut input_source = WupInputSource::open()?;
    let mut mapper = WupInputMapper::default();
    let mut ports = [WupPort::default(); 4];

    println!("WUP native monitor is running. Close the window or press Escape to exit.");

    for _frame in 0..frames {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => return Ok(()),
                _ => {}
            }
        }

        match input_source.poll_ports() {
            Ok(next_ports) => ports = next_ports,
            Err(rusb::Error::Timeout) => {}
            Err(error) => return Err(format!("WUP read failed: {error}")),
        }

        let melee = mapper.map_ports_to_melee_snapshots(ports);
        draw_monitor(
            &mut canvas,
            &InputReadout::from_wup_ports_with_melee_snapshots(ports, melee),
        )?;
        std::thread::sleep(Duration::from_nanos(TICK_NANOS));
    }

    Ok(())
}

fn draw_monitor(canvas: &mut WindowCanvas, readout: &InputReadout) -> Result<(), String> {
    canvas.set_draw_color(Color::RGB(18, 20, 22));
    canvas.clear();

    draw_label(
        canvas,
        "WUP NATIVE INPUT",
        32,
        24,
        4,
        Color::RGB(214, 218, 224),
    )?;
    draw_adapter_ports(canvas, readout)?;
    draw_player_readout(canvas, &readout.players[0], 1, 60, 106)?;
    draw_player_readout(canvas, &readout.players[1], 2, 510, 106)?;

    canvas.present();
    Ok(())
}

fn draw_adapter_ports(canvas: &mut WindowCanvas, readout: &InputReadout) -> Result<(), String> {
    let mut x = 708;
    for (index, connected) in readout.adapter_ports.iter().copied().enumerate() {
        let color = if connected {
            Color::RGB(56, 214, 146)
        } else {
            Color::RGB(73, 78, 86)
        };
        draw_rect(canvas, x, 24, 36, 28, color, true)?;
        draw_rect(canvas, x, 24, 36, 28, Color::RGB(162, 170, 180), false)?;
        draw_label(
            canvas,
            &(index + 1).to_string(),
            x + 13,
            30,
            3,
            Color::RGB(13, 15, 18),
        )?;
        x += 50;
    }
    Ok(())
}

fn draw_player_readout(
    canvas: &mut WindowCanvas,
    readout: &PlayerReadout,
    player_number: usize,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let outline = if readout.connected {
        Color::RGB(56, 214, 146)
    } else {
        Color::RGB(81, 86, 94)
    };
    let text = if readout.connected {
        Color::RGB(226, 231, 238)
    } else {
        Color::RGB(112, 119, 128)
    };

    draw_rect(
        canvas,
        x,
        y,
        PANEL_WIDTH,
        PANEL_HEIGHT,
        Color::RGB(29, 32, 36),
        true,
    )?;
    draw_rect(canvas, x, y, PANEL_WIDTH, PANEL_HEIGHT, outline, false)?;
    draw_label(
        canvas,
        &format!("P{player_number}"),
        x + 22,
        y + 20,
        5,
        text,
    )?;
    if let Some(port) = readout.source_port {
        draw_label(
            canvas,
            &format!("PORT{}", port + 1),
            x + 236,
            y + 24,
            3,
            text,
        )?;
    } else {
        draw_label(canvas, "NO PAD", x + 222, y + 24, 3, text)?;
    }

    draw_stick(canvas, readout, x + 38, y + 86)?;
    draw_c_stick(canvas, readout, x + 206, y + 244)?;
    draw_dpad(canvas, readout, x + 58, y + 270)?;
    draw_buttons(canvas, readout, x, y)?;
    Ok(())
}

fn draw_stick(
    canvas: &mut WindowCanvas,
    readout: &PlayerReadout,
    x: i32,
    y: i32,
) -> Result<(), String> {
    draw_axis_box(
        canvas,
        Rect::new(x, y, STICK_BOX, STICK_BOX),
        readout.stick_x,
        readout.stick_y,
        readout.connected,
        Color::RGB(246, 197, 83),
    )
}

fn draw_c_stick(
    canvas: &mut WindowCanvas,
    readout: &PlayerReadout,
    x: i32,
    y: i32,
) -> Result<(), String> {
    draw_label(
        canvas,
        "C",
        x + C_STICK_BOX as i32 / 2 - 6,
        y - 18,
        4,
        if readout.connected {
            Color::RGB(226, 231, 238)
        } else {
            Color::RGB(112, 119, 128)
        },
    )?;
    draw_axis_box(
        canvas,
        Rect::new(x, y, C_STICK_BOX, C_STICK_BOX),
        readout.c_stick_x,
        readout.c_stick_y,
        readout.connected,
        Color::RGB(151, 127, 235),
    )
}

fn draw_axis_box(
    canvas: &mut WindowCanvas,
    rect: Rect,
    axis_x: i16,
    axis_y: i16,
    connected: bool,
    dot_color: Color,
) -> Result<(), String> {
    let color = if connected {
        Color::RGB(186, 196, 207)
    } else {
        Color::RGB(82, 89, 99)
    };
    let center_x = rect.x() + rect.width() as i32 / 2;
    let center_y = rect.y() + rect.height() as i32 / 2;
    let radius = rect.width().min(rect.height()) as i32 / 2 - 11;

    draw_rect(
        canvas,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        Color::RGB(20, 22, 25),
        true,
    )?;
    draw_rect(
        canvas,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        color,
        false,
    )?;
    draw_line(
        canvas,
        center_x,
        rect.y() + 8,
        center_x,
        rect.y() + rect.height() as i32 - 8,
        color,
    )?;
    draw_line(
        canvas,
        rect.x() + 8,
        center_y,
        rect.x() + rect.width() as i32 - 8,
        center_y,
        color,
    )?;

    let dot_x = center_x + scale_axis(axis_x, radius);
    let dot_y = center_y - scale_axis(axis_y, radius);
    let dot = if connected {
        dot_color
    } else {
        Color::RGB(84, 88, 94)
    };
    draw_rect(canvas, dot_x - 7, dot_y - 7, 14, 14, dot, true)
}

fn draw_dpad(
    canvas: &mut WindowCanvas,
    readout: &PlayerReadout,
    x: i32,
    y: i32,
) -> Result<(), String> {
    draw_button(
        canvas,
        Rect::new(x + 24, y, 24, 24),
        "",
        readout.buttons.dpad_up,
        Color::RGB(114, 183, 245),
    )?;
    draw_button(
        canvas,
        Rect::new(x + 24, y + 48, 24, 24),
        "",
        readout.buttons.dpad_down,
        Color::RGB(114, 183, 245),
    )?;
    draw_button(
        canvas,
        Rect::new(x, y + 24, 24, 24),
        "",
        readout.buttons.dpad_left,
        Color::RGB(114, 183, 245),
    )?;
    draw_button(
        canvas,
        Rect::new(x + 48, y + 24, 24, 24),
        "",
        readout.buttons.dpad_right,
        Color::RGB(114, 183, 245),
    )
}

fn draw_buttons(
    canvas: &mut WindowCanvas,
    readout: &PlayerReadout,
    x: i32,
    y: i32,
) -> Result<(), String> {
    draw_button(
        canvas,
        Rect::new(x + 297, y + 134, 46, 46),
        "A",
        readout.buttons.attack,
        Color::RGB(67, 216, 148),
    )?;
    draw_button(
        canvas,
        Rect::new(x + 252, y + 174, 34, 34),
        "B",
        readout.buttons.special,
        Color::RGB(239, 99, 99),
    )?;
    draw_button(
        canvas,
        Rect::new(x + 342, y + 88, 36, 36),
        "X",
        readout.buttons.jump_primary,
        Color::RGB(114, 183, 245),
    )?;
    draw_button(
        canvas,
        Rect::new(x + 360, y + 176, 36, 36),
        "Y",
        readout.buttons.jump_secondary,
        Color::RGB(114, 183, 245),
    )?;
    draw_button(
        canvas,
        Rect::new(x + 296, y + 58, 64, 24),
        "Z",
        readout.buttons.grab,
        Color::RGB(151, 127, 235),
    )?;
    draw_trigger(
        canvas,
        Rect::new(x + 38, y + 58, 66, 24),
        "L",
        readout.left_trigger,
        readout.buttons.left_trigger,
        Color::RGB(246, 197, 83),
    )?;
    draw_trigger(
        canvas,
        Rect::new(x + 122, y + 58, 66, 24),
        "R",
        readout.right_trigger,
        readout.buttons.right_trigger,
        Color::RGB(246, 197, 83),
    )?;
    draw_button(
        canvas,
        Rect::new(x + 305, y + 236, 48, 24),
        "S",
        readout.buttons.start,
        Color::RGB(226, 231, 238),
    )
}

fn draw_trigger(
    canvas: &mut WindowCanvas,
    rect: Rect,
    label: &str,
    analog: u8,
    digital: bool,
    color: Color,
) -> Result<(), String> {
    draw_rect(
        canvas,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        Color::RGB(42, 46, 52),
        true,
    )?;
    let fill_width = ((rect.width() as u16 * analog as u16) / 255) as u32;
    if fill_width > 0 {
        draw_rect(
            canvas,
            rect.x(),
            rect.y(),
            fill_width,
            rect.height(),
            color,
            true,
        )?;
    }
    let border = if digital {
        Color::RGB(255, 238, 153)
    } else {
        Color::RGB(155, 165, 176)
    };
    draw_rect(
        canvas,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        border,
        false,
    )?;
    draw_label(
        canvas,
        label,
        rect.x() + rect.width() as i32 / 2 - 6,
        rect.y() + 4,
        4,
        if analog > 128 || digital {
            Color::RGB(11, 14, 17)
        } else {
            Color::RGB(176, 184, 194)
        },
    )
}

fn draw_button(
    canvas: &mut WindowCanvas,
    rect: Rect,
    label: &str,
    pressed: bool,
    pressed_color: Color,
) -> Result<(), String> {
    let fill = if pressed {
        pressed_color
    } else {
        Color::RGB(42, 46, 52)
    };
    let label_color = if pressed {
        Color::RGB(11, 14, 17)
    } else {
        Color::RGB(176, 184, 194)
    };

    draw_rect(
        canvas,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        fill,
        true,
    )?;
    draw_rect(
        canvas,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        Color::RGB(155, 165, 176),
        false,
    )?;
    let label_x = rect.x() + rect.width() as i32 / 2 - (label.len() as i32 * 12) / 2;
    let label_y = rect.y() + rect.height() as i32 / 2 - 8;
    draw_label(canvas, label, label_x, label_y, 4, label_color)
}

fn draw_label(
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

        for (row, pattern) in glyph(character).iter().enumerate() {
            for (column, pixel) in pattern.chars().enumerate() {
                if pixel == '1' {
                    canvas
                        .fill_rect(Rect::new(
                            cursor_x + column as i32 * scale,
                            y + row as i32 * scale,
                            scale as u32,
                            scale as u32,
                        ))
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        cursor_x += scale * 4;
    }
    Ok(())
}

fn glyph(character: char) -> [&'static str; 5] {
    match character {
        '0' => ["111", "101", "101", "101", "111"],
        '1' => ["010", "110", "010", "010", "111"],
        '2' => ["111", "001", "111", "100", "111"],
        '3' => ["111", "001", "111", "001", "111"],
        '4' => ["101", "101", "111", "001", "001"],
        'A' => ["010", "101", "111", "101", "101"],
        'B' => ["110", "101", "110", "101", "110"],
        'C' => ["111", "100", "100", "100", "111"],
        'D' => ["110", "101", "101", "101", "110"],
        'E' => ["111", "100", "110", "100", "111"],
        'I' => ["111", "010", "010", "010", "111"],
        'L' => ["100", "100", "100", "100", "111"],
        'N' => ["101", "111", "111", "111", "101"],
        'O' => ["111", "101", "101", "101", "111"],
        'P' => ["110", "101", "110", "100", "100"],
        'R' => ["110", "101", "110", "101", "101"],
        'S' => ["111", "100", "111", "001", "111"],
        'T' => ["111", "010", "010", "010", "010"],
        'U' => ["101", "101", "101", "101", "111"],
        'V' => ["101", "101", "101", "101", "010"],
        'W' => ["101", "101", "111", "111", "101"],
        'X' => ["101", "101", "010", "101", "101"],
        'Y' => ["101", "101", "010", "010", "010"],
        'Z' => ["111", "001", "010", "100", "111"],
        _ => ["111", "001", "011", "000", "010"],
    }
}

fn draw_rect(
    canvas: &mut WindowCanvas,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    color: Color,
    filled: bool,
) -> Result<(), String> {
    canvas.set_draw_color(color);
    let rect = Rect::new(x, y, width, height);
    if filled {
        canvas.fill_rect(rect)
    } else {
        canvas.draw_rect(rect)
    }
    .map_err(|error| error.to_string())
}

fn draw_line(
    canvas: &mut WindowCanvas,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: Color,
) -> Result<(), String> {
    canvas.set_draw_color(color);
    canvas
        .draw_line(Point::new(x1, y1), Point::new(x2, y2))
        .map_err(|error| error.to_string())
}

fn scale_axis(value: i16, radius: i32) -> i32 {
    let divisor = if value < 0 { 32_768.0 } else { 32_767.0 };
    ((value as f32 / divisor) * radius as f32).round() as i32
}
