# ADR 0015: Portable Contributor Workflow and Local Agent Guidance

状态：Accepted

日期：2026-09-05

取代：ADR 0014；部分取代 ADR 0011 的任务前置、进度消息和角色规则，以及 ADR 0013 的全员本地执行限制。

## Context

The project now accepts contributions without a specific agent host or personal development environment.
The previous shared guidance mixed repository invariants with one machine's tools, local paths, and an agent pool.
It also repeated reading prerequisites and delivery bookkeeping across the entry point, templates, and workflow.

The project owner approved simplifying the guidance, removing the Pi workflow, and keeping personal environment
instructions outside version control. This decision changes collaboration policy, not product semantics.

## Decision

- Keep shared project facts, authority boundaries, task-specific reading routes, validation, and completion criteria
  in the tracked root `AGENTS.md`. Existing domain and delivery documents provide details only when needed.
- Store optional machine/tool preferences in `info/AGENTS.local.md` under the Git common directory.
  It is local, shared by worktrees, and not versioned. Public instructions explicitly route to it when present;
  it supplements the project rules instead of replacing the shared file.
- Remove the Pi/session-pool execution contract and its obsolete loop pointers. No fixed model, warm slot,
  broker, coordinator identity, or personal tool is required for ordinary contribution.
  Historical ADRs remain identifiable as superseded records; they do not grant current workflow authority.
- A bounded user request is sufficient for local work. Use Issues for persistent or coordinated delivery.
  Continue through implementation, appropriate verification, and fixes within existing authorization.
  Unapproved external actions remain subject to confirmation; a missing tool or optional document is not a task gate.
- Keep Issue/PR bodies current. Comment on meaningful decisions, blockers, review requests, and delivery events.
  Fixed-SHA review evidence and consequential decisions retain explicit superseding history; routine progress
  no longer requires mirrored five-field messages, immutable initial summaries, or before/after merge duplicates.
- Keep final-SHA Full Gate evidence for applicable changes, Primary audit for non-mechanical implementation,
  and independent review where stage/conformance governance requires it. Scope re-review to the actual delta
  while issuing a current-target conclusion. An old verdict or run is never relabelled as current evidence.
- Local execution limits reflect each contributor's machine. The existing maintainer-machine restriction is
  retained in its local supplement; it is not a ban for every contributor.
  CI remains the required complete Rust delivery evidence. Its triggers and command sequence are unchanged.
- Protect unrelated edits, unpushed work, ownership, credentials, licensing, and historical evidence.
  Clean up owned temporary worktrees only after their work is delivered or safely retained.

## Supersession and retained guarantees

This ADR supersedes the operational role, model, tool, lifetime, approval-routing, and pool requirements of
`0014-session-pool-delivery.md`, including its requirement to introduce another named loop file for a workflow change.
The active delivery reference is now [docs/agents/issue-tracker.md](../agents/issue-tracker.md).

It supersedes conflicting mandatory-Issue, immutable-progress, mirrored-comment, fixed-role, and repeated full
re-review requirements in `0011-github-issue-pr-workflow.md`.
It narrows `0013-public-project-full-gate-ci.md` only where local machine restrictions were imposed on all contributors.

Specification authority, stage baselines, final independent review, Frozen/release requirements, DCO, and
same-SHA CI evidence remain applicable. No specification, fixture, golden, version, GitHub ruleset, or CI execution
logic changes as part of this decision. Documentation-only gate classification remains explicit.

## Validation

Check changed Markdown, local links, the public/local boundary, and removal of active references to the retired
workflow. Verify that all intended worktrees resolve the same untracked local supplement.
Rust Full Gate is not applicable to this documentation and template change.
