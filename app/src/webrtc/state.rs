use crate::signaling::{IceCandidateData, PeerId, PeerInfo, SignalingMessage};
use leptos::prelude::*;
use std::{collections::HashMap, sync::Arc, sync::Mutex};
use web_sys::{MediaStream, RtcPeerConnection};

/// Sends a message to the signaling server.
pub type Sender = Arc<dyn Fn(SignalingMessage) + Send + Sync>;

/// Called with every stream a remote peer sends us.
pub type OnTrack = Arc<dyn Fn(&PeerInfo, MediaStream) + Send + Sync>;

/// State shared by every WebRTC operation. Peer connections are not reactive,
/// so they live outside the view graph and outlive its disposal.
pub struct Ctx {
    /// The id the server handed us when we joined.
    me: Mutex<Option<PeerId>>,
    /// The name we joined with, so we can introduce ourselves in an offer.
    name: Mutex<String>,
    /// One connection per remote peer.
    pcs: Mutex<HashMap<PeerId, RtcPeerConnection>>,
    /// Candidates that arrived before we had a remote description for them.
    pending: Mutex<HashMap<PeerId, Vec<IceCandidateData>>>,
    /// Our camera + mic.
    pub local: RwSignal<Option<MediaStream>>,
    pub send_msg: Sender,
    pub on_track: OnTrack,
}

impl Ctx {
    pub fn new(local: RwSignal<Option<MediaStream>>, send_msg: Sender, on_track: OnTrack) -> Self {
        Self {
            me: Default::default(),
            name: Default::default(),
            pcs: Default::default(),
            pending: Default::default(),
            local,
            send_msg,
            on_track,
        }
    }

    /// How we introduce ourselves to other peers.
    pub fn me(&self) -> Option<PeerInfo> {
        let me = self.me.lock().expect("poisoned");
        me.clone().map(|id| PeerInfo {
            id,
            name: self.name.lock().expect("poisoned").clone(),
        })
    }

    pub fn joined(&self, peer_id: PeerId) {
        *self.me.lock().expect("poisoned") = Some(peer_id);
    }

    /// The name we are joining with.
    pub fn name(&self, name: String) {
        *self.name.lock().expect("poisoned") = name;
    }

    pub fn peer(&self, id: &PeerId) -> Option<RtcPeerConnection> {
        self.pcs.lock().expect("poisoned").get(id).cloned()
    }

    pub fn connect(&self, id: &PeerId, pc: RtcPeerConnection) {
        self.pcs.lock().expect("poisoned").insert(id.clone(), pc);
    }

    pub fn disconnect(&self, id: &PeerId) {
        if let Some(pc) = self.pcs.lock().expect("poisoned").remove(id) {
            pc.close();
        }
    }

    pub fn disconnect_all(&self) {
        for (_, pc) in self.pcs.lock().expect("poisoned").drain() {
            pc.close();
        }
    }

    /// Parks a candidate until the peer's remote description is set.
    pub fn defer(&self, id: &PeerId, candidate: IceCandidateData) {
        self.pending
            .lock()
            .expect("poisoned")
            .entry(id.clone())
            .or_default()
            .push(candidate);
    }

    /// Takes everything parked for a peer, clearing the slot.
    pub fn take_pending(&self, id: &PeerId) -> Vec<IceCandidateData> {
        self.pending
            .lock()
            .expect("poisoned")
            .remove(id)
            .unwrap_or_default()
    }
}

/// A remote participant, as rendered by the room grid.
#[derive(Clone)]
pub struct RemotePeer {
    pub info: PeerInfo,
    pub stream: MediaStream,
}

impl PartialEq for RemotePeer {
    fn eq(&self, other: &Self) -> bool {
        self.info.id == other.info.id
    }
}
