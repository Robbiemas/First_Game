use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    map_gamecube_pad_to_player_input, project_asset_root, RenderFrame, WupInputTrace, WupPort,
};
use mole_core::Frame;
use mole_core::GameCubePadStatus;
use mole_core::{
    MeleeActionStateId, MeleeInputFacts, MeleeInputSnapshot, MeleeInputThresholds, MeleeJumpInput,
    MotionState, PlayerInput, SourceActionKey, WalkSpeedBucket,
};

const DEFAULT_INPUT_TRACE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_INPUT_TRACE_MAX_FILES: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ButtonReadout {
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub z: bool,
    pub left_trigger: bool,
    pub right_trigger: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub attack: bool,
    pub special: bool,
    pub jump_primary: bool,
    pub jump_secondary: bool,
    pub jump: bool,
    pub shield: bool,
    pub grab: bool,
    pub start: bool,
}

impl ButtonReadout {
    fn from_gamecube_pad(pad: GameCubePadStatus) -> Self {
        let input = map_gamecube_pad_to_player_input(pad);

        Self {
            a: pad.buttons.a(),
            b: pad.buttons.b(),
            x: pad.buttons.x(),
            y: pad.buttons.y(),
            z: pad.buttons.z(),
            left_trigger: pad.buttons.l(),
            right_trigger: pad.buttons.r(),
            dpad_up: pad.buttons.dpad_up(),
            dpad_down: pad.buttons.dpad_down(),
            dpad_left: pad.buttons.dpad_left(),
            dpad_right: pad.buttons.dpad_right(),
            attack: pad.buttons.a(),
            special: pad.buttons.b(),
            jump_primary: pad.buttons.x(),
            jump_secondary: pad.buttons.y(),
            jump: pad.buttons.x() || pad.buttons.y(),
            shield: input.shield() || pad.left_trigger != 0 || pad.right_trigger != 0,
            grab: pad.buttons.z(),
            start: pad.buttons.start(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerReadout {
    pub connected: bool,
    pub source_port: Option<usize>,
    pub input: PlayerInput,
    pub buttons: ButtonReadout,
    pub raw_stick_x: u8,
    pub raw_stick_y: u8,
    pub raw_c_stick_x: u8,
    pub raw_c_stick_y: u8,
    pub stick_x: i16,
    pub stick_y: i16,
    pub c_stick_x: i16,
    pub c_stick_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub melee: Option<MeleeReadout>,
}

impl Default for PlayerReadout {
    fn default() -> Self {
        Self {
            connected: false,
            source_port: None,
            input: PlayerInput::neutral(),
            buttons: ButtonReadout::default(),
            raw_stick_x: 128,
            raw_stick_y: 128,
            raw_c_stick_x: 128,
            raw_c_stick_y: 128,
            stick_x: 0,
            stick_y: 0,
            c_stick_x: 0,
            c_stick_y: 0,
            left_trigger: 0,
            right_trigger: 0,
            melee: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeReadout {
    pub lstick_x: i8,
    pub lstick_y: i8,
    pub prev_lstick_x: i8,
    pub prev_lstick_y: i8,
    pub cstick_x: i8,
    pub cstick_y: i8,
    pub prev_cstick_x: i8,
    pub prev_cstick_y: i8,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub x_tap_timer: u8,
    pub y_tap_timer: u8,
    pub trigger_timer: u8,
    pub facts: MeleeInputFacts,
}

impl MeleeReadout {
    fn from_snapshot(snapshot: MeleeInputSnapshot) -> Self {
        Self {
            lstick_x: snapshot.lstick.0,
            lstick_y: snapshot.lstick.1,
            prev_lstick_x: snapshot.prev_lstick.0,
            prev_lstick_y: snapshot.prev_lstick.1,
            cstick_x: snapshot.cstick.0,
            cstick_y: snapshot.cstick.1,
            prev_cstick_x: snapshot.prev_cstick.0,
            prev_cstick_y: snapshot.prev_cstick.1,
            left_trigger: snapshot.left_trigger,
            right_trigger: snapshot.right_trigger,
            x_tap_timer: snapshot.x_tap_timer,
            y_tap_timer: snapshot.y_tap_timer,
            trigger_timer: snapshot.trigger_timer,
            facts: snapshot.facts(MeleeInputThresholds::default()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputReadout {
    pub players: [PlayerReadout; 2],
    pub adapter_ports: [bool; 4],
}

impl InputReadout {
    pub fn from_wup_ports(ports: [WupPort; 4]) -> Self {
        let mut readout = Self::default();
        let mut player_index = 0;

        for (port_index, port) in ports.iter().copied().enumerate() {
            readout.adapter_ports[port_index] = port.connected;
            if !port.connected || player_index == readout.players.len() {
                continue;
            }

            let (stick_x, stick_y) = port.pad.main_stick_i16();
            let (c_stick_x, c_stick_y) = port.pad.c_stick_i16();
            readout.players[player_index] = PlayerReadout {
                connected: true,
                source_port: Some(port_index),
                input: map_gamecube_pad_to_player_input(port.pad),
                buttons: ButtonReadout::from_gamecube_pad(port.pad),
                raw_stick_x: port.pad.stick_x,
                raw_stick_y: port.pad.stick_y,
                raw_c_stick_x: port.pad.c_stick_x,
                raw_c_stick_y: port.pad.c_stick_y,
                stick_x,
                stick_y,
                c_stick_x,
                c_stick_y,
                left_trigger: port.pad.left_trigger,
                right_trigger: port.pad.right_trigger,
                melee: None,
            };
            player_index += 1;
        }

        readout
    }

    pub fn from_wup_ports_with_melee_snapshots(
        ports: [WupPort; 4],
        snapshots: [Option<MeleeInputSnapshot>; 2],
    ) -> Self {
        let mut readout = Self::from_wup_ports(ports);
        let mut player_index = 0;

        for port in ports {
            if !port.connected || player_index == readout.players.len() {
                continue;
            }

            readout.players[player_index].melee =
                snapshots[player_index].map(MeleeReadout::from_snapshot);
            player_index += 1;
        }

        readout
    }

    pub fn to_json_line(self) -> String {
        format!(
            "{{\"players\":[{},{}]}}",
            player_to_json(self.players[0]),
            player_to_json(self.players[1])
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerInputTraceLog {
    json_line: String,
}

impl ControllerInputTraceLog {
    pub fn from_wup_trace(
        frame: Frame,
        trace: &WupInputTrace,
        before: &RenderFrame,
        after: &RenderFrame,
    ) -> Self {
        Self {
            json_line: format!(
                "{{\"frame\":{},\"input_backend\":\"wup\",\"ucf_enabled\":{},\"adapter_ports\":[{},{},{},{}],\"wup\":{},\"before\":{},\"after\":{}}}",
                frame.0,
                trace.ucf_enabled,
                trace.adapter_ports[0],
                trace.adapter_ports[1],
                trace.adapter_ports[2],
                trace.adapter_ports[3],
                wup_trace_players_to_json(trace),
                core_frame_to_json(before),
                core_frame_to_json(after)
            ),
        }
    }

    pub fn to_json_line(&self) -> String {
        self.json_line.clone()
    }
}

#[derive(Debug)]
pub struct InputTraceWriter {
    path: PathBuf,
    writer: BufWriter<File>,
    bytes_written: u64,
    max_bytes: u64,
}

impl InputTraceWriter {
    pub fn create_default() -> io::Result<Self> {
        Self::create_in_dir(project_asset_root().join("logs"))
    }

    pub fn create_in_dir(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::create_in_dir_with_limits(
            path,
            DEFAULT_INPUT_TRACE_MAX_BYTES,
            DEFAULT_INPUT_TRACE_MAX_FILES,
        )
    }

    pub fn create_in_dir_with_limits(
        path: impl AsRef<Path>,
        max_bytes: u64,
        max_files: usize,
    ) -> io::Result<Self> {
        let dir = path.as_ref();
        fs::create_dir_all(dir)?;
        prune_controller_input_trace_logs(dir, max_files.max(1).saturating_sub(1))?;
        let timestamp = unix_time_millis();
        for suffix in 0..1000 {
            let filename = if suffix == 0 {
                format!("controller-input-trace-{timestamp}.jsonl")
            } else {
                format!("controller-input-trace-{timestamp}-{suffix}.jsonl")
            };
            let path = dir.join(filename);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        writer: BufWriter::new(file),
                        bytes_written: 0,
                        max_bytes,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate unique controller input trace filename",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_line(&mut self, log: &ControllerInputTraceLog) -> io::Result<()> {
        let line = log.to_json_line();
        let line_bytes = line.len() as u64 + 1;
        if self.bytes_written.saturating_add(line_bytes) > self.max_bytes {
            self.writer.flush()?;
            return Ok(());
        }

        writeln!(self.writer, "{line}")?;
        self.bytes_written += line_bytes;
        self.writer.flush()
    }
}

fn prune_controller_input_trace_logs(dir: &Path, keep_existing: usize) -> io::Result<()> {
    let mut traces = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|file_type| file_type.is_file())
                && entry.file_name().to_str().is_some_and(|name| {
                    name.starts_with("controller-input-trace-") && name.ends_with(".jsonl")
                })
        })
        .collect::<Vec<_>>();
    traces.sort_by_key(|entry| entry.file_name());

    let remove_count = traces.len().saturating_sub(keep_existing);
    for entry in traces.into_iter().take(remove_count) {
        fs::remove_file(entry.path())?;
    }

    Ok(())
}

fn unix_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn wup_trace_players_to_json(trace: &WupInputTrace) -> String {
    format!(
        "{{\"capture_report_count\":{},\"inputs\":[{},{}],\"players\":[{},{}]}}",
        trace.capture_report_count,
        player_input_to_json(trace.inputs[0]),
        player_input_to_json(trace.inputs[1]),
        trace.players[0].map_or_else(|| "null".to_string(), wup_player_trace_to_json),
        trace.players[1].map_or_else(|| "null".to_string(), wup_player_trace_to_json)
    )
}

fn wup_player_trace_to_json(player: crate::WupPlayerInputTrace) -> String {
    format!(
        concat!(
            "{{",
            "\"source_port\":{},",
            "\"raw\":{},",
            "\"origin\":{},",
            "\"origin_adjusted\":{},",
            "\"native\":{},",
            "\"ucf\":{},",
            "\"dashback_amendment\":{},",
            "\"input\":{},",
            "\"melee\":{}",
            "}}"
        ),
        player.source_port,
        gamecube_pad_to_json(player.raw),
        gamecube_pad_to_json(player.origin),
        gamecube_pad_to_json(player.origin_adjusted),
        gamecube_pad_to_json(player.native),
        gamecube_pad_to_json(player.ucf),
        player.dashback_amendment,
        player_input_to_json(player.input),
        melee_to_json(Some(MeleeReadout::from_snapshot(player.snapshot)))
    )
}

fn gamecube_pad_to_json(pad: GameCubePadStatus) -> String {
    let (stick_x, stick_y) = pad.main_stick_i8();
    let (c_stick_x, c_stick_y) = pad.c_stick_i8();
    format!(
        concat!(
            "{{",
            "\"stick_x\":{},",
            "\"stick_y\":{},",
            "\"c_stick_x\":{},",
            "\"c_stick_y\":{},",
            "\"left_trigger\":{},",
            "\"right_trigger\":{},",
            "\"buttons\":{},",
            "\"main_x\":{},",
            "\"main_y\":{},",
            "\"c_x\":{},",
            "\"c_y\":{}",
            "}}"
        ),
        pad.stick_x,
        pad.stick_y,
        pad.c_stick_x,
        pad.c_stick_y,
        pad.left_trigger,
        pad.right_trigger,
        pad.buttons.bits(),
        stick_x,
        stick_y,
        c_stick_x,
        c_stick_y
    )
}

fn player_input_to_json(input: PlayerInput) -> String {
    format!(
        concat!(
            "{{",
            "\"bits\":{},",
            "\"stick_x\":{},",
            "\"stick_y\":{},",
            "\"c_stick_x\":{},",
            "\"c_stick_y\":{},",
            "\"left_trigger\":{},",
            "\"right_trigger\":{},",
            "\"attack\":{},",
            "\"special\":{},",
            "\"jump\":{},",
            "\"shield\":{},",
            "\"grab\":{},",
            "\"ucf_dashback_amendment\":{}",
            "}}"
        ),
        input.bits(),
        input.stick_x(),
        input.stick_y(),
        input.c_stick_x(),
        input.c_stick_y(),
        input.left_trigger_analog(),
        input.right_trigger_analog(),
        input.attack(),
        input.special(),
        input.jump(),
        input.shield(),
        input.grab(),
        input.ucf_dashback_amendment()
    )
}

fn core_frame_to_json(frame: &RenderFrame) -> String {
    format!(
        "{{\"frame\":{},\"checksum\":{},\"players\":[{},{}]}}",
        frame.frame.0,
        frame.checksum,
        core_player_to_json(frame, 0),
        core_player_to_json(frame, 1)
    )
}

fn core_player_to_json(frame: &RenderFrame, index: usize) -> String {
    format!(
        concat!(
            "{{",
            "\"index\":{},",
            "\"action_state_id\":{},",
            "\"source_action_key\":{},",
            "\"motion_state\":\"{:?}\",",
            "\"motion_state_alias\":{},",
            "\"state_frame\":{},",
            "\"animation_frame\":{},",
            "\"facing\":{},",
            "\"position_x\":{},",
            "\"position_y\":{},",
            "\"velocity_x\":{},",
            "\"velocity_y\":{},",
            "\"damage_percent\":{},",
            "\"damage_percent_temp\":{},",
            "\"damage_applied\":{},",
            "\"damage_knockback\":{},",
            "\"damage_angle\":{},",
            "\"damage_element\":{},",
            "\"hitlag_frames\":{},",
            "\"damage_hitstun_frames\":{},",
            "\"ground_velocity_x\":{},",
            "\"ground_accel_x\":{},",
            "\"ground_accel_x2\":{},",
            "\"dash_entry_velocity_delta\":{},",
            "\"dash_x0\":{},",
            "\"walk_anim_velocity_x\":{},",
            "\"walk_accel_mul_milli\":{},",
            "\"turn_facing_after\":{},",
            "\"turn_has_turned\":{},",
            "\"turn_just_turned\":{},",
            "\"turn_frames_to_turn\":{},",
            "\"turn_dash_after_direction\":{},",
            "\"turn_latched_buttons\":{},",
            "\"run_no_interrupt_frames\":{},",
            "\"motion_cmd_var0\":{},",
            "\"motion_cmd_var1\":{},",
            "\"run_brake_x0\":{},",
            "\"run_brake_frames_remaining\":{},",
            "\"turn_run_accel_mul\":{},",
            "\"turn_run_x14\":{},",
            "\"motion_anim_rate_milli\":{},",
            "\"core_facts\":{}",
            "}}"
        ),
        index,
        optional_action_state_id_json(frame.player_action_state_ids[index]),
        optional_source_action_key_json(frame.player_source_action_keys[index]),
        frame.player_motion_states[index],
        optional_motion_state_json(frame.player_motion_state_aliases[index]),
        frame.player_state_frames[index],
        frame.player_animation_frames[index],
        frame.player_facings[index],
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
        frame.player_ground_velocity_x[index],
        frame.player_ground_accel_x[index],
        frame.player_ground_accel_x2[index],
        frame.player_dash_entry_velocity_delta[index],
        frame.player_dash_x0[index],
        frame.player_walk_anim_velocity_x[index],
        frame.player_walk_accel_mul_milli[index],
        frame.player_turn_facing_after[index],
        frame.player_turn_has_turned[index],
        frame.player_turn_just_turned[index],
        frame.player_turn_frames_to_turn[index],
        frame.player_turn_dash_after_direction[index],
        frame.player_turn_latched_buttons[index],
        frame.player_run_no_interrupt_frames[index],
        frame.player_motion_cmd_var0[index],
        frame.player_motion_cmd_var1[index],
        frame.player_run_brake_x0[index],
        frame.player_run_brake_frames_remaining[index],
        frame.player_turn_run_accel_mul[index],
        frame.player_turn_run_x14[index],
        frame.player_motion_anim_rate_milli[index],
        core_facts_to_json(frame.player_debug_input_facts[index])
    )
}

fn optional_action_state_id_json(value: Option<MeleeActionStateId>) -> String {
    value
        .map(|action| action.get().to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_source_action_key_json(value: Option<SourceActionKey>) -> String {
    value
        .map(|key| format!("\"{}\"", key.as_str()))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_motion_state_json(value: Option<MotionState>) -> String {
    value
        .map(|state| format!("\"{state:?}\""))
        .unwrap_or_else(|| "null".to_string())
}

fn core_facts_to_json(facts: MeleeInputFacts) -> String {
    format!(
        concat!(
            "{{",
            "\"walk_direction\":{},",
            "\"walk_speed_bucket\":\"{}\",",
            "\"turn_direction\":{},",
            "\"horizontal_smash_direction\":{},",
            "\"held_dash_x_direction\":{},",
            "\"dash_direction\":{},",
            "\"crouch\":{},",
            "\"tap_jump\":{},",
            "\"jump_pressed\":{},",
            "\"shield_held\":{},",
            "\"air_dodge_pressed\":{},",
            "\"source_held_bits\":{},",
            "\"source_pressed_bits\":{},",
            "\"source_released_bits\":{}",
            "}}"
        ),
        facts.walk_direction,
        walk_speed_bucket_json(facts.walk_speed_bucket),
        facts.turn_direction,
        facts.horizontal_smash_direction,
        facts.held_dash_x_direction,
        facts.dash_direction,
        facts.crouch,
        facts.tap_jump,
        facts.jump_pressed,
        facts.shield_held,
        facts.air_dodge_pressed,
        facts.source_held.bits(),
        facts.source_pressed.bits(),
        facts.source_released.bits()
    )
}

fn player_to_json(player: PlayerReadout) -> String {
    format!(
        concat!(
            "{{",
            "\"connected\":{},",
            "\"source_port\":{},",
            "\"raw_main_x\":{},",
            "\"raw_main_y\":{},",
            "\"raw_c_x\":{},",
            "\"raw_c_y\":{},",
            "\"main_x\":{},",
            "\"main_y\":{},",
            "\"c_x\":{},",
            "\"c_y\":{},",
            "\"left_trigger\":{},",
            "\"right_trigger\":{},",
            "\"a\":{},",
            "\"b\":{},",
            "\"x\":{},",
            "\"y\":{},",
            "\"z\":{},",
            "\"l\":{},",
            "\"r\":{},",
            "\"start\":{},",
            "\"dpad_up\":{},",
            "\"dpad_down\":{},",
            "\"dpad_left\":{},",
            "\"dpad_right\":{},",
            "\"melee\":{}",
            "}}"
        ),
        player.connected,
        source_port_json(player.source_port),
        player.raw_stick_x,
        player.raw_stick_y,
        player.raw_c_stick_x,
        player.raw_c_stick_y,
        player.stick_x,
        player.stick_y,
        player.c_stick_x,
        player.c_stick_y,
        player.left_trigger,
        player.right_trigger,
        player.buttons.a,
        player.buttons.b,
        player.buttons.x,
        player.buttons.y,
        player.buttons.z,
        player.buttons.left_trigger,
        player.buttons.right_trigger,
        player.buttons.start,
        player.buttons.dpad_up,
        player.buttons.dpad_down,
        player.buttons.dpad_left,
        player.buttons.dpad_right,
        melee_to_json(player.melee)
    )
}

fn melee_to_json(melee: Option<MeleeReadout>) -> String {
    let Some(melee) = melee else {
        return "null".to_string();
    };

    format!(
        concat!(
            "{{",
            "\"lstick_x\":{},",
            "\"lstick_y\":{},",
            "\"prev_lstick_x\":{},",
            "\"prev_lstick_y\":{},",
            "\"cstick_x\":{},",
            "\"cstick_y\":{},",
            "\"prev_cstick_x\":{},",
            "\"prev_cstick_y\":{},",
            "\"x_tap_timer\":{},",
            "\"y_tap_timer\":{},",
            "\"trigger_timer\":{},",
            "\"left_trigger\":{},",
            "\"right_trigger\":{},",
            "\"walk_direction\":{},",
            "\"tilt_direction_x\":{},",
            "\"tilt_direction_y\":{},",
            "\"horizontal_smash_direction\":{},",
            "\"dash_direction\":{},",
            "\"crouch\":{},",
            "\"tap_jump\":{},",
            "\"button_jump_pressed\":{},",
            "\"button_jump_held\":{},",
            "\"cstick_jump\":{},",
            "\"normal_jump_input\":\"{}\",",
            "\"normal_jump_pressed\":{},",
            "\"jump_input\":\"{}\",",
            "\"jump_pressed\":{},",
            "\"fast_fall\":{},",
            "\"lstick_jump_released\":{},",
            "\"cstick_jump_released\":{},",
            "\"source_held_bits\":{},",
            "\"source_pressed_bits\":{},",
            "\"source_released_bits\":{},",
            "\"source_lr_held\":{},",
            "\"source_lr_pressed\":{},",
            "\"source_lr_released\":{},",
            "\"source_a_held\":{},",
            "\"source_a_pressed\":{},",
            "\"source_z_held\":{},",
            "\"source_z_pressed\":{},",
            "\"attack_pressed\":{},",
            "\"special_pressed\":{},",
            "\"grab_pressed\":{},",
            "\"neutral_attack_pressed\":{},",
            "\"tilt_attack_direction_x\":{},",
            "\"tilt_attack_direction_y\":{},",
            "\"smash_attack_direction_x\":{},",
            "\"smash_attack_direction_y\":{},",
            "\"shield_held\":{},",
            "\"shield_pressed\":{},",
            "\"shield_released\":{},",
            "\"analog_shield\":{},",
            "\"analog_shield_pressed\":{},",
            "\"digital_shield_pressed\":{},",
            "\"air_dodge_pressed\":{},",
            "\"left_trigger_analog_held\":{},",
            "\"right_trigger_analog_held\":{},",
            "\"left_trigger_analog_pressed\":{},",
            "\"right_trigger_analog_pressed\":{},",
            "\"left_trigger_digital_pressed\":{},",
            "\"right_trigger_digital_pressed\":{},",
            "\"cstick_direction_x\":{},",
            "\"cstick_direction_y\":{},",
            "\"cstick_smash_direction_x\":{},",
            "\"cstick_smash_direction_y\":{},",
            "\"dpad_up\":{},",
            "\"dpad_down\":{},",
            "\"dpad_left\":{},",
            "\"dpad_right\":{}",
            "}}"
        ),
        melee.lstick_x,
        melee.lstick_y,
        melee.prev_lstick_x,
        melee.prev_lstick_y,
        melee.cstick_x,
        melee.cstick_y,
        melee.prev_cstick_x,
        melee.prev_cstick_y,
        melee.x_tap_timer,
        melee.y_tap_timer,
        melee.trigger_timer,
        melee.left_trigger,
        melee.right_trigger,
        melee.facts.walk_direction,
        melee.facts.tilt_direction.0,
        melee.facts.tilt_direction.1,
        melee.facts.horizontal_smash_direction,
        melee.facts.dash_direction,
        melee.facts.crouch,
        melee.facts.tap_jump,
        melee.facts.button_jump_pressed,
        melee.facts.button_jump_held,
        melee.facts.cstick_jump,
        melee_jump_input_json(melee.facts.normal_jump_input),
        melee.facts.normal_jump_pressed,
        melee_jump_input_json(melee.facts.jump_input),
        melee.facts.jump_pressed,
        melee.facts.fast_fall,
        melee.facts.lstick_jump_released,
        melee.facts.cstick_jump_released,
        melee.facts.source_held.bits(),
        melee.facts.source_pressed.bits(),
        melee.facts.source_released.bits(),
        melee.facts.source_held.lr(),
        melee.facts.source_pressed.lr(),
        melee.facts.source_released.lr(),
        melee.facts.source_held.a(),
        melee.facts.source_pressed.a(),
        melee.facts.source_held.z(),
        melee.facts.source_pressed.z(),
        melee.facts.attack_pressed,
        melee.facts.special_pressed,
        melee.facts.grab_pressed,
        melee.facts.neutral_attack_pressed,
        melee.facts.tilt_attack_direction.0,
        melee.facts.tilt_attack_direction.1,
        melee.facts.smash_attack_direction.0,
        melee.facts.smash_attack_direction.1,
        melee.facts.shield_held,
        melee.facts.shield_pressed,
        melee.facts.shield_released,
        melee.facts.analog_shield,
        melee.facts.analog_shield_pressed,
        melee.facts.digital_shield_pressed,
        melee.facts.air_dodge_pressed,
        melee.facts.left_trigger_analog_held,
        melee.facts.right_trigger_analog_held,
        melee.facts.left_trigger_analog_pressed,
        melee.facts.right_trigger_analog_pressed,
        melee.facts.left_trigger_digital_pressed,
        melee.facts.right_trigger_digital_pressed,
        melee.facts.cstick_direction.0,
        melee.facts.cstick_direction.1,
        melee.facts.cstick_smash_direction.0,
        melee.facts.cstick_smash_direction.1,
        melee.facts.dpad_up,
        melee.facts.dpad_down,
        melee.facts.dpad_left,
        melee.facts.dpad_right
    )
}

fn melee_jump_input_json(input: MeleeJumpInput) -> &'static str {
    match input {
        MeleeJumpInput::None => "none",
        MeleeJumpInput::LStick => "lstick",
        MeleeJumpInput::XY => "xy",
        MeleeJumpInput::CStick => "cstick",
    }
}

fn walk_speed_bucket_json(bucket: WalkSpeedBucket) -> &'static str {
    match bucket {
        WalkSpeedBucket::None => "none",
        WalkSpeedBucket::Slow => "slow",
        WalkSpeedBucket::Middle => "middle",
        WalkSpeedBucket::Fast => "fast",
    }
}

fn source_port_json(source_port: Option<usize>) -> String {
    source_port
        .map(|port| port.to_string())
        .unwrap_or_else(|| "null".to_string())
}
