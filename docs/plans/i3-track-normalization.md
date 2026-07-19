# I3.6 Track Normalization Plan

## Normative closure

This work unit lowers expanded Line-owned Track declarations into immutable
canonical Tracks. Its authority is FCS Core Sections 9.1-9.5 and 17, ADR 0001,
ADR 0002, ADR 0010, the existing Track AST/schema, the canonical Line graph,
and the single chart-time map.

The closure covers:

- the four dynamic Line targets: `position`, `rotation`, `scale`, and `alpha`;
- typed canonical segment and point values with finite-value validation;
- beat/time normalization into the existing global chart-time domain;
- half-open segments, point boundary rules, deterministic source ordering, and
  ordinary segment overlap validation;
- blend, priority, fill, and before/after extrapolation descriptors;
- Core easing names, step, linear, and cubic Bezier schema values;
- direct pieces, compile-time structural conditionals, generators, typed lets,
  and emitted `segment`/`keyframe` pieces after elaboration.

`fill:error` remains an explicit canonical policy. Query-domain gap failure is
not evaluated here because runtime sampling and scroll integration are outside
I3.6. Likewise, layered Track evaluation, easing evaluation, runtime
descriptors, scroll, CanonicalChart aggregation, snapshots, FCBC, and ABI stay
with later stages.

## Owned surface

- `crates/fcs-model`: immutable canonical Track, segment, point, value,
  interpolation, blend/fill policy, and Track-set identity/order validation.
- `crates/fcs-source`: concrete expanded Track retention and lowering through
  the canonical Line and chart-time seams with stable diagnostics.
- `crates/fcs-source/tests`: focused canonical Track tests and manifest-bound
  valid/invalid Track fixture execution.

## Acceptance evidence

1. Canonical Track values expose owner Line stable ID, owner-local name, target,
   typed pieces, interpolation, blend, priority, fill, and extrapolation.
2. Lowering rejects unsupported targets/types, non-finite values, mixed source
   time types, invalid intervals, invalid easing names, and invalid Bezier
   controls.
3. Segment and point boundaries follow `[start,end)` and the Core point
   replacement rule; Track and piece ordering is deterministic.
4. Duplicate Track identities, equal-priority replace conflicts, ordinary
   segment overlaps, and point replacement conflicts use stable diagnostics.
5. Existing `track-boundaries` and `track-overlap` fixtures execute at the
   canonical boundary; focused tests cover defaults, generator expansion,
   target/type, easing, and boundary behavior.
6. Local evidence is limited to formatting, diff, Markdown, and manifest/static
   checks. Rust compilation, Clippy, nextest, fuzz, and build-artifact checks
   run only in the exact-SHA GitHub full gate.
