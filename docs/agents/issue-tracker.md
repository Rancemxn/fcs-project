# GitHub Delivery

Use this document when tracking work in GitHub, preparing a PR, reviewing a delivery, or cleaning up its worktree.
[ADR 0015](../decisions/0015-portable-contributor-workflow.md) defines the current shared workflow.
It requires no particular agent host, model, session topology, broker, or personal tool installation.

## Scope and authorization

A local task can use the user's request as its contract. Use an Issue for persistent, cross-stage, or coordinated
delivery, with an observable goal, acceptance criteria, relevant authority, dependencies, and validation.
Link an existing Issue instead of creating a duplicate. An absent Issue does not block authorized local work.

Carry out the implementation, verification, and fixes needed by the authorized task. Reuse authorization already
given for a delivery workflow; do not ask again at every step. Unapproved remote writes, merge, release, destructive
history changes, credential changes, or paid/system operations need confirmation when the proposed action is concrete.
Continue work that does not depend on that decision.

Issue/PR state arranges work and records evidence; it never defines format semantics or grants release authority.

## Branches and changes

Use a clean branch for a reviewable unit, normally based on current `origin/main`.
`codex/<issue>-<slug>` is useful for agent work with an Issue; otherwise use a descriptive branch name.
Fixes to an open PR may start from its reviewed head and target that PR branch.

Preserve unrelated changes. Use a separate worktree when parallel work or a fixed review snapshot needs isolation;
ordinary tasks do not require a pool, assignment registry, scope lease, or a fixed directory layout.
Record ownership and purpose for extra worktrees, and coordinate overlapping writers rather than overwriting them.

Review the staged diff and follow [CONTRIBUTING.md](../../CONTRIBUTING.md), including DCO sign-off.
Do not invent a contributor identity or sign-off. Open a draft PR when a reviewable result is ready and PR creation
is within the user's authorized workflow.

## Issue and PR content

Write new GitHub titles, bodies, comments, and review messages in English.
Keep the current summary accurate; do not rewrite old messages just to change their language.

An Issue needs the goal, scope, acceptance, and unresolved dependencies. A PR explains the resulting behavior,
links the Issue when one exists, and records applicable authority changes, verification, and remaining risk.
Use `Closes #<n>` only when merge should close the Issue; otherwise use `Refs #<n>`.

The body may be edited to describe the current scope and result. Add a comment for a material decision, blocker,
review request, or delivery event. Do not require identical progress reports on both Issue and PR, five fixed fields,
or a comment for each push. Link the authoritative checkpoint from the other surface when useful.

Preserve fixed-SHA audits, review verdicts, and consequential decision history. Correct them with an explicit
superseding record rather than silently changing their original evidence. GitHub timestamps are sufficient for
ordinary progress; dated ADRs and reviews retain their own dates.

Use the [triage labels](triage-labels.md) when managing Issues. Status labels describe work readiness, not
specification state or permission to merge.

## Verification and review

Select checks using [AGENTS.md](../../AGENTS.md). Local checks provide development feedback; machine-specific
execution limits belong in the untracked local supplement.

Rust, build, dependency, test, executable-fixture, or gate-execution changes require a successful
[Full Gate](../../.github/workflows/full-gate.yml) run for the final target SHA before Ready/merge.
Record its URL, run ID, event, `headSha`, and conclusion. Documentation/policy-only work uses applicable static
checks and records Rust Full Gate as not applicable. An automatically triggered run does not change that classification.

Check required PR statuses as well as the actual Full Gate evidence. An empty required-check list does not prove
Full Gate success. Missing, running, cancelled, failed, or different-SHA runs are not a pass.
A transient infrastructure failure can be retried on the same SHA; a content fix requires a new SHA and applicable checks.

Before delivery, inspect the final diff against acceptance criteria and record the result on the PR:
target SHA, scope, checks, findings or their absence, and relevant limitations.
This is the Primary audit for non-mechanical implementation; it is not independent review.
Do not mirror the record to the Issue when a link provides the same evidence.

An independent reviewer records the fixed target and inspected scope. After a new push, assess the delta and issue
a current-SHA conclusion covering the changed scope; retain useful earlier evidence without calling an old verdict
current. Full Gate freshness and any complete stage/freeze review required by governance still apply.

Ordinary work units do not wait for an asynchronous second review unless a required check, applicable stage gate,
or an existing Critical/Important finding requires it. Independent stage review and final Frozen/conformance
requirements remain governed by [specification governance](../specifications/governance.md).

A finding identifies its snapshot, location, violated requirement, reproduction or evidence gap, impact, severity,
and acceptance for a fix. An authorized contributor may implement a fix; they cannot provide independent approval
of their own change. Current Critical/Important findings block affected delivery or stage claims. Minor findings
may be deferred with an owner and follow-up. General suggestions do not automatically block or expand the task.

## Remote operations

Use GitHub's UI, API, or an available client. For automation, consume structured output rather than human tables.
Prepare multiline payloads safely, preserving real newlines and literal Markdown.

Retry only transient transport failures, with a small bounded retry budget. Before retrying a write, check whether
the original operation succeeded to prevent duplicate Issues, PRs, comments, or merges.
Authentication, authorization, validation, merge-conflict, and failed-check errors need resolution, not blind retries.
When delivery is uncertain, retain the intended payload and last error, report pending synchronization, and continue
independent local work. Verify the resulting remote state before claiming the operation succeeded.

## Completion and cleanup

A local task is complete when its acceptance criteria and applicable checks are satisfied and limitations are stated.
A PR task includes the authorized push, CI follow-through, and review handoff; merge is performed only when authorized,
the branch is mergeable, applicable checks/audits pass, and blocking findings are resolved.
Do not bypass branch protection or weaken checks to obtain a pass.

After merge, verify the merged state and linked Issue disposition. Record the delivery result once in the appropriate
place and link any residual work. Update plans, matrices, or manifests only when their owning process requires it;
do not rewrite historical reviews to match the new implementation.

Remove only your own temporary worktrees after confirming they are clean, their work is delivered or safely retained,
and no other task needs them. Check resolved paths before cleanup; use Git worktree commands for registered worktrees.
Never force-remove dirty worktrees, unpushed work, another contributor's resources, or needed evidence.
Routine cleanup satisfying these conditions does not need another approval.
