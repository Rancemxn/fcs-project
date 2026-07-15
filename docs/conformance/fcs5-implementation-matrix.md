# FCS 5 规范—实现—测试矩阵

状态日期：2026-07-15

本矩阵记录 Frozen 规范与参考实现之间的可审计关系。它不定义格式语义；发生冲突时以
`fcs.md`、`fcbc.md`、`fcs-render.md`、`fcs-conversion.md` 和绑定 conformance corpus 为准。

I0.1–I0.8 已完成：活动 `master` 只有无版本前缀的 `crates/fcs-source`，source subset 使用
Chumsky 0.11.2 的单一 spanned-token 数据流，并具备稳定诊断、严格 byte decode、固定配置
Proptest robustness 和强类型 manifest 完整性门。当前 workspace 有 134 个通过的测试；这只
证明 I0 retained subset 和治理门，不表示全部 FCS 5 source/canonical/runtime conformance 已完成。

当前进度证据：`archive/fcs4-pre-cutover` 固定在
`148936d17b671bb34968c88969ab748c818f9fc0`；唯一 crate cutover 为 `16e7db3`，稳定诊断为
`0185d8e`，Chumsky lexer/expression/document 迁移为 `7442210`、`cc8d94f`、`0b17e56f`，Frozen
generator parser 边界为 `9d88a6a`。I0.9 最终结构、依赖、质量和独立审查 gate 尚待执行。

允许的状态只有：`implemented`、`partial`、`not-started` 和 `blocked-by-I<n>`。`implemented`
表示该行所列 I0 能力已有实际测试证据；`partial` 必须在“已知偏差”列写明缺失行为和接续阶段。

| 规范条款 | 能力 | Public API | 实现文件 | Valid fixture/test | Invalid fixture/test | 现状 | 下一阶段 | 已知偏差 |
|---|---|---|---|---|---|---|---|---|
| `fcs.md` 2.1 | UTF-8、BOM、换行 | `parser::parse_document_bytes` | `parser/document.rs`, `parser/lexer.rs` | `robustness::byte_entry_decodes_once_and_preserves_utf8_error_spans` | `robustness::arbitrary_bytes_never_escape_decode_or_parse_boundaries` | implemented | I1 | 严格 UTF-8 decode、BOM/CRLF byte span 和 bounded arbitrary bytes 已覆盖；I1 只扩展到新增 grammar production |
| `fcs.md` 2.2 | 精确 source header 与版本拒绝 | `parser::parse_header` | `parser/header.rs`, `parser/lexer.rs`, `version.rs` | `frontend::parses_exact_fcs5_header`, `source.valid.minimal-fragment` | `frontend::rejects_missing_or_wrong_major_header`, `source.invalid.missing-header` | implemented | I1 | `5.0.x` 兼容规则、missing/invalid/unsupported 稳定 code 已覆盖；未来版本策略不在 I0 扩展 |
| `fcs.md` 2.3 | 空白、行注释、nested block comment | `parser::parse_document` | `parser/lexer.rs` | `lexer::nested_comments_and_string_escapes_are_deterministic` | `frontend::rejects_unclosed_trailing_block_comment` | implemented | I1 | retained subset 的 trivia 与 nested-comment contract 已完成；I1 复用同一 lexer 接入完整 grammar |
| `fcs.md` 2.4 | ASCII identifier 与保留词 | `parser::parse_expression`, `parser::parse_document` | `parser/token.rs`, `parser/lexer.rs` | `compile_time::identifiers_are_ascii_but_spans_remain_utf8_byte_offsets` | `expression::token_parser_rejects_reserved_names_and_trailing_input` | implemented | I1 | I0 token 表和 ASCII 约束已覆盖；I1 新增 source node 时不得把保留词降级为 identifier |
| `fcs.md` 2.5–2.7 | Int/Float/String/Color 与单位 literal | `parser::parse_expression` | `parser/lexer.rs`, `ast/types.rs`, `ast/color.rs` | `compile_time::parses_scalar_and_unit_literals`, `compile_time::string_escapes_match_the_documented_table` | `compile_time::unit_literals_must_remain_finite_after_conversion` | implemented | I1 | I0 literal 表、escape、精确 Beat 与 non-finite 拒绝已覆盖；I1 仅补新增 grammar 上下文 |
| `fcs.md` 2.8–2.9 | 分隔符、array/object/reference/interval | `parser::parse_expression` | 未来扩展 `parser/expression.rs`, `ast/types.rs` | `source.valid.track-boundaries`（manifest only） | I1 grammar fixtures | blocked-by-I1 | I1 | token 已保留部分分隔符；array/object/reference/interval AST 与 parser 尚未实现 |
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
| `fcs.md` 6.8 | 六类共享 elaboration budget 与 trace | `elaborator::CompileTimeLimits` | `elaborator/mod.rs`, `diagnostic.rs` | `compile_time::template_cycle_detection_scans_template_bodies_before_expansion` | `source.invalid.generator-budget`（manifest only） | partial | I2 | I0 已有表达式/调用/展开 limits 与结构化 trace 形状；六类共享计数、generator index/emit trace 尚未实现 |
| `fcs.md` 7.1–7.5 | Metadata、credits、resources、sync、custom | 无 | 未来 `ast/metadata.rs` 与 `fcs-model` | `source.valid.metadata-credits-resources-sync`（manifest only） | `source.invalid.unknown-resource`, `source.invalid.custom-duplicate-key`（manifest only） | blocked-by-I1 | I1/I3 | source AST/parser 与 canonical validation 均未实现 |
| `fcs.md` 8.1–8.3 | chartTime、tempo、offset、judgment/scroll 分离 | `ast::TempoMap` | `parser/tempo.rs`, 未来 `fcs-model` | `frontend::parses_chart_tempo_map_with_exact_beats` | `frontend::tempo_points_must_be_non_decreasing` | partial | I1/I3 | source tempoMap subset、exact Beat 和 zero-start/monotonic checks 已实现；offset、完整 source 结构和 canonical time/scroll 语义缺失 |
| `fcs.md` 9.1–9.5 | Track schema、segment/point、blend、边界 | 无 | 未来 source Track AST 与 `fcs-model` | `source.valid.track-boundaries`（manifest only） | `source.invalid.track-overlap`（manifest only） | blocked-by-I1 | I1/I3 | Frozen Track source/canonical model 尚未实现 |
| `fcs.md` 10.1–10.4 | Scroll coordinate、speed、distance、积分 | 无 | 未来 `fcs-model`, `fcs-runtime` | `source.valid.time-scroll-note`（manifest only） | numeric/runtime vectors（manifest only） | blocked-by-I3 | I3/I4 | canonical scroll model 与 reference evaluator 尚未开始 |
| `fcs.md` 11.1–11.5 | 坐标、Line、parent DAG、inherit、排序 | 无 | 未来 `fcs-model` | `source.valid.parent-transform`（manifest only） | `source.invalid.parent-cycle`（manifest only） | blocked-by-I3 | I3/I4 | I0 仅有 constructible Line identity subset；canonical graph 与 transform 未开始 |
| `fcs.md` 12.1–12.5 | Note gameplay/presentation/Hold/排序 | source constructor subset | `schema.rs`, `elaborator/entities.rs`, 未来 `fcs-model` | `compile_time::phase2_note_schema_has_exact_fields_required_flags_and_variants` | `source.invalid.hold-end`（manifest only） | partial | I2/I3 | I0 Note schema 是 retained construction subset；完整 required/dynamic rules、Hold 语义、canonical ID 与排序缺失 |
| `fcs.md` 13.1–13.4 | Runtime expression、环境和 lazy choose | source expression subset | 未来 `fcs-runtime` | `source.valid.runtime-choose`（manifest only） | environment/cycle tests（未接入） | blocked-by-I1 | I1/I4 | choose source node、runtime environment 和 DAG 均未实现；I0 只有 compile-time logical short-circuit |
| `fcs.md` 14.1–14.3 | binary64、baking、Core easing | 无 | 未来 `fcs-runtime` | `expected/numeric-vectors.toml`（manifest only） | error-budget tests（未接入） | blocked-by-I4 | I4 | reference numeric evaluator 与 baking 尚未开始 |
| `fcs.md` 15.1–15.3 | Extension、fidelity、repair | 无 | 未来 source extension AST 与 `fcs-model` | future extension fixtures | repair/provenance fixtures | blocked-by-I1 | I1/I5 | extension source grammar 先由 I1 建立，canonical fidelity/repair 由 I5 实现 |
| `fcs.md` 16 | 稳定 diagnostic categories | `diagnostic::{Diagnostic, DiagnosticCode}` | `diagnostic.rs` | `diagnostic::diagnostics_are_sorted_by_span_then_code` | `diagnostic::missing_header_has_the_frozen_code_and_byte_span` | implemented | I1–I9 | I0 已稳定公共 code/stage/severity/span/labels/trace/budget 形状；后续阶段只在实现对应语义时激活已声明 category |
| `fcs.md` 17 | Expanded source 与 canonical lowering 边界 | `ast::ExpandedSourceDocument` | `ast/entity.rs`, `elaborator/entities.rs`, 未来 `fcs-model` | `compile_time::expanded_ir_exposes_only_read_accessors` | `compile_time::generator_elaboration_fails_before_partial_output` | partial | I2/I3 | I0 retained subset 输出只含 concrete typed fields；完整 generator 消除、expanded invariants 和 canonical lowering 缺失 |
| `fcs.md` 18 | Source/static/canonical/runtime conformance runner | manifest integrity API（test only） | `tests/conformance_manifest.rs` | `conformance_manifest::typed_manifests_load_with_frozen_counts` | `conformance_manifest::manifests_preserve_integrity_invariants` | partial | I1–I4 | 22-entry manifest 已强类型加载并验证路径/schema；尚未执行 fixture 的 parser/elaborator/canonical/runtime expected 结果 |
| `fcbc.md` 全部 | FCBC 2 与 Execution ABI | 无 | 未来 `fcs-fcbc` | 804-byte golden（manifest only） | 8 mutation vectors（manifest only） | blocked-by-I7 | I7 | 活动 workspace 无 FCBC crate；旧实现只在归档分支 |
| `fcs-conversion.md` 全部 | PGR/RPE/PEC conversion 与 report | 无 | 未来 `fcs-converter` 依赖 `fcs-model` | conversion mapping vectors（manifest only） | capability boundary vectors（manifest only） | blocked-by-I6 | I5/I6/I8 | 活动 workspace 无 converter；旧 converter/IR 只在归档分支且不作为 canonical model |
| `fcs-render.md` 全部 | Render source/canonical/section/raster | 无 | 未来 `fcs-render` | semantic/raster fixtures（manifest only） | malformed render fixtures（manifest only） | blocked-by-I9 | I9 | Render source extension、canonical scene、FCBC section 与 raster harness 均未开始 |

## I0 更新规则

- I0 每完成一个 task，更新受影响行的状态、路径和测试证据；
- `implementation.feature-unavailable` 只记录在已知偏差列，不得写入 conformance expected；
- 新增 fixture 后更新对应行，不用删除既有 fixture 引用来制造绿色状态；
- I0 完成时，所有 I0-A 至 I0-H 行为必须是 `implemented` 或明确分配到后续阶段；
- matrix 变更不能替代规范版本流程。
