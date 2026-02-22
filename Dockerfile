# ── Stage 1: Build the WASM frontend ─────────────────────────────────────────
FROM rust:1.93-bookworm AS build-web

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked

WORKDIR /app
COPY Cargo.toml ./
COPY crates/otvi-core crates/otvi-core
COPY web web

WORKDIR /app/web
RUN trunk build --release

# ── Stage 2: Build the server binary ─────────────────────────────────────────
FROM rust:1.93-bookworm AS build-server

WORKDIR /app
COPY Cargo.toml ./
COPY crates crates

RUN cargo build --release -p otvi-server

# ── Stage 3: Runtime image ───────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Server binary
COPY --from=build-server /app/target/release/otvi-server /app/otvi-server

# Frontend assets
COPY --from=build-web /app/dist /app/dist

# Provider configs are mounted at runtime
RUN mkdir -p /app/providers

ENV PORT=3000
ENV PROVIDERS_DIR=/app/providers
ENV STATIC_DIR=/app/dist
ENV RUST_LOG=otvi_server=info

EXPOSE 3000

ENTRYPOINT ["/app/otvi-server"]
