//! WebRTC-based video capture for browsers (WASM)
//!
//! This module provides browser video capture using MediaDevices API
//! through WebAssembly bindings.

use super::PlatformCapture;
use crate::error::MediaError;

/// WebRTC-based video capture implementation
pub struct WebRTCCapture {
    // TODO: Add wasm-bindgen objects for MediaDevices, etc.
}

impl WebRTCCapture {
    pub fn new() -> Self {
        Self {
            // TODO: Initialize WebRTC/MediaDevices objects
        }
    }
}

impl PlatformCapture for WebRTCCapture {
    fn start_capture(&self) -> Result<(), MediaError> {
        // TODO: Implement WebRTC/MediaDevices capture start
        tracing::info!("Starting WebRTC capture (stub)");
        Ok(())
    }

    fn stop_capture(&self) -> Result<(), MediaError> {
        // TODO: Implement WebRTC capture stop
        tracing::info!("Stopping WebRTC capture (stub)");
        Ok(())
    }

    fn get_devices(&self) -> Result<Vec<String>, MediaError> {
        // TODO: Use MediaDevices.enumerateDevices()
        Ok(vec!["WebRTC Camera".to_string()])
    }
}
