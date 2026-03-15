## 1. Update Docker Publish Workflow

- [x] 1.1 Modify docker-publish.yml to add platforms matrix for multi-arch builds
- [x] 1.2 Configure platforms to include linux/amd64, linux/arm64, and linux/arm/v7
- [x] 1.3 Ensure both Dockerfile variants (Dockerfile and Dockerfile.no-frontend) use multi-arch

## 2. Test and Validate

- [x] 2.1 Test workflow on feature branch to verify multi-arch images are built
- [x] 2.2 Validate that produced images work correctly on target architectures
- [x] 2.3 Confirm backward compatibility with existing single-arch usage
- [x] 2.4 Check that image tags and naming conventions remain unchanged

## 3. Documentation and Cleanup

- [x] 3.1 Update any relevant documentation about Docker multi-arch support
- [x] 3.2 Ensure workflow runs successfully on main branch after merge
- [x] 3.3 Monitor initial runs for any issues or performance concerns