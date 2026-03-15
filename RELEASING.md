# Releasing OTVI

This repository uses a `dev` -> `main` release train.

- `dev` is the integration branch for day-to-day work.
- `main` stages the next release.
- `vX.Y.Z` tags trigger the release workflows.

## Release Scope

- Every release tag requires matching versions in `crates/otvi-core/Cargo.toml`, `crates/otvi-server/Cargo.toml`, and `web/Cargo.toml`.
- Release binaries include both a bundled `otvi-server` artifact with the frontend embedded and a server-only API artifact.
- GHCR publishes two images: `ghcr.io/rabilrbl/otvi` (bundled frontend) and `ghcr.io/rabilrbl/otvi-server` (API only).

## Release Preparation Checklist

1. Promote the intended changes from `dev` into `main`.
2. Update `crates/otvi-core/Cargo.toml`, `crates/otvi-server/Cargo.toml`, and `web/Cargo.toml` to the target release version.
4. Update in-progress docs in `docs/docs/`.
5. Add a release blog post in `docs/blog/` if public notes are needed.
6. Create the docs snapshot with `cd docs && bun run docs:version <version>`.
7. Verify `docs/versions.json` lists the new version first and that the matching `versioned_docs/` and `versioned_sidebars/` artifacts exist.
8. Run the validation commands:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd docs && bun install --frozen-lockfile && bun run build:release
```

9. Commit the release-preparation changes on `main`.
10. Create and push the release tag: `vX.Y.Z`.
11. Confirm the binary release, docs deployment, and GHCR publishing workflows all succeed.

## What the Release Workflow Validates

- the pushed tag matches `vX.Y.Z`
- `crates/otvi-core/Cargo.toml`, `crates/otvi-server/Cargo.toml`, and `web/Cargo.toml` all match the tagged version
- the docs snapshot exists for that version
- `docs/versions.json` promotes that version as the latest released docs version

## GitHub Administrator Checklist

These setup steps happen outside the repository:

1. Create the `dev` branch in GitHub.
2. Make `dev` the default branch so GitHub defaults PRs there.
3. Protect `dev` with required CI checks.
4. Protect `main` with tighter release-train controls.
5. Ensure GitHub Pages is configured for Actions-based deployment.
6. Ensure the repository token permissions allow release creation and Pages deployment.

## Rollback Guidance

- If a tag fails validation, fix the prepared repository state and create a new tag.
- Avoid mutating release metadata from workflows after the tag has been pushed.
- If a bad release is published, cut a new corrective release instead of rewriting history.

## Published Tags

- Branch images: `dev`, `main`
- Major line image: `v0`
- Major/minor image: `v0.1`
- Exact release image: `v0.1.0`
