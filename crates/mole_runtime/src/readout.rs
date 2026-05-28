use crate::{map_gamecube_pad_to_player_input, WupPort};
use mole_core::GameCubePadStatus;
use mole_core::{
    MeleeInputFacts, MeleeInputSnapshot, MeleeInputThresholds, MeleeJumpInput, PlayerInput,
    UCF_VERSION,
};

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
            shield: input.shield(),
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
    pub ucf_x_tilt_intent: bool,
    pub ucf_shield_drop_tilt_intent: bool,
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
            ucf_x_tilt_intent: snapshot.ucf_x_tilt_intent,
            ucf_shield_drop_tilt_intent: snapshot.ucf_shield_drop_tilt_intent,
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
            "\"ucf_version\":\"{}\",",
            "\"ucf_x_tilt_intent\":{},",
            "\"ucf_shield_drop_tilt_intent\":{},",
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
            "\"ucf_dashback_direction\":{},",
            "\"ucf_shield_drop\":{},",
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
        UCF_VERSION,
        melee.ucf_x_tilt_intent,
        melee.ucf_shield_drop_tilt_intent,
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
        melee.facts.ucf_dashback_direction,
        melee.facts.ucf_shield_drop,
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

fn source_port_json(source_port: Option<usize>) -> String {
    source_port
        .map(|port| port.to_string())
        .unwrap_or_else(|| "null".to_string())
}
