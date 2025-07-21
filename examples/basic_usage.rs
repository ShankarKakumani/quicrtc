//! Basic usage example for QUIC RTC
//!
//! This example demonstrates audio generation, Opus encoding/decoding, and audio rendering.

use quicrtc_media::codecs::{OpusCodec, OpusConfig, SyncDecoder, SyncEncoder};
use quicrtc_media::render::{AudioRenderConfig, AudioRenderer, CpalAudioRenderer};
use quicrtc_media::tracks::{AudioFrame, MediaFrame};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎵 Testing Audio Pipeline");
    test_audio_pipeline().await?;
    Ok(())
}

/// Test complete audio pipeline: generation -> encoding -> decoding -> rendering
async fn test_audio_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    // Configure Opus codec for 48kHz mono audio
    let opus_config = OpusConfig {
        sample_rate: 48000,
        channels: 1,
        bitrate: 64000,
        frame_duration_ms: 20,
    };
    let codec = OpusCodec::with_config(opus_config)?;

    // Configure audio renderer
    let render_config = AudioRenderConfig {
        sample_rate: 48000,
        channels: 1,
        bits_per_sample: 16,
        buffer_size: 960, // 20ms at 48kHz
        device_name: None,
        volume: 0.5, // Lower volume for testing
        enable_effects: false,
    };

    println!("🔊 Starting audio renderer...");
    let mut renderer = CpalAudioRenderer::new();
    let audio_sender = renderer.start(render_config)?;

    println!("🎶 Generating and processing audio for 3 seconds...");

    let start_time = std::time::Instant::now();
    let mut frame_count = 0;

    while start_time.elapsed() < Duration::from_secs(3) {
        // Generate a 20ms audio frame (960 samples at 48kHz)
        let samples_per_frame = 960;
        let mut samples = Vec::with_capacity(samples_per_frame);

        // Generate a 440Hz sine wave (A note)
        for i in 0..samples_per_frame {
            let t = (frame_count * samples_per_frame + i) as f32 / 48000.0;
            let sample = 0.3 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            samples.push(sample);
        }

        // Create audio frame
        let audio_frame = AudioFrame {
            samples,
            sample_rate: 48000,
            channels: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };

        let media_frame = MediaFrame::Audio(audio_frame);

        // Encode with Opus
        match codec.encode_sync(&media_frame) {
            Ok(encoded_data) => {
                println!(
                    "✅ Frame {}: Encoded {} samples into {} bytes",
                    frame_count + 1,
                    samples_per_frame,
                    encoded_data.len()
                );

                // Decode back from Opus
                match codec.decode_sync(&encoded_data) {
                    Ok(MediaFrame::Audio(decoded_frame)) => {
                        println!(
                            "✅ Frame {}: Decoded {} bytes into {} samples",
                            frame_count + 1,
                            encoded_data.len(),
                            decoded_frame.samples.len()
                        );

                        // Send to renderer
                        if let Err(e) = audio_sender.send(decoded_frame).await {
                            println!("⚠️  Failed to send to renderer: {}", e);
                            break;
                        }
                    }
                    Ok(_) => {
                        println!("❌ Decoded frame is not audio");
                    }
                    Err(e) => {
                        println!("❌ Decode error: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Encode error: {:?}", e);
            }
        }

        frame_count += 1;

        // Wait for next frame (20ms)
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    println!("🛑 Stopping audio renderer...");
    renderer.stop()?;

    println!("\n📊 Pipeline Summary:");
    println!("   Total frames processed: {}", frame_count);
    println!("   Duration: {:.1}s", start_time.elapsed().as_secs_f32());
    println!(
        "   Avg frame rate: {:.1} fps",
        frame_count as f32 / start_time.elapsed().as_secs_f32()
    );

    let stats = renderer.stats();
    println!("   Renderer stats:");
    println!("     - Frames rendered: {}", stats.frames_rendered);
    println!("     - Frames dropped: {}", stats.frames_dropped);
    println!("     - Buffer level: {:.1}%", stats.buffer_level * 100.0);

    Ok(())
}
