pub mod signaling;
pub mod webrtc;

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::{use_navigate, use_params_map},
    ParamSegment, StaticSegment,
};

use signaling::RoomId;
use webrtc::use_webrtc;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/bnb26_thekeystrokes_internal_round.css"/>

        // sets the document title
        <Title text="Welcome to Leptos"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=(StaticSegment("room"), ParamSegment("id")) view=RoomPage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let room = RwSignal::new(String::new());
    let navigate = use_navigate();

    let join = move |_| {
        let name = room.get_untracked().trim().to_string();
        if !name.is_empty() {
            navigate(&format!("/room/{name}"), Default::default());
        }
    };

    view! {
        <h1>"RoundTable"</h1>
        <input
            type="text"
            placeholder="Room name"
            prop:value=move || room.get()
            on:input:target=move |ev| room.set(ev.target().value())
        />
        <button on:click=join>"Join"</button>
    }
}

#[component]
fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();

    let webrtc = use_webrtc();
    let navigate = use_navigate();
    let join = webrtc.join;

    Effect::new(move |_| join.run((RoomId(id()), String::from("guest"))));

    view! {
        <h1>"Room: " {id}</h1>
        <Show when=move || webrtc.error.get().is_some()>
            <p class="error">{move || webrtc.error.get().unwrap_or_default()}</p>
        </Show>
        <div class="grid">
            <Video stream=move || webrtc.local.get() name=String::from("you")/>
            <For each=move || webrtc.peers.get() key=|peer| peer.info.id.0.clone() let:peer>
                <Video stream=move || Some(peer.stream.clone()) name=peer.info.name.clone()/>
            </For>
        </div>
        <button on:click=move |_| navigate("/", Default::default())>"Leave"</button>
    }
}

#[component]
fn Video(
    stream: impl Fn() -> Option<web_sys::MediaStream> + 'static,
    name: String,
) -> impl IntoView {
    let video_ref = NodeRef::<leptos::html::Video>::new();

    Effect::new(move |_| {
        if let (Some(el), Some(stream)) = (video_ref.get(), stream()) {
            el.set_src_object(Some(&stream));
        }
    });

    view! {
        <figure>
            <video node_ref=video_ref autoplay playsinline muted=true></video>
            <figcaption>{name}</figcaption>
        </figure>
    }
}
