## Why

The repository currently mixes product documentation, contributor guidance, and release-process details across a small set of files, while CI/CD and docs automation still assume a simple `main`-centric workflow. As the project grows into a larger open source codebase, maintainers need clearer community-health artifacts, a more deliberate `dev` -> `main` release train, and tag-driven release automation that keeps server releases and published docs aligned.

## What Changes

- Streamline the root `README.md` so it focuses on project overview, quick start, architecture, and links to deeper documentation instead of duplicating long-form reference material.
- Add standard contributor and community-health files such as `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `SUPPORT.md`, release guidance, PR templates, and issue templates sized for an actively maintained open source project.
- Refactor GitHub Actions so continuous integration is aligned with a `dev` integration branch and a `main` prerelease branch, with pull requests targeting `dev` by default.
- Add a tag-driven release workflow for publishing `otvi-server` releases, with room to update `otvi-core` and `otvi-web` when intentionally included in a release.
- Update the documentation release flow so tagged releases publish Docusaurus docs for the released version and the docs site defaults to the latest released version instead of presenting `Next` as the primary public label.
- Clarify which parts of the workflow are enforced in-repo versus which require GitHub repository settings (for example default branch, branch protection, and default PR base behavior).

## Capabilities

### New Capabilities
- `repository-community-health`: Standardize maintainer, contributor, support, and issue/PR intake guidance for the repository.
- `branch-based-release-governance`: Define and automate the intended `dev` and `main` branch responsibilities for CI and pull request flow.
- `tag-driven-server-release`: Define how tagged releases publish `otvi-server`, coordinate optional `otvi-core` and `otvi-web` version updates, and document the release contract.

### Modified Capabilities
- `versioned-documentation-site`: Change the docs release workflow so tagged releases publish the stable docs view as the latest released version rather than treating `Next` as the default public docs label.

## Impact

- Affected repository documentation: `README.md`, new root governance docs, and docs-maintainer guidance under `docs/`.
- Affected GitHub metadata: `.github/workflows/`, `.github/ISSUE_TEMPLATE/`, pull request templates, and related repository health files.
- Affected release process: crate/package version management, release tags, documentation publishing, and GitHub branch/review expectations.
- Affected maintainers and contributors: contribution flow, PR targeting, release preparation, and triage conventions become more explicit and enforceable.
