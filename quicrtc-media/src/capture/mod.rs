pub mod avfoundation;
pub mod directshow;
pub mod v4l2;
pub mod webrtc;

use crate::error::MediaError;

/// Simplified platform-specific capture backend trait
/// This trait is intentionally simple to avoid complex thread safety issues
pub trait PlatformCapture {
    fn start_capture(&self) -> Result<(), MediaError>;
    fn stop_capture(&self) -> Result<(), MediaError>;
    fn get_devices(&self) -> Result<Vec<String>, MediaError>;
}

/// Get the appropriate platform capture backend
pub fn get_platform_capture() -> Box<dyn PlatformCapture> {
    #[cfg(target_os = "macos")]
    {
        Box::new(avfoundation::AVFoundationCapture::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(v4l2::V4L2Capture::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(directshow::DirectShowCapture::new())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Box::new(webrtc::WebRTCCapture::new())
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows",
        target_arch = "wasm32"
    )))]
    {
        Box::new(MockCapture::new())
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
