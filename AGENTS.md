# FCS 仓库协作指南

本文件适用于整个仓库。开始工作前先阅读本文件；如果子目录中出现更具体的 `AGENTS.md`，以距离目标文件更近的规则为准。不要覆盖或回退工作区中已有的、与当前任务无关的修改。

## 仓库结构与文档入口

- 默认开发分支是 `main`；活动 workspace 只有 `crates/fcs-source`。`archive/fcs4-pre-cutover` 仅供
  阅读旧实现，不能作为活动依赖或兼容层来源；Cargo 不得使用指向 `refer/` 的 path dependency。
- 根目录只保留本文件作为协作入口。完整文档索引见 `docs/README.md`，常用入口如下：
  - `docs/CONTEXT.md`：项目术语和 single-context 词汇；
  - `docs/specifications/`：四份 FCS/FCBC/Render/Conversion 规范及规范治理；
  - `docs/conformance/`：机器可读 conformance corpus、manifest、golden 和覆盖 ledger；
  - `docs/decisions/`：Accepted ADR 和治理修订历史；
  - `docs/plans/`：路线图与阶段计划；
  - `docs/reviews/`：固定范围、hash 和复审证据；
  - `docs/agents/`：领域阅读、GitHub 交付和 triage 规则；
  - `docs/loops/`：主实现 loop 与独立审查 loop；
  - `docs/community/`：外部格式证据综合；
  - `docs/scratch/`：历史临时记录，只供追溯，不能作为当前状态入口。
- `.github/ISSUE_TEMPLATE/` 和 `.github/pull_request_template.md`：Issue/PR 的初始契约模板；后续进度与
  审查结果按 `docs/agents/issue-tracker.md` 和 `docs/loops/` 追加 comment。
- `examples/` 保存输入样例；旧 converter、CLI、VM 和 bytecode 仅从归档分支读取，不迁移回活动
  workspace。
- 修改文档或计划时，固定权威入口、边界、验收证据和禁止的便利方案；不要把实现状态复制到协作入口。

## 资料职责、权威与冲突处理

本仓库不使用一条简单的“规范 > ADR > community > refer”总排名。必须区分本项目的规范权威与
外部格式的证据权威：

- `docs/specifications/governance.md` 管理规范状态和流程；四份根规范在五个独立版本域内定义
  规范性行为和 conformance 要求。
- `docs/decisions/` 中 Accepted ADR 是已经接受的设计约束、架构边界和规范修订方向，但不是
  source grammar、二进制布局或执行语义的替代文本。Accepted ADR 与现行规范冲突时，不得任选
  一方直接实现；必须重开受影响规范，更新规范、fixture、manifest、review 和状态记录。依赖该
  语义的 I1–I9 工作只能在受影响的阶段 baseline 重新独立复审后恢复；I10/发布仍须重新 Frozen。
- Accepted ADR 是历史记录。后续决定改变它时，新建 ADR 并把旧记录标为 `Superseded` 或
  `Partially superseded`；勘误或治理补充必须以明确的 dated amendment 记录，不得静默改写历史
  背景和原决定。
- `docs/community/` 是 PGR v1/v3、RPE、PEC 等外部格式的维护后证据综合、歧义索引和研究入口，
  不定义 FCS/FCBC，也不能替代 Conversion Specification。
- `refer/chart/` 是固定 commit/hash 下外部实现、编辑器和文档的一手证据。对“某项目在某版本
  如何行为”的事实，固定快照比 community 摘要更直接；若二者冲突，应修订摘要或保留歧义。
  单个参考项目不能定义整个社区格式，多个可信来源冲突时必须记录双方，并由显式、版本化的
  semantic profile 作出项目内选择。
- 当前 parser/compiler/runtime/converter、测试和 `examples/` 只说明实现状态或 fixture 意图，
  不能静默成为新规范。实现与规范冲突时，先判断实现缺陷还是规范缺陷，再走相应修复或规范
  变更流程。

## 阅读路由

- 修改 FCS source、static、canonical 或 runtime 语义前，阅读 `docs/specifications/fcs.md`、治理文件、相关 Accepted
  ADR、conformance matrix 以及对应 fixture；修改 FCBC/packager/loader/ABI 时同理阅读
  `docs/specifications/fcbc.md` 和 ADR 0004、0005、0008、0009。
- 修改 Render Profile 或其资源解析时，阅读 `docs/specifications/fcs-render.md`、相关 Core/FCBC 条款以及 ADR 0008、
  0009。
- 修改 Conversion Specification、semantic profile、ConversionReport 或 PGR/RPE/PEC
  parser/importer/exporter 时，先读 `docs/specifications/fcs-conversion.md`、ADR 0001、0002、0005、0007、
  `docs/community/README.md` 和对应格式文档，再核对与改动直接相关的固定参考快照。
- 更新 `docs/community/`、新增外部格式结论、解释歧义或给 fixture 声明 producer/runtime 行为时，
  必须进入 `refer/chart/`；先检查参考仓库自己的规则，再验证 origin、commit/hash、目标路径、
  parser/schema、调用方和至少一个独立来源。结论记录“项目 + commit/hash + 路径 + 行为”，不能只
  写“某工具就是这样”。若本地 HEAD 与 community 记录不一致，使用记录的 commit 读取，或显式
  启动一次完整证据基线更新；不得混用快照。
- 普通 FCS parser、内部重构和与外部格式无关的实现默认排除 `refer/`，防止参考实现反向定义本
  项目语义。
- `refer/dependencies/` 保存本项目所用依赖的固定版本源码。开始依赖/API 工作时先核对
  `Cargo.toml`/`Cargo.lock` 与本地源码版本；存在匹配源码时直接阅读它，不使用 Context7 覆盖该
  版本，也不得把 `refer/` 作为 Cargo path dependency。
- 涉及格式行为的改动必须同时检查 parser、compiler、runtime、converter 和 conformance
  fixture 中受影响的层；未冻结的假设只能记录为候选，不能通过计划或实现获得规范地位。

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

- 用 `jq` 处理 `gh --json` 或 `gh api` 输出，不要解析面向人的表格文本：

  ```text
  gh issue list --state open --json number,title,labels |
    jq -r '.[] | "#\(.number)\t\(.title)\t\([.labels[].name] | join(","))"'
  gh pr view 42 --json state,isDraft,mergeable,reviewDecision,statusCheckRollup |
    jq -e '.state == "OPEN" and (.isDraft | not) and .mergeable != "CONFLICTING"'
  ```

  简单投影可直接用 `gh --jq`；需要复用 filter、组合多个输入或使用 `jq -e`
  作为门禁时使用独立 `jq`。动态值用 `--arg`/`--argjson` 传入，不要拼接 filter；
  对分页 API 使用 `gh api --paginate --slurp` 后再聚合。

- 用 `sg`（ast-grep）搜索 Rust 的语法结构；当空格、换行或具体变量名不应影响匹配时，优先于纯文本搜索：

  ```text
  sg run -l rust -p 'use $A;' crates
  sg run -l rust -p 'fn $NAME($$$ARGS) $$$BODY' crates
  ```

先用 `fd` 定位范围，再用 `rg` 或 `sg` 缩小目标。阅读实现时同时查看调用方、对应测试和
相关规范，避免只根据单个匹配结果推断行为。目标项目检查默认排除 `refer/`；只有“阅读路由”
明确要求研究外部证据或依赖源码时才进入，并遵守对应快照、版本和参考仓库规则。

## Rust 开发与验证

- 日常编译和测试不要使用 `--release`。
- 项目使用 `cargo-nextest` 运行测试，不要把普通 `cargo test` 当作默认测试命令。
- 验证遵循“足以发现当前错误的最小反馈”：在编辑循环中优先运行受影响 crate、模块、测试或
  fixture 的 focused check。不要在每个文件、patch 或 focused test 后运行全 workspace Clippy/nextest，
  也不要把全量 Clippy 作为每次 focused test 的前置条件。
- 只在 Rust 源码、build/dependency 配置、测试或可执行 fixture 发生变更，且到达一个可交付检查点时
  运行全量门禁。检查点包括：一个完整工作单元完成、PR 从 draft 转 ready、交接给其他实施者，或变更涉及
  公开接口、workspace/dependency、conformance 行为。用户明确要求全量验证时也应执行。
- 全量 Rust 检查点的默认顺序是：

  ```text
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  ```

  若格式检查只发现本次 Rust 改动的差异，运行 `cargo fmt --all` 并审查它的 diff；若失败来自与任务无关的
  既有文件，不要顺带格式化，应记录为既有门禁问题。一个检查点的全量门禁通过后，只有后续改动可能使结果
  失效时才重跑。
- 只修改 Markdown、AGENTS、Issue/PR 模板、评论、label 或其他不参与 Rust 构建的元数据时，不运行 Clippy、
  nextest 或 cargo fmt；改用 diff、链接、Markdown/YAML/JSON schema 和相关 CLI smoke check。
- 修改 source parser 或 elaborator 时先补充失败测试，并在 red→green 循环中只运行能复现当前行为的
  focused test；完成该工作单元后再进全量 Rust 检查点。converter、VM 和旧 bytecode 已不在活动
  workspace。未来跨格式语义变化必须针对 canonical model、ConversionReport、
  round-trip fixture 和 `examples/` 验证，converter 不得直接消费 source AST。
- 交付说明必须列出实际运行的 focused/full 检查、未运行的门禁及原因。不得将“未适用”写成“已通过”。
- 使用校验脚本或外部模拟器验证解析逻辑时，先确认校验脚本与模拟器的代码逻辑一致，不能用有问题的校验脚本得出结论。
- 遇到规范未定义的外部谱面边界时，研究阶段可以记录候选假设，但规范性实现不得发明“通用
  语义”。Strict mode 必须失败或要求显式 semantic profile；repair 只能修复非法或矛盾输入，
  不能替用户选择多个合法解释。只有 package/profile 明确声明、用户显式选择，或所有候选对当前
  输入 canonical-semantic-equivalent 时，才能无询问继续；假设和潜在影响必须进入交付说明与报告设计。

## Agent skills

### Issue tracker

本仓库使用 GitHub Issues 记录工作契约、依赖和验收条件，使用 Pull Requests 交付修改与验证证据。使用 `gh` 读写 Issue/PR，使用 `jq` 处理结构化 JSON。完整流程见 `docs/agents/issue-tracker.md`，接受的决策见 ADR 0011。Issue、PR 及其评论只能安排或证明工作，不能创造规范语义。

### Triage labels

使用五个 GitHub 状态 label：`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human` 和 `wontfix`。一个 open Issue 同时只保留一个状态 label；`bug`、`documentation`、`enhancement` 等是正交的类型 label。详见 `docs/agents/triage-labels.md`。

### GitHub delivery workflow

- 只读检查使用 `gh issue list/view`、`gh pr list/view/diff/checks` 和 `gh api`。创建、编辑、评论、关闭、push、review 或 merge 是外部状态变更，只在用户明确要求对应工作流时执行。
- `gh` 因 DNS、连接超时/重置、TLS 中断或 HTTP 502/503/504 等瞬时网络问题失败时，每隔 5 秒重试同一操作，首次失败后最多再试 10 次。写操作在每次重试以及稍后补同步前，必须先按稳定身份查询远程是否已生效，避免重复创建 Issue/PR、重复评论、review 或 merge。不得重试认证/权限失败、参数/校验错误、not found、合并冲突或门禁失败；应立即报告。10 次重试耗尽后，记录完整待同步 payload、稳定身份、最后错误和 `pending remote sync` 状态，继续不依赖该远端结果的安全本地工作；在下一个有意义检查点以及 handoff、PR Ready、review 或 merge 等依赖远端状态的动作前再次查询并尝试同步。待同步记录只是 transport outbox，不是第二个 tracker；不得把未确认的远端动作描述为成功。
- 开始非机械工作前，确保有一个写明范围、权威输入、验收条件、非目标、依赖和验证方法的 Issue。大型工作用 parent/sub-issue 和 blocked-by/blocking 关系，不在一个 Issue 中堆放不可独立验收的横向任务。
- 非机械 Issue 正文必须写明稳定的初始工作契约和一条实质性的初始 `Progress`，不得只保留初始对话或空模板。之后每个有意义检查点分别发送一条新的 Issue comment，不在正文或旧评论中累计、反复 edit。范围/决定变化、完成工作单元、出现/解除阻塞、获得验证结果、创建 PR 或交付状态变化时发送新消息；每条包含 Completed、Evidence、Decisions、Blockers 和 Next。更正旧消息时发送显式 superseding comment 并指出被替代内容，不静默覆盖历史；不需要为每个 commit 发一条。
- 主实现从最新 `origin/main` 创建 `codex/<issue>-<slug>` 分支；一个分支和 PR 只交付一个可审查工作单元。审查会话的 corrective branch 例外见“独立审查会话”。不要将工作区中与 Issue 无关的改动带入提交。
- PR 正文必须链接 Issue；只有 PR 合并即应关闭 Issue 时才使用 `Closes #<n>`，否则使用 `Refs #<n>`。正文同时记录规范/ADR/conformance/review 影响、实际验证命令、未执行门禁和剩余风险。
- PR 不得只有空初始说明和一串 commits。正文必须含一条实质性的初始 `Progress`，说明首个可审查 change group、原因、证据、决定和剩余项；之后每次重要 push、阻塞变化和转 Ready 前分别发送新的 PR comment，使最新消息与当前 diff/commits 一致。不得把后续进度反复 edit 到正文或旧评论中；更正使用显式 superseding comment。commit message 不能替代这些进度消息。
- Issue/PR 的 Progress 消息标题只写事件或状态，不手写 `YYYY-MM-DD` 等日历日期；时间以 GitHub 自带的 timestamp 为准。
- push 前审查 staged diff；PR 合并前检查 `gh pr checks --required`、review decision、mergeability 和未解决评论。不得用 `--admin` 绕过 branch protection，也不得为了变绿而降低测试、fixture 或 review gate。
- merge 前分别在 Issue 和 PR 中发送新的 delivery-ready Progress comment；合并后即使 Issue 已由 `Closes` 自动关闭，也要分别发送新的 final merged checkpoint，记录合并 PR/交付结果、最终验证、未完成项与后续 Issue 链接，再确认 Issue 状态和后续 blocker。Issue/PR 的进度消息是工作流证据，不获得规范权威。

#### 独立审查会话

- 主实现会话和独立审查会话只保留两个角色：当前会话是唯一实现者、唯一可以执行 `gh pr ready` 的角色，
  也是唯一 merge owner；审查会话按 `docs/loops/review-loop.md` 运行，不创建第三个可选实现会话。
- 审查者可以读取固定 Issue/PR/已合并 commit，引用历史 commit 指出漏洞，comment，提交
  `gh pr review --comment`/`--request-changes`，创建 bug/finding Issue，以及为已记录 finding 创建
  corrective PR。审查者不能合并 PR、标记 Ready、关闭主 Issue、修改主 Issue workflow label，或写入当前
  会话的工作树、活动实现分支和 `main`。
- 每次审查必须固定 `Issue/PR 或 commit + head SHA + scope + commands + acceptance gate`。当前会话在
  PR Ready/merge 前发送 `Review requested`；审查结束后审查者立即在 PR 和关联 Issue 各追加一条 append-only
  `Audit result`（被审 PR 存在时评论 PR，同时评论关联 Issue；仅有 commit 时评论关联 Issue），即使没有
  finding。消息包含 Target、Head SHA、Scope、Commands、Verdict、Findings、Gate impact、Limitations 和
  Next，不手写日期、不编辑旧消息。
- 后续 push、scope、命令、依赖 closure 或验收变化会使旧审查失效；追加 superseding/re-review 消息并对
  新 SHA 重新审查。Critical/Important finding 阻塞当前 PR Ready/merge；Minor 只有在不影响当前验收且有
  owner、follow-up Issue、目标 stage 和解除条件时才能延期。
- 审查者创建的 corrective PR 必须链接 finding Issue，并使用独立 worktree 和
  `codex/<finding>-<slug>` 分支。开放 PR 的修复分支从被审 PR 的固定 head SHA 建立、目标为活动 PR 分支；
  历史 commit 的修复分支从最新 `origin/main` 建立、目标为 `main`。审查者不得审查或批准自己创建的修复
  PR；当前会话检查、合并后，主 PR 的新 SHA 必须重新审查。
- 本段权限与 `docs/loops/review-loop.md`、`docs/agents/issue-tracker.md` 和 ADR 0011 的 dated amendment 共同构成
  当前工作流；它们不能赋予 Issue/PR 或审查评论规范权威。

### Domain docs

本仓库采用 single-context 阅读约定；读取 `docs/CONTEXT.md`，Accepted ADR 的实际位置
始终是 `docs/decisions/`，不要创建第二套 `docs/adr/`。详见 `docs/agents/domain.md`。

### Personal engineering skills

本仓库只使用 `~/.codex/skills` 中的最小个人 skill 集合：`diagnose`、`tdd`、`zoom-out`、`grill-me`、`grill-with-docs`、`improve-codebase-architecture` 和 `agent-loop`。它们是协作流程和推理纪律，不是 FCS、FCBC、Render 或 Conversion 规范的替代品；skill 的建议与本文件、根规范、治理文件或 Accepted ADR 冲突时，必须按“资料职责、权威与冲突处理”中的流程处理，不能直接以 skill 的默认做法覆盖项目约束。

#### 调用时机

- 当任务明确匹配某个 skill 的描述时调用对应 skill；用户直接点名 skill 或 slash command 时，按用户指定的 skill 执行。
- 用户报告 bug、异常、失败或性能回归并要求诊断时，使用 `diagnose`；用户要求 test-first、red-green-refactor 或 integration tests 时，使用 `tdd`。
- 对陌生代码区域需要先了解更高层的模块、调用方与系统边界时，使用 `zoom-out`。
- 需要对方案、决定或计划进行逐项压力测试时，使用 `grill-me`。若压力测试还应同步维护领域文档，使用 `grill-with-docs`；该 skill 必须使用 `docs/CONTEXT.md` 和 `docs/decisions/`，不得创建 `docs/adr/`。
- 设计模块接口、边界、seam、可测试性或 AI 可导航性时，使用 `improve-codebase-architecture`；其领域术语和 ADR 阅读同样必须遵守 `docs/agents/domain.md` 的单一上下文约定。
- 用户要求设计 agent/automation loop 的 Markdown 契约时，使用 `agent-loop`；该 skill 只能产出 `docs/loops/loop.md`（或项目已声明的 `docs/loops/` 子路径），不得执行 loop 或生成运行时机制。

#### 调用边界

- 简单问答、单文件的机械编辑、只读检查或与上述场景无关的任务不必强行调用个人 skill。
- `grill-me` 和 `grill-with-docs` 会持续追问或修改领域文档，只有在用户明确要求，或任务描述已经明确要求对应流程时才调用；不要仅凭“看起来可能有帮助”启动它们。
- 先阅读本文件及目标路径更近的 `AGENTS.md`，再按“阅读路由”读取相关规范、ADR、fixture 和现有 docs；skill 不能免除这些前置阅读。
- skill 产出的计划、术语、假设和 issue 只能记录或安排工作，不能创造新的规范语义。凡是规范未定义的边界，记录为候选并报告影响；不得用 skill 的默认推断替代显式 semantic profile、规范修订或用户选择。
- 任务结束时按本文件的 Rust 验证要求执行检查；若 skill 自带的验证或写作流程与仓库命令、目录职责或提交范围冲突，以本文件为准，并在交付说明中标明未执行的步骤及原因。

## 依赖、库/API 文档与 Context7

当 `refer/dependencies/` 中没有与项目引用版本匹配的源码时，以下场景必须使用 Context7 查询
当前库/API 信息，即使用户没有明确要求：

- 用户询问推荐加入哪些依赖。
- 用户需要库或 API 文档。
- 用户需要代码生成。
- 用户需要安装步骤或配置步骤。

若 `refer/dependencies/` 已有匹配源码，必须直接阅读该版本源码及其随附文档，不得再以 Context7
覆盖；用户明确要求比较上游最新文档时除外，但必须区分“项目固定版本”和“上游当前版本”。没有
本地匹配源码而使用 Context7 时，以其当前文档和示例为依据，再结合本仓库的 Rust edition、
workspace 结构和现有依赖作出结论。不要仅凭记忆推荐版本、API 或配置方式。

如果 Context7 出现问题，要在回复中提醒用户；通常继续使用已有仓库信息、官方资料或其他可靠来源完成对话，不必因此中断，除非用户明确要求必须依赖 Context7 或要求停止。
