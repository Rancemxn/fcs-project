# Goal & Success Signal

- **Goal:** 从 GitHub root Issue、child Issue/PR 图和已合并提交中选择最早尚未审查的固定目标，复现其
  规范、阶段 gate、测试和交付证据，先完成实现/conformance 二审，再对通过目标做有界的架构与文档
  advisory audit；及时记录可复现 finding，定位其根因；对当前 stage 且安全可修复的实现/conformance
  finding，由 reviewer 在隔离 worktree 中完成最小修复、回归验证并提交 linked corrective PR，再把新 SHA
  路由回主会话，直到当前审查 frontier 闭合。
- **Observable success signal:** 每个已审查目标都有一条 append-only `Audit result`（即使零 finding），
  其中包含目标身份、head SHA、scope、实际命令、verdict、限制、advisory 结果和 Next；所有
  Critical/Important implementation/conformance finding 都有根因证据、owner、修复路径和最新状态；安全
  可修复的 finding 有已提交的 corrective PR 和回归证据，不能安全修复的 finding 有明确的 PLANNER/HUMAN
  residual；必要的 corrective PR 已链接 finding Issue，且主会话已经重新审查修复后的新 SHA；架构/文档建议
  均已创建 HUMAN-only Issue，不进入主 loop 的 acceptance ledger；当前 frontier 没有未审查目标，也没有
  未分配的 Critical/Important finding。
- 所有已完成审查的 reviewer worktree 都已从 `/tmp` 安全清理；仍保留的 worktree 都有 owner、固定 SHA、
  未完成原因和明确的清理条件。
- 审查 loop 不声明阶段、规范版本域或 FCS 5 RC 完成；它只产生独立审查证据和 finding 路由。最终完成
  仍由主 `docs/loops/loop.md` 根据规范、fixture、baseline、gate 和合并证据判定。

# Scope & Authority

- `docs/specifications/fcs.md`、`docs/specifications/fcbc.md`、`docs/specifications/fcs-render.md`、
  `docs/specifications/fcs-conversion.md` 和
  `docs/specifications/governance.md` 是规范与状态权威；Accepted ADR 是架构/治理约束；计划、Issue、PR、
  实现、测试和本 loop 只能安排或证明工作，不能创造规范语义。
- 每次审查必须绑定一个不可漂移的快照：`Issue/PR 或 commit + head SHA + scope + commands + acceptance
  gate`。审查者不得把作者的结论、旧测试输出或未固定的工作树当作快照证据。
- 审查会话是独立于主实现会话的第二个角色。主会话保留主实现分支的 owner 身份和唯一 merge owner 权限；
  审查会话可以读取、评论、review/request changes、创建 finding Issue，并在自己拥有的 `/tmp` 隔离
  worktree 中实现、验证、提交、push 和创建 corrective PR，但不得写入主会话的工作树、活动实现分支或
  `main`，也不得合并任何 PR、将 PR 标记为 Ready、关闭主 Issue 或修改主 Issue 的 workflow label。
- 审查会话不能以审查结果修改规范状态、冻结版本域、重写 Accepted ADR 或替代 conformance artifact。
  发现规范缺口时只记录证据并按规范治理路由。

# Termination Conditions

- **Max iterations / budget:** 最多 480 个 `review-unit` iterations，与主 loop 的 240 个 work-unit 预算独立
  计算。一次 iteration 只审查一个固定目标及其通过后的架构/文档 advisory pass，不按命令数、评论数或 commit
  数计数，也不得通过拆分同一快照绕过预算。等待 frontier 的时间不消耗 review-unit 预算。
- **Goal-achievement check:** frontier 中每个已分配目标都有最新 Audit result；没有未分配的 Critical/
  Important finding；需要修复的 finding 都已链接 owner、目标 stage、依赖、根因证据、验收条件和 corrective
  PR 或明确的 HUMAN/PLANNER residual；架构/文档 advisory 都已标为 HUMAN-only 并从主 loop ledger 排除。仍在
  写入中的目标不算未完成审查目标，必须等待其固定快照。
- **Terminal completion:** reviewer 只有在 root Issue 的 I10 success signal 已满足，并且 Frontier Sync 同时确认
  没有新的固定 review target、未分配的 Critical/Important finding、待复审的 corrective PR/merged SHA，或
  reviewer 自己保留的未清理 worktree 时，才可以终止并报告审查 frontier 闭合。I10 未完成时，空的 review
  frontier 不是成功、失败或 blocker；任何 `blocked` finding、等待主会话的 corrective PR、dirty corrective
  worktree、未确认的远端同步或旧状态都只能保持 reviewer 持久目标运行并进入轮询。
- **Persistent idle wait:** 每次完成一个目标后先检查 root Issue 的 I10 success signal。若 I10 未完成且没有
  新的固定、可审目标，则等待 1 分钟后重新 Frontier Sync，并持续重复；新目标、新的 `Review requested` 或新的
  finding 会立即中断等待并开始下一 review-unit。每 10 次检查只是一个观察批次，批次结束后自动开始下一批，
  不结束 reviewer turn、不标记 `blocked`，也不消耗 480 review-unit 预算。`waiting-for-main` 只表示当前轮询
  状态，不是终止 residual；即使进程或会话被外部中断，下一次启动也必须从远端状态恢复轮询。
- **Per-target no-progress:** 两次不同的复现/证据路径没有缩小审查 residual 时，缩小 scope 或标记证据
  缺口并转 PLANNER；第三次仍无决定性证据则追加 `needs-info` 或 `ready-for-human` finding，停止扩大
  该目标。
- **Global no-progress:** 连续 3 个 review-unit 未产生新 finding、未关闭/重新分类 finding、未完成一次
  re-review，也没有新的可分配目标时，只有在仍有固定目标正在处理或存在具体证据/权限/隔离阻塞时，才记录
  `waiting-for-main` 并进入 Persistent idle wait；不得把 reviewer 持久目标标记为 `blocked` 或停止。单纯的空
  frontier 必须按 Persistent idle wait 处理；重复读取、重复评论或只编辑旧消息不算 review-unit 进展。只有
  I10 success signal 与 Terminal completion 的全部条件同时满足，才允许终止。
- **Worst-case Plan B:** 保留所有已发送 Audit result 和 finding Issue，列出未审目标、证据缺口、owner 和
  下一解除条件；不得把审查未完成描述为通过，也不得自行合并或关闭阻塞项。

# Progress Invariant

- 每个非终止 review-unit 必须完成一次固定目标的实现/conformance 审查并追加 Audit result，或在实现审查通过后
  完成架构/文档 advisory pass，或完成一个 finding 的根因确认、修复交付或重新分类，或把 scope 严格缩小到
  一个可独立验收的 residual；只报告症状、重复读取同一输出或重复评论不算进展，同时 review-unit 预算严格减少。
- 只创建 Issue、重复读取同一输出或重复评论不算 review-unit 进展；Persistent idle wait 是明确的不计预算等待
  状态。空 frontier、`blocked` finding、主会话未交付的 corrective PR、远端同步失败或 reviewer worktree 保留
  都不得触发持久目标 `blocked`；若有固定目标不能满足 invariant，记录等待原因并继续轮询，不得无限扩大 scope。
- finding ledger、Issue/PR comments、corrective PR 和 re-review SHA 必须 append-only 可追溯；旧 verdict
  失效时只追加 superseding 记录，不编辑历史消息。

# Review Frontier & Fixed Snapshot

- **Persistent objective:** root Issue #9 的当前 checkpoint、child Issue dependency graph、开放/已合并
  PR、历史 commit、finding ledger 和审查评论共同形成 frontier。动态远端状态优先于本文件中的示例；不
  把 `docs/scratch/` 或本地猜测当作当前队列。
- **Review frontier sync:** 每个 review-unit 开始前、完成后、创建 finding/corrective PR 前，以及主会话
  发送新的 `Review requested`、push、合并或改变 acceptance gate 后，重新读取远端 Issue/PR/finding 状态。
  固定 `origin/main`、被审 head SHA、开放 corrective PR、workflow/severity label、review thread 和必要
  checks；通过 `gh --json`/`gh api` 与 `jq` 检查，不依赖另一个会话的即时通知。远端状态无法确认时，不得把
  审查或 handoff 描述为完成。
- **Waiting-for-main sync:** 若发现 Critical/Important finding、待复审 corrective PR/merged SHA、主会话 dirty
  worktree、远端写入尚未确认或其他等待主会话的状态，先追加一次 `waiting-for-main` 记录并保存固定身份、SHA、
  owner、路径和解除条件；随后每 1 分钟重新执行 Frontier Sync。每 10 次只形成观察批次，批次结束后继续下一批，
  不得调用 `blocked` 终止、不得把 reviewer 目标标为 achieved/blocked，也不得清理不属于 reviewer 的 worktree。
- **Selection order:** 默认从最早仍未审查且不依赖未关闭前置 blocker 的 Issue/PR 选择目标，优先当前
  stage gate 和主会话明确发送的 `Review requested`。对明显影响当前 gate、但主会话尚未请求且已经固定的
  PR，可以主动建立审查目标；不审查仍处于写入中的 PR。
- **Review request:** 主会话在 Primary audit 通过后发送新的 `Review requested`，同时给出 PR number、associated
  Issue、head SHA、scope、权威条款、commands、验收 gate、已知 residual 和是否暂停写入。主会话可以在 reviewer
  返回前 Ready/merge；审查会话先验证远端 head SHA 与请求一致，再开始 iteration，并可审查开放 PR 或其合并后的
  固定 commit。
- **Invalidation:** 后续 push、scope 扩大/改变、验收命令或 gate 变化、依赖 closure 变化都会使旧 verdict
  失效。审查会话立即追加 `superseding/re-review` comment，指出被替代的 SHA/verdict；主会话固定新
  快照后重新请求，不编辑旧评论。

# Review Protocol

一次 review-unit 按以下顺序完成；发现不能只停留在症状描述：

1. **Bind:** 读取固定 Issue/PR/commit、head SHA、diff、规范/ADR/计划/fixture 路由和验收命令；记录
   不在 scope 内的内容。
2. **Reproduce:** 运行能发现当前错误的最小 focused checks；根据 scope 需要扩展到 conformance、hash、
   mutation、round-trip、raster 或 workspace gate。每个命令记录实际结果，不把 skipped 当作 passed。
3. **Inspect:** 对照规范条款、调用方、测试和固定 artifact 检查实现、边界、错误路径、资源/依赖和
   交付声明；可引用已合并 commit 指出历史漏洞。
4. **Root-cause analysis:** 从可复现症状沿调用链、数据流和规范边界追到第一个被违反的不变量或契约；
   对竞争性假设做最小区分验证，记录因果链、排除依据和仍未知的部分。只描述症状、未经验证的猜测或
   把作者解释当作根因，不能形成 actionable finding。若根因无法确认，必须记录证据缺口并按 Residual
   Routing 路由，不得带着猜测性修复继续。
5. **Classify and route:** 实现/conformance finding 标为 `Critical`、`Important` 或 `Minor`，判断是否阻塞
   当前 stage/PR gate，并创建或更新 finding Issue；严重度必须由影响、复现结果和根因证据支持。明确该
   finding 是当前 stage 可安全修复的代码/测试问题、later-stage 问题，还是规范/治理决策问题。
6. **Corrective implementation and delivery:** 对根因已确认、属于当前 stage 且可安全在本地收敛的
   implementation/conformance `Critical` 或 `Important` finding，reviewer 必须在 `/tmp` 独立 worktree
   中实现最小修复并补充回归覆盖，运行与变更匹配的 focused/full checks，审查 diff 后 commit、push 并
   创建链接 finding Issue 的 corrective PR。PR 和 Audit result 必须写明根因、修复边界、实际验证结果、
   base/head SHA；验证失败、暴露新根因或无法安全收敛时，保留证据并把 finding 路由为有界 residual，
   不得伪造 pass。reviewer 不得批准、Ready 或合并自己创建的 corrective PR；主会话审查、合并后，新的
   SHA 必须回到本 loop 重新审查。
7. **Advisory pass:** 只有实现/conformance verdict 为 `pass` 时，检查架构 seam、模块边界、局部性、可测试性、
   AI 可导航性，以及 docs/CONTEXT、计划、矩阵、loop 和链接的一致性。该 pass 只产生 advisory，不改变规范状态、
   stage baseline 或当前 acceptance。存在尚未合并或尚未 re-review 的 corrective PR 时，当前目标保持 `blocked`
   或 `needs-info`，不能提前产生 pass advisory。
8. **Cleanup and comment immediately:** 若不再需要本地写入，先按 Worktree Cleanup 安全清理；若必须保留，在
   `Audit result` 中记录 owner、固定 SHA 和清理条件。随后立即在被审 PR（若存在）和关联 Issue 各追加一条 Audit
   result；即使没有 finding 也必须发送，并列出 Root cause、Corrective action、Corrective PR、Regression evidence、
   `Advisories: none` 或 HUMAN-only Issue 列表。评论 append-only，不手写日期，不反复 edit 同一消息；清理未完成
   且没有 owner/condition 时，review-unit 不得算作完成。

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
  reviewer 必须按 Review Protocol 在隔离 worktree 中交付 corrective PR。`Minor` 只有在有 owner、follow-up Issue、
  目标 stage 和解除条件，并且不影响当前验收时才能延期。
- 修复 PR 使用 `Closes #<finding>`；同时以 `Refs #<reviewed-issue-or-pr>` 连接被审目标。修复合并后，
  主会话在 finding Issue 和原 PR 分别追加新的 checkpoint，再提交新 SHA 进行 re-review。
- 历史已合并 PR/commit 的 finding 不重新打开或修改原 PR；从最新 `origin/main` 创建 corrective branch，
  目标为 `main`，并保留发现 SHA。主会话审查并合并后，记录历史漏洞的修复 commit。
- 开放 PR 的 finding 从被审 PR 的固定 head SHA 创建 corrective branch，目标为该活动 PR 的分支。主会话
  在审查期间不推进活动分支；修复 PR 合并后活动 PR 获得新 head SHA，旧 Audit result 失效，必须重新审查。
- 任何 finding 都不得在只保留症状或猜测性根因的状态下标记为已交付；根因未确认、修复边界不安全或验证
  无法收敛时，必须保留 finding 并按 Residual Routing 记录证据缺口、owner 和解除条件。

## HUMAN-only advisory contract

架构和文档 advisory Issue 必须绑定被审目标、head SHA、scope、观察到的证据、建议、影响、人工 owner 和
建议处理条件，并使用 `ready-for-human` 状态及合适的 `documentation`、`workflow` 或 `enhancement` 标签。
它们不使用 `review-finding` 或 severity label，不能关闭当前实现 Issue，不能阻塞 I10，也不能被 `loop.md`
自动选为 work-unit。只有当证据升级为规范矛盾、实现缺陷或当前 conformance 违约时，才转回标准 finding contract。

# Reviewer Metadata Duties

审查者可以管理自己创建的 finding Issue 的既有元数据，但不能借此改变项目的全局治理：

- 创建 finding 时使用已有的 `review-finding` label，并根据证据添加至多一个 `severity:critical`、
  `severity:important` 或 `severity:minor`；需要时可以添加已有的 `specification` 或 `conformance`
  等正交 label。一个 open finding 仍必须保持恰好一个 workflow-state label。
- 当 finding 已能明确归属阶段时，可以给该 finding 分配已有 milestone（例如 `I2 Static Semantics`）；不
  明确时保留未设置状态，并在 Issue/comment 中写出 milestone 建议及解除条件。
- 可以用新的英文 comment 或 finding Issue 提议新增/调整 label 或 milestone，但不得直接创建、重命名、删除
  或改变全局 label/milestone 定义，也不得修改被审主 Issue 的 workflow label 或 milestone。
- 这些 metadata 只用于路由和审查证据，不能提升规范状态、改变 stage baseline、替代 owner/依赖关系，或把
  `Minor` finding 自动变成当前 gate 的阻塞项。

# Corrective Branch & Worktree Isolation

- 创建 corrective PR 必须使用 `/tmp` 下的独立 worktree 和独立分支；推荐路径为
  `/tmp/fcs-finding-<finding>-<slug>`，分支命名为 `codex/<finding>-<slug>`。单独分支不等于工作树隔离，
  两者都必须满足。reviewer loop 不得把 worktree 放在主仓库、主仓库旁、用户 home 或其他任意路径。
- 对需修复的当前-stage implementation/conformance finding，reviewer 是该 corrective worktree 的执行 owner：
  从固定 base/head SHA 建立 worktree 后，只在其中修改代码和测试，保留主会话 dirty worktree、活动实现分支和
  `main` 不变。修复必须最小化地针对已确认根因，并包含能失败于旧行为、通过于新行为的回归证据。
- corrective PR 创建前，reviewer 必须确认 worktree owner、用途、base/head SHA、分支、变更范围和验证命令；
  PR 正文或首条进度评论必须链接 finding、记录根因和实际验证结果。只创建分支或只提交猜测性 patch 不满足
  corrective delivery。
- 纯只读审查也必须使用 `/tmp` 下的快照 worktree，推荐路径为 `/tmp/fcs-review-<target>`；可使用 detached
  HEAD，不创建实现分支。主实现 worktree 不受本条路径约束。
- 创建前记录 worktree owner、用途、固定 base/head SHA、分支或 detached 状态和预计清理条件；使用
  `git worktree list --porcelain` 验证路径确实位于 `/tmp/`，不得用符号链接绕过该约束。每个 session/target
  使用唯一路径；若路径已存在，先核对 owner 和状态，不得复用或覆盖 dirty worktree。
- 审查者不得写入当前会话的 dirty worktree、活动实现分支或 `main`；纯审查可在只读固定 commit 上完成。
- 开放 PR 的 corrective branch 起点是固定 head SHA，PR base 是被审 PR 的活动分支；历史 commit 的
  corrective branch 起点是最新 `origin/main`，PR base 是 `main`。
- 当前会话负责检查 corrective diff、验证命令、required checks 和 review requirements，并合并 corrective
  PR。审查者不得批准自己的修复；主会话以 Primary Self-Audit 作为 Ready/merge 门禁，合并后的新 SHA
  仍须送回本 loop 做异步独立二审。

# Worktree Cleanup

- 只读审查在没有未记录 artifact 且不再需要本地写入后结束其 `/tmp/fcs-review-*` worktree；安全条件满足时
  应在最终 `Audit result` 前清理，以便消息记录 `Worktree: cleaned`。
- corrective worktree 在分支已 push、PR 已建立且没有待提交修改后，若 reviewer 不再需要本地修改，可以
  暂时清理；若仍可能需要修复，则保留到 PR 合并/关闭或明确放弃，并记录 owner、原因和下一条件。PR 合并
  后的最终 re-review handoff 完成时必须清理剩余 worktree。
- 清理必须由该 worktree 的 owner 执行：先确认
  `git -C <path> status --porcelain` 为空，再执行 `git worktree remove <path>`、`git worktree prune`，
  最后用 `git worktree list --porcelain` 确认路径已消失。不得使用 `--force` 删除 dirty worktree，也不得
  删除未 push 的 commit 或未记录 artifact。
- 清理失败或 worktree 变脏时，保留现场并追加 residual：包含 path、owner、固定 SHA、阻塞原因和下一清理
  条件；不得把审查标记为完全交付。主会话不得代替 reviewer 删除其 dirty worktree。

# Audit Comment Contract

审查目标完成后，在被审 PR（若存在）和关联 Issue 立即追加内容等价于以下结构的新消息：

```md
## Audit result

- Target: PR #<n> / Issue #<n> / commit <sha>
- Head SHA: `<sha>`
- Scope: <固定范围>
- Commands: `<command>` → <passed/failed/skipped>（列出实际结果）
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
- Next: <主会话或 finding owner 的下一有界动作>
```

若快照失效，追加 `## Superseding audit`，明确被替代的 head SHA、旧 verdict、新请求原因和新审查目标；
不要修改旧消息。评论标题不手写 `YYYY-MM-DD` 等日期，GitHub timestamp 是时间记录。

# Permissions & Handoff

- 审查会话可以使用 `gh issue create`、`gh issue comment`、`gh pr comment`、`gh pr review --request-changes`
  或 `--comment`，以及在独立 worktree/branch 上实现、验证、commit、push 和创建 corrective PR；所有 GitHub
  网络失败遵守 `AGENTS.md` 的 5 秒、最多 10 次重试和 pending remote sync 规则。
- 审查者只清理自己在 `/tmp` 下创建的 worktree，并且必须遵守 Worktree Cleanup；主会话只清理自己创建的
  临时 worktree，不能删除 reviewer 的 dirty worktree。
- 审查会话禁止 `gh pr merge`、`gh pr ready`、关闭主 Issue、修改主 Issue workflow label、force-push、
  修改活动实现分支，或把未确认的远端动作描述为成功。
- 主会话读取 Audit result 和 finding ledger，按严重度和依赖处理 corrective PR；它是唯一可以将 PR 标记
  Ready、合并 PR、关闭主 Issue 或声明 stage gate 的角色。
- 审查者创建的 corrective PR 不由创建者批准。主会话可以审查并合并该 PR；合并后必须以活动主 PR 的
  新 head SHA 回到 `Review request` 阶段。

# Approval Gates

- 审查会话在既定 workflow 授权内可以读取、comment、request changes、创建 finding Issue 和 corrective PR；
  这些动作仍受 `AGENTS.md` 的稳定身份查询、重试和 pending remote sync 规则约束。
- 合并、`gh pr ready`、关闭主 Issue、修改主 Issue workflow label、修改活动实现分支、force-push、降低
  required gate、公开发布和任何 destructive history/data operation 都不是审查会话的权限；需要由当前主
  会话按 `docs/loops/loop.md` 处理，或触发其 Approval Gate。
- 审查 verdict 不是 merge 授权。主会话仍必须检查 required checks、mergeability、review threads、delivery-
  ready comment、finding 状态和当前 head SHA；任何一项过期都必须重新走 handoff。

# Measurement Domain

| Review domain | Minimum evidence | Required record |
|---|---|---|
| Scope/authority | fixed SHA、规范/ADR/plan clauses、依赖 closure | target binding and limitations |
| Implementation/root cause | diff、调用方、错误路径、资源/依赖边界、从症状到违反契约的根因证据 | finding location, causal chain and impact |
| Tests/conformance | focused/full command output、回归测试、fixture/golden/hash/raster as applicable | command status, regression evidence and artifact link |
| Architecture | module boundaries、seams、dependency direction、locality、testability、AI navigability | advisory Issue or `Advisories: none` |
| Documentation | CONTEXT/spec/ADR/plan/loop links、terminology、scope/authority consistency | advisory Issue or `Advisories: none` |
| GitHub delivery | Issue/PR linkage、branch/base、review threads、required checks | Audit result and finding ledger |
| Corrective delivery | confirmed root cause、isolated worktree、最小修复 diff、回归命令结果、finding link、base/head SHA | corrective PR, commit/push evidence and re-review target |

# Residual Routing

| Residual / failure | Route | Action |
|---|---|---|
| 根因已确认、属于当前 stage 且安全可修复的 Critical/Important finding | LOCAL → 主会话 | reviewer 在 `/tmp` 隔离 worktree 中实施最小修复、补回归测试、运行验证、commit/push 并创建 linked corrective PR；阻塞主 PR，等待主会话合并后 re-review |
| 只有症状或竞争性假设，根因仍未确认 | PLANNER/HUMAN | 记录已执行的区分验证、证据缺口、owner 和解除条件；不得提交猜测性修复或报告为 actionable pass |
| 根因已确认但修复需要规范/ADR/semantic-profile 选择 | PLANNER/HUMAN | 保留根因证据和双方影响，按治理流程路由；不得用 reviewer 的偏好替代规范决定 |
| corrective patch 的 focused/full 验证失败或暴露新的根因 | LOCAL/HUMAN | 保留 dirty worktree 和实际输出，更新 finding/owner/解除条件；未收敛前不得创建虚假 pass 或关闭 finding |
| 可复现但属于 later-stage 的 finding | PLANNER | 记录 owner、目标 stage、依赖、验收条件和 follow-up Issue，不阻塞不相关当前工作 |
| Minor 且不影响当前 gate | PLANNER | 要求 owner 和解除条件；在当前 Audit result 中明确延期，不给出无条件 pass |
| 缺少固定 SHA、scope、命令或 artifact | LOCAL | 请求补齐固定输入；输入未齐前 verdict 为 `needs-info` |
| 规范/ADR/外部证据冲突 | PLANNER/HUMAN | 保留双方证据，按 specification governance 或新 ADR 路由，不自行选择公开语义 |
| 审查者角色不独立、被审目标仍在写入或工作树隔离失败 | HUMAN | 停止该 iteration，报告冲突和恢复条件 |
| reviewer worktree 不在 `/tmp`、变脏、owner/固定 SHA 缺失或无法安全清理 | LOCAL/HUMAN | 停止交付，保留现场并记录清理条件；不得使用 `--force` 或越权删除 |
| GitHub 瞬时网络失败 | LOCAL | 按重试规则查询稳定身份并重试；耗尽则保存 payload/outbox，继续安全只读审查 |
| 连续 3 个 review-unit 无进展且无新 frontier | LOCAL/WAIT | 记录 `waiting-for-main` 并按每分钟 Frontier Sync 持续轮询；不标记 reviewer 持久目标 `blocked`，直到 I10 success signal 和 Terminal completion 同时满足 |
| 无 I10 完成且无固定目标 | LOCAL/WAIT | 进入持续的 1 分钟 Frontier Sync 轮询；每 10 次形成一个非终止观察批次，目标出现立即恢复，不标记 `blocked`，不消耗 review-unit 预算 |
| 达到 480 review-unit | PLANNER | 停止当前预算内的新 review-unit 分配，保留 finding ledger 和 HUMAN-only advisory，产出后继审查 loop handoff；不得把预算耗尽描述为空 frontier 的 `blocked` |
| 架构/文档优化建议 | HUMAN | 创建 `ready-for-human` HUMAN-only Issue；不进入主 loop、不自动修复、不改变当前 gate |
