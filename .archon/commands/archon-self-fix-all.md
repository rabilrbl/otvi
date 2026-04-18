---
description: Apply all CRITICAL and HIGH findings from code review synthesis to the current branch
argument-hint: (none - reads from review synthesis artifact)
---

# Self-Fix All Review Findings

## Your Mission

Read the synthesized review findings and fix all CRITICAL and HIGH severity issues. Apply fixes directly — do not just report.

## Phase 1: LOAD REVIEW FINDINGS

```bash
ls "$ARTIFACTS_DIR/review/" 2>/dev/null || ls "$ARTIFACTS_DIR/" 2>/dev/null
cat "$ARTIFACTS_DIR/review/synthesis.md" 2>/dev/null || cat "$ARTIFACTS_DIR/synthesis.md" 2>/dev/null || echo "No synthesis found"
cat "$ARTIFACTS_DIR/review/code-review-findings.md" 2>/dev/null || cat "$ARTIFACTS_DIR/code-review-findings.md" 2>/dev/null || echo "No code review findings"
```

Get PR context:
```bash
cat "$ARTIFACTS_DIR/pr-number.txt" 2>/dev/null || echo "No PR number"
git log --oneline -5
git status
```

## Phase 2: APPLY FIXES

For each CRITICAL or HIGH finding:

1. **Untracked/uncommitted files**: Stage and commit them
   ```bash
   git status --short
   git add <file>
   git commit -m "fix: add missing <file> from review"
   ```

2. **Code bugs**: Edit the file directly and commit

3. **Missing tests**: Add the test and commit

4. **Security issues**: Fix the vulnerability and commit

Apply one fix at a time. Commit after each logical group of changes.

## Phase 3: PUSH

After all fixes are applied:
```bash
git push
```

## Phase 4: WRITE SELF-FIX REPORT

Write `$ARTIFACTS_DIR/self-fix.md`:

```markdown
# Self-Fix Report

## Findings Addressed
[List each CRITICAL/HIGH finding and what was done]

## Commits Made
[List each commit with hash and message]

## Remaining Issues (LOW/INFO only)
[Any findings intentionally not fixed with rationale]
```

```bash
mkdir -p "$ARTIFACTS_DIR"
```
