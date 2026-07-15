# FCS 5 规范—实现—测试矩阵

状态日期：2026-07-15

本矩阵记录权威规范与参考实现之间的可审计关系。它不定义格式语义；发生冲突时以
`fcs.md`、`fcbc.md`、`fcs-render.md`、`fcs-conversion.md` 和绑定 conformance corpus 为准。

2026-07-15 S14 因项目未公开且兼容成本为零，重开 FCS Core/Render Source grammar：closed enum
统一为 string、extension/preserve/render envelope 闭合、mixed Beat 删除，并明确前端阶段归属。
I0 基础设施验收不回退，但凡旧 retained behavior 与新闭合规范不符的行均重新标为 `partial`，
由 I1 按原 32-entry grammar baseline 修正；不得用历史 135-test 结果宣称这些新条款已实现。

S14 之后接受的 ADR 0007–0009 又重开了 Core、FCBC、ABI、Render 与 Conversion 的跨规范边界。
S14 grammar closure 仍是有效的范围化证据，但当前候选状态以
`docs/specification-governance.md` 为准；在新的规范 diff、fixture 和 hash 完成复审前，本矩阵不得
把旧 Frozen/Reviewed 记录当作当前完整 conformance baseline。

S15 联合候选自检、当前 root/suite/tree hash 和未关闭 blocker 见
`docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md`。该 review 明确没有 canonical/FCBC/ABI/
converter/Render 实现证据，不得用其 135 个 I0 tests 提升下表实现状态。

FCS authoring/canonical closure 又新增 7 项 canonical/error fixture，当前 manifest 为 39 entries。
这些条目只建立未来 runner 的规范绑定；除 manifest/path integrity 外，I0 没有实现 workspace
resource resolution、Note policy normalization、CanonicalCompilation 或 exact DAG lowering。

S15 Conversion closure 又建立 12 个 content-hash-bound semantic profile、7 个 parser dialect、56 个
mapping rule、32 个 diagnostic/report category、38 个 exact mapping vector、5 个 invalid vector 与
10 个 selection/ambiguity vector。
当前 Rust test 只强类型加载这些 manifest、复算 descriptor/contract SHA-256 并检查跨引用；活动
workspace 没有外部格式 parser、profile selector、canonical converter 或 target reparse comparator。

I0.1–I0.9 已完成：活动 `master` 只有无版本前缀的 `crates/fcs-source`，source subset 使用
Chumsky 0.11.2 的单一 spanned-token 数据流，并具备稳定诊断、严格 byte decode、固定配置
Proptest robustness 和强类型 manifest 完整性门。当前 workspace 有 135 个通过的测试；这只
证明 I0 retained subset 和治理门，不表示全部 FCS 5 source/canonical/runtime conformance 已完成。

当前进度证据：`archive/fcs4-pre-cutover` 固定在
`148936d17b671bb34968c88969ab748c818f9fc0`；唯一 crate cutover 为 `16e7db3`，稳定诊断为
`0185d8e`，Chumsky lexer/expression/document 迁移为 `7442210`、`cc8d94f`、`0b17e56f`，Frozen
generator parser 边界为 `9d88a6a`，raw lexer prepass 清理为 `475e137`。I0.9 最终结构、
依赖、质量、归档拓扑和独立复审 gate 已通过。

允许的状态只有：`implemented`、`partial`、`not-started` 和 `blocked-by-I<n>`。`implemented`
表示该行所列 I0 能力已有实际测试证据；`partial` 必须在“已知偏差”列写明缺失行为和接续阶段。

| 规范条款 | 能力 | Public API | 实现文件 | Valid fixture/test | Invalid fixture/test | 现状 | 下一阶段 | 已知偏差 |
|---|---|---|---|---|---|---|---|---|
| `fcs.md` 2.1 | UTF-8、BOM、换行 | `parser::parse_document_bytes` | `parser/document.rs`, `parser/lexer.rs` | `robustness::byte_entry_decodes_once_and_preserves_utf8_error_spans`, `source.valid.escaped-nul-string`（manifest only） | `robustness::arbitrary_bytes_never_escape_decode_or_parse_boundaries` | partial | I1 | 严格 UTF-8、BOM/CRLF byte span 已覆盖；raw U+0000、escaped `\0` 和 raw/escaped noncharacter 的闭合规则尚未完整实现 |
| `fcs.md` 2.2 | 精确 source header 与版本拒绝 | `parser::parse_header` | `parser/header.rs`, `parser/lexer.rs`, `version.rs` | `frontend::parses_exact_fcs5_header`, `source.valid.minimal-fragment` | `source.invalid.missing-header`, `source.invalid.header-extra-space`, `source.invalid.header-leading-zero`（后两项 manifest only） | partial | I1 | major/minor/patch category 已覆盖；S14 新增的恰好一个 ASCII space、连续三段 semver/float token 优先级与前导零规则尚未逐项实现 |
| `fcs.md` 2.3 | 空白、行注释、nested block comment | `parser::parse_document` | `parser/lexer.rs` | `lexer::nested_comments_and_string_escapes_are_deterministic`, `workspace_structure::lexer_has_no_raw_text_preparser` | `frontend::rejects_unclosed_trailing_block_comment` | implemented | I1 | retained subset 的 trivia 与 nested-comment contract 已完成；I1 复用同一 lexer 接入完整 grammar |
| `fcs.md` 2.4 | ASCII identifier、keyword 与 field-name context | `parser::parse_expression`, `parser::parse_document` | `parser/token.rs`, `parser/lexer.rs` | `compile_time::identifiers_are_ascii_but_spans_remain_utf8_byte_offsets` | `expression::token_parser_rejects_reserved_names_and_trailing_input`, `source.invalid.unresolved-schema-enum`（manifest only） | partial | I1 | ASCII identifier 已覆盖；S14 keyword 集、keyword field path 和“不猜测 bare enum”尚未实现 |
| `fcs.md` 2.5–2.7 | Numeric/String/Color 与单位 literal | `parser::parse_expression` | `parser/lexer.rs`, `ast/types.rs`, `ast/color.rs` | `compile_time::parses_scalar_and_unit_literals`, `compile_time::string_escapes_match_the_documented_table` | `compile_time::unit_literals_must_remain_finite_after_conversion`, `source.invalid.mixed-beat-literal`（manifest only） | partial | I1 | I0 scalar/unit/escape/Color subset已覆盖；负号独立 token、exact magnitude、mixed-Beat 删除和 noncharacter escape 尚未实现 |
| `fcs.md` 2.8–2.10 | 分隔符、array/object/reference、半开 interval、closed enum | `parser::parse_expression` | 未来扩展 `parser/expression.rs`, `ast/types.rs` | `source.valid.track-boundaries`, `source.valid.complete-source-grammar`（manifest only） | `source.invalid.unresolved-schema-enum`（manifest only） | blocked-by-I1 | I1 | token 已保留部分分隔符；array/object/reference、唯一半开 interval 与 schema-only cubic Bezier AST、string enum 和 keyword field access 尚未实现 |
| `fcs.md` 3.1–3.4 | 基础类型、单位和显式转换边界 | `ast::Type`, `ast::SourceLiteral` | `ast/types.rs`, `elaborator/eval.rs` | `compile_time::phase2_type_display_uses_canonical_spellings` | `compile_time::requires_exact_declared_and_return_types` | partial | I1/I2 | scalar、`vec2`、`TrackSegment`、`Keyframe` 类型形状已实现；`array`/`Track` source 类型与完整显式转换矩阵缺失 |
| `fcs.md` 4.1 | 表达式优先级和结合性 | `parser::parse_expression` | `parser/expression.rs` | `expression::token_parser_preserves_frozen_precedence_and_spans` | `compile_time::parser_rejects_trailing_or_incomplete_input` | partial | I1 | I0 retained primary/postfix/unary/power/binary 层已从同一 token stream 解析；array/object/reference/interval/choose primary 缺失 |
| `fcs.md` 4.2–4.4 | 运算矩阵、builtin、字段访问 | `elaborator::elaborate` | `elaborator/eval.rs` | `compile_time::types_and_evaluates_phase2_pure_operators` | `compile_time::evaluates_fixed_builtins_and_diagnoses_bad_calls` | partial | I2 | I0 subset 支持纯运算、short-circuit、固定 builtin 和字段访问；power elaboration及完整 Frozen operator/builtin 矩阵缺失 |
| `fcs.md` 4.5 | Runtime value 边界 | `elaborator::elaborate` | `elaborator/eval.rs`, `elaborator/entities.rs` | `compile_time::compile_time_collection_if_selects_one_branch_and_rejects_runtime_conditions` | `source.invalid.runtime-gameplay`（manifest only） | partial | I2/I4 | I0 能拒绝结构位置的非 compile-time value；完整 runtime value 类型、dynamic-field whitelist 与 DAG 在 I2/I4 实现 |
| `fcs.md` 5.1–5.2 | Document、format、profile | `parser::parse_document` | `parser/document.rs` | `source.valid.minimal-fragment`, `frontend::parses_fragment_profile` | `frontend::document_rejects_misplaced_or_duplicate_top_level_blocks` | partial | I1 | I0 精确解析 header、profile、tempo/definitions/collections subset；format features 及其余 Frozen top-level blocks 明确拒绝并留给 I1 |
| `fcs.md` 5.3 | definitions 中的 const/fn/template | `ast::DefinitionsBlock` | `ast/definitions.rs`, `parser/definitions.rs` | `compile_time::parses_definitions_with_global_byte_spans` | `compile_time::template_declaration_does_not_consume_following_definitions` | partial | I1/I2 | const/fn/template 已统一位于 definitions 并保留 span；完整 source type/statement 与全分支静态检查仍缺失 |
| `fcs.md` 5.4 | collections 与 source order | `ast::CollectionBlock` | `ast/entity.rs`, `parser/entities.rs` | `compile_time::collection_blocks_retain_forward_compatible_items_and_spans` | `frontend::duplicate_optional_top_level_blocks_report_both_declarations` | partial | I1/I2 | collection 顺序和 retained Note/Line subset 已保留；完整 collection/schema/Track owner grammar 与静态检查缺失 |
| `fcs.md` 6.1 | 绑定、作用域和禁止 shadowing | `elaborator::elaborate` | `elaborator/scope.rs` | `compile_time::rejects_duplicate_bindings_but_allows_sibling_branch_names` | `compile_time::rejects_shadowing_in_nested_scope`, `source.invalid.shadowing` | partial | I2 | I0 subset 已区分 duplicate/shadowing 并覆盖 global/builtin 冲突；完整跨 source node scope graph 缺失 |
| `fcs.md` 6.2 | 纯函数、return、调用图 | `elaborator::elaborate` | `elaborator/eval.rs`, `elaborator/cycle.rs` | `compile_time::elaborates_const_and_pure_function` | `compile_time::requires_a_return_on_every_function_path`, `compile_time::detects_const_and_function_cycles_before_evaluation` | partial | I2 | retained function subset 具备 forward reference、路径 return 与结构化 cycle trace；完整类型、builtin 和未来 AST 调用图缺失 |
| `fcs.md` 6.3–6.5 | typed template、constructor、with、if | `elaborator::elaborate` | `elaborator/entities.rs` | `compile_time::parses_and_elaborates_the_public_template_fixture` | `compile_time::entity_elaboration_reports_schema_and_template_errors` | partial | I2 | I0 Note/Line schema subset 可展开 template/with/if 且输出 lowered values；完整 schema、全分支静态检查和共享 budget context 缺失 |
| `fcs.md` 6.6 | `..<`/`..=` range 与 zero step | `ast::Generator` | `parser/entities.rs`, `ast/entity.rs` | `compile_time::parses_only_frozen_generator_range_operators` | `compile_time::bare_range_uses_the_frozen_syntax_category`, `source.invalid.generator-zero-step`（manifest only） | partial | I2 | parser 只接受 `int`/`beat` 与两种 Frozen range operator；step 表达式求值、类型一致性、方向和 zero-step 诊断尚未实现 |
| `fcs.md` 6.7 | generator body 与 typed emit | `ast::GeneratorItem` | `parser/entities.rs`, `ast/entity.rs`, `elaborator/entities.rs` | `compile_time::generator_body_retains_typed_let_and_nested_statement_spans` | `compile_time::rejects_return_and_nested_generate_in_generator_body` | partial | I1/I2 | typed let/if/emit 语法和 no-partial-output boundary 已实现；I1 补 nested/misplaced Frozen category 与 Track owner grammar，I2 实现 typed expansion |
| `fcs.md` 6.8 | 六类共享 elaboration budget、trace 与 authoring-only 消除 | `elaborator::CompileTimeLimits` | `elaborator/mod.rs`, `diagnostic.rs` | `compile_time::template_cycle_detection_scans_template_bodies_before_expansion` | `source.invalid.generator-budget`, `source.valid.canonical-equivalent-template`（manifest only） | partial | I2 | I0 已有表达式/调用/展开 limits 与结构化 trace 形状；六类共享计数、完整 concrete ExpandedSourceDocument 和 preserve/template/local 消除尚未实现 |
| `fcs.md` 7.1–7.5 | Metadata、credits、workspace resources、sync、custom | 无 | 未来 `ast/metadata.rs`、`fcs-model` 与 resource resolver | `source.valid.metadata-credits-resources-sync`, `source.valid.note-policies`（manifest only） | `source.invalid.unknown-resource`, `source.invalid.resource-path-escape`, `source.invalid.resource-hash-mismatch`, `source.invalid.custom-duplicate-key`（manifest only） | blocked-by-I1 | I1/I3/I5 | source AST/parser、workspace logical-path resolver、opaque bytes/SHA-256 和 CanonicalResourceBundle 均未实现 |
| `fcs.md` 8.1–8.3 | chartTime、tempo、offset、judgment/scroll 与 import-time source clocks | `ast::TempoMap` | `parser/tempo.rs`, `validation.rs`, 未来 `fcs-model` | `frontend::i0_retained_tempo_parser_accepts_mixed_beat_pending_i1_removal`（characterization only） | `frontend::tempo_points_must_be_non_decreasing`, `source.invalid.mixed-beat-literal`（manifest only） | partial | I1/I3/I6 | I0 parser 仍接受未定义 mixed Beat，并在 Parse stage 提前执行 zero-start/monotonic/profile validation；I1 必须移除 characterization path，I3 规范化 chartTime，I6 按 profile 在 import-time 解码外部时钟 |
| `fcs.md` 9.1–9.5 | Track schema、segment/point、blend、边界 | 无 | 未来 source Track AST 与 `fcs-model` | `source.valid.track-boundaries`（manifest only） | `source.invalid.track-overlap`（manifest only） | blocked-by-I1 | I1/I3 | Frozen Track source/canonical model 尚未实现 |
| `fcs.md` 10.1–10.4 | Scroll coordinate、speed、distance、exact integrand/积分 | 无 | 未来 `fcs-model`, `fcs-runtime` | `source.valid.time-scroll-note`（manifest only） | numeric/runtime vectors（manifest only） | blocked-by-I3 | I3/I4 | canonical scroll model、analytic/evaluable integration descriptor 与 reference evaluator 尚未开始；标准路径不得预采样 floorPosition |
| `fcs.md` 11.1–11.5 | 坐标、Line、parent DAG、inherit、排序 | 无 | 未来 `fcs-model` | `source.valid.parent-transform`（manifest only） | `source.invalid.parent-cycle`（manifest only） | blocked-by-I3 | I3/I4 | I0 仅有 constructible Line identity subset；canonical graph 与 transform 未开始 |
| `fcs.md` 12.1–12.5 | Note gameplay/presentation/Hold/policy/排序 | source constructor subset | `schema.rs`, `elaborator/entities.rs`, 未来 `fcs-model` | `compile_time::phase2_note_schema_has_exact_fields_required_flags_and_variants`, `source.valid.note-policies`（manifest only） | `source.invalid.hold-end`, `source.invalid.note-policy-disabled-sound`（manifest only） | partial | I2/I3 | I0 Note schema 是 retained subset；judgeShape object、sound/score policy normalization、required extension/resource、Hold、canonical ID 与排序缺失 |
| `fcs.md` 13.1–13.4 | Runtime expression、环境、lazy choose 与 exact DAG lowering | source expression subset | 未来 `fcs-runtime` | `source.valid.runtime-choose`, `source.valid.exact-expression-dag`（manifest only） | environment/cycle tests（未接入） | blocked-by-I1 | I1/I4 | choose source node、runtime environment 和 DAG 均未实现；I0 只有 compile-time logical short-circuit，不能据此生成 BakedCurve |
| `fcs.md` 14.1–14.3 | binary64、explicit approximation、player-local sampled boundary、Core easing | 无 | 未来 `fcs-runtime` 与 converter approximation validator | `expected/numeric-vectors.toml`, `source.valid.exact-expression-dag`（manifest only） | error-budget tests（未接入） | blocked-by-I4 | I4 实现 exact evaluator；target approximation 属 I8/converter，播放器 sampled cache 不进入规范实现状态 |
| `fcs.md` 15.1–15.3 | Extension、fidelity、preserve消除、repair | 无 | 未来 source extension AST 与 `fcs-model` | `source.valid.complete-source-grammar`, `source.valid.canonical-equivalent-template`（manifest only） | `source.invalid.unclosed-extension-payload`（manifest only） | blocked-by-I1 | I1/I5 | S14 已定义 envelope；I1 保留 source AST，I2/I3 必须从 CanonicalChart 消除 raw preserve，I5 只提取非原文 provenance fact |
| `fcs.md` 16 | 稳定 diagnostic categories | `diagnostic::{Diagnostic, DiagnosticCode}` | `diagnostic.rs` | `diagnostic::diagnostics_are_sorted_by_span_then_code` | `diagnostic::missing_header_has_the_frozen_code_and_byte_span`, `diagnostic::parser_resource_limits_use_the_stable_resource_code` | implemented | I1–I9 | S14 正式绑定既有 `resource.limit-exceeded` 及其 parser/compiler limit 边界；后续阶段只在实现对应语义时激活其他已声明 category |
| `fcs.md` 17 | Expanded source、CanonicalCompilation 与 FCBC handoff | `ast::ExpandedSourceDocument` | `ast/entity.rs`, `elaborator/entities.rs`, 未来 `fcs-model` | `compile_time::expanded_ir_exposes_only_read_accessors`, canonical-equivalent pair（manifest only） | `compile_time::generator_elaboration_fails_before_partial_output` | partial | I2/I3/I5 | I0 retained subset 输出 concrete typed fields；完整 authoring消除、CanonicalChart/ResourceBundle/DistributionMetadata 和 exact descriptor lowering 缺失 |
| `fcs.md` 18 | Source/static/canonical/runtime conformance runner | manifest integrity API（test only） | `tests/conformance_manifest.rs` | `conformance_manifest::typed_manifests_load_with_bound_counts` | `conformance_manifest::manifests_preserve_integrity_invariants` | partial | I1–I5 | Root/FCS/FCBC/Render/Conversion manifest schema 已强类型加载；39-entry FCS corpus 与新增 7 项尚未由 parser/elaborator/canonical runner 执行 |
| `fcbc.md` 全部 | FCBC 2 与 Execution ABI | manifest/hex integrity test only | 未来 `fcs-fcbc` | 864-byte empty + 1021-byte embedded-resource golden（manifest only） | 13 mutation vectors（manifest only） | blocked-by-I7 | I7 | S15 已把 corpus 升为 FCBC schema 2：one-chart、14 required sections、ResourceData 原始 bytes/hash/coverage 与 exact-only profile 已绑定；活动 workspace 仍无 writer/loader/ABI evaluator，当前测试不执行 loader diagnostic 或 runtime semantics |
| `fcs-conversion.md` 全部 | Parser/profile/Repair 分层、PGR/RPE/PEC conversion 与 report | manifest integrity API（test only） | `tests/conformance_manifest.rs`，未来 `fcs-converter` 依赖 `fcs-model` | 12-profile/7-dialect/56-rule/32-category registry、38 exact mapping 与 10 selection vectors（manifest only） | 5 invalid mapping、ambiguity/target-profile vectors（manifest only） | blocked-by-I6 | I5/I6/I8 | Registry/path/hash/cross-reference 已验证；活动 workspace 无 source parser、selector、converter、canonical golden 或 target reparse，旧 converter/IR 只在归档分支且不作为 canonical model |
| `fcs-render.md` 全部 | Render source/canonical/section/raster/resource binding | Core balanced envelope 尚未实现；manifest integrity only | 未来 `fcs-source` envelope 与 `fcs-render` | `render.source.valid.solid-rect`, semantic/raster fixture、`render.binding.embedded-image-resource`（manifest only） | `render.source.invalid.missing-viewport`, `render.source.invalid.unknown-node-kind`（manifest only） | blocked-by-I9 | I1/I9 | S14 已闭合 Render source EBNF；S15 绑定 stable resource ID→FCBC Resources 6/ResourceData 20、exact descriptor only 和 no source text/cluster/external fallback；opaque binding fixture不执行 codec decode，Render-aware parser/canonical scene/FCBC section/raster harness 均属 I9 |

## I0 更新规则

- I0 每完成一个 task，更新受影响行的状态、路径和测试证据；
- `implementation.feature-unavailable` 只记录在已知偏差列，不得写入 conformance expected；
- 新增 fixture 后更新对应行，不用删除既有 fixture 引用来制造绿色状态；
- I0 完成时，所有 I0-A 至 I0-H 行为必须是 `implemented` 或明确分配到后续阶段；
- matrix 变更不能替代规范版本流程。
