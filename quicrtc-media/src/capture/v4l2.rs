//! V4L2-based video capture for Linux
//!
//! This module provides native Linux video capture using Video4Linux2 (V4L2)
//! with support for various USB cameras and capture devices.

use super::PlatformCapture;
use crate::error::MediaError;

/// V4L2-based video capture implementation
pub struct V4L2Capture {
    // TODO: Add V4L2 device handles, format info, etc.
}

impl V4L2Capture {
    pub fn new() -> Self {
        Self {
            // TODO: Initialize V4L2 objects
        }
    }
}

impl PlatformCapture for V4L2Capture {
    fn start_capture(&self) -> Result<(), MediaError> {
        // TODO: Implement V4L2 capture start
        tracing::info!("Starting V4L2 capture (stub)");
        Ok(())
    }

    fn stop_capture(&self) -> Result<(), MediaError> {
        // TODO: Implement V4L2 capture stop
        tracing::info!("Stopping V4L2 capture (stub)");
        Ok(())
    }

    fn get_devices(&self) -> Result<Vec<String>, MediaError> {
        // TODO: Scan /dev/video* devices
        Ok(vec!["V4L2 Camera".to_string()])
    }
}
