//! Unit tests for transport layer
//!
//! This module contains all unit tests for the QUIC transport layer implementation.
//! Tests are organized by functionality and include mocking for network operations.

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use quicrtc_core::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Create a test endpoint
fn test_endpoint() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
}

#[tokio::test]
async fn test_transport_mode_serialization() {
    let mode = TransportMode::QuicNative;
    let serialized = serde_json::to_string(&mode).unwrap();
    let deserialized: TransportMode = serde_json::from_str(&serialized).unwrap();
    assert_eq!(mode, deserialized);
}

#[tokio::test]
async fn test_connection_config_default() {
    let config = ConnectionConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(10));
    assert!(config.keep_alive);
    assert_eq!(config.keep_alive_interval, Duration::from_secs(30));
    assert_eq!(config.max_idle_timeout, Duration::from_secs(60));
    assert!(config.enable_migration);
    assert!(config.preferred_transports.is_none());
}

#[tokio::test]
async fn test_connection_metrics_default() {
    let metrics = ConnectionMetrics::default();
    assert_eq!(metrics.connection_attempts, 0);
    assert_eq!(metrics.successful_connections, 0);
    assert!(metrics.failed_connections.is_empty());
    assert_eq!(metrics.migration_events, 0);
    assert!(metrics.last_attempt.is_none());
}

#[tokio::test]
async fn test_network_path_creation() {
    let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345);
    let remote_addr = test_endpoint();

    let path = NetworkPath {
        local_addr,
        remote_addr,
        interface_name: Some("eth0".to_string()),
        mtu: Some(1500),
    };

    assert_eq!(path.local_addr, local_addr);
    assert_eq!(path.remote_addr, remote_addr);
    assert_eq!(path.interface_name, Some("eth0".to_string()));
    assert_eq!(path.mtu, Some(1500));
}

#[tokio::test]
async fn test_connection_establishment_error_structure() {
    let error = QuicRtcError::Connection {
        room_id: "test-room".to_string(),
        reason: "All transport modes failed".to_string(),
        retry_in: Some(Duration::from_secs(5)),
        suggested_action: "Check network connectivity and firewall settings".to_string(),
    };

    match error {
        QuicRtcError::Connection {
            reason,
            retry_in,
            suggested_action,
            ..
        } => {
            assert_eq!(reason, "All transport modes failed");
            assert_eq!(retry_in, Some(Duration::from_secs(5)));
            assert_eq!(
                suggested_action,
                "Check network connectivity and firewall settings"
            );
        }
        _ => panic!("Expected Connection error"),
    }
}

#[tokio::test]
async fn test_quic_stream_creation() {
    let stream = QuicStream {
        id: 42,
        stream_type: StreamType::Bidirectional,
        send: None,
        recv: None,
    };

    assert_eq!(stream.id, 42);
    assert_eq!(stream.stream_type, StreamType::Bidirectional);
    assert!(stream.send.is_none());
    assert!(stream.recv.is_none());
}

#[tokio::test]
async fn test_quic_stream_send_without_send_stream() {
    let mut stream = QuicStream {
        id: 1,
        stream_type: StreamType::Unidirectional,
        send: None,
        recv: None,
    };

    let result = stream.send(b"test data").await;
    assert!(result.is_err());

    if let Err(QuicRtcError::Transport { reason }) = result {
        assert_eq!(reason, "No send stream available");
    } else {
        panic!("Expected Transport error");
    }
}

#[tokio::test]
async fn test_quic_stream_recv_without_recv_stream() {
    let mut stream = QuicStream {
        id: 1,
        stream_type: StreamType::Unidirectional,
        send: None,
        recv: None,
    };

    let result = stream.recv().await;
    assert!(result.is_err());

    if let Err(QuicRtcError::Transport { reason }) = result {
        assert_eq!(reason, "No receive stream available");
    } else {
        panic!("Expected Transport error");
    }
}

/// Mock transport for testing
struct MockTransport {
    connected: bool,
    send_data: Vec<u8>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            connected: true,
            send_data: Vec::new(),
        }
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&mut self, data: &[u8]) -> Result<(), QuicRtcError> {
        if !self.connected {
            return Err(QuicRtcError::Transport {
                reason: "Not connected".to_string(),
            });
        }
        self.send_data.extend_from_slice(data);
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Bytes>, QuicRtcError> {
        if !self.connected {
            return Err(QuicRtcError::Transport {
                reason: "Not connected".to_string(),
            });
        }
        Ok(Some(Bytes::from("test response")))
    }

    async fn close(&mut self) -> Result<(), QuicRtcError> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[tokio::test]
async fn test_mock_transport_send() {
    let mut transport = MockTransport::new();

    assert!(transport.is_connected());

    let result = transport.send(b"hello world").await;
    assert!(result.is_ok());
    assert_eq!(transport.send_data, b"hello world");
}

#[tokio::test]
async fn test_mock_transport_recv() {
    let mut transport = MockTransport::new();

    let result = transport.recv().await;
    assert!(result.is_ok());

    if let Ok(Some(data)) = result {
        assert_eq!(data, Bytes::from("test response"));
    } else {
        panic!("Expected successful receive");
    }
}

#[tokio::test]
async fn test_mock_transport_close() {
    let mut transport = MockTransport::new();

    assert!(transport.is_connected());

    let result = transport.close().await;
    assert!(result.is_ok());
    assert!(!transport.is_connected());
}

#[tokio::test]
async fn test_mobile_network_switching() {
    // WiFi path
    let wifi_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
        remote_addr: test_endpoint(),
        interface_name: Some("wlan0".to_string()),
        mtu: Some(1500),
    };

    // Cellular path
    let cellular_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)), 12346),
        remote_addr: test_endpoint(),
        interface_name: Some("rmnet0".to_string()),
        mtu: Some(1400),
    };

    // Validate different network characteristics
    assert_eq!(wifi_path.mtu, Some(1500));
    assert_eq!(cellular_path.mtu, Some(1400)); // Lower MTU for cellular

    // Validate interface naming conventions
    assert!(wifi_path
        .interface_name
        .as_ref()
        .unwrap()
        .starts_with("wlan"));
    assert!(cellular_path
        .interface_name
        .as_ref()
        .unwrap()
        .starts_with("rmnet"));
}

#[tokio::test]
async fn test_connection_config_scenarios() {
    // Mobile configuration
    let mobile_config = ConnectionConfig {
        timeout: Duration::from_secs(5),
        keep_alive: true,
        keep_alive_interval: Duration::from_secs(15),
        max_idle_timeout: Duration::from_secs(30),
        enable_migration: true,
        preferred_transports: Some(vec![
            TransportMode::QuicNative,
            TransportMode::QuicOverWebSocket,
        ]),
        quic_transport_config: Default::default(),
        resource_limits: Default::default(),
    };

    assert!(mobile_config.enable_migration);
    assert_eq!(mobile_config.timeout, Duration::from_secs(5));
    assert_eq!(mobile_config.keep_alive_interval, Duration::from_secs(15));
}

#[tokio::test]
async fn test_connection_metrics_comprehensive() {
    let mut metrics = ConnectionMetrics::default();

    // Simulate multiple connection attempts
    for _ in 0..3 {
        metrics.connection_attempts += 1;
        metrics.last_attempt = Some(Instant::now());
    }

    // Simulate failures for different transport modes
    *metrics
        .failed_connections
        .entry(TransportMode::QuicNative)
        .or_insert(0) += 2;
    *metrics
        .failed_connections
        .entry(TransportMode::QuicOverWebSocket)
        .or_insert(0) += 1;

    // Simulate successful connection
    metrics.successful_connections += 1;

    // Simulate migrations
    metrics.migration_events += 2;

    // Verify metrics
    assert_eq!(metrics.connection_attempts, 3);
    assert_eq!(metrics.successful_connections, 1);
    assert_eq!(metrics.failed_connections[&TransportMode::QuicNative], 2);
    assert_eq!(
        metrics.failed_connections[&TransportMode::QuicOverWebSocket],
        1
    );
    assert_eq!(metrics.migration_events, 2);
    assert!(metrics.last_attempt.is_some());
}
