# Goal & Success Signal

- **Goal:** 从当前已合并的 I0/I1.1 基线和 I1.2 frontier 出发，按
  `docs/plans/fcs5-roadmap.md`、各阶段计划、权威规范和治理规则持续完成 I1–I10，并在各自 owning
  stage 关闭 S15 遗留 blocker，最终在 `main` 上形成一个可复现、可发布但尚未公开发布的 FCS 5
  conformance release candidate。客观 stage gate 满足后自动衔接，不要求逐阶段人工确认。
- **Observable success signal:** 以下条件同时成立：
  - FCS Core、FCBC Container、Execution ABI、Render Profile 和 Conversion Specification 五个版本域
    均满足 `docs/specification-governance.md` 的 Frozen 条件；
  - 路线图 I1–I10 的每个 task 和阶段完成条件都有已合并实现、测试、fixture、review 与治理证据；
  - source、canonical、runtime、FCBC、converter、Render 和 CLI 都是产品实现，不以空壳、manifest
    integrity test 或 test-only oracle 冒充能力；
  - S15 的 Core fixture execution、Conversion round-trip、FCBC/Execution ABI 和 Render executable
    blocker 均由 owning stage 的机器可执行 artifact 关闭；
  - implementation matrix 不含无 owner、无下一阶段或与实际证据不符的 `partial`/`blocked` 项；
  - 所有适用的 source/canonical/runtime、golden/mutation、round-trip、semantic/raster、property/fuzz、
    CLI end-to-end、hash、link、UTF-8 和 workspace gate 通过；
  - 最终联合独立复审没有未关闭的 Critical/Important finding；
  - 所有 RC 内工作均通过 PR 合并到 `main`，root Issue 的最终证据与实际 merge/hash/gate 一致并已关闭；
  - 不存在影响规范、conformance、路线图验收、安全性、正确性或可复现性的 open Issue。只有明确属于
    RC 非目标的 Minor/增强 follow-up 可以继续开放；
  - 未为本 RC 创建公开 tag、GitHub Release，未发布 crate，也未上传公开 release/conformance bundle。
- **Observable failure signal:** 达到 240 个 work-unit iterations、满足全局 no-progress、只剩无法解除的
  HUMAN residual，或任一声称完成的 gate 仍有失败检查、过期 hash、未关闭 Critical/Important finding、
  未授权公开语义选择、未合并交付或由计划/Issue/测试偷偷创造的规范行为。

# Scope & Authority

- `docs/specification-governance.md` 管理版本状态；`fcs.md`、`fcbc.md`、`fcs-render.md` 和
  `fcs-conversion.md` 在各自版本域定义规范性行为；Accepted ADR 约束设计方向但不替代规范文本；
  `docs/plans/fcs5-roadmap.md` 是唯一总实施路线。
- `docs/community/` 是外部格式证据综合，`refer/chart/` 是固定快照下的一手证据。外部格式结论
  必须遵守仓库阅读路由、固定 commit/hash 和多来源冲突规则；单个参考实现不得成为社区规范。
- Issue、PR、计划、实现、example、fixture、reference harness、skill 和外部项目都不能静默成为新
  规范。规范缺口按治理流程处理，不能由实现便利性决定。
- I1–I9 仅在对应完整 normative dependency closure 建立 Reviewed Implementation Baseline 后实施；
  baseline 失效时只重开受影响阶段及其依赖阶段。I10/本地 RC 仍要求五个版本域 Frozen、最终联合
  独立复审和完整 executable conformance。
- 精确表达式、FCS authoring workspace、自包含单谱面 FCBC、原始资源 bytes、版本化 Conversion
  semantic profile、无默认 baking、无 FCBC source snapshot/player cache 等已接受边界必须保持。
- 不覆盖或回退无关修改，不把 `refer/` 作为 Cargo path dependency，不恢复 FCS 4 compatibility
  facade；后续领域 crate 和依赖只在 owning stage 的 gate 允许后创建或激活。

# Termination Conditions

- **Max iterations / budget:** 最多 240 个 work-unit iterations。一次 iteration 是对一个有限 Issue
  acceptance unit 的一次有界实施尝试，不是命令、commit、Progress message 或等待轮询。每次消耗一份
  预算；不得通过扩大单元、重复命名或拆出等价 Issue 绕过上限。
- **Goal-achievement check:** 对照 Goal 的全部 success signal、路线图 task、implementation matrix、
  五域状态、root/child Issue 依赖、合并 PR、finding ledger 和全部适用 domain artifact 逐项复核。
  只有这些证据同时成立才能以 achieved 终止；公开发布不属于完成条件。
- **Per-Issue no-progress:** 两次不同技术路径都没有关闭验收项或减少未决问题时转 PLANNER；第三次
  仍没有新增决定性证据时，把该 Issue 路由为 `needs-info` 或 `ready-for-human`，保留证据并转向
  其他依赖独立的工作。
- **Global no-progress:** 连续 3 个 work-unit iterations 均未关闭验收项、未新增能唯一决定下一动作的
  证据、未产生严格更小且可独立验收的 ready unit，并且整个 frontier 已无其他
  `ready-for-agent` 工作时终止。单纯新建 Issue、重复同一检查或改写说明不算进展。
- **Worst-case Plan B:** 保留所有已合并 checkpoint 和可复现 artifact，把未完成范围收敛到最早
  blocker，输出有限 backlog、依赖、residual 分类和解除条件。达到预算时由 PLANNER 产出仍指向 I10
  同一目标的后继 `loop.md`；不得把目标缩到某个阶段或降低 gate。

# Progress & Frontier Invariant

- **Persistent objective:** GitHub root Issue 固定 I10 目标、success signal、全局 blocker 和当前
  frontier；每个可独立验收的 work unit 使用 bounded child Issue 和一个 linked branch/PR。root Issue
  只在 stage gate、frontier 或重大 blocker 变化时更新，不镜像每个 commit 或 child checkpoint。
- **Current state authority:** root Issue 的最新有效 checkpoint、child Issue dependency graph、已合并
  PR 和仓库 gate artifact 共同构成当前状态证据。`.scratch/fcs5-rc` 只保留历史，不得作为当前
  request surface、iteration count 或 frontier。
- **Bounded quantity that must advance:** active child Issue 在开始时拥有有限且编号的 acceptance
  criteria 和未决 decision residual；任何非终止 iteration 必须关闭至少一个 criterion、消除一个
  decision residual、完成保持原验收覆盖的严格缩小拆分，或按 Residual Routing 退出该路径。240 预算
  同时单调递减。
- **Frontier selection:** 默认选择路线图中最早、依赖已满足的 `ready-for-agent` Issue，优先关闭
  当前 stage gate，不以容易的后期任务长期回避关键路径 blocker。
- **Safe look-ahead:** 当前路径受阻时，可以推进不依赖该 blocker 的后续规范闭包研究、fixture 设计、
  计划或独立证据，但它必须关闭一个明确的未来 gate。在前置质量门和本阶段 Reviewed Implementation
  Baseline 通过前，不创建未来产品 crate、不激活 owning-stage 依赖，也不合并依赖未稳定接口的产品
  实现。
- **Deferred Issue boundary:** 能改变当前 stage 公开产物、dependency closure 或 acceptance criteria
  的 Issue 阻塞受影响 gate，但不阻塞可分离工作；经证据证明不影响当前 stage 的 Issue 必须记录 owner、
  目标 stage、依赖与验收方法后才能延期。RC 成功时只允许明确的 post-RC Minor/增强 follow-up 开放。
- **Path invariant:** LOCAL 关闭或减少 active ledger；PLANNER 只能严格缩小、重新排序或改变匹配的
  measurement；HUMAN 路径保存选择所需证据并退出受影响范围。任何路径若既不前进也不退出，即为
  undeliverable。

# Authorized Change & Delivery

- 可以自动进行仓库内设计、实现、测试、fixture、计划、review 和治理修改，以及正常的 GitHub
  Issue/branch/push/PR/review/merge 生命周期；所有远端进度与网络行为遵守 `AGENTS.md` 和 ADR 0011，
  本文件不复制其操作规则。
- 每个 branch/PR 只交付一个可审查 work unit。提交和 push 前审查作用域与 diff；不 amend 用户提交，
  不 rebase/reset/checkout 丢弃工作，不清理无关 dirty changes。
- 普通 merge 已获持续授权，但只有 child Issue acceptance criteria、适用验证和独立复审要求全部满足，
  PR 为 Ready 且 mergeable、required checks 与 review requirements 满足、没有未解决 review thread，
  并已记录 delivery-ready 证据时才可执行。不得使用 `--admin`、force-push、降低 gate 或隐藏 finding。
- stage 的客观 gate 满足后自动进入下一 frontier；不为已经由规范、ADR、fixture 和证据唯一决定的
  普通实现选择反复请求确认。
- 规范/依赖/API 工作遵守根 `AGENTS.md` 的固定依赖源码和 Context7 路由。添加依赖必须记录版本、
  feature、MSRV、license、dependency tree 和激活范围。

# Approval Gates

Routine GitHub delivery 和满足 Authorized Change & Delivery 条件的普通 merge 已获授权。以下动作仍须
单独取得明确批准：

| Gate | Trigger | If approved | If denied |
|---|---|---|---|
| Public release | 创建公开 tag、GitHub Release、发布 crate、上传发行物或公开 conformance bundle | 只按批准范围发布并执行发布后校验 | 保留已合并的本地 RC，不把它描述为已公开发布 |
| Destructive history/data operation | 删除或重写已有 Git 历史、branch、archive、用户数据或外部数据 | 仅对明确目标执行，并先验证作用域 | 不执行；采用非破坏替代或保留 residual |
| Credential/system mutation | 使用签名密钥、付费服务、修改远端保护/配置、安装系统级软件/驱动或改机器全局配置 | 在最小权限和明确作用域内执行 | 继续所有不依赖该能力的工作，必要时路由 HUMAN |
| Copyright/license distribution | 把许可证或版权状态不明确的谱面、音频、图片、字体等纳入公开分发 | 仅分发获批且有证据记录的材料 | 只保留本地 opt-in fixture lane，不进入公开 artifact |

# Measurement Domain

每个 Issue 选择足以发现当前错误的最小 focused feedback；只在 `AGENTS.md` 定义的适用交付检查点
运行全量 Rust gate。验证记录必须区分 passed、failed、skipped 和 non-applicable，不能把缺失 gate
写成通过。

| Output domain | Verification method | Required artifact |
|---|---|---|
| 规范与治理文档 | 条款/术语/版本/交叉引用审计；example/conformance 映射；独立复审；状态转换条件复核 | 权威文件 diff、链接审计、finding ledger、状态/hash 记录 |
| GitHub delivery evidence | 核对 root/child 依赖、Issue acceptance、PR diff/merge state、review thread 与实际 gate | linked Issue/PR、merge SHA、验证结果和 residual owner；不获得规范权威 |
| Source grammar 与 AST | 每个 production 的 valid/invalid coverage；精确 span/diagnostic；完整消费；limit/property/fuzz | production ledger、fixture 执行结果、bounded fuzz/property 报告 |
| Static/elaboration/canonical | 类型、名称、展开、稳定 ID、canonical invariant、source-reorder 等价和 later-stage fixture 执行 | canonical snapshot、invariant traversal、诊断与限额结果 |
| Runtime 与数值 ABI | reference evaluator 对 typed DAG、lazy semantics、seek、Track、Distance 和困难 binary64 vector 求值 | 输入向量、expected bits/trace、reference 与产品 evaluator 对比 |
| FCBC/Execution ABI | reference writer→static bytes→独立 loader→evaluator；CRC/SHA、section/record/reference、profile、mutation | 非空 golden、声明式 manifest、mutation corpus、load/evaluation 报告 |
| Conversion | 真实固定来源 PGR v1/v3、RPE、PEC 经 exact ProfileBinding 完成 parse→canonical→target→同 profile reparse；验证 capability/error budget | source/package fixture、canonical golden、resource bundle、ConversionReport/Fidelity bytes、round-trip 报告 |
| Render | RenderSection codec、resource decode/shaping、semantic draw list 和 reference raster 容差比较 | 非空 RenderSection golden、固定 image/font、semantic snapshot、raster/diff |
| CLI 与发行组合 | 命令、profile/resource/capability/budget 参数、exit category、JSON/text diagnostic 和端到端组合 | command transcript、expected output/exit、package/tree/version 审计 |
| Rust workspace | 编辑循环运行受影响 focused check；适用 full checkpoint 按 `cargo fmt --all -- --check`、Clippy、workspace nextest 顺序执行；不用普通 `cargo test` 作默认，不用 `--release` | 精确命令/退出状态/测试计数、跳过原因、normal/dev dependency tree 和结构审计 |
| Repository/conformance integrity | file/suite/tree hash 独立复算；UTF-8/NUL/链接；archive/main/workspace/refer 边界 | hash ledger、路径计数、`git status`、结构与链接审计 |

# Residual Routing

| Residual / failure | Route: LOCAL / PLANNER / HUMAN | Action |
|---|---|---|
| focused/full test、Clippy、fmt、hash、link、manifest、golden、round-trip 或 raster 不一致 | LOCAL | 找到最先失败的原因，补匹配证据，修复并只重跑可能失效的 gate |
| 规范缺口且权威规范、Accepted ADR 和固定证据能唯一决定结果 | LOCAL | 按治理流程更新规范、fixture、manifest、review 与状态记录；重建受影响 baseline，I10/发布再完成 Frozen gate |
| 实现与规范冲突且证据表明是实现缺陷 | LOCAL | 修实现和回归证据，不让实现反向定义规范 |
| active unit 过大、验收耦合、顺序错误或 measurement domain 不匹配 | PLANNER | 保留原验收覆盖，拆成严格更小的 bounded Issues，或调整顺序/测量 |
| 两次不同技术路径仍未减少验收项或 decision residual | PLANNER | 建立最小复现并重新规划；第三次仍无决定性证据则退出该 Issue |
| 当前 stage dependency Issue 未关闭 | PLANNER | 阻塞受影响 gate，继续可分离工作；不得把挂起当作完成 |
| finding 经证据证明属于 later stage | PLANNER | 记录 owner、目标 stage、依赖与验收方法后延期，并在 owning gate 前重新进入 frontier |
| 两个以上合法设计产生 materially different 公开语义，规范/ADR/证据无法排序 | HUMAN | 提供证据、选项、影响与推荐；停止依赖该选择的实现，继续可分离工作 |
| 需要推翻 Accepted ADR 或用户已确认的产品边界 | HUMAN | 停止受影响范围，提出新 ADR 候选和迁移影响 |
| 第三次尝试仍无决定性证据，或外部输入/能力缺失 | HUMAN | 标记 `needs-info` 或 `ready-for-human`，记录最小所需输入并退出受影响路径 |
| 不可逆动作、凭据、系统配置或版权/许可证分发 | HUMAN | 触发 Approval Gate；拒绝时保留本地安全状态 |
| 连续 3 次满足全局 no-progress 且无 ready frontier | HUMAN | 终止并提交完整阻塞证据、已尝试路径和解除条件 |
| 达到 240 次上限 | PLANNER | 终止本轮，保留合并证据并产出仍指向 I10 的后继 loop 建议 |

# Subagent Using Policy

Subagent 不是每个 work unit 的必需步骤。主执行者与最多三个子任务共享工作区；同一文件集合或规范域
最多一个写入角色。只读研究可与不相关写入并行；共享文件冲突时相关写入立即停止，由主执行者统一
审查、验证和交付。子任务不得自行切换 branch、commit、push、创建/修改 Issue/PR 或 merge。

## Dispatch Point: Independent Review

- **Trigger:** stage baseline 建立/重开、规范重新 Frozen、stage 完成，或重大 binary/conversion/render
  contract 与实现准备通过 gate。
- **Role capability:** 未参与被审修改的只读独立 reviewer。
- **Tool boundary:** 只读固定 commit、规范、diff、fixture、manifest、hash、测试输出和允许的固定证据；
  不编辑、改变状态或接受作者结论代替复现。
- **Input contract:** 有限审查范围、权威条款、固定 commit/artifact、验收项、复现命令、已知 residual
  和禁止依赖的实现假设。
- **Output contract:** finding ledger；每项含 severity、紧凑位置、违反条款、可复现 artifact、影响和
  disposition 建议；另列实际复现 gate 与未覆盖范围。
- **Acceptance check:** Critical/Important 全部关闭并复审；零 finding 也必须给出审查范围、复现
  artifact 和限制。
- **Concurrency:** reviewer 不与被审 snapshot 的写入并行；可与不触及该 snapshot 的只读研究并行。
- **Failure routing:** 缺证据为 LOCAL；范围/测量不匹配为 PLANNER；角色不独立或能力不可用为 HUMAN。
- **Sub-task termination:** 一个固定审查范围，最多两次证据澄清；连续两次未减少审查 residual 时返回
  PLANNER，不自行扩大范围。

## Dispatch Point: External Evidence Research

- **Trigger:** PGR/RPE/PEC、依赖源码、codec、字体、许可证或外部 producer/runtime 行为需要固定证据。
- **Role capability:** 只读证据研究者，能核对 version、commit/hash、schema/parser、调用方和独立来源。
- **Tool boundary:** 读取仓库权威资料、允许的 `refer/` 固定快照和公开只读资料；不修改规范、不选择
  semantic profile，也不把单个实现推广为社区规范。
- **Input contract:** 一个事实问题、允许来源、固定 version/commit/hash、目标路径、冲突标准和交付格式。
- **Output contract:** “来源 + version/commit/hash + 路径/章节 + 可观察行为 + 冲突/限制”的证据表。
- **Acceptance check:** 主执行者能在固定来源复现结论，且冲突与未知项显式保留。
- **Concurrency:** 可并行调查互不依赖的问题，但全部子任务总数不超过三个。
- **Failure routing:** 可替代来源缺失为 LOCAL；来源冲突为 PLANNER；许可/访问无法解决为 HUMAN。
- **Sub-task termination:** 一个事实问题，最多两次补证；仍不收敛则返回证据缺口，不猜测结论。

## Dispatch Point: Bounded Implementation or Fixture Work

- **Trigger:** 文件范围互不重叠、规范行为已唯一确定、验收命令明确，且并行能缩短当前 ready work。
- **Role capability:** 有界写入执行者，可在授权路径内实现代码、文档或 fixture。
- **Tool boundary:** 只修改 input contract 列出的路径并运行本地非破坏工具；不改变公开语义、规范
  状态、无关修改、远端状态或提交历史。
- **Input contract:** 一个有限 deliverable、权威条款、允许/禁止路径、dirty-state、失败证据、验证
  命令、residual routing 和终止条件。
- **Output contract:** 修改路径、关键 diff、命令与精确结果、未解决 residual 和共享文件风险。
- **Acceptance check:** 主执行者审查完整 diff、确认无越权，并独立运行 domain-matched acceptance gate。
- **Concurrency:** 同一文件集合或规范域只有一个 writer；全部子任务总数不超过三个。
- **Failure routing:** 普通实现失败为 LOCAL；两次不同修正不收敛为 PLANNER；语义歧义或越权需求为
  HUMAN。
- **Sub-task termination:** 一个 deliverable，最多两次修正；连续两次未减少 acceptance residual 时
  停止并返回最小复现，不扩大任务。
