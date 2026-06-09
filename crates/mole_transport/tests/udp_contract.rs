use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use mole_core::{Frame, PlayerInput};
use mole_transport::{
    parse_stun_binding_response, InputPacket, InputPacketDatagram, PacketDecodeError, UdpTransport,
};

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
fn input_packet_datagram_round_trips_bundled_recent_inputs() {
    let newest = InputPacket::new(
        Frame(10),
        0,
        PlayerInput::neutral().with_left_stick(127, 0),
        0xfeed_beef,
    )
    .with_timing_probe(10, 8);
    let older = InputPacket::new(
        Frame(9),
        0,
        PlayerInput::neutral().with_attack(true),
        0xfeed_beee,
    )
    .with_timing_probe(9, 8);
    let oldest =
        InputPacket::new(Frame(8), 0, PlayerInput::neutral(), 0xfeed_beed).with_timing_probe(8, 8);

    let bytes = InputPacketDatagram::from_packets(vec![newest, older, oldest])
        .expect("bundle should accept recent inputs")
        .to_wire_bytes();
    let decoded =
        InputPacketDatagram::from_wire_bytes(&bytes).expect("bundle datagram should decode");

    assert_eq!(decoded.packets(), &[newest, older, oldest]);
}

#[test]
fn input_packet_datagram_decodes_legacy_single_packet() {
    let packet =
        InputPacket::new(Frame(42), 1, PlayerInput::neutral(), 0xfeed).with_timing_probe(42, 40);
    let decoded = InputPacketDatagram::from_wire_bytes(&packet.to_wire_bytes())
        .expect("legacy packet should decode as a datagram");

    assert_eq!(decoded.packets(), &[packet]);
}

#[test]
fn input_packet_datagram_rejects_non_contiguous_or_mixed_player_bundles() {
    let newest = InputPacket::new(Frame(10), 0, PlayerInput::neutral(), 0);
    let skipped = InputPacket::new(Frame(8), 0, PlayerInput::neutral(), 0);
    let other_player = InputPacket::new(Frame(9), 1, PlayerInput::neutral(), 0);

    assert_eq!(
        InputPacketDatagram::from_packets(vec![newest, skipped]),
        Err(PacketDecodeError::NonContiguousDatagram)
    );
    assert_eq!(
        InputPacketDatagram::from_packets(vec![newest, other_player]),
        Err(PacketDecodeError::MixedPlayerDatagram)
    );
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

#[test]
fn udp_transport_exchanges_bundled_recent_input_packets_between_local_sockets() {
    let socket_a = reserve_local_addr();
    let socket_b = reserve_local_addr();
    let a = UdpTransport::bind(socket_a, socket_b).expect("bind a should work");
    let b = UdpTransport::bind(socket_b, socket_a).expect("bind b should work");
    let packets = vec![
        InputPacket::new(
            Frame(12),
            0,
            PlayerInput::neutral().with_special(true),
            0x1234,
        )
        .with_timing_probe(12, 10),
        InputPacket::new(
            Frame(11),
            0,
            PlayerInput::neutral().with_attack(true),
            0x1233,
        )
        .with_timing_probe(11, 10),
    ];

    a.send_packet_datagram(&InputPacketDatagram::from_packets(packets.clone()).unwrap())
        .expect("send should work");

    let received = poll_udp_datagram(&b).expect("datagram should arrive");

    assert_eq!(received.packets(), packets.as_slice());
}

#[test]
fn stun_parser_reads_xor_mapped_ipv4_endpoint() {
    let transaction_id = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let mut response = Vec::new();
    response.extend_from_slice(&0x0101u16.to_be_bytes());
    response.extend_from_slice(&12u16.to_be_bytes());
    response.extend_from_slice(&0x2112_A442u32.to_be_bytes());
    response.extend_from_slice(&transaction_id);
    response.extend_from_slice(&0x0020u16.to_be_bytes());
    response.extend_from_slice(&8u16.to_be_bytes());
    response.push(0);
    response.push(0x01);
    response.extend_from_slice(&(41001u16 ^ 0x2112u16).to_be_bytes());
    response.extend_from_slice(
        &(u32::from(Ipv4Addr::new(203, 0, 113, 10)) ^ 0x2112_A442).to_be_bytes(),
    );

    let endpoint =
        parse_stun_binding_response(&response, transaction_id).expect("STUN response should parse");

    assert_eq!(
        endpoint,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10)), 41001)
    );
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

fn poll_udp_datagram(transport: &UdpTransport) -> Option<InputPacketDatagram> {
    for _ in 0..20 {
        if let Some(packet) = transport
            .try_recv_packet_datagram()
            .expect("recv should work")
        {
            return Some(packet);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    None
}
