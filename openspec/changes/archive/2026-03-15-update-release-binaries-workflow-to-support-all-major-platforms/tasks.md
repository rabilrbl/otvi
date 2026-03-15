## 1. Workflow Preparation

- [x] 1.1 Review current release-server.yml workflow structure
- [x] 1.2 Identify sections that need modification for matrix strategy
- [x] 1.3 Determine required Rust targets for all platforms

## 2. Matrix Strategy Implementation

- [x] 2.1 Add matrix strategy for OS and architecture combinations
- [x] 2.2 Configure matrix to include:
    - OS: [ubuntu-latest, macos-latest, windows-latest]
    - Architecture: [x86_64, aarch64]
- [x] 2.3 Add appropriate Rust target installation steps for each matrix combination
- [x] 2.4 Update Rust setup to use matrix-specific targets

## 3. Cross-compilation Build Updates

- [x] 3.1 Modify build steps to use `cargo build --target` with matrix target
- [x] 3.2 Update bundled frontend asset building to work across platforms
- [x] 3.3 Ensure WASM target is still available for frontend builds where needed
- [x] 3.4 Verify environment variables are properly set for cross-compilation

## 4. Platform-specific Packaging

- [x] 4.1 Update packaging steps to include OS and architecture in directory names
- [x] 4.2 Modify artifact naming to follow `otvi-${VERSION}-${OS}-${ARCH}.tar.gz` pattern
- [x] 4.3 Ensure both bundled and server-only binaries are packaged for each platform
- [x] 4.4 Update checksum generation to include platform-specific artifacts

## 5. Release Publication Updates

- [x] 5.1 Modify publish step to include all platform-specific artifacts
- [x] 5.2 Update file glob patterns to capture all platform variants
- [x] 5.3 Ensure both bundled and server-only binaries are published for each platform
- [x] 5.4 Verify SHA256 checksums are included for all artifacts
- [x] 5.5 Separate build and release jobs to avoid multiple release actions

## 6. Testing and Validation

- [x] 6.1 Test workflow with a pre-release tag to verify all builds succeed
- [x] 6.2 Validate that all six platform variants are produced correctly
- [x] 6.3 Check that artifact names follow the expected pattern
- [x] 6.4 Confirm that existing validation steps still work (version checking, etc.)
- [x] 6.5 Test that the workflow can be rolled back if needed

## 7. Documentation and Cleanup

- [x] 7.1 Update any relevant documentation references to the workflow
- [x] 7.2 Ensure changelog or release notes reflect the new capabilities
- [x] 7.3 Clean up any temporary files or artifacts from testing