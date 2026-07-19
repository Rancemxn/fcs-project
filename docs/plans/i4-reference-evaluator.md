# I4 Reference Evaluator and Numeric System Plan

## Normative closure

- `docs/specifications/fcs.md` sections 9.4, 13, 14.1, 14.3, and 18.
- `docs/specifications/fcbc.md` sections 13.1, 13.2, and 14.
- ADR 0005, ADR 0009, and ADR 0010.
- `docs/conformance/fcs5/expected/numeric-vectors.toml` and the runtime rows of
  `docs/conformance/fcs5-implementation-matrix.md`.

I4 consumes the immutable Canonical Chart boundary delivered by I3. It owns
exact runtime descriptors and evaluation, not source parsing, authoring
expansion, resource loading, FCBC container validation, Render behavior, or
external-format approximation.

## I4.1 owned surface

I4.1 establishes the product `fcs-runtime` crate and the Core easing boundary:

- the stable ABI mapping for easing IDs `0..=30` and their canonical names;
- finite progress validation over `[0, 1]` and exact endpoint pinning;
- the FCS section 14.3 Sine, polynomial, Expo, Circ, Back, Elastic, and Bounce
  formulas, including the normative out/inOut transforms;
- finite-result validation and stable public errors;
- a private platform math boundary for the transcendental operations that I4.8
  must replace or cross-check with an independent correctly rounded path.

The product API contains only project-owned scalar IDs, values, and errors. It
does not expose a dependency type or reuse the test-only FCBC evaluator.

## I4.2 owned surface

I4.2 adds the first product Track evaluator on the immutable
`fcs_model::CanonicalTrackSet` boundary:

- one owner/target query accepts a finite canonical chart time and an explicit
  typed base value, without importing source AST or schema-default logic;
- point and segment selection preserves half-open boundaries, point shadowing,
  gap fill, before/after extrapolation, and segment-only `holdBefore` /
  `holdAfter` boundaries;
- step, linear, every Core easing name, and cubic Bezier feed the typed
  `Float`, `Angle`, `Vec2Float`, and `Vec2Length` interpolation path;
- replace, add, and multiply layers use the normative priority and owner-local
  stable-name ordering, with finite intermediate/result checks;
- cubic Bezier inversion uses exact floating expansions and adaptive bisection.
  It returns a value only after both endpoint enclosures round to the same
  binary64 bit pattern; unsupported underflow/overflow or an exhausted 192-step
  enclosure budget returns a stable strict error rather than an approximation.

The API exposes only canonical project types and `TrackEvaluationError`.
There is no LUT, fixed Newton solve, host animation curve, sampled cache, or
FCBC/source serialization dependency.

## I4.3 owned surface

I4.3 adds the product Line transform evaluator on the immutable
`CanonicalLineGraph` and `CanonicalTrackSet` boundaries:

- one Line query evaluates position, rotation, scale, and alpha from their
  canonical base values plus the I4.2 Track path at a finite chart time;
- the local column-vector matrix preserves the exact FCS section 11.3 order,
  including a non-zero transform origin and counter-clockwise rotation;
- stable-ID topological traversal recursively composes parent position,
  rotation, scale, and alpha according to each independent inherit flag;
- world components are derived from declared component state rather than
  matrix decomposition, while `worldOrigin` is evaluated through the complete
  world matrix;
- private `nalgebra` 0.35.0 fixed-size storage is allowed, but project code
  defines every 3x3 accumulation explicitly instead of delegating the
  normative operation order to the generic multiplication/GEMM path.

The public API exposes only project-owned component, matrix, and error types.
It does not include texture anchors, scroll inheritance, source/FCBC/Render
types, player state, or the later independent reference implementation.

## Explicit non-goals

- Scroll integration, typed Expression DAG, Piecewise lowering, and exact
  integration validation (I4.4-I4.7).
- A claim that platform `f64` transcendental calls already satisfy the complete
  difficult-input correct-rounding requirement. I4.8 owns that independent
  reference closure and production/transform cross-check.
- Player-local sampled caches, BakedCurve output, Conversion approximation,
  FCBC codecs, CLI integration, or any version-domain status transition.

## I4.1 acceptance evidence

1. Every numeric ID round-trips to one enum value and one unique canonical
   name; ID 31 is rejected.
2. All 31 functions return exact `+0.0` and `1.0` endpoint bits, including an
   input of `-0.0`.
3. Algebraic midpoint vectors cover linear, Quad, Cubic, Quart, Quint, and
   Expo families; all IDs have finite midpoint and dense-sample results.
4. Every family is checked against the normative out and inOut transforms.
5. Back and Elastic overshoot/undershoot remain visible; Bounce remains finite,
   bounded, and preserves an interior rebound.
6. NaN, infinities, out-of-domain progress, and unknown IDs return stable
   errors instead of panicking or producing a non-finite result.

## I4.2 acceptance evidence

1. Point/segment tests cover pre-roll base, active interpolation, an excluded
   segment end, point continuation, gap fills, extrapolation, and explicit
   errors.
2. Step and all 31 canonical easing names execute through the Track path;
   linear vector tests prove one shared progress value and typed components.
3. Layer tests prove highest-priority replace and deterministic
   `(priority, owner-local name)` add/multiply order independently of input
   declaration order.
4. Cubic Bezier tests bind linear-equivalent bits, exact endpoint pinning, an
   overshooting-y bit vector, and the stable enclosure-unavailable path.
5. Non-finite query/base/progress/result paths and target mismatch return stable
   errors. Exact-product underflow that cannot be represented by the expansion
   is rejected conservatively.

## I4.3 acceptance evidence

1. Identity, translation, non-uniform scale, positive rotation, pivot, and
   project-owned point-application vectors bind the column-vector convention.
2. Position, rotation, scale, and alpha Tracks feed the transform query through
   the existing typed evaluator without schema-default inference.
3. Multi-level parent vectors prove recursive world origin, rotation, scale,
   alpha, and matrix composition; all disabled inherit flags restart from their
   world identity components.
4. A non-uniform parent scale plus child rotation preserves separately declared
   world rotation/scale while the resulting matrix contains geometric shear;
   no matrix decomposition is used.
5. Reordered Line declarations return identical results, and wrong namespace,
   unknown Line, Track gap, non-finite time, and overflow paths return stable
   errors.

## Delivery and residual gate

The Rust/build/test gate runs only on an exact draft-PR SHA through
`.github/workflows/full-gate.yml`. I4.1 through I4.3 are bounded `partial`
transitions: they do not close the FCS section 14 matrix row or the I4 stage.
I4.4-I4.7 still own scroll, DAG, Piecewise, and integration; I4.8 must bind
difficult transcendental/cubic-Bezier/transform vectors, an independent
implementation path, and production/reference cross-checks before strict-runtime
conformance can pass.
