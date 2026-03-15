# Contributing to OTVI

Thanks for contributing to OTVI.

## Branch Model

- `dev` is the default integration branch for features, fixes, refactors, and docs improvements.
- `main` carries the next release train and is used to stabilize the upcoming release.
- Unless a maintainer explicitly asks otherwise, open pull requests against `dev`.

## Local Setup

### Prerequisites

- Rust stable
- `wasm32-unknown-unknown`: `rustup target add wasm32-unknown-unknown`
- `trunk`: `cargo install trunk`
- `wasm-pack`: `cargo binstall wasm-pack`
- Bun for docs work and frontend package scripts

### Start the project

```bash
# backend
cargo run -p otvi-server

# frontend build
cd web && trunk build

# docs
cd docs && bun install && bun start
```

## Development Expectations

- Keep pull requests focused and reviewable.
- Add or update docs when behavior, workflows, or release procedures change.
- Follow existing Rust and workflow conventions instead of introducing parallel patterns.
- Prefer explicit, deterministic automation over hidden repository mutation.

## Validation Checklist

Run the relevant commands for the files you touched before opening a pull request.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd web && wasm-pack test --headless --firefox --features ui-test --lib
cd docs && bun install --frozen-lockfile && bun run build:release
```

If a command is not relevant to your change, say so in the pull request.

## Pull Request Guidelines

- Target `dev` unless the change is part of documented release-train work.
- Describe the change, why it exists, and how you validated it.
- Call out docs impact, release impact, and any GitHub-admin follow-up.
- Keep unrelated cleanup out of the same pull request.

## Issue Intake

- Bug reports: use the bug issue template
- Feature requests: use the feature request template
- Documentation problems: use the docs issue template
- Security reports: follow `SECURITY.md` instead of opening a public issue

## GitHub Administrator Setup

These settings must be configured in GitHub and are not fully enforceable from repository files alone:

1. Create the `dev` branch from the desired integration baseline.
2. Make `dev` the default branch so GitHub's default PR base follows the project workflow automatically.
3. Protect `dev` with required checks for CI.
4. Protect `main` with stricter merge controls for release-train promotion.
5. Confirm bots and automations such as Dependabot target `dev`.

## Release Work

If your change affects release automation, docs versioning, Docker publishing, or package versions, review `RELEASING.md` before opening the pull request.
