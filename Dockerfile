# ── Stage 1: Build the WASM frontend ─────────────────────────────────────────
FROM rust:1.94-bookworm AS build-web

ARG TRUNK_VERSION=0.21.14
ARG TARGETARCH

COPY --from=oven/bun:1 /usr/local/bin/bun /usr/local/bin/bun
RUN ln -s /usr/local/bin/bun /usr/local/bin/bunx

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN case "${TARGETARCH}" in \
        amd64) trunk_target=x86_64-unknown-linux-gnu ;; \
        arm64) trunk_target=aarch64-unknown-linux-gnu ;; \
        *) printf 'Unsupported TARGETARCH: %s\n' "${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && rustup target add wasm32-unknown-unknown \
    && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${trunk_target}.tar.gz" \
    | tar -xz -C /usr/local/bin trunk

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
    ca-certificates wget \
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
