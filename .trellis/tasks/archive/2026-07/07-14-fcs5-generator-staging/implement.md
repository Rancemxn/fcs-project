# FCS 5 generator staging boundary implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce `..<`/`..=` generator syntax and make I0 generator elaboration fail explicitly
before source-crate cutover.

**Architecture:** Keep generator nodes in the current `fcs-core/src/v5` source AST. Make the parser
strict about the two Frozen range operators and leave zero-step meaning to I2. Add one elaborator
staging error so no generator can produce a partial expanded document in I0.

**Tech Stack:** Rust 2024, existing Phase 2 parser/elaborator, cargo-nextest, Clippy, rustfmt.

---

## Task 1: Establish the red tests

**Files:**

- Modify: `crates/fcs-core/tests/fcs5_phase2.rs`

- [x] **Step 1: Add a complete generator source helper and Frozen range tests**

Replace the existing bare-range test with a helper and tests equivalent to:

```rust
fn generator_source(operator: &str, step: &str) -> String {
    format!(
        "#fcs 5.0.0\n\
         format {{ profile: fragment; }}\n\
         collections {{\n\
           notes {{\n\
             generate at: beat in 0beat{operator}4beat step {step} {{\n\
               emit tap {{ gameplay.time: at; }};\n\
             }}\n\
           }}\n\
         }}"
    )
}

#[test]
fn parses_only_frozen_generator_range_operators() {
    for (operator, inclusive_end) in [("..<", false), ("..=", true)] {
        let source = generator_source(operator, "1beat");
        let document = parse_document(&source).expect("Frozen generator range should parse");
        let CollectionItem::Generator(generator) = &document.collections[0].items[0] else {
            panic!("expected generator collection item");
        };
        assert_eq!(generator.range.inclusive_end, inclusive_end);
        assert_eq!(generator.body.len(), 1);
    }
}

#[test]
fn rejects_bare_generator_range_operator() {
    let source = generator_source("..", "1beat");
    assert_eq!(
        parse_document(&source),
        Err(ParseError::InvalidSyntax("generator range"))
    );
}

#[test]
fn retains_zero_generator_step_for_later_static_semantics() {
    let source = generator_source("..<", "0beat");
    let document = parse_document(&source).expect("zero step is syntactically valid");
    let CollectionItem::Generator(generator) = &document.collections[0].items[0] else {
        panic!("expected generator collection item");
    };
    assert!(matches!(
        generator.range.step,
        SourceExpression::Literal {
            literal: SourceLiteral::Beat(value),
            ..
        } if value == Beat::new(0, 1).unwrap()
    ));
}
```

- [x] **Step 2: Add the red elaborator staging test**

Add this test after the parser range tests:

```rust
#[test]
fn generator_elaboration_fails_before_partial_output() {
    let source = format!(
        "#fcs 5.0.0\n\
         format {{ profile: fragment; }}\n\
         collections {{\n\
           notes {{\n\
             tap {{ gameplay.time: 0beat; }};\n\
             generate at: beat in 1beat..<3beat step 1beat {{\n\
               emit tap {{ gameplay.time: at; }};\n\
             }}\n\
           }}\n\
         }}"
    );
    let document = parse_document(&source).expect("generator source should parse");
    let error = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect_err("I0 must not expand generators");
    assert!(matches!(
        error,
        Diagnostic::FeatureUnavailable {
            feature: "compile-time-generator",
            span,
        } if span.start == source.find("generate").unwrap()
    ));
}
```

Expected red state: compilation fails on the existing non-exhaustive generator match and the
missing `FeatureUnavailable` variant. If the test fails for a typo, fix the test until the failure
targets the missing staging behavior before writing production code.

- [x] **Step 3: Run the targeted tests and observe the intended red state**

`cargo nextest run -p fcs-core --test fcs5_phase2` reproduced the existing non-exhaustive
`CollectionItem::Generator` compile error before production changes.

## Task 2: Implement the minimal parser boundary

**Files:**

- Modify: `crates/fcs-core/src/v5/parser/entities.rs`

- [x] **Step 1: Replace compatibility range parsing**

Replace the existing `..=` / `..` plus optional `<` branch with the exact operator branch from
`design.md`:

```rust
let inclusive_end = if cursor.take_text("..<") {
    false
} else if cursor.take_text("..=") {
    true
} else {
    return Err(ParseError::InvalidSyntax("generator range"));
};
```

- [x] **Step 2: Remove parser-owned zero-step validation**

Delete the `is_literal_zero` helper and its invocation. Do not replace it with a different parser
check. The zero-step test must now pass through AST construction.

- [x] **Step 3: Run the targeted build and record the remaining red state**

Run:

```powershell
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: the build remains red only at the missing `FeatureUnavailable`/generator elaborator arm;
do not add a wildcard arm or any temporary production workaround just to run the tests. Task 3
closes that compile boundary and provides the first green test run.

## Task 3: Implement the explicit elaborator staging error

**Files:**

- Modify: `crates/fcs-core/src/v5/elaborator/mod.rs`
- Modify: `crates/fcs-core/src/v5/elaborator/entities.rs`

- [x] **Step 1: Add the temporary diagnostic variant**

Add to the existing `Diagnostic` enum:

```rust
FeatureUnavailable {
    feature: &'static str,
    span: SourceSpan,
},
```

- [x] **Step 2: Add the generator match arm**

In `ExpansionContext::expand_item`, add:

```rust
CollectionItem::Generator(generator) => {
    return Err(Diagnostic::FeatureUnavailable {
        feature: "compile-time-generator",
        span: generator.span,
    });
}
```

Do not call any range evaluator or append an entity before returning this error.

- [x] **Step 3: Run the targeted tests and verify green**

Run:

```powershell
cargo nextest run -p fcs-core --test fcs5_phase2
```

Expected: the parser boundary and elaborator staging tests pass; existing non-generator tests
remain green.

## Task 4: Run repository gates and inspect the diff

- [x] **Step 1: Format and lint**

Run in repository order:

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

- [x] **Step 2: Run the complete test suite**

```powershell
cargo nextest run --workspace
```

- [x] **Step 3: Run final formatting and diff checks**

```powershell
cargo fmt --all -- --check
git diff --check
```

- [x] **Step 4: Verify scope and prohibited changes**

```powershell
git diff --name-only
rg -n 'FeatureUnavailable|compile-time-generator|take_text\("\.\."\)|is_literal_zero' crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_phase2.rs
```

Expected changed files are limited to the test, parser, and elaborator files named above. The
search must not find the removed `is_literal_zero` helper or a compatibility `take_text("..")`
branch; it must find the explicit feature variant, feature string, and tests.

- [x] **Step 5: Commit the staging boundary**

```powershell
git add crates/fcs-core/src/v5/parser/entities.rs crates/fcs-core/src/v5/elaborator/mod.rs crates/fcs-core/src/v5/elaborator/entities.rs crates/fcs-core/tests/fcs5_phase2.rs
git diff --cached --check
git commit -m "fix(source): make generator staging explicit"
```

## Stop conditions and rollback

- Stop if an unknown file becomes dirty; do not include it in the commit.
- Stop if a parser test passes before the parser change; revise the test so it proves the old
  behavior is wrong.
- Stop if zero-step behavior is implemented in the parser; revert that semantic check and leave it
  for I2.
- Stop if elaboration returns a partial document or a different feature spelling.
- If a gate fails for unrelated pre-existing code, record the exact command/output and keep the
  Task 2 diff isolated; do not broaden scope to the later diagnostic or crate migration tasks.
