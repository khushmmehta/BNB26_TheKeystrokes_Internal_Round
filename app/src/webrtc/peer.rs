use super::state::Ctx;
use crate::signaling::{IceCandidateData, PeerInfo, SignalingMessage};
use leptos::prelude::*;
use leptos::wasm_bindgen::{closure::Closure, JsCast};
use std::sync::Arc;
use web_sys::{
    MediaStream, RtcIceCandidate, RtcIceCandidateInit, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcSdpType, RtcSessionDescriptionInit, RtcTrackEvent,
};

fn description(sdp: &str, kind: RtcSdpType) -> RtcSessionDescriptionInit {
    let init = RtcSessionDescriptionInit::new(kind);
    init.set_sdp(sdp);
    init
}

/// The connection to `peer`, created on first use.
pub fn connection(ctx: &Arc<Ctx>, peer: &PeerInfo) -> RtcPeerConnection {
    if let Some(pc) = ctx.peer(&peer.id) {
        return pc.clone();
    }

    // STUN/TURN arrives with coturn; host candidates are enough to try this out.
    let pc = RtcPeerConnection::new().expect("peer connection");
    if let Some(local) = ctx.local.get_untracked() {
        pc.add_stream(&local);
    }

    let ice = {
        let (ctx, peer) = (ctx.clone(), peer.clone());
        Closure::wrap(Box::new(move |ev: RtcPeerConnectionIceEvent| {
            let (Some(me), Some(c)) = (ctx.me(), ev.candidate()) else {
                return;
            };
            (ctx.send_msg)(SignalingMessage::IceCandidate {
                from: me,
                to: peer.id.clone(),
                candidate: IceCandidateData {
                    candidate: c.candidate(),
                    sdp_mid: c.sdp_mid(),
                    sdp_mline_index: c.sdp_m_line_index(),
                },
            });
        }) as Box<dyn FnMut(_)>)
    };
    pc.set_onicecandidate(Some(ice.as_ref().unchecked_ref()));
    ice.forget();

    let track = {
        let (ctx, peer) = (ctx.clone(), peer.clone());
        Closure::wrap(Box::new(move |ev: RtcTrackEvent| {
            if let Ok(stream) = ev.streams().get(0).dyn_into::<MediaStream>() {
                (ctx.on_track)(&peer, stream);
            }
        }) as Box<dyn FnMut(_)>)
    };
    pc.set_ontrack(Some(track.as_ref().unchecked_ref()));
    track.forget();

    ctx.connect(&peer.id, pc.clone());
    pc
}

/// Creates a description and sends it to `peer`.
///
/// The description is sent *before* it is set locally so that the ICE
/// candidates it triggers are queued behind it: by the time they arrive the
/// peer has a remote description to attach them to.
pub async fn negotiate(ctx: &Arc<Ctx>, peer: &PeerInfo, offer: bool) {
    let Some(me) = ctx.me() else {
        return;
    };
    let pc = connection(ctx, peer);
    let init = wasm_bindgen_futures::JsFuture::from(if offer {
        pc.create_offer()
    } else {
        pc.create_answer()
    })
    .await
    .expect("create description")
    .unchecked_into::<RtcSessionDescriptionInit>();
    let sdp = init.get_sdp().unwrap_or_default();

    let msg = if offer {
        SignalingMessage::Offer {
            from: me,
            to: peer.id.clone(),
            sdp,
        }
    } else {
        SignalingMessage::Answer {
            from: me,
            to: peer.id.clone(),
            sdp,
        }
    };
    (ctx.send_msg)(msg);

    pc.set_local_description(&init)
        .await
        .expect("local description");
}

/// Adopts a description `peer` sent us.
pub async fn set_remote(ctx: &Arc<Ctx>, peer: &PeerInfo, sdp: &str, kind: RtcSdpType) {
    let pc = connection(ctx, peer);
    pc.set_remote_description(&description(sdp, kind))
        .await
        .expect("remote description");
}

/// Adds a trickled candidate from `peer`.
pub fn add_candidate(ctx: &Arc<Ctx>, peer: &PeerInfo, candidate: IceCandidateData) {
    let init = RtcIceCandidateInit::new(&candidate.candidate);
    init.set_sdp_mid(candidate.sdp_mid.as_deref());
    init.set_sdp_m_line_index(candidate.sdp_mline_index);

    if let Ok(candidate) = RtcIceCandidate::new(&init) {
        // Rejects if the remote description has not arrived yet; the ordering in
        // `negotiate` is what keeps that from happening.
        drop(connection(ctx, peer).add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate)));
    }
}
