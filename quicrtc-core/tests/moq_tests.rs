//! Unit tests for MoQ session management
//!
//! This module contains all unit tests for the Media over QUIC (MoQ) implementation.
//! Tests cover session management, object delivery, caching, and protocol compliance.

use quicrtc_core::*;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_moq_session_creation() {
    let session = MoqSession::new(12345);

    assert_eq!(session.session_id(), 12345);
    assert_eq!(session.state(), &MoqSessionState::Establishing);
    assert!(session.announced_tracks().is_empty());
    assert!(session.subscriptions().is_empty());
    assert!(session.peer_capabilities().is_none());
}

#[tokio::test]
async fn test_moq_session_with_custom_capabilities() {
    let capabilities = MoqCapabilities {
        version: 2,
        max_tracks: 50,
        supported_track_types: vec![MoqTrackType::Video],
        max_object_size: 512 * 1024,
        supports_caching: false,
    };

    let session = MoqSession::new_with_capabilities(67890, capabilities.clone());

    assert_eq!(session.session_id(), 67890);
    assert_eq!(session.capabilities().version, 2);
    assert_eq!(session.capabilities().max_tracks, 50);
    assert_eq!(
        session.capabilities().supported_track_types,
        vec![MoqTrackType::Video]
    );
    assert_eq!(session.capabilities().max_object_size, 512 * 1024);
    assert!(!session.capabilities().supports_caching);
}

#[tokio::test]
async fn test_default_capabilities() {
    let capabilities = MoqCapabilities::default();

    assert_eq!(capabilities.version, 1);
    assert_eq!(capabilities.max_tracks, 100);
    assert_eq!(
        capabilities.supported_track_types,
        vec![MoqTrackType::Audio, MoqTrackType::Video, MoqTrackType::Data,]
    );
    assert_eq!(capabilities.max_object_size, 1024 * 1024);
    assert!(capabilities.supports_caching);
}

#[tokio::test]
async fn test_track_namespace_equality() {
    let namespace1 = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    };

    let namespace2 = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    };

    let namespace3 = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "bob/camera".to_string(),
    };

    assert_eq!(namespace1, namespace2);
    assert_ne!(namespace1, namespace3);
}

#[tokio::test]
async fn test_moq_track_creation() {
    let namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    };

    let track = MoqTrack {
        namespace: namespace.clone(),
        name: "camera".to_string(),
        track_type: MoqTrackType::Video,
    };

    assert_eq!(track.namespace, namespace);
    assert_eq!(track.name, "camera");
    assert_eq!(track.track_type, MoqTrackType::Video);
}

#[tokio::test]
async fn test_moq_subscription_states() {
    let namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/audio".to_string(),
    };

    let subscription = MoqSubscription {
        track_namespace: namespace.clone(),
        state: MoqSubscriptionState::Pending,
        priority: 1,
        start_group: None,
        end_group: None,
    };

    assert_eq!(subscription.track_namespace, namespace);
    assert_eq!(subscription.state, MoqSubscriptionState::Pending);
    assert_eq!(subscription.priority, 1);
    assert!(subscription.start_group.is_none());
    assert!(subscription.end_group.is_none());

    // Test state transitions
    assert_ne!(MoqSubscriptionState::Pending, MoqSubscriptionState::Active);
    assert_ne!(
        MoqSubscriptionState::Active,
        MoqSubscriptionState::Terminated
    );
    assert_ne!(
        MoqSubscriptionState::Pending,
        MoqSubscriptionState::Rejected("error".to_string())
    );
}

#[tokio::test]
async fn test_moq_object_creation() {
    let namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/video".to_string(),
    };

    let object = MoqObject {
        track_namespace: namespace.clone(),
        track_name: "video".to_string(),
        group_id: 12345,
        object_id: 67890,
        publisher_priority: 1,
        payload: vec![1, 2, 3, 4, 5],
        object_status: MoqObjectStatus::Normal,
        created_at: Instant::now(),
        size: 5,
    };

    assert_eq!(object.track_namespace, namespace);
    assert_eq!(object.track_name, "video");
    assert_eq!(object.group_id, 12345);
    assert_eq!(object.object_id, 67890);
    assert_eq!(object.publisher_priority, 1);
    assert_eq!(object.payload, vec![1, 2, 3, 4, 5]);
    assert_eq!(object.size, 5);

    match object.object_status {
        MoqObjectStatus::Normal => {}
        _ => panic!("Expected Normal status"),
    }
}

#[tokio::test]
async fn test_h264_frame_to_moq_object() {
    let namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    };

    let h264_frame = H264Frame {
        nal_units: vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42], // Sample H.264 NAL units
        is_keyframe: true,
        timestamp_us: 1000000, // 1 second
        sequence_number: 42,
    };

    let moq_object = MoqObject::from_h264_frame(namespace.clone(), h264_frame.clone());

    assert_eq!(moq_object.track_namespace, namespace);
    assert_eq!(moq_object.track_name, "video");
    assert_eq!(moq_object.group_id, 1000); // timestamp_us / 1000
    assert_eq!(moq_object.object_id, 42);
    assert_eq!(moq_object.publisher_priority, 1); // Keyframe has priority 1
    assert_eq!(moq_object.payload, h264_frame.nal_units);
    assert_eq!(moq_object.size, 6);
    assert!(matches!(moq_object.object_status, MoqObjectStatus::Normal));
    assert_eq!(moq_object.delivery_priority(), 1);
    assert!(!moq_object.is_control_object());
}

#[tokio::test]
async fn test_opus_frame_to_moq_object() {
    let namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/microphone".to_string(),
    };

    let opus_frame = OpusFrame {
        opus_data: vec![0xFC, 0xFF, 0xFE], // Sample Opus data
        timestamp_us: 40000,               // 40ms
        sequence_number: 100,
        sample_rate: 48000,
        channels: 2,
    };

    let moq_object = MoqObject::from_opus_frame(namespace.clone(), opus_frame.clone());

    assert_eq!(moq_object.track_namespace, namespace);
    assert_eq!(moq_object.track_name, "audio");
    assert_eq!(moq_object.group_id, 2); // timestamp_us / 20000 (20ms groups)
    assert_eq!(moq_object.object_id, 100);
    assert_eq!(moq_object.publisher_priority, 1); // Audio always priority 1
    assert_eq!(moq_object.payload, opus_frame.opus_data);
    assert_eq!(moq_object.size, 3);
    assert!(matches!(moq_object.object_status, MoqObjectStatus::Normal));
    assert_eq!(moq_object.delivery_priority(), 1);
}

#[tokio::test]
async fn test_moq_object_delivery_creation() {
    let cache_config = MoqCacheConfig::default();
    let delivery = MoqObjectDelivery::new(cache_config);

    assert_eq!(delivery.delivery_stats().objects_delivered, 0);
    assert_eq!(delivery.delivery_stats().objects_dropped, 0);
    assert_eq!(delivery.delivery_stats().queue_depth, 0);
    assert_eq!(delivery.cache_stats().total_objects, 0);
}

#[tokio::test]
async fn test_moq_object_delivery_priority_ordering() {
    let cache_config = MoqCacheConfig::default();
    let mut delivery = MoqObjectDelivery::new(cache_config);

    let namespace = TrackNamespace {
        namespace: "test.com".to_string(),
        track_name: "test/video".to_string(),
    };

    // Create objects with different statuses (different priorities)
    let normal_object = MoqObject {
        track_namespace: namespace.clone(),
        track_name: "video".to_string(),
        group_id: 1,
        object_id: 1,
        publisher_priority: 2,
        payload: vec![1],
        object_status: MoqObjectStatus::Normal,
        created_at: Instant::now(),
        size: 1,
    };

    let eog_object = MoqObject::end_of_group(namespace.clone(), "video".to_string(), 1, 2);
    let eot_object = MoqObject::end_of_track(namespace.clone(), "video".to_string(), 1, 3);

    // Enqueue in reverse priority order
    delivery.enqueue_object(normal_object).unwrap();
    delivery.enqueue_object(eog_object).unwrap();
    delivery.enqueue_object(eot_object).unwrap();

    // Dequeue should return in priority order: EndOfTrack (0), EndOfGroup (1), Normal (2)
    let dequeued1 = delivery.dequeue_object().unwrap();
    assert!(matches!(
        dequeued1.object_status,
        MoqObjectStatus::EndOfTrack
    ));

    let dequeued2 = delivery.dequeue_object().unwrap();
    assert!(matches!(
        dequeued2.object_status,
        MoqObjectStatus::EndOfGroup
    ));

    let dequeued3 = delivery.dequeue_object().unwrap();
    assert!(matches!(dequeued3.object_status, MoqObjectStatus::Normal));
}

#[tokio::test]
async fn test_moq_object_cache_basic_operations() {
    let config = MoqCacheConfig::default();
    let mut cache = MoqObjectCache::new(config);

    let namespace = TrackNamespace {
        namespace: "test.com".to_string(),
        track_name: "test/video".to_string(),
    };

    let object = MoqObject {
        track_namespace: namespace.clone(),
        track_name: "video".to_string(),
        group_id: 1,
        object_id: 42,
        publisher_priority: 1,
        payload: vec![1, 2, 3, 4, 5],
        object_status: MoqObjectStatus::Normal,
        created_at: Instant::now(),
        size: 5,
    };

    // Store object
    cache.store_object(object.clone()).unwrap();
    assert_eq!(cache.stats().total_objects, 1);
    assert_eq!(cache.stats().current_size_bytes, 5);

    // Retrieve object
    let retrieved = cache.get_object(&namespace, 42).unwrap();
    assert_eq!(retrieved.object_id, 42);
    assert_eq!(retrieved.payload, vec![1, 2, 3, 4, 5]);
    assert_eq!(cache.stats().cache_hits, 1);
    assert_eq!(cache.stats().cache_misses, 0);

    // Try to retrieve non-existent object
    let not_found = cache.get_object(&namespace, 999);
    assert!(not_found.is_none());
    assert_eq!(cache.stats().cache_misses, 1);
}

#[tokio::test]
async fn test_media_frame_structures() {
    // Test H264Frame
    let h264_frame = H264Frame {
        nal_units: vec![0x00, 0x00, 0x00, 0x01, 0x67],
        is_keyframe: true,
        timestamp_us: 1000000,
        sequence_number: 1,
    };

    assert_eq!(h264_frame.nal_units.len(), 5);
    assert!(h264_frame.is_keyframe);
    assert_eq!(h264_frame.timestamp_us, 1000000);
    assert_eq!(h264_frame.sequence_number, 1);

    // Test OpusFrame
    let opus_frame = OpusFrame {
        opus_data: vec![0xFC, 0xFF, 0xFE],
        timestamp_us: 20000,
        sequence_number: 100,
        sample_rate: 48000,
        channels: 2,
    };

    assert_eq!(opus_frame.opus_data.len(), 3);
    assert_eq!(opus_frame.timestamp_us, 20000);
    assert_eq!(opus_frame.sequence_number, 100);
    assert_eq!(opus_frame.sample_rate, 48000);
    assert_eq!(opus_frame.channels, 2);
}
