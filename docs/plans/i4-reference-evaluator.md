# I4 Reference Evaluator and Numeric System Plan

## Normative closure

- `docs/specifications/fcs.md` sections 9.4, 10.1–10.4, 11.4, 12.4, 13,
  14.1, 14.3, and 18.
- `docs/specifications/fcbc.md` sections 11, 13.1, 13.2, 14, and 15.
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

## I4.4 specification closure

Before the product evaluator is activated, the scroll dependency closure fixes
the distinction between each Line's local descriptor and the query-time
effective result:

- `q`, scroll tempo, speed environment, integration origin, and initial floor
  remain Line-local;
- `inherit.scroll=false` terminates scroll ancestry, while `true` recursively
  adds only the actual parent chain's local floor and velocity;
- effective floor reference values are combined in high precision and rounded
  once, while effective velocity uses root-to-target binary64 addition;
- floor scale, Note scroll factor, transforms, and hidden sampled caches never
  participate in composition; direct seek and error isolation are explicit;
- local reverse permission remains local, so an allowed reverse ancestor may
  make a descendant's effective velocity negative.

The literal `source.valid.scroll-inheritance` vector binds a three-Line chain,
detached inheritance, boundary continuity, reverse/zero speed, signed zero,
Note distance, direct-seek values in both directions, and an unrelated Track
gap. The product evaluator now executes this vector at the canonical boundary;
the exact-head gate and independent reference closure remain delivery evidence
and later I4 residuals.

## I4.5 owned surface

I4.5 adds the product Expression DAG boundary and direct fixture execution:

- `fcs-model` owns source-free typed nodes, exact constants, topological operands,
  environment dependency projection, structural validation, and deterministic
  shared-subgraph interning;
- `fcs-runtime` evaluates the Core scalar/vector node set with finite/domain,
  checked-integer, signed-zero, and structured error behavior; `and`, `or`, and
  `choose` remain lazy and do not evaluate unselected operands;
- `fcs-source` lowers direct runtime fields to the canonical DAG without exposing
  parser types to the runtime crate, and rejects direct `EnvP` without Piece
  context;
- `source.valid.runtime-choose` and `source.valid.exact-expression-dag` bind
  expected descriptor properties and execute through the product evaluator.

I4.5 deliberately does not claim Piecewise/EnvP lowering, expanded
template/generator expression ownership, descriptor-wide cycle analysis,
independent correctly-rounded reference evaluation, or property/fuzz closure.

## I4.6 owned surface

I4.6 adds the source-free exact Piecewise descriptor boundary:

- `fcs-model` validates finite/unbounded descriptor domains, complete ordered
  Piece partitions, child type/domain coverage, reachability, cycles, and direct
  `EnvP` misuse;
- structurally identical descriptors are interned without table indexes in
  their keys, while sorted direct roots and child-first postorder produce one
  declaration-order-independent table;
- `fcs-runtime` selects half-open/final-inclusive Pieces and rebinds normalized
  progress only for the selected child, including the fixed unbounded-side
  values required by FCBC 13.2.

I4.6 does not add FCBC serialization, source Track-to-descriptor assembly,
exact integration, an independent reference path, or property/fuzz closure.

## I4.7 owned surface

I4.7 replaces the product scroll evaluator's step-only restriction with a
deterministic direct-seek integration path:

- split at the exact origin/query, scroll-tempo, Track segment, and Track point
  boundaries already present in the canonical inputs;
- retain the constant and step analytic q-delta path, while varying linear,
  Core easing, and cubic-Bezier intervals use deterministic adaptive 15-point
  Gauss-Kronrod refinement;
- divide the ABI 1.0 `0x1p-32` target across known intervals and return
  `IntegrationBudgetExceeded` at a fixed depth/evaluation ceiling rather than
  frame accumulation, a sampled cache, or a `BakedCurve`;
- bind analytic area vectors, reverse integration, origin bits, repeated
  out-of-order queries, and the stable budget failure.

I4.7 is the product-side bounded estimator. I4.8 still owns the independent
reference implementation and difficult-input cross-check; I4.9 owns the
randomized partition/frame-rate/error-bound property corpus.

## I4.8 owned surface

I4.8 adds a test-only independent reference lane without changing the product
runtime API, and corrects the Float `pow` domain/signed-zero mismatch it exposes:

- `fcs-runtime` dev tests use pinned `astro-float` 0.9.5 with defaults disabled
  and `std` enabled; difficult primitives use directed lower and upper bounds
  and are accepted only when both round to one binary64 bit pattern;
- Core transcendental operations, easing formulas, cubic-Bezier inversion,
  scroll integration, and 3x3 matrix composition use separate test-side
  dispatch and arithmetic. The reference side does not call product private
  math, nalgebra multiplication, cache, sampled curve, or FCBC evaluator;
- `numeric-vectors.toml` uses typed `deny_unknown_fields` records, rejects
  duplicate semantic identities, and executes literal difficult-operation bits,
  stable domain errors, and easing bits through the product/reference boundary.

I4.8 does not promote `astro-float` into a normal dependency, expose reference
types, replace project-owned errors/enums, or close the randomized I4.9 corpus.

## I4.9 owned surface

I4.9 closes the bounded randomized residual without changing the product API:

- `fcs-runtime` activates the pinned workspace `proptest` 1.11.0 only as a dev
  dependency and uses a fixed ChaCha seed with 96 cases, generator depth one,
  one Track segment, at most 16 frame partitions/18 public queries, and finite
  dyadic inputs over the implemented Track domains;
- randomized constant, step, linear, Core easing, and cubic-Bezier speed Tracks
  compare direct seek with independently re-originated interval queries in both
  chart-time directions within the ABI 1.0 `0x1p-32` target;
- the linear lane compares the product integral with a separate analytic
  primitive, while repeated queries require raw-bit stability;
- shuffled Track and Line declarations prove canonical blend/topology ordering,
  and randomized transform inheritance flags preserve identical result bits;
- stable reverse-policy and non-finite query errors remain executable alongside
  the existing gap and fixed integration-budget tests.

The property lane is bounded diagnostic/conformance evidence. It does not add
runtime randomness, sampled curves, caches, FCBC assembly, or a new production
dependency.

## Explicit non-goals

- FCBC descriptor assembly/serialization and generic loader-facing Distance
  records (I7).
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

## I4.4 specification acceptance evidence

1. The manifest binds `source.valid.scroll-inheritance` and its independent
   literal expected JSON; the source has no sampled floor or hidden cache.
2. Root, child, and grandchild queries prove local q/origin values remain
   distinct while effective floor and velocity compose recursively.
3. Detached scroll inheritance, local zero speed, an allowed reverse ancestor,
   direct seek in both directions, and tempo/speed boundaries have explicit
   expected values.
4. Note distance uses the effective floor with an independent floor scale and
   scroll factor; signed-zero origin and non-origin results bind raw bits.
5. An unrelated `track.gap` is recorded as an isolated error and cannot fail a
   target on the root-to-target ancestry.

## I4.6 acceptance evidence

1. Model tests reject partition gaps, invalid endpoint placement, insufficient
   child domains, cycles, unreachable descriptors, and direct `EnvP` roots.
2. Structurally equal descriptors intern globally, signed-zero constants remain
   distinct, and reversed declaration/root input produces the same table.
3. Runtime tests bind half-open boundaries, a final inclusive endpoint, nested
   progress rebinding, and the fixed progress values for unbounded sides.

## I4.7 acceptance evidence

1. Known tempo and Track discontinuities are explicit integration boundaries;
   no uniform sample grid or retained frame state exists.
2. Constant/step inputs keep the analytic path, while linear, Core easing, and
   cubic-Bezier speed inputs use the bounded adaptive path.
3. Focused vectors bind analytic areas within `0x1p-32`, forward/reverse direct
   seek, exact origin bits, repeatability, and stable budget exhaustion.

## I4.8 acceptance evidence

1. `crates/fcs-runtime/tests/reference_numeric.rs` independently evaluates all
   Core transcendental families, explicit `atan2` axis/signed-zero cases,
   easing branches, flat/overshooting cubic-Bezier controls, non-uniform
   inherited transforms, and linear/easing/Bezier scroll integrals.
2. `docs/conformance/fcs5/expected/numeric-vectors.toml` contains literal bits
   for 24 difficult Core results, six stable domain-error cases, and 21 easing
   family/branch vectors; the executable test rejects unknown fields,
   duplicate entries, unknown operations/errors, malformed bits, and
   mismatched product/reference results.
3. The Astro oracle uses directed `Down`/`Up` bounds with adaptive precision for
   Core operations and guarded precision for easing primitives; only a shared
   binary64 result is accepted, and the dependency is absent from the normal
   `fcs-runtime` tree.
4. Reference Bezier inversion uses a separate monotone bisection and polynomial,
   matrix multiplication uses plain arrays, and the scroll lane uses independent
   analytic integrals; none calls the product Bezier, matrix, integration, or
   caching implementation.

## I4.9 acceptance evidence

1. `crates/fcs-runtime/tests/runtime_properties.rs` fixes seed `0xF0C54901`,
   disables failure persistence, and bounds the corpus to 96 cases and 16 frame
   partitions per generated query.
2. Constant, step, linear, three representative Core easing paths, and two
   legal cubic-Bezier families cover forward and reverse interval integration;
   direct and partitioned results stay within `8 * 0x1p-32` after bounded
   binary64 frame accumulation.
3. The linear property uses an independent analytic integral within the same
   bound; repeated Track/scroll queries and reordered Track/Line declarations
   preserve exact result bits.
4. Randomized transform bases and inherit flags remain declaration-order
   independent, while reverse-policy and non-finite query errors retain stable
   structured categories.

## Delivery and residual gate

The Rust/build/test acceptance gate runs on an exact draft-PR SHA through
`.github/workflows/full-gate.yml`. Local commands are diagnostic only and are
recorded separately from Action evidence. I4.1 through I4.9 close the planned
reference-evaluator implementation units, but do not by themselves change any
specification version-domain state or complete FCS 5. I4.7 owns the product
exact-integration path, I4.8 owns the independent cross-check, and I4.9 owns the
bounded deterministic property corpus; FCBC descriptor assembly remains I7.
