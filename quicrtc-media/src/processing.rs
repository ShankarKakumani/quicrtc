//! Media processing and quality control

use crate::tracks::{AudioFrame, MediaFrame, VideoFrame};
use quicrtc_core::{MoqObject, MoqObjectStatus, QuicRtcError, TrackNamespace};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// Media processor for handling MoQ objects and media frames
#[derive(Debug)]
pub struct MediaProcessor {
    // TODO: Add media processor state
}

impl MediaProcessor {
    /// Create new media processor
    pub fn new() -> Self {
        Self {}
    }

    /// Process incoming MoQ object directly (no RTP depacketization)
    pub fn process_incoming_object(
        &mut self,
        _object: MoqObject,
    ) -> Result<MediaFrame, QuicRtcError> {
        // TODO: Implement MoQ object to media frame conversion
        Ok(MediaFrame::Video(VideoFrame {
            width: 640,
            height: 480,
            data: vec![],
            timestamp: 0,
            is_keyframe: false,
        }))
    }

    /// Prepare outgoing MoQ object directly from media frame (no RTP packetization)
    pub fn prepare_outgoing_object(
        &mut self,
        _frame: MediaFrame,
    ) -> Result<MoqObject, QuicRtcError> {
        // TODO: Implement media frame to MoQ object conversion
        use quicrtc_core::{MoqObjectStatus, TrackNamespace};

        Ok(MoqObject {
            track_namespace: TrackNamespace {
                namespace: "example.com".to_string(),
                track_name: "video".to_string(),
            },
            track_name: "video".to_string(),
            group_id: 0,
            object_id: 0,
            publisher_priority: 1,
            payload: vec![],
            object_status: MoqObjectStatus::Normal,
            created_at: std::time::Instant::now(),
            size: 0,
        })
    }
}

impl Default for MediaProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// MoQ Object Assembler - replaces RTP jitter buffer functionality
///
/// This component reconstructs media frames from MoQ objects, handling:
/// - Group assembly logic for video frames and audio samples
/// - Missing object detection and handling using MoQ delivery semantics
/// - Object ordering and buffering
/// - Frame completion detection
#[derive(Debug)]
pub struct MoqObjectAssembler {
    /// Pending groups being assembled, keyed by track namespace and group ID
    pending_groups: HashMap<TrackNamespace, HashMap<u64, GroupAssembly>>,
    /// Completed frames ready for processing
    frame_buffer: FrameBuffer,
    /// Track state information
    track_state: HashMap<TrackNamespace, TrackState>,
    /// Configuration
    config: AssemblerConfig,
}

/// Configuration for the MoQ object assembler
#[derive(Debug, Clone)]
pub struct AssemblerConfig {
    /// Maximum time to wait for missing objects before giving up
    pub max_wait_time: Duration,
    /// Maximum number of pending groups per track
    pub max_pending_groups: usize,
    /// Maximum frame buffer size
    pub max_frame_buffer_size: usize,
    /// Whether to request retransmission of missing objects
    pub enable_retransmission: bool,
}

impl Default for AssemblerConfig {
    fn default() -> Self {
        Self {
            max_wait_time: Duration::from_millis(100), // 100ms max wait
            max_pending_groups: 10,
            max_frame_buffer_size: 50,
            enable_retransmission: true,
        }
    }
}

/// Assembly state for a single group (e.g., video frame or audio sample group)
#[derive(Debug)]
struct GroupAssembly {
    /// Group ID
    group_id: u64,
    /// Track namespace
    track_namespace: TrackNamespace,
    /// Objects in this group, keyed by object ID
    objects: BTreeMap<u64, MoqObject>,
    /// Expected object count (if known)
    expected_objects: Option<usize>,
    /// Whether we've seen the end-of-group marker
    end_of_group_received: bool,
    /// When this group assembly started
    started_at: Instant,
    /// Missing object IDs that we're waiting for
    missing_objects: Vec<u64>,
    /// Whether we've requested retransmission
    retransmission_requested: bool,
}

/// Frame buffer for completed media frames
#[derive(Debug)]
struct FrameBuffer {
    /// Buffered frames, keyed by track namespace
    frames: HashMap<TrackNamespace, Vec<MediaFrame>>,
    /// Maximum buffer size per track
    max_size_per_track: usize,
}

/// Track state information
#[derive(Debug)]
struct TrackState {
    /// Track namespace
    track_namespace: TrackNamespace,
    /// Last processed group ID
    last_group_id: Option<u64>,
    /// Last processed object ID within current group
    last_object_id: Option<u64>,
    /// Track type (audio/video)
    track_type: TrackType,
    /// Statistics
    stats: TrackStats,
}

/// Track type for assembly logic
#[derive(Debug, Clone, PartialEq)]
enum TrackType {
    Audio,
    Video,
    Data,
}

/// Statistics for track processing
#[derive(Debug, Default)]
pub struct TrackStats {
    /// Total objects received
    pub objects_received: u64,
    /// Total groups completed
    pub groups_completed: u64,
    /// Total missing objects detected
    pub missing_objects: u64,
    /// Total retransmission requests sent
    pub retransmission_requests: u64,
    /// Total frames assembled
    pub frames_assembled: u64,
}

impl MoqObjectAssembler {
    /// Create new MoQ object assembler with default configuration
    pub fn new() -> Self {
        Self::with_config(AssemblerConfig::default())
    }

    /// Create new MoQ object assembler with custom configuration
    pub fn with_config(config: AssemblerConfig) -> Self {
        Self {
            pending_groups: HashMap::new(),
            frame_buffer: FrameBuffer {
                frames: HashMap::new(),
                max_size_per_track: config.max_frame_buffer_size,
            },
            track_state: HashMap::new(),
            config,
        }
    }

    /// Add MoQ object and potentially complete a media frame
    ///
    /// Returns Some(MediaFrame) if a complete frame was assembled,
    /// None if more objects are needed or if the object was buffered.
    pub fn add_object(&mut self, object: MoqObject) -> Result<Option<MediaFrame>, QuicRtcError> {
        let track_namespace = object.track_namespace.clone();
        let group_id = object.group_id;
        let object_id = object.object_id;

        // Update track state
        self.update_track_state(&track_namespace, &object);

        // Handle end-of-track objects
        if object.object_status == MoqObjectStatus::EndOfTrack {
            return self.handle_end_of_track(object);
        }

        // Get or create group assembly
        let group_assembly = self.get_or_create_group_assembly(&track_namespace, group_id);

        // Add object to group
        group_assembly.objects.insert(object_id, object.clone());

        // Handle end-of-group marker
        if object.object_status == MoqObjectStatus::EndOfGroup {
            group_assembly.end_of_group_received = true;
            group_assembly.expected_objects = Some(group_assembly.objects.len());
        }

        // Check if group is complete
        if self.is_group_complete(&track_namespace, group_id)? {
            let completed_group = self.remove_completed_group(&track_namespace, group_id)?;
            let frame = self.assemble_frame_from_group(completed_group)?;

            // Update statistics
            if let Some(track_state) = self.track_state.get_mut(&track_namespace) {
                track_state.stats.groups_completed += 1;
                track_state.stats.frames_assembled += 1;
            }

            return Ok(Some(frame));
        }

        // Check for missing objects and handle them
        self.detect_and_handle_missing_objects(&track_namespace, group_id)?;

        Ok(None)
    }

    /// Handle missing MoQ objects using MoQ delivery semantics
    pub fn handle_missing_objects(&mut self, group_id: u64, missing_objects: Vec<u64>) {
        // Update statistics for all tracks that might be affected
        for track_state in self.track_state.values_mut() {
            track_state.stats.missing_objects += missing_objects.len() as u64;
        }

        // In a real implementation, this would:
        // 1. Check if the missing objects are still within the acceptable delay window
        // 2. Decide whether to wait longer or give up on the objects
        // 3. Potentially request retransmission through MoQ mechanisms
        // 4. Update group assembly state accordingly

        // For now, we'll log the missing objects
        tracing::warn!(
            "Missing objects detected in group {}: {:?}",
            group_id,
            missing_objects
        );
    }

    /// Request object retransmission using MoQ mechanisms
    pub fn request_object_retransmission(
        &mut self,
        track: &TrackNamespace,
        object_id: u64,
    ) -> Result<(), QuicRtcError> {
        if !self.config.enable_retransmission {
            return Ok(());
        }

        // Update statistics
        if let Some(track_state) = self.track_state.get_mut(track) {
            track_state.stats.retransmission_requests += 1;
        }

        // In a real implementation, this would send a retransmission request
        // through the MoQ transport layer. For now, we'll just log it.
        tracing::info!(
            "Requesting retransmission for track {:?}, object {}",
            track,
            object_id
        );

        Ok(())
    }

    /// Get next completed frame from buffer
    pub fn get_next_frame(&mut self, track: &TrackNamespace) -> Option<MediaFrame> {
        self.frame_buffer.frames.get_mut(track).and_then(|frames| {
            if frames.is_empty() {
                None
            } else {
                Some(frames.remove(0))
            }
        })
    }

    /// Check if there are any completed frames available
    pub fn has_frames(&self, track: &TrackNamespace) -> bool {
        self.frame_buffer
            .frames
            .get(track)
            .map(|frames| !frames.is_empty())
            .unwrap_or(false)
    }

    /// Get assembler statistics for a track
    pub fn get_track_stats(&self, track: &TrackNamespace) -> Option<&TrackStats> {
        self.track_state.get(track).map(|state| &state.stats)
    }

    /// Cleanup old pending groups that have exceeded the maximum wait time
    pub fn cleanup_expired_groups(&mut self) -> Result<Vec<MediaFrame>, QuicRtcError> {
        let mut completed_frames = Vec::new();
        let now = Instant::now();

        // Collect expired groups first to avoid borrowing issues
        let mut expired_groups_by_track = Vec::new();

        for (track_namespace, groups) in &self.pending_groups {
            let mut expired_groups = Vec::new();

            for (group_id, group_assembly) in groups.iter() {
                if now.duration_since(group_assembly.started_at) > self.config.max_wait_time {
                    expired_groups.push(*group_id);
                }
            }

            if !expired_groups.is_empty() {
                expired_groups_by_track.push((track_namespace.clone(), expired_groups));
            }
        }

        // Process expired groups
        for (track_namespace, expired_groups) in expired_groups_by_track {
            if let Some(groups) = self.pending_groups.get_mut(&track_namespace) {
                for group_id in expired_groups {
                    if let Some(group_assembly) = groups.remove(&group_id) {
                        tracing::warn!(
                            "Group {} for track {:?} expired after {:?}, assembling partial frame",
                            group_id,
                            track_namespace,
                            now.duration_since(group_assembly.started_at)
                        );

                        // Try to assemble a partial frame
                        // We need to call this as a separate method to avoid borrowing issues
                        match Self::assemble_frame_from_group_static(
                            group_assembly,
                            &self.track_state,
                        ) {
                            Ok(frame) => completed_frames.push(frame),
                            Err(e) => {
                                tracing::error!("Failed to assemble partial frame: {}", e);
                            }
                        }
                    }
                }
            }
        }

        // Clean up empty track entries
        self.pending_groups.retain(|_, groups| !groups.is_empty());

        Ok(completed_frames)
    }

    // Private helper methods

    fn update_track_state(&mut self, track_namespace: &TrackNamespace, object: &MoqObject) {
        let track_type = self.infer_track_type(&object.track_name);
        let track_state = self
            .track_state
            .entry(track_namespace.clone())
            .or_insert_with(|| TrackState {
                track_namespace: track_namespace.clone(),
                last_group_id: None,
                last_object_id: None,
                track_type,
                stats: TrackStats::default(),
            });

        track_state.stats.objects_received += 1;
        track_state.last_group_id = Some(object.group_id);
        track_state.last_object_id = Some(object.object_id);
    }

    fn infer_track_type(&self, track_name: &str) -> TrackType {
        if track_name.contains("video")
            || track_name.contains("camera")
            || track_name.contains("screen")
        {
            TrackType::Video
        } else if track_name.contains("audio")
            || track_name.contains("microphone")
            || track_name.contains("mic")
        {
            TrackType::Audio
        } else {
            TrackType::Data
        }
    }

    fn get_or_create_group_assembly(
        &mut self,
        track_namespace: &TrackNamespace,
        group_id: u64,
    ) -> &mut GroupAssembly {
        let track_groups = self
            .pending_groups
            .entry(track_namespace.clone())
            .or_insert_with(HashMap::new);

        track_groups
            .entry(group_id)
            .or_insert_with(|| GroupAssembly {
                group_id,
                track_namespace: track_namespace.clone(),
                objects: BTreeMap::new(),
                expected_objects: None,
                end_of_group_received: false,
                started_at: Instant::now(),
                missing_objects: Vec::new(),
                retransmission_requested: false,
            })
    }

    fn is_group_complete(
        &self,
        track_namespace: &TrackNamespace,
        group_id: u64,
    ) -> Result<bool, QuicRtcError> {
        let group_assembly = self
            .pending_groups
            .get(track_namespace)
            .and_then(|groups| groups.get(&group_id))
            .ok_or_else(|| QuicRtcError::InvalidState {
                expected: "Group assembly to exist".to_string(),
                actual: "Group assembly not found".to_string(),
            })?;

        // Group is complete if we have the end-of-group marker
        // and all expected objects (if known)
        if group_assembly.end_of_group_received {
            if let Some(expected_count) = group_assembly.expected_objects {
                Ok(group_assembly.objects.len() >= expected_count)
            } else {
                // If we don't know the expected count, assume complete when we see end-of-group
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    fn remove_completed_group(
        &mut self,
        track_namespace: &TrackNamespace,
        group_id: u64,
    ) -> Result<GroupAssembly, QuicRtcError> {
        self.pending_groups
            .get_mut(track_namespace)
            .and_then(|groups| groups.remove(&group_id))
            .ok_or_else(|| QuicRtcError::InvalidState {
                expected: "Completed group to exist".to_string(),
                actual: "Completed group not found".to_string(),
            })
    }

    fn assemble_frame_from_group(
        &self,
        group_assembly: GroupAssembly,
    ) -> Result<MediaFrame, QuicRtcError> {
        Self::assemble_frame_from_group_static(group_assembly, &self.track_state)
    }

    fn assemble_frame_from_group_static(
        group_assembly: GroupAssembly,
        track_state: &HashMap<TrackNamespace, TrackState>,
    ) -> Result<MediaFrame, QuicRtcError> {
        if group_assembly.objects.is_empty() {
            return Err(QuicRtcError::InvalidData {
                reason: "Cannot assemble frame from empty group".to_string(),
            });
        }

        // Get track type to determine assembly strategy
        let track_type = track_state
            .get(&group_assembly.track_namespace)
            .map(|state| &state.track_type)
            .unwrap_or(&TrackType::Data);

        match track_type {
            TrackType::Video => Self::assemble_video_frame_static(group_assembly),
            TrackType::Audio => Self::assemble_audio_frame_static(group_assembly),
            TrackType::Data => Self::assemble_data_frame_static(group_assembly),
        }
    }

    fn assemble_video_frame(
        &self,
        group_assembly: GroupAssembly,
    ) -> Result<MediaFrame, QuicRtcError> {
        Self::assemble_video_frame_static(group_assembly)
    }

    fn assemble_video_frame_static(
        group_assembly: GroupAssembly,
    ) -> Result<MediaFrame, QuicRtcError> {
        // For video, concatenate all object payloads in order
        let mut frame_data = Vec::new();

        for (_object_id, object) in group_assembly.objects {
            // Include all objects except empty end-of-group markers
            if object.object_status != MoqObjectStatus::EndOfGroup || !object.payload.is_empty() {
                frame_data.extend_from_slice(&object.payload);
            }
        }

        if frame_data.is_empty() {
            return Err(QuicRtcError::InvalidData {
                reason: "Video frame has no data".to_string(),
            });
        }

        // For now, assume standard resolution - in a real implementation,
        // this would be parsed from the codec data or track metadata
        Ok(MediaFrame::Video(VideoFrame {
            width: 640,
            height: 480,
            data: frame_data,
            timestamp: group_assembly.group_id,
            is_keyframe: false, // TODO: Determine from MoQ object metadata
        }))
    }

    fn assemble_audio_frame(
        &self,
        group_assembly: GroupAssembly,
    ) -> Result<MediaFrame, QuicRtcError> {
        Self::assemble_audio_frame_static(group_assembly)
    }

    fn assemble_audio_frame_static(
        group_assembly: GroupAssembly,
    ) -> Result<MediaFrame, QuicRtcError> {
        // For audio, we need to decode the objects and combine the samples
        let mut all_samples = Vec::new();
        let sample_rate = 48000; // Default
        let channels = 2; // Default

        for (_object_id, object) in group_assembly.objects {
            // Process all objects that have payload data
            if !object.payload.is_empty() {
                // In a real implementation, this would decode the Opus data
                // For now, simulate by creating samples based on payload
                let samples_per_object = 960; // 20ms at 48kHz
                let mut object_samples = Vec::with_capacity(samples_per_object * channels as usize);

                // Generate samples based on payload data
                for i in 0..(samples_per_object * channels as usize) {
                    let payload_idx = i % object.payload.len();
                    let sample_value = (object.payload[payload_idx] as f32 - 128.0) / 128.0;
                    object_samples.push(sample_value * 0.1); // Reduce amplitude
                }

                all_samples.extend(object_samples);
            }
        }

        if all_samples.is_empty() {
            return Err(QuicRtcError::InvalidData {
                reason: "Audio frame has no samples".to_string(),
            });
        }

        Ok(MediaFrame::Audio(AudioFrame {
            samples: all_samples,
            sample_rate,
            channels,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }))
    }

    fn assemble_data_frame(
        &self,
        group_assembly: GroupAssembly,
    ) -> Result<MediaFrame, QuicRtcError> {
        Self::assemble_data_frame_static(group_assembly)
    }

    fn assemble_data_frame_static(
        group_assembly: GroupAssembly,
    ) -> Result<MediaFrame, QuicRtcError> {
        // For data tracks, treat as video for now
        Self::assemble_video_frame_static(group_assembly)
    }

    fn detect_and_handle_missing_objects(
        &mut self,
        track_namespace: &TrackNamespace,
        group_id: u64,
    ) -> Result<(), QuicRtcError> {
        // First, collect information about missing objects
        let missing_objects = {
            let group_assembly = self
                .pending_groups
                .get(track_namespace)
                .and_then(|groups| groups.get(&group_id))
                .ok_or_else(|| QuicRtcError::InvalidState {
                    expected: "Group assembly to exist".to_string(),
                    actual: "Group assembly not found".to_string(),
                })?;

            // Simple gap detection - check for missing object IDs in sequence
            if group_assembly.objects.len() >= 2 {
                let object_ids: Vec<u64> = group_assembly.objects.keys().cloned().collect();
                let min_id = *object_ids.iter().min().unwrap();
                let max_id = *object_ids.iter().max().unwrap();

                let mut missing = Vec::new();
                for id in min_id..=max_id {
                    if !group_assembly.objects.contains_key(&id) {
                        missing.push(id);
                    }
                }

                if !missing.is_empty() && !group_assembly.retransmission_requested {
                    Some(missing)
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Now handle missing objects if any were found
        if let Some(missing) = missing_objects {
            // Request retransmission for missing objects first
            if self.config.enable_retransmission {
                for missing_id in &missing {
                    self.request_object_retransmission(track_namespace, *missing_id)?;
                }
            }

            // Update group assembly state
            if let Some(group_assembly) = self
                .pending_groups
                .get_mut(track_namespace)
                .and_then(|groups| groups.get_mut(&group_id))
            {
                group_assembly.missing_objects = missing;
                group_assembly.retransmission_requested = self.config.enable_retransmission;
            }
        }

        Ok(())
    }

    fn handle_end_of_track(
        &mut self,
        object: MoqObject,
    ) -> Result<Option<MediaFrame>, QuicRtcError> {
        let track_namespace = &object.track_namespace;

        // Clean up any pending groups for this track
        if let Some(groups) = self.pending_groups.remove(track_namespace) {
            tracing::info!(
                "End of track received for {:?}, cleaning up {} pending groups",
                track_namespace,
                groups.len()
            );
        }

        // Remove track state
        self.track_state.remove(track_namespace);

        // Clear frame buffer for this track
        self.frame_buffer.frames.remove(track_namespace);

        Ok(None)
    }
}

impl Default for MoqObjectAssembler {
    fn default() -> Self {
        Self::new()
    }
}

// Tests moved to tests/ directory

/// Quality control system with bandwidth estimation and MoQ-aware adaptation
#[derive(Debug)]
pub struct QualityController {
    bandwidth_estimator: BandwidthEstimator,
    quality_adapter: QualityAdapter,
    congestion_detector: CongestionDetector,
    current_settings: QualitySettings,
    config: QualityControlConfig,
}

/// Configuration for quality control
#[derive(Debug, Clone)]
pub struct QualityControlConfig {
    /// Minimum bitrate in bits per second
    pub min_bitrate: u32,
    /// Maximum bitrate in bits per second
    pub max_bitrate: u32,
    /// Target buffer duration in milliseconds
    pub target_buffer_ms: u32,
    /// Congestion detection threshold (packet loss ratio)
    pub congestion_threshold: f32,
    /// Quality adaptation step size (0.0 to 1.0)
    pub adaptation_step: f32,
    /// Enable aggressive adaptation for mobile
    pub mobile_mode: bool,
}

impl Default for QualityControlConfig {
    fn default() -> Self {
        Self {
            min_bitrate: 100_000,       // 100 kbps
            max_bitrate: 5_000_000,     // 5 Mbps
            target_buffer_ms: 200,      // 200ms buffer
            congestion_threshold: 0.05, // 5% packet loss
            adaptation_step: 0.1,       // 10% steps
            mobile_mode: false,
        }
    }
}

impl QualityControlConfig {
    /// Create mobile-optimized configuration
    pub fn mobile() -> Self {
        Self {
            min_bitrate: 50_000,        // 50 kbps
            max_bitrate: 2_000_000,     // 2 Mbps
            target_buffer_ms: 300,      // 300ms buffer for mobile
            congestion_threshold: 0.03, // 3% packet loss (more sensitive)
            adaptation_step: 0.15,      // 15% steps (more aggressive)
            mobile_mode: true,
        }
    }

    /// Create desktop-optimized configuration
    pub fn desktop() -> Self {
        Self {
            min_bitrate: 200_000,       // 200 kbps
            max_bitrate: 10_000_000,    // 10 Mbps
            target_buffer_ms: 150,      // 150ms buffer
            congestion_threshold: 0.08, // 8% packet loss
            adaptation_step: 0.08,      // 8% steps
            mobile_mode: false,
        }
    }
}

/// Current quality settings
#[derive(Debug, Clone)]
pub struct QualitySettings {
    /// Video bitrate in bits per second
    pub video_bitrate: u32,
    /// Audio bitrate in bits per second
    pub audio_bitrate: u32,
    /// Video resolution width
    pub video_width: u32,
    /// Video resolution height
    pub video_height: u32,
    /// Video framerate
    pub video_framerate: u32,
    /// Audio sample rate
    pub audio_sample_rate: u32,
    /// Audio channels
    pub audio_channels: u8,
}

impl Default for QualitySettings {
    fn default() -> Self {
        Self {
            video_bitrate: 1_000_000, // 1 Mbps
            audio_bitrate: 64_000,    // 64 kbps
            video_width: 640,
            video_height: 480,
            video_framerate: 30,
            audio_sample_rate: 48000,
            audio_channels: 2,
        }
    }
}

/// Bandwidth estimator using MoQ delivery metrics
#[derive(Debug)]
struct BandwidthEstimator {
    /// Recent bandwidth samples (bytes per second)
    samples: Vec<BandwidthSample>,
    /// Maximum number of samples to keep
    max_samples: usize,
    /// Current estimated bandwidth
    estimated_bandwidth: u32,
    /// Last update time
    last_update: Instant,
}

#[derive(Debug, Clone)]
struct BandwidthSample {
    /// Timestamp of the sample
    timestamp: Instant,
    /// Bytes transferred
    bytes: u64,
    /// Duration of the measurement
    duration: Duration,
}

/// Quality adapter that adjusts settings based on network conditions
#[derive(Debug)]
struct QualityAdapter {
    /// Adaptation history
    adaptation_history: Vec<QualityAdaptation>,
    /// Last adaptation time
    last_adaptation: Instant,
    /// Minimum time between adaptations
    min_adaptation_interval: Duration,
}

#[derive(Debug, Clone)]
struct QualityAdaptation {
    /// Timestamp of adaptation
    timestamp: Instant,
    /// Previous settings
    previous_settings: QualitySettings,
    /// New settings
    new_settings: QualitySettings,
    /// Reason for adaptation
    reason: AdaptationReason,
}

#[derive(Debug, Clone)]
enum AdaptationReason {
    /// Bandwidth increased
    BandwidthIncrease,
    /// Bandwidth decreased
    BandwidthDecrease,
    /// Congestion detected
    CongestionDetected,
    /// Buffer underrun
    BufferUnderrun,
    /// Buffer overrun
    BufferOverrun,
    /// Manual adjustment
    Manual,
}

/// Congestion detector using MoQ object delivery metrics
#[derive(Debug)]
struct CongestionDetector {
    /// Recent delivery statistics
    delivery_stats: Vec<MoqDeliveryMetrics>,
    /// Maximum number of stats to keep
    max_stats: usize,
    /// Current congestion level
    congestion_level: CongestionLevel,
    /// Last congestion check
    last_check: Instant,
}

/// MoQ-specific delivery metrics for congestion detection
#[derive(Debug, Clone)]
pub struct MoqDeliveryMetrics {
    /// Timestamp of metrics
    pub timestamp: Instant,
    /// Number of objects delivered successfully
    pub objects_delivered: u64,
    /// Number of objects lost or timed out
    pub objects_lost: u64,
    /// Average object delivery time
    pub avg_delivery_time: Duration,
    /// Maximum object delivery time
    pub max_delivery_time: Duration,
    /// Number of retransmission requests
    pub retransmission_requests: u64,
    /// Buffer level (number of pending objects)
    pub buffer_level: u64,
}

/// Congestion level indicator
#[derive(Debug, Clone, PartialEq)]
pub enum CongestionLevel {
    /// No congestion detected
    None,
    /// Light congestion
    Light,
    /// Moderate congestion
    Moderate,
    /// Heavy congestion
    Heavy,
}

impl QualityController {
    /// Create new quality controller with default configuration
    pub fn new() -> Self {
        Self::with_config(QualityControlConfig::default())
    }

    /// Create new quality controller with custom configuration
    pub fn with_config(config: QualityControlConfig) -> Self {
        Self {
            bandwidth_estimator: BandwidthEstimator::new(),
            quality_adapter: QualityAdapter::new(),
            congestion_detector: CongestionDetector::new(),
            current_settings: QualitySettings::default(),
            config,
        }
    }

    /// Adapt quality based on MoQ delivery metrics
    pub fn adapt_quality(&mut self, moq_metrics: &MoqDeliveryMetrics) -> QualitySettings {
        // Update bandwidth estimation
        self.bandwidth_estimator
            .update_from_moq_metrics(moq_metrics);

        // Update congestion detection
        self.congestion_detector.update(moq_metrics);

        // Determine if adaptation is needed
        let adaptation_needed = self.should_adapt(moq_metrics);

        if adaptation_needed {
            let new_settings = self.calculate_new_settings(moq_metrics);
            self.apply_settings(new_settings, AdaptationReason::BandwidthDecrease);
        }

        self.current_settings.clone()
    }

    /// Handle detected congestion
    pub fn handle_congestion(&mut self, congestion_level: CongestionLevel) {
        match congestion_level {
            CongestionLevel::None => {
                // No action needed
            }
            CongestionLevel::Light => {
                // Slight reduction in quality
                self.reduce_quality(0.9, AdaptationReason::CongestionDetected);
            }
            CongestionLevel::Moderate => {
                // Moderate reduction in quality
                self.reduce_quality(0.75, AdaptationReason::CongestionDetected);
            }
            CongestionLevel::Heavy => {
                // Aggressive reduction in quality
                self.reduce_quality(0.5, AdaptationReason::CongestionDetected);
            }
        }
    }

    /// Get current quality settings
    pub fn current_settings(&self) -> &QualitySettings {
        &self.current_settings
    }

    /// Get estimated bandwidth
    pub fn estimated_bandwidth(&self) -> u32 {
        self.bandwidth_estimator.estimated_bandwidth
    }

    /// Get current congestion level
    pub fn congestion_level(&self) -> &CongestionLevel {
        &self.congestion_detector.congestion_level
    }

    /// Manually set quality settings
    pub fn set_quality_settings(&mut self, settings: QualitySettings) {
        self.apply_settings(settings, AdaptationReason::Manual);
    }

    /// Get adaptation history
    pub fn adaptation_history(&self) -> &[QualityAdaptation] {
        &self.quality_adapter.adaptation_history
    }

    // Private helper methods

    fn should_adapt(&self, metrics: &MoqDeliveryMetrics) -> bool {
        let now = Instant::now();

        // Don't adapt too frequently
        if now.duration_since(self.quality_adapter.last_adaptation)
            < self.quality_adapter.min_adaptation_interval
        {
            return false;
        }

        // Check if significant change in delivery performance
        let loss_ratio = if metrics.objects_delivered + metrics.objects_lost > 0 {
            metrics.objects_lost as f32 / (metrics.objects_delivered + metrics.objects_lost) as f32
        } else {
            0.0
        };

        // Adapt if loss ratio exceeds threshold or delivery time is too high
        loss_ratio > self.config.congestion_threshold
            || metrics.avg_delivery_time
                > Duration::from_millis(self.config.target_buffer_ms as u64)
    }

    fn calculate_new_settings(&self, metrics: &MoqDeliveryMetrics) -> QualitySettings {
        let mut new_settings = self.current_settings.clone();

        // Calculate adaptation factor based on metrics
        let loss_ratio = if metrics.objects_delivered + metrics.objects_lost > 0 {
            metrics.objects_lost as f32 / (metrics.objects_delivered + metrics.objects_lost) as f32
        } else {
            0.0
        };

        let adaptation_factor = if loss_ratio > self.config.congestion_threshold {
            // Reduce quality
            1.0 - self.config.adaptation_step
        } else {
            // Increase quality (if bandwidth allows)
            1.0 + self.config.adaptation_step * 0.5 // More conservative increase
        };

        // Apply adaptation to bitrates
        new_settings.video_bitrate = ((new_settings.video_bitrate as f32 * adaptation_factor)
            as u32)
            .clamp(self.config.min_bitrate, self.config.max_bitrate);

        new_settings.audio_bitrate =
            ((new_settings.audio_bitrate as f32 * adaptation_factor) as u32).clamp(32_000, 128_000); // Audio bitrate limits

        // Adjust resolution if needed (for significant changes)
        if adaptation_factor < 0.8 {
            // Reduce resolution
            new_settings.video_width = (new_settings.video_width * 3 / 4).max(320);
            new_settings.video_height = (new_settings.video_height * 3 / 4).max(240);
        } else if adaptation_factor > 1.2 && new_settings.video_width < 1280 {
            // Increase resolution
            new_settings.video_width = (new_settings.video_width * 4 / 3).min(1920);
            new_settings.video_height = (new_settings.video_height * 4 / 3).min(1080);
        }

        new_settings
    }

    fn reduce_quality(&mut self, factor: f32, reason: AdaptationReason) {
        let mut new_settings = self.current_settings.clone();

        new_settings.video_bitrate =
            ((new_settings.video_bitrate as f32 * factor) as u32).max(self.config.min_bitrate);

        new_settings.audio_bitrate =
            ((new_settings.audio_bitrate as f32 * factor) as u32).max(32_000);

        // Reduce framerate for heavy congestion
        if factor < 0.7 {
            new_settings.video_framerate = (new_settings.video_framerate * 2 / 3).max(15);
        }

        self.apply_settings(new_settings, reason);
    }

    fn apply_settings(&mut self, new_settings: QualitySettings, reason: AdaptationReason) {
        let adaptation = QualityAdaptation {
            timestamp: Instant::now(),
            previous_settings: self.current_settings.clone(),
            new_settings: new_settings.clone(),
            reason,
        };

        self.quality_adapter.add_adaptation(adaptation);
        self.current_settings = new_settings;
    }
}

impl BandwidthEstimator {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            max_samples: 10,
            estimated_bandwidth: 1_000_000, // Start with 1 Mbps estimate
            last_update: Instant::now(),
        }
    }

    fn update_from_moq_metrics(&mut self, metrics: &MoqDeliveryMetrics) {
        let now = Instant::now();
        let duration_since_last = now.duration_since(self.last_update);

        if duration_since_last < Duration::from_millis(100) {
            return; // Don't update too frequently
        }

        // Estimate bytes transferred based on objects delivered
        // This is a rough estimate - in a real implementation, you'd track actual bytes
        let estimated_bytes = metrics.objects_delivered * 1000; // Assume 1KB per object average

        let sample = BandwidthSample {
            timestamp: now,
            bytes: estimated_bytes,
            duration: duration_since_last,
        };

        self.samples.push(sample);

        // Keep only recent samples
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }

        // Calculate new bandwidth estimate
        self.update_estimate();
        self.last_update = now;
    }

    fn update_estimate(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        // Calculate weighted average of recent samples
        let mut total_bytes = 0u64;
        let mut total_duration = Duration::ZERO;

        for sample in &self.samples {
            total_bytes += sample.bytes;
            total_duration += sample.duration;
        }

        if total_duration.as_secs_f64() > 0.0 {
            let bytes_per_second = total_bytes as f64 / total_duration.as_secs_f64();
            self.estimated_bandwidth = (bytes_per_second * 8.0) as u32; // Convert to bits per second
        }
    }
}

impl QualityAdapter {
    fn new() -> Self {
        Self {
            adaptation_history: Vec::new(),
            last_adaptation: Instant::now(),
            min_adaptation_interval: Duration::from_secs(2), // Minimum 2 seconds between adaptations
        }
    }

    fn add_adaptation(&mut self, adaptation: QualityAdaptation) {
        self.adaptation_history.push(adaptation);
        self.last_adaptation = Instant::now();

        // Keep only recent adaptations
        if self.adaptation_history.len() > 20 {
            self.adaptation_history.remove(0);
        }
    }
}

impl CongestionDetector {
    fn new() -> Self {
        Self {
            delivery_stats: Vec::new(),
            max_stats: 10,
            congestion_level: CongestionLevel::None,
            last_check: Instant::now(),
        }
    }

    fn update(&mut self, metrics: &MoqDeliveryMetrics) {
        self.delivery_stats.push(metrics.clone());

        // Keep only recent stats
        if self.delivery_stats.len() > self.max_stats {
            self.delivery_stats.remove(0);
        }

        // Update congestion level
        self.detect_congestion();
        self.last_check = Instant::now();
    }

    fn detect_congestion(&mut self) {
        if self.delivery_stats.is_empty() {
            return;
        }

        // Calculate average loss ratio over recent stats
        let mut total_delivered = 0u64;
        let mut total_lost = 0u64;
        let mut total_delivery_time = Duration::ZERO;

        for stats in &self.delivery_stats {
            total_delivered += stats.objects_delivered;
            total_lost += stats.objects_lost;
            total_delivery_time += stats.avg_delivery_time;
        }

        let loss_ratio = if total_delivered + total_lost > 0 {
            total_lost as f32 / (total_delivered + total_lost) as f32
        } else {
            0.0
        };

        let avg_delivery_time = total_delivery_time / self.delivery_stats.len() as u32;

        // Determine congestion level
        self.congestion_level =
            if loss_ratio > 0.15 || avg_delivery_time > Duration::from_millis(500) {
                CongestionLevel::Heavy
            } else if loss_ratio > 0.08 || avg_delivery_time > Duration::from_millis(300) {
                CongestionLevel::Moderate
            } else if loss_ratio > 0.03 || avg_delivery_time > Duration::from_millis(200) {
                CongestionLevel::Light
            } else {
                CongestionLevel::None
            };
    }
}

impl Default for QualityController {
    fn default() -> Self {
        Self::new()
    }
}
