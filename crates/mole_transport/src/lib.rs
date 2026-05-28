use std::collections::VecDeque;

use mole_core::{Frame, PlayerInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputPacket {
    pub frame: Frame,
    pub player_index: u8,
    pub input: PlayerInput,
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
