## Context

The current release workflow in `.github/workflows/release-server.yml` builds binaries only for the host platform (ubuntu-latest, which is Linux x86_64). The workflow builds two binaries:
1. A bundled release binary with embedded frontend (`otvi-server` with frontend assets)
2. A server-only release binary (`otvi-server` without frontend)

These are packaged and released as `otvi-${VERSION}-linux-x86_64.tar.gz` and `otvi-server-${VERSION}-linux-x86_64.tar.gz`.

To support all major platforms, we need to extend this to build for:
- Windows (x86_64 and aarch64)
- macOS (x86_64 and aarch64) 
- Linux (x86_64 and aarch64)

## Goals / Non-Goals

**Goals:**
- Build and distribute release binaries for all major platforms (Windows, macOS, Linux) on both x86_64 and aarch64 architectures
- Maintain the existing validation and packaging steps for each platform
- Preserve the existing release workflow structure and naming conventions
- Ensure version consistency across all platform-specific binaries

**Non-Goals:**
- Changing the core release validation logic (version checking, docs validation)
- Modifying how frontend assets are bundled or served
- Altering the GitHub release publishing mechanism
- Supporting additional architectures beyond x86_64 and aarch64
- Changing the repository structure or Cargo project layout

## Decisions

### Cross-compilation Approach
**Decision:** Use Rust's built-in cross-compilation capabilities with `rustup target add` for each target platform.
**Rationale:** 
- Rust has excellent cross-compilation support through its target triples
- Avoids complexity of setting up separate build environments or containers
- Leverages existing `cargo build --target` functionality
- Maintains reproducibility with locked dependencies

**Alternatives Considered:**
- Using Docker containers for each platform: Rejected due to increased complexity and slower build times
- Using cross-compilation tools like `cross`: Rejected as Rust's native toolchain is sufficient for our needs

### Matrix Strategy
**Decision:** Implement a GitHub Actions matrix strategy with OS and architecture dimensions.
**Rationale:**
- GitHub Actions matrices naturally express the cross-platform build requirement
- Allows parallel builds for faster overall workflow execution
- Clear visualization of build progress for each platform/architecture combination
- Easy to extend or modify platform/architecture support

**Alternatives Considered:**
- Sequential builds: Rejected as it would significantly increase workflow duration
- Separate workflow files per platform: Rejected due to duplication and maintenance overhead

### Artifact Naming Convention
**Decision:** Extend existing naming convention to include OS and architecture: `otvi-${VERSION}-${OS}-${ARCH}.tar.gz`
**Rationale:**
- Maintains backward compatibility with existing naming patterns
- Clearly indicates platform and architecture in the artifact name
- Follows common distribution conventions
- Easy to parse and consume in automated systems

**Alternatives Considered:**
- Including architecture in directory names only: Rejected as it makes individual artifacts harder to identify
- Using completely different naming schemes: Rejected to maintain consistency with existing releases

### Target Triple Selection
**Decision:** Use standard Rust target triples for each platform/architecture combination:
- Windows x86_64: `x86_64-pc-windows-msvc`
- Windows aarch64: `aarch64-pc-windows-msvc`
- macOS x86_64: `x86_64-apple-darwin`
- macOS aarch64: `aarch64-apple-darwin`
- Linux x86_64: `x86_64-unknown-linux-gnu`
- Linux aarch64: `aarch64-unknown-linux-gnu`
**Rationale:**
- These are the official Rust target triples for cross-compilation
- Widely recognized and supported by the Rust ecosystem
- Match what `rustup target list` provides
- Ensure compatibility with Rust's standard library and linker requirements

## Risks / Trade-offs

[Risk] Increased workflow duration due to multiple platform builds → Mitigation: Use parallel matrix execution to build all platforms concurrently
[Risk] Cross-compilation issues with platform-specific dependencies → Mitigation: Test each target triple early in development; most Rust crates are cross-platform compatible
[Risk] Larger workflow file complexity → Mitigation: Clear commenting and modular step organization
[Risk] Potential issues with Windows MSVC linker requirements → Mitigation: Ensure proper setup steps for Windows builds; may need to install build dependencies

## Migration Plan

1. Update the `release-server.yml` workflow to include a matrix strategy
2. Add steps to install required Rust targets for each platform in the matrix
3. Modify the build steps to use `cargo build --target` for each matrix combination
4. Update packaging steps to include OS and architecture in artifact names
5. Update the publish step to include all platform-specific artifacts
6. Test the workflow with a pre-release tag to verify all builds succeed
7. Once validated, use the updated workflow for all future releases

Rollback strategy: Revert the workflow file changes to the previous version if issues are encountered.

## Open Questions

- Should we continue to build the wasm32-unknown-unknown target for frontend builds, or is that handled separately?
- Do we need to consider any special environment variables or flags for specific platform builds (particularly Windows with MSVC)?
- Should we add any additional validation steps to ensure the built binaries actually run on their target platforms?