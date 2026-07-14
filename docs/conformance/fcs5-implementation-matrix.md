# FCS 5 规范—实现—测试矩阵

状态日期：2026-07-14

本矩阵记录 Frozen 规范与参考实现之间的可审计关系。它不定义格式语义；发生冲突时以
`fcs.md`、`fcbc.md`、`fcs-render.md`、`fcs-conversion.md` 和绑定 conformance corpus 为准。

I0-A（快照、归档和 `master` 分支切换）已完成；I0-B 及后续 source implementation 尚未
执行。下表中的实现路径仍使用 I0 完成后的目标路径，避免 crate cutover 后立即失效；“现状”
继续描述当前活动树中的候选实现，并不因为归档分支建立而变成已实现。

当前进度证据：`archive/fcs4-pre-cutover` 指向
`148936d17b671bb34968c88969ab748c818f9fc0`，`master` 已从该快照 fast-forward，原 feature
branch 保留。generator staging 已完成；下一项实现工作是 I0.3 唯一 crate 切换。在后续
诊断、parser 和 source crate 任务完成前，其他 source rows 仍按下表的实际偏差记录。

允许的状态只有：`implemented`、`partial`、`not-started` 和 `blocked-by-I<n>`。

| 规范条款 | 能力 | 目标 public API | 目标实现文件 | Valid fixture/test | Invalid fixture/test | 现状 | 下一阶段 | 已知偏差 |
|---|---|---|---|---|---|---|---|---|
| `fcs.md` 2.1 | UTF-8、BOM、换行 | `parser::parse_document` | `crates/fcs-source/src/parser/lexer.rs` | lexer unit tests BOM/CRLF | `decode.invalid-utf8` 由字节入口覆盖 | partial | I0-D | 候选 parser 接受 `&str`，尚无独立字节解码入口 |
| `fcs.md` 2.2 | 精确 source header 与版本拒绝 | `parser::parse_header` | `crates/fcs-source/src/parser/header.rs` | `source.valid.minimal-fragment` | `source.invalid.missing-header` | partial | I0-F | 现有 API 使用 enum error，未映射稳定 code |
| `fcs.md` 2.3 | 空白、行注释、nested block comment | `parser::parse_document` | `crates/fcs-source/src/parser/lexer.rs` | lexer unit test nested comment | `syntax.unclosed-comment` | partial | I0-D | 现有 block comment 不支持嵌套 |
| `fcs.md` 2.4 | ASCII identifier 与完整保留词 | `parser::parse_document` | `crates/fcs-source/src/parser/token.rs` | reserved-word table test | reserved word as identifier | partial | I0-D | 现有 lexer 把大多数保留词当普通 identifier |
| `fcs.md` 2.5–2.7 | Int/Float/String/Color literal | `parser::parse_expression` | `crates/fcs-source/src/parser/lexer.rs` | literal table tests | malformed/non-finite literal tests | partial | I0-D | 候选实现缺少完整稳定 diagnostic 分类 |
| `fcs.md` 2.8–2.9 | 分隔符、array/object/reference/interval | `parser::parse_expression` | `crates/fcs-source/src/parser/expression.rs` | I1 grammar fixtures | I1 grammar fixtures | blocked-by-I1 | I1 | I0 只建立 token 和 parser 框架，不伪造缺失 AST |
| `fcs.md` 3.1–3.4 | 基础类型、单位和显式转换边界 | `ast::Type`, `ast::SourceLiteral` | `crates/fcs-source/src/ast/types.rs` | `tests/compile_time.rs` type table | invalid type/conversion tests | partial | I2 | 当前 Type 缺少完整 array/Track 类型，转换矩阵未完成 |
| `fcs.md` 4.1 | 表达式优先级和结合性 | `parser::parse_expression` | `crates/fcs-source/src/parser/expression.rs` | `tests/expression.rs` precedence table | missing operand/delimiter tests | partial | I0-E | 当前 parser 是手写 token cursor，尚未覆盖 Appendix B 全部 primary |
| `fcs.md` 4.2–4.4 | 运算矩阵、builtin、字段访问 | `elaborator::elaborate` | `crates/fcs-source/src/elaborator/eval.rs` | existing compile-time tests | type/name/operator tests | partial | I2 | 候选 evaluator 不是完整 Frozen operator matrix |
| `fcs.md` 4.5 | Runtime value 边界 | `elaborator::elaborate` | `crates/fcs-source/src/elaborator/eval.rs` | runtime expression fixtures | `source.invalid.runtime-gameplay` | partial | I2 | 完整 dynamic-field whitelist 尚未实现 |
| `fcs.md` 5.1–5.2 | Document、format、profile | `parser::parse_document` | `crates/fcs-source/src/parser/document.rs` | `source.valid.minimal-fragment` | profile/misplaced block tests | partial | I0-F | 候选 top-level grammar 只覆盖早期 subset |
| `fcs.md` 5.3 | definitions 中的 const/fn/template | `ast::DefinitionsBlock` | `crates/fcs-source/src/ast/definitions.rs` | `source.valid.template-if-with` | duplicate/misplaced declaration tests | partial | I0-F | 当前 template 使用独立 top-level block，违反 Frozen grammar |
| `fcs.md` 5.4 | collections 与 source order | `ast::CollectionBlock` | `crates/fcs-source/src/ast/entity.rs` | collection parser tests | unknown/misplaced collection tests | partial | I0-F | 当前 schema collection 集仍是 Phase 2 subset |
| `fcs.md` 6.1 | 绑定、作用域和禁止 shadowing | `elaborator::elaborate` | `crates/fcs-source/src/elaborator/scope.rs` | binding tests | `source.invalid.shadowing` | partial | I2 | 已有局部检查，完整跨种类 scope graph 未完成 |
| `fcs.md` 6.2 | 纯函数、return、调用图 | `elaborator::elaborate` | `crates/fcs-source/src/elaborator/eval.rs` | function evaluation tests | cycle/missing return tests | partial | I2 | 诊断 trace 和完整路径分析未完成 |
| `fcs.md` 6.3–6.5 | typed template、constructor、with、if | `elaborator::elaborate` | `crates/fcs-source/src/elaborator/entities.rs` | `source.valid.template-if-with` | `source.invalid.template-missing-line` | partial | I2 | 现有展开需迁移 definitions 归属并统一 budget context |
| `fcs.md` 6.6 | `..<`/`..=` range 与 zero step | `ast::Generator` | `crates/fcs-source/src/parser/entities.rs` | generator range parser tests | `source.invalid.bare-range`, `source.invalid.generator-zero-step` | partial | I0-G/I2 | I0.2 已拒绝裸 `..` 并保留 zero-step 语法；elaborator 暂返回临时 `FeatureUnavailable`，zero-step 语义尚未展开求值 |
| `fcs.md` 6.7 | generator body 与 typed emit | `ast::GeneratorItem` | `crates/fcs-source/src/ast/entity.rs` | `source.valid.compile-time-generator` | nested/misplaced generator tests | blocked-by-I2 | I2 | I0 只解析并返回明确 implementation diagnostic，不输出部分结果 |
| `fcs.md` 6.8 | 六类共享 elaboration budget 与 trace | `elaborator::CompileTimeLimits` | `crates/fcs-source/src/elaborator/mod.rs` | budget unit tests | `source.invalid.generator-budget` | partial | I2 | limits 已有字段但未共享完整 context，也没有结构化 expansion trace |
| `fcs.md` 7.1–7.5 | Metadata、credits、resources、sync、custom | 无 | 未来 `crates/fcs-source/src/ast/metadata.rs` 与 `fcs-model` | `source.valid.metadata-credits-resources-sync` | `source.invalid.unknown-resource`, `source.invalid.custom-duplicate-key` | blocked-by-I1 | I1/I3 | source AST 与 canonical validation 均未实现 |
| `fcs.md` 8.1–8.3 | chartTime、tempo、offset、judgment/scroll 分离 | `ast::TempoMap`，未来 `fcs-model` | `crates/fcs-source/src/parser/tempo.rs`，未来 model | tempo parser tests | tempo invalid/non-monotonic tests | partial | I0-F/I3 | source tempo subset 已有；canonical time/scroll 语义未实现 |
| `fcs.md` 9.1–9.5 | Track schema、segment/point、blend、边界 | 无 | 未来 source Track AST 与 `fcs-model` | `source.valid.track-boundaries` | `source.invalid.track-overlap` | blocked-by-I1 | I1/I3 | 尚无 Frozen Track source/canonical model |
| `fcs.md` 10.1–10.4 | Scroll coordinate、speed、distance、积分 | 无 | 未来 `fcs-model`/`fcs-runtime` | `source.valid.time-scroll-note` | numeric/runtime vectors | blocked-by-I3 | I3/I4 | 未开始 |
| `fcs.md` 11.1–11.5 | 坐标、Line、parent DAG、inherit、排序 | 无 | 未来 `fcs-model` | `source.valid.parent-transform` | `source.invalid.parent-cycle` | blocked-by-I3 | I3/I4 | 未开始 |
| `fcs.md` 12.1–12.5 | Note gameplay/presentation/Hold/排序 | source constructor subset | `crates/fcs-source/src/schema.rs`，未来 `fcs-model` | note schema tests | `source.invalid.hold-end` | partial | I2/I3 | source schema 是候选 subset；canonical Note 未实现 |
| `fcs.md` 13.1–13.4 | Runtime expression、环境和 lazy choose | source expression subset | 未来 `fcs-runtime` | `source.valid.runtime-choose` | environment/cycle tests | blocked-by-I1 | I1/I4 | choose source node和 runtime DAG 均未实现 |
| `fcs.md` 14.1–14.3 | binary64、baking、Core easing | 无 | 未来 `fcs-runtime` | `expected/numeric-vectors.toml` | error-budget tests | blocked-by-I4 | I4 | 未开始 |
| `fcs.md` 15.1–15.3 | Extension、fidelity、repair | 无 | 未来 `fcs-model` | future extension fixtures | repair/provenance fixtures | blocked-by-I5 | I5 | 未开始 |
| `fcs.md` 16 | 稳定 diagnostic categories | `diagnostic::{Diagnostic, DiagnosticCode}` | `crates/fcs-source/src/diagnostic.rs` | `tests/diagnostic.rs` | all invalid fixtures | partial | I0-C | 当前 parser/elaborator 暴露不稳定 Rust enum variants |
| `fcs.md` 17 | Expanded source 与 canonical lowering 边界 | `ast::ExpandedSourceDocument` | `crates/fcs-source/src/ast/entity.rs`，未来 `fcs-model` | expanded invariant tests | forbidden compile-time-node tests | partial | I2/I3 | Expanded candidate 存在；完整 invariant 与 canonical lowering 未完成 |
| `fcs.md` 18 | Source/static/canonical/runtime conformance runner | manifest integrity test | `crates/fcs-source/tests/conformance_manifest.rs` | 22-entry manifest | malformed manifest unit cases | not-started | I0-G/I1–I4 | I0 只建立强类型 manifest 完整性门 |
| `fcbc.md` 全部 | FCBC 2 与 Execution ABI | 无 | 未来 `fcs-fcbc` | 804-byte golden | 8 mutation vectors | blocked-by-I7 | I7 | 旧 FCBC 实现将在 I0 从活动主线删除 |
| `fcs-conversion.md` 全部 | PGR/RPE/PEC conversion 与 report | 无 | 未来 `fcs-converter` 依赖 `fcs-model` | conversion mapping vectors | capability boundary vectors | blocked-by-I6 | I5/I6/I8 | 旧 converter/IR 将归档，不作为 canonical model |
| `fcs-render.md` 全部 | Render source/canonical/section/raster | 无 | 未来 `fcs-render` | semantic/raster fixture | malformed render fixtures | blocked-by-I9 | I9 | 未开始 |

## I0 更新规则

- I0 每完成一个 task，更新受影响行的状态、路径和测试证据；
- `implementation.feature-unavailable` 只记录在已知偏差列，不得写入 conformance expected；
- 新增 fixture 后更新对应行，不用删除既有 fixture 引用来制造绿色状态；
- I0 完成时，所有 I0-A 至 I0-H 行为必须是 `implemented` 或明确分配到后续阶段；
- matrix 变更不能替代规范版本流程。
