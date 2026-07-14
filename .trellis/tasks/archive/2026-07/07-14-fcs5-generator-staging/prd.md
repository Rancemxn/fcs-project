# FCS 5 generator staging boundary

## Goal

Make the existing FCS 5 generator parser and elaborator obey the Frozen I0 staging boundary before
the later source-crate cutover: accept only the two specified range operators, parse generator
syntax without expanding it, and return one explicit implementation diagnostic before any generated
output can escape.

## Background and confirmed facts

- The active branch is `master`; I0-A snapshot/archival work is complete and the active source tree
  is still `crates/fcs-core/src/v5`.
- The authoritative generator rules are in `fcs.md` section 6 and the implementation boundary is
  recorded in `docs/decisions/0006-unversioned-source-cutover.md` and
  `docs/plans/i0-source-cutover.md` Task 2.
- `crates/fcs-core/src/v5/parser/entities.rs::parse_generator` currently accepts `..=` and also
  accepts bare `..` while optionally consuming `<`; this makes the undocumented bare operator look
  like the half-open form.
- The same parser currently rejects literal zero `int`/`Beat` steps through `is_literal_zero`.
  Zero-step validity is static generator semantics owned by I2, not source parsing in this task.
- `crates/fcs-core/src/v5/elaborator/mod.rs::Diagnostic` has no feature-unavailable variant, and
  `elaborator/entities.rs::ExpansionContext::expand_item` has no generator arm. The current
  non-exhaustive staging state must be closed explicitly.
- `crates/fcs-core/tests/fcs5_phase2.rs` currently has one generator test using bare `..` and
  expecting it to parse.

## Requirements

### R1. Enforce Frozen range syntax

`parse_generator` must accept exactly:

- `..<` with `inclusive_end == false`;
- `..=` with `inclusive_end == true`.

Bare `..` must return `ParseError::InvalidSyntax("generator range")`. The parser must not silently
reinterpret or normalize an undocumented compatibility spelling.

### R2. Keep zero-step validation out of parsing

Remove the parser-local `is_literal_zero` check. A syntactically valid zero step must remain in the
generator AST so I2 can report the Frozen `compile-time.zero-step` semantic error when expansion is
implemented. This task does not evaluate or expand ranges.

### R3. Make I0 non-expansion explicit

Add `Diagnostic::FeatureUnavailable { feature: &'static str, span: SourceSpan }` and make collection
elaboration return this diagnostic when it encounters a `CollectionItem::Generator`. The feature
string must be exactly `"compile-time-generator"`.

The elaborator must return the error before returning an `ExpandedSourceDocument`; no partial
collection output may be observable through the public `Result`.

### R4. Test the boundary first

Tests must cover both valid operators, bare-operator rejection, zero-step parse retention, and a
generator encountered after an ordinary constructor so the explicit no-partial-expansion boundary
is exercised.

### R5. Preserve scope boundaries

Do not rename `fcs-core`, delete FCS 4 crates, add Chumsky, replace the hand-written lexer, redesign
the public diagnostic API, implement generator expansion, or validate generator direction/counting.
Those belong to later I0/I2 tasks.

## Acceptance criteria

- [x] Frozen `..<` and `..=` parser tests pass with the expected inclusive flag.
- [x] A bare `..` generator fails with `ParseError::InvalidSyntax("generator range")`.
- [x] A literal zero step parses into the generator AST rather than failing in the parser.
- [x] A parsed generator causes elaboration to return
      `Diagnostic::FeatureUnavailable { feature: "compile-time-generator", .. }`.
- [x] The feature diagnostic span starts at the `generate` keyword, and no successful expanded
      document is returned for a collection containing a generator.
- [x] The prior non-exhaustive `CollectionItem` handling is eliminated.
- [x] Targeted generator tests, workspace Clippy, workspace nextest, rustfmt check, and
      `git diff --check` pass.
- [x] No out-of-scope crate, lexer, diagnostic-API, or generator-expansion changes are present.

## Execution evidence

- Red baseline: `cargo nextest run -p fcs-core --test fcs5_phase2` initially failed on the existing
  non-exhaustive `CollectionItem::Generator` match.
- Targeted result: `fcs5_phase2` 58/58 passed after the boundary implementation.
- Clippy result: `cargo clippy --workspace --all-targets -- -D warnings` passed.
- Workspace result: `cargo nextest run --workspace` passed with 227/227 tests.
- Formatting/result checks: `cargo fmt --all -- --check` and `git diff --check` passed.
- Source commit: `eef7fbf` (`fix(source): make generator staging explicit`).
- Changed source files: parser generator range handling, elaborator diagnostic/match arm, and the
  existing Phase 2 generator tests only.

## Technical notes

The implementation is intentionally minimal: strict operator matching and removal of one parser
semantic check, one explicit elaborator error variant, one generator match arm, and focused tests.
The later stable diagnostic migration will map this temporary variant to the public
`implementation.feature-unavailable` code; this task retains the current Phase 2 enum API while
making the staging behavior unambiguous.
