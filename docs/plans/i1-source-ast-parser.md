# I1 Complete Source AST and Parser Implementation Plan

> **For agentic workers:** Execute this plan in task order and update each checkbox only after its
> tests, implementation, review, and task gate pass. No specific agent skill or orchestration
> framework is required.

> **Current status (2026-07-16):** The I1 Reviewed Implementation Baseline passed and is recorded in
> `docs/reviews/2026-07-16-i1-source-parser-baseline-review.md`; I1 Task 1 passed its implementation
> gate and Task 2 is active automatically under ADR 0010. The independently reviewed plan snapshot
> hash is retained in that record; this status-only
> amendment does not change task semantics. Render raster, Conversion round-trip and Core canonical
> execution remain later-stage blockers unless a finding changes I1's public AST/parser behavior. I0
> remains the accepted infrastructure baseline; `e25352991d626fc8b1187d7a757e251038ea1f4f` is its
> historical verification commit, not the required implementation-start HEAD.

**Goal:** Complete the FCS 5 Core source syntax representation and parser on the I0 Chumsky token,
span, limit, and diagnostic boundary. Every Core production in `fcs.md` Appendix B must have an AST
representation and valid/invalid parser evidence; every source fixture whose failure belongs to a
later phase must parse without premature semantic rejection.

**I1 completion condition:** All legal Core source in the I1 conformance boundary parses into a
fully spanned AST; malformed Core source is rejected deterministically with the most specific
baseline-bound stable parser category and bounded spans; every Appendix B production has valid and invalid test
evidence; the parse-stage conformance runner executes applicable manifest entries; all I1 quality,
robustness, dependency, and independent-review gates pass.

**Architecture at I1 completion:** `fcs-source` remains the only workspace crate. It owns source AST,
decode, lexer, parser, parser limits, source diagnostics, and the pre-existing elaborator boundary.
The source AST is not the canonical chart model. I1 does not create `fcs-model`, a runtime,
converter, FCBC, render implementation, or CLI, and no crate may consume source AST as a substitute
for the future canonical model.

**Tech stack:** Rust 2024, Chumsky 0.11.2 stable APIs with `std` and `stacker`, Proptest 1.11 for
bounded property tests, the existing Serde/TOML test-only manifest loader, cargo-nextest, Clippy,
rustfmt, `fd`, `rg`, and ast-grep. I1 activates no catalog dependency that is not already owned by
`fcs-source`. A dedicated fuzz engine or new dev dependency requires a separate dependency audit
before activation; it must not be smuggled into an AST/parser task.

---

## Authoritative inputs

Before implementing a task, read the relevant files completely:

- `AGENTS.md`;
- `fcs.md`, especially sections 1.4, 2.1–2.10, 3–9, 11–13, 15–18, Appendix B, and Appendix C;
- `fcs-render.md` section 2 only to establish the ownership boundary of `renderBlock`;
- `docs/specification-governance.md`;
- `docs/decisions/0010-stage-scoped-implementation-baselines.md`;
- `docs/reviews/2026-07-15-fcs5-source-grammar-closure-review.md` 作为 grammar closure 历史证据，
  `docs/reviews/2026-07-15-fcs5-authoring-canonical-closure-review.md` 作为当前 Core delta/gate，
  `docs/reviews/2026-07-15-fcbc2-execution-abi-closure-review.md`、
  `docs/reviews/2026-07-15-render1-resource-binding-closure-review.md`、
  `docs/reviews/2026-07-15-conversion1-semantic-profile-closure-review.md` 作为依赖域 delta，
  以及 `docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md` 记录的跨规范 blocker 与阶段 baseline
  dated amendment；
- `docs/plans/fcs5-roadmap.md`, I1 and its quality gates;
- `docs/plans/i0-source-cutover.md`, especially the final parser/diagnostic/robustness state;
- `docs/conformance/fcs5-implementation-matrix.md`;
- `conformance/manifest.toml` and `conformance/fcs5/manifest.toml`;
- every `conformance/fcs5/source/**/*.fcs` input before writing the fixture runner;
- the current `crates/fcs-source/src/{ast,parser}` implementation and their callers/tests;
- the checked-out Chumsky 0.11 source under `refer/dependencies/chumsky` when an API detail is
  uncertain. Do not query Context7 for Chumsky while that audited source is available locally.

If any decision record, previous implementation, reference project, or this plan conflicts with the
current authoritative specification or bound conformance fixture, the authoritative source wins. Record an
ambiguity; do not silently turn current behavior into a new rule.

## Specification precondition gate

Before Task 1, establish a dated I1 Reviewed Implementation Baseline. Its normative dependency closure
must include:

- the `fcs.md` source lexical, grammar, document/schema/envelope, parse-stage diagnostic, parser-limit
  and parse-conformance clauses consumed by Tasks 1–8, including Appendix B and the applicable Appendix C
  categories;
- `fcs-render.md` section 2 only for the balanced Core `renderBlock` ownership/envelope boundary;
- every `conformance/fcs5` source input and manifest expectation that I1 will parse or reject;
- ADR 0006/0008/0010 architecture constraints that keep source AST separate from canonical/runtime and
  forbid compatibility/path-dependency shortcuts.

The baseline review must list exact clause and fixture coverage, record the candidate file hashes, confirm
Appendix B has no undefined nonterminal, and report no open Critical/Important finding in this dependency
closure. It must also list out-of-scope S15 blockers and explain why they cannot change I1's public AST,
parser categories, spans, limits or envelope behavior. Render raster/resource decode/attachment semantics,
Conversion round-trip and Core canonical evaluation remain out of scope unless dependency analysis proves
otherwise.

If the review changes source syntax, parser diagnostic ownership, fixture expectations or a prerequisite
public invariant, update this plan and repeat the independent baseline review. Planning text cannot fill a
normative gap, but no additional user confirmation is required once the objective gate passes.

## Baseline and preserved invariants

I1 starts from these accepted I0 invariants and must keep them true:

- the workspace contains only unversioned `crates/fcs-source`;
- decode happens once, accepts one leading UTF-8 BOM, and reports original half-open byte spans;
- lexer and grammar consume one Chumsky spanned-token stream, with no raw-text pre-parser, cursor,
  token re-lexing, fixed parser thread, or unstable/Pratt Chumsky API;
- diagnostics do not expose Chumsky types through the public API and are sorted deterministically;
- parser limits fail before the limited work or allocation and never return partial success;
- generator syntax accepts only `int|beat`, `..<|..=`, and body `let|if|emit`; bare `..`, `return`,
  and nested generators remain rejected;
- generator elaboration remains unavailable until I2 and never emits partial output;
- existing elaborator behavior remains green unless a richer AST requires an explicit adapter;
- no Cargo path dependency points into `refer/`, and inactive catalog dependencies remain inactive.

Record the starting evidence before modifying Rust:

```powershell
git status --short --branch
git rev-parse HEAD
cargo metadata --no-deps --format-version 1
cargo tree -e normal
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

Expected baseline: clean `master`; the accepted I0 verification commit is an ancestor of HEAD; the
confirmed S14 specification/plan commit is HEAD or an ancestor; and the workspace has one package.
Record the actual pre-I1 test count instead of requiring the historical I0 count of 135. If the user
has added unrelated changes after this plan was written, preserve them and record the actual baseline
instead of resetting them.

## Scope boundaries and parsing policy

### In scope

- the complete Appendix B token set, longest-match behavior, and lexical error boundaries;
- full source types and expressions, including array/object/reference/index/`choose`;
- a fully spanned document AST for `format`, all Core top-level blocks, definitions, collections,
  entity/schema blocks, Line, Track, segments/keyframes, metadata/resources/sync, extension and
  preserve source boundaries;
- typed function, template, collection, generator, segment, and entity statement categories that
  cannot represent placement-invalid statements;
- structural top-level rules, delimiter/terminator rules, source order, duplicate block reporting,
  and trailing-input rejection;
- parse-time nested/misplaced generator categories without evaluating range values;
- deterministic recovery, public parser limits, property testing, a dedicated fuzz lane, and
  parse-stage execution of bound conformance fixtures;
- implementation-matrix and roadmap evidence updates after the gates pass.

### Explicitly out of scope

- name resolution, duplicate object/schema key validation beyond syntactic block duplication,
  type inference/checking, schema field legality, required fields, profile requirements, reference
  existence/type, resource hash validation, and extension capability checks;
- function/template return-path analysis, generator range evaluation, zero-step detection,
  expansion, shared elaboration budgets, or removal of compile-time structures;
- Track interval/easing/overlap/gap semantics, tempo/graph/Hold validation, runtime expression DAG,
  canonical lowering, repair, ConversionReport, rendering, serialization, and formatting;
- modifying baseline-bound specification behavior or rewriting normative conformance expected outputs to
  match implementation behavior without first reopening the affected baseline.

An input that is syntactically valid but semantically invalid must produce an AST in I1. For
example, zero generator step, unknown resource references, duplicate typed custom keys, Track
overlap, invalid Hold end time, and runtime gameplay dependencies belong to later stages. I1 tests
must explicitly prevent their premature rejection.

### Render, extension, and preserve boundary

S14 closes this boundary normatively:

- `extensions` contains `extension(namespace, semver) required|optional` declarations whose payload
  is an ordered Core object; namespace schemas cannot add lexer tokens or private grammar;
- `preserve` has exactly one source schema block and one extension-header/ordered-object payload;
- Core parses and preserves the versioned Render payload as a balanced Core token tree; a
  Render-aware parser applies `fcs-render.md` section 2 in I9;
- successful Core envelope parsing never implies extension capability, Render semantic validity,
  normalization, repair, or execution.

I1 must implement these exact envelopes and may not restore arbitrary opaque namespace grammar,
mixed-Beat syntax, bare schema-enum guessing, or implementation-defined Render tokens.

## Target file map

Keep existing files where their ownership remains clear. The expected additions/reorganizations
are:

```text
crates/fcs-source/src/ast/document.rs       # format and source-ordered top-level blocks
crates/fcs-source/src/ast/metadata.rs       # meta/contributor/credit/resource/artwork/sync AST
crates/fcs-source/src/ast/track.rs          # Track, segment, point, interval and settings AST
crates/fcs-source/src/ast/extension.rs      # render/extension/preserve Core envelopes
crates/fcs-source/src/parser/metadata.rs
crates/fcs-source/src/parser/schema.rs
crates/fcs-source/src/parser/tracks.rs
crates/fcs-source/src/parser/extensions.rs
crates/fcs-source/tests/source_ast.rs
crates/fcs-source/tests/grammar.rs
crates/fcs-source/tests/conformance_parse.rs
crates/fcs-source/tests/fixtures/i1/        # implementation tests, not normative corpus rewrites
```

Names may be adjusted when an existing module is the clearer owner, but the final layout must not
create versioned modules, compatibility aliases, a second parser, or a second semantic model. Any
public AST reorganization must update callers atomically; do not retain old names as deprecated
aliases unless the user separately authorizes a compatibility policy.

## Required AST properties

Before parser expansion, establish and test these representation rules:

- every source node has the half-open byte span of its complete production; identifiers, field
  paths, operators, interval endpoints, and payload boundaries retain their own useful spans;
- `Document` retains top-level source order while offering unambiguous typed access; collections,
  credits, extension entries, render siblings, segments, generator output sites, array elements,
  and object members retain source order;
- function, template, collection, generator, segment, and render-child bodies use distinct enums
  or typed wrappers so `return`, `emit`, and `generate` cannot be placed in an illegal body through
  the public AST;
- `Type` represents `array<T>`, `Track<T>`, `TrackSegment<T>`, `Keyframe<T>`, scalar `vec2<T>`, and
  entity types without accepting grammatically impossible shapes;
- expressions represent array, ordered object entries, reference, index postfix, `choose` arms,
  constructor/call/field/`with`, and existing operators without losing associativity or spans;
- schema parsing stores source expressions, half-open intervals, schema-only cubic Bezier values,
  and field paths; it does not decide whether a field is known, duplicated, required, static,
  dynamic, or type-correct;
- extension payloads preserve ordered object entries; Render payload nodes preserve ordered balanced
  Core tokens and spans; neither is a canonical model;
- token types remain crate-private; the public AST must not expose Chumsky or lexer internals.

---

### Task 1: I1.1 Lexer completeness

**Clauses:** `fcs.md` 2.1–2.8, Appendix B lexer notes, Appendix C decode/version/syntax categories.

- [x] **Step 1: Add failing lexical coverage tests.** Build table-driven tests for every reserved
  word, punctuation/operator, literal/unit suffix, delimiter, and longest-match pair. Include
  `..<`, `..=`, bare `..`, `->`, `=>`, `**`, comparison/logical operators, `true`, `false`, `null`,
  semver-versus-float tokenization, all entity/type/envelope keywords, contextual Render
  identifiers, field-name keyword context, and identifiers adjacent to punctuation.
- [x] **Step 2: Add malformed lexical tests.** Cover a second/interior BOM, raw U+0000 versus valid
  `\0`, raw/escaped Unicode noncharacters, non-ASCII identifiers, leading-zero semver, malformed or
  overflowing numeric magnitudes, invalid unit adjacency, mixed-Beat rejection, malformed Color,
  bad Unicode scalar escapes, raw newline in strings, unclosed string/comment, nested comment depth,
  and invalid standalone punctuation. Assert that leading minus is always a unary token.
- [x] **Step 3: Complete the token model and Chumsky lexer.** Preserve longest-match behavior and
  original byte spans. Reserved words must never fall back to identifiers. Do not add a raw scan or
  re-lex path to produce special diagnostics.
- [x] **Step 4: Verify limit ordering.** Source-byte, token, comment-depth, nesting, and literal-byte
  limits must be checked before the limited allocation/recursion. Add identifier and token payload
  coverage if `max_literal_bytes` already owns them; otherwise document and add the smallest generic
  public limit needed rather than one limit per keyword.
- [x] **Step 5: Run the task gate.** Clippy must run before targeted nextest. Then run lexer,
  diagnostic, robustness, and workspace-structure tests, rustfmt, and `git diff --check`.

**Task gate:** Every Appendix B terminal has a unique intended tokenization; invalid lexemes have a
bounded baseline-bound stable category/span; structural tests still prove one Chumsky token path and no raw-text
pre-parser.

**Completed 2026-07-16:** The single Chumsky stream now covers the complete Core keyword/operator/unit
set, exact header and standalone semver tokenization, source BPM magnitudes, arbitrary-length semver and
integer magnitudes, malformed lexeme sentinels, Unicode/BOM/string/comment boundaries, and structured
source/token/comment/nesting/literal budgets. Structural tests also remove and forbid the legacy fixed
parser thread so Chumsky's reviewed `stacker` feature remains the sole recursive-stack mechanism. The
Task 1 gate passed with 175/175 workspace tests; the fixed I1 Core/Render/manifest/fixture-tree hashes were
reproduced without modifying any baseline-bound input. FCBC's later `u16` version-field representability
for a source semver component greater than 65535 is routed to I7 and does not truncate or weaken the I1
source AST.

### Task 2: I1.2 Complete grammar AST and expression/type parser

**Clauses:** `fcs.md` 2.9, 3.1, 4.1, 4.5, 6.3–6.4, 13.3, Appendix B `expression`, `type`,
`entityExpression`, `schemaBlock`.

- [x] **Step 1: Add failing AST-shape tests.** Assert constructible/non-constructible type shapes,
  full spans, source ordering, distinct statement enums, array/object entries, references, index
  postfix, `choose`, `with`, Track types, and nested generic types.
- [x] **Step 2: Extend source types and expressions.** Add only syntax representation. Keep typed
  values and elaborator-only nodes separate from untyped source nodes; do not make ordered custom
  objects into hash maps.
- [x] **Step 3: Add failing expression/type parser tests.** Cover every primary/postfix/type
  production, empty/trailing-comma arrays and objects, duplicate object keys that remain syntactic,
  nested calls/index/field access, right-associative power, comparison chains, `choose` arm order,
  required `else`, `null` source representation, keyword field access such as `.length`, unresolved
  bare schema enums, and complete-input rejection.
- [x] **Step 4: Implement the Chumsky parsers.** Reuse the existing recursive expression parser and
  nesting limit. Do not use Chumsky Pratt/unstable APIs or a manually indexed token cursor. Interval
  and cubic Bezier values remain schema productions, not first-class expression values.
- [x] **Step 5: Adapt existing elaborator matches safely.** New syntax outside I2 semantics must
  return the established feature-unavailable boundary or a precise later-phase diagnostic without
  panicking. Existing supported I0 expressions must retain behavior.
- [x] **Step 6: Run the task gate.** Run Clippy, targeted expression/AST/compile-time nextest,
  rustfmt, structural search for non-exhaustive shortcuts/raw cursors, and `git diff --check`.

**Task gate:** `fcs.md` 2.9 and every Appendix B expression/type production parse with exact spans;
syntactically valid later-phase expressions survive parsing; existing elaboration cannot produce
partial output for unsupported new nodes.

**Completed:** Source expression/type ASTs and parsers now cover the Appendix B expression/type
productions, ordered arrays/objects/choose arms, references/index/postfix, nested source types,
schema-owned intervals and cubic values, and the I2 boundary. Focused expression/AST/compile-time
tests and the full workspace gates passed; semantic conversion and elaboration remain later-stage
work.

### Task 3: I1.3 Complete document and top-level parser

**Clauses:** `fcs.md` 5.1–5.2, Appendix B `document`, `topLevelBlock`, and `formatBlock`.

- [x] **Step 1: Add failing document tests.** Cover required header/format ordering, all profiles,
  `features`, all top-level block kinds in multiple orders, missing/duplicate format, duplicate
  optional blocks, unknown blocks, misplaced nested-only blocks, trailing input, and exact block
  spans. Assert missing format uses `profile.requirement-missing`, while a later format uses
  `syntax.misplaced-block`; verify top-level source order is retained.
- [x] **Step 2: Introduce the complete document AST.** Represent `FormatBlock` and feature order
  explicitly. Replace the I0 subset fields with a source-ordered top-level enum plus typed accessors
  or an equivalent representation that cannot lose order or duplicate evidence.
- [x] **Step 3: Implement top-level Chumsky composition.** Parse every Core block through its owning
  parser. Replace the current indexed `validate_top_level_blocks`/`block_extent` prevalidation with
  parser composition and state/recovery so tokens are not independently reinterpreted by a second
  grammar scanner.
- [x] **Step 4: Enforce only the frozen front-end boundary.** Require immediate `format`, exactly one
  profile and at most one features field; retain field order. Leave profile/feature compatibility and
  required-block rules such as playable audio or publishable hashes to canonical validation.
- [x] **Step 5: Run the task gate.** Run Clippy, frontend/document/diagnostic nextest, rustfmt,
  `git diff --check`, and structural searches proving that unknown/trailing blocks cannot be silently
  skipped.

**Task gate:** Header plus format is mandatory, every Core top-level block is represented,
source-order rules are retained, duplicate/unknown/misplaced blocks have deterministic diagnostics,
and there is no token-slice top-level pre-parser.

**Completed:** The source-ordered document AST and owning Chumsky top-level parsers now accept every
Core envelope and preserve the parser/static boundary. Complete-document, profile, duplicate,
misplaced, trailing-input, and manifest-boundary tests passed; profile compatibility and required
resource rules remain later semantic validation.

### Task 4: I1.4 Complete definitions and typed statement parser

**Clauses:** `fcs.md` 5.3, 6.1–6.5, Appendix B `definition`, function/template declarations and
statement blocks.

- [x] **Step 1: Add failing production tests.** Cover empty/mixed definitions, parameter lists and
  trailing commas, recursive source types, nested/else-if blocks, explicit typed lets, return forms,
  template constructible types, and every placement-invalid statement.
- [x] **Step 2: Complete typed declaration/statement ASTs.** Function statements can contain only
  `let`, function `if`, and value return. Template statements can contain only `let`, template `if`,
  and entity return. Preserve declaration, parameter, statement, branch, and expression spans.
- [x] **Step 3: Complete Chumsky definitions parsers.** Ensure recursive blocks share parser limits,
  recover at statement/declaration boundaries, and do not consume the following declaration after a
  malformed body.
- [x] **Step 4: Preserve phase ownership.** Do not check shadowing, names, types, paths, required
  returns, constructibility, cycles, or branch semantics in the parser. Existing elaborator tests
  may continue to check the retained subset.
- [x] **Step 5: Run the task gate.** Run Clippy, definitions/expression/diagnostic/compile-time
  nextest, rustfmt, and `git diff --check`.

**Task gate:** Every Appendix B definition and typed statement production has valid/invalid parser
evidence, illegal statement categories are unrepresentable or rejected at their precise span, and
semantic errors still reach I2.

**Completed:** Definitions, typed statements, template branches, return forms, and declaration
boundaries are represented and parsed with source spans; placement-invalid generator/entity/return
forms retain parser-owned categories, while scope/type/return semantics remain I2-owned.

### Task 5: I1.5 Generator placement and owner grammar

**Clauses:** `fcs.md` 5.4, 6.5–6.7, 9.1, Appendix B collection/generator/segment productions,
Appendix C generator categories.

- [x] **Step 1: Add failing context tests.** Exercise generators in top-level collections and Track
  `segments`; direct collection items; collection/segment `if`; typed local lets; nested generator;
  generator in function/template/top-level/entity field; `emit` outside a generator; `return` in a
  generator; and malformed range operators.
- [x] **Step 2: Generalize owner-aware source nodes.** A generator retains its owner/registered
  entity context structurally without resolving its type. Reuse typed generator statements across
  legal owner collections without allowing nested generators through the AST.
- [x] **Step 3: Emit baseline-bound source-structure categories.** Bare `..` remains `syntax.invalid-token`;
  syntactically recognizable nested and misplaced generators use
  `compile-time.nested-generator` and `compile-time.misplaced-generator` with `Parse` stage and the
  generator keyword as primary span. Add related owner/generator spans where useful.
- [x] **Step 4: Preserve the I2 boundary.** Parse and retain zero step, range expressions, and emit
  expressions without evaluating type, direction, reachability, count, or collection compatibility.
  Elaborating any generator must still fail before partial output.
- [x] **Step 5: Run the task gate.** Run Clippy, generator/collection/Track/diagnostic/compile-time
  nextest, rustfmt, and `git diff --check`.

**Task gate:** Legal generators parse in every Core owner grammar; nested/misplaced cases use the
baseline-bound stable categories; zero-step remains an I2 error; no parser or elaborator path emits partial data.

**Completed:** Collection, Track-segment, generator, `if`, typed `let`, and `emit` owner grammars
are covered, including nested/misplaced diagnostics and zero-step retention. No generator expansion
or semantic direction/count validation was added to I1.

### Task 6: I1.6 Complete Core schema-block parsers

**Clauses:** `fcs.md` 5.4, 7.1–7.5, 8.2, 9.1, 11–13 source shapes, 15.1–15.2, Appendix B entity,
Track, metadata, extension, preserve, and delegated render boundaries.

- [x] **Step 1: Add failing source-AST tests for Line and entities.** Cover `lines`, Line blocks,
  Note variants, `RenderNode` envelope, dotted field paths, nested `tracks`, `scrollTempoMap`,
  constructors, template calls, and chained `with` blocks.
- [x] **Step 2: Add failing Track tests.** Cover settings, required `segments` syntax, direct
  half-open interval segments, point, `segment`/`keyframe` constructors, string interpolation
  expressions/quoted names, cubic Bezier syntax, segment `if`, and segment generators. Half-open
  intervals parse only in owning schema productions; `[start,end]` always remains an ordinary
  two-element array expression.
- [x] **Step 3: Add failing metadata/resource/sync tests.** Cover the complete examples for `meta`,
  contributors/person, credits/credit, resources and resource kinds, artwork, sync preview,
  quoted closed enums, references, arrays, ordered custom objects, and custom scalar values. Keep field legality,
  duplicate custom keys, reference existence/type, hash syntax, and interval ordering for later
  semantic phases.
- [x] **Step 4: Add extension/preserve/render envelope tests.** Cover exact namespace/semver/
  required-or-optional syntax, ordered object payloads, preserve source/payload entry retention,
  balanced Render Core tokens, source spans, truncation/unbalanced delimiters, and trailing input.
  Assert that namespace-specific object semantics and Render payload semantics are not interpreted
  in I1; missing/duplicate preserve entries remain I5 schema validation.
- [x] **Step 5: Implement owning parsers and AST modules.** Reuse generic schema-field and ordered
  object components where the grammar is identical, but keep schema blocks, custom objects,
  extension envelopes/ordered payload objects, and balanced Render payloads as distinct AST types.
- [x] **Step 6: Execute all 32 bound source entries at their front-end boundary.** Valid fixtures and
  semantic-invalid fixtures whose
  manifest stage is elaborate/canonical/evaluate must parse successfully even though their expected
  later-phase result is not executed in I1.
- [x] **Step 7: Run the task gate.** Run Clippy, schema/metadata/Track/frontend/conformance-parse
  nextest, rustfmt, and `git diff --check`.

**Task gate:** Every Core top-level/schema source shape is represented with complete spans; all
currently bound syntactically valid source inputs parse; later-phase semantic-invalid fixtures are
not rejected by I1 for their semantic error.

**Completed:** The source AST/parser owns metadata, resources, sync, Line/Note/Track shapes,
extension/preserve envelopes, and balanced Render payloads. The manifest runner now executes all 39
entries at their declared frontend boundary; canonical/resource/render semantics remain assigned to
their owning later stages.

### Task 7: I1.7 Diagnostic mapping and recovery

**Clauses:** `fcs.md` 1.2, 2, 5, 6.6–6.7, 16, Appendix B, Appendix C.

- [x] **Step 1: Create a parser diagnostic inventory.** Map every lexer/grammar failure class to
  `decode.invalid-utf8`, `version.*`, the most specific `syntax.*`, the two generator placement
  categories, or the pre-existing implementation-defined parser resource-limit code. Mark every
  semantic Appendix C category as forbidden for generic parser fallback.
- [x] **Step 2: Add failing recovery tests.** Include multiple independent malformed declarations,
  missing terminators/delimiters, unclosed nested blocks, invalid top-level blocks, malformed
  expressions followed by valid siblings, duplicate blocks, and UTF-8 multibyte text before errors.
  Assert deterministic order, deduplication, primary/related half-open spans, and no output when any
  error exists.
- [x] **Step 3: Implement bounded recovery at grammar boundaries.** Recover only at unambiguous
  semicolon/block/declaration boundaries. Recovery must make progress, respect token/nesting/AST
  limits, and never reinterpret schema/type/name errors as accepted syntax.
- [x] **Step 4: Remove ad hoc diagnostic rescans.** Delete helpers that scan raw source or replay
  token slices solely to guess parser errors. Use Chumsky labels/state and parser-owned context to
  produce diagnostics.
- [x] **Step 5: Add structural regression tests.** Prevent reintroduction of raw lexer prepasses,
  indexed token cursors, independent top-level scanners, leaked Chumsky public types, and ignored
  trailing tokens.
- [x] **Step 6: Run the task gate.** Run Clippy, all diagnostic/parser tests, rustfmt, structural
  searches, and `git diff --check`.

**Task gate:** Every invalid I1 grammar test has a stable expected category and bounded primary span;
multi-error results are deterministic; malformed inputs cannot panic, loop, or return partial ASTs.

**Completed:** Diagnostic inventory and bounded Chumsky recovery are implemented with stable
category/span ownership, deterministic multi-error ordering, no partial output, and structural guards
against raw rescans, leaked token types, and ignored trailing input.

### Task 8: I1.8 Fixture execution, limits, robustness, and fuzz lane

**Clauses:** `fcs.md` 1.2–1.3, 2.1, 16, 18; roadmap I1.8.

- [x] **Step 1: Build the parse-stage fixture runner.** Reuse the typed manifest loader. Execute
  parse-stage success/error expectations and assert exact category for parse failures. Separately
  assert that all later-stage source fixtures are syntactically accepted, except fixtures whose
  source is intentionally malformed. Do not execute or rewrite elaborate/canonical/evaluate
  snapshots in I1. Include the S14 complete-envelope, escaped-NUL, header whitespace/leading-zero,
  duplicate-block, placement, unclosed-extension, mixed-Beat and unresolved-enum cases explicitly.
- [x] **Step 2: Create a production coverage ledger.** Map every Appendix B production to at least
  one valid and one invalid unit/fixture test. Map every complete FCS document example to a parse
  test; wrap fragmentary examples in the smallest legal owning production rather than pretending
  fragments are documents.
- [x] **Step 3: Extend public parser limits.** Add a generic AST-node/list-item budget if required by
  the complete grammar. Publish defaults and semantics, increment before work, and test each limit
  at `limit-1`, `limit`, and `limit+1`. Keep parser budgets distinct from I2 elaboration budgets.
- [x] **Step 4: Expand deterministic properties.** Generate arbitrary bounded bytes/UTF-8, balanced
  and unbalanced delimiters, nested comments, arrays/objects, expressions, declarations, blocks,
  and long legal documents. Assert no panic, bounded spans, deterministic result/diagnostics,
  complete consumption, and controlled limit failure.
- [x] **Step 5: Establish an independent fuzz lane.** Before choosing a runner, audit its current
  version, features, MSRV, license, and dependency tree and record the decision. Keep fuzz tooling
  outside normal/runtime dependencies. Seed the corpus from all conformance source inputs and I1
  grammar fixtures; target both byte and UTF-8 document entry points plus expression parsing; define
  a bounded smoke command for CI and an unbounded local command. If no acceptable runner is approved,
  I1 cannot be marked complete merely on Proptest evidence.
- [x] **Step 6: Run the task gate.** Run Clippy first, then the full workspace nextest suite,
  deterministic property configuration, the bounded fuzz smoke lane, rustfmt, and
  `git diff --check`.

**Task gate:** Every production is covered, applicable conformance entries execute, every public
limit has a bounded failure, long legal inputs pass, property/fuzz inputs do not panic or escape
limits, and normal dependency activation remains unchanged.

**Completed:** I1.8a/#34 and I1.8b/#36 and I1.8c/#38 are merged. The final I1.8 evidence is the
39-entry frontend runner (3 parse-success, 9 parse-error, 27 later-stage syntax acceptance), the
117-production ledger, six public parser-limit boundary contracts, 12 deterministic robustness
properties, and the isolated three-target cargo-fuzz smoke over 42 seeds. No AST/list budget was
needed; the independent unbounded lane remains documented as local-only.

### Task 9: Governance, full quality gate, and independent review

This task closes I1 after roadmap tasks I1.1–I1.8; it does not add product behavior.

**Current governance unit:** I1.1–I1.8 implementation and evidence are merged through the parser-boundary
runner, production ledger, parser-limit, deterministic-property, and isolated fuzz-lane checkpoints. The
remaining work is limited to the matrix/roadmap/AGENTS reconciliation, final integrity gates, and an
independent fixed-snapshot review; I1 is not complete until this task gate passes.

- [x] **Step 1: Update the implementation matrix.** Replace all `blocked-by-I1` rows with evidence
  and the honest later-stage status. Parser-complete rows may become `implemented`; mixed
  parser/semantic rows remain `partial` and name I2/I3/I4/I5 explicitly. Never mark Track,
  metadata, choose, extension, or conformance rows fully implemented when only their source syntax
  exists.
- [x] **Step 2: Update roadmap and plan status.** Mark I1.1–I1.8 complete only after their task gates,
  record exact tests/fixtures/fuzz commands, and make I2 the next unstarted phase. Update `AGENTS.md`
  to point to the completed I1 plan without changing baseline-bound specification behavior.
- [x] **Step 3: Audit the public/source boundary.** Confirm one source crate, one token path, no
  `v5`/FCS 4 compatibility paths, no canonical/runtime model, no path dependency into `refer/`, no
  accidental activation of future catalog dependencies, and no public Chumsky/token types.
- [ ] **Step 4: Run final gates in required order.** Run:

  ```powershell
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  cargo fmt --all
  cargo fmt --all -- --check
  cargo tree -e normal
  cargo tree -e dev
  git diff --check
  git status --short --branch
  ```

  Also run the bounded fuzz smoke command and the production-coverage audit defined in Task 8.
- [ ] **Step 5: Independently review I1.** Review Appendix B coverage, parser/static phase separation,
  AST representability, spans/recovery, limits, all `blocked-by-I1` matrix transitions, dependency
  activation, and the full I1 diff. Fix Critical/Important findings and repeat review before
  acceptance.
- [ ] **Step 6: Record exact evidence.** Capture the final master SHA, workspace packages,
  dependency tree, nextest pass count, parse fixture counts, production coverage, fuzz smoke result,
  remaining partial/blocked matrix rows, review disposition, and confirmation that baseline-bound clauses and
  fixtures were not changed without reopening the baseline.

**I1 final gate:** All I1.1–I1.8 tasks are complete; no matrix row remains `blocked-by-I1`; all legal
Core source fixtures parse; all parser-invalid fixtures have deterministic categories/spans; all
quality and independent-review gates pass; I2 has not been implemented accidentally.

## Expected matrix transitions

Use evidence rather than this table if implementation reveals a narrower result. These are the
minimum intended transitions:

| Matrix capability | I1 completion expectation |
|---|---|
| 2.1–2.7 decode/header/trivia/identifiers/literals | remain `implemented`, add complete-grammar evidence |
| 2.8–2.9 separators/array/object/reference/interval | `implemented` |
| 3.1–3.4 source types and conversions | remain `partial` for I2 semantic conversion/type rules |
| 4.1 expression precedence/primary forms | `implemented` |
| 5.1–5.2 document/format/profile syntax | parser portion `implemented`; profile semantic requirements remain assigned later if the row is split or documented |
| 5.3–5.4 definitions/collections | remain `partial` for I2 static semantics/expansion |
| 6.6–6.7 generator syntax | remain `partial` for I2 evaluation/expansion, with I1 placement grammar complete |
| 7.1–7.5 metadata/resources/sync/custom | move from `blocked-by-I1` to `partial`, next I3/I5 semantic owners explicit |
| 8.1–8.3 source time structures | remain `partial`, next I3 |
| 9.1–9.5 Track | move from `blocked-by-I1` to `partial`, next I3/I4 |
| 13.1–13.4 choose/runtime expressions | move from `blocked-by-I1` to `partial`, next I4 |
| 15.1–15.3 extension/preserve | move from `blocked-by-I1` to `partial`, next I5 |
| 18 conformance runner | remain `partial`, with parse-stage execution implemented |
| `fcs-render.md` | remain `blocked-by-I9`; record only the I1 Core balanced-envelope evidence |

Do not collapse a mixed syntax/semantic row to `implemented` merely to remove an I1 blocker. Split
the row only if doing so improves traceability and both rows retain clauses, APIs, files, tests,
status, owner, and deviation text.

## Commit and review checkpoints

Keep changes reviewable. The preferred checkpoints are lexer, AST/expression, document, definitions,
generator, schema blocks, diagnostics, robustness/fixtures, and governance. A checkpoint is not
complete until its Clippy-before-nextest gate passes. Do not rewrite unrelated user history, move
the FCS 4 archive, or publish/push without separate user authorization.

## Completion handoff

The I1 handoff must report:

- final commit(s) and exact workspace package list;
- AST/parser modules added or reorganized and any intentional public API break;
- exact Appendix B production coverage and complete-document example coverage;
- parse-stage fixture counts and which later-stage fixtures were only syntax-checked;
- exact Clippy, nextest, rustfmt, diff, property, and fuzz results;
- parser limits and defaults;
- dependency/feature changes, including fuzz-only tooling;
- implementation-matrix transitions and all remaining `partial`/blocked owners;
- render/extension/preserve boundary and any recorded baseline/specification ambiguity;
- independent-review findings and disposition;
- confirmation that baseline-bound `fcs.md`/`fcs-render.md` clauses, normative conformance
  inputs/expected files and `archive/fcs4-pre-cutover` were not changed during I1 implementation without
  reopening the baseline.

Do not begin I2 in the same implementation task. Start I2 automatically in a separate implementation
task only after its normative dependency closure has a Reviewed Implementation Baseline with no open
Critical/Important finding, the I2 plan matches the bound clauses, and the I1 quality gate passes.
