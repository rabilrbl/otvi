## Why

The current release workflow only builds binaries for the host platform, limiting distribution to users on different operating systems and architectures. Supporting all major platforms (Windows, macOS, Linux across x86_64, aarch64) will significantly increase accessibility and adoption of the Otvi television streaming platform.

## What Changes

- Update GitHub Actions workflow to build release binaries for multiple platforms using cross-compilation
- Add matrix strategy for building across different OS/architecture combinations
- Implement artifact upload for all platform-specific binaries
- Ensure version consistency across all built artifacts
- **BREAKING**: Remove single-platform build step in favor of multi-platform matrix

## Capabilities

### New Capabilities
- `multi-platform-release`: Build and distribute release binaries for Windows, macOS, and Linux on both x86_64 and aarch64 architectures

### Modified Capabilities
- `tag-driven-server-release`: Update requirements to include cross-platform binary generation and validation

## Impact

- GitHub Actions workflows in `.github/workflows/`
- Release scripts and documentation
- CI/CD pipeline configuration
- Distribution artifacts and release notes