# I3.5 Note Lowering Plan

## Normative closure

This work unit lowers expanded FCS Note declarations into immutable canonical
Notes. Its authority is FCS Core Sections 12.1-12.5, the canonical-lowering
pipeline in Section 17, ADR 0001, ADR 0002, ADR 0010, and the I3 roadmap.

The closure covers:

- the four Core Note kinds: tap, hold, flick, and drag;
- stable explicit/generated Note IDs and stable Line references;
- canonical chart-time normalization, side and judgment defaults, and Hold
  interval validation;
- closed judge-shape descriptors with finite-positive geometry validation;
- sound/score policy descriptors and disabled-judgment restrictions;
- deterministic presentation defaults (including rotation and visibility
  boundaries) without using presentation as gameplay geometry;
- canonical sorting by time, Line stable ID, document order, and Note stable
  ID.

Track normalization, scroll integration, runtime descriptors, resource byte
resolution, extension dispatch, FCBC/ABI, Render, Conversion, and release
behavior remain later bounded units.

## Owned surface

- `crates/fcs-model`: immutable canonical Note values, gameplay/presentation
  descriptors, policy validation, and deterministic Note ordering.
- `crates/fcs-source`: typed lowering from expanded Notes, existing chart-time
  and Line graph APIs, field/default validation, and canonical diagnostics.
- `crates/fcs-source/tests`: focused valid/invalid Note, policy, shape, Hold,
  stable-ID, presentation-default, and sorting evidence.

The adapter may retain static policy identities as canonical descriptors, but
must not invent a second resource-reference type or perform workspace byte
resolution. Resource kind/hash/path validation remains owned by the metadata
and resource boundaries.

## Acceptance evidence

1. All four Note kinds lower into immutable canonical values with explicit and
   generated stable IDs and stable Line references.
2. Gameplay line/time uses the existing chart-time map; side and judgment
   defaults are deterministic; disabled judgment produces no sound or score
   policy.
3. Judge-shape defaults, required geometry, forbidden fields, and finite-
   positive geometry constraints are enforced.
4. Sound and score policy combinations, required references, and Hold/non-Hold
   end-time rules are validated at the canonical boundary.
5. Presentation defaults remain separate from gameplay geometry and visibility
   runtime descriptors remain later-stage work.
6. Focused Note tests pass before the repository full gate. The completed work
   unit includes the full gate, `git diff --check`, Primary Self-Audit,
   immutable review handoff, and merged-SHA review request.
