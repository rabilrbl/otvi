---
description: Post a completion report to the GitHub issue with PR link, what was done, and follow-up suggestions
argument-hint: <issue-number>
---

# Issue Completion Report

## Your Mission

Post a final completion comment to the GitHub issue summarizing what was fixed and linking the PR.

## Phase 1: LOAD ARTIFACTS

```bash
ISSUE_NUM=$(cat "$ARTIFACTS_DIR/issue-number.txt" 2>/dev/null || echo "$ARGUMENTS")
PR_NUM=$(cat "$ARTIFACTS_DIR/pr-number.txt" 2>/dev/null)
cat "$ARTIFACTS_DIR/implementation.md" 2>/dev/null || echo "No implementation report"
cat "$ARTIFACTS_DIR/validation.md" 2>/dev/null || echo "No validation report"
cat "$ARTIFACTS_DIR/self-fix.md" 2>/dev/null || echo "No self-fix report"
```

## Phase 2: COMPOSE COMMENT

Write a comment covering:
- What was fixed (summary, not exhaustive)
- Link to PR
- Validation results (pass/partial)
- Any follow-up work suggested (unfixed LOW findings, known limitations)

Keep it concise — 3-6 bullet points max.

## Phase 3: POST TO GITHUB

```bash
gh issue comment "$ISSUE_NUM" --body "$(cat <<'COMMENT'
## Fix Applied

[Summary of what was done]

**PR**: #[PR_NUM]

**Validation**: [PASS/PARTIAL] — [brief result]

**Follow-up suggestions** (optional):
- [Any remaining work]
COMMENT
)"
```

## Phase 4: WRITE REPORT

Write `$ARTIFACTS_DIR/completion-report.md` with the comment content.

```bash
mkdir -p "$ARTIFACTS_DIR"
```
