//! Client side of the call: signaling over a websocket, media over WebRTC.

mod peer;
mod state;

pub use state::RemotePeer;

use crate::signaling::{RoomId, SignalingMessage};
use leptos::prelude::*;
use leptos_use::{use_user_media, use_websocket, UseUserMediaReturn, UseWebSocketReturn};
use std::sync::Arc;
use web_sys::{MediaStream, RtcSdpType};

const WS_URL: &str = "/ws";

pub struct UseWebRtcReturn {
    /// Remote participants that have media flowing.
    pub peers: RwSignal<Vec<RemotePeer>>,
    /// Our own camera + mic.
    pub local: RwSignal<Option<MediaStream>>,
    /// Last thing the signaling server complained about.
    pub error: RwSignal<Option<String>>,
    /// Human-readable progress, so a silent failure is impossible to miss.
    pub status: RwSignal<String>,
    pub join: Callback<(RoomId, String)>,
}

pub fn use_webrtc() -> UseWebRtcReturn {
    let peers = RwSignal::new(Vec::<RemotePeer>::new());
    let local = RwSignal::new(None::<MediaStream>);
    let error = RwSignal::new(None);
    let status = RwSignal::new(String::from("starting up"));
    let want = RwSignal::new(None::<(RoomId, String)>);

    let UseUserMediaReturn { stream, start, .. } = use_user_media();
    let UseWebSocketReturn { message, send, .. } =
        use_websocket::<String, String, codee::string::FromToStringCodec>(WS_URL);

    let ctx = Arc::new(state::Ctx::new(
        local,
        Arc::new(move |msg| send(&serde_json::to_string(&msg).expect("serializable"))),
        Arc::new(move |info, stream| {
            if let Some(mut list) = peers.try_get() {
                match list.iter_mut().find(|p| p.info.id == info.id) {
                    Some(peer) => peer.stream = stream,
                    None => list.push(RemotePeer {
                        info: info.clone(),
                        stream,
                    }),
                }
                peers.try_set(list);
            }
        }),
    ));

    // Ask for the camera, publish it, and hold the join request until the browser
    // has answered. Joining without tracks is worse than not joining: we would
    // offer an SDP with no media and the other side would see nobody.
    Effect::new(move |_| start());
    Effect::new({
        let ctx = ctx.clone();
        move |_| {
            let media = stream.get();

            match &media {
                Some(Ok(_)) => status.set(String::from("camera ready")),
                Some(Err(e)) => {
                    status.set(format!("camera blocked: {e:?}"));
                    leptos::logging::error!("camera blocked: {e:?}");
                }
                None => status.set(String::from("waiting for camera permission")),
            }
            local.set(media.clone().and_then(Result::ok));

            let Some((room_id, peer_name)) = want.get() else {
                return;
            };
            // Still pending: keep waiting, and keep `want` so we retry when the
            // camera finally lands.
            if media.is_none() {
                return;
            }
            want.set(None);

            status.set(format!("joining {room_id:?} as {peer_name}"));
            leptos::logging::log!("joining room {room_id:?} as {peer_name}");
            ctx.name(peer_name.clone());
            (ctx.send_msg)(SignalingMessage::Join { room_id, peer_name });
        }
    });

    Effect::new({
        let ctx = ctx.clone();
        move |_| {
            let Some(raw) = message.get() else { return };
            let Ok(msg) = serde_json::from_str::<SignalingMessage>(&raw) else {
                return;
            };

            let ctx = ctx.clone();
            leptos::task::spawn_local(async move {
                match msg {
                    SignalingMessage::Joined { peer_id, peers } => {
                        ctx.joined(peer_id);
                        status.set(if peers.is_empty() {
                            String::from("in the room — waiting for others to join")
                        } else {
                            format!("in the room — connecting to {} peer(s)", peers.len())
                        });
                        // We are the newcomer, so we offer to everyone already here.
                        for peer in peers {
                            peer::negotiate(&ctx, &peer, true).await;
                        }
                    }
                    SignalingMessage::PeerJoined { peer } => {
                        // Nothing to negotiate yet: they will offer to us.
                        status.set(format!("{} joined \u{2014} connecting", peer.name));
                        leptos::logging::log!("peer joined: {} {:?}", peer.name, peer.id);
                    }
                    SignalingMessage::PeerLeft { peer_id } => {
                        ctx.disconnect(&peer_id);
                        status.set(String::from("someone left the room"));
                        if let Some(list) = peers.try_get() {
                            peers.try_set(
                                list.into_iter().filter(|p| p.info.id != peer_id).collect(),
                            );
                        }
                    }
                    SignalingMessage::Offer { from, sdp, .. } => {
                        peer::set_remote(&ctx, &from, &sdp, RtcSdpType::Offer).await;
                        peer::negotiate(&ctx, &from, false).await;
                    }
                    SignalingMessage::Answer { from, sdp, .. } => {
                        peer::set_remote(&ctx, &from, &sdp, RtcSdpType::Answer).await;
                    }
                    SignalingMessage::IceCandidate {
                        from, candidate, ..
                    } => {
                        peer::add_candidate(&ctx, &from, candidate);
                    }
                    SignalingMessage::Error { message } => {
                        error.try_set(Some(message));
                    }
                    // Echo the server's keepalive so the socket is busy both ways.
                    SignalingMessage::Ping => (ctx.send_msg)(SignalingMessage::Ping),
                    SignalingMessage::Join { .. } => {}
                }
            });
        }
    });

    let join = Callback::new(move |room| want.set(Some(room)));

    // Unmounting is how you leave: the websocket drops, the server removes us
    // from the room and tells everyone else, and our connections go with it.
    on_cleanup({
        let ctx = ctx.clone();
        move || ctx.disconnect_all()
    });

    UseWebRtcReturn {
        peers,
        local,
        error,
        status,
        join,
    }
}
