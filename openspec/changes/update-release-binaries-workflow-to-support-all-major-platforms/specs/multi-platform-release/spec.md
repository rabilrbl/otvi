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