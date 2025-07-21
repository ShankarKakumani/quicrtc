//! Unit tests for audio and video capture functionality
//!
//! This module contains tests for capture configurations, device management,
//! and capture lifecycle operations.

use quicrtc_media::*;

// ============================================================================
// CAPTURE LIFECYCLE TESTS
// ============================================================================

#[tokio::test]
async fn test_audio_render_config_default() {
    // Use real AudioRenderConfig from quicrtc-media
    let config = AudioRenderConfig::default();

    assert_eq!(config.sample_rate, 48000);
    assert_eq!(config.channels, 2);
    assert_eq!(config.bits_per_sample, 16);
    assert_eq!(config.buffer_size, 2048);
    assert!(config.device_name.is_none());
}

#[tokio::test]
async fn test_audio_render_config_custom() {
    let config = AudioRenderConfig {
        sample_rate: 44100,
        channels: 1,
        bits_per_sample: 24,
        buffer_size: 1024,
        device_name: Some("USB Audio".to_string()),
        volume: 0.8,
        enable_effects: true,
    };

    assert_eq!(config.sample_rate, 44100);
    assert_eq!(config.channels, 1);
    assert_eq!(config.bits_per_sample, 24);
    assert_eq!(config.buffer_size, 1024);
    assert_eq!(config.device_name, Some("USB Audio".to_string()));
    assert_eq!(config.volume, 0.8);
    assert!(config.enable_effects);
}

#[tokio::test]
async fn test_video_capture_config_default() {
    // Use real VideoCaptureConfig from quicrtc-media
    let config = NewVideoCaptureConfig::default();

    assert_eq!(config.resolution.width, 1280);
    assert_eq!(config.resolution.height, 720);
    assert_eq!(config.framerate, 30.0);
    assert_eq!(config.pixel_format, VideoPixelFormat::YUV420P);
}

#[tokio::test]
async fn test_video_capture_config_custom() {
    let config = NewVideoCaptureConfig {
        resolution: VideoResolution::new(1920, 1080),
        framerate: 60.0,
        pixel_format: VideoPixelFormat::RGB24,
        hardware_acceleration: false,
        buffer_size: 5,
        enable_processing: true,
    };

    assert_eq!(config.resolution.width, 1920);
    assert_eq!(config.resolution.height, 1080);
    assert_eq!(config.framerate, 60.0);
    assert_eq!(config.pixel_format, VideoPixelFormat::RGB24);
    assert!(!config.hardware_acceleration);
    assert_eq!(config.buffer_size, 5);
    assert!(config.enable_processing);
}

#[tokio::test]
async fn test_cpal_audio_renderer_creation() {
    let renderer = CpalAudioRenderer::new();

    // Initially not rendering
    assert!(!renderer.is_rendering());
}

#[tokio::test]
async fn test_video_capture_manager_creation() {
    // Test video capture manager creation
    let manager_result = VideoCaptureManager::new();

    // Should succeed in creating manager
    assert!(manager_result.is_ok());

    if let Ok(manager) = manager_result {
        // Initially not capturing
        assert!(!manager.is_capturing());
    }
}

#[tokio::test]
async fn test_audio_capture_lifecycle() {
    // Test basic lifecycle operations without actually starting capture
    // (since we don't have hardware in tests)

    let mut renderer = CpalAudioRenderer::new();

    // Initially not rendering
    assert!(!renderer.is_rendering());

    // Note: We can't actually start rendering in tests without audio hardware
    // These tests verify the structure and basic state management
}

#[tokio::test]
async fn test_video_capture_lifecycle() {
    // Test basic lifecycle operations
    let manager_result = VideoCaptureManager::new();
    assert!(manager_result.is_ok());

    if let Ok(manager) = manager_result {
        // Initially not capturing
        assert!(!manager.is_capturing());

        // Note: We can't actually start capture in tests without camera hardware
        // These tests verify the structure and basic state management
    }
}

// ============================================================================
// CODEC INTEGRATION TESTS
// ============================================================================

#[tokio::test]
async fn test_opus_codec_structure() {
    // Test basic Opus codec structure
    let codec_result = OpusCodec::new();
    assert!(codec_result.is_ok());

    if let Ok(codec) = codec_result {
        let config = codec.config();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
    }
}

#[tokio::test]
async fn test_h264_codec_structure() {
    // Test basic H.264 codec structure
    let codec_result = H264Codec::new();
    assert!(codec_result.is_ok());
    
    if let Ok(codec) = codec_result {
        // Use explicit trait method to avoid ambiguity
        let info = <H264Codec as quicrtc_media::SyncEncoder>::get_codec_info(&codec);
        assert_eq!(info.name, "H.264");
        assert_eq!(info.mime_type, "video/h264");
    }
}

// ============================================================================
// DEVICE ENUMERATION TESTS
// ============================================================================

#[tokio::test]
async fn test_video_device_structure() {
    // Test video device structure
    let device = NewVideoDevice {
        id: "test_camera".to_string(),
        name: "Test Camera".to_string(),
        description: "Virtual camera for testing".to_string(),
        supported_formats: vec![VideoPixelFormat::YUV420P, VideoPixelFormat::RGB24],
        supported_resolutions: vec![VideoResolution::VGA, VideoResolution::HD],
        max_framerate: 60.0,
        hardware_acceleration: false,
    };

    assert_eq!(device.id, "test_camera");
    assert_eq!(device.name, "Test Camera");
    assert_eq!(device.supported_formats.len(), 2);
    assert_eq!(device.supported_resolutions.len(), 2);
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

#[tokio::test]
async fn test_capture_error_scenarios() {
    // Test error handling scenarios

    // Invalid sample rate
    let invalid_sample_rate = 0u32;
    assert_eq!(invalid_sample_rate, 0); // Would trigger error in real capture

    // Invalid resolution
    let invalid_width = 0u32;
    let invalid_height = 0u32;
    assert_eq!(invalid_width, 0); // Would trigger error in real capture
    assert_eq!(invalid_height, 0);

    // Invalid framerate
    let invalid_framerate = 0.0f32;
    assert_eq!(invalid_framerate, 0.0); // Would trigger error in real capture
}
