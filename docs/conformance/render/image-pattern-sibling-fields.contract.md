# Render ImagePattern sibling-fields contract

- **Fixture ID:** `render.source.contract.image-pattern-sibling-fields`
- **Status:** active source/product fixture; exact-head Full Gate pending
- **Normative clauses:** Render §§8.1, 8.2, 10, 15.2, and 17
- **Review record:** [`docs/reviews/2026-08-23-render-spec-amendments-2-0-3-corrective.md`](../../reviews/2026-08-23-render-spec-amendments-2-0-3-corrective.md)
- **Product evidence:** `source_product::source_image_pattern_sibling_fields_reach_product_raster`
- **Diagnostic evidence:** `canonical::tests::invalid_canonical_image_sampling_uses_the_render_diagnostic`

The companion [`image-pattern-sibling-fields.fcs`](image-pattern-sibling-fields.fcs) is registered in the active `source_fixture` array and is consumed directly by the product regression above. The regression checks source/canonical lowering, independent fill/stroke Paint records, per-resource sampling, shared exact pattern descriptors, FCBC writer/loader, semantic payloads, and raster coverage. The separate `render.image-pattern-fill-4x4` and `render.image-pattern-stroke-4x2` manifest fixtures supply machine semantic/raster oracles for ImagePattern fill and Line Stroke payloads. Same-head Full Gate and final Render closure are not claimed.

## Required expected contract

The active fixture proves all of the following:

1. The non-empty Rect `dash` array is accepted. Its exact closed subpath starts at `origin`, visits the upper-left, upper-right, and lower-right corners in that order, then closes to `origin`; this is clockwise in FCS Y-up coordinates. The `[1px, 2px]` sequence and `dashOffset` use exact straight-segment arclength, while cap and join remain governed by the rectangle's closed geometry.
2. The one source-level `pattern*` configuration is copied into two independent Paint records. The fill Paint references `linearSprite`; the stroke Paint references `nearestSprite`. No second fill/stroke-specific pattern configuration is present or accepted.
3. Because `patternSampling` is omitted, the fill Paint resolves `sampling = linear` from `linearSprite` and the stroke Paint resolves `sampling = nearest` from `nearestSprite`. The defaults are per-resource, not a shared fallback.
4. If either referenced image resource lacks a valid canonical `sampling` metadata field, activation must produce the stable `render.resource-decode-failed` diagnostic rather than guess or borrow the other resource's value.
5. Rect dash placement uses exact line-segment arclength. Other geometry with a normative exact arclength must likewise use it; remaining parametric curves use the normative `1/1024` sagitta, depth-32 flattening rule. An implementation may not select either algorithm arbitrarily.

The machine semantic assertions live in the product regression rather than a duplicate JSON projection; the focused canonical unit assertion binds malformed sampling metadata to `render.resource-decode-failed`.
