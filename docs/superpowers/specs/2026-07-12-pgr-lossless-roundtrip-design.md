# PGR Lossless Round-Trip — Design Document

## Problem

PGR → FCS → PGR round-trip currently loses linear interpolation information.
Three-level root cause chain:

1. **`to_fcs.rs:73-76` — half-split**: PGR linear events `[st, et, v0, v1]`
   are split into two piecewise-constant FCS intervals at the midpoint,
   destroying slope information.
2. **`pgr_writer.rs` — midpoint-sampling**: Each output PGR event sets
   `start = end = value_at_midpoint`, producing constant events even when
   the FCS expression encodes linear change.
3. **`main.rs:418-423` — `fmt_lit_expr`**: Non-literal expressions
   (easing calls) serialize as `"?"`, producing invalid FCS output.

## Solution: Approach B

Spread across 5 files, ~110 lines net change. Zero negative side effects;
all three writers (PGR, RPE, PEC) benefit from the half-split fix.

### 1. `to_fcs.rs` — emit `easeLinear` Call (was: half-split)

**File**: `crates/fcs-converter/src/to_fcs.rs`
**Function**: `push_motion_interval()`

Replace the half-split branch with an `easeLinear` expression call:

```rust
// BEFORE (lossy):
if (e.start_value - e.end_value).abs() > 1e-10 {
    let mid = (e.start_beat + end) * 0.5;
    push_to_layer(layer, field, e.start_beat, mid, &start_expr);
    push_to_layer(layer, field, mid, end, &end_expr);
}

// AFTER (lossless for linear):
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
}
```

**Effect**: A single MotionInterval with easing expression replaces two
constant intervals. All downstream consumers see correct linear
interpolation.

**Removal**: Delete the `junction_beats.push()` calls (L61-63) and the
post-loop sort+dedup (L184-188) — no longer consumed by any writer.

### 2. `pgr_writer.rs` — endpoint evaluation for single-interval windows

**File**: `crates/fcs-converter/src/from_fcs/pgr_writer.rs`
**Functions**: `motion_to_move_rotate_alpha()` internals

For each `[w0, w1]` window produced by `collect_beat_boundaries`, evaluate
the covering interval expression at both endpoints:

```rust
let covering: Vec<_> = intervals.iter()
    .filter(|iv| iv.start_beat <= w[0] && iv.end_beat >= w[1])
    .collect();

let (sv, ev) = if covering.len() == 1 {
    let expr = &covering[0].expression;
    let mut es = *env;
    es.beat = w[0];
    let sv = eval_expr(expr, &es);
    es.beat = w[1];
    let ev = eval_expr(expr, &es);
    (sv, ev)
} else {
    let bm = (w[0] + w[1]) * 0.5;
    let v = sample_at(&intervals, bm, default, env);
    (v, v)
};
```

Then output `start=sv, end=ev` (after coordinate conversion) instead of
`start=mid, end=mid`.

Applied to three fields in `motion_to_move_rotate_alpha`:
- **move_x / move_y**: replace `xv`/`yv` with per-window endpoint values
- **rotation**: replace `rv` with endpoint values, apply negation
- **alpha**: replace `av` with endpoint values

**Effect**: PGR events preserve their original `start ≠ end` form.
Round-trip is mathematically exact for `easeLinear`; constant intervals
degrade gracefully to `start = end` (current behavior).

### 3. `main.rs` — `fmt_lit_expr` → `fmt_expr`

**File**: `crates/fcs-cli/src/main.rs`
**Function**: `fmt_lit_expr` (L418-423)

Replace with a recursive `fmt_expr` that handles all Expression variants:

```rust
fn fmt_expr(e: &Expression) -> String {
    match e {
        Expression::Literal(lit) => fmt_literal(lit),
        Expression::Variable(name) => name.clone(),
        Expression::Call { name, args } => {
            let args: Vec<String> = args.iter().map(fmt_expr).collect();
            format!("{}({})", name, args.join(", "))
        }
        Expression::BinaryOp { op, left, right } => {
            format!("({} {} {})", fmt_expr(left), op.symbol(), fmt_expr(right))
        }
        Expression::UnaryOp { op, operand } => {
            format!("({}{})", op.symbol(), fmt_expr(operand))
        }
        Expression::Ternary { cond, if_true, if_false } => {
            format!("({} ? {} : {})", fmt_expr(cond), fmt_expr(if_true), fmt_expr(if_false))
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

Add `BinaryOp::symbol()` and `UnaryOp::symbol()` methods.
All call sites updated from `fmt_lit_expr` to `fmt_expr`.

### 4. `junction_beats` removal

**Files**: `ast/line.rs`, `parser/block.rs`, `to_fcs.rs`, `autofill.rs`

| File | Change |
|------|--------|
| `ast/line.rs` | Delete `junction_beats: Vec<f64>` field from `MotionLayer` |
| `ast/line.rs` | Remove from `Default` impl and all constructors |
| `parser/block.rs` | Delete `parse_junction_beats()` function |
| `to_fcs.rs` | Remove push calls and sort+dedup block |
| `autofill.rs` | Remove `junction_beats: layer.junction_beats.clone()` |

Rationale: `junction_beats` was introduced under the mistaken assumption
that PGR events must be scalar constants. The `collect_beat_boundaries()`
per-field approach replaced it in all writers. No consumer remains.

### 5. Test tolerance tightening

**File**: `crates/fcs-converter/tests/roundtrip.rs`

```rust
// BEFORE
EventTolerances {
    rotate: 1.0,
    ..Default::default()
}

// AFTER
EventTolerances {
    rotate: 0.001,
    ..Default::default()
}
```

## Scope Summary

| File | Lines changed | Type |
|------|--------------|------|
| `crates/fcs-converter/src/to_fcs.rs` | ~25 | Core fix |
| `crates/fcs-converter/src/from_fcs/pgr_writer.rs` | ~30 | Writer improvement |
| `crates/fcs-cli/src/main.rs` | ~40 | CLI expression serialization |
| `crates/fcs-core/src/ast/line.rs` | ~3 | Field removal |
| `crates/fcs-core/src/parser/block.rs` | ~10 | Parser removal |
| `crates/fcs-converter/src/from_fcs/autofill.rs` | ~1 | Clone removal |
| `crates/fcs-converter/tests/roundtrip.rs` | ~2 | Tolerance tightening |

**Total**: ~110 lines

## Side Effects Analysis

| Consumer | Effect |
|----------|--------|
| **PGR writer** | Events match count + values of original, `start≠end` preserved |
| **RPE writer** | `extract_easing()` recognizes `easeLinear` → correct easing in output |
| **PEC writer** | Fewer spurious midpoint cp/cd/ca/cv lines |
| **Eval env** | `eval_expr` already handles `Expression::Call` → `easeLinear` |
| **Parser** | Unchanged; easeLinear calls parse as standard Expression::Call |
| **fmt_expr** | New code handles all Expression variants correctly |
| **junction_beats** | Dead code removed, no regression |

## Test Plan

1. `test_pgr_roundtrip_small` — rotate tolerance drops to 0.001°
2. `test_pgr_roundtrip_medium` — rotate tolerance drops to 0.001°
3. All existing round-trip tests pass unchanged
4. `cargo test` — full suite green
5. `cargo clippy` — zero warnings
