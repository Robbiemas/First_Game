use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::{SocketAddr, UdpSocket};

use mole_core::{Frame, PlayerInput};

pub const INPUT_PACKET_VERSION: u8 = 2;
pub const INPUT_PACKET_WIRE_LEN: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputPacket {
    pub version: u8,
    pub frame: Frame,
    pub player_index: u8,
    pub input: PlayerInput,
    pub checksum: u64,
    pub sequence: u32,
    pub ack_sequence: u32,
}

impl InputPacket {
    pub const fn new(frame: Frame, player_index: u8, input: PlayerInput, checksum: u64) -> Self {
        Self {
            version: INPUT_PACKET_VERSION,
            frame,
            player_index,
            input,
            checksum,
            sequence: frame.0,
            ack_sequence: 0,
        }
    }

    pub const fn with_timing_probe(mut self, sequence: u32, ack_sequence: u32) -> Self {
        self.sequence = sequence;
        self.ack_sequence = ack_sequence;
        self
    }

    pub fn to_wire_bytes(self) -> [u8; INPUT_PACKET_WIRE_LEN] {
        let mut bytes = [0u8; INPUT_PACKET_WIRE_LEN];
        bytes[0] = self.version;
        bytes[1..5].copy_from_slice(&self.frame.0.to_be_bytes());
        bytes[5] = self.player_index;
        bytes[6..14].copy_from_slice(&self.input.bits().to_be_bytes());
        bytes[14..22].copy_from_slice(&self.checksum.to_be_bytes());
        bytes[22..26].copy_from_slice(&self.sequence.to_be_bytes());
        bytes[26..30].copy_from_slice(&self.ack_sequence.to_be_bytes());
        bytes
    }

    pub fn from_wire_bytes(bytes: &[u8]) -> Result<Self, PacketDecodeError> {
        if bytes.len() != INPUT_PACKET_WIRE_LEN {
            return Err(PacketDecodeError::WrongLength {
                expected: INPUT_PACKET_WIRE_LEN,
                actual: bytes.len(),
            });
        }
        if bytes[0] != INPUT_PACKET_VERSION {
            return Err(PacketDecodeError::UnsupportedVersion(bytes[0]));
        }

        let frame = Frame(u32::from_be_bytes(
            bytes[1..5].try_into().expect("frame slice has fixed width"),
        ));
        let player_index = bytes[5];
        let input = PlayerInput::from_bits(u64::from_be_bytes(
            bytes[6..14]
                .try_into()
                .expect("input slice has fixed width"),
        ));
        let checksum = u64::from_be_bytes(
            bytes[14..22]
                .try_into()
                .expect("checksum slice has fixed width"),
        );
        let sequence = u32::from_be_bytes(
            bytes[22..26]
                .try_into()
                .expect("sequence slice has fixed width"),
        );
        let ack_sequence = u32::from_be_bytes(
            bytes[26..30]
                .try_into()
                .expect("ack sequence slice has fixed width"),
        );

        Ok(Self {
            version: bytes[0],
            frame,
            player_index,
            input,
            checksum,
            sequence,
            ack_sequence,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDecodeError {
    WrongLength { expected: usize, actual: usize },
    UnsupportedVersion(u8),
}

pub trait Transport {
    fn send(&mut self, packet: InputPacket);
    fn try_recv(&mut self) -> Option<InputPacket>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportTimingSummary {
    pub sample_count: usize,
    pub min_rtt_frames: u32,
    pub max_rtt_frames: u32,
    pub average_rtt_milliframes: u32,
    pub average_jitter_milliframes: u32,
}

impl TransportTimingSummary {
    pub fn from_rtt_frames(samples: impl IntoIterator<Item = u32>) -> Option<Self> {
        let samples: Vec<u32> = samples.into_iter().collect();
        let sample_count = samples.len();
        if sample_count == 0 {
            return None;
        }

        let min_rtt_frames = samples
            .iter()
            .copied()
            .min()
            .expect("sample list is nonempty");
        let max_rtt_frames = samples
            .iter()
            .copied()
            .max()
            .expect("sample list is nonempty");
        let rtt_sum: u64 = samples.iter().map(|sample| u64::from(*sample)).sum();
        let average_rtt_milliframes = milliframe_average(rtt_sum, sample_count);

        let jitter_sum: u64 = samples
            .windows(2)
            .map(|pair| pair[0].abs_diff(pair[1]) as u64)
            .sum();
        let jitter_count = sample_count.saturating_sub(1);
        let average_jitter_milliframes = if jitter_count == 0 {
            0
        } else {
            milliframe_average(jitter_sum, jitter_count)
        };

        Some(Self {
            sample_count,
            min_rtt_frames,
            max_rtt_frames,
            average_rtt_milliframes,
            average_jitter_milliframes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportTimingProfile {
    pub name: String,
    pub summary: Option<TransportTimingSummary>,
}

impl TransportTimingProfile {
    pub fn measured(name: impl Into<String>, rtt_frames: impl IntoIterator<Item = u32>) -> Self {
        Self {
            name: name.into(),
            summary: TransportTimingSummary::from_rtt_frames(rtt_frames),
        }
    }

    pub fn unmeasured(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            summary: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportTimingVerdict {
    BaselinePreferred,
    CandidatePreferred,
    Tie,
    CandidateUnmeasured,
    BaselineUnmeasured,
    BothUnmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportTimingComparison {
    pub baseline: TransportTimingProfile,
    pub candidate: TransportTimingProfile,
}

impl TransportTimingComparison {
    pub const fn new(baseline: TransportTimingProfile, candidate: TransportTimingProfile) -> Self {
        Self {
            baseline,
            candidate,
        }
    }

    pub fn verdict(&self) -> TransportTimingVerdict {
        match (self.baseline.summary, self.candidate.summary) {
            (None, None) => TransportTimingVerdict::BothUnmeasured,
            (None, Some(_)) => TransportTimingVerdict::BaselineUnmeasured,
            (Some(_), None) => TransportTimingVerdict::CandidateUnmeasured,
            (Some(baseline), Some(candidate)) => {
                match (
                    candidate
                        .average_rtt_milliframes
                        .cmp(&baseline.average_rtt_milliframes),
                    candidate
                        .average_jitter_milliframes
                        .cmp(&baseline.average_jitter_milliframes),
                ) {
                    (std::cmp::Ordering::Less, _) => TransportTimingVerdict::CandidatePreferred,
                    (std::cmp::Ordering::Greater, _) => TransportTimingVerdict::BaselinePreferred,
                    (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => {
                        TransportTimingVerdict::CandidatePreferred
                    }
                    (std::cmp::Ordering::Equal, std::cmp::Ordering::Greater) => {
                        TransportTimingVerdict::BaselinePreferred
                    }
                    (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => {
                        TransportTimingVerdict::Tie
                    }
                }
            }
        }
    }

    pub fn preferred_transport_name(&self) -> &str {
        match self.verdict() {
            TransportTimingVerdict::CandidatePreferred
            | TransportTimingVerdict::BaselineUnmeasured => self.candidate.name.as_str(),
            TransportTimingVerdict::BaselinePreferred
            | TransportTimingVerdict::Tie
            | TransportTimingVerdict::CandidateUnmeasured
            | TransportTimingVerdict::BothUnmeasured => self.baseline.name.as_str(),
        }
    }
}

fn milliframe_average(sum_frames: u64, sample_count: usize) -> u32 {
    ((sum_frames * 1_000) / sample_count as u64) as u32
}

#[derive(Debug, Default, Clone)]
pub struct LoopbackTransport {
    packets: VecDeque<InputPacket>,
}

impl Transport for LoopbackTransport {
    fn send(&mut self, packet: InputPacket) {
        self.packets.push_back(packet);
    }

    fn try_recv(&mut self) -> Option<InputPacket> {
        self.packets.pop_front()
    }
}

#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    peer: SocketAddr,
}

impl UdpTransport {
    pub fn bind(local: SocketAddr, peer: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(local)?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, peer })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub const fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    pub fn send_packet(&self, packet: InputPacket) -> io::Result<()> {
        self.socket.send_to(&packet.to_wire_bytes(), self.peer)?;
        Ok(())
    }

    pub fn try_recv_packet(&self) -> io::Result<Option<InputPacket>> {
        let mut bytes = [0u8; INPUT_PACKET_WIRE_LEN];
        match self.socket.recv_from(&mut bytes) {
            Ok((len, source)) => {
                if source != self.peer {
                    return Ok(None);
                }
                InputPacket::from_wire_bytes(&bytes[..len])
                    .map(Some)
                    .map_err(|error| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}"))
                    })
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::ConnectionReset
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl Transport for UdpTransport {
    fn send(&mut self, packet: InputPacket) {
        let _ = self.send_packet(packet);
    }

    fn try_recv(&mut self) -> Option<InputPacket> {
        self.try_recv_packet().ok().flatten()
    }
}

#[cfg(feature = "webrtc")]
pub trait DataChannelPort {
    fn send_bytes(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn try_recv_bytes(&mut self) -> io::Result<Option<Vec<u8>>>;
}

#[cfg(feature = "webrtc")]
#[derive(Debug)]
pub struct WebRtcDataChannelTransport<P> {
    port: P,
}

#[cfg(feature = "webrtc")]
impl<P> WebRtcDataChannelTransport<P>
where
    P: DataChannelPort,
{
    pub const fn new(port: P) -> Self {
        Self { port }
    }

    pub fn send_packet(&mut self, packet: InputPacket) -> io::Result<()> {
        self.port.send_bytes(&packet.to_wire_bytes())
    }

    pub fn try_recv_packet(&mut self) -> io::Result<Option<InputPacket>> {
        let Some(bytes) = self.port.try_recv_bytes()? else {
            return Ok(None);
        };

        InputPacket::from_wire_bytes(&bytes)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")))
    }
}

#[cfg(feature = "webrtc")]
impl<P> Transport for WebRtcDataChannelTransport<P>
where
    P: DataChannelPort,
{
    fn send(&mut self, packet: InputPacket) {
        let _ = self.send_packet(packet);
    }

    fn try_recv(&mut self) -> Option<InputPacket> {
        self.try_recv_packet().ok().flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketAcceptResult {
    Accepted,
    Duplicate,
    UnsupportedVersion,
}

#[derive(Debug, Default, Clone)]
pub struct InputPacketInbox {
    packets: BTreeMap<(Frame, u8), InputPacket>,
}

impl InputPacketInbox {
    pub fn accept(&mut self, packet: InputPacket) -> PacketAcceptResult {
        if packet.version != INPUT_PACKET_VERSION {
            return PacketAcceptResult::UnsupportedVersion;
        }

        let key = (packet.frame, packet.player_index);
        if self.packets.contains_key(&key) {
            return PacketAcceptResult::Duplicate;
        }

        self.packets.insert(key, packet);
        PacketAcceptResult::Accepted
    }

    pub fn input(&self, frame: Frame, player_index: u8) -> Option<PlayerInput> {
        self.packets
            .get(&(frame, player_index))
            .map(|packet| packet.input)
    }
}
