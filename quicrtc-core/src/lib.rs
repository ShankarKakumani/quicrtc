//! # QUIC RTC Core
//!
//! Core QUIC transport and IETF Media over QUIC (MoQ) protocol implementation.
//! This crate provides the foundational transport layer and MoQ protocol handling
//! for the QUIC RTC system.

#![deny(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod moq;
pub mod moq_transport;
pub mod resource;
pub mod transport;

// Re-export main types
pub use error::QuicRtcError;
pub use moq::{
    H264Frame, ManagedMoqStream, MoqCacheConfig, MoqCacheStats, MoqCapabilities, MoqControlMessage,
    MoqDeliveryStats, MoqObject, MoqObjectCache, MoqObjectDelivery, MoqObjectStatus, MoqSession,
    MoqSessionState, MoqStreamEvent, MoqStreamManager, MoqStreamState, MoqStreamType,
    MoqSubscription, MoqSubscriptionState, MoqTrack, MoqTrackType, MoqWireFormat, OpusFrame,
    StreamId, StreamManagerConfig, StreamStats, TrackAlias, TrackNamespace,
};
pub use moq_transport::{MoqOverQuicTransport, MoqStream, MoqTransportEvent};
pub use resource::{
    ConnectionPool, ConnectionPoolConfig, ConnectionPoolMetrics, ConnectionPoolStats,
    ResourceLimits, ResourceManager, ResourceMonitorConfig, ResourceUsage, ResourceWarning,
    WarningSeverity,
};
pub use transport::{
    ConnectionConfig, ConnectionMetrics, ConnectionStats, NetworkPath, QuicStream, StreamType,
    Transport, TransportConnection, TransportMode,
};
