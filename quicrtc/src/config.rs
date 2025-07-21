//! Configuration types and defaults

#[cfg(feature = "media")]
use crate::VideoQuality;

/// Global QUIC RTC configuration
#[derive(Debug, Clone)]
pub struct GlobalConfig {
    /// Enable debug logging
    pub debug_logging: bool,
    /// Maximum number of concurrent rooms
    pub max_rooms: usize,
    /// Default signaling server URL
    pub default_signaling_url: Option<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            debug_logging: false,
            max_rooms: 10,
            default_signaling_url: None,
        }
    }
}

/// Room-specific configuration
#[derive(Debug, Clone)]
pub struct RoomConfig {
    /// Enable video
    pub video_enabled: bool,
    /// Enable audio
    pub audio_enabled: bool,
    /// Video quality preset
    #[cfg(feature = "media")]
    pub video_quality: VideoQuality,
    /// Signaling server URL
    pub signaling_url: Option<String>,
    /// Enable mobile optimizations
    pub mobile_optimizations: bool,
}

impl Default for RoomConfig {
    fn default() -> Self {
        Self {
            video_enabled: false,
            audio_enabled: false,
            #[cfg(feature = "media")]
            video_quality: VideoQuality::Standard,
            signaling_url: None,
            mobile_optimizations: false,
        }
    }
}