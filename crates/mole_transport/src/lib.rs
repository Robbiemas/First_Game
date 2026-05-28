use std::collections::{BTreeMap, VecDeque};

use mole_core::{Frame, PlayerInput};

pub const INPUT_PACKET_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputPacket {
    pub version: u8,
    pub frame: Frame,
    pub player_index: u8,
    pub input: PlayerInput,
    pub checksum: u64,
}

impl InputPacket {
    pub const fn new(frame: Frame, player_index: u8, input: PlayerInput, checksum: u64) -> Self {
        Self {
            version: INPUT_PACKET_VERSION,
            frame,
            player_index,
            input,
            checksum,
        }
    }
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
