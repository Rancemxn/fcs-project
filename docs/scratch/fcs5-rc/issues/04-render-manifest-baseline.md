# 04 — Restore an honest Render manifest baseline

Type: task

Status: resolved

Blocked by: 03

## Question

Does the Render conformance manifest name only artifacts that exist and are executable at the current pre-I1
stage while preserving later I9 obligations?

## Acceptance criteria

1. Entries referencing absent nonempty Render golden/semantic/raster/mutation files are removed from the current
   manifest instead of being satisfied with empty placeholders.
2. Existing PNG/WebP/TTF assets and writer/loader/decoder/shaping tests remain covered.
3. The roadmap/review/matrix records `image` and `serde_json` as pre-stage conformance test-only activation; product
   ownership remains I9 and I3/I6 respectively.
4. Manifest integrity and focused Render tests pass after Clippy.
5. Full semantic/raster/mutation artifact remains an explicit I9 acceptance item.

## Comments

- This unit must not create `fcbc_render_reference_evaluator.rs` or rasterizer stubs merely to satisfy paths.
- 2026-07-16: implementation is prepared while ticket 03 remains claimed. The nonexistent `binary_fixture`
  entry is removed, the typed manifest defaults the absent array to empty and asserts length 0, PNG/WebP/TTF
  assets plus writer/loader/decoder/shaping tests remain, and the 16-test focused lane passes. Resolve this ticket
  only after its declared blocker 03 is independently reviewed/resolved.

## Answer

Yes. The manifest now names no absent nonempty Render artifact and parses the omitted `binary_fixture` array as
an explicit empty list. Project-owned assets and executable writer/loader/decoder/shaping evidence remain. The
roadmap, review and implementation matrix record `image`/`serde_json` as pre-stage dev-only exceptions while
retaining I3/I6/I9 product ownership, and the full semantic/raster/mutation artifact remains an I9 acceptance item.
