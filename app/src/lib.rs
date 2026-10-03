mod camera_preview;

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::{use_navigate, use_params_map},
    ParamSegment, StaticSegment,
};

use camera_preview::CameraPreview;

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

    view! {
        <h1>"Room: " {id}</h1>
        <CameraPreview/>
    }
}
