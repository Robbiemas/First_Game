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
use std::collections::VecDeque;
#[cfg(feature = "wup")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
#[cfg(feature = "wup")]
use std::thread::{self, JoinHandle};
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
#[cfg(feature = "wup")]
const WUP_CAPTURE_DRAIN_LIMIT: usize = 32;
#[cfg(feature = "wup")]
const WUP_CAPTURE_QUEUE_CAPACITY: usize = 128;
#[cfg(feature = "wup")]
const WUP_GAMEPLAY_CAPTURE_POLL_TIMEOUT: Duration = Duration::ZERO;
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
    pub capture_report_count: u8,
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
        let mut trace = self.map_collapsed_ports_to_input_trace(ports);
        trace.capture_report_count = 1;
        trace
    }

    pub fn map_capture_window_to_input_trace(
        &mut self,
        samples: &[[WupPort; PORT_COUNT]],
    ) -> WupInputTrace {
        self.prime_origins_from_capture_window(samples);
        let mut trace =
            self.map_collapsed_ports_to_input_trace(collapse_wup_capture_window(samples));
        trace.capture_report_count = samples.len().min(u8::MAX as usize) as u8;
        trace
    }

    fn map_collapsed_ports_to_input_trace(
        &mut self,
        ports: [WupPort; PORT_COUNT],
    ) -> WupInputTrace {
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

    fn prime_origins_from_capture_window(&mut self, samples: &[[WupPort; PORT_COUNT]]) {
        for port_index in 0..PORT_COUNT {
            if self.origins[port_index].is_some() {
                continue;
            }
            let Some(first_connected) = samples.iter().find_map(|sample| {
                sample[port_index]
                    .connected
                    .then_some(sample[port_index].pad)
            }) else {
                continue;
            };
            self.origins[port_index] = Some(first_connected);
            self.recenter_frames[port_index] = 0;
            self.melee_processors[port_index] = MeleeInputProcessor::default();
            self.ucf_preprocessors[port_index] = UcfInputPreprocessor::default();
        }
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

pub fn collapse_wup_capture_window(samples: &[[WupPort; PORT_COUNT]]) -> [WupPort; PORT_COUNT] {
    let Some(latest) = samples.last() else {
        return [WupPort::default(); PORT_COUNT];
    };
    let mut collapsed = *latest;

    for port_index in 0..PORT_COUNT {
        if !collapsed[port_index].connected {
            continue;
        }

        let mut button_bits = 0u16;
        for sample in samples {
            if sample[port_index].connected {
                button_bits |= sample[port_index].pad.buttons.bits();
            }
        }
        collapsed[port_index].pad.buttons = GameCubeButtonState::from_bits(button_bits);
    }

    collapsed
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
    capture: Arc<WupCaptureQueue>,
    worker: Option<JoinHandle<()>>,
    mapper: WupInputMapper,
    latest: [PlayerInput; 2],
    latest_trace: Option<WupInputTrace>,
}

#[cfg(feature = "wup")]
#[derive(Debug)]
struct WupCaptureQueue {
    samples: Mutex<VecDeque<[WupPort; PORT_COUNT]>>,
    available: Condvar,
    stop: AtomicBool,
    last_error: Mutex<Option<rusb::Error>>,
}

#[cfg(feature = "wup")]
impl WupCaptureQueue {
    fn new() -> Self {
        Self {
            samples: Mutex::new(VecDeque::with_capacity(WUP_CAPTURE_QUEUE_CAPACITY)),
            available: Condvar::new(),
            stop: AtomicBool::new(false),
            last_error: Mutex::new(None),
        }
    }

    fn push_sample(&self, ports: [WupPort; PORT_COUNT]) {
        {
            let mut samples = self.samples.lock().expect("WUP sample mutex poisoned");
            if samples.len() >= WUP_CAPTURE_QUEUE_CAPACITY {
                samples.pop_front();
            }
            samples.push_back(ports);
        }
        *self.last_error.lock().expect("WUP error mutex poisoned") = None;
        self.available.notify_one();
    }

    fn record_error(&self, error: rusb::Error) {
        *self.last_error.lock().expect("WUP error mutex poisoned") = Some(error);
        self.available.notify_one();
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.available.notify_all();
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn drain_capture_window(
        &self,
        wait_timeout: Duration,
    ) -> Result<Vec<[WupPort; PORT_COUNT]>, rusb::Error> {
        let mut samples = self.samples.lock().expect("WUP sample mutex poisoned");
        if samples.is_empty() && !self.should_stop() {
            let (guard, _) = self
                .available
                .wait_timeout(samples, wait_timeout)
                .expect("WUP sample condvar poisoned");
            samples = guard;
        }

        if samples.is_empty() {
            drop(samples);
            if let Some(error) = *self.last_error.lock().expect("WUP error mutex poisoned") {
                return Err(error);
            }
            return Err(rusb::Error::Timeout);
        }

        let count = samples.len().min(WUP_CAPTURE_DRAIN_LIMIT);
        let skip = samples.len().saturating_sub(count);
        for _ in 0..skip {
            samples.pop_front();
        }

        Ok(samples.drain(..).collect())
    }
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
        let capture = Arc::new(WupCaptureQueue::new());
        let worker_capture = Arc::clone(&capture);
        let worker = thread::spawn(move || {
            let mut report = [0u8; 37];
            while !worker_capture.should_stop() {
                match handle.read_interrupt(
                    WUP_READ_ENDPOINT,
                    &mut report,
                    Duration::from_millis(2),
                ) {
                    Ok(_) => worker_capture.push_sample(parse_wup_report(report)),
                    Err(rusb::Error::Timeout) => {}
                    Err(error) => {
                        worker_capture.record_error(error);
                        thread::sleep(Duration::from_millis(2));
                    }
                }
            }

            let _ = handle.write_interrupt(
                WUP_WRITE_ENDPOINT,
                &[0x11, 0, 0, 0, 0],
                Duration::from_millis(100),
            );
            let _ = handle.release_interface(0);
        });

        Ok(Self {
            capture,
            worker: Some(worker),
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
        let samples = self
            .capture
            .drain_capture_window(WUP_GAMEPLAY_CAPTURE_POLL_TIMEOUT)?;
        let trace = self.mapper.map_capture_window_to_input_trace(&samples);
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
        let samples = self
            .capture
            .drain_capture_window(Duration::from_millis(2))?;
        samples.last().copied().ok_or(rusb::Error::Timeout)
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
        self.capture.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(all(test, feature = "wup"))]
mod tests {
    use super::*;

    #[test]
    fn capture_queue_drains_only_the_newest_bounded_window() {
        let queue = WupCaptureQueue::new();
        for stick_x in 0..40 {
            queue.push_sample(connected_sample(stick_x));
        }

        let samples = queue
            .drain_capture_window(Duration::from_millis(0))
            .expect("queued samples should drain without waiting");

        assert_eq!(samples.len(), WUP_CAPTURE_DRAIN_LIMIT);
        assert_eq!(samples[0][0].pad.stick_x, 8);
        assert_eq!(samples[31][0].pad.stick_x, 39);
    }

    #[test]
    fn capture_queue_empty_drain_times_out_without_blocking_for_usb() {
        let queue = WupCaptureQueue::new();

        let result = queue.drain_capture_window(Duration::from_millis(0));

        assert_eq!(result, Err(rusb::Error::Timeout));
    }

    #[test]
    fn gameplay_capture_poll_timeout_is_nonblocking() {
        assert_eq!(WUP_GAMEPLAY_CAPTURE_POLL_TIMEOUT, Duration::ZERO);
    }

    fn connected_sample(stick_x: u8) -> [WupPort; PORT_COUNT] {
        [
            WupPort {
                connected: true,
                pad: GameCubePadStatus {
                    stick_x,
                    ..GameCubePadStatus::neutral()
                },
            },
            WupPort::default(),
            WupPort::default(),
            WupPort::default(),
        ]
    }
}
