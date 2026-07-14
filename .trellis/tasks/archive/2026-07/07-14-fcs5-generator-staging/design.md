# Design: explicit compile-time generator staging

## Boundary and data flow

```text
source text
   │
   ▼
parse_generator
   │  accepts `..<` / `..=`; retains zero-step syntax
   ▼
CollectionItem::Generator
   │
   ▼
ExpansionContext::expand_item
   │  returns FeatureUnavailable before expansion/output
   ▼
Err(Diagnostic)
```

Generators remain represented in the source AST so later I1/I2 work can inspect their range, body,
and spans. I0 must not turn them into `ExpandedEntity` values and must not silently drop them.

## Parser change

At the range operator boundary in `parse_generator`, use longest/exact matching for the two Frozen
operators:

```rust
let inclusive_end = if cursor.take_text("..<") {
    false
} else if cursor.take_text("..=") {
    true
} else {
    return Err(ParseError::InvalidSyntax("generator range"));
};
```

The existing `until_range_operator` may continue to locate the first two-dot sequence in the
expression text. The operator branch is responsible for rejecting bare `..`; no `<` should be
consumed as an optional compatibility suffix. Remove `is_literal_zero` and its call. This keeps
syntax parsing separate from I2's zero-step semantics.

## Elaborator change

Extend the existing Phase 2 diagnostic enum in `elaborator/mod.rs`:

```rust
FeatureUnavailable {
    feature: &'static str,
    span: SourceSpan,
},
```

Add the generator branch in `ExpansionContext::expand_item`:

```rust
CollectionItem::Generator(generator) => {
    return Err(Diagnostic::FeatureUnavailable {
        feature: "compile-time-generator",
        span: generator.span,
    });
}
```

This branch performs no range evaluation, condition evaluation, template lookup, or output push.
If a constructor precedes the generator, the local vector is discarded when `Err` is returned; the
public `Result<ExpandedSourceDocument, Diagnostic>` therefore exposes no partial document.

## Tests and compatibility

Keep tests in the existing `fcs5_phase2.rs` integration test so they use the current public Phase 2
API. Replace the old bare-range test with a helper that generates a complete fragment document,
then add separate cases for operator flags, bare rejection, zero-step retention, and elaborator
staging. The test should assert the generator span begins at the `generate` keyword.

No new dependency, public module, crate, feature flag, or compatibility alias is introduced. The
temporary enum variant is deliberately compatible with the later diagnostic boundary and is not a
claim that the final stable diagnostic API has been implemented.

## Rollback and risk

- The parser change is local to `parse_generator`; if a test exposes an unrelated expression parsing
  regression, revert only the operator/check changes and keep the test as the failure evidence.
- The elaborator arm changes behavior only for generator collections, which previously could not
  be handled completely. Existing constructor, conditional, and template paths remain unchanged.
- If the full workspace is already unable to compile for a pre-existing reason, record the exact
  failure and separate it from the Task 2 diff; do not broaden the task to repair unrelated code.
