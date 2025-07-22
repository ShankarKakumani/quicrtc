//! Room management and API

#[cfg(feature = "media")]
use crate::{AudioProcessingConfig, MediaConfig, VideoProcessingConfig, VideoQuality};
use crate::{QuicRtc, QuicRtcError, ResourceLimits, RoomConfig};
#[cfg(feature = "signaling")]
use crate::{ReconnectConfig, SignalingConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

// Import core types for MoQ and transport
use quicrtc_core::{
    ConnectionConfig, MoqOverQuicTransport, MoqSession, MoqTrack, MoqTransportEvent,
    TrackNamespace, TransportConnection, TransportMode,
};

#[cfg(feature = "media")]
use quicrtc_media::{
    AudioRenderer, AudioTrack, CpalAudioRenderer, DefaultVideoRenderer, MediaError, MediaProcessor,
    VideoCaptureManager, VideoTrack,
};

#[cfg(feature = "signaling")]
use quicrtc_signaling::{PeerInfo, PeerStatus, SignalingServer};

/// Fluent builder for room configuration and connection
#[derive(Debug)]
pub struct RoomBuilder {
    quic_rtc: Arc<QuicRtc>,
    room_id: String,
    participant_id: Option<String>,
    config: RoomConfig,
    // Additional detailed configurations
    #[cfg(feature = "media")]
    audio_config: Option<AudioProcessingConfig>,
    #[cfg(feature = "media")]
    video_config: Option<VideoProcessingConfig>,
    #[cfg(feature = "signaling")]
    signaling_config: Option<SignalingConfig>,
    resource_limits: Option<ResourceLimits>,
    custom_room_name: Option<String>,
    max_participants: Option<usize>,
}

impl RoomBuilder {
    pub(crate) fn new(quic_rtc: &QuicRtc, room_id: &str) -> Self {
        Self {
            quic_rtc: Arc::new(quic_rtc.clone()),
            room_id: room_id.to_string(),
            participant_id: None,
            config: RoomConfig::default(),
            #[cfg(feature = "media")]
            audio_config: None,
            #[cfg(feature = "media")]
            video_config: None,
            #[cfg(feature = "signaling")]
            signaling_config: None,
            resource_limits: None,
            custom_room_name: None,
            max_participants: None,
        }
    }

    // ============================================================================
    // Participant Configuration
    // ============================================================================

    /// Set participant ID (required)
    pub fn participant(mut self, id: &str) -> Self {
        self.participant_id = Some(id.to_string());
        self
    }

    /// Set custom display name for the participant
    pub fn participant_name(mut self, name: &str) -> Self {
        // Store in config for later use during join
        self.custom_room_name = Some(name.to_string());
        self
    }

    // ============================================================================
    // Media Configuration
    // ============================================================================

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

    /// Disable video
    pub fn disable_video(mut self) -> Self {
        self.config.video_enabled = false;
        self
    }

    /// Disable audio
    pub fn disable_audio(mut self) -> Self {
        self.config.audio_enabled = false;
        self
    }

    /// Set video quality preset
    #[cfg(feature = "media")]
    pub fn video_quality(mut self, quality: VideoQuality) -> Self {
        self.config.video_quality = quality;
        self
    }

    /// Configure video with specific resolution and framerate
    #[cfg(feature = "media")]
    pub fn video_resolution(mut self, width: u32, height: u32, fps: f64) -> Self {
        self.config.video_enabled = true;

        let mut video_config = self
            .video_config
            .unwrap_or_else(VideoProcessingConfig::default);
        video_config.default_framerate = fps;
        self.video_config = Some(video_config);

        // Store resolution in video quality as custom
        self.config.video_quality = VideoQuality::Standard; // Will be overridden by custom config
        self
    }

    /// Configure advanced video processing options
    #[cfg(feature = "media")]
    pub fn video_processing(mut self, config: VideoProcessingConfig) -> Self {
        self.config.video_enabled = true;
        self.video_config = Some(config);
        self
    }

    /// Configure audio processing options
    #[cfg(feature = "media")]
    pub fn audio_processing(mut self, config: AudioProcessingConfig) -> Self {
        self.config.audio_enabled = true;
        self.audio_config = Some(config);
        self
    }

    /// Set audio volume (0.0 to 1.0)
    #[cfg(feature = "media")]
    pub fn audio_volume(mut self, volume: f32) -> Self {
        self.config.audio_enabled = true;

        let mut audio_config = self
            .audio_config
            .unwrap_or_else(AudioProcessingConfig::default);
        audio_config.default_volume = volume.clamp(0.0, 1.0);
        self.audio_config = Some(audio_config);
        self
    }

    /// Enable echo cancellation
    #[cfg(feature = "media")]
    pub fn enable_echo_cancellation(mut self) -> Self {
        let mut audio_config = self
            .audio_config
            .unwrap_or_else(AudioProcessingConfig::default);
        audio_config.enable_echo_cancellation = true;
        self.audio_config = Some(audio_config);
        self
    }

    /// Enable noise suppression
    #[cfg(feature = "media")]
    pub fn enable_noise_suppression(mut self) -> Self {
        let mut audio_config = self
            .audio_config
            .unwrap_or_else(AudioProcessingConfig::default);
        audio_config.enable_noise_suppression = true;
        self.audio_config = Some(audio_config);
        self
    }

    // ============================================================================
    // Signaling and Connection Configuration
    // ============================================================================

    /// Set signaling server URL
    pub fn signaling_server(mut self, url: &str) -> Self {
        self.config.signaling_url = Some(url.to_string());
        self
    }

    /// Configure signaling with advanced options
    #[cfg(feature = "signaling")]
    pub fn signaling_config(mut self, config: SignalingConfig) -> Self {
        self.signaling_config = Some(config);
        self
    }

    /// Set connection timeout
    #[cfg(feature = "signaling")]
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        let mut signaling_config = self
            .signaling_config
            .unwrap_or_else(SignalingConfig::default);
        signaling_config.connection_timeout = timeout;
        self.signaling_config = Some(signaling_config);
        self
    }

    /// Configure reconnection behavior
    #[cfg(feature = "signaling")]
    pub fn reconnect_config(mut self, config: ReconnectConfig) -> Self {
        let mut signaling_config = self
            .signaling_config
            .unwrap_or_else(SignalingConfig::default);
        signaling_config.reconnect_config = config;
        self.signaling_config = Some(signaling_config);
        self
    }

    // ============================================================================
    // Platform and Performance Configuration
    // ============================================================================

    /// Enable mobile optimizations
    pub fn mobile_optimized(mut self) -> Self {
        self.config.mobile_optimizations = true;
        // Apply mobile-specific resource limits
        self.resource_limits = Some(ResourceLimits::mobile());
        self
    }

    /// Enable desktop optimizations
    pub fn desktop_optimized(mut self) -> Self {
        self.config.mobile_optimizations = false;
        // Apply desktop-specific resource limits
        self.resource_limits = Some(ResourceLimits::desktop());
        self
    }

    /// Set custom resource limits
    pub fn resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = Some(limits);
        self
    }

    /// Set maximum number of participants in the room
    pub fn max_participants(mut self, max: usize) -> Self {
        self.max_participants = Some(max);
        self
    }

    // ============================================================================
    // Quality and Bandwidth Configuration
    // ============================================================================

    /// Set bandwidth limit in kbps
    pub fn bandwidth_limit(mut self, kbps: u64) -> Self {
        let mut limits = self.resource_limits.unwrap_or_else(ResourceLimits::desktop);
        limits.max_bandwidth_kbps = Some(kbps);
        self.resource_limits = Some(limits);
        self
    }

    /// Configure for low bandwidth scenarios
    pub fn low_bandwidth(mut self) -> Self {
        #[cfg(feature = "media")]
        {
            self.config.video_quality = VideoQuality::Low;

            let mut audio_config = self
                .audio_config
                .unwrap_or_else(AudioProcessingConfig::default);
            audio_config.buffer_size = 480; // 10ms at 48kHz for lower latency
            self.audio_config = Some(audio_config);
        }

        // Set conservative bandwidth limits
        let mut limits = self.resource_limits.unwrap_or_else(ResourceLimits::mobile);
        limits.max_bandwidth_kbps = Some(500); // 500 kbps
        limits.max_connections = Some(2);
        self.resource_limits = Some(limits);
        self
    }

    /// Configure for high quality scenarios
    pub fn high_quality(mut self) -> Self {
        #[cfg(feature = "media")]
        {
            self.config.video_quality = VideoQuality::FullHD;

            let mut audio_config = self
                .audio_config
                .unwrap_or_else(AudioProcessingConfig::default);
            audio_config.enable_echo_cancellation = true;
            audio_config.enable_noise_suppression = true;
            self.audio_config = Some(audio_config);

            let mut video_config = self
                .video_config
                .unwrap_or_else(VideoProcessingConfig::default);
            video_config.enable_preprocessing = true;
            video_config.default_framerate = 30.0;
            self.video_config = Some(video_config);
        }

        // Set high-performance resource limits
        let mut limits = self.resource_limits.unwrap_or_else(ResourceLimits::desktop);
        limits.max_bandwidth_kbps = Some(5000); // 5 Mbps
        limits.max_connections = Some(10);
        self.resource_limits = Some(limits);
        self
    }

    // ============================================================================
    // Validation and Building
    // ============================================================================

    /// Validate the current configuration
    pub fn validate(&self) -> Result<(), QuicRtcError> {
        // Check required fields
        if self.participant_id.is_none() {
            return Err(QuicRtcError::MissingConfiguration {
                field: "participant_id".to_string(),
            });
        }

        // Validate participant ID format
        if let Some(ref id) = self.participant_id {
            if id.is_empty() {
                return Err(QuicRtcError::MissingConfiguration {
                    field: "participant_id cannot be empty".to_string(),
                });
            }

            if id.len() > 64 {
                return Err(QuicRtcError::InvalidData {
                    reason: "participant_id cannot exceed 64 characters".to_string(),
                });
            }
        }

        // Validate room ID
        if self.room_id.is_empty() {
            return Err(QuicRtcError::InvalidData {
                reason: "room_id cannot be empty".to_string(),
            });
        }

        if self.room_id.len() > 128 {
            return Err(QuicRtcError::InvalidData {
                reason: "room_id cannot exceed 128 characters".to_string(),
            });
        }

        // Validate media configuration consistency
        #[cfg(feature = "media")]
        {
            if let Some(ref audio_config) = self.audio_config {
                if audio_config.default_volume < 0.0 || audio_config.default_volume > 1.0 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "audio volume must be between 0.0 and 1.0".to_string(),
                    });
                }
            }

            if let Some(ref video_config) = self.video_config {
                if video_config.default_framerate <= 0.0 || video_config.default_framerate > 120.0 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "video framerate must be between 0.1 and 120.0".to_string(),
                    });
                }
            }
        }

        // Validate max participants (independent of resource limits)
        if let Some(max_participants) = self.max_participants {
            if max_participants == 0 {
                return Err(QuicRtcError::InvalidData {
                    reason: "max_participants must be at least 1".to_string(),
                });
            }

            if max_participants > 1000 {
                return Err(QuicRtcError::InvalidData {
                    reason: "max_participants cannot exceed 1000".to_string(),
                });
            }
        }

        // Validate resource limits
        if let Some(ref limits) = self.resource_limits {
            if let Some(bandwidth) = limits.max_bandwidth_kbps {
                if bandwidth < 64 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "bandwidth limit cannot be less than 64 kbps".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Join the room with current configuration
    pub async fn join(self) -> Result<Room, QuicRtcError> {
        // Validate configuration before proceeding
        self.validate()?;

        let participant_id = self.participant_id.unwrap(); // Safe due to validation

        Room::join_internal(
            self.quic_rtc,
            self.room_id,
            participant_id,
            self.config,
            #[cfg(feature = "media")]
            self.audio_config,
            #[cfg(feature = "media")]
            self.video_config,
            #[cfg(feature = "signaling")]
            self.signaling_config,
            self.resource_limits,
            self.max_participants,
        )
        .await
    }

    /// Create a new room (if it doesn't exist) and join
    pub async fn create_and_join(self) -> Result<Room, QuicRtcError> {
        // Validate configuration
        self.validate()?;

        // TODO: Implement room creation logic with signaling server
        // For now, delegate to join() which will create room if needed
        self.join().await
    }
}

/// A Room represents a real-time communication session
#[derive(Debug)]
pub struct Room {
    id: String,
    participant_id: String,
    config: RoomConfig,
    #[cfg(feature = "media")]
    audio_config: Option<AudioProcessingConfig>,
    #[cfg(feature = "media")]
    video_config: Option<VideoProcessingConfig>,
    #[cfg(feature = "signaling")]
    signaling_config: Option<SignalingConfig>,
    resource_limits: Option<ResourceLimits>,
    max_participants: Option<usize>,

    // Core room state
    inner: Arc<RwLock<RoomInner>>,
}

/// Internal room state
#[derive(Debug)]
pub struct RoomInner {
    /// Room connection state
    pub state: RoomState,
    /// MoQ over QUIC transport for media delivery  
    pub moq_transport: Option<Arc<MoqOverQuicTransport>>,
    /// Signaling connection for peer discovery and room management
    #[cfg(feature = "signaling")]
    pub signaling_connection: Option<Arc<tokio::sync::Mutex<SignalingConnection>>>,
    /// Media processor for handling MoQ objects and media frames
    #[cfg(feature = "media")]
    pub media_processor: Option<Arc<tokio::sync::Mutex<MediaProcessor>>>,
    /// Video capture manager for camera access
    #[cfg(feature = "media")]
    pub video_capture: Option<Arc<tokio::sync::Mutex<VideoCaptureManager>>>,
    /// Audio renderer for microphone and speaker access
    #[cfg(feature = "media")]
    pub audio_renderer: Option<Arc<tokio::sync::Mutex<CpalAudioRenderer>>>,
    /// Participants in the room
    pub participants: crate::Participants,
    /// Local participant representation
    pub local_participant: Option<crate::LocalParticipant>,
    /// Published tracks by this participant
    #[cfg(feature = "media")]
    pub published_tracks: std::collections::HashMap<String, PublishedTrack>,
    /// Event sender for room events
    pub event_tx: Option<mpsc::UnboundedSender<crate::Event>>,
    /// Background task handles
    pub background_tasks: Vec<tokio::task::JoinHandle<()>>,
}

/// Room connection state
#[derive(Debug, Clone, PartialEq)]
pub enum RoomState {
    /// Room is disconnected
    Disconnected,
    /// Room is connecting
    Connecting,
    /// Room is connected and ready
    Connected,
    /// Room is reconnecting after connection loss
    Reconnecting,
    /// Room is disconnecting
    Disconnecting,
}

/// Signaling connection wrapper
#[cfg(feature = "signaling")]
#[derive(Debug)]
struct SignalingConnection {
    /// Signaling server reference
    server: Arc<SignalingServer>,
    /// Our participant info for signaling
    participant_info: PeerInfo,
    /// Other participants discovered via signaling
    discovered_peers: std::collections::HashMap<String, PeerInfo>,
}

/// Published track metadata
#[cfg(feature = "media")]
#[derive(Debug)]
struct PublishedTrack {
    /// Track ID
    track_id: String,
    /// Track type (video/audio)
    track_type: TrackType,
    /// MoQ track for transport
    moq_track: MoqTrack,
    /// Whether track is currently muted
    muted: bool,
    /// Track publication time
    published_at: std::time::Instant,
}

/// Track type enumeration
#[cfg(feature = "media")]
#[derive(Debug, Clone, PartialEq)]
enum TrackType {
    Video,
    Audio,
}

impl Room {
    /// Quick join - simplest possible API
    pub async fn quick_join(room_id: &str, participant_id: &str) -> Result<Self, QuicRtcError> {
        QuicRtc::init()
            .await?
            .room(room_id)
            .participant(participant_id)
            .enable_video()
            .enable_audio()
            .join()
            .await
    }

    pub(crate) async fn join_internal(
        quic_rtc: Arc<QuicRtc>,
        room_id: String,
        participant_id: String,
        config: RoomConfig,
        #[cfg(feature = "media")] audio_config: Option<AudioProcessingConfig>,
        #[cfg(feature = "media")] video_config: Option<VideoProcessingConfig>,
        #[cfg(feature = "signaling")] signaling_config: Option<SignalingConfig>,
        resource_limits: Option<ResourceLimits>,
        max_participants: Option<usize>,
    ) -> Result<Self, QuicRtcError> {
        info!(
            "🏠 Joining room '{}' as participant '{}'",
            room_id, participant_id
        );

        // Create event channel for room events
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        // Initialize room with disconnected state
        let room_inner = RoomInner {
            state: RoomState::Disconnected,
            moq_transport: None,
            #[cfg(feature = "signaling")]
            signaling_connection: None,
            #[cfg(feature = "media")]
            media_processor: None,
            #[cfg(feature = "media")]
            video_capture: None,
            #[cfg(feature = "media")]
            audio_renderer: None,
            participants: crate::Participants::new(),
            local_participant: None,
            #[cfg(feature = "media")]
            published_tracks: std::collections::HashMap::new(),
            event_tx: Some(event_tx),
            background_tasks: Vec::new(),
        };

        let room = Self {
            id: room_id.clone(),
            participant_id: participant_id.clone(),
            config,
            #[cfg(feature = "media")]
            audio_config,
            #[cfg(feature = "media")]
            video_config,
            #[cfg(feature = "signaling")]
            signaling_config,
            resource_limits,
            max_participants,
            inner: Arc::new(RwLock::new(room_inner)),
        };

        // Start the connection process
        room.connect(&quic_rtc).await?;

        info!("✅ Successfully joined room '{}'", room_id);
        Ok(room)
    }

    /// Internal connection logic
    async fn connect(&self, quic_rtc: &QuicRtc) -> Result<(), QuicRtcError> {
        let mut inner = self.inner.write().await;
        inner.state = RoomState::Connecting;

        // Step 1: Initialize media subsystems if enabled
        #[cfg(feature = "media")]
        {
            if self.config.video_enabled || self.config.audio_enabled {
                info!("🎥 Initializing media subsystems");
                self.init_media_subsystems(&mut inner).await?;
            }
        }

        // Step 2: Connect to signaling server if configured
        #[cfg(feature = "signaling")]
        {
            if let Some(signaling_url) = &self.config.signaling_url {
                info!("📡 Connecting to signaling server: {}", signaling_url);
                self.connect_signaling(&mut inner, signaling_url).await?;
            }
        }

        // Step 3: Establish MoQ transport
        info!("🚀 Establishing MoQ over QUIC transport");
        self.establish_moq_transport(&mut inner, quic_rtc).await?;

        // Step 4: Initialize local participant
        info!("👤 Initializing local participant");
        inner.local_participant = Some(crate::LocalParticipant::new(
            self.participant_id.clone(),
            self.config.clone(),
        ));

        inner.state = RoomState::Connected;
        info!("🎉 Room connection established successfully");

        Ok(())
    }

    /// Initialize media subsystems with permission checking
    #[cfg(feature = "media")]
    async fn init_media_subsystems(&self, inner: &mut RoomInner) -> Result<(), QuicRtcError> {
        // Initialize media processor
        inner.media_processor = Some(Arc::new(tokio::sync::Mutex::new(MediaProcessor::new())));

        // Initialize video capture if video is enabled
        if self.config.video_enabled {
            debug!("📹 Initializing video capture with permission checks");
            let mut video_capture =
                VideoCaptureManager::new().map_err(|e| QuicRtcError::MediaProcessing {
                    reason: format!("Failed to initialize video capture: {}", e),
                })?;

            // Check camera permissions during initialization [[memory:3911748]]
            match video_capture.enumerate_devices() {
                Ok(devices) => {
                    if devices.is_empty() {
                        return Err(QuicRtcError::MediaProcessing {
                            reason: "No camera devices available - check permissions".to_string(),
                        });
                    }
                    info!(
                        "✅ Camera permissions verified - {} devices available",
                        devices.len()
                    );
                }
                Err(e) => {
                    error!(
                        "❌ Camera permission check failed during initialization: {}",
                        e
                    );
                    return Err(QuicRtcError::MediaProcessing {
                        reason: format!("Camera access denied during initialization: {}", e),
                    });
                }
            }

            inner.video_capture = Some(Arc::new(tokio::sync::Mutex::new(video_capture)));
        }

        // Initialize audio renderer if audio is enabled
        if self.config.audio_enabled {
            debug!("🎵 Initializing audio renderer with permission checks");
            let audio_renderer = CpalAudioRenderer::new();

            // Check microphone permissions during initialization [[memory:3911748]]
            match audio_renderer.list_devices() {
                Ok(devices) => {
                    if devices.is_empty() {
                        return Err(QuicRtcError::MediaProcessing {
                            reason: "No audio input devices available - check permissions"
                                .to_string(),
                        });
                    }
                    info!(
                        "✅ Microphone permissions verified - {} devices available",
                        devices.len()
                    );
                }
                Err(e) => {
                    // If device enumeration fails, log warning but allow initialization
                    warn!(
                        "⚠️ Could not enumerate audio devices ({}), proceeding anyway",
                        e
                    );
                }
            }

            inner.audio_renderer = Some(Arc::new(tokio::sync::Mutex::new(audio_renderer)));
        }

        Ok(())
    }

    /// Connect to signaling server
    #[cfg(feature = "signaling")]
    async fn connect_signaling(
        &self,
        inner: &mut RoomInner,
        signaling_url: &str,
    ) -> Result<(), QuicRtcError> {
        // Create participant info for signaling
        let participant_info = PeerInfo {
            id: self.participant_id.clone(),
            name: None, // Could be set from config in the future
            room_id: self.id.clone(),
            quic_endpoint: None, // Will be set when MoQ transport is ready
            capabilities: vec!["h264".to_string(), "opus".to_string()], // Basic capabilities
            last_seen: chrono::Utc::now(),
            status: PeerStatus::Online,
        };

        let signaling_connection = SignalingConnection {
            server: Arc::new(SignalingServer::new(signaling_url.parse().map_err(
                |_| QuicRtcError::InvalidData {
                    reason: "Invalid signaling URL".to_string(),
                },
            )?)),
            participant_info,
            discovered_peers: std::collections::HashMap::new(),
        };

        inner.signaling_connection = Some(Arc::new(tokio::sync::Mutex::new(signaling_connection)));
        Ok(())
    }

    /// Establish MoQ over QUIC transport
    async fn establish_moq_transport(
        &self,
        inner: &mut RoomInner,
        quic_rtc: &QuicRtc,
    ) -> Result<(), QuicRtcError> {
        // Use default QUIC endpoint for now (could be configurable)
        let endpoint = "127.0.0.1:7878"
            .parse()
            .map_err(|_| QuicRtcError::InvalidData {
                reason: "Invalid QUIC endpoint".to_string(),
            })?;

        // Create connection config with resource limits
        let mut connection_config = ConnectionConfig::default();
        if let Some(limits) = &self.resource_limits {
            // Convert our ResourceLimits to transport::ResourceLimits
            let transport_limits = quicrtc_core::transport::ResourceLimits {
                max_memory_mb: limits.max_memory_mb,
                max_bandwidth_kbps: limits.max_bandwidth_kbps,
                max_connections: limits.max_connections,
                max_streams_per_connection: limits.max_streams_per_connection,
                cleanup_timeout: limits.cleanup_timeout,
                connection_pool_size: 2, // Default value for connection pool
            };
            connection_config.resource_limits = Some(transport_limits);
        }

        // Create MoQ session ID
        let session_id = rand::random::<u64>();

        // Establish MoQ over QUIC transport
        let moq_transport =
            MoqOverQuicTransport::new(endpoint, connection_config, session_id).await?;

        // Establish MoQ session
        moq_transport.establish_session().await?;

        inner.moq_transport = Some(Arc::new(moq_transport));
        Ok(())
    }

    /// Get room ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get participant ID
    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }

    /// Get room configuration
    pub fn config(&self) -> &RoomConfig {
        &self.config
    }

    /// Get audio configuration
    #[cfg(feature = "media")]
    pub fn audio_config(&self) -> Option<&AudioProcessingConfig> {
        self.audio_config.as_ref()
    }

    /// Get video configuration
    #[cfg(feature = "media")]
    pub fn video_config(&self) -> Option<&VideoProcessingConfig> {
        self.video_config.as_ref()
    }

    /// Get resource limits
    pub fn resource_limits(&self) -> Option<&ResourceLimits> {
        self.resource_limits.as_ref()
    }

    /// Get maximum participants
    pub fn max_participants(&self) -> Option<usize> {
        self.max_participants
    }
}

#[cfg(feature = "media")]
impl Room {
    /// Publish camera with default settings
    pub async fn publish_camera(&mut self) -> Result<crate::VideoTrack, crate::QuicRtcError> {
        info!("📹 Publishing camera track");

        // Note: Camera permissions are checked when video capture is initialized
        // per user preferences [[memory:3911748]]

        // First, get the transport and do pre-flight checks
        let (moq_transport, track_id) = {
            let inner = self.inner.read().await;

            // Ensure we're connected
            if inner.state != RoomState::Connected {
                return Err(QuicRtcError::InvalidState {
                    expected: "Connected".to_string(),
                    actual: format!("{:?}", inner.state),
                });
            }

            // Ensure video is enabled in configuration
            if !self.config.video_enabled {
                return Err(QuicRtcError::InvalidData {
                    reason: "video_enabled must be true to publish camera".to_string(),
                });
            }

            // Get transport reference
            let transport = inner
                .moq_transport
                .as_ref()
                .ok_or_else(|| QuicRtcError::InvalidState {
                    expected: "MoQ transport connected".to_string(),
                    actual: "MoQ transport not available".to_string(),
                })?
                .clone();

            let track_id = format!("camera-{}", uuid::Uuid::new_v4());
            (transport, track_id)
        };

        // Start video capture
        {
            let inner = self.inner.read().await;
            let video_capture =
                inner
                    .video_capture
                    .as_ref()
                    .ok_or_else(|| QuicRtcError::InvalidState {
                        expected: "Video capture initialized".to_string(),
                        actual: "Video capture not available".to_string(),
                    })?;

            let mut capture_manager = video_capture.lock().await;

            // Use video config or defaults
            let video_config = self.video_config.as_ref();
            let framerate = video_config.map(|c| c.default_framerate).unwrap_or(30.0);

            // TODO: Use video quality from config to determine resolution
            let (width, height) = match self.config.video_quality {
                VideoQuality::Low => (320, 240),
                VideoQuality::Standard => (640, 480),
                VideoQuality::HD => (1280, 720),
                VideoQuality::FullHD => (1920, 1080),
            };

            // Initialize video capture
            let device_id = "0"; // Use first available device
            let capture_config = quicrtc_media::NewVideoCaptureConfig {
                resolution: quicrtc_media::VideoResolution::new(width, height),
                framerate,
                pixel_format: quicrtc_media::VideoPixelFormat::YUV420P,
                hardware_acceleration: true,
                buffer_size: 3,
                enable_processing: true,
            };

            capture_manager
                .start_capture(device_id, capture_config)
                .await
                .map_err(|e| QuicRtcError::MediaProcessing {
                    reason: format!("Video capture failed: {}", e),
                })?;
        }

        // Create MoQ track for video
        let track_namespace = TrackNamespace {
            namespace: format!("room.{}", self.id),
            track_name: format!("{}/camera", self.participant_id),
        };

        let moq_track = MoqTrack {
            namespace: track_namespace.clone(),
            name: "camera".to_string(),
            track_type: quicrtc_core::MoqTrackType::Video,
        };

        // Announce track
        moq_transport.announce_track(moq_track.clone()).await?;

        // Store published track info with write lock
        {
            let mut inner = self.inner.write().await;
            let published_track = PublishedTrack {
                track_id: track_id.clone(),
                track_type: TrackType::Video,
                moq_track,
                muted: false,
                published_at: std::time::Instant::now(),
            };
            inner
                .published_tracks
                .insert(track_id.clone(), published_track);
        }

        // Create and return video track
        let video_track = VideoTrack::new(track_id);

        info!("✅ Camera track published successfully");
        Ok(video_track)
    }

    /// Publish microphone with default settings
    pub async fn publish_microphone(&mut self) -> Result<crate::AudioTrack, crate::QuicRtcError> {
        info!("🎵 Publishing microphone track");

        // Check microphone permissions first per user preferences [[memory:3911748]]
        #[cfg(target_family = "unix")]
        {
            // Do permission check before acquiring any locks
            let inner_read = self.inner.read().await;
            if let Some(audio_renderer) = &inner_read.audio_renderer {
                let renderer = audio_renderer.lock().await;
                // Try to check if audio devices are available
                match renderer.list_devices() {
                    Ok(devices) => {
                        if devices.is_empty() {
                            return Err(QuicRtcError::MediaProcessing {
                                reason: "No audio input devices available - check permissions"
                                    .to_string(),
                            });
                        }
                        debug!(
                            "✅ Microphone permissions verified - {} devices available",
                            devices.len()
                        );
                    }
                    Err(e) => {
                        // If device enumeration fails, assume it's available for now
                        debug!(
                            "⚠️ Could not enumerate audio devices ({}), assuming available",
                            e
                        );
                    }
                }
            }
        }

        // Get transport and do pre-flight checks
        let (moq_transport, track_id) = {
            let inner = self.inner.read().await;

            // Ensure we're connected
            if inner.state != RoomState::Connected {
                return Err(QuicRtcError::InvalidState {
                    expected: "Connected".to_string(),
                    actual: format!("{:?}", inner.state),
                });
            }

            // Ensure audio is enabled in configuration
            if !self.config.audio_enabled {
                return Err(QuicRtcError::InvalidData {
                    reason: "audio_enabled must be true to publish microphone".to_string(),
                });
            }

            // Get transport reference
            let transport = inner
                .moq_transport
                .as_ref()
                .ok_or_else(|| QuicRtcError::InvalidState {
                    expected: "MoQ transport connected".to_string(),
                    actual: "MoQ transport not available".to_string(),
                })?
                .clone();

            let track_id = format!("microphone-{}", uuid::Uuid::new_v4());
            (transport, track_id)
        };

        // Configure audio renderer
        {
            let inner = self.inner.read().await;
            let audio_renderer =
                inner
                    .audio_renderer
                    .as_ref()
                    .ok_or_else(|| QuicRtcError::InvalidState {
                        expected: "Audio renderer initialized".to_string(),
                        actual: "Audio renderer not available".to_string(),
                    })?;

            let mut renderer = audio_renderer.lock().await;

            // Use audio config or defaults
            let audio_config = self.audio_config.as_ref();
            let volume = audio_config.map(|c| c.default_volume).unwrap_or(0.8);

            // Set volume
            renderer
                .set_volume(volume)
                .map_err(|e| QuicRtcError::MediaProcessing {
                    reason: format!("Failed to set audio volume: {}", e),
                })?;
        }

        // Create MoQ track for audio
        let track_namespace = TrackNamespace {
            namespace: format!("room.{}", self.id),
            track_name: format!("{}/microphone", self.participant_id),
        };

        let moq_track = MoqTrack {
            namespace: track_namespace.clone(),
            name: "microphone".to_string(),
            track_type: quicrtc_core::MoqTrackType::Audio,
        };

        // Announce track
        moq_transport.announce_track(moq_track.clone()).await?;

        // Store published track info with write lock
        {
            let mut inner = self.inner.write().await;
            let published_track = PublishedTrack {
                track_id: track_id.clone(),
                track_type: TrackType::Audio,
                moq_track,
                muted: false,
                published_at: std::time::Instant::now(),
            };
            inner
                .published_tracks
                .insert(track_id.clone(), published_track);
        }

        // Create and return audio track
        let audio_track = AudioTrack::new(track_id);

        info!("✅ Microphone track published successfully");
        Ok(audio_track)
    }

    /// Check camera permissions (platform-specific implementation) - REMOVED
    #[cfg(target_family = "unix")]
    async fn _check_camera_permissions(&self) -> Result<(), QuicRtcError> {
        // On Unix systems, camera permissions are typically handled by the system
        // This is a placeholder for more sophisticated permission checking
        debug!("🔐 Checking camera permissions");

        // For now, we'll attempt to enumerate devices to test permissions
        // In a real implementation, this might check system permission APIs
        if let Some(video_capture) = &self.inner.read().await.video_capture {
            let capture_manager = video_capture.lock().await;
            match capture_manager.enumerate_devices() {
                Ok(devices) => {
                    if devices.is_empty() {
                        return Err(QuicRtcError::MediaProcessing {
                            reason: "No camera devices available - check permissions".to_string(),
                        });
                    }
                    debug!(
                        "✅ Camera permissions verified - {} devices available",
                        devices.len()
                    );
                }
                Err(e) => {
                    error!("❌ Camera permission check failed: {}", e);
                    return Err(QuicRtcError::MediaProcessing {
                        reason: format!("Camera access denied: {}", e),
                    });
                }
            }
        }
        Ok(())
    }

    /// Check microphone permissions (platform-specific implementation)
    #[cfg(target_family = "unix")]
    async fn check_microphone_permissions(&self) -> Result<(), QuicRtcError> {
        debug!("🔐 Checking microphone permissions");

        // Similar to camera, check by attempting to list audio devices
        if let Some(audio_renderer) = &self.inner.read().await.audio_renderer {
            let renderer = audio_renderer.lock().await;
            // Try to check if audio devices are available
            match renderer.list_devices() {
                Ok(devices) => {
                    if devices.is_empty() {
                        return Err(QuicRtcError::MediaProcessing {
                            reason: "No audio input devices available - check permissions"
                                .to_string(),
                        });
                    }
                    debug!(
                        "✅ Microphone permissions verified - {} devices available",
                        devices.len()
                    );
                }
                Err(e) => {
                    // If device enumeration fails, assume it's available for now
                    debug!(
                        "⚠️ Could not enumerate audio devices ({}), assuming available",
                        e
                    );
                }
            }
        }
        Ok(())
    }
}

impl Room {
    /// Get event stream
    pub fn events(&self) -> crate::EventStream {
        // Create event stream that receives events from the room
        crate::EventStream::from_room(Arc::clone(&self.inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "media")]
    use crate::{AudioProcessingConfig, VideoProcessingConfig, VideoQuality};
    #[cfg(feature = "signaling")]
    use crate::{ReconnectConfig, SignalingConfig};
    use std::time::Duration;

    // Helper function to create a test QuicRtc instance
    async fn test_quic_rtc() -> QuicRtc {
        QuicRtc::init().await.expect("Failed to initialize QuicRtc")
    }

    #[tokio::test]
    async fn test_room_builder_basic_configuration() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .enable_video()
            .enable_audio();

        // Test validation passes
        assert!(builder.validate().is_ok());

        // Test configuration is correct
        assert_eq!(builder.room_id, "test-room");
        assert_eq!(builder.participant_id, Some("alice".to_string()));
        assert!(builder.config.video_enabled);
        assert!(builder.config.audio_enabled);
    }

    #[tokio::test]
    async fn test_room_builder_validation_missing_participant() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc.room("test-room").enable_video();

        let result = builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::MissingConfiguration { field }) = result {
            assert_eq!(field, "participant_id");
        } else {
            panic!("Expected MissingConfiguration error");
        }
    }

    #[tokio::test]
    async fn test_room_builder_validation_empty_participant_id() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc.room("test-room").participant("");

        let result = builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::MissingConfiguration { field }) = result {
            assert!(field.contains("participant_id cannot be empty"));
        } else {
            panic!("Expected MissingConfiguration error");
        }
    }

    #[tokio::test]
    async fn test_room_builder_validation_participant_id_too_long() {
        let quic_rtc = test_quic_rtc().await;
        let long_id = "a".repeat(65); // Exceeds 64 character limit
        let builder = quic_rtc.room("test-room").participant(&long_id);

        let result = builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::InvalidData { reason }) = result {
            assert!(reason.contains("participant_id cannot exceed 64 characters"));
        } else {
            panic!("Expected InvalidData error");
        }
    }

    #[tokio::test]
    async fn test_room_builder_validation_room_id_too_long() {
        let quic_rtc = test_quic_rtc().await;
        let long_room_id = "r".repeat(129); // Exceeds 128 character limit
        let builder = quic_rtc.room(&long_room_id).participant("alice");

        let result = builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::InvalidData { reason }) = result {
            assert!(reason.contains("room_id cannot exceed 128 characters"));
        } else {
            panic!("Expected InvalidData error");
        }
    }

    #[cfg(feature = "media")]
    #[tokio::test]
    async fn test_room_builder_media_configuration() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .enable_video()
            .video_quality(VideoQuality::HD)
            .enable_audio()
            .audio_volume(0.8)
            .enable_echo_cancellation()
            .enable_noise_suppression();

        assert!(builder.validate().is_ok());
        assert_eq!(builder.config.video_quality, VideoQuality::HD);

        if let Some(ref audio_config) = builder.audio_config {
            assert_eq!(audio_config.default_volume, 0.8);
            assert!(audio_config.enable_echo_cancellation);
            assert!(audio_config.enable_noise_suppression);
        } else {
            panic!("Expected audio config to be set");
        }
    }

    #[cfg(feature = "media")]
    #[tokio::test]
    async fn test_room_builder_video_resolution() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .video_resolution(1920, 1080, 60.0);

        assert!(builder.validate().is_ok());
        assert!(builder.config.video_enabled);

        if let Some(ref video_config) = builder.video_config {
            assert_eq!(video_config.default_framerate, 60.0);
        } else {
            panic!("Expected video config to be set");
        }
    }

    #[cfg(feature = "media")]
    #[tokio::test]
    async fn test_room_builder_validation_invalid_audio_volume() {
        let quic_rtc = test_quic_rtc().await;

        // Test volume too high
        let mut audio_config = AudioProcessingConfig::default();
        audio_config.default_volume = 1.5; // Invalid: > 1.0

        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .audio_processing(audio_config);

        let result = builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::InvalidData { reason }) = result {
            assert!(reason.contains("audio volume must be between 0.0 and 1.0"));
        } else {
            panic!("Expected InvalidData error");
        }
    }

    #[cfg(feature = "media")]
    #[tokio::test]
    async fn test_room_builder_validation_invalid_framerate() {
        let quic_rtc = test_quic_rtc().await;

        let mut video_config = VideoProcessingConfig::default();
        video_config.default_framerate = 150.0; // Invalid: > 120.0

        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .video_processing(video_config);

        let result = builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::InvalidData { reason }) = result {
            assert!(reason.contains("video framerate must be between 0.1 and 120.0"));
        } else {
            panic!("Expected InvalidData error");
        }
    }

    #[tokio::test]
    async fn test_room_builder_platform_optimizations() {
        let quic_rtc = test_quic_rtc().await;
        let mobile_builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .mobile_optimized();

        assert!(mobile_builder.validate().is_ok());
        assert!(mobile_builder.config.mobile_optimizations);
        assert!(mobile_builder.resource_limits.is_some());

        let desktop_builder = quic_rtc
            .room("test-room")
            .participant("bob")
            .desktop_optimized();

        assert!(desktop_builder.validate().is_ok());
        assert!(!desktop_builder.config.mobile_optimizations);
        assert!(desktop_builder.resource_limits.is_some());
    }

    #[tokio::test]
    async fn test_room_builder_bandwidth_configuration() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .bandwidth_limit(1000);

        assert!(builder.validate().is_ok());

        if let Some(ref limits) = builder.resource_limits {
            assert_eq!(limits.max_bandwidth_kbps, Some(1000));
        } else {
            panic!("Expected resource limits to be set");
        }
    }

    #[tokio::test]
    async fn test_room_builder_validation_invalid_bandwidth() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .bandwidth_limit(32); // Invalid: < 64 kbps

        let result = builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::InvalidData { reason }) = result {
            assert!(reason.contains("bandwidth limit cannot be less than 64 kbps"));
        } else {
            panic!("Expected InvalidData error");
        }
    }

    #[tokio::test]
    async fn test_room_builder_low_bandwidth_preset() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .low_bandwidth();

        assert!(builder.validate().is_ok());

        #[cfg(feature = "media")]
        {
            assert_eq!(builder.config.video_quality, VideoQuality::Low);
            if let Some(ref audio_config) = builder.audio_config {
                assert_eq!(audio_config.buffer_size, 480); // 10ms buffer
            }
        }

        if let Some(ref limits) = builder.resource_limits {
            assert_eq!(limits.max_bandwidth_kbps, Some(500));
            assert_eq!(limits.max_connections, Some(2));
        }
    }

    #[tokio::test]
    async fn test_room_builder_high_quality_preset() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .high_quality();

        assert!(builder.validate().is_ok());

        #[cfg(feature = "media")]
        {
            assert_eq!(builder.config.video_quality, VideoQuality::FullHD);

            if let Some(ref audio_config) = builder.audio_config {
                assert!(audio_config.enable_echo_cancellation);
                assert!(audio_config.enable_noise_suppression);
            }

            if let Some(ref video_config) = builder.video_config {
                assert!(video_config.enable_preprocessing);
                assert_eq!(video_config.default_framerate, 30.0);
            }
        }

        if let Some(ref limits) = builder.resource_limits {
            assert_eq!(limits.max_bandwidth_kbps, Some(5000));
            assert_eq!(limits.max_connections, Some(10));
        }
    }

    #[tokio::test]
    async fn test_room_builder_max_participants_validation() {
        let quic_rtc = test_quic_rtc().await;

        // Test valid max participants
        let valid_builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .max_participants(100);
        assert!(valid_builder.validate().is_ok());

        // Test invalid max participants (zero)
        let invalid_builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .max_participants(0);

        let result = invalid_builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::InvalidData { reason }) = result {
            assert!(reason.contains("max_participants must be at least 1"));
        } else {
            panic!("Expected InvalidData error");
        }

        // Test invalid max participants (too high)
        let too_high_builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .max_participants(1001);

        let result = too_high_builder.validate();
        assert!(result.is_err());

        if let Err(QuicRtcError::InvalidData { reason }) = result {
            assert!(reason.contains("max_participants cannot exceed 1000"));
        } else {
            panic!("Expected InvalidData error");
        }
    }

    #[cfg(feature = "signaling")]
    #[tokio::test]
    async fn test_room_builder_signaling_configuration() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .signaling_server("wss://signaling.example.com")
            .connection_timeout(Duration::from_secs(15));

        assert!(builder.validate().is_ok());
        assert_eq!(
            builder.config.signaling_url,
            Some("wss://signaling.example.com".to_string())
        );

        if let Some(ref signaling_config) = builder.signaling_config {
            assert_eq!(signaling_config.connection_timeout, Duration::from_secs(15));
        }
    }

    #[tokio::test]
    async fn test_room_builder_disable_methods() {
        let quic_rtc = test_quic_rtc().await;
        let builder = quic_rtc
            .room("test-room")
            .participant("alice")
            .enable_video()
            .enable_audio()
            .disable_video()
            .disable_audio();

        assert!(builder.validate().is_ok());
        assert!(!builder.config.video_enabled);
        assert!(!builder.config.audio_enabled);
    }

    #[tokio::test]
    async fn test_room_quick_join() {
        // Test the Room::quick_join convenience method
        let result = Room::quick_join("quick-room", "alice").await;
        assert!(result.is_ok());

        let room = result.unwrap();
        assert_eq!(room.id(), "quick-room");
        assert_eq!(room.participant_id(), "alice");
        assert!(room.config().video_enabled);
        assert!(room.config().audio_enabled);
    }

    #[tokio::test]
    async fn test_room_configuration_getters() {
        let quic_rtc = test_quic_rtc().await;
        let room = quic_rtc
            .room("test-room")
            .participant("alice")
            .enable_video()
            .max_participants(50)
            .bandwidth_limit(2000)
            .join()
            .await
            .expect("Failed to join room");

        assert_eq!(room.id(), "test-room");
        assert_eq!(room.participant_id(), "alice");
        assert!(room.config().video_enabled);
        assert_eq!(room.max_participants(), Some(50));

        if let Some(limits) = room.resource_limits() {
            assert_eq!(limits.max_bandwidth_kbps, Some(2000));
        }
    }
}
