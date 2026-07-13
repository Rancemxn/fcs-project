# FCS 5 Phase 2 Typed Templates Design

## Scope

This milestone implements the Phase 2 construction surface needed to turn a
source document into non-empty lowered entities. It covers `template`
declarations, Note/Line constructors, template calls, and immutable `with`
field overrides. Collection generators, ranges, `generate`, and `emit` remain
out of scope for the following milestone.

## Source model

The document parser accepts an optional `templates` block after `definitions`
and `tempoMap`. A template has a name, typed parameters, an entity return type,
and an entity expression body. Constructors use the registered entity type (for
example `tap { gameplay.time: 1beat; }` or `Note.tap { ... }`). A `with` suffix
replaces fields on the base entity and may be nested. A `collections` block
contains named collections with constructor/template expressions; the existing
Phase 2 collection names `notes` and `judgelines` determine the target entity
type.

The source AST preserves byte spans and source expressions. Template calls are
represented as entity expressions rather than being evaluated by the general
scalar expression evaluator.

## Elaboration

Elaboration proceeds in deterministic order:

1. Check definitions and reject recursive templates before instantiation.
2. Validate each constructor against `ConstructionSchema` (entity type,
   variant, field path, duplicate path, required fields, and value type).
3. Resolve template calls, bind typed arguments, recursively elaborate the
   template body, and apply `with` overrides from inner to outer.
4. Evaluate all field expressions through the existing compile-time evaluator.
5. Emit `ExpandedEntity` records with only concrete `TypedValue`s, preserving
   source spans and deterministic field ordering.
6. Group expanded entities into `ExpandedCollection`s in source order.

Templates are immutable and cannot mutate caller fields. A template cycle is a
diagnostic containing the complete call chain. A `with` expression cannot add
unknown fields, override a field twice in one block, or change the field type.
Required fields are checked after all overrides are applied.

## Diagnostics and limits

Add dedicated diagnostics for unknown entity fields, duplicate fields, missing
required fields, non-constructible entities, recursive templates, and invalid
template arity/type. Structural conditions are limited to compile-time boolean
expressions; runtime-only names remain rejected by the existing evaluator.
Template depth and instance limits are counted during expansion and use the
existing `CompileTimeLimits` fields.

## Verification

Tests will cover successful constructor/template/`with` expansion, field
validation failures, template recursion, compile-time condition behavior, and
non-empty lowered collections. Existing Phase 1, v4, Clippy, nextest, and
rustfmt checks must remain green.
