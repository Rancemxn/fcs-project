# Phase 1 Integration and Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fast-forward the implemented FCS 5 Phase 1 branch into `master` and record its accurate feature and verification status without requiring future worktrees.

**Architecture:** Keep the existing linear commit history by fast-forwarding `master` to `fcs5-phase1-frontend`. Update the roadmap to require an isolated Git branch while making worktrees optional, and distinguish completed Phase 1 functionality from unresolved workspace-wide verification gates.

**Tech Stack:** Git, Cargo Clippy, cargo-nextest, rustfmt, Markdown.

---

### Task 1: Verify fast-forward integration preconditions

**Files:**

- Modify: Git reference `refs/heads/master`
- Read: `refs/heads/fcs5-phase1-frontend`

- [x] **Step 1: Verify both working directories are clean**

Run: `git status --short` in the root worktree and the existing Phase 1 worktree.

Expected: no output from either command.

- [x] **Step 2: Verify linear fast-forward topology**

Run: `git merge-base --is-ancestor master fcs5-phase1-frontend`.

Expected: exit code `0`, allowing `master` to advance without a merge commit.

### Task 2: Integrate Phase 1 into master

**Files:**

- Modify: Git reference `refs/heads/master`

- [x] **Step 1: Fast-forward master**

Run: `git merge --ff-only fcs5-phase1-frontend` from the root worktree.

Expected: `master` advances from `525625f` to the Phase 1 tip without conflicts or a merge commit.

- [x] **Step 2: Confirm the public v5 front end is present on master**

Run: `rg -n 'pub mod v5' crates/fcs-core/src/lib.rs`.

Expected: one match.

### Task 3: Record the branch-first workflow and Phase 1 status

**Files:**

- Modify: `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md`
- Modify: `docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md`

- [x] **Step 1: Make worktrees optional in the roadmap**

Replace the worktree-only cross-phase rule with a rule requiring an isolated Git branch and allowing a worktree only when useful.

- [x] **Step 2: Add explicit implementation state to the roadmap**

Record Phase 1 as implemented with workspace verification gates pending, and Phases 2–10 as not started.

- [x] **Step 3: Update the Phase 1 completion checklist truthfully**

Mark only the eight functionally verified Phase 1 criteria complete. Keep the existing-v4-test and workspace-gate criteria unchecked, and record the exact Clippy and COPYRIGHT-fixture blockers.

### Task 4: Verify and commit documentation state

**Files:**

- Verify: `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md`
- Verify: `docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md`

- [x] **Step 1: Check documentation formatting and diff quality**

Run: `git diff --check` and `cargo fmt --all -- --check`.

Expected: both commands succeed.

- [x] **Step 2: Re-run focused FCS 5 core verification**

Run: `cargo clippy -p fcs-core --all-targets -- -D warnings` followed by `cargo nextest run -p fcs-core --no-fail-fast`.

Expected: Clippy succeeds and all 80 core tests pass.

- [x] **Step 3: Commit only the status/documentation update**

Run: `git add docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md docs/superpowers/plans/2026-07-13-phase1-integration-status.md` then commit with message `docs: record FCS 5 Phase 1 integration status`.

Expected: a single documentation commit after the fast-forwarded implementation commits.
