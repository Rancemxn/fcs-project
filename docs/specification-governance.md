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

## 2. 当前候选版本

| 规范 | 候选版本 | 状态 |
|---|---:|---|
| FCS Core Source Specification | 5.0.0 | Frozen（2026-07-14） |
| FCBC Container Format | 2.0.0 | Frozen（2026-07-14） |
| FCS Execution ABI | 1.0.0 | Frozen（2026-07-14） |
| FCS Render Profile | 1.0.0 | Frozen（2026-07-14） |
| FCS Conversion Specification | 1.0.0 | Frozen（2026-07-14） |

Draft/Reviewed/Frozen 是仓库发布状态，不写入 FCS 或 FCBC 的 SemVer 字段。Frozen 只表示
规范文本和绑定 conformance baseline 已稳定；参考实现仍必须逐项通过 conformance 后才能宣称
对应实现 conformance。

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
