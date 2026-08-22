# FCS5 Session-Pool Delivery

本文件是 FCS 当前的唯一交付工作流契约。它取代 `docs/loops/loop.md` 与
`docs/loops/review-loop.md` 的当前执行职责，并由 ADR 0014 记录其治理边界。
本文件是 workflow contract，不是 FCS、FCBC、Execution ABI、Render 或 Conversion
规范，也不改变任何版本域的 `Frozen` 状态。

## Freeze Protocol

- 本文件只规定协作、交付、审查、会话和验证边界；规范语义仍由
  `docs/specifications/`、治理文件、Accepted ADR、conformance artifact 和 dated review
  定义。
- 本文件是**工作流冻结**（workflow freeze），不是规范版本域的 `Frozen`；后者只由
  `docs/specifications/governance.md` 管理。本文件不声明阶段完成、版本 Frozen 或公开发布。
- ADR 0014 是本工作流的 Accepted decision。后续改变角色权限、session 拓扑、lane
  生命周期、验证门禁或 supersession 关系时，必须新建 ADR 和新的命名 workflow 文档；
  不得静默修改本文件形成第二套隐含流程。
- GitHub Issue、PR、session-pool assignment、agent 报告、实现和测试只安排或证明工作，
  不得创造规范语义。
- 历史的 `1ae7ab7` 和已关闭 Issue #511 只作为旧 workflow 的来源证据，不是当前状态入口。

# Goal & Success Signal

目标是在不把主会话变成普通实现 worker 的情况下，持续交付可审查的 FCS work unit：
主会话选择和分配任务，`deliver` 完成实现，`reviewer` 对固定 SHA 独立复审，
只有 Rancemxn 可以把 PR 标记 Ready、合并 PR 或更新 `main`。

一个 work unit 只有同时满足以下条件才算交付：

- bounded Issue、branch、lane worktree、允许路径和验收条件已经固定；
- deliver 的首个可审查 commit 已建立 draft PR，后续 push 的 branch/PR 状态可回读；
- 适用的 GitHub Full Gate 对目标 SHA 成功，且 run 的 `headSha` 与目标 SHA 完全一致；纯 workflow/documentation metadata work unit 明确记录 Rust gate `non-applicable`；
- deliver 已在 PR 和 Issue（若有）追加 Primary audit result；
- 主会话已检查 diff、Issue/PR、required checks、mergeability、finding 和 scope，
  并由 Rancemxn 执行 Ready/merge；
- reviewer 已收到固定 SHA 的 Review requested，或在合并后按 review frontier 排队；
- worktree 的 owner、dirty 状态、后续清理条件和残余已记录。

该信号不代表阶段完成、规范 Frozen 或公开发布完成。阶段和发布仍由路线图、治理、
conformance、独立 review 和最终 gate 判定。

# Scope & Authority

- `AGENTS.md` 是仓库协作入口；本文件是当前交付 loop；
  `docs/agents/issue-tracker.md` 管理 GitHub Issue/PR 证据格式。
- Accepted ADR 约束架构和治理方向，但不替代规范文本。ADR 0014 只改变交付和
  agent/session 角色边界，不改变 FCS 领域语义。
- Issue/PR 是当前工作契约、依赖和交付证据的 tracker；GitHub Action 是其声明的
  SHA 验证证据。session pool 不是第二个 tracker。
- 外部格式、固定参考快照、依赖源码和 semantic profile 继续遵守根 `AGENTS.md` 的
  阅读路由，不因 researcher 或 scout 的报告而获得规范权威。

# Frontier, Budget, and No-Progress

- 当前状态以 root Issue 最新有效 checkpoint、child/finding Issue dependency graph、开放/已合并 PR、
  Action artifact、review ledger 和 worktree inventory 为准；本文件不复制瞬时 SHA 或 Issue 状态。
- 每个 work unit 必须从 bounded Issue 开始，拥有编号 acceptance、authority、non-goals、dependency
  residual 和验证方法。主交付预算最多 240 个 work-unit，review budget 最多 480 个 review-unit；
  等待 session、网络或 Action 不消耗预算。不得通过扩大单元、重复命名或拆出等价 Issue 绕过预算。
- 同一 Issue 两次不同技术路径仍未关闭验收或减少 residual 时转 PLANNER；第三次没有决定性证据时转
  `needs-info` 或 `ready-for-human`，保留证据并停止扩大范围。连续三个 work-unit 没有关闭验收、
  新增决定性证据或严格缩小的 ready unit，且无其他 ready frontier 时，进入有界等待/规划，不把重复
  读取或改写说明计为进展。
- reviewer 对固定目标也遵守根因和证据收敛规则；I10 未完成且没有固定 target 时每分钟 Frontier Sync，
  空 frontier 不是 `blocked` 或终止条件，等待不消耗 480 budget。只有 root Issue 的 I10 success signal
  满足，且没有新 target、Critical/Important finding、待复审 SHA 或 reviewer worktree 时，reviewer 才能
  终止并报告 frontier closed。
- I1–I10、五个规范版本域、Reviewed/Frozen、conformance 和公开发布的 success signal 仍由路线图、
  governance、fixture、dated review 和 root Issue 定义；本文件只定义交付如何产生和验证这些证据。

# Roles & Permissions

| 角色 | GitHub 身份 | 允许 | 禁止 |
|---|---|---|---|
| Parent coordinator | `Rancemxn` | 选择 frontier、分配 lane、审 diff、Ready、merge、更新 `main`、处理 approval gate | 不把主会话当作普通实现 writer；不在共享 lane 代写实现 |
| `deliver` | 逻辑身份 `deliver`，由 Delivery App broker 提供凭据 | 在已分配 worktree 修改文件、commit、push branch、创建/更新 draft PR、写 progress 和 Primary audit、等待并核对 Action | Ready、merge、push `main`、关闭主 Issue、修改主 Issue workflow label、绕过 branch protection |
| `reviewer` | 逻辑身份 `reviewer`，由 Review App broker 提供凭据 | 读取固定 Issue/PR/commit、review、comment、request changes、创建 finding Issue 和审查记录 | 修改代码、commit、push、创建 corrective PR、Ready、merge、写主实现 worktree |
| `researcher` | 无独立交付身份 | 读取固定来源、使用允许的 Tavily 查询、返回版本/hash/路径/行为证据 | 写规范、写仓库、选 semantic profile、创建交付状态 |
| `scout` | 无独立交付身份 | 读取代码、跟踪调用链、做静态范围调查、返回文件/行号/风险 | 写仓库、commit、push、创建或修改 Issue/PR |

Bot display name 可以从历史 App 名称迁移为 `deliver` 和 `reviewer`，头像使用 GitHub
identicon。仓库文件只记录逻辑身份；App ID、installation ID、private key、token、
broker endpoint 和 secret 不进入仓库、Issue、PR、session metadata 或 prompt。

工具 allowlist 不是安全边界。实际权限由 broker 身份、GitHub App permission、branch
protection 和主会话的 merge owner 共同保证。

# Session Pool Topology

## Warm slots

主会话保持长期运行，正常使用 `/compact`，不主动 `/new`。默认手工预热以下窗口：

```text
deliver-1
deliver-2
reviewer
researcher
scout
```

窗口是可复用的能力槽位，不绑定永久 Issue。两个 deliver 槽位可以同时领取两个互不
冲突的写 lane；reviewer、researcher 和 scout 可以同时工作。并发受 writer scope、
固定 snapshot、远端 gate 和本机资源约束，而不是只按窗口数量放行。

临时 `session_pool.spawn` 只用于短、明确、一次性的任务或热槽故障恢复，不作为主 lane
的常驻来源。managed child 的停止和 coordinator shutdown 语义不得影响手工预热的
常驻窗口。

## Discovery and cwd

session-pool 默认只发现同一 normalized cwd 的 session。所有 warm 和 temporary pool
session 从协调根目录启动：

```text
C:\Users\Admin\Desktop\fcs-project
```

session 的 `worktree` metadata 是 assignment 的绝对路径，不是 session 的启动 cwd。
不要为了让 agent 进入 lane 而把 session cwd 改到 lane worktree；这样会使 coordinator
的默认 `list` 看不到该 session。

仓库路径约定为：

```text
C:\Users\Admin\Desktop\fcs-project\main
C:\Users\Admin\Desktop\fcs-project\worktrees\<issue>-<slug>
<system-temp>\fcs-review-<target>
<system-temp>\fcs-finding-<finding>-<slug>
```

session pool 不创建、删除或锁定这些 worktree。owner、branch、base/head SHA、允许
路径和清理条件必须由 workflow assignment 和 `git worktree list --porcelain` 证明。

## Slot registration and reuse

窗口首次启动或 `/new` 后执行：

```text
/pool-ready deliver-1
/pool-meta role=deliver notes="slot=deliver-1"
```

其他槽位使用对应的 display name 和 role。coordinator 在分配前必须调用
`session_pool.list`，确认 session 新鲜、idle、同 cwd、role/slot 正确，再调用 `assign`。

assignment 结束后，child 先使用 `report` 提交短 handoff，主会话完成远端 Frontier Sync，
确认 worktree 和 assignment 可释放，再调用 `release`。child 随后可以 `/new`，新的
session ID 必须重新登记原 slot；不得把旧 session ID 当作永久身份。

assignment 中途正常禁止 `/new`。崩溃、凭据污染、上下文不可恢复时，child 先报告
worktree、dirty 状态、最近 commit、未完成 acceptance 和恢复点；主会话检查现场后
创建 successor assignment。旧 assignment 不因 session 报告而自动变成成功。

`monitor_start` 只监控 Pi session，不替代 GitHub Action 查询。普通 progress 使用
`queue` 或 `silent`；`needs_attention`、`needs_decision`、failed Action 和最终交付
才使用 `wake`。

# Assignment Contract

每个非机械 assignment 必须绑定一个 Issue 和一个独立 lane，并把以下字段传给 child：

- assignment、Issue、branch、base SHA、worktree 和 slot；
- role、GitHub logical identity、owner 和允许修改路径；
- 规范、ADR、计划、fixture 和 review 的入口；
- numbered acceptance criteria、明确 non-goals 和 dependency residual；
- 本地允许的 fmt/static 命令与必须通过的 GitHub Action evidence；
- 禁止的远端动作、禁止的文件范围和 scope conflict stop condition；
- 输出格式：changed paths、commit、PR、Action URL/run ID/event/headSha/conclusion、
  Primary audit 或 review verdict、blocker 和 Next。

Prompt 只传固定入口和边界，不复制整份规范或主会话历史。child 必须使用 FastCtx
读取仓库文件；报告是交接材料，不是规范或 GitHub 状态的替代品。

## Writer scope

每个 deliver assignment 必须声明 `allowed paths` 和 semantic domain。主会话在 assign
前检查所有 active writer 的路径和 domain：

- 相同文件、同一规范域、同一 PR 的 corrective work 只能有一个 writer；
- `Cargo.toml`、`Cargo.lock`、workspace 根配置、`.github`、规范状态、conformance
  manifest 等全局路径默认 exclusive；
- reviewer、researcher、scout 的只读 lease 可以和 writer 并行；
- reviewer 只能读取固定 SHA。目标 branch 新 push 后，旧 review snapshot、Primary audit
  或 Audit result 失效，必须追加 superseding/re-review；
- scope conflict 冻结相关 lane，不在共享 worktree 强行 reset、rebase 或覆盖他人修改。
  需要合并时另派 integration/corrective deliver lane。

## Frontier Sync

在 assignment 开始/结束、commit/push 前、创建/更新 PR 前、发送 Review requested 前，以及
Ready/review/merge 前，coordinator 或对应 bot 必须做只读 remote sync。至少固定并回读
`origin/main` SHA、Issue/PR/finding、branch/head SHA、workflow/state labels、mergeability、
review decision、required checks、comments、Action runs 和 pending outbox。查询通过相应
Delivery/Review broker 与 `gh --json`/`gh api`/`jq` 完成；不能依赖另一个 session 的即时通知。

远端状态不能确认时，assignment 只能报告 `blocked`/`needs-info`，不得 Ready/merge；同一 SHA 的
瞬时 Action 重跑不消耗 work-unit。新 SHA、scope、命令、依赖 closure 或 acceptance 变化会使旧
Primary/reviewer verdict 失效，必须追加 superseding/re-review。

# Delivery Lifecycle

1. **Select**：主会话同步 `origin/main`、root/child/finding Issue、开放 PR、required
   checks、finding 和当前 worktree，选择最早 dependency-ready bounded Issue。
2. **Prepare**：从最新 `origin/main` 建立 `codex/<issue>-<slug>` 和对应
   `worktrees/<issue>-<slug>`，固定 base SHA、scope、owner 和验收条件。
3. **Assign**：把 assignment contract 发给空闲 deliver 槽位；并行 lane 必须通过 writer
   scope 检查。
4. **Implement**：deliver 只在自己的 worktree 中修改、commit 和静态检查。首次出现
   可审查 commit 时创建 draft PR；PR body 和第一条 Progress 记录稳定契约和 non-goals。
5. **Push and gate**：deliver 使用 Delivery broker push branch。当前 Full Gate 保留
   `push`、`pull_request` 和 `workflow_dispatch`；每次 branch push 都是候选 SHA。
   deliver 等待 Action 并回读 run 的精确 `headSha`。
6. **Evidence**：同一目标 SHA 的任一成功 Full Gate run 可以作为 Full Gate evidence，
   但主会话仍须单独检查 PR required checks。`queued`、`in_progress`、失败、缺失、
   SHA 不匹配或旧 SHA 的 run 都不能写成通过。push 和 PR 产生的重复 run 都必须按实际
   状态记录。
7. **Primary audit**：deliver 在 PR 和 Issue（若有）追加 `Primary audit result`，
   固定 Target、Head SHA、Scope、Commands、Full-gate evidence、Verdict、Findings、
   Gate impact、Limitations 和 Next。Primary audit 不是 reviewer 的独立证据。
8. **Main gate**：主会话审查完整 diff、Issue/PR、required checks、mergeability、
   review threads、scope 和 audit。只有 `pass` 且没有当前 gate 的 Critical/Important
   finding 时，Rancemxn 才能 `gh pr ready` 和 merge。主会话不等待 reviewer 返回后才可
   进行普通 merge，但任何已到达的阻塞 finding 都优先冻结交付。
9. **Review request**：deliver/主会话在 Primary audit 后发送固定 SHA 的 `Review requested`。
   reviewer 异步审查开放 PR 或已合并的固定 commit；后续 push、scope、命令、依赖或
   acceptance 变化使旧请求失效。
10. **Release**：主会话确认远端 handoff 和 worktree 状态后释放 assignment；lane worktree
    只有在 owner 确认 clean、commit/PR 已记录且清理条件满足后才能 remove/prune。

## Local and remote validation

本地只允许：

- `cargo fmt --all -- --check`；
- diff、链接、Markdown/YAML/JSON/schema、路径和结构等不生成 Cargo build artifact 的
  静态检查。

任何 worktree 都不运行本地 `cargo check`、`cargo clippy`、`cargo build`、`cargo test`、
`cargo nextest`、`cargo fuzz` 或 executable fixture。测试、Clippy、build、fuzz 和可执行
fixture 只在 GitHub Actions 的干净 checkout 中执行。Local fmt/static 结果不构成 Full Gate
证据。

纯 workflow/documentation metadata 改动的 Rust Full Gate 为 non-applicable，但仍须运行
适用的 Markdown、链接、YAML/JSON/schema 和 diff 检查。`.github/workflows/full-gate.yml`
本身的执行逻辑改变时，Full Gate 重新适用。

# Review and Finding Lifecycle

reviewer 对固定目标执行 Bind、Reproduce、Inspect、Root-cause analysis、Classify and
route，并在 PR（若存在）和关联 Issue 追加 append-only `Audit result`，即使没有 finding。
reviewer 可以提交 review/comment 和 finding Issue，但不实现、不 push、不创建 corrective PR。

- merge 前到达的当前-stage Critical/Important finding 阻塞 Ready/merge；
- merge 后到达的同等级 finding 不回滚已合并 commit，但冻结受影响 stage claim 和依赖；
- 根因已确认且属于当前 stage 时，主会话派新的 deliver corrective lane；
- 开放 PR 的 corrective lane 从固定 head SHA 建立，base 为活动 PR branch；历史 merged
  commit 的 corrective lane 从最新 `origin/main` 建立，base 为 `main`；
- corrective deliver 通过新的 Full Gate 和 Primary audit 后，由 Rancemxn 审查并 merge，
  新 head SHA 必须送 reviewer re-review；
- Minor 只有在有 owner、follow-up Issue、目标 stage 和解除条件且不影响当前验收时才能延期；
- 只有症状或语义竞争性假设的 finding 不得猜测性修复，必须保留 evidence gap 并路由
  PLANNER/HUMAN；
- 架构、文档和一般优化建议创建 `ready-for-human` HUMAN-only Issue，不进入当前 gate。

reviewer 在 I10 未完成且没有固定 target 时持续 idle wait 和 Frontier Sync；空 frontier
不是成功、失败或终止条件。Review budget 不因等待消耗。

# GitHub Comment Contract

Issue/PR progress, audit and handoff payloads are native English Markdown with real LF line endings.
Prepare the complete body in a file or equivalent safe boundary; do not interpolate raw Markdown into
an unquoted shell/JSON string, and do not send literal `\\n` escapes as visible text. Every payload binds
stable identity fields (role, Issue/PR, event, branch and head SHA), uses an event/state-only H2 without a
handwritten date, and is append-only.

After each mutation, read the returned comment by URL/ID and compare it with the prepared body after only
CRLF-to-LF normalization. A failed read-back, unconfirmed remote write, or identity mismatch is not success;
record `pending remote sync` and do not repeat blindly. To correct a malformed historical comment, append a
`## Superseding ...` comment naming the replaced target, reason, fixed SHA, corrected fields and Next; never
edit or delete the old comment. Primary audit and reviewer Audit result use their fixed field sets from the
assignment contract, even when Findings is `none`.

# Approval Gates

Routine bounded delivery and a merge satisfying all stated gates are allowed only through the role permissions
above. Rancemxn approval is still required for:

| Gate | Examples | Required handling |
|---|---|---|
| Public release | tag, GitHub Release, crate, public bundle or release artifact | stop and obtain explicit approval; record post-release checks |
| Destructive history/data | rewrite/delete history, branch, archive, user or external data | stop; verify exact scope and use a non-destructive route if denied |
| Credential/system mutation | App rename/install/permission change, broker setup, branch protection, paid service, system install | separate Issue and explicit approval; never commit secrets |
| Copyright/license distribution | unclear external chart, audio, image, font or other asset in public output | retain local opt-in evidence only until rights are verified |

A reviewer or deliver bot cannot approve these gates, Ready, merge, close the primary Issue, change its
workflow label, force-push, lower required checks, or update `main`.

# Tool and Credential Boundary

Role is stable across assignments; model, effort, prompt detail, and task-specific read-only context are
selected by the parent coordinator per assignment. Historical model names or old `.pi/settings.json` overrides
are not implicit policy. The coordinator must keep the role capability and tool boundary fixed even when a
different model is selected.

新开的 child session 默认排除直接 `bash`、`find`、`grep` 工具，文件读取和搜索使用
FastCtx `read`、`grep`、`glob`。deliver 可使用受限 `fastctx_run` 做 Git、Delivery
broker、push、PR 和允许的静态命令；reviewer/researcher/scout 默认不使用进程入口，
确需读取固定 Git 状态时必须是只读命令。role-specific tool list 在窗口启动时设置；
`session_pool.spawn` 本身不提供 tool allowlist，因此 temporary child 不得承担需要更
严格角色权限的长期工作。

`fastctx_run` 不是安全沙箱。不得把 tool exclusion 当成 GitHub authorization；broker
身份、App permissions、branch protection 和 Rancemxn merge owner 才是远端安全边界。

不得把 App token/private key 写进 `.pi`、session pool metadata、Issue、PR 或仓库文件。
broker 缺失、认证失败或身份不确定时，停止远端写入并记录 `pending remote sync`；
继续不依赖远端的静态工作。

# Retry, Outbox, and Remote State

GitHub 网络失败只按根 `AGENTS.md` 和 `docs/agents/issue-tracker.md` 的瞬时失败重试规则
处理。每次写入重试前按稳定身份查询，避免重复创建 Issue/PR/comment/review/merge。达到
重试上限后保存完整 payload、稳定身份、最后错误和 `pending remote sync`；未确认的写入
不得描述为成功。

远端 Issue/PR、branch、SHA、Action、review 和 merge 是交付状态权威；session pool 的
assignment completed、child report 或 monitor transition 不能单独关闭 acceptance。

# Finding Contract

A finding binds reviewed Issue/PR/commit and discovery SHA, stable location, violated authority or delivery
clause, minimal reproduction and actual artifact, confirmed root cause or explicit evidence gap, impact,
severity, gate impact, owner, target stage, dependencies, repair boundary and acceptance. Symptoms or competing
semantic guesses are not actionable fixes. Current-stage Critical/Important findings block the affected Ready/
merge or stage claim; post-merge findings freeze dependent work without requiring rollback. Minor can be deferred
only with an owner, follow-up Issue, target stage and removal condition. Architecture/documentation suggestions
become `ready-for-human` HUMAN-only Issues unless evidence upgrades them to a defect, conformance violation or
normative conflict.

# Residual Routing

| Residual | Route | Action |
|---|---|---|
| Action failed, SHA mismatch, missing or still running | LOCAL/WAIT | keep audit blocked; inspect exact run; retry only transient infrastructure failures |
| Code/config/test failure | LOCAL | fix in a new commit/SHA and rerun applicable gate; never use local results as proof |
| New current-stage Critical/Important finding | LOCAL | freeze lane/Ready/merge; create finding and assign corrective deliver |
| Root cause or repair boundary is uncertain | PLANNER/HUMAN | preserve evidence gap; do not submit a guess-based patch |
| Two writers overlap or worktree is foreign/dirty | LOCAL | freeze; create integration/corrective lane; never reset or force-remove |
| Normative/profile choice has materially different valid outcomes | HUMAN | present evidence/options/impact; stop dependent implementation |
| Remote write or broker fails after retry budget | LOCAL/WAIT | preserve payload, stable identity, error and `pending remote sync`; do not claim success |
| Credential, system, release, destructive or license action | HUMAN | apply the matching Approval Gate before acting |

# Worktree Cleanup

Review and corrective snapshots use unique paths under the host system temp directory (`<system-temp>\fcs-review-*`
or `<system-temp>\fcs-finding-*`), with owner and fixed SHA recorded; normalize paths before checking
containment, and do not use symlink, junction, reparse point or `subst` to evade isolation. The deliberate
repository `refer` boundary is governed by root `AGENTS.md` and is not a license to share an implementation
worktree. lane、review snapshot 和 corrective snapshot 都必须有 owner、用途、固定 base/head SHA、
允许范围和清理条件。owner 先确认 `git status --porcelain` 为空，再执行适用的
`git worktree remove` 和 `git worktree prune`，最后用 `git worktree list --porcelain`
复核。不得 `--force` 删除 dirty worktree，不得删除未 push commit、未记录 artifact 或
其他会话拥有的 worktree。包含当前已有 dirty changes 的协调根目录和既有 worktree 不在
本 work unit 的清理范围内。

# Verification of This Contract

修改本文件、ADR 或 active workflow references 后，至少运行：

```text
markdownlint-cli2 --config <temporary-config> docs/loops/fcs5-session-pool-delivery.md docs/decisions/0014-session-pool-delivery.md
<applicable link/reference and YAML/JSON/schema checks>
git diff --check
git status --short
```

该 workflow-only work unit 不运行本地 Cargo test/build/lint，也不把旧 Action、旧 SHA 或
本地结果冒充当前 Full Gate evidence。
