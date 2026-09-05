# FCS 文档入口

仓库根 [AGENTS.md](../AGENTS.md) 保存共享协作规则。本目录保存项目资料；按当前任务读取相关条目，
不要求每次修改前通读全部文档。

| 路径 | 职责 |
| --- | --- |
| [CONTEXT.md](CONTEXT.md) | FCS 项目术语 |
| [specifications/](specifications/governance.md) | FCS Core、FCBC/Execution ABI、Render、Conversion 规范与治理 |
| [conformance/](conformance/README.md) | 机器可读 manifest、fixture、golden、mutation 与覆盖记录 |
| [decisions/](decisions/README.md) | Accepted ADR、设计理由与决定修订历史 |
| `plans/` | 总路线图与阶段实施计划 |
| `reviews/` | 固定范围、hash、复现命令及独立复审证据 |
| [agents/](agents/issue-tracker.md) | 按任务读取的领域、GitHub 交付与 triage 规则 |
| [community/](community/README.md) | 外部谱面格式的来源综合与歧义索引 |
| [scratch/](scratch/README.md) | 历史临时记录，不作为当前状态来源 |

规范定义语义，治理文件定义状态，Accepted ADR 约束设计方向。实现、测试、Issue、PR、计划与
review 不能自行创造规范语义。规范变化才读取相应治理流程；普通文档调整只核对受影响内容和引用。
当前共享协作方式见 [ADR 0015](decisions/0015-portable-contributor-workflow.md)；
旧工作流保存在 Git 历史和标记为已取代的 ADR 中。
