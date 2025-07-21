//! Basic usage example for QUIC RTC
//!
//! This example demonstrates real audio capture and rendering.

use quicrtc_media::capture::{AudioCapture, AudioCaptureConfig, CpalAudioCapture};
use quicrtc_media::codecs::{SyncDecoder, SyncEncoder};
use quicrtc_media::render::{AudioRenderConfig, AudioRenderer, CpalAudioRenderer};
use quicrtc_media::tracks::MediaFrame;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Testing Pipeline Debug");
    test_pipeline_debug().await?;
    Ok(())
}

/// Test pipeline with debug info to identify frame size issue
async fn test_pipeline_debug() -> Result<(), Box<dyn std::error::Error>> {
    let mut capture = CpalAudioCapture::new();

    // Configure for mono audio to match capture
    let config = quicrtc_media::codecs::OpusConfig {
        sample_rate: 48000,
        channels: 1,
        bitrate: 64000,
        frame_duration_ms: 20,
    };
    let codec = quicrtc_media::codecs::OpusCodec::with_config(config)?;

    let capture_config = AudioCaptureConfig {
        sample_rate: 48000,
        channels: 1,
        bits_per_sample: 16,
        buffer_size: 960, // Match Opus frame size (20ms at 48kHz)
        device_name: None,
        echo_cancellation: true,
        noise_suppression: true,
        auto_gain_control: true,
    };

    println!("🎤 Starting real audio capture...");
    let mut audio_receiver = capture.start(capture_config)?;

    println!("🔄 Analyzing audio frames for 2 seconds...");

    let start_time = std::time::Instant::now();
    let mut frame_count = 0;
    let mut total_samples = 0;
    let mut min_samples = usize::MAX;
    let mut max_samples = 0;

    while start_time.elapsed() < Duration::from_secs(2) {
        if let Ok(frame_result) =
            tokio::time::timeout(Duration::from_millis(100), audio_receiver.recv()).await
        {
            if let Some(audio_frame) = frame_result {
                frame_count += 1;
                let samples_count = audio_frame.samples.len();
                total_samples += samples_count;
                min_samples = min_samples.min(samples_count);
                max_samples = max_samples.max(samples_count);

                // Debug first few frames
                if frame_count <= 3 {
                    println!(
                        "   📊 Frame {}: {} samples, {} channels, {} Hz",
                        frame_count, samples_count, audio_frame.channels, audio_frame.sample_rate
                    );
                }

                // Try to encode only if frame size matches Opus expectation
                let expected_samples = 960; // 20ms at 48kHz mono
                if samples_count == expected_samples {
                    println!(
                        "   ✅ Frame {} has correct size, attempting encode...",
                        frame_count
                    );

                    let audio_frame_data = quicrtc_media::tracks::AudioFrame {
                        samples: audio_frame.samples,
                        sample_rate: audio_frame.sample_rate,
                        channels: audio_frame.channels,
                        timestamp: audio_frame.timestamp,
                    };
                    let media_frame = MediaFrame::Audio(audio_frame_data);

                    match codec.encode_sync(&media_frame) {
                        Ok(encoded_data) => {
                            println!(
                                "   🎉 Successfully encoded {} samples into {} bytes!",
                                expected_samples,
                                encoded_data.len()
                            );
                            break; // We found one that works!
                        }
                        Err(e) => {
                            println!("   ❌ Encode error even with correct size: {:?}", e);
                        }
                    }
                } else {
                    if frame_count <= 5 {
                        println!(
                            "   ⚠️  Frame {} size mismatch: got {}, expected {}",
                            frame_count, samples_count, expected_samples
                        );
                    }
                }

                if frame_count >= 20 {
                    break; // Don't run forever
                }
            }
        }
    }

    capture.stop()?;

    println!("\n📊 Frame Analysis Summary:");
    println!("   Total frames: {}", frame_count);
    println!("   Total samples: {}", total_samples);
    if frame_count > 0 {
        println!(
            "   Average samples per frame: {}",
            total_samples / frame_count
        );
        println!("   Min samples per frame: {}", min_samples);
        println!("   Max samples per frame: {}", max_samples);
        println!("   Expected samples per frame: 960 (20ms at 48kHz)");

        if min_samples == max_samples && min_samples == 960 {
            println!("   🎉 All frames have perfect size!");
        } else if min_samples != max_samples {
            println!("   ⚠️  Frame sizes vary - this is the problem!");
        } else {
            println!("   ⚠️  Consistent size but not 960 - check configuration!");
        }
    }

    Ok(())
}
