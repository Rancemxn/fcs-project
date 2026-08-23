# ADR 0014: FCS5 Session-Pool Delivery Workflow

状态：Accepted

日期：2026-08-23

## 1. 背景

当前 FCS 文档把“主实现会话”和“独立审查会话”作为主要角色。该模型适合单一实现
会话，但不能承载已经恢复的 session-pool 使用方式：一个持久的 Rancemxn 协调会话
需要同时调度多个实现 lane、只读研究窗口和固定 SHA 的异步审查窗口。历史 commit
`1ae7ab7` 曾记录过并行交付模型，但它不是当前 `origin/main` 的 active contract，
且历史 Issue #511 已关闭。

如果不显式冻结新边界，以下状态会互相冲突：

- 两个窗口是否都能写代码、谁拥有 Ready/merge 和 `main`；
- session ID、slot、assignment、branch 和 worktree 是否稳定关联；
- review 是实现前置门还是异步固定快照；
- 本地 Cargo 验证与 GitHub Full Gate 的职责；
- reviewer finding 是 reviewer 自己修还是由新的 deliver lane 修；
- `/new`、session crash、dirty worktree 和 scope conflict 如何恢复。

## 2. 决策

采用 `docs/loops/fcs5-session-pool-delivery.md` 作为当前唯一 session-pool delivery
contract：

1. **Coordinator ownership**：主会话是长期运行的 Rancemxn coordinator，负责 frontier
   sync、Issue/PR 选择、assignment、scope lease、Primary audit inspection、Ready、
   merge 和 `main`。它不是普通实现 writer，不在共享实现 lane 直接代写。
2. **Warm topology**：保持两个 warm `deliver` writer slots，以及 warm `reviewer`、
   `researcher`、`scout` slots。所有五个窗口可以活跃并发；只有两个 deliver 是 writer，
   且 writer 必须声明 allowed paths 和 semantic domain。临时 spawn 只服务短任务或故障
   恢复。
3. **Stable identity**：slot 是可复用能力身份，session ID 只代表一次运行，assignment
   代表一次 bounded work。所有窗口从协调根目录启动，assignment metadata 记录绝对
   worktree；session pool 不负责 worktree 创建、锁定或清理。
4. **Role separation**：`deliver` 由 Delivery App logical identity 在已分配 branch 上
   实现、commit、push、创建/更新 draft PR、写 progress 和 Primary audit；它不能 Ready、
   merge 或更新 `main`。`reviewer` 由 Review App logical identity 对固定 SHA 只读复审，
   可以 comment/request changes/create finding Issue，但不改代码、不 push、不创建 corrective
   PR。reviewer finding 由新的 deliver corrective lane 实现。
5. **Evidence**：首个可审查 commit 建立 draft PR。Full Gate 继续保留 `push`、
   `pull_request` 和 `workflow_dispatch` 触发。任一同一目标 SHA 的成功 Full Gate run 可以
   作为 Full Gate evidence，但 required PR checks 仍由主会话独立核对；run 的
   `headSha`、event、URL、run ID 和 conclusion 必须被记录。
6. **Validation boundary**：所有 worktree 本地只做 fmt 和不生成 Cargo artifact 的静态
   检查。Cargo check、Clippy、build、test、nextest、fuzz 和 executable fixture 统一由
   GitHub Actions 执行。workflow-only metadata 改动的 Rust Full Gate 为 non-applicable；
   修改 Full Gate 执行逻辑时重新适用。
7. **Review timing**：deliver 的 Primary audit 是 Ready/merge 所需的当前交付证据；
   reviewer 是异步二审，不是普通 work unit 的前置等待门。任何后续 push、scope、命令、
   依赖或验收变化都会使旧 audit/review snapshot 失效，并要求 superseding/re-review。
8. **Recovery**：assignment 边界才能正常 `/new`；中途 `/new` 需要先提交 recovery
   handoff 并由 coordinator 创建 successor assignment。scope conflict 冻结相关 lane，
   另派 integration/corrective deliver，不在共享 worktree reset 或覆盖他人修改。
9. **Credentials**：仓库只记录逻辑身份 `deliver`、`reviewer` 和 `Rancemxn`。App ID、
   installation、private key、token、broker endpoint 和 secret 不进仓库或 session metadata。
   历史 App/broker 的恢复、权限验证和显示名迁移是独立后续工作，不由本 ADR 授权执行。

## 3. 权衡与后果

正面后果：

- Rancemxn 保持唯一 Ready/merge/main owner，避免多个窗口把 GitHub 状态推进到不一致；
- 两个 writer lane 能并发推进不重叠 work unit，reader 窗口能持续提供静态、外部和复审证据；
- session 重启不再冒充任务完成，slot、assignment、SHA 和 worktree 形成可恢复链；
- review finding 的实现权限与 review 权限分离，corrective work 可按同一 gate/Primary audit
  路径复用；
- Full Gate 仍以同 SHA GitHub run 为门禁来源，session pool 不成为隐性 CI 替代品。

成本与限制：

- coordinator 必须维护 writer scope lease、session health、worktree owner 和远端
  Frontier Sync；session pool 本身不提供文件锁或 Git 锁；
- warm window 的长期上下文仍可能过期，`/new` 后必须重新注册 slot metadata；
- Delivery/Review App broker 的真实 permission 不能由 Markdown 证明，必须由独立恢复工作
  使用 App-level authentication 和最小权限验证；
- 任一 exact-SHA 成功 run 可作 Full Gate evidence 不等于所有 duplicate run 成功，required
  check、event、headSha 和 branch protection 仍需分别检查。

## 4. 明确禁止

- 不得将历史 `1ae7ab7`、Issue #511、旧 scratch 或 session transcript 当作当前 workflow
  权威。
- 不得把 reviewer、researcher 或 scout 变成隐式 writer，不得让 reviewer 自己创建或
  实现 corrective PR。
- 不得让 deliver Ready/merge、push `main`、绕过 branch protection 或使用未经验证的
  App token。
- 不得在多个 writer 共享 worktree 中 reset、rebase、覆盖或清理他人 dirty changes。
- 不得在任何本地 lane 运行 Cargo check/build/test/lint/fuzz 或用本地结果替代 GitHub
  Full Gate；不得在工作流文档中伪造 Action 证据。
- 不得把 session pool assignment/report、monitor 状态或 broker transport outbox 当作
  Issue/PR/Action/merge 的远端成功证明。
- 不得把规范语义、semantic profile、conformance 结论或阶段 Frozen 状态写入本 ADR。

## 5. 取代关系

本 ADR Accepted 后：

- `docs/loops/fcs5-session-pool-delivery.md` 是当前唯一 active delivery loop；
- 旧 `docs/loops/loop.md` 和 `docs/loops/review-loop.md` 降级为 superseded pointer，不再作为当前执行契约；
- ADR 0011 的 GitHub Issue/PR、append-only progress、retry/outbox、规范权威和远端证据
  条款保持有效；其关于“当前主会话是唯一实现者”和“reviewer 可创建 corrective PR”的
  历史 dated amendments 由本 ADR 及新 loop 的角色条款部分取代；
- `AGENTS.md` 和 `docs/agents/issue-tracker.md` 必须指向新 loop，不能继续把被取代角色
  描述为当前模型；
- 任何未来与本 ADR 冲突的 change 必须新建 ADR，并将本 ADR 标记为 `Superseded` 或
  `Partially superseded`，不得静默编辑历史决定。

## 6. 验收证据

本 ADR 的实现验收是 workflow/documentation scope：

- 新 loop、ADR 和 active references 的 link/reference 一致；
- `docs/loops/` 只把 `fcs5-session-pool-delivery.md` 作为 active contract；旧 loop 文件仅为明确标记的
  superseded pointer，不存在未声明的 active workflow 引用；
- `full-gate.yml` 的三个触发器不被本 ADR 改变；
- Markdown、YAML/JSON/schema/reference checks 与 `git diff --check` 通过；
- 不运行本地 Cargo build/test/lint/fuzz；没有同 SHA Full Gate 的要求，除非本 PR 改动
  workflow 执行逻辑或 Rust/build/dependency/test/executable fixture。
