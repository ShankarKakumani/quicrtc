//! Participant management and abstractions

use crate::{LocalTrack, RemoteTrack, RoomConfig};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Collection of participants in a room
#[derive(Debug)]
pub struct Participants {
    /// Remote participants by ID
    remote_participants: HashMap<String, RemoteParticipant>,
    /// Maximum number of participants allowed
    max_participants: Option<usize>,
    /// When this collection was created
    created_at: Instant,
}

impl Participants {
    /// Create a new empty participants collection
    pub fn new() -> Self {
        Self {
            remote_participants: HashMap::new(),
            max_participants: None,
            created_at: Instant::now(),
        }
    }

    /// Create a new participants collection with maximum size
    pub fn with_max_participants(max: usize) -> Self {
        Self {
            remote_participants: HashMap::new(),
            max_participants: Some(max),
            created_at: Instant::now(),
        }
    }

    /// Add a remote participant
    pub fn add_remote_participant(
        &mut self,
        participant: RemoteParticipant,
    ) -> Result<(), ParticipantError> {
        // Check if we've reached the maximum
        if let Some(max) = self.max_participants {
            if self.remote_participants.len() >= max {
                return Err(ParticipantError::MaximumParticipantsExceeded { max });
            }
        }

        // Check if participant already exists
        if self.remote_participants.contains_key(&participant.id) {
            return Err(ParticipantError::ParticipantAlreadyExists {
                participant_id: participant.id.clone(),
            });
        }

        info!("👥 Adding remote participant: {}", participant.id);
        self.remote_participants
            .insert(participant.id.clone(), participant);
        Ok(())
    }

    /// Remove a remote participant
    pub fn remove_remote_participant(&mut self, participant_id: &str) -> Option<RemoteParticipant> {
        info!("👋 Removing remote participant: {}", participant_id);
        self.remote_participants.remove(participant_id)
    }

    /// Get a remote participant by ID
    pub fn get_remote_participant(&self, participant_id: &str) -> Option<&RemoteParticipant> {
        self.remote_participants.get(participant_id)
    }

    /// Get a mutable reference to a remote participant by ID
    pub fn get_remote_participant_mut(
        &mut self,
        participant_id: &str,
    ) -> Option<&mut RemoteParticipant> {
        self.remote_participants.get_mut(participant_id)
    }

    /// Get all remote participants
    pub fn remote_participants(&self) -> impl Iterator<Item = &RemoteParticipant> {
        self.remote_participants.values()
    }

    /// Get total participant count (remote only, local participant is tracked separately)
    pub fn count(&self) -> usize {
        self.remote_participants.len()
    }

    /// Check if the room is at maximum capacity
    pub fn is_at_capacity(&self) -> bool {
        if let Some(max) = self.max_participants {
            self.remote_participants.len() >= max
        } else {
            false
        }
    }

    /// Get maximum participants limit
    pub fn max_participants(&self) -> Option<usize> {
        self.max_participants
    }

    /// Set maximum participants limit
    pub fn set_max_participants(&mut self, max: Option<usize>) {
        self.max_participants = max;
    }

    /// Check if a participant exists
    pub fn contains_participant(&self, participant_id: &str) -> bool {
        self.remote_participants.contains_key(participant_id)
    }

    /// Get participant IDs
    pub fn participant_ids(&self) -> impl Iterator<Item = &String> {
        self.remote_participants.keys()
    }

    /// Clear all participants
    pub fn clear(&mut self) {
        info!("🧹 Clearing all remote participants");
        self.remote_participants.clear();
    }
}

impl Default for Participants {
    fn default() -> Self {
        Self::new()
    }
}

/// Local participant representation
#[derive(Debug)]
pub struct LocalParticipant {
    /// Participant ID
    id: String,
    /// Display name
    name: Option<String>,
    /// Room configuration when this participant was created
    room_config: RoomConfig,
    /// Local tracks published by this participant
    local_tracks: HashMap<String, LocalTrack>,
    /// Participant metadata
    metadata: HashMap<String, String>,
    /// When this participant was created
    created_at: Instant,
    /// Connection state
    connection_state: ParticipantConnectionState,
    /// Speaking state for audio tracks
    is_speaking: bool,
    /// Whether this participant is muted (for audio)
    is_muted: bool,
    /// Whether this participant's video is disabled
    video_disabled: bool,
}

impl LocalParticipant {
    /// Create a new local participant
    pub fn new(id: String, room_config: RoomConfig) -> Self {
        info!("👤 Creating local participant: {}", id);
        Self {
            id,
            name: None,
            room_config,
            local_tracks: HashMap::new(),
            metadata: HashMap::new(),
            created_at: Instant::now(),
            connection_state: ParticipantConnectionState::Connected,
            is_speaking: false,
            is_muted: false,
            video_disabled: false,
        }
    }

    /// Create a new local participant with display name
    pub fn new_with_name(id: String, name: String, room_config: RoomConfig) -> Self {
        let mut participant = Self::new(id, room_config);
        participant.name = Some(name);
        participant
    }

    /// Get participant ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get display name
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Set display name
    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    /// Get room configuration
    pub fn room_config(&self) -> &RoomConfig {
        &self.room_config
    }

    /// Add a local track
    pub fn add_local_track(&mut self, track: LocalTrack) {
        debug!("🎵 Adding local track: {}", track.id());
        self.local_tracks.insert(track.id().to_string(), track);
    }

    /// Remove a local track
    pub fn remove_local_track(&mut self, track_id: &str) -> Option<LocalTrack> {
        debug!("🗑️ Removing local track: {}", track_id);
        self.local_tracks.remove(track_id)
    }

    /// Get a local track by ID
    pub fn get_local_track(&self, track_id: &str) -> Option<&LocalTrack> {
        self.local_tracks.get(track_id)
    }

    /// Get all local tracks
    pub fn local_tracks(&self) -> impl Iterator<Item = &LocalTrack> {
        self.local_tracks.values()
    }

    /// Get metadata
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Remove metadata value
    pub fn remove_metadata(&mut self, key: &str) -> Option<String> {
        self.metadata.remove(key)
    }

    /// Get connection state
    pub fn connection_state(&self) -> ParticipantConnectionState {
        self.connection_state
    }

    /// Set connection state
    pub fn set_connection_state(&mut self, state: ParticipantConnectionState) {
        if self.connection_state != state {
            debug!(
                "🔄 Local participant connection state changed: {:?} -> {:?}",
                self.connection_state, state
            );
            self.connection_state = state;
        }
    }

    /// Check if participant is speaking
    pub fn is_speaking(&self) -> bool {
        self.is_speaking
    }

    /// Set speaking state
    pub fn set_speaking(&mut self, speaking: bool) {
        if self.is_speaking != speaking {
            debug!("🗣️ Local participant speaking state changed: {}", speaking);
            self.is_speaking = speaking;
        }
    }

    /// Check if participant is muted
    pub fn is_muted(&self) -> bool {
        self.is_muted
    }

    /// Set muted state
    pub fn set_muted(&mut self, muted: bool) {
        if self.is_muted != muted {
            info!("🔇 Local participant muted state changed: {}", muted);
            self.is_muted = muted;
        }
    }

    /// Check if video is disabled
    pub fn is_video_disabled(&self) -> bool {
        self.video_disabled
    }

    /// Set video disabled state
    pub fn set_video_disabled(&mut self, disabled: bool) {
        if self.video_disabled != disabled {
            info!(
                "📹 Local participant video disabled state changed: {}",
                disabled
            );
            self.video_disabled = disabled;
        }
    }

    /// Get creation time
    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    /// Get how long this participant has been in the room
    pub fn duration_in_room(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Remote participant representation
#[derive(Debug, Clone)]
pub struct RemoteParticipant {
    /// Participant ID
    id: String,
    /// Display name
    name: Option<String>,
    /// Remote tracks from this participant
    remote_tracks: HashMap<String, RemoteTrack>,
    /// Participant metadata
    metadata: HashMap<String, String>,
    /// When this participant joined
    joined_at: Instant,
    /// Last time we received data from this participant
    last_seen: Option<Instant>,
    /// Connection state
    connection_state: ParticipantConnectionState,
    /// Connection quality
    connection_quality: ConnectionQuality,
    /// Speaking state for audio tracks
    is_speaking: bool,
    /// Whether this participant is muted (for audio)
    is_muted: bool,
    /// Whether this participant's video is disabled
    video_disabled: bool,
}

impl RemoteParticipant {
    /// Create a new remote participant
    pub fn new(id: String) -> Self {
        info!("👥 Creating remote participant: {}", id);
        Self {
            id,
            name: None,
            remote_tracks: HashMap::new(),
            metadata: HashMap::new(),
            joined_at: Instant::now(),
            last_seen: None,
            connection_state: ParticipantConnectionState::Connecting,
            connection_quality: ConnectionQuality::Unknown,
            is_speaking: false,
            is_muted: false,
            video_disabled: false,
        }
    }

    /// Create a new remote participant with display name
    pub fn new_with_name(id: String, name: String) -> Self {
        let mut participant = Self::new(id);
        participant.name = Some(name);
        participant
    }

    /// Get participant ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get display name
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Set display name
    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    /// Add a remote track
    pub fn add_remote_track(&mut self, track: RemoteTrack) {
        debug!("📺 Adding remote track: {}", track.id());
        self.remote_tracks.insert(track.id().to_string(), track);
    }

    /// Remove a remote track
    pub fn remove_remote_track(&mut self, track_id: &str) -> Option<RemoteTrack> {
        debug!("🗑️ Removing remote track: {}", track_id);
        self.remote_tracks.remove(track_id)
    }

    /// Get a remote track by ID
    pub fn get_remote_track(&self, track_id: &str) -> Option<&RemoteTrack> {
        self.remote_tracks.get(track_id)
    }

    /// Get all remote tracks
    pub fn remote_tracks(&self) -> impl Iterator<Item = &RemoteTrack> {
        self.remote_tracks.values()
    }

    /// Get metadata
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Remove metadata value
    pub fn remove_metadata(&mut self, key: &str) -> Option<String> {
        self.metadata.remove(key)
    }

    /// Get joined time
    pub fn joined_at(&self) -> Instant {
        self.joined_at
    }

    /// Get last seen time
    pub fn last_seen(&self) -> Option<Instant> {
        self.last_seen
    }

    /// Update last seen time
    pub fn update_last_seen(&mut self) {
        self.last_seen = Some(Instant::now());
    }

    /// Get how long this participant has been in the room
    pub fn duration_in_room(&self) -> Duration {
        self.joined_at.elapsed()
    }

    /// Get connection state
    pub fn connection_state(&self) -> ParticipantConnectionState {
        self.connection_state
    }

    /// Set connection state
    pub fn set_connection_state(&mut self, state: ParticipantConnectionState) {
        if self.connection_state != state {
            debug!(
                "🔄 Remote participant {} connection state changed: {:?} -> {:?}",
                self.id, self.connection_state, state
            );
            self.connection_state = state;
        }
    }

    /// Get connection quality
    pub fn connection_quality(&self) -> ConnectionQuality {
        self.connection_quality
    }

    /// Set connection quality
    pub fn set_connection_quality(&mut self, quality: ConnectionQuality) {
        if self.connection_quality != quality {
            debug!(
                "📊 Remote participant {} connection quality changed: {:?} -> {:?}",
                self.id, self.connection_quality, quality
            );
            self.connection_quality = quality;
        }
    }

    /// Check if participant is speaking
    pub fn is_speaking(&self) -> bool {
        self.is_speaking
    }

    /// Set speaking state
    pub fn set_speaking(&mut self, speaking: bool) {
        if self.is_speaking != speaking {
            debug!(
                "🗣️ Remote participant {} speaking state changed: {}",
                self.id, speaking
            );
            self.is_speaking = speaking;
        }
    }

    /// Check if participant is muted
    pub fn is_muted(&self) -> bool {
        self.is_muted
    }

    /// Set muted state
    pub fn set_muted(&mut self, muted: bool) {
        if self.is_muted != muted {
            debug!(
                "🔇 Remote participant {} muted state changed: {}",
                self.id, muted
            );
            self.is_muted = muted;
        }
    }

    /// Check if video is disabled
    pub fn is_video_disabled(&self) -> bool {
        self.video_disabled
    }

    /// Set video disabled state
    pub fn set_video_disabled(&mut self, disabled: bool) {
        if self.video_disabled != disabled {
            debug!(
                "📹 Remote participant {} video disabled state changed: {}",
                self.id, disabled
            );
            self.video_disabled = disabled;
        }
    }

    /// Check if participant is actively connected
    pub fn is_connected(&self) -> bool {
        matches!(self.connection_state, ParticipantConnectionState::Connected)
    }

    /// Check if participant has been seen recently (within the last 30 seconds)
    pub fn is_active(&self) -> bool {
        if let Some(last_seen) = self.last_seen {
            last_seen.elapsed() < Duration::from_secs(30)
        } else {
            false
        }
    }
}

/// Participant connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantConnectionState {
    /// Participant is connecting
    Connecting,
    /// Participant is connected and active
    Connected,
    /// Participant is reconnecting after connection loss
    Reconnecting,
    /// Participant has disconnected
    Disconnected,
    /// Participant connection failed
    Failed,
}

/// Connection quality rating
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionQuality {
    /// Quality unknown or not measured yet
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

/// Errors that can occur with participant management
#[derive(Debug, thiserror::Error)]
pub enum ParticipantError {
    /// Maximum number of participants exceeded
    #[error("Maximum number of participants ({max}) exceeded")]
    MaximumParticipantsExceeded {
        /// The maximum number of participants allowed
        max: usize,
    },

    /// Participant already exists
    #[error("Participant with ID '{participant_id}' already exists")]
    ParticipantAlreadyExists {
        /// The ID of the participant that already exists
        participant_id: String,
    },

    /// Participant not found
    #[error("Participant with ID '{participant_id}' not found")]
    ParticipantNotFound {
        /// The ID of the participant that was not found
        participant_id: String,
    },

    /// Invalid participant ID
    #[error("Invalid participant ID: {reason}")]
    InvalidParticipantId {
        /// The reason why the participant ID is invalid
        reason: String,
    },
}
