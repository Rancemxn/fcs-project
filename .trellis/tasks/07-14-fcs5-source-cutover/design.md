# Design: safe repository boundary for the FCS 5 source cutover

## Boundary

This task establishes a Git boundary, not a source-code compatibility layer. The existing feature
branch is the input state. A commit containing the approved pre-cutover work becomes the immutable
archive pointer. `master` is then fast-forwarded to that same commit and becomes the only active
development branch for the FCS 5 line.

```text
codex/fcs5-phase2-compile-time-language
                 │
                 ▼
       [pre-cutover snapshot]
          ├── archive/fcs4-pre-cutover
          └── master  (active FCS 5 development)
```

The former feature branch remains as an additional reference for auditability. It is not deleted
or rewritten.

## Snapshot composition

The snapshot is composed in logical commits so a reviewer can distinguish behavior work from
specification/governance state:

- source commit: the four generator-related Rust files and their test;
- specification commit: `AGENTS.md`, the frozen FCS/FCBC/render/conversion specifications,
  `conformance/`, and the `docs/` governance/decision/plan/review material;
- workflow commit, when needed: root `.agents/`, `.codex/`, and `.trellis/` files required to
  preserve the approved project workflow and this task's planning artifacts.

The workflow commit is deliberately explicit because these paths were untracked at inspection
time. It prevents the archive from claiming to preserve the complete current worktree while
silently leaving the project workflow outside the Git snapshot.

## Safety invariants

1. Inspect branch topology and the full status inventory before staging anything.
2. Use explicit path groups; do not use `git add .`, `git add -A`, or `git clean` for the source
   commit because that could mix unrelated work.
3. Run `git diff --cached --check` before each commit.
4. Verify that `archive/fcs4-pre-cutover` is absent immediately before creation.
5. Create the archive with `git branch archive/fcs4-pre-cutover <snapshot>` and compare both SHA
   values.
6. Switch to `master` and use `git merge --ff-only archive/fcs4-pre-cutover`.
7. If any invariant fails, stop before the next mutating command and report the exact state.

## Rollback model

No destructive rollback is needed for a failed precondition. Before the archive pointer is created,
the operation can stop with commits on the feature branch and no branch topology change. After the
pointer and fast-forward succeed, both references are immutable evidence of the same snapshot; any
later source migration can be reverted or repaired on `master` without touching the archive pointer.

The task must never use `git reset --hard`, `git checkout --`, `git clean`, branch deletion, force
updates, or remote operations.

## Expected task metadata

Creating and starting this Trellis task may modify `.trellis/tasks/07-14-fcs5-source-cutover/task.json`
after the initial planning commit. That bookkeeping change is expected and must be distinguished
from source changes in the final status report.
