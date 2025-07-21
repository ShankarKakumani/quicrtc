//! Track abstractions and media frame types

/// Audio frame representation
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Audio samples (f32 PCM data)
    pub samples: Vec<f32>,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
    /// Timestamp in milliseconds
    pub timestamp: u64,
}

/// Video frame representation
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Frame data (encoded or raw)
    pub data: Vec<u8>,
    /// Timestamp in milliseconds
    pub timestamp: u64,
    /// Whether this is a keyframe
    pub is_keyframe: bool,
}

/// Media frame types
#[derive(Debug, Clone)]
pub enum MediaFrame {
    /// Audio frame
    Audio(AudioFrame),
    /// Video frame
    Video(VideoFrame),
}

/// Video track representation
#[derive(Debug)]
pub struct VideoTrack {
    /// Track ID
    pub id: String,
    // TODO: Add video track state
}

impl VideoTrack {
    /// Create new video track
    pub fn new(id: String) -> Self {
        Self { id }
    }
    
    /// Get track ID
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Audio track representation
#[derive(Debug)]
pub struct AudioTrack {
    /// Track ID
    pub id: String,
    // TODO: Add audio track state
}

impl AudioTrack {
    /// Create new audio track
    pub fn new(id: String) -> Self {
        Self { id }
    }
    
    /// Get track ID
    pub fn id(&self) -> &str {
        &self.id
    }
}