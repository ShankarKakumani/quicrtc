pub mod avfoundation;
pub mod directshow;
pub mod v4l2;
pub mod webrtc;

use crate::error::MediaError;
use std::sync::Arc;

/// Platform-specific capture backend trait
pub trait PlatformCapture: Send + Sync {
    fn start_capture(&self) -> Result<(), MediaError>;
    fn stop_capture(&self) -> Result<(), MediaError>;
    fn get_devices(&self) -> Result<Vec<String>, MediaError>;
}

/// Get the appropriate platform capture backend
pub fn get_platform_capture() -> Arc<dyn PlatformCapture> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(avfoundation::AVFoundationCapture::new())
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(v4l2::V4L2Capture::new())
    }
    #[cfg(target_os = "windows")]
    {
        Arc::new(directshow::DirectShowCapture::new())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Arc::new(webrtc::WebRTCCapture::new())
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows",
        target_arch = "wasm32"
    )))]
    {
        Arc::new(MockCapture::new())
    }
}

/// Mock capture backend for unsupported platforms
struct MockCapture;

impl MockCapture {
    fn new() -> Self {
        Self
    }
}

impl PlatformCapture for MockCapture {
    fn start_capture(&self) -> Result<(), MediaError> {
        Ok(())
    }

    fn stop_capture(&self) -> Result<(), MediaError> {
        Ok(())
    }

    fn get_devices(&self) -> Result<Vec<String>, MediaError> {
        Ok(vec!["Mock Camera".to_string()])
    }
}
