# FCS 5 Phase 2 Typed Templates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse and elaborate FCS 5 typed templates, constructors, and `with` expressions into non-empty schema-validated `ExpandedSourceDocument` collections.

**Architecture:** Extend the v5 source AST and document parser with template and collection blocks while preserving source spans. Add a dedicated entity elaboration layer that reuses the existing scalar compile-time evaluator, validates fields through `ConstructionSchema`, and produces concrete `ExpandedEntity` values. Keep generators and `emit` out of this milestone.

**Tech Stack:** Rust 2024, existing v5 parser/AST/elaborator, `cargo-nextest`, Clippy, rustfmt.

---

### Task 1: Add template and collection source AST

**Files:**
- Modify: `crates/fcs-core/src/v5/ast/entity.rs`
- Modify: `crates/fcs-core/src/v5/ast/mod.rs`
- Modify: `crates/fcs-core/src/v5/ast/definitions.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: Write failing AST/API tests**

Add tests that construct a template declaration and assert its name, return
type, parameters, body span, and collection expression span are retained.

- [ ] **Step 2: Run the focused test and verify the expected compile failure**

Run `cargo nextest run -p fcs-core --test fcs5_phase2 template_ast`. Expect a
compile failure because template and collection declaration types are absent.

- [ ] **Step 3: Implement the minimal AST types**

Add `TemplateDeclaration`, `TemplateParameter`, `TemplatesBlock`,
`CollectionsBlock`, and collection items for entity expressions. Add optional
`templates` and `collections` fields to `Document`; export the new types.

- [ ] **Step 4: Run focused tests and existing Phase 2 tests**

Run `cargo nextest run -p fcs-core --test fcs5_phase2`. All tests should pass.

- [ ] **Step 5: Commit the AST boundary**

Run `git add crates/fcs-core/src/v5/ast crates/fcs-core/tests/fcs5_phase2.rs` and
commit with `feat(core): add FCS 5 template source AST`.

### Task 2: Parse templates, constructors, `with`, and collections

**Files:**
- Modify: `crates/fcs-core/src/v5/parser/document.rs`
- Modify: `crates/fcs-core/src/v5/parser/mod.rs`
- Create: `crates/fcs-core/src/v5/parser/entities.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: Write failing parser tests**

Add source fixtures covering a Note constructor, nested `with`, a template
call, and a `notes` collection. Assert exact names, field paths, variants, and
source spans.

- [ ] **Step 2: Run the parser tests and verify they fail**

Run `cargo nextest run -p fcs-core --test fcs5_phase2 parses_templates`. Expect
`ParseError` because the document parser does not recognize the new blocks.

- [ ] **Step 3: Implement block parsing**

Implement balanced-brace entity parsing with comment/string awareness. Parse
typed template signatures, constructor variants (`tap`, `hold`, `flick`,
`drag`, `Line`), dotted fields, scalar expressions through
`parse_expression_at`, template calls, and nested `with` suffixes. Parse named
collections and retain source order.

- [ ] **Step 4: Run parser and regression tests**

Run `cargo nextest run -p fcs-core --test fcs5_phase2` and confirm all parser,
frontend, and existing Phase 2 tests pass.

- [ ] **Step 5: Commit parser support**

Commit with `feat(parser): parse FCS 5 templates and entity collections`.

### Task 3: Add schema-aware entity elaboration

**Files:**
- Modify: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Create: `crates/fcs-core/src/v5/elaborator/entities.rs`
- Modify: `crates/fcs-core/src/v5/ast/entity.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: Write failing expansion and validation tests**

Cover successful Note construction, template argument substitution, nested
`with`, unknown field, duplicate field, missing required field, wrong field
type, wrong template arity, recursive template, and non-constructible entity.

- [ ] **Step 2: Run tests and verify they fail for the missing expansion path**

Run `cargo nextest run -p fcs-core --test fcs5_phase2 entity_expansion`. Expect
the current empty `ExpandedSourceDocument` or missing diagnostics.

- [ ] **Step 3: Implement expansion**

Add dedicated diagnostics and an entity evaluator. Resolve template names in a
deterministic map, evaluate scalar field expressions with the existing scope,
validate schema fields and required paths, apply `with` overrides immutably,
and construct `ExpandedCollection` values. Count template depth and instances
against the existing limits.

- [ ] **Step 4: Run focused tests**

Run `cargo nextest run -p fcs-core --test fcs5_phase2` and verify every new
behavior passes.

- [ ] **Step 5: Commit elaboration**

Commit with `feat(core): elaborate FCS 5 typed templates`.

### Task 4: Add compile-time structural conditions and fixture

**Files:**
- Modify: `crates/fcs-core/src/v5/elaborator/entities.rs`
- Modify: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Modify: `crates/fcs-core/tests/fcs5_phase2.rs`
- Create: `examples/fcs/fcs5-templates.fcs`

- [ ] **Step 1: Write failing conditional and fixture tests**

Assert a compile-time `if` includes only the selected entity branch and a
runtime-only condition produces `NonConstantStructuralCondition`. Load the
public fixture through the public parser and elaborator APIs.

- [ ] **Step 2: Verify the tests fail**

Run the focused tests and confirm the conditional/fixture behavior is absent.

- [ ] **Step 3: Implement the minimal conditional expansion**

Evaluate structural conditions as `TypedValue::Bool`, recursively expand the
selected branch, and reject all other values with the dedicated diagnostic.

- [ ] **Step 4: Run full required validation**

Run, in order:

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
```

- [ ] **Step 5: Update status documentation and commit**

Mark Task 5 complete in the Phase 2 plan, update the roadmap to `In progress`,
and commit the fixture and documentation with `docs: record FCS 5 template milestone`.
