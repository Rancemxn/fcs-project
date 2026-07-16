# 0010：使用阶段范围化 Reviewed Implementation Baseline 启动实现

状态：Partially superseded by ADR 0011（仅工作流追踪介质）

日期：2026-07-16

## 1. 背景

FCS 5 路线原先把 I1 的启动条件写成五个版本域全部 Frozen。该门槛能阻止实现替未闭合规范作出
决定，但把两个不同目标耦合在了一起：

- 一个具体实施阶段是否拥有足够稳定、已独立审查的输入；
- 整个 FCS 5 候选是否已经具备发布级、跨五版本域的完整 conformance baseline。

S15 暴露了这种耦合的循环：完整 Render raster、Conversion round-trip 和 Core canonical fixture
需要后续阶段的实际产品能力才能执行，而 I1 source parser 本身并不依赖这些能力。若仍要求所有
版本域先 Frozen，项目要么在实现前制造 test-only oracle 冒充产品能力，要么让与 I1 无关的后期
artifact 永久阻塞 I1。两种结果都会降低审计质量。

另一方面，直接允许实现跟随任意 Draft 文本也不可接受。阶段必须绑定精确条款、fixture、hash 和
独立 review；规范改变时，受影响实现必须重新验证，不能用“已经开始写代码”阻止规范修正。

## 2. 决策

### 2.1 新增阶段门概念

I1–I9 使用 **Reviewed Implementation Baseline（已复审实施基线）** 作为阶段启动门。它表示：某个
有限实施阶段所依赖的规范条款和 conformance 输入已经足够完整、无未关闭高严重度 finding，可由
实现机械落实。

Reviewed Implementation Baseline **不是新的规范版本状态**。规范版本状态仍只有：

```text
Draft / Reviewed / Frozen
```

建立 implementation baseline 不会把整章、整个文件或任一版本域提升为 Reviewed/Frozen，也不授权
发布、兼容性承诺或全规范 conformance 声明。

### 2.2 建立 baseline 的客观条件

每个阶段的 baseline 记录必须同时包含：

1. 阶段号、实现边界和明确排除范围；
2. 该阶段直接或经交叉引用依赖的规范条款；
3. 绑定 fixture、manifest entry、diagnostic category 和候选文件 SHA-256；
4. 未参与被审修改的独立 reviewer 对上述范围的复审证据；
5. 该范围内 Critical/Important finding 为零；
6. 阶段计划与绑定条款一致，且前置阶段质量门通过；
7. 已知的域外 blocker 及其 owner，证明它们不会改变本阶段产物的公开语义或接口。

若一个 open finding 能经 dependency closure 改变本阶段 AST、diagnostic、canonical value、ABI、资源
约束或其他公开行为，它就是 scope 内 finding，不能仅因文件或路线阶段不同而排除。

条件满足后自动开始对应阶段，无需逐阶段请求用户确认。用户介入只用于规范/ADR/证据无法唯一决定
的 materially different 公开语义，以及仓库批准门规定的不可逆或外部状态动作。

### 2.3 Baseline 的失效与重开

baseline 绑定精确 bytes，而不是只绑定 SemVer 或章节标题。建立后发生以下任一情况，必须重开受
影响阶段的 baseline：

- 绑定条款、交叉引用、fixture、expected result 或稳定 diagnostic category 改变；
- 新 finding 表明依赖闭包不完整或现有条款存在两个以上 materially different 解释；
- 实现发现规范与 fixture 不能同时满足；
- 前置阶段公开接口或 invariant 改变。

重开按影响传播，不作全局回退：只影响未使用该语义的其他阶段时，其 baseline 继续有效；若已完成
阶段的公开产物受影响，则该阶段和所有依赖阶段回到待复审状态。纯文字修正、内部重构或明确不改变
绑定行为的测试增强不使 baseline 失效，但必须记录为何 hash 变化不影响该范围。

不得为了保住 baseline 而冻结错误语义。发现规范缺陷时仍按治理流程先修规范、fixture、manifest 和
review，再修改实现。

### 2.4 I10 与发布候选仍要求完整 Frozen

I10 的 conformance release candidate gate 不使用局部 baseline 替代全局冻结。进入 I10 最终发行组合
和宣称本地 conformance RC 前，必须同时满足：

- FCS Core、FCBC Container、Execution ABI、Render Profile 和 Conversion Specification 五个版本域
  全部 Frozen；
- 跨规范独立复审没有未关闭的 Critical/Important finding；
- 全部适用 source、canonical、runtime、binary、conversion、render、CLI 与 repository gate 通过；
- implementation matrix 不再以 manifest integrity、空壳或 test-only oracle 冒充产品能力。

因此局部 baseline 只消除不必要的前置阻塞，不降低最终发布门槛。

## 3. 阶段记录职责

- `docs/specification-governance.md` 定义 baseline 与版本状态的关系和通用 gate；
- `docs/plans/fcs5-roadmap.md` 为每个阶段列出 normative dependency closure、排除范围和完成条件；
- 独立阶段计划记录具体 clause/fixture/hash checklist；
- `docs/reviews/` 保存不可静默改写的 baseline review、finding 和失效 amendment；
- `docs/conformance/fcs5-implementation-matrix.md` 记录条款 owner、阶段、实现和可执行证据；
- `.scratch/` 可以保存当前有限工作单元与 frontier，但不能获得规范权威。

历史 review 中“必须五域 Frozen 才能开始 I1”的结论保留为当时治理事实；新 dated amendment 应指出
其已由本 ADR 和当前治理规则取代，不得重写历史 hash、finding 或当时状态。

## 4. 明确禁止

- 不得把 Reviewed Implementation Baseline 缩写或展示成规范 `Reviewed` 状态；
- 不得以阶段计划、issue、实现或测试新增规范语义；
- 不得把域外 open finding 隐藏起来；必须记录其 owner、影响分析和后续 gate；
- 不得用即将实现的产品作为同一 baseline 的唯一独立 oracle；
- 不得让 test-only writer/loader/evaluator 成为未来产品 API 或 owning crate 的替代品；
- 不得因阶段已经开始而降低、删除或改写失败 fixture；
- 不得用局部 baseline 发行 FCBC、发布 crate 或宣称完整 FCS 5 conformance。

## 5. 对当前路线的影响

- I1 只需建立覆盖 FCS source syntax、source AST、parse-stage diagnostics、资源限制以及它实际读取的
  balanced profile envelope 的 baseline；Render raster、Conversion round-trip 和 Core canonical
  fixture execution 不再作为 I1 启动前置条件。
- Render 的当前 `RNR-*` finding 仍阻塞 Render 相关 baseline，但只有能改变 I1 parser 边界的 finding
  才阻塞 I1。
- I2–I9 依相同原则，在各自 dependency closure 无高严重度 finding 后自动衔接。
- S15 仍负责候选规范与 executable conformance 的逐域闭合；其未完成项不能被 implementation
  baseline 标记为已完成。
- I10 保留五域 Frozen 与最终联合复审门。

## 6. 后果

正面后果：

- source、static、canonical、runtime、container、conversion 和 render 可以按真实依赖顺序推进；
- 后期 conformance artifact 不再通过 test-only 前置实现制造循环；
- 每次规范变化的影响范围可由 clause/fixture/hash dependency 追踪；
- 自动阶段衔接具备客观、可审计条件，不依赖重复人工批准。

成本与约束：

- 每个阶段必须维护精确 dependency closure 和 baseline review；
- 跨规范 finding 需要做影响传播，不能只按文件名判断；
- 文档必须严格区分“阶段可实现”“规范 Reviewed/Frozen”和“产品 conforming”；
- I10 前仍需完成所有先前推迟的跨域 artifact 与最终联合复审。
