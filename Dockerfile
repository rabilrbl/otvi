# ── Stage 1: Build the WASM frontend ─────────────────────────────────────────
FROM rust:1.94-bookworm AS build-web

COPY --from=oven/bun:1 /usr/local/bin/bun /usr/local/bin/bun
RUN ln -s /usr/local/bin/bun /usr/local/bin/bunx

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY web web

WORKDIR /app/web
RUN bun install --frozen-lockfile
RUN trunk build --release

# ── Stage 2: Build the server binary ─────────────────────────────────────────
FROM rust:1.94-bookworm AS build-server

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY web web
COPY --from=build-web /app/dist /app/dist

# Build with optimised release profile (LTO + single codegen unit).
# The profile is defined in Cargo.toml [profile.release] below.
RUN OTVI_EMBED_FRONTEND=1 cargo build --release -p otvi-server

# ── Stage 3: Runtime image ───────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Server binary
COPY --from=build-server /app/target/release/otvi-server /app/otvi-server

# Provider configs are mounted at runtime
RUN mkdir -p /app/providers

# ── Environment ───────────────────────────────────────────────────────────────
ENV PORT=3000
ENV PROVIDERS_DIR=/app/providers
ENV RUST_LOG=otvi_server=info
# Set to "json" for structured log output (e.g. Loki, Datadog, CloudWatch).
ENV LOG_FORMAT=text
# Set CORS_ORIGINS explicitly at deploy time, e.g. "https://tv.example.com".
# Leave unset in the image so production deployments do not default to permissive CORS.

EXPOSE 3000

# ── Health check ──────────────────────────────────────────────────────────────
# Docker / Kubernetes will call /healthz every 30 s and restart the container
# if it fails 3 times in a row.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD wget -qO- http://localhost:${PORT}/healthz || exit 1

ENTRYPOINT ["/app/otvi-server"]
