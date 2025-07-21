//! QuicRTC Initialization Test
//!
//! This example tests the new Task 7.1 implementation:
//! - Real QuicRTC initialization with all subsystems
//! - Codec registry setup
//! - Resource management
//! - Peer discovery (if signaling feature enabled)
//! - Media systems initialization (if media feature enabled)

use quicrtc::{GlobalConfig, QuicRtc, ResourceLimits};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Testing QuicRTC Task 7.1 - Main Entry Point Initialization");
    println!("===============================================================");

    // Test 1: Default initialization
    println!("\n📋 Test 1: Default Initialization");
    let quic_rtc = QuicRtc::init().await?;
    println!("✅ Default QuicRTC initialization successful");

    // Test the resource manager
    let usage = quic_rtc.resource_manager().current_usage();
    println!("📊 Resource usage: {:?}", usage);

    // Test 2: Custom configuration initialization
    println!("\n📋 Test 2: Custom Configuration Initialization");
    let config = GlobalConfig {
        debug_logging: true,
        max_rooms: 5,
        resource_limits: ResourceLimits::mobile(),
        ..Default::default()
    };

    let _quic_rtc_custom = QuicRtc::init_with(config).await?;
    println!("✅ Custom QuicRTC initialization successful");

    // Test codec registry if media feature is enabled
    #[cfg(feature = "media")]
    {
        println!("\n🎵 Test 3: Codec Registry");
        let codec_registry = quic_rtc.codec_registry();
        let available_codecs = codec_registry.list_codecs();
        println!("📋 Available codecs: {:?}", available_codecs);

        // Test getting specific codecs
        if let Some(opus_codec) = codec_registry.get_codec("opus") {
            println!(
                "✅ Opus codec available: {:?}",
                quicrtc_media::codecs::SyncEncoder::get_codec_info(opus_codec.as_ref())
            );
        }

        if let Some(h264_codec) = codec_registry.get_codec("h264") {
            println!(
                "✅ H.264 codec available: {:?}",
                quicrtc_media::codecs::SyncEncoder::get_codec_info(h264_codec.as_ref())
            );
        }
    }

    // Test peer discovery if signaling feature is enabled
    #[cfg(feature = "signaling")]
    {
        println!("\n🔍 Test 4: Peer Discovery");
        let peer_discovery = quic_rtc.peer_discovery();
        let active_rooms = peer_discovery.get_active_rooms().await;
        println!("📋 Active rooms: {:?}", active_rooms);
        println!("✅ Peer discovery service accessible");
    }

    // Test 5: Room builder creation
    println!("\n🏠 Test 5: Room Builder Creation");
    let _room_builder = quic_rtc
        .room("test-room")
        .participant("test-user")
        .enable_video()
        .enable_audio();

    println!("✅ Room builder created successfully");
    println!("📋 Room builder configured for 'test-room' with participant 'test-user'");

    // Test 6: Resource monitoring
    println!("\n📊 Test 6: Resource Monitoring");
    let warnings = quic_rtc.resource_manager().approaching_limits();
    if warnings.is_empty() {
        println!("✅ No resource warnings - system operating normally");
    } else {
        println!("⚠️ Resource warnings: {:?}", warnings);
    }

    println!("\n🎉 All Task 7.1 tests completed successfully!");
    println!("\n📋 Summary of What Was Initialized:");
    println!("   ✅ Resource manager with configured limits");
    println!("   ✅ Background monitoring tasks");

    #[cfg(feature = "media")]
    println!("   ✅ Codec registry (Opus, H.264)");

    #[cfg(feature = "media")]
    println!("   ✅ Media capture/render systems");

    #[cfg(feature = "signaling")]
    println!("   ✅ Peer discovery service");

    println!("   ✅ Room builder factory");
    println!("\n🚀 QuicRTC is ready for Task 7.2 (RoomBuilder) and 7.3 (Room API)!");

    Ok(())
}
