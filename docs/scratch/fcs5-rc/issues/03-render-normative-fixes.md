# 03 — Close Render normative findings

Type: task

Status: resolved

Blocked by: 01

## Question

Can `RNR-C01`–`RNR-I08` be made single-valued across Core, FCBC/ABI, Render diagnostics and the reference
loader without expanding into the full I9 raster artifact?

## Acceptance criteria

1. D1–D4 are stated normatively in the owning specifications.
2. RNR-I01/I02/I03/I04/I07/I08 have unique validation phase, precedence and stable category.
3. The reference loader enforces `1 <= glyphId < numGlyphs` and returns `render.invalid-geometry` for semantic
   glyph violations.
4. Focused executable tests cover the currently implemented scroll/category/glyph/asset boundaries; the
   normative fixture contract covers active/visibility, attachment geometry/style isolation, zero-length stroke
   and shared-owner intersection without manufacturing the I9 semantic evaluator/rasterizer.
5. The amendment review records every finding as resolved-by-candidate but not closed pending independent review.
6. Clippy and focused nextest pass before the new hash is fixed.

## Comments

- Do not implement the absent full semantic evaluator/rasterizer here; those remain I9-owned.
- 2026-07-16: `RNR-C01`–`RNR-I08` are resolved by candidate normative text and remain pending independent
  review. Candidate hashes are `fcs.md` `2A2882E...DC58`, `fcbc.md` `245E1E...F99A`, and `fcs-render.md`
  `848D9A...B4C3`.
- Focused Clippy and 16-test nextest lane pass. Executable evidence covers scroll environment, Render-owned
  Node/attachment categories, glyph/face bounds, asset decode/shaping and manifest integrity. Full
  active/visibility, attachment geometry, zero-length stroke and shared-owner semantic/raster artifacts remain
  I9-owned and must not be manufactured here.
- Full workspace quality gate passes with 149/149 nextest tests, rustfmt check and `git diff --check`.
- The historical nonempty ABI visibility EnvB root is explicitly routed as an I7 partial exception; it is not an
  I1 source-parser blocker and the test-only loader is not described as a conforming product loader.

## Answer

Yes. The fixed candidate specifications give every RNR finding a single validation phase, dependency order and
stable category. Focused test-only evidence covers the boundaries owned at this pre-I1 stage, while the remaining
semantic/raster executable fixtures stay explicitly owned by I9. A reviewer who did not participate in the
changes verified the three candidate hashes, closed `RNR-C01`–`RNR-I08`, and reported Critical 0, Important 0,
Minor 0. The legacy visibility EnvB artifact is routed to I7 and does not enter I1's parser dependency closure.
