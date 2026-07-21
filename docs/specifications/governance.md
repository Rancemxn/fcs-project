# FCS 规范治理

## 1. 权威文档

FCS 5 规范由下列文件共同组成：

- `docs/specifications/fcs.md`：FCS Core Source Specification 与 canonical execution semantics；
- `docs/specifications/fcbc.md`：FCBC Container Format 与 FCS Execution ABI；
- `docs/specifications/fcs-render.md`：独立版本化的 FCS Render Profile；
- `docs/specifications/fcs-conversion.md`：外部格式转换、保真数据与 ConversionReport。

上述文件是规范性资料。`docs/decisions/` 只记录设计理由，不能覆盖规范；
`docs/plans/` 只记录实施顺序，不能创造格式语义。实现、测试或示例与规范冲突时，
必须先判断是实现缺陷还是规范缺陷，不得让实现行为静默成为新规范。

规范权威与外部格式证据属于不同职责，不能压成一条总排名：

- 本文件管理五个版本域的候选版本、状态、变更和冻结流程，不替代四份根规范中的具体语义；
- Accepted ADR 约束架构与后续规范修订方向，但不是 source grammar、二进制布局或执行语义的
  替代文本；
- `docs/community/` 综合 PGR、RPE、PEC 等外部格式证据、歧义与已知实现差异；
- 固定 commit/hash 的 `refer/chart/` 是某个外部项目在特定版本下的一手行为证据，但单个项目
  不能定义整个社区格式；
- 当前实现、测试和 example 只提供实现或 fixture 证据，不获得规范权威。

Accepted ADR 与现行规范发生实质冲突时，必须重开受影响的规范版本，暂停依赖该新语义的实现，
依次更新规范、fixture、manifest、review 和状态记录；不得任选 ADR 或旧规范直接实现。I1–I9 的
受影响工作只能在对应 Reviewed Implementation Baseline 重新独立复审后恢复；I10/发布仍须把受影响
版本重新 Frozen。后续决定改变一个 Accepted ADR 时，应新增 ADR 并把旧记录标为 `Superseded`
或 `Partially superseded`。勘误或治理补充可以追加 dated amendment，但不得静默改写旧决定的
历史背景。

## 2. 当前候选版本

| 规范 | 候选版本 | 状态 |
|---|---:|---|
| FCS Core Source Specification | 5.0.0 | Frozen（2026-07-21 local RC；I1–I10 product surfaces and executable conformance on `main`; Primary-audit merge policy for RC closeout; no public tag/Release） |
| FCBC Container Format | 2.0.0 | Frozen（2026-07-21 local RC；product `fcs-fcbc` framing/load/write/mutation; goldens and execution ABI queries on `main`） |
| FCS Execution ABI | 1.0.0 | Frozen（2026-07-21 local RC；product loader/evaluator/writer and nonempty execution golden closure on `main`） |
| FCS Render Profile | 1.0.0 | Frozen（2026-07-21 local RC；product `fcs-render` load/write surface and restricted asset codecs on `main`） |
| FCS Conversion Specification | 1.0.0 | Frozen（2026-07-21 local RC；product importer fixture lane, PGR export reparse, and CLI convert/report on `main`） |

Draft/Reviewed/Frozen 是仓库发布状态，不写入 FCS 或 FCBC 的 SemVer 字段。Frozen 只表示
规范文本和绑定 conformance baseline 已稳定；参考实现仍必须逐项通过 conformance 后才能宣称
对应实现 conformance。

2026-07-15 用户确认 FCS 5 尚未公开且兼容修改成本为零，因此首先撤回 FCS Core 5.0.0 和 Render
Profile 1.0.0 的旧 Frozen 状态并完成 Source grammar closure。随后用户接受 ADR 0007–0009，并
确认采用“FCS authoring source + 单谱面自包含 FCBC distribution container”、显式版本化转换
profile 和 exact-first runtime expression 边界，因此五个版本域均进入本表所示的重新修订或联合
复审状态。

`fcbc.md` 继续联合定义 FCBC Container 2.0.0 与 Execution ABI 1.0.0。旧冻结审查只对整个文件
保存一个 hash，而资源 payload、required section 和 loader contract 的修改会改变该联合文件；
因此 Container 与 ABI 一起重审，不为了保留旧状态拆分文件或使用脆弱的章节级冻结。联合重审
不预先断言 Expression DAG 指令语义已经变化，两个候选 SemVer 均保持不变。

2026-07-14 的 Frozen hash 与 2026-07-15 的 Source grammar closure Reviewed hash 只保留历史审计
用途，不代表当前候选文件 bytes。

### 2.1 Reviewed Implementation Baseline

2026-07-16 用户接受 ADR 0010，取消“I1 必须等待五个版本域全部 Frozen”的全局实现门，并保留
Draft/Reviewed/Frozen 作为唯一版本状态。I1–I9 改用阶段范围化 **Reviewed Implementation Baseline**：

1. 记录阶段边界、明确排除范围和完整 normative dependency closure；
2. 绑定具体条款、fixture/expected、稳定 diagnostic 和候选文件 SHA-256；
3. 未参与修改的独立 reviewer 对该范围没有未关闭的 Critical/Important finding；
4. 阶段计划与绑定条款一致，前置阶段质量门通过；
5. 域外 blocker 有明确 owner，且不会改变本阶段公开产物或接口。

上述条件满足后自动进入对应阶段，无需逐阶段取得用户确认。baseline 是实施许可和审计记录，不是
新的版本状态，不会把整章、整个文件或版本域提升为 Reviewed/Frozen，也不授权发布或完整
conformance 声明。能经依赖闭包改变本阶段 AST、diagnostic、canonical value、ABI、资源约束或其他
公开行为的 finding 必须算作 scope 内 finding，不能按文件名或路线阶段排除。

baseline 绑定的条款、fixture、expected、稳定 category 或前置公开 invariant 改变时，只重开受影响
阶段及其依赖阶段；不相关阶段无需全局回退。发现规范缺陷时仍须先修改规范与 conformance，再修改
实现，不得以已有实现保住错误 baseline。完整规则见
`docs/decisions/0010-stage-scoped-implementation-baselines.md`。

I4.4 scroll-composition clarification reopens the affected Core/FCBC/Execution
ABI candidate closure without changing the candidate SemVer values or wire
layout. The active FCS corpus is 42 entries after adding the literal
`source.valid.scroll-inheritance` evaluate-stage vector. Its fixed semantics
are Line-local q/tempo/speed/origin/floor plus actual-parent-only effective
floor/velocity composition, local reverse validation, direct seek, signed-zero
and ancestry-scoped error isolation; product evaluator ownership remains
I4.4, with DAG/Piecewise/integration and independent reference work still
open. The implementation owner is the I4.4 Scroll evaluator; the affected
candidate file hashes and exact merged SHA must be recorded in the delivery
review before the stage baseline can be re-established. The candidate snapshot
hashes are `fcs.md` `a19b4757fcee3e86ec647cb1248148a525824a012bf2f0cf721f54e38b712540`,
`fcbc.md` `d3687c8e71c098c0f8334f9b5abd997c5dacf2a4d844a9d70b0f35696d4c293f`,
the FCS manifest `91f6f544c186db31595ed27eebb2c0ed445a2c7ec5614658cda7a294f1a5806a`,
the source vector `96d13fdf6aedb26c60ffd21ff0ff9410076103c4732bd897661fe0b883ad7838`,
the expected vector `c940ecacc84a9b2629a28dccdd6a0f0c7193e2f088f6c616772010e65aded87d`,
and the static binding `c536db2ebdc9d6b71971bd8f04287d6f4c6242b682c2fb3c722e55409fb57a73`.
The exact merged SHA and same-head gate URL remain delivery evidence, not
normative semantics.

I10 conformance release candidate 仍要求五个版本域全部 Frozen、最终联合独立复审无未关闭
Critical/Important finding，以及完整 source/canonical/runtime/FCBC/conversion/render/CLI executable
conformance 通过。局部 baseline 不能替代该门槛。

I1 baseline 已于 2026-07-16 建立，固定 Core/Render-envelope 规范、39-entry/38-path source fixture
tree、ADR 0006/0008/0010 和 I1 计划；独立复审为 Critical 0、Important 0、Minor 0。Fixture tree
明确使用相对 `docs/conformance/fcs5` 的 forward-slash path，修正了一个在正式绑定前被 reviewer 拒绝的
Windows-separator 预计算值。完整输入、corrected hash、phase routing 和审计结果见
`docs/reviews/2026-07-16-i1-source-parser-baseline-review.md`。因此 I1 Task 1 已自动开始；这不改变本节
任何版本状态。

FCS Core 本轮 authoring/canonical delta、39-entry conformance 设计与跨规范 gate 记录在
`docs/reviews/2026-07-15-fcs5-authoring-canonical-closure-review.md`。该文件只证明 Core 修订范围，
不替代后续 FCBC/Render/Conversion closure 或最终 re-freeze review。

FCBC 2.0/Execution ABI 1.0 的 one-chart、required ResourceData、exact-only profile、Note/Distance
record、schema 2 golden/mutation delta 和自检记录在
`docs/reviews/2026-07-15-fcbc2-execution-abi-closure-review.md`。该 closure 保持 Draft；Render resource
binding 与 Conversion schema 已在后续候选 delta 中同步。非空 writer→static bytes→independent
loader/evaluator、bits/trace/direct-seek 与 mutation corpus 已由
`docs/reviews/2026-07-16-fcbc2-execution-abi-nonempty-review.md` 独立复审关闭；这是单项 blocker
closure，不是 FCBC/ABI 产品实现，也不在其余 blocker 与最终联合复审前授权重新 Frozen。

Render Profile 1.0 的 stable resource ID→FCBC Resources/ResourceData、exact descriptor only、no
source-text/cluster/external-fallback delta 与 semantic binding fixture 记录在
`docs/reviews/2026-07-15-render1-resource-binding-closure-review.md`。Opaque binding fixture 不执行
媒体 decode。完整 RenderSection layout、resource decode/shaping、semantic/raster 与 diagnostic 规范
选择曾由 `docs/reviews/2026-07-16-render1-binary-raster-closure-review.md` 独立复审；随后 executable
vector 准备阶段发现 REN-I08–I16，已由 `docs/reviews/2026-07-16-render1-normative-amendment-review.md`
追加修订并重新打开该前置 gate；follow-up 同时改变 Core `fcs.md`、FCBC/ABI 与 Render candidate
bytes。随后对 `fcs.md`、`fcbc.md`、`fcs-render.md` 固定快照的独立复审以 Critical 2、Important 8、
Minor 0 失败；详情、hash 与 Important 计数勘误见该 review 第 8 节。`RNR-C01`–`RNR-I08` 当前已由
候选规范文字逐项处理，新的 candidate hash、focused test evidence、`binary_fixture=0` 的诚实 manifest
边界和历史 visibility EnvB 例外见该 review 第 9–10 节；尚未取得新的独立复审，因此不能标记为
closed。随后未参与修改的只读 reviewer 在第 10 节固定 hash 上以 Critical 0、Important 0、Minor 0
关闭 `RNR-C01`–`RNR-I08` normative gate，见第 11 节。Static FCBC、independent loader/evaluator、
decoder/shaper/raster 和 mutation corpus 及其独立复审仍是 I9/重新 Frozen 的 gate。

Conversion Specification 1.0 的 parser/profile/Repair 分层、source/target selector、12-profile registry、
7 个 parser dialect、56 个 mapping rule、32 个 diagnostic/report category、38 个 exact mapping
vector、5 个非法边界、10 个选择/歧义向量以及 FCBC no-source-snapshot 投影记录在
`docs/reviews/2026-07-15-conversion1-semantic-profile-closure-review.md`。这些向量已由 Rust 强类型
manifest integrity test 验证，但活动 workspace 尚无 converter，也尚未提供真实外部 source→canonical
golden→target reparse 闭环；因此该 closure 仍是 Draft 候选证据。

四规范当前候选 bytes、跨域不变量、完整 workspace gate、suite/tree hash，以及 blocker ledger
统一记录在 `docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md` 及其 dated amendment。ABI
第 7.2 项已关闭；第 7.3 项的旧规范文字 closure 已被 REN-I08–I16 amendment 重新打开，新的
independent Render normative review、executable RenderSection binary/raster artifact、Conversion
真实 round-trip 和 Core fixture validation 同样开放。原联合候选自检不是最终独立 review；全部
剩余 blocker 关闭并完成联合独立复审前，本表状态不得提升为 Reviewed/Frozen。

## 3. 规范用语

本文档集合使用以下强度：

- **必须（MUST）**：conformance 的强制要求；
- **必须不（MUST NOT）**：conformance 的强制禁止；
- **应当（SHOULD）**：除非实现记录了明确且可审计的理由，否则必须遵守；
- **可以（MAY）**：可选能力；
- **非规范说明**：示例、理由或实现建议，不参与 conformance。

没有使用上述大写英文的普通说明仍按对应中文含义解释。规范示例不能覆盖明确规则。

## 4. 语义层次

每项能力必须明确属于以下层次之一：

1. **Source syntax**：源文件如何分词、解析和保留位置；
2. **Static semantics**：名称、类型、schema、作用域和编译期展开是否合法；
3. **Canonical semantics**：合法 source lowering 后表示的唯一谱面语义；
4. **Execution semantics**：给定运行时环境时必须得到的值和状态；
5. **Container/ABI**：上述语义如何编码进 FCBC 并由运行时查询；
6. **Provenance/fidelity**：来源数据如何保留，但不自动获得 Core 执行语义。

编译期结构必须在 canonical lowering 前消失。保真数据不得反向改变 Core gameplay，
除非某个已声明、已版本化的 extension 明确规定该行为。

## 5. 版本变更

所有规范采用 `major.minor.patch`：

- major：现有合法输入、二进制布局或执行语义出现不兼容变化；
- minor：保持已有语义的可选新增；
- patch：不改变任何已有合法输入语义的勘误、澄清或诊断改进。

尚未公开发布的候选版本可以在用户明确确认兼容成本为零后撤回 Frozen、保持候选 SemVer 修订并
重新走完整审查；必须保留旧审计记录并标记其已撤回。依赖新语义的 I1–I9 实现必须等受影响阶段
baseline 重新建立，I10/发布必须等版本重新 Frozen。已经公开的版本不得使用该例外。

FCS、FCBC、Execution ABI、Render Profile、Conversion Specification 和 extension schema
独立版本化。一个完全在编译期消失的 source 功能不必提升 FCBC 或 ABI；容器 framing
变化不必提升 FCS source；新增 Render 节点不必改变 FCS Core major。

## 6. 规范变更流程

任何会改变合法输入、canonical 结果、执行结果、诊断等级或 FCBC 兼容性的变更必须：

1. 指出受影响的规范与章节；
2. 描述当前行为、建议行为和动机；
3. 列出合法、非法和边界案例；
4. 说明 FCS、FCBC、ABI、Render、Conversion 哪些版本需要变化；
5. 新增或修改 conformance fixture；
6. 更新规范后再修改实现；
7. 在路线图和版本表中记录状态。

纯内部重构、性能优化和不改变诊断类别的人类文本改善不要求提升规范版本。

## 7. 章节完成条件

一章只有同时满足下列条件才可以从 Draft 标记为 Reviewed：

- 术语、单位和边界条件完整；
- 至少有一个合法示例、一个非法示例和一个边界示例；
- 静态错误与运行时行为可以区分；
- canonical lowering 结果可唯一确定；
- 相关版本影响明确；
- 已列出 conformance fixture 与预期结果；
- 不含 `TODO`、`TBD`、“有待商榷”或依赖实现自行猜测的核心行为。

整个版本只有在所有章节 Reviewed、规范交叉引用通过、测试向量可执行并完成独立审查后
才可以标记 Frozen。Frozen 后的规范变更必须遵循版本变更规则。

## 8. 实现边界

规范不规定 Rust parser 库、AST 内存布局、缓存、并行策略、GPU backend 或自适应积分
所用的具体算法。实现可以自由选择这些内部机制，但必须满足确定性、误差、资源限制和
诊断要求。

参考实现必须维护“规范条款—实现位置—测试”的对账表。发现规范矛盾时应暂停相关行为
扩张，先修正规范，不能用兼容别名、隐式修复或未记录默认值绕过问题。
