//! RoomBuilder API Demo
//!
//! This example demonstrates the comprehensive RoomBuilder API implemented in Task 7.2.
//! It showcases the fluent API design with progressive configuration for:
//! - Participant and room configuration  
//! - Media settings (audio/video quality, processing options)
//! - Platform optimizations (mobile/desktop)
//! - Bandwidth and quality presets
//! - Advanced signaling and connection settings
//! - Configuration validation

use quicrtc::{
    AudioProcessingConfig, GlobalConfig, QuicRtc, ResourceLimits, VideoProcessingConfig,
    VideoQuality,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 RoomBuilder API Demo - Task 7.2 Implementation");
    println!("====================================================");

    // Initialize QuicRTC with custom global config
    let global_config = GlobalConfig {
        debug_logging: true,
        max_rooms: 5,
        resource_limits: ResourceLimits::desktop(),
        ..Default::default()
    };

    let quic_rtc = QuicRtc::init_with(global_config).await?;
    println!("✅ QuicRTC initialized with custom configuration");

    // ============================================================================
    // Demo 1: Basic Room Configuration
    // ============================================================================
    println!("\n📋 Demo 1: Basic Room Configuration");

    let basic_room = quic_rtc
        .room("demo-room-1")
        .participant("alice")
        .enable_video()
        .enable_audio()
        .validate();

    match basic_room {
        Ok(_) => println!("✅ Basic room configuration validated successfully"),
        Err(e) => println!("❌ Basic room validation failed: {}", e),
    }

    // ============================================================================
    // Demo 2: Advanced Media Configuration
    // ============================================================================
    println!("\n📋 Demo 2: Advanced Media Configuration");

    #[cfg(feature = "media")]
    {
        let media_room = quic_rtc
            .room("demo-room-2")
            .participant("bob")
            .enable_video()
            .video_quality(VideoQuality::HD)
            .video_resolution(1920, 1080, 30.0)
            .enable_audio()
            .audio_volume(0.8)
            .enable_echo_cancellation()
            .enable_noise_suppression()
            .validate();

        match media_room {
            Ok(_) => println!("✅ Advanced media configuration validated successfully"),
            Err(e) => println!("❌ Media configuration validation failed: {}", e),
        }
    }

    #[cfg(not(feature = "media"))]
    println!("⚠️ Media features disabled - skipping advanced media demo");

    // ============================================================================
    // Demo 3: Platform Optimization Presets
    // ============================================================================
    println!("\n📋 Demo 3: Platform Optimization Presets");

    // Mobile optimization
    let mobile_room = quic_rtc
        .room("mobile-room")
        .participant("mobile-user")
        .mobile_optimized()
        .low_bandwidth()
        .validate();

    match mobile_room {
        Ok(_) => println!("✅ Mobile optimization preset validated successfully"),
        Err(e) => println!("❌ Mobile optimization validation failed: {}", e),
    }

    // Desktop optimization
    let desktop_room = quic_rtc
        .room("desktop-room")
        .participant("desktop-user")
        .desktop_optimized()
        .high_quality()
        .validate();

    match desktop_room {
        Ok(_) => println!("✅ Desktop optimization preset validated successfully"),
        Err(e) => println!("❌ Desktop optimization validation failed: {}", e),
    }

    // ============================================================================
    // Demo 4: Bandwidth and Quality Control
    // ============================================================================
    println!("\n📋 Demo 4: Bandwidth and Quality Control");

    let bandwidth_room = quic_rtc
        .room("bandwidth-room")
        .participant("bandwidth-user")
        .bandwidth_limit(2000) // 2 Mbps
        .max_participants(50)
        .validate();

    match bandwidth_room {
        Ok(_) => println!("✅ Bandwidth configuration validated successfully"),
        Err(e) => println!("❌ Bandwidth configuration validation failed: {}", e),
    }

    // ============================================================================
    // Demo 5: Signaling and Connection Configuration
    // ============================================================================
    println!("\n📋 Demo 5: Signaling and Connection Configuration");

    #[cfg(feature = "signaling")]
    {
        let signaling_room = quic_rtc
            .room("signaling-room")
            .participant("signaling-user")
            .signaling_server("wss://signaling.example.com")
            .connection_timeout(Duration::from_secs(30))
            .validate();

        match signaling_room {
            Ok(_) => println!("✅ Signaling configuration validated successfully"),
            Err(e) => println!("❌ Signaling configuration validation failed: {}", e),
        }
    }

    #[cfg(not(feature = "signaling"))]
    println!("⚠️ Signaling features disabled - skipping signaling demo");

    // ============================================================================
    // Demo 6: Custom Resource Limits
    // ============================================================================
    println!("\n📋 Demo 6: Custom Resource Limits");

    let custom_limits = ResourceLimits {
        max_memory_mb: Some(512),
        max_bandwidth_kbps: Some(1500),
        max_connections: Some(10),
        max_streams_per_connection: Some(20),
        max_cached_objects: Some(1000),
        cleanup_timeout: Duration::from_secs(60),
        warning_threshold: 0.8,
    };

    let custom_room = quic_rtc
        .room("custom-room")
        .participant("custom-user")
        .resource_limits(custom_limits)
        .validate();

    match custom_room {
        Ok(_) => println!("✅ Custom resource limits validated successfully"),
        Err(e) => println!("❌ Custom resource limits validation failed: {}", e),
    }

    // ============================================================================
    // Demo 7: Advanced Processing Configuration
    // ============================================================================
    println!("\n📋 Demo 7: Advanced Processing Configuration");

    #[cfg(feature = "media")]
    {
        let audio_config = AudioProcessingConfig {
            enable_echo_cancellation: true,
            enable_noise_suppression: true,
            buffer_size: 960, // 20ms at 48kHz
            default_volume: 0.9,
        };

        let video_config = VideoProcessingConfig {
            enable_auto_exposure: true,
            enable_auto_white_balance: true,
            default_framerate: 60.0,
            enable_preprocessing: true,
        };

        let processing_room = quic_rtc
            .room("processing-room")
            .participant("processing-user")
            .audio_processing(audio_config)
            .video_processing(video_config)
            .validate();

        match processing_room {
            Ok(_) => println!("✅ Advanced processing configuration validated successfully"),
            Err(e) => println!("❌ Processing configuration validation failed: {}", e),
        }
    }

    #[cfg(not(feature = "media"))]
    println!("⚠️ Media features disabled - skipping processing demo");

    // ============================================================================
    // Demo 8: Validation Error Scenarios
    // ============================================================================
    println!("\n📋 Demo 8: Validation Error Scenarios");

    // Test missing participant ID
    let missing_participant = quic_rtc.room("error-room").enable_video().validate();

    match missing_participant {
        Ok(_) => println!("❌ Expected validation error for missing participant"),
        Err(e) => println!("✅ Correctly caught missing participant error: {}", e),
    }

    // Test invalid bandwidth
    let invalid_bandwidth = quic_rtc
        .room("error-room")
        .participant("error-user")
        .bandwidth_limit(32) // Too low
        .validate();

    match invalid_bandwidth {
        Ok(_) => println!("❌ Expected validation error for invalid bandwidth"),
        Err(e) => println!("✅ Correctly caught invalid bandwidth error: {}", e),
    }

    // Test invalid max participants
    let invalid_participants = quic_rtc
        .room("error-room")
        .participant("error-user")
        .max_participants(0) // Invalid
        .validate();

    match invalid_participants {
        Ok(_) => println!("❌ Expected validation error for invalid max participants"),
        Err(e) => println!("✅ Correctly caught invalid max participants error: {}", e),
    }

    // ============================================================================
    // Demo 9: Successful Room Join
    // ============================================================================
    println!("\n📋 Demo 9: Successful Room Join");

    let room = quic_rtc
        .room("success-room")
        .participant("success-user")
        .enable_video()
        .enable_audio()
        .desktop_optimized()
        .join()
        .await?;

    println!("✅ Successfully joined room: {}", room.id());
    println!("👤 Participant: {}", room.participant_id());
    println!("📊 Video enabled: {}", room.config().video_enabled);
    println!("🎵 Audio enabled: {}", room.config().audio_enabled);

    // ============================================================================
    // Demo 10: Quick Join Convenience Method
    // ============================================================================
    println!("\n📋 Demo 10: Quick Join Convenience Method");

    let quick_room = quicrtc::Room::quick_join("quick-room", "quick-user").await?;
    println!("✅ Quick join successful for room: {}", quick_room.id());

    println!("\n🎉 All RoomBuilder API demos completed successfully!");
    println!("The fluent API provides comprehensive configuration with validation");

    Ok(())
}
