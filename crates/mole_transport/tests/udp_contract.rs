use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use mole_core::{Frame, PlayerInput};
use mole_transport::{
    parse_stun_binding_response, CompatibilityFingerprint, InputPacket, InputPacketDatagram,
    PacketDecodeError, UdpTransport, INPUT_PACKET_VERSION,
};

const FINGERPRINT: CompatibilityFingerprint = CompatibilityFingerprint::new(11, 22, 33);

fn legacy_v2_packet_bytes(
    frame: Frame,
    player: u8,
    input: PlayerInput,
    checksum: u64,
    sequence: u32,
    ack_sequence: u32,
) -> [u8; 30] {
    let mut bytes = [0u8; 30];
    bytes[0] = 2;
    bytes[1..5].copy_from_slice(&frame.0.to_be_bytes());
    bytes[5] = player;
    bytes[6..14].copy_from_slice(&input.bits().to_be_bytes());
    bytes[14..22].copy_from_slice(&checksum.to_be_bytes());
    bytes[22..26].copy_from_slice(&sequence.to_be_bytes());
    bytes[26..30].copy_from_slice(&ack_sequence.to_be_bytes());
    bytes
}

#[test]
fn input_packet_round_trips_through_wire_bytes() {
    let packet = InputPacket::new(
        Frame(42),
        1,
        PlayerInput::neutral()
            .with_left_stick(64, 0)
            .with_attack(true),
        0xfeed_beef_dead_cafe,
    )
    .with_checksum_frame(Frame(39))
    .with_compatibility_fingerprint(FINGERPRINT);

    let bytes = packet.to_wire_bytes();
    let decoded = InputPacket::from_wire_bytes(&bytes).expect("packet should decode");

    assert_eq!(decoded, packet);
    assert_eq!(decoded.checksum_frame, Frame(39));
    assert_ne!(decoded.checksum_frame, decoded.frame);
    assert_eq!(decoded.compatibility, FINGERPRINT);
}

#[test]
fn current_packet_rejects_unknown_fingerprint_version() {
    let packet = InputPacket::new(Frame(42), 1, PlayerInput::neutral(), 7)
        .with_compatibility_fingerprint(FINGERPRINT);
    let mut bytes = packet.to_wire_bytes();
    bytes[6] = 99;

    assert_eq!(
        InputPacket::from_wire_bytes(&bytes),
        Err(PacketDecodeError::UnsupportedFingerprintVersion(99))
    );
}

#[test]
fn legacy_v2_packet_decodes_but_retains_legacy_identity() {
    let mut bytes = [0u8; 30];
    bytes[0] = 2;
    bytes[1..5].copy_from_slice(&42u32.to_be_bytes());
    bytes[5] = 1;
    bytes[6..14].copy_from_slice(&PlayerInput::neutral().bits().to_be_bytes());
    bytes[14..22].copy_from_slice(&7u64.to_be_bytes());
    bytes[22..26].copy_from_slice(&42u32.to_be_bytes());
    bytes[26..30].copy_from_slice(&40u32.to_be_bytes());

    let decoded = InputPacket::from_wire_bytes(&bytes).expect("legacy v2 should remain decodable");

    assert_eq!(decoded.version, 2);
    assert_ne!(decoded.version, INPUT_PACKET_VERSION);
    assert_eq!(decoded.checksum_frame, Frame(42));
    assert_eq!(decoded.compatibility, CompatibilityFingerprint::legacy());
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
fn input_packet_datagram_decodes_legacy_v3_bundle_despite_current_packet_v3() {
    let first = legacy_v2_packet_bytes(Frame(8), 1, PlayerInput::neutral(), 0x808, 8, 7);
    let second = legacy_v2_packet_bytes(Frame(7), 1, PlayerInput::neutral(), 0x707, 7, 6);
    let mut bytes = vec![3, 2];
    bytes.extend_from_slice(&first[1..]);
    bytes.extend_from_slice(&second[1..]);

    let decoded = InputPacketDatagram::from_wire_bytes(&bytes).unwrap();

    assert_eq!(decoded.packets().len(), 2);
    assert_eq!(decoded.packets()[0].frame, Frame(8));
    assert_eq!(decoded.packets()[1].frame, Frame(7));
    assert_eq!(
        decoded.packets()[0].compatibility,
        CompatibilityFingerprint::legacy()
    );
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
