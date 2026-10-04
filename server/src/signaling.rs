use app::signaling::*;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

const MAX_PEERS: usize = 16;
/// Well under the idle timeout of common proxies (Caddy sits around 5min).
const HEARTBEAT_SECS: u64 = 25;

type PeerData = (PeerInfo, mpsc::Sender<SignalingMessage>);
type RoomPeers = HashMap<PeerId, PeerData>;

/// RoomId -> (PeerId -> (PeerInfo, sender))
static ROOMS: LazyLock<Arc<RwLock<HashMap<RoomId, RoomPeers>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

pub async fn ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<SignalingMessage>(32);
    let mut current_room: Option<RoomId> = None;
    let mut current_peer: Option<PeerId> = None;

    // Forward outbound messages to client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
            if sink.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Our signaling socket goes quiet once a call is set up, which is exactly
    // when a proxy in the path decides to reclaim it. Ping well inside any
    // plausible idle window. A failed send means the peer is gone.
    let ping_task = tokio::spawn({
        let tx = tx.clone();
        async move {
            let mut beat = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
            beat.tick().await;
            while tx.send(SignalingMessage::Ping).await.is_ok() {
                beat.tick().await;
            }
        }
    });

    while let Some(Ok(Message::Text(text))) = stream.next().await {
        let msg: SignalingMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                tx.send(SignalingMessage::Error {
                    message: e.to_string(),
                })
                .await
                .ok();
                continue;
            }
        };

        match msg {
            SignalingMessage::Join { room_id, peer_name } => {
                if let (Some(r), Some(p)) = (&current_room, &current_peer) {
                    leave(r, p).await;
                }

                let mut rooms = ROOMS.write().await;
                let room = rooms.entry(room_id.clone()).or_default();
                if room.len() >= MAX_PEERS {
                    tx.send(SignalingMessage::Error {
                        message: "Room full".into(),
                    })
                    .await
                    .ok();
                    continue;
                }

                let peer_id = PeerId(Uuid::new_v4().to_string());
                let peer_info = PeerInfo {
                    id: peer_id.clone(),
                    name: peer_name,
                };
                let existing: Vec<PeerInfo> = room.values().map(|(i, _)| i.clone()).collect();
                room.insert(peer_id.clone(), (peer_info.clone(), tx.clone()));
                current_room = Some(room_id.clone());
                current_peer = Some(peer_id.clone());

                tx.send(SignalingMessage::Joined {
                    peer_id: peer_id.clone(),
                    peers: existing,
                })
                .await
                .ok();

                for (_, peer_tx) in room.values() {
                    if !std::ptr::eq(peer_tx, &tx) {
                        peer_tx
                            .send(SignalingMessage::PeerJoined {
                                peer: peer_info.clone(),
                            })
                            .await
                            .ok();
                    }
                }
            }

            SignalingMessage::Offer {
                ref from, ref to, ..
            }
            | SignalingMessage::Answer {
                ref from, ref to, ..
            }
            | SignalingMessage::IceCandidate {
                ref from, ref to, ..
            } => {
                if current_peer.as_ref() != Some(&from.id) {
                    continue;
                }
                forward(&current_room, to, msg.clone()).await;
            }
            _ => {}
        }
    }

    send_task.abort();
    ping_task.abort();
    if let (Some(r), Some(p)) = (current_room, current_peer) {
        leave(&r, &p).await;
    }
}

async fn forward(room_id: &Option<RoomId>, to: &PeerId, msg: SignalingMessage) {
    if let Some(rid) = room_id {
        if let Some(room) = ROOMS.read().await.get(rid) {
            if let Some((_, tx)) = room.get(to) {
                tx.send(msg).await.ok();
            }
        }
    }
}

async fn leave(room_id: &RoomId, peer_id: &PeerId) {
    let mut rooms = ROOMS.write().await;
    if let Some(room) = rooms.get_mut(room_id) {
        if room.remove(peer_id).is_some() {
            for (_, tx) in room.values() {
                tx.send(SignalingMessage::PeerLeft {
                    peer_id: peer_id.clone(),
                })
                .await
                .ok();
            }
            if room.is_empty() {
                rooms.remove(room_id);
            }
        }
    }
}
