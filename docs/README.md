# FCS 文档入口

根目录只保留 `AGENTS.md` 作为协作规则入口。本目录保存规范、上下文、实施计划、审查证据、conformance
资料和 agent workflow；不要从 Issue、PR、实现或 scratch 记录推导新的规范语义。

## 目录

| 路径 | 职责 |
|---|---|
| `CONTEXT.md` | FCS 项目术语和 single-context 词汇 |
| `specifications/` | FCS Core、FCBC/Execution ABI、Render、Conversion 规范与治理 |
| `conformance/` | 机器可读 manifest、fixture、golden、mutation 和覆盖 ledger |
| `decisions/` | Accepted ADR、架构边界和治理 amendment |
| `plans/` | 总路线图与阶段实施计划 |
| `reviews/` | 固定范围、hash、复现命令和独立复审证据 |
| `agents/` | 领域阅读、GitHub Issue/PR 和 triage 规则 |
| `loops/` | 主实现 loop 与独立审查 loop 契约 |
| `community/` | PGR、RPE、PEC 等外部格式证据综合 |
| `scratch/` | 历史临时记录；不作为当前 request surface 或状态来源 |

## 权威顺序

规范语义由 `specifications/` 定义；规范状态由 `specifications/governance.md` 管理；Accepted ADR 只
约束设计方向；`conformance/`、`plans/`、`reviews/`、Issue、PR、实现和测试只能安排或证明工作。

修改任何领域文件前先阅读根 `AGENTS.md`、本目录中对应入口、相关 ADR、计划、fixture 和复审资料。

本次目录重构只调整文件路径；规范、fixture、golden、manifest 内容及既有 hash 不变。历史 review/plan 中的
路径文字同步到当前 `docs/` 布局，但不改变其原有结论或审计事实。
