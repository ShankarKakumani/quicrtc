//! Peer discovery integration tests
//!
//! Tests for peer discovery functionality including:
//! - Capability-based peer matching
//! - Room state synchronization  
//! - Network partition recovery
//! - Automatic cleanup of offline peers
//! - Event-driven discovery workflows

use chrono::Utc;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::timeout;

use quicrtc_signaling::{
    DiscoveryConfig, DiscoveryEvent, PeerDiscovery, PeerInfo, PeerStatus, RoomStats,
};

fn get_test_addr() -> SocketAddr {
    SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0)
}

fn create_test_peer(id: &str, room_id: &str, capabilities: Vec<String>) -> PeerInfo {
    PeerInfo {
        id: id.to_string(),
        name: Some(format!("Test Peer {}", id)),
        room_id: room_id.to_string(),
        quic_endpoint: Some(get_test_addr()),
        capabilities,
        last_seen: Utc::now(),
        status: PeerStatus::Online,
    }
}

#[tokio::test]
async fn test_peer_capability_matching() {
    let discovery = PeerDiscovery::new();
    discovery.start().await.unwrap();

    // Add peers with different capabilities
    let video_peer = create_test_peer(
        "video-peer",
        "media-room",
        vec!["h264".to_string(), "vp8".to_string()],
    );
    let audio_peer = create_test_peer(
        "audio-peer",
        "media-room",
        vec!["opus".to_string(), "g722".to_string()],
    );
    let full_peer = create_test_peer(
        "full-peer",
        "media-room",
        vec!["h264".to_string(), "opus".to_string()],
    );

    discovery.add_peer(video_peer).await.unwrap();
    discovery.add_peer(audio_peer).await.unwrap();
    discovery.add_peer(full_peer).await.unwrap();

    // Find peers with video capabilities
    let video_peers = discovery
        .find_peers_with_capabilities("media-room", &["h264".to_string()])
        .await
        .unwrap();
    assert_eq!(video_peers.len(), 2); // video-peer and full-peer

    let peer_ids: Vec<&str> = video_peers.iter().map(|p| p.id.as_str()).collect();
    assert!(peer_ids.contains(&"video-peer"));
    assert!(peer_ids.contains(&"full-peer"));

    // Find peers with audio capabilities
    let audio_peers = discovery
        .find_peers_with_capabilities("media-room", &["opus".to_string()])
        .await
        .unwrap();
    assert_eq!(audio_peers.len(), 2); // audio-peer and full-peer

    let peer_ids: Vec<&str> = audio_peers.iter().map(|p| p.id.as_str()).collect();
    assert!(peer_ids.contains(&"audio-peer"));
    assert!(peer_ids.contains(&"full-peer"));

    // Find peers with both video and audio
    let multimedia_peers = discovery
        .find_peers_with_capabilities("media-room", &["h264".to_string(), "opus".to_string()])
        .await
        .unwrap();
    assert_eq!(multimedia_peers.len(), 1); // Only full-peer
    assert_eq!(multimedia_peers[0].id, "full-peer");

    // Find peers with non-existent capability
    let special_peers = discovery
        .find_peers_with_capabilities("media-room", &["av1".to_string()])
        .await
        .unwrap();
    assert_eq!(special_peers.len(), 0);
}

#[tokio::test]
async fn test_peer_status_filtering() {
    let discovery = PeerDiscovery::new();
    discovery.start().await.unwrap();

    // Add peers with different statuses
    let online_peer = create_test_peer("online-peer", "status-room", vec!["h264".to_string()]);
    let away_peer = create_test_peer("away-peer", "status-room", vec!["opus".to_string()]);
    let offline_peer = create_test_peer("offline-peer", "status-room", vec!["vp8".to_string()]);

    discovery.add_peer(online_peer).await.unwrap();
    discovery.add_peer(away_peer).await.unwrap();
    discovery.add_peer(offline_peer).await.unwrap();

    // Update statuses
    discovery
        .update_peer_status("status-room", "away-peer", PeerStatus::Away)
        .await
        .unwrap();
    discovery
        .update_peer_status("status-room", "offline-peer", PeerStatus::Offline)
        .await
        .unwrap();

    // Get peers by status
    let online_peers = discovery
        .get_peers_by_status("status-room", PeerStatus::Online)
        .await
        .unwrap();
    assert_eq!(online_peers.len(), 1);
    assert_eq!(online_peers[0].id, "online-peer");

    let away_peers = discovery
        .get_peers_by_status("status-room", PeerStatus::Away)
        .await
        .unwrap();
    assert_eq!(away_peers.len(), 1);
    assert_eq!(away_peers[0].id, "away-peer");

    let offline_peers = discovery
        .get_peers_by_status("status-room", PeerStatus::Offline)
        .await
        .unwrap();
    assert_eq!(offline_peers.len(), 1);
    assert_eq!(offline_peers[0].id, "offline-peer");
}

#[tokio::test]
async fn test_room_synchronization_with_conflicts() {
    let discovery = PeerDiscovery::new();
    discovery.start().await.unwrap();

    let mut event_receiver = discovery.subscribe_events();

    // Add initial peers
    let peer1 = create_test_peer("peer-1", "sync-room", vec!["h264".to_string()]);
    let peer2 = create_test_peer("peer-2", "sync-room", vec!["opus".to_string()]);

    discovery.add_peer(peer1.clone()).await.unwrap();
    discovery.add_peer(peer2.clone()).await.unwrap();

    // Wait for discovery events
    let _ = event_receiver.recv().await.unwrap(); // peer-1 discovered
    let _ = event_receiver.recv().await.unwrap(); // peer-2 discovered

    // Simulate network partition recovery with updated peer list
    let updated_peer1 = PeerInfo {
        id: "peer-1".to_string(),
        name: Some("Updated Peer 1".to_string()),
        room_id: "sync-room".to_string(),
        quic_endpoint: Some(SocketAddr::new(
            Ipv4Addr::new(192, 168, 1, 100).into(),
            8080,
        )),
        capabilities: vec!["h264".to_string(), "vp9".to_string()], // Added capability
        last_seen: Utc::now(),
        status: PeerStatus::Online,
    };

    let new_peer3 = create_test_peer("peer-3", "sync-room", vec!["av1".to_string()]);

    // peer-2 is missing from the sync (simulating it went offline)
    discovery
        .synchronize_room("sync-room", vec![updated_peer1.clone(), new_peer3.clone()])
        .await
        .unwrap();

    // Should receive synchronization event
    let sync_event = timeout(Duration::from_secs(2), event_receiver.recv())
        .await
        .expect("Should receive sync event")
        .unwrap();

    match sync_event {
        DiscoveryEvent::RoomSynchronized {
            room_id,
            peer_count,
        } => {
            assert_eq!(room_id, "sync-room");
            assert_eq!(peer_count, 2); // updated_peer1 and new_peer3
        }
        _ => panic!("Expected RoomSynchronized event, got: {:?}", sync_event),
    }

    // Verify final state
    let final_peers = discovery.discover_peers("sync-room").await.unwrap();
    assert_eq!(final_peers.len(), 2);

    let peer_ids: Vec<&str> = final_peers.iter().map(|p| p.id.as_str()).collect();
    assert!(peer_ids.contains(&"peer-1"));
    assert!(peer_ids.contains(&"peer-3"));
    assert!(!peer_ids.contains(&"peer-2")); // Should be removed

    // Verify peer-1 was updated
    let updated_peer = final_peers.iter().find(|p| p.id == "peer-1").unwrap();
    assert_eq!(updated_peer.name, Some("Updated Peer 1".to_string()));
    assert_eq!(updated_peer.capabilities.len(), 2); // h264 and vp9
}

#[tokio::test]
async fn test_room_statistics() {
    let discovery = PeerDiscovery::new();
    discovery.start().await.unwrap();

    // Add peers to a room
    let peer1 = create_test_peer("stats-peer-1", "stats-room", vec!["h264".to_string()]);
    let peer2 = create_test_peer("stats-peer-2", "stats-room", vec!["opus".to_string()]);
    let peer3 = create_test_peer("stats-peer-3", "stats-room", vec!["vp8".to_string()]);

    discovery.add_peer(peer1).await.unwrap();
    discovery.add_peer(peer2).await.unwrap();
    discovery.add_peer(peer3).await.unwrap();

    // Update peer statuses
    discovery
        .update_peer_status("stats-room", "stats-peer-2", PeerStatus::Away)
        .await
        .unwrap();
    discovery
        .update_peer_status("stats-room", "stats-peer-3", PeerStatus::Offline)
        .await
        .unwrap();

    // Get room statistics
    let stats = discovery.get_room_stats("stats-room").await.unwrap();

    assert_eq!(stats.room_id, "stats-room");
    assert_eq!(stats.total_peers, 3);
    assert_eq!(stats.online_peers, 1);
    assert_eq!(stats.away_peers, 1);
    assert_eq!(stats.offline_peers, 1);
}

#[tokio::test]
async fn test_discovery_with_room_capacity_limits() {
    let config = DiscoveryConfig {
        cleanup_interval: 60,
        offline_timeout: 300,
        max_peers_per_room: 2, // Small limit for testing
    };

    let discovery = PeerDiscovery::new_with_config(config);
    discovery.start().await.unwrap();

    // Add peers up to capacity
    let peer1 = create_test_peer("capacity-peer-1", "capacity-room", vec!["h264".to_string()]);
    let peer2 = create_test_peer("capacity-peer-2", "capacity-room", vec!["opus".to_string()]);

    assert!(discovery.add_peer(peer1).await.is_ok());
    assert!(discovery.add_peer(peer2).await.is_ok());

    // Adding one more should fail
    let peer3 = create_test_peer("capacity-peer-3", "capacity-room", vec!["vp8".to_string()]);
    let result = discovery.add_peer(peer3).await;

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("Room full"));
    }

    // But adding to a different room should work
    let peer4 = create_test_peer("capacity-peer-4", "other-room", vec!["av1".to_string()]);
    assert!(discovery.add_peer(peer4).await.is_ok());
}

#[tokio::test]
async fn test_event_driven_discovery_workflow() {
    let discovery = PeerDiscovery::new();
    discovery.start().await.unwrap();

    let mut event_receiver = discovery.subscribe_events();

    // Create a test workflow: discovery -> status change -> removal
    let peer = create_test_peer("workflow-peer", "workflow-room", vec!["h264".to_string()]);

    // Step 1: Add peer (should trigger PeerDiscovered)
    discovery.add_peer(peer).await.unwrap();

    let event = timeout(Duration::from_secs(2), event_receiver.recv())
        .await
        .expect("Should receive discovery event")
        .unwrap();

    match event {
        DiscoveryEvent::PeerDiscovered { room_id, peer } => {
            assert_eq!(room_id, "workflow-room");
            assert_eq!(peer.id, "workflow-peer");
            assert_eq!(peer.status, PeerStatus::Online);
        }
        _ => panic!("Expected PeerDiscovered, got: {:?}", event),
    }

    // Step 2: Change status (should trigger PeerStatusChanged)
    discovery
        .update_peer_status("workflow-room", "workflow-peer", PeerStatus::Away)
        .await
        .unwrap();

    let event = timeout(Duration::from_secs(2), event_receiver.recv())
        .await
        .expect("Should receive status change event")
        .unwrap();

    match event {
        DiscoveryEvent::PeerStatusChanged {
            room_id,
            peer_id,
            old_status,
            new_status,
        } => {
            assert_eq!(room_id, "workflow-room");
            assert_eq!(peer_id, "workflow-peer");
            assert_eq!(old_status, PeerStatus::Online);
            assert_eq!(new_status, PeerStatus::Away);
        }
        _ => panic!("Expected PeerStatusChanged, got: {:?}", event),
    }

    // Step 3: Remove peer (should trigger PeerLeft)
    discovery
        .remove_peer("workflow-room", "workflow-peer")
        .await
        .unwrap();

    let event = timeout(Duration::from_secs(2), event_receiver.recv())
        .await
        .expect("Should receive peer left event")
        .unwrap();

    match event {
        DiscoveryEvent::PeerLeft { room_id, peer_id } => {
            assert_eq!(room_id, "workflow-room");
            assert_eq!(peer_id, "workflow-peer");
        }
        _ => panic!("Expected PeerLeft, got: {:?}", event),
    }

    // Verify peer is actually gone
    let peers = discovery.discover_peers("workflow-room").await.unwrap();
    assert_eq!(peers.len(), 0);
}

#[tokio::test]
async fn test_multiple_rooms_isolation() {
    let discovery = PeerDiscovery::new();
    discovery.start().await.unwrap();

    // Add peers to different rooms
    let room1_peer1 = create_test_peer("r1-peer1", "room-1", vec!["h264".to_string()]);
    let room1_peer2 = create_test_peer("r1-peer2", "room-1", vec!["opus".to_string()]);
    let room2_peer1 = create_test_peer("r2-peer1", "room-2", vec!["vp8".to_string()]);
    let room2_peer2 = create_test_peer("r2-peer2", "room-2", vec!["g722".to_string()]);

    discovery.add_peer(room1_peer1).await.unwrap();
    discovery.add_peer(room1_peer2).await.unwrap();
    discovery.add_peer(room2_peer1).await.unwrap();
    discovery.add_peer(room2_peer2).await.unwrap();

    // Verify room isolation
    let room1_peers = discovery.discover_peers("room-1").await.unwrap();
    assert_eq!(room1_peers.len(), 2);
    for peer in &room1_peers {
        assert!(peer.id.starts_with("r1-"));
        assert_eq!(peer.room_id, "room-1");
    }

    let room2_peers = discovery.discover_peers("room-2").await.unwrap();
    assert_eq!(room2_peers.len(), 2);
    for peer in &room2_peers {
        assert!(peer.id.starts_with("r2-"));
        assert_eq!(peer.room_id, "room-2");
    }

    // Verify nonexistent room
    let empty_room_peers = discovery.discover_peers("room-3").await.unwrap();
    assert_eq!(empty_room_peers.len(), 0);

    // Test capability search within specific rooms
    let h264_peers_room1 = discovery
        .find_peers_with_capabilities("room-1", &["h264".to_string()])
        .await
        .unwrap();
    assert_eq!(h264_peers_room1.len(), 1);
    assert_eq!(h264_peers_room1[0].id, "r1-peer1");

    let h264_peers_room2 = discovery
        .find_peers_with_capabilities("room-2", &["h264".to_string()])
        .await
        .unwrap();
    assert_eq!(h264_peers_room2.len(), 0); // No H.264 peers in room-2

    // Get active rooms
    let active_rooms = discovery.get_active_rooms().await;
    assert_eq!(active_rooms.len(), 2);
    assert!(active_rooms.contains(&"room-1".to_string()));
    assert!(active_rooms.contains(&"room-2".to_string()));
}

#[tokio::test]
async fn test_peer_cleanup_edge_cases() {
    let discovery = PeerDiscovery::new();
    discovery.start().await.unwrap();

    // Test removing non-existent peer
    let result = discovery
        .remove_peer("nonexistent-room", "nonexistent-peer")
        .await;
    assert!(result.is_ok()); // Should not fail

    // Test updating status of non-existent peer
    let result = discovery
        .update_peer_status("nonexistent-room", "nonexistent-peer", PeerStatus::Away)
        .await;
    assert!(result.is_err()); // Should fail with proper error

    // Test getting stats for non-existent room
    let result = discovery.get_room_stats("nonexistent-room").await;
    assert!(result.is_err()); // Should fail

    // Add a peer and then remove the entire room by removing all peers
    let peer = create_test_peer("cleanup-peer", "cleanup-room", vec!["h264".to_string()]);
    discovery.add_peer(peer).await.unwrap();

    // Verify room exists
    let active_rooms = discovery.get_active_rooms().await;
    assert!(active_rooms.contains(&"cleanup-room".to_string()));

    // Remove the peer
    discovery
        .remove_peer("cleanup-room", "cleanup-peer")
        .await
        .unwrap();

    // Verify room is cleaned up
    let active_rooms = discovery.get_active_rooms().await;
    assert!(!active_rooms.contains(&"cleanup-room".to_string()));
}
