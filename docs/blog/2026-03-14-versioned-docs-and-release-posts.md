---
slug: versioned-docs-and-release-posts
title: Versioned docs and release posts are now part of the site workflow
authors: [rabil]
tags: [release, documentation]
---

The OTVI documentation site now supports two publishing tracks:

<!-- truncate -->

- versioned documentation snapshots for stable releases
- blog posts for release notes and project updates

When a release is ready, cut a docs version with `bun run docs:version <version>`, add a blog post if the release needs public notes, and publish with a `vX.Y.Z` git tag.

This keeps the in-progress `Next` docs separate from frozen release documentation while giving future releases a clear place to explain notable changes.
