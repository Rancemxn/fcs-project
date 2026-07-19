# I3.8 Immutable CanonicalChart API

Status: implementation complete on the I3.8 branch; delivery remains gated by
the repository full gate, Primary Self-Audit, and the Issue/PR workflow. This
plan records the aggregate API and does not claim complete CanonicalCompilation,
runtime evaluation, resource payload resolution, or snapshot completion.

## Normative dependency closure

- `docs/specifications/fcs.md` §§1.1 and 17: CanonicalChart ownership,
  source-free fields, and the authoring/canonical boundary.
- `docs/decisions/0001-single-runtime-clock.md`,
  `docs/decisions/0002-judgment-time-and-scroll-coordinate.md`, and
  `docs/decisions/0010-stage-scoped-implementation-baselines.md`.
- The merged I3.1–I3.7 identity, time, metadata, Line, Note, Track, and scroll
  products.

## Owned surface

- Add model-owned immutable source version, profile/features, required
  extension identity, and `CanonicalChart` values.
- Aggregate the global chart-time map, metadata, Line graph, Note set, Track
  set, and Scroll set behind read-only accessors.
- Add one source compiler seam that performs elaboration internally and invokes
  existing canonical lowerers in dependency order. This API shape prevents a
  parsed document from being paired with another document's expanded output.
- Preserve the deterministic ordering already owned by each canonical
  component and prove equality when semantically unordered top-level blocks are
  rearranged.

## Explicit non-goals

This work does not add resource bytes or `CanonicalResourceBundle`,
`DistributionMetadata`, canonical JSON snapshots, missing runtime property
descriptors/evaluation, FCBC writer/loader/ABI, Render, Conversion, or CLI
behavior. It does not carry source AST, spans, comments, authoring definitions,
workspace paths, preserve payloads, or source snapshots into `fcs-model`.

## Acceptance evidence

- `canonical_chart::canonical_chart_aggregates_current_i3_products_and_identity`
  covers every aggregate field and a required extension identity.
- `canonical_chart::canonical_chart_is_stable_when_top_level_declarations_are_reordered`
  covers the specification's unordered top-level declaration boundary.
- The exact PR head must pass the repository full gate; local validation remains
  limited to formatting, diff, Markdown, and other non-building checks.
