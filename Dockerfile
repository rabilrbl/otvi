# ── Stage 1: Build the WASM frontend ─────────────────────────────────────────
FROM rust:1.83-bookworm AS build-web

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked

WORKDIR /app
COPY Cargo.toml ./
COPY crates/otvi-core crates/otvi-core
COPY web web

WORKDIR /app/web
RUN trunk build --release

# ── Stage 2: Build the server binary ─────────────────────────────────────────
FROM rust:1.83-bookworm AS build-server

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates crates

# Build with optimised release profile (LTO + single codegen unit).
# The profile is defined in Cargo.toml [profile.release] below.
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

# ── Environment ───────────────────────────────────────────────────────────────
ENV PORT=3000
ENV PROVIDERS_DIR=/app/providers
ENV STATIC_DIR=/app/dist
ENV RUST_LOG=otvi_server=info
# Set to "json" for structured log output (e.g. Loki, Datadog, CloudWatch).
ENV LOG_FORMAT=text
# Set to your frontend origin in production, e.g. "https://tv.example.com".
# Leave unset or "*" to allow all origins (development only).
ENV CORS_ORIGINS=*

EXPOSE 3000

# ── Health check ──────────────────────────────────────────────────────────────
# Docker / Kubernetes will call /healthz every 30 s and restart the container
# if it fails 3 times in a row.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD wget -qO- http://localhost:${PORT}/healthz || exit 1

ENTRYPOINT ["/app/otvi-server"]
