# junctionBeats 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。
>
> **用户指令**: "现在分析下，fcs.md的fcs规范可以怎样改，优化，调整？尽量不引入太多东西。尽量小化修改。目标是PGR转FCS再转PGR是无损的。"

**目标：** 在 FCS 规范中为 Motion Layer 增加 `junctionBeats` 字段，使 PGR→FCS→PGR round-trip 达到无损（事件边界精确保留）。

**架构：** 在 `MotionLayer` 结构体中新增 `junction_beats: Vec<f64>`，解析器支持 `junctionBeats` 语法，`to_fcs.rs` 在创建 interval 时收集原始事件边界，`pgr_writer.rs` 在生成 PGR 事件时用 junctionBeats 替代 `build_beats()` 重采样。

**技术栈：** Rust, nom 解析器, serde_json

**受影响 API / 数据结构：**
| 符号 | 变更类型 | 说明 |
|------|---------|------|
| `fcs_core::ast::line::MotionLayer` | 加字段 | `pub junction_beats: Vec<f64>` |
| `fcs_core::parser::block::parse_motion_layer` | 扩展 | 解析 `junctionBeats: [...]` 语法 |
| `fcs_converter::to_fcs::push_motion_interval` | 扩展 | 事件边界 → `layer.junction_beats` |
| `fcs_converter::from_fcs::pgr_writer::motion_to_move_rotate_alpha` | 扩展 | 优先使用 junction_beats |
| `fcs_converter::from_fcs::autofill::autofill_layer` | 未变更 | 只补 interval，不碰 junction_beats |
| `fcs_converter::from_fcs::rpe_writer` | 未变更 | 无需重采样 |
| `fcs_converter::from_fcs::pec_writer` | 未变更 | 点事件直接由 interval 端点生成 |
| `fcs.md` (§5.5.2) | 扩展 | 增加 `junctionBeats` 字段定义和示例 |

---

### 任务 1：修改 fcs.md 规范

**文件：**
- 修改：`fcs.md:443-454`

- [ ] **步骤 1：在 `layer` 子块表中增加 `junctionBeats` 行**

位置：§5.5.2 `layer` 内支持的子块表（当前行 443-454），在 `speed` 行之后、表结尾之前插入：

```
| `junctionBeats` | 时间数组 | (无) | 否 | 可选，最多 4096 个，单调递增。指示此 layer 的原始事件边界位置。\n转换器（如 PGR writer）应优先使用这些边界生成事件，而非从 interval 端点推断。无此字段 = 退化为当前行为。\n编译时丢弃（类似 RPE 扩展键），不进入 `.fcbc`。 |
```

- [ ] **步骤 2：在 §5.5.2 补充语法示例段落**

表之后、`5.5.3 speed` 之前插入：

```
`junctionBeats` 支持在 `layer` 体内作为附加声明：

```
layer {
    junctionBeats: [0.0b, 2.0b, 5.0b, 8.0b];
    positionX {
        [0.0b => 2.0b]: 200px;
        [2.0b => 5.0b]: 400px;
    }
}
```
```

- [ ] **步骤 3：Commit**

```bash
git add fcs.md
git commit -m "feat(spec): add junctionBeats field to motion layer"
```

---

### 任务 2：修改 AST — MotionLayer 加 `junction_beats` 字段

**文件：**
- 修改：`crates/fcs-core/src/ast/line.rs:34-43`

- [ ] **步骤 1：在 MotionLayer 中添加字段**

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionLayer {
    pub junction_beats: Vec<f64>,   // ← 新增，可选原始事件边界
    pub position_x: Vec<MotionInterval>,
    pub position_y: Vec<MotionInterval>,
    pub rotation: Vec<MotionInterval>,
    pub alpha: Vec<MotionInterval>,
    pub scale_x: Vec<MotionInterval>,
    pub scale_y: Vec<MotionInterval>,
    pub speed: Vec<MotionInterval>,
}
```

`#[derive(Default)]` 会自动将 `junction_beats` 初始化为空 Vec，无需额外改动。

- [ ] **步骤 2：编译确认**

运行：`cargo check`
预期：编译成功

- [ ] **步骤 3：Commit**

```bash
git add crates/fcs-core/src/ast/line.rs
git commit -m "feat(ast): add junction_beats field to MotionLayer"
```

---

### 任务 3：修改 parser — 解析 `junctionBeats` 语法

**文件：**
- 修改：`crates/fcs-core/src/parser/block.rs:269-301`

- [ ] **步骤 1：编写解析 `junctionBeats` 的辅助函数**

在 `parse_motion_layer` 函数之前或之内增加：

```rust
/// Parse `junctionBeats: [beat, beat, ...];`
fn parse_junction_beats(input: &str) -> IResult<&str, Vec<f64>> {
    let (input, _) = preceded(ws, tag("junctionBeats")).parse(input)?;
    let (input, _) = preceded(ws, char(':')).parse(input)?;
    let (input, beats) = delimited(
        preceded(ws, char('[')),
        separated_list0(
            preceded(ws, char(',')),
            preceded(ws, parse_beat_value),
        ),
        preceded(ws, char(']')),
    )
    .parse(input)?;
    let (input, _) = semicolon(input)?;
    Ok((input, beats))
}
```

- [ ] **步骤 2：在 `parse_motion_layer` 的 `many0(alt(...))` 中添加 `junctionBeats` 分支**

```rust
// 在 parse_motion_layer 函数内，many0(alt(...)) 中添加：
map(parse_junction_beats, |beats| {
    layer.junction_beats = beats;
}),
```

注意：`junctionBeats` 是字段声明（`key: value;` 格式），而 motion 属性是块（`key { }` 格式），所以需要单独解析分支。

- [ ] **步骤 3：运行现有测试验证未破坏解析**

运行：`cargo test -p fcs-core`
预期：所有测试通过

- [ ] **步骤 4：Commit**

```bash
git add crates/fcs-core/src/parser/block.rs
git commit -m "feat(parser): parse junctionBeats in motion layer"
```

---

### 任务 4：修改 `to_fcs.rs` — 收集原始事件边界

**文件：**
- 修改：`crates/fcs-converter/src/to_fcs.rs`

- [ ] **步骤 1：在 `push_motion_interval` 中收集事件边界**

在 `push_motion_interval` 函数开头增加两行：

```rust
fn push_motion_interval(
    layer: &mut MotionLayer,
    field: MotionField,
    e: &IrEvent,
    end_expr: Expression,
    start_expr: Expression,
) {
    // Collect original event boundaries for junction_beats
    layer.junction_beats.push(e.start_beat);
    layer.junction_beats.push(e.end_beat);

    let end = if e.end_beat > e.start_beat {
        // ... rest unchanged
```

- [ ] **步骤 2：在 `build_line` 结束后对 `junction_beats` 去重排序**

在所有 `for e in &line.events.alpha { ... }` 循环之后，`let motion = MotionBlock { ... }` 之前：

```rust
    // Sort and deduplicate junction beats
    layer.junction_beats.sort_by(|a, b| a.partial_cmp(b).unwrap());
    layer.junction_beats.dedup();

    let motion = MotionBlock {
        layers: vec![layer],
    };
```

- [ ] **步骤 3：运行现有测试验证**

运行：`cargo test`
预期：所有 round-trip 测试仍通过

- [ ] **步骤 4：Commit**

```bash
git add crates/fcs-converter/src/to_fcs.rs
git commit -m "feat(converter): collect original event boundaries as junction_beats"
```

---

### 任务 5：修改 `pgr_writer.rs` — 使用 `junction_beats` 替代 `build_beats`

**文件：**
- 修改：`crates/fcs-converter/src/from_fcs/pgr_writer.rs:383-468`

- [ ] **步骤 1：收集所有 layer 的 junction_beats 并覆写 beats 生成**

在 `motion_to_move_rotate_alpha` 中，先收集 union junction_beats，然后在各事件生成时使用它：

```rust
fn motion_to_move_rotate_alpha(
    motion: &Option<MotionBlock>,
    bpm: f64,
    env: &EvalEnv,
) -> (Vec<PgrEvent>, Vec<PgrEvent>, Vec<PgrEvent>) {
    let x_ivs = collect_intervals(motion, |l| &l.position_x);
    let y_ivs = collect_intervals(motion, |l| &l.position_y);
    let rot_ivs = collect_intervals(motion, |l| &l.rotation);
    let alpha_ivs = collect_intervals(motion, |l| &l.alpha);

    // Collect union of junction_beats from all layers
    let jb: Vec<f64> = match motion {
        Some(m) => {
            let mut all: Vec<f64> = m
                .layers
                .iter()
                .flat_map(|l| l.junction_beats.iter().copied())
                .collect();
            all.sort_by(|a, b| a.partial_cmp(b).unwrap());
            all.dedup();
            all
        }
        None => vec![],
    };

    // Helper: build events from beats, with fallback to build_beats
    let build_from_beats = |intervals: &[MotionInterval]| -> Vec<f64> {
        if !jb.is_empty() {
            jb.clone()
        } else {
            let mut s = std::collections::BTreeSet::new();
            for iv in intervals {
                s.insert((iv.start_beat * 1e6) as i64);
                s.insert((iv.end_beat * 1e6) as i64);
            }
            s.iter().map(|b| *b as f64 / 1e6).collect()
        }
    };
```

Move events (现有代码改 beats 来源)：

```rust
    let move_beats = if !jb.is_empty() {
        jb.clone()
    } else {
        build_beats(&[&x_ivs, &y_ivs])
    };
    let moves: Vec<PgrEvent> = move_beats
        .windows(2)
        .map(|w| { ... })  // 现有闭包内容不变
        .collect();
```

Rotate events 同理：

```rust
    let rot_beats = build_from_beats(&rot_ivs);
    let rots: Vec<PgrEvent> = rot_beats
        .windows(2)
        .map(|w| { ... })  // 现有闭包内容不变
        .collect();
```

Alpha events 同理。

- [ ] **步骤 2：更新 round-trip 测试 — 收紧 PGR round-trip 容差**

在 `crates/fcs-converter/tests/roundtrip.rs` 中：

```rust
// test_pgr_roundtrip_small: rotate: 1.0  (原 90.0)
// test_pgr_roundtrip_medium: rotate: 1.0  (原 90.0)
```

- [ ] **步骤 3：运行全量测试验证 round-trip 无损**

运行：`cargo test`
预期：PGR round-trip（small + medium）在 rotate 容差 1.0 内通过

- [ ] **步骤 4：Commit**

```bash
git add crates/fcs-converter/src/from_fcs/pgr_writer.rs crates/fcs-converter/tests/roundtrip.rs
git commit -m "feat(converter): use junction_beats for PGR event generation"
```

---

### 任务 6：最终验证

- [ ] **步骤 1：clippy 检查**

运行：`cargo clippy -- -D warnings`
预期：无警告

- [ ] **步骤 2：全量测试**

运行：`cargo test`
预期：全部通过

- [ ] **步骤 3：cargo fmt**

运行：`cargo fmt`
预期：格式化完成无报错
