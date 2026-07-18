# I3.2 Exact Beat and Tempo Normalization

Status: implementation complete on the I3.2 branch; delivery remains gated by the repository full gate,
Primary Self-Audit, and the Issue/PR workflow. This plan records the bounded implementation contract and
does not change `docs/loops/loop.md` or promote any specification version to Reviewed/Frozen.

## Normative dependency closure

- `docs/specifications/fcs.md` §§8.1–8.3: the single physical `chartTime`, global `chartBeat`, tempo-map
  integration, negative/final extrapolation, same-beat step semantics, inverse mapping, audio offset, and
  separation from `lineScrollCoordinate`.
- `docs/decisions/0001-single-runtime-clock.md` and `docs/decisions/0002-judgment-time-and-scroll-coordinate.md`.
- `docs/decisions/0010-stage-scoped-implementation-baselines.md` and the I3.1 merged baseline at
  `fd473c7d3cabf3f7ae1015f23578c7ba2d5e65fe`.
- `docs/conformance/fcs5/expected/numeric-vectors.toml` and the `time-scroll-note` and canonical-equivalence
  source fixtures.

## Owned surface

- Add an immutable `fcs-model` exact `Beat`, validated piecewise `ChartTimeMap`, `CanonicalTime`, and
  finite `AudioOffset` API.
- Convert expanded source tempo points into the canonical map without silently changing parser ownership.
- Convert Note `gameplay.time` from exact source Beat to canonical chartTime while retaining exact Beat
  provenance and stable Note identity.
- Preserve the explicit optional Note `id` field required by FCS §12.2 and connect it to the I3.1 identity
  seam.

## Explicit non-goals

This work does not implement line-local clocks, scroll-distance integration, Track semantics, complete Note
gameplay/presentation lowering, metadata/resources, conversion semantic profiles, runtime descriptors, FCBC,
Render, CLI, or release/Frozen claims. No source parser rejection is added for tempo legality; invalid tempo
maps remain parser-accepted and fail at canonical validation.

## Acceptance evidence

- Exact piecewise mapping and unique inverse, including zero, negative, and final-point extrapolation.
- Last-point-wins behavior for same-beat tempo steps.
- Finite-input overflow rejection at audio/inverse boundaries.
- Parser acceptance followed by stable `TempoError` canonical rejection for invalid BPM and non-monotonic
  points.
- Direct, template, generator, and source-reordered Note fixtures produce equivalent timing when compared by
  stable identity; exact source Beat remains observable as provenance.
- Focused and full Rust gates are recorded in the Issue, PR, and Primary Self-Audit comments.
