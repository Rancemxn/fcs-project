# I0.3 Unique Source Crate Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mixed FCS 4/FCS 5 workspace with one buildable, unversioned `fcs-source` source crate.

**Architecture:** Move the candidate implementation from `crates/fcs-core/src/v5` into the unversioned `crates/fcs-source/src` layout. Remove legacy active crates and modules, retain only source-facing modules, and expose no compatibility re-export. The archive branch remains the only home of the deleted FCS 4 toolchain.

**Tech Stack:** Rust 2024, Cargo workspace, cargo-nextest, Clippy, rustfmt, `fd`, and `rg`.

---

## Task 1: Add the structural red test

**Files:**

- Modify: `crates/fcs-core/tests/workspace_structure.rs`

- [ ] **Step 1: Keep the target topology assertions as the first test boundary**

The test must assert the package name is `fcs-source` and that the active repository has no
`crates/fcs-core`, `crates/fcs-cli`, `crates/fcs-converter`, `crates/fcs-source/src/v4`, or
`crates/fcs-source/src/v5` path. Use the existing test body from
`docs/plans/i0-source-cutover.md` Task 3 without weakening any assertion.

- [ ] **Step 2: Run the focused red check**

Run:

```powershell
cargo nextest run -p fcs-core --test workspace_structure
```

Expected red state: the current package identity is `fcs-core`, and the command may first
report the pre-existing library compilation errors in `src/units` and `src/v5`. Record that
the red state is caused by the pre-cutover topology, not by changing the test expectation.

- [ ] **Step 3: Commit the red test**

```powershell
git add crates/fcs-core/tests/workspace_structure.rs
git commit -m "test(source): lock the unique crate cutover topology"
```

## Task 2: Move the retained source implementation

**Files:**

- Rename: `crates/fcs-core/` to `crates/fcs-source/`
- Rename: `crates/fcs-source/src/v5/ast/` to `crates/fcs-source/src/ast/`
- Rename: `crates/fcs-source/src/v5/elaborator/` to `crates/fcs-source/src/elaborator/`
- Rename: `crates/fcs-source/src/v5/parser/` to `crates/fcs-source/src/parser/`
- Rename: `crates/fcs-source/src/v5/schema.rs` to `crates/fcs-source/src/schema.rs`
- Rename: `crates/fcs-source/src/v5/validation.rs` to `crates/fcs-source/src/validation.rs`
- Rename: `crates/fcs-source/src/v5/version.rs` to `crates/fcs-source/src/version.rs`
- Delete: `crates/fcs-source/src/v5/mod.rs`
- Move: `crates/fcs-source/src/v5/ast/color.rs` as the retained source color module

- [ ] **Step 1: Remove legacy active source modules**

Delete only the active legacy directories under the renamed crate:

```text
src/ast/
src/bytecode/
src/compiler/
src/error/
src/parser/
src/units/
src/vm/
```

Do not touch `refer/` or `archive/fcs4-pre-cutover`.

- [ ] **Step 2: Promote the candidate module tree**

Move the listed `v5` directories/files to their unversioned paths. Ensure `ast/mod.rs`
declares and re-exports `color`, and ensure the new crate root does not declare a `v5` module.

- [ ] **Step 3: Update source-root imports mechanically**

Inside `crates/fcs-source`, change imports to the new roots:

```text
crate::v5::ast         -> crate::ast
crate::v5::elaborator  -> crate::elaborator
crate::v5::parser      -> crate::parser
crate::v5::schema      -> crate::schema
crate::v5::validation  -> crate::validation
crate::v5::version     -> crate::version
fcs_core::v5::         -> fcs_source::
fcs_core::             -> fcs_source::
```

Do not alter `FCS 5`, `5.0.0`, or `FCS_SOURCE_VERSION` strings.

## Task 3: Replace manifests and source entry point

**Files:**

- Modify: `Cargo.toml`
- Modify: `crates/fcs-source/Cargo.toml`
- Modify: `crates/fcs-source/src/lib.rs`
- Regenerate: `Cargo.lock`

- [ ] **Step 1: Set the single workspace member**

The root manifest must contain exactly:

```toml
[workspace]
resolver = "2"
members = ["crates/fcs-source"]

[workspace.package]
version = "0.2.0"
edition = "2024"
license = "MIT"
```

- [ ] **Step 2: Set the source package manifest**

`crates/fcs-source/Cargo.toml` must contain package name `fcs-source`, the workspace version,
edition, license, description `FCS source parser and compile-time elaborator`, and an empty
`[dependencies]` table at this stage.

- [ ] **Step 3: Replace the crate root**

`crates/fcs-source/src/lib.rs` must expose only `ast`, `elaborator`, `parser`, `schema`, and
`version`, with `validation` private. Do not expose legacy modules or a compatibility facade.

- [ ] **Step 4: Regenerate the lockfile**

Run:

```powershell
cargo generate-lockfile
```

Expected: the active lockfile has only the `fcs-source` package and no active dependency on
`fcs-cli`, `fcs-converter`, `nom`, `thiserror`, `serde`, or `bytemuck`.

## Task 4: Migrate FCS 5 tests and examples

**Files:**

- Rename: `crates/fcs-source/tests/fcs5_frontend.rs` to `crates/fcs-source/tests/frontend.rs`
- Rename: `crates/fcs-source/tests/fcs5_phase2.rs` to `crates/fcs-source/tests/compile_time.rs`
- Rename: `examples/fcs/fcs5-chart.fcs` to `examples/fcs/chart.fcs`
- Rename: `examples/fcs/fcs5-fragment.fcs` to `examples/fcs/fragment.fcs`
- Rename: `examples/fcs/fcs5-templates.fcs` to `examples/fcs/templates.fcs`
- Delete: `examples/fcs/easing.fcs`
- Delete: `examples/fcs/empty.fcs`
- Delete: `examples/fcs/multi-line.fcs`
- Delete: `examples/fcs/overlapping.fcs`
- Delete: `examples/fcs/simple.fcs`
- Delete: `examples/fcs/template.fcs`

- [ ] **Step 1: Update test crate references and helper paths**

Change FCS 5 test imports to `fcs_source::...` and update helper paths to `chart.fcs`,
`fragment.fcs`, and `templates.fcs`. Keep all test assertions unchanged.

- [ ] **Step 2: Remove only obsolete active FCS 4 examples**

Delete the six listed FCS 4 examples. Keep PGR, RPE, PEC, and copyright inputs.

## Task 5: Run gates, inspect absence, and commit the cutover

**Files:**

- Verify: entire active workspace, `Cargo.lock`, and archive branch

- [ ] **Step 1: Run quality gates in repository order**

Run:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit 0 and the migrated FCS 5 tests execute under package `fcs-source`.

- [ ] **Step 2: Verify the target topology**

Run:

```powershell
fd --hidden --exclude .git --exclude target --type d '^(v4|v5)$' crates
rg -n --hidden -g '!/.git' -g '!target/**' -g '!refer/**' 'fcs_core|crate::v5|fcs_source::v5|#fcs v4' crates examples Cargo.toml
cargo metadata --no-deps --format-version 1
```

Expected: the searches produce no output and Cargo metadata lists exactly one workspace package
named `fcs-source`.

- [ ] **Step 3: Verify archive safety**

Run:

```powershell
git rev-parse archive/fcs4-pre-cutover
git merge-base --is-ancestor archive/fcs4-pre-cutover master
```

Expected: the archive commit remains `148936d17b671bb34968c88969ab748c818f9fc0` and is an
ancestor of the active `master`.

- [ ] **Step 4: Commit the destructive cutover**

```powershell
git add -A -- Cargo.toml Cargo.lock crates examples/fcs docs/superpowers
git commit -m "refactor: replace the FCS 4 workspace with fcs-source"
```

Do not begin I0.4, I1, or any canonical/runtime implementation in this batch.
