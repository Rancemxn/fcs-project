# Superseded Review Loop Pointer

状态：Superseded

此路径曾保存独立 reviewer loop。为保持历史计划、review 和外部链接可解析，文件暂时保留为
pointer；它不再授予 reviewer 写代码、创建 corrective PR 或管理 worktree 的权限。

当前唯一 active delivery and review contract：

- [`fcs5-session-pool-delivery.md`](fcs5-session-pool-delivery.md)
- [`ADR 0014`](../decisions/0014-session-pool-delivery.md)

当前 `reviewer` 只审查固定 SHA、comment/request changes 和创建 finding；已确认 finding
由新的 isolated `deliver` corrective lane 实现，最终由 Rancemxn 审查并合并。不要从本 pointer
或历史引用恢复旧 reviewer-writer 模型。
