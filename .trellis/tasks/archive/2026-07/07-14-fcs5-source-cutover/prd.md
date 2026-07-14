# FCS 5 source cutover: archive the pre-cutover tree and activate `master`

## Goal

Preserve the exact repository state immediately before the FCS 5.0 source cutover, then make
`master` the default active development branch for the unversioned FCS 5 implementation.

This task covers the repository/branch boundary only. It does not implement the parser, rename
crates, delete the old active FCS 4 tree, or begin the later I0 source migration tasks.

## Background and confirmed facts

- The current branch is `codex/fcs5-phase2-compile-time-language`.
- `git merge-base master HEAD` equals `git rev-parse master`, so the current branch is a linear
  descendant of `master`.
- `archive/fcs4-pre-cutover` does not exist and must not be overwritten if it appears before the
  archive operation.
- The current worktree contains known FCS 5 generator changes, frozen specification documents,
  conformance fixtures, governance/planning documents, Trellis/Codex project scaffolding, and
  deletions of obsolete `docs/superpowers` material.
- FCS 4 remains available through the archive branch only after this task; the active tree will be
  migrated by the later I0 tasks in `docs/plans/i0-source-cutover.md`.

## Requirements

### R1. Preserve the pre-cutover snapshot

Before changing branches, inspect and classify every path reported by `git status --short`. Preserve
all known in-scope tracked and untracked work in commits; do not reset, clean, stash destructively,
or silently discard files.

The snapshot must include the current generator work, frozen FCS/FCBC/render/conversion documents,
conformance corpus, governance/decision/plan/review documents, project instructions, and the
Trellis/Codex scaffolding required by the approved workflow task.

### R2. Keep commits auditable

Use the I0 plan's logical commit boundaries:

1. generator parser/AST/test work;
2. frozen specifications, conformance, governance, plans, review, and project instructions;
3. project workflow/task scaffolding if it is not already part of the second commit.

Each commit must pass `git diff --cached --check`, and no unrelated file may be included merely to
make the worktree clean.

### R3. Create an immutable archive pointer

After the pre-cutover commits, create `archive/fcs4-pre-cutover` at the exact snapshot commit. If
the branch already exists, stop and compare it; never move or overwrite it.

Verify that the archive contains the old FCS 4 core, CLI, and converter paths.

### R4. Activate `master`

Switch to `master` and fast-forward it to the archive snapshot. The result must have:

- current branch `master`;
- `archive/fcs4-pre-cutover` and `master` at the same snapshot commit;
- no merge commit;
- the original `codex/fcs5-phase2-compile-time-language` branch retained;
- no deletion of any old branch.

### R5. Do not perform later I0 implementation work

Do not rename `crates/fcs-core`, delete old active code, add Chumsky, or alter parser behavior in
this task. Those actions begin only in the subsequent I0 tasks after this cutover boundary is
verified.

## Acceptance criteria

- [x] The complete pre-operation `git status --short` inventory is recorded in the task/journal
      evidence, and every in-scope path is preserved.
- [x] The generator work and frozen FCS 5 planning state are committed in auditable logical commits.
- [x] `archive/fcs4-pre-cutover` exists, points to the exact pre-cutover snapshot, and was never
      overwritten.
- [x] The archive contains the old FCS 4 core, CLI, and converter paths.
- [x] `master` is the current branch and fast-forwards to the exact archive commit.
- [x] `codex/fcs5-phase2-compile-time-language` still exists.
- [x] No source crate migration or parser implementation has been performed as part of this task.
- [x] Final status and branch verification commands are captured; any expected post-start task
      metadata change is explicitly identified.

## Execution evidence

- Preflight branch: `codex/fcs5-phase2-compile-time-language`.
- Preflight archive guard: `archive/fcs4-pre-cutover` absent.
- Source preservation commit: `967e952` (`wip(source): preserve pre-cutover generator parser`).
- Specification/conformance commit: `0ff9cec` (`docs: freeze specifications and plan the source cutover`).
- Workflow/task preservation commit: `148936d` (`chore: preserve project workflow for source cutover`).
- Archive branch: `archive/fcs4-pre-cutover` at `148936d17b671bb34968c88969ab748c818f9fc0`.
- Active branch after cutover: `master` at the same SHA; `git merge --ff-only` succeeded.
- Retained branch: `codex/fcs5-phase2-compile-time-language`.
- Final controlled worktree status: clean.
- Archived legacy path verification: 65 paths under old `fcs-core` AST/parser/compiler/bytecode/VM,
  `fcs-cli`, and `fcs-converter` locations.

## Out of scope

- FCS 5 lexer/parser/compiler implementation.
- Generator semantic expansion or FCBC generation.
- Removing FCS 4 from the active tree.
- Renaming `crates/fcs-core` to `crates/fcs-source`.
- Creating canonical/runtime/FCBC/converter/render/CLI crates.
- Remote push, pull request creation, branch deletion, or history rewriting.
