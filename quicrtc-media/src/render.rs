//! Audio and video rendering functionality
//!
//! This module provides interfaces and implementations for rendering audio
//! to speakers and video to displays.

use crate::tracks::{AudioFrame, VideoFrame};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

// Real audio rendering dependencies
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};

/// Errors that can occur during rendering
#[derive(Error, Debug)]
pub enum RenderError {
    /// Device not found or not available
    #[error("Device not found: {device}")]
    DeviceNotFound {
        /// Device name that was not found
        device: String,
    },

    /// Permission denied to access device
    #[error("Permission denied to access device")]
    PermissionDenied,

    /// Device is already in use
    #[error("Device is busy: {device}")]
    DeviceBusy {
        /// Device name that is busy
        device: String,
    },

    /// Configuration not supported
    #[error("Configuration not supported: {reason}")]
    ConfigurationNotSupported {
        /// Reason why configuration is not supported
        reason: String,
    },

    /// Render stream error
    #[error("Render stream error: {reason}")]
    StreamError {
        /// Reason for the stream error
        reason: String,
    },

    /// Hardware error
    #[error("Hardware error: {reason}")]
    HardwareError {
        /// Reason for the hardware error
        reason: String,
    },

    /// Buffer underrun
    #[error("Buffer underrun")]
    BufferUnderrun,

    /// Video format not supported
    #[error("Video format not supported: {format}")]
    UnsupportedVideoFormat {
        /// Unsupported format
        format: String,
    },
}

/// Audio rendering configuration
#[derive(Debug, Clone)]
pub struct AudioRenderConfig {
    /// Sample rate in Hz (e.g., 48000, 44100)
    pub sample_rate: u32,

    /// Number of audio channels (1 = mono, 2 = stereo)
    pub channels: u8,

    /// Bits per sample (typically 16 or 24)
    pub bits_per_sample: u16,

    /// Buffer size in samples
    pub buffer_size: u32,

    /// Device name (None for default device)
    pub device_name: Option<String>,

    /// Master volume (0.0 to 1.0)
    pub volume: f32,

    /// Enable audio effects
    pub enable_effects: bool,
}

impl Default for AudioRenderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2, // Stereo for output
            bits_per_sample: 16,
            buffer_size: 1024,
            device_name: None,
            volume: 1.0,
            enable_effects: false,
        }
    }
}

/// Audio output device information
#[derive(Debug, Clone)]
pub struct AudioOutputDevice {
    /// Device identifier
    pub id: String,

    /// Human-readable device name
    pub name: String,

    /// Whether this is the default device
    pub is_default: bool,

    /// Supported sample rates
    pub supported_sample_rates: Vec<u32>,

    /// Supported channel counts
    pub supported_channels: Vec<u8>,

    /// Maximum volume level
    pub max_volume: f32,
}

/// Audio rendering statistics
#[derive(Debug, Clone)]
pub struct AudioRenderStats {
    /// Total frames rendered
    pub frames_rendered: u64,

    /// Frames dropped due to buffer underrun
    pub frames_dropped: u64,

    /// Current buffer level (0.0 to 1.0)
    pub buffer_level: f32,

    /// Current output level (0.0 to 1.0)
    pub output_level: f32,

    /// Whether audio is currently being rendered
    pub is_rendering: bool,

    /// Audio latency in milliseconds
    pub latency_ms: f32,
}

/// Video rendering configuration
#[derive(Debug, Clone)]
pub struct VideoRenderConfig {
    /// Display width in pixels (0 for auto-detect)
    pub width: u32,

    /// Display height in pixels (0 for auto-detect)
    pub height: u32,

    /// Target framerate for display
    pub framerate: u32,

    /// Video format expected for input
    pub format: String,

    /// Device name (None for default display)
    pub device_name: Option<String>,

    /// Whether to enable vsync
    pub vsync: bool,

    /// Scaling mode ("stretch", "letterbox", "crop")
    pub scaling_mode: String,

    /// Whether to enable hardware acceleration
    pub hardware_acceleration: bool,

    /// Display brightness (0.0 to 1.0)
    pub brightness: f32,

    /// Display contrast (0.0 to 1.0)
    pub contrast: f32,
}

impl Default for VideoRenderConfig {
    fn default() -> Self {
        Self {
            width: 0,  // Auto-detect
            height: 0, // Auto-detect
            framerate: 30,
            format: "YUV420".to_string(),
            device_name: None,
            vsync: true,
            scaling_mode: "letterbox".to_string(),
            hardware_acceleration: true,
            brightness: 0.5,
            contrast: 0.5,
        }
    }
}

/// Video output device information
#[derive(Debug, Clone)]
pub struct VideoOutputDevice {
    /// Device identifier
    pub id: String,

    /// Human-readable device name
    pub name: String,

    /// Whether this is the default display
    pub is_default: bool,

    /// Supported resolutions (width, height)
    pub supported_resolutions: Vec<(u32, u32)>,

    /// Supported refresh rates
    pub supported_refresh_rates: Vec<u32>,

    /// Supported video formats
    pub supported_formats: Vec<String>,

    /// Whether device supports hardware acceleration
    pub has_hardware_acceleration: bool,

    /// Maximum supported brightness level
    pub max_brightness: f32,
}

/// Video rendering statistics
#[derive(Debug, Clone)]
pub struct VideoRenderStats {
    /// Total frames rendered
    pub frames_rendered: u64,

    /// Frames dropped due to buffer issues or late delivery
    pub frames_dropped: u64,

    /// Current buffer level (0.0 to 1.0)
    pub buffer_level: f32,

    /// Current rendering framerate
    pub current_framerate: f32,

    /// Whether video is currently being rendered
    pub is_rendering: bool,

    /// Average frame processing time in milliseconds
    pub avg_frame_time_ms: f32,

    /// Display latency in milliseconds
    pub latency_ms: f32,
}

/// Video processing configuration for display enhancement
#[derive(Debug, Clone)]
pub struct VideoDisplayConfig {
    /// Enable frame interpolation for smooth playback
    pub frame_interpolation: bool,

    /// Enable deinterlacing for interlaced content
    pub deinterlacing: bool,

    /// Enable color correction
    pub color_correction: bool,

    /// Brightness adjustment (-1.0 to 1.0, 0.0 = no change)
    pub brightness_adjustment: f32,

    /// Contrast adjustment (-1.0 to 1.0, 0.0 = no change)
    pub contrast_adjustment: f32,

    /// Saturation adjustment (-1.0 to 1.0, 0.0 = no change)
    pub saturation_adjustment: f32,

    /// Gamma correction (0.5 to 2.0, 1.0 = no change)
    pub gamma_correction: f32,
}

impl Default for VideoDisplayConfig {
    fn default() -> Self {
        Self {
            frame_interpolation: false,
            deinterlacing: true,
            color_correction: false,
            brightness_adjustment: 0.0,
            contrast_adjustment: 0.0,
            saturation_adjustment: 0.0,
            gamma_correction: 1.0,
        }
    }
}

/// Trait for audio rendering implementations
pub trait AudioRenderer: Send + Sync {
    /// Start audio rendering with the given configuration
    fn start(&mut self, config: AudioRenderConfig)
        -> Result<mpsc::Sender<AudioFrame>, RenderError>;

    /// Stop audio rendering
    fn stop(&mut self) -> Result<(), RenderError>;

    /// Get current rendering statistics
    fn stats(&self) -> AudioRenderStats;

    /// List available audio output devices
    fn list_devices(&self) -> Result<Vec<AudioOutputDevice>, RenderError>;

    /// Check if currently rendering
    fn is_rendering(&self) -> bool;

    /// Set master volume (0.0 to 1.0)
    fn set_volume(&mut self, volume: f32) -> Result<(), RenderError>;

    /// Get current volume
    fn volume(&self) -> f32;
}

/// Trait for video rendering implementations
pub trait VideoRenderer: Send + Sync {
    /// Start video rendering with the given configuration
    fn start(&mut self, config: VideoRenderConfig)
        -> Result<mpsc::Sender<VideoFrame>, RenderError>;

    /// Stop video rendering
    fn stop(&mut self) -> Result<(), RenderError>;

    /// Get current rendering statistics
    fn stats(&self) -> VideoRenderStats;

    /// List available video output devices
    fn list_devices(&self) -> Result<Vec<VideoOutputDevice>, RenderError>;

    /// Check if currently rendering
    fn is_rendering(&self) -> bool;

    /// Set display configuration
    fn set_display_config(&mut self, config: VideoDisplayConfig) -> Result<(), RenderError>;

    /// Get current display configuration
    fn display_config(&self) -> &VideoDisplayConfig;
}

/// Audio buffer for managing playback timing
#[derive(Debug)]
struct AudioBuffer {
    samples: Vec<f32>,
    read_pos: usize,
    write_pos: usize,
    capacity: usize,
    underruns: u64,
}

impl AudioBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            samples: vec![0.0; capacity],
            read_pos: 0,
            write_pos: 0,
            capacity,
            underruns: 0,
        }
    }

    fn write(&mut self, data: &[f32]) -> usize {
        let available = self.available_write();
        let to_write = data.len().min(available);

        for i in 0..to_write {
            self.samples[self.write_pos] = data[i];
            self.write_pos = (self.write_pos + 1) % self.capacity;
        }

        to_write
    }

    fn read(&mut self, data: &mut [f32]) -> usize {
        let available = self.available_read();
        let to_read = data.len().min(available);

        if to_read < data.len() {
            self.underruns += 1;
            // Fill remaining with silence
            for i in to_read..data.len() {
                data[i] = 0.0;
            }
        }

        for i in 0..to_read {
            data[i] = self.samples[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }

        to_read
    }

    fn available_read(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - self.read_pos + self.write_pos
        }
    }

    fn available_write(&self) -> usize {
        self.capacity - self.available_read() - 1
    }

    fn level(&self) -> f32 {
        self.available_read() as f32 / self.capacity as f32
    }
}

/// Default audio renderer implementation
pub struct DefaultAudioRenderer {
    config: Option<AudioRenderConfig>,
    stats: AudioRenderStats,
    is_rendering: bool,
    volume: f32,
    buffer: Option<Arc<std::sync::Mutex<AudioBuffer>>>,
    _render_handle: Option<tokio::task::JoinHandle<()>>,
}

impl DefaultAudioRenderer {
    /// Create a new audio renderer instance
    pub fn new() -> Self {
        Self {
            config: None,
            stats: AudioRenderStats {
                frames_rendered: 0,
                frames_dropped: 0,
                buffer_level: 0.0,
                output_level: 0.0,
                is_rendering: false,
                latency_ms: 20.0, // Typical low-latency value
            },
            is_rendering: false,
            volume: 1.0,
            buffer: None,
            _render_handle: None,
        }
    }

    /// Apply volume and effects to audio samples
    fn process_audio(&self, samples: &mut [f32]) {
        // Apply master volume
        for sample in samples.iter_mut() {
            *sample *= self.volume;
            *sample = sample.clamp(-1.0, 1.0); // Prevent clipping
        }
    }

    /// Convert mono to stereo if needed
    fn convert_channels(
        &self,
        input: &[f32],
        output: &mut [f32],
        input_channels: u8,
        output_channels: u8,
    ) {
        match (input_channels, output_channels) {
            (1, 2) => {
                // Mono to stereo - duplicate samples
                for (i, &sample) in input.iter().enumerate() {
                    output[i * 2] = sample;
                    output[i * 2 + 1] = sample;
                }
            }
            (2, 1) => {
                // Stereo to mono - average channels
                for i in 0..input.len() / 2 {
                    output[i] = (input[i * 2] + input[i * 2 + 1]) * 0.5;
                }
            }
            _ => {
                // Same channel count - direct copy
                let len = input.len().min(output.len());
                output[..len].copy_from_slice(&input[..len]);
            }
        }
    }

    /// Simulate audio rendering (for testing and platforms without audio support)
    async fn simulate_render(
        &mut self,
        config: AudioRenderConfig,
        mut receiver: mpsc::Receiver<AudioFrame>,
        buffer: Arc<std::sync::Mutex<AudioBuffer>>,
    ) {
        let frame_duration = std::time::Duration::from_millis(20); // 20ms frames

        // Start playback task
        let playback_buffer = buffer.clone();
        let playback_config = config.clone();
        let _playback_handle = tokio::spawn(async move {
            let samples_per_frame = (playback_config.sample_rate as f32 * 0.02) as usize;
            let mut output_samples =
                vec![0.0f32; samples_per_frame * playback_config.channels as usize];

            loop {
                {
                    let mut buf = playback_buffer.lock().unwrap();
                    buf.read(&mut output_samples);
                }

                // Simulate audio output (in real implementation, this would go to audio hardware)
                tokio::time::sleep(frame_duration).await;
            }
        });

        // Process incoming frames
        while let Some(frame) = receiver.recv().await {
            let mut processed_samples = frame.samples.clone();
            self.process_audio(&mut processed_samples);

            // Convert channels if needed
            let output_samples = if frame.channels != config.channels {
                let output_len = if config.channels > frame.channels {
                    processed_samples.len() * config.channels as usize / frame.channels as usize
                } else {
                    processed_samples.len() * config.channels as usize / frame.channels as usize
                };

                let mut output = vec![0.0f32; output_len];
                self.convert_channels(
                    &processed_samples,
                    &mut output,
                    frame.channels,
                    config.channels,
                );
                output
            } else {
                processed_samples
            };

            // Write to buffer
            {
                let mut buf = buffer.lock().unwrap();
                let written = buf.write(&output_samples);
                if written < output_samples.len() {
                    self.stats.frames_dropped += 1;
                }
                self.stats.buffer_level = buf.level();
            }

            self.stats.frames_rendered += 1;

            // Calculate output level
            let rms = (output_samples.iter().map(|s| s * s).sum::<f32>()
                / output_samples.len() as f32)
                .sqrt();
            self.stats.output_level = rms;
        }
    }
}

impl AudioRenderer for DefaultAudioRenderer {
    fn start(
        &mut self,
        config: AudioRenderConfig,
    ) -> Result<mpsc::Sender<AudioFrame>, RenderError> {
        if self.is_rendering {
            return Err(RenderError::StreamError {
                reason: "Already rendering".to_string(),
            });
        }

        let (sender, receiver) = mpsc::channel(32);

        // Create audio buffer
        let buffer_size = config.buffer_size as usize * config.channels as usize * 4; // 4x buffer for safety
        let buffer = Arc::new(std::sync::Mutex::new(AudioBuffer::new(buffer_size)));

        self.config = Some(config.clone());
        self.is_rendering = true;
        self.stats.is_rendering = true;
        self.buffer = Some(buffer.clone());

        // Start render task
        let mut render_instance = DefaultAudioRenderer::new();
        render_instance.is_rendering = true;
        render_instance.volume = self.volume;

        let handle = tokio::spawn(async move {
            render_instance
                .simulate_render(config, receiver, buffer)
                .await;
        });

        self._render_handle = Some(handle);

        Ok(sender)
    }

    fn stop(&mut self) -> Result<(), RenderError> {
        self.is_rendering = false;
        self.stats.is_rendering = false;

        if let Some(handle) = self._render_handle.take() {
            handle.abort();
        }

        self.buffer = None;

        Ok(())
    }

    fn stats(&self) -> AudioRenderStats {
        let mut stats = self.stats.clone();

        // Update buffer level if we have a buffer
        if let Some(buffer) = &self.buffer {
            if let Ok(buf) = buffer.lock() {
                stats.buffer_level = buf.level();
            }
        }

        stats
    }

    fn list_devices(&self) -> Result<Vec<AudioOutputDevice>, RenderError> {
        // Return mock devices for now
        Ok(vec![
            AudioOutputDevice {
                id: "default".to_string(),
                name: "Default Audio Output".to_string(),
                is_default: true,
                supported_sample_rates: vec![44100, 48000],
                supported_channels: vec![1, 2],
                max_volume: 1.0,
            },
            AudioOutputDevice {
                id: "builtin_speakers".to_string(),
                name: "Built-in Speakers".to_string(),
                is_default: false,
                supported_sample_rates: vec![44100, 48000],
                supported_channels: vec![2],
                max_volume: 1.0,
            },
            AudioOutputDevice {
                id: "headphones".to_string(),
                name: "Headphones".to_string(),
                is_default: false,
                supported_sample_rates: vec![44100, 48000],
                supported_channels: vec![2],
                max_volume: 1.0,
            },
        ])
    }

    fn is_rendering(&self) -> bool {
        self.is_rendering
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), RenderError> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(RenderError::ConfigurationNotSupported {
                reason: "Volume must be between 0.0 and 1.0".to_string(),
            });
        }

        self.volume = volume;
        Ok(())
    }

    fn volume(&self) -> f32 {
        self.volume
    }
}

impl Default for DefaultAudioRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Real audio renderer implementation using CPAL
pub struct CpalAudioRenderer {
    is_rendering: Arc<AtomicBool>,
    stats: AudioRenderStats,
    volume: f32,
    // Audio buffer for storing incoming frames
    audio_buffer: Arc<std::sync::Mutex<VecDeque<AudioFrame>>>,
}

impl CpalAudioRenderer {
    /// Create a new CPAL audio renderer instance
    pub fn new() -> Self {
        Self {
            is_rendering: Arc::new(AtomicBool::new(false)),
            stats: AudioRenderStats {
                frames_rendered: 0,
                frames_dropped: 0,
                buffer_level: 0.0,
                output_level: 0.0,
                is_rendering: false,
                latency_ms: 20.0,
            },
            volume: 1.0,
            audio_buffer: Arc::new(std::sync::Mutex::new(VecDeque::new())),
        }
    }

    /// Apply volume and effects to audio samples
    fn process_audio(&self, samples: &mut [f32]) {
        // Apply master volume
        for sample in samples.iter_mut() {
            *sample *= self.volume;
            *sample = sample.clamp(-1.0, 1.0); // Prevent clipping
        }
    }

    /// Convert audio format between different channel counts and sample rates
    fn convert_audio_format(
        input: &[f32],
        input_channels: u8,
        input_sample_rate: u32,
        output_channels: u16,
        output_sample_rate: u32,
    ) -> Vec<f32> {
        let mut output = input.to_vec();

        // Handle channel conversion
        if input_channels != output_channels as u8 {
            output = match (input_channels, output_channels) {
                (1, 2) => {
                    // Mono to stereo - duplicate samples
                    input
                        .iter()
                        .flat_map(|&sample| vec![sample, sample])
                        .collect()
                }
                (2, 1) => {
                    // Stereo to mono - average channels
                    input
                        .chunks_exact(2)
                        .map(|chunk| (chunk[0] + chunk[1]) * 0.5)
                        .collect()
                }
                _ => output, // Same channel count or unsupported conversion
            };
        }

        // Handle sample rate conversion (simple nearest-neighbor for now)
        if input_sample_rate != output_sample_rate {
            let ratio = output_sample_rate as f32 / input_sample_rate as f32;
            let new_length = (output.len() as f32 * ratio) as usize;
            let mut resampled = Vec::with_capacity(new_length);

            for i in 0..new_length {
                let src_index = (i as f32 / ratio) as usize;
                if src_index < output.len() {
                    resampled.push(output[src_index]);
                } else {
                    resampled.push(0.0);
                }
            }
            output = resampled;
        }

        output
    }
}

impl AudioRenderer for CpalAudioRenderer {
    fn start(
        &mut self,
        config: AudioRenderConfig,
    ) -> Result<mpsc::Sender<AudioFrame>, RenderError> {
        if self.is_rendering.load(Ordering::Relaxed) {
            return Err(RenderError::StreamError {
                reason: "Already rendering".to_string(),
            });
        }

        // Get the default host
        let host = cpal::default_host();

        // Get the default output device
        let device = if let Some(device_name) = &config.device_name {
            host.output_devices()
                .map_err(|e| RenderError::HardwareError {
                    reason: format!("Failed to enumerate devices: {}", e),
                })?
                .find(|d| d.name().unwrap_or_default() == *device_name)
                .ok_or_else(|| RenderError::DeviceNotFound {
                    device: device_name.clone(),
                })?
        } else {
            host.default_output_device()
                .ok_or_else(|| RenderError::DeviceNotFound {
                    device: "default output device".to_string(),
                })?
        };

        // Get the default output config
        let supported_config =
            device
                .default_output_config()
                .map_err(|e| RenderError::ConfigurationNotSupported {
                    reason: format!("Failed to get default output config: {}", e),
                })?;

        // Build the output stream config
        let stream_config = cpal::StreamConfig {
            channels: config.channels as cpal::ChannelCount,
            sample_rate: cpal::SampleRate(config.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(config.buffer_size),
        };

        let (sender, mut receiver) = mpsc::channel::<AudioFrame>(32);
        let is_rendering = self.is_rendering.clone();
        let audio_buffer = self.audio_buffer.clone();
        let volume = self.volume;

        // Start a task to receive frames and put them in the buffer
        let buffer_task = audio_buffer.clone();
        let task_is_rendering = is_rendering.clone();
        tokio::spawn(async move {
            while let Some(frame) = receiver.recv().await {
                if !task_is_rendering.load(Ordering::Relaxed) {
                    break;
                }

                {
                    let mut buffer = buffer_task.lock().unwrap();
                    buffer.push_back(frame);

                    // Keep buffer size reasonable
                    while buffer.len() > 10 {
                        buffer.pop_front();
                    }
                }
            }
        });

        // Create the output stream
        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::I16 => {
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        if !is_rendering.load(Ordering::Relaxed) {
                            // Fill with silence
                            data.fill(0);
                            return;
                        }

                        // Get data from buffer
                        let frame_data = {
                            let mut buffer = audio_buffer.lock().unwrap();
                            buffer.pop_front()
                        };

                        if let Some(mut frame) = frame_data {
                            // Convert format if needed
                            let mut processed_samples = Self::convert_audio_format(
                                &frame.samples,
                                frame.channels,
                                frame.sample_rate,
                                stream_config.channels,
                                stream_config.sample_rate.0,
                            );

                            // Apply volume
                            for sample in processed_samples.iter_mut() {
                                *sample *= volume;
                                *sample = sample.clamp(-1.0, 1.0);
                            }

                            // Convert to i16 and copy to output
                            let samples_to_copy = data.len().min(processed_samples.len());
                            for (i, &sample) in
                                processed_samples.iter().take(samples_to_copy).enumerate()
                            {
                                data[i] = (sample * i16::MAX as f32) as i16;
                            }

                            // Fill remaining with silence if needed
                            if samples_to_copy < data.len() {
                                data[samples_to_copy..].fill(0);
                            }
                        } else {
                            // No data available, fill with silence
                            data.fill(0);
                        }
                    },
                    move |err| {
                        eprintln!("Audio render stream error: {}", err);
                    },
                    None,
                )
            }
            cpal::SampleFormat::U16 => device.build_output_stream(
                &stream_config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    if !is_rendering.load(Ordering::Relaxed) {
                        data.fill(u16::MAX / 2);
                        return;
                    }

                    let frame_data = {
                        let mut buffer = audio_buffer.lock().unwrap();
                        buffer.pop_front()
                    };

                    if let Some(frame) = frame_data {
                        let mut processed_samples = Self::convert_audio_format(
                            &frame.samples,
                            frame.channels,
                            frame.sample_rate,
                            stream_config.channels,
                            stream_config.sample_rate.0,
                        );

                        for sample in processed_samples.iter_mut() {
                            *sample *= volume;
                            *sample = sample.clamp(-1.0, 1.0);
                        }

                        let samples_to_copy = data.len().min(processed_samples.len());
                        for (i, &sample) in
                            processed_samples.iter().take(samples_to_copy).enumerate()
                        {
                            data[i] = ((sample + 1.0) * (u16::MAX as f32 / 2.0)) as u16;
                        }

                        if samples_to_copy < data.len() {
                            data[samples_to_copy..].fill(u16::MAX / 2);
                        }
                    } else {
                        data.fill(u16::MAX / 2);
                    }
                },
                move |err| {
                    eprintln!("Audio render stream error: {}", err);
                },
                None,
            ),
            cpal::SampleFormat::F32 => device.build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !is_rendering.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        return;
                    }

                    let frame_data = {
                        let mut buffer = audio_buffer.lock().unwrap();
                        buffer.pop_front()
                    };

                    if let Some(frame) = frame_data {
                        let mut processed_samples = Self::convert_audio_format(
                            &frame.samples,
                            frame.channels,
                            frame.sample_rate,
                            stream_config.channels,
                            stream_config.sample_rate.0,
                        );

                        for sample in processed_samples.iter_mut() {
                            *sample *= volume;
                            *sample = sample.clamp(-1.0, 1.0);
                        }

                        let samples_to_copy = data.len().min(processed_samples.len());
                        data[..samples_to_copy]
                            .copy_from_slice(&processed_samples[..samples_to_copy]);

                        if samples_to_copy < data.len() {
                            data[samples_to_copy..].fill(0.0);
                        }
                    } else {
                        data.fill(0.0);
                    }
                },
                move |err| {
                    eprintln!("Audio render stream error: {}", err);
                },
                None,
            ),
            sample_format => {
                return Err(RenderError::ConfigurationNotSupported {
                    reason: format!("Unsupported sample format: {:?}", sample_format),
                });
            }
        }
        .map_err(|e| RenderError::StreamError {
            reason: format!("Failed to build output stream: {}", e),
        })?;

        // Start the stream
        stream.play().map_err(|e| RenderError::StreamError {
            reason: format!("Failed to start stream: {}", e),
        })?;

        self.is_rendering.store(true, Ordering::Relaxed);
        self.stats.is_rendering = true;

        // Keep stream alive
        std::mem::forget(stream);

        Ok(sender)
    }

    fn stop(&mut self) -> Result<(), RenderError> {
        self.is_rendering.store(false, Ordering::Relaxed);
        self.stats.is_rendering = false;

        // Clear audio buffer
        {
            let mut buffer = self.audio_buffer.lock().unwrap();
            buffer.clear();
        }

        Ok(())
    }

    fn stats(&self) -> AudioRenderStats {
        let mut stats = self.stats.clone();

        // Update buffer level
        {
            let buffer = self.audio_buffer.lock().unwrap();
            stats.buffer_level = buffer.len() as f32 / 10.0; // Max buffer size is 10
        }

        stats
    }

    fn list_devices(&self) -> Result<Vec<AudioOutputDevice>, RenderError> {
        let host = cpal::default_host();

        let devices = host
            .output_devices()
            .map_err(|e| RenderError::HardwareError {
                reason: format!("Failed to enumerate output devices: {}", e),
            })?;

        let default_device = host.default_output_device();
        let default_device_name = default_device
            .as_ref()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();

        let mut audio_devices = Vec::new();

        for device in devices {
            let name = device
                .name()
                .unwrap_or_else(|_| "Unknown Device".to_string());
            let is_default = name == default_device_name;

            // Get supported configurations
            let supported_configs =
                device
                    .supported_output_configs()
                    .map_err(|e| RenderError::HardwareError {
                        reason: format!("Failed to get supported configs for {}: {}", name, e),
                    })?;

            let mut supported_sample_rates = Vec::new();
            let mut supported_channels = Vec::new();

            for config in supported_configs {
                // Add sample rate range
                let min_rate = config.min_sample_rate().0;
                let max_rate = config.max_sample_rate().0;

                // Add common sample rates within the range
                for &rate in &[8000, 16000, 22050, 44100, 48000, 96000] {
                    if rate >= min_rate && rate <= max_rate {
                        if !supported_sample_rates.contains(&rate) {
                            supported_sample_rates.push(rate);
                        }
                    }
                }

                // Add channel count
                let channels = config.channels() as u8;
                if !supported_channels.contains(&channels) {
                    supported_channels.push(channels);
                }
            }

            supported_sample_rates.sort();
            supported_channels.sort();

            audio_devices.push(AudioOutputDevice {
                id: name.clone(),
                name,
                is_default,
                supported_sample_rates,
                supported_channels,
                max_volume: 1.0,
            });
        }

        Ok(audio_devices)
    }

    fn is_rendering(&self) -> bool {
        self.is_rendering.load(Ordering::Relaxed)
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), RenderError> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(RenderError::ConfigurationNotSupported {
                reason: "Volume must be between 0.0 and 1.0".to_string(),
            });
        }

        self.volume = volume;
        Ok(())
    }

    fn volume(&self) -> f32 {
        self.volume
    }
}

impl Default for CpalAudioRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame buffer for managing video display timing
#[derive(Debug)]
struct VideoFrameBuffer {
    frames: std::collections::VecDeque<VideoFrame>,
    max_size: usize,
    dropped_frames: u64,
}

impl VideoFrameBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            frames: std::collections::VecDeque::new(),
            max_size,
            dropped_frames: 0,
        }
    }

    fn push(&mut self, frame: VideoFrame) -> bool {
        if self.frames.len() >= self.max_size {
            // Drop oldest frame
            self.frames.pop_front();
            self.dropped_frames += 1;
        }
        self.frames.push_back(frame);
        true
    }

    fn pop(&mut self) -> Option<VideoFrame> {
        self.frames.pop_front()
    }

    fn level(&self) -> f32 {
        self.frames.len() as f32 / self.max_size as f32
    }

    fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Default video renderer implementation
pub struct DefaultVideoRenderer {
    config: Option<VideoRenderConfig>,
    display_config: VideoDisplayConfig,
    stats: VideoRenderStats,
    is_rendering: bool,
    buffer: Option<Arc<std::sync::Mutex<VideoFrameBuffer>>>,
    _render_handle: Option<tokio::task::JoinHandle<()>>,
}

impl DefaultVideoRenderer {
    /// Create a new video renderer instance
    pub fn new() -> Self {
        Self {
            config: None,
            display_config: VideoDisplayConfig::default(),
            stats: VideoRenderStats {
                frames_rendered: 0,
                frames_dropped: 0,
                buffer_level: 0.0,
                current_framerate: 0.0,
                is_rendering: false,
                avg_frame_time_ms: 16.67, // ~60 FPS
                latency_ms: 16.67,        // 1 frame at 60 FPS
            },
            is_rendering: false,
            buffer: None,
            _render_handle: None,
        }
    }

    /// Apply display processing to video frames
    fn process_video_frame(&self, frame: &mut VideoFrame) {
        if self.display_config.brightness_adjustment != 0.0 {
            self.apply_brightness_adjustment(frame);
        }

        if self.display_config.contrast_adjustment != 0.0 {
            self.apply_contrast_adjustment(frame);
        }

        if self.display_config.saturation_adjustment != 0.0 {
            self.apply_saturation_adjustment(frame);
        }

        if self.display_config.gamma_correction != 1.0 {
            self.apply_gamma_correction(frame);
        }
    }

    /// Apply brightness adjustment to video frame
    fn apply_brightness_adjustment(&self, frame: &mut VideoFrame) {
        let adjustment = self.display_config.brightness_adjustment;

        if frame.data.len() >= (frame.width * frame.height * 3 / 2) as usize {
            let y_size = (frame.width * frame.height) as usize;

            // Adjust Y channel (luminance) for brightness
            for i in 0..y_size {
                let original = frame.data[i] as f32;
                let adjusted = original + (adjustment * 128.0);
                frame.data[i] = adjusted.clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Apply contrast adjustment to video frame
    fn apply_contrast_adjustment(&self, frame: &mut VideoFrame) {
        let adjustment = self.display_config.contrast_adjustment;
        let contrast_factor = 1.0 + adjustment;

        if frame.data.len() >= (frame.width * frame.height * 3 / 2) as usize {
            let y_size = (frame.width * frame.height) as usize;

            // Adjust Y channel (luminance) for contrast
            for i in 0..y_size {
                let original = frame.data[i] as f32;
                // Apply contrast around middle gray (128)
                let adjusted = 128.0 + (original - 128.0) * contrast_factor;
                frame.data[i] = adjusted.clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Apply saturation adjustment to video frame
    fn apply_saturation_adjustment(&self, frame: &mut VideoFrame) {
        let adjustment = self.display_config.saturation_adjustment;
        let saturation_factor = 1.0 + adjustment;

        if frame.data.len() >= (frame.width * frame.height * 3 / 2) as usize {
            let width = frame.width as usize;
            let height = frame.height as usize;
            let y_size = width * height;
            let uv_size = y_size / 4;

            // Adjust U and V channels (chrominance) for saturation
            for i in 0..uv_size {
                let u_idx = y_size + i;
                let v_idx = y_size + uv_size + i;

                if u_idx < frame.data.len() && v_idx < frame.data.len() {
                    // Convert to signed values (U and V are typically 128-centered)
                    let u = (frame.data[u_idx] as i32 - 128) as f32;
                    let v = (frame.data[v_idx] as i32 - 128) as f32;

                    // Apply saturation
                    let u_adj = (u * saturation_factor).clamp(-128.0, 127.0);
                    let v_adj = (v * saturation_factor).clamp(-128.0, 127.0);

                    // Convert back to unsigned
                    frame.data[u_idx] = (u_adj + 128.0) as u8;
                    frame.data[v_idx] = (v_adj + 128.0) as u8;
                }
            }
        }
    }

    /// Apply gamma correction to video frame
    fn apply_gamma_correction(&self, frame: &mut VideoFrame) {
        let gamma = self.display_config.gamma_correction;

        if frame.data.len() >= (frame.width * frame.height * 3 / 2) as usize {
            let y_size = (frame.width * frame.height) as usize;

            // Apply gamma correction to Y channel (luminance)
            for i in 0..y_size {
                let normalized = frame.data[i] as f32 / 255.0;
                let corrected = normalized.powf(1.0 / gamma);
                frame.data[i] = (corrected * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Scale frame to target resolution with specified scaling mode
    fn scale_frame(
        &self,
        frame: &VideoFrame,
        target_width: u32,
        target_height: u32,
        scaling_mode: &str,
    ) -> VideoFrame {
        // For simplicity, this implementation just returns the original frame
        // A real implementation would perform actual scaling based on the scaling mode
        match scaling_mode {
            "stretch" => {
                // Would stretch the frame to exact target dimensions
                frame.clone()
            }
            "letterbox" => {
                // Would maintain aspect ratio with black bars if needed
                frame.clone()
            }
            "crop" => {
                // Would crop to fit target dimensions
                frame.clone()
            }
            _ => frame.clone(),
        }
    }

    /// Simulate video rendering (for testing and platforms without display support)
    async fn simulate_render(
        &mut self,
        config: VideoRenderConfig,
        mut receiver: mpsc::Receiver<VideoFrame>,
        buffer: Arc<std::sync::Mutex<VideoFrameBuffer>>,
    ) {
        let frame_duration = std::time::Duration::from_millis(1000 / config.framerate as u64);
        let start_time = std::time::Instant::now();
        let mut frame_count = 0u64;

        // Start display task
        let display_buffer = buffer.clone();
        let display_config = config.clone();
        let _display_handle = tokio::spawn(async move {
            loop {
                let frame_start = std::time::Instant::now();

                // Try to get a frame from buffer
                let frame_available = {
                    let mut buf = display_buffer.lock().unwrap();
                    buf.pop().is_some()
                };

                if frame_available {
                    // Simulate frame display processing time
                    // In a real implementation, this would render to display hardware
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                }

                // Wait for next display cycle
                let processing_time = frame_start.elapsed();
                if let Some(remaining) = frame_duration.checked_sub(processing_time) {
                    tokio::time::sleep(remaining).await;
                }
            }
        });

        // Process incoming frames
        while let Some(mut frame) = receiver.recv().await {
            let frame_start = std::time::Instant::now();

            // Apply display processing
            self.process_video_frame(&mut frame);

            // Scale frame if needed
            let processed_frame = if config.width > 0
                && config.height > 0
                && (frame.width != config.width || frame.height != config.height)
            {
                self.scale_frame(&frame, config.width, config.height, &config.scaling_mode)
            } else {
                frame
            };

            // Add to buffer
            let was_added = {
                let mut buf = buffer.lock().unwrap();
                buf.push(processed_frame)
            };

            if !was_added {
                self.stats.frames_dropped += 1;
            }

            self.stats.frames_rendered += 1;
            frame_count += 1;

            // Update statistics
            let elapsed = start_time.elapsed().as_secs_f32();
            if elapsed > 0.0 {
                self.stats.current_framerate = frame_count as f32 / elapsed;
            }

            let processing_time = frame_start.elapsed().as_millis() as f32;
            self.stats.avg_frame_time_ms = (self.stats.avg_frame_time_ms + processing_time) / 2.0;

            // Update buffer level
            {
                let buf = buffer.lock().unwrap();
                self.stats.buffer_level = buf.level();
            }
        }
    }
}

impl VideoRenderer for DefaultVideoRenderer {
    fn start(
        &mut self,
        config: VideoRenderConfig,
    ) -> Result<mpsc::Sender<VideoFrame>, RenderError> {
        if self.is_rendering {
            return Err(RenderError::StreamError {
                reason: "Already rendering".to_string(),
            });
        }

        // Validate configuration
        if config.framerate == 0 || config.framerate > 120 {
            return Err(RenderError::ConfigurationNotSupported {
                reason: "Invalid framerate".to_string(),
            });
        }

        let (sender, receiver) = mpsc::channel(32);

        // Create video frame buffer
        let buffer_size = (config.framerate * 2) as usize; // 2 seconds worth of frames
        let buffer = Arc::new(std::sync::Mutex::new(VideoFrameBuffer::new(buffer_size)));

        self.config = Some(config.clone());
        self.is_rendering = true;
        self.stats.is_rendering = true;
        self.buffer = Some(buffer.clone());

        // Start render task
        let mut render_instance = DefaultVideoRenderer::new();
        render_instance.is_rendering = true;
        render_instance.display_config = self.display_config.clone();

        let handle = tokio::spawn(async move {
            render_instance
                .simulate_render(config, receiver, buffer)
                .await;
        });

        self._render_handle = Some(handle);

        Ok(sender)
    }

    fn stop(&mut self) -> Result<(), RenderError> {
        self.is_rendering = false;
        self.stats.is_rendering = false;
        self.stats.current_framerate = 0.0;

        if let Some(handle) = self._render_handle.take() {
            handle.abort();
        }

        self.buffer = None;

        Ok(())
    }

    fn stats(&self) -> VideoRenderStats {
        let mut stats = self.stats.clone();

        // Update buffer level if we have a buffer
        if let Some(buffer) = &self.buffer {
            if let Ok(buf) = buffer.lock() {
                stats.buffer_level = buf.level();
            }
        }

        stats
    }

    fn list_devices(&self) -> Result<Vec<VideoOutputDevice>, RenderError> {
        // Return mock devices for now
        Ok(vec![
            VideoOutputDevice {
                id: "default".to_string(),
                name: "Default Display".to_string(),
                is_default: true,
                supported_resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
                supported_refresh_rates: vec![30, 60, 120],
                supported_formats: vec![
                    "YUV420".to_string(),
                    "RGB24".to_string(),
                    "NV12".to_string(),
                ],
                has_hardware_acceleration: true,
                max_brightness: 1.0,
            },
            VideoOutputDevice {
                id: "builtin_display".to_string(),
                name: "Built-in Display".to_string(),
                is_default: false,
                supported_resolutions: vec![(1920, 1080), (1366, 768), (1280, 720)],
                supported_refresh_rates: vec![60],
                supported_formats: vec!["YUV420".to_string(), "RGB24".to_string()],
                has_hardware_acceleration: true,
                max_brightness: 1.0,
            },
            VideoOutputDevice {
                id: "external_monitor".to_string(),
                name: "External Monitor".to_string(),
                is_default: false,
                supported_resolutions: vec![
                    (3840, 2160), // 4K
                    (2560, 1440), // 1440p
                    (1920, 1080), // 1080p
                    (1280, 720),  // 720p
                ],
                supported_refresh_rates: vec![30, 60, 120, 144],
                supported_formats: vec![
                    "YUV420".to_string(),
                    "YUV444".to_string(),
                    "RGB24".to_string(),
                    "RGB32".to_string(),
                ],
                has_hardware_acceleration: true,
                max_brightness: 1.0,
            },
        ])
    }

    fn is_rendering(&self) -> bool {
        self.is_rendering
    }

    fn set_display_config(&mut self, config: VideoDisplayConfig) -> Result<(), RenderError> {
        // Validate gamma correction range
        if config.gamma_correction < 0.5 || config.gamma_correction > 2.0 {
            return Err(RenderError::ConfigurationNotSupported {
                reason: "Gamma correction must be between 0.5 and 2.0".to_string(),
            });
        }

        // Validate adjustment ranges
        if config.brightness_adjustment < -1.0 || config.brightness_adjustment > 1.0 {
            return Err(RenderError::ConfigurationNotSupported {
                reason: "Brightness adjustment must be between -1.0 and 1.0".to_string(),
            });
        }

        if config.contrast_adjustment < -1.0 || config.contrast_adjustment > 1.0 {
            return Err(RenderError::ConfigurationNotSupported {
                reason: "Contrast adjustment must be between -1.0 and 1.0".to_string(),
            });
        }

        if config.saturation_adjustment < -1.0 || config.saturation_adjustment > 1.0 {
            return Err(RenderError::ConfigurationNotSupported {
                reason: "Saturation adjustment must be between -1.0 and 1.0".to_string(),
            });
        }

        self.display_config = config;
        Ok(())
    }

    fn display_config(&self) -> &VideoDisplayConfig {
        &self.display_config
    }
}

impl Default for DefaultVideoRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// Tests moved to tests/ directory
