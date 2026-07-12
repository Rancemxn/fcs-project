# FCS 规范优化：Motion Layer 增加 `junctionBeats`

**日期**: 2026-07-12
**状态**: 已批准
**用户指令**: "现在分析下，fcs.md的fcs规范可以怎样改，优化，调整？尽量不引入太多东西。尽量小化修改。目标是PGR转FCS再转PGR是无损的。"
**设计者**: Claude Code (经用户确认批准)

## 技术背景

本项目的核心 IR（中间表示）是 `IrChart`，有三个格式转换方向：
- `fcs_converter::pgr::parse_pgr()` → `IrChart` → `fcs_converter::to_fcs::ir_to_fcs()` → `fcs_core::ast::Document` → `fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json()`
- 同理 RPE/PEC 路径：各自 parse → IrChart → ir_to_fcs → Document → 各自 writer

PGR 格式使用 piecewise-constant 事件（start/end/value 三元组），FCS 格式使用 easing 区间模型。当前转换链在 `pgr_writer.rs` 中用 `build_beats()` 从 interval 边界重建事件边界，这是 round-trip 事件数差异的根因。

## 数据模式变更

```rust
// fcs-core AST 新增字段
pub struct MotionLayer {
    pub junction_beats: Vec<f64>,   // 新增，默认空
    // ... 其余字段不变
}

// MotionLayer 序列化示例
// motion layer {
//     junctionBeats: [0.0b, 2.0b, 5.0b];
//     positionX {
//         [0.0b => 2.0b]: 200px;
//         [2.0b => 5.0b]: 400px;
//     }
// }
```

## 受影响 API

| 符号 | 变更类型 | 说明 |
|------|---------|------|
| `fcs_core::ast::MotionLayer` | 加字段 | `pub junction_beats: Vec<f64>` |
| `fcs_core::parser::block::parse_motion_body` | 扩展 | 解析 `junctionBeats` 语法 |
| `fcs_converter::to_fcs::push_motion_interval` | 扩展 | 收集原始事件边界 |
| `fcs_converter::from_fcs::pgr_writer::motion_to_move_rotate_alpha` | 扩展 | 读取 junctionBeats 替代 build_beats |
| `fcs_converter::from_fcs::rpe_writer` | 未变更 | 天生无需重采样 |
| `fcs_converter::from_fcs::pec_writer` | 未变更 | 天生无需重采样 |
| `fcs_converter::from_fcs::autofill::autofill_layer` | 未变更 | 只补 interval，不增事件点 |

## 问题

PGR 转 FCS 后，PGR 原始事件边界在 FCS 区间模型中丢失。回写时 `pgr_writer.rs` 使用 `build_beats()` 从 interval 端点重建边界，与原边界不同，导致事件序列变化。

## 设计

### 1. FCS 规范改动 (§5.5.2 Motion Layer)

在 `layer { }` 体内增加可选字段：

```
junctionBeats: time*;
```

- 可选，最多 4096 个 entry
- 值单调递增（以 beat 为单位）
- 指示此 layer 的原始事件边界位置
- 转换器（如 PGR writer）应优先使用这些边界生成事件，而非从 interval 端点推断
- 无此字段 = 退化为当前行为
- 编译阶段丢弃（类似 RPE 扩展键），不进入字节码

### 2. AST 改动 (`fcs-core/src/ast/`)

```rust
pub struct MotionLayer {
    pub junction_beats: Vec<f64>,   // ← 新增，默认空
    pub position_x: Vec<MotionInterval>,
    pub position_y: Vec<MotionInterval>,
    pub rotation: Vec<MotionInterval>,
    pub alpha: Vec<MotionInterval>,
    pub speed: Vec<MotionInterval>,
}
```

### 3. `to_fcs.rs` 改动

在 `push_motion_interval()` 处收集原始事件边界，写入 `junction_beats`：

```
每处理一个 IrEvent，将它的 start_beat 和 end_beat 收集中
去重并排序后存入 layer.junction_beats
```

### 4. `pgr_writer.rs` 改动

`motion_to_move_rotate_alpha()` 读到 `junction_beats` 时：

- 如果 `junction_beats` 非空 → 用它替代 `build_beats()`
- 如果 `junction_beats` 为空 → 退化为当前 `build_beats()` 逻辑

`motion_to_speed_events()` 不受影响（已经是直接 interval 转换）。

### 5. 影响范围

| 模块 | 需要修改 |
|------|---------|
| `fcs-core/src/ast/mod.rs` | MotionLayer 加字段 |
| `fcs-core/src/parser/block.rs` | 解析 junctionBeats 语法 |
| `to_fcs.rs` | 收集原始边界写入 junctionBeats |
| `pgr_writer.rs` | 读取 junctionBeats 替代 build_beats |
| `autofill.rs` | 不受影响（只补 interval，不碰 junctionBeats）|
| `rpe_writer.rs` | 不受影响（无 build_beats 模式）|
| `pec_writer.rs` | 不受影响（点事件直接从 interval 端点生成）|

### 6. 边界情况

- **用户手动修改 interval 端点**：junctionBeats 存旧的原始边界，writer 在这些时间点对当前 intervals 求值，不会出现不一致
- **junctionBeats 落在所有 interval 覆盖范围外**：事件序列为空。此情况仅发生在用户手动修改后，自动化转换路径不会出现
- **多 layer**：每个 layer 独立持有 junctionBeats，互不影响

### 7. 非设计方案

方案 B（要求 contiguous intervals）被否决——它强制转换器补齐 gap，对 PEC 等点事件格式产生大量膨胀，且与 FCS easing 模型原有设计意图不符。
