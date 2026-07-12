# Examples & Tests 重构设计

## 问题

`examples/` 目录全部被 `.gitignore` 忽略，所有示例谱面（`sample.fcs`、`test.pgr.json` 等）已被删除，导致 `roundtrip.rs` 中的测试全部无法运行。需要重构为：

1. 只忽略 `examples/COPYRIGHT/`（社区谱面），其余示例版本管理
2. 按格式组织示例谱面，覆盖从简单到复杂的场景
3. 测试文件按格式拆分，每种格式独立文件
4. COPYRIGHT 谱面做动态扫描测试

## 目录结构

```
examples/
├── COPYRIGHT/                     # gitignore，社区谱面
│   ├── 128.json                   # PEC
│   ├── 3007.json                  # PEC
│   ├── 10176.json                 # RPE
│   ├── 10674.json                 # RPE
│   ├── 4886210000956270.json      # PGR
│   └── ... 其余 11 个
├── fcs/
│   ├── empty.fcs                  # 空谱面（边界测试）
│   ├── simple.fcs                 # 简单谱面（1 线，少量音符）
│   ├── multi-line.fcs             # 多判定线谱面
│   ├── easing.fcs                 # 各类 easing 覆盖
│   ├── template.fcs               # template 调用
│   └── overlapping.fcs            # 重叠/反向区间
├── pgr/
│   ├── simple.pgr.json            # 1 线、少量事件
│   └── features.pgr.json          # V3 字段、多 BPM 变化
├── rpe/
│   ├── simple.rpe.json            # 简单 RPE 谱面
│   └── extremes.rpe.json          # 边界值、大旋转
└── pec/
    ├── simple.pec                 # 简单 PEC 谱面
    └── all-notes.pec              # 所有音符类型 + fake
```

## .gitignore 改动

```gitignore
# 修改前
/examples

# 修改后
/examples/COPYRIGHT
```

## 测试文件结构

```
tests/
├── common/
│   └── mod.rs                     # 已有 helpers（compare_events_sampled, compare_notes_exact）
├── fcs_tests.rs                   # FCS 解析 + round-trip + 针对性测试
├── pgr_tests.rs                   # PGR 解析 + round-trip
├── rpe_tests.rs                   # RPE 解析 + round-trip
├── pec_tests.rs                   # PEC 解析 + round-trip
├── cross_format_tests.rs          # 跨格式转换（PGR→RPE, RPE→PGR, PEC→PGR）
└── copyright_tests.rs             # COPYRIGHT 动态扫描测试
```

每个格式测试文件包含 3 类测试函数：
1. `test_parse_<format>_simple` — 解析简单谱面，断言 AST 字段值
2. `test_<format>_roundtrip_small` — parse → IR → FCS → writer → parse，compare_events_sampled
3. `test_<format>_<feature>` — 针对性验证（如 easing、空谱面、边界值）

## 测试谱面设计

### FCS 谱面

**`empty.fcs`**：空文档，无 `judgelines` 块。验证 parser 返回空 lines 列表。

**`simple.fcs`**：1 条判定线、`bpm: 120`、3 个 tap 音符（time 0.0b, 2.0b, 4.0b）、1 个 hold（time 6.0b, visibleTime 4.0b）。positionX 依次 -200px, 0px, 200px, -200px。

**`multi-line.fcs`**：3 条判定线，不同 z_order、不同 BPM，覆盖 inherit 参数。

**`easing.fcs`**：1 条线，1 个 motion 区间，覆盖 5 种代表性 easing：easeLinear、easeOutSine、easeInOutCubic、easeOutElastic、easeInBounce。验证 evaluator 和 writer 的处理。

**`template.fcs`**：1 个 `#template` 块，1 条线通过 `#useTemplate` 调用。

**`overlapping.fcs`**：motion 区间时间范围重叠（positionX [0, 8] 和 [4, 12]），验证 parser 容错和 writer 采样。

### PGR 谱面

**`simple.pgr.json`**：1 条线、1 个 speed 事件、1 个 moveX 事件、1 个 rotate 事件、1 个 alpha 事件、若干 tap 音符。V1 格式。

**`features.pgr.json`**：V3 格式（含 `start2`/`end2` 字段），多条线，多种 BPM。

### RPE 谱面

**`simple.rpe.json`**：1 条线、常用事件、若干 notes。

**`extremes.rpe.json`**：极大 rotation 值（~72000）、极小 positionX、延迟时间线。

### PEC 谱面

**`simple.pec`**：1 条线、bp 事件、n1/n2/n4 各 1、cp 运动。

**`all-notes.pec`**：四种音符（n1/n2/n3/n4）+ fake 标记，包含 holdTime（n3 可选参数）。

## 测试内容详述

### fcs_tests.rs

```rust
#[test] fn test_parse_fcs_empty()        // 空谱面 → 0 lines
#[test] fn test_parse_fcs_simple()       // AST 断言：3 tap, 1 hold, positionX
#[test] fn test_parse_fcs_multi_line()   // 3 条线，z_order 检查
#[test] fn test_parse_fcs_easing()       // 5 种 easing，检查 Expression::Call
#[test] fn test_parse_fcs_template()     // template 引用解析
#[test] fn test_parse_fcs_overlapping()  // 重叠区间容错
#[test] fn test_fcs_roundtrip_simple()   // FCS → IR → FCS 二次转谱对比
```

### pgr_tests.rs

```rust
#[test] fn test_parse_pgr_simple()       // AST 验证
#[test] fn test_parse_pgr_features()     // V3 字段验证
#[test] fn test_pgr_roundtrip_simple()   // PGR → FCS → PGR
#[test] fn test_pgr_roundtrip_features() // 带 V3 字段的 round-trip
```

### rpe_tests.rs

```rust
#[test] fn test_parse_rpe_simple()       // AST 验证
#[test] fn test_parse_rpe_extremes()     // 边界值解析
#[test] fn test_rpe_roundtrip_simple()   // RPE → FCS → RPE
#[test] fn test_rpe_roundtrip_extremes() // 边界值 round-trip
```

### pec_tests.rs

```rust
#[test] fn test_parse_pec_simple()       // AST 验证
#[test] fn test_parse_pec_all_notes()    // 四种音符类型 + fake
#[test] fn test_pec_roundtrip_simple()   // PEC → FCS → PEC
#[test] fn test_pec_roundtrip_all_notes()// 音符 round-trip
```

### cross_format_tests.rs

```rust
#[test] fn test_cross_pgr_to_rpe()      // PGR → FCS → RPE
#[test] fn test_cross_pgr_to_pec()      // PGR → FCS → PEC
#[test] fn test_cross_rpe_to_pgr()      // RPE → FCS → PGR
#[test] fn test_cross_pec_to_pgr()      // PEC → FCS → PGR
```

### copyright_tests.rs

```rust
#[test] fn test_copyright_files_exist() // 确认 COPYRIGHT/ 下有文件（非空）
#[test] fn test_copyright_all_parse()   // 遍历 COPYRIGHT/*.json + *.pec，全部解析成功
```

`test_copyright_all_parse` 动态扫描 `examples/COPYRIGHT/` 目录，按扩展名选择 parser（`.json` → 尝试 PGR 或 RPE parser，`.pec` → PEC parser），断言全部解析成功。不要求 round-trip（原始谱面不在版本控制中，无法做对比基准）。

## 相关文件清单

| 操作 | 文件 | 说明 |
|------|------|------|
| 修改 | `.gitignore` | `/examples` → `/examples/COPYRIGHT` |
| 创建 | `examples/fcs/empty.fcs` | 空文档 |
| 创建 | `examples/fcs/simple.fcs` | 1 线、3 tap、1 hold |
| 创建 | `examples/fcs/multi-line.fcs` | 3 条线 |
| 创建 | `examples/fcs/easing.fcs` | 5 种 easing |
| 创建 | `examples/fcs/template.fcs` | template 调用 |
| 创建 | `examples/fcs/overlapping.fcs` | 重叠区间 |
| 创建 | `examples/pgr/simple.pgr.json` | 简单 PGR |
| 创建 | `examples/pgr/features.pgr.json` | V3 格式 |
| 创建 | `examples/rpe/simple.rpe.json` | 简单 RPE |
| 创建 | `examples/rpe/extremes.rpe.json` | 边界值 RPE |
| 创建 | `examples/pec/simple.pec` | 简单 PEC |
| 创建 | `examples/pec/all-notes.pec` | 四种音符 PEC |
| 删除 | `crates/fcs-converter/tests/roundtrip.rs` | 拆分为新文件 |
| 创建 | `crates/fcs-converter/tests/fcs_tests.rs` | FCS 测试 |
| 创建 | `crates/fcs-converter/tests/pgr_tests.rs` | PGR 测试 |
| 创建 | `crates/fcs-converter/tests/rpe_tests.rs` | RPE 测试 |
| 创建 | `crates/fcs-converter/tests/pec_tests.rs` | PEC 测试 |
| 创建 | `crates/fcs-converter/tests/cross_format_tests.rs` | 跨格式测试 |
| 创建 | `crates/fcs-converter/tests/copyright_tests.rs` | COPYRIGHT 扫描 |
