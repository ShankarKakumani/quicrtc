//! Resource management system for QUIC RTC
//!
//! This module provides resource monitoring, limits enforcement, and cleanup
//! mechanisms to ensure efficient operation across mobile and desktop platforms.

use crate::error::QuicRtcError;
use crate::transport::TransportConnection;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info};
use uuid::Uuid;

/// Resource limits configuration
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory usage in MB (None = unlimited)
    pub max_memory_mb: Option<u64>,
    /// Maximum bandwidth usage in kbps (None = unlimited)
    pub max_bandwidth_kbps: Option<u64>,
    /// Maximum number of concurrent connections (None = unlimited)
    pub max_connections: Option<u32>,
    /// Maximum streams per connection (None = unlimited)
    pub max_streams_per_connection: Option<u32>,
    /// Maximum number of MoQ objects in cache
    pub max_cached_objects: Option<u32>,
    /// Resource cleanup timeout
    pub cleanup_timeout: Duration,
    /// Warning threshold (percentage of limit)
    pub warning_threshold: f32,
}

impl ResourceLimits {
    /// Mobile-optimized resource limits (conservative for battery and memory)
    pub fn mobile() -> Self {
        Self {
            max_memory_mb: Some(50),              // Conservative for mobile
            max_bandwidth_kbps: Some(2000),       // 2 Mbps max for cellular
            max_connections: Some(5),             // Limited concurrent calls
            max_streams_per_connection: Some(10), // Audio + video + data
            max_cached_objects: Some(100),        // Small cache for mobile
            cleanup_timeout: Duration::from_secs(5),
            warning_threshold: 0.8, // Warn at 80%
        }
    }

    /// Desktop-optimized resource limits (higher performance)
    pub fn desktop() -> Self {
        Self {
            max_memory_mb: Some(200),             // Higher limits for desktop
            max_bandwidth_kbps: Some(10000),      // 10 Mbps max
            max_connections: Some(20),            // More concurrent calls
            max_streams_per_connection: Some(50), // Multiple video qualities
            max_cached_objects: Some(500),        // Larger cache for performance
            cleanup_timeout: Duration::from_secs(10),
            warning_threshold: 0.85, // Warn at 85%
        }
    }

    /// Server-optimized resource limits (high capacity)
    pub fn server() -> Self {
        Self {
            max_memory_mb: Some(1024),             // 1GB for server
            max_bandwidth_kbps: Some(100000),      // 100 Mbps
            max_connections: Some(1000),           // Many concurrent connections
            max_streams_per_connection: Some(100), // SFU capabilities
            max_cached_objects: Some(10000),       // Large cache
            cleanup_timeout: Duration::from_secs(30),
            warning_threshold: 0.9, // Warn at 90%
        }
    }

    /// Unlimited resources (for testing)
    pub fn unlimited() -> Self {
        Self {
            max_memory_mb: None,
            max_bandwidth_kbps: None,
            max_connections: None,
            max_streams_per_connection: None,
            max_cached_objects: None,
            cleanup_timeout: Duration::from_secs(60),
            warning_threshold: 0.95,
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        // Default to desktop configuration
        Self::desktop()
    }
}

/// Current resource usage
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    /// Current memory usage in MB
    pub memory_mb: u64,
    /// Current bandwidth usage in kbps
    pub bandwidth_kbps: u64,
    /// Number of active connections
    pub active_connections: u32,
    /// Number of active streams across all connections
    pub active_streams: u32,
    /// Number of cached MoQ objects
    pub cached_objects: u32,
    /// CPU usage percentage (0.0 to 100.0)
    pub cpu_usage_percent: f32,
    /// Timestamp when usage was measured
    pub measured_at: Instant,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            memory_mb: 0,
            bandwidth_kbps: 0,
            active_connections: 0,
            active_streams: 0,
            cached_objects: 0,
            cpu_usage_percent: 0.0,
            measured_at: Instant::now(),
        }
    }
}

/// Resource warning types
#[derive(Debug, Clone)]
pub enum ResourceWarning {
    /// Memory usage approaching limit
    MemoryApproachingLimit {
        /// Current memory usage in MB
        current_mb: u64,
        /// Memory limit in MB
        limit_mb: u64,
        /// Percentage of limit being used
        percentage: f32,
    },
    /// Bandwidth usage approaching limit
    BandwidthApproachingLimit {
        /// Current bandwidth usage in kbps
        current_kbps: u64,
        /// Bandwidth limit in kbps
        limit_kbps: u64,
        /// Percentage of limit being used
        percentage: f32,
    },
    /// Too many connections
    ConnectionsApproachingLimit {
        /// Current number of connections
        current: u32,
        /// Connection limit
        limit: u32,
        /// Percentage of limit being used
        percentage: f32,
    },
    /// Too many streams
    StreamsApproachingLimit {
        /// Current number of streams
        current: u32,
        /// Stream limit
        limit: u32,
        /// Percentage of limit being used
        percentage: f32,
    },
    /// Cache approaching limit
    CacheApproachingLimit {
        /// Current number of cached objects
        current: u32,
        /// Cache limit
        limit: u32,
        /// Percentage of limit being used
        percentage: f32,
    },
    /// High CPU usage
    HighCpuUsage {
        /// Current CPU usage percentage
        current_percent: f32,
    },
}

impl ResourceWarning {
    /// Get severity level of the warning
    pub fn severity(&self) -> WarningSeverity {
        match self {
            ResourceWarning::MemoryApproachingLimit { percentage, .. }
            | ResourceWarning::BandwidthApproachingLimit { percentage, .. }
            | ResourceWarning::ConnectionsApproachingLimit { percentage, .. }
            | ResourceWarning::StreamsApproachingLimit { percentage, .. }
            | ResourceWarning::CacheApproachingLimit { percentage, .. } => {
                if *percentage >= 0.95 {
                    WarningSeverity::Critical
                } else if *percentage >= 0.9 {
                    WarningSeverity::High
                } else if *percentage >= 0.8 {
                    WarningSeverity::Medium
                } else {
                    WarningSeverity::Low
                }
            }
            ResourceWarning::HighCpuUsage { current_percent } => {
                if *current_percent >= 90.0 {
                    WarningSeverity::Critical
                } else if *current_percent >= 80.0 {
                    WarningSeverity::High
                } else {
                    WarningSeverity::Medium
                }
            }
        }
    }

    /// Get recommended action for this warning
    pub fn recommended_action(&self) -> String {
        match self {
            ResourceWarning::MemoryApproachingLimit { .. } => {
                "Consider reducing video quality or closing unused connections".to_string()
            }
            ResourceWarning::BandwidthApproachingLimit { .. } => {
                "Reduce video bitrate or switch to audio-only mode".to_string()
            }
            ResourceWarning::ConnectionsApproachingLimit { .. } => {
                "Close idle connections or increase connection limits".to_string()
            }
            ResourceWarning::StreamsApproachingLimit { .. } => {
                "Reduce number of simultaneous streams per connection".to_string()
            }
            ResourceWarning::CacheApproachingLimit { .. } => {
                "Clear cached objects or increase cache size".to_string()
            }
            ResourceWarning::HighCpuUsage { .. } => {
                "Reduce processing load or enable hardware acceleration".to_string()
            }
        }
    }
}

/// Warning severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningSeverity {
    /// Low severity warning
    Low,
    /// Medium severity warning
    Medium,
    /// High severity warning
    High,
    /// Critical severity warning
    Critical,
}

/// Resource monitoring configuration
#[derive(Debug, Clone)]
pub struct ResourceMonitorConfig {
    /// How often to collect resource metrics
    pub collection_interval: Duration,
    /// How long to keep historical metrics
    pub history_retention: Duration,
    /// Enable automatic cleanup when approaching limits
    pub auto_cleanup: bool,
    /// Enable battery optimization (mobile only)
    pub battery_optimization: bool,
}

impl Default for ResourceMonitorConfig {
    fn default() -> Self {
        Self {
            collection_interval: Duration::from_secs(5),
            history_retention: Duration::from_secs(300), // 5 minutes
            auto_cleanup: true,
            battery_optimization: false,
        }
    }
}

/// Connection pool for efficient resource management
#[derive(Debug)]
pub struct ConnectionPool {
    /// Active connections currently in use
    active_connections: HashMap<Uuid, Arc<RwLock<TransportConnection>>>,
    /// Idle connections available for reuse
    idle_connections: Vec<Arc<RwLock<TransportConnection>>>,
    /// Pool configuration
    config: ConnectionPoolConfig,
    /// Pool metrics
    metrics: ConnectionPoolMetrics,
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of idle connections to keep
    pub max_idle_connections: u32,
    /// How long to keep idle connections before cleanup
    pub idle_timeout: Duration,
    /// Maximum total connections (active + idle)
    pub max_total_connections: u32,
    /// Enable connection reuse
    pub enable_reuse: bool,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_idle_connections: 5,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            max_total_connections: 50,
            enable_reuse: true,
        }
    }
}

/// Connection pool metrics
#[derive(Debug, Clone)]
pub struct ConnectionPoolMetrics {
    /// Total number of connections created
    pub total_created: u64,
    /// Number of connections reused from pool
    pub reused_connections: u64,
    /// Number of connections returned to pool
    pub returned_connections: u64,
    /// Number of idle connections cleaned up
    pub cleaned_up_connections: u64,
    /// Current pool efficiency (reuse rate)
    pub efficiency_percentage: f32,
}

impl Default for ConnectionPoolMetrics {
    fn default() -> Self {
        Self {
            total_created: 0,
            reused_connections: 0,
            returned_connections: 0,
            cleaned_up_connections: 0,
            efficiency_percentage: 0.0,
        }
    }
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: ConnectionPoolConfig) -> Self {
        Self {
            active_connections: HashMap::new(),
            idle_connections: Vec::new(),
            config,
            metrics: ConnectionPoolMetrics::default(),
        }
    }

    /// Get a connection from the pool or create a new one
    pub async fn get_connection(
        &mut self,
        endpoint: std::net::SocketAddr,
    ) -> Result<Arc<RwLock<TransportConnection>>, QuicRtcError> {
        // Try to reuse an idle connection if available and reuse is enabled
        if self.config.enable_reuse && !self.idle_connections.is_empty() {
            if let Some(connection) = self.idle_connections.pop() {
                let connection_id = connection.read().connection_id();
                self.active_connections
                    .insert(connection_id, connection.clone());
                self.metrics.reused_connections += 1;
                self.update_efficiency();

                debug!("Reused connection from pool: {}", connection_id);
                return Ok(connection);
            }
        }

        // Check if we're at the total connection limit
        let total_connections = self.active_connections.len() + self.idle_connections.len();
        if total_connections >= self.config.max_total_connections as usize {
            return Err(QuicRtcError::ResourceLimit {
                resource: format!(
                    "Total connections limit ({}) exceeded",
                    self.config.max_total_connections
                ),
            });
        }

        // Create a new connection
        let connection_config = crate::transport::ConnectionConfig::default();
        let transport_connection =
            TransportConnection::establish_with_fallback(endpoint, connection_config).await?;
        let connection = Arc::new(RwLock::new(transport_connection));

        let connection_id = connection.read().connection_id();
        self.active_connections
            .insert(connection_id, connection.clone());
        self.metrics.total_created += 1;
        self.update_efficiency();

        info!("Created new connection: {}", connection_id);
        Ok(connection)
    }

    /// Return a connection to the pool
    pub fn return_connection(&mut self, connection: Arc<RwLock<TransportConnection>>) {
        let connection_id = connection.read().connection_id();

        // Remove from active connections
        if self.active_connections.remove(&connection_id).is_some() {
            // Check if connection is still alive and add to idle pool if under limit
            if connection.read().is_connected()
                && self.idle_connections.len() < self.config.max_idle_connections as usize
            {
                self.idle_connections.push(connection);
                self.metrics.returned_connections += 1;
                debug!("Returned connection to pool: {}", connection_id);
            } else {
                debug!(
                    "Discarded connection (pool full or connection dead): {}",
                    connection_id
                );
            }
        }

        self.update_efficiency();
    }

    /// Clean up idle connections that have exceeded the timeout
    pub async fn cleanup_idle_connections(&mut self) {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (index, connection) in self.idle_connections.iter().enumerate() {
            let connection_guard = connection.read();

            // Check if connection is dead or has exceeded idle timeout
            if !connection_guard.is_connected() {
                to_remove.push(index);
                continue;
            }

            // For simplicity, we'll consider all idle connections as potentially expired
            // In a real implementation, we'd track when each connection was last used
            to_remove.push(index);
        }

        // Remove in reverse order to maintain indices
        for &index in to_remove.iter().rev() {
            if let Some(connection) = self.idle_connections.get(index) {
                let connection_id = connection.read().connection_id();
                debug!("Cleaning up idle connection: {}", connection_id);

                // Close the connection gracefully
                if let Some(mut conn) = connection.try_write() {
                    let _ = conn.close().await;
                }
            }
            self.idle_connections.remove(index);
            self.metrics.cleaned_up_connections += 1;
        }

        self.update_efficiency();
    }

    /// Get pool statistics
    pub fn get_pool_stats(&self) -> ConnectionPoolStats {
        ConnectionPoolStats {
            active_connections: self.active_connections.len() as u32,
            idle_connections: self.idle_connections.len() as u32,
            total_connections: (self.active_connections.len() + self.idle_connections.len()) as u32,
            metrics: self.metrics.clone(),
        }
    }

    /// Update efficiency calculation
    fn update_efficiency(&mut self) {
        if self.metrics.total_created > 0 {
            self.metrics.efficiency_percentage = (self.metrics.reused_connections as f32
                / self.metrics.total_created as f32)
                * 100.0;
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct ConnectionPoolStats {
    /// Number of active connections
    pub active_connections: u32,
    /// Number of idle connections
    pub idle_connections: u32,
    /// Total connections in pool
    pub total_connections: u32,
    /// Pool metrics
    pub metrics: ConnectionPoolMetrics,
}

/// Main resource manager
#[derive(Debug)]
pub struct ResourceManager {
    /// Resource limits configuration
    limits: ResourceLimits,
    /// Current resource usage
    current_usage: Arc<RwLock<ResourceUsage>>,
    /// Resource usage history
    usage_history: Arc<RwLock<Vec<ResourceUsage>>>,
    /// Connection pool
    connection_pool: Arc<tokio::sync::RwLock<ConnectionPool>>,
    /// Monitor configuration
    monitor_config: ResourceMonitorConfig,
    /// Warning channel sender
    warning_tx: mpsc::UnboundedSender<ResourceWarning>,
    /// Cleanup task handle
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    /// Monitoring task handle
    monitor_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new(limits: ResourceLimits) -> (Self, mpsc::UnboundedReceiver<ResourceWarning>) {
        let (warning_tx, warning_rx) = mpsc::unbounded_channel();

        let manager = Self {
            limits,
            current_usage: Arc::new(RwLock::new(ResourceUsage::default())),
            usage_history: Arc::new(RwLock::new(Vec::new())),
            connection_pool: Arc::new(tokio::sync::RwLock::new(ConnectionPool::new(
                ConnectionPoolConfig::default(),
            ))),
            monitor_config: ResourceMonitorConfig::default(),
            warning_tx,
            cleanup_handle: None,
            monitor_handle: None,
        };

        (manager, warning_rx)
    }

    /// Create a mobile-optimized resource manager
    pub fn mobile() -> (Self, mpsc::UnboundedReceiver<ResourceWarning>) {
        let mut limits = ResourceLimits::mobile();
        let (mut manager, warning_rx) = Self::new(limits);

        // Enable mobile optimizations
        manager.monitor_config.battery_optimization = true;
        manager.monitor_config.collection_interval = Duration::from_secs(10); // Less frequent on mobile

        (manager, warning_rx)
    }

    /// Create a desktop-optimized resource manager
    pub fn desktop() -> (Self, mpsc::UnboundedReceiver<ResourceWarning>) {
        Self::new(ResourceLimits::desktop())
    }

    /// Start resource monitoring
    pub async fn start_monitoring(&mut self) -> Result<(), QuicRtcError> {
        if self.monitor_handle.is_some() {
            return Ok(()); // Already monitoring
        }

        let current_usage = self.current_usage.clone();
        let usage_history = self.usage_history.clone();
        let limits = self.limits.clone();
        let warning_tx = self.warning_tx.clone();
        let interval_duration = self.monitor_config.collection_interval;
        let history_retention = self.monitor_config.history_retention;

        let monitor_handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                // Collect current resource usage
                let usage = Self::collect_resource_usage().await;

                // Update current usage
                {
                    let mut current = current_usage.write();
                    *current = usage.clone();
                }

                // Add to history
                {
                    let mut history = usage_history.write();
                    history.push(usage.clone());

                    // Clean up old history entries
                    let cutoff_time = Instant::now() - history_retention;
                    history.retain(|entry| entry.measured_at > cutoff_time);
                }

                // Check for warnings
                let warnings = Self::check_for_warnings(&usage, &limits);
                for warning in warnings {
                    if let Err(_) = warning_tx.send(warning) {
                        break; // Receiver dropped, stop monitoring
                    }
                }
            }
        });

        self.monitor_handle = Some(monitor_handle);
        info!("Started resource monitoring");
        Ok(())
    }

    /// Stop resource monitoring
    pub async fn stop_monitoring(&mut self) {
        if let Some(handle) = self.monitor_handle.take() {
            handle.abort();
            info!("Stopped resource monitoring");
        }

        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
            info!("Stopped cleanup task");
        }
    }

    /// Start automatic cleanup task
    pub async fn start_auto_cleanup(&mut self) -> Result<(), QuicRtcError> {
        if !self.monitor_config.auto_cleanup {
            return Ok(());
        }

        if self.cleanup_handle.is_some() {
            return Ok(()); // Already running
        }

        // For now, we'll just log that auto cleanup is requested
        // In a production implementation, this would spawn a background task
        // that periodically calls cleanup_resources()
        info!("Auto cleanup requested (manual cleanup available via cleanup_resources())");
        Ok(())
    }

    /// Check if current usage is within limits
    pub fn check_limits(&self) -> Result<(), QuicRtcError> {
        let usage = self.current_usage.read();

        // Check memory limit
        if let Some(limit) = self.limits.max_memory_mb {
            if usage.memory_mb > limit {
                return Err(QuicRtcError::ResourceLimit {
                    resource: format!(
                        "Memory usage ({} MB) exceeds limit ({} MB)",
                        usage.memory_mb, limit
                    ),
                });
            }
        }

        // Check bandwidth limit
        if let Some(limit) = self.limits.max_bandwidth_kbps {
            if usage.bandwidth_kbps > limit {
                return Err(QuicRtcError::ResourceLimit {
                    resource: format!(
                        "Bandwidth usage ({} kbps) exceeds limit ({} kbps)",
                        usage.bandwidth_kbps, limit
                    ),
                });
            }
        }

        // Check connection limit
        if let Some(limit) = self.limits.max_connections {
            if usage.active_connections > limit {
                return Err(QuicRtcError::ResourceLimit {
                    resource: format!(
                        "Active connections ({}) exceed limit ({})",
                        usage.active_connections, limit
                    ),
                });
            }
        }

        // Check streams limit
        if let Some(limit) = self.limits.max_streams_per_connection {
            if usage.active_streams > limit {
                return Err(QuicRtcError::ResourceLimit {
                    resource: format!(
                        "Active streams ({}) exceed limit ({})",
                        usage.active_streams, limit
                    ),
                });
            }
        }

        Ok(())
    }

    /// Get current resource usage
    pub fn current_usage(&self) -> ResourceUsage {
        self.current_usage.read().clone()
    }

    /// Get resource usage history
    pub fn usage_history(&self) -> Vec<ResourceUsage> {
        self.usage_history.read().clone()
    }

    /// Check if approaching any resource limits
    pub fn approaching_limits(&self) -> Vec<ResourceWarning> {
        let usage = self.current_usage.read();
        Self::check_for_warnings(&usage, &self.limits)
    }

    /// Force cleanup of resources
    pub async fn cleanup_resources(&mut self) -> Result<(), QuicRtcError> {
        info!("Starting manual resource cleanup");

        // Clean up connection pool
        {
            let mut pool = self.connection_pool.write().await;
            pool.cleanup_idle_connections().await;
        }

        // Clear old usage history
        {
            let mut history = self.usage_history.write();
            let cutoff_time = Instant::now() - self.monitor_config.history_retention;
            let old_count = history.len();
            history.retain(|entry| entry.measured_at > cutoff_time);
            let new_count = history.len();

            if old_count > new_count {
                debug!(
                    "Cleaned up {} old usage history entries",
                    old_count - new_count
                );
            }
        }

        info!("Resource cleanup completed");
        Ok(())
    }

    /// Get connection pool reference
    pub fn connection_pool(&self) -> Arc<tokio::sync::RwLock<ConnectionPool>> {
        self.connection_pool.clone()
    }

    /// Update resource limits
    pub fn update_limits(&mut self, new_limits: ResourceLimits) {
        self.limits = new_limits;
        info!("Updated resource limits");
    }

    /// Get current resource limits
    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// Collect current resource usage (platform-specific implementation)
    async fn collect_resource_usage() -> ResourceUsage {
        // In a real implementation, this would use platform-specific APIs
        // to collect actual memory, CPU, and network usage

        ResourceUsage {
            memory_mb: Self::get_memory_usage(),
            bandwidth_kbps: Self::get_bandwidth_usage(),
            active_connections: Self::get_active_connections(),
            active_streams: Self::get_active_streams(),
            cached_objects: Self::get_cached_objects(),
            cpu_usage_percent: Self::get_cpu_usage(),
            measured_at: Instant::now(),
        }
    }

    /// Get current memory usage
    fn get_memory_usage() -> u64 {
        // TODO: Implement platform-specific memory monitoring
        // - Windows: GetProcessMemoryInfo
        // - Linux: /proc/self/status or /proc/self/statm
        // - macOS: task_info with TASK_BASIC_INFO
        todo!("Implement platform-specific memory usage monitoring")
    }

    /// Get current bandwidth usage
    fn get_bandwidth_usage() -> u64 {
        // TODO: Implement network I/O tracking from QUIC connections
        // Should aggregate bytes sent/received across all active connections
        // and calculate bandwidth over a sliding time window
        todo!("Implement bandwidth usage tracking from QUIC connections")
    }

    /// Get number of active connections
    fn get_active_connections() -> u32 {
        // TODO: Track this from the connection pool
        // Should return the actual count of active connections
        todo!("Implement active connection counting from connection pool")
    }

    /// Get number of active streams
    fn get_active_streams() -> u32 {
        // TODO: Track this across all connections
        // Should aggregate stream counts from all active connections
        todo!("Implement active stream counting across all connections")
    }

    /// Get number of cached objects
    fn get_cached_objects() -> u32 {
        // TODO: Track this from the MoQ object cache
        // Should return the actual count of cached MoQ objects
        todo!("Implement cached object counting from MoQ cache")
    }

    /// Get current CPU usage
    fn get_cpu_usage() -> f32 {
        // TODO: Implement platform-specific CPU monitoring
        // - Windows: GetSystemTimes or PdhCollectQueryData
        // - Linux: /proc/stat parsing
        // - macOS: host_processor_info with PROCESSOR_CPU_LOAD_INFO
        todo!("Implement platform-specific CPU usage monitoring")
    }

    /// Check for resource warnings
    pub fn check_for_warnings(
        usage: &ResourceUsage,
        limits: &ResourceLimits,
    ) -> Vec<ResourceWarning> {
        let mut warnings = Vec::new();

        // Check memory warning
        if let Some(limit) = limits.max_memory_mb {
            let percentage = usage.memory_mb as f32 / limit as f32;
            if percentage >= limits.warning_threshold {
                warnings.push(ResourceWarning::MemoryApproachingLimit {
                    current_mb: usage.memory_mb,
                    limit_mb: limit,
                    percentage,
                });
            }
        }

        // Check bandwidth warning
        if let Some(limit) = limits.max_bandwidth_kbps {
            let percentage = usage.bandwidth_kbps as f32 / limit as f32;
            if percentage >= limits.warning_threshold {
                warnings.push(ResourceWarning::BandwidthApproachingLimit {
                    current_kbps: usage.bandwidth_kbps,
                    limit_kbps: limit,
                    percentage,
                });
            }
        }

        // Check connections warning
        if let Some(limit) = limits.max_connections {
            let percentage = usage.active_connections as f32 / limit as f32;
            if percentage >= limits.warning_threshold {
                warnings.push(ResourceWarning::ConnectionsApproachingLimit {
                    current: usage.active_connections,
                    limit,
                    percentage,
                });
            }
        }

        // Check streams warning
        if let Some(limit) = limits.max_streams_per_connection {
            let percentage = usage.active_streams as f32 / limit as f32;
            if percentage >= limits.warning_threshold {
                warnings.push(ResourceWarning::StreamsApproachingLimit {
                    current: usage.active_streams,
                    limit,
                    percentage,
                });
            }
        }

        // Check cache warning
        if let Some(limit) = limits.max_cached_objects {
            let percentage = usage.cached_objects as f32 / limit as f32;
            if percentage >= limits.warning_threshold {
                warnings.push(ResourceWarning::CacheApproachingLimit {
                    current: usage.cached_objects,
                    limit,
                    percentage,
                });
            }
        }

        // Check CPU warning
        if usage.cpu_usage_percent >= 80.0 {
            warnings.push(ResourceWarning::HighCpuUsage {
                current_percent: usage.cpu_usage_percent,
            });
        }

        warnings
    }
}

impl Drop for ResourceManager {
    fn drop(&mut self) {
        // Clean up monitoring tasks
        if let Some(handle) = self.monitor_handle.take() {
            handle.abort();
        }

        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_presets() {
        let mobile = ResourceLimits::mobile();
        let desktop = ResourceLimits::desktop();
        let server = ResourceLimits::server();

        // Mobile should have more conservative limits
        assert!(mobile.max_memory_mb.unwrap() < desktop.max_memory_mb.unwrap());
        assert!(mobile.max_bandwidth_kbps.unwrap() < desktop.max_bandwidth_kbps.unwrap());
        assert!(mobile.max_connections.unwrap() < desktop.max_connections.unwrap());

        // Server should have highest limits
        assert!(server.max_memory_mb.unwrap() > desktop.max_memory_mb.unwrap());
        assert!(server.max_bandwidth_kbps.unwrap() > desktop.max_bandwidth_kbps.unwrap());
        assert!(server.max_connections.unwrap() > desktop.max_connections.unwrap());
    }

    #[test]
    fn test_resource_warning_severity() {
        let warning = ResourceWarning::MemoryApproachingLimit {
            current_mb: 95,
            limit_mb: 100,
            percentage: 0.95,
        };

        assert_eq!(warning.severity(), WarningSeverity::Critical);

        let action = warning.recommended_action();
        assert!(!action.is_empty());
    }

    #[tokio::test]
    async fn test_resource_manager_creation() {
        let (manager, _warning_rx) = ResourceManager::new(ResourceLimits::mobile());

        let usage = manager.current_usage();
        assert_eq!(usage.memory_mb, 0); // Should start at 0

        let limits = manager.limits();
        assert_eq!(limits.max_memory_mb, Some(50)); // Mobile preset
    }

    #[tokio::test]
    async fn test_connection_pool() {
        let mut pool = ConnectionPool::new(ConnectionPoolConfig::default());

        let stats = pool.get_pool_stats();
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.idle_connections, 0);

        // Test that pool starts empty
        assert!(pool.idle_connections.is_empty());
        assert!(pool.active_connections.is_empty());
    }

    #[test]
    fn test_resource_usage_default() {
        let usage = ResourceUsage::default();

        assert_eq!(usage.memory_mb, 0);
        assert_eq!(usage.bandwidth_kbps, 0);
        assert_eq!(usage.active_connections, 0);
        assert_eq!(usage.active_streams, 0);
        assert_eq!(usage.cached_objects, 0);
        assert_eq!(usage.cpu_usage_percent, 0.0);
    }

    #[test]
    fn test_check_for_warnings() {
        let limits = ResourceLimits::mobile();
        let usage = ResourceUsage {
            memory_mb: 45,           // 90% of 50MB limit
            bandwidth_kbps: 1800,    // 90% of 2000 kbps limit
            active_connections: 4,   // 80% of 5 connection limit
            active_streams: 8,       // 80% of 10 stream limit
            cached_objects: 90,      // 90% of 100 object limit
            cpu_usage_percent: 85.0, // High CPU usage
            measured_at: Instant::now(),
        };

        let warnings = ResourceManager::check_for_warnings(&usage, &limits);

        // Should have warnings for memory, bandwidth, cache, and CPU
        assert!(warnings.len() >= 4);

        // Check that we have the expected warning types
        let has_memory_warning = warnings
            .iter()
            .any(|w| matches!(w, ResourceWarning::MemoryApproachingLimit { .. }));
        let has_bandwidth_warning = warnings
            .iter()
            .any(|w| matches!(w, ResourceWarning::BandwidthApproachingLimit { .. }));
        let has_cpu_warning = warnings
            .iter()
            .any(|w| matches!(w, ResourceWarning::HighCpuUsage { .. }));

        assert!(has_memory_warning);
        assert!(has_bandwidth_warning);
        assert!(has_cpu_warning);
    }
}
