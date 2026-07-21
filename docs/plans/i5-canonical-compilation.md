# I5 CanonicalCompilation, metadata, resources, sync, and fidelity

Status: I5.1-I5.7 implementation establishes the full I5 canonical-compilation
stage boundary: profile requirements, contributor/credit, opaque resources,
sync formula/preview, typed-custom limits, source-free provenance/stale tracking,
CanonicalCompilation aggregation, and deterministic ConversionReport/RepairRecord
model. This plan still does not claim Render scene, converter execution, FCBC
product, or FCS 5 release completion.


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

## I5.2 owned surface

- `CanonicalContributor` is no longer a generic standard-field bag. It exposes
  the exact source ID, required non-empty name, ordered aliases, and an
  insertion-ordered identifier object whose values are statically strings.
  Omitted aliases and identifiers become typed empty collections.
- `CanonicalCreditRole` separates the twelve standard roles from exact custom
  ASCII IDs. The ambiguous spelling `artist` remains a custom role and is never
  rewritten to `composer`; empty, non-ASCII, and `custom(...)` spellings fail.
- The canonical credit vector retains source display order, and each credit's
  contributor vector retains its declared order. A reference is accepted only
  through the contributor-typed schema; unknown, resource-only, or repeated
  references fail without namespace inference.
- Contributor declarations remain a deterministic ID-keyed map, so their
  declaration order is non-semantic. Credit order remains semantic and changes
  canonical equality when reordered.
- Credit stable-ID generation and FCBC record assembly remain I7-owned. FCS 5
  fixes exact generated textual identity only for Line and Note, so I5.2 does
  not invent a generated credit ID spelling.

## I5.2 acceptance evidence

- The `metadata_graph` suite covers typed contributor fields and defaults,
  identifier insertion order/string typing/duplicate keys, all twelve standard
  roles, exact custom `artist`, contributor and credit order, missing/empty
  names, invalid custom roles, and duplicate/unknown/wrong-kind references.
- `source.valid.contributor-credit-closure`,
  `source.invalid.contributor-missing-name`,
  `source.invalid.credit-duplicate-contributor`, and
  `source.invalid.credit-resource-reference` are manifest-bound and execute
  through `conformance_manifest::i5_contributor_credit_fixtures_execute_at_the_canonical_boundary`.
- Acceptance requires the exact PR head to pass `.github/workflows/full-gate.yml`
  and a passing Primary Self-Audit with no unresolved Critical/Important finding.

## I5.3 owned surface

- `Document::canonical_resource_bundle` is the only filesystem-bearing source
  boundary in this work unit. It requires an explicit workspace root and public
  `ResourceLimits`; parsing, static checking, `canonical_metadata`, and
  `canonical_chart` remain filesystem-free.
- Logical member spelling is validated before filesystem access. Resolution
  canonicalizes the supplied root and member, accepts only regular files whose
  resolved target remains below that root, accepts an in-root symlink target,
  and reports missing, directory, non-regular, and escaping targets as
  `resource.unknown-reference` without embedding host paths in the model.
- Resource count, maximum single-resource bytes, and maximum total bytes are
  public implementation limits. Metadata length is checked before allocation;
  reads are additionally bounded against concurrent growth. Every failure uses
  `resource.limit-exceeded` with kind, limit, observed value, and source span.
- `CanonicalContentSha256`, `CanonicalBundledResource`, and
  `CanonicalResourceBundle` retain computed SHA-256 and exact opaque bytes under
  the stable textual resource ID. Constructors reject a declared/computed hash
  mismatch and duplicate bundle IDs. A bundle retains unused declarations and
  does not merge distinct IDs with equal bytes or hashes.
- Image and texture descriptors materialize `colorSpace`, `alpha`, and
  `sampling` in the Render-defined canonical order and validate their exact
  enums. Standard `font/ttf` descriptors materialize the exact Core
  `fontProfile`, `shapingProfile`, and `faceCount` object. Other resource kinds
  do not inherit these fields.
- The resolver never decodes, guesses, transcodes, normalizes, or repairs media.
  PNG/WebP capability and TrueType table validation remain I9-owned; FCBC
  resource u64 identity, record assembly, and byte layout remain I7-owned.
- Production hashing uses the already active cataloged `sha2` 0.11.0 source.
  Cataloged `tempfile` 3.27.0 is activated only as an `fcs-source` dev dependency
  for isolated filesystem and symlink integration tests.

## I5.3 acceptance evidence

- `resource_bundle` covers deterministic ID order, opaque invalid-codec bytes,
  unused resources, equal-content distinct IDs, computed/matching/mismatched
  hashes, no path-bearing model field, image/texture/font metadata order and
  defaults, missing/directory/non-regular members, in-root and escaping
  symlinks, all three public budgets, and the filesystem-free metadata boundary.
- `fcs_model::metadata::tests::resource_bundle_constructors_defend_hash_and_logical_id_invariants`
  proves the source-free constructors reject inconsistent hashes and duplicate
  logical IDs.
- `source.valid.metadata-credits-resources-sync`,
  `source.invalid.resource-path-escape`,
  `source.invalid.resource-hash-mismatch`, and
  `source.invalid.resource-missing-member` execute through
  `conformance_manifest::i5_resource_fixtures_execute_at_the_workspace_bundle_boundary`.
- Acceptance requires the exact PR head to pass `.github/workflows/full-gate.yml`
  and a passing Primary Self-Audit with no unresolved Critical/Important finding.

## I5.4 owned surface

- `AudioOffset` remains the only public affine map between chart time and audio
  time: `audioTime = chartTime + audioOffset` and the exact inverse. Positive,
  zero, and negative offsets share one implementation used by player and
  converter consumers.
- `CanonicalPreview` is an audio-domain half-open interval with
  `end > start >= 0` and `contains_audio_time` membership on `[start, end)`.
- `CanonicalSync` constructs only legal products: preview requires
  `primaryAudio`; shared helpers expose `audio_time`, `chart_time`, and
  chart-time preview membership through the same formula.
- Source lowering reuses those constructors. Invalid preview domains emit
  `type.invalid-operation`; preview without primary audio emits
  `resource.unknown-reference`.
- Shared vectors in `docs/conformance/fcs5/expected/sync-shared-vectors.toml`
  and the existing metadata fixture expected offset equation execute through
  model methods and the canonical metadata boundary.

## I5.4 acceptance evidence

- Model tests pin bidirectional formula vectors, non-finite rejection, half-open
  preview membership, and constructor rejection of preview without primary audio.
- `sync_shared_vectors` executes the TOML shared vectors and the metadata fixture
  expected equation.
- `metadata_graph` and
  `conformance_manifest::i5_sync_fixtures_execute_at_the_canonical_boundary`
  execute the valid sync fixture and the two invalid preview fixtures.
- Acceptance requires the exact PR head to pass `.github/workflows/full-gate.yml`
  and a passing Primary Self-Audit with no unresolved Critical/Important finding.

## I5.5 owned surface

- `CustomValueLimits` is the public compiler-profile limit object for typed
  custom trees: depth, total object fields, per-string UTF-8 bytes, and total
  estimated custom-tree bytes.
- `Document::canonical_metadata` uses Core defaults; tests and later host
  profiles can pass tighter limits through `canonical_metadata_with_limits`.
- Limit failures use `resource.limit-exceeded` with budget kind/limit/observed
  and source span. Duplicate object keys remain `schema.duplicate-field`.
- Homogeneous arrays, ordered objects, finite scalars/colors, and explicit empty
  array element typing remain FCBC-compatible restrictions on the shared value
  surface.

## I5.5 acceptance evidence

- `custom_value_limits` covers default acceptance and independent depth, field,
  string, and total-byte budgets plus the existing duplicate-key fixture.
- `conformance_manifest::i5_custom_fixtures_execute_at_the_canonical_boundary`
  executes `source.invalid.custom-duplicate-key`.
- Acceptance requires the exact PR head to pass `.github/workflows/full-gate.yml`
  and a passing Primary Self-Audit with no unresolved Critical/Important finding.

## I5.6 owned surface

- `OriginState` is the closed Conversion §5.2 set and is never inferred from
  value-vs-default comparison.
- `RestrictedProvenanceFact` retains logical source locator/value/order,
  mapping-rule refs, optional semantic status, and dependency edges without
  source AST, absolute paths, or raw snapshots.
- `ProvenanceGraph` validates dependency closure/cycles and propagates
  user-modified edits as stale dependents.
- `DistributionMetadata` holds provenance facts, input content hashes, and
  ordered custom objects. Empty distribution is the native FCS compile default.
- `CanonicalCompilation` aggregates chart, resource bundle, and distribution;
  stripping distribution leaves execution products intact.
- `Document::canonical_compilation` assembles the three products with empty
  native distribution metadata and does not retain workspace absolute paths in
  distribution facts.

## I5.6 acceptance evidence

- `fcs_model::provenance` unit tests cover closed origin spellings, absolute/URI
  locator rejection, transitive stale propagation, cycle/missing dependency
  rejection, and chart/bundle/distribution separation.
- Acceptance requires the exact PR head to pass `.github/workflows/full-gate.yml`
  and a passing Primary Self-Audit with no unresolved Critical/Important finding.

## I5.7 owned surface

- `RepairMode` carries explicit enablement and ordered authorized rule refs.
- `RepairRecord` records source locator, diagnostic category, action, rule,
  old/new typed values, and semantic impact; unauthorized rules fail.
- `ConversionReport` owns deterministic entries, repairs, summary counts, and
  Conversion §7.1 status aggregation precedence.
- Entries order by phase, entity id, field key, rule id, then entry id.
- `DistributionMetadata` now retains ordered `repair_records` after provenance
  while native FCS compile distribution remains empty.

## I5.7 acceptance evidence

- `fcs_model::report` unit tests cover status aggregation precedence,
  unauthorized repair rejection, repaired-vs-lossless forcing, failed outranking
  repaired, and deterministic entry ordering.
- Acceptance requires the exact PR head to pass `.github/workflows/full-gate.yml`
  and a passing Primary Self-Audit with no unresolved Critical/Important finding.

## Remaining after I5

- I5 stage residual is closed at the I5 product boundary. FCBC encoding,
  conversion importer execution, Render, CLI, and later stages remain open and
  are not claimed by I5.1-I5.7.
