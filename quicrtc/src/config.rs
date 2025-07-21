//! Configuration types and defaults

#[cfg(feature = "media")]
use crate::VideoQuality;
use crate::{ConnectionPoolConfig, ResourceLimits};
use std::time::Duration;

/// Global QUIC RTC configuration
#[derive(Debug, Clone)]
pub struct GlobalConfig {
    /// Enable debug logging
    pub debug_logging: bool,
    /// Maximum number of concurrent rooms
    pub max_rooms: usize,
    /// Default signaling server URL
    pub default_signaling_url: Option<String>,
    /// Resource limits for transport layer
    pub resource_limits: ResourceLimits,
    /// Connection pool configuration
    pub connection_pool: ConnectionPoolConfig,
    /// Codec preferences and settings
    pub codec_config: CodecConfig,
    /// Media system configuration
    #[cfg(feature = "media")]
    pub media_config: MediaConfig,
    /// Signaling system configuration
    #[cfg(feature = "signaling")]
    pub signaling_config: SignalingConfig,
}

/// Codec system configuration
#[derive(Debug, Clone)]
pub struct CodecConfig {
    /// Enable Opus audio codec
    pub enable_opus: bool,
    /// Enable H.264 video codec
    pub enable_h264: bool,
    /// Default audio sample rate
    pub default_audio_sample_rate: u32,
    /// Default audio bitrate (bps)
    pub default_audio_bitrate: u32,
    /// Default video bitrate (bps)
    pub default_video_bitrate: u32,
    /// Enable hardware acceleration when available
    pub enable_hardware_acceleration: bool,
}

/// Media system configuration
#[cfg(feature = "media")]
#[derive(Debug, Clone)]
pub struct MediaConfig {
    /// Enable automatic device enumeration on startup
    pub enumerate_devices_on_startup: bool,
    /// Default video quality
    pub default_video_quality: VideoQuality,
    /// Maximum video capture resolution
    pub max_video_resolution: (u32, u32),
    /// Audio processing settings
    pub audio_processing: AudioProcessingConfig,
    /// Video processing settings
    pub video_processing: VideoProcessingConfig,
}

/// Audio processing configuration
#[cfg(feature = "media")]
#[derive(Debug, Clone)]
pub struct AudioProcessingConfig {
    /// Enable echo cancellation
    pub enable_echo_cancellation: bool,
    /// Enable noise suppression
    pub enable_noise_suppression: bool,
    /// Audio buffer size
    pub buffer_size: usize,
    /// Audio render volume (0.0 to 1.0)
    pub default_volume: f32,
}

/// Video processing configuration
#[cfg(feature = "media")]
#[derive(Debug, Clone)]
pub struct VideoProcessingConfig {
    /// Enable automatic exposure adjustment
    pub enable_auto_exposure: bool,
    /// Enable automatic white balance
    pub enable_auto_white_balance: bool,
    /// Default framerate
    pub default_framerate: f64,
    /// Enable video preprocessing
    pub enable_preprocessing: bool,
}

/// Signaling system configuration
#[cfg(feature = "signaling")]
#[derive(Debug, Clone)]
pub struct SignalingConfig {
    /// Connection timeout for signaling server
    pub connection_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Reconnection attempt configuration
    pub reconnect_config: ReconnectConfig,
    /// Enable automatic peer discovery
    pub enable_peer_discovery: bool,
}

/// Reconnection configuration
#[cfg(feature = "signaling")]
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Enable automatic reconnection
    pub enabled: bool,
    /// Initial retry delay
    pub initial_delay: Duration,
    /// Maximum retry delay
    pub max_delay: Duration,
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
    /// Maximum number of retry attempts
    pub max_attempts: u32,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            debug_logging: false,
            max_rooms: 10,
            default_signaling_url: None,
            resource_limits: ResourceLimits::desktop(),
            connection_pool: ConnectionPoolConfig::default(),
            codec_config: CodecConfig::default(),
            #[cfg(feature = "media")]
            media_config: MediaConfig::default(),
            #[cfg(feature = "signaling")]
            signaling_config: SignalingConfig::default(),
        }
    }
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            enable_opus: true,
            enable_h264: true,
            default_audio_sample_rate: 48000,
            default_audio_bitrate: 64000,
            default_video_bitrate: 1_000_000,
            enable_hardware_acceleration: true,
        }
    }
}

#[cfg(feature = "media")]
impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            enumerate_devices_on_startup: true,
            default_video_quality: VideoQuality::Standard,
            max_video_resolution: (1920, 1080),
            audio_processing: AudioProcessingConfig::default(),
            video_processing: VideoProcessingConfig::default(),
        }
    }
}

#[cfg(feature = "media")]
impl Default for AudioProcessingConfig {
    fn default() -> Self {
        Self {
            enable_echo_cancellation: true,
            enable_noise_suppression: true,
            buffer_size: 960, // 20ms at 48kHz
            default_volume: 0.8,
        }
    }
}

#[cfg(feature = "media")]
impl Default for VideoProcessingConfig {
    fn default() -> Self {
        Self {
            enable_auto_exposure: true,
            enable_auto_white_balance: true,
            default_framerate: 30.0,
            enable_preprocessing: true,
        }
    }
}

#[cfg(feature = "signaling")]
impl Default for SignalingConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(10),
            heartbeat_interval: Duration::from_secs(30),
            reconnect_config: ReconnectConfig::default(),
            enable_peer_discovery: true,
        }
    }
}

#[cfg(feature = "signaling")]
impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            max_attempts: 5,
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
