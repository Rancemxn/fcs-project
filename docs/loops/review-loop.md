# Goal & Success Signal

- **Goal:** 从 GitHub root Issue、child Issue/PR 图和已合并提交中选择最早尚未审查的固定目标，复现其
  规范、阶段 gate、测试和交付证据，及时记录可复现 finding，并把需要修复的 finding 路由给当前主实现
  会话，直到当前审查 frontier 闭合。
- **Observable success signal:** 每个已审查目标都有一条 append-only `Audit result`（即使零 finding），
  其中包含目标身份、head SHA、scope、实际命令、verdict、限制和 Next；所有 Critical/Important finding
  都有 owner、修复路径和最新状态；必要的 corrective PR 已链接 finding Issue，且主会话已经重新审查
  修复后的新 SHA；当前 frontier 没有未审查目标，也没有未分配的 Critical/Important finding。
- 审查 loop 不声明阶段、规范版本域或 FCS 5 RC 完成；它只产生独立审查证据和 finding 路由。最终完成
  仍由主 `docs/loops/loop.md` 根据规范、fixture、baseline、gate 和合并证据判定。

# Scope & Authority

- `docs/specifications/fcs.md`、`docs/specifications/fcbc.md`、`docs/specifications/fcs-render.md`、
  `docs/specifications/fcs-conversion.md` 和
  `docs/specifications/governance.md` 是规范与状态权威；Accepted ADR 是架构/治理约束；计划、Issue、PR、
  实现、测试和本 loop 只能安排或证明工作，不能创造规范语义。
- 每次审查必须绑定一个不可漂移的快照：`Issue/PR 或 commit + head SHA + scope + commands + acceptance
  gate`。审查者不得把作者的结论、旧测试输出或未固定的工作树当作快照证据。
- 审查会话是独立于主实现会话的第二个角色。主会话是唯一实现者和唯一 merge owner；审查会话可以
  读取、评论、review/request changes、创建 finding Issue 和 corrective PR，但不得合并任何 PR、将 PR
  标记为 Ready、关闭主 Issue、修改主 Issue 的 workflow label，或写入主会话的工作树、活动实现分支
  和 `main`。
- 审查会话不能以审查结果修改规范状态、冻结版本域、重写 Accepted ADR 或替代 conformance artifact。
  发现规范缺口时只记录证据并按规范治理路由。

# Termination Conditions

- **Max iterations / budget:** 最多 240 个 `review-unit` iterations，与主 loop 的 240 个 work-unit
  预算独立计算。一次 iteration 只审查一个固定目标，不按命令数、评论数或 commit 数计数，也不得通过
  拆分同一快照绕过预算。
- **Goal-achievement check:** frontier 中每个已分配目标都有最新 Audit result；没有未分配的 Critical/
  Important finding；需要修复的 finding 都已链接 owner、目标 stage、依赖、验收条件和 corrective PR 或
  明确的 HUMAN/PLANNER residual。仍在写入中的目标不算未完成审查目标，必须等待其固定快照。
- **Per-target no-progress:** 两次不同的复现/证据路径没有缩小审查 residual 时，缩小 scope 或标记证据
  缺口并转 PLANNER；第三次仍无决定性证据则追加 `needs-info` 或 `ready-for-human` finding，停止扩大
  该目标。
- **Global no-progress:** 连续 3 个 review-unit 未产生新 finding、未关闭/重新分类 finding、未完成一次
  re-review，也没有新的可分配目标时终止并报告阻塞证据。重复读取、重复评论或只编辑旧消息不算进展。
- **Worst-case Plan B:** 保留所有已发送 Audit result 和 finding Issue，列出未审目标、证据缺口、owner 和
  下一解除条件；不得把审查未完成描述为通过，也不得自行合并或关闭阻塞项。

# Progress Invariant

- 每个非终止 review-unit 必须完成一次固定目标的审查并追加 Audit result，或关闭/重新分类至少一个
  finding，或把 scope 严格缩小到一个可独立验收的 residual；同时 review-unit 预算严格减少。
- 只创建 Issue、重复读取同一输出、重复评论或等待远端状态不算进展。若当前目标不能满足 invariant，必须
  按 no-progress 或 Residual Routing 退出，不得无限扩大 scope。
- finding ledger、Issue/PR comments、corrective PR 和 re-review SHA 必须 append-only 可追溯；旧 verdict
  失效时只追加 superseding 记录，不编辑历史消息。

# Review Frontier & Fixed Snapshot

- **Persistent objective:** root Issue #9 的当前 checkpoint、child Issue dependency graph、开放/已合并
  PR、历史 commit、finding ledger 和审查评论共同形成 frontier。动态远端状态优先于本文件中的示例；不
  把 `docs/scratch/` 或本地猜测当作当前队列。
- **Selection order:** 默认从最早仍未审查且不依赖未关闭前置 blocker 的 Issue/PR 选择目标，优先当前
  stage gate 和主会话明确发送的 `Review requested`。对明显影响当前 gate、但主会话尚未请求且已经固定的
  PR，可以主动建立审查目标；不审查仍处于写入中的 PR。
- **Review request:** 主会话在 PR Ready/merge 前发送新的 `Review requested`，同时给出 PR number、
  associated Issue、head SHA、scope、权威条款、commands、验收 gate、已知 residual 和是否暂停写入。
  审查会话先验证远端 head SHA 与请求一致，再开始 iteration。
- **Invalidation:** 后续 push、scope 扩大/改变、验收命令或 gate 变化、依赖 closure 变化都会使旧 verdict
  失效。审查会话立即追加 `superseding/re-review` comment，指出被替代的 SHA/verdict；主会话固定新
  快照后重新请求，不编辑旧评论。

# Review Protocol

一次 review-unit 按以下顺序完成：

1. **Bind:** 读取固定 Issue/PR/commit、head SHA、diff、规范/ADR/计划/fixture 路由和验收命令；记录
   不在 scope 内的内容。
2. **Reproduce:** 运行能发现当前错误的最小 focused checks；根据 scope 需要扩展到 conformance、hash、
   mutation、round-trip、raster 或 workspace gate。每个命令记录实际结果，不把 skipped 当作 passed。
3. **Inspect:** 对照规范条款、调用方、测试和固定 artifact 检查实现、边界、错误路径、资源/依赖和
   交付声明；可引用已合并 commit 指出历史漏洞。
4. **Classify:** 每项 finding 标为 `Critical`、`Important` 或 `Minor`，并判断是否阻塞当前 stage/PR gate。
   严重度必须由影响和可复现证据支持，不以个人偏好升降级。
5. **Route:** 对 finding 创建或更新 Issue；需要代码/测试/文档修复时创建 corrective PR。审查者不得
   合并、Ready 或批准自己创建的 corrective PR；由主会话审查、合并，再把主 PR 的新 SHA 送回本 loop。
6. **Comment immediately:** 审查结束后立即在被审 PR（若存在）和关联 Issue 各追加一条 Audit result；即使
   没有 finding 也必须发送。评论 append-only，不手写日期，不反复 edit 同一消息。

# Finding Contract & Routing

每个 finding Issue 的初始正文至少包含：

- 被审 Issue/PR/commit 与发现时的 head SHA；
- 文件、符号或稳定位置；
- 违反的规范条款、ADR/计划 gate 或交付约束；
- 最小复现命令和实际输出/artifact；
- 影响、严重度、当前 gate 是否阻塞；
- owner、目标 stage、依赖、验收条件和预期 corrective PR。

路由规则：

- 当前被审 Issue 的本地 finding 默认作为其 child/parent 关系下的 finding Issue；跨阶段或 root-level
  问题才直接挂 root Issue #9。不要把 later-stage finding 伪装成当前 stage 的缺陷关闭条件。
- 当前 stage 的 `Critical`/`Important` finding 阻塞 frontier 和主 PR Ready/merge；`Minor` 只有在有 owner、
  follow-up Issue、目标 stage 和解除条件，并且不影响当前验收时才能延期。
- 修复 PR 使用 `Closes #<finding>`；同时以 `Refs #<reviewed-issue-or-pr>` 连接被审目标。修复合并后，
  主会话在 finding Issue 和原 PR 分别追加新的 checkpoint，再提交新 SHA 进行 re-review。
- 历史已合并 PR/commit 的 finding 不重新打开或修改原 PR；从最新 `origin/main` 创建 corrective branch，
  目标为 `main`，并保留发现 SHA。主会话审查并合并后，记录历史漏洞的修复 commit。
- 开放 PR 的 finding 从被审 PR 的固定 head SHA 创建 corrective branch，目标为该活动 PR 的分支。主会话
  在审查期间不推进活动分支；修复 PR 合并后活动 PR 获得新 head SHA，旧 Audit result 失效，必须重新审查。

# Corrective Branch & Worktree Isolation

- 创建 corrective PR 必须使用独立 worktree 和独立分支；建议命名为 `codex/<finding>-<slug>`。单独分支
  不等于工作树隔离，两者都必须满足。
- 审查者不得写入当前会话的 dirty worktree、活动实现分支或 `main`；纯审查可在只读固定 commit 上完成。
- 开放 PR 的 corrective branch 起点是固定 head SHA，PR base 是被审 PR 的活动分支；历史 commit 的
  corrective branch 起点是最新 `origin/main`，PR base 是 `main`。
- 当前会话负责检查 corrective diff、验证命令、required checks 和 review requirements，并合并 corrective
  PR。审查者不得批准自己的修复；主 PR 只有在修复后新 SHA 通过本 loop 的独立 Audit result 后才能 Ready/merge。

# Audit Comment Contract

审查目标完成后，在被审 PR（若存在）和关联 Issue 立即追加内容等价于以下结构的新消息：

```md
## Audit result

- Target: PR #<n> / Issue #<n> / commit <sha>
- Head SHA: `<sha>`
- Scope: <固定范围>
- Commands: `<command>` → <passed/failed/skipped>（列出实际结果）
- Verdict: `pass` / `blocked` / `needs-info`
- Findings: <none 或 #finding 列表，含 severity>
- Gate impact: <当前 stage/PR gate 是否阻塞>
- Limitations: <未覆盖范围或 none>
- Next: <主会话或 finding owner 的下一有界动作>
```

若快照失效，追加 `## Superseding audit`，明确被替代的 head SHA、旧 verdict、新请求原因和新审查目标；
不要修改旧消息。评论标题不手写 `YYYY-MM-DD` 等日期，GitHub timestamp 是时间记录。

# Permissions & Handoff

- 审查会话可以使用 `gh issue create`、`gh issue comment`、`gh pr comment`、`gh pr review --request-changes`
  或 `--comment`，以及在独立 worktree/branch 上创建和 push corrective PR；所有 GitHub 网络失败遵守
  `AGENTS.md` 的 5 秒、最多 10 次重试和 pending remote sync 规则。
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
| Implementation | diff、调用方、错误路径、资源/依赖边界 | finding location and impact |
| Tests/conformance | focused/full command output、fixture/golden/hash/raster as applicable | command status and artifact link |
| GitHub delivery | Issue/PR linkage、branch/base、review threads、required checks | Audit result and finding ledger |
| Corrective delivery | isolated worktree、finding link、base/head SHA、主会话 merge evidence | corrective PR and re-review target |

# Residual Routing

| Residual / failure | Route | Action |
|---|---|---|
| 可复现的 Critical/Important finding | LOCAL → 主会话 | 创建 finding Issue 和 corrective PR；阻塞主 PR，等待修复并 re-review |
| 可复现但属于 later-stage 的 finding | PLANNER | 记录 owner、目标 stage、依赖、验收条件和 follow-up Issue，不阻塞不相关当前工作 |
| Minor 且不影响当前 gate | PLANNER | 要求 owner 和解除条件；在当前 Audit result 中明确延期，不给出无条件 pass |
| 缺少固定 SHA、scope、命令或 artifact | LOCAL | 请求补齐固定输入；输入未齐前 verdict 为 `needs-info` |
| 规范/ADR/外部证据冲突 | PLANNER/HUMAN | 保留双方证据，按 specification governance 或新 ADR 路由，不自行选择公开语义 |
| 审查者角色不独立、被审目标仍在写入或工作树隔离失败 | HUMAN | 停止该 iteration，报告冲突和恢复条件 |
| GitHub 瞬时网络失败 | LOCAL | 按重试规则查询稳定身份并重试；耗尽则保存 payload/outbox，继续安全只读审查 |
| 连续 3 个 review-unit 无进展且无新 frontier | HUMAN | 终止本轮，报告未审目标、证据缺口和解除条件 |
| 达到 240 review-unit | PLANNER | 终止本轮，保留 finding ledger，产出后继审查 loop 建议 |
