use mole_core::{Frame, PlayerInput};
use mole_transport::{
    InputPacket, InputPacketInbox, LoopbackTransport, PacketAcceptResult, Transport,
    TransportTimingComparison, TransportTimingProfile, TransportTimingVerdict,
    INPUT_PACKET_VERSION,
};

#[test]
fn loopback_transport_delivers_input_packets() {
    let mut transport = LoopbackTransport::default();
    let packet = InputPacket::new(Frame(9), 0, PlayerInput::neutral().with_attack(true), 0xabc);

    transport.send(packet);

    assert_eq!(transport.try_recv(), Some(packet));
}

#[test]
fn loopback_transport_returns_none_when_empty() {
    let mut transport = LoopbackTransport::default();

    assert_eq!(transport.try_recv(), None);
}

#[test]
fn input_packets_are_versioned_and_carry_checksums() {
    let packet = InputPacket::new(
        Frame(12),
        1,
        PlayerInput::neutral().with_left_stick(-64, 0),
        0xfeed_beef,
    );

    assert_eq!(packet.version, INPUT_PACKET_VERSION);
    assert_eq!(packet.frame, Frame(12));
    assert_eq!(packet.player_index, 1);
    assert_eq!(packet.input.stick_x(), -64);
    assert_eq!(packet.checksum, 0xfeed_beef);
}

#[test]
fn input_packets_carry_timing_probes_separate_from_gameplay_input() {
    let packet = InputPacket::new(Frame(12), 1, PlayerInput::neutral(), 0xfeed_beef)
        .with_timing_probe(99, 88);

    assert_eq!(packet.sequence, 99);
    assert_eq!(packet.ack_sequence, 88);
    assert_eq!(packet.input, PlayerInput::neutral());
    assert_eq!(packet.checksum, 0xfeed_beef);
}

#[test]
fn loopback_transport_preserves_packet_order() {
    let mut transport = LoopbackTransport::default();
    let first = InputPacket::new(Frame(1), 0, PlayerInput::neutral(), 11);
    let second = InputPacket::new(Frame(2), 0, PlayerInput::neutral().with_special(true), 22);

    transport.send(first);
    transport.send(second);

    assert_eq!(transport.try_recv(), Some(first));
    assert_eq!(transport.try_recv(), Some(second));
    assert_eq!(transport.try_recv(), None);
}

#[test]
fn packet_inbox_rejects_duplicate_input_frames() {
    let mut inbox = InputPacketInbox::default();
    let first = InputPacket::new(Frame(7), 1, PlayerInput::neutral().with_attack(true), 111);
    let duplicate = InputPacket::new(Frame(7), 1, PlayerInput::neutral().with_special(true), 222);

    assert_eq!(inbox.accept(first), PacketAcceptResult::Accepted);
    assert_eq!(inbox.accept(duplicate), PacketAcceptResult::Duplicate);
    assert_eq!(inbox.input(Frame(7), 1), Some(first.input));
}

#[test]
fn packet_inbox_exposes_dropped_frames_as_missing() {
    let mut inbox = InputPacketInbox::default();

    inbox.accept(InputPacket::new(Frame(4), 0, PlayerInput::neutral(), 44));

    assert_eq!(inbox.input(Frame(4), 0), Some(PlayerInput::neutral()));
    assert_eq!(inbox.input(Frame(5), 0), None);
}

#[test]
fn transport_timing_summary_reports_latency_and_jitter_in_milliframes() {
    let profile = TransportTimingProfile::measured("direct_udp", [2, 4, 3]);
    let summary = profile
        .summary
        .expect("measured profile should expose a timing summary");

    assert_eq!(summary.sample_count, 3);
    assert_eq!(summary.min_rtt_frames, 2);
    assert_eq!(summary.max_rtt_frames, 4);
    assert_eq!(summary.average_rtt_milliframes, 3_000);
    assert_eq!(summary.average_jitter_milliframes, 1_500);
}

#[test]
fn transport_timing_comparison_keeps_udp_baseline_when_candidate_is_unmeasured() {
    let direct_udp = TransportTimingProfile::measured("direct_udp", [2, 3, 2, 3]);
    let webrtc = TransportTimingProfile::unmeasured("webrtc_datachannel");

    let comparison = TransportTimingComparison::new(direct_udp, webrtc);

    assert_eq!(
        comparison.verdict(),
        TransportTimingVerdict::CandidateUnmeasured
    );
    assert_eq!(comparison.preferred_transport_name(), "direct_udp");
}

#[test]
fn transport_timing_comparison_uses_jitter_after_latency_tie() {
    let direct_udp = TransportTimingProfile::measured("direct_udp", [3, 3, 3, 3]);
    let webrtc = TransportTimingProfile::measured("webrtc_datachannel", [2, 4, 2, 4]);

    let comparison = TransportTimingComparison::new(direct_udp, webrtc);

    assert_eq!(
        comparison.verdict(),
        TransportTimingVerdict::BaselinePreferred
    );
    assert_eq!(comparison.preferred_transport_name(), "direct_udp");
}
