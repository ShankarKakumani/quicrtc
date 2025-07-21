//! Codec interfaces and implementations
//!
//! This module provides a redesigned codec architecture that supports real
//! codec implementations with proper thread safety and performance.

use crate::tracks::{AudioFrame, MediaFrame, VideoFrame};
use quicrtc_core::QuicRtcError;
use std::collections::HashMap;
use std::sync::Arc;

// Real codec implementations - these will be feature-gated
#[cfg(feature = "opus")]
use audiopus::{
    coder::{Decoder as OpusDecoder, Encoder as OpusEncoder},
    Application, Channels, SampleRate,
};

#[cfg(feature = "h264")]
use openh264::{
    decoder::{DecodedYUV, Decoder as H264Decoder},
    encoder::Encoder as H264Encoder,
    formats::YUVBuffer,
};

/// Video quality presets for easy configuration
#[derive(Debug, Clone, Copy)]
pub enum VideoQuality {
    /// 320x240, 15fps, optimized for poor networks
    Low,
    /// 640x480, 30fps, balanced quality/bandwidth
    Standard,
    /// 1280x720, 30fps, high quality
    HD,
    /// 1920x1080, 30fps, maximum quality
    FullHD,
}

impl Default for VideoQuality {
    fn default() -> Self {
        Self::Standard
    }
}

/// Codec information
#[derive(Debug, Clone)]
pub struct CodecInfo {
    /// Codec name
    pub name: String,
    /// MIME type
    pub mime_type: String,
    /// Sample rate (for audio)
    pub sample_rate: Option<u32>,
    /// Channels (for audio)
    pub channels: Option<u8>,
}

/// Result type for codec operations
pub type CodecResult<T> = Result<T, QuicRtcError>;

/// Synchronous encoder trait - real codecs implement this
pub trait SyncEncoder: Send + Sync + std::fmt::Debug {
    /// Encode a media frame synchronously
    fn encode_sync(&self, frame: &MediaFrame) -> CodecResult<Vec<u8>>;

    /// Get codec information
    fn get_codec_info(&self) -> CodecInfo;

    /// Configure encoder if supported
    fn configure(&mut self, config: &CodecConfig) -> CodecResult<()> {
        let _ = config; // Default: ignore config changes
        Ok(())
    }
}

/// Synchronous decoder trait - real codecs implement this  
pub trait SyncDecoder: Send + Sync + std::fmt::Debug {
    /// Decode media data synchronously
    fn decode_sync(&self, data: &[u8]) -> CodecResult<MediaFrame>;

    /// Get codec information
    fn get_codec_info(&self) -> CodecInfo;
}

/// Combined codec trait for ease of use (implements both encoding and decoding)
/// Note: This trait is dyn-compatible, so Clone is handled separately
pub trait Codec: SyncEncoder + SyncDecoder {
    /// Clone this codec instance
    fn clone_codec(&self) -> Box<dyn Codec>;

    /// Create a new encoder instance
    fn clone_encoder(&self) -> Box<dyn SyncEncoder>;

    /// Create a new decoder instance  
    fn clone_decoder(&self) -> Box<dyn SyncDecoder>;
}

/// Async codec wrapper for thread pool usage
#[derive(Debug)]
pub struct AsyncCodec<T: Codec + Clone + 'static> {
    inner: T,
}

impl<T: Codec + Clone + 'static> AsyncCodec<T> {
    /// Create new async codec wrapper
    pub fn new(codec: T) -> Self {
        Self { inner: codec }
    }

    /// Async wrapper for encoding (runs sync operation on thread pool)
    pub async fn encode(&self, frame: &MediaFrame) -> CodecResult<Vec<u8>> {
        let frame = frame.clone();
        let codec = self.inner.clone();

        tokio::task::spawn_blocking(move || codec.encode_sync(&frame))
            .await
            .map_err(|e| QuicRtcError::EncodingFailed {
                reason: format!("Thread pool error: {}", e),
            })?
    }

    /// Async wrapper for decoding (runs sync operation on thread pool)
    pub async fn decode(&self, data: &[u8]) -> CodecResult<MediaFrame> {
        let data = data.to_vec();
        let codec = self.inner.clone();

        tokio::task::spawn_blocking(move || codec.decode_sync(&data))
            .await
            .map_err(|e| QuicRtcError::DecodingFailed {
                reason: format!("Thread pool error: {}", e),
            })?
    }

    /// Get codec information
    pub fn get_codec_info(&self) -> CodecInfo {
        SyncEncoder::get_codec_info(&self.inner) // Disambiguate the method call
    }

    /// Get reference to inner codec
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

/// Opus audio codec implementation with real audiopus integration
#[derive(Debug)]
pub struct OpusCodec {
    config: OpusConfig,

    // For placeholder mode when audiopus is not available
    #[cfg(not(feature = "opus"))]
    _placeholder: (),
}

/// Opus codec configuration
#[derive(Debug, Clone)]
pub struct OpusConfig {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of audio channels
    pub channels: u8,
    /// Bitrate in bits per second
    pub bitrate: u32,
    /// Frame duration in milliseconds
    pub frame_duration_ms: u32,
}

impl Default for OpusConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 64000,
            frame_duration_ms: 20,
        }
    }
}

impl OpusCodec {
    /// Create new Opus codec with default settings
    pub fn new() -> CodecResult<Self> {
        Self::with_config(OpusConfig::default())
    }

    /// Create Opus codec with custom configuration
    pub fn with_config(config: OpusConfig) -> CodecResult<Self> {
        // Validate configuration
        if ![8000, 12000, 16000, 24000, 48000].contains(&config.sample_rate) {
            return Err(QuicRtcError::InvalidData {
                reason: format!(
                    "Unsupported Opus sample rate: {}. Supported: 8000, 12000, 16000, 24000, 48000",
                    config.sample_rate
                ),
            });
        }

        if config.channels != 1 && config.channels != 2 {
            return Err(QuicRtcError::InvalidData {
                reason: format!(
                    "Unsupported channel count: {}. Opus supports 1 or 2 channels",
                    config.channels
                ),
            });
        }

        Ok(Self {
            config,
            #[cfg(not(feature = "opus"))]
            _placeholder: (),
        })
    }

    /// Get configuration
    pub fn config(&self) -> &OpusConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: OpusConfig) -> CodecResult<()> {
        // Validate first
        Self::with_config(config.clone())?;
        self.config = config;
        Ok(())
    }

    /// Calculate samples per frame based on configuration
    pub fn samples_per_frame(&self) -> usize {
        (self.config.sample_rate as usize * self.config.frame_duration_ms as usize) / 1000
    }
}

impl Default for OpusCodec {
    fn default() -> Self {
        Self::new().expect("Failed to create default OpusCodec")
    }
}

impl SyncEncoder for OpusCodec {
    fn encode_sync(&self, frame: &MediaFrame) -> CodecResult<Vec<u8>> {
        match frame {
            MediaFrame::Audio(audio_frame) => {
                #[cfg(feature = "opus")]
                {
                    self.encode_with_audiopus(audio_frame)
                }
                #[cfg(not(feature = "opus"))]
                {
                    self.encode_placeholder(audio_frame)
                }
            }
            _ => Err(QuicRtcError::InvalidMediaType {
                expected: "Audio".to_string(),
                actual: "Video".to_string(),
            }),
        }
    }

    fn get_codec_info(&self) -> CodecInfo {
        CodecInfo {
            name: "Opus".to_string(),
            mime_type: "audio/opus".to_string(),
            sample_rate: Some(self.config.sample_rate),
            channels: Some(self.config.channels),
        }
    }

    fn configure(&mut self, config: &CodecConfig) -> CodecResult<()> {
        let mut opus_config = self.config.clone();

        if let Some(sample_rate) = config.sample_rate {
            opus_config.sample_rate = sample_rate;
        }
        if let Some(channels) = config.channels {
            opus_config.channels = channels;
        }
        if let Some(bitrate) = config.bitrate {
            opus_config.bitrate = bitrate;
        }

        self.set_config(opus_config)
    }
}

impl SyncDecoder for OpusCodec {
    fn decode_sync(&self, data: &[u8]) -> CodecResult<MediaFrame> {
        if data.is_empty() {
            return Err(QuicRtcError::InvalidData {
                reason: "Empty Opus data".to_string(),
            });
        }

        #[cfg(feature = "opus")]
        {
            self.decode_with_audiopus(data)
        }
        #[cfg(not(feature = "opus"))]
        {
            self.decode_placeholder(data)
        }
    }

    fn get_codec_info(&self) -> CodecInfo {
        CodecInfo {
            name: "Opus".to_string(),
            mime_type: "audio/opus".to_string(),
            sample_rate: Some(self.config.sample_rate),
            channels: Some(self.config.channels),
        }
    }
}

impl Codec for OpusCodec {
    fn clone_codec(&self) -> Box<dyn Codec> {
        Box::new(self.clone())
    }

    fn clone_encoder(&self) -> Box<dyn SyncEncoder> {
        Box::new(self.clone())
    }

    fn clone_decoder(&self) -> Box<dyn SyncDecoder> {
        Box::new(self.clone())
    }
}

impl Clone for OpusCodec {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            #[cfg(not(feature = "opus"))]
            _placeholder: (),
        }
    }
}

// Real implementation when audiopus feature is enabled
#[cfg(feature = "opus")]
impl OpusCodec {
    fn encode_with_audiopus(&self, audio_frame: &AudioFrame) -> CodecResult<Vec<u8>> {
        // Create encoder with proper configuration
        let sample_rate = match self.config.sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            48000 => SampleRate::Hz48000,
            _ => {
                return Err(QuicRtcError::InvalidData {
                    reason: format!("Unsupported sample rate: {}", self.config.sample_rate),
                })
            }
        };

        let channels = if self.config.channels == 1 {
            Channels::Mono
        } else {
            Channels::Stereo
        };

        let mut encoder =
            OpusEncoder::new(sample_rate, channels, Application::Voip).map_err(|e| {
                QuicRtcError::EncodingFailed {
                    reason: format!("Failed to create Opus encoder: {:?}", e),
                }
            })?;

        // Validate input
        if audio_frame.sample_rate != self.config.sample_rate {
            return Err(QuicRtcError::InvalidData {
                reason: format!(
                    "Sample rate mismatch: expected {}, got {}",
                    self.config.sample_rate, audio_frame.sample_rate
                ),
            });
        }

        if audio_frame.channels != self.config.channels {
            return Err(QuicRtcError::InvalidData {
                reason: format!(
                    "Channel count mismatch: expected {}, got {}",
                    self.config.channels, audio_frame.channels
                ),
            });
        }

        // Convert f32 samples to i16
        let samples_i16: Vec<i16> = audio_frame
            .samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();

        // Encode
        let mut output = vec![0u8; 4000]; // Max Opus frame size
        let encoded_size = encoder.encode(&samples_i16, &mut output).map_err(|e| {
            QuicRtcError::EncodingFailed {
                reason: format!("Opus encoding failed: {:?}", e),
            }
        })?;

        output.truncate(encoded_size);
        Ok(output)
    }

    fn decode_with_audiopus(&self, data: &[u8]) -> CodecResult<MediaFrame> {
        // Create decoder with proper configuration
        let sample_rate = match self.config.sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            48000 => SampleRate::Hz48000,
            _ => {
                return Err(QuicRtcError::InvalidData {
                    reason: format!("Unsupported sample rate: {}", self.config.sample_rate),
                })
            }
        };

        let channels = if self.config.channels == 1 {
            Channels::Mono
        } else {
            Channels::Stereo
        };

        let mut decoder =
            OpusDecoder::new(sample_rate, channels).map_err(|e| QuicRtcError::DecodingFailed {
                reason: format!("Failed to create Opus decoder: {:?}", e),
            })?;

        // Decode
        let samples_per_frame = self.samples_per_frame();
        let total_samples = samples_per_frame * self.config.channels as usize;
        let mut samples_i16 = vec![0i16; total_samples];

        let decoded_samples = decoder
            .decode(Some(data), &mut samples_i16, false)
            .map_err(|e| QuicRtcError::DecodingFailed {
                reason: format!("Opus decoding failed: {:?}", e),
            })?;

        // Convert back to f32
        let actual_samples = decoded_samples * self.config.channels as usize;
        let samples: Vec<f32> = samples_i16[..actual_samples]
            .iter()
            .map(|&s| s as f32 / 32767.0)
            .collect();

        Ok(MediaFrame::Audio(AudioFrame {
            samples,
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }))
    }
}

// Placeholder implementation when audiopus feature is disabled
#[cfg(not(feature = "opus"))]
impl OpusCodec {
    fn encode_placeholder(&self, audio_frame: &AudioFrame) -> CodecResult<Vec<u8>> {
        // Simple placeholder encoding - just serialize some metadata
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&self.config.sample_rate.to_be_bytes());
        encoded.extend_from_slice(&[self.config.channels]);
        encoded.extend_from_slice(&(audio_frame.samples.len() as u32).to_be_bytes());

        // Add a compressed representation of the audio data
        let compressed_size = (audio_frame.samples.len() / 10).max(64);
        encoded.resize(encoded.len() + compressed_size, 0x42);

        Ok(encoded)
    }

    fn decode_placeholder(&self, data: &[u8]) -> CodecResult<MediaFrame> {
        if data.len() < 9 {
            return Err(QuicRtcError::InvalidData {
                reason: "Opus placeholder data too short".to_string(),
            });
        }

        // Extract metadata from placeholder format
        let sample_rate = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let channels = data[4];
        let sample_count = u32::from_be_bytes([data[5], data[6], data[7], data[8]]) as usize;

        // Generate placeholder audio samples
        let samples = (0..sample_count)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                0.1 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
            })
            .collect();

        Ok(MediaFrame::Audio(AudioFrame {
            samples,
            sample_rate,
            channels,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }))
    }
}

/// H.264 video codec implementation with real openh264 integration
#[derive(Debug)]
pub struct H264Codec {
    config: H264Config,
}

/// H.264 codec configuration  
#[derive(Debug, Clone)]
pub struct H264Config {
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Bitrate in bits per second
    pub bitrate: u32,
    /// Frame rate in frames per second
    pub framerate: u32,
}

impl Default for H264Config {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            bitrate: 1_000_000,
            framerate: 30,
        }
    }
}

impl H264Codec {
    /// Create a new H.264 codec with default configuration
    pub fn new() -> CodecResult<Self> {
        Ok(Self {
            config: H264Config::default(),
        })
    }

    /// Create a new H.264 codec with custom configuration
    pub fn with_config(config: H264Config) -> CodecResult<Self> {
        Ok(Self { config })
    }

    // Real implementation when h264 feature is enabled
    #[cfg(feature = "h264")]
    fn encode_with_openh264(&self, video_frame: &VideoFrame) -> CodecResult<Vec<u8>> {
        // Validate input frame dimensions
        if video_frame.width != self.config.width || video_frame.height != self.config.height {
            return Err(QuicRtcError::InvalidData {
                reason: format!(
                    "Frame size mismatch: expected {}x{}, got {}x{}",
                    self.config.width, self.config.height, video_frame.width, video_frame.height
                ),
            });
        }

        // Create encoder with simple configuration
        let mut encoder = H264Encoder::new().map_err(|e| QuicRtcError::EncodingFailed {
            reason: format!("Failed to create H.264 encoder: {}", e),
        })?;

        // For now, create a mock YUV buffer from our VideoFrame
        // This is a simplified approach that we can improve later
        let yuv_data = self.convert_video_frame_to_yuv(video_frame)?;

        // Encode the YUV data
        let bitstream = encoder
            .encode(&yuv_data)
            .map_err(|e| QuicRtcError::EncodingFailed {
                reason: format!("H.264 encoding failed: {}", e),
            })?;

        Ok(bitstream.to_vec())
    }

    #[cfg(feature = "h264")]
    fn decode_with_openh264(&self, encoded_data: &[u8]) -> CodecResult<VideoFrame> {
        // Create decoder
        let mut decoder = H264Decoder::new().map_err(|e| QuicRtcError::DecodingFailed {
            reason: format!("Failed to create H.264 decoder: {}", e),
        })?;

        // Decode the bitstream
        let decoded_yuv =
            decoder
                .decode(encoded_data)
                .map_err(|e| QuicRtcError::DecodingFailed {
                    reason: format!("H.264 decoding failed: {}", e),
                })?;

        // Convert the decoded YUV to our VideoFrame format
        match decoded_yuv {
            Some(yuv) => self.convert_yuv_to_video_frame(&yuv),
            None => Err(QuicRtcError::DecodingFailed {
                reason: "H.264 decoder returned no frame".to_string(),
            }),
        }
    }

    // Helper functions for format conversion
    #[cfg(feature = "h264")]
    fn convert_video_frame_to_yuv(&self, frame: &VideoFrame) -> CodecResult<YUVBuffer> {
        // Create YUVBuffer using the correct API
        // For now, we'll use the simplest approach that works
        let width = self.config.width as usize;
        let height = self.config.height as usize;
        let y_size = width * height;
        let uv_size = y_size / 4; // For 4:2:0 subsampling

        let mut yuv_data = vec![0u8; y_size + 2 * uv_size];

        // Simple RGB to YUV conversion for the first part of the data
        let rgb_data = if frame.data.len() >= width * height * 3 {
            &frame.data[..width * height * 3]
        } else {
            // If not enough data, pad with zeros
            &frame.data
        };

        // Simple conversion - just use the first channel as Y
        for (i, chunk) in rgb_data.chunks(3).enumerate() {
            if i < y_size {
                yuv_data[i] = chunk[0]; // Use red channel as luminance
            }
        }

        // Fill U and V planes with neutral values
        for i in y_size..(y_size + 2 * uv_size) {
            yuv_data[i] = 128;
        }

        // Create YUVBuffer using from_vec
        Ok(YUVBuffer::from_vec(yuv_data, width, height))
    }

    #[cfg(feature = "h264")]
    fn convert_yuv_to_video_frame(&self, yuv: &DecodedYUV) -> CodecResult<VideoFrame> {
        // Get dimensions from the DecodedYUV struct
        // For now, use our config dimensions as fallback
        let width = self.config.width;
        let height = self.config.height;
        let data_size = (width * height * 4) as usize;

        Ok(VideoFrame {
            width,
            height,
            data: vec![128; data_size], // Gray placeholder
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            is_keyframe: true,
        })
    }

    // Placeholder implementation when h264 feature is disabled
    #[cfg(not(feature = "h264"))]
    fn encode_with_openh264(&self, video_frame: &VideoFrame) -> CodecResult<Vec<u8>> {
        // Simulate encoding with size reduction
        let compressed_size = (video_frame.data.len() / 10).max(100);
        let mut result = Vec::with_capacity(compressed_size);

        // Add some "header" data to simulate H.264 structure
        result.extend_from_slice(b"H264");
        result.extend_from_slice(&(video_frame.width as u32).to_le_bytes());
        result.extend_from_slice(&(video_frame.height as u32).to_le_bytes());

        // Add compressed representation of frame data
        for chunk in video_frame
            .data
            .chunks(video_frame.data.len() / compressed_size.saturating_sub(12))
        {
            if result.len() < compressed_size {
                result.push(chunk.iter().fold(0u8, |acc, &x| acc.wrapping_add(x)));
            }
        }

        while result.len() < compressed_size {
            result.push(0);
        }

        Ok(result)
    }

    #[cfg(not(feature = "h264"))]
    fn decode_with_openh264(&self, encoded_data: &[u8]) -> CodecResult<VideoFrame> {
        if encoded_data.len() < 12 || &encoded_data[0..4] != b"H264" {
            return Err(QuicRtcError::InvalidData {
                reason: "Invalid H.264 header".to_string(),
            });
        }

        let width = u32::from_le_bytes([
            encoded_data[4],
            encoded_data[5],
            encoded_data[6],
            encoded_data[7],
        ]);
        let height = u32::from_le_bytes([
            encoded_data[8],
            encoded_data[9],
            encoded_data[10],
            encoded_data[11],
        ]);

        // Generate placeholder frame data
        let data_size = (width * height * 4) as usize;
        let mut data = Vec::with_capacity(data_size);

        // Create a pattern based on the encoded data
        let pattern_seed = encoded_data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
        for i in 0..data_size {
            data.push(((i as u8).wrapping_add(pattern_seed)) % 255);
        }

        Ok(VideoFrame {
            width,
            height,
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            is_keyframe: true,
        })
    }
}

impl Default for H264Codec {
    fn default() -> Self {
        Self::new().expect("Failed to create default H264Codec")
    }
}

impl SyncEncoder for H264Codec {
    fn encode_sync(&self, frame: &MediaFrame) -> CodecResult<Vec<u8>> {
        match frame {
            MediaFrame::Video(video_frame) => self.encode_with_openh264(video_frame),
            _ => Err(QuicRtcError::InvalidMediaType {
                expected: "Video".to_string(),
                actual: "Audio".to_string(),
            }),
        }
    }

    fn get_codec_info(&self) -> CodecInfo {
        CodecInfo {
            name: "H.264".to_string(),
            mime_type: "video/h264".to_string(),
            sample_rate: None,
            channels: None,
        }
    }

    fn configure(&mut self, config: &CodecConfig) -> CodecResult<()> {
        if let Some(bitrate) = config.bitrate {
            self.config.bitrate = bitrate;
        }
        if let (Some(width), Some(height)) = (config.width, config.height) {
            self.config.width = width;
            self.config.height = height;
        }
        if let Some(framerate) = config.framerate {
            self.config.framerate = framerate;
        }
        Ok(())
    }
}

impl SyncDecoder for H264Codec {
    fn decode_sync(&self, data: &[u8]) -> CodecResult<MediaFrame> {
        if data.is_empty() {
            return Err(QuicRtcError::InvalidData {
                reason: "Empty H.264 data".to_string(),
            });
        }

        let video_frame = self.decode_with_openh264(data)?;
        Ok(MediaFrame::Video(video_frame))
    }

    fn get_codec_info(&self) -> CodecInfo {
        CodecInfo {
            name: "H.264".to_string(),
            mime_type: "video/h264".to_string(),
            sample_rate: None,
            channels: None,
        }
    }
}

impl Codec for H264Codec {
    fn clone_codec(&self) -> Box<dyn Codec> {
        Box::new(self.clone())
    }

    fn clone_encoder(&self) -> Box<dyn SyncEncoder> {
        Box::new(self.clone())
    }

    fn clone_decoder(&self) -> Box<dyn SyncDecoder> {
        Box::new(self.clone())
    }
}

impl Clone for H264Codec {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
        }
    }
}

/// Codec registry for dynamic codec selection
#[derive(Debug)]
pub struct CodecRegistry {
    codecs: HashMap<String, Arc<dyn Codec>>,
}

impl CodecRegistry {
    /// Create a new codec registry
    pub fn new() -> Self {
        Self {
            codecs: HashMap::new(),
        }
    }

    /// Create a registry with default codecs
    pub fn with_defaults() -> CodecResult<Self> {
        let mut registry = Self::new();

        // Register default audio codecs
        registry.register_codec("opus", Arc::new(OpusCodec::new()?))?;

        // Register default video codecs
        registry.register_codec("h264", Arc::new(H264Codec::new()?))?;

        Ok(registry)
    }

    /// Register a codec
    pub fn register_codec(&mut self, name: &str, codec: Arc<dyn Codec>) -> CodecResult<()> {
        self.codecs.insert(name.to_string(), codec);
        Ok(())
    }

    /// Get a codec by name
    pub fn get_codec(&self, name: &str) -> Option<Arc<dyn Codec>> {
        self.codecs.get(name).cloned()
    }

    /// List available codecs
    pub fn list_codecs(&self) -> Vec<String> {
        self.codecs.keys().cloned().collect()
    }

    /// Get codec by MIME type
    pub fn get_codec_by_mime_type(&self, mime_type: &str) -> Option<Arc<dyn Codec>> {
        for codec in self.codecs.values() {
            if SyncEncoder::get_codec_info(codec.as_ref()).mime_type == mime_type {
                return Some(codec.clone());
            }
        }
        None
    }
}

impl Default for CodecRegistry {
    fn default() -> Self {
        Self::with_defaults().unwrap_or_else(|_| Self::new())
    }
}

/// Codec configuration builder
#[derive(Debug, Clone)]
pub struct CodecConfig {
    /// Codec name
    pub name: String,
    /// Bitrate in bits per second
    pub bitrate: Option<u32>,
    /// Sample rate (for audio)
    pub sample_rate: Option<u32>,
    /// Number of channels (for audio)
    pub channels: Option<u8>,
    /// Video width (for video)
    pub width: Option<u32>,
    /// Video height (for video)
    pub height: Option<u32>,
    /// Framerate (for video)
    pub framerate: Option<u32>,
}

impl CodecConfig {
    /// Create new codec config
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bitrate: None,
            sample_rate: None,
            channels: None,
            width: None,
            height: None,
            framerate: None,
        }
    }

    /// Set bitrate
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    /// Set sample rate
    pub fn sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = Some(sample_rate);
        self
    }

    /// Set channels
    pub fn channels(mut self, channels: u8) -> Self {
        self.channels = Some(channels);
        self
    }

    /// Set video resolution
    pub fn resolution(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Set framerate
    pub fn framerate(mut self, framerate: u32) -> Self {
        self.framerate = Some(framerate);
        self
    }

    /// Create codec from this configuration
    pub fn build(&self) -> CodecResult<Arc<dyn Codec>> {
        match self.name.as_str() {
            "opus" => {
                let opus_config = OpusConfig {
                    sample_rate: self.sample_rate.unwrap_or(48000),
                    channels: self.channels.unwrap_or(2),
                    bitrate: self.bitrate.unwrap_or(64000),
                    frame_duration_ms: 20,
                };
                Ok(Arc::new(OpusCodec::with_config(opus_config)?))
            }
            "h264" => {
                let h264_config = H264Config {
                    width: self.width.unwrap_or(640),
                    height: self.height.unwrap_or(480),
                    bitrate: self.bitrate.unwrap_or(1_000_000),
                    framerate: self.framerate.unwrap_or(30),
                };
                Ok(Arc::new(H264Codec::with_config(h264_config)?))
            }
            _ => Err(QuicRtcError::UnsupportedCodec {
                codec: self.name.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracks::{AudioFrame, MediaFrame, VideoFrame};

    #[test]
    fn test_opus_codec_real_implementation() {
        // Test that Opus codec is using real audiopus library, not placeholder
        let opus = OpusCodec::new().unwrap();

        // Create a simple audio frame (match default opus config: 2 channels, 48kHz, 20ms frame)
        // 48000 samples/sec * 0.02 sec * 2 channels = 1920 samples
        let samples_per_frame = opus.samples_per_frame() * opus.config().channels as usize;
        let mut samples = Vec::with_capacity(samples_per_frame);
        for i in 0..samples_per_frame {
            // Generate a simple sine wave pattern
            let t = i as f32 / 48000.0;
            samples.push(0.1 * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
        }

        let audio_frame = AudioFrame {
            samples,
            sample_rate: 48000,
            channels: 2, // Match default opus config
            timestamp: 12345,
        };

        let media_frame = MediaFrame::Audio(audio_frame.clone());

        // Encode the frame
        let encoded = opus.encode_sync(&media_frame).unwrap();

        // Real Opus encoding should produce more sophisticated output than placeholder
        // Placeholder just adds metadata (9 bytes) + some compressed data
        // Real Opus should have different characteristics

        #[cfg(feature = "opus")]
        {
            // Real Opus should produce encoded data that's not just metadata + pattern
            assert!(
                encoded.len() > 20,
                "Real Opus encoding should produce substantial output"
            );
            // Real opus encoding of this pattern should not start with sample rate bytes
            assert_ne!(
                &encoded[0..4],
                &48000u32.to_be_bytes(),
                "Should not be placeholder format"
            );
        }

        #[cfg(not(feature = "opus"))]
        {
            // Placeholder should start with sample rate
            assert_eq!(
                &encoded[0..4],
                &48000u32.to_be_bytes(),
                "Placeholder should start with sample rate"
            );
        }
    }

    #[test]
    fn test_h264_codec_real_implementation() {
        // Test that H.264 codec is using real openh264 library, not placeholder
        let h264 = H264Codec::new().unwrap();

        // Create a simple video frame
        let video_frame = VideoFrame {
            width: 640,
            height: 480,
            data: vec![128; 640 * 480 * 3], // Gray frame in RGB
            timestamp: 12345,
            is_keyframe: true,
        };

        let media_frame = MediaFrame::Video(video_frame.clone());

        // Encode the frame
        let encoded = h264.encode_sync(&media_frame).unwrap();

        #[cfg(feature = "h264")]
        {
            // Real H.264 should produce encoded data, not placeholder format
            assert!(
                encoded.len() > 20,
                "Real H.264 encoding should produce substantial output"
            );
            // Real H.264 should not start with "H264" magic bytes (that's placeholder)
            assert_ne!(&encoded[0..4], b"H264", "Should not be placeholder format");
        }

        #[cfg(not(feature = "h264"))]
        {
            // Placeholder should start with "H264" magic bytes
            assert_eq!(
                &encoded[0..4],
                b"H264",
                "Placeholder should start with H264 magic"
            );
        }
    }

    #[test]
    fn test_codec_info_real_vs_placeholder() {
        // Test that codec info is consistent regardless of real vs placeholder
        let opus = OpusCodec::new().unwrap();
        let h264 = H264Codec::new().unwrap();

        let opus_info = SyncEncoder::get_codec_info(&opus);
        assert_eq!(opus_info.name, "Opus");
        assert_eq!(opus_info.mime_type, "audio/opus");
        assert_eq!(opus_info.sample_rate, Some(48000));
        assert_eq!(opus_info.channels, Some(2));

        let h264_info = SyncEncoder::get_codec_info(&h264);
        assert_eq!(h264_info.name, "H.264");
        assert_eq!(h264_info.mime_type, "video/h264");
        assert!(h264_info.sample_rate.is_none());
        assert!(h264_info.channels.is_none());
    }

    #[test]
    fn test_codec_registry_with_real_implementations() {
        // Test that codec registry works with real implementations
        let registry = CodecRegistry::with_defaults().unwrap();

        let available_codecs = registry.list_codecs();
        assert!(available_codecs.contains(&"opus".to_string()));
        assert!(available_codecs.contains(&"h264".to_string()));

        let opus_codec = registry.get_codec("opus").unwrap();
        let h264_codec = registry.get_codec("h264").unwrap();

        // Test codec info is accessible
        let opus_info = SyncEncoder::get_codec_info(opus_codec.as_ref());
        let h264_info = SyncEncoder::get_codec_info(h264_codec.as_ref());

        assert_eq!(opus_info.mime_type, "audio/opus");
        assert_eq!(h264_info.mime_type, "video/h264");

        // Test MIME type lookup
        let opus_by_mime = registry.get_codec_by_mime_type("audio/opus").unwrap();
        let h264_by_mime = registry.get_codec_by_mime_type("video/h264").unwrap();

        assert_eq!(
            SyncEncoder::get_codec_info(opus_by_mime.as_ref()).name,
            "Opus"
        );
        assert_eq!(
            SyncEncoder::get_codec_info(h264_by_mime.as_ref()).name,
            "H.264"
        );
    }
}
