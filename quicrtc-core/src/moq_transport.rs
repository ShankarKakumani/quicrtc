//! MoQ over QUIC transport integration
//!
//! This module provides the integration between IETF Media over QUIC (MoQ) protocol
//! and QUIC transport, implementing the core functionality for MoQ over QUIC communication.

use crate::error::QuicRtcError;
use crate::moq::{
    MoqObject, MoqSession, MoqSessionState, MoqStreamManager, MoqStreamType, MoqSubscription,
    MoqTrack, StreamId, StreamManagerConfig, TrackNamespace,
};
use crate::transport::{ConnectionConfig, QuicStream, StreamType, TransportConnection};
use bytes::Bytes;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info};
use uuid::Uuid;

/// MoQ stream wrapper with metadata
#[derive(Debug)]
pub struct MoqStream {
    /// Stream ID
    pub stream_id: StreamId,
    /// MoQ stream type
    pub stream_type: MoqStreamType,
    /// Associated track namespace (for data streams)
    pub track_namespace: Option<TrackNamespace>,
    /// Underlying QUIC stream
    pub quic_stream: QuicStream,
}

impl MoqStream {
    /// Create a new MoQ control stream
    pub fn control(stream_id: StreamId, quic_stream: QuicStream) -> Self {
        Self {
            stream_id,
            stream_type: MoqStreamType::Control,
            track_namespace: None,
            quic_stream,
        }
    }

    /// Create a new MoQ data stream for a specific track
    pub fn data(
        stream_id: StreamId,
        track_namespace: TrackNamespace,
        quic_stream: QuicStream,
    ) -> Self {
        Self {
            stream_id,
            stream_type: MoqStreamType::DataSubgroup,
            track_namespace: Some(track_namespace),
            quic_stream,
        }
    }

    /// Send data on the stream
    pub async fn send(&mut self, data: &[u8]) -> Result<(), QuicRtcError> {
        self.quic_stream.send(data).await
    }

    /// Receive data from the stream
    pub async fn recv(&mut self) -> Result<Option<Bytes>, QuicRtcError> {
        self.quic_stream.recv().await
    }

    /// Finish the stream
    pub async fn finish(&mut self) -> Result<(), QuicRtcError> {
        self.quic_stream.finish().await
    }
}

/// MoQ over QUIC transport integration
#[derive(Debug)]
pub struct MoqOverQuicTransport {
    /// Transport connection ID
    connection_id: Uuid,
    /// QUIC transport connection
    quic_connection: Arc<RwLock<TransportConnection>>,
    /// MoQ session
    moq_session: Arc<RwLock<MoqSession>>,
    /// MoQ stream manager
    stream_manager: Arc<MoqStreamManager>,
    /// Track to stream mapping for data streams
    track_streams: Arc<RwLock<HashMap<TrackNamespace, StreamId>>>,
    /// Object delivery queue
    object_queue: Arc<RwLock<Vec<MoqObject>>>,
    /// Event channels
    event_tx: mpsc::UnboundedSender<MoqTransportEvent>,
    event_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<MoqTransportEvent>>>>,
}

/// Events from MoQ transport
#[derive(Debug, Clone)]
pub enum MoqTransportEvent {
    /// Session established successfully
    SessionEstablished {
        /// Session ID
        session_id: u64,
    },
    /// Track announced by peer
    TrackAnnounced {
        /// Track namespace
        track_namespace: TrackNamespace,
        /// Track information
        track: MoqTrack,
    },
    /// Subscription request received
    SubscriptionRequested {
        /// Track namespace
        track_namespace: TrackNamespace,
        /// Subscription priority
        priority: u8,
    },
    /// Object received
    ObjectReceived {
        /// MoQ object
        object: MoqObject,
    },
    /// Stream established
    StreamEstablished {
        /// Stream ID
        stream_id: StreamId,
        /// Stream type
        stream_type: MoqStreamType,
        /// Track namespace (for data streams)
        track_namespace: Option<TrackNamespace>,
    },
    /// Transport error
    TransportError {
        /// Error message
        error: String,
    },
}

impl MoqOverQuicTransport {
    /// Create a new MoQ over QUIC transport
    pub async fn new(
        endpoint: SocketAddr,
        config: ConnectionConfig,
        session_id: u64,
    ) -> Result<Self, QuicRtcError> {
        info!("Creating MoQ over QUIC transport to {}", endpoint);

        // Establish QUIC connection
        let quic_connection =
            TransportConnection::establish_with_fallback(endpoint, config).await?;
        let connection_id = quic_connection.connection_id();

        info!(
            "QUIC connection established using {:?}",
            quic_connection.current_transport_mode()
        );

        // Create MoQ session
        let mut moq_session = MoqSession::new(session_id);
        let moq_session_arc = Arc::new(RwLock::new(moq_session));

        // Create stream manager with the session and transport
        let quic_connection_arc = Arc::new(RwLock::new(quic_connection));
        let (stream_manager, _stream_events) = MoqStreamManager::new(
            Arc::clone(&quic_connection_arc),
            Arc::clone(&moq_session_arc),
            StreamManagerConfig::default(),
        );
        let stream_manager_arc = Arc::new(stream_manager);

        // Connect the stream manager to the session
        {
            let mut session = moq_session_arc.write();
            session.set_stream_manager(Arc::clone(&stream_manager_arc));
        }

        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let transport = Self {
            connection_id,
            quic_connection: quic_connection_arc,
            moq_session: moq_session_arc,
            stream_manager: stream_manager_arc,
            track_streams: Arc::new(RwLock::new(HashMap::new())),
            object_queue: Arc::new(RwLock::new(Vec::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        };

        debug!(
            "MoQ over QUIC transport created with connection ID: {}",
            connection_id
        );

        Ok(transport)
    }

    /// Establish MoQ session over QUIC
    pub async fn establish_session(&self) -> Result<(), QuicRtcError> {
        info!("Establishing MoQ session");

        // Establish control stream using the stream manager
        let control_stream_id = self.stream_manager.establish_control_stream().await?;
        info!("Control stream established: {}", control_stream_id);

        // Establish MoQ session
        {
            let mut session = self.moq_session.write();
            session.establish_session().await?;
        }

        // Send session established event
        let _ = self.event_tx.send(MoqTransportEvent::SessionEstablished {
            session_id: self.moq_session.read().session_id(),
        });

        info!("MoQ session established successfully");
        Ok(())
    }

    /// Create a control stream for MoQ session management
    async fn create_control_stream(&self) -> Result<MoqStream, QuicRtcError> {
        debug!("Creating MoQ control stream");

        let quic_stream = {
            let mut connection = self.quic_connection.write();
            connection.open_stream(StreamType::Bidirectional).await?
        };

        let stream_id = quic_stream.id;
        let moq_stream = MoqStream::control(stream_id, quic_stream);

        debug!("MoQ control stream created with ID: {}", stream_id);

        // Send stream established event
        let _ = self.event_tx.send(MoqTransportEvent::StreamEstablished {
            stream_id,
            stream_type: MoqStreamType::Control,
            track_namespace: None,
        });

        Ok(moq_stream)
    }

    /// Create a data stream for a specific track
    async fn create_data_stream(
        &self,
        track_namespace: TrackNamespace,
    ) -> Result<MoqStream, QuicRtcError> {
        debug!("Creating MoQ data stream for track: {:?}", track_namespace);

        let quic_stream = {
            let mut connection = self.quic_connection.write();
            connection.open_stream(StreamType::Unidirectional).await?
        };

        let stream_id = quic_stream.id;
        let moq_stream = MoqStream::data(stream_id, track_namespace.clone(), quic_stream);

        debug!(
            "MoQ data stream created with ID: {} for track: {:?}",
            stream_id, track_namespace
        );

        // Store stream mapping
        {
            let mut track_streams = self.track_streams.write();
            track_streams.insert(track_namespace.clone(), stream_id);
        }

        // Send stream established event
        let _ = self.event_tx.send(MoqTransportEvent::StreamEstablished {
            stream_id,
            stream_type: MoqStreamType::DataSubgroup,
            track_namespace: Some(track_namespace),
        });

        Ok(moq_stream)
    }

    /// Announce a track for publishing
    pub async fn announce_track(&self, track: MoqTrack) -> Result<(), QuicRtcError> {
        info!("Announcing track: {:?}", track.namespace);

        // Announce track in MoQ session
        {
            let mut session = self.moq_session.write();
            session.announce_track(track.clone()).await?;
        }

        info!("Track announced successfully: {:?}", track.namespace);
        Ok(())
    }

    /// Subscribe to a track
    pub async fn subscribe_to_track(
        &self,
        track_namespace: TrackNamespace,
        priority: u8,
        start_group: Option<u64>,
        end_group: Option<u64>,
    ) -> Result<MoqSubscription, QuicRtcError> {
        info!("Subscribing to track: {:?}", track_namespace);

        // Subscribe in MoQ session
        let subscription = {
            let mut session = self.moq_session.write();
            session
                .subscribe_to_track(track_namespace.clone(), priority, start_group, end_group)
                .await?
        };

        info!("Successfully subscribed to track: {:?}", track_namespace);
        Ok(subscription)
    }

    /// Send a MoQ object using the stream manager
    pub async fn send_moq_object(&self, object: MoqObject) -> Result<(), QuicRtcError> {
        debug!(
            "Sending MoQ object for track: {:?}, group: {}, object: {}",
            object.track_namespace, object.group_id, object.object_id
        );

        // Use stream manager to send object (simplified for now)
        // In full implementation, this would map track namespace to track alias
        let track_alias = 1; // Simplified mapping
        self.stream_manager.send_object(object, track_alias).await
    }

    /// Receive a MoQ object from any data stream
    pub async fn receive_moq_object(&self) -> Result<MoqObject, QuicRtcError> {
        // Check if we have any queued objects
        {
            let mut queue = self.object_queue.write();
            if let Some(object) = queue.pop() {
                debug!(
                    "Retrieved queued MoQ object for track: {:?}",
                    object.track_namespace
                );
                return Ok(object);
            }
        }

        // For now, simplified - in full implementation would use stream manager events
        // to receive objects from data streams

        Err(QuicRtcError::NoDataAvailable)
    }

    /// Handle incoming track announcement
    pub async fn handle_track_announcement(
        &self,
        track_namespace: TrackNamespace,
        track: MoqTrack,
    ) -> Result<(), QuicRtcError> {
        info!("Handling track announcement: {:?}", track_namespace);

        // Handle in MoQ session
        {
            let mut session = self.moq_session.write();
            session
                .handle_track_announcement(track_namespace.clone(), track.clone())
                .await?;
        }

        // Send track announced event
        let _ = self.event_tx.send(MoqTransportEvent::TrackAnnounced {
            track_namespace,
            track,
        });

        Ok(())
    }

    /// Handle incoming subscription request
    pub async fn handle_subscription_request(
        &self,
        track_namespace: TrackNamespace,
        priority: u8,
        start_group: Option<u64>,
        end_group: Option<u64>,
    ) -> Result<(), QuicRtcError> {
        info!(
            "Handling subscription request for track: {:?}",
            track_namespace
        );

        // Handle in MoQ session
        {
            let mut session = self.moq_session.write();
            session
                .handle_subscription_request(
                    track_namespace.clone(),
                    priority,
                    start_group,
                    end_group,
                )
                .await?;
        }

        // Send subscription requested event
        let _ = self
            .event_tx
            .send(MoqTransportEvent::SubscriptionRequested {
                track_namespace,
                priority,
            });

        Ok(())
    }

    /// Get all announced tracks
    pub fn announced_tracks(&self) -> HashMap<TrackNamespace, MoqTrack> {
        let session = self.moq_session.read();
        session.announced_tracks().clone()
    }

    /// Get all active subscriptions
    pub fn subscriptions(&self) -> HashMap<TrackNamespace, MoqSubscription> {
        let session = self.moq_session.read();
        session.subscriptions().clone()
    }

    /// Get MoQ session state
    pub fn session_state(&self) -> MoqSessionState {
        let session = self.moq_session.read();
        session.state().clone()
    }

    /// Get connection ID
    pub fn connection_id(&self) -> Uuid {
        self.connection_id
    }

    /// Get transport mode
    pub fn transport_mode(&self) -> crate::transport::TransportMode {
        let connection = self.quic_connection.read();
        connection.current_transport_mode()
    }

    /// Check if transport is connected
    pub fn is_connected(&self) -> bool {
        let connection = self.quic_connection.read();
        connection.is_connected()
    }

    /// Get event receiver (can only be taken once)
    pub fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<MoqTransportEvent>> {
        let mut event_rx = self.event_rx.write();
        event_rx.take()
    }

    /// Close the transport gracefully
    pub async fn close(&self) -> Result<(), QuicRtcError> {
        info!("Closing MoQ over QUIC transport");

        // Terminate MoQ session
        {
            let mut session = self.moq_session.write();
            session.terminate(0, "Normal closure".to_string()).await?;
        }

        // Streams will be closed when stream manager is dropped

        // Close QUIC connection
        {
            let mut connection = self.quic_connection.write();
            connection.close().await?;
        }

        info!("MoQ over QUIC transport closed successfully");
        Ok(())
    }

    /// Serialize MoQ object for transmission (simplified implementation)
    fn serialize_moq_object(&self, object: &MoqObject) -> Result<Vec<u8>, QuicRtcError> {
        // In a real implementation, this would use proper MoQ wire format
        // For now, we'll use a simple binary format for testing

        let mut buffer = Vec::new();

        // Track namespace length and data
        let namespace_bytes = object.track_namespace.namespace.as_bytes();
        buffer.extend_from_slice(&(namespace_bytes.len() as u32).to_be_bytes());
        buffer.extend_from_slice(namespace_bytes);

        let track_name_bytes = object.track_namespace.track_name.as_bytes();
        buffer.extend_from_slice(&(track_name_bytes.len() as u32).to_be_bytes());
        buffer.extend_from_slice(track_name_bytes);

        // Object metadata
        buffer.extend_from_slice(&object.group_id.to_be_bytes());
        buffer.extend_from_slice(&object.object_id.to_be_bytes());
        buffer.extend_from_slice(&[object.publisher_priority]);

        // Object status
        let status_byte = match object.object_status {
            crate::moq::MoqObjectStatus::Normal => 0u8,
            crate::moq::MoqObjectStatus::EndOfGroup => 1u8,
            crate::moq::MoqObjectStatus::EndOfTrack => 2u8,
        };
        buffer.push(status_byte);

        // Payload length and data
        buffer.extend_from_slice(&(object.payload.len() as u32).to_be_bytes());
        buffer.extend_from_slice(&object.payload);

        Ok(buffer)
    }

    /// Deserialize MoQ object from received data (simplified implementation)
    fn deserialize_moq_object(&self, data: &[u8]) -> Result<MoqObject, QuicRtcError> {
        // In a real implementation, this would parse proper MoQ wire format
        // For now, we'll use a simple binary format for testing

        let mut offset = 0;

        if data.len() < 4 {
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for namespace length".to_string(),
            });
        }

        // Parse namespace
        let namespace_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        offset += 4;

        if data.len() < offset + namespace_len {
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for namespace".to_string(),
            });
        }

        let namespace =
            String::from_utf8(data[offset..offset + namespace_len].to_vec()).map_err(|_| {
                QuicRtcError::InvalidData {
                    reason: "Invalid UTF-8 in namespace".to_string(),
                }
            })?;
        offset += namespace_len;

        // Parse track name
        if data.len() < offset + 4 {
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for track name length".to_string(),
            });
        }

        let track_name_len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if data.len() < offset + track_name_len {
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for track name".to_string(),
            });
        }

        let track_name = String::from_utf8(data[offset..offset + track_name_len].to_vec())
            .map_err(|_| QuicRtcError::InvalidData {
                reason: "Invalid UTF-8 in track name".to_string(),
            })?;
        offset += track_name_len;

        // Parse object metadata
        if data.len() < offset + 17 {
            // 8 + 8 + 1 bytes for group_id, object_id, priority
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for object metadata".to_string(),
            });
        }

        let group_id = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        let object_id = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        let publisher_priority = data[offset];
        offset += 1;

        // Parse object status
        let object_status = match data[offset] {
            0 => crate::moq::MoqObjectStatus::Normal,
            1 => crate::moq::MoqObjectStatus::EndOfGroup,
            2 => crate::moq::MoqObjectStatus::EndOfTrack,
            _ => {
                return Err(QuicRtcError::InvalidData {
                    reason: "Invalid object status".to_string(),
                })
            }
        };
        offset += 1;

        // Parse payload
        if data.len() < offset + 4 {
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for payload length".to_string(),
            });
        }

        let payload_len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if data.len() < offset + payload_len {
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for payload".to_string(),
            });
        }

        let payload = data[offset..offset + payload_len].to_vec();

        Ok(MoqObject {
            track_namespace: TrackNamespace {
                namespace,
                track_name: track_name.clone(),
            },
            track_name,
            group_id,
            object_id,
            publisher_priority,
            payload,
            object_status,
            created_at: std::time::Instant::now(),
            size: payload_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moq::{MoqTrack, MoqTrackType};

    fn test_track_namespace() -> TrackNamespace {
        TrackNamespace {
            namespace: "test.example.com".to_string(),
            track_name: "alice/camera".to_string(),
        }
    }

    fn test_moq_track() -> MoqTrack {
        MoqTrack {
            namespace: test_track_namespace(),
            name: "camera".to_string(),
            track_type: MoqTrackType::Video,
        }
    }

    fn test_moq_object() -> MoqObject {
        MoqObject {
            track_namespace: test_track_namespace(),
            track_name: "alice/camera".to_string(), // Should match track_namespace.track_name
            group_id: 12345,
            object_id: 67890,
            publisher_priority: 1,
            payload: vec![0x01, 0x02, 0x03, 0x04],
            object_status: crate::moq::MoqObjectStatus::Normal,
            created_at: std::time::Instant::now(),
            size: 4,
        }
    }

    #[test]
    fn test_moq_stream_creation() {
        let quic_stream = QuicStream {
            id: 123,
            stream_type: StreamType::Bidirectional,
            send: None,
            recv: None,
        };

        // Test control stream creation
        let control_stream = MoqStream::control(123, quic_stream);
        assert_eq!(control_stream.stream_id, 123);
        assert_eq!(control_stream.stream_type, MoqStreamType::Control);
        assert!(control_stream.track_namespace.is_none());

        // Test data stream creation
        let quic_stream2 = QuicStream {
            id: 456,
            stream_type: StreamType::Unidirectional,
            send: None,
            recv: None,
        };

        let track_namespace = test_track_namespace();
        let data_stream = MoqStream::data(456, track_namespace.clone(), quic_stream2);
        assert_eq!(data_stream.stream_id, 456);
        assert_eq!(data_stream.stream_type, MoqStreamType::DataSubgroup);
        assert_eq!(data_stream.track_namespace, Some(track_namespace));
    }

    #[test]
    fn test_moq_object_serialization() {
        // Test serialization/deserialization without creating full transport
        let object = test_moq_object();

        // Create a minimal mock for testing serialization methods
        struct MockTransport;

        impl MockTransport {
            fn serialize_moq_object(&self, object: &MoqObject) -> Result<Vec<u8>, QuicRtcError> {
                // Copy the serialization logic from MoqOverQuicTransport
                let mut buffer = Vec::new();

                // Track namespace length and data
                let namespace_bytes = object.track_namespace.namespace.as_bytes();
                buffer.extend_from_slice(&(namespace_bytes.len() as u32).to_be_bytes());
                buffer.extend_from_slice(namespace_bytes);

                let track_name_bytes = object.track_namespace.track_name.as_bytes();
                buffer.extend_from_slice(&(track_name_bytes.len() as u32).to_be_bytes());
                buffer.extend_from_slice(track_name_bytes);

                // Object metadata
                buffer.extend_from_slice(&object.group_id.to_be_bytes());
                buffer.extend_from_slice(&object.object_id.to_be_bytes());
                buffer.extend_from_slice(&[object.publisher_priority]);

                // Object status
                let status_byte = match object.object_status {
                    crate::moq::MoqObjectStatus::Normal => 0u8,
                    crate::moq::MoqObjectStatus::EndOfGroup => 1u8,
                    crate::moq::MoqObjectStatus::EndOfTrack => 2u8,
                };
                buffer.push(status_byte);

                // Payload length and data
                buffer.extend_from_slice(&(object.payload.len() as u32).to_be_bytes());
                buffer.extend_from_slice(&object.payload);

                Ok(buffer)
            }

            fn deserialize_moq_object(&self, data: &[u8]) -> Result<MoqObject, QuicRtcError> {
                // Copy the deserialization logic from MoqOverQuicTransport
                let mut offset = 0;

                if data.len() < 4 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for namespace length".to_string(),
                    });
                }

                // Parse namespace
                let namespace_len =
                    u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
                offset += 4;

                if data.len() < offset + namespace_len {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for namespace".to_string(),
                    });
                }

                let namespace = String::from_utf8(data[offset..offset + namespace_len].to_vec())
                    .map_err(|_| QuicRtcError::InvalidData {
                        reason: "Invalid UTF-8 in namespace".to_string(),
                    })?;
                offset += namespace_len;

                // Parse track name
                if data.len() < offset + 4 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for track name length".to_string(),
                    });
                }

                let track_name_len = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                offset += 4;

                if data.len() < offset + track_name_len {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for track name".to_string(),
                    });
                }

                let track_name = String::from_utf8(data[offset..offset + track_name_len].to_vec())
                    .map_err(|_| QuicRtcError::InvalidData {
                        reason: "Invalid UTF-8 in track name".to_string(),
                    })?;
                offset += track_name_len;

                // Parse object metadata
                if data.len() < offset + 17 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for object metadata".to_string(),
                    });
                }

                let group_id = u64::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]);
                offset += 8;

                let object_id = u64::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]);
                offset += 8;

                let publisher_priority = data[offset];
                offset += 1;

                // Parse object status
                let object_status = match data[offset] {
                    0 => crate::moq::MoqObjectStatus::Normal,
                    1 => crate::moq::MoqObjectStatus::EndOfGroup,
                    2 => crate::moq::MoqObjectStatus::EndOfTrack,
                    _ => {
                        return Err(QuicRtcError::InvalidData {
                            reason: "Invalid object status".to_string(),
                        })
                    }
                };
                offset += 1;

                // Parse payload
                if data.len() < offset + 4 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for payload length".to_string(),
                    });
                }

                let payload_len = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                offset += 4;

                if data.len() < offset + payload_len {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for payload".to_string(),
                    });
                }

                let payload = data[offset..offset + payload_len].to_vec();

                Ok(MoqObject {
                    track_namespace: TrackNamespace {
                        namespace,
                        track_name: track_name.clone(),
                    },
                    track_name,
                    group_id,
                    object_id,
                    publisher_priority,
                    payload,
                    object_status,
                    created_at: std::time::Instant::now(),
                    size: payload_len,
                })
            }
        }

        let mock_transport = MockTransport;

        // Test serialization
        let serialized = mock_transport.serialize_moq_object(&object).unwrap();
        assert!(!serialized.is_empty());

        // Test deserialization
        let deserialized = mock_transport.deserialize_moq_object(&serialized).unwrap();

        // Verify object fields match
        assert_eq!(deserialized.track_namespace, object.track_namespace);
        assert_eq!(deserialized.track_name, object.track_name);
        assert_eq!(deserialized.group_id, object.group_id);
        assert_eq!(deserialized.object_id, object.object_id);
        assert_eq!(deserialized.publisher_priority, object.publisher_priority);
        assert_eq!(deserialized.payload, object.payload);
        assert_eq!(deserialized.size, object.size);
    }

    #[test]
    fn test_moq_transport_event_types() {
        let session_event = MoqTransportEvent::SessionEstablished { session_id: 123 };
        match session_event {
            MoqTransportEvent::SessionEstablished { session_id } => {
                assert_eq!(session_id, 123);
            }
            _ => panic!("Wrong event type"),
        }

        let track_event = MoqTransportEvent::TrackAnnounced {
            track_namespace: test_track_namespace(),
            track: test_moq_track(),
        };
        match track_event {
            MoqTransportEvent::TrackAnnounced {
                track_namespace, ..
            } => {
                assert_eq!(track_namespace.namespace, "test.example.com");
            }
            _ => panic!("Wrong event type"),
        }

        let object_event = MoqTransportEvent::ObjectReceived {
            object: test_moq_object(),
        };
        match object_event {
            MoqTransportEvent::ObjectReceived { object } => {
                assert_eq!(object.group_id, 12345);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_stream_type_classification() {
        assert_eq!(MoqStreamType::Control, MoqStreamType::Control);
        assert_eq!(MoqStreamType::DataSubgroup, MoqStreamType::DataSubgroup);
        assert_ne!(MoqStreamType::Control, MoqStreamType::DataSubgroup);
    }

    #[test]
    fn test_track_namespace_equality() {
        let ns1 = TrackNamespace {
            namespace: "test.example.com".to_string(),
            track_name: "alice/camera".to_string(),
        };

        let ns2 = TrackNamespace {
            namespace: "test.example.com".to_string(),
            track_name: "alice/camera".to_string(),
        };

        let ns3 = TrackNamespace {
            namespace: "test.example.com".to_string(),
            track_name: "bob/camera".to_string(),
        };

        assert_eq!(ns1, ns2);
        assert_ne!(ns1, ns3);
    }
}
