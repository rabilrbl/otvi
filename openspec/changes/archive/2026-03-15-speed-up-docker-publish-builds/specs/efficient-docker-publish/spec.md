## ADDED Requirements

### Requirement: Docker publish workflow MUST reuse shared Buildx cache
The Docker publish workflow SHALL import and export Buildx cache for each published image so unchanged dependency layers can be reused across GitHub Actions runs.

#### Scenario: Publish run reuses cached dependency layers
- **WHEN** the Docker publish workflow runs for an image whose dependency layers were built by a previous run
- **THEN** the workflow restores Buildx cache before the build starts
- **AND** the build is able to reuse unchanged dependency layers instead of recompiling them from scratch

#### Scenario: Publish run refreshes cache after build
- **WHEN** the Docker publish workflow finishes building and pushing an image successfully
- **THEN** the workflow exports the resulting Buildx cache for use by later publish runs

### Requirement: Frontend tool installation in Docker publish MUST prefer prebuilt `trunk`
The Docker build path for the frontend-enabled image SHALL install a pinned `trunk` version from a prebuilt binary artifact for the active Linux architecture instead of compiling `trunk` from source during image publication.

#### Scenario: Build frontend-enabled image for amd64
- **WHEN** the Docker publish workflow builds the frontend-enabled image for `linux/amd64`
- **THEN** the build installs the pinned `trunk` release asset matching `x86_64-unknown-linux-gnu`
- **AND** the build does not run `cargo install trunk`

#### Scenario: Build frontend-enabled image for arm64
- **WHEN** the Docker publish workflow builds the frontend-enabled image for `linux/arm64`
- **THEN** the build installs the pinned `trunk` release asset matching `aarch64-unknown-linux-gnu`
- **AND** the build does not compile `trunk` from source under emulation

### Requirement: Docker publish optimization MUST preserve current published image contract
The optimized Docker publish workflow SHALL continue publishing the same image repositories, tags, and supported architectures as before the optimization change.

#### Scenario: Branch publish run completes after optimization
- **WHEN** the Docker publish workflow runs from a supported branch push
- **THEN** it publishes the same image names and branch-derived tags as before
- **AND** the published manifest still includes both `linux/amd64` and `linux/arm64`

#### Scenario: Release tag publish run completes after optimization
- **WHEN** the Docker publish workflow runs for a supported release tag
- **THEN** it publishes the same semver-derived tags as before
- **AND** downstream consumers can keep pulling the image tags without changing their references
