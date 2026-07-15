# FCS 规范治理

## 1. 权威文档

FCS 5 规范由下列文件共同组成：

- `fcs.md`：FCS Core Source Specification 与 canonical execution semantics；
- `fcbc.md`：FCBC Container Format 与 FCS Execution ABI；
- `fcs-render.md`：独立版本化的 FCS Render Profile；
- `fcs-conversion.md`：外部格式转换、保真数据与 ConversionReport。

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
依次更新规范、fixture、manifest、review 和状态记录，再经独立复审重新 Frozen；不得任选 ADR
或旧规范直接实现。后续决定改变一个 Accepted ADR 时，应新增 ADR 并把旧记录标为 `Superseded`
或 `Partially superseded`。勘误或治理补充可以追加 dated amendment，但不得静默改写旧决定的
历史背景。

## 2. 当前候选版本

| 规范 | 候选版本 | 状态 |
|---|---:|---|
| FCS Core Source Specification | 5.0.0 | Draft（authoring/canonical closure 与联合候选自检完成；等待完整 fixture validation 与独立复审） |
| FCBC Container Format | 2.0.0 | Draft（one-chart/ResourceData/exact-only 与非空 ABI artifact 已独立复审；等待 Render byte vector、其余 blocker 与最终联合复审） |
| FCS Execution ABI | 1.0.0 | Draft（非空 byte/evaluation/mutation blocker 已独立复审关闭；等待其余跨规范 blocker 与最终联合复审） |
| FCS Render Profile | 1.0.0 | Draft（RenderSection/decoder/shaping/raster 规范文字已独立复审；等待完整 executable artifact、其余 blocker 与最终联合复审） |
| FCS Conversion Specification | 1.0.0 | Draft（semantic profile/selection/report closure 与联合候选自检完成；等待真实 round-trip fixture 与独立复审） |

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

2026-07-16 用户取消了 I1 的特殊历史人工批准门。当前 I1–I10 统一使用以下客观阶段门：

1. 当前阶段涉及的规范域均为 Frozen；
2. 独立复审没有未关闭的 Critical 或 Important finding；
3. 当前阶段计划与最终规范条款一致；
4. 前置阶段的质量门已经通过。

四项条件全部满足后自动进入对应阶段，无需逐阶段取得用户确认。对 I1 而言，S15 的五个版本域
均属于相关规范域；其他实施阶段同样不得依赖尚未 Frozen 的对应规范域。

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
选择已由 `docs/reviews/2026-07-16-render1-binary-raster-closure-review.md` 独立复审闭合；该 review
不包含 executable artifact。Static FCBC、independent loader/evaluator、decoder/shaper/raster 和 mutation
corpus 及其独立复审仍是重新 Frozen 的 gate。

Conversion Specification 1.0 的 parser/profile/Repair 分层、source/target selector、12-profile registry、
7 个 parser dialect、56 个 mapping rule、32 个 diagnostic/report category、38 个 exact mapping
vector、5 个非法边界、10 个选择/歧义向量以及 FCBC no-source-snapshot 投影记录在
`docs/reviews/2026-07-15-conversion1-semantic-profile-closure-review.md`。这些向量已由 Rust 强类型
manifest integrity test 验证，但活动 workspace 尚无 converter，也尚未提供真实外部 source→canonical
golden→target reparse 闭环；因此该 closure 仍是 Draft 候选证据。

四规范当前候选 bytes、跨域不变量、完整 workspace gate、suite/tree hash，以及 blocker ledger
统一记录在 `docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md` 及其 dated amendment。ABI
第 7.2 项已关闭；第 7.3 项所需规范文字已独立复审，但 executable RenderSection binary/raster
artifact 仍未实现；Conversion 真实 round-trip 和 Core fixture validation 同样开放。原联合候选自检
不是最终独立 review；全部剩余 blocker 关闭并完成联合独立复审前，本表状态不得提升为
Reviewed/Frozen。

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
重新走完整审查；必须保留旧审计记录、标记其已撤回，并在重新 Frozen 前禁止实现阶段依赖新
语义。已经公开的版本不得使用该例外。

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
