# FCS Architecture Decision Records

ADR 文件是 append-only 的历史决定记录。编号不保证唯一；`0012` 已被两个历史决定使用。引用 ADR 时
必须给出文件名或完整标题，不能只写编号。后续决定改变既有 ADR 时，新增 ADR 并在索引和旧记录中
标明 supersession，不重编号或静默改写历史文件。

| 编号 | 文件 | 标题 | 状态 |
| ---: | --- | --- | --- |
| 0001 | [`0001-single-runtime-clock.md`](0001-single-runtime-clock.md) | 运行时只有一个物理主时钟 | Accepted |
| 0002 | [`0002-judgment-time-and-scroll-coordinate.md`](0002-judgment-time-and-scroll-coordinate.md) | 分离判定时间与滚动坐标 | Accepted |
| 0003 | [`0003-compile-time-structure-only.md`](0003-compile-time-structure-only.md) | Template 和 Generator 只存在于编译期 | Accepted |
| 0004 | [`0004-independent-version-domains.md`](0004-independent-version-domains.md) | FCS、FCBC、Execution ABI 和 Render 独立版本化 | Accepted |
| 0005 | [`0005-error-bounded-numerics.md`](0005-error-bounded-numerics.md) | 数值正确性由误差界而非固定采样率定义 | Accepted（默认 baking 范围由 0009 收窄） |
| 0006 | [`0006-unversioned-source-cutover.md`](0006-unversioned-source-cutover.md) | 归档 FCS 4 并将无版本前缀的 source crate 作为唯一主线 | Accepted |
| 0007 | [`0007-versioned-conversion-semantic-profiles.md`](0007-versioned-conversion-semantic-profiles.md) | 外部谱面格式使用显式、版本化的语义 Profile 解释 | Accepted |
| 0008 | [`0008-fcs-authoring-fcbc-distribution-boundary.md`](0008-fcs-authoring-fcbc-distribution-boundary.md) | FCS 是制谱源格式，FCBC 是自包含分发与执行容器 | Accepted |
| 0009 | [`0009-player-local-baking-shared-runtime.md`](0009-player-local-baking-shared-runtime.md) | 精确表达式默认执行，烘焙仅是播放器本地可选策略 | Accepted |
| 0010 | [`0010-stage-scoped-implementation-baselines.md`](0010-stage-scoped-implementation-baselines.md) | 使用阶段范围化 Reviewed Implementation Baseline 启动实现 | Partially superseded by [`0011-github-issue-pr-workflow.md`](0011-github-issue-pr-workflow.md)（仅工作流追踪介质） |
| 0011 | [`0011-github-issue-pr-workflow.md`](0011-github-issue-pr-workflow.md) | 使用 GitHub Issue 与 Pull Request 交付工作 | Partially superseded by [0015](0015-portable-contributor-workflow.md)（任务、进度及角色规则） |
| 0012 | [`0012-canonical-textual-id-encoding.md`](0012-canonical-textual-id-encoding.md) | Canonical textual ID 编码与 typed stable ID 冲突策略 | Accepted |
| 0012 | [`0012-public-runner-private-gates.md`](0012-public-runner-private-gates.md) | 使用公开执行仓库运行私有项目完整门禁 | Superseded by [`0013-public-project-full-gate-ci.md`](0013-public-project-full-gate-ci.md) |
| 0013 | [`0013-public-project-full-gate-ci.md`](0013-public-project-full-gate-ci.md) | 在公开项目仓库运行完整门禁 | Partially superseded by [0015](0015-portable-contributor-workflow.md)（仅全员本地执行限制） |
| 0014 | [`0014-session-pool-delivery.md`](0014-session-pool-delivery.md) | FCS5 Session-Pool Delivery Workflow | Superseded by [0015](0015-portable-contributor-workflow.md) |
| 0015 | [`0015-portable-contributor-workflow.md`](0015-portable-contributor-workflow.md) | 公共协作规则与本机开发约定分离 | Accepted |
