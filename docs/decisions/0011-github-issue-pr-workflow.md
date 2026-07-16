# 0011：使用 GitHub Issue 与 Pull Request 交付工作

状态：Accepted

日期：2026-07-17

## 1. 背景

ADR 0010 允许 `.scratch/` 保存当前有限工作单元与 frontier，但本地 Markdown tracker 无法直接提供远程协作、状态查询、依赖关系、分支关联、CI 门禁和合并审计。仓库已配置 GitHub remote，`gh` 已登录，因此可以使用同一个远程系统连接工作契约、实现分支、验证证据与复审结果。

Issue/PR 的便利性也可能模糊资料权威：Issue 中的验收条件、PR 中的实现或已通过的 CI 都不能代替根规范、conformance corpus、dated review 或 Reviewed Implementation Baseline。

## 2. 决策

- GitHub Issues 是当前工作契约、依赖关系、状态与验收条件的唯一当前 tracker；不再使用 `.scratch/` 作为当前 request surface 或 frontier。
- Pull Requests 交付一个可独立审查的工作单元，并链接对应 Issue、验证命令、规范/ADR/conformance/review 影响与剩余风险。
- 分支从 `main` 创建，使用 `codex/<issue>-<slug>` 命名；Issue 使用 parent/sub-issue 和 blocked-by/blocking 表达分解与依赖。
- 使用 `gh` 读写 GitHub 状态；程序化检查使用 `gh --json` 或 `gh api` 输出，由 `jq` 投影、聚合或以 `jq -e` 形成门禁。
- `gh` 只在 DNS、超时/连接重置、TLS 中断或 HTTP 502/503/504 等瞬时网络失败时每隔 5 秒重试，最多重试 5 次。写操作重试前必须先查询远程是否已成功；认证/权限、校验、not found、冲突或门禁失败不属于可重试网络故障。
- Issue、PR、label、comment、branch 和 CI 只有工作流与实施证据职责，不获得规范权威。规范修订、fixture/manifest 绑定、dated review、baseline 和 Frozen gate 仍按 `docs/specification-governance.md` 执行。

## 3. 后果

正面后果：

- 工作范围、依赖、分支、提交、CI、review 和 merge 共享可查询的远程审计链；
- Issue 与 PR 可以在多个 agent/人类会话之间保持稳定引用；
- 结构化 JSON 避免 agent 依赖面向人的 CLI 表格输出。

成本与约束：

- 创建、编辑、评论、push、review、close 和 merge 都是外部状态变更，必须在用户授权的工作流范围内执行；
- 断网或 GitHub 故障时可以继续安全的本地实现与验证，但不得伪造远程 Issue/PR 状态；
- 历史 `.scratch/` 内容若需迁移，必须保留来源与决定语境，不得把临时记录冒充为规范或 review 证据。

## 4. 明确禁止

- 不得用 Issue/PR 正文或评论替代根规范、Accepted ADR、conformance fixture/manifest 或 dated review。
- 不得为了合并而删除失败 fixture、降低测试、改写历史 finding 或隐藏 open blocker。
- 不得使用 `gh pr merge --admin` 绕过 branch protection 或 required checks。
- 不得解析 `gh` 的面向人表格作为自动化接口；必须使用结构化 JSON。
