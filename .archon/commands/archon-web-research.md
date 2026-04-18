---
description: Research web for context relevant to a GitHub issue - RustSec advisories, dependency solutions, upstream fixes
argument-hint: <issue-number|description>
---

# Web Research

**Input**: $ARGUMENTS

## Your Mission

Research the web to gather external context that will help resolve the issue. Focus on:
- Security advisories (RustSec, CVEs, GitHub Security Advisories)
- Upstream changelogs and release notes for affected dependencies
- Known workarounds and community solutions
- Compatible version ranges and migration guides

## Phase 1: READ ISSUE CONTEXT

Read the issue data from artifacts if available:
```bash
cat "$ARTIFACTS_DIR/issue.json" 2>/dev/null || gh issue view "$ARGUMENTS" --json title,body,labels 2>/dev/null || echo "No issue context found"
```

Read classification:
```bash
cat "$ARTIFACTS_DIR/classification.json" 2>/dev/null || echo "{}"
```

## Phase 2: WEB RESEARCH

Based on the issue content, search the web for:

1. **Dependency advisories**: For each vulnerable crate mentioned, search RustSec advisory details
2. **Fixed versions**: Look up changelogs/releases for patched versions
3. **Migration paths**: Find upgrade guides or breaking change notes
4. **Alternative approaches**: If no fix exists (e.g., `rsa` advisory), research alternatives

Use WebSearch tool to look up:
- RustSec advisories for the specific CVE/RUSTSEC IDs mentioned
- Upstream crate release notes (crates.io, GitHub releases)
- Community discussions (GitHub issues, Reddit, Stack Overflow) about the same problem

## Phase 3: WRITE ARTIFACT

Write findings to `$ARTIFACTS_DIR/web-research.md`:

```markdown
# Web Research Findings

## Issue Context
[Summary of what issue is about]

## Advisory Details
[For each advisory: what it is, severity, attack vector, fix status]

## Dependency Update Analysis
[What versions are available, what versions fix the issue]

## Recommended Approach
[Based on research: best path forward]

## References
[URLs researched]
```

**Important**: If the issue body already contains comprehensive research (reproduction steps, dry-run outputs, advisory details), extract and organize that information - do not duplicate work.

Write the artifact:
```bash
# Ensure artifacts dir exists
mkdir -p "$ARTIFACTS_DIR"
# Write research findings (use Write tool or heredoc)
```
