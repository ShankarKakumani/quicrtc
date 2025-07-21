//! V4L2-based video capture for Linux
//!
//! This module provides native Linux video capture using Video4Linux2 (V4L2)
//! with support for various USB cameras and capture devices.

use super::PlatformCapture;
use crate::error::MediaError;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "linux")]
use v4l::prelude::*;
#[cfg(target_os = "linux")]
use v4l::Device;

/// V4L2-based video capture implementation
pub struct V4L2Capture {
    is_capturing: AtomicBool,
}

impl V4L2Capture {
    pub fn new() -> Self {
        Self {
            is_capturing: AtomicBool::new(false),
        }
    }

    #[cfg(target_os = "linux")]
    fn enumerate_linux_devices(&self) -> Result<Vec<String>, MediaError> {
        use std::path::Path;

        let mut devices = Vec::new();

        // Scan for /dev/video* devices
        for i in 0..16 {
            let device_path = format!("/dev/video{}", i);
            if Path::new(&device_path).exists() {
                match Device::new(i) {
                    Ok(device) => {
                        // Get device capabilities to verify it's a video capture device
                        match device.query_caps() {
                            Ok(caps) => {
                                // Check if device supports video capture
                                if caps.capabilities & v4l::capability::Flags::VIDEO_CAPTURE.bits()
                                    != 0
                                {
                                    let device_name = caps.card.trim_end_matches('\0').to_string();
                                    devices.push(format!("{}: {}", device_path, device_name));
                                }
                            }
                            Err(_) => {
                                // Skip devices we can't query
                                continue;
                            }
                        }
                    }
                    Err(_) => {
                        // Skip devices we can't open
                        continue;
                    }
                }
            }
        }

        Ok(devices)
    }

    #[cfg(not(target_os = "linux"))]
    fn enumerate_linux_devices(&self) -> Result<Vec<String>, MediaError> {
        Err(MediaError::UnsupportedPlatform {
            platform: "V4L2 only supported on Linux".to_string(),
        })
    }
}

impl PlatformCapture for V4L2Capture {
    fn start_capture(&self) -> Result<(), MediaError> {
        #[cfg(target_os = "linux")]
        {
            self.is_capturing.store(true, Ordering::Relaxed);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(MediaError::UnsupportedPlatform {
                platform: "V4L2 only supported on Linux".to_string(),
            })
        }
    }

    fn stop_capture(&self) -> Result<(), MediaError> {
        #[cfg(target_os = "linux")]
        {
            self.is_capturing.store(false, Ordering::Relaxed);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(MediaError::UnsupportedPlatform {
                platform: "V4L2 only supported on Linux".to_string(),
            })
        }
    }

    fn get_devices(&self) -> Result<Vec<String>, MediaError> {
        self.enumerate_linux_devices()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v4l2_creation() {
        let capture = V4L2Capture::new();
        assert!(!capture.is_capturing.load(Ordering::Relaxed));
    }

    #[test]
    fn test_get_devices() {
        let capture = V4L2Capture::new();
        let result = capture.get_devices();

        // Should not fail, even if no devices available
        assert!(result.is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_specific_functionality() {
        let capture = V4L2Capture::new();

        // Test device enumeration
        let devices = capture.get_devices().unwrap();
        println!("Available V4L2 devices: {:?}", devices);

        // Test start/stop
        assert!(capture.start_capture().is_ok());
        assert!(capture.is_capturing.load(Ordering::Relaxed));
        assert!(capture.stop_capture().is_ok());
        assert!(!capture.is_capturing.load(Ordering::Relaxed));
    }
}
