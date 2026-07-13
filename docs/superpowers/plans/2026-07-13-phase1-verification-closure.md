# FCS 5 Phase 1 验证闭环实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让干净 checkout 的默认 workspace Clippy、nextest 与 rustfmt 验证可重复通过，同时将未跟踪版权谱面的 round-trip 验证保留为显式 opt-in 测试 lane。

**Architecture:** 将 converter integration-test 的共享逻辑拆成按需加载的 `paths` 与 `roundtrip` 模块，避免每个 test binary 编译未使用 helper。使用 Cargo `required-features` 把依赖外部版权数据的 `copyright_tests` 从默认 target 集移出；feature 启用后通过 `FCS_COPYRIGHT_DIR` 或忽略目录运行同一套严格验证。

**Tech Stack:** Rust 2024、Cargo feature/test target、Cargo Clippy、cargo-nextest、rustfmt、PowerShell。

---

## 分支准备

本计划不使用 worktree。开始前确认根工作区干净，并从 `master` 创建普通分支：

```text
git status --short
git switch -c codex/phase1-verification-closure
```

`git status --short` 的预期输出为空。

## 文件结构

| 文件 | 操作 | 职责 |
|---|---|---|
| `crates/fcs-converter/tests/common/mod.rs` | 删除 | 取消所有 integration-test 都加载的聚合模块。 |
| `crates/fcs-converter/tests/common/paths.rs` | 新建 | 仓库相对路径解析。 |
| `crates/fcs-converter/tests/common/roundtrip.rs` | 新建 | 仅供 round-trip 测试使用的事件采样比较。 |
| `crates/fcs-converter/tests/{cross_format_tests,fcs_tests,pgr_tests,rpe_tests,pec_tests,copyright_tests}.rs` | 修改 | 通过 `#[path]` 仅导入实际需要的 helper。 |
| `crates/fcs-converter/Cargo.toml` | 修改 | 定义 `copyright-fixtures` feature 和带 `required-features` 的 test target。 |
| `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md` | 修改 | Phase 1 状态更新为 `Complete`。 |
| `docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md` | 修改 | 关闭默认验证 checklist，记录 opt-in 版权 lane。 |

### Task 1: 建立按需加载的测试 helper

**Files:**

- Create: `crates/fcs-converter/tests/common/paths.rs`
- Create: `crates/fcs-converter/tests/common/roundtrip.rs`
- Delete: `crates/fcs-converter/tests/common/mod.rs`
- Modify: `crates/fcs-converter/tests/cross_format_tests.rs`
- Modify: `crates/fcs-converter/tests/fcs_tests.rs`
- Modify: `crates/fcs-converter/tests/pgr_tests.rs`
- Modify: `crates/fcs-converter/tests/rpe_tests.rs`
- Modify: `crates/fcs-converter/tests/pec_tests.rs`

- [ ] **Step 1: 记录当前 Clippy 失败基线**

Run:

```text
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: FAIL. `fcs-converter` integration test binary reports unused `sample_event_value`、`chart_time_range`、`EventTolerances`、`compare_events_sampled` 或 `compare_notes_exact`。

- [ ] **Step 2: 创建只含路径职责的 helper**

Create `crates/fcs-converter/tests/common/paths.rs` with exactly:

```rust
/// Resolve a path relative to the project root from the converter crate.
pub fn manifest_path(rel: &str) -> String {
    let dir = env!("CARGO_MANIFEST_DIR");
    let full = std::path::Path::new(dir).join("../../").join(rel);
    full.to_string_lossy().to_string()
}
```

- [ ] **Step 3: 创建只含 round-trip 比较职责的 helper**

Create `crates/fcs-converter/tests/common/roundtrip.rs` with exactly:

```rust
use fcs_converter::ir::*;

/// Linear interpolation: value at time_beat given a time-sorted event list.
pub fn sample_event_value(events: &[IrEvent], time_beat: f64) -> Option<f64> {
    let idx = events.partition_point(|e| e.start_beat <= time_beat);
    let idx = idx.checked_sub(1)?;
    let e = &events[idx];

    if (e.start_beat - time_beat).abs() < 1e-12 && e.start_beat == e.end_beat {
        return Some(e.start_value);
    }

    if time_beat >= e.start_beat && time_beat < e.end_beat {
        if (e.end_beat - e.start_beat).abs() < 1e-12 {
            return Some(e.start_value);
        }
        let t = (time_beat - e.start_beat) / (e.end_beat - e.start_beat);
        return Some(e.start_value + (e.end_value - e.start_value) * t);
    }

    None
}

/// Find the earliest event or note start and the latest event or note end.
pub fn chart_time_range(chart: &IrChart) -> (f64, f64) {
    let mut min_t = f64::MAX;
    let mut max_t = f64::MIN;
    for line in &chart.lines {
        for note in line.notes_above.iter().chain(&line.notes_below) {
            let end = note.time_beat + note.hold_beat;
            if note.time_beat < min_t {
                min_t = note.time_beat;
            }
            if end > max_t {
                max_t = end;
            }
        }
        let bundle = &line.events;
        for ev in bundle
            .speed
            .iter()
            .chain(&bundle.move_x)
            .chain(&bundle.move_y)
            .chain(&bundle.rotate)
            .chain(&bundle.alpha)
            .chain(&bundle.scale_x)
            .chain(&bundle.scale_y)
            .chain(&bundle.color)
        {
            if ev.start_beat < min_t {
                min_t = ev.start_beat;
            }
            if ev.end_beat > max_t {
                max_t = ev.end_beat;
            }
        }
    }
    if min_t == f64::MAX {
        (0.0, 0.0)
    } else {
        (min_t, max_t)
    }
}

/// Per-event-type tolerance limits for sampled validation.
pub struct EventTolerances {
    pub move_x: f64,
    pub move_y: f64,
    pub rotate: f64,
    pub alpha: f64,
    pub speed: f64,
}

/// Compare event values by sampling evenly spaced beat times.
pub fn compare_events_sampled(
    orig: &IrChart,
    rt: &IrChart,
    num_samples: usize,
    tol: EventTolerances,
) {
    let (t_start, t_end) = chart_time_range(orig);
    let rt_range = chart_time_range(rt);
    let t_end = t_end.max(rt_range.1);
    let span = if (t_end - t_start).abs() < 1e-12 {
        1.0
    } else {
        t_end - t_start
    };

    let sample_times: Vec<f64> = (0..num_samples)
        .map(|i| t_start + span * (i as f64 / (num_samples.max(1) - 1) as f64))
        .collect();

    let mut max_move_x = 0.0f64;
    let mut max_move_y = 0.0f64;
    let mut max_rotate = 0.0f64;
    let mut max_alpha = 0.0f64;
    let mut max_speed = 0.0f64;

    for (ol, rl) in orig.lines.iter().zip(&rt.lines) {
        for &t in &sample_times {
            if let Some(ov) = sample_event_value(&ol.events.move_x, t)
                && let Some(rv) = sample_event_value(&rl.events.move_x, t)
            {
                max_move_x = max_move_x.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.move_y, t)
                && let Some(rv) = sample_event_value(&rl.events.move_y, t)
            {
                max_move_y = max_move_y.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.rotate, t)
                && let Some(rv) = sample_event_value(&rl.events.rotate, t)
            {
                max_rotate = max_rotate.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.alpha, t)
                && let Some(rv) = sample_event_value(&rl.events.alpha, t)
            {
                max_alpha = max_alpha.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.speed, t)
                && let Some(rv) = sample_event_value(&rl.events.speed, t)
            {
                max_speed = max_speed.max((ov - rv).abs());
            }
        }
    }

    assert!(
        max_move_x < tol.move_x,
        "moveX sampled max diff {max_move_x:.4} >= tolerance {}",
        tol.move_x
    );
    assert!(
        max_move_y < tol.move_y,
        "moveY sampled max diff {max_move_y:.4} >= tolerance {}",
        tol.move_y
    );
    assert!(
        max_rotate < tol.rotate,
        "rotate sampled max diff {max_rotate:.4} >= tolerance {}",
        tol.rotate
    );
    assert!(
        max_alpha < tol.alpha,
        "alpha sampled max diff {max_alpha:.4} >= tolerance {}",
        tol.alpha
    );
    assert!(
        max_speed < tol.speed,
        "speed sampled max diff {max_speed:.4} >= tolerance {}",
        tol.speed
    );
}
```

Do not add `impl Default for EventTolerances` or `compare_notes_exact`: neither has a call site.

- [ ] **Step 4: Replace broad `mod common` imports with selective modules**

At the top of `cross_format_tests.rs` and `fcs_tests.rs`, replace:

```rust
mod common;
```

with:

```rust
#[path = "common/paths.rs"]
mod paths;
```

Replace every `common::manifest_path` with `paths::manifest_path` in those two files.

At the top of `pgr_tests.rs`, `rpe_tests.rs`, and `pec_tests.rs`, replace the same declaration with:

```rust
#[path = "common/paths.rs"]
mod paths;
#[path = "common/roundtrip.rs"]
mod roundtrip;
```

In those three files, replace `common::manifest_path` with `paths::manifest_path`, `common::compare_events_sampled` with `roundtrip::compare_events_sampled`, and `common::EventTolerances` with `roundtrip::EventTolerances`.

Delete `crates/fcs-converter/tests/common/mod.rs` after every default test target has a direct replacement module.

- [ ] **Step 5: Run targeted converter tests and Clippy**

Run in this order:

```text
cargo clippy -p fcs-converter --test cross_format_tests --test fcs_tests --test pgr_tests --test rpe_tests --test pec_tests -- -D warnings
cargo nextest run -p fcs-converter --test cross_format_tests --test fcs_tests --test pgr_tests --test rpe_tests --test pec_tests
```

Expected: Clippy succeeds without `dead_code`; all tracked-fixture converter tests pass.

- [ ] **Step 6: Commit selective helper loading**

```text
git add crates/fcs-converter/tests/common crates/fcs-converter/tests/cross_format_tests.rs crates/fcs-converter/tests/fcs_tests.rs crates/fcs-converter/tests/pgr_tests.rs crates/fcs-converter/tests/rpe_tests.rs crates/fcs-converter/tests/pec_tests.rs
git commit -m "test: split converter integration helpers"
```

### Task 2: Make copyright fixture validation an explicit Cargo lane

**Files:**

- Modify: `crates/fcs-converter/Cargo.toml`
- Modify: `crates/fcs-converter/tests/copyright_tests.rs`

- [ ] **Step 1: Record that copyright tests currently run by default**

Run:

```text
cargo nextest list --workspace
```

Expected: output contains all three `fcs-converter::copyright_tests` entries before feature-gating.

- [ ] **Step 2: Add the feature-gated integration-test target**

Append this exact configuration after `[dependencies]` in `crates/fcs-converter/Cargo.toml`:

```toml
[features]
copyright-fixtures = []

[[test]]
name = "copyright_tests"
path = "tests/copyright_tests.rs"
required-features = ["copyright-fixtures"]
```

- [ ] **Step 3: Make copyright tests use the selective modules and configurable fixture directory**

Replace the top of `crates/fcs-converter/tests/copyright_tests.rs` with:

```rust
//! Dynamic scan of local community charts.
//!
//! This test target is opt-in: run it with `--features copyright-fixtures`.
//! Set `FCS_COPYRIGHT_DIR` to a private fixture directory, or populate the
//! ignored `examples/COPYRIGHT` fallback directory.

#[path = "common/paths.rs"]
mod paths;
#[path = "common/roundtrip.rs"]
mod roundtrip;

use std::path::{Path, PathBuf};
```

Delete the unused `parse_any` function. Add this helper immediately after `const SAMPLED_SIZE_LIMIT`:

```rust
fn copyright_dir() -> PathBuf {
    let dir = match std::env::var("FCS_COPYRIGHT_DIR") {
        Ok(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => PathBuf::from(paths::manifest_path("examples/COPYRIGHT")),
    };
    assert!(
        dir.is_dir(),
        "COPYRIGHT fixture directory missing: {}. Set FCS_COPYRIGHT_DIR or populate examples/COPYRIGHT.",
        dir.display()
    );
    dir
}
```

In each copyright test, replace:

```rust
let dir = Path::new(&common::manifest_path("examples/COPYRIGHT")).to_path_buf();
```

with:

```rust
let dir = copyright_dir();
```

Replace `common::compare_events_sampled` with `roundtrip::compare_events_sampled` and `common::EventTolerances` with `roundtrip::EventTolerances`. Keep `Path` imported because `file_size` still accepts `&Path`.

- [ ] **Step 4: Verify the default target list no longer includes copyright tests**

Run:

```text
cargo nextest list --workspace
```

Expected: output does not contain `copyright_tests`.


- [ ] **Step 5: Verify opt-in missing-fixture diagnostics**

Run in PowerShell with an explicit nonexistent override:

```text
$env:FCS_COPYRIGHT_DIR = (Join-Path $env:TEMP 'fcs-missing-copyright-fixtures')
cargo nextest run -p fcs-converter --features copyright-fixtures --test copyright_tests
Remove-Item Env:FCS_COPYRIGHT_DIR -ErrorAction SilentlyContinue
```

Expected: FAIL in `test_copyright_files_exist` with an error containing both `FCS_COPYRIGHT_DIR` and `examples/COPYRIGHT`. The failure proves the opt-in lane does not silently skip missing private data.

- [ ] **Step 6: Commit the opt-in copyright lane**

```text
git add crates/fcs-converter/Cargo.toml crates/fcs-converter/tests/copyright_tests.rs
git commit -m "test: gate copyright fixtures behind feature"
```

### Task 3: Close the default Phase 1 validation gate

**Files:**

- Modify: `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md`
- Modify: `docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md`

- [ ] **Step 1: Run the complete default validation sequence**

Run in this exact order:

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
```

Expected: all three commands exit with code `0`. `cargo nextest run --workspace` must not list or execute `copyright_tests`.

- [ ] **Step 2: Update the Phase 1 roadmap status**

In `docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md`, change the Phase 1 status cell from:

```text
Implemented; workspace validation gates pending
```

to:

```text
Complete
```

- [ ] **Step 3: Update the Phase 1 checklist and replace obsolete blocker text**

In `docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md`:

1. Mark these two entries complete:

```markdown
- [x] Existing v4 converter and tests still compile and pass.
- [x] Workspace Clippy, nextest and rustfmt checks pass.
```

2. Replace the `## Phase closure blockers` section with:

````markdown
## Optional copyright-fixture validation

The default workspace gate uses only tracked fixtures. To validate private
community charts, enable the dedicated test lane and provide a fixture path:

```text
$env:FCS_COPYRIGHT_DIR = 'D:\chart-fixtures\COPYRIGHT'
cargo nextest run -p fcs-converter --features copyright-fixtures --test copyright_tests
```

When the feature is enabled, a missing `FCS_COPYRIGHT_DIR` and missing
`examples/COPYRIGHT` fallback directory is an explicit test failure.
````

- [ ] **Step 4: Commit the Phase 1 closure record**

```text
git add docs/superpowers/plans/2026-07-13-fcs5-implementation-roadmap.md docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md
git commit -m "docs: close FCS 5 Phase 1 validation"
```

### Task 4: Report optional-fixture coverage separately

**Files:**

- Verify: `crates/fcs-converter/tests/copyright_tests.rs`

- [ ] **Step 1: Run the opt-in lane only when a fixture directory is available**

Run:

```text
$env:FCS_COPYRIGHT_DIR = 'D:\chart-fixtures\COPYRIGHT'
cargo nextest run -p fcs-converter --features copyright-fixtures --test copyright_tests
```

Expected: record the actual result. Do not claim this lane passed unless the command exits with code `0` against an available fixture directory.

- [ ] **Step 2: Report default and optional validation independently**

The completion report must distinguish:

```text
Default workspace gate: cargo clippy / cargo nextest / cargo fmt
Optional copyright-fixture lane: feature-enabled copyright_tests
```

Do not change Phase 1's default-gate completion status based on the absence of private fixture data.

## Plan self-review

- Spec coverage: Task 1 implements selective helper compilation; Task 2 implements the opt-in external-fixture lane; Task 3 closes the documented default gate; Task 4 preserves separate reporting for private-data validation.
- Placeholder scan: the plan has no deferred implementation markers; `roundtrip.rs` contains complete function bodies and exact tolerance assertions.
- Consistency: all test files use `paths` for manifest resolution and only round-trip targets use `roundtrip`; the Cargo feature and the test command both use the exact name `copyright-fixtures`.
