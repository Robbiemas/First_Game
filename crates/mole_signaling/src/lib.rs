use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingMessage {
    #[serde(rename = "room_create")]
    CreateRoom { peer_id: String },
    #[serde(rename = "room_join")]
    JoinRoom { room_code: String, peer_id: String },
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
