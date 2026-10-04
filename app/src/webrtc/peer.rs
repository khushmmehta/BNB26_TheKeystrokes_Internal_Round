use super::state::Ctx;
use crate::signaling::{IceCandidateData, PeerInfo, SignalingMessage};
use leptos::prelude::*;
use leptos::wasm_bindgen::{closure::Closure, JsCast, JsValue};
use std::sync::Arc;
use web_sys::{
    MediaStream, RtcConfiguration, RtcIceCandidate, RtcIceCandidateInit, RtcIceServer,
    RtcPeerConnection, RtcPeerConnectionIceEvent, RtcSdpType, RtcSessionDescriptionInit,
    RtcTrackEvent,
};

/// Cloudflare's STUN is free and reveals the public address a NAT hides. TURN
/// is only needed behind symmetric NATs, and it arrives free with the SFU --
/// so while we mesh, STUN alone is the right trade.
const STUN: &str = "stun:stun.cloudflare.com:3478";

fn config() -> RtcConfiguration {
    // iceServers must be an array of RTCIceServer *dictionaries*. Passing bare
    // strings makes the RTCPeerConnection constructor throw
    // "Element of 'iceServers' ... can't be converted to a dictionary", which
    // took down every connection.
    let server = RtcIceServer::new();
    server.set_urls(&web_sys::js_sys::Array::of1(&JsValue::from_str(STUN)));

    let config = RtcConfiguration::new();
    config.set_ice_servers(&web_sys::js_sys::Array::of1(&server.into()));
    config
}

fn description(sdp: &str, kind: RtcSdpType) -> RtcSessionDescriptionInit {
    let init = RtcSessionDescriptionInit::new(kind);
    init.set_sdp(sdp);
    init
}

/// The connection to `peer`, created on first use.
///
/// Returns None if the browser refuses the connection. That used to be an
/// `.expect`, which panicked inside a spawned task and left the app silently
/// doing nothing with no error anywhere.
pub fn connection(ctx: &Arc<Ctx>, peer: &PeerInfo) -> Option<RtcPeerConnection> {
    if let Some(pc) = ctx.peer(&peer.id) {
        return Some(pc.clone());
    }

    let config = config();
    let pc = match RtcPeerConnection::new_with_configuration(&config) {
        Ok(pc) => pc,
        Err(e) => {
            leptos::logging::error!("peer connection to {} failed: {e:?}", peer.name);
            return None;
        }
    };
    // addStream works for us: it is deprecated but functional, and the media
    // path is proven. Do not "upgrade" this to addTrack without re-testing
    // end to end -- it is not what makes the connection work.
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
    Some(pc)
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
    let Some(pc) = connection(ctx, peer) else {
        return;
    };
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

/// Adopts a description `peer` sent us, then flushes any candidates that
/// arrived while we were still waiting for it.
pub async fn set_remote(ctx: &Arc<Ctx>, peer: &PeerInfo, sdp: &str, kind: RtcSdpType) {
    let Some(pc) = connection(ctx, peer) else {
        return;
    };
    pc.set_remote_description(&description(sdp, kind))
        .await
        .expect("remote description");

    for candidate in ctx.take_pending(&peer.id) {
        push_candidate(&pc, candidate);
    }
}

/// Adds a trickled candidate from `peer`.
///
/// Each incoming message runs as its own task, so a candidate can be handled
/// while we are still awaiting `set_remote_description` -- and addIceCandidate
/// rejects in that state. Park those until there is a description to attach to.
pub fn add_candidate(ctx: &Arc<Ctx>, peer: &PeerInfo, candidate: IceCandidateData) {
    let Some(pc) = connection(ctx, peer) else {
        return;
    };
    if pc.remote_description().is_none() {
        ctx.defer(&peer.id, candidate);
        return;
    }
    push_candidate(&pc, candidate);
}

fn push_candidate(pc: &RtcPeerConnection, candidate: IceCandidateData) {
    let init = RtcIceCandidateInit::new(&candidate.candidate);
    init.set_sdp_mid(candidate.sdp_mid.as_deref());
    init.set_sdp_m_line_index(candidate.sdp_mline_index);

    if let Ok(candidate) = RtcIceCandidate::new(&init) {
        drop(pc.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate)));
    }
}
