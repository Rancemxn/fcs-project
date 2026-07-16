# GitHub Issues and Pull Requests

GitHub Issues are the repository's work contracts. Pull Requests deliver one reviewable implementation unit and its verification evidence. ADR 0011 accepts this workflow. Neither surface has normative authority: specifications, governance, Accepted ADRs, conformance artifacts, and dated reviews retain the responsibilities defined by `AGENTS.md`.

Use the authenticated `gh` CLI for repository operations. Prefer `--json` plus `jq` over parsing human-readable tables.

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

Use parent/sub-issues for a large effort and dependency links for sequencing:

```text
gh issue create --title "..." --body-file issue.md --label needs-triage
gh issue create --title "..." --body-file child.md --parent 12 --blocked-by 10,11
gh issue edit 12 --add-sub-issue 13 --add-blocked-by 9
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

Start from current `origin/main`. Use `codex/<issue>-<slug>` and keep one reviewable unit per branch. `gh issue develop <number> --base main --name <branch> --checkout` may create and link the branch when the working tree is clean.

Before editing:

1. Read `AGENTS.md` and any closer instructions.
2. Follow the applicable specification/ADR/conformance/review reading route.
3. Reconfirm that the Issue is consistent with the current normative dependency closure.
4. Preserve unrelated worktree changes.

During implementation, append new scope to the Issue or open a follow-up Issue; do not silently expand the PR.

## Pull Request contract

Open a draft PR when early CI or interface feedback is useful. Mark it ready only after the intended scope and local gates are complete.

The PR body records:

- `Closes #<n>` when merge should close the Issue, otherwise `Refs #<n>`;
- summary and non-goals;
- specification, ADR, conformance, review, and version-state impact;
- tests and exact commands run;
- skipped/unavailable gates and reason;
- residual risk and follow-up Issues.

Useful commands:

```text
gh pr create --draft --base main --title "..." --body-file pr.md
gh pr diff <number>
gh pr checks <number> --required
gh pr view <number> --json reviewDecision,mergeable,statusCheckRollup,files |
  jq -S '.'
gh pr ready <number>
```

Do not merge until required checks pass, review requirements are satisfied, the branch is mergeable, and all Critical/Important findings in the applicable gate are closed. Never use `gh pr merge --admin` to bypass protection. Merge only when the user has authorized it.

## Completion

After merge:

1. Confirm the linked Issue closed as intended.
2. Record residual work as linked Issues rather than hidden PR notes.
3. Update plans, implementation matrix, conformance manifests, or dated reviews only when their owning process requires it.
4. Do not rewrite historical review evidence to match the merged implementation.
