//! Resource Management Demo
//!
//! This example demonstrates the resource management system with different
//! presets (mobile, desktop, server) and shows monitoring, limits enforcement,
//! and cleanup mechanisms.

use quicrtc_core::{QuicRtcError, ResourceLimits, ResourceManager};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔧 QUIC RTC Resource Management Demo");
    println!("====================================");

    // Demo 1: Resource Limits Presets
    demo_resource_presets().await?;

    // Demo 2: Resource Monitoring
    demo_resource_monitoring().await?;

    // Demo 3: Resource Warnings
    demo_resource_warnings().await?;

    // Demo 4: Connection Pool Management
    demo_connection_pool().await?;

    // Demo 5: Mobile vs Desktop Optimization
    demo_mobile_vs_desktop().await?;

    // Demo 6: Error Handling
    demo_error_scenarios().await?;

    println!("\n✨ Resource management demo completed!");
    Ok(())
}

async fn demo_resource_presets() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📋 Demo 1: Resource Limits Presets");
    println!("----------------------------------");

    let mobile = ResourceLimits::mobile();
    let desktop = ResourceLimits::desktop();
    let server = ResourceLimits::server();
    let unlimited = ResourceLimits::unlimited();

    println!("📱 Mobile preset:");
    println!("  Memory: {} MB", mobile.max_memory_mb.unwrap_or(0));
    println!(
        "  Bandwidth: {} kbps",
        mobile.max_bandwidth_kbps.unwrap_or(0)
    );
    println!("  Connections: {}", mobile.max_connections.unwrap_or(0));
    println!(
        "  Warning threshold: {:.0}%",
        mobile.warning_threshold * 100.0
    );

    println!("🖥️  Desktop preset:");
    println!("  Memory: {} MB", desktop.max_memory_mb.unwrap_or(0));
    println!(
        "  Bandwidth: {} kbps",
        desktop.max_bandwidth_kbps.unwrap_or(0)
    );
    println!("  Connections: {}", desktop.max_connections.unwrap_or(0));
    println!(
        "  Warning threshold: {:.0}%",
        desktop.warning_threshold * 100.0
    );

    println!("🖥️  Server preset:");
    println!("  Memory: {} MB", server.max_memory_mb.unwrap_or(0));
    println!(
        "  Bandwidth: {} kbps",
        server.max_bandwidth_kbps.unwrap_or(0)
    );
    println!("  Connections: {}", server.max_connections.unwrap_or(0));
    println!(
        "  Warning threshold: {:.0}%",
        server.warning_threshold * 100.0
    );

    println!("♾️  Unlimited preset:");
    println!(
        "  Memory: {}",
        if unlimited.max_memory_mb.is_none() {
            "Unlimited"
        } else {
            "Limited"
        }
    );
    println!(
        "  Bandwidth: {}",
        if unlimited.max_bandwidth_kbps.is_none() {
            "Unlimited"
        } else {
            "Limited"
        }
    );
    println!(
        "  Connections: {}",
        if unlimited.max_connections.is_none() {
            "Unlimited"
        } else {
            "Limited"
        }
    );

    Ok(())
}

async fn demo_resource_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Demo 2: Resource Monitoring");
    println!("------------------------------");

    // Create a resource manager with mobile preset
    let (mut manager, _warning_rx) = ResourceManager::mobile();

    println!("🚀 Starting resource monitoring...");

    // Start monitoring
    manager.start_monitoring().await?;
    manager.start_auto_cleanup().await?;

    let current_usage = manager.current_usage();
    println!("📈 Current resource usage:");
    println!("  Memory: {} MB", current_usage.memory_mb);
    println!("  Bandwidth: {} kbps", current_usage.bandwidth_kbps);
    println!("  Connections: {}", current_usage.active_connections);
    println!("  CPU: {:.1}%", current_usage.cpu_usage_percent);

    // Check if within limits
    match manager.check_limits() {
        Ok(()) => println!("✅ All resources within limits"),
        Err(e) => println!("❌ Resource limit exceeded: {}", e),
    }

    // Check for warnings
    let warnings = manager.approaching_limits();
    if warnings.is_empty() {
        println!("🟢 No resource warnings");
    } else {
        println!("⚠️  Resource warnings: {}", warnings.len());
    }

    // Wait a bit for monitoring
    sleep(Duration::from_millis(100)).await;

    // Stop monitoring
    manager.stop_monitoring().await;
    println!("🛑 Stopped resource monitoring");

    Ok(())
}

async fn demo_resource_warnings() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n⚠️  Demo 3: Resource Warnings");
    println!("----------------------------");

    let (manager, _warning_rx) = ResourceManager::desktop();

    println!("🔥 Simulating high resource usage (using static simulation)...");

    // Create a simulated high usage scenario
    let simulated_usage = quicrtc_core::ResourceUsage {
        memory_mb: 180,          // 90% of desktop limit (200MB)
        bandwidth_kbps: 9000,    // 90% of desktop limit (10000 kbps)
        active_connections: 18,  // 90% of desktop limit (20)
        active_streams: 45,      // 90% of desktop limit (50)
        cached_objects: 450,     // 90% of desktop limit (500)
        cpu_usage_percent: 85.0, // High CPU usage
        measured_at: std::time::Instant::now(),
    };

    // Check what warnings this usage would generate
    let warnings =
        quicrtc_core::ResourceManager::check_for_warnings(&simulated_usage, manager.limits());
    println!("📢 Generated {} warnings:", warnings.len());

    for (i, warning) in warnings.iter().enumerate() {
        println!("  {}. Severity: {:?}", i + 1, warning.severity());
        println!("     Action: {}", warning.recommended_action());
    }

    Ok(())
}

async fn demo_connection_pool() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏊 Demo 4: Connection Pool Management");
    println!("------------------------------------");

    let (manager, _warning_rx) = ResourceManager::desktop();
    let pool = manager.connection_pool();

    // Get initial stats
    let stats = pool.read().await.get_pool_stats();
    println!("📊 Initial pool stats:");
    println!("  Active connections: {}", stats.active_connections);
    println!("  Idle connections: {}", stats.idle_connections);
    println!("  Total connections: {}", stats.total_connections);
    println!("  Efficiency: {:.1}%", stats.metrics.efficiency_percentage);

    println!("📈 Simulated pool usage (in real scenario, connections would be created):");
    println!("  Total created: 10");
    println!("  Reused: 7");
    println!("  Returned: 5");
    println!("  Cleaned up: 2");
    println!("  Efficiency: {:.1}%", (7.0 / 10.0) * 100.0);

    Ok(())
}

async fn demo_mobile_vs_desktop() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📱🆚🖥️  Demo 5: Mobile vs Desktop Optimization");
    println!("----------------------------------------------");

    // Create mobile and desktop managers
    let (mobile_manager, _mobile_warnings) = ResourceManager::mobile();
    let (desktop_manager, _desktop_warnings) = ResourceManager::desktop();

    println!("📱 Mobile configuration:");
    let mobile_limits = mobile_manager.limits();
    println!(
        "  Memory limit: {} MB",
        mobile_limits.max_memory_mb.unwrap()
    );
    println!(
        "  Bandwidth limit: {} kbps",
        mobile_limits.max_bandwidth_kbps.unwrap()
    );
    println!("  Cleanup timeout: {:?}", mobile_limits.cleanup_timeout);

    println!("🖥️  Desktop configuration:");
    let desktop_limits = desktop_manager.limits();
    println!(
        "  Memory limit: {} MB",
        desktop_limits.max_memory_mb.unwrap()
    );
    println!(
        "  Bandwidth limit: {} kbps",
        desktop_limits.max_bandwidth_kbps.unwrap()
    );
    println!("  Cleanup timeout: {:?}", desktop_limits.cleanup_timeout);

    // Compare efficiency
    let mobile_efficiency = calculate_efficiency_score(mobile_limits);
    let desktop_efficiency = calculate_efficiency_score(desktop_limits);

    println!("📊 Efficiency comparison:");
    println!("  Mobile score: {:.1}", mobile_efficiency);
    println!("  Desktop score: {:.1}", desktop_efficiency);

    if mobile_efficiency < desktop_efficiency {
        println!("  🏆 Mobile is more resource-efficient (lower is better)");
    } else {
        println!("  🏆 Desktop has higher resource capacity");
    }

    // Demonstrate cleanup differences
    println!("🧹 Cleanup timeout comparison:");
    println!(
        "  Mobile: {:?} (aggressive cleanup)",
        mobile_limits.cleanup_timeout
    );
    println!(
        "  Desktop: {:?} (relaxed cleanup)",
        desktop_limits.cleanup_timeout
    );

    Ok(())
}

fn calculate_efficiency_score(limits: &ResourceLimits) -> f64 {
    // Simple efficiency score based on resource limits
    // Lower score = more efficient (more restrictive limits)
    let memory_score = limits.max_memory_mb.unwrap_or(1000) as f64;
    let bandwidth_score = limits.max_bandwidth_kbps.unwrap_or(100000) as f64 / 1000.0;
    let connection_score = limits.max_connections.unwrap_or(100) as f64 * 10.0;

    (memory_score + bandwidth_score + connection_score) / 3.0
}

async fn demo_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n❌ Demo 6: Error Handling");
    println!("------------------------");

    // Create a manager with very low limits for testing
    let strict_limits = ResourceLimits {
        max_memory_mb: Some(10),       // Very low memory limit
        max_bandwidth_kbps: Some(100), // Very low bandwidth limit
        max_connections: Some(1),      // Only 1 connection allowed
        max_streams_per_connection: Some(2),
        max_cached_objects: Some(5),
        cleanup_timeout: Duration::from_secs(1),
        warning_threshold: 0.5, // Warn at 50%
    };

    let (manager, _warnings) = ResourceManager::new(strict_limits.clone());

    // Test simulated usage that exceeds limits
    let excessive_usage = quicrtc_core::ResourceUsage {
        memory_mb: 15,         // Exceed 10MB limit
        bandwidth_kbps: 50,    // Below limit
        active_connections: 1, // At limit
        active_streams: 2,
        cached_objects: 3,
        cpu_usage_percent: 25.0,
        measured_at: std::time::Instant::now(),
    };

    // Check what would happen with this usage
    let warnings =
        quicrtc_core::ResourceManager::check_for_warnings(&excessive_usage, manager.limits());
    if !warnings.is_empty() {
        println!(
            "⚠️  Would generate {} warnings for this usage",
            warnings.len()
        );
    }

    println!("🚫 Resource limits demonstration:");
    println!(
        "  Memory limit: {} MB",
        strict_limits.max_memory_mb.unwrap()
    );
    println!(
        "  Bandwidth limit: {} kbps",
        strict_limits.max_bandwidth_kbps.unwrap()
    );
    println!(
        "  Connection limit: {}",
        strict_limits.max_connections.unwrap()
    );

    Ok(())
}
