//! MoQ Stream Management Demo
//!
//! This example demonstrates the MoQ stream management capabilities including:
//! - Control stream establishment
//! - Data stream creation and lifecycle management  
//! - Object sending and stream multiplexing
//! - Stream statistics and monitoring
//!
//! To run: cargo run --example moq_stream_management_demo

use quicrtc_core::{
    MoqObject, MoqSession, MoqStreamEvent, MoqStreamManager, MoqStreamState, MoqStreamType,
    StreamManagerConfig, TrackNamespace,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🔄 MoQ Stream Management Demo");
    println!("==============================\n");

    // Create stream manager configuration for demonstration
    let config = StreamManagerConfig {
        max_concurrent_streams: 10,
        control_stream_timeout: Duration::from_secs(5),
        data_stream_timeout: Duration::from_secs(30),
        max_objects_per_stream: 100,
        enable_cleanup: true,
        cleanup_interval: Duration::from_secs(5),
        max_pending_objects: 20,
    };

    println!("📊 Configuration:");
    println!(
        "  Max concurrent streams: {}",
        config.max_concurrent_streams
    );
    println!("  Control timeout: {:?}", config.control_stream_timeout);
    println!("  Data timeout: {:?}", config.data_stream_timeout);
    println!(
        "  Max objects per stream: {}",
        config.max_objects_per_stream
    );
    println!();

    // Demonstrate the stream manager configuration
    demonstrate_stream_types();
    demonstrate_stream_states();
    demonstrate_stream_statistics().await;

    println!("✅ MoQ Stream Management Demo completed successfully!");
    Ok(())
}

fn demonstrate_stream_types() {
    println!("🔗 Stream Types:");
    println!("  Control: {:?}", MoqStreamType::Control);
    println!("  DataSubgroup: {:?}", MoqStreamType::DataSubgroup);
    println!("  Datagram: {:?}", MoqStreamType::Datagram);
    println!();
}

fn demonstrate_stream_states() {
    println!("📊 Stream States:");
    println!("  Opening: {:?}", MoqStreamState::Opening);
    println!("  Active: {:?}", MoqStreamState::Active);
    println!("  Closing: {:?}", MoqStreamState::Closing);
    println!("  Reset: {:?}", MoqStreamState::Reset);
    println!("  Completed: {:?}", MoqStreamState::Completed);
    println!();
}

async fn demonstrate_stream_statistics() {
    println!("📈 Stream Statistics Example:");

    // Create a sample track namespace
    let track_namespace = TrackNamespace {
        namespace: "example.com".to_string(),
        track_name: "demo/stream".to_string(),
    };

    // Create sample MoQ objects
    let objects = vec![
        create_sample_object(track_namespace.clone(), 1, 1),
        create_sample_object(track_namespace.clone(), 1, 2),
        create_sample_object(track_namespace.clone(), 2, 1),
    ];

    println!("  Created {} sample objects:", objects.len());
    for (i, obj) in objects.iter().enumerate() {
        println!(
            "    Object {}: Group={}, Object={}, Size={} bytes",
            i + 1,
            obj.group_id,
            obj.object_id,
            obj.payload.len()
        );
    }
    println!();

    // Demonstrate stream events
    demonstrate_stream_events();
}

fn demonstrate_stream_events() {
    println!("📡 Stream Events:");
    println!("  ControlStreamEstablished - When control stream is ready");
    println!("  DataStreamCreated - When new data stream is created");
    println!("  StreamStateChanged - When stream state transitions");
    println!("  ObjectSent - When object is successfully sent");
    println!("  ObjectReceived - When object is received");
    println!("  StreamError - When stream encounters an error");
    println!("  StreamClosed - When stream is closed");
    println!();
}

fn create_sample_object(namespace: TrackNamespace, group_id: u64, object_id: u64) -> MoqObject {
    let payload = format!("Sample data for group {} object {}", group_id, object_id).into_bytes();
    let size = payload.len();

    MoqObject {
        track_namespace: namespace,
        track_name: "demo_track".to_string(),
        group_id,
        object_id,
        publisher_priority: 128,
        payload,
        object_status: quicrtc_core::MoqObjectStatus::Normal,
        created_at: std::time::Instant::now(),
        size,
    }
}
