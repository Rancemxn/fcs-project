---
name: Bug report
about: Report reproducible incorrect behavior or a regression
title: "bug: "
labels: "bug,needs-triage"
assignees: ""
---

## Observed behavior

Describe the failure, including the exact diagnostic or output.

## Expected behavior

Identify the governing specification clause or explain why this is internal-only behavior.

## Reproduction

Provide the smallest input and exact command that reproduces the issue.

## Environment

Record commit, platform, tool versions, and relevant configuration.

## Evidence and impact

List affected fixtures, manifests, public interfaces, stages, or version domains. Do not include secrets or private input.

## Acceptance criteria

- [ ] A deterministic failing test or fixture reproduces the issue.
- [ ] The cause is distinguished from any specification ambiguity.
- [ ] The fix passes the repository quality gates.
