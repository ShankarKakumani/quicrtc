//! Signaling server implementation

use crate::protocol::{MoqSessionAnswer, MoqSessionOffer, SignalingMessage, SignalingResponse};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use quicrtc_core::QuicRtcError;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

/// Participant information in a room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    /// Unique participant ID
    pub id: String,
    /// Display name
    pub name: Option<String>,
    /// WebSocket connection for signaling
    pub connection_id: String,
    /// MoQ session capabilities
    pub capabilities: Vec<String>,
    /// QUIC endpoint address for direct connection
    pub quic_endpoint: Option<SocketAddr>,
}

/// Room state and participant management
#[derive(Debug, Clone)]
pub struct Room {
    /// Unique room ID
    pub id: String,
    /// Room display name
    pub name: Option<String>,
    /// List of participants in the room
    pub participants: HashMap<String, Participant>,
    /// Room creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Maximum number of participants allowed
    pub max_participants: usize,
    /// Room metadata
    pub metadata: HashMap<String, String>,
}

impl Room {
    /// Create a new room
    pub fn new(id: String, name: Option<String>) -> Self {
        Self {
            id,
            name,
            participants: HashMap::new(),
            created_at: chrono::Utc::now(),
            max_participants: 100, // Default limit
            metadata: HashMap::new(),
        }
    }

    /// Add a participant to the room
    pub fn add_participant(&mut self, participant: Participant) -> Result<(), QuicRtcError> {
        if self.participants.len() >= self.max_participants {
            return Err(QuicRtcError::RoomFull {
                room_id: self.id.clone(),
                max_participants: self.max_participants,
            });
        }

        if self.participants.contains_key(&participant.id) {
            return Err(QuicRtcError::ParticipantAlreadyExists {
                room_id: self.id.clone(),
                participant_id: participant.id,
            });
        }

        self.participants
            .insert(participant.id.clone(), participant);
        Ok(())
    }

    /// Remove a participant from the room
    pub fn remove_participant(&mut self, participant_id: &str) -> Option<Participant> {
        self.participants.remove(participant_id)
    }

    /// Get participant by ID
    pub fn get_participant(&self, participant_id: &str) -> Option<&Participant> {
        self.participants.get(participant_id)
    }

    /// List all participants except the specified one
    pub fn other_participants(&self, exclude_id: &str) -> Vec<&Participant> {
        self.participants
            .values()
            .filter(|p| p.id != exclude_id)
            .collect()
    }
}

/// WebSocket connection wrapper
type WebSocketConnection = WebSocketStream<TcpStream>;

/// Active WebSocket connections mapped by connection ID
type Connections = Arc<DashMap<String, WebSocketConnection>>;

/// Signaling server for peer discovery and room management
#[derive(Debug, Clone)]
pub struct SignalingServer {
    /// Address the server binds to
    pub bind_addr: SocketAddr,
    rooms: Arc<RwLock<HashMap<String, Room>>>,
    connections: Connections,
    participant_to_connection: Arc<DashMap<String, String>>,
}

impl SignalingServer {
    /// Create new signaling server
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            rooms: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(DashMap::new()),
            participant_to_connection: Arc::new(DashMap::new()),
        }
    }

    /// Start the signaling server
    pub async fn start(&self) -> Result<(), QuicRtcError> {
        let listener = TcpListener::bind(self.bind_addr).await.map_err(|e| {
            QuicRtcError::ServerStartFailed {
                address: self.bind_addr,
                source: e.into(),
            }
        })?;

        tracing::info!("Signaling server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::debug!("New connection from {}", addr);
                    self.handle_connection(stream).await;
                }
                Err(e) => {
                    tracing::error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle incoming WebSocket connection
    async fn handle_connection(&self, stream: TcpStream) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                tracing::error!("WebSocket handshake failed: {}", e);
                return;
            }
        };

        let connection_id = Uuid::new_v4().to_string();
        tracing::debug!("WebSocket connection established: {}", connection_id);

        // Store connection
        self.connections.insert(connection_id.clone(), ws_stream);

        // Handle messages for this connection
        if let Err(e) = self.handle_messages(connection_id.clone()).await {
            tracing::error!("Connection {} error: {}", connection_id, e);
        }

        // Cleanup on disconnect
        self.cleanup_connection(&connection_id).await;
    }

    /// Handle messages from a WebSocket connection
    async fn handle_messages(&self, connection_id: String) -> Result<(), QuicRtcError> {
        while let Some(mut connection) = self.connections.get_mut(&connection_id) {
            match connection.next().await {
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<SignalingMessage>(&text) {
                        Ok(message) => {
                            if let Err(e) = self
                                .handle_signaling_message(connection_id.clone(), message)
                                .await
                            {
                                tracing::error!("Failed to handle message: {}", e);
                                self.send_error(&connection_id, e).await;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Invalid message format: {}", e);
                            self.send_error(
                                &connection_id,
                                QuicRtcError::InvalidMessage {
                                    message: text,
                                    source: e.into(),
                                },
                            )
                            .await;
                        }
                    }
                }
                Some(Ok(Message::Close(_))) => {
                    tracing::debug!("Connection {} closed", connection_id);
                    break;
                }
                Some(Err(e)) => {
                    tracing::error!("WebSocket error on connection {}: {}", connection_id, e);
                    break;
                }
                None => {
                    tracing::debug!("Connection {} stream ended", connection_id);
                    break;
                }
                _ => {
                    // Ignore other message types (Binary, Ping, Pong)
                }
            }
        }
        Ok(())
    }

    /// Handle a signaling message
    async fn handle_signaling_message(
        &self,
        connection_id: String,
        message: SignalingMessage,
    ) -> Result<(), QuicRtcError> {
        match message {
            SignalingMessage::JoinRoom {
                room_id,
                participant_id,
                participant_name,
                capabilities,
                quic_endpoint,
            } => {
                self.handle_join_room(
                    connection_id,
                    room_id,
                    participant_id,
                    participant_name,
                    capabilities,
                    quic_endpoint,
                )
                .await
            }
            SignalingMessage::LeaveRoom {
                room_id,
                participant_id,
            } => {
                self.handle_leave_room(connection_id, room_id, participant_id)
                    .await
            }
            SignalingMessage::CreateRoom {
                room_id,
                room_name,
                max_participants,
            } => {
                self.handle_create_room(connection_id, room_id, room_name, max_participants)
                    .await
            }
            SignalingMessage::MoqSessionOffer {
                room_id,
                target_participant,
                offer,
            } => {
                self.handle_moq_session_offer(connection_id, room_id, target_participant, offer)
                    .await
            }
            SignalingMessage::MoqSessionAnswer {
                room_id,
                target_participant,
                answer,
            } => {
                self.handle_moq_session_answer(connection_id, room_id, target_participant, answer)
                    .await
            }
            SignalingMessage::ListRooms => self.handle_list_rooms(connection_id).await,
            SignalingMessage::GetRoomInfo { room_id } => {
                self.handle_get_room_info(connection_id, room_id).await
            }
        }
    }

    /// Handle room join request
    async fn handle_join_room(
        &self,
        connection_id: String,
        room_id: String,
        participant_id: String,
        participant_name: Option<String>,
        capabilities: Vec<String>,
        quic_endpoint: Option<SocketAddr>,
    ) -> Result<(), QuicRtcError> {
        let participant = Participant {
            id: participant_id.clone(),
            name: participant_name,
            connection_id: connection_id.clone(),
            capabilities,
            quic_endpoint,
        };

        // Add participant to room
        {
            let mut rooms = self.rooms.write().await;
            let room = rooms
                .get_mut(&room_id)
                .ok_or_else(|| QuicRtcError::RoomNotFound {
                    room_id: room_id.clone(),
                })?;

            room.add_participant(participant.clone())?;
        }

        // Track participant connection
        self.participant_to_connection
            .insert(participant_id.clone(), connection_id.clone());

        // Send join success response
        self.send_response(
            &connection_id,
            SignalingResponse::JoinedRoom {
                room_id: room_id.clone(),
                participant_id: participant_id.clone(),
            },
        )
        .await;

        // Notify other participants
        self.broadcast_to_room(
            &room_id,
            &participant_id,
            SignalingResponse::ParticipantJoined {
                room_id: room_id.clone(),
                participant: participant,
            },
        )
        .await;

        tracing::info!("Participant {} joined room {}", participant_id, room_id);
        Ok(())
    }

    /// Handle room leave request
    async fn handle_leave_room(
        &self,
        connection_id: String,
        room_id: String,
        participant_id: String,
    ) -> Result<(), QuicRtcError> {
        // Remove participant from room
        let removed_participant = {
            let mut rooms = self.rooms.write().await;
            if let Some(room) = rooms.get_mut(&room_id) {
                room.remove_participant(&participant_id)
            } else {
                None
            }
        };

        if removed_participant.is_some() {
            // Remove connection tracking
            self.participant_to_connection.remove(&participant_id);

            // Send leave success response
            self.send_response(
                &connection_id,
                SignalingResponse::LeftRoom {
                    room_id: room_id.clone(),
                    participant_id: participant_id.clone(),
                },
            )
            .await;

            // Notify other participants
            self.broadcast_to_room(
                &room_id,
                &participant_id,
                SignalingResponse::ParticipantLeft {
                    room_id: room_id.clone(),
                    participant_id: participant_id.clone(),
                },
            )
            .await;

            tracing::info!("Participant {} left room {}", participant_id, room_id);
        }

        Ok(())
    }

    /// Handle room creation request
    async fn handle_create_room(
        &self,
        connection_id: String,
        room_id: String,
        room_name: Option<String>,
        max_participants: Option<usize>,
    ) -> Result<(), QuicRtcError> {
        let mut room = Room::new(room_id.clone(), room_name);
        if let Some(max) = max_participants {
            room.max_participants = max;
        }

        // Create room
        {
            let mut rooms = self.rooms.write().await;
            if rooms.contains_key(&room_id) {
                return Err(QuicRtcError::RoomAlreadyExists { room_id });
            }
            rooms.insert(room_id.clone(), room);
        }

        // Send creation success response
        self.send_response(
            &connection_id,
            SignalingResponse::RoomCreated {
                room_id: room_id.clone(),
            },
        )
        .await;

        tracing::info!("Room {} created", room_id);
        Ok(())
    }

    /// Handle MoQ session offer
    async fn handle_moq_session_offer(
        &self,
        _connection_id: String,
        room_id: String,
        target_participant: String,
        offer: MoqSessionOffer,
    ) -> Result<(), QuicRtcError> {
        // Forward offer to target participant
        if let Some(target_connection) = self.participant_to_connection.get(&target_participant) {
            self.send_response(
                &target_connection,
                SignalingResponse::MoqSessionOffer {
                    room_id,
                    source_participant: offer.participant_id.clone(),
                    offer,
                },
            )
            .await;
        } else {
            return Err(QuicRtcError::ParticipantNotFound {
                room_id,
                participant_id: target_participant,
            });
        }
        Ok(())
    }

    /// Handle MoQ session answer
    async fn handle_moq_session_answer(
        &self,
        _connection_id: String,
        room_id: String,
        target_participant: String,
        answer: MoqSessionAnswer,
    ) -> Result<(), QuicRtcError> {
        // Forward answer to target participant
        if let Some(target_connection) = self.participant_to_connection.get(&target_participant) {
            self.send_response(
                &target_connection,
                SignalingResponse::MoqSessionAnswer {
                    room_id,
                    source_participant: answer.participant_id.clone(),
                    answer,
                },
            )
            .await;
        } else {
            return Err(QuicRtcError::ParticipantNotFound {
                room_id,
                participant_id: target_participant,
            });
        }
        Ok(())
    }

    /// Handle list rooms request
    async fn handle_list_rooms(&self, connection_id: String) -> Result<(), QuicRtcError> {
        let rooms = self.rooms.read().await;
        let room_list: Vec<_> = rooms
            .values()
            .map(|room| (room.id.clone(), room.name.clone(), room.participants.len()))
            .collect();

        self.send_response(
            &connection_id,
            SignalingResponse::RoomList { rooms: room_list },
        )
        .await;
        Ok(())
    }

    /// Handle get room info request
    async fn handle_get_room_info(
        &self,
        connection_id: String,
        room_id: String,
    ) -> Result<(), QuicRtcError> {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(&room_id) {
            let participants: Vec<_> = room.participants.values().cloned().collect();
            self.send_response(
                &connection_id,
                SignalingResponse::RoomInfo {
                    room_id: room.id.clone(),
                    room_name: room.name.clone(),
                    participants,
                    created_at: room.created_at,
                    max_participants: room.max_participants,
                },
            )
            .await;
        } else {
            return Err(QuicRtcError::RoomNotFound { room_id });
        }
        Ok(())
    }

    /// Send response to a specific connection
    async fn send_response(&self, connection_id: &str, response: SignalingResponse) {
        if let Some(mut connection) = self.connections.get_mut(connection_id) {
            let message = match serde_json::to_string(&response) {
                Ok(json) => Message::Text(json),
                Err(e) => {
                    tracing::error!("Failed to serialize response: {}", e);
                    return;
                }
            };

            if let Err(e) = connection.send(message).await {
                tracing::error!("Failed to send message to {}: {}", connection_id, e);
            }
        }
    }

    /// Send error to a specific connection
    async fn send_error(&self, connection_id: &str, error: QuicRtcError) {
        self.send_response(
            connection_id,
            SignalingResponse::Error {
                error: error.to_string(),
                error_code: error.error_code(),
            },
        )
        .await;
    }

    /// Broadcast message to all participants in a room except sender
    async fn broadcast_to_room(
        &self,
        room_id: &str,
        exclude_participant: &str,
        response: SignalingResponse,
    ) {
        let participants = {
            let rooms = self.rooms.read().await;
            if let Some(room) = rooms.get(room_id) {
                room.other_participants(exclude_participant)
                    .into_iter()
                    .map(|p| p.connection_id.clone())
                    .collect::<Vec<_>>()
            } else {
                return;
            }
        };

        for connection_id in participants {
            self.send_response(&connection_id, response.clone()).await;
        }
    }

    /// Cleanup connection and associated participant
    async fn cleanup_connection(&self, connection_id: &str) {
        // Remove connection
        self.connections.remove(connection_id);

        // Find and remove participant from rooms
        let participant_id = self
            .participant_to_connection
            .iter()
            .find(|entry| entry.value() == connection_id)
            .map(|entry| entry.key().clone());

        if let Some(participant_id) = participant_id {
            // Remove from all rooms
            let room_ids: Vec<String> = {
                let rooms = self.rooms.read().await;
                rooms
                    .iter()
                    .filter(|(_, room)| room.participants.contains_key(&participant_id))
                    .map(|(room_id, _)| room_id.clone())
                    .collect()
            };

            for room_id in room_ids {
                let _ = self
                    .handle_leave_room(connection_id.to_string(), room_id, participant_id.clone())
                    .await;
            }

            self.participant_to_connection.remove(&participant_id);
        }
    }

    /// Stop the signaling server
    pub async fn stop(&self) -> Result<(), QuicRtcError> {
        // Close all connections
        self.connections.clear();
        self.participant_to_connection.clear();

        // Clear all rooms
        self.rooms.write().await.clear();

        tracing::info!("Signaling server stopped");
        Ok(())
    }

    /// Get current rooms (for monitoring/debugging)
    pub async fn get_rooms(&self) -> Vec<Room> {
        self.rooms.read().await.values().cloned().collect()
    }

    /// Get participant count across all rooms
    pub async fn total_participants(&self) -> usize {
        self.rooms
            .read()
            .await
            .values()
            .map(|room| room.participants.len())
            .sum()
    }
}
