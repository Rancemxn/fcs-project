# I10.6 Evidence Ledger

The current CLI product partition was delivered by Issue #489: all 32 canonical
source fixtures execute through `check`, and the 12 successful chart fixtures
also execute through `compile` and mandatory Core load. The exact-head Full
Gate evidence is GitHub Actions run 30729340879, job 91446683051, on head
e0140f7ca15cc21620ef3ad8e44b4ece03a346c2; it passed the repository Full Gate.

This ledger binds the I10 product and domain obligations to checked-in executable
surfaces. It records evidence scope, not a Frozen or final I10 claim. Local Rust
tests, fuzz, and executable fixtures are not run; the complete Full Gate is run
only by GitHub Actions on the exact target head SHA. Current rows cite the exact
head Actions Full Gate as executable evidence for this assembly; Codespaces are
not a verification environment for the current I10 work.

The independent solid-fill Path semantic/raster follow-up passed GitHub Actions
Full Gate run 31283895136, job 93169439677, on exact head
a32ac311fb3171064e5c67967f160f93ab133e9c (`workflow_dispatch`). It covers Path
descriptor evaluation, curve/arc flattening, fill-rule containment, and the
reference raster path. The Path stroke follow-up then passed Actions Full Gate
run 31285800196, job 93174258827, on exact head
a9eb9594fb70b507f5c7738857b63fbc234519aa (`workflow_dispatch`). It covers the
Path StrokeRecord writer/loader handoff, explicit Close/open-subpath handling,
dash phase, cap/join coverage, and zero-length flattened segments. Source
Stroke grammar/lowering and Path text remain open.

The bounded Path stroke-only follow-up passed GitHub Actions Full Gate run
31290037232, job 93185526962, on exact head
`f65e3300131758d886bf3557d4daddb85dc3aa39` (`workflow_dispatch`). It permits a
Path writer/loader round trip with `strokeRef` and no `fillPaint`, and verifies
the decoded StrokeRecord, stroked-path semantic output, and non-empty raster
coverage through `source_product::canonical_path_writer_reaches_product_render_loader`.
Source Path grammar/lowering, Polygon stroke coverage is recorded below, broader
canonical writer coverage, and final Render closure remain open. Local tests,
fuzz, and executable fixtures remain unrun; Codespaces remain outside the
verification environment, and GitHub Actions remains the only complete Full
Gate source.

The bounded Polyline stroke follow-up passed GitHub Actions Full Gate run
31290853047, job 93187597128, on exact head
`cb8936f9faaecb83c397f8aed1c1d9bf707278ae` (`workflow_dispatch`). It permits a
stroke-only open Polyline writer/loader round trip, preserves the implicit fill
closure separately from `closed=false` stroke semantics, and verifies decoded
StrokeRecord binding, semantic stroke output, and non-empty raster coverage
through `source_product::canonical_polyline_stroke_writer_reaches_product_render_loader`.
Polygon stroke coverage, source stroke lowering, broader canonical writer
coverage, and final Render closure remain open. Local tests, fuzz, and executable
fixtures remain unrun; Codespaces remain outside the verification environment,
and GitHub Actions remains the only complete Full Gate source.

The canonical Clip writer follow-up passed GitHub Actions Full Gate run
31286712377, job 93176761343, on exact head
2d31b7bfc2dc346b9fbb92875f37966cafeaab04 (`workflow_dispatch`). It covers
stable-ID-sorted ClipRecord emission, NodeRecord `clipRef` binding, ClipGroup
loader validation, inherited clip-chain semantic output, and bounded raster
coverage. Source ClipGroup/clip lowering, source Text grammar/lowering, Text
stroke coverage, and broader Render closure remain open. Local tests,
fuzz, and executable fixtures remain unrun;
Codespaces remain outside the verification environment, and GitHub Actions is
the only complete Full Gate source.

The canonical Text/GlyphRun writer follow-up passed GitHub Actions Full Gate run
31287462784, job 93178786167, on exact head
`a6ff197c276e25f3092bd0d4410ff13f22f21db3` (`workflow_dispatch`). It covers
Text Geometry `glyphRunRefs`, stable-ID-sorted GlyphRunRecord emission, the
normalized placement fields, `font/ttf` resource binding, product font decode,
and loader ownership/reference validation through
`source_product::canonical_text_writer_reaches_product_render_loader`. Source
Text grammar/lowering, fallback or shaping beyond `simple-ltr-1`, Text stroke
coverage is recorded below, and broader Render closure remain open. Text
semantic/raster evidence is recorded below. Local tests, fuzz, and executable
fixtures remain unrun; Codespaces remain outside the verification environment,
and GitHub Actions remains the only
complete Full Gate source.

The bounded Text semantic/raster follow-up passed GitHub Actions Full Gate run
31288628033, job 93181879775, on exact head
`b4169c9f26c86c8a090bd63f3c91db263a5e3658` (`workflow_dispatch`). It evaluates
loader-validated Text geometry from decoded `truetype-glyf-1` outlines, applies
GlyphRun size/runOffset/pen/placement metrics and Text origin, expands TrueType
quadratic contours, and reuses the nonzero fill plus 8x8 reference raster path
through `source_product::canonical_text_writer_reaches_product_render_loader`.
Source Text grammar/lowering, fallback or shaping beyond `simple-ltr-1`, Text
stroke coverage is recorded below, and broader Render closure remain open. Local
tests, fuzz, and executable fixtures remain unrun; Codespaces remain outside the
verification environment, and GitHub Actions remains the only complete Full Gate source.

The bounded Text stroke follow-up passed GitHub Actions Full Gate run
31289376238, job 93183857394, on exact head
`825ecd480679f5a5348ef20a6df0424fa7b65167` (`workflow_dispatch`). It permits Text
fill, stroke, or stroke-only product writer output, reuses decoded glyph contours
as Path geometry for stroke semantics/rasterization, and verifies the
`source_product::canonical_text_stroke_writer_reaches_product_render_loader`
writer/loader, semantic, and non-empty raster path. Source Text grammar/lowering,
fallback or shaping beyond `simple-ltr-1`, and broader Render closure remain open.
Local tests, fuzz, and executable fixtures remain unrun; Codespaces remain outside
the verification environment, and GitHub Actions remains the only complete Full
Gate source.

| Obligation | Executable evidence | Boundary and status |
|---|---|---|
| Source parse boundary: all 55 declared FCS fixtures | `fcs_source_fixtures_execute_at_the_declared_frontend_boundary` in `crates/fcs-source/tests/conformance_manifest.rs` | Dispatches parse success/errors and accepts later-stage sources at the parser boundary. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Source elaborate boundary: generators, templates, branches, descending ranges, budget diagnostics | `i2_public_conformance_fixtures_execute_through_the_elaborator`; `i2_elaborate_error_fixtures_keep_static_diagnostics_and_budget_trace`; `compile_time::generator_iteration_budget_is_shared_across_expansion`; `compile_time::generated_node_budget_is_checked_before_output_append` | Elaborated output and stable budget/span/trace expectations. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Source canonical boundary: profiles, metadata/credits, resources, tracks, scroll, sync, custom values | `i5_profile_fixtures_execute_at_the_canonical_validation_boundary`; `i5_contributor_credit_fixtures_execute_at_the_canonical_boundary`; `i5_resource_fixtures_execute_at_the_workspace_bundle_boundary`; `i5_sync_fixtures_execute_at_the_canonical_boundary`; `i5_custom_fixtures_execute_at_the_canonical_boundary`; `i3_track_fixtures_execute_at_the_canonical_boundary`; `i3_scroll_fixture_executes_at_the_canonical_boundary` | Canonical validation, workspace-root resource resolution, hashes, limits, and expected diagnostics. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Source evaluate boundary: scroll inheritance and exact numeric vectors | `i4_scroll_inheritance_fixture_binds_literal_composition_vectors`; `reference_numeric::numeric_vector_toml_is_strict_and_executable` | Product runtime evaluator is exercised against declared values and Float64 bit expectations. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Normative source examples and grammar | `complete_source_grammar_fixture_parses_with_all_top_level_kinds`; `source.valid.complete-source-grammar`; `source.valid.compile-time-generator`; `source.valid.minimal-chart`; `source.valid.appendix-a-minimal-complete` via `appendix_a_fixture_expands_four_notes_at_exact_beats_and_eliminates_authoring_structure`; `examples/fcs/fragment.fcs` via `repository_fcs_examples_execute_at_their_applicable_product_boundaries` (`format` plus parser validation), and `examples/fcs/{chart,templates}.fcs` via the same test's `check` path; CLI `check_executes_manifest_declared_canonical_fixtures` (all 32 canonical fixtures) and `compile_executes_manifest_declared_canonical_fixtures_through_core_load` (12 successful chart fixtures) | Complete grammar, Appendix A, and executable checked-in examples are covered. Implemented; current exact-head Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 passed. |
| Core profiles and resource/compile limits | `every_legal_primary_profile_and_orthogonal_feature_combination_is_accepted`; `every_public_parser_limit_has_a_bounded_failure`; `enforces_public_count_single_and_total_byte_budgets`; CLI `compile_honors_public_compile_limits`; CLI `compile_uses_explicit_resource_resolver_root` | Five source profiles, bounded parser/compiler/resource paths, and explicit resolver roots. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Canonical compilation and FCBC handoff | `native_canonical_compilation_has_empty_distribution_metadata`; `write_from_compilation_round_trips_through_product_load`; CLI `compile_executes_manifest_declared_canonical_fixtures_through_core_load`; CLI `compile_emits_loadable_fcbc_from_chart_with_line_and_note` | Canonical/provenance-only assembly reaches mandatory Core load. Implemented for the declared product subset; full cross-domain closure remains open. |
| FCBC goldens | `loads_minimal_runtime_golden_framing`; `loads_embedded_resource_golden_framing`; `loads_nonempty_execution_golden_framing`; CLI `inspect_executes_every_fcbc_golden_through_declared_core_contract` | All three manifest goldens are inspected through product Core-load success/rejection contracts; checked-in bytes/profile/hash/section/resource assertions remain independent framing checks. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| FCBC mutations | `framing_mutations_reject_via_product_load_container`; `nonempty_execution_mutations_reject_via_product_core_load`; `embedded_resource_mutations_reject_via_product_core_load`; `header_mutations_reject_on_both_product_surfaces`; `section_table_mutations_reject_with_the_layout_categories`; `core_mutations_reject_with_the_content_categories` | Framing, required sections, Core content, and resource corruption categories. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Runtime ABI execution | `a_native_hold_is_executable_through_the_abi`; `a_native_sub_beat_tempo_map_survives_revalidation_and_executes`; `decoded_expression_evaluation_matches_expected_bits_and_lazy_trace` | Product writer/loader/evaluator and exact execution vectors. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Runtime properties | `track_queries_are_bit_stable_and_declaration_order_independent`; `direct_scroll_seek_is_partition_invariant_within_error_budget`; `linear_scroll_matches_independent_integral_bound`; `transform_graph_is_declaration_order_independent`; `runtime_error_categories_remain_stable` | Fixed-seed bounded property lane. Implemented in source; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Conversion registries and six public fixtures | `public_fixture_corpus_executes_with_expected_reports`; `public_manifest_declares_required_metadata_fields`; CLI `report_executes_every_public_conversion_fixture` | PGR, RPE, and PEC public import fixtures, expected reports, profile/dialect/hash registry closure. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Conversion export and semantic reparse | `every_registry_target_profile_has_export_reparse_evidence`; `public_pgr_feature_fixture_roundtrips_through_export`; `rpe_and_pec_export_reparse_compare`; `pec_feature_motion_roundtrip_and_mutation_are_observable`; CLI `convert_executes_every_declared_public_export_reparse_fixture` | Declared target profiles and three manifest export/reparse fixtures. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Conversion report, authorization, and hard limits | `successful_approximation_reports_verified_maximum_and_segment_count`; `authorized_metadata_drop_is_applied_by_the_writer_and_reported`; `unused_drop_authorization_cannot_mask_a_direct_roundtrip_mismatch`; `report_limit_rejects_export_before_target_output`; CLI `convert_requires_separate_source_and_target_floor_scale_bindings`; CLI `convert_rejects_unbound_typed_profile_parameters`; CLI `convert_exports_public_pgr_fixture_with_explicit_target_capability`; CLI `convert_exports_public_rpe_and_pec_fixtures` | Capability negotiation, report ordering, authorization, reparse comparison, and output limits. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Render source and canonical product paths | `solid_rect_source_reaches_product_render_loader`; `nested_shape_source_reaches_product_render_loader`; `point_geometry_source_reaches_product_render_loader`; `image_source_reaches_product_render_loader_with_resource_metadata`; `linear_gradient_source_reaches_product_loader_semantics_and_raster`; `radial_gradient_source_reaches_product_loader_semantics_and_raster`; `radial_gradient_source_rejects_negative_radius`; `canonical_line_stroke_writer_reaches_product_render_loader`; `canonical_image_pattern_writer_reaches_product_render_loader`; `canonical_path_writer_reaches_product_render_loader`; `canonical_text_writer_reaches_product_render_loader`; CLI `render_manifest_source_and_product_paths_are_exercised` | Source-to-CanonicalCompilation-to-FCBC-to-Render loader for the bounded supported product subset, including FCBC linear-gradient kind 2, radial-gradient kind 3, canonical Line geometry/StrokeRecord, canonical ImagePattern paint writer/loader round-trips, canonical Path geometry/PathRecord writer/loader round-trips, and canonical Text geometry/GlyphRunRecord/font resource writer/loader round-trips. #503 exact-head PR Full Gate run 31263734355 on head db77489ba2eece0d4cdb930702772e2b3a382021 and post-merge push Full Gate run 31264032780 on merge head 288a74743c9526a4e4d7ade9c9886da0ef9468c7 both passed; #506 exact-head PR Full Gate run 31265522453 on head d124e71176c7317e9abf3c760352d24d767cf55c and post-merge push Full Gate run 31265766131 on merge head 7c92d5ffc1fcd383503107f7d81b60dfcf52f7a4 both passed; #509 exact-head PR Full Gate run 31281600305 (job 93163741093) on head 69d6a546e1c3284142e3c1f6a1f6be28423e2ae7 passed; the Path stroke follow-up Full Gate run 31285800196 (job 93174258827) passed on exact head a9eb9594fb70b507f5c7738857b63fbc234519aa; the Text/GlyphRun follow-up Full Gate run 31287462784 (job 93178786167) passed on exact head a6ff197c276e25f3092bd0d4410ff13f22f21db3. Full Render semantic/raster closure remains within the final #296/#9 gate. |
| Render semantic and raster domain | `product_render_write_load_eval_and_raster`; `semantic_query_honors_active_half_open_interval`; `semantic_query_propagates_node_transform_into_world_bounds`; `semantic_draw_ops_preserve_composite_and_inherited_clip_chain`; `solid_rect_raster_composites_multiple_rect_ops_in_draw_order`; `solid_rect_raster_applies_rect_clip_coverage`; `solid_core_shapes_rasterize_with_boundary_coverage`; `image_geometry_exposes_sampling_and_rasterizes_decoded_pixels`; `linear_gradient_pad_and_repeat_apply_at_declared_boundaries`; `linear_gradient_source_reaches_product_loader_semantics_and_raster`; `radial_gradient_solves_quadratic_and_applies_spread`; `radial_gradient_with_no_nonnegative_root_is_transparent`; `radial_gradient_source_reaches_product_loader_semantics_and_raster`; `image_pattern_paint_reaches_semantics_and_rasterizes_decoded_pixels`; `semantic_tests::image_pattern_transform_inverse_and_repeat_edges_are_stable`; `line_stroke_paint_reaches_semantics_and_rasterizes_coverage`; `semantic_tests::line_stroke_caps_and_dash_boundaries_are_stable`; `canonical_line_stroke_writer_reaches_product_render_loader`; `canonical_image_pattern_writer_reaches_product_render_loader`; `canonical_path_writer_reaches_product_render_loader` | Scene ordering, attachments, transforms, compositing, clipping, image sampling, linear/radial gradient and ImagePattern inverse-transform, repeat-axis, nearest/linear query-time evaluation, existing-FCBC Line stroke width/cap/dash coverage, canonical Line and ImagePattern writer/loader table binding, canonical Path geometry/PathRecord writer/loader table binding, solid-fill Path descriptor evaluation, curve/arc flattening, fill-rule containment, and RGBA raster evidence. #503 exact-head PR Full Gate run 31263734355 on head db77489ba2eece0d4cdb930702772e2b3a382021 and post-merge push Full Gate run 31264032780 on merge head 288a74743c9526a4e4d7ade9c9886da0ef9468c7 both passed; #506 exact-head PR Full Gate run 31265522453 on head d124e71176c7317e9abf3c760352d24d767cf55c and post-merge push Full Gate run 31265766131 on merge head 7c92d5ffc1fcd383503107f7d81b60dfcf52f7a4 both passed; #509 exact-head PR Full Gate run 31281600305 (job 93163741093) on head 69d6a546e1c3284142e3c1f6a1f6be28423e2ae7 passed. Solid-fill Path follow-up Full Gate run 31283895136 (job 93169439677) on exact head a32ac311fb3171064e5c67967f160f93ab133e9c passed; Path stroke semantic/raster and final I9 closure remain open within the #296/#9 gate. Text semantic/raster is recorded in the bounded delta row below. |
| Render Text semantic/raster delta | `source_product::canonical_text_writer_reaches_product_render_loader` | Loader-validated Text glyph outlines, GlyphRun metrics, Text origin, nonzero fill, and 8x8 reference raster coverage. Implemented as a bounded delta; GitHub Actions Full Gate run 31288628033 on exact head `b4169c9f26c86c8a090bd63f3c91db263a5e3658` passed. Source Text grammar/lowering, fallback/shaping beyond `simple-ltr-1`, Text stroke, and broader Render closure remain open. |
| Render Text stroke delta | `source_product::canonical_text_stroke_writer_reaches_product_render_loader` | Text fill/stroke/stroke-only writer handoff, decoded glyph contour stroke semantics, and non-empty raster coverage. Implemented as a bounded delta; GitHub Actions Full Gate run 31289376238 on exact head `825ecd480679f5a5348ef20a6df0424fa7b65167` passed. Source Text grammar/lowering, fallback/shaping beyond `simple-ltr-1`, and broader Render closure remain open. |
| Render Path stroke-only delta | `source_product::canonical_path_writer_reaches_product_render_loader` | Stroke-only Path writer handoff, decoded StrokeRecord binding, stroked-path semantic output, and non-empty raster coverage. Implemented as a bounded delta; GitHub Actions Full Gate run 31290037232 on exact head `f65e3300131758d886bf3557d4daddb85dc3aa39` passed. Source Path grammar/lowering, Polyline/Polygon stroke coverage, broader canonical writer coverage, and final Render closure remain open. |
| Render Polyline stroke delta | `source_product::canonical_polyline_stroke_writer_reaches_product_render_loader` | Stroke-only open Polyline writer handoff, decoded StrokeRecord binding, open-path semantic output, and non-empty raster coverage. Implemented as a bounded delta; GitHub Actions Full Gate run 31290853047 on exact head `cb8936f9faaecb83c397f8aed1c1d9bf707278ae` passed. Polygon stroke coverage, source stroke lowering, broader canonical writer coverage, and final Render closure remain open. |
| Render resource and limit boundaries | `every_public_render_limit_has_focused_boundary_evidence`; `group_and_clip_depth_overflow_stays_distinct_from_graph_cycles`; CLI `render_manifest_source_and_product_paths_are_exercised` | Resource identity/metadata, bounded decoder/loader/raster limits, and embedded opaque binding. The CLI binding path stops at compile/Core/FCBC framing and intentionally does not decode the opaque bytes; decodable Render semantics remain on explicit `inspect --render` and domain paths. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| Six public CLI commands | `version_reports_workspace_version`; `check_accepts_minimal_valid_source`; `format_uses_the_fixed_text_policy_and_preserves_canonical_chart`; `compile_executes_manifest_declared_canonical_fixtures_through_core_load`; `inspect_executes_every_fcbc_golden_through_declared_core_contract`; `convert_runs_public_pgr_fixture`; `report_executes_every_public_conversion_fixture` | `check`, `format`, `compile`, `inspect`, `convert`, and `report` are the fixed product surface. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| CLI product fixture delta | `check_executes_manifest_declared_canonical_fixtures`; `repository_fcs_examples_execute_at_their_applicable_product_boundaries`; `convert_executes_every_declared_public_export_reparse_fixture`; `render_manifest_source_and_product_paths_are_exercised`; `inventory_matches_product_metadata_and_registries` | Product-entry evidence for applicable canonical/example, Conversion, FCBC, Render, and distribution paths. Implemented; does not replace domain oracles or final cross-domain closure. |
| Fuzz smoke | `scripts/fcs5-fuzz-smoke.sh` targets: `document_bytes`, `document_utf8`, `expression`, `fcbc_container`, `render_section`, `asset_image`, `asset_font`, `import_pgr`, `import_rpe`, `import_pec` | Bounded `FCS_FUZZ_RUNS=1024` contract. Implemented; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success and not run locally. |
| Artifact and metadata agreement | `distribution::inventory_matches_product_metadata_and_registries`; `docs/conformance/fcs5/distribution.toml` `package_required_files`, `utf8_paths`, `contribution_policy_file`, and `license_sha256`; Cargo package listings | Binds the AGPL-3.0-or-later root license text and hash, workspace package metadata, DCO plus inbound=outbound policy, seven package required paths, and UTF-8 conformance inputs. Implemented locally in source; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |

## Matrix reconciliation

The current matrix contains no data row with `blocked-by-I<n>` status. Every
current `partial` row and each active product row is accounted for below; the
reconciliation records evidence coverage and does not promote a row to
`implemented` or a version domain to Frozen.

| Matrix row | Ledger evidence | Unresolved status or gate |
|---|---|---|
| `fcs.md` 4.5 | Source elaborate boundary; Core profiles and resource/compile limits | Runtime environment, lazy runtime selection, and exact DAG work remain in the final #296/#9 gate; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 5.1–5.2 | Source canonical boundary; Render source and canonical product paths | Remaining Render/profile closure remains in the final #296/#9 gate; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 7.1–7.5 | Source canonical boundary; Artifact and metadata agreement | Canonical resource/metadata residuals remain in the final #296/#9 gate; artifact contract is bounded but final I10 closure remains open. |
| `fcs.md` 8.1–8.3 | Source evaluate boundary; Runtime properties | Later runtime assembly and version-domain work remain in the final #296/#9 gate; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 10.1–10.4 | Source evaluate boundary; Runtime properties | Remaining DAG/Piecewise and stage-scoped runtime closure remain in the final #296/#9 gate; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 11.1–11.5 | Source evaluate boundary; Runtime properties; Runtime ABI execution | Remaining runtime/error-isolation and cross-domain assembly remain in the final #296/#9 gate; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 12.1–12.5 | Source canonical boundary; Runtime ABI execution | Remaining runtime visibility/resource closure remains in the final #296/#9 gate; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 13.1–13.4 | Runtime ABI execution; Runtime properties | Randomized property and FCBC/runtime assembly work remains in the final #296/#9 gate; the exact-head Full Gate passed (run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2). |
| `fcs.md` 14.1–14.3 | Runtime ABI execution; Runtime properties | I4.9 property evidence exists, but stage and version-domain closure remain in the final #296/#9 gate; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 15.1–15.3 | Source parse boundary; Source canonical boundary; Normative source examples and grammar | Fidelity/repair/Render semantics remain in the final #296/#9 gate; Appendix A is bound to the manifest and focused elaboration/canonical plus CLI canonical/Core-load evidence; Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 success. |
| `fcs.md` 17 | Canonical compilation and FCBC handoff; Artifact and metadata agreement | Product subset is covered; complete cross-domain closure remains open in the final #296/#9 gate. The exact-head Full Gate passed (run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2). |
| `fcs.md` 18 | All source-stage rows; Runtime properties; Fuzz smoke | Domain runners and bounded targets are implemented; runtime/property/fuzz evidence passed on the final SHA (run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2). |
| `fcbc.md` all | FCBC goldens; FCBC mutations; Runtime ABI execution | Product and mutation surfaces are covered; the exact-head Full Gate passed (run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2) and final cross-domain review remains pending. |
| `fcs-render.md` all | Render source and canonical product paths; Render semantic and raster domain; Render resource and limit boundaries | Full Render scene/semantic/raster closure remains partial within the final #296/#9 gate; exact-head Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 passed. |
| `fcs-conversion.md` all | Conversion registries and six public fixtures; Conversion export and semantic reparse; Conversion report, authorization, and hard limits | Remaining conversion-stage and cross-domain closure remains partial within the final #296/#9 gate; exact-head Full Gate run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2 passed. |
| I10.1–I10.5 | Six public CLI commands; CLI product fixture delta; Artifact and metadata agreement | Bounded artifact contract is implemented and unpublished; all 32 canonical fixtures now execute through CLI `check`, with exact-head Full Gate run 30729340879 on e0140f7ca15cc21620ef3ad8e44b4ece03a346c2; final cross-domain audit, joint review, and re-freeze remain open under #296/#9; #452 and #489 are completed delivery units. |

## Remaining I10 gates

This ledger does not close I10. The current CLI work-unit SHA passed the GitHub Full Gate (run 30729340879 on head e0140f7ca15cc21620ef3ad8e44b4ece03a346c2), but the remaining acceptance gates are the final
cross-domain execution audit, independent joint review, and five-domain
Frozen/re-freeze evidence. Issues #296 and #9 remain open until those gates are
independently evidenced.
