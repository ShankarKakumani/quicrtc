//! # QUIC RTC Signaling
//!
//! Signaling server and peer discovery functionality for QUIC RTC.
//! Handles room management, participant discovery, and MoQ session negotiation.

#![deny(missing_docs)]
#![warn(clippy::all)]

pub mod discovery;
pub mod protocol;
pub mod server;

// Re-export main types
pub use discovery::PeerDiscovery;
pub use server::SignalingServer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::*;
    use crate::server::*;
    use std::net::{Ipv4Addr, SocketAddr};


    fn test_addr() -> SocketAddr {
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0)
    }

    #[test]
    fn test_room_creation() {
        let room = Room::new("test-room".to_string(), Some("Test Room".to_string()));
        assert_eq!(room.id, "test-room");
        assert_eq!(room.name, Some("Test Room".to_string()));
        assert_eq!(room.participants.len(), 0);
        assert_eq!(room.max_participants, 100);
    }

    #[test]
    fn test_participant_creation() {
        let participant = Participant {
            id: "test-participant".to_string(),
            name: Some("Test User".to_string()),
            connection_id: "conn-123".to_string(),
            capabilities: vec!["h264".to_string(), "opus".to_string()],
            quic_endpoint: Some(test_addr()),
        };

        assert_eq!(participant.id, "test-participant");
        assert_eq!(participant.name, Some("Test User".to_string()));
        assert_eq!(participant.capabilities, vec!["h264", "opus"]);
        assert!(participant.quic_endpoint.is_some());
    }

    #[test]
    fn test_room_participant_management() {
        let mut room = Room::new("test-room".to_string(), None);

        let participant1 = Participant {
            id: "participant1".to_string(),
            name: Some("User 1".to_string()),
            connection_id: "conn-1".to_string(),
            capabilities: vec![],
            quic_endpoint: None,
        };

        let participant2 = Participant {
            id: "participant2".to_string(),
            name: Some("User 2".to_string()),
            connection_id: "conn-2".to_string(),
            capabilities: vec![],
            quic_endpoint: None,
        };

        // Test adding participants
        assert!(room.add_participant(participant1.clone()).is_ok());
        assert_eq!(room.participants.len(), 1);

        assert!(room.add_participant(participant2.clone()).is_ok());
        assert_eq!(room.participants.len(), 2);

        // Test duplicate participant
        let duplicate = Participant {
            id: "participant1".to_string(), // Same ID
            name: Some("Duplicate User".to_string()),
            connection_id: "conn-3".to_string(),
            capabilities: vec![],
            quic_endpoint: None,
        };
        assert!(room.add_participant(duplicate).is_err());

        // Test getting participant
        assert!(room.get_participant("participant1").is_some());
        assert!(room.get_participant("nonexistent").is_none());

        // Test other participants
        let others = room.other_participants("participant1");
        assert_eq!(others.len(), 1);
        assert_eq!(others[0].id, "participant2");

        // Test removing participant
        let removed = room.remove_participant("participant1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "participant1");
        assert_eq!(room.participants.len(), 1);
    }

    #[test]
    fn test_room_max_participants() {
        let mut room = Room::new("test-room".to_string(), None);
        room.max_participants = 2;

        let participant1 = Participant {
            id: "participant1".to_string(),
            name: None,
            connection_id: "conn-1".to_string(),
            capabilities: vec![],
            quic_endpoint: None,
        };

        let participant2 = Participant {
            id: "participant2".to_string(),
            name: None,
            connection_id: "conn-2".to_string(),
            capabilities: vec![],
            quic_endpoint: None,
        };

        let participant3 = Participant {
            id: "participant3".to_string(),
            name: None,
            connection_id: "conn-3".to_string(),
            capabilities: vec![],
            quic_endpoint: None,
        };

        // Add participants up to limit
        assert!(room.add_participant(participant1).is_ok());
        assert!(room.add_participant(participant2).is_ok());

        // Adding one more should fail
        assert!(room.add_participant(participant3).is_err());
    }

    #[test]
    fn test_signaling_server_creation() {
        let server = SignalingServer::new(test_addr());
        assert_eq!(server.get_rooms().len(), 0);
        assert_eq!(server.total_participants(), 0);
    }

    #[test]
    fn test_signaling_message_serialization() {
        let join_message = SignalingMessage::JoinRoom {
            room_id: "test-room".to_string(),
            participant_id: "user-123".to_string(),
            participant_name: Some("Test User".to_string()),
            capabilities: vec!["h264".to_string()],
            quic_endpoint: Some(test_addr()),
        };

        // Test serialization
        let json = serde_json::to_string(&join_message).unwrap();
        assert!(json.contains("JoinRoom"));
        assert!(json.contains("test-room"));
        assert!(json.contains("user-123"));

        // Test deserialization
        let deserialized: SignalingMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            SignalingMessage::JoinRoom {
                room_id,
                participant_id,
                ..
            } => {
                assert_eq!(room_id, "test-room");
                assert_eq!(participant_id, "user-123");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_signaling_response_serialization() {
        let response = SignalingResponse::JoinedRoom {
            room_id: "test-room".to_string(),
            participant_id: "user-123".to_string(),
        };

        // Test serialization
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("JoinedRoom"));

        // Test deserialization
        let deserialized: SignalingResponse = serde_json::from_str(&json).unwrap();
        match deserialized {
            SignalingResponse::JoinedRoom {
                room_id,
                participant_id,
            } => {
                assert_eq!(room_id, "test-room");
                assert_eq!(participant_id, "user-123");
            }
            _ => panic!("Wrong response type"),
        }
    }

    #[test]
    fn test_moq_session_offer_serialization() {
        let offer = MoqSessionOffer {
            participant_id: "participant-1".to_string(),
            quic_endpoint: test_addr(),
            moq_version: "draft-ietf-moq-transport-04".to_string(),
            publish_namespaces: vec!["video/camera".to_string()],
            subscribe_namespaces: vec!["video/camera".to_string(), "audio/mic".to_string()],
            capabilities: vec!["h264".to_string(), "opus".to_string()],
            session_id: "session-123".to_string(),
        };

        // Test serialization
        let json = serde_json::to_string(&offer).unwrap();
        assert!(json.contains("participant-1"));
        assert!(json.contains("session-123"));

        // Test deserialization
        let deserialized: MoqSessionOffer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.participant_id, "participant-1");
        assert_eq!(deserialized.session_id, "session-123");
        assert_eq!(deserialized.capabilities.len(), 2);
    }

    #[test]
    fn test_moq_session_answer_serialization() {
        let answer = MoqSessionAnswer {
            participant_id: "participant-2".to_string(),
            quic_endpoint: test_addr(),
            moq_version: "draft-ietf-moq-transport-04".to_string(),
            accepted_publish_namespaces: vec!["video/camera".to_string()],
            accepted_subscribe_namespaces: vec!["audio/mic".to_string()],
            session_id: "session-123".to_string(),
            accepted: true,
        };

        // Test serialization
        let json = serde_json::to_string(&answer).unwrap();
        assert!(json.contains("participant-2"));
        assert!(json.contains("true"));

        // Test deserialization
        let deserialized: MoqSessionAnswer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.participant_id, "participant-2");
        assert_eq!(deserialized.session_id, "session-123");
        assert!(deserialized.accepted);
    }

    #[test]
    fn test_protocol_message_variants() {
        // Test all SignalingMessage variants
        let messages = vec![
            SignalingMessage::JoinRoom {
                room_id: "room1".to_string(),
                participant_id: "user1".to_string(),
                participant_name: None,
                capabilities: vec![],
                quic_endpoint: None,
            },
            SignalingMessage::LeaveRoom {
                room_id: "room1".to_string(),
                participant_id: "user1".to_string(),
            },
            SignalingMessage::CreateRoom {
                room_id: "room1".to_string(),
                room_name: Some("Test Room".to_string()),
                max_participants: Some(50),
            },
            SignalingMessage::ListRooms,
            SignalingMessage::GetRoomInfo {
                room_id: "room1".to_string(),
            },
        ];

        for message in messages {
            let json = serde_json::to_string(&message).unwrap();
            let _deserialized: SignalingMessage = serde_json::from_str(&json).unwrap();
            // If we get here without panicking, serialization/deserialization works
        }
    }

    #[tokio::test]
    async fn test_peer_discovery_creation() {
        let discovery = PeerDiscovery::new();
        let peers = discovery.discover_peers("test-room").await.unwrap();
        assert_eq!(peers.len(), 0); // Should be empty for placeholder implementation
    }

    #[test]
    fn test_participant_serialization() {
        let participant = Participant {
            id: "test-participant".to_string(),
            name: Some("Test User".to_string()),
            connection_id: "conn-123".to_string(),
            capabilities: vec!["h264".to_string()],
            quic_endpoint: Some(test_addr()),
        };

        // Test serialization
        let json = serde_json::to_string(&participant).unwrap();
        assert!(json.contains("test-participant"));
        assert!(json.contains("Test User"));
        assert!(json.contains("h264"));

        // Test deserialization
        let deserialized: Participant = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-participant");
        assert_eq!(deserialized.name, Some("Test User".to_string()));
        assert_eq!(deserialized.capabilities, vec!["h264"]);
    }
}
