# 0012：使用公开执行仓库运行私有项目完整门禁

状态：Superseded by ADR 0013

日期：2026-07-19

## 1. 背景

`fcs-project` 的 Rust workspace、独立 fuzz workspace 和后续 conformance artifact 会持续增加本地
编译时间与磁盘占用。当前本地资源不足以稳定并行保存多个完整 gate 的 `target/`，但降低 Clippy、
nextest、fuzz 或 dependency closure 门禁会破坏可复现交付和 I10 证据。

GitHub Actions 可以提供独立的临时 runner，但 `fcs-project` 是私有仓库。直接在公开事件上读取私有
源码会扩大 secret 暴露面；把完整日志、源码、二进制或 fuzz 数据上传到公开执行仓库也会破坏私有
边界。与此同时，sccache 的 GitHub Actions backend 主要使用短期 runtime token，而用于私有仓库
Actions read/write 的细粒度 token 是否能跨仓库访问 cache data plane 尚无本项目实测证据。

因此需要把完整 Rust 门禁、私有源码访问、公开 runner、审计证据和实验性跨仓库 cache 明确分层，
同时保持 ADR 0011 的 Primary Self-Audit、PR Ready 和 merge ownership 不变。

## 2. 决策

### 2.1 仓库、触发与快照

- 使用公开仓库 `Rancemxn/fcs-action` 作为临时执行器；私有 `fcs-project` 仍是源码、Issue、PR、
  review、gate 结论和合并状态的权威仓库。
- 第一版只接受受信维护者发起的 `workflow_dispatch`，必填输入是完整的 40 位 `project_sha`。
  不自动响应 public fork、`pull_request`、`pull_request_target` 或其他不受信 payload，也不建立
  自动 status bridge。
- workflow 使用 `FCSPROJECT` 只读取得私有仓库的精确 commit，checkout 后必须验证 `HEAD` 与输入
  SHA 完全相同。PR gate 绑定同步到最新 `main` 后的不可变 PR head；不使用临时 synthetic merge
  ref。base 或 head 改变会使旧结果失效，必须重新 dispatch。
- `FCSCACHE` 只用于实验性访问私有 `fcs-project` 的 Actions cache。两个 secret 只进入需要它们的
  step，不写入命令、summary、artifact、cache key 或日志。

### 2.2 Runner、工具与完整门禁

- 每次 dispatch 在 `ubuntu-24.04` 的单个顺序 job 中运行，timeout 为 60 分钟；全仓库使用一个
  concurrency group，`cancel-in-progress: false`，避免不同快照相互取消或争用 cache writer。
- 首版固定 Rust `1.97.1`、cargo-nextest `0.9.140`、cargo-fuzz `0.13.2` 和 sccache `0.16.0`。
  所有第三方 `uses:` 必须固定完整 commit SHA。版本更新必须经过独立 PR、重新执行完整门禁并更新
  cache contract version；浮动 tag 不能成为可复现证据。
- 每次 dispatch 都必须完整、不可跳过地执行以下 gate：
  1. 验证精确 checkout SHA；
  2. 对 root workspace 和 `fuzz/` workspace 执行 locked fetch；
  3. 执行 root/fuzz locked metadata 与 dependency-tree 检查；
  4. 执行 `cargo fmt --all -- --check`；
  5. 执行 `cargo clippy --workspace --all-targets -- -D warnings`；
  6. 执行 `cargo nextest run --workspace`；
  7. 以 `FCS_FUZZ_RUNS=32` 执行 `scripts/fcs5-fuzz-smoke.sh bounded`；
  8. 执行 `git diff --check` 并确认 worktree clean。
- cache 初始化、读取、写入或统计失败不得跳过、缩短或改变上述 gate。完整 gate 任何一步失败时，
  该 run 都不能形成通过证据。

### 2.3 sccache 实验

- sccache GHA backend 首版是非阻塞实验，不是 gate。`SCCACHE_GHA_CACHE_TO` 和
  `SCCACHE_GHA_CACHE_FROM` 使用稳定且不含 `project_sha` 的 cache key；
  `SCCACHE_GHA_VERSION` 至少绑定 runner OS/architecture、Rust version 和显式 cache contract
  version，sccache 自身另把工具版本写入 backend version。toolchain 或 backend contract 改变时
  提升 contract version 以整体失效。
- `SCCACHE_GHA_CACHE_TO` 与 `SCCACHE_GHA_CACHE_FROM` 使用该稳定 namespace；
  `SCCACHE_GHA_CACHE_URL` 和 `SCCACHE_GHA_RUNTIME_TOKEN` 只在 sccache step 中指向
  候选私有 cache endpoint 和 credential。HTTP 401/403、rate limit、backend unavailable、
  零远端命中或写入失败必须记录为 cache experiment failure，但 Rust gate 继续执行。
- 只有同一 `project_sha` 的两次独立完整 gate 均通过，private `fcs-project` cache inventory 出现对应
  metadata，且第二次 run 的脱敏统计显示 remote hits，才能接受跨仓库 cache。首轮结果、单次命中、
  本地命中或公开 `fcs-action` cache metadata 都不能证明该设计成立。

### 2.4 证据与审计职责

- 公开仓库只上传脱敏 `summary.json`，内容限于输入/实际 SHA、固定工具版本、各 gate 状态与耗时、
  退出分类和无 credential/path 的 sccache 聚合统计。不得上传 source、`target/`、binary、完整日志、
  fuzz corpus、crash dump 或 conformance/release bundle。
- 每个 gate 的原始 stdout/stderr 只写入 runner 上的临时文件。公开 Actions log 只输出脱敏状态；
  即使 gate 失败也不回显原始编译、测试或 fuzz 日志，并在 artifact 上传前删除临时文件。
- `summary.json` 和 Action run 只是可复查的执行证据。主会话仍须在私有 PR 与关联 Issue 上针对固定
  head SHA 执行并记录 Primary Self-Audit，且仍是唯一可以 Ready/merge 的角色；远端 CI 不能替代
  ADR 0011 的 audit、finding、mergeability、required-check 或 unresolved-thread gate。
- 结果由主会话读取并追加到私有 Issue/PR checkpoint。第一版不把公开仓库 run 自动写回私有 PR，
  也不把 GitHub comment 当作规范、conformance 或 implementation baseline 权威。

## 3. 后果

完整 Rust、nextest 和 bounded fuzz gate 可以转移到临时 runner，本地只需保留 focused feedback 和
必要的固定 worktree。精确 SHA、固定工具和完整单 job 顺序使每次结果可追溯，公开 workflow 也能被
审查而不公开私有源码或完整构建产物。

代价是每次 full gate 都消耗 GitHub runner 时间，第一版需要维护者手动 dispatch 和回填证据；公开
仓库会暴露 gate 命令、固定工具版本、运行状态和脱敏耗时。跨仓库 GHA cache 可能因 runtime-token
协议不接受细粒度 token 而失败，因此首版必须能够在完全没有远端 cache 的情况下正确完成。

本决定只改变协作与交付架构，不改变 FCS Core、FCBC Container、Execution ABI、Render Profile、
Conversion Specification、fixture、manifest、Reviewed Implementation Baseline 或版本状态；
`docs/CONTEXT.md` 不增加领域术语。

## 4. 明确禁止

- 不得在不受信事件、public fork 或 `pull_request_target` 中读取 `FCSPROJECT` 或 `FCSCACHE`。
- 不得使用浮动 action tag、未固定工具版本、moving branch/ref 或 synthetic merge ref 声称精确快照
  已通过。
- 不得将 cache failure 隐藏为 cache hit，也不得用 cache failure 掩盖 Rust gate failure；二者必须
  分别记录。
- 不得把 `project_sha` 加入稳定 cache namespace 来伪造隔离，否则第二次同提交命中不能证明跨提交
  复用能力。
- 不得上传、输出或通过 cache 泄露私有源码、credential、完整路径、构建产物、完整日志、fuzz 数据
  或发布/conformance bundle。
- 不得把公开 Action success 当作 Primary Self-Audit、独立 reviewer verdict、规范性 conformance
  结论、PR Ready 授权或 merge 授权。
