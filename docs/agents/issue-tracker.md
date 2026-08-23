# GitHub Issues and Pull Requests

GitHub Issues are the repository's work contracts. Pull Requests deliver one reviewable implementation unit and its verification evidence. ADR 0011 accepts this workflow. Neither surface has normative authority: specifications, governance, Accepted ADRs, conformance artifacts, and dated reviews retain the responsibilities defined by `AGENTS.md`.

Use the authenticated `gh` CLI for repository operations. Prefer `--json` plus `jq` over parsing human-readable tables.

All new GitHub Issue/PR titles, bodies, comments, and review messages must be written in English. Existing messages
are append-only history and are not rewritten solely for language migration.

For transient network failures only (DNS, timeout/reset, interrupted TLS, or HTTP 502/503/504), wait 5 seconds and retry the same `gh` operation up to ten times after the initial failure. Before every mutation retry, query by stable identity to determine whether the previous attempt already succeeded. Never blindly repeat Issue/PR creation, comments, reviews, or merge. Do not retry authentication/authorization failures, invalid input, not-found responses, merge conflicts, or failed checks; report them immediately.

After ten retries, preserve the exact payload, stable identity, last error, and a `pending remote sync` marker, then continue safe local work that does not depend on the remote action succeeding. This local record is a transport outbox, not a second Issue tracker. At the next meaningful checkpoint, and before handoff, PR Ready, review, merge, or another transition that depends on remote state, query first and retry synchronization under the same duplicate-prevention rule. Never claim that a deferred action happened remotely. If the missing remote state is itself a prerequisite for an irreversible or externally visible transition, defer that transition rather than the local work.

Use event- or state-only headings for Issue and PR progress messages. Do not manually add calendar dates such as `YYYY-MM-DD`; the GitHub message timestamp is the time record.

## Issue contract

Before implementation, ensure the Issue records:

- goal and observable acceptance criteria;
- scope and explicit non-goals;
- owning specification clauses, governance state, Accepted ADRs, current plan/review, and fixtures when applicable;
- dependencies and blocked-by/blocking relationships;
- expected implementation and public-interface impact;
- verification commands and required conformance evidence;
- unresolved semantic questions and their owner.

An Issue may arrange specification work but cannot decide format or runtime semantics. If two materially different behaviors remain valid, stop implementation and route the choice through specification governance.

## Issue progress messages

The Issue body is the stable initial work contract and must contain one substantive initial `Progress` checkpoint for every non-mechanical unit. Do not leave the body as an initial conversation, an unfilled template, or a raw pointer elsewhere.

After creation, send each later checkpoint as a new Issue comment. Do not repeatedly edit the body or an earlier comment to accumulate progress. Send a checkpoint when scope or a decision changes, a meaningful work unit completes, a blocker appears or clears, verification produces a decision-relevant result, a PR opens, or delivery state changes. One message covers one meaningful checkpoint; do not mirror every commit.

Each progress message contains:

- **Completed**: the work unit or state transition;
- **Evidence**: commits, PR, tests, fixtures, review, or inspected output;
- **Decisions**: why the chosen direction or change group exists;
- **Blockers**: current blockers or `none`;
- **Next**: the next bounded action or final disposition.

If an earlier message is wrong or obsolete, post a new message that explicitly identifies and supersedes the affected statement; preserve the old message as history instead of silently rewriting it. Before merge or an explicit close, send a separate delivery-ready comment. After delivery, send a separate final comment with the merged/delivered result, final verification, residual work, and follow-up Issues even if `Closes #<n>` has already closed the Issue.

Use parent/sub-issues for a large effort and dependency links for sequencing:

```text
gh issue create --title "..." --body-file issue.md --label needs-triage
gh issue create --title "..." --body-file child.md --parent 12 --blocked-by 10,11
gh issue edit 12 --add-sub-issue 13 --add-blocked-by 9
gh issue comment 12 --body-file progress-checkpoint.md
```

## Triage

Apply exactly one workflow-state label to each open Issue:

```text
needs-triage -> needs-info | ready-for-agent | ready-for-human | wontfix
needs-info   -> needs-triage
```

After new information resolves the gap, return the Issue to `needs-triage` before declaring it ready. Type labels such as `bug`, `documentation`, and `enhancement` may coexist with one state label. See `triage-labels.md`.

There is no separate `in-progress` label. An assignee plus a linked development branch or open PR records that a `ready-for-agent` Issue has been claimed; retain `ready-for-agent` until merge closes the Issue or new evidence requires re-triage.

## Inspect with gh and jq

Use structured output for automation and audit checks:

```text
gh issue list --state open --limit 200 \
  --json number,title,labels,assignees,blockedBy,updatedAt |
  jq -r '.[] | {number, title, labels: [.labels[].name], blocked_by: [.blockedBy[].number]}'

gh issue view 42 --json number,title,body,state,labels,assignees,subIssues,blockedBy,url |
  jq -S '.'

gh pr view 17 --json state,isDraft,mergeable,reviewDecision,statusCheckRollup,closingIssuesReferences |
  jq -e '.state == "OPEN" and (.isDraft | not) and .mergeable != "CONFLICTING"'
```

Use `jq -r` for plain strings, `jq -S` for stable key ordering, and `jq -e` when a filter is a gate. Pass dynamic data with `--arg` or `--argjson`. For APIs beyond built-in `gh --json`, use `gh api`; combine all pages with `--paginate --slurp` before aggregation.

## Branch and implementation

Start from current `origin/main` for a primary implementation branch. Use `codex/<issue>-<slug>` and keep one
reviewable unit per branch. `gh issue develop <number> --base main --name <branch> --checkout` may create and link
the branch when the working tree is clean. Corrective branches are assigned to an isolated `deliver` corrective
lane by the parent coordinator: open-PR fixes start at the reviewed fixed head SHA and target the active PR branch;
historical merged-commit fixes start at latest `origin/main` and target `main`. They must not write the primary
implementation worktree.

Before editing:

1. Read `AGENTS.md` and any closer instructions.
2. Follow the applicable specification/ADR/conformance/review reading route.
3. Reconfirm that the Issue is consistent with the current normative dependency closure.
4. Preserve unrelated worktree changes.

During implementation, announce changed scope in a new Issue comment that explicitly supersedes the affected contract statement, or open a follow-up Issue. Re-triage when the change affects readiness or authority; do not silently expand the PR.

## Pull Request contract

Open a draft PR at the first complete reviewable SHA. Mark it ready only after the intended scope, local static
checks, and every applicable same-SHA GitHub full gate are complete.

The PR body records the stable initial delivery contract:

- `Closes #<n>` when merge should close the Issue, otherwise `Refs #<n>`;
- summary and non-goals;
- specification, ADR, conformance, review, and version-state impact;
- tests and exact commands run;
- skipped/unavailable gates and reason;
- residual risk and follow-up Issues.

It also contains one substantive initial `Progress` checkpoint. Group the initial commits by meaningful outcome and explain what the group changed, why it was necessary, the evidence and decisions it produced, current blockers, and the next step. A raw commit list is not progress.

After the PR is created, every later meaningful checkpoint is a new PR comment. Post one after each material push, when blockers change, and before marking the PR ready so the latest message matches the current diff and commit set. Do not repeatedly edit the PR body or an earlier comment. Correct stale information with a new explicitly superseding comment. A single-checkpoint PR still needs one substantive initial message; it does not need one message per commit.

Select validation according to `AGENTS.md`. Local work is limited to non-building static checks. A documentation-only or
workflow-policy-documentation-only PR has no required Rust full gate; a `.github/workflows/full-gate.yml` implementation,
Rust/build/dependency/test/executable-fixture change must have a successful same-SHA GitHub full-gate run before the PR is
ready or merged. A cache miss is not a gate failure, and a local Cargo result cannot replace the Action run.

Useful commands:

```text
gh pr create --draft --base main --title "..." --body-file pr.md
gh pr comment 17 --body-file progress-checkpoint.md
gh pr diff <number>
gh pr checks <number> --required
gh pr view <number> --json reviewDecision,mergeable,statusCheckRollup,files |
  jq -S '.'
gh pr ready <number>
```

Do not merge until required checks pass, review requirements are satisfied, the branch is mergeable, a passing `Primary audit result` is recorded, the fixed-head reviewer returns pass, and all Critical/Important findings in the applicable gate are closed. Never use `gh pr merge --admin` to bypass protection. Merge only when the user has authorized it.

## Session-pool delivery and review

The active delivery contract is `docs/loops/fcs5-session-pool-delivery.md`, accepted by ADR 0014. It
supersedes the current execution-role clauses in the former `docs/loops/loop.md`,
`docs/loops/review-loop.md`, and the conflicting dated amendments of ADR 0011. The old loop paths are
kept only as superseded pointers so historical plan and review links remain resolvable.

The parent Rancemxn session is the persistent coordinator and the only actor allowed to run `gh pr ready`,
merge a PR, close the delivery Issue when appropriate, or update `main`. It is dispatcher, gatekeeper,
allocator, and merge owner only: it selects frontier, assigns bounded Issues, receives child reports, checks
acceptance/remote state, routes follow-ups, and performs the final Ready/merge decision. It does not implement,
investigate, gather ordinary product evidence, or run ordinary product commands. All implementation, research,
and review work is assigned through `session_pool` to unattended children. Four active assignments are allowed;
each has its own slot, monitor with `wake` policy, and deadline of at least one hour. Slots are reusable after
coordinator acceptance and release; session IDs are per-run. Every assignment must carry the Issue, base/head
SHA, branch, absolute worktree, owner, slot/monitor, deadline, allowed paths, authority, acceptance, non-goals,
validation, and handoff fields. Session-pool state does not replace remote Issue/PR/Action state and does not
provide file locking.

`deliver` may modify only its assigned worktree, commit and push its branch as the Delivery logical identity,
create/update the draft PR, append progress and Primary audit, and wait for exact-head-SHA Action evidence.
It may not mark a PR Ready, merge, push `main`, change the primary Issue workflow label, or bypass branch
protection. The first reviewable commit creates a draft PR. A successful Full Gate run from either `push`,
`pull_request`, or `workflow_dispatch` is usable as evidence only when its recorded `headSha` exactly matches
the target SHA; required PR checks, mergeability, review threads, and branch protection are checked separately
by Rancemxn. Running, missing, failed, old-SHA, or mismatched runs are not passes.

Before Ready/merge, `deliver` records `Primary audit result` on the PR and associated Issue when applicable.
The record binds `Issue/PR or commit + head SHA + scope + commands + full-gate evidence + acceptance gate`
and includes Target, Head SHA, Scope, Commands, Full-gate evidence, Verdict, Findings, Gate impact,
Limitations, and Next. After Primary audit, the coordinator assigns that exact fixed head to `reviewer`.
Ready/merge remains blocked until reviewer returns pass and no unresolved Critical/Important finding remains.

The unattended reviewer reads and audits the fixed snapshot, may comment, request changes, and create a finding
Issue, but may not call `ask_user_question`, edit code, commit, push, or create a corrective PR. A disagreement
or confirmed current-stage Critical/Important finding is assigned by the coordinator to a new isolated `deliver`
corrective lane from the fixed reviewed SHA. Open-PR corrective work targets the active PR branch; historical
merged-commit corrective work targets `main`. After corrective gate and Primary audit, the coordinator sends the
new SHA to reviewer again. Repeat until reviewer pass and no unresolved Critical/Important finding; only then does
Rancemxn perform the final diff/required-check/mergeability/scope gate and merge. Reviewer advisories are HUMAN-only
Issues and do not block the gate unless evidence upgrades them to an implementation, conformance, or normative
finding.

All local worktrees are limited to `cargo fmt --all -- --check` and non-building static checks. Children never
run local Cargo check, Clippy, build, test, nextest, fuzz, or executable fixtures; those checks come from GitHub
Actions. If a child has an exceptional need for main-lane compilation, it reports the exact command and reason
to the coordinator instead of running it. Unattended children never call `ask_user_question`; they return
`needs-decision`/`blocked` with exact evidence, alternatives, impact, and recommendation. The coordinator either
assigns follow-up investigation or opens a `ready-for-human` HUMAN-only Issue, skips that blocked frontier, and
continues independent work. A documentation/workflow-policy-metadata-only change has Rust Full Gate
`non-applicable`; changing `.github/workflows/full-gate.yml` execution logic or Rust/build/dependency/test/
executable-fixture content makes the applicable gate mandatory. Do not report session reports, monitor completion,
local results, cached output, or pending remote sync as GitHub success.

Use `.github/ISSUE_TEMPLATE/review_finding.md` for reviewer-created findings so the fixed snapshot, severity, gate
impact, reproduction, owner, target stage, and corrective acceptance conditions are not omitted. The reviewer does not
create the corrective PR; the linked `deliver` corrective assignment records its branch, worktree, root cause,
regression evidence, and handoff.

## Completion

After merge:

1. Send a new final merged/delivered progress comment to the PR and Issue, then confirm the linked Issue closed as intended.
2. Record residual work as linked Issues rather than hidden PR notes.
3. Update plans, implementation matrix, conformance manifests, or dated reviews only when their owning process requires it.
4. Do not rewrite historical review evidence to match the merged implementation.
