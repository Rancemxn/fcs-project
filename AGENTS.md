# FCS 仓库协作指南

本文件适用于整个仓库。开始工作前先阅读本文件；如果子目录中出现更具体的 `AGENTS.md`，以距离目标文件更近的规则为准。不要覆盖或回退工作区中已有的、与当前任务无关的修改。

## 仓库结构与规范入口

- 当前默认开发分支是 `main`，workspace 只有唯一、无版本前缀的
  `crates/fcs-source`。`archive/fcs4-pre-cutover` 保存完整切换前工作树；FCS 4 core、旧
  converter、旧 CLI、VM 和 bytecode 仅可从该归档分支读取，不属于活动实现。I0 决策由
  `docs/decisions/0006-unversioned-source-cutover.md` 定义，逐步执行和当前状态见
  `docs/plans/i0-source-cutover.md`。后续 canonical/runtime/FCBC/converter/render/CLI crate
  只在对应路线阶段按需创建。
- I0 不保留旧 package/module 名称、feature flag 或兼容 re-export。Cargo 不得使用指向
  `refer/` 的 path dependency。
- `fcs.md` 是 FCS 5 Core source、canonical model 和运行时语义的权威规范。
- `fcbc.md` 是 FCBC 2 容器与 Execution ABI 的权威规范；`fcs-render.md` 和
  `fcs-conversion.md` 分别定义 Render Profile 与转换/保真行为。
- `docs/specification-governance.md` 是规范候选版本、当前状态、变更流程和冻结条件的唯一当前
  入口；不要在本文件复制容易过期的完整状态表。旧 freeze/review 文件只保存其发生时的审计
  事实，不覆盖治理文件中的当前状态。
- I1–I9 使用 ADR 0010 定义的阶段范围化 Reviewed Implementation Baseline：当前阶段的规范依赖
  closure、绑定 fixture/hash 和独立复审不得有未关闭的 Critical/Important finding，阶段计划必须与
  绑定条款一致，且前置质量门通过。满足后自动进入对应阶段，无需再次取得用户确认。该 baseline
  不是新的版本状态，也不把 Draft 提升为 Reviewed/Frozen；I10 conformance RC 仍要求五个版本域
  全部 Frozen、最终联合独立复审和完整 executable conformance 通过。Source grammar closure
  的已审范围和历史证据见
  `docs/reviews/2026-07-15-fcs5-source-grammar-closure-review.md`；当前 authoring/canonical
  closure 的 delta 与跨规范 gate 见
  `docs/reviews/2026-07-15-fcs5-authoring-canonical-closure-review.md`；FCBC 2/Execution ABI 1 的
  ResourceData、exact-only 与 schema 2 golden delta 见
  `docs/reviews/2026-07-15-fcbc2-execution-abi-closure-review.md`；非空 ABI writer→static
  bytes→independent loader/evaluator、bits/trace/direct-seek 与 mutation 的闭合和独立复审见
  `docs/reviews/2026-07-16-fcbc2-execution-abi-nonempty-review.md`；Render stable-resource binding、
  exact descriptor 与 no-source-text delta 见
  `docs/reviews/2026-07-15-render1-resource-binding-closure-review.md`，RenderSection layout、decoder/
  shaping、semantic/raster 与 diagnostic 规范文字的历史独立闭合见
  `docs/reviews/2026-07-16-render1-binary-raster-closure-review.md`；REN-I08–I16 的规范重开与当前复审
  入口见 `docs/reviews/2026-07-16-render1-normative-amendment-review.md`；Conversion parser/profile/Repair 分层、
  12-profile registry、mapping/selection vector 与 no-source-snapshot projection 见
  `docs/reviews/2026-07-15-conversion1-semantic-profile-closure-review.md`。四规范联合候选自检及其
  dated amendment、当前 hash/test evidence、仍开放的 Render/Conversion/Core fixture blocker 与最终
  联合独立复审要求统一见 `docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md`；该文件及
  ABI blocker 的单域关闭都不表示重新 Frozen。I1 source AST/parser 的阶段 dependency closure、
  corrected forward-slash fixture-tree hash、0/0/0 独立复审与自动实施许可见
  `docs/reviews/2026-07-16-i1-source-parser-baseline-review.md`；当前已进入 I1 Task 1。
- `docs/plans/fcs5-roadmap.md` 是唯一总实施路线图；最近完成阶段的详细记录为
  `docs/plans/i0-source-cutover.md`。当前活动 I1 的独立阶段计划为
  `docs/plans/i1-source-ast-parser.md`。计划只能安排工作，不能创造格式语义。
- `examples/` 保存各格式输入样例；I0 删除活动 FCS 4 examples，但保留 PGR/RPE/PEC 与版权
  输入，供未来 converter 重建时复用。旧 converter 测试由归档分支保存，不迁移到 source crate。
- 制订，修改任何文档或者计划时，思考：
	* 项目最容易在哪些地方踩坑；
	* 哪些设计可能在后期形成技术债；
	* 哪些接口和边界需要提前固定；
	* 后续实现时应该遵循哪些原则；
	* 哪些看似方便的方案应该明确禁止。

## 资料职责、权威与冲突处理

本仓库不使用一条简单的“规范 > ADR > community > refer”总排名。必须区分本项目的规范权威与
外部格式的证据权威：

- `docs/specification-governance.md` 管理规范状态和流程；四份根规范在五个独立版本域内定义
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

- 修改 FCS source、static、canonical 或 runtime 语义前，阅读 `fcs.md`、治理文件、相关 Accepted
  ADR、conformance matrix 以及对应 fixture；修改 FCBC/packager/loader/ABI 时同理阅读
  `fcbc.md` 和 ADR 0004、0005、0008、0009。
- 修改 Render Profile 或其资源解析时，阅读 `fcs-render.md`、相关 Core/FCBC 条款以及 ADR 0008、
  0009。
- 修改 Conversion Specification、semantic profile、ConversionReport 或 PGR/RPE/PEC
  parser/importer/exporter 时，先读 `fcs-conversion.md`、ADR 0001、0002、0005、0007、
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
- 每次运行测试前先执行 Clippy；推荐使用：

  ```text
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  ```

- 任务结束时运行 `cargo fmt --all` 统一 Rust 代码格式。若只需检查格式，可使用 `cargo fmt --all -- --check`。
- 修改 source parser 或 elaborator 时先补充失败测试；converter、VM 和旧 bytecode 已不在活动
  workspace。未来跨格式语义变化必须针对 canonical model、ConversionReport、
  round-trip fixture 和 `examples/` 验证，converter 不得直接消费 source AST。
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
- 开始非机械工作前，确保有一个写明范围、权威输入、验收条件、非目标、依赖和验证方法的 Issue。大型工作用 parent/sub-issue 和 blocked-by/blocking 关系，不在一个 Issue 中堆放不可独立验收的横向任务。
- 从最新 `origin/main` 创建 `codex/<issue>-<slug>` 分支；一个分支和 PR 只交付一个可审查工作单元。不要将工作区中与 Issue 无关的改动带入提交。
- PR 正文必须链接 Issue；只有 PR 合并即应关闭 Issue 时才使用 `Closes #<n>`，否则使用 `Refs #<n>`。正文同时记录规范/ADR/conformance/review 影响、实际验证命令、未执行门禁和剩余风险。
- push 前审查 staged diff；PR 合并前检查 `gh pr checks --required`、review decision、mergeability 和未解决评论。不得用 `--admin` 绕过 branch protection，也不得为了变绿而降低测试、fixture 或 review gate。
- 合并后确认 Issue 状态和后续 blocker，必要时在 Issue 中留下最终验证、未完成项与后续 Issue 链接。

### Domain docs

本仓库采用 single-context 阅读约定；存在根目录 `CONTEXT.md` 时读取它，Accepted ADR 的实际位置
始终是 `docs/decisions/`，不要创建第二套 `docs/adr/`。详见 `docs/agents/domain.md`。

### Personal engineering skills

本仓库只使用 `~/.codex/skills` 中的最小个人 skill 集合：`diagnose`、`tdd`、`zoom-out`、`grill-me`、`grill-with-docs`、`improve-codebase-architecture` 和 `agent-loop`。它们是协作流程和推理纪律，不是 FCS、FCBC、Render 或 Conversion 规范的替代品；skill 的建议与本文件、根规范、治理文件或 Accepted ADR 冲突时，必须按“资料职责、权威与冲突处理”中的流程处理，不能直接以 skill 的默认做法覆盖项目约束。

#### 调用时机

- 当任务明确匹配某个 skill 的描述时调用对应 skill；用户直接点名 skill 或 slash command 时，按用户指定的 skill 执行。
- 用户报告 bug、异常、失败或性能回归并要求诊断时，使用 `diagnose`；用户要求 test-first、red-green-refactor 或 integration tests 时，使用 `tdd`。
- 对陌生代码区域需要先了解更高层的模块、调用方与系统边界时，使用 `zoom-out`。
- 需要对方案、决定或计划进行逐项压力测试时，使用 `grill-me`。若压力测试还应同步维护领域文档，使用 `grill-with-docs`；该 skill 必须使用根目录 `CONTEXT.md` 和 `docs/decisions/`，不得创建 `docs/adr/`。
- 设计模块接口、边界、seam、可测试性或 AI 可导航性时，使用 `improve-codebase-architecture`；其领域术语和 ADR 阅读同样必须遵守 `docs/agents/domain.md` 的单一上下文约定。
- 用户要求设计 agent/automation loop 的 Markdown 契约时，使用 `agent-loop`；该 skill 只能产出 `loop.md`，不得执行 loop 或生成运行时机制。

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
