# FCS 仓库协作指南

本文件适用于整个仓库。开始工作前先阅读本文件；如果子目录中出现更具体的 `AGENTS.md`，以距离目标文件更近的规则为准。不要覆盖或回退工作区中已有的、与当前任务无关的修改。

## 仓库结构与权威资料

- 当前工作树已完成 I0-A 快照与分支切换：`master` 是活动开发分支，
  `archive/fcs4-pre-cutover` 保存完整切换前工作树；活动树暂时仍包含 `fcs-core`、旧
  `fcs-converter` 和旧 `fcs-cli`，因为 I0-B 的唯一 source crate 切换尚未开始。已确认的
  I0 目标由 `docs/decisions/0006-unversioned-source-cutover.md` 定义，逐步执行见
  `docs/plans/i0-source-cutover.md`：
  - I0-B 删除活动 FCS 4、旧 converter 和旧 CLI；
  - `crates/fcs-core/src/v5` 提升为唯一、无版本前缀的 `crates/fcs-source`；
  - 后续 canonical/runtime/FCBC/converter/render/CLI crate 到对应路线阶段按需创建。
- I0 不保留 `fcs_core`、`v5` module、feature flag 或兼容 re-export。`refer/chumsky` 只用于
  审阅 Chumsky 源码；Cargo 不得使用指向 `refer/` 的 path dependency。
- `fcs.md` 是 FCS 5 Core source、canonical model 和运行时语义的权威规范。
- `fcbc.md` 是 FCBC 2 容器与 Execution ABI 的权威规范；`fcs-render.md` 和
  `fcs-conversion.md` 分别定义 Render Profile 与转换/保真行为。
- `docs/specification-governance.md` 定义规范状态和变更流程，
  `docs/plans/fcs5-roadmap.md` 是唯一总实施路线图；当前阶段详细计划为
  `docs/plans/i0-source-cutover.md`。`docs/decisions/` 只记录设计理由，不得覆盖四份权威规范。
- 涉及格式行为的改动先对照对应权威规范，再检查 parser、compiler、runtime、converter 和
  conformance fixture；实现现状不能静默成为新规范。
- `examples/` 保存各格式输入样例；I0 删除活动 FCS 4 examples，但保留 PGR/RPE/PEC 与版权
  输入，供未来 converter 重建时复用。旧 converter 测试由归档分支保存，不迁移到 source crate。
- `CLAUDE.md` 中的项目级约定同样适用；本文件对搜索工具和 Context7 的规则作明确补充。

## 搜索与代码理解

按用途选择搜索工具，优先从仓库根目录开始，并排除 `.git` 和生成目录：

- 用 `fd` 找文件和目录：

  ```text
  fd --hidden --exclude .git --type f
  fd --hidden --exclude .git AGENTS.md
  ```

- 用 `rg` 搜索文本、符号、错误信息和配置：

  ```text
  rg -n --hidden -g '!/.git' 'pattern' .
  rg -n 'parse_document|nextest|Context7' crates docs
  ```

- 用 `sg`（ast-grep）搜索 Rust 的语法结构；当空格、换行或具体变量名不应影响匹配时，优先于纯文本搜索：

  ```text
  sg run -l rust -p 'use $A;' crates
  sg run -l rust -p 'fn $NAME($$$ARGS) $$$BODY' crates
  ```

先用 `fd` 定位范围，再用 `rg` 或 `sg` 缩小目标。阅读实现时同时查看调用方、对应测试和
相关规范，避免只根据单个匹配结果推断行为。目标项目检查默认排除 `refer/`；只有明确研究
参考实现时才进入该目录，并先检查参考仓库自己的规则和版本。

## Rust 开发与验证

- 日常编译和测试不要使用 `--release`。
- 项目使用 `cargo-nextest` 运行测试，不要把普通 `cargo test` 当作默认测试命令。
- 每次运行测试前先执行 Clippy；推荐使用：

  ```text
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  ```

- 任务结束时运行 `cargo fmt --all` 统一 Rust 代码格式。若只需检查格式，可使用 `cargo fmt --all -- --check`。
- 修改 source parser 或 elaborator 时先补充失败测试；完成 I0-B 后 converter、VM 和旧 bytecode
  才不在活动 workspace。未来跨格式语义变化必须针对 canonical model、ConversionReport、
  round-trip fixture 和 `examples/` 验证，converter 不得直接消费 source AST。
- 使用校验脚本或外部模拟器验证解析逻辑时，先确认校验脚本与模拟器的代码逻辑一致，不能用有问题的校验脚本得出结论。
- 遇到规范未定义的谱面解析边界时，先记录假设并按通用语义继续推进；完成后在交付说明中明确告知用户这些假设和潜在影响。

## 依赖、库/API 文档与 Context7

以下场景必须使用 Context7 查询当前库/API 信息，即使用户没有明确要求：

- 用户询问推荐加入哪些依赖。
- 用户需要库或 API 文档。
- 用户需要代码生成。
- 用户需要安装步骤或配置步骤。

使用 Context7 时，以其提供的当前文档和示例为依据，再结合本仓库的 Rust edition、workspace 结构和现有依赖作出结论。不要仅凭记忆推荐版本、API 或配置方式。

如果 Context7 出现问题，要在回复中提醒用户；通常继续使用已有仓库信息、官方资料或其他可靠来源完成对话，不必因此中断，除非用户明确要求必须依赖 Context7 或要求停止。
<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->

## 子代理使用

子代理在我们的工作里用于探索，他是你的探子。
把子代理当成你手边最顺手的、用于「宽而重」读取的工具。工作的任何时候，只要你觉得需要就可以派。只有在它能减少主线程上下文污染、提高并行度或者提供独立核验的时候才使用。
必须遵守：你需要更激进和更频繁地调用子代理，在任何需要的情况下，而不仅仅只是在对话的开头。我们需要更频繁的子代理调用来避免上下文腐烂，你承担子代理编排者的角色。

### 何时直接处理

直接读取以及处理以下内容，不派子代理：

- 已知位置的小文件、少量代码或者单一事实；
- 即将修改的具体代码；
- 派发、等待以及复核的成本不低于自己读取的任务。
- 奠基性文档，无论多长都自己读：架构文档、设计文档、交接备忘录（在别的工作流里可能是别的名字）等用来让你建立全局视角、充当后续判断地基的文件——它们的价值全在细节与脉络，一经子代理转译即失真，长度不构成外包的理由。

### 何时适合派发

适合交给子代理的：

- 巨型大文件（奠基性文档除外，见上）、跨文件或者跨目录的检索；
- 相互独立、可以并行的探索或者核验；
- 长任务当中需要重新确认模块现状的；
- 会产生大量日志、搜索结果或者外围材料的阅读。

多个独立的任务应当并发派发。

### 委派与验证

给子代理的任务必须是自包含的，说明检索范围、具体问题以及期望的输出。精度重要的时候，要求返回 `file:line`、符号名以及必要的关键原文——这些出处就是你之后廉价复核的抓手。

子代理的结果只是线索，可能遗漏或者出错。但复核不是把它读过的东西重读一遍，那样这次派发就白费了——你买的是「压缩」，重读会把压缩当场退光。复核 = 顺着它给的 `file:line` 以及关键原文来。抽查真的需要主代理亲自阅读的那几小部分，别去重新通读整份材料；既然把「读」外包了出去，就靠它压缩之后的结论来干活，只在结论要紧或者可疑的时候回去点验出处。

唯二需要你亲自完整读原文的是：① 即将修改的确切代码，② 奠基性文档——这两类本就不外包（见「何时直接处理」）。对它们，子代理至多帮你定位，读由你亲自来：定位与阅读是分工，并非重复劳动。

子代理默认只做探索、检索以及核验。代码修改、方案取舍以及最终验证由主代理来负责。

### 派发机制

- 是否派、派几个由主代理自主决定，无需用户明确要求；较重的探索应当拆成多个独立的轻任务来并发派发。
- 我们系统允许最大并行7个会话进程。所以你最多可以并行分派 6 个子代理；子代理模型的成本较低，无需去顾虑并行派发的成本，只要任务需要就积极使用。
- 子代理一律使用默认配置：工具支持角色参数的时候显式指定 `agent_role = "default"` 或者 `agent_type = "default"`；不支持的时候省略角色、由泛型派生加载 `default.toml`。禁用 `explorer`、`worker` 或者其他角色。
- 派生的时候**必须**显式 `fork_turns = "none"`，不复制主代理的历史，让每个探子都保持干净、快、不背主代理正在腐烂的上下文（代价即上文「任务必须自包含」）。
- 需要多个子代理的时候在同一轮并发派发；派发之后主代理立即 `wait_agent`，停止其余的分析、检索、命令执行以及文件修改，直至全部返回。
- 收到某个子代理结果之后，如果提供了 `close_agent` 就必须立即关闭；每个子代理只用一轮，不复用、不追派。
- 特别注意：子代理自派生起累计运行 10 分钟仍未完成：视为异常，主代理必须介入、不得继续盲等；检查代理状态或运行记录，已有可用 MESSAGE 时采用其部分结果，然后停止这个子代理。并自行判断是否需要再派生或拆分更小任务重新分派。

## Context Engineering skills

本仓库已安装 `Agent-Skills-for-Context-Engineering` 项目级 skills：

- 可发现的 skill 目录位于 `.agents/skills/<skill-name>/`，每个目录必须按原布局保留 `SKILL.md` 以及需要时的 `references/`、`scripts/` 和 `tests/`。
- 完整上游副本位于 `.agents/vendor/Agent-Skills-for-Context-Engineering/`，用于审阅、更新和运行其确定性校验；不要从 vendor 路径建立 Cargo 依赖，也不要把研究框架和示例当作项目运行时依赖。
- 使用渐进式披露：先根据任务匹配 skill 的描述，再只读取被触发 skill 的 `SKILL.md`，只有正文明确需要时才继续读取对应 reference 或 script。
- 只激活与当前任务直接相关的一个或少数 skills，不要为了“加载完整知识”一次性激活全部 skills。普通 FCS parser、runtime、fixture、文档或 Rust 重构任务，如果不涉及下表主题，不需要调用这些 skills。
- 这些 skills 提供上下文工程和 agent harness 的方法，不改变 FCS 语义。涉及 FCS 行为时，`fcs.md`、`fcbc.md`、`fcs-render.md`、`fcs-conversion.md` 及相关 Trellis 规范优先；用户明确指令优先于所有通用 skill 建议。

### 调用时机

| Skill | 在这些情况下调用 | 不要在这些情况下调用 |
|---|---|---|
| `context-fundamentals` | 需要建立 context window、attention、system prompt、tool output 或上下文组成的基本模型时；开始设计 agent context 架构且尚未确定术语时 | 只是在阅读普通项目说明、编写与 context 无关的 Rust 代码时 |
| `context-degradation` | 排查 lost-in-the-middle、context poisoning、distraction、attention 退化或长会话质量下降时 | 普通业务 bug、parser 错误或与模型上下文无关的性能问题 |
| `context-compression` | 需要压缩会话、工具输出、轨迹或持久状态，同时保留决策和下一步时 | 只是删除无用日志或做普通文件压缩时 |
| `context-optimization` | 优化 token budget、检索精度、prefix reuse、masking、partitioning 或上下文分配时 | 只优化 Rust 运行时间、二进制大小或 Cargo 构建时间时 |
| `latent-briefing` | 编排器需要把任务相关状态以 KV/cache compaction 形式交给 worker，且运行时可控制 worker KV/cache 时 | 普通文本 handoff、普通子代理委派，或运行时无法控制模型 KV 状态时 |
| `multi-agent-patterns` | 设计 orchestrator、peer-to-peer、hierarchical agent、handoff、隔离上下文或并行协作时 | 单代理即可完成的任务，或仅按仓库既有规则进行一次探索时 |
| `long-horizon-prompting` | 编写或审阅长时间运行、并行编排任务的 task brief，需要 success predicate、停止条件、审计门和 effort floor 时 | 短小的一次性提示、普通 commit message 或简单需求说明时 |
| `memory-systems` | 设计跨会话记忆、长期/短期记忆、实体追踪、图记忆或检索与更新语义时 | 仅在当前会话临时记录一个变量或普通 changelog 时 |
| `tool-design` | 新增或重构 agent tool，定义工具契约、合并工具面、改善错误返回或工具描述时 | 只调用既有工具，或修改普通 Rust 函数但不改变 agent tool 契约时 |
| `filesystem-context` | 需要把大段上下文、计划、scratchpad、工具输出或跨代理协作状态落到文件系统，并支持按需发现时 | 普通源码文件读写、一次性小文档编辑或已有 Trellis 日志流程足够时 |
| `hosted-agents` | 设计远程 sandbox、后台 coding agent、warm pool、快照持久化、多客户端或多人协作基础设施时 | 本地当前工作树中的普通开发、测试或脚本执行时 |
| `evaluation` | 为 agent 行为建立确定性检查、rubric、回归 fixture、生产监控或质量门时 | 仅验证普通 Rust 编译错误，且不涉及 agent 行为质量时 |
| `advanced-evaluation` | 使用 LLM-as-a-Judge、pairwise comparison、rubric generation、calibration 或 bias mitigation 时 | 只有 deterministic schema、manifest、编译或单元测试检查时 |
| `harness-engineering` | 设计自主运行 loop、locked metrics、durable logs、novelty gate、rollback 或 human approval boundary 时 | 普通 agent prompt、一次性脚本或没有自主优化循环的开发流程时 |
| `self-improvement-loops` | 允许 harness、scaffold、skill 或 agent 流程自修改，并需要 bounded edits、acceptance gates、回滚和多样性约束时 | 普通手工修 bug、固定规则的 CI、或不会修改自身的 agent 流程时 |
| `project-development` | 评估某项工作是否适合 LLM、设计 acquire/prepare/process/parse/render 管线、结构化输出或部署路径时 | FCS 已确定的局部实现、普通依赖升级或与 LLM 项目无关的模块开发时 |
| `bdi-mental-states` | 需要把外部结构化上下文建模为 beliefs、desires、intentions，并要求可解释的理性行动轨迹时 | 只处理普通状态机、Rust 数据结构或不需要 BDI 语义的 agent 状态时 |

### 组合与边界

- 先选择主 skill，再按其 `Integration` 和 `References` 说明补充相邻 skill；通常从 `context-fundamentals` 建立模型，再进入一个专门问题 skill 即可。
- 长任务若需要可恢复的工作状态，优先考虑 `filesystem-context`；只有确实需要跨会话记忆语义时才增加 `memory-systems`。
- 只有在存在多个 agent 或自主 loop 时才使用 `multi-agent-patterns`、`harness-engineering` 或 `self-improvement-loops`；“任务很复杂”本身不是调用理由。
- `evaluation` 负责确定性验收和回归，`advanced-evaluation` 只在需要模型评审或偏差控制时加入；不能用模型评审替代已有的编译、测试、schema 和规范检查。
- 对本仓库的格式语义、解析边界和实现规范，始终先读对应权威文档与更近层级的 `AGENTS.md`；Context Engineering skills 不能覆盖这些项目规则。
