use mole_signaling::{
    DirectEndpoint, SessionDescription, SignalingMessage, SignalingValidationError,
};

#[test]
fn signaling_messages_round_trip_through_json_with_stable_type_tags() {
    let messages = [
        SignalingMessage::create_room("peer-a"),
        SignalingMessage::join_room("ABCD12", "peer-b"),
        SignalingMessage::offer(
            "ABCD12",
            "peer-a",
            SessionDescription::new("v=0\r\no=- offer"),
        ),
        SignalingMessage::answer(
            "ABCD12",
            "peer-b",
            SessionDescription::new("v=0\r\no=- answer"),
        ),
        SignalingMessage::ice_candidate("ABCD12", "peer-a", "candidate:1 udp 2122260223"),
        SignalingMessage::direct_endpoint(
            "ABCD12",
            "peer-a",
            DirectEndpoint::udp("127.0.0.1:41001"),
        ),
    ];

    for (message, expected_type) in messages.into_iter().zip([
        "room_create",
        "room_join",
        "offer",
        "answer",
        "ice_candidate",
        "direct_endpoint",
    ]) {
        let json = message.to_json().expect("message should serialize");
        let decoded = SignalingMessage::from_json(&json).expect("message should deserialize");

        assert_eq!(decoded, message);
        assert!(json.contains(&format!("\"type\":\"{expected_type}\"")));
    }
}

#[test]
fn signaling_messages_validate_required_setup_fields() {
    let missing_room = SignalingMessage::join_room("", "peer-b");
    let missing_sdp = SignalingMessage::offer("ABCD12", "peer-a", SessionDescription::new(""));
    let invalid_endpoint =
        SignalingMessage::direct_endpoint("ABCD12", "peer-a", DirectEndpoint::udp("no-port"));

    assert_eq!(
        missing_room.validate(),
        Err(SignalingValidationError::InvalidRoomCode)
    );
    assert_eq!(
        missing_sdp.validate(),
        Err(SignalingValidationError::EmptySessionDescription)
    );
    assert_eq!(
        invalid_endpoint.validate(),
        Err(SignalingValidationError::InvalidDirectEndpoint)
    );
}

#[test]
fn signaling_messages_are_separate_from_gameplay_input_packets() {
    let message = SignalingMessage::direct_endpoint(
        "ABCD12",
        "peer-a",
        DirectEndpoint::udp("127.0.0.1:41001"),
    );

    let json = message.to_json().expect("message should serialize");

    assert!(!json.contains("\"frame\""));
    assert!(!json.contains("\"input\""));
    assert!(!json.contains("\"checksum\""));
}
