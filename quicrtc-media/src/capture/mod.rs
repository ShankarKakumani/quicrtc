//! Cross-platform video capture using nokhwa
//!
//! This module provides unified video capture across all platforms using the
//! battle-tested nokhwa crate. No more platform-specific code needed!
//!
//! Supported platforms (automatically via nokhwa):
//! - macOS: AVFoundation backend  
//! - Linux: V4L2 backend
//! - Windows: DirectShow/MediaFoundation backend
//! - Web/WASM: MediaDevices API backend

use crate::error::MediaError;

/// Cross-platform camera capture using nokhwa
/// This is the only capture backend we need - nokhwa handles all platforms!
pub struct NokhwaCapture {
    camera: parking_lot::Mutex<Option<nokhwa::Camera>>,
}

impl NokhwaCapture {
    pub fn new() -> Self {
        Self {
            camera: parking_lot::Mutex::new(None),
        }
    }

    /// Get available camera devices across all platforms
    pub fn get_devices(&self) -> Result<Vec<String>, MediaError> {
        use nokhwa::utils::ApiBackend;

        let devices = nokhwa::query(ApiBackend::Auto).map_err(|e| MediaError::DeviceError {
            message: format!("Failed to query devices: {}", e),
        })?;

        let device_names: Vec<String> = devices
            .into_iter()
            .map(|info| info.human_name().to_string())
            .collect();

        tracing::info!("🔍 Found {} camera devices via nokhwa", device_names.len());
        Ok(device_names)
    }

    /// Start camera capture
    pub fn start_capture(&self) -> Result<(), MediaError> {
        use nokhwa::{
            pixel_format::RgbFormat,
            utils::{CameraIndex, RequestedFormat, RequestedFormatType},
        };

        let index = CameraIndex::Index(0);
        let format =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        let mut camera =
            nokhwa::Camera::new(index, format).map_err(|e| MediaError::DeviceError {
                message: format!("Failed to create camera: {}", e),
            })?;

        camera.open_stream().map_err(|e| MediaError::DeviceError {
            message: format!("Failed to open camera stream: {}", e),
        })?;

        *self.camera.lock() = Some(camera);

        tracing::info!("✅ Nokhwa camera capture started successfully");
        Ok(())
    }

    /// Stop camera capture
    pub fn stop_capture(&self) -> Result<(), MediaError> {
        let mut camera_guard = self.camera.lock();
        if let Some(mut camera) = camera_guard.take() {
            camera.stop_stream().map_err(|e| MediaError::DeviceError {
                message: format!("Failed to stop camera stream: {}", e),
            })?;
        }

        tracing::info!("📷 Nokhwa camera capture stopped");
        Ok(())
    }

    /// Get a frame from the camera
    pub fn get_frame(&self) -> Result<Option<Vec<u8>>, MediaError> {
        let mut camera_guard = self.camera.lock();
        if let Some(camera) = camera_guard.as_mut() {
            match camera.frame() {
                Ok(buffer) => {
                    // Convert buffer to raw data - buffer contains the frame data
                    let raw_data = buffer.buffer().to_vec();
                    Ok(Some(raw_data))
                }
                Err(e) => {
                    tracing::warn!("Failed to get frame: {}", e);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Check if camera is currently capturing
    pub fn is_capturing(&self) -> bool {
        self.camera.lock().is_some()
    }
}

/// Get the cross-platform capture backend
/// Much simpler now - just return NokhwaCapture for all platforms!
pub fn get_capture_backend() -> NokhwaCapture {
    NokhwaCapture::new()
}
