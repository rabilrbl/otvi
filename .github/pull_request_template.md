## Summary

- What changed?
- Why is this change needed?

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features`
- [ ] `cd web && wasm-pack test --headless --firefox --features ui-test --lib`
- [ ] `cd docs && bun install --frozen-lockfile && bun run build:release`
- [ ] Not applicable (explain below)

## Docs Impact

- [ ] No docs changes required
- [ ] Docs updated in this PR
- [ ] Follow-up docs work required

## Release Impact

- [ ] No release-train impact
- [ ] Affects `main` release-train promotion
- [ ] Affects `vX.Y.Z` tagging or release validation
- [ ] Affects docs versioning or release blog workflow

## Branch Target

This repository expects normal pull requests to target `dev`. If this PR targets another branch, explain why.

## Notes

Anything reviewers or maintainers should know.
