#![allow(dead_code)] // wired up in the SFU migration step
//! Client for the Cloudflare Realtime SFU Connection API.
//!
//! This is the only module that knows the SFU's REST shapes. Everything else
//! speaks in terms of [`TrackSpec`], [`Received`] and [`Mapping`], so if the
//! (still experimental) API moves, this file absorbs it.

use app::signaling::SignalingError;
use reqwest::{Client, Method};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const BASE: &str = "https://rtc.live.cloudflare.com/v1";

/// A track to publish, named by the `mid` of the transceiver carrying it.
#[derive(Clone, Serialize)]
#[serde(tag = "location", rename_all = "lowercase")]
pub enum TrackSpec {
    Local {
        mid: String,
        #[serde(rename = "trackName")]
        track_name: String,
    },
    Remote {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "trackName")]
        track_name: String,
    },
}

/// A publication we have been granted: which publisher, which track, and the
/// receiving `mid` the SFU assigned it. The client needs the `mid` to route
/// `ontrack` to the right video element.
#[derive(Debug, Clone)]
pub struct Mapping {
    pub mid: String,
    pub session_id: String,
    pub track_name: String,
}

/// The SFU's offer, plus the mapping of what we just asked to receive.
pub struct Received {
    pub offer: String,
    pub tracks: Vec<Mapping>,
}

pub struct Sfu {
    /// Built once and reused, so every call shares a keep-alive pool.
    client: Client,
    app_id: String,
    secret: String,
}

impl Sfu {
    /// Reads credentials from the environment, failing fast so a missing
    /// variable surfaces at boot rather than three minutes into a call.
    pub fn from_env() -> Self {
        Self {
            client: Client::new(),
            app_id: env("REALTIME_SFU_APP_ID"),
            secret: env("REALTIME_SFU_APP_SECRET"),
        }
    }

    /// Opens a session. Publishing and subscribing each need their own.
    pub async fn new_session(&self) -> Result<String, SignalingError> {
        #[derive(Deserialize)]
        struct Created {
            #[serde(rename = "sessionId")]
            session_id: String,
        }
        let created: Created = self
            .call(Method::POST, "/sessions/new", None::<Value>)
            .await?;
        Ok(created.session_id)
    }

    /// Publishes local tracks using our offer, returning the SFU's answer.
    pub async fn publish(
        &self,
        session: &str,
        offer: &str,
        tracks: &[TrackSpec],
    ) -> Result<String, SignalingError> {
        let exchange: Exchange = self
            .call(
                Method::POST,
                &format!("/sessions/{session}/tracks/new"),
                Some(json!({
                    "sessionDescription": { "type": "offer", "sdp": offer },
                    "tracks": tracks,
                })),
            )
            .await?;
        exchange.description(SdpKind::Answer)
    }

    /// Asks for remote tracks by `(publisher session id, track name)`. The SFU
    /// offers first; we reply through [`Sfu::renegotiate`].
    pub async fn subscribe(
        &self,
        session: &str,
        want: &[(String, String)],
    ) -> Result<Received, SignalingError> {
        let tracks: Vec<_> = want
            .iter()
            .map(|(session_id, track_name)| TrackSpec::Remote {
                session_id: session_id.clone(),
                track_name: track_name.clone(),
            })
            .collect();

        let exchange: Exchange = self
            .call(
                Method::POST,
                &format!("/sessions/{session}/tracks/new"),
                Some(json!({ "tracks": tracks })),
            )
            .await?;
        let offer = exchange.description(SdpKind::Offer)?;

        // Per-track results come back in request order, so zipping gives us
        // the mid the SFU assigned to each publication we asked for.
        let tracks = exchange
            .tracks
            .iter()
            .zip(want)
            .filter_map(|(result, (session_id, track_name))| {
                Some(Mapping {
                    mid: result.mid.clone()?,
                    session_id: session_id.clone(),
                    track_name: track_name.clone(),
                })
            })
            .collect();

        Ok(Received { offer, tracks })
    }

    /// Hands the client's answer back to the SFU.
    pub async fn renegotiate(&self, session: &str, answer: &str) -> Result<(), SignalingError> {
        let _: Value = self
            .call(
                Method::PUT,
                &format!("/sessions/{session}/renegotiate"),
                Some(json!({ "sessionDescription": { "type": "answer", "sdp": answer } })),
            )
            .await?;
        Ok(())
    }

    async fn call<R: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<R, SignalingError> {
        let url = format!("{BASE}/apps/{}{path}", self.app_id);
        let request = self.client.request(method, url).bearer_auth(&self.secret);
        let request = match body {
            Some(body) => request.json(&body),
            None => request,
        };

        let response = request
            .send()
            .await
            .map_err(|e| SignalingError::Internal(format!("sfu {path}: {e}")))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| SignalingError::Internal(format!("sfu {path}: {e}")))?;
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);

        if !status.is_success() {
            return Err(SignalingError::Internal(format!(
                "sfu {path}: {status}: {text}"
            )));
        }
        if let Some(code) = value.get("errorCode").and_then(Value::as_str) {
            let detail = value
                .get("errorDescription")
                .and_then(Value::as_str)
                .unwrap_or_default();
            return Err(SignalingError::Internal(format!(
                "sfu {path}: {code}: {detail}"
            )));
        }
        serde_json::from_value(value)
            .map_err(|e| SignalingError::Internal(format!("sfu {path}: unexpected response: {e}")))
    }
}

fn env(key: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => panic!("{key} is not set -- copy .env.example to .env and fill it in"),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SdpKind {
    Offer,
    Answer,
}

impl SdpKind {
    fn name(self) -> &'static str {
        match self {
            SdpKind::Offer => "offer",
            SdpKind::Answer => "answer",
        }
    }
}

#[derive(Deserialize)]
struct Exchange {
    #[serde(rename = "sessionDescription", default)]
    session_description: Option<Sdp>,
    #[serde(default)]
    tracks: Vec<TrackResult>,
}

impl Exchange {
    /// Pulls out the SDP, rejecting the response if any track was refused.
    fn description(&self, expected: SdpKind) -> Result<String, SignalingError> {
        if let Some(failed) = self.tracks.iter().find(|t| t.error_code.is_some()) {
            return Err(SignalingError::Internal(format!(
                "sfu rejected a track: {}",
                failed.detail()
            )));
        }

        let description = self.session_description.as_ref().ok_or_else(|| {
            SignalingError::Internal("sfu returned no session description".into())
        })?;

        if description.kind != expected.name() {
            return Err(SignalingError::Internal(format!(
                "sfu sent a {:?}, expected {:?}",
                description.kind,
                expected.name()
            )));
        }
        Ok(description.sdp.clone())
    }
}

#[derive(Deserialize)]
struct Sdp {
    #[serde(rename = "type")]
    kind: String,
    sdp: String,
}

#[derive(Deserialize)]
struct TrackResult {
    #[serde(default)]
    mid: Option<String>,
    #[serde(rename = "errorCode", default)]
    error_code: Option<String>,
    #[serde(rename = "errorDescription", default)]
    error_description: Option<String>,
}

impl TrackResult {
    fn detail(&self) -> String {
        match (&self.error_code, &self.error_description) {
            (Some(code), Some(detail)) => format!("{code}: {detail}"),
            (Some(code), None) => format!("{code} on mid {:?}", self.mid),
            _ => format!("no error code on mid {:?}", self.mid),
        }
    }
}
