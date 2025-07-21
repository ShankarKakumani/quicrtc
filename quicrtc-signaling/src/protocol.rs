//! Signaling protocol messages

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// MoQ session offer for establishing peer connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoqSessionOffer {
    /// Participant ID making the offer
    pub participant_id: String,
    /// QUIC connection parameters
    pub quic_endpoint: SocketAddr,
    /// Supported MoQ version
    pub moq_version: String,
    /// Track namespaces this participant can publish
    pub publish_namespaces: Vec<String>,
    /// Track namespaces this participant wants to subscribe to
    pub subscribe_namespaces: Vec<String>,
    /// Additional capabilities
    pub capabilities: Vec<String>,
    /// Session ID for correlation
    pub session_id: String,
}

/// MoQ session answer responding to an offer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoqSessionAnswer {
    /// Participant ID responding
    pub participant_id: String,
    /// QUIC connection parameters
    pub quic_endpoint: SocketAddr,
    /// Accepted MoQ version
    pub moq_version: String,
    /// Track namespaces accepted for publishing
    pub accepted_publish_namespaces: Vec<String>,
    /// Track namespaces accepted for subscription
    pub accepted_subscribe_namespaces: Vec<String>,
    /// Session ID from the offer
    pub session_id: String,
    /// Whether the session is accepted
    pub accepted: bool,
}

/// Signaling protocol messages for MoQ session negotiation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalingMessage {
    /// Join room request
    JoinRoom {
        /// Room ID
        room_id: String,
        /// Participant ID
        participant_id: String,
        /// Optional participant display name
        participant_name: Option<String>,
        /// MoQ capabilities
        capabilities: Vec<String>,
        /// QUIC endpoint for direct connections
        quic_endpoint: Option<SocketAddr>,
    },
    /// Leave room request
    LeaveRoom {
        /// Room ID
        room_id: String,
        /// Participant ID
        participant_id: String,
    },
    /// Create room request
    CreateRoom {
        /// Room ID
        room_id: String,
        /// Optional room display name
        room_name: Option<String>,
        /// Maximum participants allowed
        max_participants: Option<usize>,
    },
    /// MoQ session offer to establish direct peer connection
    MoqSessionOffer {
        /// Room ID where participants are
        room_id: String,
        /// Target participant to connect with
        target_participant: String,
        /// MoQ session offer details
        offer: MoqSessionOffer,
    },
    /// MoQ session answer in response to offer
    MoqSessionAnswer {
        /// Room ID where participants are
        room_id: String,
        /// Target participant (who made the offer)
        target_participant: String,
        /// MoQ session answer details
        answer: MoqSessionAnswer,
    },
    /// List all available rooms
    ListRooms,
    /// Get detailed information about a specific room
    GetRoomInfo {
        /// Room ID to get info for
        room_id: String,
    },
}

/// Server response messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalingResponse {
    /// Successfully joined room
    JoinedRoom {
        /// Room ID
        room_id: String,
        /// Participant ID
        participant_id: String,
    },
    /// Successfully left room
    LeftRoom {
        /// Room ID
        room_id: String,
        /// Participant ID
        participant_id: String,
    },
    /// Room created successfully
    RoomCreated {
        /// Room ID
        room_id: String,
    },
    /// Participant joined notification
    ParticipantJoined {
        /// Room ID
        room_id: String,
        /// New participant information
        participant: crate::server::Participant,
    },
    /// Participant left notification
    ParticipantLeft {
        /// Room ID
        room_id: String,
        /// Participant ID that left
        participant_id: String,
    },
    /// MoQ session offer forwarded from another participant
    MoqSessionOffer {
        /// Room ID
        room_id: String,
        /// Participant making the offer
        source_participant: String,
        /// MoQ session offer details
        offer: MoqSessionOffer,
    },
    /// MoQ session answer forwarded from another participant
    MoqSessionAnswer {
        /// Room ID
        room_id: String,
        /// Participant responding
        source_participant: String,
        /// MoQ session answer details
        answer: MoqSessionAnswer,
    },
    /// List of available rooms
    RoomList {
        /// List of (room_id, room_name, participant_count)
        rooms: Vec<(String, Option<String>, usize)>,
    },
    /// Detailed room information
    RoomInfo {
        /// Room ID
        room_id: String,
        /// Room display name
        room_name: Option<String>,
        /// List of participants in the room
        participants: Vec<crate::server::Participant>,
        /// Room creation timestamp
        created_at: chrono::DateTime<chrono::Utc>,
        /// Maximum participants allowed
        max_participants: usize,
    },
    /// Error response
    Error {
        /// Error message
        error: String,
        /// Error code for programmatic handling
        error_code: String,
    },
}
