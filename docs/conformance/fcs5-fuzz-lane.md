# FCS 5 source deterministic and fuzz lane

This document records the I1.8c fuzz-runner decision and the reproducible smoke
contract, extended by #338 to every hostile-input surface. The lane is
robustness-boundary evidence only; it does not create source, static,
canonical, runtime, FCBC, Render, or Conversion semantics.

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
- `fcbc_container`: every byte input goes through `fcs_fcbc::load_container` and `fcs_fcbc::load_chart`; the loader must reject or accept without panic, abort, native-stack exhaustion, or hang. This is the surface where #313 and #329 lived.
- `render_section`: every byte input goes through `fcs_render::load_render`, reaching the RenderSection/resource record decoding and graph checks behind the Core load.
- `asset_image`: a selector byte plus payload goes through `fcs_render::decode_image` across every legal media-type/color-space/alpha combination.
- `asset_font`: every byte input goes through `fcs_render::decode_font`, and valid UTF-8 inputs are additionally shaped against the project-built font via `shape_simple_ltr` (the checksum gate makes arbitrary-byte font decoding shallow; hostile text is the reachable surface).
- `import_pgr` / `import_rpe`: byte inputs go through `SourceArtifact` → `parse_json_document` → `parse_pgr_document`/`parse_rpe_document` under default limits.
- `import_pec`: byte inputs go through `SourceArtifact` → `parse_pec_document` under default limits.

The non-source targets assert freedom from panic, abort, stack exhaustion,
out-of-memory, and hangs only; category-set and semantic conformance remain
owned by the mutation corpora and conformance manifests, not by this lane.

The deterministic property lane in `crates/fcs-source/tests/robustness.rs`
uses a fixed ChaCha seed and bounded cases. It covers arbitrary bytes/UTF-8,
nested delimiters/comments, parser limits, expressions, and complete source
fixtures; the fuzz targets provide an independent libFuzzer execution path.

## Corpus and commands

`scripts/fcs5-fuzz-seeds.py` materializes per-target seed corpora into a
temporary directory tree; `fuzz/corpus/README.md` records the seed source for
each target. Seeds come only from checked-in evidence — the FCS manifest
fixtures and examples for the source targets, every manifest-declared Core and
Render binary golden for `fcbc_container`/`render_section`, and the public
conversion fixture sources for the importer targets. Asset targets start with
the fixed PNG/WebP/font and declared shaping input. The smoke lane first runs
`scripts/test-fcs5-fuzz-seeds.py` to check the Render seed, media selectors,
font/text inputs, and preservation of cached corpus files.

Bounded smoke (the delivery command):

```text
FCS_FUZZ_RUNS=1024 scripts/fcs5-fuzz-smoke.sh bounded
```

This passes `-runs=1024` to each libFuzzer target with `max_len=65536`; each
target's seed set is loaded before that bounded run budget. It does not write
generated corpus or artifacts into the repository. Local exploration uses:

```text
scripts/fcs5-fuzz-smoke.sh unbounded
```

The unbounded command is intentionally not a normal workspace test and is not
required for CI.

Deep fuzzing runs weekly in `.github/workflows/weekly-fuzz.yml`: a per-target
matrix, 30 minutes of coverage-guided fuzzing each on the stable toolchain
with `--sanitizer none`, the corpus grown across runs via the Actions cache,
and crash inputs uploaded as run artifacts on failure. The weekly workflow is
separate from the ADR 0013 full gate and is not merge evidence; a crash it
finds is triaged as a review finding against the owning surface.

## Known residual: brace-only parser recovery cost

Issue #520 reported that `document_bytes` and `document_utf8` timed out on `main` for five
consecutive weekly runs. The root cause was the document-level recovery in
`crates/fcs-source/src/parser/document.rs`: `nested_delimiters` matches only a complete balanced
group, so an unclosed `{` or `[` degraded `skip_then_retry_until` into skipping one token and
re-attempting the whole `top_level_item_parser()` alternation, which is polynomial in the token
count after the first unclosed delimiter. Issue #523, merged as `b518eab`, made
`delimiter_balance` report the innermost unclosed delimiter that is not a brace whenever one
exists, so those inputs are now rejected at the lexical gate. The brace-only exemption that
document-level recovery depends on is preserved verbatim, and
`robustness::an_unfinished_block_still_reaches_document_level_recovery` pins it.

A document whose **only** imbalance is an unclosed brace still reaches the per-token retry loop.
A `Weekly fuzz` dispatch on `b518eab`, run 32552513637, succeeded on all ten targets, so 30
minutes of coverage-guided search per parser target did not reach that class. That is negative
evidence, not a bound. Issue #520 remains open while this brace-only residual is unbounded: if a
future weekly run goes red, triage whether the input is brace-only, and if it is, choose between
resynchronizing recovery on top-level boundaries without disturbing `misplaced_item_parser`,
`unknown_item_parser`, or `trailing_item_parser`, and publishing a parse-work limit, which would
need a specification amendment. The 10-second assertion in
`robustness::unclosed_brackets_are_reported_even_when_a_brace_is_innermost` guards the fixed route,
and is a guard rather than a performance budget.
