//! Peer discovery service

use chrono::{DateTime, Utc};
use quicrtc_core::QuicRtcError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Peer information for discovery
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Unique peer ID
    pub id: String,
    /// Display name
    pub name: Option<String>,
    /// Room ID where peer is located
    pub room_id: String,
    /// QUIC endpoint for direct connection
    pub quic_endpoint: Option<std::net::SocketAddr>,
    /// MoQ capabilities
    pub capabilities: Vec<String>,
    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
    /// Peer status
    pub status: PeerStatus,
}

/// Status of a discovered peer
#[derive(Debug, Clone, PartialEq)]
pub enum PeerStatus {
    /// Peer is online and available
    Online,
    /// Peer is temporarily away
    Away,
    /// Peer is offline
    Offline,
}

/// Events emitted by the peer discovery service
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// A new peer was discovered
    PeerDiscovered {
        /// Room ID where peer was discovered
        room_id: String,
        /// Information about the discovered peer  
        peer: PeerInfo,
    },
    /// A peer left or went offline
    PeerLeft {
        /// Room ID where peer left
        room_id: String,
        /// ID of the peer that left
        peer_id: String,
    },
    /// A peer's status changed
    PeerStatusChanged {
        /// Room ID where status changed
        room_id: String,
        /// ID of the peer whose status changed
        peer_id: String,
        /// Previous status
        old_status: PeerStatus,
        /// New status
        new_status: PeerStatus,
    },
    /// Room state has been synchronized
    RoomSynchronized {
        /// Room ID that was synchronized
        room_id: String,
        /// Number of peers after synchronization
        peer_count: usize,
    },
}

/// Peer discovery for finding and connecting participants
#[derive(Debug)]
pub struct PeerDiscovery {
    /// Discovered peers organized by room
    peers: Arc<RwLock<HashMap<String, HashMap<String, PeerInfo>>>>,
    /// Event broadcaster for discovery events
    event_sender: broadcast::Sender<DiscoveryEvent>,
    /// Discovery service configuration
    config: DiscoveryConfig,
}

/// Configuration for the peer discovery service
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// How often to clean up offline peers (seconds)
    pub cleanup_interval: u64,
    /// How long before marking a peer as offline (seconds)
    pub offline_timeout: u64,
    /// Maximum number of peers to track per room
    pub max_peers_per_room: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            cleanup_interval: 60, // 1 minute
            offline_timeout: 300, // 5 minutes
            max_peers_per_room: 100,
        }
    }
}

impl PeerDiscovery {
    /// Create new peer discovery service
    pub fn new() -> Self {
        Self::new_with_config(DiscoveryConfig::default())
    }

    /// Create new peer discovery service with custom configuration
    pub fn new_with_config(config: DiscoveryConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);

        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            config,
        }
    }

    /// Start the peer discovery service background tasks
    pub async fn start(&self) -> Result<(), QuicRtcError> {
        // Start cleanup task
        let peers = Arc::clone(&self.peers);
        let event_sender = self.event_sender.clone();
        let cleanup_interval = self.config.cleanup_interval;
        let offline_timeout = self.config.offline_timeout;

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(cleanup_interval));

            loop {
                interval.tick().await;
                Self::cleanup_offline_peers(&peers, &event_sender, offline_timeout).await;
            }
        });

        Ok(())
    }

    /// Discover peers in a room
    pub async fn discover_peers(&self, room_id: &str) -> Result<Vec<PeerInfo>, QuicRtcError> {
        let peers = self.peers.read().await;

        if let Some(room_peers) = peers.get(room_id) {
            Ok(room_peers.values().cloned().collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Add a new peer to discovery
    pub async fn add_peer(&self, peer: PeerInfo) -> Result<(), QuicRtcError> {
        let room_id = peer.room_id.clone();
        let peer_id = peer.id.clone();

        {
            let mut peers = self.peers.write().await;
            let room_peers = peers.entry(room_id.clone()).or_insert_with(HashMap::new);

            // Check room capacity
            if room_peers.len() >= self.config.max_peers_per_room
                && !room_peers.contains_key(&peer_id)
            {
                return Err(QuicRtcError::RoomFull {
                    room_id: room_id.clone(),
                    max_participants: self.config.max_peers_per_room,
                });
            }

            let is_new = !room_peers.contains_key(&peer_id);
            room_peers.insert(peer_id.clone(), peer.clone());

            if is_new {
                // Emit discovery event
                let _ = self.event_sender.send(DiscoveryEvent::PeerDiscovered {
                    room_id: room_id.clone(),
                    peer: peer.clone(),
                });
            }
        }

        tracing::info!("Added peer {} to room {}", peer_id, room_id);
        Ok(())
    }

    /// Remove a peer from discovery
    pub async fn remove_peer(&self, room_id: &str, peer_id: &str) -> Result<(), QuicRtcError> {
        {
            let mut peers = self.peers.write().await;
            if let Some(room_peers) = peers.get_mut(room_id) {
                if room_peers.remove(peer_id).is_some() {
                    // Clean up empty rooms
                    if room_peers.is_empty() {
                        peers.remove(room_id);
                    }

                    // Emit departure event
                    let _ = self.event_sender.send(DiscoveryEvent::PeerLeft {
                        room_id: room_id.to_string(),
                        peer_id: peer_id.to_string(),
                    });
                }
            }
        }

        tracing::info!("Removed peer {} from room {}", peer_id, room_id);
        Ok(())
    }

    /// Update peer status
    pub async fn update_peer_status(
        &self,
        room_id: &str,
        peer_id: &str,
        new_status: PeerStatus,
    ) -> Result<(), QuicRtcError> {
        let old_status = {
            let mut peers = self.peers.write().await;
            if let Some(room_peers) = peers.get_mut(room_id) {
                if let Some(peer) = room_peers.get_mut(peer_id) {
                    let old = peer.status.clone();
                    peer.status = new_status.clone();
                    peer.last_seen = Utc::now();
                    Some(old)
                } else {
                    return Err(QuicRtcError::ParticipantNotFound {
                        room_id: room_id.to_string(),
                        participant_id: peer_id.to_string(),
                    });
                }
            } else {
                return Err(QuicRtcError::RoomNotFound {
                    room_id: room_id.to_string(),
                });
            }
        };

        if let Some(old_status) = old_status {
            if old_status != new_status {
                // Emit status change event
                let _ = self.event_sender.send(DiscoveryEvent::PeerStatusChanged {
                    room_id: room_id.to_string(),
                    peer_id: peer_id.to_string(),
                    old_status,
                    new_status,
                });
            }
        }

        Ok(())
    }

    /// Synchronize room state - useful for handling network partitions
    pub async fn synchronize_room(
        &self,
        room_id: &str,
        current_peers: Vec<PeerInfo>,
    ) -> Result<(), QuicRtcError> {
        {
            let mut peers = self.peers.write().await;
            let room_peers = peers
                .entry(room_id.to_string())
                .or_insert_with(HashMap::new);

            // Update existing peers and add new ones
            for peer in current_peers.iter() {
                room_peers.insert(peer.id.clone(), peer.clone());
            }

            // Remove peers that are no longer present
            let current_peer_ids: std::collections::HashSet<String> =
                current_peers.iter().map(|p| p.id.clone()).collect();

            room_peers.retain(|id, _| current_peer_ids.contains(id));
        }

        // Emit synchronization event
        let _ = self.event_sender.send(DiscoveryEvent::RoomSynchronized {
            room_id: room_id.to_string(),
            peer_count: current_peers.len(),
        });

        tracing::info!(
            "Synchronized room {} with {} peers",
            room_id,
            current_peers.len()
        );
        Ok(())
    }

    /// Subscribe to discovery events
    pub fn subscribe_events(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.event_sender.subscribe()
    }

    /// Get peers in a specific room with a given status
    pub async fn get_peers_by_status(
        &self,
        room_id: &str,
        status: PeerStatus,
    ) -> Result<Vec<PeerInfo>, QuicRtcError> {
        let peers = self.peers.read().await;

        if let Some(room_peers) = peers.get(room_id) {
            Ok(room_peers
                .values()
                .filter(|peer| peer.status == status)
                .cloned()
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Find peers with specific capabilities
    pub async fn find_peers_with_capabilities(
        &self,
        room_id: &str,
        required_capabilities: &[String],
    ) -> Result<Vec<PeerInfo>, QuicRtcError> {
        let peers = self.peers.read().await;

        if let Some(room_peers) = peers.get(room_id) {
            Ok(room_peers
                .values()
                .filter(|peer| {
                    required_capabilities
                        .iter()
                        .all(|cap| peer.capabilities.contains(cap))
                })
                .cloned()
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get room statistics
    pub async fn get_room_stats(&self, room_id: &str) -> Result<RoomStats, QuicRtcError> {
        let peers = self.peers.read().await;

        if let Some(room_peers) = peers.get(room_id) {
            let total = room_peers.len();
            let online = room_peers
                .values()
                .filter(|p| p.status == PeerStatus::Online)
                .count();
            let away = room_peers
                .values()
                .filter(|p| p.status == PeerStatus::Away)
                .count();
            let offline = room_peers
                .values()
                .filter(|p| p.status == PeerStatus::Offline)
                .count();

            Ok(RoomStats {
                room_id: room_id.to_string(),
                total_peers: total,
                online_peers: online,
                away_peers: away,
                offline_peers: offline,
            })
        } else {
            Err(QuicRtcError::RoomNotFound {
                room_id: room_id.to_string(),
            })
        }
    }

    /// Clean up offline peers (internal method)
    async fn cleanup_offline_peers(
        peers: &Arc<RwLock<HashMap<String, HashMap<String, PeerInfo>>>>,
        event_sender: &broadcast::Sender<DiscoveryEvent>,
        offline_timeout: u64,
    ) {
        let cutoff_time = Utc::now() - chrono::Duration::seconds(offline_timeout as i64);
        let mut rooms_to_remove = Vec::new();
        let mut events = Vec::new();

        {
            let mut peers_guard = peers.write().await;

            for (room_id, room_peers) in peers_guard.iter_mut() {
                let mut peers_to_remove = Vec::new();

                for (peer_id, peer) in room_peers.iter_mut() {
                    if peer.last_seen < cutoff_time && peer.status != PeerStatus::Offline {
                        peer.status = PeerStatus::Offline;
                        events.push(DiscoveryEvent::PeerStatusChanged {
                            room_id: room_id.clone(),
                            peer_id: peer_id.clone(),
                            old_status: PeerStatus::Online, // Simplification for cleanup
                            new_status: PeerStatus::Offline,
                        });
                    }

                    // Remove peers that have been offline for too long
                    if peer.last_seen
                        < cutoff_time - chrono::Duration::seconds(offline_timeout as i64)
                    {
                        peers_to_remove.push(peer_id.clone());
                    }
                }

                // Remove offline peers
                for peer_id in peers_to_remove {
                    room_peers.remove(&peer_id);
                    events.push(DiscoveryEvent::PeerLeft {
                        room_id: room_id.clone(),
                        peer_id,
                    });
                }

                // Mark empty rooms for removal
                if room_peers.is_empty() {
                    rooms_to_remove.push(room_id.clone());
                }
            }

            // Remove empty rooms
            for room_id in rooms_to_remove {
                peers_guard.remove(&room_id);
            }
        }

        // Emit events
        for event in events {
            let _ = event_sender.send(event);
        }
    }

    /// Get all room IDs with active peers
    pub async fn get_active_rooms(&self) -> Vec<String> {
        let peers = self.peers.read().await;
        peers.keys().cloned().collect()
    }
}

/// Room statistics
#[derive(Debug, Clone)]
pub struct RoomStats {
    /// Room ID
    pub room_id: String,
    /// Total number of peers
    pub total_peers: usize,
    /// Number of online peers
    pub online_peers: usize,
    /// Number of away peers  
    pub away_peers: usize,
    /// Number of offline peers
    pub offline_peers: usize,
}

impl Default for PeerDiscovery {
    fn default() -> Self {
        Self::new()
    }
}
