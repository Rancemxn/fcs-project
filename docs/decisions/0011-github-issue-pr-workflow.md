# 0011：使用 GitHub Issue 与 Pull Request 交付工作

状态：Accepted

日期：2026-07-17

## 1. 背景

ADR 0010 允许 `docs/scratch/` 保存当前有限工作单元与 frontier，但本地 Markdown tracker 无法直接提供远程协作、状态查询、依赖关系、分支关联、CI 门禁和合并审计。仓库已配置 GitHub remote，`gh` 已登录，因此可以使用同一个远程系统连接工作契约、实现分支、验证证据与复审结果。

Issue/PR 的便利性也可能模糊资料权威：Issue 中的验收条件、PR 中的实现或已通过的 CI 都不能代替根规范、conformance corpus、dated review 或 Reviewed Implementation Baseline。

## 2. 决策

- GitHub Issues 是当前工作契约、依赖关系、状态与验收条件的唯一当前 tracker；不再使用 `docs/scratch/` 作为当前 request surface 或 frontier。
- Pull Requests 交付一个可独立审查的工作单元，并链接对应 Issue、验证命令、规范/ADR/conformance/review 影响与剩余风险。
- 分支从 `main` 创建，使用 `codex/<issue>-<slug>` 命名；Issue 使用 parent/sub-issue 和 blocked-by/blocking 表达分解与依赖。
- 使用 `gh` 读写 GitHub 状态；程序化检查使用 `gh --json` 或 `gh api` 输出，由 `jq` 投影、聚合或以 `jq -e` 形成门禁。
- `gh` 只在 DNS、超时/连接重置、TLS 中断或 HTTP 502/503/504 等瞬时网络失败时每隔 5 秒重试，最多重试 5 次。写操作重试前必须先查询远程是否已成功；认证/权限、校验、not found、冲突或门禁失败不属于可重试网络故障。当前上限与耗尽后的处理已由第 6 节 dated amendment 修订。
- Issue、PR、label、comment、branch 和 CI 只有工作流与实施证据职责，不获得规范权威。规范修订、fixture/manifest 绑定、dated review、baseline 和 Frozen gate 仍按 `docs/specifications/governance.md` 执行。

## 3. 后果

正面后果：

- 工作范围、依赖、分支、提交、CI、review 和 merge 共享可查询的远程审计链；
- Issue 与 PR 可以在多个 agent/人类会话之间保持稳定引用；
- 结构化 JSON 避免 agent 依赖面向人的 CLI 表格输出。

成本与约束：

- 创建、编辑、评论、push、review、close 和 merge 都是外部状态变更，必须在用户授权的工作流范围内执行；
- 断网或 GitHub 故障时可以继续安全的本地实现与验证，但不得伪造远程 Issue/PR 状态；
- 历史 `docs/scratch/` 内容若需迁移，必须保留来源与决定语境，不得把临时记录冒充为规范或 review 证据。

## 4. 明确禁止

- 不得用 Issue/PR 正文或评论替代根规范、Accepted ADR、conformance fixture/manifest 或 dated review。
- 不得为了合并而删除失败 fixture、降低测试、改写历史 finding 或隐藏 open blocker。
- 不得使用 `gh pr merge --admin` 绕过 branch protection 或 required checks。
- 不得解析 `gh` 的面向人表格作为自动化接口；必须使用结构化 JSON。

## 5. 2026-07-17 dated amendment：Issue/PR 进度叙事

用户补充接受：Issue 和 PR 不得只保留初始对话、空模板、零散评论或原始 commit 列表。非机械 Issue 必须在正文中以 `Progress` 记录工作契约、有意义检查点、证据、决定、阻塞与下一步；PR 必须按 commit/变更组解释完成内容与原因，并在重要 push 后和转 Ready 前保持正文与当前 diff 一致。

进度以有意义工作单元为粒度，不强制每个 commit 一条。评论可以保留讨论时序，但不能替代正文中的当前可信摘要。Issue/PR 进度仍只是工作流证据，不得替代根规范、conformance artifact、dated review 或 implementation baseline。

## 6. 2026-07-17 dated amendment：分条进度消息与延后远端同步

用户进一步修订第 2、5 节：Issue/PR 正文只保存稳定的初始契约和一条实质性初始 Progress。之后每个
有意义检查点分别发送一条新 comment，不再把全部进度累计到正文或旧评论中，也不为日常进度反复
edit 同一个消息。每条消息仍包含 Completed、Evidence、Decisions、Blockers 和 Next；若旧消息需要
更正，发送明确指出被替代内容的 superseding comment，保留原消息作为历史。delivery-ready 与
final merged checkpoint 也分别使用新消息。该决定取代第 5 节关于正文持续维护当前摘要、评论不能
替代正文的要求；commit 列表仍不能替代进度叙事。GitHub Progress 消息标题只写事件或状态，不
手写日历日期；时间由消息自带的 timestamp 记录。治理文件自身的 dated amendment 不受此消息格式
规则影响。

瞬时网络失败仍每隔 5 秒重试，但首次失败后最多再试 10 次，取代第 2 节的 5 次上限。每次重试写
操作以及稍后补同步前，都必须先按稳定身份查询远端，避免重复创建、评论、review 或 merge。10 次
耗尽后，保存完整 payload、稳定身份、最后错误和 `pending remote sync` 标记，继续所有不依赖该远端
结果的安全本地工作；在下一个有意义检查点，以及 handoff、PR Ready、review、merge 等依赖远端状态
的动作前再次查询并尝试同步。该本地记录只是 transport outbox，不是第二个 tracker；未确认的远端
动作不得描述为成功，依赖远端前置状态的外部转换必须延后。

## 7. 2026-07-17 dated amendment：独立审查会话与 corrective branch

用户进一步接受两个并行但权限分离的工作角色：当前主实现会话和独立审查会话。不创建第三个可选实现
会话。当前会话是唯一实现者、唯一可以将 PR 标记为 Ready 的角色和唯一 merge owner；审查会话使用独立
的 `docs/loops/review-loop.md`，可以读取固定快照、comment、review/request changes、创建 bug/finding Issue 以及
为已记录 finding 创建 corrective PR，但不得合并任何 PR、标记 Ready、关闭主 Issue、修改主 Issue
workflow label，或写入当前会话的工作树、活动实现分支和 `main`。审查者不得审查或批准自己创建的
corrective PR；当前会话负责检查、审查和合并它。

所有非机械实现 PR 在 Ready/merge 前必须有绑定 `Issue/PR 或 commit + head SHA + scope + commands +
acceptance gate` 的独立审查。审查结束后，审查者必须立即在被审 PR（若存在）和关联 Issue 各追加一条
append-only `Audit result`，即使零 finding；后续 push、scope、命令、依赖 closure 或验收变化会使旧 verdict 失效，
必须追加 superseding/re-review 消息并以新 SHA 重新审查。Critical/Important finding 阻塞主 PR；Minor
只能按有 owner、follow-up Issue、目标 stage 和解除条件的延期规则处理。

corrective PR 必须使用独立 worktree 和 `codex/<finding>-<slug>` 分支。开放 PR 的 finding 从被审 PR 的
固定 head SHA 创建分支，PR base 为该活动 PR 分支；主会话在审查期间不推进活动分支，修复合并后对新
head SHA 重新审查。历史已合并 commit 的 finding 从最新 `origin/main` 创建分支，PR base 为 `main`，
不重新打开原 PR。该分支策略是对第 2 节“分支从 `main` 创建”约束的明确例外；其余 Issue/PR、重试、
进度消息和规范权威边界保持不变。
