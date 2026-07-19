# 0013：在公开项目仓库运行完整门禁

状态：Accepted

日期：2026-07-19

取代：ADR 0012（公开执行仓库与私有项目门禁）

## 1. 背景

`fcs-project` 已转为公开仓库，`Rancemxn/fcs-action` 已删除。ADR 0012 为保护私有源码而引入的跨仓库
checkout、secret、脱敏 artifact 和实验性 sccache 边界不再解决实际问题，继续保留只会增加门禁维护成本。

完整 Rust、nextest 和 bounded fuzz gate 仍需在 GitHub runner 上执行，以减少本地算力和磁盘占用。

## 2. 决策

- 在本仓库使用一个 `full-gate.yml` workflow，响应 pull request、`main` push 和人工 dispatch。
- workflow 只授予 `contents: read`，不使用 secret，不上传 artifact，也不跨仓库 checkout。
- 单个 `ubuntu-24.04` job 依次执行仓库现有的 locked dependency、fmt、Clippy、nextest、bounded fuzz、
  diff 和 clean-worktree gate；任一步失败即使整个 job 失败。
- 使用 `actions/checkout@v7`、`dtolnay/rust-toolchain@stable`、
  `cargo-bins/cargo-binstall@main` 和 `Swatinem/rust-cache@v2`。
  cargo-binstall 以 `--secure` 安装 cargo-nextest 和 cargo-fuzz；rust-cache 使用默认
  GitHub backend，并覆盖 root 与 `fuzz/` workspace。
- 同一 workflow/ref 的旧 run 可取消。Action run 是固定 commit 的执行证据，但不替代 ADR 0011 的
  Primary Self-Audit、异步独立复审、Ready 或 merge gate。

## 3. 后果

CI 配置与被测源码位于同一公开仓库，pull request 可直接获得完整门禁结果，不再需要手工 dispatch、
token、结果搬运或第二个仓库。稳定大版本/channel ref 会随上游维护更新；失败时直接在本仓库按普通
依赖/CI 修复流程处理，不再维护跨仓库 cache 协议。

本决定只改变协作与交付基础设施，不改变 FCS、FCBC、Render、Conversion、fixture 或实现语义。

## 4. 明确禁止

- 不得恢复已删除的公开执行仓库、私有 checkout token、sccache 或自定义跨仓库 cache。
- 不得用 cache、取消策略或 workflow 条件跳过、缩短或掩盖完整门禁。
- 不得把 Action success 描述为规范性 conformance、独立 reviewer verdict 或 merge 授权。
