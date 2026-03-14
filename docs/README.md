# OTVI Documentation

This directory contains the OTVI documentation site, built with [Docusaurus](https://docusaurus.io/).

The site supports both versioned docs and a blog for release posts. The active documentation lives in `docs/docs/`, frozen releases are stored in Docusaurus versioned-docs artifacts, and release posts live under `docs/blog/` and are served from `/blogs`.

## Prerequisites

- [Bun](https://bun.sh/) (recommended) or Node.js 20+

## Development

```bash
# Install dependencies
bun install

# Start the development server
bun start

# Build for production
bun run build

# Create a new versioned docs snapshot
bun run docs:version <version>

# Serve the production build locally
bun run serve
```

## Release Workflow

Use this checklist when preparing a release that should publish updated documentation.

1. Update the in-progress docs in `docs/docs/`.
2. If the release needs public notes, add a new post in `docs/blog/` using the existing blog front matter structure.
3. Create a frozen docs snapshot with `bun run docs:version <version>`.
4. Review the generated artifacts in `docs/versions.json`, `docs/versioned_docs/`, and `docs/versioned_sidebars/`.
5. Verify the site locally with `bun install --frozen-lockfile` and `bun run build:release`.
6. Commit the docs changes, then create and push a release tag in the `vX.Y.Z` format.

Only `v*` tags are treated as release-doc publishing tags. Existing ad hoc or test tags outside that pattern are ignored by the docs deploy workflow.

## Project Structure

```
docs/
├── blog/                  # Release notes and project announcements
├── docs/                  # Markdown documentation pages
│   ├── introduction.md
│   ├── getting-started.md
│   ├── architecture.md
│   ├── configuration.md
│   ├── providers/         # Provider guide
│   ├── api-reference/     # API reference
│   ├── frontend.md
│   ├── deployment.md
│   └── admin-guide.md
├── versioned_docs/        # Frozen docs snapshots created by Docusaurus
├── versioned_sidebars/    # Sidebar snapshots for versioned docs
├── versions.json          # Published docs versions metadata
├── src/
│   ├── pages/             # Custom pages (landing page)
│   └── css/               # Custom styles
├── docusaurus.config.ts   # Docusaurus configuration
├── sidebars.ts            # Sidebar navigation
└── package.json
```
