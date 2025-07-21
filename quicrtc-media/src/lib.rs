//! # QUIC RTC Media
//!
//! Media processing, codec handling, and quality control for QUIC RTC.
//! This crate handles all media-specific functionality including encoding,
//! decoding, and MoQ object processing.

#![warn(clippy::all)]

pub mod capture;
pub mod codecs;
pub mod error;
pub mod processing;
pub mod render;
pub mod tracks;
pub mod video_capture;
pub mod video_render;

// Re-export main types
// Note: capture module exports temporarily disabled due to refactoring
// TODO: Re-enable once platform-specific implementations are complete
pub use codecs::{
    Codec, CodecConfig, CodecInfo, CodecRegistry, H264Codec, OpusCodec, SyncDecoder, SyncEncoder,
    VideoQuality,
};
pub use error::{ErrorCategory, MediaError, MediaResult};
pub use processing::{
    CongestionLevel, MediaProcessor, MoqDeliveryMetrics, MoqObjectAssembler, QualityControlConfig,
    QualityController, QualitySettings, TrackStats,
};
pub use render::{
    AudioOutputDevice, AudioRenderConfig, AudioRenderStats, AudioRenderer, CpalAudioRenderer,
    DefaultAudioRenderer, DefaultVideoRenderer, RenderError, VideoDisplayConfig, VideoOutputDevice,
    VideoRenderConfig, VideoRenderStats, VideoRenderer,
};
pub use tracks::{AudioFrame, AudioTrack, MediaFrame, VideoFrame, VideoTrack};
pub use video_capture::{
    CaptureStats, FrameMetadata, FrameProcessor, FrameProcessorConfig,
    VideoCaptureConfig as NewVideoCaptureConfig, VideoCaptureEvent, VideoCaptureManager,
    VideoDevice as NewVideoDevice, VideoPixelFormat, VideoResolution,
};
pub use video_render::{
    SoftwareRenderer, VideoDisplayMode, VideoRenderBackend,
    VideoRenderConfig as NewVideoRenderConfig, VideoRenderEvent, VideoRenderManager,
    VideoRenderStats as NewVideoRenderStats, VideoRenderer as NewVideoRenderer, VideoScalingMode,
};
