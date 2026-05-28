#![cfg(feature = "webrtc")]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;

use mole_core::{Frame, PlayerInput};
use mole_signaling::{SessionDescription, SignalingMessage};
use mole_transport::{DataChannelPort, InputPacket, Transport, WebRtcDataChannelTransport};

#[test]
fn webrtc_data_channel_transport_delivers_input_packets() {
    let (channel_a, channel_b) = MemoryDataChannel::pair();
    let mut peer_a = WebRtcDataChannelTransport::new(channel_a);
    let mut peer_b = WebRtcDataChannelTransport::new(channel_b);
    let packet = InputPacket::new(
        Frame(12),
        0,
        PlayerInput::neutral().with_left_stick(64, 0),
        0xfeed,
    )
    .with_timing_probe(12, 8);

    peer_a.send(packet);

    assert_eq!(peer_b.try_recv(), Some(packet));
}

#[test]
fn webrtc_data_channel_transport_uses_same_packet_ordering_contract() {
    let (channel_a, channel_b) = MemoryDataChannel::pair();
    let mut peer_a = WebRtcDataChannelTransport::new(channel_a);
    let mut peer_b = WebRtcDataChannelTransport::new(channel_b);
    let first = InputPacket::new(Frame(1), 0, PlayerInput::neutral(), 11);
    let second = InputPacket::new(Frame(2), 0, PlayerInput::neutral().with_special(true), 22);

    peer_a.send(first);
    peer_a.send(second);

    assert_eq!(peer_b.try_recv(), Some(first));
    assert_eq!(peer_b.try_recv(), Some(second));
    assert_eq!(peer_b.try_recv(), None);
}

#[test]
fn webrtc_setup_data_flows_through_signaling_not_gameplay_packets() {
    let offer = SignalingMessage::offer(
        "ABCD12",
        "peer-a",
        SessionDescription::new("v=0\r\no=- webrtc offer"),
    );

    let json = offer.to_json().expect("offer should serialize");

    assert!(json.contains("\"type\":\"offer\""));
    assert!(!json.contains("\"frame\""));
    assert!(!json.contains("\"checksum\""));
    assert!(!json.contains("\"input\""));
}

#[derive(Debug, Default)]
struct MemoryDataChannel {
    incoming: Rc<RefCell<VecDeque<Vec<u8>>>>,
    outgoing: Rc<RefCell<VecDeque<Vec<u8>>>>,
}

impl MemoryDataChannel {
    fn pair() -> (Self, Self) {
        let a_to_b = Rc::new(RefCell::new(VecDeque::new()));
        let b_to_a = Rc::new(RefCell::new(VecDeque::new()));
        (
            Self {
                incoming: Rc::clone(&b_to_a),
                outgoing: Rc::clone(&a_to_b),
            },
            Self {
                incoming: a_to_b,
                outgoing: b_to_a,
            },
        )
    }
}

impl DataChannelPort for MemoryDataChannel {
    fn send_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.outgoing.borrow_mut().push_back(bytes.to_vec());
        Ok(())
    }

    fn try_recv_bytes(&mut self) -> io::Result<Option<Vec<u8>>> {
        Ok(self.incoming.borrow_mut().pop_front())
    }
}
