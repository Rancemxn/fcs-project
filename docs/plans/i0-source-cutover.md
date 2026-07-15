# I0 Source Cutover Implementation Plan

> **For agentic workers:** Implement this plan task-by-task and update each checkbox (`- [ ]`) as
> its step is completed. No specific agent skill or orchestration framework is required.

> **Current status (2026-07-15):** I0-A through the unique `fcs-source` cutover and stable
> diagnostics are complete. Tasks 8-12 are complete: document, definition, template, and collection
> parsing consume one spanned token stream; the Frozen generator grammar is exact at the I0
> implementation boundary; and byte/property robustness plus typed conformance-manifest gates are
> active. Governance guidance, roadmap, implementation matrix, and freeze-review evidence now match
> the post-cutover workspace. The workspace contains only `fcs-source` and has 134 passing tests.
> Task 13 final structure, dependency, quality, topology, and independent-review gates remain.

**Goal:** Archive the complete pre-cutover FCS 4 workspace, make `master` the sole development
branch, replace the mixed `fcs-core`/`v5` tree with an unversioned `fcs-source` crate, establish a
Chumsky-based parser and stable diagnostic boundary, and finish I0 with a green, auditable
workspace.

**Architecture after Task 4:** `fcs-source` owns source AST, lexing/parsing, static semantics,
compile-time elaboration, construction schema, source versions, and structured diagnostics. FCS 4,
the old CLI, and the old converter remain only on `archive/fcs4-pre-cutover`; future
canonical/runtime/FCBC/converter/render/CLI crates are created in their own roadmap stages and
cannot use source AST as a canonical model.

**Tech Stack:** Rust 2024, Chumsky 0.11.2 stable APIs with its `stacker` feature, Proptest 1.11 for
parser robustness, Serde 1.0.228 for test manifest structs, TOML 1.1.3 for conformance manifest
parsing, cargo-nextest, Clippy, rustfmt, fd, rg, and ast-grep.

**Dependency activation policy:** `Cargo.toml` owns one audited workspace dependency catalog. I0
activates only Chumsky for `fcs-source` runtime code and Proptest/Serde/TOML for its tests. The
remaining entries are approved candidates with a first owning stage; a catalog entry alone must not
be added to `fcs-source`, resolved into its dependency tree, or treated as implemented behavior.
Each owner rechecks the current API, features, MSRV, license, and transitive tree before activation.

| Dependency | First owning stage | Activation rule |
|---|---|---|
| `chumsky` 0.11.2 + `stacker` | I0 | active in `fcs-source`; alpha, Pratt, and unstable remain disabled |
| `proptest` 1.11.0 | I0 | active only as a `fcs-source` dev dependency |
| `serde` 1.0.228 / `toml` 1.1.3 | I0 | active only for typed conformance manifest tests |
| `serde_json` 1.0.150 | I3/I6 | test snapshots first; importer/report serialization when converter exists |
| `nalgebra` 0.35.0 | I4 | private `f64` production transform backend; independent reference path remains separate |
| `sha2` 0.11.0 | I5/I7 | resource, stable ID, source, and golden SHA-256 hashing |
| `crc` 3.4.0 | I7 | FCBC CRC-32/ISO-HDLC only |
| `image` 0.25.10 | I9 | defaults disabled; render crate enables only specified codecs and decode limits |
| `clap` 4.6.1 | I10 | CLI crate only |
| `zip` 8.6.0 | I6/I10 | defaults disabled; activate only for an explicit package-input surface |

---

## Authoritative inputs

Before executing any task, read these files completely:

- `AGENTS.md`
- `docs/decisions/0006-unversioned-source-cutover.md`
- `docs/plans/fcs5-roadmap.md`
- `docs/specification-governance.md`
- `docs/reviews/2026-07-14-fcs5-freeze-review.md`
- `fcs.md`, especially sections 2–6, 16, Appendix B, and Appendix C
- `conformance/manifest.toml`
- `conformance/fcs5/manifest.toml`
- `refer/dependencies/chumsky/Cargo.toml`
- Chumsky tag `0.11` versions of `examples/nano_rust.rs`, `src/input.rs`, and
  `guide/error_and_recovery.md`, read with `git -C refer/dependencies/chumsky show 0.11:<path>`

The decision record controls implementation structure only. If it conflicts with `fcs.md` or the
conformance corpus, the Frozen specification and corpus win.

## Scope boundaries

I0 implements the following source subset on the new parser framework:

- exact `#fcs 5.0.0` header and format profiles;
- tempo map and the current fragment/chart profile rules;
- current typed scalar literals, `vec2`, names, unary/binary expressions, calls, and field access;
- `const`, pure `fn`, typed `let`, statement `if`, and return statements;
- typed templates inside `definitions`, constructors, `with`, collection `if`, generator syntax,
  and `emit` source nodes;
- stable diagnostics, resource limits, and conformance manifest integrity.

I0 does not claim complete source conformance. The following remain assigned to I1 unless a task
below explicitly requires a representation boundary:

- complete metadata/contributors/credits/resources/artwork/sync/custom data grammar;
- complete Line, Track, segment, keyframe, render, extension, and preserve source grammar;
- array/object/reference/index/choose expression source nodes not already needed by the migrated
  candidate implementation;
- every Appendix B production and every source example.

I2 owns complete static semantics and generator expansion. I0 must never produce partial generator
output or treat the temporary implementation diagnostic as a valid FCS outcome.

## Target file map

The active workspace after Task 4 contains:

```text
Cargo.toml
Cargo.lock
crates/fcs-source/Cargo.toml
crates/fcs-source/src/lib.rs
crates/fcs-source/src/diagnostic.rs
crates/fcs-source/src/version.rs
crates/fcs-source/src/validation.rs
crates/fcs-source/src/schema.rs
crates/fcs-source/src/ast/color.rs
crates/fcs-source/src/ast/definitions.rs
crates/fcs-source/src/ast/entity.rs
crates/fcs-source/src/ast/time.rs
crates/fcs-source/src/ast/types.rs
crates/fcs-source/src/ast/mod.rs
crates/fcs-source/src/parser/token.rs
crates/fcs-source/src/parser/input.rs
crates/fcs-source/src/parser/lexer.rs
crates/fcs-source/src/parser/expression.rs
crates/fcs-source/src/parser/definitions.rs
crates/fcs-source/src/parser/entities.rs
crates/fcs-source/src/parser/document.rs
crates/fcs-source/src/parser/header.rs
crates/fcs-source/src/parser/tempo.rs
crates/fcs-source/src/parser/mod.rs
crates/fcs-source/src/elaborator/cycle.rs
crates/fcs-source/src/elaborator/entities.rs
crates/fcs-source/src/elaborator/eval.rs
crates/fcs-source/src/elaborator/scope.rs
crates/fcs-source/src/elaborator/mod.rs
crates/fcs-source/tests/frontend.rs
crates/fcs-source/tests/compile_time.rs
crates/fcs-source/tests/workspace_structure.rs
crates/fcs-source/tests/diagnostic.rs
crates/fcs-source/tests/expression.rs
crates/fcs-source/tests/conformance_manifest.rs
docs/conformance/fcs5-implementation-matrix.md
```

The active workspace no longer contains `crates/fcs-core`, `crates/fcs-cli`, or
`crates/fcs-converter`.

### Task 1: Commit the exact pre-cutover state and create the archive branch — completed

**Files:**

- Preserve: all current tracked and untracked work shown by `git status --short`
- Record in handoff: archive commit SHA and branch names

- [x] **Step 1: Confirm the expected starting branch and linear history**

Run:

```powershell
git branch --show-current
git merge-base master HEAD
git rev-parse master
git status --short --branch
```

Expected:

- current branch is `codex/fcs5-phase2-compile-time-language`;
- `git merge-base master HEAD` equals `git rev-parse master`;
- status contains the known generator changes, Frozen documents, conformance corpus, decision/plan
  documents and governance cleanup;
- no unrelated user file is silently included.

If `archive/fcs4-pre-cutover` already exists, stop and compare its SHA with the intended snapshot;
never move or overwrite an existing archive pointer.

- [x] **Step 2: Commit the pre-cutover generator work without changing it**

Run:

```powershell
git add -- crates/fcs-core/src/v5/ast/entity.rs crates/fcs-core/src/v5/ast/mod.rs crates/fcs-core/src/v5/parser/entities.rs crates/fcs-core/tests/fcs5_phase2.rs
git diff --cached --check
git commit -m "wip(source): preserve pre-cutover generator parser"
```

Expected: one commit containing only the four generator-related files. The known non-exhaustive
elaborator state is allowed in this archival commit and is fixed in Task 2.

- [x] **Step 3: Commit the Frozen specification and I0 planning state**

Run:

```powershell
git add -A -- AGENTS.md fcs.md fcbc.md fcs-render.md fcs-conversion.md conformance docs
git diff --cached --check
git commit -m "docs: freeze specifications and plan the source cutover"
git status --short
```

Expected: the commit contains the Frozen specifications, governance, roadmap, conformance corpus,
review, decisions, I0 plan, implementation matrix, AGENTS changes, and deletion of obsolete dated
plans. `git status --short` prints nothing.

The untracked project workflow and Trellis/Codex scaffolding were committed separately as
`148936d` (`chore: preserve project workflow for source cutover`) so the archive preserves the
complete inspected worktree without mixing workflow files into the specification commit.

- [x] **Step 4: Create and verify the immutable archive pointer**

Run:

```powershell
$snapshot = git rev-parse HEAD
if (git show-ref --verify --quiet refs/heads/archive/fcs4-pre-cutover) { throw "archive/fcs4-pre-cutover already exists" }
git branch archive/fcs4-pre-cutover $snapshot
git rev-parse archive/fcs4-pre-cutover
git show -s --format='%H %s' archive/fcs4-pre-cutover
git ls-tree -r --name-only archive/fcs4-pre-cutover -- crates/fcs-core/src/ast crates/fcs-core/src/parser crates/fcs-core/src/compiler crates/fcs-core/src/bytecode crates/fcs-core/src/vm crates/fcs-cli crates/fcs-converter
```

Expected and observed: both SHA outputs equal `$snapshot`; the archive snapshot is
`148936d17b671bb34968c88969ab748c818f9fc0` with subject
`chore: preserve project workflow for source cutover`; the final command lists the old FCS 4 core,
CLI, and converter paths preserved by the archive.

- [x] **Step 5: Fast-forward `master` and make it the active development branch**

Run:

```powershell
git switch master
git merge --ff-only archive/fcs4-pre-cutover
git branch --show-current
git rev-parse HEAD
git rev-parse archive/fcs4-pre-cutover
```

Expected and observed at cutover time: current branch is `master`; both SHA outputs are equal; no
merge commit is created. Trellis subsequently added task-archive and session-journal bookkeeping
commits on `master`; the archive pointer remains unchanged and is an ancestor of current `master`.

- [x] **Step 6: Do not delete the former feature branch**

Run:

```powershell
git branch --list master archive/fcs4-pre-cutover codex/fcs5-phase2-compile-time-language
```

Expected: all three branches exist. Branch cleanup is outside I0 and requires a separate explicit
decision.

**I0-A evidence:** source preservation commit `967e952`, specification/conformance commit
`0ff9cec`, workflow preservation/archive snapshot `148936d`, archive branch
`archive/fcs4-pre-cutover`, active branch `master`, and retained branch
`codex/fcs5-phase2-compile-time-language`. I0-A is complete; Task 2 is now complete as the first
generator staging implementation step.

### Task 2: Close the generator staging boundary before moving files — completed

**Files:**

- Modify: `crates/fcs-core/src/v5/parser/entities.rs`
- Modify: `crates/fcs-core/src/v5/elaborator/entities.rs`
- Modify: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Modify: `crates/fcs-core/tests/fcs5_phase2.rs`

- [x] **Step 1: Replace the draft range test with Frozen operator tests**

Add these cases to `crates/fcs-core/tests/fcs5_phase2.rs`:

```rust
#[test]
fn parses_half_open_and_inclusive_generator_ranges() {
    for (operator, inclusive_end) in [("..<", false), ("..=", true)] {
        let source = format!(
            "#fcs 5.0.0\n\
             format {{ profile: fragment; }}\n\
             collections {{ notes {{\
               generate at: beat in 0beat{operator}4beat step 1beat {{\
                 emit tap {{ gameplay.time: at; }};\
               }}\
             }} }}"
        );
        let document = parse_document(&source).expect("Frozen range syntax should parse");
        let CollectionItem::Generator(generator) = &document.collections[0].items[0] else {
            panic!("expected generator collection item");
        };
        assert_eq!(generator.range.inclusive_end, inclusive_end);
    }
}

#[test]
fn rejects_bare_generator_range_operator() {
    let source = "#fcs 5.0.0\n\
                  format { profile: fragment; }\n\
                  collections { notes {\
                    generate at: beat in 0beat..4beat step 1beat {\
                      emit tap { gameplay.time: at; };\
                    }\
                  } }";
    assert_eq!(
        parse_document(source),
        Err(ParseError::InvalidSyntax("generator range"))
    );
}
```

- [x] **Step 2: Add a failing elaborator test for explicit staging**

```rust
#[test]
fn generator_elaboration_fails_before_partial_output() {
    let source = "#fcs 5.0.0\n\
                  format { profile: fragment; }\n\
                  collections { notes {\
                    tap { gameplay.time: 0beat; };\
                    generate at: beat in 1beat..<3beat step 1beat {\
                      emit tap { gameplay.time: at; };\
                    }\
                  } }";
    let document = parse_document(source).expect("generator source should parse");
    assert!(matches!(
        elaborate(&document, &phase2_schema(), CompileTimeLimits::default()),
        Err(Diagnostic::FeatureUnavailable {
            feature: "compile-time-generator",
            ..
        })
    ));
}
```

- [x] **Step 3: Run the tests and observe the intended failures**

Run:

```powershell
cargo nextest run -p fcs-core --test fcs5_phase2
```

Observed: the pre-change build failed on the non-exhaustive `CollectionItem::Generator` match;
after the test was corrected and the boundary implementation was applied, the targeted suite passed
58/58.

- [x] **Step 4: Make range tokenization strict in the existing parser**

Replace the operator branch in `parse_generator` with:

```rust
let inclusive_end = if cursor.take_text("..<") {
    false
} else if cursor.take_text("..=") {
    true
} else {
    return Err(ParseError::InvalidSyntax("generator range"));
};
```

Delete `is_literal_zero`; zero-step validity belongs to elaboration.

- [x] **Step 5: Add the explicit temporary elaborator error**

Add to the existing diagnostic enum:

```rust
FeatureUnavailable {
    feature: &'static str,
    span: SourceSpan,
},
```

Add this match arm before constructor/expression handling:

```rust
CollectionItem::Generator(generator) => {
    return Err(Diagnostic::FeatureUnavailable {
        feature: "compile-time-generator",
        span: generator.span,
    });
}
```

- [x] **Step 6: Run targeted and full gates**

Run in order:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Observed: Clippy passed with `-D warnings`; workspace nextest passed 227/227; rustfmt check and
diff check passed; the previous non-exhaustive `Generator` error is absent.

- [x] **Step 7: Commit the staging boundary**

Commit: `eef7fbf` (`fix(source): make generator staging explicit`). The temporary task archive and
session journal commits are bookkeeping only; no source files outside this task were changed.

```powershell
git add crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "fix(source): make generator staging explicit"
```

### Task 3: Write the structural cutover test

**Files:**

- Create: `crates/fcs-core/tests/workspace_structure.rs`

- [x] **Step 1: Add the failing structure test**

```rust
use std::path::Path;

#[test]
fn workspace_has_one_unversioned_source_implementation() {
    assert_eq!(env!("CARGO_PKG_NAME"), "fcs-source");

    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate must live under <repo>/crates/<name>");

    for removed in [
        "crates/fcs-core",
        "crates/fcs-cli",
        "crates/fcs-converter",
        "crates/fcs-source/src/v4",
        "crates/fcs-source/src/v5",
    ] {
        assert!(
            !repository.join(removed).exists(),
            "legacy path remains active: {removed}"
        );
    }
}
```

- [x] **Step 2: Run it and verify the correct red state**

```powershell
cargo nextest run -p fcs-core --test workspace_structure
```

Expected: FAIL at `CARGO_PKG_NAME`, reporting `fcs-core` instead of `fcs-source`.

### Task 4: Delete FCS 4 and promote the source crate

**Files:**

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Rename: `crates/fcs-core/` to `crates/fcs-source/`
- Delete: `crates/fcs-cli/`
- Delete: `crates/fcs-converter/`
- Delete: old V4 modules under `crates/fcs-core/src/`
- Rename: V5 source modules to unversioned paths listed in the target file map
- Rename: `crates/fcs-core/tests/fcs5_frontend.rs` to `crates/fcs-source/tests/frontend.rs`
- Rename: `crates/fcs-core/tests/fcs5_phase2.rs` to `crates/fcs-source/tests/compile_time.rs`
- Rename: `examples/fcs/fcs5-chart.fcs` to `examples/fcs/chart.fcs`
- Rename: `examples/fcs/fcs5-fragment.fcs` to `examples/fcs/fragment.fcs`
- Rename: `examples/fcs/fcs5-templates.fcs` to `examples/fcs/templates.fcs`
- Delete: `examples/fcs/easing.fcs`
- Delete: `examples/fcs/empty.fcs`
- Delete: `examples/fcs/multi-line.fcs`
- Delete: `examples/fcs/overlapping.fcs`
- Delete: `examples/fcs/simple.fcs`
- Delete: `examples/fcs/template.fcs`

- [x] **Step 1: Move the only retained V4 value type into the candidate AST**

Move `crates/fcs-core/src/units/color.rs` to `crates/fcs-core/src/v5/ast/color.rs` and add to
`crates/fcs-core/src/v5/ast/mod.rs`:

```rust
mod color;

pub use color::Color;
```

Change candidate imports from `crate::units::Color` to `crate::v5::ast::Color` and test imports
from `fcs_core::units::Color` to `fcs_core::v5::ast::Color`.

- [x] **Step 2: Remove active V4 crates and modules**

Delete exactly:

```text
crates/fcs-cli/
crates/fcs-converter/
crates/fcs-core/src/ast/
crates/fcs-core/src/bytecode/
crates/fcs-core/src/compiler/
crates/fcs-core/src/error/
crates/fcs-core/src/parser/
crates/fcs-core/src/units/
crates/fcs-core/src/vm/
```

Do not delete anything under `refer/` or `archive/fcs4-pre-cutover`.

- [x] **Step 3: Rename the crate and promote candidate modules**

Perform these repository renames:

```text
crates/fcs-core                         -> crates/fcs-source
crates/fcs-source/src/v5/ast           -> crates/fcs-source/src/ast
crates/fcs-source/src/v5/elaborator    -> crates/fcs-source/src/elaborator
crates/fcs-source/src/v5/parser        -> crates/fcs-source/src/parser
crates/fcs-source/src/v5/schema.rs     -> crates/fcs-source/src/schema.rs
crates/fcs-source/src/v5/validation.rs -> crates/fcs-source/src/validation.rs
crates/fcs-source/src/v5/version.rs    -> crates/fcs-source/src/version.rs
```

Delete the now-empty `crates/fcs-source/src/v5/mod.rs` and `src/v5/` directory.

- [x] **Step 4: Replace package and workspace manifests**

`Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/fcs-source"]

[workspace.package]
version = "0.2.0"
edition = "2024"
license = "MIT"
```

`crates/fcs-source/Cargo.toml` at this stage:

```toml
[package]
name = "fcs-source"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "FCS source parser and compile-time elaborator"

[dependencies]
```

- [x] **Step 5: Replace the crate entry point**

`crates/fcs-source/src/lib.rs`:

```rust
//! FCS source syntax, parsing, static semantics, and compile-time elaboration.
//!
//! Canonical chart, runtime, FCBC, conversion, render, and CLI concerns live in
//! separate roadmap crates and must not be represented by this source AST.

pub mod ast;
pub mod elaborator;
pub mod parser;
pub mod schema;
pub mod version;

mod validation;
```

- [x] **Step 6: Remove version prefixes from all Rust paths**

Make these mechanical replacements only inside `crates/fcs-source`:

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

Do not replace textual `FCS 5`, `5.0.0`, or `FCS_SOURCE_VERSION` references.

- [x] **Step 7: Rename tests and active FCS 5 examples**

Apply the test/example renames listed in this task. Update helper paths from
`fcs5-chart.fcs`, `fcs5-fragment.fcs`, and `fcs5-templates.fcs` to `chart.fcs`,
`fragment.fcs`, and `templates.fcs`.

Delete the six FCS 4 examples listed in this task. Do not delete PGR/RPE/PEC or copyright
fixtures; they remain reference inputs even though the converter crate is absent.

- [x] **Step 8: Regenerate the lockfile through Cargo**

```powershell
cargo generate-lockfile
```

Expected: the lockfile no longer contains `fcs-cli`, `fcs-converter`, `nom`, `thiserror`,
`serde`, or `bytemuck` at this intermediate stage.

- [x] **Step 9: Run the structural and migrated test gates**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit 0; at least the 82 pre-cutover candidate tests execute under package
`fcs-source`; `workspace_structure` passes.

- [x] **Step 10: Verify absence instead of trusting renames**

```powershell
fd --hidden --exclude .git --exclude target --type d '^(v4|v5)$' crates
rg -n --hidden -g '!/.git' -g '!target/**' -g '!refer/**' 'fcs_core|crate::v5|fcs_source::v5|#fcs v4' crates examples Cargo.toml
cargo metadata --no-deps --format-version 1
```

Expected: both searches produce no matches; metadata lists exactly `fcs-source` as a workspace
member.

- [x] **Step 11: Commit the destructive cutover**

```powershell
git add -A -- Cargo.toml Cargo.lock crates examples/fcs
git commit -m "refactor: replace the FCS 4 workspace with fcs-source"
```

### Task 5: Establish stable diagnostics and multi-error parse output

**Files:**

- Create: `crates/fcs-source/src/diagnostic.rs`
- Create: `crates/fcs-source/tests/diagnostic.rs`
- Modify: `crates/fcs-source/src/lib.rs`
- Modify: `crates/fcs-source/src/parser/*.rs`
- Modify: `crates/fcs-source/src/elaborator/*.rs`
- Modify: `crates/fcs-source/src/validation.rs`
- Modify: `crates/fcs-source/tests/frontend.rs`
- Modify: `crates/fcs-source/tests/compile_time.rs`

- [x] **Step 1: Write public contract tests first**

`crates/fcs-source/tests/diagnostic.rs`:

```rust
use fcs_source::ast::SourceSpan;
use fcs_source::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::parse_document;
use fcs_source::schema::phase2_schema;

#[test]
fn missing_header_has_the_frozen_code_and_byte_span() {
    let result = parse_document("format { profile: fragment; }");
    let errors = result.into_result().expect_err("missing header must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::VERSION_MISSING_HEADER);
    assert_eq!(errors[0].stage(), DiagnosticStage::Parse);
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(0, 0)
    );
}

#[test]
fn diagnostics_are_sorted_by_span_then_code() {
    let result = parse_document("#fcs 5.0.0\nformat { profile: nope; extra }");
    let diagnostics = result.diagnostics();
    assert!(diagnostics.len() >= 2, "recovery must retain both independent errors");
    assert!(diagnostics.windows(2).all(|pair| {
        let left = {
            let span = pair[0].primary_span();
            (span.start, span.end)
        };
        let right = {
            let span = pair[1].primary_span();
            (span.start, span.end)
        };
        left < right || (left == right && pair[0].code() <= pair[1].code())
    }));
}

#[test]
fn same_scope_duplicate_binding_is_distinct_from_shadowing() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\n\
                  definitions { const A: int = 1; const A: int = 2; }";
    let document = parse_document(source)
        .into_result()
        .expect("duplicate binding is an elaboration error");
    let errors = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect_err("duplicate definitions must fail");
    assert_eq!(errors[0].code(), DiagnosticCode::NAME_DUPLICATE);
    assert!(!errors[0].labels().is_empty());
}
```

- [x] **Step 2: Run the tests and verify the API is absent**

```powershell
cargo nextest run -p fcs-source --test diagnostic
```

Expected: compilation fails because `fcs_source::diagnostic` and `ParseOutput` do not exist.

- [x] **Step 3: Add the diagnostic data model**

Create `crates/fcs-source/src/diagnostic.rs` with this public shape. It includes every Frozen
Appendix C category, even when I0 does not emit a category owned by I1–I4 yet, so later stages do
not invent incompatible spellings. `resource.limit-exceeded` is the I0 implementation-defined
subcategory for parser resource limits; it is distinct from
`compile-time.budget-exceeded`, which is reserved for the six elaboration budgets in FCS 6.8.

```rust
use std::fmt;

use crate::ast::SourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    pub const DECODE_INVALID_UTF8: Self = Self("decode.invalid-utf8");
    pub const VERSION_MISSING_HEADER: Self = Self("version.missing-header");
    pub const VERSION_INVALID: Self = Self("version.invalid");
    pub const VERSION_UNSUPPORTED: Self = Self("version.unsupported");
    pub const SYNTAX_INVALID_TOKEN: Self = Self("syntax.invalid-token");
    pub const SYNTAX_UNCLOSED_COMMENT: Self = Self("syntax.unclosed-comment");
    pub const SYNTAX_UNCLOSED_STRING: Self = Self("syntax.unclosed-string");
    pub const SYNTAX_TRAILING_INPUT: Self = Self("syntax.trailing-input");
    pub const SYNTAX_MISPLACED_BLOCK: Self = Self("syntax.misplaced-block");
    pub const NAME_UNKNOWN: Self = Self("name.unknown");
    pub const NAME_DUPLICATE: Self = Self("name.duplicate");
    pub const NAME_SHADOWED: Self = Self("name.shadowed");
    pub const NAME_CYCLE: Self = Self("name.cycle");
    pub const TYPE_MISMATCH: Self = Self("type.mismatch");
    pub const TYPE_INVALID_OPERATION: Self = Self("type.invalid-operation");
    pub const TYPE_INVALID_CONVERSION: Self = Self("type.invalid-conversion");
    pub const SCHEMA_UNKNOWN_FIELD: Self = Self("schema.unknown-field");
    pub const SCHEMA_DUPLICATE_FIELD: Self = Self("schema.duplicate-field");
    pub const SCHEMA_MISSING_REQUIRED_FIELD: Self = Self("schema.missing-required-field");
    pub const SCHEMA_NON_CONSTRUCTIBLE: Self = Self("schema.non-constructible");
    pub const SCHEMA_COLLECTION_TYPE_MISMATCH: Self =
        Self("schema.collection-type-mismatch");
    pub const SCHEMA_DYNAMIC_FIELD_FORBIDDEN: Self = Self("schema.dynamic-field-forbidden");
    pub const COMPILE_TIME_NON_CONSTANT_CONDITION: Self =
        Self("compile-time.non-constant-condition");
    pub const COMPILE_TIME_INVALID_RANGE: Self = Self("compile-time.invalid-range");
    pub const COMPILE_TIME_ZERO_STEP: Self = Self("compile-time.zero-step");
    pub const COMPILE_TIME_NESTED_GENERATOR: Self = Self("compile-time.nested-generator");
    pub const COMPILE_TIME_MISPLACED_GENERATOR: Self =
        Self("compile-time.misplaced-generator");
    pub const COMPILE_TIME_BUDGET_EXCEEDED: Self = Self("compile-time.budget-exceeded");
    pub const NUMERIC_NON_FINITE: Self = Self("numeric.non-finite");
    pub const NUMERIC_DIVIDE_BY_ZERO: Self = Self("numeric.divide-by-zero");
    pub const NUMERIC_DOMAIN: Self = Self("numeric.domain");
    pub const NUMERIC_OVERFLOW: Self = Self("numeric.overflow");
    pub const TEMPO_INVALID: Self = Self("tempo.invalid");
    pub const TEMPO_NON_MONOTONIC: Self = Self("tempo.non-monotonic");
    pub const TRACK_INVALID_INTERVAL: Self = Self("track.invalid-interval");
    pub const TRACK_OVERLAP: Self = Self("track.overlap");
    pub const TRACK_REPLACE_CONFLICT: Self = Self("track.replace-conflict");
    pub const TRACK_GAP: Self = Self("track.gap");
    pub const TRACK_INVALID_EASING: Self = Self("track.invalid-easing");
    pub const GRAPH_UNKNOWN_PARENT: Self = Self("graph.unknown-parent");
    pub const GRAPH_CYCLE: Self = Self("graph.cycle");
    pub const NOTE_INVALID_HOLD: Self = Self("note.invalid-hold");
    pub const RESOURCE_UNKNOWN_REFERENCE: Self = Self("resource.unknown-reference");
    pub const RESOURCE_TYPE_MISMATCH: Self = Self("resource.type-mismatch");
    pub const RESOURCE_HASH_MISMATCH: Self = Self("resource.hash-mismatch");
    pub const EXPRESSION_CYCLE: Self = Self("expression.cycle");
    pub const EXPRESSION_ENVIRONMENT_UNAVAILABLE: Self =
        Self("expression.environment-unavailable");
    pub const BAKING_ERROR_BUDGET_UNSATISFIED: Self =
        Self("baking.error-budget-unsatisfied");
    pub const EXTENSION_UNSUPPORTED_REQUIRED: Self =
        Self("extension.unsupported-required");
    pub const PROFILE_REQUIREMENT_MISSING: Self = Self("profile.requirement-missing");
    pub const REPAIR_APPLIED: Self = Self("repair.applied");
    pub const RESOURCE_LIMIT_EXCEEDED: Self = Self("resource.limit-exceeded");
    pub const IMPLEMENTATION_FEATURE_UNAVAILABLE: Self =
        Self("implementation.feature-unavailable");

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStage {
    Decode,
    Parse,
    Elaborate,
    Canonical,
    Evaluate,
    Implementation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExpansionTraceKind {
    Const,
    Function,
    Template,
    Collection,
    Range,
    Generator,
    Emit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionTraceFrame {
    kind: ExpansionTraceKind,
    subject: Option<String>,
    index: Option<usize>,
    emitted_type: Option<String>,
    span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetDetails {
    kind: String,
    limit: usize,
    observed: usize,
}

impl BudgetDetails {
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub const fn limit(&self) -> usize {
        self.limit
    }

    pub const fn observed(&self) -> usize {
        self.observed
    }
}

impl ExpansionTraceFrame {
    pub(crate) fn new(
        kind: ExpansionTraceKind,
        subject: Option<String>,
        index: Option<usize>,
        emitted_type: Option<String>,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind,
            subject,
            index,
            emitted_type,
            span,
        }
    }

    pub const fn kind(&self) -> ExpansionTraceKind {
        self.kind
    }

    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    pub const fn index(&self) -> Option<usize> {
        self.index
    }

    pub fn emitted_type(&self) -> Option<&str> {
        self.emitted_type.as_deref()
    }

    pub const fn span(&self) -> Option<SourceSpan> {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    span: SourceSpan,
    message: String,
}

impl DiagnosticLabel {
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    stage: DiagnosticStage,
    severity: DiagnosticSeverity,
    message: String,
    primary_span: SourceSpan,
    labels: Vec<DiagnosticLabel>,
    expansion_trace: Vec<ExpansionTraceFrame>,
    budget: Option<BudgetDetails>,
}

impl Diagnostic {
    pub(crate) fn new(
        code: DiagnosticCode,
        stage: DiagnosticStage,
        message: impl Into<String>,
        primary_span: SourceSpan,
    ) -> Self {
        Self {
            code,
            stage,
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            expansion_trace: Vec::new(),
            budget: None,
        }
    }

    pub(crate) fn with_label(
        mut self,
        span: SourceSpan,
        message: impl Into<String>,
    ) -> Self {
        self.labels.push(DiagnosticLabel {
            span,
            message: message.into(),
        });
        self
    }

    pub(crate) fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub(crate) fn with_trace_frame(mut self, frame: ExpansionTraceFrame) -> Self {
        self.expansion_trace.push(frame);
        self
    }

    pub(crate) fn with_budget(mut self, kind: impl Into<String>, limit: usize, observed: usize) -> Self {
        self.budget = Some(BudgetDetails {
            kind: kind.into(),
            limit,
            observed,
        });
        self
    }

    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    pub const fn stage(&self) -> DiagnosticStage {
        self.stage
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn primary_span(&self) -> SourceSpan {
        self.primary_span
    }

    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }

    pub fn expansion_trace(&self) -> &[ExpansionTraceFrame] {
        &self.expansion_trace
    }

    pub fn budget(&self) -> Option<&BudgetDetails> {
        self.budget.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseOutput<T> {
    output: Option<T>,
    diagnostics: Vec<Diagnostic>,
}

impl<T> ParseOutput<T> {
    pub(crate) fn new(output: Option<T>, mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort_by_key(|diagnostic| {
            (
                diagnostic.primary_span.start,
                diagnostic.primary_span.end,
                diagnostic.code,
            )
        });
        diagnostics.dedup_by(|left, right| {
            left.code == right.code && left.primary_span == right.primary_span
        });
        Self {
            output: diagnostics.is_empty().then_some(output).flatten(),
            diagnostics,
        }
    }

    pub fn output(&self) -> Option<&T> {
        self.output.as_ref()
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_result(self) -> Result<T, Vec<Diagnostic>> {
        match (self.output, self.diagnostics) {
            (Some(output), diagnostics) if diagnostics.is_empty() => Ok(output),
            (_, diagnostics) => Err(diagnostics),
        }
    }
}
```

Export `pub mod diagnostic;` from `lib.rs`.

- [x] **Step 4: Map every existing error variant explicitly**

Use this mapping; do not invent a second spelling:

| Existing error | Stable code |
|---|---|
| MissingHeader | `version.missing-header` |
| InvalidVersion | `version.invalid` |
| UnsupportedSourceVersion | `version.unsupported` |
| MissingRequiredBlock | `profile.requirement-missing` |
| InvalidTempoMap | `tempo.invalid` or `tempo.non-monotonic` according to cause |
| InvalidSyntax | the most specific `syntax.*` code |
| DuplicateBinding | `name.duplicate` |
| ShadowedBinding | `name.shadowed` |
| UnknownName/UnknownTemplate/UnknownCollection | `name.unknown` |
| RecursiveConst/RecursiveFunction/RecursiveTemplate | `name.cycle` |
| TypeMismatch/MissingReturn/InvalidReturn/WrongArity | `type.mismatch` |
| InvalidOperation | `type.invalid-operation`, except divide-by-zero/non-finite/domain/overflow causes map to the corresponding `numeric.*` category |
| UnknownEntityField | `schema.unknown-field` |
| DuplicateEntityField | `schema.duplicate-field` |
| MissingRequiredField | `schema.missing-required-field` |
| NonConstructibleEntity | `schema.non-constructible` |
| CollectionTypeMismatch | `schema.collection-type-mismatch` |
| NonConstantStructuralCondition | `compile-time.non-constant-condition` |
| LimitExceeded | `compile-time.budget-exceeded` |
| FeatureUnavailable | `implementation.feature-unavailable` |

Keep secondary spans as labels for shadowed/duplicate declarations and cycle participants.
For recursive const/function/template diagnostics, also materialize the ordered dependency chain as
`ExpansionTraceFrame` entries of the corresponding kind; labels alone are not the structured cycle
trace required by FCS Appendix C.

All diagnostics emitted by I0 have `severity = Error`; no I0 path creates a warning. Cycle and
budget diagnostics must append ordered `ExpansionTraceFrame` values rather than flattening the
trace into the human message. The public code constants for canonical/evaluate categories are
forward-compatible declarations only in I0. Task 10 adds the byte entry point that emits
`decode.invalid-utf8`; the existing `&str` entry points continue to accept already-decoded UTF-8.

- [x] **Step 5: Change parser and elaborator entry points**

Public parser signatures are exact and uniform:

```rust
pub fn parse_header(source: &str) -> ParseOutput<Version>;
pub fn parse_document(source: &str) -> ParseOutput<Document>;
pub fn parse_expression(source: &str) -> ParseOutput<SourceExpression>;
pub fn parse_type(source: &str) -> ParseOutput<Type>;
```

`parse_header` validates the first header (including an optional leading BOM) and returns only the
version; it does not expose the remainder. It may be called on a complete document, while
`parse_document` is the entry point that requires the complete document and rejects trailing
non-trivia input. `parse_expression_with_limits`, `parse_type_with_limits`, and
`parse_document_with_limits` are public
bounded variants; `parse_header` always uses the default source limit because it only validates the
header prefix. Internally `parse_header_tokens` consumes exactly the header and its required line
boundary, then returns the parser cursor to `parse_document_tokens`; it must not call `end()` and
must not mistake the first document block for a duplicate header. `parse_document` owns header
parsing and no longer exposes a raw unconsumed string.
Public elaboration returns:

```rust
pub fn elaborate(
    document: &Document,
    schema: &ConstructionSchema,
    limits: CompileTimeLimits,
) -> Result<ExpandedSourceDocument, Vec<Diagnostic>>;
```

Internal helpers may return one `Diagnostic`; the public entry wraps it in a one-element vector.
Delete the old public `elaborator::Diagnostic` enum. `elaborator::mod.rs` must instead
`pub use crate::diagnostic::Diagnostic`, and every old variant construction must be converted at
the same time to `Diagnostic::new(code, stage, message, span)`, adding labels for secondary
spans and preserving the old structured payload in the message only where no public field exists
yet. The only I0 implementation diagnostic is constructed with
`DiagnosticCode::IMPLEMENTATION_FEATURE_UNAVAILABLE` and
`DiagnosticStage::Implementation`; all other I0 diagnostics use `Parse` or `Elaborate`.
`CompileTimeLimits` remains public, but its six fields are not parser limits and its error path is
the only I0 path allowed to emit `compile-time.budget-exceeded`.
When migrating the existing `Budget` helper, every limit failure must call
`with_budget(kind, limit, observed)` before returning; do not keep the old `limit: &'static str`
payload as message-only data. I0 may still use separate pre-existing budgets where the matrix
assigns shared-budget semantics to I2, but every emitted budget diagnostic must already expose its
kind, configured limit, observed count, primary span, and ordered trace.
Update `validation.rs` at the same boundary: `validate_profile` returns the project `Diagnostic`
and maps a chart/playable/publishable document without `tempoMap` to
`profile.requirement-missing` at the document/profile span. No validation helper may retain a
`ParseError` return type after this task. This profile diagnostic is emitted at `DiagnosticStage::Parse`;
only schema/name/type/compile-time checks performed after a `Document` exists use `Elaborate`.
Change `Scope::declare` while migrating diagnostics: a name already present in the current frame
produces `name.duplicate`, while a name found only in an enclosing frame produces `name.shadowed`;
both diagnostics label the previous declaration span. Do not preserve the old single
`ShadowedBinding` variant for both cases. Run the same duplicate-name preflight over the root
definitions namespace before `cycle::reject_cycles`/`eval::check_and_evaluate`; a `BTreeMap::insert`
overwrite is not an acceptable duplicate check, and const/function/template names share the one
Frozen definitions namespace.

- [x] **Step 6: Rewrite tests to assert code/span instead of private variants**

Add shared helpers inside each integration test file:

```rust
use fcs_source::diagnostic::{Diagnostic, ParseOutput};

fn only_error<T>(output: ParseOutput<T>) -> Diagnostic {
    let mut diagnostics = match output.into_result() {
        Ok(_) => panic!("source must fail"),
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(diagnostics.len(), 1);
    diagnostics.pop().unwrap()
}
```

Replace variant matches with stable code and relevant payload assertions through message/labels.
In particular, migrate every old `let (rest, version) = parse_header(...)` assertion to
`parse_header(...).into_result() == Ok(version)` and move document-level trailing-input checks to
`parse_document`; no test may depend on a raw remainder string. Migrate `parse_type(...).unwrap()`
and `parse_expression(...).unwrap()` to `into_result().expect(...)` (or the shared helper) so all
four public parser functions exercise the same `ParseOutput` contract.

- [x] **Step 7: Run diagnostics and full tests**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit 0; public tests no longer import `ParseError` or match elaborator enum
variants.

- [x] **Step 8: Commit the diagnostic boundary**

```powershell
git add crates/fcs-source
git commit -m "refactor(source): stabilize diagnostic output"
```

### Task 6: Replace the hand-written lexer with Chumsky

**Files:**

- Modify: `Cargo.toml`
- Modify: `crates/fcs-source/Cargo.toml`
- Create: `crates/fcs-source/src/parser/token.rs`
- Create: `crates/fcs-source/src/parser/input.rs`
- Rewrite: `crates/fcs-source/src/parser/lexer.rs`
- Modify: `crates/fcs-source/src/parser/mod.rs`
- Modify: `Cargo.lock`

- [x] **Step 1: Add lexer contract tests**

Add these unit tests at the bottom of `src/parser/lexer.rs` so private tokens do not become public
test API:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceSpan;
    use crate::diagnostic::DiagnosticCode;
    use crate::parser::{ParseLimits, input::{source_span, SpannedToken}, token::Punctuation};

    fn tokens(source: &str) -> Vec<SpannedToken> {
        lex(source, ParseLimits::default()).expect("source should lex")
    }

    #[test]
    fn longest_match_distinguishes_range_operators() {
        let exclusive = tokens("0beat..<4beat");
        assert!(matches!(
            exclusive[1].0,
            Token::Punctuation(Punctuation::RangeExclusive)
        ));
        let inclusive = tokens("0beat..=4beat");
        assert!(matches!(
            inclusive[1].0,
            Token::Punctuation(Punctuation::RangeInclusive)
        ));
    }

    #[test]
    fn bare_range_is_not_two_dot_tokens() {
        let errors = lex("0beat..4beat", ParseLimits::default()).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    }

    #[test]
    fn unicode_spans_are_utf8_byte_offsets() {
        let source = "\"雪\" + value";
        let tokens = tokens(source);
        assert_eq!(source_span(tokens[0].1), SourceSpan::new(0, "\"雪\"".len()));
        assert_eq!(source_span(tokens[2].1).start, "\"雪\" + ".len());
    }

    #[test]
    fn nested_comments_and_string_escapes_are_deterministic() {
        let tokens = tokens("/* outer /* inner */ end */ \"a\\n\\u{96ea}\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::Literal(_)));
    }
}
```

Add adjacent unit cases for BOM, CRLF, unclosed nested comment, comment-depth limit, unclosed
string, unknown escape, malformed color, non-finite float, source byte limit, token count limit,
and literal length limit. Assert the three resource-limit cases use
`DiagnosticCode::RESOURCE_LIMIT_EXCEEDED`, and assert the non-finite literal uses
`DiagnosticCode::NUMERIC_NON_FINITE`.

- [x] **Step 2: Run lexer tests and observe failures**

```powershell
cargo nextest run -p fcs-source --lib
```

Expected: compilation fails because token/input modules, `ParseLimits`, and the new lexer boundary
do not exist.

- [x] **Step 3: Add the audited workspace catalog and activate Chumsky**

Root `Cargo.toml`:

```toml
[workspace.dependencies]
chumsky = { version = "=0.11.2", default-features = false, features = ["std", "stacker"] }
clap = { version = "4.6.1", features = ["derive"] }
crc = "3.4.0"
image = { version = "0.25.10", default-features = false }
nalgebra = { version = "0.35.0", default-features = false, features = ["std"] }
proptest = { version = "1.11.0", default-features = false, features = ["std", "handle-panics"] }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.150"
sha2 = { version = "0.11.0", default-features = false }
toml = { version = "1.1.3", default-features = false, features = ["parse", "serde"] }
zip = { version = "8.6.0", default-features = false }
```

`crates/fcs-source/Cargo.toml` at this task activates only the parser dependency:

```toml
[dependencies]
chumsky.workspace = true
```

Run `cargo update -p chumsky --precise 0.11.2` after the first dependency resolution and verify
`Cargo.lock` records `0.11.2`, not an alpha release. The `stacker` feature may add `stacker`, `psm`,
`cc`, and platform support crates transitively. Explicit source/nesting limits remain the semantic
resource boundary because stack growth can be a no-op on unsupported targets. If WebAssembly is
added to the supported target matrix, it needs a target-specific parser-stack decision and compile
gate before support is claimed.

- [x] **Step 4: Define owned tokens and punctuation**

`token.rs` must define three layers:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Literal(SourceLiteral),
    Identifier(String),
    Keyword(Keyword),
    Punctuation(Punctuation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Punctuation {
    LeftParenthesis,
    RightParenthesis,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Colon,
    Semicolon,
    Dot,
    At,
    Arrow,
    FatArrow,
    RangeExclusive,
    RangeInclusive,
    Plus,
    Minus,
    Star,
    Power,
    Slash,
    Percent,
    Bang,
    Equal,
    EqualEqual,
    BangEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    AndAnd,
    OrOr,
}
```

`Keyword` must contain every reserved word listed in `fcs.md` section 2.4; implement one exact
`from_identifier(&str) -> Option<Keyword>` mapping and test every reserved word through a table.
Reserved words are not general `Name` expressions. The token parser must nevertheless handle the
Frozen lexical roles explicitly: `true` and `false` become boolean literals, `vec2` is accepted
only as the built-in callable used by the `vec2(...)` AST special case, and scalar/entity type
keywords are accepted only by `parse_type` or constructor/template grammar. `null`, arrays,
objects, references, and other reserved constructs outside the I0 AST are rejected rather than
reclassified as identifiers.

- [x] **Step 5: Define parser input aliases and span conversion**

`input.rs`:

```rust
use chumsky::{input::MappedInput, prelude::SimpleSpan};

use super::token::Token;
use crate::ast::SourceSpan;

pub(crate) type ChumskySpan = SimpleSpan<usize>;
pub(crate) type SpannedToken = (Token, ChumskySpan);
pub(crate) type TokenInput<'tokens> =
    MappedInput<'tokens, Token, ChumskySpan, &'tokens [SpannedToken]>;

pub(crate) fn source_span(span: ChumskySpan) -> SourceSpan {
    SourceSpan::new(span.start, span.end)
}
```

- [x] **Step 6: Implement the Chumsky lexer**

Follow the stable `refer/dependencies/chumsky` tag `0.11` `nano_rust` pattern. The parser factory
receives the limits that affect recursive trivia and literal conversion; do not hide those limits
in globals:

```rust
fn lexer<'source>(limits: ParseLimits) -> impl Parser<
    'source,
    &'source str,
    Vec<SpannedToken>,
    extra::Err<Rich<'source, char, ChumskySpan>>,
> {
    choice((literal(limits.max_literal_bytes), identifier_or_keyword(), punctuation()))
        .map_with(|token, extra| (token, extra.span()))
        .padded_by(trivia(limits.max_comment_depth).repeated())
        .padded()
        .repeated()
        .collect()
}
```

Implement nested block comments as a recursive trivia parser. Do not use `skip_then_retry_until`
for valid tokenization; unknown characters must emit a diagnostic at their exact byte span.
The literal parsers must implement the Frozen lexical forms rather than delegating to permissive
host parsers: decimal integer/float grammar from FCS 2.5 (no leading `+`, hex, `NaN`, or
`Infinity`), `\n`, `\r`, `\t`, `\\`, `\"`, `\0`, and `\u{1–6 hex digits}` string escapes from
FCS 2.6 (reject unknown escapes, raw newlines, surrogate values, and non-scalar values), and
exactly six or eight hexadecimal digits for colors from FCS 2.7. Preserve raw token spans while
storing decoded string values and canonical color values. A malformed exponent must be rejected as
one invalid numeric token rather than split into an integer followed by an identifier. The numeric lexer emits a separate
`Minus` punctuation token; unary parsing supplies the permitted negative sign, so a sign is never
silently absorbed into a literal token and a leading `+` remains invalid.

- [x] **Step 7: Enforce explicit limits outside parser recursion**

Define the public limits value in `parser/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub max_source_bytes: usize,
    pub max_tokens: usize,
    pub max_nesting_depth: usize,
    pub max_comment_depth: usize,
    pub max_literal_bytes: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 16 * 1024 * 1024,
            max_tokens: 1_000_000,
            max_nesting_depth: 512,
            max_comment_depth: 256,
            max_literal_bytes: 1 * 1024 * 1024,
        }
    }
}
```

The crate-private lexer boundary is:

```rust
pub(super) fn lex(
    source: &str,
    limits: ParseLimits,
) -> Result<Vec<SpannedToken>, Vec<Diagnostic>>;
```

`lex` rejects `source.len() > limits.max_source_bytes` before constructing the Chumsky parser,
calls `lexer(limits).parse(source)`, converts every `Rich` error, and then enforces the token and
literal limits on the returned spans. The parser factory itself receives the comment/literal limits
so nested trivia and literal parsers cannot recurse or allocate without the same bound.

Reject source length before lexing and token count immediately after lexing. Track delimiter nesting
over the token stream before invoking recursive grammar, and track active nested block-comment
depth inside the recursive trivia parser. `max_literal_bytes` counts the raw UTF-8 bytes covered by
each literal token span, including string delimiters and escape spelling. These are implementation
resource limits, not FCS syntax aliases; violations use `resource.limit-exceeded` and never
`compile-time.budget-exceeded`.

- [x] **Step 8: Convert Rich errors into stable diagnostics**

Map unclosed comment/string explicitly. Map unknown token, invalid punctuation, malformed color,
unknown escape, and a guarded single-dot failure to `syntax.invalid-token`; map a syntactically
valid decimal whose exact conversion is non-finite to `numeric.non-finite`. Preserve `Rich::span()`
through `source_span`; do not expose Chumsky types from `fcs-source` public modules.

The punctuation parser must recognize `..<` and `..=` before an explicit rejecting `just("..")`
parser, and the single-dot parser must be guarded by
`just('.').then_ignore(just('.').not())`. The rejecting parser uses `try_map`/`Rich::custom` and
consumes both dots, so `a.b` is valid, `a..b` produces one `syntax.invalid-token` diagnostic at the
two-dot span, and the lexer cannot silently split a forbidden bare range into two `Dot` tokens. A
leading BOM is consumed exactly once as trivia while all token spans continue to use offsets in the
original source; a later U+FEFF is not trivia and is rejected as an invalid token.

- [x] **Step 9: Run lexer, existing, and dependency checks**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo tree -p fcs-source
cargo fmt --all -- --check
git diff --check
```

Expected: all tests pass; the normal tree contains Chumsky `0.11.2` and its reviewed stack-growth
dependencies. It contains no Chumsky `1.0.0-alpha.*` and has no direct Logos, Ariadne, Winnow,
`nom`, `bytemuck`, or `thiserror` dependency. Future-stage workspace catalog entries remain absent
from the `fcs-source` tree until an owning crate activates them.

- [x] **Step 10: Commit the lexer migration**

```powershell
git add Cargo.toml crates/fcs-source Cargo.lock
git commit -m "refactor(parser): tokenize FCS source with Chumsky"
```

### Task 7: Rebuild expression parsing on stable Chumsky combinators

**Files:**

- Rewrite: `crates/fcs-source/src/parser/expression.rs`
- Modify: `crates/fcs-source/src/parser/mod.rs`
- Modify: `crates/fcs-source/src/ast/types.rs`
- Modify: `crates/fcs-source/src/elaborator/eval.rs`
- Create: `crates/fcs-source/tests/expression.rs`
- Modify: `crates/fcs-source/tests/compile_time.rs`

- [x] **Step 1: Add precedence and span tests**

Add table tests for:

```text
a || b && c
a == b < c
a < b <= c
a + b * c
-a * b
f(a).field
vec2(1px, 2px)
(a + b) * c
```

For every table row, assert the exact AST shape and complete half-open span. Add invalid cases for
missing operands, unmatched delimiters, chained trailing input, and reserved words used as names.

- [x] **Step 2: Verify the red state against the token parser API**

```powershell
cargo nextest run -p fcs-source --test expression
```

Expected: compilation or assertions fail because `expression.rs` still consumes raw text or the old
token representation.

- [x] **Step 3: Implement parser aliases over spanned tokens**

Use the tag `0.11` split-token pattern:

```rust
use chumsky::input::Input as _;

let end = ChumskySpan::new((), source.len()..source.len());
let input = tokens.as_slice().split_token_span(end);
expression_parser().parse(input);
```

Parser functions are generic over `ValueInput<Token = Token, Span = ChumskySpan>` as demonstrated
by `refer/dependencies/chumsky` tag `0.11` `examples/nano_rust.rs`.

- [x] **Step 4: Add the public expression entry points**

After the lexer returns tokens, keep Chumsky types inside `parser` and expose only the project
result type:

```rust
pub fn parse_expression(source: &str) -> ParseOutput<SourceExpression> {
    parse_expression_with_limits(source, ParseLimits::default())
}

pub fn parse_expression_with_limits(
    source: &str,
    limits: ParseLimits,
) -> ParseOutput<SourceExpression> {
    match lex(source, limits) {
        Ok(tokens) => parse_expression_tokens(source, &tokens),
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}
```

`parse_expression_tokens` is the crate-private Chumsky token parser implemented in the next step.

Implement `parse_type` in the same task; it must not keep the old raw-string cursor. Its public
boundary is:

```rust
pub fn parse_type(source: &str) -> ParseOutput<Type> {
    match lex(source, ParseLimits::default()) {
        Ok(tokens) => parse_type_tokens(source, &tokens),
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}
```

`parse_type_tokens` consumes the entire token input and accepts the I0 type grammar
`bool | int | float | string | time | beat | length | angle | color | Note | Line | RenderNode`
and the recursive generic forms `vec2<T>`, `TrackSegment<T>`, and `Keyframe<T>`. It rejects
unknown generic names, missing angle delimiters, nested types that are not legal under the
existing `Type` representation, reserved words in type positions, and trailing input with the
same stable diagnostic boundary as expressions. Document parsers call this crate-private token
parser directly so a nested type does not re-lex a substring or lose its original byte span.

- [x] **Step 5: Implement one precedence function per Frozen grammar layer**

Use `foldl_with` for left-associative layers and `recursive` for unary/primary nesting. The `power`
layer is right-associative; the Frozen precedence order makes unary bind more tightly than power,
so `-a ** b` parses as `(-a) ** b`. Do not use
`Parser::pratt`, `unstable`, or a manually indexed token cursor. The I0 subset must preserve current
AST behavior for literals, names, unary, binary, call, field access, vec2, and parentheses. In
particular, the reserved `vec2` token is lowered to the same special two-argument
`SourceExpression::Vec2` node as the candidate parser; it is never accepted as a user binding name.

Add `BinaryOperator::Power` to the source AST and cover it in the expression parser. Until I2
defines the complete numeric/domain rule, the elaborator maps power evaluation to the existing
`type.invalid-operation` path rather than silently evaluating it with host-specific behavior.

Tokens for Appendix B constructs not represented by the I0 AST must fail with
`syntax.invalid-token` at the first unsupported token; they must not be skipped or converted into a
different expression.

Chumsky's reviewed `stacker` feature owns recursive stack growth. Do not clone the token stream,
spawn a parser thread, or reserve a fixed 64 MiB stack. Reject inputs beyond `ParseLimits` before
recursive grammar and let an ordinary parser panic remain a test failure rather than converting an
arbitrary panic into a resource-limit diagnostic.

Preserve the Frozen comparison-chain meaning for the retained order-operator subset: parse
`a < b <= c` as the equivalent `a < b && b <= c` tree, with the complete source span on the
generated logical node and the middle operand retained in both comparison nodes. Do not silently
left-associate it as `(a < b) <= c`. Add this case to the exact-AST table; a later I1 AST extension
may replace the representation without changing the source meaning.

- [x] **Step 6: Preserve deterministic parse error selection**

Use `.labelled()` and `.as_context()` at expression, argument-list, and delimiter boundaries.
Convert all resulting Rich errors, sort by byte span then code, and deduplicate identical
`(code, span)` pairs before constructing `ParseOutput`.

- [x] **Step 7: Run expression and full gates**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected: all existing expression/type/elaborator tests and new precedence tests pass.

- [x] **Step 8: Commit expression parsing**

```powershell
git add crates/fcs-source/src crates/fcs-source/tests
git commit -m "refactor(parser): parse expressions from spanned tokens"
```

### Task 8: Rebuild document, definition, template, and collection grammar

**Files:**

- Rewrite: `crates/fcs-source/src/parser/header.rs`
- Rewrite: `crates/fcs-source/src/parser/tempo.rs`
- Rewrite: `crates/fcs-source/src/parser/definitions.rs`
- Rewrite: `crates/fcs-source/src/parser/entities.rs`
- Rewrite: `crates/fcs-source/src/parser/document.rs`
- Modify: `crates/fcs-source/src/parser/mod.rs`
- Modify: `crates/fcs-source/src/ast/definitions.rs`
- Modify: `crates/fcs-source/src/ast/entity.rs`
- Modify: `crates/fcs-source/src/ast/mod.rs`
- Modify: `crates/fcs-source/src/elaborator/cycle.rs`
- Modify: `crates/fcs-source/src/elaborator/entities.rs`
- Modify: `crates/fcs-source/src/elaborator/eval.rs`
- Modify: `crates/fcs-source/src/elaborator/scope.rs`
- Modify: `crates/fcs-source/tests/frontend.rs`
- Modify: `crates/fcs-source/tests/compile_time.rs`
- Modify: `examples/fcs/templates.fcs`

- [x] **Step 1: Add characterization tests for every migrated block**

Add one valid and one invalid test for:

```text
header
format
tempoMap
definitions const
definitions fn
definitions template
function let/if/return
template let/if/return entity
collection constructor
collection if
with expression
generator/emit source node
trailing input
duplicate/misplaced top-level block
```

All valid examples use only Frozen grammar. The templates example must place template declarations
inside `definitions`; a top-level `templates` block must fail with `syntax.misplaced-block`.
The I0 `format` subset is intentionally `format { profile: <profile>; }`: it preserves the current
`Document` AST and does not yet add the Frozen `features` list. A `features` field must be rejected
with `syntax.invalid-token` rather than ignored; I1 adds the source feature node and profile
validation. Add one regression case for this explicit deferral.
Duplicate `format`, `tempoMap`, `definitions`, or `collections` blocks use
`name.duplicate` at the second block span with the first block span as a label; an unknown or
misplaced block uses `syntax.misplaced-block` at its keyword. This distinction must survive parser
recovery and be asserted in the characterization tests.

- [x] **Step 2: Run the new cases and capture draft divergences**

```powershell
cargo nextest run -p fcs-source --test frontend --test compile_time
```

Expected: failures identify old raw-text parsers, top-level `templates`, draft range spelling, or
non-token-aware scanning. Record only these expected failures in the task review; investigate any
unrelated failure before continuing.

- [x] **Step 3: Move template declarations into definitions AST**

`DefinitionsBlock` owns `Vec<Definition>`, and `Definition` gains a template variant:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    Const(ConstDeclaration),
    Function(FunctionDeclaration),
    Template(TemplateDeclaration),
}
```

Move `TemplateParameter` and `TemplateDeclaration` to `ast/definitions.rs` (they are definitions,
not collection entities), remove `TemplatesBlock`, and define the template body with typed
template statements:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateDeclaration {
    pub return_type: Type,
    pub name: String,
    pub name_span: SourceSpan,
    pub parameters: Vec<TemplateParameter>,
    pub body: Vec<TemplateStatement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateStatement {
    Let(LetStatement),
    If(TemplateIfStatement),
    Return(ReturnEntityStatement),
}

impl TemplateStatement {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Let(statement) => statement.span,
            Self::If(statement) => statement.span,
            Self::Return(statement) => statement.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateIfStatement {
    pub condition: SourceExpression,
    pub then_branch: Vec<TemplateStatement>,
    pub else_branch: Vec<TemplateStatement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnEntityStatement {
    pub value: EntityExpression,
    pub span: SourceSpan,
}
```

Parse the Frozen order `template constructibleType identifier(parameters) { ... }`; do not accept
the old draft `template name(parameters) -> Type` spelling. Remove `Document.templates` and
`TemplatesBlock`. Update elaborator lookup to collect templates from `Document.definitions`. Do not
accept a separate top-level templates block.
Update `ast/mod.rs` to re-export `TemplateParameter`, `TemplateDeclaration`, `TemplateStatement`,
`TemplateIfStatement`, and `ReturnEntityStatement` from `definitions`; `ast/entity.rs` must
re-export neither `TemplatesBlock` nor the moved declaration types. `Document` retains only its
optional `DefinitionsBlock` and collection list—there is no compatibility field for old template
blocks.

The parser must accept only these template statements: `let name: type = expression;`,
`if expression { templateStatement* } else { templateStatement* }`, and
`return entityExpression;`. The `else` branch may be absent in the AST, but elaboration then
requires every reachable path to return. A template return type is the leading constructible type;
I0 delegates whether that type is present in the selected `ConstructionSchema` to elaboration.

The elaborator migration is part of this task, not an I2 placeholder: collect template declarations
from the definitions block; include template-to-template calls in the cycle graph; type-check every
template branch and every return before selecting a branch; evaluate template `let` bindings in a
child `Scope`; and expand the selected `ReturnEntityStatement` through the existing constructor,
call, and `with` validation. `TemplateIfStatement` must use the same compile-time-bool rule as
function and collection `if`. `expand_template_call` must pass template parameters and local lets
to entity-expression expansion, increment the existing template-instance/depth counters, and
never append a partially constructed entity. `FunctionStatement` and `TemplateStatement` remain
distinct enums because a function return is a value expression while a template return is an
entity expression.

- [x] **Step 4: Compose document parsers from token parsers**

Each module exposes one crate-private parser function; `document.rs` composes them and consumes
`end()`. Block parsers must use delimited token combinators and structured recovery boundaries, not
raw substring search.

The public document entry points mirror expression parsing:

```rust
pub fn parse_document(source: &str) -> ParseOutput<Document> {
    parse_document_with_limits(source, ParseLimits::default())
}

pub fn parse_document_with_limits(
    source: &str,
    limits: ParseLimits,
) -> ParseOutput<Document> {
    match lex(source, limits) {
        Ok(tokens) => parse_document_tokens(source, &tokens),
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}
```

`parse_document_tokens` is crate-private and must reject non-trivia input after the document parser.

- [x] **Step 5: Restrict I0 to the declared source subset**

Recognize unknown reserved top-level block starts so the diagnostic is `syntax.misplaced-block` at
the block keyword. Also recognize the legacy plural identifier `templates` explicitly and report
it as `syntax.misplaced-block`; it is not a Frozen reserved word, but silently treating it as an
ordinary unknown identifier would make the migration failure nondeterministic. Do not create empty
AST placeholders for metadata, Track, render, extension, or preserve blocks; I1 adds those source
nodes with their fixtures.

- [x] **Step 6: Remove the old scanning implementation completely**

After token parser tests pass, delete every old cursor/string scanning helper, including functions
matching:

```text
Cursor
until_top_level
until_keyword_or
until_range_operator
position_before
take_text
skip_trivia
```

Verify with:

```powershell
rg -n 'struct Cursor|until_top_level|until_keyword_or|until_range_operator|position_before|take_text' crates/fcs-source/src/parser
```

Expected: no matches.

- [x] **Step 7: Run all parser and elaborator tests**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit 0; no parser module parses nested grammar by substring scanning.

- [x] **Step 8: Complete the document grammar migration implementation**

```powershell
git add crates/fcs-source examples/fcs/templates.fcs
git commit -m "refactor(parser): compose FCS document grammar with Chumsky"
```

### Task 9: Finalize the I0 generator boundary on the new parser

**Files:**

- Modify: `crates/fcs-source/src/parser/entities.rs`
- Modify: `crates/fcs-source/src/elaborator/entities.rs`
- Modify: `crates/fcs-source/src/diagnostic.rs`
- Modify: `crates/fcs-source/src/ast/definitions.rs`
- Modify: `crates/fcs-source/src/ast/entity.rs`
- Modify: `crates/fcs-source/tests/compile_time.rs`
- Read: `conformance/fcs5/source/invalid/bare-range.fcs`
- Read: `conformance/fcs5/source/invalid/generator-zero-step.fcs`

- [x] **Step 1: Add final public diagnostic tests**

```rust
#[test]
fn bare_range_uses_the_frozen_syntax_category() {
    let output = parse_document(include_str!(
        "../../../conformance/fcs5/source/invalid/bare-range.fcs"
    ));
    let errors = output.into_result().expect_err("bare range must fail");
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
}

#[test]
fn valid_generator_is_rejected_only_at_the_implementation_boundary() {
    let source = "#fcs 5.0.0\n\
                  format { profile: fragment; }\n\
                  collections { notes {\
                    generate at: beat in 0beat..<3beat step 1beat {\
                      emit tap { gameplay.time: at; };\
                    }\
                  } }";
    let document = parse_document(source)
        .into_result()
        .expect("minimal generator syntax must parse");
    let errors = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect_err("I0 must not partially elaborate generators");
    assert_eq!(
        errors[0].code(),
        DiagnosticCode::IMPLEMENTATION_FEATURE_UNAVAILABLE
    );
    assert_eq!(errors[0].stage(), DiagnosticStage::Implementation);
}
```

- [x] **Step 2: Run and observe any remaining mismatch**

```powershell
cargo nextest run -p fcs-source --test compile_time
```

Expected: any failure points to range token mapping or the temporary diagnostic conversion, not a
non-exhaustive match.

- [x] **Step 3: Enforce exact generator grammar**

The parser accepts only:

```text
generate identifier : (int | beat) in expression (..< | ..=) expression step expression
{ generatorStatement* }
```

Generator body accepts typed `let`, compile-time `if`, and `emit`. I0 parses generators only as
collection items in the retained fragment/chart subset; Track/segment generators remain I1 source
grammar. I0 must not accidentally recurse into a generator parser from generator statements. It
does not claim to emit the I2-only `compile-time.nested-generator` or
`compile-time.misplaced-generator` categories yet: nested or misplaced generator forms are
rejected as unsupported source-subset syntax and are recorded as blocked-by-I2 in the matrix.
I2 must replace that rejection with the Frozen categories before those fixtures are executed.

Reuse the typed source binding shape already used by function statements:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum GeneratorItem {
    Let(LetStatement),
    Conditional {
        condition: SourceExpression,
        then_items: Vec<GeneratorItem>,
        else_items: Vec<GeneratorItem>,
        span: SourceSpan,
    },
    Emit(EntityExpression),
}

impl GeneratorItem {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Let(statement) => statement.span,
            Self::Conditional { span, .. } => *span,
            Self::Emit(expression) => expression.span(),
        }
    }
}
```

Use this minimum parser fixture for the local binding regression:

```fcs
generate i: int in 0..=1 step 1 {
    let t: int = i;
    if true {
        emit tap { gameplay.time: 0beat; };
    }
}
```

Assert the `Let` initializer span and the `Conditional`/`Emit` nesting. Add one parser test that rejects `return` inside a
generator body. The I0 elaborator still returns `implementation.feature-unavailable` before
evaluating any generator item. Add an explicit assertion that a valid generator containing a
zero-step literal parses successfully and is rejected at the same implementation boundary; this
prevents the parser from reintroducing the old literal-only zero-step check.

- [x] **Step 4: Keep zero-step validation out of parse**

Verify a literal zero step parses into AST. Until I2 implements range evaluation, elaboration hits
the feature-unavailable boundary before range output. Do not emit `compile-time.zero-step` without
evaluating the typed step expression.

- [x] **Step 5: Run full gates and commit**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
git add crates/fcs-source
git commit -m "fix(source): enforce Frozen generator syntax"
```

### Task 10: Add byte decoding and the property robustness gate

**Files:**

- Modify: `crates/fcs-source/Cargo.toml`
- Modify: `crates/fcs-source/src/parser/document.rs`
- Modify: `crates/fcs-source/src/parser/mod.rs`
- Create: `crates/fcs-source/tests/robustness.rs`
- Modify: `Cargo.lock`

- [x] **Step 1: Add failing byte-entry contract tests**

Add ordinary regression tests for valid UTF-8, a malformed byte in the middle of otherwise valid
source, and an incomplete UTF-8 sequence at EOF. Invalid input must produce exactly
`decode.invalid-utf8` at `DiagnosticStage::Decode`; its primary span begins at
`Utf8Error::valid_up_to()` and ends after `error_len()` bytes, or at input length for an incomplete
sequence. It must not lex a valid prefix or return a partial document.

- [x] **Step 2: Activate the bounded Proptest configuration**

`crates/fcs-source/Cargo.toml`:

```toml
[dev-dependencies]
proptest.workspace = true
```

Use this checked-in configuration:

```rust
use proptest::{
    prelude::ProptestConfig,
    test_runner::{RngAlgorithm, RngSeed},
};

ProptestConfig {
    cases: 512,
    failure_persistence: None,
    rng_algorithm: RngAlgorithm::ChaCha,
    rng_seed: RngSeed::Fixed(0xF0C5_0001),
    ..ProptestConfig::default()
}
```

The baseline nextest lane must be reproducible and must not write `proptest-regressions` into the
worktree. Convert each minimized failure into an ordinary named regression test; randomized/fuzz
expansion remains available to later dedicated lanes.

- [x] **Step 3: Add the byte API without a second parser**

Add public bounded and default entry points:

```rust
pub fn parse_document_bytes(source: &[u8]) -> ParseOutput<Document>;
pub fn parse_document_bytes_with_limits<L: Into<ParseLimits>>(
    source: &[u8],
    limits: L,
) -> ParseOutput<Document>;
```

Decode once with `std::str::from_utf8`, then delegate to the same token/document parser used by the
`&str` API. Do not add a lossy decoder, encoding detector, byte lexer, or new dependency.

- [x] **Step 4: Add parser invariants as properties**

Generate bounded arbitrary byte vectors and UTF-8 strings containing delimiters, comments,
escapes, numeric fragments, and non-ASCII scalars. Assert for every case:

- parsing never panics or aborts;
- every diagnostic span is ordered and bounded by the original byte length;
- spans over valid UTF-8 begin and end on UTF-8 boundaries;
- the same input and limits produce identical output and ordered diagnostics;
- diagnostics imply `output().is_none()`;
- source, token, nesting, comment, and literal limits fail with `resource.limit-exceeded` before
  unbounded recursion or allocation;
- invalid UTF-8 fails only at decode and never reaches syntax diagnostics.

- [x] **Step 5: Run robustness and full gates**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run -p fcs-source --test robustness
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
git add crates/fcs-source/Cargo.toml crates/fcs-source/src/parser crates/fcs-source/tests/robustness.rs Cargo.lock
git commit -m "test(source): harden byte parsing properties"
```

### Task 11: Add the typed conformance manifest integrity gate

**Files:**

- Modify: `crates/fcs-source/Cargo.toml`
- Create: `crates/fcs-source/tests/conformance_manifest.rs`
- Modify: `Cargo.lock`

- [x] **Step 1: Add a typed manifest loader test**

Define these strongly typed test-only structs (all with `#[serde(deny_unknown_fields)]`) for root
suites and FCS fixtures; do not deserialize into `toml::Value` and inspect keys ad hoc:

```rust
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootManifest {
    schema_version: u32,
    freeze_baseline: String,
    suite: Vec<SuiteEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteEntry {
    id: String,
    specification: String,
    version: String,
    manifest: String,
    mutations: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcsManifest {
    schema_version: u32,
    fcs_version: String,
    fixture: Vec<FixtureEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureEntry {
    id: String,
    path: String,
    stage: FixtureStage,
    expect: FixtureExpectation,
    diagnostic: Option<String>,
    expected: Option<String>,
    vector: Option<String>,
    limits: Option<FixtureLimits>,
    trace_contains: Option<Vec<String>>,
    clauses: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixtureStage { Parse, Elaborate, Canonical, Evaluate }

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixtureExpectation { Success, Error }

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureLimits {
    #[serde(rename = "maxGeneratorIterations")]
    max_generator_iterations: Option<usize>,
}
```

Apply `deny_unknown_fields` to every struct (including `RootManifest`, `SuiteEntry`, `FcsManifest`,
and `FixtureEntry`) in the actual code. The first test must load both manifests with
`toml::from_str`, assert schema version 1, assert the root has six suites, and assert the current
FCS fixture count is 22.
Resolve the repository root from `Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")`; read
`conformance/manifest.toml` and `conformance/fcs5/manifest.toml` as UTF-8, and never use the
process current directory as the base for fixture paths.

- [x] **Step 2: Confirm the cataloged test dependencies are active**

```powershell
cargo nextest run -p fcs-source --test conformance_manifest
```

Serde derive and TOML were already activated when the approved I0 dependency catalog was applied,
so this historical red step was superseded by the dependency update.

- [x] **Step 3: Activate the cataloged manifest dev-dependencies**

```toml
[dev-dependencies]
serde.workspace = true
toml.workspace = true
```

TOML's `parse` feature transitively uses Winnow. This is an approved test-only implementation
detail, not permission to add Winnow as a direct dependency or build a second FCS parser with it.

- [x] **Step 4: Validate manifest invariants**

Tests must assert:

- suite and fixture IDs are nonempty and unique;
- root `freeze_baseline` is `2026-07-14`, and the FCS manifest `fcs_version` is `5.0.0`;
- root suite manifest paths resolve relative to `conformance/`, FCS fixture paths and their
  `expected`/vector references resolve relative to `conformance/fcs5/`, and every resolved path
  stays below the canonical `conformance/` directory after component checking;
- every referenced file exists and is a regular file;
- stages are one of `parse`, `elaborate`, `canonical`, `evaluate`;
- expectations are `success` or `error`;
- error entries have one diagnostic; success entries have none;
- clauses are nonempty;
- expected/vector references exist;
- no expected diagnostic starts with `implementation.`;
- fixture IDs `source.invalid.bare-range`, `source.invalid.generator-zero-step`, and
  `source.valid.compile-time-generator` exist with their Frozen expected outcomes.

Do not execute canonical/evaluate fixtures in I0.

- [x] **Step 5: Run manifest and full gates**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit 0; manifest tests report 6 root suites and 22 FCS fixtures.

- [x] **Step 6: Complete the manifest gate implementation**

```powershell
git add crates/fcs-source/Cargo.toml crates/fcs-source/tests/conformance_manifest.rs Cargo.lock
git commit -m "test(conformance): validate the Frozen source manifest"
```

### Task 12: Complete the implementation matrix and governance updates

**Files:**

- Modify: `docs/conformance/fcs5-implementation-matrix.md`
- Modify: `docs/plans/fcs5-roadmap.md`
- Modify: `docs/reviews/2026-07-14-fcs5-freeze-review.md`
- Modify: `AGENTS.md`
- Modify: `docs/decisions/0006-unversioned-source-cutover.md` only if implementation exposed a
  factual path/API error

- [x] **Step 1: Update every I0 matrix row with evidence**

Before editing the matrix, update `AGENTS.md`'s repository-structure paragraph from the pre-cutover
state to the post-cutover state: active `master` contains only `crates/fcs-source`, while the old
FCS 4/core/CLI/converter paths are available only through `archive/fcs4-pre-cutover`. Remove the
pre-cutover wording and the active `crates/fcs-core/src/v5` target path; retain the fd/rg/sg,
nextest, Context7, and authority rules. This makes the repository guidance agree with the final
structural gate rather than describing the state that was just deleted.

For each row, replace planned paths/status with actual paths and one of:

```text
implemented
partial
not-started
blocked-by-I1
blocked-by-I2
blocked-by-I3
blocked-by-I4
blocked-by-I5
blocked-by-I6
blocked-by-I7
blocked-by-I8
blocked-by-I9
blocked-by-I10
```

Every `implemented` row needs at least one test or conformance fixture. Every `partial` row needs a
concrete missing behavior and its next stage.

- [x] **Step 2: Verify roadmap no longer promises V4 coexistence**

```powershell
$stale = rg -n 'v4.*(共存|并行|兼容)|v5.*re-export|fcs_core|crate::v5|fcs_source::v5|crates/fcs-core/src/v5' docs/plans/fcs5-roadmap.md AGENTS.md
if ($stale) { throw "stale active-version claim found:`n$stale" }
```

Expected: no stale active-version or compatibility claim. Bare mentions of `fcs-core`, `fcs-cli`,
or `fcs-converter` are not searched here because archive/path explanations and future roadmap
crate names are legitimate; the structural absence gate in Task 13 checks the active tree.

- [x] **Step 3: Recompute the revised roadmap hash without overwriting history**

```powershell
Get-FileHash -Algorithm SHA256 docs/plans/fcs5-roadmap.md
```

Append the actual revised hash to the freeze review amendment. Keep the original
`96c0398165c280c9c923c424c49e6c5e1f4512290f349846908ef6aada7edbf5` as the frozen-date
historical value.

- [x] **Step 4: Re-run Markdown and unresolved-marker checks**

Check target-repository Markdown, excluding `refer/`, `.git/`, and `target/`, for:

- balanced fences;
- blank line after headings;
- trailing whitespace;
- U+FFFD;
- stale legacy-plan path references;
- unresolved `T[O]DO`, `T[B]D`, “有待商榷”, or “待定” markers outside text that explicitly
  prohibits those markers.

- [x] **Step 5: Commit governance evidence**

```powershell
git add -A -- AGENTS.md docs
git commit -m "docs: record the completed I0 source cutover"
```

### Task 13: Run final structure, quality, and review gates

**Files:**

- Verify: entire workspace
- Verify: `archive/fcs4-pre-cutover`
- Verify: current `master`

- [ ] **Step 1: Run the structure searches**

```powershell
fd --hidden --exclude .git --exclude target --type d '^(v4|v5)$' crates
rg -n --hidden -g '!/.git' -g '!target/**' -g '!refer/**' 'fcs_core|crate::v5|fcs_source::v5|#fcs v4' crates examples Cargo.toml
rg -n --hidden -g '!/.git' -g '!target/**' -g '!refer/**' 'implementation\.' conformance
```

Expected: all three commands produce no matches.

- [ ] **Step 2: Verify Cargo topology and dependency policy**

```powershell
cargo metadata --no-deps --format-version 1
cargo tree -p fcs-source -e normal
cargo tree -p fcs-source -e dev
cargo tree -p fcs-source -e features
```

Expected:

- one workspace package named `fcs-source`;
- Chumsky is exactly `0.11.2`, with `std` and `stacker`, and no alpha release;
- no direct Logos, Ariadne, Winnow, `nom`, `bytemuck`, or `thiserror` dependency;
- Proptest, Serde, and TOML appear only through dev/test resolution;
- Winnow appears only below TOML's approved dev-only parse feature;
- `serde_json`, `nalgebra`, `sha2`, `crc`, `image`, `clap`, and `zip` are cataloged at the workspace
  root but absent from the `fcs-source` normal and dev trees until their owning stages activate them;
- all resolved crates use registry sources and no dependency points into `refer/`.

- [ ] **Step 3: Run quality gates in repository order**

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected: every command exits 0. Record the exact nextest pass count in the final handoff; do not
reuse the pre-cutover count.

- [ ] **Step 4: Verify archive and branch topology**

```powershell
git branch --show-current
git rev-parse archive/fcs4-pre-cutover
git merge-base --is-ancestor archive/fcs4-pre-cutover master
git log --oneline --decorate archive/fcs4-pre-cutover..master
```

Expected: current branch is `master`; archive is an ancestor of master; the range lists only I0
implementation/governance commits.

- [ ] **Step 5: Request independent review**

Review scope:

- archive safety and branch topology;
- absence of V4 and versioned implementation paths;
- crate dependency direction;
- Chumsky stable API usage and no hidden raw-text parser;
- diagnostic code/span determinism;
- generator no-partial-output boundary;
- conformance manifest integrity;
- roadmap/matrix/review consistency.

Critical or Important findings must be fixed and re-reviewed before marking I0 complete.

- [ ] **Step 6: Confirm clean final state**

```powershell
git status --short --branch
```

Expected: `## master` with no modified, deleted, or untracked files.

## Completion handoff

The I0 handoff must report:

- archive SHA and current master SHA;
- commits created by Tasks 1–12;
- exact workspace package list;
- exact dependency tree summary;
- exact Clippy and nextest outcomes and nextest pass count;
- matrix rows still `partial` or blocked and their assigned stages;
- confirmation that no Frozen specification version changed;
- confirmation that no remote/default-branch hosting setting was changed because no remote existed.

Do not begin I1 in the same implementation task. I1 starts only after I0 review is accepted.
