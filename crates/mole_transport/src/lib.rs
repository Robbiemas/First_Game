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
