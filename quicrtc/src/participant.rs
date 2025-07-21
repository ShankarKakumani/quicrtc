//! Participant management and abstractions

/// Collection of participants in a room
#[derive(Debug)]
pub struct Participants {
    // TODO: Implement participant collection
}

/// Local participant representation
#[derive(Debug)]
pub struct LocalParticipant {
    id: String,
    // TODO: Add local participant state
}

impl LocalParticipant {
    /// Get participant ID
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Remote participant representation
#[derive(Debug)]
pub struct RemoteParticipant {
    id: String,
    // TODO: Add remote participant state
}

impl RemoteParticipant {
    /// Get participant ID
    pub fn id(&self) -> &str {
        &self.id
    }
}