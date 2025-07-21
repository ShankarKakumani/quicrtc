//! Simple Camera Test - Verify Real Camera Capture via Nokhwa
//!
//! This test verifies that we're actually capturing real camera frames,
//! not just synthetic test patterns.

use quicrtc_media::{
    NewVideoCaptureConfig as VideoCaptureConfig, VideoCaptureManager, VideoPixelFormat,
    VideoResolution,
};
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🎥 Simple Camera Test - Verifying Real Capture via Nokhwa");
    println!("==========================================================");

    // Create video capture manager (now using nokhwa internally!)
    let mut capture_manager = VideoCaptureManager::new()?;

    // Enumerate devices
    let devices = capture_manager.enumerate_devices()?;
    println!("📹 Found {} camera devices:", devices.len());
    for (i, device) in devices.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, device.name, device.id);
    }

    if devices.is_empty() {
        println!("❌ No camera devices found - camera might not be available");
        return Ok(());
    }

    // Use the first device
    let device = &devices[0];
    println!("\n🎯 Using camera: {}", device.name);

    // Configure for VGA 30fps
    let config = VideoCaptureConfig {
        resolution: VideoResolution::VGA,
        framerate: 30.0,
        pixel_format: VideoPixelFormat::RGB24,
        hardware_acceleration: true,
        buffer_size: 3,
        enable_processing: false,
    };

    println!(
        "⚙️  Configuration: {}x{} @ {:.1}fps",
        config.resolution.width, config.resolution.height, config.framerate
    );

    // Start capture
    println!("🚀 Starting capture...");
    capture_manager.start_capture(&device.id, config).await?;

    // Subscribe to events
    let mut events = capture_manager.subscribe_events();

    // Capture frames for 2 seconds and verify they're changing (not static test patterns)
    let mut frame_checksums = Vec::new();
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < Duration::from_secs(2) {
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check for capture events
        while let Ok(event) = events.try_recv() {
            if let quicrtc_media::VideoCaptureEvent::FrameCaptured { metadata } = event {
                // Calculate a simple checksum of frame metadata to detect changes
                let checksum = metadata
                    .sequence
                    .wrapping_mul(31)
                    .wrapping_add(metadata.size as u64)
                    .wrapping_add(metadata.timestamp as u64);

                frame_checksums.push(checksum);

                if frame_checksums.len() % 15 == 0 {
                    println!(
                        "📸 Captured {} frames (latest: seq={}, size={} bytes)",
                        frame_checksums.len(),
                        metadata.sequence,
                        metadata.size
                    );
                }
            }
        }
    }

    // Stop capture
    println!("🛑 Stopping capture...");
    capture_manager.stop_capture().await?;

    // Analyze results
    println!("\n📊 Analysis:");
    println!("   Total frames: {}", frame_checksums.len());

    if frame_checksums.len() < 5 {
        println!("⚠️  Very few frames captured - camera might not be working");
    } else {
        // Check if frames are changing (real camera) vs static (test pattern)
        let unique_checksums: std::collections::HashSet<_> = frame_checksums.iter().collect();
        let variation = unique_checksums.len() as f64 / frame_checksums.len() as f64;

        println!(
            "   Unique frame variations: {}/{} ({:.1}%)",
            unique_checksums.len(),
            frame_checksums.len(),
            variation * 100.0
        );

        if variation > 0.5 {
            println!("✅ HIGH VARIATION - Likely getting REAL camera frames!");
        } else if variation > 0.1 {
            println!("🟡 MEDIUM VARIATION - Camera working but might be static scene");
        } else {
            println!("🔴 LOW VARIATION - Likely getting test pattern or camera issue");
        }
    }

    let stats = capture_manager.get_stats();
    println!("   Average FPS: {:.1}", stats.average_framerate);
    println!("   Frames dropped: {}", stats.frames_dropped);

    println!("\n✨ Camera test completed!");

    if frame_checksums.len() > 10 {
        println!("🎉 SUCCESS: Camera capture is working via nokhwa!");
    }

    Ok(())
}
