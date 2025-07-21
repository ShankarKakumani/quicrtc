//! IETF MoQ Wire Format Implementation
//!
//! This module implements the wire format for Media over QUIC (MoQ) protocol
//! according to draft-ietf-moq-transport-13 specification.
//!
//! The implementation provides binary encoding and decoding for:
//! - Control messages (Section 8 of the spec)
//! - Data streams and datagrams (Section 9 of the spec)
//! - Variable-length integer encoding (from QUIC RFC 9000)

use crate::error::QuicRtcError;
use crate::moq::{MoqControlMessage, MoqObject, TrackNamespace};
use bytes::{Buf, BufMut, BytesMut};
use std::io::Cursor;

/// MoQ Wire Format encoder/decoder
#[derive(Debug)]
pub struct MoqWireFormat;

/// Variable-length integer encoding following QUIC specification (RFC 9000, Section 16)
impl MoqWireFormat {
    /// Encode a variable-length integer
    pub fn encode_varint(value: u64, buf: &mut BytesMut) {
        if value < 0x40 {
            // 6-bit value, 1 byte
            buf.put_u8(value as u8);
        } else if value < 0x4000 {
            // 14-bit value, 2 bytes
            buf.put_u16((0x4000 | value) as u16);
        } else if value < 0x40000000 {
            // 30-bit value, 4 bytes
            buf.put_u32((0x80000000 | value) as u32);
        } else if value < 0x4000000000000000 {
            // 62-bit value, 8 bytes
            buf.put_u64(0xC000000000000000 | value);
        } else {
            // Value too large for varint encoding
            panic!("Value too large for varint encoding: {}", value);
        }
    }

    /// Decode a variable-length integer
    pub fn decode_varint(buf: &mut Cursor<&[u8]>) -> Result<u64, QuicRtcError> {
        if !buf.has_remaining() {
            return Err(QuicRtcError::InvalidData {
                reason: "No data available for varint".to_string(),
            });
        }

        let first_byte = buf.get_u8();
        let prefix = first_byte >> 6;

        match prefix {
            0 => {
                // 6-bit value
                Ok((first_byte & 0x3F) as u64)
            }
            1 => {
                // 14-bit value
                if !buf.has_remaining() {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for 14-bit varint".to_string(),
                    });
                }
                let second_byte = buf.get_u8();
                Ok((((first_byte & 0x3F) as u64) << 8) | (second_byte as u64))
            }
            2 => {
                // 30-bit value
                if buf.remaining() < 3 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for 30-bit varint".to_string(),
                    });
                }
                let remaining = buf.get_uint(3) as u64;
                Ok((((first_byte & 0x3F) as u64) << 24) | remaining)
            }
            3 => {
                // 62-bit value
                if buf.remaining() < 7 {
                    return Err(QuicRtcError::InvalidData {
                        reason: "Insufficient data for 62-bit varint".to_string(),
                    });
                }
                let remaining = buf.get_uint(7) as u64;
                Ok((((first_byte & 0x3F) as u64) << 56) | remaining)
            }
            _ => unreachable!(),
        }
    }

    /// Encode a length-prefixed byte string
    pub fn encode_bytes(data: &[u8], buf: &mut BytesMut) {
        Self::encode_varint(data.len() as u64, buf);
        buf.extend_from_slice(data);
    }

    /// Decode a length-prefixed byte string
    pub fn decode_bytes(buf: &mut Cursor<&[u8]>) -> Result<Vec<u8>, QuicRtcError> {
        let length = Self::decode_varint(buf)? as usize;

        if buf.remaining() < length {
            return Err(QuicRtcError::InvalidData {
                reason: format!(
                    "Insufficient data: need {} bytes, have {}",
                    length,
                    buf.remaining()
                ),
            });
        }

        let mut data = vec![0u8; length];
        buf.copy_to_slice(&mut data);
        Ok(data)
    }
}

/// Control Message Wire Format (Section 8 of MoQ spec)
impl MoqWireFormat {
    /// Encode a control message
    pub fn encode_control_message(
        message: &MoqControlMessage,
        buf: &mut BytesMut,
    ) -> Result<(), QuicRtcError> {
        match message {
            MoqControlMessage::Setup {
                version,
                capabilities,
            } => {
                Self::encode_varint(0x20, buf); // CLIENT_SETUP message type

                // Encode version
                Self::encode_varint(*version as u64, buf);

                // Encode capabilities (simplified - would need full parameter encoding)
                Self::encode_varint(capabilities.max_tracks as u64, buf);
                Self::encode_varint(capabilities.max_object_size, buf);
            }

            MoqControlMessage::SetupOk {
                version,
                capabilities,
            } => {
                Self::encode_varint(0x21, buf); // SERVER_SETUP message type

                // Encode version
                Self::encode_varint(*version as u64, buf);

                // Encode capabilities
                Self::encode_varint(capabilities.max_tracks as u64, buf);
                Self::encode_varint(capabilities.max_object_size, buf);
            }

            MoqControlMessage::Announce {
                track_namespace,
                track,
            } => {
                Self::encode_varint(0x06, buf); // ANNOUNCE message type

                // Encode track namespace
                Self::encode_track_namespace(track_namespace, buf)?;

                // Encode track name
                Self::encode_bytes(track.name.as_bytes(), buf);
            }

            MoqControlMessage::AnnounceOk { track_namespace } => {
                Self::encode_varint(0x07, buf); // ANNOUNCE_OK message type
                Self::encode_track_namespace(track_namespace, buf)?;
            }

            MoqControlMessage::Subscribe {
                track_namespace,
                priority,
                start_group,
                end_group,
            } => {
                Self::encode_varint(0x03, buf); // SUBSCRIBE message type

                // Encode request ID (simplified)
                Self::encode_varint(1, buf);

                // Encode track namespace
                Self::encode_track_namespace(track_namespace, buf)?;

                // Encode subscription parameters
                Self::encode_varint(*priority as u64, buf);

                // Encode group range
                match start_group {
                    Some(start) => {
                        Self::encode_varint(1, buf); // Has start group
                        Self::encode_varint(*start, buf);
                    }
                    None => {
                        Self::encode_varint(0, buf); // No start group
                    }
                }

                match end_group {
                    Some(end) => {
                        Self::encode_varint(1, buf); // Has end group
                        Self::encode_varint(*end, buf);
                    }
                    None => {
                        Self::encode_varint(0, buf); // No end group
                    }
                }
            }

            MoqControlMessage::SubscribeOk { track_namespace } => {
                Self::encode_varint(0x04, buf); // SUBSCRIBE_OK message type

                // Encode request ID
                Self::encode_varint(1, buf);

                // Encode track namespace
                Self::encode_track_namespace(track_namespace, buf)?;
            }

            MoqControlMessage::Unsubscribe { track_namespace } => {
                Self::encode_varint(0x0A, buf); // UNSUBSCRIBE message type

                // Encode request ID
                Self::encode_varint(1, buf);

                // Encode track namespace
                Self::encode_track_namespace(track_namespace, buf)?;
            }

            MoqControlMessage::Terminate { code, reason } => {
                Self::encode_varint(0x10, buf); // GOAWAY message type (closest to terminate)

                // Encode termination code
                Self::encode_varint(*code as u64, buf);

                // Encode reason phrase
                Self::encode_bytes(reason.as_bytes(), buf);
            }

            _ => {
                return Err(QuicRtcError::MoqProtocol {
                    reason: "Unsupported control message type for encoding".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Decode a control message
    pub fn decode_control_message(data: &[u8]) -> Result<MoqControlMessage, QuicRtcError> {
        let mut buf = Cursor::new(data);

        let message_type = Self::decode_varint(&mut buf)?;

        match message_type {
            0x20 => {
                // CLIENT_SETUP
                let version = Self::decode_varint(&mut buf)? as u32;
                let max_tracks = Self::decode_varint(&mut buf)? as u32;
                let max_object_size = Self::decode_varint(&mut buf)?;

                Ok(MoqControlMessage::Setup {
                    version,
                    capabilities: crate::moq::MoqCapabilities {
                        version,
                        max_tracks,
                        supported_track_types: vec![
                            crate::moq::MoqTrackType::Audio,
                            crate::moq::MoqTrackType::Video,
                            crate::moq::MoqTrackType::Data,
                        ],
                        max_object_size,
                        supports_caching: true,
                    },
                })
            }

            0x21 => {
                // SERVER_SETUP
                let version = Self::decode_varint(&mut buf)? as u32;
                let max_tracks = Self::decode_varint(&mut buf)? as u32;
                let max_object_size = Self::decode_varint(&mut buf)?;

                Ok(MoqControlMessage::SetupOk {
                    version,
                    capabilities: crate::moq::MoqCapabilities {
                        version,
                        max_tracks,
                        supported_track_types: vec![
                            crate::moq::MoqTrackType::Audio,
                            crate::moq::MoqTrackType::Video,
                            crate::moq::MoqTrackType::Data,
                        ],
                        max_object_size,
                        supports_caching: true,
                    },
                })
            }

            0x06 => {
                // ANNOUNCE
                let track_namespace = Self::decode_track_namespace(&mut buf)?;
                let track_name_bytes = Self::decode_bytes(&mut buf)?;
                let track_name =
                    String::from_utf8(track_name_bytes).map_err(|_| QuicRtcError::InvalidData {
                        reason: "Invalid UTF-8 in track name".to_string(),
                    })?;

                Ok(MoqControlMessage::Announce {
                    track_namespace: track_namespace.clone(),
                    track: crate::moq::MoqTrack {
                        namespace: track_namespace,
                        name: track_name,
                        track_type: crate::moq::MoqTrackType::Data, // Default
                    },
                })
            }

            0x07 => {
                // ANNOUNCE_OK
                let track_namespace = Self::decode_track_namespace(&mut buf)?;
                Ok(MoqControlMessage::AnnounceOk { track_namespace })
            }

            0x03 => {
                // SUBSCRIBE
                let _request_id = Self::decode_varint(&mut buf)?;
                let track_namespace = Self::decode_track_namespace(&mut buf)?;
                let priority = Self::decode_varint(&mut buf)? as u8;

                // Decode group range
                let start_group = if Self::decode_varint(&mut buf)? == 1 {
                    Some(Self::decode_varint(&mut buf)?)
                } else {
                    None
                };

                let end_group = if Self::decode_varint(&mut buf)? == 1 {
                    Some(Self::decode_varint(&mut buf)?)
                } else {
                    None
                };

                Ok(MoqControlMessage::Subscribe {
                    track_namespace,
                    priority,
                    start_group,
                    end_group,
                })
            }

            0x04 => {
                // SUBSCRIBE_OK
                let _request_id = Self::decode_varint(&mut buf)?;
                let track_namespace = Self::decode_track_namespace(&mut buf)?;
                Ok(MoqControlMessage::SubscribeOk { track_namespace })
            }

            0x0A => {
                // UNSUBSCRIBE
                let _request_id = Self::decode_varint(&mut buf)?;
                let track_namespace = Self::decode_track_namespace(&mut buf)?;
                Ok(MoqControlMessage::Unsubscribe { track_namespace })
            }

            0x10 => {
                // GOAWAY (treated as terminate)
                let code = Self::decode_varint(&mut buf)? as u32;
                let reason_bytes = Self::decode_bytes(&mut buf)?;
                let reason =
                    String::from_utf8(reason_bytes).map_err(|_| QuicRtcError::InvalidData {
                        reason: "Invalid UTF-8 in reason phrase".to_string(),
                    })?;

                Ok(MoqControlMessage::Terminate { code, reason })
            }

            _ => Err(QuicRtcError::MoqProtocol {
                reason: format!("Unknown control message type: {}", message_type),
            }),
        }
    }

    /// Encode track namespace according to MoQ specification
    fn encode_track_namespace(
        namespace: &TrackNamespace,
        buf: &mut BytesMut,
    ) -> Result<(), QuicRtcError> {
        // Encode namespace tuple
        Self::encode_bytes(namespace.namespace.as_bytes(), buf);

        // Encode track name
        Self::encode_bytes(namespace.track_name.as_bytes(), buf);

        Ok(())
    }

    /// Decode track namespace according to MoQ specification
    fn decode_track_namespace(buf: &mut Cursor<&[u8]>) -> Result<TrackNamespace, QuicRtcError> {
        // Decode namespace
        let namespace_bytes = Self::decode_bytes(buf)?;
        let namespace =
            String::from_utf8(namespace_bytes).map_err(|_| QuicRtcError::InvalidData {
                reason: "Invalid UTF-8 in namespace".to_string(),
            })?;

        // Decode track name
        let track_name_bytes = Self::decode_bytes(buf)?;
        let track_name =
            String::from_utf8(track_name_bytes).map_err(|_| QuicRtcError::InvalidData {
                reason: "Invalid UTF-8 in track name".to_string(),
            })?;

        Ok(TrackNamespace {
            namespace,
            track_name,
        })
    }
}

/// Data Stream and Datagram Wire Format (Section 9 of MoQ spec)
impl MoqWireFormat {
    /// Encode MoQ object for data stream transmission
    pub fn encode_object_stream(
        object: &MoqObject,
        track_alias: u64,
        buf: &mut BytesMut,
    ) -> Result<(), QuicRtcError> {
        // Subgroup Header format (Section 9.4.2)
        Self::encode_varint(track_alias, buf);
        Self::encode_varint(object.group_id, buf);
        Self::encode_varint(object.object_id, buf); // Subgroup ID (simplified)
        Self::encode_varint(object.publisher_priority as u64, buf);

        // Object Header within subgroup
        Self::encode_varint(object.object_id, buf);
        Self::encode_varint(object.payload.len() as u64, buf);

        // Object status
        let status = match object.object_status {
            crate::moq::MoqObjectStatus::Normal => 0u8,
            crate::moq::MoqObjectStatus::EndOfGroup => 1u8,
            crate::moq::MoqObjectStatus::EndOfTrack => 2u8,
        };
        buf.put_u8(status);

        // Object payload
        buf.extend_from_slice(&object.payload);

        Ok(())
    }

    /// Decode MoQ object from data stream
    pub fn decode_object_stream(data: &[u8]) -> Result<(u64, MoqObject), QuicRtcError> {
        let mut buf = Cursor::new(data);

        // Decode subgroup header
        let track_alias = Self::decode_varint(&mut buf)?;
        let group_id = Self::decode_varint(&mut buf)?;
        let _subgroup_id = Self::decode_varint(&mut buf)?;
        let publisher_priority = Self::decode_varint(&mut buf)? as u8;

        // Decode object header
        let object_id = Self::decode_varint(&mut buf)?;
        let payload_length = Self::decode_varint(&mut buf)? as usize;

        if buf.remaining() < payload_length + 1 {
            return Err(QuicRtcError::InvalidData {
                reason: "Insufficient data for object payload".to_string(),
            });
        }

        // Decode object status
        let status_byte = buf.get_u8();
        let object_status = match status_byte {
            0 => crate::moq::MoqObjectStatus::Normal,
            1 => crate::moq::MoqObjectStatus::EndOfGroup,
            2 => crate::moq::MoqObjectStatus::EndOfTrack,
            _ => {
                return Err(QuicRtcError::InvalidData {
                    reason: format!("Invalid object status: {}", status_byte),
                });
            }
        };

        // Decode payload
        let mut payload = vec![0u8; payload_length];
        buf.copy_to_slice(&mut payload);

        let object = MoqObject {
            track_namespace: TrackNamespace {
                namespace: "default".to_string(), // Would be resolved by track alias
                track_name: "unknown".to_string(),
            },
            track_name: "unknown".to_string(),
            group_id,
            object_id,
            publisher_priority,
            payload,
            object_status,
            created_at: std::time::Instant::now(),
            size: payload_length,
        };

        Ok((track_alias, object))
    }

    /// Encode MoQ object for datagram transmission (Section 9.3)
    pub fn encode_object_datagram(
        object: &MoqObject,
        track_alias: u64,
        buf: &mut BytesMut,
    ) -> Result<(), QuicRtcError> {
        // Object Datagram format
        Self::encode_varint(track_alias, buf);
        Self::encode_varint(object.group_id, buf);
        Self::encode_varint(object.object_id, buf);
        Self::encode_varint(object.publisher_priority as u64, buf);

        // Object status
        let status = match object.object_status {
            crate::moq::MoqObjectStatus::Normal => 0u8,
            crate::moq::MoqObjectStatus::EndOfGroup => 1u8,
            crate::moq::MoqObjectStatus::EndOfTrack => 2u8,
        };
        buf.put_u8(status);

        // Object payload (no length prefix for datagrams)
        buf.extend_from_slice(&object.payload);

        Ok(())
    }

    /// Decode MoQ object from datagram
    pub fn decode_object_datagram(data: &[u8]) -> Result<(u64, MoqObject), QuicRtcError> {
        let mut buf = Cursor::new(data);

        // Decode object datagram header
        let track_alias = Self::decode_varint(&mut buf)?;
        let group_id = Self::decode_varint(&mut buf)?;
        let object_id = Self::decode_varint(&mut buf)?;
        let publisher_priority = Self::decode_varint(&mut buf)? as u8;

        if !buf.has_remaining() {
            return Err(QuicRtcError::InvalidData {
                reason: "No data for object status".to_string(),
            });
        }

        // Decode object status
        let status_byte = buf.get_u8();
        let object_status = match status_byte {
            0 => crate::moq::MoqObjectStatus::Normal,
            1 => crate::moq::MoqObjectStatus::EndOfGroup,
            2 => crate::moq::MoqObjectStatus::EndOfTrack,
            _ => {
                return Err(QuicRtcError::InvalidData {
                    reason: format!("Invalid object status: {}", status_byte),
                });
            }
        };

        // Remaining data is payload
        let remaining = buf.remaining();
        let mut payload = vec![0u8; remaining];
        buf.copy_to_slice(&mut payload);

        let object = MoqObject {
            track_namespace: TrackNamespace {
                namespace: "default".to_string(),
                track_name: "unknown".to_string(),
            },
            track_name: "unknown".to_string(),
            group_id,
            object_id,
            publisher_priority,
            payload,
            object_status,
            created_at: std::time::Instant::now(),
            size: remaining,
        };

        Ok((track_alias, object))
    }
}

/// Utility functions for wire format validation
impl MoqWireFormat {
    /// Validate that a buffer contains a complete varint
    pub fn validate_varint(data: &[u8]) -> Result<(u64, usize), QuicRtcError> {
        if data.is_empty() {
            return Err(QuicRtcError::InvalidData {
                reason: "Empty data for varint".to_string(),
            });
        }

        let first_byte = data[0];
        let prefix = first_byte >> 6;
        let expected_length = match prefix {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => unreachable!(),
        };

        if data.len() < expected_length {
            return Err(QuicRtcError::InvalidData {
                reason: format!(
                    "Insufficient data for varint: need {} bytes, have {}",
                    expected_length,
                    data.len()
                ),
            });
        }

        let mut buf = Cursor::new(data);
        let value = Self::decode_varint(&mut buf)?;
        Ok((value, expected_length))
    }

    /// Calculate the encoded size of a varint
    pub fn varint_size(value: u64) -> usize {
        if value < 0x40 {
            1
        } else if value < 0x4000 {
            2
        } else if value < 0x40000000 {
            4
        } else if value < 0x4000000000000000 {
            8
        } else {
            panic!("Value too large for varint encoding: {}", value);
        }
    }

    /// Estimate the encoded size of a control message
    pub fn estimate_control_message_size(message: &MoqControlMessage) -> usize {
        match message {
            MoqControlMessage::Setup { .. } => 32, // Conservative estimate
            MoqControlMessage::SetupOk { .. } => 32,
            MoqControlMessage::Announce {
                track_namespace,
                track,
            } => {
                8 + track_namespace.namespace.len()
                    + track_namespace.track_name.len()
                    + track.name.len()
            }
            MoqControlMessage::Subscribe {
                track_namespace, ..
            } => 16 + track_namespace.namespace.len() + track_namespace.track_name.len(),
            _ => 64, // Conservative default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moq::{MoqCapabilities, MoqTrackType};

    #[test]
    fn test_varint_encoding_decoding() {
        let test_values = vec![
            0,
            63,
            64,
            16383,
            16384,
            1073741823,
            1073741824,
            u64::MAX >> 2,
        ];

        for value in test_values {
            let mut buf = BytesMut::new();
            MoqWireFormat::encode_varint(value, &mut buf);

            let mut cursor = Cursor::new(buf.as_ref());
            let decoded = MoqWireFormat::decode_varint(&mut cursor).unwrap();

            assert_eq!(
                value, decoded,
                "Varint encoding/decoding mismatch for value {}",
                value
            );
        }
    }

    #[test]
    fn test_control_message_encoding() {
        let setup_message = MoqControlMessage::Setup {
            version: 1,
            capabilities: MoqCapabilities {
                version: 1,
                max_tracks: 100,
                supported_track_types: vec![MoqTrackType::Audio, MoqTrackType::Video],
                max_object_size: 1024 * 1024,
                supports_caching: true,
            },
        };

        let mut buf = BytesMut::new();
        MoqWireFormat::encode_control_message(&setup_message, &mut buf).unwrap();

        // Verify we can decode it back
        let decoded = MoqWireFormat::decode_control_message(&buf).unwrap();

        match decoded {
            MoqControlMessage::Setup {
                version,
                capabilities,
            } => {
                assert_eq!(version, 1);
                assert_eq!(capabilities.max_tracks, 100);
            }
            _ => panic!("Unexpected message type after decoding"),
        }
    }

    #[test]
    fn test_track_namespace_encoding() {
        let namespace = TrackNamespace {
            namespace: "example.com".to_string(),
            track_name: "video/camera1".to_string(),
        };

        let mut buf = BytesMut::new();
        MoqWireFormat::encode_track_namespace(&namespace, &mut buf).unwrap();

        let mut cursor = Cursor::new(buf.as_ref());
        let decoded = MoqWireFormat::decode_track_namespace(&mut cursor).unwrap();

        assert_eq!(namespace.namespace, decoded.namespace);
        assert_eq!(namespace.track_name, decoded.track_name);
    }

    #[test]
    fn test_object_stream_encoding() {
        use crate::moq::MoqObjectStatus;

        let object = MoqObject {
            track_namespace: TrackNamespace {
                namespace: "test".to_string(),
                track_name: "video".to_string(),
            },
            track_name: "video".to_string(),
            group_id: 42,
            object_id: 1,
            publisher_priority: 5,
            payload: vec![1, 2, 3, 4, 5],
            object_status: MoqObjectStatus::Normal,
            created_at: std::time::Instant::now(),
            size: 5,
        };

        let mut buf = BytesMut::new();
        MoqWireFormat::encode_object_stream(&object, 123, &mut buf).unwrap();

        let (track_alias, decoded_object) = MoqWireFormat::decode_object_stream(&buf).unwrap();

        assert_eq!(track_alias, 123);
        assert_eq!(decoded_object.group_id, 42);
        assert_eq!(decoded_object.object_id, 1);
        assert_eq!(decoded_object.publisher_priority, 5);
        assert_eq!(decoded_object.payload, vec![1, 2, 3, 4, 5]);
    }
}
 