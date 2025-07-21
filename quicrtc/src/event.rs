//! Event system for room and participant events

use crate::{RemoteParticipant, RemoteTrack};

/// Room events
#[derive(Debug)]
pub enum Event {
    /// A participant joined the room
    ParticipantJoined(RemoteParticipant),
    /// A participant left the room
    ParticipantLeft(RemoteParticipant),
    /// A track was received from a remote participant
    TrackReceived(RemoteTrack),
    /// A track was removed
    TrackRemoved(RemoteTrack),
}

/// Stream of room events
#[derive(Debug)]
pub struct EventStream {
    // TODO: Implement event stream
}

impl EventStream {
    /// Create new event stream
    pub fn new() -> Self {
        Self {}
    }
    
    /// Get the next event
    pub async fn next(&mut self) -> Option<Event> {
        // TODO: Implement event polling
        None
    }
}