use mole_signaling::{
    DirectEndpoint, FriendConnectPair, SessionDescription, SignalingMessage,
    SignalingValidationError, SupabaseRealtimeConfig, SupabaseRealtimeLimits,
    SupabaseRealtimePurpose, SupabaseUsageError,
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
        SignalingMessage::match_start("ABCD12", "peer-a"),
    ];

    for (message, expected_type) in messages.into_iter().zip([
        "room_create",
        "room_join",
        "offer",
        "answer",
        "ice_candidate",
        "direct_endpoint",
        "match_start",
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

#[test]
fn supabase_policy_allows_only_lobby_and_setup_purposes() {
    assert_eq!(
        SupabaseRealtimePurpose::Presence.validate_for_supabase(),
        Ok(())
    );
    assert_eq!(
        SupabaseRealtimePurpose::Matchmaking.validate_for_supabase(),
        Ok(())
    );
    assert_eq!(
        SupabaseRealtimePurpose::RoomCode.validate_for_supabase(),
        Ok(())
    );
    assert_eq!(
        SupabaseRealtimePurpose::SetupMessage.validate_for_supabase(),
        Ok(())
    );
    assert_eq!(
        SupabaseRealtimePurpose::GameplayInput.validate_for_supabase(),
        Err(SupabaseUsageError::GameplayInputNotAllowed)
    );
}

#[test]
fn free_plan_limits_show_naive_sixty_hertz_gameplay_exceeds_setup_budget() {
    let free_limits = SupabaseRealtimeLimits::free_plan();
    let naive_two_player_input_events =
        SupabaseRealtimeLimits::gameplay_input_events_per_second(2, 60);

    assert_eq!(free_limits.concurrent_connections, 200);
    assert_eq!(free_limits.messages_per_second, 100);
    assert_eq!(free_limits.presence_messages_per_second, 20);
    assert_eq!(naive_two_player_input_events, 120);
    assert!(!free_limits.can_fit_messages_per_second(naive_two_player_input_events));
}

#[test]
fn supabase_realtime_config_normalizes_rest_url_to_websocket_endpoint() {
    let config = SupabaseRealtimeConfig::new(
        "https://rhxagobpcbmodzgymrgo.supabase.co/rest/v1/",
        "sb_publishable_QU6ckldfsBXPBcw8bT65Qg_dFZVXSNG",
    )
    .expect("public Supabase config should be accepted");

    assert_eq!(
        config.project_url(),
        "https://rhxagobpcbmodzgymrgo.supabase.co"
    );
    assert_eq!(
        config.websocket_url(),
        "wss://rhxagobpcbmodzgymrgo.supabase.co/realtime/v1/websocket?apikey=sb_publishable_QU6ckldfsBXPBcw8bT65Qg_dFZVXSNG&vsn=1.0.0"
    );
}

#[test]
fn supabase_realtime_config_rejects_secret_keys_for_shipped_runtime() {
    let error = SupabaseRealtimeConfig::new(
        "https://rhxagobpcbmodzgymrgo.supabase.co",
        "sb_secret_mv7RV000000000000000000000000000000000000",
    )
    .expect_err("secret keys must not be accepted by friend-connect runtime config");

    assert_eq!(
        error,
        "Supabase key must be a publishable or anon public key"
    );
}

#[test]
fn direct_endpoint_broadcast_payload_carries_setup_only() {
    let payload =
        SupabaseRealtimeConfig::setup_broadcast_payload(SignalingMessage::direct_endpoint(
            "M0LE42",
            "peer-a",
            DirectEndpoint::udp("203.0.113.10:41001"),
        ))
        .expect("direct endpoint setup message should serialize");

    assert!(payload.contains("\"event\":\"setup\""));
    assert!(payload.contains("\"direct_endpoint\""));
    assert!(!payload.contains("\"checksum\""));
    assert!(!payload.contains("\"input\""));
}

#[test]
fn match_start_broadcast_payload_stays_setup_only() {
    let payload = SupabaseRealtimeConfig::setup_broadcast_payload(SignalingMessage::match_start(
        "M0LE42", "peer-a",
    ))
    .expect("match start setup message should serialize");

    assert!(payload.contains("\"event\":\"setup\""));
    assert!(payload.contains("\"match_start\""));
    assert!(!payload.contains("\"frame\""));
    assert!(!payload.contains("\"checksum\""));
    assert!(!payload.contains("\"input\""));
}

#[test]
fn repeated_match_start_frames_keep_setup_shape_and_distinct_refs() {
    let frames = SupabaseRealtimeConfig::match_start_broadcast_frames_for_room(
        "M0LE42", "M0LE42", "1", 2, 3,
    )
    .expect("repeated match start frames should serialize");

    assert_eq!(frames.len(), 3);
    assert!(frames[0].contains("\"ref\":\"2\""));
    assert!(frames[1].contains("\"ref\":\"3\""));
    assert!(frames[2].contains("\"ref\":\"4\""));
    for frame in frames {
        assert!(frame.contains("\"event\":\"setup\""));
        assert!(frame.contains("\"match_start\""));
        assert!(frame.contains("\"room_code\":\"M0LE42\""));
        assert!(!frame.contains("\"checksum\""));
        assert!(!frame.contains("\"input\""));
    }
}

#[test]
fn friend_connect_pair_derives_symmetric_room_and_player_slot() {
    let local_a = FriendConnectPair::new("M0LEA1", "M0LEB2").expect("pair should be valid");
    let local_b = FriendConnectPair::new("M0LEB2", "M0LEA1").expect("pair should be valid");

    assert_eq!(local_a.room_code(), local_b.room_code());
    assert_eq!(local_a.local_player_index(), 0);
    assert_eq!(local_b.local_player_index(), 1);
    assert_eq!(local_a.topic(), local_b.topic());
    assert!(local_a.topic().starts_with("realtime:mole-friend-connect-"));
}

#[test]
fn shared_code_lobby_setup_uses_visible_host_code_room() {
    let join = SupabaseRealtimeConfig::join_room_frame("M0LEA1", "1")
        .expect("shared host code should be a valid Realtime room");
    let broadcast = SupabaseRealtimeConfig::broadcast_frame_for_room(
        "M0LEA1",
        "1",
        "2",
        SignalingMessage::direct_endpoint(
            "M0LEA1",
            "M0LEB2",
            DirectEndpoint::udp("203.0.113.10:41001"),
        ),
    )
    .expect("direct endpoint should serialize for the shared-code lobby");

    assert!(join.contains("realtime:mole-friend-connect-M0LEA1"));
    assert!(broadcast.contains("realtime:mole-friend-connect-M0LEA1"));
    assert!(broadcast.contains("\"room_code\":\"M0LEA1\""));
    assert!(broadcast.contains("\"peer_id\":\"M0LEB2\""));
    assert!(!broadcast.contains("\"checksum\""));
    assert!(!broadcast.contains("\"input\""));
}

#[test]
fn supabase_realtime_join_and_broadcast_frames_match_official_protocol_shape() {
    let pair = FriendConnectPair::new("M0LEA1", "M0LEB2").expect("pair should be valid");
    let join = SupabaseRealtimeConfig::join_frame(&pair, "1");
    let broadcast = SupabaseRealtimeConfig::broadcast_frame(
        &pair,
        "1",
        "2",
        SignalingMessage::direct_endpoint(
            pair.room_code(),
            pair.local_peer_id(),
            DirectEndpoint::udp("203.0.113.10:41001"),
        ),
    )
    .expect("broadcast frame should serialize");

    assert!(join.contains("\"event\":\"phx_join\""));
    assert!(join.contains("\"broadcast\":{\"ack\":false,\"self\":true}"));
    assert!(join.contains("\"postgres_changes\":[]"));
    assert!(broadcast.contains("\"event\":\"broadcast\""));
    assert!(broadcast.contains("\"direct_endpoint\""));
    assert!(!broadcast.contains("\"checksum\""));
}

#[test]
fn supabase_realtime_parses_incoming_setup_broadcast_message() {
    let pair = FriendConnectPair::new("M0LEA1", "M0LEB2").expect("pair should be valid");
    let incoming = format!(
        r#"{{"topic":"{}","event":"broadcast","payload":{{"event":"setup","payload":{{"type":"direct_endpoint","room_code":"{}","peer_id":"M0LEB2","endpoint":{{"udp_addr":"203.0.113.10:41001"}}}}}},"ref":null,"join_ref":null}}"#,
        pair.topic(),
        pair.room_code()
    );

    let parsed = SupabaseRealtimeConfig::parse_setup_broadcast(&incoming)
        .expect("incoming setup broadcast should parse")
        .expect("incoming broadcast should contain a signaling message");

    assert_eq!(
        parsed,
        SignalingMessage::direct_endpoint(
            pair.room_code(),
            "M0LEB2",
            DirectEndpoint::udp("203.0.113.10:41001")
        )
    );
}
