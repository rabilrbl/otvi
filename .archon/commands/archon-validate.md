---
description: Validate that an implemented fix works correctly - build, test, audit
argument-hint: (none - validates current branch state)
---

# Validate Fix

## Your Mission

Validate that the implemented fix is correct and complete.

## Phase 1: READ CONTEXT

```bash
cat "$ARTIFACTS_DIR/implementation.md" 2>/dev/null || echo "No implementation report found"
git log --oneline -5
git diff HEAD~1 --stat 2>/dev/null || git diff --cached --stat
```

## Phase 2: BUILD VALIDATION

```bash
# Check compilation
cargo check 2>&1
```

```bash
# Full build
cargo build 2>&1 | tail -20
```

## Phase 3: TEST VALIDATION

```bash
cargo test 2>&1 | tail -40
```

## Phase 4: SECURITY AUDIT (if applicable)

```bash
cargo audit 2>&1 | tail -30
```

## Phase 5: WRITE VALIDATION REPORT

Determine overall result:
- **PASS**: Build succeeds, tests pass, no new issues introduced
- **PARTIAL**: Fix works but some advisories remain (acceptable if documented)
- **FAIL**: Build broken or tests fail

Write `$ARTIFACTS_DIR/validation.md`:

```markdown
# Validation Report

## Build: PASS/FAIL
[Output summary]

## Tests: PASS/FAIL/PARTIAL
[Output summary]

## Security Audit: PASS/PARTIAL/FAIL
[Remaining advisories if any, with justification]

## Overall: PASS/PARTIAL/FAIL
[Final verdict and any caveats]
```

```bash
mkdir -p "$ARTIFACTS_DIR"
```

If validation fails with build errors or test failures, investigate and fix before proceeding.
