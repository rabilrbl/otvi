## Why

The Docker publish workflow now spends more than two hours on some multi-architecture runs, making image publication unreliable and expensive. Recent GitHub Actions logs show the slowdown comes from rebuilding Rust-heavy Docker layers for `linux/arm64`, especially compiling `trunk` from source and rebuilding without shared Buildx cache.

## What Changes

- Update the Docker publish workflow to reuse Buildx cache across runs for both image variants
- Change frontend build tool installation in Docker builds so `trunk` is installed from prebuilt binaries instead of compiling it from source during image publication
- Preserve the current published image names, tags, and supported platforms while reducing avoidable CI work
- Add validation and documentation for the optimized Docker publish path so future workflow changes do not regress runtime

## Capabilities

### New Capabilities
- `efficient-docker-publish`: Build and publish multi-architecture Docker images with shared build cache and prebuilt frontend tooling so publish runs avoid unnecessary recompilation

### Modified Capabilities

None.

## Impact

- GitHub Actions workflow in `.github/workflows/docker-publish.yml`
- Docker build definitions in `Dockerfile` and related image build stages
- Docker publishing runtime and registry cache behavior
- Contributor and operator documentation for Docker publishing and frontend tool installation
