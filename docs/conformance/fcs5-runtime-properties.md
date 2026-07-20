# FCS 5 deterministic runtime property lane

This lane closes the bounded I4.9 property residual for the product runtime
evaluator. It is test evidence, not a sampled runtime representation or a new
source of FCS semantics.

## Fixed bounds

- implementation: `crates/fcs-runtime/tests/runtime_properties.rs`;
- dependency: dev-only `proptest` 1.11.0 at the fixed dependency baseline SHA;
- RNG: ChaCha with seed `0xF0C54901` and no persisted regression file;
- cases: 96 per property;
- generated strategy depth: one Track layer;
- generated Track segments: one per randomized speed Track;
- frame partitions: `1..=16` per query;
- public runtime queries: at most 18 per generated case;
- inputs: finite dyadic binary64 values inside `[-4, 4]`;
- product integration remains bounded by its fixed depth and 65,536-evaluation
  query budget.

## Properties

1. Repeated Track and scroll queries preserve raw result bits.
2. Reordered Track declarations preserve replace/add/multiply results.
3. Reordered Line declarations preserve randomized inherited transforms.
4. Constant, step, linear, Core easing, and cubic-Bezier scroll Tracks produce
   direct-seek and re-originated partition sums within `8 * 0x1p-32` in both
   chart-time directions.
5. Randomized linear scroll agrees with an independent analytic integral within
   the same accumulation bound.
6. Reverse-policy and non-finite query errors retain stable structured variants;
   focused unit tests continue to bind Track gaps and integration-budget
   exhaustion.

The factor of eight covers the bounded binary64 accumulation of up to sixteen
independently evaluated frame intervals. It does not relax the product
per-query portable-evaluable target of `0x1p-32`.

## Verification

The acceptance command runs only in `.github/workflows/full-gate.yml`:

```text
cargo nextest run -p fcs-runtime --test runtime_properties
```

Acceptance requires the repository full gate on the exact PR head SHA. Local
worktree checks are static only and do not compile or execute this lane.
