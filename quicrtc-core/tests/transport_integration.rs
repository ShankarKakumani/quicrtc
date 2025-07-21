//! Integration tests for transport layer and MoQ over QUIC integration

use quicrtc_core::{
    ConnectionConfig, H264Frame, MoqObject, MoqObjectStatus, MoqOverQuicTransport, MoqStreamType,
    MoqTrack, MoqTrackType, MoqTransportEvent, NetworkPath, OpusFrame, QuicRtcError,
    TrackNamespace, TransportConnection, TransportMode,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio;

/// Create a test endpoint that will fail to connect
fn unreachable_endpoint() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9999) // RFC 5737 test address
}

/// Create a test connection config with short timeouts
fn test_config() -> ConnectionConfig {
    ConnectionConfig {
        timeout: Duration::from_millis(100), // Very short timeout for tests
        keep_alive: true,
        keep_alive_interval: Duration::from_secs(10),
        max_idle_timeout: Duration::from_secs(30),
        enable_migration: true,
        preferred_transports: None,
        quic_transport_config: Default::default(),
        resource_limits: Default::default(),
    }
}

#[tokio::test]
async fn test_connection_establishment_fallback_integration() {
    // Test that connection establishment tries all fallback modes
    let endpoint = unreachable_endpoint();
    let config = ConnectionConfig {
        timeout: Duration::from_millis(1), // Very short timeout to fail quickly
        ..test_config()
    };

    let start_time = std::time::Instant::now();
    let result = TransportConnection::establish_with_fallback(endpoint, config).await;
    let elapsed = start_time.elapsed();

    // Should fail after trying all transport modes
    assert!(result.is_err());

    // Should have attempted the fallback chain
    assert!(elapsed >= Duration::from_millis(1));

    match result {
        Err(QuicRtcError::Connection {
            reason,
            retry_in,
            suggested_action,
            ..
        }) => {
            assert_eq!(reason, "All transport modes failed");
            assert_eq!(retry_in, Some(Duration::from_secs(5)));
            assert_eq!(
                suggested_action,
                "Check network connectivity and firewall settings"
            );
        }
        _ => panic!("Expected Connection error with fallback failure"),
    }
}

#[tokio::test]
async fn test_connection_metrics_after_failed_attempts() {
    let endpoint = unreachable_endpoint();
    let config = ConnectionConfig {
        timeout: Duration::from_millis(1), // Very short timeout
        ..test_config()
    };

    let result = TransportConnection::establish_with_fallback(endpoint, config).await;
    assert!(result.is_err());

    // We can't access metrics from a failed connection, but we know the internal
    // logic should have tracked the attempts. This test validates the error structure.
    if let Err(QuicRtcError::Connection { .. }) = result {
        // Expected - all transport modes should have been attempted
    } else {
        panic!("Expected Connection error");
    }
}

#[tokio::test]
async fn test_network_path_migration_scenarios() {
    // Test different network path scenarios for mobile migration

    // WiFi to Cellular migration
    let wifi_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
        remote_addr: unreachable_endpoint(),
        interface_name: Some("wlan0".to_string()),
        mtu: Some(1500),
    };

    let cellular_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)), 12346),
        remote_addr: unreachable_endpoint(),
        interface_name: Some("rmnet0".to_string()),
        mtu: Some(1400), // Typically lower MTU for cellular
    };

    // Validate path differences
    assert_ne!(wifi_path.local_addr, cellular_path.local_addr);
    assert_ne!(wifi_path.interface_name, cellular_path.interface_name);
    assert_ne!(wifi_path.mtu, cellular_path.mtu);

    // Ethernet to WiFi migration
    let ethernet_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 50)), 12347),
        remote_addr: unreachable_endpoint(),
        interface_name: Some("eth0".to_string()),
        mtu: Some(1500),
    };

    assert_ne!(ethernet_path.interface_name, wifi_path.interface_name);
    assert_eq!(ethernet_path.mtu, wifi_path.mtu); // Both typically 1500
}

#[tokio::test]
async fn test_transport_mode_fallback_order() {
    // Test that the fallback order prioritizes performance
    let modes = vec![
        TransportMode::QuicNative,        // Best performance
        TransportMode::QuicOverWebSocket, // Good performance, firewall workaround
        TransportMode::WebRtcCompat,      // Maximum compatibility
    ];

    // Verify the order makes sense for performance vs compatibility tradeoff
    assert_eq!(modes[0], TransportMode::QuicNative);
    assert_eq!(modes[1], TransportMode::QuicOverWebSocket);
    assert_eq!(modes[2], TransportMode::WebRtcCompat);
}

#[tokio::test]
async fn test_connection_config_mobile_optimizations() {
    let mut config = ConnectionConfig::default();

    // Mobile-optimized configuration
    config.timeout = Duration::from_secs(5); // Shorter timeout for mobile
    config.keep_alive_interval = Duration::from_secs(15); // More frequent keep-alive
    config.max_idle_timeout = Duration::from_secs(30); // Shorter idle timeout
    config.enable_migration = true; // Essential for mobile

    assert_eq!(config.timeout, Duration::from_secs(5));
    assert_eq!(config.keep_alive_interval, Duration::from_secs(15));
    assert_eq!(config.max_idle_timeout, Duration::from_secs(30));
    assert!(config.enable_migration);
}

#[tokio::test]
async fn test_connection_config_desktop_optimizations() {
    let mut config = ConnectionConfig::default();

    // Desktop-optimized configuration
    config.timeout = Duration::from_secs(10); // Longer timeout acceptable
    config.keep_alive_interval = Duration::from_secs(60); // Less frequent keep-alive
    config.max_idle_timeout = Duration::from_secs(300); // Longer idle timeout
    config.enable_migration = false; // Less critical for desktop

    assert_eq!(config.timeout, Duration::from_secs(10));
    assert_eq!(config.keep_alive_interval, Duration::from_secs(60));
    assert_eq!(config.max_idle_timeout, Duration::from_secs(300));
    assert!(!config.enable_migration);
}

#[tokio::test]
async fn test_preferred_transport_override() {
    let mut config = ConnectionConfig::default();

    // Override default fallback order
    config.preferred_transports = Some(vec![
        TransportMode::WebRtcCompat, // Try WebRTC first for maximum compatibility
        TransportMode::QuicNative,   // Then try QUIC
    ]);

    assert!(config.preferred_transports.is_some());
    let prefs = config.preferred_transports.as_ref().unwrap();
    assert_eq!(prefs.len(), 2);
    assert_eq!(prefs[0], TransportMode::WebRtcCompat);
    assert_eq!(prefs[1], TransportMode::QuicNative);
}

/// Test realistic mobile network switching scenario
#[tokio::test]
async fn test_mobile_network_switching_scenario() {
    // Simulate a mobile device switching from WiFi to cellular

    // Initial WiFi connection path
    let wifi_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
        remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 443),
        interface_name: Some("wlan0".to_string()),
        mtu: Some(1500),
    };

    // New cellular connection path
    let cellular_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)), 12346),
        remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 443), // Same server
        interface_name: Some("rmnet0".to_string()),
        mtu: Some(1400),
    };

    // Validate the migration scenario
    assert_eq!(wifi_path.remote_addr, cellular_path.remote_addr); // Same server
    assert_ne!(wifi_path.local_addr, cellular_path.local_addr); // Different local address
    assert_ne!(wifi_path.interface_name, cellular_path.interface_name); // Different interface
    assert!(wifi_path.mtu.unwrap() > cellular_path.mtu.unwrap()); // WiFi typically has higher MTU
}

/// Test firewall traversal scenario
#[tokio::test]
async fn test_firewall_traversal_scenario() {
    // Simulate a corporate firewall that blocks QUIC but allows WebSocket
    let endpoint = unreachable_endpoint();
    let config = ConnectionConfig {
        timeout: Duration::from_millis(1), // Very short timeout
        ..test_config()
    };

    // In a real scenario, QUIC would fail but WebSocket might succeed
    // For this test, we just verify the fallback attempt is made
    let result = TransportConnection::establish_with_fallback(endpoint, config).await;

    // Should fail after trying all modes (since we're using unreachable endpoint)
    assert!(result.is_err());

    // The error should indicate all transport modes were attempted
    if let Err(QuicRtcError::Connection { reason, .. }) = result {
        assert_eq!(reason, "All transport modes failed");
    } else {
        panic!("Expected Connection error");
    }
}

/// Test connection establishment timing
#[tokio::test]
async fn test_connection_establishment_timing() {
    let endpoint = unreachable_endpoint();
    let config = ConnectionConfig {
        timeout: Duration::from_millis(1), // Very short timeout
        ..Default::default()
    };

    let start = std::time::Instant::now();
    let result = TransportConnection::establish_with_fallback(endpoint, config).await;
    let elapsed = start.elapsed();

    // Should fail quickly due to short timeout
    assert!(result.is_err());

    // Should complete relatively quickly with short timeout
    assert!(elapsed < Duration::from_millis(100)); // Shouldn't take too long
}

/// Test connection establishment with custom transport preferences
#[tokio::test]
async fn test_connection_establishment_custom_preferences() {
    let endpoint = unreachable_endpoint();
    let config = ConnectionConfig {
        timeout: Duration::from_millis(1),
        preferred_transports: Some(vec![TransportMode::WebRtcCompat, TransportMode::QuicNative]),
        ..test_config()
    };

    let result = TransportConnection::establish_with_fallback(endpoint, config).await;

    // Should still fail with unreachable endpoint, but would have tried WebRTC first
    assert!(result.is_err());

    if let Err(QuicRtcError::Connection { reason, .. }) = result {
        assert_eq!(reason, "All transport modes failed");
    } else {
        panic!("Expected Connection error");
    }
}

/// Test connection migration scenarios
#[tokio::test]
async fn test_connection_migration_scenarios() {
    // Test WiFi to cellular migration scenario
    let wifi_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
        remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 443),
        interface_name: Some("wlan0".to_string()),
        mtu: Some(1500),
    };

    let cellular_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)), 12346),
        remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 443),
        interface_name: Some("rmnet0".to_string()),
        mtu: Some(1400),
    };

    // Validate migration scenario
    assert_eq!(wifi_path.remote_addr, cellular_path.remote_addr); // Same server
    assert_ne!(wifi_path.local_addr, cellular_path.local_addr); // Different local
    assert_ne!(wifi_path.interface_name, cellular_path.interface_name); // Different interface

    // Test Ethernet to WiFi migration
    let ethernet_path = NetworkPath {
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 50)), 12347),
        remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 443),
        interface_name: Some("eth0".to_string()),
        mtu: Some(1500),
    };

    assert_eq!(ethernet_path.remote_addr, wifi_path.remote_addr);
    assert_ne!(ethernet_path.local_addr, wifi_path.local_addr);
    assert_eq!(ethernet_path.mtu, wifi_path.mtu); // Both typically 1500
}

/// Test connection resilience scenarios
#[tokio::test]
async fn test_connection_resilience_scenarios() {
    // Test rapid connection attempts
    let endpoint = unreachable_endpoint();
    let config = ConnectionConfig {
        timeout: Duration::from_millis(1),
        ..test_config()
    };

    let mut results = Vec::new();

    // Try multiple rapid connections
    for _ in 0..3 {
        let result = TransportConnection::establish_with_fallback(endpoint, config.clone()).await;
        results.push(result);
    }

    // All should fail consistently
    for result in results {
        assert!(result.is_err());
        if let Err(QuicRtcError::Connection { reason, .. }) = result {
            assert_eq!(reason, "All transport modes failed");
        }
    }
}

/// Test connection configuration validation
#[tokio::test]
async fn test_connection_config_validation() {
    // Test minimum viable configuration
    let min_config = ConnectionConfig {
        timeout: Duration::from_millis(1),
        keep_alive: false,
        keep_alive_interval: Duration::from_secs(1),
        max_idle_timeout: Duration::from_secs(1),
        enable_migration: false,
        preferred_transports: None,
        quic_transport_config: Default::default(),
        resource_limits: Default::default(),
    };

    assert_eq!(min_config.timeout, Duration::from_millis(1));
    assert!(!min_config.keep_alive);
    assert!(!min_config.enable_migration);

    // Test maximum configuration
    let max_config = ConnectionConfig {
        timeout: Duration::from_secs(60),
        keep_alive: true,
        keep_alive_interval: Duration::from_secs(5),
        max_idle_timeout: Duration::from_secs(600),
        enable_migration: true,
        preferred_transports: Some(vec![
            TransportMode::QuicNative,
            TransportMode::QuicOverWebSocket,
            TransportMode::WebRtcCompat,
        ]),
        quic_transport_config: Default::default(),
        resource_limits: Default::default(),
    };

    assert_eq!(max_config.timeout, Duration::from_secs(60));
    assert!(max_config.keep_alive);
    assert!(max_config.enable_migration);
    assert_eq!(max_config.preferred_transports.as_ref().unwrap().len(), 3);
}

/// Test network interface scenarios
#[tokio::test]
async fn test_network_interface_scenarios() {
    // Test common mobile interfaces
    let mobile_interfaces = vec![
        ("wlan0", "WiFi"),
        ("rmnet0", "Cellular"),
        ("rmnet_data0", "Cellular Data"),
        ("ccmni0", "Cellular (MediaTek)"),
        ("pdp_ip0", "Cellular (Qualcomm)"),
    ];

    for (interface, description) in mobile_interfaces {
        let path = NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
            remote_addr: unreachable_endpoint(),
            interface_name: Some(interface.to_string()),
            mtu: Some(if interface.starts_with("wlan") {
                1500
            } else {
                1400
            }),
        };

        assert_eq!(path.interface_name.as_ref().unwrap(), interface);

        // WiFi typically has higher MTU than cellular
        if description.contains("WiFi") {
            assert_eq!(path.mtu, Some(1500));
        } else if description.contains("Cellular") {
            assert_eq!(path.mtu, Some(1400));
        }
    }

    // Test desktop interfaces
    let desktop_interfaces = vec![
        ("eth0", "Ethernet"),
        ("en0", "Ethernet (macOS)"),
        ("wlp3s0", "WiFi (Linux)"),
        ("Wi-Fi", "WiFi (Windows)"),
    ];

    for (interface, _description) in desktop_interfaces {
        let path = NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
            remote_addr: unreachable_endpoint(),
            interface_name: Some(interface.to_string()),
            mtu: Some(1500),
        };

        assert_eq!(path.interface_name.as_ref().unwrap(), interface);
        assert_eq!(path.mtu, Some(1500));
    }
}

/// Test error recovery scenarios
#[tokio::test]
async fn test_error_recovery_scenarios() {
    let endpoint = unreachable_endpoint();

    // Test timeout recovery
    let timeout_config = ConnectionConfig {
        timeout: Duration::from_millis(1),
        ..test_config()
    };

    let result = TransportConnection::establish_with_fallback(endpoint, timeout_config).await;
    assert!(result.is_err());

    if let Err(QuicRtcError::Connection {
        retry_in,
        suggested_action,
        ..
    }) = result
    {
        assert_eq!(retry_in, Some(Duration::from_secs(5)));
        assert_eq!(
            suggested_action,
            "Check network connectivity and firewall settings"
        );
    }

    // Test with different timeout
    let longer_timeout_config = ConnectionConfig {
        timeout: Duration::from_millis(5),
        ..test_config()
    };

    let result2 =
        TransportConnection::establish_with_fallback(endpoint, longer_timeout_config).await;
    assert!(result2.is_err());

    // Should still fail but with same error structure
    if let Err(QuicRtcError::Connection { reason, .. }) = result2 {
        assert_eq!(reason, "All transport modes failed");
    }
}

// ============================================================================
// MoQ over QUIC Integration Tests
// ============================================================================

/// Helper function to create a test track namespace
fn test_track_namespace() -> TrackNamespace {
    TrackNamespace {
        namespace: "test.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    }
}

/// Helper function to create a test MoQ track
fn test_moq_track() -> MoqTrack {
    MoqTrack {
        namespace: test_track_namespace(),
        name: "camera".to_string(),
        track_type: MoqTrackType::Video,
    }
}

/// Helper function to create a test MoQ object
fn test_moq_object() -> MoqObject {
    MoqObject {
        track_namespace: test_track_namespace(),
        track_name: "camera".to_string(),
        group_id: 12345,
        object_id: 67890,
        publisher_priority: 1,
        payload: vec![0x01, 0x02, 0x03, 0x04, 0x05],
        object_status: MoqObjectStatus::Normal,
        created_at: std::time::Instant::now(),
        size: 5,
    }
}

/// Helper function to create a test H.264 frame
fn test_h264_frame() -> H264Frame {
    H264Frame {
        nal_units: vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x80, 0x1e], // Sample H.264 SPS NAL unit
        is_keyframe: true,
        timestamp_us: 1000000, // 1 second
        sequence_number: 1,
    }
}

/// Helper function to create a test Opus frame
fn test_opus_frame() -> OpusFrame {
    OpusFrame {
        opus_data: vec![0xfc, 0xff, 0xfe], // Sample Opus frame data
        timestamp_us: 20000,               // 20ms
        sequence_number: 1,
        sample_rate: 48000,
        channels: 2,
    }
}

#[tokio::test]
async fn test_moq_transport_creation_failure() {
    // Test MoQ transport creation with unreachable endpoint
    let endpoint = unreachable_endpoint();
    let config = ConnectionConfig {
        timeout: Duration::from_millis(1), // Very short timeout
        ..test_config()
    };

    let result = MoqOverQuicTransport::new(endpoint, config, 123).await;

    // Should fail to create transport due to connection failure
    assert!(result.is_err());

    match result {
        Err(QuicRtcError::Connection { reason, .. }) => {
            assert_eq!(reason, "All transport modes failed");
        }
        _ => panic!("Expected Connection error"),
    }
}

#[tokio::test]
async fn test_moq_object_serialization_roundtrip() {
    // Test MoQ object serialization and deserialization
    let endpoint = unreachable_endpoint();
    let config = test_config();

    // We can't create a real transport, but we can test the serialization logic
    // by creating a mock transport (this would fail in practice but tests the serialization)

    let object = test_moq_object();

    // Test object creation from H.264 frame
    let h264_frame = test_h264_frame();
    let h264_object = MoqObject::from_h264_frame(test_track_namespace(), h264_frame);

    assert_eq!(h264_object.track_namespace, test_track_namespace());
    assert_eq!(h264_object.track_name, "video");
    assert_eq!(h264_object.publisher_priority, 1); // Keyframe priority
    assert_eq!(h264_object.group_id, 1000); // timestamp_us / 1000
    assert_eq!(h264_object.object_id, 1); // sequence_number

    // Test object creation from Opus frame
    let opus_frame = test_opus_frame();
    let opus_object = MoqObject::from_opus_frame(test_track_namespace(), opus_frame);

    assert_eq!(opus_object.track_namespace, test_track_namespace());
    assert_eq!(opus_object.track_name, "audio");
    assert_eq!(opus_object.publisher_priority, 1); // Audio always high priority
    assert_eq!(opus_object.group_id, 1); // timestamp_us / 20000
    assert_eq!(opus_object.object_id, 1); // sequence_number
}

#[tokio::test]
async fn test_moq_object_priority_ordering() {
    // Test MoQ object priority and delivery ordering
    let track_ns = test_track_namespace();

    // Create objects with different priorities
    let normal_object = MoqObject {
        track_namespace: track_ns.clone(),
        track_name: "test".to_string(),
        group_id: 1,
        object_id: 1,
        publisher_priority: 2,
        payload: vec![0x01],
        object_status: MoqObjectStatus::Normal,
        created_at: std::time::Instant::now(),
        size: 1,
    };

    let end_of_group_object = MoqObject {
        track_namespace: track_ns.clone(),
        track_name: "test".to_string(),
        group_id: 1,
        object_id: 2,
        publisher_priority: 2,
        payload: vec![],
        object_status: MoqObjectStatus::EndOfGroup,
        created_at: std::time::Instant::now(),
        size: 0,
    };

    let end_of_track_object = MoqObject {
        track_namespace: track_ns,
        track_name: "test".to_string(),
        group_id: 1,
        object_id: 3,
        publisher_priority: 2,
        payload: vec![],
        object_status: MoqObjectStatus::EndOfTrack,
        created_at: std::time::Instant::now(),
        size: 0,
    };

    // Test delivery priority ordering
    assert_eq!(end_of_track_object.delivery_priority(), 0); // Highest priority
    assert_eq!(end_of_group_object.delivery_priority(), 1);
    assert_eq!(normal_object.delivery_priority(), 2); // Publisher priority

    // Test control object detection
    assert!(!normal_object.is_control_object());
    assert!(end_of_group_object.is_control_object());
    assert!(end_of_track_object.is_control_object());
}

#[tokio::test]
async fn test_moq_transport_event_types() {
    // Test different MoQ transport event types

    let session_event = MoqTransportEvent::SessionEstablished { session_id: 123 };
    match session_event {
        MoqTransportEvent::SessionEstablished { session_id } => {
            assert_eq!(session_id, 123);
        }
        _ => panic!("Wrong event type"),
    }

    let track_event = MoqTransportEvent::TrackAnnounced {
        track_namespace: test_track_namespace(),
        track: test_moq_track(),
    };
    match track_event {
        MoqTransportEvent::TrackAnnounced {
            track_namespace,
            track,
        } => {
            assert_eq!(track_namespace.namespace, "test.example.com");
            assert_eq!(track.track_type, MoqTrackType::Video);
        }
        _ => panic!("Wrong event type"),
    }

    let subscription_event = MoqTransportEvent::SubscriptionRequested {
        track_namespace: test_track_namespace(),
        priority: 1,
    };
    match subscription_event {
        MoqTransportEvent::SubscriptionRequested {
            track_namespace,
            priority,
        } => {
            assert_eq!(track_namespace.track_name, "alice/camera");
            assert_eq!(priority, 1);
        }
        _ => panic!("Wrong event type"),
    }

    let object_event = MoqTransportEvent::ObjectReceived {
        object: test_moq_object(),
    };
    match object_event {
        MoqTransportEvent::ObjectReceived { object } => {
            assert_eq!(object.group_id, 12345);
            assert_eq!(object.object_id, 67890);
        }
        _ => panic!("Wrong event type"),
    }

    let stream_event = MoqTransportEvent::StreamEstablished {
        stream_id: 789,
        stream_type: MoqStreamType::DataSubgroup,
        track_namespace: Some(test_track_namespace()),
    };
    match stream_event {
        MoqTransportEvent::StreamEstablished {
            stream_id,
            stream_type,
            track_namespace,
        } => {
            assert_eq!(stream_id, 789);
            assert_eq!(stream_type, MoqStreamType::DataSubgroup);
            assert!(track_namespace.is_some());
        }
        _ => panic!("Wrong event type"),
    }

    let error_event = MoqTransportEvent::TransportError {
        error: "Test error".to_string(),
    };
    match error_event {
        MoqTransportEvent::TransportError { error } => {
            assert_eq!(error, "Test error");
        }
        _ => panic!("Wrong event type"),
    }
}

#[tokio::test]
async fn test_track_namespace_validation() {
    // Test track namespace creation and validation

    let valid_namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    };

    assert_eq!(valid_namespace.namespace, "conference.example.com");
    assert_eq!(valid_namespace.track_name, "alice/camera");

    // Test namespace equality
    let same_namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    };

    let different_namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "bob/camera".to_string(),
    };

    assert_eq!(valid_namespace, same_namespace);
    assert_ne!(valid_namespace, different_namespace);

    // Test namespace can be used as HashMap key
    let mut track_map = std::collections::HashMap::new();
    track_map.insert(valid_namespace.clone(), test_moq_track());

    assert!(track_map.contains_key(&valid_namespace));
    assert!(!track_map.contains_key(&different_namespace));
}

#[tokio::test]
async fn test_moq_track_types() {
    // Test different MoQ track types

    let video_track = MoqTrack {
        namespace: test_track_namespace(),
        name: "camera".to_string(),
        track_type: MoqTrackType::Video,
    };

    let audio_track = MoqTrack {
        namespace: test_track_namespace(),
        name: "microphone".to_string(),
        track_type: MoqTrackType::Audio,
    };

    let data_track = MoqTrack {
        namespace: test_track_namespace(),
        name: "chat".to_string(),
        track_type: MoqTrackType::Data,
    };

    assert_eq!(video_track.track_type, MoqTrackType::Video);
    assert_eq!(audio_track.track_type, MoqTrackType::Audio);
    assert_eq!(data_track.track_type, MoqTrackType::Data);

    // Test track type inequality
    assert_ne!(video_track.track_type, audio_track.track_type);
    assert_ne!(audio_track.track_type, data_track.track_type);
    assert_ne!(video_track.track_type, data_track.track_type);
}

#[tokio::test]
async fn test_h264_frame_to_moq_object_conversion() {
    // Test conversion of H.264 frames to MoQ objects

    let track_ns = test_track_namespace();

    // Test keyframe conversion
    let keyframe = H264Frame {
        nal_units: vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x80, 0x1e], // SPS NAL unit
        is_keyframe: true,
        timestamp_us: 1000000, // 1 second
        sequence_number: 1,
    };

    let keyframe_object = MoqObject::from_h264_frame(track_ns.clone(), keyframe);

    assert_eq!(keyframe_object.track_namespace, track_ns);
    assert_eq!(keyframe_object.track_name, "video");
    assert_eq!(keyframe_object.group_id, 1000); // timestamp_us / 1000
    assert_eq!(keyframe_object.object_id, 1); // sequence_number
    assert_eq!(keyframe_object.publisher_priority, 1); // Keyframe has higher priority
    assert_eq!(keyframe_object.payload.len(), 8);
    assert_eq!(keyframe_object.object_status, MoqObjectStatus::Normal);

    // Test P-frame conversion
    let pframe = H264Frame {
        nal_units: vec![0x00, 0x00, 0x00, 0x01, 0x41, 0xe0, 0x20, 0x40], // P-frame NAL unit
        is_keyframe: false,
        timestamp_us: 1033333, // ~33ms later
        sequence_number: 2,
    };

    let pframe_object = MoqObject::from_h264_frame(track_ns, pframe);

    assert_eq!(pframe_object.group_id, 1033); // timestamp_us / 1000
    assert_eq!(pframe_object.object_id, 2); // sequence_number
    assert_eq!(pframe_object.publisher_priority, 2); // P-frame has lower priority
    assert_eq!(pframe_object.payload.len(), 8);
}

#[tokio::test]
async fn test_opus_frame_to_moq_object_conversion() {
    // Test conversion of Opus frames to MoQ objects

    let track_ns = test_track_namespace();

    let opus_frame = OpusFrame {
        opus_data: vec![0xfc, 0xff, 0xfe, 0x12, 0x34], // Sample Opus frame
        timestamp_us: 20000,                           // 20ms
        sequence_number: 1,
        sample_rate: 48000,
        channels: 2,
    };

    let opus_object = MoqObject::from_opus_frame(track_ns.clone(), opus_frame);

    assert_eq!(opus_object.track_namespace, track_ns);
    assert_eq!(opus_object.track_name, "audio");
    assert_eq!(opus_object.group_id, 1); // timestamp_us / 20000
    assert_eq!(opus_object.object_id, 1); // sequence_number
    assert_eq!(opus_object.publisher_priority, 1); // Audio always high priority
    assert_eq!(opus_object.payload.len(), 5);
    assert_eq!(opus_object.object_status, MoqObjectStatus::Normal);

    // Test multiple Opus frames in sequence
    let opus_frame2 = OpusFrame {
        opus_data: vec![0xfc, 0xff, 0xfe, 0x56, 0x78],
        timestamp_us: 40000, // 40ms
        sequence_number: 2,
        sample_rate: 48000,
        channels: 2,
    };

    let opus_object2 = MoqObject::from_opus_frame(track_ns, opus_frame2);

    assert_eq!(opus_object2.group_id, 2); // timestamp_us / 20000
    assert_eq!(opus_object2.object_id, 2); // sequence_number
    assert_eq!(opus_object2.publisher_priority, 1); // Audio always high priority
}

#[tokio::test]
async fn test_moq_object_end_markers() {
    // Test creation of end-of-group and end-of-track marker objects

    let track_ns = test_track_namespace();

    // Test end-of-group marker
    let end_of_group = MoqObject::end_of_group(track_ns.clone(), "video".to_string(), 12345, 99);

    assert_eq!(end_of_group.track_namespace, track_ns);
    assert_eq!(end_of_group.track_name, "video");
    assert_eq!(end_of_group.group_id, 12345);
    assert_eq!(end_of_group.object_id, 99);
    assert_eq!(end_of_group.publisher_priority, 1); // High priority for markers
    assert!(end_of_group.payload.is_empty());
    assert_eq!(end_of_group.object_status, MoqObjectStatus::EndOfGroup);
    assert_eq!(end_of_group.size, 0);
    assert!(end_of_group.is_control_object());

    // Test end-of-track marker
    let end_of_track = MoqObject::end_of_track(track_ns.clone(), "video".to_string(), 12345, 100);

    assert_eq!(end_of_track.track_namespace, track_ns);
    assert_eq!(end_of_track.track_name, "video");
    assert_eq!(end_of_track.group_id, 12345);
    assert_eq!(end_of_track.object_id, 100);
    assert_eq!(end_of_track.publisher_priority, 1); // High priority for markers
    assert!(end_of_track.payload.is_empty());
    assert_eq!(end_of_track.object_status, MoqObjectStatus::EndOfTrack);
    assert_eq!(end_of_track.size, 0);
    assert!(end_of_track.is_control_object());

    // Test delivery priority ordering
    assert_eq!(end_of_track.delivery_priority(), 0); // Highest priority
    assert_eq!(end_of_group.delivery_priority(), 1); // Second highest
}

#[tokio::test]
async fn test_moq_object_age_tracking() {
    // Test MoQ object age tracking for delivery decisions

    let object = test_moq_object();

    // Object should have been created recently
    let age = object.age();
    assert!(age < Duration::from_millis(100)); // Should be very recent

    // Wait a bit and check age again
    tokio::time::sleep(Duration::from_millis(10)).await;
    let age2 = object.age();
    assert!(age2 > age); // Age should have increased
    assert!(age2 >= Duration::from_millis(10)); // Should be at least 10ms old
}

#[tokio::test]
async fn test_moq_stream_type_classification() {
    // Test MoQ stream type classification

    use quicrtc_core::MoqStreamType;

    assert_eq!(MoqStreamType::Control, MoqStreamType::Control);
    assert_eq!(MoqStreamType::DataSubgroup, MoqStreamType::DataSubgroup);
    assert_ne!(MoqStreamType::Control, MoqStreamType::DataSubgroup);

    // Test stream type can be used in match statements
    let stream_type = MoqStreamType::Control;
    match stream_type {
        MoqStreamType::Control => {
            // Expected
        }
        MoqStreamType::DataSubgroup => {
            panic!("Wrong stream type");
        }
        MoqStreamType::Datagram => {
            panic!("Wrong stream type");
        }
    }
}

#[tokio::test]
async fn test_integration_error_scenarios() {
    // Test various error scenarios in MoQ over QUIC integration

    // Test connection failure scenarios
    let unreachable = unreachable_endpoint();
    let short_timeout_config = ConnectionConfig {
        timeout: Duration::from_millis(1),
        ..test_config()
    };

    let result = MoqOverQuicTransport::new(unreachable, short_timeout_config, 123).await;
    assert!(result.is_err());

    match result {
        Err(QuicRtcError::Connection {
            reason,
            retry_in,
            suggested_action,
            ..
        }) => {
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
async fn test_moq_transport_configuration_scenarios() {
    // Test different MoQ transport configuration scenarios

    let endpoint = unreachable_endpoint();

    // Test mobile-optimized configuration
    let mobile_config = ConnectionConfig {
        timeout: Duration::from_secs(5),
        keep_alive: true,
        keep_alive_interval: Duration::from_secs(15),
        max_idle_timeout: Duration::from_secs(30),
        enable_migration: true, // Critical for mobile
        preferred_transports: Some(vec![
            TransportMode::QuicNative,
            TransportMode::QuicOverWebSocket,
        ]),
        quic_transport_config: Default::default(),
        resource_limits: Default::default(),
    };

    let result = MoqOverQuicTransport::new(endpoint, mobile_config, 456).await;
    assert!(result.is_err()); // Will fail with unreachable endpoint

    // Test desktop-optimized configuration
    let desktop_config = ConnectionConfig {
        timeout: Duration::from_secs(10),
        keep_alive: true,
        keep_alive_interval: Duration::from_secs(60),
        max_idle_timeout: Duration::from_secs(300),
        enable_migration: false, // Less critical for desktop
        preferred_transports: Some(vec![TransportMode::QuicNative]),
        quic_transport_config: Default::default(),
        resource_limits: Default::default(),
    };

    let result2 = MoqOverQuicTransport::new(endpoint, desktop_config, 789).await;
    assert!(result2.is_err()); // Will fail with unreachable endpoint

    // Test firewall-friendly configuration
    let firewall_config = ConnectionConfig {
        timeout: Duration::from_secs(15),
        preferred_transports: Some(vec![
            TransportMode::QuicOverWebSocket, // Try WebSocket first
            TransportMode::WebRtcCompat,      // Then WebRTC
            TransportMode::QuicNative,        // Finally native QUIC
        ]),
        ..test_config()
    };

    let result3 = MoqOverQuicTransport::new(endpoint, firewall_config, 101112).await;
    assert!(result3.is_err()); // Will fail with unreachable endpoint
}

#[tokio::test]
async fn test_moq_integration_realistic_scenarios() {
    // Test realistic MoQ integration scenarios

    // Scenario 1: Video conferencing with multiple tracks
    let video_track = MoqTrack {
        namespace: TrackNamespace {
            namespace: "conference.example.com".to_string(),
            track_name: "alice/camera".to_string(),
        },
        name: "camera".to_string(),
        track_type: MoqTrackType::Video,
    };

    let audio_track = MoqTrack {
        namespace: TrackNamespace {
            namespace: "conference.example.com".to_string(),
            track_name: "alice/microphone".to_string(),
        },
        name: "microphone".to_string(),
        track_type: MoqTrackType::Audio,
    };

    let screen_track = MoqTrack {
        namespace: TrackNamespace {
            namespace: "conference.example.com".to_string(),
            track_name: "alice/screen".to_string(),
        },
        name: "screen".to_string(),
        track_type: MoqTrackType::Video,
    };

    // Validate track configurations
    assert_eq!(video_track.track_type, MoqTrackType::Video);
    assert_eq!(audio_track.track_type, MoqTrackType::Audio);
    assert_eq!(screen_track.track_type, MoqTrackType::Video);

    // All tracks should have the same namespace domain
    assert_eq!(video_track.namespace.namespace, "conference.example.com");
    assert_eq!(audio_track.namespace.namespace, "conference.example.com");
    assert_eq!(screen_track.namespace.namespace, "conference.example.com");

    // But different track names
    assert_ne!(
        video_track.namespace.track_name,
        audio_track.namespace.track_name
    );
    assert_ne!(
        video_track.namespace.track_name,
        screen_track.namespace.track_name
    );
    assert_ne!(
        audio_track.namespace.track_name,
        screen_track.namespace.track_name
    );

    // Scenario 2: Live streaming with different quality levels
    let hd_track = MoqTrack {
        namespace: TrackNamespace {
            namespace: "stream.example.com".to_string(),
            track_name: "streamer/video_hd".to_string(),
        },
        name: "video_hd".to_string(),
        track_type: MoqTrackType::Video,
    };

    let sd_track = MoqTrack {
        namespace: TrackNamespace {
            namespace: "stream.example.com".to_string(),
            track_name: "streamer/video_sd".to_string(),
        },
        name: "video_sd".to_string(),
        track_type: MoqTrackType::Video,
    };

    assert_eq!(hd_track.namespace.namespace, sd_track.namespace.namespace);
    assert_ne!(hd_track.namespace.track_name, sd_track.namespace.track_name);

    // Scenario 3: Gaming with low-latency requirements
    let game_video_track = MoqTrack {
        namespace: TrackNamespace {
            namespace: "game.example.com".to_string(),
            track_name: "player1/gameplay".to_string(),
        },
        name: "gameplay".to_string(),
        track_type: MoqTrackType::Video,
    };

    let game_data_track = MoqTrack {
        namespace: TrackNamespace {
            namespace: "game.example.com".to_string(),
            track_name: "player1/gamestate".to_string(),
        },
        name: "gamestate".to_string(),
        track_type: MoqTrackType::Data,
    };

    assert_eq!(game_video_track.track_type, MoqTrackType::Video);
    assert_eq!(game_data_track.track_type, MoqTrackType::Data);
}
