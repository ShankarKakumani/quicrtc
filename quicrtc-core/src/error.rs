//! Error types for QUIC RTC

use std::time::Duration;
use thiserror::Error;

/// Main error type for QUIC RTC operations
#[derive(Error, Debug)]
pub enum QuicRtcError {
    /// Initialization error
    #[error("Initialization failed: {reason}")]
    Initialization {
        /// Reason for initialization failure
        reason: String,
    },
    
    /// Missing configuration error
    #[error("Missing required configuration: {field}")]
    MissingConfiguration {
        /// Missing configuration field
        field: String,
    },
    
    /// Connection error
    #[error("Connection failed for room {room_id}: {reason}")]
    Connection {
        /// Room ID where connection failed
        room_id: String,
        /// Reason for connection failure
        reason: String,
        /// Suggested retry delay
        retry_in: Option<Duration>,
        /// Suggested action to resolve the issue
        suggested_action: String,
    },
    
    /// Transport error
    #[error("Transport error: {reason}")]
    Transport {
        /// Reason for transport error
        reason: String,
    },
    
    /// MoQ protocol error
    #[error("MoQ protocol error: {reason}")]
    MoqProtocol {
        /// Reason for protocol error
        reason: String,
    },
    
    /// Media processing error
    #[error("Media processing error: {reason}")]
    MediaProcessing {
        /// Reason for media error
        reason: String,
    },
    
    /// Resource limit exceeded
    #[error("Resource limit exceeded: {resource}")]
    ResourceLimit {
        /// Resource that exceeded limit
        resource: String,
    },
    
    /// Invalid state error
    #[error("Invalid state: expected {expected}, got {actual}")]
    InvalidState {
        /// Expected state
        expected: String,
        /// Actual state
        actual: String,
    },
    
    /// Protocol error
    #[error("Protocol error: {message}")]
    ProtocolError {
        /// Error message
        message: String,
    },
    
    /// Session setup failed
    #[error("Session setup failed (code {code}): {reason}")]
    SessionSetupFailed {
        /// Error code
        code: u32,
        /// Error reason
        reason: String,
    },
    
    /// Unsupported version
    #[error("Unsupported version: {version}")]
    UnsupportedVersion {
        /// Unsupported version number
        version: u32,
    },
    
    /// Unsupported track type
    #[error("Unsupported track type: {track_type}")]
    UnsupportedTrackType {
        /// Unsupported track type
        track_type: String,
    },
    
    /// Track limit exceeded
    #[error("Track limit exceeded: {limit}")]
    TrackLimitExceeded {
        /// Maximum number of tracks allowed
        limit: u32,
    },
    
    /// Track announce failed
    #[error("Track announce failed for {track_namespace} (code {code}): {reason}")]
    TrackAnnounceFailed {
        /// Track namespace
        track_namespace: String,
        /// Error code
        code: u32,
        /// Error reason
        reason: String,
    },
    
    /// Subscription failed
    #[error("Subscription failed for {track_namespace} (code {code}): {reason}")]
    SubscriptionFailed {
        /// Track namespace
        track_namespace: String,
        /// Error code
        code: u32,
        /// Error reason
        reason: String,
    },
    
    /// Cache full error
    #[error("Cache full: current size {current_size} bytes exceeds maximum {max_size} bytes")]
    CacheFull {
        /// Current cache size in bytes
        current_size: usize,
        /// Maximum cache size in bytes
        max_size: usize,
    },
    
    /// Track cache full error
    #[error("Track cache full for {track_name}: current objects {current_objects} exceeds maximum {max_objects}")]
    TrackCacheFull {
        /// Track name
        track_name: String,
        /// Current number of objects
        current_objects: usize,
        /// Maximum number of objects
        max_objects: usize,
    },
    
    /// Track not found error
    #[error("Track not found: {track_namespace}")]
    TrackNotFound {
        /// Track namespace
        track_namespace: String,
    },
    
    /// Stream not found error
    #[error("Stream not found: {stream_id}")]
    StreamNotFound {
        /// Stream ID
        stream_id: u64,
    },
    
    /// No data available error
    #[error("No data available")]
    NoDataAvailable,
    
    /// Invalid operation error
    #[error("Invalid operation: {operation}")]
    InvalidOperation {
        /// Operation that was invalid
        operation: String,
    },

    /// Resource exhausted error
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted {
        /// Resource that was exhausted
        resource: String,
    },

    /// Operation timed out error
    #[error("Operation timed out: {operation} after {duration:?}")]
    Timeout {
        /// Operation that timed out
        operation: String,
        /// Duration after which timeout occurred
        duration: std::time::Duration,
    },
    
    /// Invalid data error
    #[error("Invalid data: {reason}")]
    InvalidData {
        /// Reason for invalid data
        reason: String,
    },
    
    /// Invalid media type error
    #[error("Invalid media type: expected {expected}, got {actual}")]
    InvalidMediaType {
        /// Expected media type
        expected: String,
        /// Actual media type
        actual: String,
    },
    
    /// Unsupported codec error
    #[error("Unsupported codec: {codec}")]
    UnsupportedCodec {
        /// Codec name
        codec: String,
    },

    /// Encoding operation failed
    #[error("Encoding failed: {reason}")]
    EncodingFailed {
        /// Reason for failure
        reason: String,
    },

    /// Decoding operation failed  
    #[error("Decoding failed: {reason}")]
    DecodingFailed {
        /// Reason for failure
        reason: String,
    },

    /// Server start failed
    #[error("Failed to start server on {address}: {source}")]
    ServerStartFailed {
        /// Address that failed to bind
        address: std::net::SocketAddr,
        /// Underlying error
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Room not found
    #[error("Room not found: {room_id}")]
    RoomNotFound {
        /// Room ID that was not found
        room_id: String,
    },

    /// Room already exists
    #[error("Room already exists: {room_id}")]
    RoomAlreadyExists {
        /// Room ID that already exists
        room_id: String,
    },

    /// Room full
    #[error("Room {room_id} is full (max participants: {max_participants})")]
    RoomFull {
        /// Room ID that is full
        room_id: String,
        /// Maximum participants allowed
        max_participants: usize,
    },

    /// Participant already exists
    #[error("Participant {participant_id} already exists in room {room_id}")]
    ParticipantAlreadyExists {
        /// Room ID
        room_id: String,
        /// Participant ID that already exists
        participant_id: String,
    },

    /// Participant not found
    #[error("Participant {participant_id} not found in room {room_id}")]
    ParticipantNotFound {
        /// Room ID
        room_id: String,
        /// Participant ID that was not found
        participant_id: String,
    },

    /// Invalid message format
    #[error("Invalid message format: {message}, error: {source}")]
    InvalidMessage {
        /// Invalid message content
        message: String,
        /// Parsing error
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl QuicRtcError {
    /// Get error code for programmatic handling
    pub fn error_code(&self) -> String {
        match self {
            QuicRtcError::Initialization { .. } => "INITIALIZATION_FAILED".to_string(),
            QuicRtcError::MissingConfiguration { .. } => "MISSING_CONFIGURATION".to_string(),
            QuicRtcError::Connection { .. } => "CONNECTION_FAILED".to_string(),
            QuicRtcError::Transport { .. } => "TRANSPORT_ERROR".to_string(),
            QuicRtcError::MoqProtocol { .. } => "MOQ_PROTOCOL_ERROR".to_string(),
            QuicRtcError::MediaProcessing { .. } => "MEDIA_PROCESSING_ERROR".to_string(),
            QuicRtcError::ResourceLimit { .. } => "RESOURCE_LIMIT_EXCEEDED".to_string(),
            QuicRtcError::InvalidState { .. } => "INVALID_STATE".to_string(),
            QuicRtcError::ProtocolError { .. } => "PROTOCOL_ERROR".to_string(),
            QuicRtcError::SessionSetupFailed { .. } => "SESSION_SETUP_FAILED".to_string(),
            QuicRtcError::UnsupportedVersion { .. } => "UNSUPPORTED_VERSION".to_string(),
            QuicRtcError::UnsupportedTrackType { .. } => "UNSUPPORTED_TRACK_TYPE".to_string(),
            QuicRtcError::TrackLimitExceeded { .. } => "TRACK_LIMIT_EXCEEDED".to_string(),
            QuicRtcError::TrackAnnounceFailed { .. } => "TRACK_ANNOUNCE_FAILED".to_string(),
            QuicRtcError::SubscriptionFailed { .. } => "SUBSCRIPTION_FAILED".to_string(),
            QuicRtcError::CacheFull { .. } => "CACHE_FULL".to_string(),
            QuicRtcError::TrackCacheFull { .. } => "TRACK_CACHE_FULL".to_string(),
            QuicRtcError::TrackNotFound { .. } => "TRACK_NOT_FOUND".to_string(),
            QuicRtcError::StreamNotFound { .. } => "STREAM_NOT_FOUND".to_string(),
            QuicRtcError::NoDataAvailable => "NO_DATA_AVAILABLE".to_string(),
            QuicRtcError::InvalidData { .. } => "INVALID_DATA".to_string(),
            QuicRtcError::InvalidMediaType { .. } => "INVALID_MEDIA_TYPE".to_string(),
            QuicRtcError::UnsupportedCodec { .. } => "UNSUPPORTED_CODEC".to_string(),
            QuicRtcError::InvalidOperation { .. } => "INVALID_OPERATION".to_string(),
            QuicRtcError::ResourceExhausted { .. } => "RESOURCE_EXHAUSTED".to_string(),
            QuicRtcError::Timeout { .. } => "TIMEOUT".to_string(),
        QuicRtcError::EncodingFailed { .. } => "ENCODING_FAILED".to_string(),
        QuicRtcError::DecodingFailed { .. } => "DECODING_FAILED".to_string(),
            QuicRtcError::ServerStartFailed { .. } => "SERVER_START_FAILED".to_string(),
            QuicRtcError::RoomNotFound { .. } => "ROOM_NOT_FOUND".to_string(),
            QuicRtcError::RoomAlreadyExists { .. } => "ROOM_ALREADY_EXISTS".to_string(),
            QuicRtcError::RoomFull { .. } => "ROOM_FULL".to_string(),
            QuicRtcError::ParticipantAlreadyExists { .. } => "PARTICIPANT_ALREADY_EXISTS".to_string(),
            QuicRtcError::ParticipantNotFound { .. } => "PARTICIPANT_NOT_FOUND".to_string(),
            QuicRtcError::InvalidMessage { .. } => "INVALID_MESSAGE".to_string(),
        }
    }
}