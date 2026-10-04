use app::*;
use axum::{routing::get, Router};
use leptos::logging::log;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};

mod sfu;
mod signaling;

use crate::sfu::Sfu;
use crate::signaling::ws_handler;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::sync::Arc;

/// Proves our SFU credentials work: opens a real session and reports its id.
async fn sfu_health(State(sfu): State<Arc<Sfu>>) -> impl IntoResponse {
    match sfu.new_session().await {
        Ok(session) => Json(json!({ "ok": true, "session": session })),
        Err(e) => Json(json!({ "ok": false, "error": e.to_string() })),
    }
}

#[tokio::main]
async fn main() {
    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(App);

    // Read SFU credentials now, so a missing variable fails at boot rather than
    // halfway through the first call.
    let sfu = Arc::new(Sfu::from_env());

    // Build the leptos app router first
    let leptos_app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options.clone());

    // Then merge with our custom routes
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/sfu/health", get(sfu_health))
        .merge(leptos_app)
        .with_state(sfu);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
