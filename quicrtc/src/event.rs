//! Event system for room and participant events

use crate::{LocalTrack, RemoteParticipant, RemoteTrack};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::debug;

/// Room events that can occur during a session
#[derive(Debug, Clone)]
pub enum Event {
    /// A participant joined the room
    ParticipantJoined {
        /// The participant that joined
        participant: RemoteParticipant,
    },
    /// A participant left the room
    ParticipantLeft {
        /// The participant that left
        participant: RemoteParticipant,
    },
    /// A participant's connection state changed
    ParticipantConnectionChanged {
        /// Participant ID
        participant_id: String,
        /// New connection state
        state: crate::participant::ParticipantConnectionState,
    },
    /// A participant started speaking
    ParticipantStartedSpeaking {
        /// Participant ID
        participant_id: String,
    },
    /// A participant stopped speaking
    ParticipantStoppedSpeaking {
        /// Participant ID
        participant_id: String,
    },
    /// A track was received from a remote participant
    TrackReceived {
        /// The track that was received
        track: RemoteTrack,
    },
    /// A track was removed/ended by a remote participant
    TrackRemoved {
        /// The track that was removed
        track: RemoteTrack,
    },
    /// A local track was published
    LocalTrackPublished {
        /// The local track that was published
        track: LocalTrack,
    },
    /// A local track was unpublished
    LocalTrackUnpublished {
        /// The local track that was unpublished
        track: LocalTrack,
    },
    /// A track's mute state changed
    TrackMuteChanged {
        /// Track ID
        track_id: String,
        /// Participant ID that owns the track
        participant_id: String,
        /// Whether the track is now muted
        muted: bool,
    },
    /// Room connection state changed
    RoomConnectionChanged {
        /// New connection state
        state: crate::room::RoomState,
    },
    /// Network quality changed for the room
    NetworkQualityChanged {
        /// Overall quality score (0-100)
        quality_score: u8,
        /// Detailed quality metrics
        metrics: NetworkQualityMetrics,
    },
    /// An error occurred in the room
    RoomError {
        /// Error that occurred
        error: String,
        /// Whether this error is recoverable
        recoverable: bool,
    },
    /// Room was disconnected
    RoomDisconnected {
        /// Reason for disconnection
        reason: String,
    },
    /// Room is reconnecting
    RoomReconnecting {
        /// Attempt number
        attempt: u32,
    },
    /// Room successfully reconnected
    RoomReconnected,
}

impl Event {
    /// Get the event type as a string
    pub fn event_type(&self) -> &'static str {
        match self {
            Event::ParticipantJoined { .. } => "participant_joined",
            Event::ParticipantLeft { .. } => "participant_left",
            Event::ParticipantConnectionChanged { .. } => "participant_connection_changed",
            Event::ParticipantStartedSpeaking { .. } => "participant_started_speaking",
            Event::ParticipantStoppedSpeaking { .. } => "participant_stopped_speaking",
            Event::TrackReceived { .. } => "track_received",
            Event::TrackRemoved { .. } => "track_removed",
            Event::LocalTrackPublished { .. } => "local_track_published",
            Event::LocalTrackUnpublished { .. } => "local_track_unpublished",
            Event::TrackMuteChanged { .. } => "track_mute_changed",
            Event::RoomConnectionChanged { .. } => "room_connection_changed",
            Event::NetworkQualityChanged { .. } => "network_quality_changed",
            Event::RoomError { .. } => "room_error",
            Event::RoomDisconnected { .. } => "room_disconnected",
            Event::RoomReconnecting { .. } => "room_reconnecting",
            Event::RoomReconnected => "room_reconnected",
        }
    }

    /// Check if this is a participant-related event
    pub fn is_participant_event(&self) -> bool {
        matches!(
            self,
            Event::ParticipantJoined { .. }
                | Event::ParticipantLeft { .. }
                | Event::ParticipantConnectionChanged { .. }
                | Event::ParticipantStartedSpeaking { .. }
                | Event::ParticipantStoppedSpeaking { .. }
        )
    }

    /// Check if this is a track-related event
    pub fn is_track_event(&self) -> bool {
        matches!(
            self,
            Event::TrackReceived { .. }
                | Event::TrackRemoved { .. }
                | Event::LocalTrackPublished { .. }
                | Event::LocalTrackUnpublished { .. }
                | Event::TrackMuteChanged { .. }
        )
    }

    /// Check if this is a connection-related event
    pub fn is_connection_event(&self) -> bool {
        matches!(
            self,
            Event::RoomConnectionChanged { .. }
                | Event::NetworkQualityChanged { .. }
                | Event::RoomDisconnected { .. }
                | Event::RoomReconnecting { .. }
                | Event::RoomReconnected
        )
    }

    /// Check if this is an error event
    pub fn is_error_event(&self) -> bool {
        matches!(self, Event::RoomError { .. })
    }
}

/// Network quality metrics for detailed analysis
#[derive(Debug, Clone)]
pub struct NetworkQualityMetrics {
    /// Overall quality score (0-100)
    pub overall_score: u8,
    /// Upload quality score (0-100)
    pub upload_score: u8,
    /// Download quality score (0-100)
    pub download_score: u8,
    /// Round-trip time in milliseconds
    pub rtt_ms: f64,
    /// Packet loss percentage (0-100)
    pub packet_loss_percentage: f64,
    /// Available bandwidth estimate in kbps
    pub available_bandwidth_kbps: u32,
    /// Current bandwidth usage in kbps
    pub current_bandwidth_kbps: u32,
}

impl NetworkQualityMetrics {
    /// Create metrics indicating excellent quality
    pub fn excellent() -> Self {
        Self {
            overall_score: 95,
            upload_score: 95,
            download_score: 95,
            rtt_ms: 20.0,
            packet_loss_percentage: 0.1,
            available_bandwidth_kbps: 5000,
            current_bandwidth_kbps: 1000,
        }
    }

    /// Create metrics indicating poor quality
    pub fn poor() -> Self {
        Self {
            overall_score: 25,
            upload_score: 30,
            download_score: 20,
            rtt_ms: 200.0,
            packet_loss_percentage: 8.0,
            available_bandwidth_kbps: 500,
            current_bandwidth_kbps: 400,
        }
    }

    /// Get quality rating based on overall score
    pub fn quality_rating(&self) -> QualityRating {
        match self.overall_score {
            80..=100 => QualityRating::Excellent,
            60..=79 => QualityRating::Good,
            40..=59 => QualityRating::Fair,
            20..=39 => QualityRating::Poor,
            0..=19 => QualityRating::VeryPoor,
            _ => QualityRating::Unknown,
        }
    }
}

/// Quality rating enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityRating {
    /// Connection quality is unknown
    Unknown,
    /// Excellent connection quality
    Excellent,
    /// Good connection quality
    Good,
    /// Fair connection quality
    Fair,
    /// Poor connection quality
    Poor,
    /// Very poor connection quality
    VeryPoor,
}

/// Stream of room events for async iteration
#[derive(Debug)]
pub struct EventStream {
    /// Receiver for events
    receiver: mpsc::UnboundedReceiver<Event>,
    /// Optional room inner reference for cleanup
    _room_inner: Option<Arc<RwLock<crate::room::RoomInner>>>,
}

impl EventStream {
    /// Create a new event stream with a receiver
    pub fn new(receiver: mpsc::UnboundedReceiver<Event>) -> Self {
        Self {
            receiver,
            _room_inner: None,
        }
    }

    /// Create an event stream from a room's internal state
    pub fn from_room(room_inner: Arc<RwLock<crate::room::RoomInner>>) -> Self {
        // Create a new receiver channel for this event stream
        let (_tx, rx): (mpsc::UnboundedSender<Event>, mpsc::UnboundedReceiver<Event>) =
            mpsc::unbounded_channel();

        // In a real implementation, we would start a background task that
        // forwards events from the room's event channel to this receiver.
        // For now, we'll create a placeholder stream.

        Self {
            receiver: rx,
            _room_inner: Some(room_inner),
        }
    }

    /// Get the next event from the stream
    pub async fn next(&mut self) -> Option<Event> {
        self.receiver.recv().await
    }

    /// Try to get the next event without blocking
    pub fn try_next(&mut self) -> Result<Option<Event>, mpsc::error::TryRecvError> {
        match self.receiver.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(mpsc::error::TryRecvError::Disconnected)
            }
        }
    }

    /// Close the event stream
    pub fn close(&mut self) {
        self.receiver.close();
    }

    /// Check if the event stream is closed
    pub fn is_closed(&self) -> bool {
        self.receiver.is_closed()
    }
}

/// Event handler for callback-style event processing
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender
    event_tx: mpsc::UnboundedSender<Event>,
    /// Background task handle
    _task_handle: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    /// Create a new event handler with callback functions
    pub fn new<F>(mut callback: F) -> Self
    where
        F: FnMut(Event) + Send + 'static,
    {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

        let task_handle = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                debug!("📡 Processing event: {}", event.event_type());
                callback(event);
            }
        });

        Self {
            event_tx,
            _task_handle: task_handle,
        }
    }

    /// Send an event to the handler
    pub fn send_event(&self, event: Event) -> Result<(), mpsc::error::SendError<Event>> {
        self.event_tx.send(event)
    }

    /// Get a sender for events
    pub fn sender(&self) -> mpsc::UnboundedSender<Event> {
        self.event_tx.clone()
    }
}

/// Event filter for selective event processing
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// Whether to include participant events
    pub include_participant_events: bool,
    /// Whether to include track events
    pub include_track_events: bool,
    /// Whether to include connection events
    pub include_connection_events: bool,
    /// Whether to include error events
    pub include_error_events: bool,
    /// Specific event types to include (if specified, overrides other filters)
    pub specific_event_types: Option<Vec<String>>,
}

impl EventFilter {
    /// Create a filter that includes all events
    pub fn all() -> Self {
        Self {
            include_participant_events: true,
            include_track_events: true,
            include_connection_events: true,
            include_error_events: true,
            specific_event_types: None,
        }
    }

    /// Create a filter that includes only participant events
    pub fn participant_only() -> Self {
        Self {
            include_participant_events: true,
            include_track_events: false,
            include_connection_events: false,
            include_error_events: false,
            specific_event_types: None,
        }
    }

    /// Create a filter that includes only track events
    pub fn track_only() -> Self {
        Self {
            include_participant_events: false,
            include_track_events: true,
            include_connection_events: false,
            include_error_events: false,
            specific_event_types: None,
        }
    }

    /// Create a filter that includes only connection events
    pub fn connection_only() -> Self {
        Self {
            include_participant_events: false,
            include_track_events: false,
            include_connection_events: true,
            include_error_events: false,
            specific_event_types: None,
        }
    }

    /// Create a filter for specific event types
    pub fn specific(event_types: Vec<String>) -> Self {
        Self {
            include_participant_events: false,
            include_track_events: false,
            include_connection_events: false,
            include_error_events: false,
            specific_event_types: Some(event_types),
        }
    }

    /// Check if an event should be included based on this filter
    pub fn should_include(&self, event: &Event) -> bool {
        // If specific event types are specified, use those
        if let Some(ref specific_types) = self.specific_event_types {
            return specific_types.contains(&event.event_type().to_string());
        }

        // Otherwise, use category-based filtering
        (self.include_participant_events && event.is_participant_event())
            || (self.include_track_events && event.is_track_event())
            || (self.include_connection_events && event.is_connection_event())
            || (self.include_error_events && event.is_error_event())
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::all()
    }
}

/// Filtered event stream that only yields events matching a filter
#[derive(Debug)]
pub struct FilteredEventStream {
    /// Underlying event stream
    stream: EventStream,
    /// Event filter
    filter: EventFilter,
}

impl FilteredEventStream {
    /// Create a new filtered event stream
    pub fn new(stream: EventStream, filter: EventFilter) -> Self {
        Self { stream, filter }
    }

    /// Get the next event that matches the filter
    pub async fn next(&mut self) -> Option<Event> {
        loop {
            match self.stream.next().await {
                Some(event) => {
                    if self.filter.should_include(&event) {
                        return Some(event);
                    }
                    // Continue to next event if this one doesn't match filter
                }
                None => return None,
            }
        }
    }

    /// Try to get the next filtered event without blocking
    pub fn try_next(&mut self) -> Result<Option<Event>, mpsc::error::TryRecvError> {
        loop {
            match self.stream.try_next()? {
                Some(event) => {
                    if self.filter.should_include(&event) {
                        return Ok(Some(event));
                    }
                    // Continue to next event if this one doesn't match filter
                }
                None => return Ok(None),
            }
        }
    }

    /// Update the filter
    pub fn set_filter(&mut self, filter: EventFilter) {
        self.filter = filter;
    }

    /// Get the current filter
    pub fn filter(&self) -> &EventFilter {
        &self.filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalTrack, RemoteParticipant, RemoteTrack};
    use quicrtc_core::{MoqTrack, MoqTrackType, TrackNamespace};

    fn create_test_remote_participant() -> RemoteParticipant {
        RemoteParticipant::new("test-participant".to_string())
    }

    fn create_test_remote_track() -> RemoteTrack {
        let moq_track = MoqTrack {
            namespace: TrackNamespace {
                namespace: "test.room".to_string(),
                track_name: "test/video".to_string(),
            },
            name: "video".to_string(),
            track_type: MoqTrackType::Video,
        };

        RemoteTrack::video(
            "test-track".to_string(),
            "test-participant".to_string(),
            crate::track::TrackSource::Camera,
            moq_track,
        )
    }

    fn create_test_local_track() -> LocalTrack {
        let moq_track = MoqTrack {
            namespace: TrackNamespace {
                namespace: "test.room".to_string(),
                track_name: "local/video".to_string(),
            },
            name: "video".to_string(),
            track_type: MoqTrackType::Video,
        };

        LocalTrack::video(
            "local-track".to_string(),
            crate::track::TrackSource::Camera,
            moq_track,
        )
    }

    #[test]
    fn test_event_type_classification() {
        let participant = create_test_remote_participant();
        let track = create_test_remote_track();

        let participant_event = Event::ParticipantJoined {
            participant: participant.clone(),
        };
        assert!(participant_event.is_participant_event());
        assert!(!participant_event.is_track_event());
        assert!(!participant_event.is_connection_event());

        let track_event = Event::TrackReceived { track };
        assert!(!track_event.is_participant_event());
        assert!(track_event.is_track_event());
        assert!(!track_event.is_connection_event());

        let connection_event = Event::RoomReconnected;
        assert!(!connection_event.is_participant_event());
        assert!(!connection_event.is_track_event());
        assert!(connection_event.is_connection_event());

        let error_event = Event::RoomError {
            error: "Test error".to_string(),
            recoverable: true,
        };
        assert!(error_event.is_error_event());
        assert!(!error_event.is_connection_event());
    }

    #[test]
    fn test_event_filter() {
        let participant = create_test_remote_participant();
        let track = create_test_remote_track();

        let participant_event = Event::ParticipantJoined {
            participant: participant.clone(),
        };
        let track_event = Event::TrackReceived { track };
        let connection_event = Event::RoomReconnected;

        // Test all events filter
        let all_filter = EventFilter::all();
        assert!(all_filter.should_include(&participant_event));
        assert!(all_filter.should_include(&track_event));
        assert!(all_filter.should_include(&connection_event));

        // Test participant-only filter
        let participant_filter = EventFilter::participant_only();
        assert!(participant_filter.should_include(&participant_event));
        assert!(!participant_filter.should_include(&track_event));
        assert!(!participant_filter.should_include(&connection_event));

        // Test specific event types filter
        let specific_filter = EventFilter::specific(vec!["participant_joined".to_string()]);
        assert!(specific_filter.should_include(&participant_event));
        assert!(!specific_filter.should_include(&track_event));
        assert!(!specific_filter.should_include(&connection_event));
    }

    #[test]
    fn test_network_quality_metrics() {
        let excellent = NetworkQualityMetrics::excellent();
        assert_eq!(excellent.quality_rating(), QualityRating::Excellent);

        let poor = NetworkQualityMetrics::poor();
        assert_eq!(poor.quality_rating(), QualityRating::Poor);

        let custom = NetworkQualityMetrics {
            overall_score: 65,
            upload_score: 70,
            download_score: 60,
            rtt_ms: 80.0,
            packet_loss_percentage: 2.0,
            available_bandwidth_kbps: 2000,
            current_bandwidth_kbps: 800,
        };
        assert_eq!(custom.quality_rating(), QualityRating::Good);
    }

    #[tokio::test]
    async fn test_event_stream_basic() {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut event_stream = EventStream::new(rx);

        let participant = create_test_remote_participant();
        let event = Event::ParticipantJoined {
            participant: participant.clone(),
        };

        // Send event
        tx.send(event.clone()).unwrap();

        // Receive event
        let received_event = event_stream.next().await.unwrap();
        assert_eq!(received_event.event_type(), "participant_joined");
    }

    #[tokio::test]
    async fn test_filtered_event_stream() {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_stream = EventStream::new(rx);
        let filter = EventFilter::participant_only();
        let mut filtered_stream = FilteredEventStream::new(event_stream, filter);

        let participant = create_test_remote_participant();
        let track = create_test_remote_track();

        let participant_event = Event::ParticipantJoined {
            participant: participant.clone(),
        };
        let track_event = Event::TrackReceived { track };

        // Send both events
        tx.send(participant_event.clone()).unwrap();
        tx.send(track_event).unwrap();

        // Should only receive the participant event
        let received_event = filtered_stream.next().await.unwrap();
        assert_eq!(received_event.event_type(), "participant_joined");

        // Should not receive any more events (track event was filtered out)
        tx.send(Event::RoomReconnected).unwrap();
        assert!(filtered_stream.try_next().unwrap().is_none());
    }
}
