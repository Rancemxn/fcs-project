# FCS 仓库协作指南

本文件是所有贡献者共享的项目规则；更近的目录规则适用于其对应范围。任务中的明确授权和范围优先，
已有授权无需逐步重复确认。保护与当前任务无关的修改。

## 项目与资料入口

默认分支为 `main`，Rust workspace 使用 edition 2024。目录以 [Cargo.toml](Cargo.toml) 为准：

| 路径 | 职责 |
| --- | --- |
| `crates/fcs-source` | FCS source 解析与编译 |
| `crates/fcs-model`、`crates/fcs-runtime` | Canonical model 与执行语义 |
| `crates/fcs-fcbc` | FCBC 容器、加载与分发 |
| `crates/fcs-conversion` | 外部格式导入、导出及转换报告 |
| `crates/fcs-render`、`crates/fcs-cli` | Render Profile 与产品命令行 |
| [docs/README.md](docs/README.md) | 规范、ADR、计划、conformance 与 review 索引 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 许可证与提交所需的 DCO sign-off |
| `examples/`、`fuzz/` | 输入示例与 fuzz 目标 |

旧 FCS 4 实现保留在 `archive/fcs4-pre-cutover` 分支；不作为活动依赖或兼容层来源。
`refer/` 可保存只读参考快照，不是必备开发环境，也不得成为 Cargo path dependency。

## 按任务读取

首次进入相关领域时定位需要的条款、调用方和测试；已读且未变化的资料不重复通读。
普通文案、机械修改和只读问答不需要完整规范、ADR、计划和 review 的前置阅读。

| 任务 | 按影响范围读取 |
| --- | --- |
| Source、static、canonical、runtime 行为 | [FCS Core](docs/specifications/fcs.md) 的相关条款与对应 fixture |
| FCBC、loader、packager、ABI | [FCBC / Execution ABI](docs/specifications/fcbc.md) 及相关 Core 条款 |
| Render 与资源解析 | [Render Profile](docs/specifications/fcs-render.md) 及相关 Core/FCBC 条款 |
| 外部格式、semantic profile、ConversionReport | [Conversion Specification](docs/specifications/fcs-conversion.md)、[community 索引](docs/community/README.md) 和对应格式证据 |
| 规范变更、阶段 baseline、Frozen 声明 | [规范治理](docs/specifications/governance.md)、受影响 ADR、manifest 与 review |
| 术语、设计冲突或陌生领域边界 | [domain.md](docs/agents/domain.md) 指向的相关术语与决定 |
| Issue、PR、提交、审查或 worktree 整理 | [交付规则](docs/agents/issue-tracker.md) 的对应章节 |

依赖/API 工作先核对 `Cargo.toml` 与 `Cargo.lock`；现有代码或匹配版本资料不能解决疑问时，
再查询该版本的官方资料。区分固定版本与上游最新版本，不强制特定搜索服务。
Skills 仅在任务匹配且当前环境可用时使用；不要求安装个人技能集合，也不因技能缺失阻塞普通任务。

## 权威与语义边界

- 根规范定义项目行为；治理文件管理状态；Accepted ADR 约束设计方向。实现、测试、计划、
  Issue 和审查报告只能安排或证明工作。ADR 索引位于 [docs/decisions/](docs/decisions/README.md)。
- 实现与规范冲突时先判断缺陷所在。实质规范冲突按治理流程更新受影响条款与 conformance，
  暂停依赖该语义的工作；无关任务继续，不能把局部问题扩大为全项目冻结。
- 修改既有决定时记录明确的修订或后继 ADR，不静默改写历史。常规措辞和工具偏好整理不触发规范重开。
- 外部格式结论应绑定项目、commit/hash、路径及行为；本地快照和上游固定版本均可作为来源。
  新的通用结论或歧义选择应交叉核对独立来源，并使用显式、版本化的 semantic profile。
- Strict mode 不猜测未定义语义；repair 不替用户选择多个合法解释。规范未授权的选择须明确记录，
  仅停止依赖该选择的实现。不要通过放宽 fixture、诊断或输入校验使检查变绿。

## 验证

| 改动 | 所需证据 |
| --- | --- |
| 只读审查 | 相关文件或状态的核对结果；无需启动构建 |
| 普通文档、协作规则、模板等不影响执行的元数据 | `git diff --check`、相关链接及适用格式检查；Rust Full Gate 不适用 |
| Rust、依赖、构建、测试、可执行 fixture 或 gate 执行逻辑 | 有意义的针对性验证；交付前取得最终目标 SHA 的 GitHub Full Gate 成功记录 |
| 规范、manifest、golden 或 conformance 变化 | 条款与预期绑定检查，以及受影响的执行验证和治理审查；不能仅按文件扩展名或 `docs/` 路径豁免 |

完整 Rust 门禁的命令以 [.github/workflows/full-gate.yml](.github/workflows/full-gate.yml) 为准，
包含 locked dependency、fmt、Clippy、nextest、bounded fuzz 和干净工作树检查。
开发反馈选最小有意义的检查；回归修复留下能复现原问题的测试或 fixture。
本机若不适合运行相关检查，使用远端执行；本地机器的资源限制不作为全体贡献者的禁令。

修复本次改动导致的失败并复查受影响范围。新变化、失败或未解决风险才需要追加验证，
不要无理由重复已经有效的检查。交付证据必须说明命令、结果和限制；
适用 Full Gate 必须核对 run URL、ID、event、`headSha` 与 conclusion。
旧 SHA、pending、失败和不适用都不能写成通过；CI 成功不等于独立审查、Frozen 或发布授权。

## 执行与完成

- 本地任务可以用用户请求界定范围；长期、跨阶段或需要协作跟踪的交付使用 Issue。
  一个 PR 保持一个可审查工作单元，提交前检查实际 diff 并遵守 DCO。
- 在已授权范围内继续读取、实现、验证、修复和整理，不在首版实现后提前停工。
  GitHub 写入、合并和发布遵守已有授权；未授权的外部动作在结果可审查后集中确认。
- 本地任务完成：验收满足、适用检查完成、由本次改动造成的问题已处理、剩余限制明确。
  PR 交付还需完成已授权的推送、CI 和审查步骤；合并、阶段闭合、Frozen 与发布各自按其门槛判断。
- 只读审查以有依据的报告结束。缺少信息时继续可独立推进的部分，只询问确实影响后续决定的问题。
- 保护未提交、未推送和其他贡献者的成果；仅清理自己创建且满足交付与保留条件的临时资源。
  不绕过分支保护，不静默重写历史，不提交凭据或权利不明的外部素材。

## 可选的本地补充

个人工具、操作系统和资源限制不写入此文件。用 `git rev-parse --path-format=absolute --git-common-dir`
定位 Git 公共目录；如其 `info/AGENTS.local.md` 存在，在首次选择本地工具或执行验证前读取。
该文件由所有本地 worktree 共用且不受 Git 跟踪；不存在时正常工作，无需创建或安装任何工具。
本地补充只约定环境，不改变项目语义、验收标准或任务中的明确授权。
