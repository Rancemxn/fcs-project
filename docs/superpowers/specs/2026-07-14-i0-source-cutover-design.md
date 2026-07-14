# I0.3 Unique Source Crate Cutover Design

## Goal

Make `crates/fcs-source` the only active FCS source implementation on `master`, while
preserving the complete pre-cutover FCS 4 workspace exclusively on
`archive/fcs4-pre-cutover`.

## Scope

This batch performs the structural I0.3 cutover and nothing beyond it:

- retain the candidate FCS 5 source AST, parser, schema, version, and elaborator;
- promote `crates/fcs-core/src/v5` to unversioned `crates/fcs-source/src` paths;
- remove active FCS 4 core, CLI, converter, bytecode, compiler, and VM code;
- make the root workspace contain exactly `fcs-source`;
- preserve PGR/RPE/PEC and copyright inputs as future converter fixtures;
- make the structural test and the migrated source tests target `fcs-source`;
- update module paths without adding compatibility re-exports.

I0.4 stable diagnostics, I0.5 Chumsky migration, manifest execution, complete grammar,
canonical model, runtime, FCBC, conversion, rendering, and CLI reconstruction are out of
scope for this batch.

## Architecture

`fcs-source` owns only FCS source concerns: source AST, parsing, source version constants,
construction schema, static checks, and compile-time elaboration. It must not expose the
legacy FCS 4 AST or retain a `v5` module. Future canonical/runtime/FCBC/converter/render
crates will consume a later canonical boundary rather than source AST.

The retained candidate `Color` type moves with the source AST. Its implementation must not
remain under the legacy `units` module. The crate root exposes `ast`, `elaborator`, `parser`,
`schema`, and `version`; validation remains an internal module.

## Test-first workflow

The structural test is the red test for the batch. Before the cutover it must fail because
the package is still `fcs-core` and the legacy paths exist. After the cutover it must pass,
along with the migrated FCS 5 frontend and compile-time tests.

The quality gate is ordered as required by the repository:

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

The final structural search must find no active `v4`/`v5` directory, `crate::v5`,
`fcs_core`, `fcs_source::v5`, or `#fcs v4` reference in the active source tree.

## Non-goals and safety constraints

- Do not modify `fcs.md`, `fcbc.md`, `fcs-render.md`, or `fcs-conversion.md`.
- Do not modify `refer/` or the archive branch.
- Do not add an interim compatibility facade.
- Do not claim conformance for fixtures whose stage requires I1–I9 functionality.
- Do not begin I0.4 or I1 in the same implementation batch.
