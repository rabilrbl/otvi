## Context

The current Docker setup consists of two Dockerfiles:
1. `Dockerfile` - Builds the server with embedded frontend WASM assets
2. `Dockerfile.no-frontend` - Builds only the server binary (no frontend)

Both Dockerfiles use multi-stage builds with Rust and Debian-slim base images. The current GitHub Actions workflow (`docker-publish.yml`) uses `docker/build-push-action@v7` with `docker/setup-buildx-action@v4` already configured, but it's not utilizing Buildx's multi-architecture capabilities.

The workflow currently builds and pushes single-architecture images (matching the runner's architecture, which is amd64 on Ubuntu-latest).

## Goals / Non-Goals

**Goals:**
- Build and publish Docker images for multiple CPU architectures (amd64, arm64, etc.)
- Maintain backward compatibility with existing single-arch usage
- Keep the same image tags and naming conventions
- Support both Dockerfile variants (with and without frontend)

**Non-Goals:**
- Changing the base images or build process fundamentally
- Adding support for obscure or rarely used architectures
- Modifying the application code itself
- Changing the versioning or tagging strategy

## Decisions

### Use Docker Buildx's built-in multi-arch support
**Rationale:** The workflow already has `docker/setup-buildx-action@v4` configured. We just need to add platform matrix to the build-push step.
**Alternatives considered:** 
- Using manifest tool or manual manifest creation (more complex, error-prone)
- Building sequentially on different runners (slower, more complex)

### Add platforms matrix to build step
**Rationale:** Using `platforms` parameter in `docker/build-push-action@v7` is the standard way to build multi-arch images with Buildx.
**Alternatives considered:**
- Custom build script with `docker buildx build --platform` (more verbose, duplicates action functionality)
- Separate jobs per architecture (wastes resources, harder to maintain)

### Preserve existing tagging and naming
**Rationale:** Avoid breaking changes to downstream consumers of the Docker images.
**Alternatives considered:**
- Adding architecture suffixes to tags (would break existing deployments)
- Creating new repository for multi-arch images (fragments the image namespace)

### Support both Dockerfiles in the matrix
**Rationale:** Both image variants (with and without frontend) should support multi-arch.
**Alternatives considered:**
- Only multi-arch for main Dockerfile (inconsistent support)
- Separate workflows for each Dockerfile (duplicates configuration)

## Risks / Trade-offs

[Increased build time] → Multi-arch builds take longer than single-arch as they need to compile for each platform
[Potential compatibility issues] → Need to ensure Rust cross-compilation works for target architectures → Mitigation: Test on actual hardware or use QEMU via Buildx
[Larger storage usage] → Multi-arch images consume more storage in the registry → Mitigation: Same as before since we're replacing single arch with multi-arch (not adding)
[Complexity in debugging] → Issues might be architecture-specific → Mitigation: Clear documentation and testing approach

## Migration Plan

1. Update the docker-publish.yml workflow to add platform matrix
2. Test the workflow on a feature branch
3. Verify multi-arch images work correctly by pulling and running on different architectures
4. Merge to main branch
5. Monitor for any issues in production usage

## Open Questions

- Should we enable multi-arch for all tags or only specific ones (like semver tags)?
- What specific architectures should we target beyond amd64 and arm64?
- Do we need to adjust resource limits for the longer build times?