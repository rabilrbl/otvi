## 1. Docker Build Optimization

- [x] 1.1 Update `Dockerfile` to install a pinned `trunk` release via a binary-first path instead of `cargo install trunk --locked`
- [x] 1.2 Verify the frontend-enabled Docker build still works for both `linux/amd64` and `linux/arm64` target contexts after the `trunk` installation change

## 2. Workflow Cache Optimization

- [x] 2.1 Update `.github/workflows/docker-publish.yml` to restore and export Buildx cache for each published image
- [x] 2.2 Preserve the existing image names, tag metadata, and `linux/amd64,linux/arm64` publish contract while adding cache configuration

## 3. Validation and Documentation

- [x] 3.1 Update Docker or contributor documentation to reflect the preferred binary-first `trunk` installation path where relevant
- [x] 3.2 Run targeted validation for the Docker publish changes and confirm the optimized workflow configuration is syntactically correct
