use std::net::{SocketAddr, UdpSocket};

use mole_core::{Frame, PlayerInput};
use mole_transport::{InputPacket, PacketDecodeError, UdpTransport};

#[test]
fn input_packet_round_trips_through_wire_bytes() {
    let packet = InputPacket::new(
        Frame(42),
        1,
        PlayerInput::neutral()
            .with_left_stick(64, 0)
            .with_attack(true),
        0xfeed_beef_dead_cafe,
    );

    let bytes = packet.to_wire_bytes();
    let decoded = InputPacket::from_wire_bytes(&bytes).expect("packet should decode");

    assert_eq!(decoded, packet);
}

#[test]
fn input_packet_round_trips_timing_probe_fields_through_wire_bytes() {
    let packet =
        InputPacket::new(Frame(42), 1, PlayerInput::neutral(), 0xfeed).with_timing_probe(123, 97);

    let bytes = packet.to_wire_bytes();
    let decoded = InputPacket::from_wire_bytes(&bytes).expect("packet should decode");

    assert_eq!(decoded.sequence, 123);
    assert_eq!(decoded.ack_sequence, 97);
    assert_eq!(decoded, packet);
}

#[test]
fn input_packet_rejects_wrong_wire_version() {
    let packet = InputPacket::new(Frame(1), 0, PlayerInput::neutral(), 0);
    let mut bytes = packet.to_wire_bytes();
    bytes[0] = 99;

    assert_eq!(
        InputPacket::from_wire_bytes(&bytes),
        Err(PacketDecodeError::UnsupportedVersion(99))
    );
}

#[test]
fn udp_transport_returns_none_when_empty_without_blocking() {
    let peer_addr = reserve_local_addr();
    let transport =
        UdpTransport::bind("127.0.0.1:0".parse().unwrap(), peer_addr).expect("bind should work");

    assert_eq!(transport.try_recv_packet().unwrap(), None);
}

#[test]
fn udp_transport_treats_absent_peer_as_no_packet() {
    let peer_addr = reserve_local_addr();
    let transport =
        UdpTransport::bind("127.0.0.1:0".parse().unwrap(), peer_addr).expect("bind should work");
    let packet = InputPacket::new(Frame(1), 0, PlayerInput::neutral(), 0xfeed);

    transport.send_packet(packet).expect("send should work");

    for _ in 0..20 {
        assert_eq!(
            transport
                .try_recv_packet()
                .expect("absent UDP peer should not be fatal"),
            None
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn udp_transport_exchanges_input_packets_between_local_sockets() {
    let socket_a = reserve_local_addr();
    let socket_b = reserve_local_addr();
    let a = UdpTransport::bind(socket_a, socket_b).expect("bind a should work");
    let b = UdpTransport::bind(socket_b, socket_a).expect("bind b should work");
    let packet = InputPacket::new(
        Frame(8),
        0,
        PlayerInput::neutral().with_special(true),
        0x1234,
    );

    a.send_packet(packet).expect("send should work");

    let received = poll_udp_packet(&b).expect("packet should arrive");

    assert_eq!(received, packet);
}

fn reserve_local_addr() -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("reserve socket");
    socket.local_addr().expect("local addr")
}

fn poll_udp_packet(transport: &UdpTransport) -> Option<InputPacket> {
    for _ in 0..20 {
        if let Some(packet) = transport.try_recv_packet().expect("recv should work") {
            return Some(packet);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    None
}
