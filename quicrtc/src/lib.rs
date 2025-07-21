//! # QUIC RTC - Next-Generation Real-Time Communication
//!
//! QUIC RTC is a real-time communication library that implements the IETF Media over QUIC (MoQ)
//! standard to provide performance improvements and developer experience enhancements compared
//! to traditional WebRTC solutions.
//!
//! ## Key Features
//!
//! - **Pure MoQ over QUIC**: No RTP layer - direct media object delivery
//! - **Mobile-First Design**: Built-in connection migration and battery optimization
//! - **Simple API**: Clean, async Rust API that eliminates WebRTC complexity
//! - **Standards Compliant**: Full IETF Media over QUIC implementation
//! - **Cross-Platform**: Native support for mobile, desktop, and web
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use quicrtc::QuicRtc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize QUIC RTC
//!     let quic_rtc = QuicRtc::init()?;
//!     
//!     // Join a room with video and audio
//!     let mut room = quic_rtc
//!         .room("my-room")
//!         .participant("alice")
//!         .enable_video()
//!         .enable_audio()
//!         .join().await?;
//!     
//!     // Publish camera and microphone
//!     let _video_track = room.publish_camera().await?;
//!     let _audio_track = room.publish_microphone().await?;
//!     
//!     // Handle events
//!     let mut events = room.events();
//!     while let Some(event) = events.next().await {
//!         println!("Room event: {:?}", event);
//!     }
//!     
//!     Ok(())
//! }
//! ```

#![deny(missing_docs)]
#![warn(clippy::all)]

// Re-export core types for easy access
pub use quicrtc_core::{
    ConnectionConfig, ConnectionPool, ConnectionPoolConfig, H264Frame, MoqCacheConfig,
    MoqCacheStats, MoqDeliveryStats, MoqObject, MoqObjectCache, MoqObjectDelivery, MoqObjectStatus,
    MoqSession, MoqTrack, NetworkPath, OpusFrame, QuicRtcError, ResourceLimits, ResourceManager,
    ResourceUsage, ResourceWarning, TrackNamespace, TransportConnection, TransportMode,
    WarningSeverity,
};

#[cfg(feature = "media")]
pub use quicrtc_media::{
    codecs::{Codec, CodecInfo, VideoQuality},
    tracks::{AudioTrack, MediaFrame, VideoTrack},
};

#[cfg(feature = "signaling")]
pub use quicrtc_signaling::{PeerDiscovery, SignalingServer};

#[cfg(feature = "diagnostics")]
pub use quicrtc_diagnostics::{ConnectionInfo, ConnectionStats, NetworkProfiler};

// Public API modules
pub mod config;
pub mod event;
pub mod participant;
pub mod room;
pub mod track;

// Re-export main API types
pub use config::{CodecConfig, GlobalConfig, RoomConfig};

#[cfg(feature = "media")]
pub use config::{AudioProcessingConfig, MediaConfig, VideoProcessingConfig};

#[cfg(feature = "signaling")]
pub use config::{ReconnectConfig, SignalingConfig};

pub use event::{Event, EventStream};
pub use participant::{LocalParticipant, Participants, RemoteParticipant};
pub use room::{Room, RoomBuilder};
pub use track::{LocalTrack, RemoteTrack};

/// Main entry point for QUIC RTC
#[derive(Debug, Clone)]
pub struct QuicRtc {
    inner: std::sync::Arc<QuicRtcInner>,
}

#[derive(Debug)]
struct QuicRtcInner {
    /// Global configuration
    config: GlobalConfig,
    /// Resource manager for connection limits and monitoring
    resource_manager: std::sync::Arc<ResourceManager>,
    /// Resource warning receiver
    _warning_receiver: tokio::sync::mpsc::UnboundedReceiver<ResourceWarning>,
    /// Codec registry for media processing
    #[cfg(feature = "media")]
    codec_registry: std::sync::Arc<quicrtc_media::CodecRegistry>,
    /// Peer discovery service
    #[cfg(feature = "signaling")]
    peer_discovery: std::sync::Arc<quicrtc_signaling::PeerDiscovery>,
    /// Background task handles for cleanup
    _background_tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl QuicRtc {
    /// Initialize QUIC RTC with default settings
    ///
    /// This performs full initialization of all subsystems:
    /// - Transport layer and connection pooling
    /// - Codec registry (Opus, H.264)
    /// - Resource management
    /// - Signaling and peer discovery
    /// - Media capture/render systems
    ///
    /// # Example
    /// ```rust,no_run
    /// use quicrtc::QuicRtc;
    ///
    /// # async fn example() -> Result<(), quicrtc::QuicRtcError> {
    /// let quic_rtc = QuicRtc::init().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn init() -> Result<Self, QuicRtcError> {
        Self::init_with(GlobalConfig::default()).await
    }

    /// Initialize with custom global configuration
    ///
    /// # Example
    /// ```rust,no_run
    /// use quicrtc::{QuicRtc, GlobalConfig, ResourceLimits};
    ///
    /// # async fn example() -> Result<(), quicrtc::QuicRtcError> {
    /// let config = GlobalConfig {
    ///     debug_logging: true,
    ///     resource_limits: ResourceLimits::mobile(),
    ///     ..Default::default()
    /// };
    /// let quic_rtc = QuicRtc::init_with(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn init_with(config: GlobalConfig) -> Result<Self, QuicRtcError> {
        tracing::info!("🚀 Initializing QUIC RTC with configuration: {:?}", config);

        // Initialize logging if requested
        if config.debug_logging {
            Self::init_logging()?;
        }

        // 1. Initialize resource management
        tracing::debug!("📊 Initializing resource manager");
        let (resource_manager, warning_receiver) =
            ResourceManager::new(config.resource_limits.clone());
        let resource_manager = std::sync::Arc::new(resource_manager);

        // 2. Initialize codec registry
        #[cfg(feature = "media")]
        let codec_registry = {
            tracing::debug!("🎵 Initializing codec registry");
            Self::init_codec_registry(&config.codec_config)?
        };

        // 3. Initialize peer discovery
        #[cfg(feature = "signaling")]
        let peer_discovery = {
            tracing::debug!("🔍 Initializing peer discovery");
            Self::init_peer_discovery(&config.signaling_config)?
        };

        // 4. Initialize media systems
        #[cfg(feature = "media")]
        {
            tracing::debug!("🎥 Initializing media systems");
            Self::init_media_systems(&config.media_config)?;
        }

        // 5. Start background tasks
        tracing::debug!("⚙️ Starting background maintenance tasks");
        let background_tasks = Self::start_background_tasks(
            std::sync::Arc::clone(&resource_manager),
            #[cfg(feature = "signaling")]
            std::sync::Arc::clone(&peer_discovery),
        )
        .await?;

        tracing::info!("✅ QUIC RTC initialization complete");

        Ok(Self {
            inner: std::sync::Arc::new(QuicRtcInner {
                config,
                resource_manager,
                _warning_receiver: warning_receiver,
                #[cfg(feature = "media")]
                codec_registry,
                #[cfg(feature = "signaling")]
                peer_discovery,
                _background_tasks: background_tasks,
            }),
        })
    }

    /// Initialize logging system
    fn init_logging() -> Result<(), QuicRtcError> {
        // Only initialize if not already initialized
        let _ = tracing_subscriber::fmt()
            .with_env_filter("quicrtc=debug,info")
            .try_init();
        Ok(())
    }

    /// Initialize codec registry with configured codecs
    #[cfg(feature = "media")]
    fn init_codec_registry(
        config: &CodecConfig,
    ) -> Result<std::sync::Arc<quicrtc_media::CodecRegistry>, QuicRtcError> {
        let mut registry = quicrtc_media::CodecRegistry::new();

        if config.enable_opus {
            let opus_config = quicrtc_media::codecs::OpusConfig {
                sample_rate: config.default_audio_sample_rate,
                channels: 2, // Stereo by default
                bitrate: config.default_audio_bitrate,
                frame_duration_ms: 20,
            };
            let opus_codec =
                std::sync::Arc::new(quicrtc_media::codecs::OpusCodec::with_config(opus_config)?);
            registry.register_codec("opus", opus_codec)?;
            tracing::debug!("✅ Registered Opus codec");
        }

        if config.enable_h264 {
            let h264_config = quicrtc_media::codecs::H264Config {
                width: 1280,
                height: 720,
                bitrate: config.default_video_bitrate,
                framerate: 30,
            };
            let h264_codec =
                std::sync::Arc::new(quicrtc_media::codecs::H264Codec::with_config(h264_config)?);
            registry.register_codec("h264", h264_codec)?;
            tracing::debug!("✅ Registered H.264 codec");
        }

        Ok(std::sync::Arc::new(registry))
    }

    /// Initialize peer discovery service
    #[cfg(feature = "signaling")]
    fn init_peer_discovery(
        config: &SignalingConfig,
    ) -> Result<std::sync::Arc<quicrtc_signaling::PeerDiscovery>, QuicRtcError> {
        let discovery_config = quicrtc_signaling::DiscoveryConfig {
            cleanup_interval: config.heartbeat_interval.as_secs(),
            offline_timeout: config.connection_timeout.as_secs(),
            max_peers_per_room: 100, // Default limit
        };

        let discovery = std::sync::Arc::new(quicrtc_signaling::PeerDiscovery::new_with_config(
            discovery_config,
        ));
        Ok(discovery)
    }

    /// Initialize media capture and render systems
    #[cfg(feature = "media")]
    fn init_media_systems(config: &MediaConfig) -> Result<(), QuicRtcError> {
        if config.enumerate_devices_on_startup {
            // Initialize video capture system to enumerate devices
            tracing::debug!("📹 Enumerating video capture devices");
            let capture_manager = quicrtc_media::VideoCaptureManager::new().map_err(|e| {
                QuicRtcError::Initialization {
                    reason: format!("Failed to initialize video capture: {}", e),
                }
            })?;

            match capture_manager.enumerate_devices() {
                Ok(devices) => {
                    tracing::info!("📹 Found {} video capture devices", devices.len());
                    for (i, device) in devices.iter().enumerate() {
                        tracing::debug!("  {}. {} ({})", i + 1, device.name, device.id);
                    }
                }
                Err(e) => {
                    tracing::warn!("⚠️ Failed to enumerate video devices: {}", e);
                }
            }
        }

        tracing::debug!("✅ Media systems initialized");
        Ok(())
    }

    /// Start background maintenance tasks
    async fn start_background_tasks(
        resource_manager: std::sync::Arc<ResourceManager>,
        #[cfg(feature = "signaling")] peer_discovery: std::sync::Arc<
            quicrtc_signaling::PeerDiscovery,
        >,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>, QuicRtcError> {
        let mut tasks = Vec::new();

        // Resource monitoring task
        {
            let resource_manager = std::sync::Arc::clone(&resource_manager);
            let task = tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;

                    // Check resource usage and cleanup if needed
                    if let Err(e) = resource_manager.check_limits() {
                        tracing::warn!("⚠️ Resource limit check failed: {}", e);
                    }

                    let warnings = resource_manager.approaching_limits();
                    if !warnings.is_empty() {
                        tracing::warn!("⚠️ Resource warnings: {:?}", warnings);
                    }
                }
            });
            tasks.push(task);
        }

        // Peer discovery service task
        #[cfg(feature = "signaling")]
        {
            peer_discovery
                .start()
                .await
                .map_err(|e| QuicRtcError::Initialization {
                    reason: format!("Failed to start peer discovery: {}", e),
                })?;
            tracing::debug!("✅ Peer discovery service started");
        }

        tracing::debug!("✅ Started {} background tasks", tasks.len());
        Ok(tasks)
    }

    /// Get resource manager (for monitoring)
    pub fn resource_manager(&self) -> &ResourceManager {
        &self.inner.resource_manager
    }

    /// Get codec registry (for advanced codec operations)
    #[cfg(feature = "media")]
    pub fn codec_registry(&self) -> &quicrtc_media::CodecRegistry {
        &self.inner.codec_registry
    }

    /// Get peer discovery service (for manual peer management)
    #[cfg(feature = "signaling")]
    pub fn peer_discovery(&self) -> &quicrtc_signaling::PeerDiscovery {
        &self.inner.peer_discovery
    }

    /// Create a room builder for the given room ID
    ///
    /// # Example
    /// ```rust,no_run
    /// use quicrtc::QuicRtc;
    ///
    /// # async fn example() -> Result<(), quicrtc::QuicRtcError> {
    /// let quic_rtc = QuicRtc::init()?;
    /// let room = quic_rtc
    ///     .room("my-room")
    ///     .participant("alice")
    ///     .enable_video()
    ///     .join().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn room(&self, id: &str) -> RoomBuilder {
        RoomBuilder::new(self, id)
    }
}
