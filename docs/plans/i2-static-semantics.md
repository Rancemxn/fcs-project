# I2 Static Semantics and Compile-time Expansion

Status: I2 implementation and public-conformance evidence corrected through PR #102; primary delivery gates
pass, while the superseding independent re-audit residual is recorded in
`docs/reviews/2026-07-17-i2-static-semantics-implementation-closure-review.md`.

This plan is the bounded execution contract for I2. It schedules implementation work; it does not
create FCS semantics, promote a specification version, or authorize work owned by I3 and later stages.
The current branch starts from the merged I1.9 checkpoint `0681ddd8561ef3072e1175b13e2ab3b214aab364`.

## Current-state closure handoff

`origin/main` is `2d24494354b4b10fe8dbd30cdadf8d1f5d22f4c8`, including corrective PRs #92 and #96 and
PRs #97, #98, and #102 for Issues #84, #87, #88, #91, #93, and #99. Primary Self-Audit and the applicable
local Rust gates passed for each merge. Independent Audit results are `pass` for the fixed merged SHAs of
PRs #92, #96, and #102; PR #97 requires re-review after the former #99-blocked result, and PR #98 still awaits its
merged-SHA audit. This asynchronous residual does not block the primary session's handoff to I3.1, but it
keeps the corrected I2 stage claim provisional and must freeze I2-dependent work if a later Critical or
Important finding appears.

I3.1 Canonical IDs is the next bounded frontier. This handoff does not implement I3 or promote any
specification version.

## Objective

Implement the FCS 5 static-semantics and compile-time-elaboration boundary described by `fcs.md` 3.1–3.4,
4.1–4.5, 5.3–5.4, and 6.1–6.8. A successful I2 elaboration returns an `ExpandedSourceDocument` whose
collection output is concrete, typed, deterministic, and free of compile-time structure. Runtime property
expressions remain source/canonical inputs for I3/I4 and are not evaluated or sampled by I2.

I2 begins only after the I2 Reviewed Implementation Baseline review records the exact dependency closure,
fixture/expected hashes, diagnostic ownership, and zero open Critical/Important findings. The baseline is a
stage gate, not a FCS Core or ABI version-state transition.

## Normative dependency closure

The implementation and tests must read these inputs as a single closure:

- `fcs.md` 1.1–1.4 for phase ownership and deterministic front-end boundaries;
- `fcs.md` 3.1–3.4 for type families, units, defaults, and explicit conversions;
- `fcs.md` 4.1–4.5 for operators, builtins, field access, and runtime-value exclusion;
- `fcs.md` 5.3–5.4 for definitions, collection ownership, and Track-local collection shape;
- `fcs.md` 6.1–6.8 for bindings, pure functions, templates, constructors, `with`, compile-time `if`,
  generators, `emit`, and the six shared budgets;
- `fcs.md` 12.1–12.3 only for the construction-schema boundary needed to type and validate static
  constructors; canonical Note identity and presentation/runtime behavior remain later-stage work;
- `docs/specifications/governance.md` and ADR 0003, 0008, 0009, 0010;
- `docs/plans/fcs5-roadmap.md`, `docs/plans/i1-source-ast-parser.md`, and the merged I1 baseline review;
- `docs/conformance/manifest.toml`, `docs/conformance/fcs5/manifest.toml`, the I2 fixture sources, and their expected
  outputs;
- the current `crates/fcs-source` AST, schema, elaborator, diagnostic API, callers, and tests.

No implementation decision in this plan may override the specification, an Accepted ADR, or a bound fixture.
If a conflict is found, stop the affected implementation slice and route a specification/fixture/review
change through governance before changing Rust behavior.

## Explicit non-goals and ownership boundaries

I2 does not create or activate `fcs-model`, `fcs-runtime`, `fcs-fcbc`, `fcs-converter`, `fcs-render`, or a
CLI crate. It does not implement or decide:

- canonical IDs, tempo normalization, Track interval semantics, parent graphs, Note lowering, resource
  resolution, or any other I3 canonical rule;
- runtime expression DAG evaluation, easing, distance, binary64 reference evaluation, or `choose` runtime
  availability (I4);
- metadata/resource/fidelity/provenance bundles or repair/report behavior (I5);
- PGR/RPE/PEC profile selection or conversion (I6/I8);
- FCBC/container/Execution ABI bytes or loader validation (I7);
- Render semantic/raster/resource behavior (I9);
- CLI, package/distribution metadata, public release, tag, crate publication, or conformance bundle (I10).

I2 may preserve spans, expansion traces, and authoring provenance for diagnostics. It must not leak a source
template, generator, local binding, unselected branch, or runtime dependency into the canonical-facing
expanded output. A runtime property expression is not compile-time structure and must remain representable.

## Current module map and seams

The active workspace has one package, `crates/fcs-source`:

| Layer | Current responsibility | I2 seam |
| --- | --- | --- |
| `ast/*` | Spanned source document, definitions, entities, generator nodes, typed values and expanded entities | Add only the typed/static output needed by this stage; source AST remains distinct from future canonical model |
| `schema.rs` | Immutable construction schema and collection registry | Extend schema data/validation only where `fcs.md` 5.4/6.3/12.1–12.3 require it |
| `elaborator/mod.rs` | Public `elaborate` entry point, limits and diagnostic mapping | Own one shared context and stable public read-only result/error API |
| `elaborator/scope.rs` | Immutable lexical frames | Enforce no ancestor shadowing, sibling-branch locality, typed bindings, and forward-reference policy |
| `elaborator/cycle.rs` | Existing declaration-cycle checks | Generalize to a deterministic const/function/template dependency graph before evaluation |
| `elaborator/eval.rs` | Existing pure expression checking/evaluation and builtins | Complete the type matrix, finite/domain checks, conversions, short circuit, and shared budget accounting |
| `elaborator/entities.rs` | Existing Note/Line constructor/template/`with` expansion | Add typed branch checking, complete schema constraints, generator expansion and concrete-output invariants |
| `diagnostic.rs` | Stable dotted categories, spans, traces and budget detail | Keep categories stable; add fields only when required by the bound 6.8 contract |
| `tests/compile_time.rs` and conformance tests | Existing red/green evidence and pre-I2 boundary tests | Convert feature-unavailable generator paths into I2 behavior and add fixture/property/limit coverage |

The public API risk is concentrated at `elaborate`, `CompileTimeLimits`, `ExpandedSourceDocument`, typed
expressions/values, schema registration, and diagnostics. Any public signature or semantic change must be
reviewed as an API change and included in the delivery gate; no compatibility facade or second AST is allowed.

## Bounded task ledger

Each task is one vertical, independently testable work unit. The implementation order is fixed unless a
dependency is proven and recorded in a new progress message.

### I2.1 Type matrix

- Owns: `fcs.md` 3.1–3.4 and 4.1–4.5.
- Add a declarative operator/conversion/builtin matrix test so every supported pair is enumerated and every
  omitted pair has a stable `type.invalid-operation` or `type.invalid-conversion` result.
- Cover generic type restrictions (`vec2<T>`, `array<T>`), exact unit separation, rational Beat values,
  explicit conversions, finite/domain/overflow/divide-by-zero errors, comparison chains, short-circuiting,
  array length/index, field access and runtime-environment rejection.
- Red tests precede implementation; focused expression tests are the feedback loop.

### I2.2 Scope and name resolution

- Owns: `fcs.md` 6.1 and the name/type portion of 6.3–6.7.
- Collect all definitions before resolving references, allowing forward references while retaining source
  spans and deterministic declaration order.
- Use lexical frames for parameters, locals, branch scopes, and generator variables; reject same-scope and
  ancestor shadowing, allow sibling branch reuse, and reserve builtins/user names without overloads.
- Add valid/invalid tests for unresolved names, duplicate bindings, ancestor shadowing, sibling names and
  generator-only bindings.

### I2.3 Dependency graph

- Owns: the no-cycle requirements in `fcs.md` 6.1, 6.2, and 6.3.
- Build separate const/function/template nodes with legal cross-kind edges; reject cycles before depth or
  evaluation work and report the shortest deterministic cycle with complete related spans.
- Add declaration-order permutation and const/function/template cycle tests.

### I2.4 Function elaboration

- Owns: `fcs.md` 6.2 plus function-call portions of 4.3.
- Validate typed parameters, exact pure-value return type, every reachable return path, arity, purity, and
  operation/expression budgets. Reject entity returns and template/emit/generate calls from functions.
- Add tests for valid functions, missing returns, wrong arity/types, forbidden calls, domain errors and
  deterministic call traces.

### I2.5 Template statements and constructors

- Owns: `fcs.md` 6.3–6.5 and construction portions of 12.1–12.3.
- Validate typed template parameters and entity return type, all branches, constructor field paths/types,
  required/default fields, closed schema values, immutable `with`, composition and recursion traces.
- The unselected branch is statically checked but only the selected branch is evaluated/instantiated.
- Add the `template-if-with` fixture and focused tests for missing fields, unresolved schema enum, field
  duplication, composition, recursive templates and runtime gameplay dependencies.

### I2.6 Generator range

- Owns: `fcs.md` 6.6.
- Evaluate only compile-time `int`/`beat` ranges with equal endpoint/step types, exact rational/integer
  arithmetic, positive/negative steps, empty ranges, inclusive reachability and checked overflow.
- Compute each value as `start + index * step`; never use repeated floating-point addition. Expose `index`,
  `range.start/end/step/count` in a generator frame and reject zero step before iteration.
- Add ascending/descending, empty, inclusive, zero-step, overflow and range-count boundary tests.

### I2.7 Generator body and emit

- Owns: `fcs.md` 6.6–6.7.
- Type-check local `let`, compile-time `if`, and `emit`; reject nested generators, runtime dependencies and
  wrong collection entity types. Preserve collection and iteration source order.
- Add the `compile-time-generator` fixture and tests proving no partial output on any failure.

### I2.8 Unified budget

- Owns: `fcs.md` 6.8 and ADR 0003 resource-bound consequences.
- Replace independent nested counters with one elaboration context carrying the six budgets:
  `maxExpansionDepth`, `maxGeneratedNodes`, `maxGeneratorIterations`, `maxTemplateInstances`,
  `maxCompileTimeOperations`, and `maxExpressionNodes`.
- Increment before work/allocation; cycle checks precede depth accounting; nested calls reuse the same context.
  Budget diagnostics contain kind, limit, observed count, primary span, and ordered expansion frames for
  function/template/collection/range/generator/index/emit as applicable.
- Run the manifest `generator-budget` limit case and per-budget boundary tests.

### I2.9 Expanded output invariant

- Owns: the compile-time-elimination rule in `fcs.md` 6.8 and the I3 input boundary.
- Make construction of `ExpandedSourceDocument` private or otherwise invariant-preserving, expose only
  read-only traversal, and assert every output entity is concrete and typed. No source compile-time node,
  local name, unselected branch, generator declaration or runtime environment read may survive.
- Add recursive invariant traversal and source declaration reorder tests; preserve runtime property
  expressions as explicit later-stage values.

### I2.10 Public conformance fixture and delivery gate

- Owns: executable S2/S3 evidence for 6.1–6.8 and construction clauses.
- Execute `compile-time-generator`, `template-if-with`, and `int-range-descending`; assert expected output
  shape, source order, diagnostics and budget traces. Execute the six elaborate-error fixtures and ensure
  parse-stage categories are not used for static errors.
- Update the I2 implementation matrix and evidence only after all task gates pass; create the next child
  Issue for the earliest I3 baseline only after I2 is independently reviewed and merged.

## I2 delivery checkpoint

The I2.1–I2.9 implementation work units are merged on `main` through PR #65. The I2.10 delivery unit
executes the three valid public elaboration fixtures and the six bound elaborate-error/budget fixtures
through the public `fcs_source::elaborator::elaborate` API. Its executable evidence must assert the expected
expanded shape, deterministic source order, exact Beat/Length values, selected template/`with` behavior,
concrete-output invariants, stable static diagnostic categories, and the shared generator-budget trace.
The expected JSON and manifest entries remain fixture authority; they do not become a second semantic model.

I2 remains a stage-scoped implementation baseline, not a FCS Core version-state transition. Runtime
expression/DAG lowering, canonical model construction, and all later product crates remain owned by I3+.
After the I2.10 PR passes its full Rust gate and independent fixed-SHA audit, the owning matrix and roadmap
may advance the frontier to the earliest I3 baseline Issue.

## Historical I2 implementation closure snapshot

The following closure text records the pre-correction snapshot and is retained for audit history. Current
state is defined by the Current-state closure handoff above.

The I2.10 delivery and its corrective review chain are now complete. PR #72 delivered the public fixture
lane and merged at `117e23f906b8a1d224e8cb09adc95d2f0894931d`; reviewer re-review of the I2.1 correction
recorded Important finding #81 at merge `c762c630b5b9bb09418f9d543200ad4daab7ec84`; PR #82 corrected that
path and merged at `3a484e5a8205f7af8c4a23776111e7a1d80dcf62`. The independent Audit result for PR #82/Issue
#81 is `pass`, and Issues #75–#77/#81 are closed.

The final delivery evidence is `cargo fmt --all -- --check`, workspace Clippy, workspace nextest `286/286`,
the compile-time focused lane `138/138`, and `git diff --check`. The full fixed scope and hashes are recorded
in `docs/reviews/2026-07-17-i2-static-semantics-implementation-closure-review.md`.

This closes the I2 Reviewed Implementation Baseline only. The five specification domains remain Draft;
canonical IDs/time/graph/Track/Note lowering and every I3+ product remain out of scope. The next child may
bind the I3.1 Canonical IDs frontier after the closure review is merged.

## Fixture and diagnostic matrix

| Fixture | Stage/expectation | I2 obligation |
| --- | --- | --- |
| `source.valid.compile-time-generator` | elaborate success | exact expansion, source order, concrete Note output |
| `source.valid.template-if-with` | elaborate success | typed template, selected branch, immutable `with` |
| `source.valid.int-range-descending` | elaborate success | exact descending inclusive range and `index` |
| `source.invalid.unresolved-schema-enum` | elaborate `name.unknown` | do not infer a bare identifier as a schema string |
| `source.invalid.generator-zero-step` | elaborate `compile-time.zero-step` | reject before iteration/allocation |
| `source.invalid.shadowing` | elaborate `name.shadowed` | reject ancestor shadowing; retain related span |
| `source.invalid.template-missing-line` | elaborate `schema.missing-required-field` | validate required field at template return |
| `source.invalid.runtime-gameplay` | elaborate `schema.dynamic-field-forbidden` | reject runtime environment in structural field |
| manifest `source.invalid.generator-budget` | elaborate `compile-time.budget-exceeded` | shared generator counter and ordered trace |

Existing parse-error fixtures remain parser-owned. Existing canonical/evaluate fixtures must be syntax
accepted by I2 and must not be claimed as I2 semantic success; their canonical/runtime obligations belong to
I3/I4.

## Verification contract

### Baseline/document-only checkpoint

Before Rust changes, run only checks applicable to plan/review metadata:

```text
cargo metadata --no-deps --format-version 1
git diff --check
```

Also verify Markdown links, clause/fixture paths, SHA-256 values, and that the working tree contains no
unrelated changes. Clippy, nextest and rustfmt are not required for this documentation-only baseline.

### I2 Rust delivery checkpoints

During each task, run the smallest affected `cargo nextest`/test or targeted parser/elaborator command. Do
not run the workspace gate after every patch. At I2.10, public API/conformance changes, PR Ready, and merge
handoff run, in repository order:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
git diff --check
```

Record exact counts, focused commands, skipped gates and reasons in Issue/PR progress messages. A skipped gate
is not a pass. No `--release`, ordinary `cargo test`, `refer/` path dependency, or public artifact is allowed.

## Completion and invalidation

I2 is complete only when I2.1–I2.10 acceptance, S2/S3 fixture evidence, expanded-output invariants, full
workspace gate, implementation-matrix update, and an independent fixed-snapshot review all pass with zero
open Critical/Important findings. The five specification domains remain Draft. Any change to a bound clause,
fixture/expected file, diagnostic category, public invariant, or construction-schema behavior invalidates
this baseline and requires a new review before dependent work resumes.

## Residual routing

Unresolved semantics that are uniquely determined by the closure are LOCAL implementation fixes. A scope,
fixture, or measurement mismatch is PLANNER. A conflict between two materially different legal semantics,
an Accepted ADR change, missing external authority, or an unavailable independent review is HUMAN. Later-stage
blockers must be recorded with their owning stage and acceptance evidence; they do not become I2 semantics by
convenience.
