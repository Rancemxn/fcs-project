# FCS 仓库协作指南

本文件适用于整个仓库。开始工作前先阅读本文件；如果子目录中出现更具体的 `AGENTS.md`，以距离目标文件更近的规则为准。不要覆盖或回退工作区中已有的、与当前任务无关的修改。

## 仓库结构与规范入口

- 当前活动开发分支是 `master`，workspace 只有唯一、无版本前缀的
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
- I1 Rust 实现使用客观阶段门：S15 五个相关版本域均为 Frozen、独立复审没有未关闭的
  Critical/Important finding、I1 阶段计划与最终规范一致，且 I0/前置阶段质量门通过。四项条件
  全部满足后自动进入 I1，无需再次取得用户确认；I1–I10 的阶段衔接同理。Source grammar closure
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
  shaping、semantic/raster 与 diagnostic 规范文字的独立闭合见
  `docs/reviews/2026-07-16-render1-binary-raster-closure-review.md`；Conversion parser/profile/Repair 分层、
  12-profile registry、mapping/selection vector 与 no-source-snapshot projection 见
  `docs/reviews/2026-07-15-conversion1-semantic-profile-closure-review.md`。四规范联合候选自检及其
  dated amendment、当前 hash/test evidence、仍开放的 Render/Conversion/Core fixture blocker 与最终
  联合独立复审要求统一见 `docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md`；该文件及
  ABI blocker 的单域关闭都不表示重新 Frozen。
- `docs/plans/fcs5-roadmap.md` 是唯一总实施路线图；最近完成阶段的详细记录为
  `docs/plans/i0-source-cutover.md`。I1 的独立阶段计划草案为
  `docs/plans/i1-source-ast-parser.md`。计划只能安排工作，不能创造格式语义。
- `examples/` 保存各格式输入样例；I0 删除活动 FCS 4 examples，但保留 PGR/RPE/PEC 与版权
  输入，供未来 converter 重建时复用。旧 converter 测试由归档分支保存，不迁移到 source crate。

## 资料职责、权威与冲突处理

本仓库不使用一条简单的“规范 > ADR > community > refer”总排名。必须区分本项目的规范权威与
外部格式的证据权威：

- `docs/specification-governance.md` 管理规范状态和流程；四份根规范在五个独立版本域内定义
  规范性行为和 conformance 要求。
- `docs/decisions/` 中 Accepted ADR 是已经接受的设计约束、架构边界和规范修订方向，但不是
  source grammar、二进制布局或执行语义的替代文本。Accepted ADR 与现行规范冲突时，不得任选
  一方直接实现；必须重开受影响规范，更新规范、fixture、manifest、review 和状态记录，重新
  Frozen 后再实现。
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

本仓库使用本地 Markdown 文件记录 issue 和 spec，位置为 `.scratch/<feature-slug>/`。详见 `docs/agents/issue-tracker.md`。

### Triage labels

使用默认的五个 triage label：`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human` 和 `wontfix`。详见 `docs/agents/triage-labels.md`。

### Domain docs

本仓库采用 single-context 布局，使用根目录的 `CONTEXT.md` 和 `docs/adr/`。详见 `docs/agents/domain.md`。

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
