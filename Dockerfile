# Built inside the image on purpose: the host is CachyOS with glibc 2.44, which
# no Debian/Ubuntu base is new enough to run a host-built binary on.
FROM rust:1-bookworm AS build

# cargo-leptos builds both halves (server + wasm) and wires up wasm-bindgen.
RUN cargo install cargo-leptos --locked

# The wasm toolchain lives in its own layer so editing source never
# re-downloads it. wasm-opt is deliberately skipped via metadata below --
# it is a multi-minute download on every cold build and buys nothing here.
RUN rustup target add wasm32-unknown-unknown \
 && cargo install wasm-bindgen-cli --version 0.2.129 --locked

WORKDIR /app

# Manifests first so dependency compilation survives source-only edits.
COPY Cargo.toml Cargo.lock ./
COPY app/Cargo.toml app/
COPY frontend/Cargo.toml frontend/
COPY server/Cargo.toml server/
RUN mkdir -p app/src frontend/src server/src \
 && echo "" > app/src/lib.rs && echo "" > frontend/src/lib.rs && echo "fn main() {}" > server/src/main.rs \
 && cargo build --release -p server --locked \
 && rm -rf app/src frontend/src server/src

COPY . .
# Touch so cargo-leptos does not reuse the placeholder's fingerprint.
RUN touch app/src/lib.rs server/src/main.rs

# Keep this as plain --release. cargo-leptos picks the profile itself, and the
# COPY below hardcodes target/release/server, so the two must agree: with no
# --release the binary lands in target/debug and the COPY picks up the stub left
# behind by the dependency-cache step, so the container exits 0 and the tunnel
# 502s. Do not parameterise the profile without also fixing the copy path.
RUN cargo leptos build --release

FROM debian:bookworm-slim AS runtime

# ca-certificates so reqwest can validate Cloudflare's TLS certs.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /app/target/release/server /usr/local/bin/server
COPY --from=build /app/target/site /srv/site

# site-addr in Cargo.toml is 127.0.0.1, which the tunnel cannot reach.
ENV LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_SITE_ROOT=/srv/site \
    RUST_LOG=info

EXPOSE 3000
CMD ["/usr/local/bin/server"]