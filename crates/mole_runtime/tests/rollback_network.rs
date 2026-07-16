use mole_core::{Frame, PlayerInput};
use mole_rollback::RollbackSession;
use mole_runtime::{default_play_world, step_world_with_source_collisions};
use mole_transport::{
    CompatibilityFingerprint, InputPacket, InputPacketDatagram, InputPacketInbox,
    PacketAcceptResult,
};

const ROLLBACK_WINDOW: u32 = 7;
const FRAME_COUNT: u32 = 18;
const SESSION: CompatibilityFingerprint = CompatibilityFingerprint::new(11, 22, 33);

struct Peer {
    local_player: usize,
    inbox: InputPacketInbox,
    session: RollbackSession,
}

impl Peer {
    fn new(local_player: usize) -> Self {
        Self {
            local_player,
            inbox: InputPacketInbox::with_rollback_horizon(ROLLBACK_WINDOW, SESSION),
            session: RollbackSession::new_with_step(
                default_play_world(),
                ROLLBACK_WINDOW as usize,
                step_world_with_source_collisions,
            ),
        }
    }

    fn receive(&mut self, bytes: &[u8], current_frame: Frame) -> Vec<PacketAcceptResult> {
        let datagram = InputPacketDatagram::from_wire_bytes(bytes).expect("wire datagram decodes");
        datagram
            .packets()
            .iter()
            .copied()
            .map(|packet| {
                assert_eq!(packet.player_index as usize, 1 - self.local_player);
                let result = self.inbox.accept(packet);
                if result == PacketAcceptResult::Accepted && packet.frame.0 < current_frame.0 {
                    self.session.confirm_input(
                        packet.frame,
                        packet.player_index as usize,
                        packet.input,
                        current_frame,
                    );
                }
                result
            })
            .collect()
    }

    fn advance(&mut self, frame: Frame) {
        let mut inputs = [None; 2];
        inputs[self.local_player] = Some(input_for(self.local_player, frame));
        inputs[1 - self.local_player] = self.inbox.input(frame, (1 - self.local_player) as u8);
        self.session.advance_with_prediction(frame, inputs);
    }
}

#[derive(Clone)]
struct Delivery {
    tick: u32,
    destination: usize,
    bytes: Vec<u8>,
}

fn input_for(player: usize, frame: Frame) -> PlayerInput {
    let stick = if (frame.0 + player as u32) % 4 < 2 {
        96
    } else {
        -96
    };
    PlayerInput::neutral()
        .with_left_stick(stick, 0)
        .with_attack((frame.0 + player as u32 * 2) % 7 == 3)
        .with_special((frame.0 + player as u32) % 11 == 5)
}

fn packet(player: usize, frame: Frame) -> InputPacket {
    InputPacket::new(frame, player as u8, input_for(player, frame), 0)
        .without_checksum()
        .with_timing_probe(frame.0, frame.0.saturating_sub(1))
        .with_compatibility_fingerprint(SESSION)
}

fn retransmit_datagram(player: usize, newest: Frame) -> Vec<u8> {
    let oldest = newest.0.saturating_sub(ROLLBACK_WINDOW);
    let packets = (oldest..=newest.0)
        .rev()
        .map(|frame| packet(player, Frame(frame)))
        .collect();
    InputPacketDatagram::from_packets(packets)
        .expect("newest-first retransmit bundle is valid")
        .to_wire_bytes()
}

fn schedule_send(queue: &mut Vec<Delivery>, source: usize, newest: Frame, tick: u32) {
    let bytes = retransmit_datagram(source, newest);

    // Lose every copy of player zero's frame two until the retransmit at frame nine,
    // exactly seven frames later. Other deterministic delays naturally reorder bundles.
    if source == 0 && (2..9).contains(&newest.0) {
        return;
    }
    let delay = if source == 0 && newest == Frame(9) {
        0
    } else {
        (newest.0 * 3 + source as u32 * 2) % 4
    };
    queue.push(Delivery {
        tick: tick + delay,
        destination: 1 - source,
        bytes: bytes.clone(),
    });

    // Duplicate selected wire datagrams after the original delivery.
    if (newest.0 + source as u32) % 5 == 1 {
        queue.push(Delivery {
            tick: tick + delay + 2,
            destination: 1 - source,
            bytes,
        });
    }
}

fn deliver_due(
    queue: &mut Vec<Delivery>,
    peers: &mut [Peer; 2],
    tick: u32,
    current_frame: Frame,
) -> Vec<PacketAcceptResult> {
    let mut results = Vec::new();
    let mut index = 0;
    while index < queue.len() {
        if queue[index].tick > tick {
            index += 1;
            continue;
        }
        let delivery = queue.remove(index);
        results.extend(peers[delivery.destination].receive(&delivery.bytes, current_frame));
    }
    results
}

#[test]
fn adverse_network_retransmission_converges_two_rollback_peers() {
    let mut peers = [Peer::new(0), Peer::new(1)];
    let mut queue = Vec::new();
    let mut accept_results = Vec::new();

    for frame_number in 0..FRAME_COUNT {
        let frame = Frame(frame_number);
        schedule_send(&mut queue, 0, frame, frame_number);
        schedule_send(&mut queue, 1, frame, frame_number);
        accept_results.extend(deliver_due(&mut queue, &mut peers, frame_number, frame));
        peers[0].advance(frame);
        peers[1].advance(frame);
    }

    // Keep retransmitting the final window while simulation is paused, as a real
    // transport does between render frames, then drain every bounded delivery.
    for tick in FRAME_COUNT..FRAME_COUNT + ROLLBACK_WINDOW + 4 {
        schedule_send(&mut queue, 0, Frame(FRAME_COUNT - 1), tick);
        schedule_send(&mut queue, 1, Frame(FRAME_COUNT - 1), tick);
        accept_results.extend(deliver_due(
            &mut queue,
            &mut peers,
            tick,
            Frame(FRAME_COUNT),
        ));
    }

    assert!(accept_results.contains(&PacketAcceptResult::Duplicate));
    assert_eq!(
        peers[0].session.world().checksum(),
        peers[1].session.world().checksum()
    );
    for frame_number in FRAME_COUNT - ROLLBACK_WINDOW..FRAME_COUNT {
        let frame = Frame(frame_number);
        let left = peers[0].session.historical_checksum(frame);
        let right = peers[1].session.historical_checksum(frame);
        assert!(
            left.is_some(),
            "peer zero did not finalize frame {frame_number}"
        );
        assert_eq!(left, right, "finalized frame {frame_number} diverged");
    }
}

#[test]
fn packet_beyond_rollback_horizon_is_rejected_without_fabricating_history() {
    let mut peer = Peer::new(1);
    for frame_number in 0..=10 {
        peer.advance(Frame(frame_number));
    }
    let newest = retransmit_datagram(0, Frame(10));
    peer.receive(&newest, Frame(11));
    let before = peer.session.world().checksum();
    let before_history = peer.session.historical_checksum(Frame(2));
    let stale = InputPacketDatagram::from_packets(vec![packet(0, Frame(2))])
        .unwrap()
        .to_wire_bytes();
    let results = peer.receive(&stale, Frame(11));

    assert_eq!(results, vec![PacketAcceptResult::OutsideRollbackHorizon]);
    assert_eq!(peer.session.world().checksum(), before);
    assert_eq!(before_history, None);
    assert_eq!(peer.session.historical_checksum(Frame(2)), None);
}
