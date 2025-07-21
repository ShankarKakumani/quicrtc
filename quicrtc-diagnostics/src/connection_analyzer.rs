//! Connection state analysis and diagnostics

use quicrtc_core::TransportMode;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Connection information and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// Current transport mode
    pub transport_mode: TransportMode,
    /// Connection duration
    pub duration: Duration,
    /// Round-trip time
    pub rtt: Duration,
    /// Bandwidth estimate
    pub bandwidth_estimate: u64,
    /// Connection state
    pub state: ConnectionState,
}

/// Connection state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Connecting
    Connecting,
    /// Connected
    Connected,
    /// Reconnecting
    Reconnecting,
    /// Disconnected
    Disconnected,
}

/// Detailed connection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Packets sent
    pub packets_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Packet loss rate
    pub packet_loss_rate: f64,
    /// Jitter
    pub jitter: Duration,
}