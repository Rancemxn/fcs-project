# FCS 5 Source Production Coverage Ledger

This ledger is the I1.8 production-coverage evidence for the active `fcs-source` parser. It records
parser-boundary evidence only; it does not promote a source shape to static, canonical, Render, or
runtime semantics. `conformance_manifest::fcs_source_fixtures_execute_at_the_declared_frontend_boundary`
executes all 39 FCS manifest entries (3 parse-success, 9 parse-error, and 27 later-stage syntax-acceptance
entries).

## Evidence keys

| Key | Evidence |
|---|---|
| `C` | `crates/fcs-source/tests/conformance_manifest.rs`: manifest-driven execution of every bound source entry. |
| `G` | `source_ast::complete_source_grammar_fixture_parses_with_all_top_level_kinds`. |
| `A` | `crates/fcs-source/tests/source_ast.rs` typed AST, Track, metadata, extension/preserve, and boundary tests. |
| `E` | `crates/fcs-source/tests/expression.rs` and `compile_time.rs` expression/type/statement tests. |
| `D` | `crates/fcs-source/tests/diagnostic.rs` and `frontend.rs` category, span, malformed, duplicate, recovery, and trailing-input tests. |
| `R` | `crates/fcs-source/tests/robustness.rs` byte/UTF-8, limit, span, determinism, and no-panic properties. |
| `L` | Later-stage semantic-invalid fixtures in `C`: they must parse successfully; their semantic error is an explicit later-stage boundary, not an I1 parser failure. |

`C` is the valid/invalid fixture reference for productions that are exercised by the complete corpus. `D`
is the invalid syntax reference for malformed delimiters, tokens, declarations, and source-structure
boundaries. `L` is used where an invalid semantic fixture is intentionally legal source syntax. A row with
both `D` and `L` distinguishes parser-invalid input from later-phase-invalid input rather than pretending
that one phase owns the other.

## Complete source examples

The repository's complete FCS examples are each parsed by an owning test; fragmentary production snippets
remain wrapped in the smallest legal document/block fixture.

| Input | Parse evidence | Boundary |
|---|---|---|
| `examples/fcs/fragment.fcs` | `frontend::parses_public_fcs5_fixtures` | complete fragment document |
| `examples/fcs/chart.fcs` | `frontend::parses_public_fcs5_fixtures` | complete chart document with tempo map |
| `examples/fcs/templates.fcs` | `compile_time::parses_and_elaborates_the_public_template_fixture` | complete template/collection document |
| `docs/conformance/fcs5/source/valid/complete-source-grammar.fcs` | `source_ast::complete_source_grammar_fixture_parses_with_all_top_level_kinds` and `conformance_manifest::fcs_source_fixtures_execute_at_the_declared_frontend_boundary` | complete Appendix B envelope |
| all 50 entries in `docs/conformance/fcs5/manifest.toml` | `conformance_manifest::fcs_source_fixtures_execute_at_the_declared_frontend_boundary` | 3 parse-success, 9 parse-error, 38 later-stage syntax-acceptance entries; owning canonical/evaluate tests execute the applicable later boundary |

## Document, format, and lexical envelope

| Appendix B production(s) | Valid evidence | Invalid or boundary evidence |
|---|---|---|
| `document`, `header`, `semver` | `C`, `G`, `D::parses_exact_fcs5_header` | `D::bound_parse_error_fixtures_keep_stable_categories_and_spans`, `D::missing_header_has_the_frozen_code_and_byte_span`, `R` |
| `topLevelBlock` | `G`, `C` | `D::document_boundary_diagnostics_are_stable_and_spanned`, duplicate/unknown/misplaced cases in `C` |
| `formatBlock`, `formatField` | `G`, `D::parses_fragment_profile` | `D::document_boundary_diagnostics_are_stable_and_spanned`, duplicate-field tests |
| `profile`, `featureArray`, `profileFeature` | `G`, `D::format_features_are_retained_in_the_source_ast` | `D::rejects_unknown_profile`, malformed feature/terminator cases |
| `bom`, `asciiSpace`, `newline`, `identifier`, `keyword` | `D::header_immediately_follows_the_optional_bom`, lexer keyword tests | `D::additional_bom_and_non_ascii_identifier_spans_are_exact`, `D::nul_and_unicode_noncharacters_obey_the_lexical_boundary` |
| `uintMagnitude`, `floatMagnitude`, `stringLiteral`, `colorLiteral` | `E::parses_scalar_and_unit_literals`, string/color lexer tests | `D::malformed_numeric_candidates_are_one_lexical_error`, `D::malformed_color_string_and_comment_spans_are_stable` |

## Metadata, schema, time, and resources

| Appendix B production(s) | Valid evidence | Invalid or boundary evidence |
|---|---|---|
| `metaBlock`, `artworkBlock`, `syncBlock` | `A::metadata_schema_ast_retains_ordered_declarations_and_spans`, `G` | `C` parse-stage/semantic-invalid split; duplicate and malformed schema cases in `D` |
| `contributorsBlock`, `contributorDecl` | `A`, `G` | `C` later-stage schema/resource-invalid inputs (`L`) |
| `creditsBlock`, `creditDecl` | `A`, `G` | `C` later-stage schema-invalid inputs (`L`) |
| `resourcesBlock`, `resourceDecl`, `resourceKind` | `A::every_core_resource_kind_has_a_typed_source_node`, `G`, `resource_bundle::builds_deterministic_opaque_bundle_without_path_or_content_deduplication`, `conformance_manifest::i5_resource_fixtures_execute_at_the_workspace_bundle_boundary` | `L::unknown-resource`, `L::resource-path-escape`, `L::resource-hash-mismatch`, `L::resource-missing-member`, `resource_bundle::{rejects_missing_directory_and_non_regular_workspace_members,accepts_in_root_symlink_and_rejects_symlink_escape,enforces_public_count_single_and_total_byte_budgets}`; filesystem/hash checks execute only at the explicit canonical bundle boundary, never in the parser |
| `tempoMapBlock`, `tempoPoint`, `bpmLiteral` | `D::source_parser_retains_tempo_maps_for_later_validation`, `G` | `D::rejects_removed_mixed_beat_literal`, `L` for sign/order/profile validity |
| `schemaBlock`, `schemaField`, `schemaValue` | `A::metadata_schema_ast_retains_ordered_declarations_and_spans`, `G` | `D::extension_payload_duplicate_keys_remain_ordered_and_unbalanced_envelopes_fail`, `L::custom-duplicate-key` |
| `fieldPath`, `fieldName` | `A`, `E::parser_supports_references_index_postfix_and_keyword_field_names` | `D::additional_bom_and_non_ascii_identifier_spans_are_exact`, malformed field-path cases |

## Definitions and statements

| Appendix B production(s) | Valid evidence | Invalid or boundary evidence |
|---|---|---|
| `definitionsBlock`, `definition` | `A::definitions_retain_else_if_as_a_nested_typed_statement`, `G` | `D::malformed_definition_body_does_not_swallow_following_declaration` |
| `constDecl`, `functionDecl`, `templateDecl` | `E::parses_typed_templates_and_collections_with_source_spans`, `G` | `E::requires_a_return_on_every_function_path` (later elaboration), malformed statement tests in `A` |
| `parameters`, `parameter` | `E::parses_typed_templates_and_collections_with_source_spans` | malformed definition/function declaration cases in `A` and `D` |
| `statementBlock`, `templateBlock` | `E`, `A` | `A::definition_bodies_reject_owner_invalid_generator_and_entity_statements` |
| `functionStatement`, `templateStatement` | `E`, `A` | owner-invalid statement cases in `A` |
| `letDecl`, `functionIf`, `templateIf` | `E::generator_body_retains_typed_let_and_nested_statement_spans`, `A` | malformed-body and misplaced-generator cases in `D`/`C` |
| `returnValue`, `returnEntity` | `E`, `A` | `A::template_returns_reject_value_only_expression_forms`, owner-invalid return cases |

## Lines, collections, and generators

| Appendix B production(s) | Valid evidence | Invalid or boundary evidence |
|---|---|---|
| `linesBlock`, `lineDecl` | `A::track_ast_retains_settings_direct_segments_points_and_spans`, `G` | `L::track-overlap`, `D` malformed declaration/recovery cases |
| `collectionsBlock`, `collection`, `collectionName` | `A::collection_generators_retain_their_owner_context`, `G` | `C::nested-generator`, `C::misplaced-generator`, `D` |
| `collectionItem`, `collectionIf` | `A`, `E` | owner-placement and malformed-body cases in `A`/`D` |
| `generator`, `rangeType`, `rangeOperator` | `A::track_generators_retain_track_owner_and_schema_cubic_values`, `E` | `C::bare-range`, `L::generator-zero-step`; zero-step is not a parser error |
| `generatorStatement`, `generatorIf`, `emitStatement` | `E::generator_body_retains_typed_let_and_nested_statement_spans`, `A` | `C::nested-generator`, `C::misplaced-generator`, owner-invalid cases in `A` |

## Entity, Track, and interpolation source shapes

| Appendix B production(s) | Valid evidence | Invalid or boundary evidence |
|---|---|---|
| `entityExpression`, `entityPrimary` | `A`, `E`, `G` | `L::runtime-gameplay`, `L::template-missing-line` |
| `entityConstructor`, `noteVariant` | `A`, `E` | `L::note-policy-disabled-sound`, `L::hold-end` |
| `entityBlock`, `tracksBlock`, `trackDecl` | `A::track_ast_retains_settings_direct_segments_points_and_spans`, `G` | `L::track-overlap`, malformed/unclosed group recovery in `D` |
| `trackSetting`, `segmentsBlock`, `segmentItem` | `A`, `G` | `D::document_recovery_reports_independent_errors_without_partial_output`, `L::track-overlap` |
| `segmentIf`, `directSegment`, `directPoint` | `A::track_ast_retains_settings_direct_segments_points_and_spans` | `L::track-overlap`, malformed interval cases in `D` |
| `halfOpenInterval` | `A::track_ast_retains_settings_direct_segments_points_and_spans` | `C::bare-range`, ordinary array misuse is rejected by `E`/`D` |
| `interpolation`, `cubicBezierValue` | `A::track_generators_retain_track_owner_and_schema_cubic_values` | malformed expression/constructor cases in `D`/`E` |
| `scrollTempoMapBlock`, `scrollTempoPoint` | `A`, `G` | `L` for tempo ordering/profile semantics; `D::source_parser_retains_tempo_maps_for_later_validation` |

## Extensions, preserve, and Render envelope

| Appendix B production(s) | Valid evidence | Invalid or boundary evidence |
|---|---|---|
| `extensionsBlock`, `extensionDecl`, `extensionHeader`, `extensionRequirement` | `A::extension_preserve_and_render_envelopes_retain_order_and_balanced_spans`, `G` | `C::unclosed-extension-payload`, `D::extension_payload_duplicate_keys_remain_ordered_and_unbalanced_envelopes_fail` |
| `preserveBlock`, `preserveItem`, `preserveSource`, `preservePayload` | `A`, `G` | unbalanced/truncated preserve cases in `A`/`D`; missing/duplicate cardinality is `L` for later schema validation |
| `renderBlock`, `balancedTokenBlock`, `balancedToken` | `A`, `G` | `A::extension_preserve_and_render_envelopes_retain_order_and_balanced_spans`, truncated envelope test |
| `balancedParenGroup`, `balancedBracketGroup`, `nonDelimiterToken` | `A`, `D`, `R` | unbalanced delimiter and trailing-input tests in `A`/`D`/`R` |

## Expressions, literals, and types

| Appendix B production(s) | Valid evidence | Invalid or boundary evidence |
|---|---|---|
| `expression`, `logicalOr`, `logicalAnd`, `equality`, `ordering` | `E::token_parser_preserves_frozen_precedence_and_spans`, operator tests | `E::token_parser_rejects_reserved_names_and_trailing_input`, malformed expression tests in `D` |
| `sum`, `product`, `power`, `unary`, `postfix` | `E::parses_every_binary_operator`, `E::parses_unary_operators_before_postfix_and_binary_operators` | `E::power_is_right_associative`, trailing/incomplete expression tests |
| `primary`, `literal`, `booleanLiteral`, `nullLiteral`, `numberLiteral` | `E::parses_scalar_and_unit_literals`, literal lexer tests | `D::malformed_numeric_candidates_are_one_lexical_error`, non-finite/raw-scalar cases |
| `unitLiteral`, `unitSuffix` | `E::parses_scalar_and_unit_literals` | `D::invalid_unit_adjacency_is_one_lexical_error` |
| `vec2Constructor`, `arguments`, `reference` | `E::parses_names_calls_fields_parentheses_and_vec2_construction`, reference/index tests | malformed call/reference/trailing cases in `E`/`D` |
| `array`, `object` | `E::parser_accepts_empty_array_source_nodes`, `E::parser_preserves_ordered_object_entries_and_duplicate_keys` | `E::parser_rejects_object_keys_that_are_not_string_literals`, malformed delimiter tests |
| `chooseExpression`, `chooseArm`, `elseArm` | `E::parser_preserves_choose_arm_order_and_else_value` | `E::parser_requires_choose_when_arms_and_else`, malformed expression tests |
| `type`, `scalarType`, `entityType`, `constructibleType` | `E::parses_nested_type_syntax`, `E::parses_scalar_and_recursive_track_types` | `E::type_parser_exposes_the_same_bounded_diagnostic_boundary`, static type-invalid fixtures are `L` |

## Limit decision

The current grammar has no unbounded AST/list allocation path outside the already published lexer and
parser limits. `R::every_public_parser_limit_has_exact_boundary_evidence` and
`R::every_parser_limit_has_a_bounded_failure` cover `max_source_bytes`, `max_tokens`, `max_token_bytes`,
`max_nesting_depth`, `max_comment_depth`, and `max_literal_bytes` with structured
`resource.limit-exceeded` diagnostics, bounded spans, and no partial output. No new AST/list budget is
introduced by this ledger; adding one later requires a separate public limit contract and limit-1/limit/
limit+1 evidence rather than silently relying on an implementation allocation bound.

The deterministic property and independent fuzz-lane audit is closed by I1.8c/#38. The final evidence is
the fixed-seed 12-property robustness lane plus the three-target bounded libFuzzer smoke over the 42-seed
corpus; the unbounded command remains local-only and is not required for the I1 gate.
