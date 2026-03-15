# OTVI

OTVI (Open TV Interface) is a YAML-driven television streaming platform. It lets providers expose login, channel browsing, and playback flows through configuration instead of custom per-provider application code.

## What It Includes

- `otvi-server` - Axum backend for auth, provider integration, streaming APIs, and static asset serving
- `otvi-core` - shared types, provider schema, and template/extraction utilities
- `otvi-web` - Leptos WebAssembly frontend for login, channel browsing, and playback
- `docs/` - Docusaurus documentation site with versioned release snapshots and a release blog

## Architecture

```text
providers/*.yaml
      |
      v
+-------------------+       HTTP       +------------------------+
| otvi-server       | ---------------> | provider APIs          |
| - Axum API        |                  | auth / channels / DRM  |
| - schema endpoint |
| - static assets   | <--------------- +------------------------+
+---------+---------+
          |
          | JSON
          v
+-------------------+
| otvi-web          |
| - login           |
| - channels        |
| - player          |
+-------------------+
```

## Quick Start

### Prerequisites

- Rust stable
- `trunk` for the frontend: `cargo install trunk`
- `wasm32-unknown-unknown`: `rustup target add wasm32-unknown-unknown`
- `wasm-pack` for frontend UI tests: `cargo binstall wasm-pack`
- Bun for the docs site and frontend package scripts

### Local Development

```bash
# build the frontend
cd web && trunk build

# run the backend
cargo run -p otvi-server
```

The app serves on `http://localhost:3000` by default.

### Common Commands

```bash
# full Rust test suite
cargo test --workspace --all-features

# frontend UI tests
cd web && wasm-pack test --headless --firefox --features ui-test --lib

# docs site
cd docs && bun install && bun run build
```

## Documentation

- Product and operator docs: `docs/`
- Docs site maintainer workflow: `docs/README.md`
- Contributing guide: `CONTRIBUTING.md`
- Release process: `RELEASING.md`
- Security policy: `SECURITY.md`
- Support guide: `SUPPORT.md`
- Code of conduct: `CODE_OF_CONDUCT.md`

## Repository Workflow

- `dev` is the integration branch for normal pull requests
- `main` stages the next release and should stay closer to release-candidate quality
- Release automation publishes binaries and containers from `vX.Y.Z` tags
- Tagged releases require matching versions in `crates/otvi-core/Cargo.toml`, `crates/otvi-server/Cargo.toml`, and `web/Cargo.toml`
- GHCR images are published without a floating `latest` tag; use `dev`, `main`, `v0`, `v0.1`, or a full release tag such as `v0.1.0`
- Public docs default to the latest released version; unreleased docs remain available separately

Repository files can document and enforce part of this model, but GitHub settings still need administrator setup. See `CONTRIBUTING.md` and `RELEASING.md`.

## License

CC BY-NC-SA 4.0. See `LICENSE`.
