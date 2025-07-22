//! Track management and abstractions

use quicrtc_core::{MoqTrack, TrackNamespace};
use std::time::Instant;
use tracing::{debug, info};

#[cfg(feature = "media")]
use quicrtc_media::{AudioTrack, VideoTrack};

/// Local track representation for tracks published by this participant
#[derive(Debug, Clone)]
pub struct LocalTrack {
    /// Track ID
    id: String,
    /// Track kind (audio/video)
    kind: TrackKind,
    /// Track source (camera, microphone, screen)
    source: TrackSource,
    /// MoQ track for transport
    moq_track: MoqTrack,
    /// Whether track is currently muted
    muted: bool,
    /// Track publication time
    published_at: Instant,
    /// Track state
    state: TrackState,
    /// Track settings
    settings: TrackSettings,
    /// Statistics
    stats: TrackStats,
}

impl LocalTrack {
    /// Create a new local video track
    #[cfg(feature = "media")]
    pub fn video(id: String, source: TrackSource, moq_track: MoqTrack) -> Self {
        info!(
            "📹 Creating local video track: {} (source: {:?})",
            id, source
        );
        Self {
            id,
            kind: TrackKind::Video,
            source,
            moq_track,
            muted: false,
            published_at: Instant::now(),
            state: TrackState::Ready,
            settings: TrackSettings::video_default(),
            stats: TrackStats::default(),
        }
    }

    /// Create a new local audio track
    #[cfg(feature = "media")]
    pub fn audio(id: String, source: TrackSource, moq_track: MoqTrack) -> Self {
        info!(
            "🎵 Creating local audio track: {} (source: {:?})",
            id, source
        );
        Self {
            id,
            kind: TrackKind::Audio,
            source,
            moq_track,
            muted: false,
            published_at: Instant::now(),
            state: TrackState::Ready,
            settings: TrackSettings::audio_default(),
            stats: TrackStats::default(),
        }
    }

    /// Get track ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get track kind
    pub fn kind(&self) -> TrackKind {
        self.kind
    }

    /// Get track source
    pub fn source(&self) -> TrackSource {
        self.source
    }

    /// Get MoQ track
    pub fn moq_track(&self) -> &MoqTrack {
        &self.moq_track
    }

    /// Check if track is muted
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Mute the track
    pub fn mute(&mut self) {
        if !self.muted {
            info!("🔇 Muting local track: {}", self.id);
            self.muted = true;
        }
    }

    /// Unmute the track
    pub fn unmute(&mut self) {
        if self.muted {
            info!("🔊 Unmuting local track: {}", self.id);
            self.muted = false;
        }
    }

    /// Toggle mute state
    pub fn toggle_mute(&mut self) {
        if self.muted {
            self.unmute();
        } else {
            self.mute();
        }
    }

    /// Get track state
    pub fn state(&self) -> TrackState {
        self.state
    }

    /// Set track state
    pub fn set_state(&mut self, state: TrackState) {
        if self.state != state {
            debug!(
                "🔄 Local track {} state changed: {:?} -> {:?}",
                self.id, self.state, state
            );
            self.state = state;
        }
    }

    /// Get track settings
    pub fn settings(&self) -> &TrackSettings {
        &self.settings
    }

    /// Set track settings
    pub fn set_settings(&mut self, settings: TrackSettings) {
        debug!("⚙️ Updating local track {} settings", self.id);
        self.settings = settings;
    }

    /// Get track statistics
    pub fn stats(&self) -> &TrackStats {
        &self.stats
    }

    /// Update track statistics
    pub fn update_stats(&mut self, stats: TrackStats) {
        self.stats = stats;
    }

    /// Get publication time
    pub fn published_at(&self) -> Instant {
        self.published_at
    }

    /// Get how long this track has been published
    pub fn publication_duration(&self) -> std::time::Duration {
        self.published_at.elapsed()
    }

    /// Check if track is ready for publishing
    pub fn is_ready(&self) -> bool {
        matches!(self.state, TrackState::Ready | TrackState::Publishing)
    }

    /// Check if track is currently publishing
    pub fn is_publishing(&self) -> bool {
        matches!(self.state, TrackState::Publishing)
    }

    /// Check if track has failed
    pub fn is_failed(&self) -> bool {
        matches!(self.state, TrackState::Failed)
    }
}

/// Remote track representation for tracks from other participants
#[derive(Debug, Clone)]
pub struct RemoteTrack {
    /// Track ID
    id: String,
    /// Participant ID that owns this track
    participant_id: String,
    /// Track kind (audio/video)
    kind: TrackKind,
    /// Track source (camera, microphone, screen)
    source: TrackSource,
    /// MoQ track for transport
    moq_track: MoqTrack,
    /// Whether track is currently muted by remote participant
    muted: bool,
    /// When this track was first received
    received_at: Instant,
    /// Track state
    state: TrackState,
    /// Track settings (as received from remote)
    settings: TrackSettings,
    /// Reception statistics
    stats: TrackStats,
}

impl RemoteTrack {
    /// Create a new remote video track
    #[cfg(feature = "media")]
    pub fn video(
        id: String,
        participant_id: String,
        source: TrackSource,
        moq_track: MoqTrack,
    ) -> Self {
        info!(
            "📺 Creating remote video track: {} from {}",
            id, participant_id
        );
        Self {
            id,
            participant_id,
            kind: TrackKind::Video,
            source,
            moq_track,
            muted: false,
            received_at: Instant::now(),
            state: TrackState::Receiving,
            settings: TrackSettings::video_default(),
            stats: TrackStats::default(),
        }
    }

    /// Create a new remote audio track
    #[cfg(feature = "media")]
    pub fn audio(
        id: String,
        participant_id: String,
        source: TrackSource,
        moq_track: MoqTrack,
    ) -> Self {
        info!(
            "🎵 Creating remote audio track: {} from {}",
            id, participant_id
        );
        Self {
            id,
            participant_id,
            kind: TrackKind::Audio,
            source,
            moq_track,
            muted: false,
            received_at: Instant::now(),
            state: TrackState::Receiving,
            settings: TrackSettings::audio_default(),
            stats: TrackStats::default(),
        }
    }

    /// Get track ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get participant ID
    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }

    /// Get track kind
    pub fn kind(&self) -> TrackKind {
        self.kind
    }

    /// Get track source
    pub fn source(&self) -> TrackSource {
        self.source
    }

    /// Get MoQ track
    pub fn moq_track(&self) -> &MoqTrack {
        &self.moq_track
    }

    /// Check if track is muted by the remote participant
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Set mute state (updated from remote participant)
    pub fn set_muted(&mut self, muted: bool) {
        if self.muted != muted {
            debug!(
                "🔇 Remote track {} mute state changed to: {}",
                self.id, muted
            );
            self.muted = muted;
        }
    }

    /// Get track state
    pub fn state(&self) -> TrackState {
        self.state
    }

    /// Set track state
    pub fn set_state(&mut self, state: TrackState) {
        if self.state != state {
            debug!(
                "🔄 Remote track {} state changed: {:?} -> {:?}",
                self.id, self.state, state
            );
            self.state = state;
        }
    }

    /// Get track settings
    pub fn settings(&self) -> &TrackSettings {
        &self.settings
    }

    /// Set track settings (from remote updates)
    pub fn set_settings(&mut self, settings: TrackSettings) {
        debug!("⚙️ Updating remote track {} settings", self.id);
        self.settings = settings;
    }

    /// Get track statistics
    pub fn stats(&self) -> &TrackStats {
        &self.stats
    }

    /// Update track statistics
    pub fn update_stats(&mut self, stats: TrackStats) {
        self.stats = stats;
    }

    /// Get reception time
    pub fn received_at(&self) -> Instant {
        self.received_at
    }

    /// Get how long we've been receiving this track
    pub fn reception_duration(&self) -> std::time::Duration {
        self.received_at.elapsed()
    }

    /// Check if track is actively receiving data
    pub fn is_receiving(&self) -> bool {
        matches!(self.state, TrackState::Receiving)
    }

    /// Check if track has failed
    pub fn is_failed(&self) -> bool {
        matches!(self.state, TrackState::Failed)
    }

    /// Check if track is paused
    pub fn is_paused(&self) -> bool {
        matches!(self.state, TrackState::Paused)
    }
}

/// Track kind enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    /// Audio track
    Audio,
    /// Video track
    Video,
}

impl std::fmt::Display for TrackKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackKind::Audio => write!(f, "audio"),
            TrackKind::Video => write!(f, "video"),
        }
    }
}

/// Track source enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSource {
    /// Camera/webcam video
    Camera,
    /// Microphone audio
    Microphone,
    /// Screen sharing video
    Screen,
    /// Application sharing video
    Application,
    /// System audio (like desktop audio)
    SystemAudio,
    /// File playback
    File,
    /// Other/unknown source
    Unknown,
}

impl std::fmt::Display for TrackSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackSource::Camera => write!(f, "camera"),
            TrackSource::Microphone => write!(f, "microphone"),
            TrackSource::Screen => write!(f, "screen"),
            TrackSource::Application => write!(f, "application"),
            TrackSource::SystemAudio => write!(f, "system_audio"),
            TrackSource::File => write!(f, "file"),
            TrackSource::Unknown => write!(f, "unknown"),
        }
    }
}

/// Track state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackState {
    /// Track is ready for operation
    Ready,
    /// Track is actively publishing (local tracks)
    Publishing,
    /// Track is actively receiving (remote tracks)
    Receiving,
    /// Track is paused
    Paused,
    /// Track has failed
    Failed,
    /// Track is ending
    Ending,
    /// Track has ended
    Ended,
}

/// Track settings for configuration
#[derive(Debug, Clone)]
pub struct TrackSettings {
    /// Maximum bitrate in bps
    pub max_bitrate: Option<u32>,
    /// Target bitrate in bps
    pub target_bitrate: Option<u32>,
    /// Maximum framerate (for video)
    pub max_framerate: Option<f64>,
    /// Target framerate (for video)
    pub target_framerate: Option<f64>,
    /// Maximum resolution (for video)
    pub max_resolution: Option<(u32, u32)>,
    /// Target resolution (for video)
    pub target_resolution: Option<(u32, u32)>,
    /// Enable adaptive bitrate
    pub adaptive_bitrate: bool,
    /// Enable degradation (resolution/framerate reduction under poor conditions)
    pub enable_degradation: bool,
}

impl TrackSettings {
    /// Default settings for video tracks
    pub fn video_default() -> Self {
        Self {
            max_bitrate: Some(2_000_000),    // 2 Mbps
            target_bitrate: Some(1_000_000), // 1 Mbps
            max_framerate: Some(30.0),
            target_framerate: Some(30.0),
            max_resolution: Some((1920, 1080)),
            target_resolution: Some((1280, 720)),
            adaptive_bitrate: true,
            enable_degradation: true,
        }
    }

    /// Default settings for audio tracks
    pub fn audio_default() -> Self {
        Self {
            max_bitrate: Some(128_000),   // 128 kbps
            target_bitrate: Some(64_000), // 64 kbps
            max_framerate: None,          // Not applicable to audio
            target_framerate: None,
            max_resolution: None, // Not applicable to audio
            target_resolution: None,
            adaptive_bitrate: true,
            enable_degradation: false, // Audio degradation is typically binary (on/off)
        }
    }

    /// High quality video settings
    pub fn video_high_quality() -> Self {
        Self {
            max_bitrate: Some(5_000_000),    // 5 Mbps
            target_bitrate: Some(3_000_000), // 3 Mbps
            max_framerate: Some(60.0),
            target_framerate: Some(30.0),
            max_resolution: Some((1920, 1080)),
            target_resolution: Some((1920, 1080)),
            adaptive_bitrate: true,
            enable_degradation: true,
        }
    }

    /// Low bandwidth settings for video
    pub fn video_low_bandwidth() -> Self {
        Self {
            max_bitrate: Some(500_000),    // 500 kbps
            target_bitrate: Some(300_000), // 300 kbps
            max_framerate: Some(15.0),
            target_framerate: Some(15.0),
            max_resolution: Some((640, 480)),
            target_resolution: Some((320, 240)),
            adaptive_bitrate: true,
            enable_degradation: true,
        }
    }

    /// High quality audio settings
    pub fn audio_high_quality() -> Self {
        Self {
            max_bitrate: Some(256_000),    // 256 kbps
            target_bitrate: Some(128_000), // 128 kbps
            max_framerate: None,
            target_framerate: None,
            max_resolution: None,
            target_resolution: None,
            adaptive_bitrate: true,
            enable_degradation: false,
        }
    }
}

impl Default for TrackSettings {
    fn default() -> Self {
        Self::video_default()
    }
}

/// Track statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct TrackStats {
    /// Total bytes sent/received
    pub bytes_transferred: u64,
    /// Total packets sent/received
    pub packets_transferred: u64,
    /// Total frames sent/received
    pub frames_transferred: u64,
    /// Packets lost
    pub packets_lost: u64,
    /// Average bitrate in bps
    pub avg_bitrate: u32,
    /// Current bitrate in bps
    pub current_bitrate: u32,
    /// Average frame rate
    pub avg_framerate: f64,
    /// Current frame rate
    pub current_framerate: f64,
    /// Current resolution (for video)
    pub current_resolution: Option<(u32, u32)>,
    /// Round-trip time in milliseconds
    pub rtt_ms: Option<f64>,
    /// Jitter in milliseconds
    pub jitter_ms: Option<f64>,
    /// Network quality score (0-100)
    pub quality_score: Option<u8>,
}

impl TrackStats {
    /// Calculate packet loss percentage
    pub fn packet_loss_percentage(&self) -> f64 {
        if self.packets_transferred == 0 {
            0.0
        } else {
            (self.packets_lost as f64 / (self.packets_transferred + self.packets_lost) as f64)
                * 100.0
        }
    }

    /// Check if stats indicate good quality
    pub fn is_good_quality(&self) -> bool {
        let packet_loss = self.packet_loss_percentage();
        let has_good_rtt = self.rtt_ms.map_or(true, |rtt| rtt < 100.0);
        let has_low_jitter = self.jitter_ms.map_or(true, |jitter| jitter < 30.0);

        packet_loss < 5.0 && has_good_rtt && has_low_jitter
    }

    /// Get overall quality rating
    pub fn quality_rating(&self) -> TrackQuality {
        if let Some(score) = self.quality_score {
            match score {
                80..=100 => TrackQuality::Excellent,
                60..=79 => TrackQuality::Good,
                40..=59 => TrackQuality::Fair,
                20..=39 => TrackQuality::Poor,
                0..=19 => TrackQuality::VeryPoor,
                _ => TrackQuality::Unknown,
            }
        } else if self.is_good_quality() {
            TrackQuality::Good
        } else {
            TrackQuality::Poor
        }
    }
}

/// Track quality rating
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackQuality {
    /// Quality unknown
    Unknown,
    /// Excellent quality
    Excellent,
    /// Good quality
    Good,
    /// Fair quality
    Fair,
    /// Poor quality
    Poor,
    /// Very poor quality
    VeryPoor,
}

/// Errors that can occur with track management
#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    /// Track not found
    #[error("Track with ID '{track_id}' not found")]
    TrackNotFound {
        /// The ID of the track that was not found
        track_id: String,
    },

    /// Invalid track configuration
    #[error("Invalid track configuration: {reason}")]
    InvalidConfiguration {
        /// The reason why the configuration is invalid
        reason: String,
    },

    /// Track state error
    #[error("Invalid track state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        /// The current state of the track
        from: TrackState,
        /// The target state that is invalid
        to: TrackState,
    },

    /// Track already exists
    #[error("Track with ID '{track_id}' already exists")]
    TrackAlreadyExists {
        /// The ID of the track that already exists
        track_id: String,
    },

    /// Media error
    #[cfg(feature = "media")]
    #[error("Media error: {0}")]
    MediaError(#[from] quicrtc_media::MediaError),

    /// MoQ transport error
    #[error("MoQ transport error: {0}")]
    TransportError(#[from] quicrtc_core::QuicRtcError),
}
