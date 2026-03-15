## Context

The repository currently has a small-project shape: `README.md` carries orientation plus deep operational reference material, contributor-facing repository health files are mostly absent, CI assumes `main` is the primary integration branch, and docs deployment already responds to `v*` tags but still presents unreleased docs as `Next` in the public site.

The requested change is cross-cutting because it touches repository governance, contribution flow, GitHub metadata, Docusaurus versioning behavior, and release automation. It also spans both in-repo enforcement and out-of-band GitHub settings: repository files can define workflows, templates, and documented policy, but default branch selection, default PR base behavior, and branch protection rules must still be configured in GitHub.

Current project constraints that shape the design:

- The codebase is a Rust workspace with `otvi-core`, `otvi-server`, and `otvi-web` packages already versioned independently in their manifests.
- The documentation site uses Docusaurus with repository-backed version snapshots under `docs/versions.json`, `docs/versioned_docs/`, and `docs/versioned_sidebars/`.
- Existing docs automation already deploys from release tags, and the current OpenSpec guidance for docs favors checked-in version artifacts over generating or mutating them during deploy.
- The requested branch model is intentionally asymmetric: `dev` is the collaboration branch, while `main` holds the next release train and acts like a prerelease or beta branch.

## Goals / Non-Goals

**Goals:**

- Split repository orientation, contributor policy, support guidance, security reporting, and release operations into purpose-built files instead of overloading `README.md`.
- Provide standard GitHub intake surfaces for large-project maintenance, including issue templates and a pull request template.
- Align CI with the desired branch model: normal contribution flow targets `dev`, while `main` remains the release-train branch.
- Establish a documented, tag-driven server release contract that makes `otvi-server` the canonical release stream while allowing `otvi-core` and `otvi-web` to move only when explicitly included.
- Change the public docs posture so the latest released docs are the default stable view, while unreleased docs remain available as the in-progress version.
- Make all required manual GitHub settings explicit so repository automation and administrator configuration stay consistent.

**Non-Goals:**

- Changing application runtime behavior in `crates/otvi-server/`, `crates/otvi-core/`, or `web/` beyond version metadata and release-facing packaging concerns.
- Introducing a brand-new release platform or custom documentation system outside GitHub Actions and Docusaurus.
- Fully automating GitHub repository settings that must be applied by an administrator outside the repository.
- Creating per-component independent release trains for every package in the workspace.

## Decisions

### 1. Use standard repository health documents and GitHub-native templates

The repository will add purpose-specific Markdown documents at the root (for example `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `SUPPORT.md`, and `RELEASING.md`) plus GitHub issue and pull request templates under `.github/`.

Rationale:

- This matches common open source expectations and GitHub community health conventions.
- It keeps `README.md` concise and discoverable while still making deeper process docs available.
- It reduces duplication between the root README and the Docusaurus site.

Alternatives considered:

- Keep all contributor and release guidance in `README.md`: simple, but it does not scale and makes the primary entrypoint noisy.
- Move everything into the docs site only: useful for long-form docs, but weaker for GitHub-native contributor flow and community profile discovery.

### 2. Treat `dev` as the integration branch and `main` as the release-train branch

Repository docs and workflows will encode the intended responsibilities as follows:

- `dev`: default target for feature and fix pull requests, primary CI branch, active integration surface.
- `main`: branch used to stage the next release, receive promoted changes from `dev`, and represent unreleased but stabilizing work.

Rationale:

- This preserves a stabilization lane before release without requiring long-lived feature branches to target the release branch directly.
- It matches the user's intended mental model of `main` as "next release" rather than day-to-day integration.

Alternatives considered:

- Conventional GitHub flow with all work targeting `main`: simpler, but it does not match the requested release discipline.
- GitFlow-style multiple long-lived release/hotfix branches: more formal, but heavier than needed for this project.

Platform limitation:

- The default pull request base branch cannot be fully enforced by repository files alone; the proposal will document the required GitHub setting change and branch protection expectations.

### 3. Use an explicit server release tag pattern and make `otvi-server` the canonical public release stream

The release workflow will be designed around a single documented server-release tag pattern, preferably `server-vX.Y.Z`, rather than a generic `vX.Y.Z` tag.

Rationale:

- The project already contains multiple versioned packages (`otvi-core`, `otvi-server`, `otvi-web`), so an explicit server tag avoids ambiguity if component-specific releases are introduced later.
- It makes the docs publication contract clearer because the docs version tracks the canonical server release rather than an unspecified workspace version.

Alternatives considered:

- Keep generic `vX.Y.Z` tags: shorter and compatible with current docs automation, but less explicit once multiple publishable components exist.
- Create fully separate tag families for every package: precise, but more operational overhead than the current request calls for.

### 4. Require explicit release preparation and use tag-driven CD for validation and publication

The repository will favor a release-preparation flow in which manifests, docs versions, and release notes are updated in normal commits before the release tag is pushed. The tag-driven workflow then validates the prepared state, publishes the server release, and deploys the matching docs.

Rationale:

- Checked-in version changes and Docusaurus version artifacts are reviewable in pull requests.
- It avoids workflows that mutate manifests or generate long-lived docs artifacts during deploy.
- It reduces the chance of a tag producing a release that does not match repository state.

Alternatives considered:

- Auto-commit version bumps and docs snapshots from the tag workflow: attractive at first glance, but it creates opaque, hard-to-review automation and increases rollback complexity.
- Manual release publication with no tag-triggered automation: flexible, but weaker in repeatability and auditability.

Implementation consequence:

- `otvi-server` version changes are mandatory for a server release.
- `otvi-core` and `otvi-web` version changes are optional and must only be included when maintainers intentionally prepare them for the same release.
- The release workflow should fail on version mismatch or missing release artifacts instead of silently changing unrelated files.

### 5. Make the latest released docs the stable default and keep unreleased docs as an explicit in-progress version

The Docusaurus configuration will be adjusted so the latest released version becomes the default public docs experience, while the current unreleased docs remain available in version navigation as the forward-looking branch of documentation.

Rationale:

- External users typically want the latest released behavior by default, not unreleased changes.
- The project already has versioned docs artifacts, so Docusaurus can present stable-versus-current views using first-party capabilities.
- This better matches the new `dev`/`main` release-train structure, where unreleased docs are valuable but should not overshadow stable guidance.

Alternatives considered:

- Keep `current`/`Next` as the default landing experience: easier to maintain, but confusing for users who expect released software documentation.
- Publish separate docs sites per tag: possible, but duplicates built-in Docusaurus version navigation and complicates discoverability.

### 6. Separate CI, release, and docs concerns into clearer workflow responsibilities

The GitHub Actions layout should be refactored so continuous integration, tag-driven release publication, and documentation deployment have distinct responsibilities and branch/tag triggers.

Rationale:

- CI policies for `dev` and `main` are different from release publication concerns.
- Docs deployment should be tied to the release contract and repository-backed versioning artifacts.
- Clear workflow boundaries make later maintenance and permissions review easier.

Alternatives considered:

- Keep all current behavior in a couple of broad workflows: workable, but harder to reason about once branch governance and release automation become stricter.

## Risks / Trade-offs

- [Risk] The nonstandard `dev` -> `main` flow may confuse outside contributors. -> Mitigation: document branch roles prominently in `CONTRIBUTING.md`, PR templates, and release guidance.
- [Risk] Some expectations, such as default PR base branch and branch protection, cannot be enforced entirely in-repo. -> Mitigation: add an explicit administrator checklist and call out required GitHub settings in repository docs.
- [Risk] Moving material out of `README.md` can temporarily make information feel harder to find. -> Mitigation: keep the README focused but link clearly to the new governance and docs entrypoints.
- [Risk] Tag naming changes from `v*` to `server-v*` may break existing maintainer habits or scripts. -> Mitigation: document the new tag contract and update workflows consistently in one change.
- [Risk] Requiring prepared manifest/docs versions before tagging adds release ceremony. -> Mitigation: provide `RELEASING.md` with a repeatable checklist and keep the release workflow deterministic.
- [Risk] Stable-default docs can drift from unreleased branch docs if maintainers forget to version snapshots. -> Mitigation: validate docs-version artifacts as part of the documented release preparation flow.

## Migration Plan

1. Add repository health documents, templates, and contributor guidance while trimming `README.md` to a concise orientation document.
2. Refactor CI to run for pull requests targeting `dev` and for pushes to both `dev` and `main`, while documenting the required GitHub repository settings.
3. Introduce the server-release workflow and align docs deployment triggers with the chosen release tag pattern.
4. Update Docusaurus configuration and docs maintainer guidance so the latest released version is the default public docs surface.
5. Create the `dev` branch and apply GitHub default-branch, branch-protection, and PR-base settings outside the repository.
6. Validate the first release under the new process by preparing release metadata in-repo, pushing the release tag, and confirming matching docs deployment.

## Open Questions

- Should the first implementation preserve backward compatibility with existing `v*` tags for a transition period, or switch directly to `server-v*` only?
- What exact server artifact target should the release workflow publish first (for example GitHub Release assets only, container images, or both)?
- Should the unreleased docs label remain `Next`, or should it be renamed to something more explicit such as `Unreleased` or `Main` once stable docs become default?
