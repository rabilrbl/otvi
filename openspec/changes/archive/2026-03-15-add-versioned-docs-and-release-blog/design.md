## Context

The repository already contains a Docusaurus site under `docs/`, but it is configured as a single-version documentation site with the blog disabled and a navbar that only exposes documentation and GitHub. The deployment workflow publishes on pushes to `main`, which is useful for keeping the site current but does not establish a release process for frozen doc snapshots tied to git tags.

This change spans site configuration, content structure, maintainers' workflow, and CI automation. It must stay within Docusaurus' supported versioning and blog features, use the existing Bun-based docs toolchain, and avoid inventing a custom release-publishing mechanism when Docusaurus and GitHub Actions already provide the needed primitives.

## Goals / Non-Goals

**Goals:**
- Enable Docusaurus versioned docs using the built-in versioning workflow and repository metadata.
- Expose a first-class blog section in the site header so future release posts have a stable home.
- Make the docs configuration follow common Docusaurus best practices, including explicit edit links, feed generation, and maintainable navbar/footer setup.
- Extend docs deployment automation so tagged releases publish the newly versioned docs site without requiring a separate manual deployment step.
- Document the maintainer workflow for cutting doc versions and adding release blog posts.

**Non-Goals:**
- Redesigning the docs site's visual identity beyond configuration and navigation changes needed for the new content surfaces.
- Generating release blog content automatically from git history or GitHub Releases.
- Introducing a second hosting platform or moving away from GitHub Pages.
- Changing application runtime code outside what is necessary to support docs and release publishing workflows.

## Decisions

### 1. Use built-in Docusaurus docs versioning instead of a custom tagged-docs layout

The site will adopt Docusaurus' native versioned docs flow, including `versions.json`, `versioned_docs`, and `versioned_sidebars`, rather than building a custom tag-to-folder publishing convention.

Rationale:
- This is the documented Docusaurus approach and keeps future upgrades straightforward.
- It allows the site to present `next` docs plus frozen release versions without custom routing code.
- It keeps version metadata explicit in the repository, which is easier to review in pull requests.

Alternatives considered:
- Build one site per tag and publish to separate paths manually: workable, but duplicates Docusaurus behavior and complicates navigation.
- Keep only latest docs and link to release notes elsewhere: simpler, but does not satisfy the need for version-based documentation.

### 2. Enable the blog through the classic preset and surface it directly in the navbar

The Docusaurus blog will be enabled in the existing classic preset, and a `Blogs` navbar item will be added alongside `Docs` so release communication stays visible and consistent with the main documentation experience.

Rationale:
- The blog is already a supported first-party Docusaurus content type with RSS/Atom feeds, archive pages, and edit-link support.
- A top-level navbar entry matches the user's request and makes future release posts discoverable without adding custom pages.

Alternatives considered:
- Put release notes under docs only: this preserves versioning but loses the chronological publishing model that blogs provide.
- Link externally to GitHub Releases: useful as a complement, but weaker for curated release narratives and site search/navigation.

### 3. Tagged release publishing will extend the existing docs deployment workflow, not replace it

The GitHub Pages deployment workflow will be updated to run for both `main` pushes affecting docs and release tags that indicate a new published version. The workflow should build from the checked-out ref so a release tag deploys the site content associated with that release.

Rationale:
- One workflow keeps Pages deployment logic centralized.
- Tag-triggered builds align with release publication while preserving branch-triggered previews for the latest docs site state.
- Building from the tagged ref ensures the generated site reflects the exact checked-in versioned docs metadata.

Alternatives considered:
- Create a separate release-docs workflow: more separation, but duplicates build and deploy logic.
- Publish docs only on `main` after tagging: fragile, because it depends on post-tag branch state matching the intended release snapshot.

### 4. Release workflow guidance will rely on explicit versioning commands and checked-in content

Maintainers will create a docs version explicitly before tagging a release and will add a matching blog post when a release warrants public notes. The repository documentation will describe the commands, expected generated files, and the order of operations.

Rationale:
- Docusaurus versioning is an explicit repository change, so maintainers need documented steps rather than hidden automation.
- Keeping blog posts checked in preserves reviewability and lets release content ship with the same commit/tag as the version snapshot.

Alternatives considered:
- Auto-create versions inside CI from tags: attractive, but it mutates generated artifacts during deploy and makes review difficult.
- Leave the process undocumented: likely to create inconsistent version snapshots and forgotten blog setup.

## Risks / Trade-offs

- [Tag-triggered deploys may race with main-branch deploys] -> Mitigation: keep Pages workflow concurrency enabled with a single shared group so deployments serialize cleanly.
- [If maintainers forget to cut a docs version before tagging, the deployed release may not contain a frozen snapshot] -> Mitigation: document the required release checklist in `docs/README.md` and CI comments where practical.
- [Adding blog support increases site maintenance surface] -> Mitigation: use Docusaurus defaults and keep blog scope focused on release communication.
- [Checked-in versioned docs can increase repository size over time] -> Mitigation: version only on meaningful releases and rely on Docusaurus' intended storage model.

## Migration Plan

- Add Docusaurus versioning metadata and blog configuration in the docs site.
- Create the initial version snapshot from the current stable docs baseline and check in the generated versioned docs artifacts.
- Add blog scaffolding and maintainer documentation for future release posts.
- Update the GitHub Actions workflow to build and deploy on both `main` docs changes and new release tags.
- After merge, use the documented release process for the next tagged release; if rollback is required, revert the docs-site and workflow changes together so configuration and deployment behavior stay aligned.

## Open Questions

- Which tag pattern should be treated as a release-docs publish trigger if the repository uses both semantic tags and ad hoc tags?
- Should every tagged release require a blog post, or should the workflow support tags with versioned docs only?
