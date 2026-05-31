use crate::map_gamecube_pad_to_player_input;
use mole_core::{
    GameCubeButtonState, GameCubePadStatus, MeleeInputProcessor, MeleeInputSnapshot, PlayerInput,
};
use mole_input::{
    gamecube_pad_with_origin, native_gamecube_pad_with_origin, UcfInputPreprocessor,
    UcfPreprocessedPad,
};

#[cfg(feature = "wup")]
use crate::InputSource;
#[cfg(feature = "wup")]
use mole_core::Frame;

#[cfg(feature = "wup")]
use std::time::Duration;

#[cfg(feature = "wup")]
const WUP_VENDOR_ID: u16 = 0x057e;
#[cfg(feature = "wup")]
const WUP_PRODUCT_ID: u16 = 0x0337;
#[cfg(feature = "wup")]
const WUP_READ_ENDPOINT: u8 = 0x81;
#[cfg(feature = "wup")]
const WUP_WRITE_ENDPOINT: u8 = 0x02;

const WUP_REPORT_ID: u8 = 0x21;
const PORT_COUNT: usize = 4;
const PORT_STRIDE: usize = 9;
const PORTS_OFFSET: usize = 1;
pub const GAMECUBE_RECENTER_FRAMES: u16 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WupInputConfig {
    pub ucf_enabled: bool,
}

impl Default for WupInputConfig {
    fn default() -> Self {
        Self { ucf_enabled: true }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WupPort {
    pub connected: bool,
    pub pad: GameCubePadStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WupInputTrace {
    pub ucf_enabled: bool,
    pub adapter_ports: [bool; PORT_COUNT],
    pub inputs: [PlayerInput; 2],
    pub players: [Option<WupPlayerInputTrace>; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WupPlayerInputTrace {
    pub source_port: usize,
    pub raw: GameCubePadStatus,
    pub origin: GameCubePadStatus,
    pub origin_adjusted: GameCubePadStatus,
    pub native: GameCubePadStatus,
    pub ucf: GameCubePadStatus,
    pub dashback_amendment: bool,
    pub snapshot: MeleeInputSnapshot,
    pub input: PlayerInput,
}

pub fn parse_wup_report(report: [u8; 37]) -> [WupPort; PORT_COUNT] {
    if report[0] != WUP_REPORT_ID {
        return [WupPort::default(); PORT_COUNT];
    }

    let mut ports = [WupPort::default(); PORT_COUNT];
    for (port_index, port) in ports.iter_mut().enumerate() {
        let offset = PORTS_OFFSET + port_index * PORT_STRIDE;
        *port = parse_port(&report[offset..offset + PORT_STRIDE]);
    }
    ports
}

pub fn map_wup_ports_to_player_inputs(ports: [WupPort; PORT_COUNT]) -> [PlayerInput; 2] {
    let mut inputs = [PlayerInput::neutral(), PlayerInput::neutral()];
    let mut player_index = 0;

    for port in ports {
        if port.connected {
            inputs[player_index] = map_gamecube_pad_to_player_input(port.pad);
            player_index += 1;
            if player_index == inputs.len() {
                break;
            }
        }
    }

    inputs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WupInputMapper {
    config: WupInputConfig,
    origins: [Option<GameCubePadStatus>; PORT_COUNT],
    recenter_frames: [u16; PORT_COUNT],
    melee_processors: [MeleeInputProcessor; PORT_COUNT],
    ucf_preprocessors: [UcfInputPreprocessor; PORT_COUNT],
}

impl Default for WupInputMapper {
    fn default() -> Self {
        Self::new(WupInputConfig::default())
    }
}

impl WupInputMapper {
    pub fn new(config: WupInputConfig) -> Self {
        Self {
            config,
            origins: [None; PORT_COUNT],
            recenter_frames: [0; PORT_COUNT],
            melee_processors: [MeleeInputProcessor::default(); PORT_COUNT],
            ucf_preprocessors: [UcfInputPreprocessor::default(); PORT_COUNT],
        }
    }
}

impl WupInputMapper {
    pub fn map_ports(&mut self, ports: [WupPort; PORT_COUNT]) -> [PlayerInput; 2] {
        self.map_ports_to_input_trace(ports).inputs
    }

    pub fn map_ports_to_melee_snapshots(
        &mut self,
        ports: [WupPort; PORT_COUNT],
    ) -> [Option<MeleeInputSnapshot>; 2] {
        self.map_ports_to_input_trace(ports)
            .players
            .map(|trace| trace.map(|player| player.snapshot))
    }

    pub fn map_ports_to_input_trace(&mut self, ports: [WupPort; PORT_COUNT]) -> WupInputTrace {
        let mut trace = WupInputTrace {
            ucf_enabled: self.config.ucf_enabled,
            ..WupInputTrace::default()
        };
        let mut player_index = 0;

        for (port_index, port) in ports.into_iter().enumerate() {
            trace.adapter_ports[port_index] = port.connected;
            if !port.connected {
                self.origins[port_index] = None;
                self.recenter_frames[port_index] = 0;
                self.melee_processors[port_index] = MeleeInputProcessor::default();
                self.ucf_preprocessors[port_index] = UcfInputPreprocessor::default();
                continue;
            }

            if self.origins[port_index].is_none() {
                self.origins[port_index] = Some(port.pad);
                self.melee_processors[port_index] = MeleeInputProcessor::default();
                self.ucf_preprocessors[port_index] = UcfInputPreprocessor::default();
            }
            if self.update_recenter_combo(port_index, port.pad) {
                self.melee_processors[port_index] = MeleeInputProcessor::default();
                self.ucf_preprocessors[port_index] = UcfInputPreprocessor::default();
            }

            if player_index < trace.players.len() {
                let origin = self.origins[port_index].unwrap_or(port.pad);
                let calibrated = gamecube_pad_with_origin(port.pad, origin);
                let native = native_gamecube_pad_with_origin(port.pad, origin);
                let preprocessed = if self.config.ucf_enabled {
                    self.ucf_preprocessors[port_index]
                        .preprocess_pad_with_native_result(calibrated, native)
                } else {
                    UcfPreprocessedPad {
                        pad: native,
                        dashback_amendment: false,
                    }
                };
                let snapshot = self.melee_processors[port_index].update(preprocessed.pad);
                let input = player_input_from_snapshot(snapshot)
                    .with_ucf_dashback_amendment(preprocessed.dashback_amendment);
                trace.inputs[player_index] = input;
                trace.players[player_index] = Some(WupPlayerInputTrace {
                    source_port: port_index,
                    raw: port.pad,
                    origin,
                    origin_adjusted: calibrated,
                    native,
                    ucf: preprocessed.pad,
                    dashback_amendment: preprocessed.dashback_amendment,
                    snapshot,
                    input,
                });
                player_index += 1;
            }
        }

        trace
    }

    fn update_recenter_combo(&mut self, port_index: usize, pad: GameCubePadStatus) -> bool {
        if pad.buttons.x() && pad.buttons.y() && pad.buttons.start() {
            self.recenter_frames[port_index] += 1;
            if self.recenter_frames[port_index] >= GAMECUBE_RECENTER_FRAMES {
                self.origins[port_index] = Some(pad);
                self.recenter_frames[port_index] = 0;
                return true;
            }
            return false;
        }

        self.recenter_frames[port_index] = 0;
        false
    }
}

fn player_input_from_snapshot(snapshot: MeleeInputSnapshot) -> PlayerInput {
    PlayerInput::neutral()
        .with_left_stick(snapshot.lstick.0, snapshot.lstick.1)
        .with_c_stick(snapshot.cstick.0, snapshot.cstick.1)
        .with_left_trigger_analog(snapshot.left_trigger)
        .with_right_trigger_analog(snapshot.right_trigger)
        .with_left_trigger_digital(snapshot.held.l())
        .with_right_trigger_digital(snapshot.held.r())
        .with_attack(snapshot.held.a())
        .with_special(snapshot.held.b())
        .with_jump_primary(snapshot.held.x())
        .with_jump_secondary(snapshot.held.y())
        .with_grab(snapshot.held.z())
        .with_start(snapshot.held.start())
        .with_dpad_up(snapshot.held.dpad_up())
        .with_dpad_down(snapshot.held.dpad_down())
        .with_dpad_left(snapshot.held.dpad_left())
        .with_dpad_right(snapshot.held.dpad_right())
}

fn parse_port(bytes: &[u8]) -> WupPort {
    let status = bytes[0];
    let controller_type = (status >> 4) & 0b11;
    let connected = matches!(controller_type, 1 | 2);
    if !connected {
        return WupPort::default();
    }

    let buttons = u16::from_le_bytes([bytes[1], bytes[2]]);
    WupPort {
        connected,
        pad: GameCubePadStatus {
            stick_x: bytes[3],
            stick_y: bytes[4],
            c_stick_x: bytes[5],
            c_stick_y: bytes[6],
            left_trigger: bytes[7],
            right_trigger: bytes[8],
            buttons: GameCubeButtonState::from_bits(buttons),
        },
    }
}

#[cfg(feature = "wup")]
pub struct WupInputSource {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    mapper: WupInputMapper,
    latest: [PlayerInput; 2],
    latest_trace: Option<WupInputTrace>,
}

#[cfg(feature = "wup")]
impl WupInputSource {
    pub fn open() -> Result<Self, String> {
        Self::open_with_config(WupInputConfig::default())
    }

    pub fn open_with_config(config: WupInputConfig) -> Result<Self, String> {
        let handle = rusb::open_device_with_vid_pid(WUP_VENDOR_ID, WUP_PRODUCT_ID)
            .ok_or_else(|| "WUP-028 was not found on WinUSB/libusb.".to_string())?;

        if handle.kernel_driver_active(0).unwrap_or(false) {
            handle
                .detach_kernel_driver(0)
                .map_err(|error| error.to_string())?;
        }
        handle
            .claim_interface(0)
            .map_err(|error| error.to_string())?;
        handle
            .write_interrupt(WUP_WRITE_ENDPOINT, &[0x13], Duration::from_millis(100))
            .map_err(|error| error.to_string())?;

        Ok(Self {
            handle,
            mapper: WupInputMapper::new(config),
            latest: [PlayerInput::neutral(), PlayerInput::neutral()],
            latest_trace: None,
        })
    }

    pub fn poll_adapter(&mut self) -> Result<[PlayerInput; 2], rusb::Error> {
        self.latest = self.poll_traced_adapter()?.inputs;
        Ok(self.latest)
    }

    pub fn poll_traced_adapter(&mut self) -> Result<WupInputTrace, rusb::Error> {
        let ports = self.poll_ports()?;
        let trace = self.mapper.map_ports_to_input_trace(ports);
        self.latest = trace.inputs;
        self.latest_trace = Some(trace);
        Ok(trace)
    }

    pub const fn latest_inputs(&self) -> [PlayerInput; 2] {
        self.latest
    }

    pub const fn latest_trace(&self) -> Option<WupInputTrace> {
        self.latest_trace
    }

    pub fn poll_ports(&mut self) -> Result<[WupPort; PORT_COUNT], rusb::Error> {
        let mut report = [0u8; 37];
        self.handle
            .read_interrupt(WUP_READ_ENDPOINT, &mut report, Duration::from_millis(2))?;
        Ok(parse_wup_report(report))
    }
}

#[cfg(feature = "wup")]
impl InputSource for WupInputSource {
    fn poll_inputs(&mut self, _frame: Frame) -> [PlayerInput; 2] {
        match self.poll_adapter() {
            Ok(inputs) => inputs,
            Err(rusb::Error::Timeout) => self.latest,
            Err(_) => [PlayerInput::neutral(), PlayerInput::neutral()],
        }
    }
}

#[cfg(feature = "wup")]
impl Drop for WupInputSource {
    fn drop(&mut self) {
        let _ = self.handle.write_interrupt(
            WUP_WRITE_ENDPOINT,
            &[0x11, 0, 0, 0, 0],
            Duration::from_millis(100),
        );
        let _ = self.handle.release_interface(0);
    }
}
