# 0014：FCS5 Parallel PR Delivery 命名冻结工作流

状态：Accepted

日期：2026-08-09

部分取代：ADR 0011（GitHub Issue/PR 工作流）的 §7–§9 与执行契约文件列表；ADR 0013
（公开项目仓库完整门禁）的 §2.1 触发器条款。ADR 0011 §5–§6（进度消息、重试/outbox）与
ADR 0013 §4（禁止条款）保持有效。

## 1. 背景

`docs/loops/loop.md` 与 `docs/loops/review-loop.md` 定义了主实现会话与独立审查会话两个角色，
但把实现、审查、Ready、merge 全部绑定在单一持久会话和固定文件上：本地编译被放开为开发反馈、
full gate 依赖 PR 自动触发或 Codespace 直接执行、工作流变更必须逐处改写两份 loop 契约。随着
Delivery App 与 Review App 已安装并固定，交付需要改为由父编排者调度的动态并行 lane：一个
Issue-first lane 一个 writer，异步独立复审，main/Ready/merge 只保留给 Rancemxn。

本 ADR 只改变协作与交付基础设施，不改变 FCS、FCBC、Render、Conversion、fixture 或实现语义
（沿用 ADR 0012 §3 / ADR 0013 §3 的表述）。

## 2. 决策

- 创建**命名冻结工作流** `docs/loops/fcs5-parallel-pr-delivery.md` 作为唯一交付契约，吸收并取代
  `docs/loops/loop.md` 与 `docs/loops/review-loop.md`；删除两份旧文件并消除全部悬空引用。该
  冻结是工作流冻结，不是规范版本域 `Frozen`（后者仍只由 `docs/specifications/governance.md`
  管理）。实质工作流变更必须产出新的命名工作流文档和 superseding ADR，勘误以 dated errata 记录。
- **动态 Issue-first lane：** branch `codex/<issue>-<slug>`，lane worktree
  `C:/Users/Admin/Desktop/fcs-project/worktree/<issue>-<slug>`，base 为最新 `origin/main`，一个
  lane 一个 writer；lane 使用 sparse checkout 排除 `refer` 并建立只读 `refer` junction 指向
  `C:/Users/Admin/Desktop/fcs-project/main/refer`。父编排者拥有
  `C:/Users/Admin/Desktop/fcs-project`，动态调度互不冲突的 lane，综合审查结果，排序 merge，
  并把冲突委托给独立 corrective lane。跨 junction 禁止 `git clean -fdx`/`-fdX`。
- **权限：** `Rancemxn` 是唯一可以更新 `main`、标记 Ready、合并 PR 的角色；Delivery App
  （`fcs5-delivery-rancemxn[bot]`）拥有 Issue、lane push、draft PR 与交付进度（含
  `Primary audit result`）；Review App（`fcs5-review-rancemxn[bot]`）拥有固定 SHA 审查、
  audit/finding 评论，绝不实现或 push。两个 Bot 都不能绕过 main 规则。
- **测量域：** 禁止一切本地编译、lint、测试、fuzz 与可执行 fixture 运行；只允许
  `cargo fmt --all -- --check` 与非编译静态检查。本地结果永远不是门禁证据。Rust 证据只来自
  对候选 SHA 的 Action run。
- **full gate 触发器：** `.github/workflows/full-gate.yml` 改为 `workflow_dispatch` only，移除
  自动 `pull_request` 与 `push main` 触发器；保留 exact-SHA 验证步骤与 ADR 0013 的完整命令
  序列。成功且同 SHA 的候选 SHA run 是 Ready/merge 前的强制前置条件。`weekly-fuzz.yml` 保持
  schedule 触发、明确 non-gate、不变。Codespace 直接 Full Gate 条款退役，由候选 SHA dispatch
  取代。
- **工具：** FastCtx 是规范性文件访问/搜索/替换工具；通用 Bash 默认禁止，只有经显式命名批准
  的进程（原生 Windows Git、App broker、静态校验器、`cargo fmt`）例外。
- **Subagent 配置：** 项目 `.pi/settings.json` 固定全部内置 agent 为
  `opencode-go/deepseek-v4-flash`，thinking 默认 `max`（简单有界任务 `high`，纯机械任务 `off`），
  直接 FastCtx 工具 allowlist，reviewer/scout 只读、worker writer，researcher 保留 Tavily。
- **替换持久会话：** 不再保留单一持久实现/审查会话；父编排者在隔离 lane 中启动独立 Pi
  实例/subagent，Review App 异步二审，全部并行子任务不超过三个。

## 3. 后果

正面后果：

- 并行 lane 消除单会话瓶颈；Issue-first + 一个 writer 保证每 lane 可独立验收；
- App 身份使远端状态可审计，Bot 无 main/Ready/merge 权限降低越权风险；
- 候选 SHA dispatch 使每个 gate 证据绑定精确 commit，撤除自动触发器减少噪音 run。

成本与约束：

- 父编排者必须维护 lane 调度与冲突避免；lane 数量与并行度受「不超过三个并行子任务」约束；
- 本地不再有任何编译反馈，迭代依赖远端候选 SHA run，单次反馈延迟增加；
- 两个 Bot 的写入面受 GitHub App scope 限制，无法覆盖的操作必须由 Rancemxn 执行。

## 4. 明确禁止

- 不得把本工作流冻结描述为规范 `Frozen`，也不得用它改变任何版本域状态。
- 任何人（含两个 Bot）除 Rancemxn 外不得更新 `main`、标记 Ready 或合并 PR。
- 不得重新加入 full-gate 的自动 `pull_request`/`push main` 触发器。
- 不得在任何 lane 或 corrective worktree 运行本地编译、lint、测试、fuzz 或可执行 fixture，也不得
  把本地结果写成 gate evidence。
- 不得对包含 `refer` junction 的路径执行 `git clean -fdx`/`-fdX`，不得用 junction 绕过 reviewer
  隔离的 `/tmp` 约束。
- 不得恢复 Codespace 直接 Full Gate 条款或 ADR 0012 的外部执行仓库机制。
- 不得用 Action success 冒充规范性 conformance、独立 reviewer verdict 或 merge 授权。
