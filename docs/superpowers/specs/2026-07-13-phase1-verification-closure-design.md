# FCS 5 Phase 1 验证闭环设计

## 状态

已确认。本文定义关闭 FCS 5 Phase 1 workspace 质量 gate 的范围和测试架构；不增加 FCS 5 语义，也不开始 Phase 2。

## 目标

让干净 clone 的默认 workspace 验证可重复通过，同时保留对未纳入版本控制的社区版权谱面的严格 round-trip 验证能力。

默认验证命令必须在没有 `examples/COPYRIGHT` 目录时可执行：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

拥有版权 fixture 的开发者或 CI 可以显式运行独立测试 lane，验证同一批社区谱面。

## 非目标

- 不修改 FCS 5 `v5` parser、AST、版本、tempo 或 profile 语义。
- 不迁移 CLI、converter、FCBC 或 Render Profile。
- 不将版权谱面提交到仓库。
- 不通过 `#[allow(dead_code)]`、静默 skip 或降低 round-trip 比较精度来让默认验证变绿。

## 当前问题

`crates/fcs-converter/tests/common/mod.rs` 同时包含路径定位和 round-trip 比较 helper。每个 integration-test crate 均通过 `mod common;` 编译这个文件，但 PGR、RPE、PEC 之外的 test crate 并不使用事件比较 helper，因此 `cargo clippy --workspace --all-targets -- -D warnings` 将它们判定为 dead code。

其中 `compare_notes_exact` 没有调用方，`EventTolerances::default()` 也没有调用方。其余事件比较 helper 只服务于 PGR/RPE/PEC round-trip 测试和版权 round-trip 测试。

`examples/COPYRIGHT` 被 `.gitignore` 明确排除，但 `copyright_tests` 目前默认参与 workspace 测试。因此没有私有 fixture 的干净 checkout 不可能通过默认 `nextest`。

## 设计

### 测试 helper 的按需模块边界

移除 `crates/fcs-converter/tests/common/mod.rs`，替换为两个独立文件：

```text
crates/fcs-converter/tests/common/paths.rs
crates/fcs-converter/tests/common/roundtrip.rs
```

`paths.rs` 只提供从 `CARGO_MANIFEST_DIR` 解析仓库相对路径的函数。版权 fixture 目录解析位于唯一使用它的 `copyright_tests` target：`FCS_COPYRIGHT_DIR` 存在且非空时优先使用它；否则回退到仓库根目录的 `examples/COPYRIGHT`。

`roundtrip.rs` 只保留下列实际使用的比较能力：

- `sample_event_value`；
- `chart_time_range`；
- `EventTolerances`；
- `compare_events_sampled`。

删除未使用的 `compare_notes_exact` 和 `impl Default for EventTolerances`。这不是 API 变更：两者都位于 integration-test 私有模块，且没有调用方。

每个 integration-test crate 使用 `#[path = "common/…"] mod …;` 按需加载模块：

| Test target | `paths.rs` | `roundtrip.rs` |
|---|:---:|:---:|
| `cross_format_tests` | 是 | 否 |
| `fcs_tests` | 是 | 否 |
| `pgr_tests` | 是 | 是 |
| `rpe_tests` | 是 | 是 |
| `pec_tests` | 是 | 是 |
| `copyright_tests` | 是 | 是 |

这样每一个 test binary 只编译实际依赖的 helper，Clippy 能继续识别未来真正无用的测试代码。

### 版权 fixture 的 opt-in 测试 lane

在 `crates/fcs-converter/Cargo.toml` 增加不引入依赖的 feature 和显式 test target：

```toml
[features]
copyright-fixtures = []

[[test]]
name = "copyright_tests"
path = "tests/copyright_tests.rs"
required-features = ["copyright-fixtures"]
```

因此默认 workspace Clippy 和 nextest 不构建或执行 `copyright_tests`；这不是跳过失败，而是把依赖外部、未跟踪数据的验证移到明确的 opt-in lane。

版权测试启用 feature 后的 fixture 路径优先级为：

1. 非空 `FCS_COPYRIGHT_DIR` 环境变量；
2. `examples/COPYRIGHT`。

若启用 `copyright-fixtures` 但两个位置都没有可读 fixture，测试必须失败并报出明确的准备说明。不得自动成功或跳过。

本地或拥有 fixture 的 CI 使用：

```text
$env:FCS_COPYRIGHT_DIR = 'D:\chart-fixtures\COPYRIGHT'
cargo nextest run -p fcs-converter --features copyright-fixtures --test copyright_tests
```

该 lane 保持现有解析、同格式 round-trip、note count 和小文件 200 点事件采样比较；只改变它的启动条件和路径来源。

### Phase 1 状态更新

实现并验证默认 gate 后：

- roadmap 中 Phase 1 状态从 `Implemented; workspace validation gates pending` 更新为 `Complete`；
- Phase 1 completion checklist 中“Existing v4 converter and tests still compile and pass”与“Workspace Clippy, nextest and rustfmt checks pass”标记完成；
- “Phase closure blockers”替换为“Optional copyright-fixture validation”，记录 feature、环境变量和命令；
- 默认 gate 的通过不宣称版权 fixture lane 已运行。该 lane 在有 fixture 的环境中仍必须单独报告结果。

## 验证策略

实施按以下顺序验证：

1. 先执行 workspace Clippy，确认所有默认 test target 无 warning-as-error；
2. 再执行 workspace nextest，确认所有默认 test target 通过；
3. 检查 rustfmt；
4. 验证 `copyright_tests` 未启用 feature 时不出现在默认 nextest target 列表；
5. 在没有 fixture 的环境中启用 feature，确认失败信息指出 `FCS_COPYRIGHT_DIR` 与回退目录；
6. 仅当版权 fixture 可用时，运行 opt-in lane 并保留其实际结果。

## 完成条件

Phase 1 默认 workspace gate 关闭需要同时满足：

- 默认 workspace Clippy 退出码为零；
- 默认 workspace nextest 退出码为零；
- rustfmt 检查退出码为零；
- 默认测试集中不包含需要私有版权 fixture 的 test target；
- 文档准确区分默认 gate 与 opt-in 版权验证。

达到这些条件后，才能开始 Phase 2 的 compile-time language 详细设计与实施计划。
