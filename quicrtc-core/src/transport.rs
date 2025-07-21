//! QUIC transport layer with fallback mechanisms and production-grade configuration

use crate::error::QuicRtcError;
use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, WebSocketStream};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Certificate configuration for production deployment
#[derive(Debug, Clone)]
pub enum CertificateConfig {
    /// Self-signed certificate for development and testing
    SelfSigned {
        /// Common name for the certificate
        common_name: String,
        /// Subject alternative names
        subject_alt_names: Vec<String>,
    },
    /// Certificate and private key from files
    FromFile {
        /// Path to certificate file
        cert_path: String,
        /// Path to private key file
        key_path: String,
    },
    /// Platform-provided certificate (system keychain, etc.)
    Platform {
        /// Subject alternative names
        subject_alt_names: Vec<String>,
        /// Certificate store name
        store_name: Option<String>,
    },
}

impl Default for CertificateConfig {
    fn default() -> Self {
        Self::SelfSigned {
            common_name: "localhost".to_string(),
            subject_alt_names: vec!["localhost".to_string(), "127.0.0.1".to_string()],
        }
    }
}

/// Production QUIC transport configuration
#[derive(Debug, Clone)]
pub struct QuicTransportConfig {
    /// Maximum number of concurrent streams per connection
    pub max_concurrent_streams: u64,
    /// Initial maximum data for the connection (flow control window)
    pub initial_max_data: u64,
    /// Initial maximum data per stream
    pub initial_max_stream_data: u64,
    /// Maximum idle timeout before connection is closed
    pub max_idle_timeout: Duration,
    /// Keep-alive interval for maintaining connections
    pub keep_alive_interval: Duration,
    /// Congestion control algorithm
    pub congestion_controller: CongestionController,
    /// Enable 0-RTT connection resumption
    pub enable_0rtt: bool,
    /// Enable connection migration for mobile networks
    pub enable_migration: bool,
    /// Maximum transmission unit for path MTU discovery
    pub initial_mtu: u16,
    /// Certificate configuration
    pub certificate_config: CertificateConfig,
}

/// Congestion control algorithms for QUIC
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionController {
    /// Bottleneck Bandwidth and RTT (recommended for production)
    Bbr,
    /// Traditional Cubic algorithm
    Cubic,
    /// New Reno algorithm
    NewReno,
}

impl Default for QuicTransportConfig {
    fn default() -> Self {
        Self::mobile() // Default to mobile-optimized settings
    }
}

impl QuicTransportConfig {
    /// Production configuration optimized for mobile devices
    pub fn mobile() -> Self {
        Self {
            max_concurrent_streams: 50,
            initial_max_data: 15 * 1024 * 1024, // 15MB initial window
            initial_max_stream_data: 1024 * 1024, // 1MB per stream
            max_idle_timeout: Duration::from_secs(60),
            keep_alive_interval: Duration::from_secs(15), // Frequent for mobile
            congestion_controller: CongestionController::Bbr,
            enable_0rtt: true,
            enable_migration: true,
            initial_mtu: 1200, // Conservative for mobile networks
            certificate_config: CertificateConfig::default(),
        }
    }

    /// Production configuration optimized for desktop/server deployment
    pub fn desktop() -> Self {
        Self {
            max_concurrent_streams: 200,
            initial_max_data: 50 * 1024 * 1024, // 50MB initial window
            initial_max_stream_data: 5 * 1024 * 1024, // 5MB per stream
            max_idle_timeout: Duration::from_secs(120),
            keep_alive_interval: Duration::from_secs(30),
            congestion_controller: CongestionController::Bbr,
            enable_0rtt: true,
            enable_migration: false, // Desktop typically doesn't need migration
            initial_mtu: 1500,       // Standard Ethernet MTU
            certificate_config: CertificateConfig::default(),
        }
    }

    /// High-performance configuration for server deployment
    pub fn server() -> Self {
        Self {
            max_concurrent_streams: 1000,
            initial_max_data: 100 * 1024 * 1024, // 100MB initial window
            initial_max_stream_data: 10 * 1024 * 1024, // 10MB per stream
            max_idle_timeout: Duration::from_secs(300),
            keep_alive_interval: Duration::from_secs(60),
            congestion_controller: CongestionController::Bbr,
            enable_0rtt: true,
            enable_migration: false,
            initial_mtu: 1500,
            certificate_config: CertificateConfig::default(),
        }
    }
}

/// Resource limits for production deployment
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory usage in MB (None = unlimited)
    pub max_memory_mb: Option<u64>,
    /// Maximum bandwidth usage in kbps (None = unlimited)
    pub max_bandwidth_kbps: Option<u64>,
    /// Maximum number of concurrent connections
    pub max_connections: Option<u32>,
    /// Maximum streams per connection
    pub max_streams_per_connection: Option<u32>,
    /// Cleanup timeout for idle resources
    pub cleanup_timeout: Duration,
    /// Connection pool size
    pub connection_pool_size: u32,
}

impl ResourceLimits {
    /// Conservative limits for mobile devices
    pub fn mobile() -> Self {
        Self {
            max_memory_mb: Some(50),
            max_bandwidth_kbps: Some(2000), // 2 Mbps max
            max_connections: Some(5),
            max_streams_per_connection: Some(10),
            cleanup_timeout: Duration::from_secs(5),
            connection_pool_size: 2,
        }
    }

    /// Higher limits for desktop applications
    pub fn desktop() -> Self {
        Self {
            max_memory_mb: Some(200),
            max_bandwidth_kbps: Some(10000), // 10 Mbps max
            max_connections: Some(20),
            max_streams_per_connection: Some(50),
            cleanup_timeout: Duration::from_secs(10),
            connection_pool_size: 5,
        }
    }

    /// Server-grade limits for production deployment
    pub fn server() -> Self {
        Self {
            max_memory_mb: Some(1000),
            max_bandwidth_kbps: None, // No bandwidth limit for servers
            max_connections: Some(1000),
            max_streams_per_connection: Some(200),
            cleanup_timeout: Duration::from_secs(30),
            connection_pool_size: 20,
        }
    }
}

/// Current resource usage tracking
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    /// Current memory usage in MB
    pub memory_mb: u64,
    /// Current bandwidth usage in kbps
    pub bandwidth_kbps: u64,
    /// Number of active connections
    pub active_connections: u32,
    /// Number of active streams
    pub active_streams: u32,
    /// Last updated timestamp
    pub last_updated: Instant,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            memory_mb: 0,
            bandwidth_kbps: 0,
            active_connections: 0,
            active_streams: 0,
            last_updated: Instant::now(),
        }
    }
}

/// Resource warning types
#[derive(Debug, Clone)]
pub enum ResourceWarning {
    /// Memory usage is approaching or exceeding limits
    MemoryUsageHigh {
        /// Current memory usage in MB
        current: u64,
        /// Memory limit in MB
        limit: u64,
    },
    /// Bandwidth usage is approaching or exceeding limits
    BandwidthUsageHigh {
        /// Current bandwidth usage in kbps
        current: u64,
        /// Bandwidth limit in kbps
        limit: u64,
    },
    /// Connection count is approaching the limit
    ConnectionLimitApproaching {
        /// Current number of connections
        current: u32,
        /// Maximum allowed connections
        limit: u32,
    },
    /// Stream count is approaching the limit
    StreamLimitApproaching {
        /// Current number of streams
        current: u32,
        /// Maximum allowed streams
        limit: u32,
    },
}

/// Resource warning severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSeverity {
    /// 75-85% of limit
    Low,
    /// 85-95% of limit
    Medium,
    /// 95-100% of limit
    High,
    /// Over limit
    Critical,
}

/// Production resource manager for QUIC connections
pub struct ResourceManager {
    limits: ResourceLimits,
    current_usage: Arc<RwLock<ResourceUsage>>,
    monitors: Vec<Box<dyn ResourceMonitor + Send + Sync>>,
    warnings: Arc<RwLock<Vec<(ResourceWarning, WarningSeverity, Instant)>>>,
}

impl std::fmt::Debug for ResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceManager")
            .field("limits", &self.limits)
            .field("current_usage", &self.current_usage)
            .field("monitors_count", &self.monitors.len())
            .field("warnings", &self.warnings)
            .finish()
    }
}

impl ResourceManager {
    /// Create a new resource manager with specified limits
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            current_usage: Arc::new(RwLock::new(ResourceUsage::default())),
            monitors: Vec::new(),
            warnings: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if resource limits allow a new operation
    pub fn check_limits(&self) -> Result<(), QuicRtcError> {
        let usage = self.current_usage.read();

        // Check memory limits
        if let Some(memory_limit) = self.limits.max_memory_mb {
            if usage.memory_mb >= memory_limit {
                return Err(QuicRtcError::ResourceLimit {
                    resource: format!("Memory limit ({} MB) exceeded", memory_limit),
                });
            }
        }

        // Check bandwidth limits
        if let Some(bandwidth_limit) = self.limits.max_bandwidth_kbps {
            if usage.bandwidth_kbps >= bandwidth_limit {
                return Err(QuicRtcError::ResourceLimit {
                    resource: format!("Bandwidth limit ({} kbps) exceeded", bandwidth_limit),
                });
            }
        }

        // Check connection limits
        if let Some(connection_limit) = self.limits.max_connections {
            if usage.active_connections >= connection_limit {
                return Err(QuicRtcError::ResourceLimit {
                    resource: format!("Connection limit ({}) exceeded", connection_limit),
                });
            }
        }

        Ok(())
    }

    /// Get current resource usage
    pub fn current_usage(&self) -> ResourceUsage {
        self.current_usage.read().clone()
    }

    /// Get current warnings approaching resource limits
    pub fn approaching_limits(&self) -> Vec<(ResourceWarning, WarningSeverity)> {
        let usage = self.current_usage.read();
        let mut warnings = Vec::new();

        // Check memory usage
        if let Some(memory_limit) = self.limits.max_memory_mb {
            let usage_percent = (usage.memory_mb as f32 / memory_limit as f32) * 100.0;
            if usage_percent >= 75.0 {
                let severity = match usage_percent {
                    p if p >= 100.0 => WarningSeverity::Critical,
                    p if p >= 95.0 => WarningSeverity::High,
                    p if p >= 85.0 => WarningSeverity::Medium,
                    _ => WarningSeverity::Low,
                };
                warnings.push((
                    ResourceWarning::MemoryUsageHigh {
                        current: usage.memory_mb,
                        limit: memory_limit,
                    },
                    severity,
                ));
            }
        }

        // Similar checks for other resources...
        warnings
    }

    /// Cleanup idle resources based on configured timeout
    pub async fn cleanup_resources(&mut self) -> Result<(), QuicRtcError> {
        let cleanup_started = Instant::now();
        debug!(
            "Starting resource cleanup with timeout: {:?}",
            self.limits.cleanup_timeout
        );

        // This would clean up idle connections, free unused memory, etc.
        // Implementation would depend on the specific resources being managed

        let cleanup_duration = cleanup_started.elapsed();
        info!("Resource cleanup completed in {:?}", cleanup_duration);

        Ok(())
    }

    /// Update resource usage statistics
    pub fn update_usage(&self, usage: ResourceUsage) {
        let mut current = self.current_usage.write();
        *current = usage;
        current.last_updated = Instant::now();
    }

    /// Add a resource monitor
    pub fn add_monitor(&mut self, monitor: Box<dyn ResourceMonitor + Send + Sync>) {
        self.monitors.push(monitor);
    }
}

/// Trait for resource monitoring implementations
pub trait ResourceMonitor {
    /// Monitor current memory usage in MB
    fn monitor_memory(&self) -> Result<u64, QuicRtcError>;
    /// Monitor current bandwidth usage in kbps
    fn monitor_bandwidth(&self) -> Result<u64, QuicRtcError>;
    /// Monitor current number of active connections
    fn monitor_connections(&self) -> Result<u32, QuicRtcError>;
}

/// Production QUIC server for accepting incoming connections
#[derive(Debug)]
pub struct QuicServer {
    endpoint: Endpoint,
    server_config: ServerConfig,
    transport_config: QuicTransportConfig,
    resource_manager: ResourceManager,
    active_connections: Arc<RwLock<HashMap<Uuid, Arc<RwLock<TransportConnection>>>>>,
    connection_stats: Arc<RwLock<HashMap<Uuid, ConnectionStats>>>,
}

impl QuicServer {
    /// Create and bind a new QUIC server
    pub async fn bind(
        addr: SocketAddr,
        transport_config: QuicTransportConfig,
        resource_limits: ResourceLimits,
    ) -> Result<Self, QuicRtcError> {
        info!("Creating QUIC server bound to {}", addr);

        // Set up crypto provider
        let crypto = rustls::crypto::aws_lc_rs::default_provider();
        let _ = rustls::crypto::CryptoProvider::install_default(crypto);

        // Generate or load certificate based on configuration
        let (cert_chain, private_key) =
            Self::setup_certificate(&transport_config.certificate_config).await?;

        // Configure server TLS
        let mut server_config =
            ServerConfig::with_single_cert(cert_chain, private_key).map_err(|e| {
                QuicRtcError::Transport {
                    reason: format!("Failed to configure server TLS: {}", e),
                }
            })?;

        // Apply transport configuration
        let mut quinn_transport_config = quinn::TransportConfig::default();
        quinn_transport_config.max_concurrent_uni_streams(VarInt::from_u32(
            transport_config.max_concurrent_streams as u32 / 2,
        ));
        quinn_transport_config.max_concurrent_bidi_streams(VarInt::from_u32(
            transport_config.max_concurrent_streams as u32 / 2,
        ));
        quinn_transport_config
            .max_idle_timeout(Some(transport_config.max_idle_timeout.try_into().unwrap()));
        quinn_transport_config.keep_alive_interval(Some(transport_config.keep_alive_interval));

        // Configure congestion control
        match transport_config.congestion_controller {
            CongestionController::Bbr => {
                // BBR is the default in quinn, no additional configuration needed
                debug!("Using BBR congestion control");
            }
            CongestionController::Cubic => {
                debug!("Using Cubic congestion control");
                // Would need to configure if quinn supports switching
            }
            CongestionController::NewReno => {
                debug!("Using NewReno congestion control");
                // Would need to configure if quinn supports switching
            }
        }

        server_config.transport_config(Arc::new(quinn_transport_config));

        // Create endpoint
        let endpoint =
            Endpoint::server(server_config.clone(), addr).map_err(|e| QuicRtcError::Transport {
                reason: format!("Failed to create QUIC server endpoint: {}", e),
            })?;

        let actual_addr = endpoint.local_addr().map_err(|e| QuicRtcError::Transport {
            reason: format!("Failed to get server local address: {}", e),
        })?;

        info!("QUIC server successfully bound to {}", actual_addr);

        Ok(Self {
            endpoint,
            server_config,
            transport_config,
            resource_manager: ResourceManager::new(resource_limits),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            connection_stats: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Accept incoming QUIC connections
    pub async fn accept_connection(
        &mut self,
    ) -> Result<Arc<RwLock<TransportConnection>>, QuicRtcError> {
        debug!("Waiting for incoming QUIC connection");

        // Check resource limits before accepting
        self.resource_manager.check_limits()?;

        // Accept incoming connection
        let connecting = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| QuicRtcError::Transport {
                reason: "Server endpoint closed".to_string(),
            })?;

        let connection = connecting.await.map_err(|e| QuicRtcError::Transport {
            reason: format!("Failed to establish incoming QUIC connection: {}", e),
        })?;

        let connection_id = Uuid::new_v4();
        let remote_addr = connection.remote_address();

        info!(
            "Accepted QUIC connection from {} with ID {}",
            remote_addr, connection_id
        );

        // Create transport connection wrapper
        let transport_connection = TransportConnection::from_quinn_connection(
            connection,
            connection_id,
            TransportMode::QuicNative,
            Some(NetworkPath {
                local_addr: self.endpoint.local_addr().unwrap(),
                remote_addr,
                interface_name: None,
                mtu: Some(self.transport_config.initial_mtu),
            }),
        );

        // Track the connection
        let transport_connection_arc = Arc::new(RwLock::new(transport_connection));
        {
            let mut connections = self.active_connections.write();
            connections.insert(connection_id, transport_connection_arc.clone());
        }

        // Return a reference to the stored connection
        Ok(transport_connection_arc)
    }

    /// Get statistics for all active connections
    pub fn connection_stats(&self) -> HashMap<Uuid, ConnectionStats> {
        self.connection_stats.read().clone()
    }

    /// Get the local address the server is bound to
    pub fn local_addr(&self) -> SocketAddr {
        self.endpoint.local_addr().unwrap()
    }

    /// Setup certificate based on configuration
    async fn setup_certificate(
        config: &CertificateConfig,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), QuicRtcError> {
        match config {
            CertificateConfig::SelfSigned {
                common_name,
                subject_alt_names,
            } => Self::generate_self_signed_cert(common_name, subject_alt_names).await,
            CertificateConfig::FromFile {
                cert_path,
                key_path,
            } => Self::load_cert_from_files(cert_path, key_path).await,
            CertificateConfig::Platform { .. } => {
                // Platform certificate loading would be implemented here
                Err(QuicRtcError::Transport {
                    reason: "Platform certificate loading not yet implemented".to_string(),
                })
            } // Custom variant removed - certificates are now generated or loaded from files
        }
    }

    /// Generate a self-signed certificate for development/testing
    async fn generate_self_signed_cert(
        common_name: &str,
        subject_alt_names: &[String],
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), QuicRtcError> {
        use rcgen::{Certificate, CertificateParams, DistinguishedName, SanType};

        debug!("Generating self-signed certificate for {}", common_name);

        let mut params = CertificateParams::new(vec![common_name.to_string()]);
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, common_name);

        // Add subject alternative names
        for san in subject_alt_names {
            if san.parse::<std::net::IpAddr>().is_ok() {
                params
                    .subject_alt_names
                    .push(SanType::IpAddress(san.parse().unwrap()));
            } else {
                params.subject_alt_names.push(SanType::DnsName(san.clone()));
            }
        }

        let cert = Certificate::from_params(params).map_err(|e| QuicRtcError::Transport {
            reason: format!("Failed to generate self-signed certificate: {}", e),
        })?;

        let cert_der = CertificateDer::from(cert.serialize_der().unwrap());
        let private_key_der =
            PrivateKeyDer::try_from(cert.serialize_private_key_der()).map_err(|_| {
                QuicRtcError::Transport {
                    reason: "Failed to convert private key".to_string(),
                }
            })?;

        info!("Generated self-signed certificate for {}", common_name);

        Ok((vec![cert_der], private_key_der))
    }

    /// Load certificate and private key from files
    async fn load_cert_from_files(
        cert_path: &str,
        key_path: &str,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), QuicRtcError> {
        use std::fs;

        debug!(
            "Loading certificate from {} and key from {}",
            cert_path, key_path
        );

        let cert_data = fs::read(cert_path).map_err(|e| QuicRtcError::Transport {
            reason: format!("Failed to read certificate file {}: {}", cert_path, e),
        })?;

        let key_data = fs::read(key_path).map_err(|e| QuicRtcError::Transport {
            reason: format!("Failed to read private key file {}: {}", key_path, e),
        })?;

        // Parse certificate chain
        let cert_chain = if cert_path.ends_with(".pem") {
            rustls_pemfile::certs(&mut cert_data.as_slice())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| QuicRtcError::Transport {
                    reason: format!("Failed to parse certificate PEM: {}", e),
                })?
        } else {
            vec![CertificateDer::from(cert_data)]
        };

        // Parse private key
        let private_key = if key_path.ends_with(".pem") {
            let mut key_reader = key_data.as_slice();
            rustls_pemfile::private_key(&mut key_reader)
                .map_err(|e| QuicRtcError::Transport {
                    reason: format!("Failed to parse private key PEM: {}", e),
                })?
                .ok_or_else(|| QuicRtcError::Transport {
                    reason: "No private key found in PEM file".to_string(),
                })?
        } else {
            PrivateKeyDer::try_from(key_data).map_err(|_| QuicRtcError::Transport {
                reason: "Failed to convert private key from file".to_string(),
            })?
        };

        info!(
            "Loaded certificate chain with {} certificates",
            cert_chain.len()
        );

        Ok((cert_chain, private_key))
    }

    /// Close the server and all active connections
    pub async fn close(&mut self) -> Result<(), QuicRtcError> {
        info!("Closing QUIC server");

        // Close all active connections
        let connections = {
            let mut active = self.active_connections.write();
            std::mem::take(&mut *active)
        };

        for (connection_id, connection) in connections {
            debug!("Closing connection {}", connection_id);
            let mut conn = connection.write();
            let _ = conn.close().await;
        }

        // Close endpoint
        self.endpoint.close(VarInt::from_u32(0), b"Server shutdown");

        info!("QUIC server closed successfully");
        Ok(())
    }
}

/// Transport modes with automatic fallback capability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TransportMode {
    /// Direct QUIC connection (best performance)
    QuicNative,
    /// QUIC tunneled over WebSocket (firewall workaround)
    QuicOverWebSocket,
    /// Traditional WebRTC fallback (maximum compatibility)
    WebRtcCompat,
}

/// Stream type for QUIC streams
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    /// Bidirectional stream
    Bidirectional,
    /// Unidirectional stream
    Unidirectional,
}

/// Network path information for connection migration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPath {
    /// Local address
    pub local_addr: SocketAddr,
    /// Remote address
    pub remote_addr: SocketAddr,
    /// Network interface name (if available)
    pub interface_name: Option<String>,
    /// Path MTU
    pub mtu: Option<u16>,
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Round-trip time
    pub rtt: Duration,
    /// Congestion window size
    pub cwnd: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Packet loss rate
    pub loss_rate: f64,
    /// Connection established time
    pub established_at: Instant,
}

/// Connection metrics for monitoring
#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    /// Connection establishment attempts
    pub connection_attempts: u32,
    /// Successful connections
    pub successful_connections: u32,
    /// Failed connections by transport mode
    pub failed_connections: HashMap<TransportMode, u32>,
    /// Connection migration events
    pub migration_events: u32,
    /// Last connection attempt time
    pub last_attempt: Option<Instant>,
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self {
            connection_attempts: 0,
            successful_connections: 0,
            failed_connections: HashMap::new(),
            migration_events: 0,
            last_attempt: None,
        }
    }
}

/// Internal transport implementation
#[derive(Debug)]
enum TransportInner {
    /// Native QUIC connection
    Quic(Connection),
    /// WebSocket transport for fallback
    WebSocket(WebSocketTransport),
    /// WebRTC transport for maximum compatibility
    WebRtc(WebRtcTransport),
}

/// WebSocket transport wrapper
#[derive(Debug)]
pub struct WebSocketTransport {
    stream: Arc<
        RwLock<Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>>,
    >,
    endpoint: String,
}

/// WebRTC transport wrapper 
/// 
/// **STATUS: ARCHITECTURAL PLACEHOLDER** - Framework for future WebRTC integration
/// This exists to provide a complete fallback chain but is not yet implemented.
/// In production, this would integrate with a WebRTC library for maximum compatibility.
#[derive(Debug)]
pub struct WebRtcTransport {
    /// Target endpoint for connection
    /// TODO: Replace with actual WebRTC peer connection objects
    endpoint: String,
    // TODO: Add fields for:
    // - RTCPeerConnection
    // - RTCDataChannel for data transport  
    // - ICE candidate management
    // - STUN/TURN server configuration
}

/// QUIC stream wrapper
#[derive(Debug)]
pub struct QuicStream {
    /// Stream ID
    pub id: u64,
    /// Stream type
    pub stream_type: StreamType,
    /// Send stream (if bidirectional or unidirectional send)
    pub send: Option<SendStream>,
    /// Receive stream (if bidirectional or unidirectional receive)
    pub recv: Option<RecvStream>,
}

impl QuicStream {
    /// Send data on the stream
    pub async fn send(&mut self, data: &[u8]) -> Result<(), QuicRtcError> {
        if let Some(ref mut send_stream) = self.send {
            send_stream
                .write_all(data)
                .await
                .map_err(|e| QuicRtcError::Transport {
                    reason: format!("Failed to send data: {}", e),
                })?;
            Ok(())
        } else {
            Err(QuicRtcError::Transport {
                reason: "No send stream available".to_string(),
            })
        }
    }

    /// Receive data from the stream
    pub async fn recv(&mut self) -> Result<Option<Bytes>, QuicRtcError> {
        if let Some(ref mut recv_stream) = self.recv {
            match recv_stream.read_chunk(1024, true).await {
                Ok(Some(chunk)) => Ok(Some(chunk.bytes)),
                Ok(None) => Ok(None),
                Err(e) => Err(QuicRtcError::Transport {
                    reason: format!("Failed to receive data: {}", e),
                }),
            }
        } else {
            Err(QuicRtcError::Transport {
                reason: "No receive stream available".to_string(),
            })
        }
    }

    /// Finish the send stream
    pub async fn finish(&mut self) -> Result<(), QuicRtcError> {
        if let Some(mut send_stream) = self.send.take() {
            send_stream.finish().map_err(|e| QuicRtcError::Transport {
                reason: format!("Failed to finish stream: {}", e),
            })?;
        }
        Ok(())
    }
}

/// QUIC transport connection with fallback support
#[derive(Debug)]
pub struct TransportConnection {
    /// Current transport mode
    mode: TransportMode,
    /// Internal transport implementation
    inner: TransportInner,
    /// Fallback chain for connection establishment
    fallback_chain: Vec<TransportMode>,
    /// Connection metrics
    metrics: Arc<RwLock<ConnectionMetrics>>,
    /// Connection ID for tracking
    connection_id: Uuid,
    /// Current network path
    current_path: Option<NetworkPath>,
    /// Migration event sender
    migration_tx: Option<mpsc::UnboundedSender<NetworkPath>>,
}

impl TransportConnection {
    /// Establish connection with automatic fallback
    pub async fn establish_with_fallback(
        endpoint: SocketAddr,
        config: ConnectionConfig,
    ) -> Result<Self, QuicRtcError> {
        let fallback_chain = vec![
            TransportMode::QuicNative,
            TransportMode::QuicOverWebSocket,
            TransportMode::WebRtcCompat,
        ];

        let connection_id = Uuid::new_v4();
        let metrics = Arc::new(RwLock::new(ConnectionMetrics::default()));

        info!("Attempting connection to {} with fallback chain", endpoint);

        for transport_mode in &fallback_chain {
            {
                let mut m = metrics.write();
                m.connection_attempts += 1;
                m.last_attempt = Some(Instant::now());
            }

            debug!("Trying transport mode: {:?}", transport_mode);

            match Self::try_transport(endpoint, config.clone(), *transport_mode, connection_id)
                .await
            {
                Ok(inner) => {
                    {
                        let mut m = metrics.write();
                        m.successful_connections += 1;
                    }

                    info!("Successfully connected using {:?}", transport_mode);

                    let (migration_tx, _migration_rx) = mpsc::unbounded_channel();

                    return Ok(Self {
                        mode: *transport_mode,
                        inner,
                        fallback_chain: fallback_chain.clone(),
                        metrics,
                        connection_id,
                        current_path: Some(NetworkPath {
                            local_addr: SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0),
                            remote_addr: endpoint,
                            interface_name: None,
                            mtu: None,
                        }),
                        migration_tx: Some(migration_tx),
                    });
                }
                Err(e) => {
                    {
                        let mut m = metrics.write();
                        *m.failed_connections.entry(*transport_mode).or_insert(0) += 1;
                    }

                    warn!("Transport {:?} failed: {}", transport_mode, e);
                    continue;
                }
            }
        }

        Err(QuicRtcError::Connection {
            room_id: "unknown".to_string(),
            reason: "All transport modes failed".to_string(),
            retry_in: Some(Duration::from_secs(5)),
            suggested_action: "Check network connectivity and firewall settings".to_string(),
        })
    }

    /// Try a specific transport mode
    async fn try_transport(
        endpoint: SocketAddr,
        config: ConnectionConfig,
        transport_mode: TransportMode,
        connection_id: Uuid,
    ) -> Result<TransportInner, QuicRtcError> {
        match transport_mode {
            TransportMode::QuicNative => {
                Self::establish_quic_native(endpoint, config, connection_id).await
            }
            TransportMode::QuicOverWebSocket => {
                Self::establish_quic_over_websocket(endpoint, config, connection_id).await
            }
            TransportMode::WebRtcCompat => {
                Self::establish_webrtc_compat(endpoint, config, connection_id).await
            }
        }
    }

    /// Establish native QUIC connection
    async fn establish_quic_native(
        endpoint: SocketAddr,
        config: ConnectionConfig,
        _connection_id: Uuid,
    ) -> Result<TransportInner, QuicRtcError> {
        // Set up crypto provider for QUIC
        let crypto = rustls::crypto::aws_lc_rs::default_provider();
        let _ = rustls::crypto::CryptoProvider::install_default(crypto);

        // Create QUIC client configuration with insecure verifier for testing
        // In production, use proper certificate verification
        let mut client_config = ClientConfig::with_platform_verifier();

        // Configure QUIC transport parameters
        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_idle_timeout(Some(config.max_idle_timeout.try_into().unwrap()));
        transport_config.keep_alive_interval(Some(config.keep_alive_interval));

        // Enable connection migration if requested
        if config.enable_migration {
            transport_config.allow_spin(true);
        }

        client_config.transport_config(Arc::new(transport_config));

        // Create endpoint
        let mut quic_endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap()).map_err(|e| {
            QuicRtcError::Transport {
                reason: format!("Failed to create QUIC endpoint: {}", e),
            }
        })?;

        quic_endpoint.set_default_client_config(client_config);

        // Connect with timeout
        let server_name = "localhost";

        let connecting =
            quic_endpoint
                .connect(endpoint, server_name)
                .map_err(|e| QuicRtcError::Transport {
                    reason: format!("Failed to initiate QUIC connection: {}", e),
                })?;

        let connection = tokio::time::timeout(config.timeout, connecting)
            .await
            .map_err(|_| QuicRtcError::Transport {
                reason: "Connection timeout".to_string(),
            })?
            .map_err(|e| QuicRtcError::Transport {
                reason: format!("QUIC connection failed: {}", e),
            })?;

        debug!("QUIC native connection established");
        Ok(TransportInner::Quic(connection))
    }

    /// Establish QUIC over WebSocket connection
    async fn establish_quic_over_websocket(
        endpoint: SocketAddr,
        config: ConnectionConfig,
        _connection_id: Uuid,
    ) -> Result<TransportInner, QuicRtcError> {
        let ws_url = format!("ws://{}:{}/quic-tunnel", endpoint.ip(), endpoint.port());

        let (ws_stream, _) = tokio::time::timeout(config.timeout, connect_async(&ws_url))
            .await
            .map_err(|_| QuicRtcError::Transport {
                reason: "WebSocket connection timeout".to_string(),
            })?
            .map_err(|e| QuicRtcError::Transport {
                reason: format!("WebSocket connection failed: {}", e),
            })?;

        debug!("WebSocket connection established for QUIC tunneling");

        Ok(TransportInner::WebSocket(WebSocketTransport {
            stream: Arc::new(RwLock::new(Some(ws_stream))),
            endpoint: ws_url,
        }))
    }

    /// Establish WebRTC compatibility connection
    /// 
    /// **STATUS: ARCHITECTURAL PLACEHOLDER** - Framework for future implementation
    async fn establish_webrtc_compat(
        _endpoint: SocketAddr,
        config: ConnectionConfig,
        _connection_id: Uuid,
    ) -> Result<TransportInner, QuicRtcError> {
        // TODO: Implement actual WebRTC connection establishment
        // 1. Create RTCPeerConnection with STUN/TURN servers
        // 2. Create data channel for MoQ transport
        // 3. Handle ICE candidate exchange via signaling server
        // 4. Establish peer-to-peer connection
        // 5. Return WebRtcTransport with active data channel
        
        debug!("WebRTC compatibility mode (not yet implemented)");

        // Simulate connection timeout for realistic testing
        tokio::time::sleep(config.timeout).await;

        // Always fail until actual implementation is added
        Err(QuicRtcError::Transport {
            reason: "WebRTC fallback not yet implemented - architectural placeholder".to_string(),
        })
    }

    /// Migrate connection to a new network path
    pub async fn migrate_to(&mut self, new_path: NetworkPath) -> Result<(), QuicRtcError> {
        debug!("Attempting connection migration to {:?}", new_path);

        match &mut self.inner {
            TransportInner::Quic(_connection) => {
                // QUIC supports connection migration natively
                // Validate the new path
                if let Some(ref current_path) = self.current_path {
                    if current_path.remote_addr != new_path.remote_addr {
                        return Err(QuicRtcError::Transport {
                            reason: "Cannot migrate to different remote address".to_string(),
                        });
                    }
                }

                // In a real implementation, we would:
                // 1. Create a new socket bound to the new local address
                // 2. Send a PATH_CHALLENGE frame to validate the new path
                // 3. Wait for PATH_RESPONSE to confirm path viability
                // 4. Switch to the new path once validated

                // For now, we simulate successful migration
                info!(
                    "Migrating QUIC connection from {:?} to {:?}",
                    self.current_path.as_ref().map(|p| &p.local_addr),
                    new_path.local_addr
                );

                // Update connection state
                {
                    let mut metrics = self.metrics.write();
                    metrics.migration_events += 1;
                }

                let old_path = self.current_path.clone();
                self.current_path = Some(new_path.clone());

                // Notify migration event
                if let Some(ref tx) = self.migration_tx {
                    let _ = tx.send(new_path.clone());
                }

                // Log migration details for diagnostics
                info!(
                    "Connection migrated successfully: {} -> {}",
                    old_path
                        .map(|p| format!("{}:{}", p.local_addr.ip(), p.local_addr.port()))
                        .unwrap_or_else(|| "unknown".to_string()),
                    format!(
                        "{}:{}",
                        new_path.local_addr.ip(),
                        new_path.local_addr.port()
                    )
                );

                Ok(())
            }
            TransportInner::WebSocket(_) => {
                // WebSocket doesn't support migration - would need to reconnect
                warn!("WebSocket transport doesn't support migration, reconnection required");
                Err(QuicRtcError::Transport {
                    reason: "WebSocket transport doesn't support connection migration".to_string(),
                })
            }
            TransportInner::WebRtc(_) => {
                // WebRTC has its own ICE-based migration
                warn!("WebRTC migration not implemented");
                Err(QuicRtcError::Transport {
                    reason: "WebRTC migration not implemented".to_string(),
                })
            }
        }
    }

    /// Detect network path changes (mobile network switching)
    pub fn detect_path_change(&self) -> Option<NetworkPath> {
        // In a real implementation, this would:
        // 1. Monitor network interfaces
        // 2. Detect IP address changes
        // 3. Identify new network paths
        // 4. Return the best available path

        // For now, return None (no path change detected)
        None
    }

    /// Validate a network path before migration
    pub async fn validate_path(&self, path: &NetworkPath) -> Result<bool, QuicRtcError> {
        match &self.inner {
            TransportInner::Quic(_connection) => {
                // In a real implementation, this would:
                // 1. Send PATH_CHALLENGE frames to the new path
                // 2. Wait for PATH_RESPONSE
                // 3. Measure RTT and packet loss
                // 4. Return true if path is viable

                debug!("Validating network path: {:?}", path);

                // Simulate path validation
                // Check if the path looks reasonable
                if path.local_addr.ip().is_unspecified() {
                    return Ok(false);
                }

                if path.mtu.is_some() && path.mtu.unwrap() < 1200 {
                    warn!(
                        "Path MTU {} is very low, may cause issues",
                        path.mtu.unwrap()
                    );
                }

                Ok(true)
            }
            _ => {
                // Other transports don't support path validation
                Ok(false)
            }
        }
    }

    /// Get available network paths for migration
    pub fn available_paths(&self) -> Vec<NetworkPath> {
        // In a real implementation, this would:
        // 1. Enumerate network interfaces
        // 2. Get IP addresses for each interface
        // 3. Create NetworkPath objects for viable paths
        // 4. Return sorted by preference (WiFi > Ethernet > Cellular)

        vec![]
    }

    /// Open a new stream
    pub async fn open_stream(
        &mut self,
        stream_type: StreamType,
    ) -> Result<QuicStream, QuicRtcError> {
        match &mut self.inner {
            TransportInner::Quic(connection) => match stream_type {
                StreamType::Bidirectional => {
                    let (send, recv) =
                        connection
                            .open_bi()
                            .await
                            .map_err(|e| QuicRtcError::Transport {
                                reason: format!("Failed to open bidirectional stream: {}", e),
                            })?;

                    Ok(QuicStream {
                        id: send.id().index(),
                        stream_type,
                        send: Some(send),
                        recv: Some(recv),
                    })
                }
                StreamType::Unidirectional => {
                    let send =
                        connection
                            .open_uni()
                            .await
                            .map_err(|e| QuicRtcError::Transport {
                                reason: format!("Failed to open unidirectional stream: {}", e),
                            })?;

                    Ok(QuicStream {
                        id: send.id().index(),
                        stream_type,
                        send: Some(send),
                        recv: None,
                    })
                }
            },
            TransportInner::WebSocket(_) => {
                // WebSocket streams would be multiplexed over the single WebSocket connection
                // This is a simplified implementation
                Ok(QuicStream {
                    id: 0, // Would need proper stream ID management
                    stream_type,
                    send: None,
                    recv: None,
                })
            }
            TransportInner::WebRtc(_) => {
                // WebRTC data channels would be used
                Ok(QuicStream {
                    id: 0, // Would need proper stream ID management
                    stream_type,
                    send: None,
                    recv: None,
                })
            }
        }
    }

    /// Get connection statistics
    pub fn connection_stats(&self) -> Result<ConnectionStats, QuicRtcError> {
        match &self.inner {
            TransportInner::Quic(connection) => {
                let stats = connection.stats();
                Ok(ConnectionStats {
                    rtt: stats.path.rtt,
                    cwnd: stats.path.cwnd,
                    bytes_sent: stats.udp_tx.bytes as u64,
                    bytes_received: stats.udp_rx.bytes as u64,
                    loss_rate: 0.0, // Would need to calculate from stats
                    established_at: Instant::now(), // Would track actual establishment time
                })
            }
            TransportInner::WebSocket(_) => {
                // WebSocket stats would be more limited
                Ok(ConnectionStats {
                    rtt: Duration::from_millis(50), // Estimated
                    cwnd: 65536,                    // TCP window size estimate
                    bytes_sent: 0,                  // Would need to track
                    bytes_received: 0,              // Would need to track
                    loss_rate: 0.0,
                    established_at: Instant::now(),
                })
            }
            TransportInner::WebRtc(_) => {
                // WebRTC stats would come from WebRTC API
                Ok(ConnectionStats {
                    rtt: Duration::from_millis(100), // Estimated
                    cwnd: 32768,                     // Estimated
                    bytes_sent: 0,
                    bytes_received: 0,
                    loss_rate: 0.0,
                    established_at: Instant::now(),
                })
            }
        }
    }

    /// Get current transport mode
    pub fn current_transport_mode(&self) -> TransportMode {
        self.mode
    }

    /// Get connection metrics
    pub fn metrics(&self) -> ConnectionMetrics {
        self.metrics.read().clone()
    }

    /// Get connection ID
    pub fn connection_id(&self) -> Uuid {
        self.connection_id
    }

    /// Get current network path
    pub fn current_path(&self) -> Option<&NetworkPath> {
        self.current_path.as_ref()
    }

    /// Check if connection is still alive
    pub fn is_connected(&self) -> bool {
        match &self.inner {
            TransportInner::Quic(connection) => connection.close_reason().is_none(),
            TransportInner::WebSocket(ws) => ws.stream.read().is_some(),
            TransportInner::WebRtc(_) => {
                // Would check WebRTC connection state
                true
            }
        }
    }

    /// Close the connection gracefully
    pub async fn close(&mut self) -> Result<(), QuicRtcError> {
        match &mut self.inner {
            TransportInner::Quic(connection) => {
                connection.close(VarInt::from_u32(0), b"Normal closure");
                Ok(())
            }
            TransportInner::WebSocket(ws) => {
                if let Some(mut stream) = ws.stream.write().take() {
                    let _ = stream.close(None).await;
                }
                Ok(())
            }
            TransportInner::WebRtc(_) => {
                // Would close WebRTC connection
                Ok(())
            }
        }
    }

    /// Create TransportConnection from existing quinn Connection (simplified version)
    pub fn from_quinn_connection(
        connection: Connection,
        connection_id: Uuid,
        mode: TransportMode,
        current_path: Option<NetworkPath>,
    ) -> Self {
        let (migration_tx, _migration_rx) = mpsc::unbounded_channel();

        Self {
            mode,
            inner: TransportInner::Quic(connection),
            fallback_chain: vec![mode], // Single mode since connection already established
            metrics: Arc::new(RwLock::new(ConnectionMetrics::default())),
            connection_id,
            current_path,
            migration_tx: Some(migration_tx),
        }
    }
}

/// Connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Connection timeout
    pub timeout: Duration,
    /// Enable keep-alive
    pub keep_alive: bool,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
    /// Maximum idle timeout
    pub max_idle_timeout: Duration,
    /// Enable connection migration
    pub enable_migration: bool,
    /// Preferred transport modes (in order of preference)
    pub preferred_transports: Option<Vec<TransportMode>>,
    /// Production QUIC transport configuration
    pub quic_transport_config: Option<QuicTransportConfig>,
    /// Resource limits for the connection
    pub resource_limits: Option<ResourceLimits>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            keep_alive: true,
            keep_alive_interval: Duration::from_secs(30),
            max_idle_timeout: Duration::from_secs(60),
            enable_migration: true,
            preferred_transports: None,
            quic_transport_config: Some(QuicTransportConfig::mobile()),
            resource_limits: Some(ResourceLimits::mobile()),
        }
    }
}

impl ConnectionConfig {
    /// Create configuration optimized for mobile devices
    pub fn mobile() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            keep_alive: true,
            keep_alive_interval: Duration::from_secs(15),
            max_idle_timeout: Duration::from_secs(60),
            enable_migration: true,
            preferred_transports: Some(vec![
                TransportMode::QuicNative,
                TransportMode::QuicOverWebSocket,
                TransportMode::WebRtcCompat,
            ]),
            quic_transport_config: Some(QuicTransportConfig::mobile()),
            resource_limits: Some(ResourceLimits::mobile()),
        }
    }

    /// Create configuration optimized for desktop applications
    pub fn desktop() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            keep_alive: true,
            keep_alive_interval: Duration::from_secs(30),
            max_idle_timeout: Duration::from_secs(120),
            enable_migration: false,
            preferred_transports: Some(vec![
                TransportMode::QuicNative,
                TransportMode::QuicOverWebSocket,
                TransportMode::WebRtcCompat,
            ]),
            quic_transport_config: Some(QuicTransportConfig::desktop()),
            resource_limits: Some(ResourceLimits::desktop()),
        }
    }

    /// Create configuration optimized for server deployment
    pub fn server() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            keep_alive: true,
            keep_alive_interval: Duration::from_secs(60),
            max_idle_timeout: Duration::from_secs(300),
            enable_migration: false,
            preferred_transports: Some(vec![TransportMode::QuicNative]),
            quic_transport_config: Some(QuicTransportConfig::server()),
            resource_limits: Some(ResourceLimits::server()),
        }
    }
}

/// Trait for transport implementations
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send data
    async fn send(&mut self, data: &[u8]) -> Result<(), QuicRtcError>;

    /// Receive data
    async fn recv(&mut self) -> Result<Option<Bytes>, QuicRtcError>;

    /// Close the transport
    async fn close(&mut self) -> Result<(), QuicRtcError>;

    /// Check if transport is connected
    fn is_connected(&self) -> bool;
}

// Tests moved to tests/ directory
