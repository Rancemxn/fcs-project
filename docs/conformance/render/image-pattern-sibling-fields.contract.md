# Render ImagePattern sibling-fields contract

- **Fixture ID:** `render.source.contract.image-pattern-sibling-fields`
- **Status:** checked-in specification contract; not an active executable fixture
- **Normative clauses:** Render §§8.1, 8.2, 10, 15.2, and 17
- **Review record:** [`docs/reviews/2026-08-23-render-spec-amendments-2-0-3-corrective.md`](../../reviews/2026-08-23-render-spec-amendments-2-0-3-corrective.md)
- **Activation owner:** I9 Render source/canonical ImagePattern-lowering unit
- **Activation condition:** register this source in `manifest.toml` and add machine semantic/diagnostic expected output in the same implementation change, then run the applicable same-head Full Gate.

The companion [`image-pattern-sibling-fields.fcs`](image-pattern-sibling-fields.fcs) is a complete source example. It is intentionally not listed in the active `source_fixture` array: the current implementation does not lower source `imagePattern` sibling fields, so registering it now would make the existing executable lane red. This contract records the required future evidence without claiming parser, canonical, loader, semantic, raster, or Full Gate success.

## Required expected contract

When activated, the fixture must prove all of the following:

1. The empty `dash` array is accepted as a complete solid stroke. It performs no total validation, phase normalization, or dash-element traversal; cap and join remain governed by the rectangle's closed geometry.
2. The one source-level `pattern*` configuration is copied into two independent Paint records. The fill Paint references `linearSprite`; the stroke Paint references `nearestSprite`. No second fill/stroke-specific pattern configuration is present or accepted.
3. Because `patternSampling` is omitted, the fill Paint resolves `sampling = linear` from `linearSprite` and the stroke Paint resolves `sampling = nearest` from `nearestSprite`. The defaults are per-resource, not a shared fallback.
4. If either referenced image resource lacks a valid canonical `sampling` metadata field, activation must produce the stable `render.resource-decode-failed` diagnostic rather than guess or borrow the other resource's value.
5. If a future fixture supplies a geometry with a normative exact arclength, dash placement must use that exact arclength. Other parametric curves must use the normative `1/1024` sagitta, depth-32 flattening rule; an implementation may not select either algorithm arbitrarily.

No expected JSON, semantic draw list, raster golden, or diagnostic assertion is checked in here because those are implementation-owned outputs and this PR does not implement the activation owner.
