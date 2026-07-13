# FCS 5 Phase 2 编译期语言实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在保持 v4 API 和 Phase 1 FCS 5 front end 不变的前提下，实现强类型、有限、纯函数式的编译期语言，并输出可供 Phase 3 canonical lowering 消费的 `ExpandedSourceDocument`。

**Architecture:** 将 Phase 2 分为四个必须线性完成的里程碑：P2.1 typed language kernel，P2.2 construction schema 与 typed template，P2.3 `generate`/`emit`，P2.4 budget、trace 与 expanded IR。parser 只生成带 span 的 source AST；`v5::elaborator` 是唯一的名称解析、类型检查、常量求值、schema 验证与 expansion 入口。

**Tech Stack:** Rust 2024、现有 `nom` 8、`thiserror`、Cargo Clippy、cargo-nextest、rustfmt。

---

## 分支与全局约束

从干净的 `master` 创建普通分支，不创建 worktree：

```text
git status --short
git switch -c codex/fcs5-phase2-compile-time-language
```

每个任务先写失败测试，再实现最小代码；每个里程碑完成时按顺序运行：

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
```

不得修改 v4 `ast`、`parser`、`compiler`、`converter` 或 CLI 默认入口。任何 FCS 5 新 API 位于 `fcs_core::v5`。

## 文件地图

| 路径 | 责任 |
|---|---|
| `crates/fcs-core/src/v5/ast/types.rs` | `Type`、`SourceSpan`、`SourceExpression`、`TypedExpression` 与 `TypedValue`。 |
| `crates/fcs-core/src/v5/ast/definitions.rs` | definitions、const、let、fn、template 的 source AST。 |
| `crates/fcs-core/src/v5/ast/entity.rs` | source entity constructor、field path、collection、range 与 expanded IR。 |
| `crates/fcs-core/src/v5/ast/mod.rs` | 公开 Phase 2 AST 类型并将 definitions/collections 纳入 `Document`。 |
| `crates/fcs-core/src/v5/parser/lexer.rs` | Unicode-safe source token、span、注释和 literal tokenization。 |
| `crates/fcs-core/src/v5/parser/expression.rs` | Phase 2 expression、type syntax、field path、operator precedence。 |
| `crates/fcs-core/src/v5/parser/definitions.rs` | `definitions` block、const/fn/template 语法。 |
| `crates/fcs-core/src/v5/parser/entity.rs` | constructor、`with`、collection、generate/emit/range 语法。 |
| `crates/fcs-core/src/v5/parser/document.rs` | 将 Phase 2 blocks 连接到 document parser，并保留 Phase 1 profile/tempo 验证。 |
| `crates/fcs-core/src/v5/schema.rs` | `ConstructionSchema`、Phase 2 Note/Line bootstrap registry。 |
| `crates/fcs-core/src/v5/elaborator/mod.rs` | `elaborate` public entrypoint、diagnostic 与 limits。 |
| `crates/fcs-core/src/v5/elaborator/{scope,eval,cycle,template,generator}.rs` | scope、常量/函数求值、图环检测、template/with、generator expansion。 |
| `crates/fcs-core/tests/fcs5_phase2.rs` | Phase 2 全部 public API 行为测试。 |
| `examples/fcs/fcs5-compile-time.fcs` | definitions、with 与 beat generator 的公开 fixture。 |
| `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md` | 将 Phase 2 状态同步为 In progress / Complete。 |

## Milestone P2.1：typed language kernel

### Task 1: 建立 span、类型和 expression source AST

**Files:**

- Create: `crates/fcs-core/src/v5/ast/types.rs`
- Modify: `crates/fcs-core/src/v5/ast/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: 写失败的 public type tests**

Add tests that import these public types:

```rust
use fcs_core::v5::ast::{SourceSpan, Type, TypedValue};

#[test]
fn phase2_types_keep_units_distinct() {
    assert_ne!(Type::Beat, Type::Time);
    assert_eq!(Type::Vec2(Box::new(Type::Length)).to_string(), "vec2<length>");
    assert_eq!(SourceSpan::new(3, 7).len(), 4);
    assert_eq!(TypedValue::Int(4).ty(), Type::Int);
}
```

- [ ] **Step 2: 验证测试因 Phase 2 API 缺失而失败**

Run:

```text
cargo clippy -p fcs-core --test fcs5_phase2 -- -D warnings
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: compile failure resolving `SourceSpan`、`Type` 或 `TypedValue`。

- [ ] **Step 3: 实现最小公开类型层**

Create the following public shape in `ast/types.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan { pub start: usize, pub end: usize }

impl SourceSpan {
    pub const fn new(start: usize, end: usize) -> Self { Self { start, end } }
    pub const fn len(self) -> usize { self.end - self.start }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Bool, Int, Float, String, Time, Beat, Length, Angle, Color,
    Vec2(Box<Type>), Note, Line, RenderNode,
    TrackSegment(Box<Type>), Keyframe(Box<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedValue {
    Bool(bool), Int(i64), Float(f64), String(String),
    Time(f64), Beat(Beat), Length(f64), Angle(f64), Color(Color),
    Vec2(Box<TypedValue>, Box<TypedValue>),
}
```

Add `Type: Display` with canonical spellings from the design and `TypedValue::ty`. Add `SourceExpression` variants for literal, name, unary, binary, call, field access and `vec2`, each carrying `SourceSpan`; elaboration must produce `TypedExpression { expression, ty, span }` and concrete `TypedValue` only after type checking succeeds.

- [ ] **Step 4: Re-run the focused test**

Run:

```text
cargo clippy -p fcs-core --test fcs5_phase2 -- -D warnings
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: Phase 2 type tests pass.

- [ ] **Step 5: Commit the typed AST base**

```text
git add crates/fcs-core/src/v5/ast crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(core): add FCS 5 typed expression AST"
```

### Task 2: Parse typed declarations and expressions

**Files:**

- Create: `crates/fcs-core/src/v5/parser/lexer.rs`
- Create: `crates/fcs-core/src/v5/parser/expression.rs`
- Modify: `crates/fcs-core/src/v5/parser/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: 写失败的 parser tests**

Add tests for explicit types and precedence:

```rust
use fcs_core::v5::parser::parse_expression;

#[test]
fn parses_typed_phase2_expression_shape() {
    let expression = parse_expression("1beat + 2beat * 3").unwrap();
    assert_eq!(expression.span().start, 0);
}

#[test]
fn parses_nested_type_syntax() {
    assert_eq!(fcs_core::v5::parser::parse_type("vec2<length>").unwrap(), Type::Vec2(Box::new(Type::Length)));
}
```

- [ ] **Step 2: 验证 parser tests 失败**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 parses_typed_phase2_expression_shape parses_nested_type_syntax
```

Expected: compile failure because `parse_expression` and `parse_type` are not exported.

- [ ] **Step 3: 实现 lexer 与 expression parser**

Implement a token stream with byte-offset `SourceSpan`, skipping line/block comments and preserving literal spans. `parse_type` must accept exactly the scalar names in `Type` plus recursive `vec2<T>`、`TrackSegment<T>`、`Keyframe<T>`; all other names produce `ParseError::InvalidSyntax("type")`.

`parse_expression` must parse literals, identifiers, calls, dotted field access, parentheses, unary `-`/`!`, multiplicative operators, additive operators, comparison/equality operators, and boolean operators. The parser only builds source AST; no type checking occurs in this task.

- [ ] **Step 4: Re-run parser tests and existing Phase 1 tests**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: all core tests pass, including existing `fcs5_frontend`.

- [ ] **Step 5: Commit lexer and expression parsing**

```text
git add crates/fcs-core/src/v5/parser crates/fcs-core/src/v5/ast crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(parser): parse FCS 5 typed expressions"
```

### Task 3: Elaborate const, let and pure fn

**Files:**

- Create: `crates/fcs-core/src/v5/ast/definitions.rs`
- Create: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Create: `crates/fcs-core/src/v5/elaborator/scope.rs`
- Create: `crates/fcs-core/src/v5/elaborator/eval.rs`
- Create: `crates/fcs-core/src/v5/elaborator/cycle.rs`
- Modify: `crates/fcs-core/src/v5/mod.rs`
- Modify: `crates/fcs-core/src/v5/parser/definitions.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: 写失败的 elaboration tests**

Add public tests for type checking, shadowing and pure functions:

```rust
#[test]
fn elaborates_const_and_pure_function() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  const SPACING: length = 120px;
  fn twice(value: length) -> length { return value * 2; }
}"#;
    let document = parse_document(source).unwrap();
    assert!(elaborate(&document, &phase2_schema(), CompileTimeLimits::default()).is_ok());
}

#[test]
fn rejects_shadowing_in_nested_scope() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { fn f(value: int) -> int { let value: int = 1; return value; } }"#;
    assert!(matches!(elaborate_source(source), Err(Diagnostic::ShadowedBinding { .. })));
}
```

- [ ] **Step 2: 验证 tests 因 definitions/elaborator 缺失而失败**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 elaborates_const_and_pure_function rejects_shadowing_in_nested_scope
```

Expected: parser rejects `definitions` or compile fails because `elaborate` API is absent.

- [ ] **Step 3: 实现 definitions AST、scope 和 evaluator**

Add source AST for typed const, local let, function parameter, function declaration, return and compile-time if. Extend `Document` with `definitions: Option<DefinitionsBlock>`.

Expose:

```rust
pub fn elaborate(
    document: &Document,
    schema: &ConstructionSchema,
    limits: CompileTimeLimits,
) -> Result<ExpandedSourceDocument, Diagnostic>;
```

Implement immutable lexical scope. Insert every binding name into every ancestor-name set before allowing a declaration; return `Diagnostic::ShadowedBinding` for same-scope and nested reuse. Evaluate only typed literals, pure operators, `pi` and the fixed-signature builtins `sin`、`cos`、`toFloat`、`approxEq`.

Build separate const and function dependency graphs before evaluation. Return `Diagnostic::RecursiveConst` or `Diagnostic::RecursiveFunction` for a cycle; never execute a recursive call until a limit is reached.

- [ ] **Step 4: Verify language kernel behavior**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: const/function tests pass; existing Phase 1 tests remain green.

- [ ] **Step 5: Commit the language kernel**

```text
git add crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(core): elaborate FCS 5 compile-time values"
```

## Milestone P2.2：construction schema 与 typed template

### Task 4: Define bootstrap construction schemas and entity AST

**Files:**

- Create: `crates/fcs-core/src/v5/ast/entity.rs`
- Create: `crates/fcs-core/src/v5/schema.rs`
- Modify: `crates/fcs-core/src/v5/ast/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: 写失败的 schema tests**

Add tests for the Phase 2 bootstrap surface:

```rust
#[test]
fn phase2_schema_requires_note_time_and_types_position() {
    let schema = phase2_schema();
    let note = schema.entity(&Type::Note).unwrap();
    assert_eq!(note.field("gameplay.time").unwrap().ty, Type::Beat);
    assert_eq!(note.field("presentation.positionX").unwrap().ty, Type::Length);
}

#[test]
fn render_node_is_not_constructible_in_phase2() {
    assert!(matches!(phase2_schema().entity(&Type::RenderNode), None));
}
```

- [ ] **Step 2: Verify schema tests fail**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 phase2_schema_requires_note_time_and_types_position render_node_is_not_constructible_in_phase2
```

Expected: compile failure because `ConstructionSchema` and `phase2_schema` are absent.

- [ ] **Step 3: Implement schema and source entity structures**

Add immutable `ConstructionSchema` with `EntitySchema`、`FieldSchema`、`CollectionSchema`. Use `BTreeMap` for deterministic field and entity lookup. Register exactly the Note fields and Line identity fields listed in the approved Phase 2 design; register `notes → Note` and `judgelines → Line` collections.

Add AST for `EntityConstructor`、`EntityField`、`WithExpression`、`CollectionBlock` and `ExpandedEntity`. Keep entity variant as an explicit `NoteVariant` for `tap`、`hold`、`flick`、`drag`; do not validate Hold timing or line semantics here.

- [ ] **Step 4: Verify schema tests pass**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: bootstrap schema tests pass with no Phase 3 runtime model dependency.

- [ ] **Step 5: Commit schemas and source entity AST**

```text
git add crates/fcs-core/src/v5/ast crates/fcs-core/src/v5/schema.rs crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(core): add FCS 5 construction schemas"
```

### Task 5: Parse and elaborate templates with `with`

**Files:**

- Create: `crates/fcs-core/src/v5/parser/entity.rs`
- Modify: `crates/fcs-core/src/v5/parser/definitions.rs`
- Create: `crates/fcs-core/src/v5/elaborator/template.rs`
- Modify: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: Write failing template tests**

Add a successful composition test and two field-validation tests:

```rust
#[test]
fn template_with_expands_to_typed_note() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  template Note base(time: beat, x: length) {
    return tap { gameplay.time: time; presentation.positionX: x; };
  }
  template Note large(time: beat, x: length) {
    return base(time, x) with { presentation.scaleX: 1.25; };
  }
}"#;
    let expanded = elaborate_source(source).unwrap();
    assert!(expanded.collections.is_empty());
}

#[test]
fn rejects_unknown_with_field() {
    assert!(matches!(elaborate_source(TEMPLATE_WITH_UNKNOWN_FIELD), Err(Diagnostic::UnknownEntityField { .. })));
}
```

- [ ] **Step 2: Verify template tests fail**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 template_with_expands_to_typed_note rejects_unknown_with_field
```

Expected: parser rejects `template`/`return`, or elaborator reports missing template support.

- [ ] **Step 3: Implement constructor/template elaboration**

Parse `template Type name(parameters) { statements }`, `return`, constructor expressions and `base with { field: value; }`. Type-check template parameters and return expression. Validate each constructor field against the immutable schema, require Note `gameplay.time`, reject duplicate field paths and reject constructors for unavailable schemas.

Implement template dependency graph checking before instantiation. Report `Diagnostic::RecursiveTemplate` with the complete template chain. Statement-level `if` is accepted only after its condition evaluates to compile-time `bool`; false branches produce no entity and runtime names produce `Diagnostic::NonConstantStructuralCondition`.

- [ ] **Step 4: Verify template behavior and no v4 regression**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: template tests, all Phase 1 tests and v4 core tests pass.

- [ ] **Step 5: Commit typed template expansion**

```text
git add crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(core): elaborate FCS 5 typed templates"
```

## Milestone P2.3：collection generator 与 emit

### Task 6: Parse collections, ranges and generator syntax

**Files:**

- Modify: `crates/fcs-core/src/v5/parser/document.rs`
- Modify: `crates/fcs-core/src/v5/parser/entity.rs`
- Modify: `crates/fcs-core/src/v5/ast/entity.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: Write failing beat-generator parser tests**

Add a test that parses the approved half-open syntax:

```rust
#[test]
fn parses_beat_generator_without_float_accumulation_syntax() {
    let document = parse_document(r#"#fcs 5.0.0
format { profile: fragment; }
notes { generate at: beat in 0beat..<4beat step 1beat { emit tap { gameplay.time: at; }; } }"#).unwrap();
    assert_eq!(document.collections.len(), 1);
}
```

- [ ] **Step 2: Verify the parser test fails**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 parses_beat_generator_without_float_accumulation_syntax
```

Expected: parser rejects the `notes` collection or `generate` keyword.

- [ ] **Step 3: Implement collection and range AST/parser**

Extend `Document` with `collections: Vec<CollectionBlock>`. Parse registered collection names, `generate variable: Type in start..<end step value`, `start..=end`, `emit expression`, local `let`, and compile-time `if` blocks. Reject generator syntax in definitions/template/function parser contexts and reject a generator nested inside another generator during parsing.

Represent inclusive/exclusive range end explicitly; do not convert range endpoints to `f64`.

- [ ] **Step 4: Run parser tests**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: generator syntax test passes and all existing tests remain green.

- [ ] **Step 5: Commit collection parser support**

```text
git add crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(parser): parse FCS 5 generators"
```

### Task 7: Expand generators exactly and type-check emit

**Files:**

- Create: `crates/fcs-core/src/v5/elaborator/generator.rs`
- Modify: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Modify: `crates/fcs-core/src/v5/ast/entity.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: Write failing expansion tests**

Add tests for half-open beat expansion and emit type checking:

```rust
#[test]
fn expands_half_open_beat_range_exactly() {
    let expanded = elaborate_source(GENERATED_TAPS).unwrap();
    let notes = &expanded.collections[0].entities;
    assert_eq!(notes.len(), 4);
    assert_eq!(notes[3].field("gameplay.time").unwrap().value, TypedValue::Beat(Beat::new(3, 1).unwrap()));
}

#[test]
fn rejects_line_emitted_in_notes_collection() {
    assert!(matches!(elaborate_source(WRONG_EMIT_TYPE), Err(Diagnostic::WrongCollectionEmitType { .. })));
}
```

- [ ] **Step 2: Verify expansion tests fail**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 expands_half_open_beat_range_exactly rejects_line_emitted_in_notes_collection
```

Expected: elaborator does not yet expand generators or does not enforce collection type.

- [ ] **Step 3: Implement exact range expansion**

Evaluate range start/end/step as compile-time values. Accept only `int` and `beat`, require equal endpoint/step types, reject zero step, and calculate each value as `start + index × step`; for beat use `Beat::checked_add` and rational multiplication rather than repeated floating-point addition.

Bind `index`、`range.start`、`range.end`、`range.step`、`range.count` in a child immutable scope. Elaborate each `emit` expression to an entity and require an exact match to the collection schema emitted type. Generators are not nested and their bodies may not capture mutable state because no mutable state exists.

- [ ] **Step 4: Verify generator expansion**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: exact range and wrong-emit tests pass.

- [ ] **Step 5: Commit generator expansion**

```text
git add crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(core): expand FCS 5 generators"
```

## Milestone P2.4：limits、trace 与 expanded source output

### Task 8: Enforce limits and produce ExpandedSourceDocument

**Files:**

- Modify: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Create: `crates/fcs-core/src/v5/elaborator/budget.rs`
- Modify: `crates/fcs-core/src/v5/ast/entity.rs`
- Test: `crates/fcs-core/tests/fcs5_phase2.rs`

- [ ] **Step 1: Write failing budget and lowering tests**

Add tests:

```rust
#[test]
fn reports_generator_budget_with_index_trace() {
    let limits = CompileTimeLimits { max_generator_iterations: 2, ..CompileTimeLimits::default() };
    let error = elaborate_source_with_limits(THREE_TAPS, limits).unwrap_err();
    assert!(matches!(error, Diagnostic::BudgetExceeded { kind: BudgetKind::GeneratorIterations, trace } if trace.contains("index=2")));
}

#[test]
fn expanded_document_contains_no_compile_time_nodes() {
    let expanded = elaborate_source(GENERATED_TAPS).unwrap();
    assert!(expanded.collections.iter().all(|collection| collection.entities.iter().all(ExpandedEntity::is_lowered)));
}
```

- [ ] **Step 2: Verify budget/lowering tests fail**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 reports_generator_budget_with_index_trace expanded_document_contains_no_compile_time_nodes
```

Expected: limits are absent or expanded entities still contain source expressions/template references.

- [ ] **Step 3: Add limits, trace and final output validation**

Implement `CompileTimeLimits` defaults for all six approved limits. Count expression nodes during type checking, operations during evaluator execution, template instances/depth during template expansion, and generated entities/iterations during generator expansion.

Each `Diagnostic::BudgetExceeded` contains the budget kind plus an ordered trace containing function/template names, collection kind, range and current generator index. Ensure the final conversion into `ExpandedSourceDocument` owns only concrete typed values and `ExpandedEntity` records; reject any unresolved expression, source template call or generator node at the conversion boundary.

- [ ] **Step 4: Verify limits and lowered IR**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: budget trace and no-compile-time-node tests pass.

- [ ] **Step 5: Commit limits and expanded source IR**

```text
git add crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_phase2.rs
git commit -m "feat(core): lower FCS 5 compile-time language"
```

### Task 9: Add public fixture, status documentation and full verification

**Files:**

- Create: `examples/fcs/fcs5-compile-time.fcs`
- Modify: `crates/fcs-core/tests/fcs5_phase2.rs`
- Modify: `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md`

- [ ] **Step 1: Write a failing fixture-loading test**

Add this public test to `crates/fcs-core/tests/fcs5_phase2.rs`:

```rust
#[test]
fn parses_and_elaborates_public_compile_time_fixture() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/fcs/fcs5-compile-time.fcs");
    let source = std::fs::read_to_string(path).unwrap();
    let document = fcs_core::v5::parser::parse_document(&source).unwrap();
    let expanded = fcs_core::v5::elaborator::elaborate(
        &document,
        &fcs_core::v5::schema::phase2_schema(),
        fcs_core::v5::elaborator::CompileTimeLimits::default(),
    )
    .unwrap();

    assert_eq!(expanded.collections.len(), 1);
    assert_eq!(expanded.collections[0].entities.len(), 4);
    assert!(expanded.collections[0]
        .entities
        .iter()
        .all(|entity| entity.entity_type == fcs_core::v5::ast::Type::Note));
}
```

- [ ] **Step 2: Verify fixture test fails because the file is absent**

Run:

```text
cargo nextest run -p fcs-core --test fcs5_phase2 parses_and_elaborates_public_compile_time_fixture
```

Expected: fixture file-not-found failure.

- [ ] **Step 3: Add the public compile-time fixture**

Create `examples/fcs/fcs5-compile-time.fcs` with exactly:

```fcs
#fcs 5.0.0
format { profile: fragment; }

definitions {
    const OFFSET: length = 120px;

    fn position(index: int) -> length {
        return toFloat(index) * OFFSET;
    }

    template Note baseTap(hitTime: beat, x: length) {
        return tap {
            gameplay.time: hitTime;
            presentation.positionX: x;
        };
    }

    template Note accentTap(hitTime: beat, x: length) {
        return baseTap(hitTime, x) with {
            presentation.scaleX: 1.25;
            presentation.color: #FFAA00FF;
        };
    }
}

notes {
    generate at: beat in 0beat..<4beat step 1beat {
        emit accentTap(at, position(index));
    }
}
```

- [ ] **Step 4: Run all required verification**

Run in this exact order:

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
```

Expected: all commands exit with code `0`.

- [ ] **Step 5: Update roadmap status and commit completion**

Change Phase 2 status in `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md` from `Not started` to `Complete`, then run:

```text
git add crates/fcs-core examples/fcs/fcs5-compile-time.fcs docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md
git commit -m "docs: complete FCS 5 Phase 2"
```

## Plan self-review

- Spec coverage: Tasks 1–3 implement P2.1, Tasks 4–5 implement P2.2, Tasks 6–7 implement P2.3, and Tasks 8–9 implement P2.4 plus public fixture and Phase status.
- Type consistency: `elaborate`, `ConstructionSchema`, `CompileTimeLimits`, `ExpandedSourceDocument`, `TypedValue` and `Diagnostic` use the same names and responsibilities as the approved Phase 2 design.
- Scope: runtime chart semantics, expression DAG, FCBC, CLI and converter migrations are deliberately absent; only source construction contracts are registered.
