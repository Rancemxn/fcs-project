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

## Explicit non-goals

- Track selection, interpolation, fill, extrapolation, blend, and priority
  evaluation (I4.2).
- Matrix/parent transform evaluation or `nalgebra` activation (I4.3).
- Scroll integration, typed Expression DAG, Piecewise lowering, and exact
  integration validation (I4.4-I4.7).
- A claim that platform `f64` transcendental calls already satisfy the complete
  difficult-input correct-rounding requirement. I4.8 owns that independent
  reference closure and production cross-check.
- Player-local sampled caches, BakedCurve output, Conversion approximation,
  FCBC codecs, CLI integration, or any version-domain status transition.

## Acceptance evidence

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

## Delivery and residual gate

The Rust/build/test gate runs only on an exact draft-PR SHA through
`.github/workflows/full-gate.yml`. I4.1 remains a bounded `partial` transition:
it does not close the FCS section 14 matrix row or the I4 stage. I4.8 must bind
difficult transcendental bit vectors, an independent implementation path, and
production/reference cross-checks before strict-runtime conformance can pass.
