# I3.7 Canonical Scroll Model

Status: implementation complete on the I3.7 branch; delivery remains gated by
the repository full gate, Primary Self-Audit, and the Issue/PR workflow. This plan
records the bounded canonical seam and does not claim I4 evaluator or runtime
descriptor completion.

## Normative dependency closure

- `docs/specifications/fcs.md` §§8.3, 10.1–10.4, and 11.2: line-local `q`,
  scroll tempo normalization,
  direct piecewise integration, speed policy, floor origin, and floor scale.
- `docs/decisions/0001-single-runtime-clock.md` and
  `docs/decisions/0002-judgment-time-and-scroll-coordinate.md`.
- `docs/decisions/0010-stage-scoped-implementation-baselines.md` and the
  merged I3.6 baseline.

## Owned surface

- Add immutable `CanonicalScrollCoordinate`, `CanonicalScrollLine`, and
  `CanonicalScrollSet` values.
- Normalize global, beat-key, and time-key scroll tempo maps into chart-time
  points.
- Provide exact direct-seek piecewise integration with `q(0)=0`, negative-time
  and final-BPM extrapolation, duplicate-key final-value behavior, and stable
  Line ordering.
- Preserve `floorScale`, `integrationOrigin`, `initialFloorPosition`, and
  reverse-scroll policy at the canonical Line boundary.
- Bind `source.valid.time-scroll-note` to this canonical boundary without
  claiming dynamic evaluation.

## Explicit non-goals

This work does not implement dynamic `scrollSpeed`, easing or Track evaluation,
`EnvQ`/`EnvD` dependency analysis, runtime descriptors, FCBC Distance/ABI, Note
local-Y evaluation, or sampled floor-position caches.

## Acceptance evidence

- Model tests cover direct seek, zero, negative/final extrapolation, duplicate
  keys, zero/reverse speed policy, and stable set ordering.
- Source tests cover global and beat-key normalization plus Line base policy values.
- The conformance manifest test executes `source.valid.time-scroll-note` through
  the canonical scroll boundary.
