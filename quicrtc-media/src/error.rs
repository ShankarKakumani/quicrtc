//! Media processing error types and handling
//!
//! This module defines all error types used throughout the media processing library,
//! providing clear error messages and context for debugging and error handling.

use thiserror::Error;

/// Main error type for media processing operations
#[derive(Error, Debug)]
pub enum MediaError {
    /// I/O operation failed
    #[error("I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// Invalid configuration provided
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration {
        /// Error message
        message: String,
    },

    /// Encoding operation failed
    #[error("Encoding failed: {codec} - {reason}")]
    EncodingFailed {
        /// Codec name
        codec: String,
        /// Failure reason
        reason: String,
    },

    /// Decoding operation failed
    #[error("Decoding failed: {codec} - {reason}")]
    DecodingFailed {
        /// Codec name
        codec: String,
        /// Failure reason
        reason: String,
    },

    /// Invalid media type error
    #[error("Invalid media type: expected {expected}, got {actual}")]
    InvalidMediaType {
        /// Expected media type
        expected: String,
        /// Actual media type
        actual: String,
    },

    /// Unsupported platform error
    #[error("Unsupported platform: {platform}")]
    UnsupportedPlatform {
        /// Platform name
        platform: String,
    },

    /// Unsupported format error
    #[error("Unsupported format: {format}")]
    UnsupportedFormat {
        /// Format description
        format: String,
    },

    /// Invalid frame data error
    #[error("Invalid frame data: expected {expected} bytes, got {actual}")]
    InvalidFrameData {
        /// Expected data size
        expected: usize,
        /// Actual data size
        actual: usize,
    },

    /// Device enumeration failed
    #[error("Device enumeration failed: {reason}")]
    DeviceEnumerationFailed {
        /// Failure reason
        reason: String,
    },

    /// Device not found error
    #[error("Device not found: {device_id}")]
    DeviceNotFound {
        /// Device identifier
        device_id: String,
    },

    /// Capture not active error
    #[error("Capture not active")]
    CaptureNotActive,

    /// Configuration error
    #[error("Configuration error: {message}")]
    ConfigurationError {
        /// Error message
        message: String,
    },

    /// Buffer overflow error
    #[error("Buffer overflow: {size} bytes")]
    BufferOverflow {
        /// Buffer size that overflowed
        size: usize,
    },

    /// Timeout error
    #[error("Operation timed out after {duration:?}")]
    Timeout {
        /// Duration after which timeout occurred
        duration: std::time::Duration,
    },

    /// Resource not available
    #[error("Resource not available: {resource}")]
    ResourceNotAvailable {
        /// Resource name
        resource: String,
    },

    /// Hardware acceleration not available
    #[error("Hardware acceleration not available: {reason}")]
    HardwareAccelerationNotAvailable {
        /// Reason why hardware acceleration is not available
        reason: String,
    },

    /// Permission denied error
    #[error("Permission denied: {operation}")]
    PermissionDenied {
        /// Operation that was denied
        operation: String,
    },

    /// Audio specific errors
    #[error("Audio error: {message}")]
    Audio {
        /// Error message
        message: String,
    },

    /// Video specific errors
    #[error("Video error: {message}")]
    Video {
        /// Error message
        message: String,
    },

    /// Codec initialization failed
    #[error("Codec initialization failed: {codec} - {reason}")]
    CodecInitializationFailed {
        /// Codec name
        codec: String,
        /// Failure reason
        reason: String,
    },

    /// Invalid state for operation
    #[error("Invalid state: {message}")]
    InvalidState {
        /// State error message
        message: String,
    },

    /// FFI error from external libraries
    #[error("FFI error: {library} - {message}")]
    FfiError {
        /// Library name
        library: String,
        /// Error message
        message: String,
    },

    /// Memory allocation failed
    #[error("Memory allocation failed: {size} bytes")]
    MemoryAllocationFailed {
        /// Size that failed to allocate
        size: usize,
    },

    /// Sample rate mismatch
    #[error("Sample rate mismatch: expected {expected}, got {actual}")]
    SampleRateMismatch {
        /// Expected sample rate
        expected: u32,
        /// Actual sample rate
        actual: u32,
    },

    /// Channel count mismatch
    #[error("Channel count mismatch: expected {expected}, got {actual}")]
    ChannelCountMismatch {
        /// Expected channel count
        expected: u32,
        /// Actual channel count
        actual: u32,
    },

    /// Bandwidth estimation error
    #[error("Bandwidth estimation error: {message}")]
    BandwidthEstimationError {
        /// Error message
        message: String,
    },
}

/// Result type alias for media operations
pub type MediaResult<T> = Result<T, MediaError>;

impl MediaError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            MediaError::Io { .. } => true,
            MediaError::Timeout { .. } => true,
            MediaError::ResourceNotAvailable { .. } => true,
            MediaError::BufferOverflow { .. } => true,
            MediaError::EncodingFailed { .. } => false,
            MediaError::DecodingFailed { .. } => false,
            MediaError::UnsupportedPlatform { .. } => false,
            MediaError::UnsupportedFormat { .. } => false,
            MediaError::CodecInitializationFailed { .. } => false,
            MediaError::PermissionDenied { .. } => false,
            MediaError::HardwareAccelerationNotAvailable { .. } => true,
            _ => false,
        }
    }

    /// Get error category
    pub fn category(&self) -> ErrorCategory {
        match self {
            MediaError::Io { .. } => ErrorCategory::System,
            MediaError::InvalidConfiguration { .. } => ErrorCategory::Configuration,
            MediaError::EncodingFailed { .. } => ErrorCategory::Codec,
            MediaError::DecodingFailed { .. } => ErrorCategory::Codec,
            MediaError::InvalidMediaType { .. } => ErrorCategory::Format,
            MediaError::UnsupportedPlatform { .. } => ErrorCategory::Platform,
            MediaError::UnsupportedFormat { .. } => ErrorCategory::Format,
            MediaError::InvalidFrameData { .. } => ErrorCategory::Data,
            MediaError::DeviceEnumerationFailed { .. } => ErrorCategory::Device,
            MediaError::DeviceNotFound { .. } => ErrorCategory::Device,
            MediaError::CaptureNotActive => ErrorCategory::State,
            MediaError::ConfigurationError { .. } => ErrorCategory::Configuration,
            MediaError::BufferOverflow { .. } => ErrorCategory::Memory,
            MediaError::Timeout { .. } => ErrorCategory::System,
            MediaError::ResourceNotAvailable { .. } => ErrorCategory::System,
            MediaError::HardwareAccelerationNotAvailable { .. } => ErrorCategory::Platform,
            MediaError::PermissionDenied { .. } => ErrorCategory::System,
            MediaError::Audio { .. } => ErrorCategory::Audio,
            MediaError::Video { .. } => ErrorCategory::Video,
            MediaError::CodecInitializationFailed { .. } => ErrorCategory::Codec,
            MediaError::InvalidState { .. } => ErrorCategory::State,
            MediaError::FfiError { .. } => ErrorCategory::System,
            MediaError::MemoryAllocationFailed { .. } => ErrorCategory::Memory,
            MediaError::SampleRateMismatch { .. } => ErrorCategory::Audio,
            MediaError::ChannelCountMismatch { .. } => ErrorCategory::Audio,
            MediaError::BandwidthEstimationError { .. } => ErrorCategory::Network,
        }
    }
}

/// Error categories for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// System-level errors (I/O, permissions, etc.)
    System,
    /// Configuration and parameter errors
    Configuration,
    /// Codec-related errors
    Codec,
    /// Format and data structure errors
    Format,
    /// Platform compatibility errors
    Platform,
    /// Data validation errors
    Data,
    /// Device and hardware errors
    Device,
    /// State management errors
    State,
    /// Memory management errors
    Memory,
    /// Audio-specific errors
    Audio,
    /// Video-specific errors
    Video,
    /// Network-related errors
    Network,
}

/// Helper trait for converting platform-specific errors
pub trait IntoMediaError {
    fn into_media_error(self, context: &str) -> MediaError;
}

impl IntoMediaError for Box<dyn std::error::Error> {
    fn into_media_error(self, context: &str) -> MediaError {
        MediaError::FfiError {
            library: "external".to_string(),
            message: format!("{}: {}", context, self),
        }
    }
}

impl IntoMediaError for String {
    fn into_media_error(self, context: &str) -> MediaError {
        MediaError::FfiError {
            library: "external".to_string(),
            message: format!("{}: {}", context, self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let io_error = MediaError::Io {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        };
        assert_eq!(io_error.category(), ErrorCategory::System);
        assert!(io_error.is_recoverable());

        let codec_error = MediaError::CodecInitializationFailed {
            codec: "H.264".to_string(),
            reason: "Hardware not available".to_string(),
        };
        assert_eq!(codec_error.category(), ErrorCategory::Codec);
        assert!(!codec_error.is_recoverable());
    }

    #[test]
    fn test_error_display() {
        let error = MediaError::InvalidFrameData {
            expected: 1024,
            actual: 512,
        };
        assert_eq!(
            error.to_string(),
            "Invalid frame data: expected 1024 bytes, got 512"
        );
    }

    #[test]
    fn test_error_from_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let media_error = MediaError::from(io_error);

        match media_error {
            MediaError::Io { .. } => (),
            _ => panic!("Expected Io error variant"),
        }
    }
}
