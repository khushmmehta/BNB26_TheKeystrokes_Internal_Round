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
    pub join: Callback<(RoomId, String)>,
}

pub fn use_webrtc() -> UseWebRtcReturn {
    let peers = RwSignal::new(Vec::<RemotePeer>::new());
    let local = RwSignal::new(None::<MediaStream>);
    let error = RwSignal::new(None);
    let want = RwSignal::new(None::<(RoomId, String)>);

    let UseUserMediaReturn { stream, start, .. } = use_user_media();
    let UseWebSocketReturn { message, send, .. } =
        use_websocket::<String, String, codee::string::FromToStringCodec>(WS_URL);

    let ctx = Arc::new(state::Ctx::new(
        local.clone(),
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

    // Ask for the camera, publish it, and hold the join request until the
    // browser has answered: our tracks have to be in place before we offer.
    Effect::new(move |_| start());
    Effect::new({
        let ctx = ctx.clone();
        move |_| {
            let media = stream.get();
            local.set(media.clone().and_then(Result::ok));

            let (Some((room_id, peer_name)), Some(_)) = (want.get(), media) else {
                return;
            };
            want.set(None);
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
                        // We are the newcomer, so we offer to everyone already here.
                        for peer in peers {
                            peer::negotiate(&ctx, &peer, true).await;
                        }
                    }
                    SignalingMessage::PeerJoined { .. } => {
                        // Nothing to do: they will offer to us.
                    }
                    SignalingMessage::PeerLeft { peer_id } => {
                        ctx.disconnect(&peer_id);
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
        join,
    }
}
