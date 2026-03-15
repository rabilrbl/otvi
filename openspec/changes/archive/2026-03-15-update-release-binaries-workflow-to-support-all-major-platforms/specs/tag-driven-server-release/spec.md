## MODIFIED Requirements

### Requirement: Server release automation MUST use one documented server tag format
The repository SHALL define one canonical `vX.Y.Z` tag pattern for releases, and release publication workflows SHALL trigger only for tags that match that documented release pattern.

#### Scenario: Matching server release tag is pushed
- **WHEN** a maintainer pushes a tag that matches the documented `vX.Y.Z` release pattern
- **THEN** GitHub Actions starts the server release workflow for that tag

#### Scenario: Non-release tag is pushed
- **WHEN** a maintainer pushes a tag that does not match the documented `vX.Y.Z` release pattern
- **THEN** the server release workflow does not run

### Requirement: The release process MUST use explicit component version scope
The repository SHALL require `otvi-core`, `otvi-server`, and `otvi-web` to share the tagged release version for every `vX.Y.Z` release. Release publication MUST fail if any tracked package version does not match the tag version.

#### Scenario: Release version is prepared consistently across packages
- **WHEN** maintainers prepare a tagged release
- **THEN** `crates/otvi-core/Cargo.toml`, `crates/otvi-server/Cargo.toml`, and `web/Cargo.toml` are all updated to the same release version before the tag is pushed

#### Scenario: Package version mismatch exists
- **WHEN** a tagged release is pushed with mismatched package versions
- **THEN** the release workflow fails before publication and reports which package version is out of sync

### Requirement: Tag-driven server release automation MUST validate prepared release metadata
The release workflow SHALL derive the release version from the pushed `vX.Y.Z` tag and SHALL validate that the repository state prepared for that tag contains matching release metadata for all required packages. The workflow MUST fail on mismatches instead of implicitly rewriting unrelated component versions.

#### Scenario: Prepared metadata matches the release tag
- **WHEN** a matching `vX.Y.Z` tag is pushed and the prepared repository metadata matches that version
- **THEN** the workflow continues with release publication using the tagged repository state

#### Scenario: Prepared metadata does not match the release tag
- **WHEN** a matching `vX.Y.Z` tag is pushed but any required package release metadata does not match the tagged version
- **THEN** the workflow fails before publication and reports the mismatch

### Requirement: Tagged server releases MUST coordinate documentation publication
The release process SHALL require a documentation version snapshot and release-facing docs content for the tagged release version, and the tag-triggered workflows SHALL publish documentation from the same tagged repository state.

#### Scenario: Release tag includes matching docs artifacts
- **WHEN** a matching `vX.Y.Z` tag is pushed after maintainers prepared the docs version snapshot for that release
- **THEN** the release and docs workflows publish artifacts built from the same tagged repository state

#### Scenario: Release tag is missing required docs preparation
- **WHEN** a matching `vX.Y.Z` tag is pushed without the required docs-release artifacts for that version
- **THEN** the documented validation flow identifies the release as incomplete before successful publication

## ADDED Requirements

### Requirement: Release workflow MUST build binaries for all major platforms
The release workflow SHALL compile and package binaries for Windows, macOS, and Linux on both x86_64 and aarch64 architectures using Rust's cross-compilation capabilities.

#### Scenario: Building for Windows x86_64
- **WHEN** a release tag is pushed that matches the vX.Y.Z pattern
- **THEN** the workflow builds the server binary for `x86_64-pc-windows-msvc` target

#### Scenario: Building for Windows aarch64
- **WHEN** a release tag is pushed that matches the vX.Y.Z pattern
- **THEN** the workflow builds the server binary for `aarch64-pc-windows-msvc` target

#### Scenario: Building for macOS x86_64
- **WHEN** a release tag is pushed that matches the vX.Y.Z pattern
- **THEN** the workflow builds the server binary for `x86_64-apple-darwin` target

#### Scenario: Building for macOS aarch64
- **WHEN** a release tag is pushed that matches the vX.Y.Z pattern
- **THEN** the workflow builds the server binary for `aarch64-apple-darwin` target

#### Scenario: Building for Linux x86_64
- **WHEN** a release tag is pushed that matches the vX.Y.Z pattern
- **THEN** the workflow builds the server binary for `x86_64-unknown-linux-gnu` target

#### Scenario: Building for Linux aarch64
- **WHEN** a release tag is pushed that matches the vX.Y.Z pattern
- **THEN** the workflow builds the server binary for `aarch64-unknown-linux-gnu` target

### Requirement: Release workflow MUST package binaries with platform-specific naming
The release workflow SHALL name artifacts to clearly indicate the operating system and architecture for each binary package.

#### Scenario: Packaging Windows x86_64 binary
- **WHEN** the Windows x86_64 binary is built
- **THEN** it is packaged as `otvi-${VERSION}-windows-x86_64.tar.gz`

#### Scenario: Packaging Windows aarch64 binary
- **WHEN** the Windows aarch64 binary is built
- **THEN** it is packaged as `otvi-${VERSION}-windows-aarch64.tar.gz`

#### Scenario: Packaging macOS x86_64 binary
- **WHEN** the macOS x86_64 binary is built
- **THEN** it is packaged as `otvi-${VERSION}-macos-x86_64.tar.gz`

#### Scenario: Packaging macOS aarch64 binary
- **WHEN** the macOS aarch64 binary is built
- **THEN** it is packaged as `otvi-${VERSION}-macos-aarch64.tar.gz`

#### Scenario: Packaging Linux x86_64 binary
- **WHEN** the Linux x86_64 binary is built
- **THEN** it is packaged as `otvi-${VERSION}-linux-x86_64.tar.gz`

#### Scenario: Packaging Linux aarch64 binary
- **WHEN** the Linux aarch64 binary is built
- **THEN** it is packaged as `otvi-${VERSION}-linux-aarch64.tar.gz`

### Requirement: Release workflow MUST publish all platform-specific binaries
The release workflow SHALL upload and publish all platform-specific binary artifacts to the GitHub release.

#### Scenario: Publishing multi-platform binaries
- **WHEN** all platform-specific binaries are built and packaged
- **THEN** the workflow publishes all six platform variants as part of the GitHub release