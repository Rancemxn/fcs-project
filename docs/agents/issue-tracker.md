# GitHub Issues and Pull Requests

GitHub Issues are the repository's work contracts. Pull Requests deliver one reviewable implementation unit and its verification evidence. ADR 0011 (as amended by ADR 0014) accepts this workflow. Neither surface has normative authority: specifications, governance, Accepted ADRs, conformance artifacts, and dated reviews retain the responsibilities defined by `AGENTS.md`.

Use the authenticated `gh` CLI or the Delivery/Review App identities for repository operations. Prefer `--json` plus
`jq` over parsing human-readable tables. All roles operate under `docs/loops/fcs5-parallel-pr-delivery.md`
(ADR 0011 as amended by ADR 0014); only `Rancemxn` may mark PRs Ready, merge, or update `main`.

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

## Lane and implementation

Each work unit is Issue-first: a bounded child Issue, then a lane. The lane branch is `codex/<issue>-<slug>` and the
lane worktree is `worktree/<issue>-<slug>` under `C:/Users/Admin/Desktop/fcs-project`, created from the latest
`origin/main`; one lane has exactly one writer. The lane uses a sparse checkout excluding `refer` and a read-only
`refer` junction to `main/refer` (the real tree lives once; `/refer` is gitignored). `git clean -fdx`/`-fdX` across the
junction is forbidden. Corrective lanes created by the parent orchestrator follow the `/tmp` isolation and worktree
rules below and must not write the primary lane worktree.

Before editing:

1. Read `AGENTS.md` and any closer instructions.
2. Follow the applicable specification/ADR/conformance/review reading route.
3. Reconfirm that the Issue is consistent with the current normative dependency closure.
4. Preserve unrelated worktree changes.

During implementation, announce changed scope in a new Issue comment that explicitly supersedes the affected contract statement, or open a follow-up Issue. Re-triage when the change affects readiness or authority; do not silently expand the PR.

## Pull Request contract

Open a draft PR at the first complete SHA that needs Rust gate feedback. Submit each SHA that needs Rust evidence as
a candidate SHA: dispatch `.github/workflows/full-gate.yml` via `workflow_dispatch` on a ref resolving to that exact
SHA and verify the run `headSha`. The workflow has no automatic `pull_request`/`push main` triggers (ADR 0013 as
amended by ADR 0014). Only `Rancemxn` may mark the PR ready, and only after the intended scope, local static checks,
and every applicable same-SHA candidate-SHA full gate are complete.

The PR body records the stable initial delivery contract:

- `Closes #<n>` when merge should close the Issue, otherwise `Refs #<n>`;
- summary and non-goals;
- specification, ADR, conformance, review, and version-state impact;
- tests and exact commands run;
- skipped/unavailable gates and reason;
- residual risk and follow-up Issues.

It also contains one substantive initial `Progress` checkpoint. Group the initial commits by meaningful outcome and explain what the group changed, why it was necessary, the evidence and decisions it produced, current blockers, and the next step. A raw commit list is not progress.

After the PR is created, every later meaningful checkpoint is a new PR comment. Post one after each material push, when blockers change, and before marking the PR ready so the latest message matches the current diff and commit set. Do not repeatedly edit the PR body or an earlier comment. Correct stale information with a new explicitly superseding comment. A single-checkpoint PR still needs one substantive initial message; it does not need one message per commit.

Select validation according to `AGENTS.md`. Local work is limited to `cargo fmt --all -- --check` and non-compiling
static checks; local compilation, lint, tests, fuzz, and executable fixtures are prohibited, and local results are
never gate evidence. A documentation-only or workflow-policy-documentation-only PR has no required Rust full gate; a
`.github/workflows/full-gate.yml` implementation, Rust/build/dependency/test/executable-fixture change must have a
successful same-SHA candidate-SHA full-gate run before the PR is ready or merged. A cache miss is not a gate failure,
and a local Cargo result cannot replace the Action run.

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

Only `Rancemxn` merges, and only when required checks pass, review requirements are satisfied, the branch is
mergeable, a passing `Primary audit result` is recorded, and all Primary-audit Critical/Important findings in the
applicable gate are closed. The Review App may still be pending; any reviewer finding that arrives before or after
merge follows the routing rules below. Never use `gh pr merge --admin` to bypass protection.

## Primary self-audit and independent review (Apps)

The Delivery App (`fcs5-delivery-rancemxn[bot]`) and the Review App (`fcs5-review-rancemxn[bot]`) are the two
automation roles; `Rancemxn` is the sole actor allowed to run `gh pr ready`, merge, or update `main`. Before
Ready/merge, the Delivery App performs a direct Primary Self-Audit without a subagent and records
`Primary audit result`; the Review App audits fixed SHAs under `docs/loops/fcs5-parallel-pr-delivery.md` as an
asynchronous second-pass role and never implements or pushes.

The Review App may:

- read a fixed Issue, PR, or merged commit and cite a historical commit to identify a defect;
- append comments to the PR and associated Issue, and submit `gh pr review --comment` or
  `--request-changes`;
- create a bug/finding Issue containing the discovery SHA, location, normative/ADR/plan clause, reproduction command,
  impact, severity, owner, target stage, dependencies, and acceptance conditions;
- apply existing orthogonal labels (`review-finding`, domain labels, and at most one `severity:*` label) to finding
  Issues it creates and assign an existing milestone when the target stage is known;
- propose, by a new English comment or Issue, changes to the global label or milestone taxonomy without applying those
  global changes itself;
- propose corrective action for a recorded finding; corrective PRs (linking `Closes #<finding>` and
  `Refs #<reviewed-issue-or-pr>`) are delegated to a corrective lane by the parent orchestrator.

The Review App may not:

- merge a PR, mark a PR Ready, close the primary Issue, change its workflow label, or modify a lane's
  active implementation branch, `main`, or worktree;
- create or redefine global labels/milestones, or change the primary Issue's labels or milestone;
- review or approve a corrective PR that it created (it never creates corrective PRs). The parent orchestrator
  delegates corrective PRs to a separate corrective lane; the primary lane inspects and reviews that PR, and the
  primary PR's new head SHA must then be independently reviewed again.

Every Primary or reviewer audit binds `Issue/PR or commit + head SHA + scope + commands + full-gate evidence + acceptance gate`. Before a primary
PR is Ready or merged, the Delivery App records `Primary audit result` on the PR (when one exists) and associated Issue;
the lane may continue toward Ready/merge after a passing Primary audit without waiting for the reviewer. It then posts
`Review requested`; after the fixed snapshot is audited, the Review App immediately appends one `Audit result` comment
to the reviewed PR and associated Issue, even when there are no findings. Primary messages include Target, Head SHA,
Scope, Commands, Full-gate evidence, Verdict, Findings, Gate impact, Limitations, and Next. Reviewer messages include
those fields plus Root cause, Corrective action, Corrective PR, Regression evidence, Advisories, and Worktree. `Advisories`
is reviewer-only. Do not hand-write dates or edit old messages.
A later push, scope, command, or acceptance change invalidates the affected audit; append a superseding/re-review message and
audit the new SHA.

Finding routing is strict: a Primary-audit Critical/Important finding in the current stage blocks the primary PR from
Ready/merge. A reviewer Critical/Important implementation/conformance finding that arrives after merge freezes the
affected stage claim and dependent work until corrected; it does not require rollback. A Minor finding may be deferred only
when it cannot affect current acceptance and has an owner, follow-up Issue, target stage, and removal condition. A local
implementation finding is normally a child/related Issue of the reviewed Issue; only a cross-stage or root-level finding is
attached directly to root Issue #9.

After implementation/conformance review passes, the Review App may audit architecture and documentation. An
optimization, terminology, link, plan, or maintainability suggestion is a HUMAN-only Issue with `ready-for-human` plus an
appropriate `documentation`, `workflow`, or `enhancement` label. It is not a `review-finding`, does not enter the
workflow doc's acceptance ledger, and does not block I10. If evidence shows a normative contradiction, implementation
defect, or current conformance violation, route it back through the standard finding contract instead.

Every corrective PR is created by the parent orchestrator in an isolated corrective lane using a `/tmp` worktree and
`codex/<finding>-<slug>` branch:

- for an open PR, start from the reviewed PR's fixed head SHA and set the PR base to that active PR branch; the
  primary lane does not advance the active branch during the audit, and the new head is re-audited after the fix
  merges;
- for a merged historical commit, start from the latest `origin/main` and set the PR base to `main`; do not reopen the
  original PR.

A separate branch is not a substitute for an isolated worktree; both are required. Network failure, retry, pending
remote sync, and duplicate-write prevention continue to follow the `gh` rules at the top of this document.

Use `.github/ISSUE_TEMPLATE/review_finding.md` for reviewer-created findings so the fixed snapshot, severity, gate
impact, reproduction, owner, target stage, and corrective acceptance conditions are not omitted.

## Completion

After merge:

1. Send a new final merged/delivered progress comment to the PR and Issue, then confirm the linked Issue closed as intended.
2. Record residual work as linked Issues rather than hidden PR notes.
3. Update plans, implementation matrix, conformance manifests, or dated reviews only when their owning process requires it.
4. Do not rewrite historical review evidence to match the merged implementation.
