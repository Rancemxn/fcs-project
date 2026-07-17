# I2 Static Semantics Reviewed Implementation Baseline

Status: PASS candidate for implementation baseline

This record fixes the I2 dependency closure and review snapshot. It is an implementation permission for I2
only; it does not promote FCS Core 5.0.0 or any other specification domain from Draft to Reviewed/Frozen and
does not authorize release.

## Fixed repository snapshot

- Starting implementation commit: `0681ddd8561ef3072e1175b13e2ab3b214aab364`.
- Active workspace: one unversioned package, `crates/fcs-source`.
- Precondition: I1.9 PR #41 is merged; I1 baseline review is PASS with zero open Critical/Important findings.
- Review method: fixed-snapshot, read-only replay of the closure, plan, fixtures, manifest, public seams and
  existing tests. No implementation result is treated as normative authority.

## Normative input hash ledger

SHA-256 is over raw file bytes:

| Input | SHA-256 |
| --- | --- |
| `fcs.md` | `2a2882e60aeef4d96fdb9f7c3cd65b143ea9d9c61f971ece24a8b2791627dc58` |
| `docs/specifications/governance.md` | `8956fca502df543291eb275da3295b28339c5654750f02ccf0731681fbde72f5` |
| `docs/decisions/0003-compile-time-structure-only.md` | `15c24f7d0a220f6acaab3418fe9c2c4b02da380afeb86dddf677e9352a28c1e3` |
| `docs/decisions/0008-fcs-authoring-fcbc-distribution-boundary.md` | `853b5cc765917a8a103b8027dd66541d1dd1d4c1aa64c4be617be2917ba72e9b` |
| `docs/decisions/0009-player-local-baking-shared-runtime.md` | `8da3f5e0d0170c2dc181ada12d798b7e827ec66935c1ec42b948e33eca507487` |
| `docs/decisions/0010-stage-scoped-implementation-baselines.md` | `bd7a0086f5e9b95a4b99fe98eccc2882ff8e3a214f6ec6753010f5d27cd81ad9` |
| `docs/plans/fcs5-roadmap.md` | `04a5d5513e1ec9a972f51a8e800cf813e1e1e36b68268844d9dacb42cfdb113b` |
| `docs/plans/i1-source-ast-parser.md` | `c4194edeabb5066efce1f7884b12a113f119a9b26295cf44da89905d3e93a37f` |
| `docs/conformance/manifest.toml` | `231f4505de29c854201057f97706295756109a98dcc7ac99f08ca21cd3f96fe8` |
| `docs/conformance/fcs5/manifest.toml` | `4d8dfb7ba2d636cd94a02f39807c7a0da6185b73213f50dd77e8f2a69c5b25f4` |

## Bound clauses and fixtures

The review covers `fcs.md` 1.1–1.4, 3.1–3.4, 4.1–4.5, 5.3–5.4, 6.1–6.8, and the construction-schema
boundary in 12.1–12.3. The following fixture/expected bytes are bound to the baseline:

| Path | SHA-256 |
| --- | --- |
| `source/valid/compile-time-generator.fcs` | `29c52da21be17e1f15c5cc5197cd3a5adbbfba5dcf9b932b57f655d067aaa002` |
| `source/valid/template-if-with.fcs` | `f0fe267cab88b291d3f98e6da4ed0f5c40a2e7108a5af568d0cb32ed015b3830` |
| `source/valid/int-range-descending.fcs` | `d39f291198912ce929b7eaa2b1c5d4edc31d9e411927dfafdc28ebef960b4aee` |
| `source/invalid/unresolved-schema-enum.fcs` | `66bda837b590dfb3eceec6ac9d81d79b575757a2a4ac48f87a41b4a7feb97333` |
| `source/invalid/generator-zero-step.fcs` | `64be919de19ad7355326b4bc3200db121934dba01ea07b0814852e7e03a7a8e8` |
| `source/invalid/shadowing.fcs` | `bec4b1cf82581b37e73dfd9d9faf15e7fe165bdbd11daae648b291e055aed7ad` |
| `source/invalid/template-missing-line.fcs` | `0ab572d4d0df7f412b5480a9c1b4ff2fca3cddb094ccd7bd72498eeecf06201b` |
| `source/invalid/runtime-gameplay.fcs` | `95513fa8133cd172c0c0b8ba8a69b5cd264a8500273b5c8dc2f056bcf3473829` |
| `expected/compile-time-generator.json` | `590b13511df40c091412b25afc07862cd867e3297132616090601d9bd61713c8` |
| `expected/template-if-with.json` | `80f4e670e77d0fbe1ba596a6cd41e4d9b756ef463214cdac88fe4c321b986188` |
| `expected/int-range-descending.json` | `8adb4097d2ed6477ce32702397d8cf11b9b59a7a92f8d97a2d0fffa562e10343` |

The manifest's `source.invalid.generator-budget` reuses the compile-time-generator source with
`maxGeneratorIterations = 2`, expected `compile-time.budget-exceeded`, and trace fragments
`collection=notes`, `index=2`, `emit=Note`.

## Closure review

### Type and expression closure

The bound clauses define all primitive/entity types, generic restrictions, unit exactness, explicit
conversion, operator result matrix, builtin signatures, finite/domain behavior, field availability, and the
compile-time/runtime boundary. The current AST preserves source expressions and type syntax; I2 owns the
missing validation/evaluation rather than changing parser categories.

### Scope and graph closure

The bound clauses uniquely require complete definition collection, forward references, no same-scope or
ancestor shadowing, sibling branch locality, and acyclic const/function/template dependencies. Existing
scope/cycle modules are the implementation seam; deterministic spans and shortest cycle traces are acceptance
evidence.

### Function/template/constructor closure

Function bodies are pure, typed, and total over reachable paths. Templates return one declared entity type;
all branches are checked, only the selected branch is instantiated, required fields are complete before
`with`, and schema field paths/closed values are explicit. Existing Note/Line construction schema is a
partial seam and must not be mistaken for full canonical schema.

### Generator and budget closure

Only `int` and exact rational `beat` ranges are permitted; `..<`/`..=` semantics, sign, emptiness, inclusive
reachability, overflow, and zero-step diagnostics are explicit. A single context owns all six budgets and
increments before work/allocation; cycle checks precede depth accounting. Trace and budget detail are already
publicly shaped in `diagnostic.rs` and require complete I2 population.

### Expanded-output and phase closure

All compile-time structures disappear before canonical lowering. `ExpandedSourceDocument` may retain spans,
traces and provenance, but output entities must be concrete and read-only. Runtime property expressions remain
exact later-stage inputs. This separates I2 from I3 canonical lowering and I4 evaluation, and prevents source
AST reuse as a canonical model.

## Diagnostic ownership

I2 owns `name.unknown`, `name.duplicate`, `name.shadowed`, `name.cycle`, `type.mismatch`,
`type.invalid-operation`, `type.invalid-conversion`, `schema.unknown-field`, `schema.duplicate-field`,
`schema.missing-required-field`, `schema.non-constructible`, `schema.collection-type-mismatch`,
`schema.dynamic-field-forbidden`, `compile-time.non-constant-condition`, `compile-time.invalid-range`,
`compile-time.zero-step`, `compile-time.budget-exceeded`, `numeric.non-finite`,
`numeric.divide-by-zero`, `numeric.domain`, and `numeric.overflow`, subject to the exact clause/fixture
binding. Parser categories remain I1-owned; canonical/runtime/resource/track/graph categories remain later
stage-owned unless the closure explicitly assigns a static construction check.

## Public API and implementation risks

1. `ExpandedSourceDocument` can accidentally become a second canonical model. Keep construction private,
   document the invariant, and expose immutable traversal only.
2. Independent counters can accept a program that exceeds the shared 6.8 budget. Thread one context through
   constants, calls, templates, `with`, branches, ranges and emits.
3. Schema enum inference can turn an unresolved identifier into a string and violate 2.10/6.1. Resolve names
   first; only an explicit string value or typed const may satisfy a string field.
4. Runtime `choose`/environment access can leak into structural decisions. Reject it in I2 with the bound
   diagnostic and leave exact runtime descriptors for I3/I4.
5. Float accumulation can make range count/endpoint platform-dependent. Use checked integer/rational
   `start + index * step`.
6. Reusing source AST in later crates would cross ADR 0008/0009 boundaries. I2 output is an input seam, not
   permission for future crates to consume source nodes.

## Verification replay

The fixed snapshot was checked with:

```text
git status --short --branch
git rev-parse HEAD
cargo metadata --no-deps --format-version 1
git diff --check
sha256sum <all files in the two hash tables above>
```

The pre-existing I1 quality gate is accepted as a prerequisite: fmt, Clippy, workspace nextest, dependency
tree, fixture/hash audit, and I1 independent review are recorded in
`docs/reviews/2026-07-16-i1-source-parser-baseline-review.md`. This plan-only change does not rerun Rust
tests; that omission is intentional and follows `AGENTS.md`.

## Finding ledger and disposition

| Severity | Finding | Disposition |
| --- | --- | --- |
| Critical | None in the fixed I2 dependency closure | Closed; no normative change required |
| Important | None in the fixed I2 dependency closure | Closed; no normative change required |
| Minor | Existing I0 elaborator is incomplete and generator returns `implementation.feature-unavailable` | Accepted as the explicit I2 implementation frontier; tracked by #42 and I2.1–I2.10 |

The Minor residual does not change the I2 public contract and is the reason this baseline exists. It must not
be silently reclassified as a semantic choice. Any future finding that changes a bound clause, fixture,
diagnostic category, schema boundary, or expanded-output invariant reopens this baseline under ADR 0010.

## Review conclusion

The I2 plan matches the bound clauses, fixtures, diagnostic ownership, public seams, and later-stage boundaries;
the I1 prerequisite gate is present; and no Critical/Important finding remains open. Subject to recording the
review at the committed snapshot and delivering the plan-only PR, I2.1 may begin automatically after merge.
