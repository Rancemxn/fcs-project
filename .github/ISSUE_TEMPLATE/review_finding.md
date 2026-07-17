---
name: Independent review finding
about: Record a reproducible defect found in an Issue, PR, or historical commit
title: "finding: "
labels: "needs-triage"
assignees: ""
---

## Reviewed snapshot

- Reviewed Issue / PR / commit:
- Head SHA:
- Review scope:
- Parent or related Issue:

## Finding

- Severity: `Critical` / `Important` / `Minor`
- Current stage/gate impact:
- File, symbol, or stable location:
- Governing specification / ADR / plan clause:
- Observed behavior:
- Expected behavior:

## Reproduction and evidence

```text
<!-- exact command(s), output, fixture, hash, or other reproducible artifact -->
```

## Routing

- Owner:
- Target stage:
- Dependencies:
- Corrective PR:
- Acceptance conditions:

The reviewer may create this Issue and a corrective PR, but may not merge or mark the corrective PR Ready. The primary
session owns review, merge, and re-review of the new head SHA.

## Progress

### Finding recorded

- Completed: captured the fixed snapshot, finding, and reproduction.
- Evidence: <!-- command, output, fixture, or link -->
- Decisions: <!-- severity and why this is a defect rather than an unresolved semantic choice -->
- Blockers: <!-- missing evidence/owner, or none -->
- Next: <!-- bounded corrective action or explicit residual route -->

Keep this initial checkpoint in the body. Send every later meaningful checkpoint as a new Issue comment with the same
five fields; do not append to or repeatedly edit this message. Use an event- or state-only heading without a manually
written date; GitHub supplies the timestamp.
