# FCS 5 source deterministic and fuzz lane

This document records the I1.8c fuzz-runner decision and the reproducible smoke
contract. The lane is parser-boundary evidence only; it does not create source,
static, canonical, runtime, FCBC, Render, or Conversion semantics.

## Runner audit

| Component | Pin/evidence | Decision |
|---|---|---|
| `cargo-fuzz` | 0.13.2; crates.io package; MIT OR Apache-2.0; source requires Unix-like x86-64/AArch64, LLVM sanitizer support, C++11, and a nightly compiler | selected as the orchestration CLI, installed as a developer tool rather than a workspace dependency |
| `libfuzzer-sys` | 0.4.13; `fuzz/Cargo.toml` exact pin; MIT OR Apache-2.0 AND NCSA; depends on `arbitrary` 1 and build-depends on `cc` 1.0.83 | selected as the fuzz engine binding, isolated to the independent fuzz workspace |
| FCS normal workspace | `cargo tree -e dev -p fcs-source` contains `proptest` 1.11.0 but no libFuzzer crate | unchanged; fuzz tooling is not a normal/runtime dependency |

`cargo-fuzz` 0.13.2 declares no `rust-version`; its current source checks
nightly/stable sanitizer support. The repository's stable toolchain is retained
for normal gates; a nightly toolchain is required only for the unbounded local
libFuzzer lane. `fuzz/Cargo.lock` is the dependency-tree artifact for the
isolated workspace.

The audited `cargo-fuzz` 0.13.2 runtime dependency roots are
`anyhow` 1.0.102, `cargo_metadata` 0.23.1, `clap` 4.6.1,
`current_platform` 0.2.0, `num_cpus` 1.17.0, `rayon` 1.12.0,
`rustc_version` 0.4.1, `tempfile` 3.27.0, and `toml` 1.1.2. Its dev-only
roots are not activated by the installed CLI. These roots were checked with
`cargo info cargo-fuzz --verbose` and `cargo tree --locked` against the
unpacked 0.13.2 package source.
The fuzz workspace uses `libfuzzer-sys`'s default `link_libfuzzer` feature and
does not enable `arbitrary-derive`; its locked tree is independently recorded
in `fuzz/Cargo.lock`.

## Targets and invariants

- `document_bytes`: every byte input goes through `parse_document_bytes`; all diagnostic spans remain within the byte input, and invalid output never exposes a partial AST.
- `document_utf8`: valid UTF-8 inputs go through `parse_document`; spans remain UTF-8 character boundaries, and invalid output never exposes a partial AST.
- `expression`: valid UTF-8 inputs go through `parse_expression` with the same span and no-partial-output invariants.

The deterministic property lane in `crates/fcs-source/tests/robustness.rs`
uses a fixed ChaCha seed and bounded cases. It covers arbitrary bytes/UTF-8,
nested delimiters/comments, parser limits, expressions, and complete source
fixtures; the fuzz targets provide an independent libFuzzer execution path.

## Corpus and commands

`fuzz/corpus/README.md` and `scripts/fcs5-fuzz-smoke.sh` materialize one seed
for each of the 39 manifest entries plus the three public FCS examples into a
temporary corpus. The same 42 seeds are passed to all three targets, so the
byte, UTF-8, and expression entry points share the complete source corpus.

Bounded smoke (the delivery command):

```text
FCS_FUZZ_RUNS=32 scripts/fcs5-fuzz-smoke.sh bounded
```

This passes `-runs=32` to each libFuzzer target with `max_len=65536`; the 42
seed files are loaded before that bounded run budget. It does not write
generated corpus or artifacts into the repository. Local exploration
uses:

```text
scripts/fcs5-fuzz-smoke.sh unbounded
```

The unbounded command is intentionally not a normal workspace test and is not
required for CI.
