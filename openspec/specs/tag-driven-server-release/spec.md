## ADDED Requirements

### Requirement: Server release automation MUST use one documented server tag format
The repository SHALL define one canonical tag pattern for `otvi-server` releases, and release publication workflows SHALL trigger only for tags that match that documented server-release pattern.

#### Scenario: Matching server release tag is pushed
- **WHEN** a maintainer pushes a tag that matches the documented `otvi-server` release pattern
- **THEN** GitHub Actions starts the server release workflow for that tag

#### Scenario: Non-release tag is pushed
- **WHEN** a maintainer pushes a tag that does not match the documented `otvi-server` release pattern
- **THEN** the server release workflow does not run

### Requirement: The release process MUST use explicit component version scope
The repository SHALL treat `otvi-server` as the canonical public release stream. A server release MUST include the tagged `otvi-server` version, and `otvi-core` or `otvi-web` version changes MUST only be included when maintainers explicitly prepared those components for the same release.

#### Scenario: Server-only release is prepared
- **WHEN** maintainers prepare a release that only changes `otvi-server`
- **THEN** the documented release process requires version and release metadata updates for `otvi-server` without forcing unrelated version changes in `otvi-core` or `otvi-web`

#### Scenario: Multi-component release is prepared intentionally
- **WHEN** maintainers intentionally prepare `otvi-core` or `otvi-web` for the same release train as `otvi-server`
- **THEN** the documented release process allows those components to be versioned and published as part of the same reviewed release preparation

### Requirement: Tag-driven server release automation MUST validate prepared release metadata
The server release workflow SHALL derive the release version from the pushed server-release tag and SHALL validate that the repository state prepared for that tag contains matching release metadata for the components included in scope. The workflow MUST fail on mismatches instead of implicitly rewriting unrelated component versions.

#### Scenario: Prepared metadata matches the release tag
- **WHEN** a matching server-release tag is pushed and the prepared repository metadata matches that version
- **THEN** the workflow continues with release publication using the tagged repository state

#### Scenario: Prepared metadata does not match the release tag
- **WHEN** a matching server-release tag is pushed but `otvi-server` release metadata does not match the tagged version
- **THEN** the workflow fails before publication and reports the mismatch

### Requirement: Tagged server releases MUST coordinate documentation publication
The release process SHALL require a documentation version snapshot and release-facing docs content for the tagged server version, and the tag-triggered workflows SHALL publish documentation from the same tagged repository state.

#### Scenario: Release tag includes matching docs artifacts
- **WHEN** a matching server-release tag is pushed after maintainers prepared the docs version snapshot for that release
- **THEN** the release and docs workflows publish artifacts built from the same tagged repository state

#### Scenario: Release tag is missing required docs preparation
- **WHEN** a matching server-release tag is pushed without the required docs-release artifacts for that version
- **THEN** the documented validation flow identifies the release as incomplete before successful publication
