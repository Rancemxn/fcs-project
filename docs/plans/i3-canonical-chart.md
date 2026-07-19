# I3.8–I3.9 CanonicalChart API and test snapshot

Status: I3.8 is merged. I3.9 implementation adds a test-only snapshot adapter
and golden; its delivery remains gated by the repository full gate, Primary
Self-Audit, and the Issue/PR workflow. This plan does not claim complete
CanonicalCompilation, runtime evaluation, or resource payload resolution.

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
- Keep the I3.9 JSON boundary under `crates/fcs-source/tests/support/`. The
  adapter explicitly projects every current `CanonicalChart` field through
  public read-only accessors and pretty-prints a `serde_json::Value`; it does
  not add serde derives, JSON dependencies, methods, or representation choices
  to `fcs-model`.
- Preserve normative array order, including Line stable-ID map order and
  topological order, Note gameplay order, Track order and piece order, scroll
  Line order, tempo-point order, credits, required extensions, and ordered
  custom-object entries. Ordinary object keys use the default BTreeMap-backed
  `serde_json::Map` from the workspace-locked 1.0.150 source snapshot.
- Emit stable IDs with namespace, textual spelling, and unsigned value; emit
  Beat values as exact numerator/denominator pairs; and emit canonical finite
  Float64 values as JSON numbers without a `Debug` fallback.

## Explicit non-goals

This work does not add resource bytes or `CanonicalResourceBundle`,
`DistributionMetadata`, a product or normative canonical JSON format, missing
runtime property descriptors/evaluation, FCBC writer/loader/ABI, Render,
Conversion, or CLI behavior. It does not carry source AST, spans, comments,
authoring definitions, workspace paths, preserve payloads, or source snapshots
into `fcs-model`.

## Acceptance evidence

- `canonical_chart::canonical_chart_aggregates_current_i3_products_and_identity`
  covers every aggregate field and a required extension identity.
- `canonical_chart::canonical_chart_is_stable_when_top_level_declarations_are_reordered`
  covers the specification's unordered top-level declaration boundary.
- `canonical_chart_snapshot::direct_and_template_authoring_produce_the_checked_in_canonical_snapshot`
  proves the direct and template/preserve fixtures lower to byte-identical
  pretty snapshots matching one checked-in golden.
- `canonical_chart_snapshot::canonical_snapshot_excludes_authoring_and_workspace_state`
  recursively rejects source text, workspace paths, spans, template/generator/
  local authoring nodes, and preserve payload state at the snapshot boundary.
- The exact PR head must pass the repository full gate; local validation remains
  limited to formatting, diff, Markdown, and other non-building checks.
