# FCS 仓库协作指南

本文件适用于整个仓库。开始工作前先阅读本文件；如果子目录中出现更具体的 `AGENTS.md`，以距离目标文件更近的规则为准。不要覆盖或回退工作区中已有的、与当前任务无关的修改。

## 仓库结构与权威资料

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
- `docs/specification-governance.md` 定义规范状态和变更流程，
  `docs/plans/fcs5-roadmap.md` 是唯一总实施路线图；当前阶段详细计划为
  `docs/plans/i0-source-cutover.md`。`docs/decisions/` 只记录设计理由，不得覆盖四份权威规范。
- 涉及格式行为的改动先对照对应权威规范，再检查 parser、compiler、runtime、converter 和
  conformance fixture；实现现状不能静默成为新规范。
- `examples/` 保存各格式输入样例；I0 删除活动 FCS 4 examples，但保留 PGR/RPE/PEC 与版权
  输入，供未来 converter 重建时复用。旧 converter 测试由归档分支保存，不迁移到 source crate。
- `refer/`下的项目是外部项目。仅供参考。chart是Phigros社区里的项目。dependencies是本项目使用的依赖的源代码。**如果dependencies下已经有依赖的源代码，请不要查阅Context7，直接阅读dependencies。**
- 本文件对搜索工具和 Context7 的规则作明确补充。

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
- 修改 source parser 或 elaborator 时先补充失败测试；converter、VM 和旧 bytecode 已不在活动
  workspace。未来跨格式语义变化必须针对 canonical model、ConversionReport、
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
