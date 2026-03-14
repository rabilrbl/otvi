## 1. Enable Docusaurus versioning and blog surfaces

- [x] 1.1 Update `docs/docusaurus.config.ts` to enable versioned docs and the Docusaurus blog, including a `Blogs` navbar item, blog/edit-link metadata, and version-navigation settings that follow Docusaurus guidance.
- [x] 1.2 Create the initial checked-in versioning artifacts for the current docs baseline (`versions.json`, versioned docs, and versioned sidebars) using the supported Docusaurus versioning workflow.
- [x] 1.3 Add the blog directory and initial release-post scaffolding needed so future release notes can be published through the Docusaurus blog system.

## 2. Document the maintainer release workflow

- [x] 2.1 Update `docs/README.md` with a release checklist covering how to cut a docs version, where generated artifacts are stored, and how to add a release blog post before tagging.
- [x] 2.2 Review related docs-site metadata or package scripts and add any minimal workflow helpers needed to keep versioning and release-post creation repeatable.

## 3. Automate tagged docs publishing

- [x] 3.1 Update `.github/workflows/docs-deploy.yml` so the docs build and Pages deployment run for both docs-related pushes to `main` and release tags that match the project's documented tag pattern.
- [x] 3.2 Ensure the docs deployment workflow builds from the checked-out triggering ref, keeps concurrency safe, and continues to install/build the Docusaurus site with Bun.

## 4. Verify the docs release flow

- [x] 4.1 Run the docs site's install, typecheck, and build commands after the configuration changes and fix any issues introduced by versioning or blog enablement.
- [x] 4.2 Validate that the generated site includes version navigation, a working `Blogs` header entry, and deployment-ready output for both the latest docs and frozen version snapshots.
