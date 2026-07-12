# FCS 仓库协作指南

本文件适用于整个仓库。开始工作前先阅读本文件；如果子目录中出现更具体的 `AGENTS.md`，以距离目标文件更近的规则为准。不要覆盖或回退工作区中已有的、与当前任务无关的修改。

## 仓库结构与权威资料

- `Cargo.toml` 是 workspace 根配置，当前包含三个 crate：
  - `crates/fcs-core`：FCS 解析器、AST、编译器、字节码和 VM。
  - `crates/fcs-converter`：PGR/RPE/PEC 与 FCS 之间的 IR 和格式转换器。
  - `crates/fcs-cli`：编译、检查和格式转换的命令行入口。
- `fcs.md` 是 FCS 语法、单位系统、运行时语义和字节码格式的主要规范。涉及格式行为的改动先对照它，再检查现有解析器、编译器和转换器实现。
- `examples/` 保存各格式的输入样例；`crates/fcs-converter/tests/` 保存跨格式、往返转换和版权样例测试。
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

先用 `fd` 定位范围，再用 `rg` 或 `sg` 缩小目标。阅读实现时同时查看调用方、对应测试和相关规范，避免只根据单个匹配结果推断行为。

## Rust 开发与验证

- 日常编译和测试不要使用 `--release`。
- 项目使用 `cargo-nextest` 运行测试，不要把普通 `cargo test` 当作默认测试命令。
- 每次运行测试前先执行 Clippy；推荐使用：

  ```text
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  ```

- 任务结束时运行 `cargo fmt --all` 统一 Rust 代码格式。若只需检查格式，可使用 `cargo fmt --all -- --check`。
- 修改解析、编译、VM 或转换逻辑时，优先补充覆盖行为变化的测试；跨格式语义变化应检查相应 round-trip 测试和 `examples/` 样例。
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
