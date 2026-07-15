# Goal & Success Signal

- **Goal:** 从当前 FCS 5 工作区出发，按 `docs/plans/fcs5-roadmap.md` 和各权威规范持续完成 S15
  剩余闭合工作以及 I1–I10 的文档、conformance、Rust 实现、独立复审和治理记录，形成一个本地、
  可发布但尚未对外发布的 FCS 5 conformance release candidate。每个阶段在客观 gate 满足后自动
  进入下一阶段，不要求用户逐阶段确认。
- **Observable success signal:** 以下条件同时成立：
  - FCS Core、FCBC Container、Execution ABI、Render Profile 和 Conversion Specification 的当前
    候选版本均满足治理文件规定的 Frozen 条件；
  - S15 的 Execution ABI、RenderSection、Conversion round-trip 和 Core fixture validation blocker
    均由机器可执行 artifact 关闭；
  - 路线图 I1–I10 的每个 task 与阶段完成条件均有对应实现、测试、fixture、review 和记录；
  - source、canonical、runtime、FCBC、converter、Render 和 CLI 都是实际实现，不以空壳、manifest
    integrity test 或 test-only oracle 冒充产品能力；
  - implementation matrix 没有无 owner、无下一阶段或与实际证据不符的 `partial`/`blocked` 项；
  - 所有适用的 source fixture、binary golden/mutation、reference evaluation、conversion
    round-trip、render semantic/raster、property/fuzz、CLI end-to-end 和 workspace gate 通过；
  - 所有绑定 file/suite/tree hash 与实际 bytes 一致，本地链接和 UTF-8 检查通过；
  - 独立复审没有未关闭的 Critical 或 Important finding；
  - 工作区拥有可审查的本地提交历史和完整交付报告，但没有在未获批准时 push、tag、release 或
    发布 crate/artifact。
- **Observable failure signal:** 达到迭代上限、满足无进展条件、出现必须路由 HUMAN 的 residual，
  或任一声称完成的 checkpoint 仍有失败 gate、过期 hash、未关闭 Critical/Important finding、
  未经授权的公开语义选择或被测试/计划偷偷创造的规范行为。

# Scope & Authority

- `docs/specification-governance.md` 管理规范状态；`fcs.md`、`fcbc.md`、`fcs-render.md` 和
  `fcs-conversion.md` 在各自版本域定义规范性行为；Accepted ADR 约束设计方向但不替代规范文本；
  `docs/plans/fcs5-roadmap.md` 是唯一总实施路线。
- `docs/community/` 是外部格式证据综合，`refer/chart/` 是固定快照下的一手证据。外部格式结论
  必须遵守仓库阅读路由、固定 commit/hash 和多来源冲突规则；单个参考实现不得成为社区规范。
- 当前实现、example、fixture、reference harness 和外部项目都不能静默成为新规范。实现暴露规范
  缺口时，先按治理流程修订 Draft/重开的规范、fixture、manifest 和 review，经独立复审重新 Frozen
  后再让依赖该语义的产品实现继续。
- 用户已授权取消“I1 必须再次由用户确认计划”这一特殊历史门禁。执行者应将 `AGENTS.md`、治理
  文件、路线图和 I1 计划中的旧文字改成客观 gate：相关规范 Frozen、独立复审无未关闭
  Critical/Important、阶段计划与最终条款一致、前置质量门通过。满足后可自动开始 I1；I1–I10
  亦按相同原则自动衔接。
- 精确表达式、FCS authoring workspace、自包含单谱面 FCBC、原始资源 bytes、版本化 conversion
  semantic profile、无默认 baking、无 FCBC source snapshot/player cache 等已接受边界必须保持；
  改变这些边界需要 HUMAN routing，而不是由实现便利性决定。
- 不得覆盖或回退已有无关修改，不得把 `refer/` 作为 Cargo path dependency，不得恢复 FCS 4
  compatibility facade；只有路线阶段需要时才创建后续领域 crate。

# Termination Conditions

- **Max iterations / budget:** 最多 160 次迭代。一次迭代只承诺一个有限、具有明确验收项和匹配
  artifact 的工作单元；不得用扩大单次范围绕过上限。达到上限时停止，保留已验证检查点，输出
  已完成证据、当前 gate、剩余有限 backlog、residual 分类和下一份缩小范围的 loop 建议；不得
  声称目标完成或降低质量门。
- **Goal-achievement check:** 对照 Goal 的 observable success signal、路线图 task、implementation
  matrix、规范状态表、独立 finding ledger 和全部适用验证 artifact 逐项复核。只有所有条件同时
  满足才能终止为 achieved；实际 push/release 不属于完成条件。
- **No-progress condition:** 同一个阻塞条件连续 3 次迭代同时满足以下全部条件：没有关闭 active
  work unit 的任何 acceptance criterion；没有新增能唯一决定下一动作的验证证据；没有把问题拆成
  严格更小且分别有限可验收的单元；没有其他不依赖该 blocker 的安全路线图工作可推进。任务困难、
  一次方案失败、测试暴露 bug、发现新 finding 或需要规范修订本身不算无进展。
- **Worst-case Plan B:** 保留所有已验证提交和 artifact，把未完成范围收敛到最靠前的阶段或 blocker，
  为下一循环提供有限 backlog。不得通过把 Draft/Reviewed 冒充 Frozen、manifest integrity 冒充
  execution conformance、test-only harness 冒充产品实现、删除失败 fixture、放宽误差界、跳过
  fuzz/raster/round-trip 或临时诊断替代规范 category 来制造完成。

# Progress Invariant

- **Bounded quantity that must advance:** 当前 active work unit 在选定时必须拥有一个有限且编号的
  acceptance-criteria ledger。任何非终止迭代都必须使其中至少一个未满足 criterion 变为由
  domain-matched artifact 证明的 satisfied；迭代预算同时从 160 单调递减。
- **How each path advances or exits:** LOCAL 路径关闭至少一个 criterion；PLANNER 路径只能把工作
  改写为严格更小、各自有限且保持原验收覆盖的单元，并在本次迭代结束前选择可执行单元或终止；
  HUMAN 路径记录证据后退出受阻范围，并继续所有可分离工作，直到只剩 HUMAN residual 时终止。
  新发现的 finding 必须进入有 owner、severity、验收方法的有限 ledger，不能作为无限扩展范围的
  借口，也不能被忽略以维持表面进度。

# Reversible Change Authority

- 可以自动编辑仓库内文件、运行工具、更新 Draft 规范和计划、创建/切换本地 branch/worktree、
  stage 并创建本地检查点提交，无需逐次询问。
- 每个提交只包含一个可审查的规范、fixture、实现、review 或治理检查点。提交前运行与风险匹配的
  Measurement Domain gate；失败或尚未关闭 finding 的中间提交必须明确标为 Draft，不得描述为阶段
  完成、Reviewed 或 Frozen。
- 不 amend 用户已有提交，不 rebase/reset/checkout 丢弃现有工作，不清理无关 dirty changes，
  不让并行写入者覆盖共享文件。默认不 push。
- 规范/依赖/API 工作遵守根 `AGENTS.md` 的本地固定依赖源码与 Context7 路由；添加依赖必须记录
  版本、feature、MSRV、license、dependency tree 和激活范围，不能仅凭记忆。

# Approval Gates

只有不可逆或外部状态动作设置审批门。普通设计、实现、测试、计划更新、独立复审修复、规范状态
按客观条件迁移和本地提交不设人工门。

| Gate | Trigger | If approved | If denied |
|---|---|---|---|
| Remote source-control mutation | `git push`、force-push、创建/更新远程 PR 或修改远端 branch | 只执行获批的具体远端动作并记录结果 | 保留经验证的本地 branch/commit，继续所有可分离工作 |
| Public release | 创建公开 tag、GitHub Release、发布 crate、上传发行物或公开 conformance bundle | 按批准范围发布并执行发布后校验 | 保留本地 release candidate，不把未发布描述为已发布 |
| Destructive history/data operation | 删除或重写已有 Git 历史、branch、archive、用户数据或外部数据 | 仅对已明确目标执行，并先验证作用域 | 不执行；采用非破坏替代或保留 residual |
| Credential/system mutation | 使用凭据、签名密钥、付费服务、修改远端配置、安装系统级软件/驱动或改机器全局配置 | 在最小权限和明确作用域内执行 | 继续不依赖该能力的工作，必要时路由 HUMAN |
| Copyright/license distribution | 准备分发许可证或版权状态不明确的社区谱面、音频、图片、字体或其他资源 | 仅分发获批且有记录的材料 | 只保留本地 opt-in fixture lane，不纳入公开 artifact |

# Measurement Domain

| Output domain | Verification method | Required artifact |
|---|---|---|
| 规范与治理文档 | 条款/术语/版本/交叉引用审计；规范 example 与 conformance 映射；独立复审；状态转换条件复核 | 权威文件 diff、无缺失本地链接、finding ledger、状态/hash 记录 |
| Source grammar 与 AST | 每个 grammar production 的 valid/invalid coverage；精确 span/diagnostic；完整输入消费；limit/property/fuzz | production ledger、parse fixture 结果、bounded fuzz/property 报告 |
| Static/elaboration/canonical | 类型、名称、展开、稳定 ID、canonical invariant 和 source-reorder 等价性测试；later-stage fixture 真正执行 | canonical snapshot、invariant traversal、诊断与限额测试 |
| Runtime 与数值 ABI | reference evaluator 对 typed DAG、lazy semantics、seek、Track、Distance 和困难 binary64 vector 求值 | 输入向量、expected bits/trace、reference 与产品 evaluator 对比结果 |
| FCBC/Execution ABI | reference writer→static bytes→独立 loader→evaluator；CRC/SHA、section/record/reference、profile 和 mutation 验证 | 非空 `.hex` golden、声明式 manifest、mutation corpus、loader/evaluation 报告 |
| Conversion | 真实公开 PGR v1/v3、RPE、PEC source/package 经 exact ProfileBinding 解析、canonical、target、同 profile reparse 比较；capability/error-budget 边界 | 固定来源 fixture、canonical golden、resource bundle、ConversionReport/Fidelity bytes、round-trip 报告 |
| Render | RenderSection byte codec、resource decode/shaping、semantic draw list 与 reference raster 在规定容差内比较 | 非空 RenderSection golden、固定 image/font、semantic snapshot、raster image/diff |
| CLI 与发行组合 | 对每个命令、profile/resource/capability 参数、exit category、JSON/text diagnostic 和端到端组合执行 | command transcript、expected output/exit、package/tree/version 审计 |
| Rust workspace | 先 Clippy，再 nextest；rustfmt、diff、normal/dev dependency tree、结构搜索；不用普通 `cargo test` 作为默认，不用 `--release` | 完整命令与退出状态、精确测试通过/跳过数、依赖树摘要 |
| Repository/conformance integrity | 文件/树 hash 独立复算；UTF-8/NUL/链接检查；archive/master/workspace/refer 边界检查 | hash ledger、路径计数、`git status`、结构与链接审计结果 |

# Residual Routing

| Residual / failure | Route: LOCAL / PLANNER / HUMAN | Action |
|---|---|---|
| Clippy、test、fmt、hash、链接、manifest、golden、round-trip 或 raster 不一致 | LOCAL | 找到最先失败的真实原因，补匹配测试/证据，修复并重跑对应 gate |
| 规范缺口但权威规范、Accepted ADR 和固定证据能唯一决定结果 | LOCAL | 修订受影响规范、fixture、manifest、review 与治理记录，再独立复审 |
| 实现与规范冲突且证据表明是实现缺陷 | LOCAL | 修实现和回归测试，不让实现反向定义规范 |
| 工作单元过大、验收耦合、计划顺序错误或 measurement domain 不匹配 | PLANNER | 保留原验收覆盖，拆成更小有限单元或调整顺序/测量方法 |
| 同一 LOCAL 方案连续两次失败但存在其他技术路线 | PLANNER | 建立最小复现，替换方案；不得仅重复同一动作 |
| 两个以上合法设计产生 materially different 公开语义，规范/ADR/证据无法排序 | HUMAN | 提供证据、选项、影响与推荐值；继续所有不依赖该选择的工作 |
| 需要推翻 Accepted ADR 或用户已确认的产品边界 | HUMAN | 停止受影响实现，提出新 ADR 候选和迁移影响 |
| 不可逆动作、凭据、系统配置、许可证或版权分发问题 | HUMAN | 触发对应 Approval Gate；拒绝时保留本地安全状态 |
| 同一外部 blocker 连续三次满足 no-progress 且无安全替代工作 | HUMAN | 提交完整阻塞证据、已尝试路径和解除阻塞所需的最小输入 |
| 达到 160 次上限 | PLANNER | 终止本循环，输出剩余有限 backlog 和下一份缩小范围的 loop 建议 |

# Subagent Using Policy

所有角色共享工作区。最多并发三个子任务；同一权威规范域或文件集合最多一个写入角色。只读研究
可以与不相关写入并行；独立 reviewer 不得参与被审内容的设计或修改。共享文件发生冲突时，相关
写入立即停止并由主执行者整合。主执行者始终负责最终 diff、验证、治理状态和提交。

## Dispatch Point: Independent Review

- **Trigger:** 规范重新 Frozen、阶段完成、重大二进制/转换/渲染 contract 或重大实现准备通过 gate。
- **Role capability:** 未参与被审修改的只读独立 reviewer。
- **Tool boundary:** 只读规范、diff、fixture、manifest、hash、测试输出和固定参考资料；不得编辑、
  stage、commit、改变状态或接受作者结论代替复现。
- **Input contract:** 明确审查范围、权威条款、候选 commit/diff、验收项、运行命令、已知 residual 和
  禁止依赖的实现假设。
- **Output contract:** finding ledger；每项包含 severity、文件/紧凑位置、违反条款、可复现证据、
  影响和 disposition 建议；另列实际复现的 gate 与未覆盖范围。
- **Acceptance check:** Critical/Important finding 全部关闭并复审；无 finding 时也必须给出审查范围、
  复现 artifact 和限制，不能只写“通过”或提供内部推理。
- **Concurrency:** reviewer 可与不触及被审快照的只读研究并行；不得与被审写入并行。
- **Failure routing:** 缺证据为 LOCAL；审查范围/测量不匹配为 PLANNER；角色不独立或能力不可用为
  HUMAN。
- **Sub-task termination:** 一个有限审查范围，最多两次修正请求；连续两次未减少审查验收项则返回
  PLANNER，不自行扩大范围。

## Dispatch Point: External Evidence Research

- **Trigger:** PGR/RPE/PEC、依赖源码、codec、字体、许可证或外部 producer/runtime 行为需要固定证据。
- **Role capability:** 只读证据研究者，能够核对版本、schema、parser、调用方和独立来源。
- **Tool boundary:** 读取仓库权威资料、`refer/` 固定快照和公开只读资料；遵守参考仓库规则；不得
  修改权威规范、选择 semantic profile 或把单个实现推广成社区规范。
- **Input contract:** 具体事实问题、允许来源、要求的 commit/hash、目标路径、冲突标准和交付格式。
- **Output contract:** “项目/来源 + commit/hash/version + 路径/章节 + 可观察行为 + 冲突/限制”的证据表。
- **Acceptance check:** 主执行者能在固定来源复现每个结论；community 摘要与一手证据冲突被明确
  标出；规范选择仍由权威流程完成。
- **Concurrency:** 可同时进行多个互不依赖的只读调查，但总子任务数不超过三。
- **Failure routing:** 缺单一来源可换独立来源为 LOCAL；证据冲突为 PLANNER；许可/访问不可解决为
  HUMAN。
- **Sub-task termination:** 一个事实问题或一个固定格式/profile；最多两次补证，未收敛则返回证据
  缺口，不猜测结论。

## Dispatch Point: Bounded Implementation or Fixture Work

- **Trigger:** 子任务文件范围互不重叠、规范行为已唯一确定、验收命令明确，并且并行能缩短等待。
- **Role capability:** 有界全能力执行者，可在授权文件范围内实现代码、文档或 fixture。
- **Tool boundary:** 只修改输入合同列出的路径；可运行本地非破坏工具；不得改变产品语义、权威
  规范状态、用户无关修改、远端状态或提交历史。
- **Input contract:** 一个有限 deliverable、权威条款、允许/禁止文件、现有 dirty-state 说明、失败
  测试、验证命令、质量门、residual 路由和终止条件。
- **Output contract:** 实际修改路径、关键 diff、运行命令与精确结果、未解决 residual、对共享文件的
  任何风险；不得只声明完成。
- **Acceptance check:** 主执行者审查 diff，确认无越权修改，并独立运行 domain-matched gate；只有
  artifact 与测试都满足合同才接受。
- **Concurrency:** 同一文件集合或规范域只允许一个写入者；最多三个总子任务；主执行者整合后再
  提交。
- **Failure routing:** 普通实现/测试失败为 LOCAL；两次不同修正仍不收敛为 PLANNER；语义歧义、
  越权需求或不可逆动作立即返回 HUMAN/Approval Gate。
- **Sub-task termination:** 一个 deliverable，最多两次修正尝试；连续两次未减少验收项时停止并返回
  最小复现和 residual，不扩大任务。
