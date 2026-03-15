## Context

The Docker publish workflow in `.github/workflows/docker-publish.yml` builds two images (`otvi` and `otvi-server`) for `linux/amd64` and `linux/arm64` on `ubuntu-latest` using `docker/build-push-action@v7`. Recent GitHub Actions runs show the heaviest delay occurs in the `otvi` image's `linux/arm64` path, where `Dockerfile` installs `trunk` with `cargo install trunk --locked`, performs a full `trunk build --release`, and then compiles `otvi-server` again for release.

The logs also show no effective remote Buildx cache, so expensive Rust and Bun-related layers are rebuilt from scratch on each publish run. This change needs to preserve the current published image tags and multi-architecture output while removing avoidable work from the publish pipeline.

## Goals / Non-Goals

**Goals:**
- Reduce Docker publish runtime for both image variants without changing published image tags or supported architectures
- Eliminate source compilation of global frontend tooling during Docker publish when a trusted prebuilt binary is available
- Reuse Buildx cache across GitHub Actions runs so unchanged dependency layers are not rebuilt from scratch
- Keep the workflow and Dockerfiles understandable for maintainers troubleshooting future CI regressions

**Non-Goals:**
- Dropping multi-architecture image support
- Re-architecting the application build into separate release pipelines outside Docker publish
- Changing runtime image behavior, application features, or deployment configuration
- Introducing custom build orchestration beyond GitHub Actions and Docker Buildx

## Decisions

### Use prebuilt `trunk` binaries during Docker builds
**Decision:** Replace `cargo install trunk --locked` in the Docker build path with installation from a prebuilt `trunk` release artifact.

**Rationale:**
- GitHub Actions logs show compiling `trunk` from source dominates the slow `linux/arm64` build path
- `trunk` publishes Linux binaries for both `x86_64` and `aarch64`, matching the platforms used in Docker publish
- The project guidance already prefers `cargo binstall` for global tools, which aligns with treating `trunk` as a reusable tool rather than a project dependency

**Alternatives considered:**
- Keep `cargo install trunk --locked`: rejected because it preserves the largest known source of wasted build time
- Download release tarballs with custom shell logic: viable, but less maintainable than a standard binary installation path if `cargo-binstall` can resolve the release asset predictably
- Prebuild a custom base image with `trunk` included: rejected for now because it adds another image lifecycle to maintain

### Add GitHub Actions-backed Buildx cache for Docker publish
**Decision:** Configure the Docker publish workflow to import and export Buildx cache using the GitHub Actions cache backend.

**Rationale:**
- The current workflow does not provide `cache-from` or `cache-to`, so dependency layers are rebuilt from scratch every run
- Buildx's native GitHub Actions cache backend fits the existing workflow with minimal operational overhead
- Shared cache is especially valuable for Rust dependency compilation and Bun install layers that change less often than application source

**Alternatives considered:**
- No shared cache: rejected because it leaves known repeat work untouched
- Registry-backed cache images: viable, but adds registry management complexity and is not necessary for the first optimization step
- Splitting each architecture into fully separate workflows: rejected because it increases workflow surface area before simpler cache wins are captured

### Keep the current multi-architecture manifest strategy
**Decision:** Preserve the existing single-step multi-platform publish flow and optimize within it instead of removing `linux/arm64` or introducing architecture-specific publish jobs.

**Rationale:**
- The product goal remains multi-architecture Docker support
- The current workflow already produces the right image contract; the problem is runtime, not output shape
- Keeping the existing publish contract reduces downstream risk while allowing measurable performance improvements

**Alternatives considered:**
- Temporarily publish amd64 only: rejected because it would reduce platform support
- Move arm64 builds to a native runner immediately: promising future improvement, but it requires separate runner strategy and infrastructure decisions beyond the minimal fix set

## Risks / Trade-offs

[Risk] Prebuilt `trunk` binaries may change availability or asset naming across releases -> Mitigation: pin the installed `trunk` version and document the expected target assets

[Risk] Buildx cache growth may increase GitHub cache storage usage -> Mitigation: scope cache entries to the Docker publish workflow and monitor cache effectiveness against runtime improvement

[Risk] Cache reuse can hide dependency refresh mistakes if Dockerfile layer ordering is poor -> Mitigation: keep dependency-copy steps explicit and validate that application source changes still invalidate the correct layers

[Risk] Emulated `linux/arm64` compilation may remain slower than desired even after removing `trunk` compilation -> Mitigation: treat native arm64 runners as a follow-up option if caching and binary installation do not reduce runtime enough

## Migration Plan

1. Update `Dockerfile` so the frontend build stage installs a pinned `trunk` release via a binary-first path.
2. Update `.github/workflows/docker-publish.yml` to enable Buildx cache import/export for publish builds.
3. Validate that both Docker image variants still build and push with the existing tag scheme.
4. Compare GitHub Actions runtime and cache reuse against the recent slow baseline.
5. If runtime is still unacceptable, evaluate a follow-up change for native arm64 runners or deeper Docker layer restructuring.

Rollback strategy: revert the Dockerfile and workflow changes to return to the previous publish behavior.

## Open Questions

- Should the project standardize a pinned `trunk` version in one shared location for both Docker and local setup docs?
- Is GitHub Actions cache sufficient for the expected publish frequency, or will registry-backed cache become necessary later?
- After removing `trunk` source compilation, is the remaining `linux/arm64` server build time acceptable without native arm64 runners?
