//! Video Capture Demo
//!
//! This example demonstrates the cross-platform video capture capabilities
//! using real platform backends (AVFoundation, V4L2, DirectShow).

use quicrtc_media::{
    NewVideoCaptureConfig as VideoCaptureConfig, VideoCaptureManager, VideoPixelFormat,
    VideoResolution,
};
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🎥 QUIC RTC Video Capture Demo");
    println!("==============================");

    // Demo 1: Platform Detection and Device Enumeration
    println!("\n📹 Demo 1: Platform Video Capture Detection");
    demo_platform_detection().await?;

    // Demo 2: Device Enumeration
    println!("\n📋 Demo 2: Video Device Enumeration");
    demo_device_enumeration().await?;

    // Demo 3: Video Capture Configuration
    println!("\n⚙️  Demo 3: Video Capture Configuration");
    demo_video_capture_config().await?;

    // Demo 4: Real-time Video Capture (if devices available)
    println!("\n🔴 Demo 4: Real-time Video Capture Test");
    demo_real_time_capture().await?;

    println!("\n✨ Video capture demo completed!");
    Ok(())
}

async fn demo_platform_detection() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        println!("🍎 Platform: macOS - Using AVFoundation video capture");
        println!("   • Native camera access via AVFoundation");
        println!("   • Hardware-accelerated video processing");
        println!("   • Metal rendering support");
    }

    #[cfg(target_os = "linux")]
    {
        println!("🐧 Platform: Linux - Using V4L2 video capture");
        println!("   • Video4Linux2 device access");
        println!("   • USB camera support");
        println!("   • Multiple format support (YUYV, MJPEG)");
    }

    #[cfg(target_os = "windows")]
    {
        println!("🪟 Platform: Windows - Using DirectShow video capture");
        println!("   • DirectShow filter graph");
        println!("   • COM object management");
        println!("   • Multiple camera support");
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        println!("❓ Platform: Unknown - Using fallback mock capture");
        println!("   • Mock video capture for unsupported platforms");
        println!("   • Testing and development support");
    }

    Ok(())
}

async fn demo_device_enumeration() -> Result<(), Box<dyn std::error::Error>> {
    println!("Enumerating video capture devices...");

    match VideoCaptureManager::new() {
        Ok(manager) => {
            match manager.enumerate_devices() {
                Ok(devices) => {
                    if devices.is_empty() {
                        println!("⚠️  No video capture devices found");
                    } else {
                        println!("✅ Found {} video capture device(s):", devices.len());
                        for (index, device) in devices.iter().enumerate() {
                            println!("   {}. {} (ID: {})", index + 1, device.name, device.id);
                            println!("      Description: {}", device.description);

                            // Show supported resolutions
                            println!(
                                "      Supported resolutions: {:?}",
                                device.supported_resolutions
                            );
                            println!("      Supported formats: {:?}", device.supported_formats);
                            println!("      Max framerate: {} fps", device.max_framerate);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to enumerate devices: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create video capture manager: {}", e);
        }
    }

    Ok(())
}

async fn demo_video_capture_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("Demonstrating video capture configurations...");

    // Create different capture configurations
    let configs = vec![
        (
            "VGA 30fps",
            VideoCaptureConfig {
                resolution: VideoResolution {
                    width: 640,
                    height: 480,
                },
                framerate: 30.0,
                pixel_format: VideoPixelFormat::YUV420P,
                hardware_acceleration: true,
                buffer_size: 3,
                enable_processing: false,
            },
        ),
        (
            "HD 60fps",
            VideoCaptureConfig {
                resolution: VideoResolution {
                    width: 1280,
                    height: 720,
                },
                framerate: 60.0,
                pixel_format: VideoPixelFormat::YUV420P,
                hardware_acceleration: true,
                buffer_size: 3,
                enable_processing: true,
            },
        ),
        (
            "Full HD",
            VideoCaptureConfig {
                resolution: VideoResolution {
                    width: 1920,
                    height: 1080,
                },
                framerate: 30.0,
                pixel_format: VideoPixelFormat::MJPEG,
                hardware_acceleration: true,
                buffer_size: 3,
                enable_processing: true,
            },
        ),
    ];

    for (name, config) in configs {
        println!("📐 Configuration: {}", name);
        println!(
            "   Resolution: {}x{}",
            config.resolution.width, config.resolution.height
        );
        println!("   Framerate: {} fps", config.framerate);
        println!("   Format: {:?}", config.pixel_format);
        println!("   Processing enabled: {}", config.enable_processing);

        // Validate configuration
        match config.validate() {
            Ok(_) => println!("   ✅ Configuration valid"),
            Err(e) => println!("   ❌ Configuration invalid: {}", e),
        }
        println!();
    }

    Ok(())
}

async fn demo_real_time_capture() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing real-time video capture...");

    let mut manager = VideoCaptureManager::new()?;
    let devices = manager.enumerate_devices()?;

    if devices.is_empty() {
        println!("⚠️  No devices available for capture test");
        return Ok(());
    }

    let device = &devices[0];
    println!("📹 Using device: {}", device.name);

    // Create a basic capture configuration
    let config = VideoCaptureConfig {
        resolution: VideoResolution {
            width: 640,
            height: 480,
        },
        framerate: 30.0,
        pixel_format: VideoPixelFormat::YUV420P,
        hardware_acceleration: true,
        buffer_size: 3,
        enable_processing: false,
    };

    println!("⚙️  Starting capture with configuration:");
    println!(
        "   Resolution: {}x{}",
        config.resolution.width, config.resolution.height
    );
    println!("   Framerate: {} fps", config.framerate);

    // Start capture with timeout to prevent hanging
    println!("📡 Attempting to start capture...");
    let start_result = tokio::time::timeout(
        Duration::from_secs(10), // 10 second timeout
        manager.start_capture(&device.id, config),
    )
    .await;

    match start_result {
        Ok(Ok(_)) => {
            println!("✅ Video capture started successfully");

            // Monitor capture for a short time
            println!("📊 Monitoring capture for 3 seconds...");

            let start_time = std::time::Instant::now();
            let mut last_stats_time = start_time;
            let mut stats_count = 0;

            while start_time.elapsed() < Duration::from_secs(3) && stats_count < 3 {
                tokio::time::sleep(Duration::from_millis(1000)).await;

                // Get current statistics
                let stats = manager.get_stats();
                let elapsed = last_stats_time.elapsed();

                if elapsed >= Duration::from_secs(1) {
                    println!(
                        "📈 Stats - Frames: {}, FPS: {:.1}, Buffer: {:.1}%",
                        stats.frames_captured,
                        stats.current_framerate,
                        stats.buffer_utilization * 100.0
                    );
                    last_stats_time = std::time::Instant::now();
                    stats_count += 1;
                }

                // Check if still capturing
                if !manager.is_capturing() {
                    println!("⚠️  Capture stopped unexpectedly");
                    break;
                }
            }

            // Stop capture
            println!("🛑 Stopping capture...");
            let stop_result =
                tokio::time::timeout(Duration::from_secs(5), manager.stop_capture()).await;

            match stop_result {
                Ok(Ok(_)) => println!("✅ Video capture stopped successfully"),
                Ok(Err(e)) => println!("⚠️  Error stopping capture: {}", e),
                Err(_) => println!("⚠️  Timeout stopping capture"),
            }

            // Final statistics
            let final_stats = manager.get_stats();
            println!("\n📊 Final Statistics:");
            println!("   Total frames captured: {}", final_stats.frames_captured);
            println!("   Frames dropped: {}", final_stats.frames_dropped);
            println!(
                "   Average framerate: {:.2} fps",
                final_stats.average_framerate
            );
            println!(
                "   Total duration: {:.2}s",
                final_stats.duration.as_secs_f64()
            );
        }
        Ok(Err(e)) => {
            println!("❌ Failed to start video capture: {}", e);

            // Provide specific guidance for permission errors
            let error_str = e.to_string();
            if error_str.contains("permission denied")
                || error_str.contains("permission not determined")
                || error_str.contains("permission restricted")
            {
                println!();
                println!("🔒 Camera Permission Required:");
                println!("   1. Open System Preferences > Security & Privacy > Camera");
                println!(
                    "   2. Add your terminal app (e.g., Terminal.app, iTerm.app) to allowed apps"
                );
                println!("   3. Restart this demo");
                println!();
                println!("   Or run with demo mode: cargo run --example video_capture_demo --features demo-mode");
            } else {
                println!("   This might be due to camera hardware access or other device issues");
            }
        }
        Err(_) => {
            println!("⏰ Timeout starting video capture (>10 seconds)");
            println!("   This suggests the camera initialization is hanging");
            println!("   On macOS, this often indicates:");
            println!("   • Camera permission not granted");
            println!("   • Another app is using the camera");
            println!("   • Hardware access issues");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_platform_detection() {
        assert!(demo_platform_detection().await.is_ok());
    }

    #[tokio::test]
    async fn test_device_enumeration() {
        assert!(demo_device_enumeration().await.is_ok());
    }

    #[tokio::test]
    async fn test_video_capture_config() {
        assert!(demo_video_capture_config().await.is_ok());
    }
}
