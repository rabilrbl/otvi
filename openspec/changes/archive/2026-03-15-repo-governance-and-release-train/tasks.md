## 1. Repository governance documents and templates

- [x] 1.1 Rewrite `README.md` into a concise repository entrypoint and move deep contributor, support, security, and release guidance into dedicated root Markdown files.
- [x] 1.2 Add community-health documents such as `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `SUPPORT.md`, and `RELEASING.md`, including an explicit section for GitHub-admin-only settings.
- [x] 1.3 Add `.github/ISSUE_TEMPLATE/` files for bug reports, feature requests, and documentation issues, plus `.github/pull_request_template.md` aligned with the new contribution and release flow.

## 2. Branch model and CI workflow refactor

- [x] 2.1 Update `.github/workflows/ci.yml` so pull requests target `dev` and pushes to both `dev` and `main` run the required validation jobs.
- [x] 2.2 Refactor CI workflow structure, naming, permissions, and branch-aware behavior so integration validation and release-train validation are easier to maintain.
- [x] 2.3 Document the operational steps required outside the repository to create `dev`, set the effective default PR base to `dev`, and apply branch protections for `dev` and `main`.

## 3. Tag-driven server release and docs publication

- [x] 3.1 Add a dedicated tag-triggered release workflow for the documented `vX.Y.Z` release tag pattern, including validation that prepared version metadata matches the pushed tag.
- [x] 3.2 Update docs deployment automation so it uses the same server release tag contract and publishes from the tagged repository state.
- [x] 3.3 Encode the release process so `otvi-core`, `otvi-server`, and `otvi-web` version changes are all required for tagged releases.

## 4. Stable-default documentation experience

- [x] 4.1 Update `docs/docusaurus.config.ts` and related docs guidance so the latest released docs version is the default public view and the unreleased docs remain available as a separate version choice.
- [x] 4.2 Update `docs/README.md` and release guidance to describe the reviewed release-preparation flow for docs version snapshots, release notes, and release tags.
- [x] 4.3 Ensure docs links, edit URLs, and release-label wording stay consistent with the `dev`/`main` branch model and the chosen release tag pattern.

## 5. Verification

- [x] 5.1 Validate the updated repository health and template files by reviewing GitHub-facing paths and ensuring all linked guidance resolves correctly.
- [x] 5.2 Run the relevant workflow validation commands and documentation build checks after the workflow and docs changes, fixing any issues introduced by the refactor.
- [x] 5.3 Perform a dry-run review of the documented release checklist to confirm the first release under the new process is executable end to end.
