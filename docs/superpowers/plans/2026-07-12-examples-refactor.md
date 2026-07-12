# Examples & Tests 重构实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 重构 examples 目录（按格式分组、部分版本管理）和测试文件（按格式拆分、新增格式特定测试 + COPYRIGHT 扫描测试）。

**架构：** (1) 修改 .gitignore 只忽略 COPYRIGHT，(2) 创建 13 个按格式分组的示例谱面，(3) 删除旧 roundtrip.rs 并创建 6 个按格式/功能拆分的新测试文件，(4) 全量验证。

**技术栈：** Rust, FCS AST, PGR/RPE/PEC parsers & writers, fcs-core

---

### 任务 1：.gitignore + 目录脚手架 + 移除旧测试

**说明：** 修改 .gitignore，创建 examples 子目录，删除旧的 roundtrip.rs。

**涉及文件：**
- 修改：`.gitignore`
- 删除：`crates/fcs-converter/tests/roundtrip.rs`

- [ ] **步骤 1：修改 .gitignore**

将 `/examples` 改为 `/examples/COPYRIGHT`：

```gitignore
/target
**/*.fcbc
.DS_Store
/refer
/examples/COPYRIGHT
/.claude
/.ccg
```

- [ ] **步骤 2：创建目录结构**

```bash
cd C:/Users/Admin/Desktop/fcs && mkdir -p examples/fcs examples/pgr examples/rpe examples/pec
```

- [ ] **步骤 3：删除旧的 roundtrip.rs**

```bash
rm crates/fcs-converter/tests/roundtrip.rs
```

注意：roundtrip.rs 中 `common::compare_events_sampled` 和 `common::compare_notes_exact` 辅助函数定义在 `tests/common/mod.rs` 中，不受影响。

- [ ] **步骤 4：编译验证（无测试）**

```bash
cargo check
```

预期：编译通过。由于测试文件被删除，可能没有测试可运行。

- [ ] **步骤 5：Commit**

```bash
git add .gitignore
git rm crates/fcs-converter/tests/roundtrip.rs
git commit -m "chore: update gitignore, scaffold examples dirs, remove old roundtrip.rs"
```

---

### 任务 2：创建 FCS 示例谱面

**说明：** 在 `examples/fcs/` 下创建 6 个 FCS 格式测试谱面。

**涉及文件：**
- 创建：`examples/fcs/empty.fcs`
- 创建：`examples/fcs/simple.fcs`
- 创建：`examples/fcs/multi-line.fcs`
- 创建：`examples/fcs/easing.fcs`
- 创建：`examples/fcs/template.fcs`
- 创建：`examples/fcs/overlapping.fcs`

- [ ] **步骤 1：创建 `empty.fcs`**

```fcs
#fcs v4.0.0
name: "Empty";
offset: 0ms;
```

无 judgelines 块。parser 应返回空 lines 列表。

- [ ] **步骤 2：创建 `simple.fcs`**

```fcs
#fcs v4.0.0
name: "Simple FCS";
offset: 0ms;
judgelines: [
    line {
        name: "Main";
        bpm: 120;
        zOrder: 0;
        color: #ffffffff;
        motion {
            layer {
                positionX {
                    [0.0b => 8.0b]: 0px;
                }
                rotation {
                    [0.0b => 8.0b]: easeLinear(b, 0.0b, 8.0b, 0deg, 90deg, 0.0, 1.0);
                }
            }
        }
        notes {
            tap(time: 0.0b, positionX: -200px);
            tap(time: 2.0b, positionX: 0px);
            tap(time: 4.0b, positionX: 200px);
            hold(time: 6.0b, positionX: -200px, visibleTime: 4.0b);
        }
    }
];
```

1 条线、120 BPM、3 tap + 1 hold、positionX 依次 -200/0/200/-200px。

- [ ] **步骤 3：创建 `multi-line.fcs`**

```fcs
#fcs v4.0.0
name: "Multi Line";
offset: 0ms;
judgelines: [
    line {
        name: "Top";
        bpm: 140;
        zOrder: 10;
        color: #ff0000ff;
        inherit { position: true; rotation: false; }
        notes {
            tap(time: 0.0b, positionX: 0px);
        }
    }
    line {
        name: "Mid";
        bpm: 120;
        zOrder: 5;
        color: #00ff00ff;
        inherit { position: true; rotation: true; }
        notes {
            tap(time: 0.0b, positionX: 200px);
            tap(time: 2.0b, positionX: -200px);
        }
    }
    line {
        name: "Bot";
        bpm: 100;
        zOrder: 0;
        color: #0000ffff;
        notes {
            tap(time: 0.0b, positionX: -200px);
            tap(time: 1.0b, positionX: 0px);
            tap(time: 2.0b, positionX: 200px);
            tap(time: 3.0b, positionX: 0px);
        }
    }
];
```

3 条线、不同 BPM/z_order/color，覆盖 inherit 设置。

- [ ] **步骤 4：创建 `easing.fcs`**

```fcs
#fcs v4.0.0
name: "Easing Coverage";
offset: 0ms;
judgelines: [
    line {
        name: "Main";
        bpm: 120;
        zOrder: 0;
        color: #ffffffff;
        motion {
            layer {
                positionX {
                    [0.0b => 4.0b]: easeLinear(b, 0.0b, 4.0b, -300px, 300px, 0.0, 1.0);
                    [4.0b => 8.0b]: easeOutSine(b, 4.0b, 8.0b, 300px, 0px, 0.0, 1.0);
                    [8.0b => 12.0b]: easeInOutCubic(b, 8.0b, 12.0b, 0px, 300px, 0.0, 1.0);
                }
                rotation {
                    [0.0b => 6.0b]: easeOutElastic(b, 0.0b, 6.0b, 0deg, 720deg, 0.0, 1.0);
                    [6.0b => 12.0b]: easeInBounce(b, 6.0b, 12.0b, 0deg, 180deg, 0.0, 1.0);
                }
            }
        }
        notes {
            tap(time: 0.0b, positionX: 0px);
            tap(time: 6.0b, positionX: 100px);
            tap(time: 12.0b, positionX: -100px);
        }
    }
];
```

5 种 easing（linear、outSine、inOutCubic、outElastic、inBounce），分布在 positionX 和 rotation 字段。

- [ ] **步骤 5：创建 `template.fcs`**

```fcs
#fcs v4.0.0
name: "Template Test";
offset: 0ms;
#template tap_note(time: time, x: length) : note {
    tap(time: $time, positionX: $x * 2px);
}
judgelines: [
    line {
        name: "Main";
        bpm: 120;
        zOrder: 0;
        color: #ffffffff;
        #useTemplate tap_note(time: 0.0b, x: 0px);
        #useTemplate tap_note(time: 2.0b, x: 100px);
        #useTemplate tap_note(time: 4.0b, x: -100px);
    }
];
```

1 个 template 块，3 次 template 调用。测试 template 解析路径。

- [ ] **步骤 6：创建 `overlapping.fcs`**

```fcs
#fcs v4.0.0
name: "Overlapping Intervals";
offset: 0ms;
judgelines: [
    line {
        name: "Main";
        bpm: 120;
        zOrder: 0;
        color: #ffffffff;
        motion {
            layer {
                positionX {
                    [0.0b => 8.0b]: 0px;
                    [4.0b => 12.0b]: 200px;
                    [8.0b => 16.0b]: easeLinear(b, 8.0b, 16.0b, 200px, 400px, 0.0, 1.0);
                }
                rotation {
                    [16.0b => 8.0b]: 45deg;
                }
                alpha {
                    [0.0b => 4.0b]: easeLinear(b, 0.0b, 4.0b, 0.0, 1.0, 0.0, 1.0);
                }
            }
        }
        notes {
            tap(time: 0.0b, positionX: 0px);
        }
    }
];
```

positionX 区间时间重叠、rotation 开始时间 > 结束时间（反向区间）、alpha 正常区间。验证 parser 容错。

- [ ] **步骤 7：验证 FCS 解析**

```bash
cd C:/Users/Admin/Desktop/fcs && cargo check
```

- [ ] **步骤 8：Commit**

```bash
git add examples/fcs/
git commit -m "feat(examples): add FCS test charts (empty, simple, multi-line, easing, template, overlapping)"
```

---

### 任务 3：创建 PGR / RPE / PEC 示例谱面

**说明：** 在 `examples/pgr/`、`examples/rpe/`、`examples/pec/` 下创建 6 个外部格式测试谱面。

**涉及文件：**
- 创建：`examples/pgr/simple.pgr.json`
- 创建：`examples/pgr/features.pgr.json`
- 创建：`examples/rpe/simple.rpe.json`
- 创建：`examples/rpe/extremes.rpe.json`
- 创建：`examples/pec/simple.pec`
- 创建：`examples/pec/all-notes.pec`

- [ ] **步骤 1：创建 `simple.pgr.json`（V1 格式）**

```json
{
  "formatVersion": 1,
  "title": "Simple PGR",
  "artist": "Test",
  "charter": "Test",
  "music": {"id": 0, "preview": 0},
  "cover": {"id": 0},
  "offset": 0,
  "lines": [
    {
      "bpm": 120,
      "notesAbove": [
        {"type": 1, "time": 0, "positionX": 0, "speed": 1.0, "size": 1.0, "above": 1},
        {"type": 1, "time": 4, "positionX": 1, "speed": 1.0, "size": 1.0, "above": 0},
        {"type": 1, "time": 8, "positionX": 2, "speed": 1.5, "size": 1.0, "above": 1}
      ],
      "notesBelow": [],
      "speedEvents": [
        {"startTime": 0, "endTime": 16, "start": 1.0, "end": 1.0, "value": 1}
      ],
      "moveXEvents": [
        {"startTime": 0, "endTime": 16, "start": 0.5, "end": 0.8, "start2": 0, "end2": 0, "value": 1}
      ],
      "rotateEvents": [
        {"startTime": 0, "endTime": 8, "start": 0, "end": 180, "start2": 0, "end2": 0, "value": 1},
        {"startTime": 8, "endTime": 16, "start": 180, "end": 0, "start2": 0, "end2": 0, "value": 1}
      ],
      "alphaEvents": [
        {"startTime": 0, "endTime": 16, "start": 1.0, "end": 0.5, "start2": 0, "end2": 0, "value": 1}
      ]
    }
  ]
}
```

1 条线、3 个音符（含 above/below）、speed/moveX/rotate/alpha 各类事件各一。V1 格式（无 floorPosition 等 V3 字段）。

- [ ] **步骤 2：创建 `features.pgr.json`（V3 格式）**

```json
{
  "formatVersion": 3,
  "title": "PGR V3 Features",
  "artist": "Test",
  "charter": "Test",
  "music": {"id": 0, "preview": 0},
  "cover": {"id": 0},
  "offset": 0,
  "lines": [
    {
      "bpm": 120,
      "notesAbove": [
        {"type": 1, "time": 0, "positionX": 0, "speed": 1.0, "size": 1.0, "above": 1},
        {"type": 3, "time": 4, "positionX": 1, "speed": 1.0, "size": 1.0, "above": 1, "visibleTime": 2}
      ],
      "notesBelow": [],
      "speedEvents": [
        {"startTime": 0, "endTime": 8, "start": 1.0, "end": 2.0, "value": 1}
      ],
      "moveXEvents": [
        {"startTime": 0, "endTime": 16, "start": 0.3, "end": 0.7, "start2": 0.5, "end2": 0.5, "value": 1}
      ],
      "moveYEvents": [
        {"startTime": 0, "endTime": 16, "start": 0.5, "end": 0.5, "start2": 0.4, "end2": 0.6, "value": 1}
      ]
    },
    {
      "bpm": 160,
      "notesAbove": [
        {"type": 1, "time": 8, "positionX": 0, "speed": 1.0, "size": 1.0, "above": 1}
      ],
      "notesBelow": [],
      "speedEvents": [],
      "moveXEvents": [],
      "rotateEvents": [],
      "alphaEvents": []
    }
  ]
}
```

V3 格式、2 条线、不同 BPM、含 moveY（start2/end2）、hold 类型音符（type=3）。

- [ ] **步骤 3：创建 `simple.rpe.json`**

```json
{
  "META": {
    "name": "Simple RPE",
    "offset": 0,
    "composer": "Test",
    "charter": "Test"
  },
  "judgeLineList": [
    {
      "bpm": 120,
      "extended": [],
      "eventList": {
        "speedEvent": [
          {"startTime": 0, "endTime": 16000, "start": 1.0, "end": 1.0}
        ],
        "positionX": [
          {"startTime": 0, "endTime": 16000, "start": 0, "end": 0, "easing": "easeLinear"}
        ]
      },
      "notesList": [
        {"type": "Tap", "time": 0, "positionX": 0, "speed": 1.0, "above": 1},
        {"type": "Tap", "time": 2000, "positionX": 200, "speed": 1.0, "above": 1},
        {"type": "Tap", "time": 4000, "positionX": -200, "speed": 1.0, "above": 1},
        {"type": "Hold", "time": 6000, "positionX": 0, "speed": 1.0, "above": 1, "holdTime": 2000}
      ]
    }
  ]
}
```

1 条线、120 BPM、4 个 notes（3 tap + 1 hold）、speedEvent 和 positionX 事件各一。

- [ ] **步骤 4：创建 `extremes.rpe.json`**

```json
{
  "META": {
    "name": "RPE Extremes",
    "offset": 0,
    "composer": "Test",
    "charter": "Test"
  },
  "judgeLineList": [
    {
      "bpm": 120,
      "extended": [],
      "eventList": {
        "speedEvent": [
          {"startTime": 0, "endTime": 10000, "start": 1.0, "end": 1.0}
        ],
        "positionX": [
          {"startTime": 0, "endTime": 5000, "start": -675, "end": 675, "easing": "easeLinear"}
        ],
        "rotate": [
          {"startTime": 0, "endTime": 10000, "start": -72000, "end": 72000, "easing": "easeLinear"}
        ]
      },
      "notesList": [
        {"type": "Tap", "time": 0, "positionX": -675, "speed": 1.0, "above": 1},
        {"type": "Tap", "time": 5000, "positionX": 675, "speed": 1.0, "above": 1}
      ]
    }
  ]
}
```

极值测试：positionX -675→675（全范围）、rotation -72000→72000（极值）。

- [ ] **步骤 5：创建 `simple.pec`**

```pec
0
bp 0.00 120
n1 0 0.00 1024 1 0
# 1.000
& 1.000
n2 0 4096.00 0 1024 1 0
# 1.000
& 1.000
n4 0 8192.00 1024 1 0
# 1.000
& 1.000
cp 0 0.00 1024 1789
cp 0 8192.00 1024 1024
cd 0 0.00 0.00000
cd 0 8192.00 90.00000
```

1 条 line、120 BPM、n1（tap）在 0、n2（hold）在 2 拍、n4（drag）在 4 拍、cp/cd 运动事件。

- [ ] **步骤 6：创建 `all-notes.pec`**

```pec
0
bp 0.00 120
n1 0 0.00 1024 1 0
# 1.000
& 1.000
n2 0 2048.00 2 1024 1 0
# 1.000
& 1.000
n3 0 4096.00 1024 1 0 2048.00
# 1.000
& 1.000
n4 0 6144.00 1024 1 0
# 1.000
& 1.000
n1 0 8192.00 1024 0 1
# 1.000
& 1.000
```

n1（tap）、n2（hold, visT=2）、n3（flick, holdTime=1 拍）、n4（drag）、n1 fake=1（假音符）。覆盖 4 种音符类型 + fake 标记。

- [ ] **步骤 7：Commit**

```bash
git add examples/pgr/ examples/rpe/ examples/pec/
git commit -m "feat(examples): add PGR, RPE, PEC test charts"
```

---

### 任务 4：创建测试文件

**说明：** 将 `manifest_path` 辅助函数移至 `common/mod.rs`，创建 6 个按格式/功能拆分的测试文件。

**涉及文件：**
- 修改：`crates/fcs-converter/tests/common/mod.rs`
- 创建：`crates/fcs-converter/tests/fcs_tests.rs`
- 创建：`crates/fcs-converter/tests/pgr_tests.rs`
- 创建：`crates/fcs-converter/tests/rpe_tests.rs`
- 创建：`crates/fcs-converter/tests/pec_tests.rs`
- 创建：`crates/fcs-converter/tests/cross_format_tests.rs`
- 创建：`crates/fcs-converter/tests/copyright_tests.rs`

- [ ] **步骤 1：在 `common/mod.rs` 中添加 `manifest_path`**

在 `common/mod.rs` 末尾添加：
```rust
/// Resolve a path relative to the project root
/// from a crate's manifest directory (tests live in `crates/fcs-converter/`).
pub fn manifest_path(rel: &str) -> String {
    let dir = env!("CARGO_MANIFEST_DIR");
    let full = std::path::Path::new(dir).join("../../").join(rel);
    full.to_string_lossy().to_string()
}
```

- [ ] **步骤 2：创建 `fcs_tests.rs`**

```rust
//! FCS format: parse tests only (FCS→FCS round-trip not yet supported as a library).

mod common;

use fcs_core::parser::parse_document;

fn load_fcs(name: &str) -> fcs_core::ast::Document {
    let path = common::manifest_path(&format!("examples/fcs/{name}"));
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    let (rest, doc) = parse_document(&src)
        .unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));
    assert!(rest.trim().is_empty(), "{name}: unparsed trailing content");
    doc
}

#[test]
fn test_parse_fcs_empty() {
    let doc = load_fcs("empty.fcs");
    assert_eq!(doc.meta.name, "Empty");
    assert!(doc.judgelines.lines.is_empty(), "expected 0 lines");
}

#[test]
fn test_parse_fcs_simple() {
    let doc = load_fcs("simple.fcs");
    assert_eq!(doc.judgelines.lines.len(), 1);
    let line = &doc.judgelines.lines[0];
    assert_eq!(line.notes.instances.len(), 4);
    use fcs_core::ast::NoteKind;
    assert_eq!(line.notes.instances[0].kind, NoteKind::Tap);
    assert_eq!(line.notes.instances[3].kind, NoteKind::Hold);
    let motion = line.motion.as_ref().expect("expected motion block");
    assert!(!motion.layers[0].position_x.is_empty());
    assert!(!motion.layers[0].rotation.is_empty());
}

#[test]
fn test_parse_fcs_multi_line() {
    let doc = load_fcs("multi-line.fcs");
    assert_eq!(doc.judgelines.lines.len(), 3);
    assert_eq!(doc.judgelines.lines[0].z_order, 10);
    assert_eq!(doc.judgelines.lines[1].z_order, 5);
    assert_eq!(doc.judgelines.lines[2].z_order, 0);
}

#[test]
fn test_parse_fcs_easing() {
    use fcs_core::ast::Expression;
    let doc = load_fcs("easing.fcs");
    let layer = &doc.judgelines.lines[0].motion.as_ref().unwrap().layers[0];
    assert_eq!(layer.position_x.len(), 3);
    match &layer.position_x[0].expression {
        Expression::Call { name, .. } => assert_eq!(name, "easeLinear"),
        _ => panic!("expected easeLinear Call"),
    }
    match &layer.position_x[1].expression {
        Expression::Call { name, .. } => assert_eq!(name, "easeOutSine"),
        _ => panic!("expected easeOutSine Call"),
    }
    match &layer.rotation[0].expression {
        Expression::Call { name, .. } => assert_eq!(name, "easeOutElastic"),
        _ => panic!("expected easeOutElastic Call"),
    }
}

#[test]
fn test_parse_fcs_template() {
    let doc = load_fcs("template.fcs");
    assert!(!doc.judgelines.lines.is_empty());
    assert_eq!(doc.judgelines.lines[0].notes.instances.len(), 3);
    assert!(doc.templates.is_some());
}

#[test]
fn test_parse_fcs_overlapping() {
    let doc = load_fcs("overlapping.fcs");
    let motion = &doc.judgelines.lines[0].motion.as_ref().unwrap().layers[0];
    assert_eq!(motion.position_x.len(), 3);
    assert_eq!(motion.rotation.len(), 1);
    assert_eq!(motion.alpha.len(), 1);
}
```

- [ ] **步骤 3：创建 `pgr_tests.rs`**

```rust
//! PGR format: parse + round-trip tests.

mod common;

use fcs_converter::ir::IrChart;
use fcs_converter::pgr::parse_pgr;
use fcs_converter::to_fcs::ir_to_fcs;
use fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json;

fn load_pgr(name: &str) -> IrChart {
    let path = common::manifest_path(&format!("examples/pgr/{name}"));
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    parse_pgr(&src).unwrap_or_else(|e| panic!("failed to parse {name}: {e}"))
}

fn roundtrip_pgr(chart: &IrChart) -> IrChart {
    let doc = ir_to_fcs(chart);
    let out = fcs_to_pgr_json(&doc, 3);
    parse_pgr(&out).unwrap()
}

#[test]
fn test_parse_pgr_simple() {
    let chart = load_pgr("simple.pgr.json");
    assert_eq!(chart.lines.len(), 1);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    assert_eq!(chart.lines[0].notes_above.len(), 3);
    assert_eq!(chart.lines[0].events.speed.len(), 1);
    assert_eq!(chart.lines[0].events.move_x.len(), 1);
    assert_eq!(chart.lines[0].events.rotate.len(), 2);
    assert_eq!(chart.lines[0].events.alpha.len(), 1);
}

#[test]
fn test_parse_pgr_features() {
    let chart = load_pgr("features.pgr.json");
    assert_eq!(chart.lines.len(), 2);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    assert!((chart.lines[1].bpm - 160.0).abs() < 1e-6);
    assert_eq!(chart.lines[0].events.move_y.len(), 1);
}

#[test]
fn test_pgr_roundtrip_simple() {
    let orig = load_pgr("simple.pgr.json");
    let rt = roundtrip_pgr(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(
        &orig, &rt, 200,
        common::EventTolerances { rotate: 0.001, ..Default::default() },
    );
}

#[test]
fn test_pgr_roundtrip_features() {
    let orig = load_pgr("features.pgr.json");
    let rt = roundtrip_pgr(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(&orig, &rt, 200, common::EventTolerances::default());
}
```

- [ ] **步骤 4：创建 `rpe_tests.rs`**

```rust
//! RPE format: parse + round-trip tests.

mod common;

use fcs_converter::ir::IrChart;
use fcs_converter::rpe::parse_rpe;
use fcs_converter::to_fcs::ir_to_fcs;
use fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json;

fn load_rpe(name: &str) -> IrChart {
    let path = common::manifest_path(&format!("examples/rpe/{name}"));
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    parse_rpe(&src).unwrap_or_else(|e| panic!("failed to parse {name}: {e}"))
}

fn roundtrip_rpe(chart: &IrChart) -> IrChart {
    let doc = ir_to_fcs(chart);
    let out = fcs_to_rpe_json(&doc);
    parse_rpe(&out).unwrap()
}

#[test]
fn test_parse_rpe_simple() {
    let chart = load_rpe("simple.rpe.json");
    assert_eq!(chart.lines.len(), 1);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    assert_eq!(chart.lines[0].notes_above.len(), 4);
}

#[test]
fn test_parse_rpe_extremes() {
    let chart = load_rpe("extremes.rpe.json");
    assert!((chart.lines[0].notes_above[0].position_x + 675.0).abs() < 1.0);
    assert!((chart.lines[0].notes_above[1].position_x - 675.0).abs() < 1.0);
}

#[test]
fn test_rpe_roundtrip_simple() {
    let orig = load_rpe("simple.rpe.json");
    let rt = roundtrip_rpe(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(
        &orig, &rt, 200,
        common::EventTolerances { rotate: 40000.0, ..Default::default() },
    );
}

#[test]
fn test_rpe_roundtrip_extremes() {
    let orig = load_rpe("extremes.rpe.json");
    let rt = roundtrip_rpe(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(
        &orig, &rt, 200,
        common::EventTolerances { rotate: 40000.0, ..Default::default() },
    );
}
```

- [ ] **步骤 5：创建 `pec_tests.rs`**

```rust
//! PEC format: parse + round-trip tests.

mod common;

use fcs_converter::ir::IrChart;
use fcs_converter::pec::parse_pec;
use fcs_converter::to_fcs::ir_to_fcs;
use fcs_converter::from_fcs::pec_writer::fcs_to_pec;

fn load_pec(name: &str) -> IrChart {
    let path = common::manifest_path(&format!("examples/pec/{name}"));
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    parse_pec(&src).unwrap_or_else(|e| panic!("failed to parse {name}: {e}"))
}

fn roundtrip_pec(chart: &IrChart) -> IrChart {
    let doc = ir_to_fcs(chart);
    let out = fcs_to_pec(&doc);
    parse_pec(&out).unwrap()
}

#[test]
fn test_parse_pec_simple() {
    let chart = load_pec("simple.pec");
    assert_eq!(chart.lines.len(), 1);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    use fcs_converter::ir::IrNoteKind;
    assert_eq!(chart.lines[0].notes_above[0].kind, IrNoteKind::Tap);
    assert_eq!(chart.lines[0].notes_above[1].kind, IrNoteKind::Hold);
    assert_eq!(chart.lines[0].notes_above[2].kind, IrNoteKind::Drag);
}

#[test]
fn test_parse_pec_all_notes() {
    let chart = load_pec("all-notes.pec");
    use fcs_converter::ir::IrNoteKind;
    assert_eq!(chart.lines[0].notes_above[0].kind, IrNoteKind::Tap);
    assert_eq!(chart.lines[0].notes_above[1].kind, IrNoteKind::Hold);
    assert_eq!(chart.lines[0].notes_above[2].kind, IrNoteKind::Flick);
    assert_eq!(chart.lines[0].notes_above[3].kind, IrNoteKind::Drag);
    assert!(chart.lines[0].notes_above[4].is_fake);
}

#[test]
fn test_pec_roundtrip_simple() {
    let orig = load_pec("simple.pec");
    let rt = roundtrip_pec(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(
        &orig, &rt, 200,
        common::EventTolerances {
            rotate: 40000.0, move_x: 1000.0, move_y: 1000.0,
            speed: 10.0, alpha: 2.0,
        },
    );
}

#[test]
fn test_pec_roundtrip_all_notes() {
    let orig = load_pec("all-notes.pec");
    let rt = roundtrip_pec(&orig);
    assert_eq!(orig.lines[0].notes_above.len(), rt.lines[0].notes_above.len());
}
```

- [ ] **步骤 6：创建 `cross_format_tests.rs`**

```rust
//! Cross-format conversion smoke tests.

mod common;

fn load_pgr(name: &str) -> fcs_converter::ir::IrChart {
    let path = common::manifest_path(&format!("examples/pgr/{name}"));
    let src = std::fs::read_to_string(&path).unwrap();
    fcs_converter::pgr::parse_pgr(&src).unwrap()
}

fn load_rpe(name: &str) -> fcs_converter::ir::IrChart {
    let path = common::manifest_path(&format!("examples/rpe/{name}"));
    let src = std::fs::read_to_string(&path).unwrap();
    fcs_converter::rpe::parse_rpe(&src).unwrap()
}

fn load_pec(name: &str) -> fcs_converter::ir::IrChart {
    let path = common::manifest_path(&format!("examples/pec/{name}"));
    let src = std::fs::read_to_string(&path).unwrap();
    fcs_converter::pec::parse_pec(&src).unwrap()
}

#[test]
fn test_cross_pgr_to_rpe() {
    let ir = load_pgr("simple.pgr.json");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rpe_str = fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json(&doc);
    let ir_rpe = fcs_converter::rpe::parse_rpe(&rpe_str).unwrap();
    assert_eq!(ir.lines.len(), ir_rpe.lines.len());
}

#[test]
fn test_cross_pgr_to_pec() {
    let ir = load_pgr("simple.pgr.json");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pec_str = fcs_converter::from_fcs::pec_writer::fcs_to_pec(&doc);
    let ir_pec = fcs_converter::pec::parse_pec(&pec_str).unwrap();
    assert_eq!(ir.lines.len(), ir_pec.lines.len());
}

#[test]
fn test_cross_rpe_to_pgr() {
    let ir = load_rpe("simple.rpe.json");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pgr_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_pgr = fcs_converter::pgr::parse_pgr(&pgr_str).unwrap();
    assert_eq!(ir.lines.len(), ir_pgr.lines.len());
}

#[test]
fn test_cross_pec_to_pgr() {
    let ir = load_pec("simple.pec");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pgr_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_pgr = fcs_converter::pgr::parse_pgr(&pgr_str).unwrap();
    assert_eq!(ir.lines.len(), ir_pgr.lines.len());
}
```

- [ ] **步骤 7：创建 `copyright_tests.rs`**

```rust
//! Dynamic scan of COPYRIGHT charts: verify all community charts parse.
//! These charts are NOT version-controlled (see .gitignore).

mod common;

use std::path::Path;

#[test]
fn test_copyright_files_exist() {
    let dir = Path::new(&common::manifest_path("examples/COPYRIGHT"));
    assert!(dir.exists(), "COPYRIGHT directory missing");
    let entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json" || ext == "pec"))
        .collect();
    assert!(!entries.is_empty(), "no copyright chart files found");
}

#[test]
fn test_copyright_all_parse() {
    let dir = Path::new(&common::manifest_path("examples/COPYRIGHT"));
    let mut parsed = 0u32;
    let mut errors = Vec::new();

    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if !path.extension().map_or(false, |ext| ext == "json" || ext == "pec") {
            continue;
        }

        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => { errors.push(format!("{name}: read error: {e}")); continue; }
        };

        let result = match path.extension().and_then(|e| e.to_str()) {
            Some("pec") => fcs_converter::pec::parse_pec(&src).map(|_| ()),
            Some("json") => fcs_converter::pgr::parse_pgr(&src)
                .or_else(|_| fcs_converter::rpe::parse_rpe(&src))
                .map(|_| ()),
            _ => unreachable!(),
        };

        match result {
            Ok(()) => parsed += 1,
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }

    if !errors.is_empty() {
        panic!(
            "{}/{} files failed:\n{}",
            errors.len(),
            parsed + errors.len() as u32,
            errors.join("\n")
        );
    }
    assert!(parsed > 0, "no copyright chart files were parsed");
}
```

- [ ] **步骤 8：编译验证**

```bash
cd C:/Users/Admin/Desktop/fcs && cargo check 2>&1
```

修复任何编译错误后再继续。

- [ ] **步骤 9：Commit**

```bash
git add crates/fcs-converter/tests/common/mod.rs crates/fcs-converter/tests/fcs_tests.rs crates/fcs-converter/tests/pgr_tests.rs crates/fcs-converter/tests/rpe_tests.rs crates/fcs-converter/tests/pec_tests.rs crates/fcs-converter/tests/cross_format_tests.rs crates/fcs-converter/tests/copyright_tests.rs
git commit -m "feat(tests): split tests by format, add FCS/PGR/RPE/PEC/cross/copyright test files"
```

---

### 任务 5：全量验证 + 最终提交

**说明：** 运行全部测试、clippy、fmt，清理遗留任务状态文件。

- [ ] **步骤 1：全量测试**

```bash
cd C:/Users/Admin/Desktop/fcs && cargo nextest run
```

预期：所有测试通过。

- [ ] **步骤 2：clippy**

```bash
cargo clippy -- -D warnings
```

预期：零 warning。

- [ ] **步骤 3：fmt**

```bash
cargo fmt
```

- [ ] **步骤 4：Commit**

```bash
git add -A
git commit -m "chore: final cleanup

- all tests pass
- clippy zero warnings
- cargo fmt clean"
```
