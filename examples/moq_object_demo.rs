//! MoQ Object Handling and Delivery Demo
//! 
//! This example demonstrates the MoQ object creation, delivery, and caching
//! functionality implemented in task 3.2.

use quicrtc::{
    H264Frame, MoqCacheConfig, MoqObject, MoqObjectDelivery, MoqObjectStatus, 
    OpusFrame, TrackNamespace
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎬 MoQ Object Handling and Delivery Demo");
    println!("========================================\n");

    // Create track namespaces
    let video_namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/camera".to_string(),
    };
    
    let audio_namespace = TrackNamespace {
        namespace: "conference.example.com".to_string(),
        track_name: "alice/microphone".to_string(),
    };

    // Demo 1: Create MoQ objects from media frames
    println!("📹 Demo 1: Creating MoQ Objects from Media Frames");
    println!("--------------------------------------------------");
    
    // Create H.264 video frame (keyframe)
    let h264_keyframe = H264Frame {
        nal_units: vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x80, 0x1E], // Sample SPS NAL unit
        is_keyframe: true,
        timestamp_us: 1_000_000, // 1 second
        sequence_number: 1,
    };
    
    let video_object = MoqObject::from_h264_frame(video_namespace.clone(), h264_keyframe);
    println!("✅ Created H.264 keyframe MoQ object:");
    println!("   - Track: {}/{}", video_object.track_namespace.namespace, video_object.track_namespace.track_name);
    println!("   - Group ID: {} (timestamp in ms)", video_object.group_id);
    println!("   - Object ID: {}", video_object.object_id);
    println!("   - Priority: {} (keyframes have priority 1)", video_object.publisher_priority);
    println!("   - Size: {} bytes", video_object.size);
    println!("   - Delivery Priority: {}", video_object.delivery_priority());
    
    // Create H.264 video frame (non-keyframe)
    let h264_frame = H264Frame {
        nal_units: vec![0x00, 0x00, 0x00, 0x01, 0x41, 0xE0], // Sample P-frame NAL unit
        is_keyframe: false,
        timestamp_us: 1_033_333, // ~30fps later
        sequence_number: 2,
    };
    
    let video_object2 = MoqObject::from_h264_frame(video_namespace.clone(), h264_frame);
    println!("✅ Created H.264 P-frame MoQ object:");
    println!("   - Priority: {} (P-frames have priority 2)", video_object2.publisher_priority);
    
    // Create Opus audio frame
    let opus_frame = OpusFrame {
        opus_data: vec![0xFC, 0xFF, 0xFE, 0x00, 0x01, 0x02], // Sample Opus data
        timestamp_us: 20_000, // 20ms
        sequence_number: 100,
        sample_rate: 48000,
        channels: 2,
    };
    
    let audio_object = MoqObject::from_opus_frame(audio_namespace.clone(), opus_frame);
    println!("✅ Created Opus audio MoQ object:");
    println!("   - Track: {}/{}", audio_object.track_namespace.namespace, audio_object.track_namespace.track_name);
    println!("   - Group ID: {} (20ms audio groups)", audio_object.group_id);
    println!("   - Priority: {} (audio always priority 1)", audio_object.publisher_priority);
    println!("   - Size: {} bytes", audio_object.size);

    // Demo 2: Object delivery system with prioritization
    println!("\n🚀 Demo 2: Object Delivery System with Prioritization");
    println!("-----------------------------------------------------");
    
    let cache_config = MoqCacheConfig {
        max_size_bytes: 1024 * 1024, // 1MB cache
        max_objects_per_track: 100,
        object_ttl: std::time::Duration::from_secs(30),
        enable_lru_eviction: true,
    };
    
    let mut delivery_system = MoqObjectDelivery::new(cache_config);
    
    // Create objects with different priorities
    let end_of_track = MoqObject::end_of_track(
        video_namespace.clone(),
        "video".to_string(),
        1000,
        999,
    );
    
    let end_of_group = MoqObject::end_of_group(
        video_namespace.clone(),
        "video".to_string(),
        1000,
        100,
    );
    
    // Enqueue objects in random order
    println!("📤 Enqueueing objects in random order...");
    delivery_system.enqueue_object(video_object2.clone())?; // Priority 2
    delivery_system.enqueue_object(audio_object.clone())?;  // Priority 1
    delivery_system.enqueue_object(end_of_track.clone())?;  // Priority 0 (highest)
    delivery_system.enqueue_object(video_object.clone())?; // Priority 1
    delivery_system.enqueue_object(end_of_group.clone())?; // Priority 1
    
    println!("✅ Enqueued 5 objects, queue depth: {}", delivery_system.delivery_stats().queue_depth);
    
    // Dequeue objects - should come out in priority order
    println!("\n📥 Dequeuing objects (should be in priority order):");
    let mut dequeue_order = Vec::new();
    while let Some(object) = delivery_system.dequeue_object() {
        let priority = object.delivery_priority();
        let status = match object.object_status {
            MoqObjectStatus::Normal => "Normal",
            MoqObjectStatus::EndOfGroup => "EndOfGroup",
            MoqObjectStatus::EndOfTrack => "EndOfTrack",
        };
        println!("   - Object ID {}: Priority {}, Status: {}", object.object_id, priority, status);
        dequeue_order.push(priority);
    }
    
    // Verify priority ordering (should be ascending: 0, 1, 1, 1, 2)
    let is_sorted = dequeue_order.windows(2).all(|w| w[0] <= w[1]);
    println!("✅ Objects dequeued in correct priority order: {}", is_sorted);

    // Demo 3: Object caching system
    println!("\n💾 Demo 3: Object Caching System");
    println!("--------------------------------");
    
    // Re-enqueue some objects to test caching
    delivery_system.enqueue_object(video_object.clone())?;
    delivery_system.enqueue_object(audio_object.clone())?;
    
    println!("📊 Cache statistics after enqueueing:");
    let cache_stats = delivery_system.cache_stats();
    println!("   - Total objects cached: {}", cache_stats.total_objects);
    println!("   - Current cache size: {} bytes", cache_stats.current_size_bytes);
    println!("   - Cache hits: {}", cache_stats.cache_hits);
    println!("   - Cache misses: {}", cache_stats.cache_misses);
    
    // Test cache retrieval
    let cached_video = delivery_system.get_cached_object(&video_namespace, 1);
    let cached_audio = delivery_system.get_cached_object(&audio_namespace, 100);
    let not_found = delivery_system.get_cached_object(&video_namespace, 999);
    
    println!("🔍 Cache retrieval test:");
    println!("   - Video object (ID 1): {}", if cached_video.is_some() { "Found ✅" } else { "Not found ❌" });
    println!("   - Audio object (ID 100): {}", if cached_audio.is_some() { "Found ✅" } else { "Not found ❌" });
    println!("   - Non-existent object (ID 999): {}", if not_found.is_some() { "Found ❌" } else { "Not found ✅" });
    
    let final_cache_stats = delivery_system.cache_stats();
    println!("📊 Final cache statistics:");
    println!("   - Cache hits: {}", final_cache_stats.cache_hits);
    println!("   - Cache misses: {}", final_cache_stats.cache_misses);
    println!("   - Hit ratio: {:.1}%", 
        (final_cache_stats.cache_hits as f64 / (final_cache_stats.cache_hits + final_cache_stats.cache_misses) as f64) * 100.0);

    // Demo 4: Congestion control - drop low priority objects
    println!("\n🚦 Demo 4: Congestion Control - Drop Low Priority Objects");
    println!("--------------------------------------------------------");
    
    // Fill queue with mixed priority objects
    for i in 1..=10 {
        let priority = (i % 4) + 1; // Priorities 1-4
        let object = MoqObject {
            track_namespace: video_namespace.clone(),
            track_name: "video".to_string(),
            group_id: 2000,
            object_id: i,
            publisher_priority: priority as u8,
            payload: vec![i as u8; 10], // 10 bytes each
            object_status: MoqObjectStatus::Normal,
            created_at: std::time::Instant::now(),
            size: 10,
        };
        delivery_system.enqueue_object(object)?;
    }
    
    println!("📤 Enqueued 10 objects with mixed priorities (1-4)");
    println!("   - Queue depth before congestion control: {}", delivery_system.delivery_stats().queue_depth);
    
    // Drop objects with priority > 2 (simulate congestion)
    let dropped_count = delivery_system.drop_low_priority_objects(2);
    println!("🗑️  Dropped {} objects with priority > 2", dropped_count);
    println!("   - Queue depth after congestion control: {}", delivery_system.delivery_stats().queue_depth);
    
    let final_stats = delivery_system.delivery_stats();
    println!("📊 Final delivery statistics:");
    println!("   - Objects delivered: {}", final_stats.objects_delivered);
    println!("   - Objects dropped: {}", final_stats.objects_dropped);
    println!("   - Peak queue depth: {}", final_stats.peak_queue_depth);
    println!("   - Average delivery latency: {:.2}ms", final_stats.avg_delivery_latency_ms);

    println!("\n🎉 Demo completed successfully!");
    println!("   ✅ MoQ object creation from H.264 and Opus frames");
    println!("   ✅ Priority-based object delivery system");
    println!("   ✅ Object caching with LRU eviction");
    println!("   ✅ Congestion control with priority dropping");
    
    Ok(())
}