---
description: Review changed code for unnecessary complexity and simplify where possible
argument-hint: (none - reviews current branch diff)
---

# Simplify Changes

## Your Mission

Review the diff on the current branch and simplify any unnecessarily complex changes. Fix issues directly.

## Phase 1: GET DIFF

```bash
git diff main...HEAD 2>/dev/null || git diff HEAD~1 2>/dev/null | head -200
```

## Phase 2: ASSESS

For each changed file, check:
- Are there redundant changes mixed with the fix?
- Are there overly complex implementations when simpler ones exist?
- Are there unnecessary comments, dead code, or debug artifacts?

For dependency-only changes (Cargo.lock, Cargo.toml), simplify is usually a no-op.

## Phase 3: SIMPLIFY (if needed)

If changes can be simplified:
1. Edit the file
2. Run `cargo build` or relevant validator to confirm still works
3. Commit: `git commit -m "simplify: <what was simplified>"`

If no simplification needed, note that explicitly.

## Phase 4: WRITE REPORT

Write `$ARTIFACTS_DIR/simplify.md`:

```markdown
# Simplify Report

## Assessment
[What was reviewed]

## Changes Made
[What was simplified, or "No changes needed"]
```

```bash
mkdir -p "$ARTIFACTS_DIR"
```
