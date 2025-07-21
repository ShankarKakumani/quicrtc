//! IETF Media over QUIC (MoQ) protocol implementation

use crate::error::QuicRtcError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

pub mod wire_format;
pub mod stream_manager;

pub use wire_format::MoqWireFormat;
pub use stream_manager::{
    MoqStreamManager, MoqStreamEvent, MoqStreamState, MoqStreamType,
    StreamManagerConfig, ManagedMoqStream, StreamStats, StreamId, TrackAlias
};

/// MoQ session management with track management and subscription handling
#[derive(Debug)]
pub struct MoqSession {
    session_id: u64,
    state: MoqSessionState,
    announced_tracks: HashMap<TrackNamespace, MoqTrack>,
    subscriptions: HashMap<TrackNamespace, MoqSubscription>,
    capabilities: MoqCapabilities,
    peer_capabilities: Option<MoqCapabilities>,
    control_sender: mpsc::UnboundedSender<MoqControlMessage>,
    control_receiver: mpsc::UnboundedReceiver<MoqControlMessage>,
}

/// MoQ session state
#[derive(Debug, Clone, PartialEq)]
pub enum MoqSessionState {
    /// Session is being established
    Establishing,
    /// Session is active and ready for track operations
    Active,
    /// Session is being terminated
    Terminating,
    /// Session has been terminated
    Terminated,
}

/// MoQ session capabilities
#[derive(Debug, Clone)]
pub struct MoqCapabilities {
    /// Supported MoQ version
    pub version: u32,
    /// Maximum number of concurrent tracks
    pub max_tracks: u32,
    /// Supported track types
    pub supported_track_types: Vec<MoqTrackType>,
    /// Maximum object size in bytes
    pub max_object_size: u64,
    /// Support for object caching
    pub supports_caching: bool,
}

impl Default for MoqCapabilities {
    fn default() -> Self {
        Self {
            version: 1,
            max_tracks: 100,
            supported_track_types: vec![
                MoqTrackType::Audio,
                MoqTrackType::Video,
                MoqTrackType::Data,
            ],
            max_object_size: 1024 * 1024, // 1MB
            supports_caching: true,
        }
    }
}

/// MoQ subscription information
#[derive(Debug, Clone)]
pub struct MoqSubscription {
    /// Track namespace being subscribed to
    pub track_namespace: TrackNamespace,
    /// Subscription state
    pub state: MoqSubscriptionState,
    /// Subscription priority (lower number = higher priority)
    pub priority: u8,
    /// Start group ID (None for live subscription)
    pub start_group: Option<u64>,
    /// End group ID (None for ongoing subscription)
    pub end_group: Option<u64>,
}

/// MoQ subscription state
#[derive(Debug, Clone, PartialEq)]
pub enum MoqSubscriptionState {
    /// Subscription request sent, waiting for response
    Pending,
    /// Subscription is active
    Active,
    /// Subscription has been rejected
    Rejected(String),
    /// Subscription has been terminated
    Terminated,
}

/// MoQ control messages for session management
#[derive(Debug, Clone)]
pub enum MoqControlMessage {
    /// Session setup message
    Setup {
        /// MoQ protocol version
        version: u32,
        /// Session capabilities
        capabilities: MoqCapabilities,
    },
    /// Session setup response
    SetupOk {
        /// Agreed MoQ protocol version
        version: u32,
        /// Peer capabilities
        capabilities: MoqCapabilities,
    },
    /// Session setup error
    SetupError {
        /// Error code
        code: u32,
        /// Error reason
        reason: String,
    },
    /// Announce a track
    Announce {
        /// Track namespace
        track_namespace: TrackNamespace,
        /// Track information
        track: MoqTrack,
    },
    /// Announce response
    AnnounceOk {
        /// Track namespace
        track_namespace: TrackNamespace,
    },
    /// Announce error
    AnnounceError {
        /// Track namespace
        track_namespace: TrackNamespace,
        /// Error code
        code: u32,
        /// Error reason
        reason: String,
    },
    /// Subscribe to a track
    Subscribe {
        /// Track namespace
        track_namespace: TrackNamespace,
        /// Subscription priority
        priority: u8,
        /// Start group ID (None for live)
        start_group: Option<u64>,
        /// End group ID (None for ongoing)
        end_group: Option<u64>,
    },
    /// Subscribe response
    SubscribeOk {
        /// Track namespace
        track_namespace: TrackNamespace,
    },
    /// Subscribe error
    SubscribeError {
        /// Track namespace
        track_namespace: TrackNamespace,
        /// Error code
        code: u32,
        /// Error reason
        reason: String,
    },
    /// Unsubscribe from a track
    Unsubscribe {
        /// Track namespace
        track_namespace: TrackNamespace,
    },
    /// Session termination
    Terminate {
        /// Termination code
        code: u32,
        /// Termination reason
        reason: String,
    },
}

impl MoqObject {
    /// Create MoQ object from H.264 frame (no RTP packetization)
    pub fn from_h264_frame(track_namespace: TrackNamespace, frame: H264Frame) -> Self {
        let size = frame.nal_units.len();
        Self {
            track_namespace,
            track_name: "video".to_string(),
            group_id: frame.timestamp_us / 1000, // Convert to milliseconds for grouping
            object_id: frame.sequence_number,
            publisher_priority: if frame.is_keyframe { 1 } else { 2 }, // Keyframes have higher priority
            payload: frame.nal_units,
            object_status: MoqObjectStatus::Normal,
            created_at: std::time::Instant::now(),
            size,
        }
    }

    /// Create MoQ object from Opus audio frame (no RTP packetization)
    pub fn from_opus_frame(track_namespace: TrackNamespace, frame: OpusFrame) -> Self {
        let size = frame.opus_data.len();
        Self {
            track_namespace,
            track_name: "audio".to_string(),
            group_id: frame.timestamp_us / 20000, // 20ms audio groups
            object_id: frame.sequence_number,
            publisher_priority: 1, // Audio always high priority
            payload: frame.opus_data,
            object_status: MoqObjectStatus::Normal,
            created_at: std::time::Instant::now(),
            size,
        }
    }

    /// Create end-of-group marker object
    pub fn end_of_group(
        track_namespace: TrackNamespace,
        track_name: String,
        group_id: u64,
        object_id: u64,
    ) -> Self {
        Self {
            track_namespace,
            track_name,
            group_id,
            object_id,
            publisher_priority: 1, // End markers have high priority
            payload: Vec::new(),
            object_status: MoqObjectStatus::EndOfGroup,
            created_at: std::time::Instant::now(),
            size: 0,
        }
    }

    /// Create end-of-track marker object
    pub fn end_of_track(
        track_namespace: TrackNamespace,
        track_name: String,
        group_id: u64,
        object_id: u64,
    ) -> Self {
        Self {
            track_namespace,
            track_name,
            group_id,
            object_id,
            publisher_priority: 1, // End markers have high priority
            payload: Vec::new(),
            object_status: MoqObjectStatus::EndOfTrack,
            created_at: std::time::Instant::now(),
            size: 0,
        }
    }

    /// Get object priority for delivery ordering
    pub fn delivery_priority(&self) -> u8 {
        match self.object_status {
            MoqObjectStatus::EndOfTrack => 0, // Highest priority
            MoqObjectStatus::EndOfGroup => 1,
            MoqObjectStatus::Normal => self.publisher_priority,
        }
    }

    /// Check if object is a control/marker object
    pub fn is_control_object(&self) -> bool {
        matches!(
            self.object_status,
            MoqObjectStatus::EndOfGroup | MoqObjectStatus::EndOfTrack
        )
    }

    /// Get object age since creation
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

impl MoqSession {
    /// Create new MoQ session with default capabilities
    pub fn new(session_id: u64) -> Self {
        let (control_sender, control_receiver) = mpsc::unbounded_channel();

        Self {
            session_id,
            state: MoqSessionState::Establishing,
            announced_tracks: HashMap::new(),
            subscriptions: HashMap::new(),
            capabilities: MoqCapabilities::default(),
            peer_capabilities: None,
            control_sender,
            control_receiver,
        }
    }

    /// Create new MoQ session with custom capabilities
    pub fn new_with_capabilities(session_id: u64, capabilities: MoqCapabilities) -> Self {
        let (control_sender, control_receiver) = mpsc::unbounded_channel();

        Self {
            session_id,
            state: MoqSessionState::Establishing,
            announced_tracks: HashMap::new(),
            subscriptions: HashMap::new(),
            capabilities,
            peer_capabilities: None,
            control_sender,
            control_receiver,
        }
    }

    /// Get session ID
    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Get current session state
    pub fn state(&self) -> &MoqSessionState {
        &self.state
    }

    /// Get session capabilities
    pub fn capabilities(&self) -> &MoqCapabilities {
        &self.capabilities
    }

    /// Get peer capabilities (if session is established)
    pub fn peer_capabilities(&self) -> Option<&MoqCapabilities> {
        self.peer_capabilities.as_ref()
    }

    /// Establish MoQ session with capability exchange
    pub async fn establish_session(&mut self) -> Result<(), QuicRtcError> {
        if self.state != MoqSessionState::Establishing {
            return Err(QuicRtcError::InvalidState {
                expected: "Establishing".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        // Send setup message with our capabilities
        let setup_msg = MoqControlMessage::Setup {
            version: self.capabilities.version,
            capabilities: self.capabilities.clone(),
        };

        self.send_control_message(setup_msg).await?;

        // Wait for setup response
        match self.receive_control_message().await? {
            MoqControlMessage::SetupOk {
                version,
                capabilities,
            } => {
                if version != self.capabilities.version {
                    return Err(QuicRtcError::ProtocolError {
                        message: format!(
                            "Version mismatch: expected {}, got {}",
                            self.capabilities.version, version
                        ),
                    });
                }

                self.peer_capabilities = Some(capabilities);
                self.state = MoqSessionState::Active;
                Ok(())
            }
            MoqControlMessage::SetupError { code, reason } => {
                self.state = MoqSessionState::Terminated;
                Err(QuicRtcError::SessionSetupFailed { code, reason })
            }
            _ => {
                self.state = MoqSessionState::Terminated;
                Err(QuicRtcError::ProtocolError {
                    message: "Unexpected message during session setup".to_string(),
                })
            }
        }
    }

    /// Handle incoming session setup request
    pub async fn handle_setup_request(
        &mut self,
        version: u32,
        peer_capabilities: MoqCapabilities,
    ) -> Result<(), QuicRtcError> {
        if self.state != MoqSessionState::Establishing {
            return Err(QuicRtcError::InvalidState {
                expected: "Establishing".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        // Check version compatibility
        if version != self.capabilities.version {
            let error_msg = MoqControlMessage::SetupError {
                code: 1,
                reason: format!("Unsupported version: {}", version),
            };
            self.send_control_message(error_msg).await?;
            self.state = MoqSessionState::Terminated;
            return Err(QuicRtcError::UnsupportedVersion { version });
        }

        // Store peer capabilities and send response
        self.peer_capabilities = Some(peer_capabilities);

        let response = MoqControlMessage::SetupOk {
            version: self.capabilities.version,
            capabilities: self.capabilities.clone(),
        };

        self.send_control_message(response).await?;
        self.state = MoqSessionState::Active;

        Ok(())
    }

    /// Announce a track for publishing
    pub async fn announce_track(&mut self, track: MoqTrack) -> Result<(), QuicRtcError> {
        if self.state != MoqSessionState::Active {
            return Err(QuicRtcError::InvalidState {
                expected: "Active".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        // Check if track type is supported by peer
        if let Some(peer_caps) = &self.peer_capabilities {
            if !peer_caps.supported_track_types.contains(&track.track_type) {
                return Err(QuicRtcError::UnsupportedTrackType {
                    track_type: format!("{:?}", track.track_type),
                });
            }
        }

        // Check track limits
        if let Some(peer_caps) = &self.peer_capabilities {
            if self.announced_tracks.len() >= peer_caps.max_tracks as usize {
                return Err(QuicRtcError::TrackLimitExceeded {
                    limit: peer_caps.max_tracks,
                });
            }
        }

        let announce_msg = MoqControlMessage::Announce {
            track_namespace: track.namespace.clone(),
            track: track.clone(),
        };

        self.send_control_message(announce_msg).await?;

        // Wait for announce response
        match self.receive_control_message().await? {
            MoqControlMessage::AnnounceOk { track_namespace } => {
                if track_namespace == track.namespace {
                    self.announced_tracks.insert(track.namespace.clone(), track);
                    Ok(())
                } else {
                    Err(QuicRtcError::ProtocolError {
                        message: "Track namespace mismatch in announce response".to_string(),
                    })
                }
            }
            MoqControlMessage::AnnounceError {
                track_namespace,
                code,
                reason,
            } => {
                if track_namespace == track.namespace {
                    Err(QuicRtcError::TrackAnnounceFailed {
                        track_namespace: track_namespace.track_name,
                        code,
                        reason,
                    })
                } else {
                    Err(QuicRtcError::ProtocolError {
                        message: "Track namespace mismatch in announce error".to_string(),
                    })
                }
            }
            _ => Err(QuicRtcError::ProtocolError {
                message: "Unexpected message during track announce".to_string(),
            }),
        }
    }

    /// Handle incoming track announcement
    pub async fn handle_track_announcement(
        &mut self,
        track_namespace: TrackNamespace,
        track: MoqTrack,
    ) -> Result<(), QuicRtcError> {
        if self.state != MoqSessionState::Active {
            return Err(QuicRtcError::InvalidState {
                expected: "Active".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        // Check if we support this track type
        if !self
            .capabilities
            .supported_track_types
            .contains(&track.track_type)
        {
            let error_msg = MoqControlMessage::AnnounceError {
                track_namespace: track_namespace.clone(),
                code: 2,
                reason: format!("Unsupported track type: {:?}", track.track_type),
            };
            self.send_control_message(error_msg).await?;
            return Ok(());
        }

        // Check track limits
        if self.announced_tracks.len() >= self.capabilities.max_tracks as usize {
            let error_msg = MoqControlMessage::AnnounceError {
                track_namespace: track_namespace.clone(),
                code: 3,
                reason: "Track limit exceeded".to_string(),
            };
            self.send_control_message(error_msg).await?;
            return Ok(());
        }

        // Accept the track announcement
        let response = MoqControlMessage::AnnounceOk {
            track_namespace: track_namespace.clone(),
        };

        self.send_control_message(response).await?;

        // Store the announced track (from peer)
        // Note: In a real implementation, we might want to separate our tracks from peer tracks
        self.announced_tracks.insert(track_namespace, track);

        Ok(())
    }

    /// Subscribe to a track
    pub async fn subscribe_to_track(
        &mut self,
        track_namespace: TrackNamespace,
        priority: u8,
        start_group: Option<u64>,
        end_group: Option<u64>,
    ) -> Result<MoqSubscription, QuicRtcError> {
        if self.state != MoqSessionState::Active {
            return Err(QuicRtcError::InvalidState {
                expected: "Active".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        // Create subscription
        let subscription = MoqSubscription {
            track_namespace: track_namespace.clone(),
            state: MoqSubscriptionState::Pending,
            priority,
            start_group,
            end_group,
        };

        // Send subscribe message
        let subscribe_msg = MoqControlMessage::Subscribe {
            track_namespace: track_namespace.clone(),
            priority,
            start_group,
            end_group,
        };

        self.send_control_message(subscribe_msg).await?;

        // Wait for subscribe response
        match self.receive_control_message().await? {
            MoqControlMessage::SubscribeOk {
                track_namespace: resp_namespace,
            } => {
                if resp_namespace == track_namespace {
                    let mut active_subscription = subscription;
                    active_subscription.state = MoqSubscriptionState::Active;
                    self.subscriptions
                        .insert(track_namespace, active_subscription.clone());
                    Ok(active_subscription)
                } else {
                    Err(QuicRtcError::ProtocolError {
                        message: "Track namespace mismatch in subscribe response".to_string(),
                    })
                }
            }
            MoqControlMessage::SubscribeError {
                track_namespace: resp_namespace,
                code,
                reason,
            } => {
                if resp_namespace == track_namespace {
                    Err(QuicRtcError::SubscriptionFailed {
                        track_namespace: track_namespace.track_name,
                        code,
                        reason,
                    })
                } else {
                    Err(QuicRtcError::ProtocolError {
                        message: "Track namespace mismatch in subscribe error".to_string(),
                    })
                }
            }
            _ => Err(QuicRtcError::ProtocolError {
                message: "Unexpected message during track subscribe".to_string(),
            }),
        }
    }

    /// Handle incoming subscription request
    pub async fn handle_subscription_request(
        &mut self,
        track_namespace: TrackNamespace,
        priority: u8,
        start_group: Option<u64>,
        end_group: Option<u64>,
    ) -> Result<(), QuicRtcError> {
        if self.state != MoqSessionState::Active {
            return Err(QuicRtcError::InvalidState {
                expected: "Active".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        // Check if we have announced this track
        if !self.announced_tracks.contains_key(&track_namespace) {
            let error_msg = MoqControlMessage::SubscribeError {
                track_namespace: track_namespace.clone(),
                code: 4,
                reason: "Track not found".to_string(),
            };
            self.send_control_message(error_msg).await?;
            return Ok(());
        }

        // Accept the subscription
        let response = MoqControlMessage::SubscribeOk {
            track_namespace: track_namespace.clone(),
        };

        self.send_control_message(response).await?;

        // Store the subscription (from peer)
        let subscription = MoqSubscription {
            track_namespace: track_namespace.clone(),
            state: MoqSubscriptionState::Active,
            priority,
            start_group,
            end_group,
        };

        self.subscriptions.insert(track_namespace, subscription);

        Ok(())
    }

    /// Unsubscribe from a track
    pub async fn unsubscribe_from_track(
        &mut self,
        track_namespace: &TrackNamespace,
    ) -> Result<(), QuicRtcError> {
        if self.state != MoqSessionState::Active {
            return Err(QuicRtcError::InvalidState {
                expected: "Active".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        // Remove subscription
        if let Some(mut subscription) = self.subscriptions.remove(track_namespace) {
            subscription.state = MoqSubscriptionState::Terminated;

            // Send unsubscribe message
            let unsubscribe_msg = MoqControlMessage::Unsubscribe {
                track_namespace: track_namespace.clone(),
            };

            self.send_control_message(unsubscribe_msg).await?;
        }

        Ok(())
    }

    /// Get all announced tracks
    pub fn announced_tracks(&self) -> &HashMap<TrackNamespace, MoqTrack> {
        &self.announced_tracks
    }

    /// Get all active subscriptions
    pub fn subscriptions(&self) -> &HashMap<TrackNamespace, MoqSubscription> {
        &self.subscriptions
    }

    /// Get subscription for a specific track
    pub fn get_subscription(&self, track_namespace: &TrackNamespace) -> Option<&MoqSubscription> {
        self.subscriptions.get(track_namespace)
    }

    /// Terminate the session
    pub async fn terminate(&mut self, code: u32, reason: String) -> Result<(), QuicRtcError> {
        if self.state == MoqSessionState::Terminated {
            return Ok(());
        }

        self.state = MoqSessionState::Terminating;

        let terminate_msg = MoqControlMessage::Terminate { code, reason };
        self.send_control_message(terminate_msg).await?;

        self.state = MoqSessionState::Terminated;

        Ok(())
    }

    /// Process incoming control message
    pub async fn process_control_message(
        &mut self,
        message: MoqControlMessage,
    ) -> Result<(), QuicRtcError> {
        match message {
            MoqControlMessage::Setup {
                version,
                capabilities,
            } => self.handle_setup_request(version, capabilities).await,
            MoqControlMessage::Announce {
                track_namespace,
                track,
            } => self.handle_track_announcement(track_namespace, track).await,
            MoqControlMessage::Subscribe {
                track_namespace,
                priority,
                start_group,
                end_group,
            } => {
                self.handle_subscription_request(track_namespace, priority, start_group, end_group)
                    .await
            }
            MoqControlMessage::Unsubscribe { track_namespace } => {
                // Handle unsubscribe by removing the subscription
                if let Some(mut subscription) = self.subscriptions.remove(&track_namespace) {
                    subscription.state = MoqSubscriptionState::Terminated;
                }
                Ok(())
            }
            MoqControlMessage::Terminate { code: _, reason: _ } => {
                self.state = MoqSessionState::Terminated;
                Ok(())
            }
            _ => {
                // Other messages are responses that should be handled by the waiting methods
                Ok(())
            }
        }
    }

    /// Send control message (placeholder - would use actual transport in real implementation)
    async fn send_control_message(&self, _message: MoqControlMessage) -> Result<(), QuicRtcError> {
        // In a real implementation, this would serialize and send the message over QUIC
        // For now, we'll just simulate success
        Ok(())
    }

    /// Receive control message (placeholder - would use actual transport in real implementation)
    async fn receive_control_message(&mut self) -> Result<MoqControlMessage, QuicRtcError> {
        // In a real implementation, this would receive and deserialize messages from QUIC
        // For now, we'll simulate a successful response based on the last operation
        // This is a placeholder that would be replaced with actual transport integration

        // Return a dummy success response for testing
        Ok(MoqControlMessage::SetupOk {
            version: 1,
            capabilities: MoqCapabilities::default(),
        })
    }
}

/// MoQ Object as defined by IETF specification
#[derive(Debug, Clone)]
pub struct MoqObject {
    /// Track namespace
    pub track_namespace: TrackNamespace,
    /// Track name
    pub track_name: String,
    /// Group ID (for video: timestamp, for audio: time window)
    pub group_id: u64,
    /// Object ID (for video: frame sequence, for audio: sample sequence)
    pub object_id: u64,
    /// Publisher priority (lower number = higher priority)
    pub publisher_priority: u8,
    /// Payload data (direct codec output)
    pub payload: Vec<u8>,
    /// Object status
    pub object_status: MoqObjectStatus,
    /// Object creation timestamp (for delivery ordering)
    pub created_at: std::time::Instant,
    /// Object size in bytes (for caching decisions)
    pub size: usize,
}

/// MoQ object delivery status
#[derive(Debug, Clone, PartialEq)]
pub enum MoqObjectStatus {
    /// Normal object
    Normal,
    /// Last object in group (e.g., end of video frame)
    EndOfGroup,
    /// Track is ending
    EndOfTrack,
}

/// MoQ Track representation
#[derive(Debug, Clone)]
pub struct MoqTrack {
    /// Track namespace
    pub namespace: TrackNamespace,
    /// Track name
    pub name: String,
    /// Track type
    pub track_type: MoqTrackType,
}

/// MoQ track types
#[derive(Debug, Clone, PartialEq)]
pub enum MoqTrackType {
    /// Audio track
    Audio,
    /// Video track
    Video,
    /// Data track
    Data,
}

/// Track namespace following MoQ specification
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackNamespace {
    /// Namespace (e.g., "conference.example.com")
    pub namespace: String,
    /// Track name (e.g., "alice/camera")
    pub track_name: String,
}

/// H.264 video frame for MoQ object creation
#[derive(Debug, Clone)]
pub struct H264Frame {
    /// NAL units (direct H.264 output, no RTP headers)
    pub nal_units: Vec<u8>,
    /// Whether this is a keyframe (I-frame)
    pub is_keyframe: bool,
    /// Frame timestamp in microseconds
    pub timestamp_us: u64,
    /// Frame sequence number
    pub sequence_number: u64,
}

/// Opus audio frame for MoQ object creation
#[derive(Debug, Clone)]
pub struct OpusFrame {
    /// Opus encoded data (direct Opus output, no RTP headers)
    pub opus_data: Vec<u8>,
    /// Frame timestamp in microseconds
    pub timestamp_us: u64,
    /// Frame sequence number
    pub sequence_number: u64,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
}

/// MoQ object delivery system over QUIC streams
#[derive(Debug)]
pub struct MoqObjectDelivery {
    /// Pending objects waiting for delivery
    pending_objects: std::collections::BinaryHeap<PrioritizedObject>,
    /// Object cache for efficient delivery
    object_cache: MoqObjectCache,
    /// Delivery statistics
    stats: MoqDeliveryStats,
}

/// Prioritized object wrapper for delivery ordering
#[derive(Debug)]
struct PrioritizedObject {
    object: MoqObject,
    priority: u8,
    enqueue_time: std::time::Instant,
}

impl PartialEq for PrioritizedObject {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PrioritizedObject {}

impl PartialOrd for PrioritizedObject {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedObject {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Lower priority number = higher priority (reverse order for max heap)
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.enqueue_time.cmp(&self.enqueue_time)) // FIFO for same priority
    }
}

/// MoQ object cache for efficient delivery and storage
#[derive(Debug)]
pub struct MoqObjectCache {
    /// Cached objects by track namespace and object ID
    objects: HashMap<TrackNamespace, HashMap<u64, CachedObject>>,
    /// Cache configuration
    config: MoqCacheConfig,
    /// Cache statistics
    stats: MoqCacheStats,
}

/// Cached object with metadata
#[derive(Debug, Clone)]
struct CachedObject {
    object: MoqObject,
    access_count: u64,
    last_accessed: std::time::Instant,
    cache_time: std::time::Instant,
}

/// MoQ cache configuration
#[derive(Debug, Clone)]
pub struct MoqCacheConfig {
    /// Maximum cache size in bytes
    pub max_size_bytes: usize,
    /// Maximum number of objects per track
    pub max_objects_per_track: usize,
    /// Object TTL in cache
    pub object_ttl: std::time::Duration,
    /// Enable LRU eviction
    pub enable_lru_eviction: bool,
}

impl Default for MoqCacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 10 * 1024 * 1024, // 10MB
            max_objects_per_track: 1000,
            object_ttl: std::time::Duration::from_secs(30),
            enable_lru_eviction: true,
        }
    }
}

/// MoQ cache statistics
#[derive(Debug, Clone, Default)]
pub struct MoqCacheStats {
    /// Total objects cached
    pub total_objects: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Objects evicted
    pub objects_evicted: u64,
    /// Current cache size in bytes
    pub current_size_bytes: usize,
}

/// MoQ delivery statistics
#[derive(Debug, Clone, Default)]
pub struct MoqDeliveryStats {
    /// Objects delivered successfully
    pub objects_delivered: u64,
    /// Objects dropped due to congestion
    pub objects_dropped: u64,
    /// Average delivery latency
    pub avg_delivery_latency_ms: f64,
    /// Current queue depth
    pub queue_depth: usize,
    /// Peak queue depth
    pub peak_queue_depth: usize,
}

impl MoqObjectDelivery {
    /// Create new object delivery system
    pub fn new(cache_config: MoqCacheConfig) -> Self {
        Self {
            pending_objects: std::collections::BinaryHeap::new(),
            object_cache: MoqObjectCache::new(cache_config),
            stats: MoqDeliveryStats::default(),
        }
    }

    /// Enqueue object for delivery
    pub fn enqueue_object(&mut self, object: MoqObject) -> Result<(), QuicRtcError> {
        let priority = object.delivery_priority();
        let prioritized = PrioritizedObject {
            object: object.clone(),
            priority,
            enqueue_time: std::time::Instant::now(),
        };

        self.pending_objects.push(prioritized);
        self.stats.queue_depth = self.pending_objects.len();

        if self.stats.queue_depth > self.stats.peak_queue_depth {
            self.stats.peak_queue_depth = self.stats.queue_depth;
        }

        // Cache the object for potential retransmission
        self.object_cache.store_object(object)?;

        Ok(())
    }

    /// Dequeue next object for delivery (highest priority first)
    pub fn dequeue_object(&mut self) -> Option<MoqObject> {
        if let Some(prioritized) = self.pending_objects.pop() {
            self.stats.queue_depth = self.pending_objects.len();

            // Update delivery statistics
            let delivery_latency = prioritized.enqueue_time.elapsed();
            self.update_delivery_latency(delivery_latency.as_millis() as f64);
            self.stats.objects_delivered += 1;

            Some(prioritized.object)
        } else {
            None
        }
    }

    /// Get object from cache
    pub fn get_cached_object(
        &mut self,
        track_namespace: &TrackNamespace,
        object_id: u64,
    ) -> Option<MoqObject> {
        self.object_cache.get_object(track_namespace, object_id)
    }

    /// Drop objects with lower priority to manage congestion
    pub fn drop_low_priority_objects(&mut self, max_priority: u8) -> usize {
        let original_len = self.pending_objects.len();

        // Collect objects to keep
        let mut objects_to_keep = Vec::new();
        while let Some(prioritized) = self.pending_objects.pop() {
            if prioritized.priority <= max_priority {
                objects_to_keep.push(prioritized);
            } else {
                self.stats.objects_dropped += 1;
            }
        }

        // Re-add kept objects
        for obj in objects_to_keep {
            self.pending_objects.push(obj);
        }

        let dropped_count = original_len - self.pending_objects.len();
        self.stats.queue_depth = self.pending_objects.len();

        dropped_count
    }

    /// Get current delivery statistics
    pub fn delivery_stats(&self) -> &MoqDeliveryStats {
        &self.stats
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> &MoqCacheStats {
        self.object_cache.stats()
    }

    /// Clear expired objects from queue and cache
    pub fn cleanup_expired(&mut self, max_age: std::time::Duration) {
        // Clean up expired objects from queue
        let mut objects_to_keep = Vec::new();
        while let Some(prioritized) = self.pending_objects.pop() {
            if prioritized.object.age() <= max_age {
                objects_to_keep.push(prioritized);
            } else {
                self.stats.objects_dropped += 1;
            }
        }

        // Re-add non-expired objects
        for obj in objects_to_keep {
            self.pending_objects.push(obj);
        }

        self.stats.queue_depth = self.pending_objects.len();

        // Clean up cache
        self.object_cache.cleanup_expired();
    }

    fn update_delivery_latency(&mut self, latency_ms: f64) {
        // Simple exponential moving average
        let alpha = 0.1;
        if self.stats.avg_delivery_latency_ms == 0.0 {
            self.stats.avg_delivery_latency_ms = latency_ms;
        } else {
            self.stats.avg_delivery_latency_ms =
                alpha * latency_ms + (1.0 - alpha) * self.stats.avg_delivery_latency_ms;
        }
    }
}

impl MoqObjectCache {
    /// Create new object cache
    pub fn new(config: MoqCacheConfig) -> Self {
        Self {
            objects: HashMap::new(),
            config,
            stats: MoqCacheStats::default(),
        }
    }

    /// Store object in cache
    pub fn store_object(&mut self, object: MoqObject) -> Result<(), QuicRtcError> {
        // Check cache size limits
        if self.stats.current_size_bytes + object.size > self.config.max_size_bytes {
            self.evict_objects()?;
        }

        // Check per-track object limit before getting mutable reference
        let needs_track_eviction =
            if let Some(track_objects) = self.objects.get(&object.track_namespace) {
                track_objects.len() >= self.config.max_objects_per_track
            } else {
                false
            };

        if needs_track_eviction {
            self.evict_track_objects(&object.track_namespace)?;
        }

        let track_objects = self
            .objects
            .entry(object.track_namespace.clone())
            .or_insert_with(HashMap::new);

        let cached_object = CachedObject {
            object: object.clone(),
            access_count: 0,
            last_accessed: std::time::Instant::now(),
            cache_time: std::time::Instant::now(),
        };

        track_objects.insert(object.object_id, cached_object);
        self.stats.total_objects += 1;
        self.stats.current_size_bytes += object.size;

        Ok(())
    }

    /// Get object from cache
    pub fn get_object(
        &mut self,
        track_namespace: &TrackNamespace,
        object_id: u64,
    ) -> Option<MoqObject> {
        if let Some(track_objects) = self.objects.get_mut(track_namespace) {
            if let Some(cached_object) = track_objects.get_mut(&object_id) {
                // Check if object has expired
                if cached_object.cache_time.elapsed() > self.config.object_ttl {
                    self.stats.current_size_bytes -= cached_object.object.size;
                    track_objects.remove(&object_id);
                    self.stats.cache_misses += 1;
                    return None;
                }

                // Update access statistics
                cached_object.access_count += 1;
                cached_object.last_accessed = std::time::Instant::now();
                self.stats.cache_hits += 1;

                return Some(cached_object.object.clone());
            }
        }

        self.stats.cache_misses += 1;
        None
    }

    /// Get cache statistics
    pub fn stats(&self) -> &MoqCacheStats {
        &self.stats
    }

    /// Clean up expired objects
    pub fn cleanup_expired(&mut self) {
        let mut expired_objects = Vec::new();

        for (track_namespace, track_objects) in &self.objects {
            for (object_id, cached_object) in track_objects {
                if cached_object.cache_time.elapsed() > self.config.object_ttl {
                    expired_objects.push((
                        track_namespace.clone(),
                        *object_id,
                        cached_object.object.size,
                    ));
                }
            }
        }

        for (track_namespace, object_id, size) in expired_objects {
            if let Some(track_objects) = self.objects.get_mut(&track_namespace) {
                track_objects.remove(&object_id);
                self.stats.current_size_bytes -= size;
                self.stats.objects_evicted += 1;
            }
        }

        // Remove empty track entries
        self.objects
            .retain(|_, track_objects| !track_objects.is_empty());
    }

    /// Evict objects to free space
    fn evict_objects(&mut self) -> Result<(), QuicRtcError> {
        if !self.config.enable_lru_eviction {
            return Err(QuicRtcError::CacheFull {
                current_size: self.stats.current_size_bytes,
                max_size: self.config.max_size_bytes,
            });
        }

        // Collect all objects with their last access time
        let mut all_objects = Vec::new();
        for (track_namespace, track_objects) in &self.objects {
            for (object_id, cached_object) in track_objects {
                all_objects.push((
                    track_namespace.clone(),
                    *object_id,
                    cached_object.last_accessed,
                    cached_object.object.size,
                ));
            }
        }

        // Sort by last accessed time (oldest first)
        all_objects.sort_by_key(|(_, _, last_accessed, _)| *last_accessed);

        // Evict oldest objects until we have enough space
        let target_size = self.config.max_size_bytes / 2; // Free up to 50% of cache
        let mut current_size = self.stats.current_size_bytes;

        for (track_namespace, object_id, _, size) in all_objects {
            if current_size <= target_size {
                break;
            }

            if let Some(track_objects) = self.objects.get_mut(&track_namespace) {
                track_objects.remove(&object_id);
                current_size -= size;
                self.stats.objects_evicted += 1;
            }
        }

        self.stats.current_size_bytes = current_size;

        // Remove empty track entries
        self.objects
            .retain(|_, track_objects| !track_objects.is_empty());

        Ok(())
    }

    /// Evict objects from a specific track
    fn evict_track_objects(
        &mut self,
        track_namespace: &TrackNamespace,
    ) -> Result<(), QuicRtcError> {
        if let Some(track_objects) = self.objects.get_mut(track_namespace) {
            if !self.config.enable_lru_eviction {
                return Err(QuicRtcError::TrackCacheFull {
                    track_name: track_namespace.track_name.clone(),
                    current_objects: track_objects.len(),
                    max_objects: self.config.max_objects_per_track,
                });
            }

            // Collect objects with their last access time
            let mut track_object_list = Vec::new();
            for (object_id, cached_object) in track_objects.iter() {
                track_object_list.push((
                    *object_id,
                    cached_object.last_accessed,
                    cached_object.object.size,
                ));
            }

            // Sort by last accessed time (oldest first)
            track_object_list.sort_by_key(|(_, last_accessed, _)| *last_accessed);

            // Evict oldest objects until we're under the limit
            let target_count = self.config.max_objects_per_track / 2; // Free up to 50% of track cache
            let mut current_count = track_objects.len();

            for (object_id, _, size) in track_object_list {
                if current_count <= target_count {
                    break;
                }

                track_objects.remove(&object_id);
                self.stats.current_size_bytes -= size;
                self.stats.objects_evicted += 1;
                current_count -= 1;
            }
        }

        Ok(())
    }
}

// Tests moved to tests/ directory
