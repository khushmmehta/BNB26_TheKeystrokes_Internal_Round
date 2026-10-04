use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Unique identifier for a peer in a room
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub String);

/// Unique identifier for a room
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub String);

/// Signaling messages exchanged between client and server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SignalingMessage {
    /// Client -> Server: Join a room
    Join { room_id: RoomId, peer_name: String },
    /// Server -> Client: Successfully joined, receives list of existing peers
    Joined {
        peer_id: PeerId,
        peers: Vec<PeerInfo>,
    },
    /// Server -> Client: New peer joined the room
    PeerJoined { peer: PeerInfo },
    /// Server -> Client: Peer left the room
    PeerLeft { peer_id: PeerId },
    /// Client <-> Server -> Client: WebRTC Offer
    Offer {
        from: PeerInfo,
        to: PeerId,
        sdp: String,
    },
    /// Client <-> Server -> Client: WebRTC Answer
    Answer {
        from: PeerInfo,
        to: PeerId,
        sdp: String,
    },
    /// Client <-> Server -> Client: ICE Candidate
    IceCandidate {
        from: PeerInfo,
        to: PeerId,
        candidate: IceCandidateData,
    },
    /// Server -> Client: Error message
    Error { message: String },
}

/// Information about a peer in the room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: PeerId,
    pub name: String,
}

/// ICE Candidate data (serializable subset of RTCIceCandidateInit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidateData {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
}

/// Errors that can occur during signaling
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum SignalingError {
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    #[error("Peer not found: {0}")]
    PeerNotFound(String),
    #[error("Room full")]
    RoomFull,
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
    #[error("Internal server error: {0}")]
    Internal(String),
}
