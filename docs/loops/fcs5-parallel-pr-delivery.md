# FCS5 Parallel PR Delivery（冻结工作流）

本文档是唯一命名交付契约。它吸收并取代原 `docs/loops/loop.md`（主实现 loop）与
`docs/loops/review-loop.md`（独立审查 loop），并合并两套角色的义务：重试与 pending remote sync
outbox、append-only 进度、Primary audit 与 Audit result、finding contract、Approval Gates、
Measurement Domain、worktree 清理与 Residual Routing 全部保留，只是改由并行 lane、两个 GitHub App
和父编排者执行。

## Freeze Protocol

- 本文件是**工作流冻结**（workflow freeze），不是规范版本域 `Frozen`（`docs/CONTEXT.md` 的
  `Frozen` 是版本域术语，只由 `docs/specifications/governance.md` 管理）。本冻结不改变任何
  规范状态，也不把本文件变成规范性资料。
- 冻结含义：本文件是 FCS5 交付的唯一当前执行契约。勘误、澄清和排版修正以**dated errata**
  追加到本文件末尾或作为 superseding comment 记录，不静默改写既有条款。
- **实质变更**（改变角色权限、lane 结构、gate 义务、预算或 supersession 关系）必须：
  1. 新建 ADR 并在本文件记录 supersession；
  2. 产出新的命名工作流文档（如 `docs/loops/<new-name>.md`）；
  3. 新文档明确声明取代本文件后，才能删除或冻结本文件。
- 本文件不是执行器或运行时机制；它不产生规范语义、不替代 Issue/PR、计划、复审或 fixture
  证据，也不自行声明阶段完成。

# Goal & Success Signal

- **Goal:** 从 GitHub root Issue 的最新有效 checkpoint 和最早 dependency-ready frontier 出发，按
  `docs/plans/fcs5-roadmap.md`、各阶段计划、权威规范和治理规则持续完成 I1–I10，并在各自 owning
  stage 关闭 S15 遗留 blocker，最终在 `main` 上形成一个可复现、可发布但尚未公开发布的 FCS 5
  conformance release candidate。客观 stage gate 满足后自动衔接，不要求逐阶段人工确认。
- **Observable success signal:** 以下条件同时成立：
  - FCS Core、FCBC Container、Execution ABI、Render Profile 和 Conversion Specification 五个版本域
    均满足 `docs/specifications/governance.md` 的 Frozen 条件；
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
  - 每个非机械实现 PR 都有 Delivery App 针对固定 `Issue/PR + head SHA + scope + commands + full-gate
    evidence` 的 append-only `Primary audit result`，并在通过后才由 Rancemxn Ready/merge；Review App
    可以随后追加 `Audit result` 二审，审查失效时已重新审查，且最终 I10 frontier 没有未关闭的
    Critical/Important finding；
  - 已结束 work-unit 的临时 lane worktree 均已安全清理；仍在使用的隔离 worktree 都有 owner、用途、
    固定 SHA 和明确的清理条件，不存在无人负责的 stale worktree；
  - 不存在影响规范、conformance、路线图验收、安全性、正确性或可复现性的 open Issue。只有明确属于
    RC 非目标的 Minor/增强 follow-up 可以继续开放；
  - 未为本 RC 创建公开 tag、GitHub Release，未发布 crate，也未上传公开 release/conformance bundle。
- **Observable failure signal:** 达到 240 个 work-unit iterations、满足全局 no-progress、只剩无法解除的
  HUMAN residual，或任一声称完成的 gate 仍有失败检查、过期 hash、未关闭 Critical/Important finding、
  未授权公开语义选择、未合并交付或由计划/Issue/测试偷偷创造的规范行为。

# Scope & Authority

- `docs/specifications/governance.md` 管理版本状态；`docs/specifications/fcs.md`、
  `docs/specifications/fcbc.md`、`docs/specifications/fcs-render.md` 和
  `docs/specifications/fcs-conversion.md` 在各自版本域定义规范性行为；Accepted ADR 约束设计方向但不替代规范文本；
  `docs/plans/fcs5-roadmap.md` 是唯一总实施路线。
- 本文件是设计契约，不是执行器或运行时机制；它不产生规范语义、不替代 Issue/PR、计划、复审或
  fixture 证据，也不自行声明阶段完成。
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

# Roles & Permissions

| 角色 | 身份 | 权限 |
|---|---|---|
| 人类 owner | `Rancemxn` | 唯一可以更新 `main`、将 PR 标记 Ready、合并 PR、关闭 root/主 Issue、修改主 Issue workflow label、批准 Approval Gates 的角色；`gh pr ready`、`gh pr merge`、push `main` 只属于 Rancemxn |
| Delivery App | `fcs5-delivery-rancemxn[bot]` | 拥有 Issue 创建/编辑/评论、lane 分支 push、draft PR 创建/评论、`Primary audit result` 与 delivery Progress 评论；绝不 Ready/merge/push `main`，绝不关闭主 Issue |
| Review App | `fcs5-review-rancemxn[bot]` | 拥有固定 SHA 的 `Audit result`、finding Issue 创建/评论、`gh pr review --comment`/`--request-changes`；绝不实现、不 push、不创建 corrective PR、不 Ready/merge/push `main` |
| 父编排者 | parent orchestrator（运行在 `C:/Users/Admin/Desktop/fcs-project`） | 动态调度互不冲突的 lane、为每个 lane 启动独立 Pi 实例/subagent、综合 Review App 结果、排序 merge 请求、把冲突委托给独立 corrective lane；不绕过 Rancemxn 的 main/Ready/merge 权限 |
| Lane agent | 每个 lane 一个 writer（`worker`），另设 `reviewer`/`scout`（只读）、`researcher`（只读 + Tavily）、`delegate`/`oracle`（只读咨询） | 按 `.pi/settings.json` 的 `subagents.agentOverrides` 固定 model、thinking 与 FastCtx 工具 allowlist；worker 是唯一 writer，reviewer/scout 只读 |

- 两个 Bot 都不能绕过 main 规则：任何 Bot 的 comment、run 或 progress 都不构成 merge 授权。
- 独立 Pi 实例/subagent 不是第三个实现会话，而是父编排者控制的 lane 角色；subagent 不得自行
  切换 branch、commit、push、创建/修改 Issue/PR、Ready 或 merge。
- 所有角色共用同一份 GitHub Comment Markdown Contract 与重试/outbox 规则（见下）。

## Subagent 配置契约

- 项目 `.pi/settings.json` 固定全部内置 agent：
  - model 统一为 `opencode-go/deepseek-v4-flash`；
  - thinking 默认 `max`；只有简单有界任务才允许 `high`；只有纯机械任务才允许 `off`；
  - 工具为直接 FastCtx allowlist：`worker` 是唯一 writer（`acceptanceRole: writer`，含
    `edit`/`write`/`replace`/`run`/`run_background`/`job_*`），是唯一 lane/仓库写入者；
    `reviewer`/`scout` 是仓库只读（`acceptanceRole: read-only`，`read`/`grep`/`glob`/`run`），其中
    `scout` 额外保留 `write`，但只限于非仓库 artifact 输出；`researcher` 仓库只读，保留
    `tavily_hikari`（Tavily）与 artifact-only `write`；
  - artifact-only `write`（`scout`/`researcher`）绝不授权仓库编辑或第二个 lane writer；任何 agent
    都不得通过 tool 绕过 lane 隔离或获得 Ready/merge/push `main` 能力。

# Termination Conditions

- **Max iterations / budget:** 最多 240 个 work-unit iterations（主交付预算）与 480 个 review-unit
  （独立复审预算），两套预算独立计算。一次 iteration 是对一个有限 Issue acceptance unit 的一次
  有界实施尝试，不是命令、commit、Progress message 或等待轮询；一次 review-unit 只审查一个固定
  目标及其通过后的架构/文档 advisory pass。不得通过扩大单元、重复命名或拆出等价 Issue 绕过上限；
  frontier 等待不消耗任何预算。
- **Goal-achievement check:** 对照 Goal 的全部 success signal、路线图 task、implementation matrix、
  五域状态、root/child Issue 依赖、合并 PR、finding ledger 和全部适用 domain artifact 逐项复核。
  只有这些证据同时成立才能以 achieved 终止；公开发布不属于完成条件。
- **Per-Issue no-progress:** 两次不同技术路径都没有关闭验收项或减少未决问题时转 PLANNER；第三次
  仍没有新增决定性证据时，把该 Issue 路由为 `needs-info` 或 `ready-for-human`，保留证据并转向
  其他依赖独立的工作。审查侧同样：两次不同的复现/证据路径没有缩小 residual 时缩小 scope 或标记
  证据缺口并转 PLANNER；第三个不同 SHA 仍无决定性证据则追加 `needs-info`/`ready-for-human` finding。
- **Global no-progress:** 连续 3 个 work-unit iterations 均未关闭验收项、未新增能唯一决定下一动作的
  证据、未产生严格更小且可独立验收的 ready unit，并且整个 frontier 已无其他 `ready-for-agent` 工作时
  终止。单纯新建 Issue、重复同一检查或改写说明不算进展。
- **Persistent idle wait（Review App）:** I10 未完成且无固定可审目标时，每 1 分钟执行一次 Frontier
  Sync 并持续重复；每 10 次只是一个观察批次，批次结束后自动继续，不结束 reviewer 目标、不标记
  `blocked`、不消耗 review-unit 预算。`waiting-for-main` 只表示轮询状态，不是终止 residual；进程被
  外部中断后，下次启动也必须从远端状态恢复轮询。
- **Terminal completion（Review App）:** 只有 root Issue 的 I10 success signal 已满足，并且 Frontier
  Sync 同时确认没有新的固定 review target、未分配的 Critical/Important finding、待复审的 corrective
  PR/merged SHA 或 reviewer 自己保留的未清理 worktree 时，才可以终止并报告审查 frontier 闭合。
- **Worst-case Plan B:** 保留所有已合并 checkpoint 和可复现 artifact，把未完成范围收敛到最早
  blocker，输出有限 backlog、依赖、residual 分类和解除条件。达到预算时由 PLANNER 产出仍指向 I10
  同一目标的**新命名工作流文档**（按 Freeze Protocol 取代本文件）；不得把目标缩到某个阶段或降低
  gate。Review App 的 Plan B 保留所有已发送 Audit result 和 finding Issue，列出未审目标、证据缺口、
  owner 和下一解除条件；不得把审查未完成描述为通过，也不得自行合并或关闭阻塞项。

# Progress & Frontier Invariant

- **Persistent objective:** GitHub root Issue 固定 I10 目标、success signal、全局 blocker 和当前
  frontier；每个可独立验收的 work unit 使用 bounded child Issue 和一个 linked lane/PR。root Issue
  只在 stage gate、frontier 或重大 blocker 变化时更新，不镜像每个 commit 或 child checkpoint。
- **Current state authority:** root Issue 的最新有效 checkpoint、child Issue dependency graph、已合并
  PR、Review App 的 finding ledger 和仓库 gate artifact 共同构成当前状态证据。`docs/scratch/fcs5-rc`
  只保留历史，不得作为当前 request surface、iteration count 或 frontier。本文件不复制瞬时
  commit/Issue 状态；若文档与动态证据冲突，按该 authority 修订文档，不能据此声称完成。
- **Frontier synchronization:** lane 与 Review App 异步运行，不能假定任何一方会收到事件通知。每个
  work-unit 开始和结束时、提交或 push 前、创建或更新 PR 前、发送 `Review requested` 前，以及依赖
  远端状态的动作（Ready、review、merge）前，必须执行一次只读 Frontier Sync。Sync 至少核对
  `origin/main`、root/child/finding Issue、开放 PR、workflow/severity label、PR head SHA、mergeability、
  review decision、required checks 和最新 comments；使用 `gh --json`/`gh api` 与 `jq`（经 Delivery/Review
  App 的 token，不使用本地 `gh`），并遵守 Retry & Outbox 规则。
- **New finding gate:** 当前 work-unit 合并前发现 `Critical`/`Important` finding、声明当前 gate 被阻塞的
  finding 或与当前 dependency closure 不一致的 corrective PR 时，立即冻结该 work-unit 的提交、push、
  PR Ready 和 merge。合并后的异步 reviewer 若发现同等级问题，不回滚已合并提交；冻结受影响的
  阶段声明和依赖其正确性的后续 work-unit，处理 corrective PR 并重新验证。只能保留不触及受影响
  快照、且明确关闭未来 gate 的安全 look-ahead；later-stage 或符合延期条件的 Minor 必须追加 owner、
  目标 stage、解除条件和 Issue 后才可继续。
- **Sync record:** 每次交付检查点记录查询到的 `origin/main` SHA、活动 Issue/PR/finding、阻塞分类和下一
  动作；不要把本地猜测或旧文本当作 frontier。
- **Bounded quantity that must advance:** active child Issue 在开始时拥有有限且编号的 acceptance
  criteria 和未决 decision residual；任何非终止 iteration 必须关闭至少一个 criterion、消除一个
  decision residual、完成保持原验收覆盖的严格缩小拆分，或按 Residual Routing 退出该路径。主交付
  的 240 预算单调递减；Review App 的 480 review-unit 预算独立递减。
- **Remote gate state:** 需要编译或测试反馈的修改以 draft PR 上的新固定 SHA 通过
  `workflow_dispatch` 触发候选 SHA full gate（见 Measurement Domain）；`queued`/`in_progress` 只是待验证
  状态，不算通过或 iteration 进展。成功的同 SHA run 可以关闭验证项；失败 run 必须产生决定性证据
  并由修正后的新 SHA 推进，否则按 no-progress 路由。同 SHA 的瞬时基础设施重跑和等待 Action 都不
  消耗 work-unit；新 SHA 取消的旧 run 是过期证据，不算当前 gate 失败；cache miss 也不改变 gate。
- **Frontier selection:** 默认选择路线图中最早、依赖已满足的 `ready-for-agent` Issue，优先关闭
  当前 stage gate，不以容易的后期任务长期回避关键路径 blocker。父编排者按依赖闭包并行调度
  互不冲突的 lane。
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

# Lane Lifecycle

- **Issue-first:** 每个 work unit 从 bounded child Issue 开始（写明范围、权威输入、验收条件、非目标、
  依赖和验证方法），然后建立 lane；不在没有 Issue 的情况下开 branch。
- **Lane 拓扑:** 父编排者拥有 `C:/Users/Admin/Desktop/fcs-project`；`main` worktree 固定为
  `C:/Users/Admin/Desktop/fcs-project/main`；每个 lane worktree 固定为
  `C:/Users/Admin/Desktop/fcs-project/worktree/<issue>-<slug>`，branch 命名为
  `codex/<issue>-<slug>`，从最新 `origin/main` 创建。
- **One writer per lane:** 同一 lane 只有一个 worker 写入；父编排者调度时保证不同 lane 的文件集合
  互不冲突，同一文件集合或规范域只有一个 writer。全部并行子任务总数不超过三个。
- **Sparse checkout 与 `refer` junction:** lane worktree 使用 sparse checkout 排除 `refer`，并建立
  只读 `refer` junction 指向 `main/refer`（真实树只存在一次，junction 使 lane 可见）。`/refer` 已在
  `.gitignore`，junction 不影响 lane 的 git status。review-loop 的 `/tmp` junction 禁令只约束 reviewer
  隔离 worktree，不适用于本 deliberate `refer` junction。
- **禁止 `git clean -fdx` / `git clean -fdX`:** 跨 junction 的强制清理会删除 `main/refer` 的真实数据；
  任何 lane 都不得对包含 junction 的路径执行上述命令。
- 任何额外 worktree 都必须有 owner、用途、固定起点 SHA、允许写入的路径和清理条件；路径、分支或
  detached 状态必须能由 `git worktree list --porcelain` 复现。
- worktree 只有在其改动已提交并完成必要的远端 handoff，或明确作为只读审查快照完成记录后，才算使用
  完毕。使用完毕后，owner 必须先确认 `git -C <path> status --porcelain` 为空，再执行
  `git worktree remove <path>`，随后执行 `git worktree prune` 并重新检查 worktree 列表。
- 不得用 `git worktree remove --force` 掩盖未提交修改、未 push 的 commit 或未记录的 artifact。清理条件
  不满足时保留 worktree，记录 owner、阻塞和下一清理条件，并按 Residual Routing 处理。
- lane 之间不得删除对方拥有的 dirty worktree；stale/失联 worktree 在 Frontier Sync 中确认其状态并路由为
  residual。

# Primary Self-Audit

- Primary Self-Audit 是每个非机械实现 work-unit 的即时交付门禁，由 Delivery App 代表 lane 直接执行，
  不调用 subagent，也不把 Review App 的异步二审冒充为当前 gate 证据。
- 在适用的 `.github/workflows/full-gate.yml` run 对同一 head SHA 成功，或 Rust gate 已按规则明确为
  non-applicable 后，暂停该 head 的写入，固定
  `Issue/PR + head SHA + scope + commands + full-gate evidence + acceptance gate`，
  对照规范、ADR、计划、fixture、调用方、diff 和实际验证 artifact 做 domain-matched 检查。
- Delivery App 必须在关联 Issue 和 PR（若存在）分别追加一条 `## Primary audit result`。消息包含 Target、
  Head SHA、Scope、Commands、Full-gate evidence、Verdict、Findings、Gate impact、Limitations 和 Next；
  它不包含 reviewer-only `Advisories`，并与 Review App 的 `## Audit result` 明确区分。
- `pass` 只表示当前固定快照没有未解决的 Critical/Important finding，适用 gate 已实际通过，且没有越权
  语义选择；通过后 lane 才可请求 Rancemxn Ready/merge 并继续 frontier，不等待 reviewer。
- Rust/build/dependency/test/executable-fixture 或 `.github/workflows/full-gate.yml` 实现变更的
  Full-gate evidence 必须包含 workflow/run URL、run ID、event、精确 `headSha` 和 `success` conclusion。
  纯文档或非构建元数据写 `non-applicable` 及理由；缺失、运行中、失败、SHA 不匹配或 GitHub 不可确认时，
  verdict 只能是 `blocked` 或 `needs-info`。
- 自审发现问题时，必须在当前 lane 修复或路由 residual，追加 superseding Primary audit；不能把未解决
  finding 描述为通过。后续 push、scope、命令或 acceptance 变化会使旧 Primary audit 失效。
- 自审通过后发送 `Review requested`，说明 Review App 是异步二审；合并后若 SHA、scope 或 gate 变化，
  重新固定合并后的目标供复审。Review App 的 Critical/Important 结果按 New finding gate 处理，
  架构/文档建议按 HUMAN-only 路由处理。

# Independent Review Handoff

- 非机械实现 PR 在进入 Ready 或合并前必须完成 Primary Self-Audit；Review App 是异步二审，不再是每个
  work-unit 的前置等待门。Delivery App 在 Primary audit 通过后发送 `Review requested`，固定被审 PR、
  关联 Issue、head SHA、审查 scope、规范/ADR 条款、复现命令、full-gate evidence、已知 residual 和验收
  gate，并继续不依赖 reviewer 即时返回的安全交付。
- Review App 绑定不可漂移的快照：`Issue/PR 或 commit + head SHA + scope + commands + full-gate
  evidence + acceptance gate`。审查者不得把作者的结论、旧测试输出或未固定的工作树当作快照证据。
- 审查期间若 head SHA、scope、验收命令或依赖证据变化，原审查立即失效；必须追加新的
  `Review requested`，Review App 追加 `superseding/re-review` 说明并以新快照重新开始。旧评论和
  finding 不得被编辑或静默覆盖。
- Primary audit 的 Critical/Important finding 未关闭时，不得将主 PR 标记为 Ready 或合并；Review App
  在合并后发现的同等级 finding 冻结受影响的 stage claim 和后续依赖 work-unit，但不要求回滚已合并 PR。
  Minor 只有在不影响当前验收、规范依赖 closure 或阶段 gate，且有明确 owner、目标 Issue 和解除条件时
  才能延期。
- Review App 不得审查或批准自己创建的内容（它不创建 corrective PR）；corrective PR 由父编排者委托给
  独立 corrective lane 创建，仍由主 lane 的 Delivery App 审查其 diff、处理 required checks，Rancemxn
  合并。corrective PR 合并后，主 PR 的新 head SHA 必须重新请求审查。
- Review App 在 FCS5/I10 尚未完成时不得因 `blocked` finding、等待 corrective PR、dirty corrective
  worktree、未确认的远端同步或暂时空 frontier 终止持久目标；按 Persistent idle wait 每分钟 Frontier
  Sync，直到 I10 success signal 与 review frontier 闭合的全部条件同时满足。480 个 review-unit 只限制
  实际审查预算，不限制该等待。

# Review Protocol

一次 review-unit 按以下顺序完成；发现不能只停留在症状描述：

1. **Bind:** 读取固定 Issue/PR/commit、head SHA、diff、规范/ADR/计划/fixture 路由和验收命令；记录
   不在 scope 内的内容。
2. **Reproduce:** 适用时先复用目标同 SHA 的成功 full-gate evidence。只有纯文档或不改变执行逻辑的
   workflow-policy metadata 才可标记 Rust gate non-applicable，其他 workflow 实现变化必须核对适用
   gate，并做不产生构建产物的静态检查。若竞争性假设必须靠执行区分，先创建记录 unknown root
   cause/evidence gap 的 finding，在其独立 branch 提交最小诊断或回归测试，push 后对解析为该 SHA 的
   ref 运行 `workflow_dispatch` full gate，并核对 run `headSha`。预期失败的 run 是 red evidence，
   不是 pass。
3. **Inspect:** 对照规范条款、调用方、测试和固定 artifact 检查实现、边界、错误路径、资源/依赖和
   交付声明；可引用已合并 commit 指出历史漏洞。
4. **Root-cause analysis:** 从可复现症状沿调用链、数据流和规范边界追到第一个被违反的不变量或契约；
   对竞争性假设做最小区分验证，记录因果链、排除依据和仍未知的部分。只描述症状、未经验证的猜测或
   把作者解释当作根因，不能形成 actionable finding。若根因无法确认，必须记录证据缺口并按 Residual
   Routing 路由，不得带着猜测性修复继续。
5. **Classify and route:** 实现/conformance finding 标为 `Critical`、`Important` 或 `Minor`，判断是否阻塞
   当前 stage/PR gate，并创建或更新 finding Issue；严重度必须由影响、复现结果和根因证据支持。明确该
   finding 是当前 stage 可安全修复的代码/测试问题、later-stage 问题，还是规范/治理决策问题。
6. **Corrective delivery:** 对根因已确认、属于当前 stage 且可安全收敛的 implementation/conformance
   `Critical` 或 `Important` finding，父编排者委托独立 corrective lane 实现最小修复并补充回归覆盖；
   corrective lane 使用 `/tmp` 下独立 worktree 与 `codex/<finding>-<slug>` 分支，审查静态 diff 后
   commit、push 并创建链接 finding Issue 的 draft corrective PR；编译、测试、fuzz 和可执行 fixture
   只由该 PR 的 GitHub full gate 执行。PR 和 Audit result 必须写明根因、修复边界、red/green run、
   base/head SHA；验证失败、暴露新根因或无法安全收敛时，保留证据并把 finding 路由为有界 residual，
   不得伪造 pass。corrective lane 不得批准、Ready 或合并自己创建的 corrective PR；主 lane 审查、
   Rancemxn 合并后，新的 SHA 必须送回 Review App 重新审查。
7. **Advisory pass:** 只有实现/conformance verdict 为 `pass` 时，检查架构 seam、模块边界、局部性、
   可测试性、AI 可导航性，以及 docs/CONTEXT、计划、矩阵、loop 和链接的一致性。该 pass 只产生
   advisory，不改变规范状态、stage baseline 或当前 acceptance。存在尚未合并或尚未 re-review 的
   corrective PR 时，当前目标保持 `blocked` 或 `needs-info`，不能提前产生 pass advisory。
8. **Cleanup and comment immediately:** 若不再需要本地写入，先按 Worktree Cleanup 安全清理；若必须
   保留，在 `Audit result` 中记录 owner、固定 SHA 和清理条件。随后立即在被审 PR（若存在）和关联
   Issue 各追加一条 Audit result；即使没有 finding 也必须发送，并列出 Root cause、Corrective action、
   Corrective PR、Regression evidence、`Advisories: none` 或 HUMAN-only Issue 列表。评论 append-only，
   不手写日期，不反复 edit 同一消息；清理未完成且没有 owner/condition 时，review-unit 不得算作完成。

# Finding Contract & Routing

每个 finding Issue 的初始正文至少包含：

- 被审 Issue/PR/commit 与发现时的 head SHA；
- 文件、符号或稳定位置；
- 违反的规范条款、ADR/计划 gate 或交付约束；
- 最小复现命令和实际输出/artifact；
- 已确认的根因、从症状到违反边界的因果链、支持证据和已排除的竞争性假设；若尚未确认，必须明确证据缺口和路由；
- 影响、严重度、当前 gate 是否阻塞；
- owner、目标 stage、依赖、修复边界、回归验收条件和预期 corrective PR；
- corrective PR URL，或在 PR 尚未创建时标明 pending 状态及下一有界动作。

路由规则：

- 当前被审 Issue 的本地 finding 默认作为其 child/parent 关系下的 finding Issue；跨阶段或 root-level
  问题才直接挂 root Issue #9。不要把 later-stage finding 伪装成当前 stage 的缺陷关闭条件。
- 当前 stage 的 `Critical`/`Important` finding 阻塞 frontier 和主 PR Ready/merge；若根因已确认且安全可修复，
  按 Review Protocol 在独立 corrective lane 交付 corrective PR。`Minor` 只有在有 owner、follow-up Issue、
  目标 stage 和解除条件，并且不影响当前验收时才能延期。
- 修复 PR 使用 `Closes #<finding>`；同时以 `Refs #<reviewed-issue-or-pr>` 连接被审目标。修复合并后，
  在 finding Issue 和原 PR 分别追加新的 checkpoint，再提交新 SHA 进行 re-review。
- 历史已合并 PR/commit 的 finding 不重新打开或修改原 PR；从最新 `origin/main` 创建 corrective branch，
  目标为 `main`，并保留发现 SHA。
- 开放 PR 的 finding 从被审 PR 的固定 head SHA 创建 corrective branch，目标为该活动 PR 的分支。主 lane
  在审查期间不推进活动分支；修复 PR 合并后活动 PR 获得新 head SHA，旧 Audit result 失效，必须重新审查。
- 任何 finding 都不得在只保留症状或猜测性根因的状态下标记为已交付；根因未确认、修复边界不安全或验证
  无法收敛时，必须保留 finding 并按 Residual Routing 记录证据缺口、owner 和解除条件。

## HUMAN-only advisory contract

架构和文档 advisory Issue 必须绑定被审目标、head SHA、scope、观察到的证据、建议、影响、人工 owner 和
建议处理条件，并使用 `ready-for-human` 状态及合适的 `documentation`、`workflow` 或 `enhancement` 标签。
它们不使用 `review-finding` 或 severity label，不能关闭当前实现 Issue，不能阻塞 I10，也不能被本工作流
自动选为 work-unit。只有当证据升级为规范矛盾、实现缺陷或当前 conformance 违约时，才转回标准 finding
contract。

# Reviewer Metadata Duties

Review App 可以管理自己创建的 finding Issue 的既有元数据，但不能借此改变项目的全局治理：

- 创建 finding 时使用已有的 `review-finding` label，并根据证据添加至多一个 `severity:critical`、
  `severity:important` 或 `severity:minor`；需要时可以添加已有的 `specification` 或 `conformance`
  等正交 label。一个 open finding 仍必须保持恰好一个 workflow-state label。
- 当 finding 已能明确归属阶段时，可以给该 finding 分配已有 milestone；不明确时保留未设置状态，并在
  Issue/comment 中写出 milestone 建议及解除条件。
- 可以用新的英文 comment 或 finding Issue 提议新增/调整 label 或 milestone，但不得直接创建、重命名、删除
  或改变全局 label/milestone 定义，也不得修改被审主 Issue 的 workflow label 或 milestone。
- 这些 metadata 只用于路由和审查证据，不能提升规范状态、改变 stage baseline、替代 owner/依赖关系，或把
  `Minor` finding 自动变成当前 gate 的阻塞项。

# Corrective Lane & Worktree Isolation

- 创建 corrective PR 必须使用 `/tmp` 下的独立 worktree 和独立分支；推荐路径为
  `/tmp/fcs-finding-<finding>-<slug>`，分支命名为 `codex/<finding>-<slug>`。单独分支不等于工作树隔离，
  两者都必须满足。corrective worktree 不得放在主仓库、主仓库旁、用户 home 或其他任意路径。
- 本工作流中的 `/tmp` 指宿主的系统临时目录：POSIX 宿主即 `/tmp`；Windows 宿主为用户 profile 下的
  默认系统临时目录 `C:\Users\<user>\AppData\Local\Temp`，推荐路径按该目录等价展开
  （如 `<系统临时目录>\fcs-finding-<finding>-<slug>`）。Windows 系统临时目录位于用户 profile 之内不违反
  「不得放在用户 home」；该禁令继续覆盖 home 下系统临时目录以外的任意路径，以及主仓库、主仓库旁和
  其他任意路径。
- 路径验证以该目录为准：`git worktree list --porcelain` 输出的绝对路径位于系统临时目录下即满足
  「位于 `/tmp/`」。比较前先归一化两侧：统一路径分隔符、按平台规则统一大小写、解析 8.3 短名和
  drive substitution，再判断包含关系。锚点是上述默认系统临时目录本身，不是 `TMP`/`TEMP` 环境变量的
  当前取值；不得用符号链接、目录 junction 或其他 reparse point、`subst` 映射，或重定义 `TMP`/`TEMP`
  绕过该约束。Audit result 的 `Worktree` 字段记录保留 worktree 时必须写归一化后的绝对路径，使主 lane
  能独立复核隔离是否成立。
- 此禁令只约束 corrective/review 隔离 worktree。lane 的 deliberate `refer` junction（Lane Lifecycle）
  是显式授权例外，两者不冲突。
- 对需修复的当前-stage implementation/conformance finding，corrective lane 是该 worktree 的执行 owner：
  从固定 base/head SHA 建立 worktree 后，只在其中修改代码和测试，保留主 lane dirty worktree、活动实现
  分支和 `main` 不变。修复必须最小化地针对已确认根因，并包含能失败于旧行为、通过于新行为的回归证据。
- corrective worktree 不运行任何编译、测试、fuzz 或会生成 Cargo build artifact 的命令。根因区分需要
  执行时，先推送 finding branch 的诊断 SHA，以 `workflow_dispatch` full gate 取得 red evidence；只有根因
  确认后才能创建 corrective PR，随后以该 PR 新 SHA 的成功 run 取得 green evidence。两个 run 都必须记录
  URL/ID、event、精确 `headSha` 和 conclusion。
- 纯只读审查也必须使用 `/tmp` 下的快照 worktree，推荐路径为 `/tmp/fcs-review-<target>`；可使用 detached
  HEAD，不创建实现分支。主 lane worktree 不受本条路径约束。
- 创建前记录 worktree owner、用途、固定 base/head SHA、分支或 detached 状态和预计清理条件；使用
  `git worktree list --porcelain` 验证路径确实位于 `/tmp/`，不得用符号链接绕过该约束。每个 target 使用
  唯一路径；若路径已存在，先核对 owner 和状态，不得复用或覆盖 dirty worktree。
- 开放 PR 的 corrective branch 起点是固定 head SHA，PR base 是被审 PR 的活动分支；历史 commit 的
  corrective branch 起点是最新 `origin/main`，PR base 是 `main`。

# Worktree Cleanup

- 只读审查在没有未记录 artifact 且不再需要本地写入后结束其 `/tmp/fcs-review-*` worktree；安全条件满足时
  应在最终 `Audit result` 前清理，以便消息记录 `Worktree: cleaned`。
- corrective worktree 在分支已 push、PR 已建立且没有待提交修改后，若不再需要本地修改，可以
  暂时清理；若仍可能需要修复，则保留到 PR 合并/关闭或明确放弃，并记录 owner、原因和下一条件。PR 合并
  后的最终 re-review handoff 完成时必须清理剩余 worktree。
- 清理必须由该 worktree 的 owner 执行：先确认
  `git -C <path> status --porcelain` 为空，再执行 `git worktree remove <path>`、`git worktree prune`，
  最后用 `git worktree list --porcelain` 确认路径已消失。不得使用 `--force` 删除 dirty worktree，也不得
  删除未 push 的 commit 或未记录 artifact。lane 清理必须确认未跨越 `refer` junction，且禁止
  `git clean -fdx`/`-fdX`。
- 清理失败或 worktree 变脏时，保留现场并追加 residual：包含 path、owner、固定 SHA、阻塞原因和下一清理
  条件；不得把交付标记为完全完成。其他 lane 不得代替 owner 删除其 dirty worktree。

# GitHub Comment Markdown Contract

所有发往 GitHub Issue/PR 的 progress、audit、handoff 或 delivery comment 都是一个原生 Markdown
文档，不是 shell 片段、JSON 字符串或终端输出预览。所有角色（Delivery App、Review App、lane、父编排者）
必须同时遵守以下不变量：

- payload 使用真实的 LF 换行；不得把 JSON 转义后的换行、字面量的反斜杠-n 或一整段单行字符串当作 Markdown
  正文发送。
- Markdown 中的反引号、美元符号、反斜杠、尖括号、竖线和列表标记必须按正文字符保留；不得让 shell command
  substitution、未引用的字符串插值或 HTML 转义改写它们。
- 原始正文不得拼接进 shell command string 或未经保护的 JSON/双引号参数。写入边界必须使用能保留原始正文、真实
  换行和所有 Markdown 标点的 body file、stdin 或等价的安全 API 参数；本契约约束 payload，不限定具体工具。
- 文档中的 fenced template 只说明评论内容，外层 fence 不属于待发送正文；发送前必须保留模板内部的标题、空行、
  列表和 code span。

## Shape and read-back gate

- 事件标题使用一个 H2，并与 `Primary audit result`、`Review requested`、`Audit result`、
  `Delivery-ready Progress` 或 `Superseding ...` 等固定事件名称一致；标题不手写日期。
- 标题与正文、各段落与列表、列表与表格或 fenced block 之间保留空行；每个 top-level list item 从新行开始，
  不把多个字段折叠成一个段落。
- 写入前保留准备发送的完整正文和稳定身份（target、event、Issue/PR、head SHA）；重试同一远端动作时不得
  生成第二种序列化版本。
- 写入后必须按返回的 comment URL/ID 重新读取正文，并在只允许 CRLF-to-LF 归一化的前提下与准备正文比较。
  未通过比对、远端写入尚未确认或出现字面转义换行时，不得记录为成功。
- 发现格式错误时不得编辑或删除历史 comment；立即追加 `## Superseding ...`，指出被替代 comment、原因、固定
  target/head SHA、修正后的字段和 Next。修正 comment 本身也必须经过同一 read-back gate。

## Markdown validation

在提交或修改本节的 comment template 后，必须运行以下仓库级 Markdown 检查，并要求 exit 0：

~~~sh
tmpdir="$(mktemp -d)"
printf '{ "MD013": false, "MD025": false, "MD060": false }' > "$tmpdir/.markdownlint.jsonc"
markdownlint-cli2 --config "$tmpdir/.markdownlint.jsonc" docs/loops/fcs5-parallel-pr-delivery.md
rm -r "$tmpdir"
~~~

本仓库使用全局 `markdownlint-cli2`（未安装 `markdownlint` CLI）；cli2 没有 `--disable` 参数，因此 MD013、MD025 和
MD060 通过写入临时 `.markdownlint.jsonc` 配置文件豁免，该文件不得进入仓库提交。这三个例外是本仓库这份契约的
显式排版例外：中文长行不强制硬折行；每个独立契约章节保留 H1；现有表格保留 compact pipe 风格。除这三条外不得
禁用规则；markdownlint 通过也不能替代 GitHub comment payload 的 read-back gate。

~~~md
## Primary audit result

- Target: PR #<n> / Issue #<n>
- Head SHA: <sha>
- Scope: <fixed scope>
- Commands: <command> -> <passed/failed/skipped and actual result>
- Full-gate evidence: <workflow/run URL + run ID + event + exact head SHA + conclusion, or non-applicable with reason>
- Verdict: pass / blocked / needs-info
- Findings: none / <finding list with severity>
- Gate impact: <current gate impact>
- Limitations: <none or uncovered scope>
- Next: <one bounded next action>
~~~

~~~md
## Audit result

- Target: PR #<n> / Issue #<n> / commit <sha>
- Head SHA: `<sha>`
- Scope: <固定范围>
- Commands: `<command>` → <passed/failed/skipped>（列出实际结果）
- Full-gate evidence: <workflow/run URL + run ID + event + exact head SHA + conclusion, or non-applicable with reason>
- Root cause: <已确认的因果链与证据，或明确 unknown/evidence gap 及路由>
- Corrective action: <隔离 worktree 中的修复范围、commit/push 状态；或 not applicable 及原因>
- Corrective PR: <#<n>/URL，或 pending residual/none>
- Regression evidence: <新增或复用的回归测试、实际输出/artifact，或 none 及原因>
- Verdict: `pass` / `blocked` / `needs-info`
- Findings: <none 或 #finding 列表，含 severity>
- Advisories: <none 或 HUMAN-only Issue 列表；不改变当前 gate>
- Gate impact: <当前 stage/PR gate 是否阻塞>
- Limitations: <未覆盖范围或 none>
- Worktree: <cleaned，或 retained + owner/condition>
- Next: <主 lane 或 finding owner 的下一有界动作>
~~~

若快照失效，追加 `## Superseding audit`，明确被替代的 head SHA、旧 verdict、新请求原因和新审查目标；
不要修改旧消息。评论标题不手写 `YYYY-MM-DD` 等日期，GitHub timestamp 是时间记录。

# Authorized Change & Delivery

- 可以自动进行仓库内设计、实现、测试、fixture、计划、review 和治理修改，以及正常的 GitHub
  Issue/branch/push/PR 生命周期；所有远端进度与网络行为遵守 Retry & Outbox 与 ADR 0011（经 ADR 0014
  修订），本文件不复制其操作规则。
- 每个 branch/PR 只交付一个可审查 work unit。提交和 push 前审查作用域与 diff；不 amend 用户提交，
  不 rebase/reset/checkout 丢弃工作，不清理无关 dirty changes。
- 普通 merge 已获持续授权，但只有 child Issue acceptance criteria、适用验证和 Primary audit `pass` 全部满足、
  PR 为 Ready 且 mergeable、required checks 与 review requirements 满足、没有未解决 review thread，并已记录
  delivery-ready 证据，且由 Rancemxn 执行时才可合并。Review App 的待审状态不阻塞本次 merge；任何已到达的
  失效/阻塞 verdict 仍按 New finding gate 处理。不得使用 `--admin`、force-push、降低 gate 或隐藏 finding。
- Rancemxn 是唯一 merge owner、唯一 `gh pr ready` 执行者、唯一可以 push `main` 的角色。Delivery App 与
  Review App 都不得 Ready/merge/push `main`、关闭主 Issue 或修改主 Issue workflow label；corrective PR
  必须链接 finding Issue，且最终由 Rancemxn 合并。
- stage 的客观 gate 满足后自动进入下一 frontier；不为已经由规范、ADR、fixture 和证据唯一决定的
  普通实现选择反复请求确认。
- 规范/依赖/API 工作遵守根 `AGENTS.md` 的固定依赖源码和 `tavily_hikari` 路由。添加依赖必须记录版本、
  feature、MSRV、license、dependency tree 和激活范围。

## Retry & Outbox

- 远端操作（经 App token 或 `gh`）因 DNS、连接超时/重置、TLS 中断或 HTTP 502/503/504 等瞬时网络问题
  失败时，每隔 5 秒重试同一操作，首次失败后最多再试 10 次。写操作在每次重试以及稍后补同步前，必须先按
  稳定身份查询远程是否已生效，避免重复创建 Issue/PR、重复评论、review 或 merge。不得重试认证/权限失败、
  参数/校验错误、not found、合并冲突或门禁失败；应立即报告。
- 10 次重试耗尽后，记录完整待同步 payload、稳定身份、最后错误和 `pending remote sync` 状态，继续不依赖
  该远端结果的安全本地工作；在下一个有意义检查点以及 handoff、PR Ready、review 或 merge 等依赖远端状态
  的动作前再次查询并尝试同步。待同步记录只是 transport outbox，不是第二个 tracker；不得把未确认的远端
  动作描述为成功。

# Approval Gates

Routine GitHub delivery 和满足 Authorized Change & Delivery 条件的普通 merge 已获授权。以下动作仍须
单独取得 Rancemxn 的明确批准：

| Gate | Trigger | If approved | If denied |
|---|---|---|---|
| Public release | 创建公开 tag、GitHub Release、发布 crate、上传发行物或公开 conformance bundle | 只按批准范围发布并执行发布后校验 | 保留已合并的本地 RC，不把它描述为已公开发布 |
| Destructive history/data operation | 删除或重写已有 Git 历史、branch、archive、用户数据或外部数据 | 仅对明确目标执行，并先验证作用域 | 不执行；采用非破坏替代或保留 residual |
| Credential/system mutation | 使用签名密钥、付费服务、修改远端保护/配置、安装系统级软件/驱动或改机器全局配置 | 在最小权限和明确作用域内执行 | 继续所有不依赖该能力的工作，必要时路由 HUMAN |
| Copyright/license distribution | 把许可证或版权状态不明确的谱面、音频、图片、字体等纳入公开分发 | 仅分发获批且有证据记录的材料 | 只保留本地 opt-in fixture lane，不进入公开 artifact |

Review App 在既定 workflow 授权内可以读取、comment、request changes 和创建 finding Issue；merge、
`gh pr ready`、关闭主 Issue、修改主 Issue workflow label、修改活动实现分支、force-push、降低 required
gate、公开发布和任何 destructive history/data operation 都不是 Review App 或 Delivery App 的权限。

# Measurement Domain

**本地禁止一切编译、lint、测试、fuzz 与可执行 fixture 运行**（`cargo check`/`clippy`/`build`/`nextest`/
`cargo fuzz` 等全部禁止）。本地只允许：

- `cargo fmt --all -- --check`（格式检查，不编译）；
- 非编译静态检查：diff 检查、链接审计、Markdown/YAML/JSON/schema 校验、结构/路径审计。

本地结果不产生门禁证据：不得写成通过，不得替代任何 full-gate step，不得进入 Primary audit 或 Review
App `Audit result` 的 full-gate evidence。测试、fuzz 和可执行 fixture 证据只来自 GitHub Actions：对
明确提交的**候选 SHA** 通过 `workflow_dispatch` 运行 `.github/workflows/full-gate.yml`，并核对 run 的
`headSha` 与目标 SHA 完全一致。每周 `weekly-fuzz.yml` 保持 schedule 触发且明确 non-gate，不参与候选
SHA 门禁，也不被本规则改变。成功且同 SHA 的 Action run 是 Ready/merge 前的强制前置条件；Action success
不构成 merge 授权。

| Output domain | Verification method | Required artifact |
|---|---|---|
| 规范与治理文档 | 条款/术语/版本/交叉引用审计；example/conformance 映射；独立复审；状态转换条件复核 | 权威文件 diff、链接审计、finding ledger、状态/hash 记录 |
| GitHub delivery evidence | 核对 root/child 依赖、Issue acceptance、PR diff/merge state、review thread 与同 SHA Action gate | linked Issue/PR、merge SHA、run URL/ID/event/headSha/conclusion 和 residual owner；不获得规范权威 |
| Source grammar 与 AST | 每个 production 的 valid/invalid coverage；精确 span/diagnostic；完整消费；limit/property/fuzz | production ledger、fixture 执行结果、bounded fuzz/property 报告 |
| Static/elaboration/canonical | 类型、名称、展开、稳定 ID、canonical invariant、source-reorder 等价和 later-stage fixture 执行 | canonical snapshot、invariant traversal、诊断与限额结果 |
| Runtime 与数值 ABI | reference evaluator 对 typed DAG、lazy semantics、seek、Track、Distance 和困难 binary64 vector 求值 | 输入向量、expected bits/trace、reference 与产品 evaluator 对比 |
| FCBC/Execution ABI | reference writer→static bytes→独立 loader→evaluator；CRC/SHA、section/record/reference、profile、mutation | 非空 golden、声明式 manifest、mutation corpus、load/evaluation 报告 |
| Conversion | 真实固定来源 PGR v1/v3、RPE、PEC 经 exact ProfileBinding 完成 parse→canonical→target→同 profile reparse；验证 capability/error budget | source/package fixture、canonical golden、resource bundle、ConversionReport/Fidelity bytes、round-trip 报告 |
| Render | RenderSection codec、resource decode/shaping、semantic draw list 和 reference raster 容差比较 | 非空 RenderSection golden、固定 image/font、semantic snapshot、raster/diff |
| CLI 与发行组合 | 命令、profile/resource/capability/budget 参数、exit category、JSON/text diagnostic 和端到端组合 | command transcript、expected output/exit、package/tree/version 审计 |
| Rust workspace | 对候选 SHA 运行 ADR 0013 的完整 GitHub gate（`workflow_dispatch`，核对 run `headSha`）；不在本地运行测试或 fuzz | workflow/run URL、run ID、event、精确 head SHA、conclusion、step 结果和跳过原因 |
| Repository/conformance integrity | file/suite/tree hash 独立复算；UTF-8/NUL/链接；archive/main/workspace/refer 边界 | hash ledger、路径计数、`git status`、结构与链接审计 |

## 候选 SHA Full Gate

- `.github/workflows/full-gate.yml` 只响应 `workflow_dispatch`，不再有自动 `pull_request` 或 `push main`
  触发器。运行方式：对解析为目标 SHA 的 branch/tag ref 人工 dispatch，然后回读并确认 run 的 `headSha`
  与目标 SHA 完全一致；SHA 不匹配的 run 不是该 SHA 的证据。
- workflow 保持 ADR 0013 的完整命令序列：locked dependency（root + fuzz）、`cargo metadata --locked
  --offline`、`cargo tree`、`cargo fmt --all -- --check`、Clippy `-D warnings`、nextest、bounded fuzz
  smoke（`FCS_FUZZ_RUNS=1024`）、`git diff --check` 与 clean-worktree gate，以及 SHA-verify step。
- 成功且同 SHA 的候选 SHA run 是 Ready/merge 的强制前置条件。`queued`/`in_progress`、缺失、失败或
  SHA 不匹配都不能写成通过；GitHub 暂时不可用时只能继续不依赖远端结果的静态工作，不得 Ready 或 merge。
- `Swatinem/rust-cache` 的 hit/miss 只影响性能，不改变命令、结论或验收。瞬时基础设施失败可以在同一
  SHA 重跑；代码、测试或配置失败必须修正后推送新 SHA。新 SHA 导致旧 run 被取消时，旧 run 只是过期
  证据，不是当前 SHA 的 gate 失败。
- 只修改 Markdown、AGENTS、Issue/PR 模板、评论、label 或其他不参与构建且不改变 gate 执行逻辑的元数据时，
  Rust full gate 为 non-applicable；使用 diff、链接、Markdown/YAML/JSON/schema 和相关 CLI smoke check。
  `.github/workflows/full-gate.yml` 的实现变化属于适用 gate，必须由候选 SHA 的成功 run 证明。
- 交付说明必须分别列出本地静态检查（含结果）和远端 full-gate evidence，以及未运行门禁及原因。不得将
  `queued`、缺失、失败或 non-applicable 写成通过。

# Tool Boundaries

- 本地文件读取、搜索、替换与定位使用 FastCtx MCP 工具（`read`、`grep`、`glob`、`replace`），传入绝对路径；
  它们按文件实际编码读取并保留行号。只有 FastCtx 报告编码不明确时才按其候选显式传入 `encoding`。
- FastCtx `run` 只用于真正需要进程的场合：原生 Windows Git、Delivery/Review App broker、Markdown/YAML/
  JSON/链接/静态校验器、`cargo fmt`。**通用 Bash 是例外且默认禁止**：不使用 shell 命令读取/搜索文件、
  不使用 `cat`/`head`/`tail`/`grep`/`find`/`ls`/`type`/`Get-Content` 做文件系统检查，不用重定向读写文件。
  需要显式命名批准才能豁免。
- 远端 GitHub 状态经 Delivery/Review App（token 身份）查询与写入；不在本地用 `gh` 代替 App 身份。

# Residual Routing

| Residual / failure | Route: LOCAL / PLANNER / HUMAN | Action |
|---|---|---|
| GitHub full gate 的 test、Clippy、fmt、hash、link、manifest、golden、round-trip 或 raster 不一致 | LOCAL | 从 Action log 找到最先失败的原因，修复后推送新 SHA；不得以本地编译或测试替代远端 gate |
| 适用 full gate 缺失、运行中、SHA 不匹配或 GitHub 暂时不可用 | LOCAL/WAIT | 保持 Primary audit 为 `blocked`/`needs-info`，继续可分离的本地静态工作并按 Retry & Outbox 恢复；不得 Ready/merge |
| Action cache miss，但同 SHA full gate 成功 | LOCAL | 记录为性能信息，不改变 gate；只有反复异常 miss 影响完成时才建立 workflow residual |
| Frontier Sync 发现新的当前 stage Critical/Important finding 或未关闭 corrective PR | LOCAL | 冻结受影响 work-unit 的提交、push、Ready 和 merge；处理 finding、合并修复并在新 SHA 上重新审查 |
| Review App 发现 Critical/Important 缺陷 | LOCAL | 建立/链接 finding Issue，父编排者委托 corrective lane 修复，Rancemxn 合并 corrective PR，并对新 head SHA 重新请求审查 |
| Review App 发现 later-stage 或不影响当前 gate 的 Minor | PLANNER | 记录 owner、目标 stage、依赖、验收条件和 follow-up Issue；不得伪装为当前阶段完成 |
| 审查快照的 SHA、scope、命令或验收变化 | LOCAL | 追加 superseding/re-review 记录，废弃旧 verdict，固定新快照后重新审查 |
| 根因已确认、属于当前 stage 且安全可修复的 Critical/Important finding | LOCAL → 主 lane | corrective lane 在 `/tmp` 隔离 worktree 中实施最小修复、补回归测试、commit/push 并创建 linked corrective PR；由同 SHA GitHub full gate 验证，阻塞主 PR 并等待合并后 re-review |
| 只有症状或竞争性假设，根因仍未确认 | PLANNER/HUMAN | 记录已执行的区分验证、证据缺口、owner 和解除条件；不得提交猜测性修复或报告为 actionable pass |
| 根因已确认但修复需要规范/ADR/semantic-profile 选择 | PLANNER/HUMAN | 保留根因证据和双方影响，按治理流程路由；不得用偏好替代规范决定 |
| diagnostic/corrective SHA 的 full gate 失败或暴露新的根因 | LOCAL/HUMAN | 保留 worktree 和 Action evidence，修正后推送新 SHA 或更新 finding/owner/解除条件；不得以本地 Cargo 或虚假 pass 收敛 |
| 规范缺口且权威规范、Accepted ADR 和固定证据能唯一决定结果 | LOCAL | 按治理流程更新规范、fixture、manifest、review 与状态记录；重建受影响 baseline，I10/发布再完成 Frozen gate |
| 实现与规范冲突且证据表明是实现缺陷 | LOCAL | 修实现和回归证据，不让实现反向定义规范 |
| active unit 过大、验收耦合、顺序错误或 measurement domain 不匹配 | PLANNER | 保留原验收覆盖，拆成严格更小的 bounded Issues，或调整顺序/测量 |
| 两次不同技术路径仍未减少验收项或 decision residual | PLANNER | 建立最小复现并重新规划；第三次仍无决定性证据则退出该 Issue |
| 当前 stage dependency Issue 未关闭 | PLANNER | 阻塞受影响 gate，继续可分离工作；不得把挂起当作完成 |
| 临时 worktree 脏、路径不明、owner 消失或清理条件未满足 | LOCAL/HUMAN | 保留 worktree 和未提交证据，指定 owner/下一条件；不得强制删除或把清理失败描述为完成 |
| reviewer worktree 不在 `/tmp`、变脏、owner/固定 SHA 缺失或无法安全清理 | LOCAL/HUMAN | 停止交付，保留现场并记录清理条件；不得使用 `--force` 或越权删除 |
| lane 或 `git clean -fdx/-fdX` 跨越 `refer` junction | HUMAN | 立即停止，核对 `main/refer` 完整性与 owner；按 Approval Gate 处理 destructive operation |
| finding 经证据证明属于 later stage | PLANNER | 记录 owner、目标 stage、依赖与验收方法后延期，并在 owning gate 前重新进入 frontier |
| 两个以上合法设计产生 materially different 公开语义，规范/ADR/证据无法排序 | HUMAN | 提供证据、选项、影响与推荐；停止依赖该选择的实现，继续可分离工作 |
| 需要推翻 Accepted ADR 或用户已确认的产品边界 | HUMAN | 停止受影响范围，提出新 ADR 候选和迁移影响 |
| 第三次尝试仍无决定性证据，或外部输入/能力缺失 | HUMAN | 标记 `needs-info` 或 `ready-for-human`，记录最小所需输入并退出受影响路径 |
| 不可逆动作、凭据、系统配置或版权/许可证分发 | HUMAN | 触发 Approval Gate；拒绝时保留本地安全状态 |
| 连续 3 次满足全局 no-progress 且无 ready frontier | LOCAL/WAIT | 记录 `waiting-for-main` 并每分钟 Frontier Sync；Review App 持久目标不得终止或标记 `blocked`，直至 I10 success signal 与 review frontier 闭合 |
| 达到 240 次 work-unit 上限 | PLANNER | 终止本轮，保留合并证据并产出仍指向 I10 的新命名后继工作流建议 |
| 达到 480 review-unit 上限 | PLANNER | 停止当前预算内的新 review-unit 分配，保留 finding ledger 和 HUMAN-only advisory，产出后继审查 handoff；不得把预算耗尽描述为空 frontier 的 `blocked` |
| Primary audit 发现当前 work-unit 的 Critical/Important finding | LOCAL | 停止 Ready/merge，修复或建立 finding Issue，追加 superseding Primary audit 后再交付 |
| 架构优化、文档改善或一般建议 | HUMAN | 创建 `ready-for-human` 的 HUMAN-only Issue；不由本工作流自动处理，也不阻塞 I10 |
| 审查者角色不独立、被审目标仍在写入或工作树隔离失败 | HUMAN | 停止该 iteration，报告冲突和恢复条件 |
| GitHub 瞬时网络失败 | LOCAL | 按 Retry & Outbox 查询稳定身份并重试；耗尽则保存 payload/outbox，继续安全只读工作 |

# Subagent and Session Policy

- 不创建第三个可选实现会话。每个 lane 只有一个 writer（`worker`），它是该 lane 的唯一实现者；
  Primary Self-Audit 由 Delivery App 完成，不调用 subagent。内部 subagent 不是独立交付角色，最多用于
  只读研究或父编排者明确授权的有界本地草稿。它们不得自行切换 branch、commit、push、创建/修改
  Issue/PR、review、Ready 或 merge；父编排者统一审查共享工作区、验证和交付。
- 父编排者为每个 lane 启动独立的 Pi 实例/subagent，动态调度互不冲突的 lane，综合 Review App 结果，
  排序 merge 请求，并把冲突委托给独立 corrective lane。全部并行子任务总数不超过三个。
- 思考策略：默认 `max`；只有简单有界任务才允许 `high`；只有纯机械任务才允许 `off`。
- 工具边界：按 `.pi/settings.json` 的 allowlist 执行；reviewer/scout 只读，worker writer，
  researcher 保留 Tavily。任何 agent 都不得获得 Ready/merge/push `main` 能力。
- 任务结束时按 Measurement Domain 执行静态检查；若 skill 自带的验证或写作流程与仓库命令、目录职责或
  提交范围冲突，以本文件为准，并在交付说明中标明未执行的步骤及原因。
