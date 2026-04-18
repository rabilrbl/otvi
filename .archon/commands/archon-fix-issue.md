---
description: Implement the fix for a GitHub issue based on investigation/plan artifact
argument-hint: (none - reads from $ARTIFACTS_DIR/investigation.md)
---

# Fix Issue

## Your Mission

Implement the fix described in the investigation artifact. Produce a working, tested implementation.

## Phase 1: LOAD CONTEXT

Read the investigation plan:
```bash
cat "$ARTIFACTS_DIR/investigation.md" 2>/dev/null || echo "No investigation found"
```

Read web research if available:
```bash
cat "$ARTIFACTS_DIR/web-research.md" 2>/dev/null || echo "No web research found"
```

Check current state:
```bash
git status
git branch
```

## Phase 2: IMPLEMENT

Follow the implementation plan from `investigation.md` exactly. For each change:

1. Make the minimal change required to fix the issue
2. Do not refactor unrelated code
3. Do not add features beyond the fix scope

Common patterns by issue type:

**Dependency updates** (Cargo.toml/Cargo.lock):
```bash
# Update specific packages
cargo update -p <crate-name>
# Verify lockfile updated
cargo tree -i <crate-name>
```

**Bug fixes**: Edit the relevant source files directly.

**Configuration changes**: Edit config files with minimal diff.

## Phase 3: VALIDATE

After implementing, run validation:

```bash
# Build first
cargo build 2>&1 | tail -20

# Run tests
cargo test 2>&1 | tail -30

# Run audit if this was a security fix
cargo audit 2>/dev/null && echo "AUDIT: PASS" || echo "AUDIT: advisory remaining (check if expected)"
```

## Phase 4: WRITE IMPLEMENTATION REPORT

Write `$ARTIFACTS_DIR/implementation.md`:

```markdown
# Implementation Report

## Changes Made
[List each file changed and why]

## Validation Results
[Build/test/audit output]

## Remaining Issues
[Anything not fully resolved and why]
```

```bash
mkdir -p "$ARTIFACTS_DIR"
```
