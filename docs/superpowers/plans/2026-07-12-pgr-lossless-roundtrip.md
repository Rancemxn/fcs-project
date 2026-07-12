# PGR Lossless Round-Trip 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 修复 PGR→FCS→PGR 反向转换链，消除 half-split 引入的线性信息丢失，使 round-trip 做到数学无损。

**架构：** 三层修复：(1) to_fcs.rs 用 easeLinear Call 替代 half-split，(2) pgr_writer.rs 对单区间窗口做端值评估取代 midpoint-sampling，(3) main.rs 用递归 fmt_expr 替代只能处 Literal 的 fmt_lit_expr。同时移除不再使用的 junction_beats。

**技术栈：** Rust, FCS AST (Expression::Call, easeLinear evaluator), PGR writer

---

### 任务 1：移除 junction_beats（死代码清理）

**说明：** `junction_beats` 已无任消费，从 AST、parser、CLI 序列化、autofill 中完整移除。

**涉及文件：**
- 修改：`crates/fcs-core/src/ast/line.rs:36`
- 修改：`crates/fcs-core/src/parser/block.rs:269-292`
- 修改：`crates/fcs-converter/src/to_fcs.rs:61-63, 184-188`
- 修改：`crates/fcs-converter/src/from_fcs/autofill.rs:69`
- 修改：`crates/fcs-cli/src/main.rs:356-366`

- [ ] **步骤 1：从 AST `MotionLayer` 删除 `junction_beats` 字段**

`ast/line.rs:34-44`：
```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionLayer {
    // 删除: pub junction_beats: Vec<f64>,
    pub position_x: Vec<MotionInterval>,
    pub position_y: Vec<MotionInterval>,
    pub rotation: Vec<MotionInterval>,
    pub alpha: Vec<MotionInterval>,
    pub scale_x: Vec<MotionInterval>,
    pub scale_y: Vec<MotionInterval>,
    pub speed: Vec<MotionInterval>,
}
```

`Default` derive 会自动处理剩余字段。

- [ ] **步骤 2：从 parser 删除 `parse_junction_beats`**

`parser/block.rs:269-292`：
删除整个 `parse_junction_beats` 函数（L269-281），并从 `many0(alt((` 列表（L289-306）中删除 `map(parse_junction_beats, ...)` 那一支。

删除 L269-281：
```rust
// 删除此函数
fn parse_junction_beats(input: &str) -> IResult<&str, Vec<f64>> { ... }
```

L289 alt 中删除：
```rust
// 删除这一支
map(parse_junction_beats, |beats| {
    layer.junction_beats = beats;
}),
```

- [ ] **步骤 3：从 `to_fcs.rs` 删除 junction_beats 相关代码**

`to_fcs.rs:61-63`：
```rust
// 删除以下三行
// Collect original event boundaries for junction_beats
layer.junction_beats.push(e.start_beat);
layer.junction_beats.push(e.end_beat);
```

`to_fcs.rs:184-188`：
```rust
// 删除以下五行
// Sort and deduplicate junction beats
layer
    .junction_beats
    .sort_by(|a, b| a.partial_cmp(b).unwrap());
layer.junction_beats.dedup();
```

- [ ] **步骤 4：从 `autofill.rs` 删除 junction_beats clone**

`autofill.rs:69`：
```rust
// 删除这一行
junction_beats: layer.junction_beats.clone(),
```

- [ ] **步骤 5：从 `main.rs` CLI 序列化删除 junction_beats 输出**

`main.rs:356-366`：
```rust
// 删除整个 if 块
if !layer.junction_beats.is_empty() {
    let beats: Vec<String> = layer
        .junction_beats
        .iter()
        .map(|b| format!("{}b", b))
        .collect();
    o.push_str(&format!(
        "                junctionBeats: [{}];\n",
        beats.join(", ")
    ));
}
```

- [ ] **步骤 6：编译验证删除无误**

运行：`cargo check`。预期：编译通过，无 warning。

- [ ] **步骤 7：Commit**

```bash
git add crates/fcs-core/src/ast/line.rs crates/fcs-core/src/parser/block.rs crates/fcs-converter/src/to_fcs.rs crates/fcs-converter/src/from_fcs/autofill.rs crates/fcs-cli/src/main.rs
git commit -m "refactor: remove unused junction_beats field

No consumers remain — all writers use per-field collect_beat_boundaries instead.
Removes from AST, parser, CLI serialization, and autofill."
```

---

### 任务 2：to_fcs.rs — 用 easeLinear Call 替代 half-split

**说明：** PGR 线性事件 `[st, et, v0, v1]` 不再拆成两段常值区间，而是发射 `easeLinear(b, st, et, v0, v1, 0, 1)` Call 表达式。

**涉及文件：**
- 修改：`crates/fcs-converter/src/to_fcs.rs:54-80`

- [ ] **步骤 1：重写 `push_motion_interval`**

`to_fcs.rs:54-80`：
```rust
fn push_motion_interval(
    layer: &mut MotionLayer,
    field: MotionField,
    e: &IrEvent,
    end_expr: Expression,
    start_expr: Expression,
) {
    let end = if e.end_beat > e.start_beat {
        e.end_beat
    } else {
        e.start_beat + EPS
    };
    // PGR events with start ≠ end use linear interpolation over [start_beat, end_beat].
    // Emit an easeLinear call so the linear transition is preserved in FCS,
    // enabling lossless round-trip through the PGR writer.
    if (e.start_value - e.end_value).abs() > 1e-10 {
        let ease_call = Expression::Call {
            name: "easeLinear".into(),
            args: vec![
                Expression::Variable("b".into()),
                q_time_beat(e.start_beat),
                q_time_beat(end),
                start_expr,
                end_expr,
                Expression::Literal(Literal::Float(0.0)),
                Expression::Literal(Literal::Float(1.0)),
            ],
        };
        push_to_layer(layer, field, e.start_beat, end, &ease_call);
    } else {
        push_to_layer(layer, field, e.start_beat, end, &end_expr);
    }
}
```

注意：删除之前 L61-63 的 `junction_beats.push` 已在任务 1 中完成。

- [ ] **步骤 2：编译验证**

运行：`cargo check`。预期：编译通过。

- [ ] **步骤 3：Commit**

```bash
git add crates/fcs-converter/src/to_fcs.rs
git commit -m "feat(converter): use easeLinear call instead of half-split in push_motion_interval

Preserves linear interpolation information in FCS AST, enabling
lossless PGR round-trip."
```

---

### 任务 3：pgr_writer.rs — 端值评估取代 midpoint-sampling

**说明：** 对每个 `[w0, w1]` 窗口，检查覆盖的 MotionInterval 数量。如果只覆盖一个，在 w0 和 w1 分别 evaluate 表达式取端值（而非在中点采样取常值）。

**涉及文件：**
- 修改：`crates/fcs-converter/src/from_fcs/pgr_writer.rs:388-471`

- [ ] **步骤 1：新增辅助函数 `covering_endpoints`（在 `sample_at` 旁边）**

`pgr_writer.rs`，在 `sample_at` 函数之后添加：
```rust
/// For a window [window_start, window_end], check if exactly one interval
/// covers the entire window. If so, evaluate the expression at both endpoints
/// and return (start_value, end_value). Otherwise return None (caller should
/// fall back to midpoint sampling).
fn covering_endpoints(
    intervals: &[MotionInterval],
    window_start: f64,
    window_end: f64,
    env: &EvalEnv,
) -> Option<(f64, f64)> {
    let covering: Vec<&MotionInterval> = intervals
        .iter()
        .filter(|iv| iv.start_beat <= window_start && iv.end_beat >= window_end)
        .collect();
    if covering.len() == 1 {
        let expr = &covering[0].expression;
        let mut es = *env;
        es.beat = window_start;
        let sv = eval_expr(expr, &es);
        es.beat = window_end;
        let ev = eval_expr(expr, &es);
        Some((sv, ev))
    } else {
        None
    }
}
```

- [ ] **步骤 2：修改 move_x/move_y 窗口处理**

`pgr_writer.rs:403-424`，替换 `move_beats.windows(2).map(...)` 闭包：

```rust
let moves: Vec<PgrEvent> = move_beats
    .windows(2)
    .map(|w| {
        let (xv_s, xv_e) = covering_endpoints(&x_ivs, w[0], w[1], &es)
            .unwrap_or_else(|| {
                let bm = (w[0] + w[1]) * 0.5;
                es.beat = bm;
                es.seconds = time::beat_to_seconds(bm, bpm);
                let v = sample_at(&x_ivs, bm, 0.0, &es);
                (v, v)
            });
        let (yv_s, yv_e) = covering_endpoints(&y_ivs, w[0], w[1], &es)
            .unwrap_or_else(|| {
                let bm = (w[0] + w[1]) * 0.5;
                es.beat = bm;
                es.seconds = time::beat_to_seconds(bm, bpm);
                let v = sample_at(&y_ivs, bm, 0.0, &es);
                (v, v)
            });
        let pgr_xs = (coord::fcs_px_to_rpe_x(xv_s) + 675.0) / 1350.0;
        let pgr_xe = (coord::fcs_px_to_rpe_x(xv_e) + 675.0) / 1350.0;
        let pgr_ys = (coord::fcs_px_to_rpe_y(yv_s) + 450.0) / 900.0;
        let pgr_ye = (coord::fcs_px_to_rpe_y(yv_e) + 450.0) / 900.0;
        PgrEvent {
            start_time: time::beat_to_pgr_t(w[0]),
            end_time: time::beat_to_pgr_t(w[1]),
            start: pgr_xs,
            end: pgr_xe,
            start2: pgr_ys,
            end2: pgr_ye,
            value: 1.0,
        }
    })
    .collect();
```

- [ ] **步骤 3：修改 rotate 窗口处理**

`pgr_writer.rs:428-446`，替换 `rot_beats.windows(2).map(...)` 闭包：

```rust
let rots: Vec<PgrEvent> = rot_beats
    .windows(2)
    .map(|w| {
        let (sv, ev) = covering_endpoints(&rot_ivs, w[0], w[1], &es)
            .unwrap_or_else(|| {
                let bm = (w[0] + w[1]) * 0.5;
                es.beat = bm;
                es.seconds = time::beat_to_seconds(bm, bpm);
                let v = sample_at(&rot_ivs, bm, 0.0, &es);
                (v, v)
            });
        PgrEvent {
            start_time: time::beat_to_pgr_t(w[0]),
            end_time: time::beat_to_pgr_t(w[1]),
            start: -sv,
            end: -ev,
            start2: 0.0,
            end2: 0.0,
            value: 1.0,
        }
    })
    .collect();
```

- [ ] **步骤 4：修改 alpha 窗口处理**

`pgr_writer.rs:449-468`，替换 `alpha_beats.windows(2).map(...)` 闭包：

```rust
let alphas: Vec<PgrEvent> = alpha_beats
    .windows(2)
    .map(|w| {
        let (sv, ev) = covering_endpoints(&alpha_ivs, w[0], w[1], &es)
            .unwrap_or_else(|| {
                let bm = (w[0] + w[1]) * 0.5;
                es.beat = bm;
                es.seconds = time::beat_to_seconds(bm, bpm);
                let v = sample_at(&alpha_ivs, bm, 1.0, &es);
                (v, v)
            });
        PgrEvent {
            start_time: time::beat_to_pgr_t(w[0]),
            end_time: time::beat_to_pgr_t(w[1]),
            start: sv,
            end: ev,
            start2: 0.0,
            end2: 0.0,
            value: 1.0,
        }
    })
    .collect();
```

- [ ] **步骤 5：编译验证**

运行：`cargo check`。预期：编译通过。

- [ ] **步骤 6：Commit**

```bash
git add crates/fcs-converter/src/from_fcs/pgr_writer.rs
git commit -m "feat(converter): use covering_endpoints for PGR event generation

Replaces midpoint-sampling with endpoint evaluation for single-interval
windows, preserving start≠end linear events through the round-trip."
```

---

### 任务 4：main.rs — fmt_lit_expr → 递归 fmt_expr

**说明：** 将 `fmt_lit_expr` 升级为递归处理所有 Expression 变体的 `fmt_expr`，使 CLI 能序列化 `Expression::Call`（如 easeLinear 函数调用）。

**涉及文件：**
- 修改：`crates/fcs-cli/src/main.rs:380, 412, 418-423`

- [ ] **步骤 1：添加符号辅助函数**

在 `main.rs` 的 `fmt_expr` 函数前添加：

```rust
fn binop_symbol(op: &fcs_core::ast::BinaryOp) -> &'static str {
    use fcs_core::ast::BinaryOp;
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Pow => "^",
    }
}

fn unop_symbol(op: &fcs_core::ast::UnaryOp) -> &'static str {
    use fcs_core::ast::UnaryOp;
    match op {
        UnaryOp::Neg => "-",
    }
}
```

- [ ] **步骤 2：将 `fmt_lit_expr` 替换为递归 `fmt_expr`**

`main.rs:418-423` 替换为：
```rust
fn fmt_expr(e: &fcs_core::ast::Expression) -> String {
    use fcs_core::ast::Expression;
    match e {
        Expression::Literal(lit) => fmt_literal(lit),
        Expression::Variable(name) => name.clone(),
        Expression::BinaryOp { op, left, right } => {
            format!("({} {} {})", fmt_expr(left), binop_symbol(op), fmt_expr(right))
        }
        Expression::UnaryOp { op, operand } => {
            format!("({}{})", unop_symbol(op), fmt_expr(operand))
        }
        Expression::Call { name, args } => {
            let args: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("{}({})", name, args.join(", "))
        }
        Expression::Ternary { cond, if_true, if_false } => {
            format!(
                "({} ? {} : {})",
                fmt_expr(cond),
                fmt_expr(if_true),
                fmt_expr(if_false)
            )
        }
        Expression::ChainCompare { left, ops } => {
            let mut s = fmt_expr(left);
            for (op, expr) in ops {
                s = format!("{} {} {}", s, op.as_str(), fmt_expr(expr));
            }
            s
        }
    }
}
```

- [ ] **步骤 3：更新所有调用点**

`main.rs:380`：`fmt_lit_expr` → `fmt_expr`
```rust
let expr_str = fmt_expr(&intv.expression);
```

`main.rs:412`：`fmt_lit_expr` → `fmt_expr`
```rust
fcs_core::ast::NotePropertyValue::Expr(e) => fmt_expr(e),
```

- [ ] **步骤 4：编译验证**

运行：`cargo check`。预期：编译通过。

- [ ] **步骤 5：Commit**

```bash
git add crates/fcs-cli/src/main.rs
git commit -m "feat(cli): replace fmt_lit_expr with recursive fmt_expr

Handles all Expression variants (Call, BinaryOp, UnaryOp, Ternary,
ChainCompare) instead of only Literal. Enables CLI serialization of
easing function calls."
```

---

### 任务 5：收紧测试 tolerance + 全量验证

**说明：** PGR round-trip 的 rotate tolerance 从 1.0° 收紧到 0.001°，证明无损。

**涉及文件：**
- 修改：`crates/fcs-converter/tests/roundtrip.rs:69-73, 140-144`

- [ ] **步骤 1：收紧 tolerance**

`roundtrip.rs`，两处：
```rust
common::EventTolerances {
    rotate: 0.001,  // 从 1.0 收紧到 0.001
    ..Default::default()
},
```

- [ ] **步骤 2：运行全量测试**

```bash
cargo nextest run
```

预期：所有测试通过。

- [ ] **步骤 3：运行 clippy**

```bash
cargo clippy -- -D warnings
```

预期：零 warning。

- [ ] **步骤 4：cargo fmt**

```bash
cargo fmt
```

- [ ] **步骤 5：Commit**

```bash
git add crates/fcs-converter/tests/roundtrip.rs
git commit -m "test: tighten PGR round-trip rotate tolerance to 0.001

The easeLinear fix and endpoint evaluation make the round-trip
mathematically lossless for linear events."
```
