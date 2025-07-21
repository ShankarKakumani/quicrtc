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
    MoqCacheStats, MoqDeliveryStats, MoqObject, MoqObjectCache, MoqObjectDelivery, 
    MoqObjectStatus, MoqSession, MoqTrack, NetworkPath, OpusFrame, QuicRtcError, 
    ResourceLimits, ResourceManager, ResourceUsage, ResourceWarning, TrackNamespace,
    TransportConnection, TransportMode, WarningSeverity,
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
pub use config::{GlobalConfig, RoomConfig};
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
    runtime: tokio::runtime::Runtime,
    config: GlobalConfig,
}

impl QuicRtc {
    /// Initialize QUIC RTC with default settings
    ///
    /// # Example
    /// ```rust,no_run
    /// use quicrtc::QuicRtc;
    ///
    /// let quic_rtc = QuicRtc::init()?;
    /// # Ok::<(), quicrtc::QuicRtcError>(())
    /// ```
    pub fn init() -> Result<Self, QuicRtcError> {
        Self::init_with(GlobalConfig::default())
    }

    /// Initialize with custom global configuration
    pub fn init_with(config: GlobalConfig) -> Result<Self, QuicRtcError> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| QuicRtcError::Initialization {
            reason: format!("Failed to create async runtime: {}", e),
        })?;

        Ok(Self {
            inner: std::sync::Arc::new(QuicRtcInner { runtime, config }),
        })
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
