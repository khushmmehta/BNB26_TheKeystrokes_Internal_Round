pub mod signaling;
pub mod webrtc;

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::{use_navigate, use_params_map, use_query_map},
    ParamSegment, StaticSegment,
};

use leptos::wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{IntersectionObserver, IntersectionObserverEntry, NodeList};

use web_sys::js_sys::Array;

use signaling::RoomId;
use webrtc::use_webrtc;

/// `NodeList` has no iterator in this web-sys build; walk it by index.
fn items(list: &NodeList) -> Vec<web_sys::Node> {
    (0..list.length()).filter_map(|i| list.item(i)).collect()
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="description" content="Peer-to-peer video, end to end. Built by The Keystrokes."/>
                <link rel="preconnect" href="https://fonts.googleapis.com"/>
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous"/>
                <link
                    href="https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700;800&family=DM+Mono:wght@400;500&display=swap"
                    rel="stylesheet"
                />
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

    // Scroll reveals. The reference did this with 25KB of DOM-patching JS; one
    // IntersectionObserver in an Effect does the same with no runtime shipped.
    Effect::new(move |_| {
        // SSR has no DOM; the reveal is purely cosmetic.
        if cfg!(not(target_arch = "wasm32")) {
            return;
        }

        let targets = web_sys::window()
            .and_then(|w| w.document())
            .map(|d| {
                d.query_selector_all(".section, .finale, .team, .marquee")
                    .map(|n| {
                        items(&n)
                            .into_iter()
                            .filter_map(|x| x.dyn_into().ok())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        if targets.is_empty() {
            return;
        }

        // The callback receives a js_sys::Array of entries, not a NodeList.
        let closure = Closure::wrap(Box::new(move |entries: Array| {
            for entry in entries
                .iter()
                .filter_map(|e| e.dyn_into::<IntersectionObserverEntry>().ok())
            {
                if entry.is_intersecting() {
                    entry.target().set_attribute("data-shown", "").ok();
                }
            }
        }) as Box<dyn FnMut(Array)>);
        let Ok(observer) = IntersectionObserver::new(
            closure
                .as_ref()
                .unchecked_ref::<web_sys::js_sys::Function>(),
        ) else {
            return;
        };
        closure.forget();
        for node in targets {
            observer.observe(&node);
        }
    });

    view! {
        <Stylesheet id="leptos" href="/pkg/rt3.css"/>

        // sets the document title
        <Title text="Welcome to Leptos"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=StaticSegment("join") view=JoinPage/>
                    <Route path=(StaticSegment("room"), ParamSegment("id")) view=RoomPage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let navigate = use_navigate();
    let go_join = navigate.clone();

    view! {
        <div class="landing">
            <div class="copy">
                <p class="eyebrow">
                    <i/> "real-time communication" <b/> "rt / 001"
                </p>

                <div class="title">
                    <h1>"Every"</h1>
                    <h1 class="outline">"voice."</h1>
                    <h1>"Every " <span class="accent">"word."</span></h1>
                </div>

                <p class="tagline">
                    "RoundTable turns a group call into a living layer — speaker identity, "
                    "presence, and a table that only ever holds the people you invited."
                </p>

                <div class="actions">
                    <button class="cta" on:click=move |_| navigate.clone()("/join", Default::default())>
                        "start a meeting"
                    </button>
                    <a class="cta ghost" href="#stack">"how it works"</a>
                </div>

                <div class="hud">
                    <span>"● signal found"</span>
                    <span>"media never leaves the edge"</span>
                    <span>"no waiting room"</span>
                </div>
            </div>

            <div class="stage">
                <span class="stage-id">"001"</span>
                <div class="orb o1"/>
                <div class="orb o2"/>
                <div class="orb o3"/>

                <div class="core">
                    <div class="core-glow"/>
                    <div class="core-ring"/>
                    <div class="core-center">
                        <i/>
                        <i/>
                        <i/>
                        <i/>
                        <i/>
                    </div>
                    <small class="core-live">"live"</small>
                </div>

                <div class="chip c1 green">
                    <b>"Ishan"</b>
                    <span>"anyone else seeing this?"</span>
                </div>
                <div class="chip c2 blue">
                    <b>"Chetan"</b>
                    <span>"loud and clear."</span>
                </div>
                <div class="chip c3 violet">
                    <b>"Khush"</b>
                    <span>"let's begin."</span>
                </div>
                <div class="chip c4 orange">
                    <b>"Amir"</b>
                    <span>"ready when you are."</span>
                </div>

                <div class="scanner"/>
            </div>
        </div>

        <div class="marquee">
            <div>
                <span>"peer-to-peer"</span>
                <span>"no accounts"</span>
                <span>"edge routed"</span>
                <span>"captions ready"</span>
                <span>"peer-to-peer"</span>
                <span>"no accounts"</span>
                <span>"edge routed"</span>
                <span>"captions ready"</span>
            </div>
        </div>

        <div class="sections">
            <section class="section" id="how">
                <span class="kicker">"01 / what it is"</span>
                <h2>"A table, not a lobby."</h2>
                <p class="lede">
                    "RoundTable is a small room with a fixed number of chairs. You type a meeting id, "
                    "you give a name, and you are on the table. No account, no waiting room, no link "
                    "that expires in an hour."
                </p>

                <div class="cards">
                    <article class="card" data-num="01">
                        <h3>"Identity first"</h3>
                        <p>
                            "Your name is the only credential. It rides with you into the room and labels "
                            "your tile, so everyone knows who is actually speaking."
                        </p>
                    </article>
                    <article class="card" data-num="02">
                        <h3>"Media, not metadata"</h3>
                        <p>
                            "Audio and video route peer to peer, or through Cloudflare's edge SFU when "
                            "a room grows. Our server brokers signalling and never touches a frame."
                        </p>
                    </article>
                    <article class="card" data-num="03">
                        <h3>"Built for captions"</h3>
                        <p>
                            "Because media is reachable at the edge, audio can be tapped and transcribed "
                            "server-side — which is what makes per-person live captions tractable."
                        </p>
                    </article>
                </div>
            </section>

            <section class="section" id="stack">
                <span class="kicker">"02 / the stack"</span>
                <h2>"Rust where it matters."</h2>
                <p class="lede">
                    "No Node toolchain, no animation library, no component kit. The whole product is "
                    "Rust plus CSS, which is why it compiles to a single wasm bundle and boots instantly."
                </p>

                <table class="stack-table">
                    <tbody>
                        <tr>
                            <td>"frontend"</td>
                            <td>
                                <b>Leptos 0.8</b> " — server rendered, hydrated on the client, reactive "
                                "signals. Styling is SCSS compiled by lightningcss. Animations are pure "
                                "CSS keyframes, so there is no animation runtime to ship."
                            </td>
                        </tr>
                        <tr>
                            <td>"media"</td>
                            <td>
                                <b>web-sys</b> " raw " <b>RTCPeerConnection</b> " against "
                                <b>Cloudflare Realtime</b> " — a serverless SFU with STUN and TURN in "
                                "330+ cities. One connection per participant, so cost is flat as rooms grow."
                            </td>
                        </tr>
                        <tr>
                            <td>"signalling"</td>
                            <td>
                                <b>Axum</b> " + " <b>tokio</b> " — rooms are an in-memory map, peers forward "
                                "straight to each other over a single websocket, with a keepalive that stops "
                                "proxies reaping idle sockets."
                            </td>
                        </tr>
                        <tr>
                            <td>"delivery"</td>
                            <td>
                                <b>Cloudflare Tunnel</b> " — the server publishes over an outbound tunnel on "
                                "your own domain. Zero open ports, no static IP, and the home IP never "
                                "appears in DNS."
                            </td>
                        </tr>
                    </tbody>
                </table>
            </section>

            <section class="section" id="how-it-works">
                <span class="kicker">"03 / how a call happens"</span>
                <h2>"Four steps, no ceremony."</h2>
                <p class="lede">
                    "The part that usually takes a user half a minute takes about four seconds here."
                </p>

                <div class="cards">
                    <article class="card" data-num="01">
                        <h3>"Pick an id"</h3>
                        <p>"Anything you can say out loud. It becomes the room."</p>
                    </article>
                    <article class="card" data-num="02">
                        <h3>"Grant the camera"</h3>
                        <p>"One prompt, remembered. Nothing is uploaded to store."</p>
                    </article>
                    <article class="card" data-num="03">
                        <h3>"Negotiate"</h3>
                        <p>"A websocket tells the room you arrived. One offer, one answer."</p>
                    </article>
                    <article class="card" data-num="04">
                        <h3>"Talk"</h3>
                        <p>"Media flows directly. Leaving is just closing the tab."</p>
                    </article>
                </div>
            </section>
        </div>

        <section class="finale">
            <h2>"Pull up a chair."</h2>
            <p>"No sign-up, no install, no configuration. A meeting id is the whole handshake."</p>
            <button class="cta" on:click=move |_| go_join("/join", Default::default())>
                "start a meeting"
            </button>
        </section>

        <div class="team">
            <h2>"the keystrokes"</h2>
            <ul>
                <li>"Khush Mehta"</li>
                <li>"Amir Karim"</li>
                <li>"Ishan Maheshwari"</li>
                <li>"Chetan Chaudhary"</li>
            </ul>
        </div>
    }
}

/// Create or join. Its own page, so the landing stays a read.
#[component]
fn JoinPage() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let meeting = RwSignal::new(String::new());
    let navigate = use_navigate();

    let join = Callback::new(move |()| {
        let (who, id) = (
            name.get_untracked().trim().to_string(),
            meeting.get_untracked().trim().to_string(),
        );
        if !who.is_empty() && !id.is_empty() {
            navigate(&format!("/room/{id}?name={who}"), Default::default());
        }
    });

    view! {
        <div class="portal">
            <div class="portal-card">
                <a class="back" href="/">"← back"</a>

                <p class="eyebrow">
                    <i/> "create or join" <b/> "rt / 002"
                </p>

                <h1 class="portal-title">
                    <span>"Take a "</span>
                    <span class="accent">"seat."</span>
                </h1>

                <p class="tagline">
                    "Same meeting id, same table. No account, no invite link."
                </p>

                <form class="join" on:submit=move |ev| {
                    ev.prevent_default();
                    join.run(());
                }>
                    <div class="field">
                        <label for="who">"your name"</label>
                        <input
                            id="who"
                            type="text"
                            placeholder="Ada Lovelace"
                            autofocus
                            prop:value=move || name.get()
                            on:input:target=move |ev| name.set(ev.target().value())
                        />
                    </div>

                    <div class="field">
                        <label for="meeting">"meeting id"</label>
                        <input
                            id="meeting"
                            type="text"
                            placeholder="design-sync"
                            prop:value=move || meeting.get()
                            on:input:target=move |ev| meeting.set(ev.target().value())
                        />
                    </div>

                    <button class="cta" type="submit">"join meeting"</button>
                    <p class="hint">"the id is the room — share it out loud"</p>
                </form>

                <div class="hud">
                    <span>"● signal found"</span>
                    <span>"camera stays local"</span>
                </div>
            </div>
        </div>
    }
}

#[component]
fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();

    // Display name comes from the landing page as ?name=
    let query = use_query_map();
    let who = move || {
        query
            .get()
            .get("name")
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| String::from("guest"))
    };

    let webrtc = use_webrtc();
    let navigate = use_navigate();
    let join = webrtc.join;
    let name = who;

    Effect::new(move |_| join.run((RoomId(id()), name())));

    view! {
        <div class="room">
            <h1>"roundtable / " {move || id()}</h1>
            <p class="status">{move || webrtc.status.get()}</p>

            <Show when=move || webrtc.error.get().is_some()>
                <p class="error">{move || webrtc.error.get().unwrap_or_default()}</p>
            </Show>

            <div class="grid">
                <Video stream=move || webrtc.local.get() name=format!("{} (you)", who()) mirror=true/>
                <For each=move || webrtc.peers.get() key=|peer| peer.info.id.0.clone() let:peer>
                    <Video stream=move || Some(peer.stream.clone()) name=peer.info.name.clone()/>
                </For>
            </div>

            <button class="leave" on:click=move |_| navigate("/", Default::default())>
                "Leave"
            </button>
        </div>
    }
}

#[component]
fn Video(
    stream: impl Fn() -> Option<web_sys::MediaStream> + 'static,
    name: String,
    #[prop(optional)] mirror: bool,
) -> impl IntoView {
    let video_ref = NodeRef::<leptos::html::Video>::new();

    Effect::new(move |_| {
        if let (Some(el), Some(stream)) = (video_ref.get(), stream()) {
            el.set_src_object(Some(&stream));
        }
    });

    view! {
        <figure class="tile">
            <video
                node_ref=video_ref
                autoplay
                playsinline
                muted=true
                class=if mirror { "mirror" } else { "peer" }
            ></video>
            <figcaption>{name}</figcaption>
        </figure>
    }
}
