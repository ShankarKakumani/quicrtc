//! Room management and API

use crate::{QuicRtc, QuicRtcError, RoomConfig};
#[cfg(feature = "media")]
use crate::VideoQuality;
use std::sync::Arc;

/// Fluent builder for room configuration and connection
#[derive(Debug)]
pub struct RoomBuilder {
    quic_rtc: Arc<QuicRtc>,
    room_id: String,
    participant_id: Option<String>,
    config: RoomConfig,
}

impl RoomBuilder {
    pub(crate) fn new(quic_rtc: &QuicRtc, room_id: &str) -> Self {
        Self {
            quic_rtc: Arc::new(quic_rtc.clone()),
            room_id: room_id.to_string(),
            participant_id: None,
            config: RoomConfig::default(),
        }
    }
    
    /// Set participant ID (required)
    pub fn participant(mut self, id: &str) -> Self {
        self.participant_id = Some(id.to_string());
        self
    }
    
    /// Enable video with default settings
    pub fn enable_video(mut self) -> Self {
        self.config.video_enabled = true;
        self
    }
    
    /// Enable audio with default settings  
    pub fn enable_audio(mut self) -> Self {
        self.config.audio_enabled = true;
        self
    }
    
    /// Set video quality preset
    #[cfg(feature = "media")]
    pub fn video_quality(mut self, quality: VideoQuality) -> Self {
        self.config.video_quality = quality;
        self
    }
    
    /// Set signaling server URL
    pub fn signaling_server(mut self, url: &str) -> Self {
        self.config.signaling_url = Some(url.to_string());
        self
    }
    
    /// Enable mobile optimizations
    pub fn mobile_optimized(mut self) -> Self {
        self.config.mobile_optimizations = true;
        self
    }
    
    /// Join the room with current configuration
    pub async fn join(self) -> Result<Room, QuicRtcError> {
        let participant_id = self.participant_id
            .ok_or_else(|| QuicRtcError::MissingConfiguration { 
                field: "participant_id".to_string() 
            })?;
            
        Room::join_internal(self.quic_rtc, self.room_id, participant_id, self.config).await
    }
    
    /// Create a new room (if it doesn't exist) and join
    pub async fn create_and_join(self) -> Result<Room, QuicRtcError> {
        // TODO: Implement room creation logic
        self.join().await
    }
}

/// A Room represents a real-time communication session
#[derive(Debug)]
pub struct Room {
    id: String,
    participant_id: String,
    // TODO: Add internal room state
}

impl Room {
    /// Quick join - simplest possible API
    pub async fn quick_join(room_id: &str, participant_id: &str) -> Result<Self, QuicRtcError> {
        QuicRtc::init()?
            .room(room_id)
            .participant(participant_id)
            .enable_video()
            .enable_audio()
            .join().await
    }
    
    pub(crate) async fn join_internal(
        _quic_rtc: Arc<QuicRtc>,
        room_id: String,
        participant_id: String,
        _config: RoomConfig,
    ) -> Result<Self, QuicRtcError> {
        // TODO: Implement actual room joining logic
        Ok(Self {
            id: room_id,
            participant_id,
        })
    }
    
    /// Get room ID
    pub fn id(&self) -> &str {
        &self.id
    }
    
    /// Get participant ID
    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }
    
    /// Publish camera with default settings (stub implementation)
    #[cfg(feature = "media")]
    pub async fn publish_camera(&mut self) -> Result<crate::VideoTrack, crate::QuicRtcError> {
        // TODO: Implement camera publishing
        Ok(crate::VideoTrack::new("camera".to_string()))
    }
    
    /// Publish microphone with default settings (stub implementation)
    #[cfg(feature = "media")]
    pub async fn publish_microphone(&mut self) -> Result<crate::AudioTrack, crate::QuicRtcError> {
        // TODO: Implement microphone publishing
        Ok(crate::AudioTrack::new("microphone".to_string()))
    }
    
    /// Get event stream (stub implementation)
    pub fn events(&self) -> crate::EventStream {
        // TODO: Implement event stream
        crate::EventStream::new()
    }
}