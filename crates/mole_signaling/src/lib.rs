use std::{net::SocketAddr, sync::Once, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tungstenite::Message;

static RUSTLS_CRYPTO_PROVIDER: Once = Once::new();
pub const FRIEND_CONNECT_DIRECTORY_ROOM: &str = "M0LELOBBY";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingMessage {
    #[serde(rename = "room_create")]
    CreateRoom {
        peer_id: String,
    },
    #[serde(rename = "room_join")]
    JoinRoom {
        room_code: String,
        peer_id: String,
    },
    Offer {
        room_code: String,
        peer_id: String,
        description: SessionDescription,
    },
    Answer {
        room_code: String,
        peer_id: String,
        description: SessionDescription,
    },
    IceCandidate {
        room_code: String,
        peer_id: String,
        candidate: String,
    },
    DirectEndpoint {
        room_code: String,
        peer_id: String,
        endpoint: DirectEndpoint,
    },
    MatchStart {
        room_code: String,
        peer_id: String,
    },
    LobbyAdvertise {
        room_code: String,
        peer_id: String,
    },
}

impl SignalingMessage {
    pub fn create_room(peer_id: impl Into<String>) -> Self {
        Self::CreateRoom {
            peer_id: peer_id.into(),
        }
    }

    pub fn join_room(room_code: impl Into<String>, peer_id: impl Into<String>) -> Self {
        Self::JoinRoom {
            room_code: room_code.into(),
            peer_id: peer_id.into(),
        }
    }

    pub fn offer(
        room_code: impl Into<String>,
        peer_id: impl Into<String>,
        description: SessionDescription,
    ) -> Self {
        Self::Offer {
            room_code: room_code.into(),
            peer_id: peer_id.into(),
            description,
        }
    }

    pub fn answer(
        room_code: impl Into<String>,
        peer_id: impl Into<String>,
        description: SessionDescription,
    ) -> Self {
        Self::Answer {
            room_code: room_code.into(),
            peer_id: peer_id.into(),
            description,
        }
    }

    pub fn ice_candidate(
        room_code: impl Into<String>,
        peer_id: impl Into<String>,
        candidate: impl Into<String>,
    ) -> Self {
        Self::IceCandidate {
            room_code: room_code.into(),
            peer_id: peer_id.into(),
            candidate: candidate.into(),
        }
    }

    pub fn direct_endpoint(
        room_code: impl Into<String>,
        peer_id: impl Into<String>,
        endpoint: DirectEndpoint,
    ) -> Self {
        Self::DirectEndpoint {
            room_code: room_code.into(),
            peer_id: peer_id.into(),
            endpoint,
        }
    }

    pub fn match_start(room_code: impl Into<String>, peer_id: impl Into<String>) -> Self {
        Self::MatchStart {
            room_code: room_code.into(),
            peer_id: peer_id.into(),
        }
    }

    pub fn lobby_advertise(room_code: impl Into<String>, peer_id: impl Into<String>) -> Self {
        Self::LobbyAdvertise {
            room_code: room_code.into(),
            peer_id: peer_id.into(),
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    pub fn validate(&self) -> Result<(), SignalingValidationError> {
        let peer_id = match self {
            Self::CreateRoom { peer_id } => peer_id,
            Self::JoinRoom { room_code, peer_id }
            | Self::Offer {
                room_code, peer_id, ..
            }
            | Self::Answer {
                room_code, peer_id, ..
            }
            | Self::IceCandidate {
                room_code, peer_id, ..
            }
            | Self::DirectEndpoint {
                room_code, peer_id, ..
            }
            | Self::MatchStart {
                room_code, peer_id, ..
            }
            | Self::LobbyAdvertise {
                room_code, peer_id, ..
            } => {
                if !is_valid_room_code(room_code) {
                    return Err(SignalingValidationError::InvalidRoomCode);
                }
                peer_id
            }
        };

        if peer_id.trim().is_empty() {
            return Err(SignalingValidationError::EmptyPeerId);
        }

        match self {
            Self::Offer { description, .. } | Self::Answer { description, .. } => {
                if description.sdp.trim().is_empty() {
                    return Err(SignalingValidationError::EmptySessionDescription);
                }
            }
            Self::IceCandidate { candidate, .. } => {
                if candidate.trim().is_empty() {
                    return Err(SignalingValidationError::EmptyIceCandidate);
                }
            }
            Self::DirectEndpoint { endpoint, .. } => {
                if endpoint.udp_addr.parse::<SocketAddr>().is_err() {
                    return Err(SignalingValidationError::InvalidDirectEndpoint);
                }
            }
            Self::CreateRoom { .. } | Self::JoinRoom { .. } => {}
            Self::MatchStart { .. } | Self::LobbyAdvertise { .. } => {}
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescription {
    pub sdp: String,
}

impl SessionDescription {
    pub fn new(sdp: impl Into<String>) -> Self {
        Self { sdp: sdp.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectEndpoint {
    pub udp_addr: String,
}

impl DirectEndpoint {
    pub fn udp(udp_addr: impl Into<String>) -> Self {
        Self {
            udp_addr: udp_addr.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDirectEndpoint {
    pub peer_id: String,
    pub endpoint: DirectEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingValidationError {
    EmptyPeerId,
    InvalidRoomCode,
    EmptySessionDescription,
    EmptyIceCandidate,
    InvalidDirectEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupabaseRealtimePurpose {
    Presence,
    Matchmaking,
    RoomCode,
    SetupMessage,
    GameplayInput,
}

impl SupabaseRealtimePurpose {
    pub const fn validate_for_supabase(self) -> Result<(), SupabaseUsageError> {
        match self {
            Self::Presence | Self::Matchmaking | Self::RoomCode | Self::SetupMessage => Ok(()),
            Self::GameplayInput => Err(SupabaseUsageError::GameplayInputNotAllowed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupabaseUsageError {
    GameplayInputNotAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupabaseRealtimeLimits {
    pub concurrent_connections: u32,
    pub messages_per_second: u32,
    pub presence_messages_per_second: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupabaseRealtimeConfig {
    project_url: String,
    publishable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendConnectPair {
    local_peer_id: String,
    remote_peer_id: String,
    room_code: String,
}

impl FriendConnectPair {
    pub fn new(
        local_peer_id: impl AsRef<str>,
        remote_peer_id: impl AsRef<str>,
    ) -> Result<Self, String> {
        let local_peer_id = normalize_friend_peer_id(local_peer_id.as_ref())?;
        let remote_peer_id = normalize_friend_peer_id(remote_peer_id.as_ref())?;
        if local_peer_id == remote_peer_id {
            return Err("local and remote friend-connect codes must differ".to_string());
        }

        let room_code = derive_friend_room_code(&local_peer_id, &remote_peer_id);
        Ok(Self {
            local_peer_id,
            remote_peer_id,
            room_code,
        })
    }

    pub fn local_peer_id(&self) -> &str {
        &self.local_peer_id
    }

    pub fn remote_peer_id(&self) -> &str {
        &self.remote_peer_id
    }

    pub fn room_code(&self) -> &str {
        &self.room_code
    }

    pub fn topic(&self) -> String {
        SupabaseRealtimeConfig::room_topic(&self.room_code)
    }

    pub fn local_player_index(&self) -> u8 {
        if self.local_peer_id < self.remote_peer_id {
            0
        } else {
            1
        }
    }
}

impl SupabaseRealtimeConfig {
    pub fn new(
        project_url: impl AsRef<str>,
        publishable_key: impl AsRef<str>,
    ) -> Result<Self, String> {
        let project_url = normalize_supabase_project_url(project_url.as_ref())?;
        let publishable_key = publishable_key.as_ref().trim().to_string();
        if !is_public_supabase_key(&publishable_key) {
            return Err("Supabase key must be a publishable or anon public key".to_string());
        }

        Ok(Self {
            project_url,
            publishable_key,
        })
    }

    pub fn project_url(&self) -> &str {
        &self.project_url
    }

    pub fn publishable_key(&self) -> &str {
        &self.publishable_key
    }

    pub fn websocket_url(&self) -> String {
        format!(
            "{}/realtime/v1/websocket?apikey={}&vsn=1.0.0",
            self.project_url.replacen("https://", "wss://", 1),
            self.publishable_key
        )
    }

    pub fn room_topic(room_code: &str) -> String {
        format!("realtime:mole-friend-connect-{room_code}")
    }

    pub fn join_frame(pair: &FriendConnectPair, ref_id: &str) -> String {
        Self::join_topic_frame(&pair.topic(), ref_id)
    }

    pub fn join_room_frame(room_code: &str, ref_id: &str) -> Result<String, String> {
        let room_code = normalize_friend_peer_id(room_code)?;
        Ok(Self::join_topic_frame(
            &Self::room_topic(&room_code),
            ref_id,
        ))
    }

    pub fn lobby_directory_join_frame(ref_id: &str) -> String {
        Self::join_topic_frame(&Self::room_topic(FRIEND_CONNECT_DIRECTORY_ROOM), ref_id)
    }

    fn join_topic_frame(topic: &str, ref_id: &str) -> String {
        serde_json::to_string(&json!({
            "topic": topic,
            "event": "phx_join",
            "payload": {
                "config": {
                    "broadcast": {
                        "ack": false,
                        "self": true
                    },
                    "presence": {
                        "enabled": false
                    },
                    "postgres_changes": [],
                    "private": false
                }
            },
            "ref": ref_id,
            "join_ref": ref_id
        }))
        .expect("static Realtime join payload should serialize")
    }

    pub fn broadcast_frame(
        pair: &FriendConnectPair,
        join_ref: &str,
        ref_id: &str,
        message: SignalingMessage,
    ) -> Result<String, String> {
        Self::broadcast_frame_for_room(pair.room_code(), join_ref, ref_id, message)
    }

    pub fn broadcast_frame_for_room(
        room_code: &str,
        join_ref: &str,
        ref_id: &str,
        message: SignalingMessage,
    ) -> Result<String, String> {
        let room_code = normalize_friend_peer_id(room_code)?;
        message.validate().map_err(|error| format!("{error:?}"))?;
        serde_json::to_string(&json!({
            "topic": Self::room_topic(&room_code),
            "event": "broadcast",
            "payload": {
                "event": "setup",
                "payload": message
            },
            "ref": ref_id,
            "join_ref": join_ref
        }))
        .map_err(|error| error.to_string())
    }

    pub fn lobby_advertise_frame(
        room_code: &str,
        local_peer_id: &str,
        join_ref: &str,
        ref_id: &str,
    ) -> Result<String, String> {
        let room_code = normalize_friend_peer_id(room_code)?;
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        Self::broadcast_frame_for_room(
            FRIEND_CONNECT_DIRECTORY_ROOM,
            join_ref,
            ref_id,
            SignalingMessage::lobby_advertise(room_code, local_peer_id),
        )
    }

    pub fn setup_broadcast_payload(message: SignalingMessage) -> Result<String, String> {
        message.validate().map_err(|error| format!("{error:?}"))?;
        serde_json::to_string(&json!({
            "event": "setup",
            "payload": message,
        }))
        .map_err(|error| error.to_string())
    }

    pub fn parse_setup_broadcast(json_text: &str) -> Result<Option<SignalingMessage>, String> {
        let value: serde_json::Value =
            serde_json::from_str(json_text).map_err(|error| error.to_string())?;
        if value.get("event").and_then(serde_json::Value::as_str) != Some("broadcast") {
            return Ok(None);
        }
        let Some(payload) = value.get("payload") else {
            return Ok(None);
        };
        if payload.get("event").and_then(serde_json::Value::as_str) != Some("setup") {
            return Ok(None);
        }
        let Some(message_value) = payload.get("payload") else {
            return Ok(None);
        };
        let message: SignalingMessage =
            serde_json::from_value(message_value.clone()).map_err(|error| error.to_string())?;
        message.validate().map_err(|error| format!("{error:?}"))?;
        Ok(Some(message))
    }

    pub fn exchange_direct_endpoint(
        &self,
        pair: &FriendConnectPair,
        local_endpoint: DirectEndpoint,
    ) -> Result<DirectEndpoint, String> {
        ensure_rustls_crypto_provider();
        let (mut socket, _response) =
            tungstenite::connect(self.websocket_url()).map_err(|error| error.to_string())?;
        let join_ref = "1";
        socket
            .send(Message::Text(Self::join_frame(pair, join_ref).into()))
            .map_err(|error| error.to_string())?;
        let setup = SignalingMessage::direct_endpoint(
            pair.room_code(),
            pair.local_peer_id(),
            local_endpoint,
        );
        socket
            .send(Message::Text(
                Self::broadcast_frame(pair, join_ref, "2", setup)?.into(),
            ))
            .map_err(|error| error.to_string())?;

        loop {
            let message = socket.read().map_err(|error| error.to_string())?;
            let Message::Text(text) = message else {
                continue;
            };
            let Some(SignalingMessage::DirectEndpoint {
                peer_id, endpoint, ..
            }) = Self::parse_setup_broadcast(text.as_str())?
            else {
                continue;
            };
            if peer_id == pair.local_peer_id() {
                continue;
            }
            if peer_id != pair.remote_peer_id() {
                continue;
            }
            return Ok(endpoint);
        }
    }

    pub fn host_direct_endpoint(
        &self,
        room_code: &str,
        local_peer_id: &str,
        local_endpoint: DirectEndpoint,
    ) -> Result<RemoteDirectEndpoint, String> {
        ensure_rustls_crypto_provider();
        let room_code = normalize_friend_peer_id(room_code)?;
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        let (mut socket, _response) =
            tungstenite::connect(self.websocket_url()).map_err(|error| error.to_string())?;
        let join_ref = "1";
        socket
            .send(Message::Text(
                Self::join_room_frame(&room_code, join_ref)?.into(),
            ))
            .map_err(|error| error.to_string())?;

        loop {
            let message = socket.read().map_err(|error| error.to_string())?;
            let Message::Text(text) = message else {
                continue;
            };
            let Some(SignalingMessage::DirectEndpoint {
                room_code: message_room,
                peer_id,
                endpoint,
            }) = Self::parse_setup_broadcast(text.as_str())?
            else {
                continue;
            };
            if message_room != room_code {
                continue;
            }
            if peer_id == local_peer_id {
                continue;
            }
            let response =
                SignalingMessage::direct_endpoint(&room_code, &local_peer_id, local_endpoint);
            socket
                .send(Message::Text(
                    Self::broadcast_frame_for_room(&room_code, join_ref, "2", response)?.into(),
                ))
                .map_err(|error| error.to_string())?;
            return Ok(RemoteDirectEndpoint { peer_id, endpoint });
        }
    }

    pub fn advertise_lobby_repeated(
        &self,
        room_code: &str,
        local_peer_id: &str,
        repeat_count: usize,
        repeat_interval: Duration,
    ) -> Result<(), String> {
        self.advertise_lobby_repeated_until(
            room_code,
            local_peer_id,
            repeat_count,
            repeat_interval,
            || false,
        )
    }

    pub fn advertise_lobby_repeated_until(
        &self,
        room_code: &str,
        local_peer_id: &str,
        repeat_count: usize,
        repeat_interval: Duration,
        mut should_stop: impl FnMut() -> bool,
    ) -> Result<(), String> {
        ensure_rustls_crypto_provider();
        let room_code = normalize_friend_peer_id(room_code)?;
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        let (mut socket, _response) =
            tungstenite::connect(self.websocket_url()).map_err(|error| error.to_string())?;
        let join_ref = "1";
        socket
            .send(Message::Text(
                Self::lobby_directory_join_frame(join_ref).into(),
            ))
            .map_err(|error| error.to_string())?;
        let count = repeat_count.max(1);
        for index in 0..count {
            if should_stop() {
                return Ok(());
            }
            if index > 0 && !repeat_interval.is_zero() {
                std::thread::sleep(repeat_interval);
                if should_stop() {
                    return Ok(());
                }
            }
            let ref_id = (index + 2).to_string();
            socket
                .send(Message::Text(
                    Self::lobby_advertise_frame(&room_code, &local_peer_id, join_ref, &ref_id)?
                        .into(),
                ))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn stream_lobby_directory(
        &self,
        local_peer_id: &str,
        sender: std::sync::mpsc::Sender<Result<String, String>>,
    ) -> Result<(), String> {
        ensure_rustls_crypto_provider();
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        let (mut socket, _response) =
            tungstenite::connect(self.websocket_url()).map_err(|error| error.to_string())?;
        let join_ref = "1";
        socket
            .send(Message::Text(
                Self::lobby_directory_join_frame(join_ref).into(),
            ))
            .map_err(|error| error.to_string())?;

        loop {
            let message = socket.read().map_err(|error| error.to_string())?;
            let Message::Text(text) = message else {
                continue;
            };
            let Some(SignalingMessage::LobbyAdvertise {
                room_code, peer_id, ..
            }) = Self::parse_setup_broadcast(text.as_str())?
            else {
                continue;
            };
            if peer_id == local_peer_id || room_code == local_peer_id {
                continue;
            }
            if sender.send(Ok(room_code)).is_err() {
                return Ok(());
            }
        }
    }

    pub fn join_direct_endpoint(
        &self,
        room_code: &str,
        local_peer_id: &str,
        local_endpoint: DirectEndpoint,
    ) -> Result<RemoteDirectEndpoint, String> {
        ensure_rustls_crypto_provider();
        let room_code = normalize_friend_peer_id(room_code)?;
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        let (mut socket, _response) =
            tungstenite::connect(self.websocket_url()).map_err(|error| error.to_string())?;
        let join_ref = "1";
        socket
            .send(Message::Text(
                Self::join_room_frame(&room_code, join_ref)?.into(),
            ))
            .map_err(|error| error.to_string())?;
        let join = SignalingMessage::join_room(&room_code, &local_peer_id);
        socket
            .send(Message::Text(
                Self::broadcast_frame_for_room(&room_code, join_ref, "2", join)?.into(),
            ))
            .map_err(|error| error.to_string())?;
        let setup = SignalingMessage::direct_endpoint(&room_code, &local_peer_id, local_endpoint);
        socket
            .send(Message::Text(
                Self::broadcast_frame_for_room(&room_code, join_ref, "3", setup)?.into(),
            ))
            .map_err(|error| error.to_string())?;

        loop {
            let message = socket.read().map_err(|error| error.to_string())?;
            let Message::Text(text) = message else {
                continue;
            };
            let Some(SignalingMessage::DirectEndpoint {
                room_code: message_room,
                peer_id,
                endpoint,
            }) = Self::parse_setup_broadcast(text.as_str())?
            else {
                continue;
            };
            if message_room != room_code {
                continue;
            }
            if peer_id != room_code {
                continue;
            }
            return Ok(RemoteDirectEndpoint { peer_id, endpoint });
        }
    }

    pub fn broadcast_match_start(&self, pair: &FriendConnectPair) -> Result<(), String> {
        self.broadcast_match_start_in_room(pair.room_code(), pair.local_peer_id())
    }

    pub fn broadcast_match_start_in_room(
        &self,
        room_code: &str,
        local_peer_id: &str,
    ) -> Result<(), String> {
        self.broadcast_match_start_repeated_in_room(room_code, local_peer_id, 1, Duration::ZERO)
    }

    pub fn broadcast_match_start_repeated_in_room(
        &self,
        room_code: &str,
        local_peer_id: &str,
        repeat_count: usize,
        repeat_interval: Duration,
    ) -> Result<(), String> {
        ensure_rustls_crypto_provider();
        let room_code = normalize_friend_peer_id(room_code)?;
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        let (mut socket, _response) =
            tungstenite::connect(self.websocket_url()).map_err(|error| error.to_string())?;
        let join_ref = "1";
        socket
            .send(Message::Text(
                Self::join_room_frame(&room_code, join_ref)?.into(),
            ))
            .map_err(|error| error.to_string())?;
        let frames = Self::match_start_broadcast_frames_for_room(
            &room_code,
            &local_peer_id,
            join_ref,
            2,
            repeat_count,
        )?;
        for (index, frame) in frames.into_iter().enumerate() {
            if index > 0 && !repeat_interval.is_zero() {
                std::thread::sleep(repeat_interval);
            }
            socket
                .send(Message::Text(frame.into()))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn match_start_broadcast_frames_for_room(
        room_code: &str,
        local_peer_id: &str,
        join_ref: &str,
        first_ref: u32,
        repeat_count: usize,
    ) -> Result<Vec<String>, String> {
        let room_code = normalize_friend_peer_id(room_code)?;
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        let count = repeat_count.max(1);
        let start = SignalingMessage::match_start(&room_code, &local_peer_id);
        (0..count)
            .map(|index| {
                let ref_id = first_ref.saturating_add(index as u32).to_string();
                Self::broadcast_frame_for_room(&room_code, join_ref, &ref_id, start.clone())
            })
            .collect()
    }

    pub fn wait_for_match_start(&self, pair: &FriendConnectPair) -> Result<String, String> {
        self.wait_for_match_start_in_room(
            pair.room_code(),
            pair.local_peer_id(),
            pair.remote_peer_id(),
        )
    }

    pub fn wait_for_match_start_in_room(
        &self,
        room_code: &str,
        local_peer_id: &str,
        remote_peer_id: &str,
    ) -> Result<String, String> {
        ensure_rustls_crypto_provider();
        let room_code = normalize_friend_peer_id(room_code)?;
        let local_peer_id = normalize_friend_peer_id(local_peer_id)?;
        let remote_peer_id = normalize_friend_peer_id(remote_peer_id)?;
        let (mut socket, _response) =
            tungstenite::connect(self.websocket_url()).map_err(|error| error.to_string())?;
        let join_ref = "1";
        socket
            .send(Message::Text(
                Self::join_room_frame(&room_code, join_ref)?.into(),
            ))
            .map_err(|error| error.to_string())?;

        loop {
            let message = socket.read().map_err(|error| error.to_string())?;
            let Message::Text(text) = message else {
                continue;
            };
            let Some(SignalingMessage::MatchStart {
                room_code: message_room,
                peer_id,
            }) = Self::parse_setup_broadcast(text.as_str())?
            else {
                continue;
            };
            if message_room != room_code {
                continue;
            }
            if peer_id == local_peer_id {
                continue;
            }
            if peer_id != remote_peer_id {
                continue;
            }
            return Ok(peer_id);
        }
    }
}

fn ensure_rustls_crypto_provider() {
    RUSTLS_CRYPTO_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

impl SupabaseRealtimeLimits {
    pub const fn free_plan() -> Self {
        Self {
            concurrent_connections: 200,
            messages_per_second: 100,
            presence_messages_per_second: 20,
        }
    }

    pub const fn can_fit_messages_per_second(self, events_per_second: u32) -> bool {
        events_per_second <= self.messages_per_second
    }

    pub const fn gameplay_input_events_per_second(players: u32, simulation_hz: u32) -> u32 {
        players.saturating_mul(simulation_hz)
    }
}

fn is_valid_room_code(room_code: &str) -> bool {
    (4..=12).contains(&room_code.len())
        && room_code
            .chars()
            .all(|character| character.is_ascii_digit() || character.is_ascii_uppercase())
}

fn normalize_supabase_project_url(project_url: &str) -> Result<String, String> {
    let mut url = project_url.trim().trim_end_matches('/').to_string();
    if url.ends_with("/rest/v1") {
        url.truncate(url.len() - "/rest/v1".len());
    }
    if !url.starts_with("https://") || !url.ends_with(".supabase.co") {
        return Err(
            "Supabase project URL must look like https://<project-ref>.supabase.co".to_string(),
        );
    }
    Ok(url)
}

fn is_public_supabase_key(key: &str) -> bool {
    key.starts_with("sb_publishable_") || key.starts_with("eyJ")
}

fn normalize_friend_peer_id(peer_id: &str) -> Result<String, String> {
    let peer_id = peer_id.trim().to_ascii_uppercase();
    if !is_valid_room_code(&peer_id) {
        return Err("friend-connect code must be 4-12 uppercase letters or digits".to_string());
    }
    Ok(peer_id)
}

fn derive_friend_room_code(local_peer_id: &str, remote_peer_id: &str) -> String {
    let (left, right) = if local_peer_id < remote_peer_id {
        (local_peer_id, remote_peer_id)
    } else {
        (remote_peer_id, local_peer_id)
    };
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in left.bytes().chain([b':']).chain(right.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("M{:011X}", hash & 0x0000_0fff_ffff_ffff)
}
