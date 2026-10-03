use leptos::prelude::*;
use leptos_use::{use_user_media, UseUserMediaReturn};

#[component]
pub fn CameraPreview() -> impl IntoView {
    let video_ref = NodeRef::<leptos::html::Video>::new();

    let UseUserMediaReturn {
        stream,
        start,
        stop,
        enabled,
        ..
    } = use_user_media();

    Effect::new(move |_| {
        match stream.get() {
            Some(Ok(s)) => {
                video_ref.with(|v| {
                    if let Some(v) = v {
                        v.set_src_object(Some(&s));
                    }
                });
                return;
            }
            Some(Err(e)) => leptos::logging::error!("Failed to get media stream: {:?}", e),
            None => leptos::logging::log!("No stream yet"),
        }

        video_ref.with(|v| {
            if let Some(v) = v {
                v.set_src_object(None);
            }
        });
    });

    match stream.get_untracked() {
        Some(Err(e)) => Some(format!("Could not start the camera: {e:?}")),
        _ => None,
    };

    let toggle = move |_| {
        if enabled.get_untracked() {
            stop();
        } else {
            start();
        }
    };

    view! {
        <div class="camera-preview">
        <div>
                        <button on:click=toggle>
                            {move || if enabled.get() { "Stop camera" } else { "Start camera" }}
                        </button>
                    </div>

            <div>
                <video
                    node_ref=video_ref
                    controls=false
                    autoplay=true
                    muted=true
                    class="preview"
                ></video>
            </div>
        </div>
    }
}
