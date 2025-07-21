//! Connection Fallback Demo
//! 
//! This example demonstrates the QUIC connection establishment with automatic
//! fallback to WebSocket and WebRTC transports when QUIC is not available.

use quicrtc_core::{TransportConnection, ConnectionConfig, TransportMode, NetworkPath};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 QUIC RTC Connection Fallback Demo");
    println!("=====================================");

    // Target endpoint (this will fail since no server is running)
    let endpoint = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    println!("📡 Target endpoint: {}", endpoint);

    // Demo 1: Default fallback behavior
    println!("\n🔄 Demo 1: Default Fallback Chain");
    println!("Attempting connection with default fallback: QUIC → WebSocket → WebRTC");
    
    let default_config = ConnectionConfig::default();
    match TransportConnection::establish_with_fallback(endpoint, default_config).await {
        Ok(connection) => {
            println!("✅ Connected using: {:?}", connection.current_transport_mode());
            println!("📊 Connection metrics: {:?}", connection.metrics());
        }
        Err(e) => {
            println!("❌ All transports failed: {}", e);
        }
    }

    // Demo 2: Mobile-optimized configuration
    println!("\n📱 Demo 2: Mobile-Optimized Configuration");
    println!("Using shorter timeouts and aggressive migration settings");
    
    let mobile_config = ConnectionConfig {
        timeout: Duration::from_secs(3),
        keep_alive: true,
        keep_alive_interval: Duration::from_secs(15),
        max_idle_timeout: Duration::from_secs(30),
        enable_migration: true,
        preferred_transports: Some(vec![
            TransportMode::QuicNative,
            TransportMode::QuicOverWebSocket,
        ]),
    };

    match TransportConnection::establish_with_fallback(endpoint, mobile_config).await {
        Ok(connection) => {
            println!("✅ Mobile connection established: {:?}", connection.current_transport_mode());
            
            // Demo connection migration
            println!("\n🔄 Testing Connection Migration");
            demo_connection_migration(connection).await;
        }
        Err(e) => {
            println!("❌ Mobile connection failed: {}", e);
        }
    }

    // Demo 3: Custom transport preferences
    println!("\n⚙️  Demo 3: Custom Transport Preferences");
    println!("Trying WebRTC first, then QUIC");
    
    let custom_config = ConnectionConfig {
        timeout: Duration::from_secs(2),
        preferred_transports: Some(vec![
            TransportMode::WebRtcCompat,
            TransportMode::QuicNative,
        ]),
        ..Default::default()
    };

    match TransportConnection::establish_with_fallback(endpoint, custom_config).await {
        Ok(connection) => {
            println!("✅ Custom connection established: {:?}", connection.current_transport_mode());
        }
        Err(e) => {
            println!("❌ Custom connection failed: {}", e);
        }
    }

    // Demo 4: Network path scenarios
    println!("\n🌐 Demo 4: Network Path Scenarios");
    demo_network_paths().await;

    println!("\n✨ Demo completed!");
    Ok(())
}

async fn demo_connection_migration(mut connection: TransportConnection) {
    println!("Current path: {:?}", connection.current_path());
    
    // Simulate network switching scenarios
    let network_paths = vec![
        // WiFi network
        NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
            remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            interface_name: Some("wlan0".to_string()),
            mtu: Some(1500),
        },
        // Cellular network
        NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)), 12346),
            remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            interface_name: Some("rmnet0".to_string()),
            mtu: Some(1400),
        },
        // Ethernet network
        NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 50)), 12347),
            remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            interface_name: Some("eth0".to_string()),
            mtu: Some(1500),
        },
    ];

    for (i, path) in network_paths.iter().enumerate() {
        println!("🔄 Attempting migration to path {}: {:?}", i + 1, path.interface_name);
        
        // Validate path first
        match connection.validate_path(path).await {
            Ok(true) => {
                println!("✅ Path validation successful");
                
                // Attempt migration
                match connection.migrate_to(path.clone()).await {
                    Ok(()) => {
                        println!("✅ Migration successful to {}", 
                                path.interface_name.as_ref().unwrap_or(&"unknown".to_string()));
                        println!("📊 Migration events: {}", connection.metrics().migration_events);
                    }
                    Err(e) => {
                        println!("❌ Migration failed: {}", e);
                    }
                }
            }
            Ok(false) => {
                println!("❌ Path validation failed");
            }
            Err(e) => {
                println!("❌ Path validation error: {}", e);
            }
        }
    }
}

async fn demo_network_paths() {
    let paths = vec![
        ("WiFi", NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345),
            remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            interface_name: Some("wlan0".to_string()),
            mtu: Some(1500),
        }),
        ("Cellular", NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)), 12346),
            remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            interface_name: Some("rmnet0".to_string()),
            mtu: Some(1400),
        }),
        ("Ethernet", NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 50)), 12347),
            remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            interface_name: Some("eth0".to_string()),
            mtu: Some(1500),
        }),
        ("Low MTU", NetworkPath {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 12348),
            remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            interface_name: Some("ppp0".to_string()),
            mtu: Some(576),
        }),
    ];

    for (name, path) in paths {
        println!("🌐 {} Network:", name);
        println!("   Local: {}", path.local_addr);
        println!("   Interface: {}", path.interface_name.as_ref().unwrap_or(&"unknown".to_string()));
        println!("   MTU: {}", path.mtu.unwrap_or(0));
        
        // Check for potential issues
        if path.mtu.is_some() && path.mtu.unwrap() < 1200 {
            println!("   ⚠️  Warning: Low MTU may cause performance issues");
        }
        
        if path.local_addr.ip().is_unspecified() {
            println!("   ❌ Error: Unspecified local address");
        } else {
            println!("   ✅ Path looks valid");
        }
        
        println!();
    }
}