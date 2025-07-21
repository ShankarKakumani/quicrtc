//! MoQ Stream Management Implementation
//!
//! This module implements comprehensive Media over QUIC (MoQ) stream management
//! according to IETF draft-ietf-moq-transport-13 specification.

use crate::error::QuicRtcError;
use crate::moq::{MoqControlMessage, MoqObject, MoqSession, MoqWireFormat};
use crate::transport::{QuicStream, StreamType, TransportConnection};
use bytes::BytesMut;
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Stream ID type for QUIC streams
pub type StreamId = u64;

/// Track alias type for efficient stream management
pub type TrackAlias = u64;

/// MoQ stream types according to IETF specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoqStreamType {
    /// Bidirectional control stream for session management (Section 3.3)
    Control,
    /// Unidirectional data streams for object delivery (Section 9.4)
    DataSubgroup,
    /// Datagram delivery for low-latency objects (Section 9.3)
    Datagram,
}

/// Stream state management according to IETF MoQ specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoqStreamState {
    /// Stream is being established
    Opening,
    /// Stream is active and ready for data
    Active,
    /// Stream is being gracefully closed
    Closing,
    /// Stream has been reset or terminated
    Reset,
    /// Stream has completed successfully
    Completed,
}

/// Stream usage statistics
#[derive(Debug, Default, Clone)]
pub struct StreamStats {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total objects sent
    pub objects_sent: u64,
    /// Total objects received
    pub objects_received: u64,
    /// Number of times stream was blocked by flow control
    pub flow_control_blocks: u64,
    /// Average object delivery latency
    pub avg_delivery_latency: Duration,
}

/// Managed MoQ stream with full lifecycle tracking
#[derive(Debug)]
pub struct ManagedMoqStream {
    /// Stream ID
    pub stream_id: StreamId,
    /// MoQ stream type
    pub stream_type: MoqStreamType,
    /// Current stream state
    pub state: MoqStreamState,
    /// Associated track alias (for data streams)
    pub track_alias: Option<TrackAlias>,
    /// Subgroup ID (for subgroup streams)
    pub subgroup_id: Option<u64>,
    /// Stream priority (0 = highest, 255 = lowest)
    pub priority: u8,
    /// Creation timestamp
    pub created_at: Instant,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// Underlying QUIC stream
    pub quic_stream: QuicStream,
    /// Pending objects queue
    pub pending_objects: VecDeque<MoqObject>,
    /// Statistics
    pub stats: StreamStats,
}

/// Stream manager configuration
#[derive(Debug, Clone)]
pub struct StreamManagerConfig {
    /// Maximum concurrent streams
    pub max_concurrent_streams: u32,
    /// Control stream timeout
    pub control_stream_timeout: Duration,
    /// Data stream idle timeout
    pub data_stream_timeout: Duration,
    /// Maximum objects per stream before creating new stream
    pub max_objects_per_stream: u32,
    /// Enable automatic stream cleanup
    pub enable_cleanup: bool,
    /// Cleanup interval
    pub cleanup_interval: Duration,
    /// Maximum pending objects per stream
    pub max_pending_objects: u32,
}

impl Default for StreamManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 100,
            control_stream_timeout: Duration::from_secs(30),
            data_stream_timeout: Duration::from_secs(60),
            max_objects_per_stream: 1000,
            enable_cleanup: true,
            cleanup_interval: Duration::from_secs(10),
            max_pending_objects: 50,
        }
    }
}

/// Stream manager events
#[derive(Debug, Clone)]
pub enum MoqStreamEvent {
    /// Control stream established
    ControlStreamEstablished {
        /// Stream identifier
        stream_id: StreamId,
    },
    /// Data stream created for track
    DataStreamCreated {
        /// Stream identifier
        stream_id: StreamId,
        /// Track alias
        track_alias: TrackAlias,
        /// Subgroup identifier
        subgroup_id: Option<u64>,
    },
    /// Stream state changed
    StreamStateChanged {
        /// Stream identifier
        stream_id: StreamId,
        /// Previous state
        old_state: MoqStreamState,
        /// New state
        new_state: MoqStreamState,
    },
    /// Object sent on stream
    ObjectSent {
        /// Stream identifier
        stream_id: StreamId,
        /// Object data
        object: MoqObject,
        /// Delivery latency
        delivery_latency: Duration,
    },
    /// Object received on stream
    ObjectReceived {
        /// Stream identifier
        stream_id: StreamId,
        /// Object data
        object: MoqObject,
    },
    /// Stream error occurred
    StreamError {
        /// Stream identifier
        stream_id: StreamId,
        /// Error message
        error: String,
    },
    /// Stream closed
    StreamClosed {
        /// Stream identifier
        stream_id: StreamId,
        /// Close reason
        reason: String,
    },
}

/// MoQ Stream Manager - Comprehensive stream lifecycle management
#[derive(Debug)]
pub struct MoqStreamManager {
    /// Transport connection for creating streams
    transport: Arc<RwLock<TransportConnection>>,
    /// MoQ session for protocol logic
    session: Arc<RwLock<MoqSession>>,
    /// Active streams by stream ID
    streams: Arc<RwLock<HashMap<StreamId, ManagedMoqStream>>>,
    /// Track alias to stream mapping
    track_streams: Arc<RwLock<HashMap<TrackAlias, Vec<StreamId>>>>,
    /// Control stream ID (single bidirectional stream)
    control_stream_id: Arc<RwLock<Option<StreamId>>>,
    /// Stream creation semaphore for flow control
    stream_semaphore: Arc<Semaphore>,
    /// Event notification channels
    event_tx: mpsc::UnboundedSender<MoqStreamEvent>,
    /// Configuration
    config: StreamManagerConfig,
}

/// Stream manager statistics
#[derive(Debug, Clone)]
pub struct MoqStreamManagerStats {
    /// Total number of streams
    pub total_streams: u32,
    /// Number of control streams
    pub control_streams: u32,
    /// Number of data streams
    pub data_streams: u32,
    /// Number of active streams
    pub active_streams: u32,
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Total objects sent
    pub total_objects_sent: u64,
    /// Total objects received
    pub total_objects_received: u64,
}

impl ManagedMoqStream {
    /// Create a new control stream
    pub fn new_control(stream_id: StreamId, quic_stream: QuicStream) -> Self {
        let now = Instant::now();
        Self {
            stream_id,
            stream_type: MoqStreamType::Control,
            state: MoqStreamState::Opening,
            track_alias: None,
            subgroup_id: None,
            priority: 0, // Control streams have highest priority
            created_at: now,
            last_activity: now,
            quic_stream,
            pending_objects: VecDeque::new(),
            stats: StreamStats::default(),
        }
    }

    /// Create a new data subgroup stream
    pub fn new_data_subgroup(
        stream_id: StreamId,
        track_alias: TrackAlias,
        subgroup_id: u64,
        priority: u8,
        quic_stream: QuicStream,
    ) -> Self {
        let now = Instant::now();
        Self {
            stream_id,
            stream_type: MoqStreamType::DataSubgroup,
            state: MoqStreamState::Opening,
            track_alias: Some(track_alias),
            subgroup_id: Some(subgroup_id),
            priority,
            created_at: now,
            last_activity: now,
            quic_stream,
            pending_objects: VecDeque::new(),
            stats: StreamStats::default(),
        }
    }

    /// Update stream state and last activity
    pub fn set_state(&mut self, new_state: MoqStreamState) -> MoqStreamState {
        let old_state = self.state.clone();
        self.state = new_state;
        self.last_activity = Instant::now();
        old_state
    }

    /// Check if stream is idle based on timeout
    pub fn is_idle(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout && self.pending_objects.is_empty()
    }

    /// Check if stream can accept more objects
    pub fn can_accept_objects(&self, max_pending: u32) -> bool {
        self.state == MoqStreamState::Active && (self.pending_objects.len() as u32) < max_pending
    }

    /// Add object to pending queue
    pub fn enqueue_object(&mut self, object: MoqObject) -> Result<(), QuicRtcError> {
        if self.state != MoqStreamState::Active {
            return Err(QuicRtcError::InvalidState {
                expected: "Active".to_string(),
                actual: format!("{:?}", self.state),
            });
        }

        self.pending_objects.push_back(object);
        self.last_activity = Instant::now();
        Ok(())
    }

    /// Send next pending object
    pub async fn send_next_object(&mut self) -> Result<Option<MoqObject>, QuicRtcError> {
        if let Some(object) = self.pending_objects.pop_front() {
            let send_start = Instant::now();

            // Encode object using wire format
            let mut buffer = BytesMut::new();
            match self.stream_type {
                MoqStreamType::DataSubgroup => {
                    if let Some(track_alias) = self.track_alias {
                        MoqWireFormat::encode_object_stream(&object, track_alias, &mut buffer)?;
                    } else {
                        return Err(QuicRtcError::InvalidState {
                            expected: "Track alias for data stream".to_string(),
                            actual: "None".to_string(),
                        });
                    }
                }
                MoqStreamType::Datagram => {
                    if let Some(track_alias) = self.track_alias {
                        MoqWireFormat::encode_object_datagram(&object, track_alias, &mut buffer)?;
                    } else {
                        return Err(QuicRtcError::InvalidState {
                            expected: "Track alias for datagram".to_string(),
                            actual: "None".to_string(),
                        });
                    }
                }
                MoqStreamType::Control => {
                    return Err(QuicRtcError::InvalidOperation {
                        operation: "Send object on control stream".to_string(),
                    });
                }
            }

            // Send over QUIC stream
            self.quic_stream.send(&buffer).await?;

            // Update statistics
            let delivery_latency = send_start.elapsed();
            self.stats.bytes_sent += buffer.len() as u64;
            self.stats.objects_sent += 1;
            self.stats.avg_delivery_latency =
                (self.stats.avg_delivery_latency + delivery_latency) / 2;
            self.last_activity = Instant::now();

            debug!(
                "Sent object on stream {}: group={}, object={}, bytes={}, latency={:?}",
                self.stream_id,
                object.group_id,
                object.object_id,
                buffer.len(),
                delivery_latency
            );

            Ok(Some(object))
        } else {
            Ok(None)
        }
    }

    /// Close stream gracefully
    pub async fn close(&mut self) -> Result<(), QuicRtcError> {
        self.set_state(MoqStreamState::Closing);
        self.quic_stream.finish().await?;
        self.set_state(MoqStreamState::Completed);
        Ok(())
    }
}

impl MoqStreamManager {
    /// Create new MoQ stream manager
    pub fn new(
        transport: Arc<RwLock<TransportConnection>>,
        session: Arc<RwLock<MoqSession>>,
        config: StreamManagerConfig,
    ) -> (Self, mpsc::UnboundedReceiver<MoqStreamEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let stream_semaphore = Arc::new(Semaphore::new(config.max_concurrent_streams as usize));

        let manager = Self {
            transport,
            session,
            streams: Arc::new(RwLock::new(HashMap::new())),
            track_streams: Arc::new(RwLock::new(HashMap::new())),
            control_stream_id: Arc::new(RwLock::new(None)),
            stream_semaphore,
            event_tx,
            config,
        };

        (manager, event_rx)
    }

    /// Establish control stream for MoQ session (Section 3.3)
    pub async fn establish_control_stream(&self) -> Result<StreamId, QuicRtcError> {
        info!("Establishing MoQ control stream");

        // Check if control stream already exists
        {
            let control_id = self.control_stream_id.read();
            if control_id.is_some() {
                return Err(QuicRtcError::InvalidState {
                    expected: "No control stream".to_string(),
                    actual: "Control stream already exists".to_string(),
                });
            }
        }

        // Acquire permit for stream creation
        let _permit =
            self.stream_semaphore
                .acquire()
                .await
                .map_err(|_| QuicRtcError::ResourceExhausted {
                    resource: "Stream permits".to_string(),
                })?;

        // Create bidirectional QUIC stream
        let quic_stream = {
            let mut transport = self.transport.write();
            transport.open_stream(StreamType::Bidirectional).await?
        };

        let stream_id = quic_stream.id;
        let mut managed_stream = ManagedMoqStream::new_control(stream_id, quic_stream);
        managed_stream.set_state(MoqStreamState::Active);

        // Store control stream
        {
            let mut streams = self.streams.write();
            streams.insert(stream_id, managed_stream);
        }

        {
            let mut control_id = self.control_stream_id.write();
            *control_id = Some(stream_id);
        }

        // Send event
        let _ = self
            .event_tx
            .send(MoqStreamEvent::ControlStreamEstablished { stream_id });

        info!("MoQ control stream established: {}", stream_id);
        Ok(stream_id)
    }

    /// Send control message on control stream
    pub async fn send_control_message(
        &self,
        message: MoqControlMessage,
    ) -> Result<(), QuicRtcError> {
        let control_stream_id = {
            let control_id = self.control_stream_id.read();
            control_id.ok_or_else(|| QuicRtcError::InvalidState {
                expected: "Control stream established".to_string(),
                actual: "No control stream".to_string(),
            })?
        };

        // Encode control message
        let mut buffer = BytesMut::new();
        MoqWireFormat::encode_control_message(&message, &mut buffer)?;

        // Send with timeout
        let send_future = async {
            let mut streams = self.streams.write();
            if let Some(stream) = streams.get_mut(&control_stream_id) {
                stream.quic_stream.send(&buffer).await?;
                stream.stats.bytes_sent += buffer.len() as u64;
                stream.last_activity = Instant::now();
                Ok(())
            } else {
                Err(QuicRtcError::StreamNotFound {
                    stream_id: control_stream_id,
                })
            }
        };

        timeout(self.config.control_stream_timeout, send_future)
            .await
            .map_err(|_| QuicRtcError::Timeout {
                operation: "Send control message".to_string(),
                duration: self.config.control_stream_timeout,
            })??;

        debug!("Sent control message: {:?}", message);
        Ok(())
    }

    /// Create data stream for track objects (Section 9.4)
    pub async fn create_data_stream(
        &self,
        track_alias: TrackAlias,
        subgroup_id: u64,
        priority: u8,
    ) -> Result<StreamId, QuicRtcError> {
        debug!(
            "Creating data stream for track alias: {}, subgroup: {}",
            track_alias, subgroup_id
        );

        // Acquire permit for stream creation
        let _permit =
            self.stream_semaphore
                .acquire()
                .await
                .map_err(|_| QuicRtcError::ResourceExhausted {
                    resource: "Stream permits".to_string(),
                })?;

        // Create unidirectional QUIC stream
        let quic_stream = {
            let mut transport = self.transport.write();
            transport.open_stream(StreamType::Unidirectional).await?
        };

        let stream_id = quic_stream.id;
        let mut managed_stream = ManagedMoqStream::new_data_subgroup(
            stream_id,
            track_alias,
            subgroup_id,
            priority,
            quic_stream,
        );
        managed_stream.set_state(MoqStreamState::Active);

        // Store stream
        {
            let mut streams = self.streams.write();
            streams.insert(stream_id, managed_stream);
        }

        // Update track mapping
        {
            let mut track_streams = self.track_streams.write();
            track_streams
                .entry(track_alias)
                .or_insert_with(Vec::new)
                .push(stream_id);
        }

        // Send event
        let _ = self.event_tx.send(MoqStreamEvent::DataStreamCreated {
            stream_id,
            track_alias,
            subgroup_id: Some(subgroup_id),
        });

        debug!(
            "Data stream created: {} for track: {}",
            stream_id, track_alias
        );
        Ok(stream_id)
    }

    /// Send object on appropriate stream
    pub async fn send_object(
        &self,
        object: MoqObject,
        track_alias: TrackAlias,
    ) -> Result<(), QuicRtcError> {
        let send_start = Instant::now();

        // Find or create appropriate stream for this object
        let stream_id = self
            .find_or_create_stream_for_object(&object, track_alias)
            .await?;

        // Send object on the stream
        {
            let mut streams = self.streams.write();
            if let Some(stream) = streams.get_mut(&stream_id) {
                stream.enqueue_object(object.clone())?;
                if let Some(sent_object) = stream.send_next_object().await? {
                    let delivery_latency = send_start.elapsed();

                    // Send event
                    let _ = self.event_tx.send(MoqStreamEvent::ObjectSent {
                        stream_id,
                        object: sent_object,
                        delivery_latency,
                    });
                }
            } else {
                return Err(QuicRtcError::StreamNotFound { stream_id });
            }
        }

        Ok(())
    }

    /// Find or create appropriate stream for object
    async fn find_or_create_stream_for_object(
        &self,
        object: &MoqObject,
        track_alias: TrackAlias,
    ) -> Result<StreamId, QuicRtcError> {
        // Check existing streams for this track
        {
            let track_streams = self.track_streams.read();
            let streams = self.streams.read();

            if let Some(stream_ids) = track_streams.get(&track_alias) {
                for &stream_id in stream_ids {
                    if let Some(stream) = streams.get(&stream_id) {
                        // Use stream if it's active and can accept more objects
                        if stream.can_accept_objects(self.config.max_pending_objects)
                            && stream.stats.objects_sent < self.config.max_objects_per_stream as u64
                        {
                            return Ok(stream_id);
                        }
                    }
                }
            }
        }

        // Create new stream
        let subgroup_id = object.group_id;
        let priority = object.publisher_priority;

        self.create_data_stream(track_alias, subgroup_id, priority)
            .await
    }

    /// Close stream and cleanup resources
    pub async fn close_stream(&self, stream_id: StreamId) -> Result<(), QuicRtcError> {
        debug!("Closing stream: {}", stream_id);

        let reason = {
            let mut streams = self.streams.write();
            if let Some(mut stream) = streams.remove(&stream_id) {
                let _ = stream.close().await;
                "Normal closure".to_string()
            } else {
                return Err(QuicRtcError::StreamNotFound { stream_id });
            }
        };

        // Remove from track mapping
        {
            let mut track_streams = self.track_streams.write();
            for stream_list in track_streams.values_mut() {
                stream_list.retain(|&id| id != stream_id);
            }
        }

        // Send event
        let _ = self
            .event_tx
            .send(MoqStreamEvent::StreamClosed { stream_id, reason });

        debug!("Stream closed: {}", stream_id);
        Ok(())
    }

    /// Get stream statistics
    pub fn get_stream_stats(&self, stream_id: StreamId) -> Option<StreamStats> {
        let streams = self.streams.read();
        streams.get(&stream_id).map(|stream| stream.stats.clone())
    }

    /// Get all active streams
    pub fn get_active_streams(&self) -> Vec<StreamId> {
        let streams = self.streams.read();
        streams
            .iter()
            .filter(|(_, stream)| stream.state == MoqStreamState::Active)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get summary statistics
    pub fn get_summary_stats(&self) -> MoqStreamManagerStats {
        let streams = self.streams.read();
        let total_streams = streams.len() as u32;
        let control_streams = streams
            .iter()
            .filter(|(_, s)| s.stream_type == MoqStreamType::Control)
            .count() as u32;
        let data_streams = streams
            .iter()
            .filter(|(_, s)| s.stream_type == MoqStreamType::DataSubgroup)
            .count() as u32;
        let active_streams = streams
            .iter()
            .filter(|(_, s)| s.state == MoqStreamState::Active)
            .count() as u32;

        let total_bytes_sent: u64 = streams.values().map(|s| s.stats.bytes_sent).sum();
        let total_bytes_received: u64 = streams.values().map(|s| s.stats.bytes_received).sum();
        let total_objects_sent: u64 = streams.values().map(|s| s.stats.objects_sent).sum();
        let total_objects_received: u64 = streams.values().map(|s| s.stats.objects_received).sum();

        MoqStreamManagerStats {
            total_streams,
            control_streams,
            data_streams,
            active_streams,
            total_bytes_sent,
            total_bytes_received,
            total_objects_sent,
            total_objects_received,
        }
    }
}

// Clone implementation for sharing stream manager across tasks
impl Clone for MoqStreamManager {
    fn clone(&self) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
            session: Arc::clone(&self.session),
            streams: Arc::clone(&self.streams),
            track_streams: Arc::clone(&self.track_streams),
            control_stream_id: Arc::clone(&self.control_stream_id),
            stream_semaphore: Arc::clone(&self.stream_semaphore),
            event_tx: self.event_tx.clone(),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::QuicStream;

    #[test]
    fn test_managed_stream_creation() {
        let quic_stream = QuicStream {
            id: 123,
            stream_type: StreamType::Bidirectional,
            send: None,
            recv: None,
        };

        let control_stream = ManagedMoqStream::new_control(123, quic_stream);
        assert_eq!(control_stream.stream_id, 123);
        assert_eq!(control_stream.stream_type, MoqStreamType::Control);
        assert_eq!(control_stream.state, MoqStreamState::Opening);
        assert!(control_stream.track_alias.is_none());

        let quic_stream2 = QuicStream {
            id: 456,
            stream_type: StreamType::Unidirectional,
            send: None,
            recv: None,
        };

        let data_stream = ManagedMoqStream::new_data_subgroup(456, 1, 100, 128, quic_stream2);
        assert_eq!(data_stream.stream_id, 456);
        assert_eq!(data_stream.stream_type, MoqStreamType::DataSubgroup);
        assert_eq!(data_stream.track_alias, Some(1));
        assert_eq!(data_stream.subgroup_id, Some(100));
        assert_eq!(data_stream.priority, 128);
    }

    #[test]
    fn test_stream_state_transitions() {
        let quic_stream = QuicStream {
            id: 789,
            stream_type: StreamType::Unidirectional,
            send: None,
            recv: None,
        };

        let mut stream = ManagedMoqStream::new_data_subgroup(789, 2, 200, 64, quic_stream);

        assert_eq!(stream.state, MoqStreamState::Opening);

        let old_state = stream.set_state(MoqStreamState::Active);
        assert_eq!(old_state, MoqStreamState::Opening);
        assert_eq!(stream.state, MoqStreamState::Active);

        stream.set_state(MoqStreamState::Closing);
        assert_eq!(stream.state, MoqStreamState::Closing);
    }

    #[test]
    fn test_stream_manager_config() {
        let config = StreamManagerConfig::default();
        assert_eq!(config.max_concurrent_streams, 100);
        assert_eq!(config.max_objects_per_stream, 1000);
        assert!(config.enable_cleanup);
    }
}
 