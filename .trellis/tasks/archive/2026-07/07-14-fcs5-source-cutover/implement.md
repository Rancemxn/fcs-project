# Implementation plan: FCS 5 source cutover boundary

## Phase 0 — planning gate

- [x] Obtain user consent to create a Trellis task and enter planning.
- [x] Create `.trellis/tasks/07-14-fcs5-source-cutover`.
- [x] Record requirements, design, acceptance criteria, and rollback constraints.
- [x] Review this plan and activate the task with `task.py start` before mutating the repository.

## Phase 1 — read-only preflight

Run and record:

```powershell
git branch --show-current
git merge-base master HEAD
git rev-parse master
git status --short --branch
git branch --list --all
git show-ref --verify --quiet refs/heads/archive/fcs4-pre-cutover
git log --oneline --decorate -8
```

Expected starting state:

- current branch: `codex/fcs5-phase2-compile-time-language`;
- merge-base and `master` resolve to the same commit;
- archive branch is absent;
- all dirty paths match the known inventory in `prd.md`.

The archive branch was absent and the dirty paths matched the known inventory; execution proceeded.

## Phase 2 — commit the exact pre-cutover work

1. Stage only the four generator files and test:

   ```powershell
   git add -- crates/fcs-core/src/v5/ast/entity.rs crates/fcs-core/src/v5/ast/mod.rs crates/fcs-core/src/v5/parser/entities.rs crates/fcs-core/tests/fcs5_phase2.rs
   git diff --cached --check
   git commit -m "wip(source): preserve pre-cutover generator parser"
   ```

2. Stage the frozen specifications, conformance corpus, governance documents, and project
   instructions:

   ```powershell
   git add -- AGENTS.md fcs.md fcbc.md fcs-render.md fcs-conversion.md conformance docs
   git diff --cached --check
   git commit -m "docs: freeze specifications and plan the source cutover"
   ```

3. Stage the approved workflow scaffolding and Trellis task artifacts that were part of the
   inspected worktree. Review the staged path list before committing:

   ```powershell
   git add -- .agents .codex .trellis
   git diff --cached --name-status
   git diff --cached --check
   git commit -m "chore: preserve project workflow for source cutover"
   ```

   The staged path group matched the preflight inventory. Two generated Trellis Markdown hard-break
   spaces were intentionally retained; source/specification staged diffs passed `git diff --check`.

## Phase 3 — create and verify the archive branch

```powershell
$snapshot = git rev-parse HEAD
if (git show-ref --verify --quiet refs/heads/archive/fcs4-pre-cutover) { throw "archive/fcs4-pre-cutover already exists" }
git branch archive/fcs4-pre-cutover $snapshot
git rev-parse archive/fcs4-pre-cutover
git show -s --format='%H %s' archive/fcs4-pre-cutover
git ls-tree -r --name-only archive/fcs4-pre-cutover -- crates/fcs-core/src/ast crates/fcs-core/src/parser crates/fcs-core/src/compiler crates/fcs-core/src/bytecode crates/fcs-core/src/vm crates/fcs-cli crates/fcs-converter
```

Both SHA outputs must equal `$snapshot`, and the last command must list the old FCS 4 paths.

## Phase 4 — activate `master`

```powershell
git switch master
git merge --ff-only archive/fcs4-pre-cutover
git branch --show-current
git rev-parse HEAD
git rev-parse archive/fcs4-pre-cutover
git branch --list master archive/fcs4-pre-cutover codex/fcs5-phase2-compile-time-language
```

Expected and observed: current branch is `master`; `HEAD` and archive have the same SHA; all three
branches exist; no merge commit is created.

Update the Trellis task branch metadata to `master` if required by the task script, then capture
the resulting status. Do not delete the former feature branch.

## Phase 5 — verification and handoff

Run:

```powershell
git status --short --branch
git log --oneline --decorate --graph -8
git merge-base --is-ancestor archive/fcs4-pre-cutover master
git ls-tree -r --name-only archive/fcs4-pre-cutover -- crates/fcs-core/src/ast crates/fcs-core/src/parser crates/fcs-core/src/compiler crates/fcs-core/src/bytecode crates/fcs-core/src/vm crates/fcs-cli crates/fcs-converter
```

Observed: the controlled worktree is clean. Do not run parser implementation, crate renames, source
deletion, or broad Rust quality checks as part of this cutover-only task.

## Stop conditions

- unknown dirty path;
- existing archive branch;
- failed staged diff check;
- failed commit;
- `master` not a fast-forward target;
- archive and `master` resolving to different commits;
- missing old FCS 4 paths in the archive;
- any request to delete or rewrite the former feature branch.
