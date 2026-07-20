# I5 CanonicalCompilation, metadata, resources, sync, and fidelity

Status: I5.1 implementation establishes the canonical profile-requirement
boundary. I5.2-I5.7 remain open and this plan does not claim a complete
`CanonicalCompilation`, Render scene, converter, FCBC product, or FCS 5 release.

## Normative dependency closure

- `docs/specifications/fcs.md` §§5.1-5.2, 7, 8, 11, and 16-18 define the
  profile matrix, stable diagnostic category, current metadata/resource/sync
  surface, and `CanonicalCompilation` boundary.
- `docs/specifications/fcs-render.md` owns Render payload grammar, canonical
  scene semantics, and render-resource references. I5.1 may inspect only the
  versioned balanced Render envelope retained by the Core AST.
- Accepted ADR 0010 keeps I5 evidence stage scoped. The merged I1-I4 source,
  canonical, and runtime products are dependencies, not substitutes for I5.
- `docs/plans/fcs5-roadmap.md` is the task authority. I5.3 owns workspace bytes
  and hash verification; I9 owns executable Render closure.

## I5.1 owned surface

- `Document::validate_profile_requirements` is the public canonical-stage
  boundary. It elaborates before counting gameplay Lines, reuses canonical
  metadata validation, and never moves profile compatibility into parsing.
- Every non-fragment primary profile has the chart tempo/time-model constraint.
  Missing `tempoMap` is `profile.requirement-missing`; a present but invalid map
  retains the more specific `tempo.invalid` result.
- The effective playable/renderable capability set is the union of the primary
  playable/renderable profile and explicit features. Repeating the same
  capability is idempotent because FCS §5.2 does not prohibit it.
- Playable capability requires a canonical audio `sync.primaryAudio`, a sync
  block, and at least one Line after template/generator elaboration.
- Renderable capability requires a versioned Render envelope. Its payload,
  scene graph, and referenced-resource closure remain I9-owned.
- Publishable requires the four named metadata fields, at least one lowered
  credit, declared SHA-256 for every source resource, and at least one explicit
  playable or renderable feature. I5.3 verifies declared hashes against bytes.
- Diagnostics use canonical-stage `profile.requirement-missing`, source-bounded
  spans, and deterministic `(start, end, message)` order. Existing metadata,
  resource, Line, elaboration, and tempo diagnostics remain more specific.
- `Document::canonical_chart` invokes the same gate before constructing a
  canonical product. A tempo-less fragment may pass profile validation but
  cannot become a `CanonicalChart`, whose required tempo map is fixed by §17;
  no default tempo is synthesized.

## I5.1 acceptance evidence

- `profile_validation::every_legal_primary_profile_and_orthogonal_feature_combination_is_accepted`
  enumerates the legal five-profile matrix, including empty fragment features,
  redundant idempotent capabilities, orthogonal additions, and both features.
- `profile_validation::minimal_profiles_do_not_inherit_orthogonal_or_publishable_requirements`
  proves the minimal chart, playable, renderable, publishable-playable, and
  publishable-renderable closures without unrelated capability inputs;
  `playable_capability_counts_a_line_created_only_by_elaboration` binds a Line
  that exists only after template expansion.
- The remaining `profile_validation` tests isolate missing tempo, sync, primary
  audio, gameplay Line, Render envelope, publishable feature, metadata, credit,
  and declared hash, plus diagnostic precedence/order and the fragment/chart
  product boundary.
- `source.valid.profile-publishable-both`,
  `source.invalid.profile-fragment-feature`, and
  `source.invalid.profile-publishable-requirements` are manifest-bound and run
  through `conformance_manifest::i5_profile_fixtures_execute_at_the_canonical_validation_boundary`.
- Acceptance requires the exact PR head to pass `.github/workflows/full-gate.yml`
  and a passing Primary Self-Audit with no unresolved Critical/Important finding.

## Remaining I5 work

- I5.2 completes contributor/credit role, display-order, identifier, and typed
  reference behavior beyond the count consumed by I5.1.
- I5.3 resolves component-normalized workspace paths, enforces root/symlink
  safety, reads opaque bytes, computes SHA-256, verifies declarations, and
  builds `CanonicalResourceBundle`.
- I5.4 closes the sync/preview formula and shared player/converter vectors.
- I5.5 binds typed custom value limits and FCBC-compatible restrictions.
- I5.6 adds provenance and stale-dependency tracking without source AST leakage
  into `CanonicalChart`.
- I5.7 adds the deterministic report/repair model. None of these residuals is
  implemented or claimed by I5.1.
