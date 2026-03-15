# OTVI Documentation

This directory contains the Docusaurus documentation site for OTVI.

## Docs Model

- `docs/docs/` contains the unreleased documentation tracked on `main`
- `docs/versioned_docs/`, `docs/versioned_sidebars/`, and `docs/versions.json` store release snapshots checked into the repository
- the published site defaults to the latest released docs version
- the unreleased docs remain available in version navigation as `Unreleased`
- release notes and announcements live in `docs/blog/`

## Development

```bash
bun install
bun start
bun run build
bun run serve
```

## Maintainer Release Workflow

Use this flow when preparing a server release that updates public docs.

1. Update unreleased docs in `docs/docs/` on `main`.
2. Add a release blog post in `docs/blog/` when public release notes are needed.
3. Create the docs snapshot with `bun run docs:version <version>`.
4. Confirm `docs/versions.json` lists the new version first.
5. Confirm the generated artifacts exist in `docs/versioned_docs/version-<version>/` and `docs/versioned_sidebars/version-<version>-sidebars.json`.
6. Validate the site with `bun install --frozen-lockfile` and `bun run build:release`.
7. Commit the docs changes as part of the reviewed release preparation.
8. Confirm `crates/otvi-core/Cargo.toml`, `crates/otvi-server/Cargo.toml`, and `web/Cargo.toml` all match `<version>`.
9. Push the matching release tag: `v<version>`.

The docs deployment workflow publishes on:

- pushes to `main`, so unreleased docs remain visible in version navigation
- `v*` tags, so the tagged release state is published immediately

## Edit Links and Branching

- public stable docs default to the latest released version snapshot
- `Unreleased` docs represent the next release train tracked from `main`
- normal code and docs pull requests should still target `dev` unless the work is part of documented release promotion

## Project Structure

```text
docs/
|- blog/
|- docs/
|- versioned_docs/
|- versioned_sidebars/
|- versions.json
|- src/
|- docusaurus.config.ts
|- sidebars.ts
`- package.json
```
