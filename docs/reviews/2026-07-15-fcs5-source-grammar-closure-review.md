# FCS 5.0 Source Grammar Closure Review

日期：2026-07-15

状态：Historical Reviewed evidence — 当前候选已由后续 ADR 0007–0009 重开为 Draft

## 授权与版本处理

用户确认 FCS 5 尚未公开且兼容修改成本为零，授权修改 FCS 5.0.0。依据
`docs/specification-governance.md` 的未发布候选例外，撤回 2026-07-14 对 FCS Core 5.0.0 和
Render Profile 1.0.0 的 Frozen 结论，在相同候选 SemVer 上执行 Source grammar closure，不保留
旧临时语法或兼容 alias。FCBC 2.0.0、Execution ABI 1.0.0 和 Conversion Specification 1.0.0
未重开。

## 发现的问题

I1 计划审查确认旧规范不能作为完整 parser 的唯一输入：

- Appendix B 引用 `metaBlock`、`tempoMapBlock`、`extensionsBlock`、`preserveBlock`、
  `renderBlock` 等 10 个未定义顶级 nonterminal；
- profile feature、Note/Track enum 与 resource/render enum 使用保留词或 bare identifier，但
  expression grammar 不允许其作为稳定 typed value；
- extension/preserve 没有可由未知实现完整保存的 Core envelope；
- Render 只有示例、没有 source EBNF，且 `linear-srgb` 违反 Core identifier grammar；
- parser、source-structure、static 和 canonical diagnostic ownership 不明确；
- raw NUL 与 `\0`、negative numeric token、closed interval、mixed-Beat 和 semver 空白/前导零边界
  未闭合；
- 三段 semver 与 float tokenization 重叠，`entityExpression` 使用直接左递归，schema
  `cubicBezier(...)` 也没有可达 production；
- 旧 conformance corpus 没有上述 grammar 的有效失败证据。

## 规范性修订

### Core lexical 与 value syntax

- header 固定一个 ASCII space；semver 是连续三段 lexeme、优先于 float，且各段禁止前导零；
- raw U+0000 拒绝，`\0` 允许；raw/escaped Unicode noncharacter 均拒绝；
- leading minus 始终是 unary token，decimal magnitude 先精确保留，static phase 再检查 int64；
- decimal/unit suffix longest-match，移除未定义的 `[whole,numerator,denominator]beat`；
- Core source interval 只有 `[start,end)`；`[start,end]` 始终是 array；
- closed enum 静态类型统一为 `string`，直接值使用 string literal；unresolved identifier 不得按
  field context 猜成 enum；
- keyword 可以在 `fieldName` context 使用，解决 `line:`、`.length`、`render.enabled` 等字段。
- `entityExpression` 改为非左递归的 primary + `with` suffix；cubic Bezier 固定为 direct segment
  与 schema interpolation field 共用的 schema-only value。

### Document、schema 与 envelope

- `format` 紧跟 header；profile/features field 和 duplicate/top-level order 规则闭合；
- metadata、contributors、credits、resources、artwork、sync、tempoMap、Line、collection、Track、
  segment、extension 和 preserve production 全部写入 Appendix B；
- extension 固定为 namespace/semver/required-or-optional 加 ordered Core object，namespace schema
  不得增加 lexer token 或私有 grammar；
- preserve 固定为 source schema block 与 extension-header/ordered-object payload；byte-exact raw
  snapshot 必须显式编码进 string；
- Core parser 对 Render 只验证 version envelope、Core token 和 brace/parenthesis/bracket 的正确
  嵌套；Render Profile section 2 定义 viewport/layer/children/node 的完整内部 EBNF。

### 阶段与 diagnostic

- 新增 decode、parse、source structure、static/elaborate、canonical、evaluate 责任表；
- parse/source-structure 明确 missing/delayed/duplicate format/top-level 与 generator placement
  的 category 和 span 所有权；
- nested/misplaced generator 保留专用 `compile-time.*` category，tempo/profile capability/schema/
  graph/numeric 语义不得提前为 parser error；
- 正式绑定既有 `resource.limit-exceeded`，要求 kind/limit/observed/span 且工作前检查。

## Grammar closure 证据

机械审计结果：

- Core Appendix B：117 个已定义 production，未定义引用 0，直接左递归 0；
- 旧 10 个未定义顶级 nonterminal：每项恰好 1 个定义；
- Render Source：10 个本地 production，外部引用全部显式导入 Core Appendix B；
- 规范/合法 fixture 中 bare schema enum 搜索结果 0；唯一 bare `above` 位于预期
  `name.unknown` 的 invalid fixture；
- FCS manifest：32 entries；
- Render manifest schema 2：1 个 semantic/raster fixture 和 3 个 source fixture；
- conformance tree：51 files。

新增或重新绑定的边界包括 complete top-level envelope、escaped NUL、header extra-space/
leading-zero、duplicate top-level block、nested/misplaced generator、unclosed extension payload、
mixed-Beat rejection、unresolved schema enum、Render missing viewport 和 unknown node kind。

## 实现状态

本次没有开始 I1 parser。I0 workspace 仍为唯一 `fcs-source`，但 implementation matrix 已将受 S14
影响的旧 `implemented` 行诚实重开为 `partial`。现有 mixed-Beat 成功测试改名为 I0
characterization，必须在 I1 删除；tempo/profile canonical validation 仍错误地位于 Parse stage，
同样由 I1/I3 按矩阵迁移。

Manifest integrity test 已扩展为强类型加载 32-entry FCS manifest、Render schema 2、全部 source/
expected/raster path 和 success/error diagnostic invariant。当前验证：

```text
cargo clippy --workspace --all-targets -- -D warnings  PASS
cargo nextest run --workspace                         PASS 135/135
cargo fmt --all -- --check                            PASS
git diff --check                                      PASS
grammar/manifest mechanical audit                     PASS
```

这些结果只证明 corpus/governance 变更没有破坏 I0 基线，不表示新 Source grammar 已由 Rust parser
实现。

## Reviewed 内容哈希

哈希按文件原始 UTF-8 bytes 计算：

```text
fcs.md
492b266f4efe14ebac5fb3025bcb02c59b1b7155d21a1332d26cef028c4148b6

fcs-render.md
d3939f40277165e9ab8f2e02ae4db741062c89412938e7acbdbcaf334b191d87

docs/specification-governance.md
2a914693863c2150f62892f9b043f0f82fa51395c9968797290654300fdff6db

docs/plans/fcs5-roadmap.md
b5b3af7ab3ffea59adf06cdb6f6b0ad9ade83a241fede673d99866a0bf25927c

docs/plans/i1-source-ast-parser.md
a33532ab6efec779734c0eabcb5f9a32ca4d6016a489425c658af5c9a8b9da01

conformance tree (51 files; paths relative to conformance/ with / separators; ordinal path order;
UTF-8(path) + NUL + bytes + NUL)
9e6c872da283ab7d0970d7893310d1693ef2ea2c20317fe63de413fbc95a388f
```

## 独立复审 gate

重新 Frozen 前，独立 reviewer 必须：

1. 逐项确认 Appendix B/Core lexical external 与 Render import 无未定义引用；
2. 检查所有规范性 source 示例符合同一 keyword/string-enum/interval/envelope grammar；
3. 检查 extension unknown-optional preservation 不依赖私有 lexer；
4. 检查 parse/source-structure/static/canonical diagnostic ownership；
5. 强类型加载 FCS/Render manifest 并复算上述哈希；
6. 审查 S14 全部 diff，没有无关 canonical/runtime/FCBC/conversion 语义变化；
7. 将发现分为 Critical、Important、Minor；关闭所有 Critical/Important 后才可把 FCS Core 和
   Render Profile 状态改回 Frozen。

在该 gate 通过并由用户重新确认 I1 计划前，不开始 I1 Rust 实现。

## 后续规范状态

本 review 写成后，用户又接受 ADR 0007–0009 和 `docs/community/` evidence baseline。上述 hash、
fixture 与检查项仍是 Source grammar closure 的有效历史证据，但不覆盖后续 exact-first lowering、
FCBC 内嵌资源、Render resource resolution 和 conversion semantic profile 修订。

因此本文件中的独立复审 gate 不再单独足以把当前 FCS Core/Render 候选标回 Frozen。后续复审必须
在保留本 gate 的全部 grammar 检查之外，审查新增的跨规范 diff、conformance 与当前 hash；当前
状态和 I1 启动条件以 `docs/specification-governance.md` 与 `docs/plans/fcs5-roadmap.md` 为准。
FCS Core 后续 authoring/canonical delta 与从 32 项增长到 39 项的 manifest 记录见
`docs/reviews/2026-07-15-fcs5-authoring-canonical-closure-review.md`。
