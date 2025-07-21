//! IETF MoQ Wire Format Demo
//!
//! This example demonstrates the IETF Media over QUIC (MoQ) wire format implementation
//! including control message encoding/decoding and data stream/datagram encoding.
//!
//! To run: cargo run --example moq_wire_format_demo

use bytes::BytesMut;
use quicrtc_core::{
    MoqCapabilities, MoqControlMessage, MoqObject, MoqObjectStatus, MoqTrack, MoqTrackType,
    MoqWireFormat, TrackNamespace,
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 IETF MoQ Wire Format Demo");
    println!("=====================================\n");

    // Demo 1: Variable-length integer encoding
    demo_varint_encoding()?;

    // Demo 2: Control message encoding
    demo_control_messages()?;

    // Demo 3: Object data encoding (streams and datagrams)
    demo_object_encoding()?;

    // Demo 4: Complete MoQ session simulation
    demo_session_simulation()?;

    println!("✅ All MoQ wire format demos completed successfully!");
    Ok(())
}

fn demo_varint_encoding() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔢 Demo 1: Variable-length Integer Encoding");
    println!("-------------------------------------------");

    let test_values = vec![
        0u64,          // 1 byte
        63,            // 1 byte (max for 6-bit)
        64,            // 2 bytes
        16383,         // 2 bytes (max for 14-bit)
        16384,         // 4 bytes
        1073741823,    // 4 bytes (max for 30-bit)
        1073741824,    // 8 bytes
        u64::MAX >> 2, // 8 bytes (max for 62-bit)
    ];

    for value in test_values {
        let mut buf = BytesMut::new();
        MoqWireFormat::encode_varint(value, &mut buf);

        println!(
            "Value: {:<12} -> {} bytes: {:02X?}",
            value,
            buf.len(),
            buf.as_ref()
        );

        // Verify round-trip encoding
        let mut cursor = std::io::Cursor::new(buf.as_ref());
        let decoded = MoqWireFormat::decode_varint(&mut cursor)?;
        assert_eq!(value, decoded);
    }

    println!("✅ Variable-length integer encoding/decoding verified\n");
    Ok(())
}

fn demo_control_messages() -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Demo 2: MoQ Control Message Encoding");
    println!("---------------------------------------");

    // Create sample capabilities
    let capabilities = MoqCapabilities {
        version: 1,
        max_tracks: 100,
        supported_track_types: vec![MoqTrackType::Audio, MoqTrackType::Video, MoqTrackType::Data],
        max_object_size: 10 * 1024 * 1024, // 10MB
        supports_caching: true,
    };

    // Demo CLIENT_SETUP message
    let setup_msg = MoqControlMessage::Setup {
        version: 1,
        capabilities: capabilities.clone(),
    };

    let mut buf = BytesMut::new();
    MoqWireFormat::encode_control_message(&setup_msg, &mut buf)?;
    println!("CLIENT_SETUP encoded: {} bytes", buf.len());
    println!("Raw bytes: {:02X?}", &buf[..std::cmp::min(32, buf.len())]);

    // Decode and verify
    let decoded = MoqWireFormat::decode_control_message(&buf)?;
    match decoded {
        MoqControlMessage::Setup { version, .. } => {
            println!("✅ Decoded CLIENT_SETUP: version = {}", version);
        }
        _ => return Err("Unexpected message type".into()),
    }

    // Demo ANNOUNCE message
    let track_ns = TrackNamespace {
        namespace: "example.com".to_string(),
        track_name: "live/camera1".to_string(),
    };

    let track = MoqTrack {
        namespace: track_ns.clone(),
        name: "high-quality-stream".to_string(),
        track_type: MoqTrackType::Video,
    };

    let announce_msg = MoqControlMessage::Announce {
        track_namespace: track_ns,
        track,
    };

    buf.clear();
    MoqWireFormat::encode_control_message(&announce_msg, &mut buf)?;
    println!("ANNOUNCE encoded: {} bytes", buf.len());

    let decoded = MoqWireFormat::decode_control_message(&buf)?;
    match decoded {
        MoqControlMessage::Announce {
            track_namespace, ..
        } => {
            println!(
                "✅ Decoded ANNOUNCE: namespace = '{}', track = '{}'",
                track_namespace.namespace, track_namespace.track_name
            );
        }
        _ => return Err("Unexpected message type".into()),
    }

    // Demo SUBSCRIBE message
    let subscribe_msg = MoqControlMessage::Subscribe {
        track_namespace: TrackNamespace {
            namespace: "example.com".to_string(),
            track_name: "live/camera1".to_string(),
        },
        priority: 5,
        start_group: Some(100),
        end_group: None, // Subscribe to all future groups
    };

    buf.clear();
    MoqWireFormat::encode_control_message(&subscribe_msg, &mut buf)?;
    println!("SUBSCRIBE encoded: {} bytes", buf.len());

    let decoded = MoqWireFormat::decode_control_message(&buf)?;
    match decoded {
        MoqControlMessage::Subscribe {
            priority,
            start_group,
            end_group,
            ..
        } => {
            println!(
                "✅ Decoded SUBSCRIBE: priority = {}, start_group = {:?}, end_group = {:?}",
                priority, start_group, end_group
            );
        }
        _ => return Err("Unexpected message type".into()),
    }

    println!("✅ Control message encoding/decoding verified\n");
    Ok(())
}

fn demo_object_encoding() -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Demo 3: MoQ Object Data Encoding");
    println!("-----------------------------------");

    // Create a sample media object
    let media_data = create_sample_media_data();
    let object = MoqObject {
        track_namespace: TrackNamespace {
            namespace: "example.com".to_string(),
            track_name: "live/camera1".to_string(),
        },
        track_name: "high-quality-stream".to_string(),
        group_id: 42,
        object_id: 1,
        publisher_priority: 5,
        payload: media_data,
        object_status: MoqObjectStatus::Normal,
        created_at: Instant::now(),
        size: 1024,
    };

    let track_alias = 123u64;

    // Demo 3.1: Stream encoding (Section 9.4 of MoQ spec)
    println!("📊 Stream Encoding (for reliable delivery):");
    let mut stream_buf = BytesMut::new();
    MoqWireFormat::encode_object_stream(&object, track_alias, &mut stream_buf)?;
    println!("Stream encoded: {} bytes", stream_buf.len());
    println!("Header bytes: {:02X?}", &stream_buf[..32]);

    // Decode stream object
    let (decoded_alias, decoded_object) = MoqWireFormat::decode_object_stream(&stream_buf)?;
    println!(
        "✅ Decoded stream: track_alias = {}, group_id = {}, object_id = {}, payload_size = {}",
        decoded_alias,
        decoded_object.group_id,
        decoded_object.object_id,
        decoded_object.payload.len()
    );

    // Demo 3.2: Datagram encoding (Section 9.3 of MoQ spec)
    println!("\n📡 Datagram Encoding (for low-latency delivery):");
    let mut datagram_buf = BytesMut::new();
    MoqWireFormat::encode_object_datagram(&object, track_alias, &mut datagram_buf)?;
    println!("Datagram encoded: {} bytes", datagram_buf.len());
    println!("Header bytes: {:02X?}", &datagram_buf[..32]);

    // Decode datagram object
    let (decoded_alias, decoded_object) = MoqWireFormat::decode_object_datagram(&datagram_buf)?;
    println!(
        "✅ Decoded datagram: track_alias = {}, group_id = {}, object_id = {}, payload_size = {}",
        decoded_alias,
        decoded_object.group_id,
        decoded_object.object_id,
        decoded_object.payload.len()
    );

    // Compare encoding sizes
    println!("\n📊 Encoding Comparison:");
    println!(
        "Stream encoding:   {} bytes (includes length prefix)",
        stream_buf.len()
    );
    println!(
        "Datagram encoding: {} bytes (no length prefix)",
        datagram_buf.len()
    );
    println!(
        "Difference:        {} bytes",
        stream_buf.len() as i32 - datagram_buf.len() as i32
    );

    println!("✅ Object encoding/decoding verified\n");
    Ok(())
}

fn demo_session_simulation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 Demo 4: Complete MoQ Session Simulation");
    println!("------------------------------------------");

    println!("🤝 Simulating MoQ session establishment...");

    // Step 1: Client setup
    let client_setup = MoqControlMessage::Setup {
        version: 1,
        capabilities: MoqCapabilities {
            version: 1,
            max_tracks: 50,
            supported_track_types: vec![MoqTrackType::Audio, MoqTrackType::Video],
            max_object_size: 5 * 1024 * 1024,
            supports_caching: true,
        },
    };

    let mut buf = BytesMut::new();
    MoqWireFormat::encode_control_message(&client_setup, &mut buf)?;
    println!("📤 Client sends CLIENT_SETUP: {} bytes", buf.len());

    // Step 2: Server response
    let server_setup = MoqControlMessage::SetupOk {
        version: 1,
        capabilities: MoqCapabilities {
            version: 1,
            max_tracks: 100,
            supported_track_types: vec![
                MoqTrackType::Audio,
                MoqTrackType::Video,
                MoqTrackType::Data,
            ],
            max_object_size: 10 * 1024 * 1024,
            supports_caching: true,
        },
    };

    buf.clear();
    MoqWireFormat::encode_control_message(&server_setup, &mut buf)?;
    println!("📥 Server responds SERVER_SETUP: {} bytes", buf.len());

    // Step 3: Publisher announces track
    let announce = MoqControlMessage::Announce {
        track_namespace: TrackNamespace {
            namespace: "sports.tv".to_string(),
            track_name: "football/live/field1".to_string(),
        },
        track: MoqTrack {
            namespace: TrackNamespace {
                namespace: "sports.tv".to_string(),
                track_name: "football/live/field1".to_string(),
            },
            name: "4k-main-feed".to_string(),
            track_type: MoqTrackType::Video,
        },
    };

    buf.clear();
    MoqWireFormat::encode_control_message(&announce, &mut buf)?;
    println!("📢 Publisher announces track: {} bytes", buf.len());

    // Step 4: Subscriber subscribes
    let subscribe = MoqControlMessage::Subscribe {
        track_namespace: TrackNamespace {
            namespace: "sports.tv".to_string(),
            track_name: "football/live/field1".to_string(),
        },
        priority: 1,             // High priority for live sports
        start_group: Some(1000), // Start from current position
        end_group: None,         // Subscribe indefinitely
    };

    buf.clear();
    MoqWireFormat::encode_control_message(&subscribe, &mut buf)?;
    println!("🔔 Subscriber subscribes: {} bytes", buf.len());

    // Step 5: Data transmission simulation
    println!("\n📺 Simulating live video data transmission...");

    let video_frames = vec![
        ("I-Frame", 8192, MoqObjectStatus::Normal), // Key frame
        ("P-Frame", 2048, MoqObjectStatus::Normal), // Predicted frame
        ("P-Frame", 1536, MoqObjectStatus::Normal), // Predicted frame
        ("P-Frame", 1024, MoqObjectStatus::EndOfGroup), // Last frame of GOP
    ];

    let mut total_bytes = 0;
    for (frame_idx, (frame_type, size, status)) in video_frames.iter().enumerate() {
        let frame_object = MoqObject {
            track_namespace: TrackNamespace {
                namespace: "sports.tv".to_string(),
                track_name: "football/live/field1".to_string(),
            },
            track_name: "4k-main-feed".to_string(),
            group_id: 1001,
            object_id: frame_idx as u64,
            publisher_priority: if frame_type == &"I-Frame" { 1 } else { 2 }, // I-frames higher priority
            payload: vec![0u8; *size], // Simulated video data
            object_status: status.clone(),
            created_at: Instant::now(),
            size: *size,
        };

        // Use datagram for low latency
        buf.clear();
        MoqWireFormat::encode_object_datagram(&frame_object, 42, &mut buf)?;
        total_bytes += buf.len();

        println!(
            "🎥 {} #{}: {} bytes (priority: {}, status: {:?})",
            frame_type,
            frame_idx,
            buf.len(),
            frame_object.publisher_priority,
            status
        );
    }

    println!("\n📊 Session Statistics:");
    println!("Total control messages: 4");
    println!("Total video frames: 4");
    println!("Total bytes transmitted: {} bytes", total_bytes);
    println!("✅ MoQ session simulation completed successfully");

    Ok(())
}

fn create_sample_media_data() -> Vec<u8> {
    // Create sample media data (could be video frame, audio packet, etc.)
    let mut data = Vec::with_capacity(1024);

    // Simulate a compressed video frame header
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // NAL unit start code
    data.extend_from_slice(&[0x67]); // SPS NAL unit type

    // Add some random-ish data to simulate compressed media
    for i in 0..1019 {
        data.push(((i * 17 + 42) % 256) as u8);
    }

    data
}
 