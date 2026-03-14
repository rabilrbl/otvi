## Why

The Docusaurus site currently ships only a single live docs set, has no blog surface for release communication, and its automation is oriented around branch pushes instead of versioned documentation releases. As OTVI evolves, maintainers need a predictable way to preserve version-specific docs, publish release notes as blog posts, and automatically deploy updated docs when tagged releases are cut.

## What Changes

- Enable Docusaurus versioned documentation so tagged releases can keep a stable snapshot of the docs alongside the in-progress `next` docs.
- Turn on the Docusaurus blog and expose a `Blogs` entry in the main navbar next to `Docs`, with configuration aligned to Docusaurus best practices for edit links, feeds, and navigation.
- Update docs-site structure and guidance so versioning and release-blog workflows are clear for maintainers.
- Update GitHub Actions so documentation is built and published automatically for new release tags, while preserving the existing main-branch docs deployment flow where appropriate.

## Capabilities

### New Capabilities
- `versioned-documentation-site`: Manage versioned docs, release-blog publishing, and automated tagged-release docs deployment for the Docusaurus site.

### Modified Capabilities
- None.

## Impact

- Affected docs site configuration: `docs/docusaurus.config.ts`, `docs/package.json`, `docs/README.md`, and versioning metadata under `docs/`
- Affected docs content structure: docs versions, blog content scaffolding, navbar/footer navigation, and maintainers' publishing workflow
- Affected CI/CD: `.github/workflows/docs-deploy.yml` and related release-tag publishing automation
- Affected project process: release documentation now includes version snapshots and optional blog posts for tagged releases
