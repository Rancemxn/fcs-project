# PGR v1/v3 格式与语义证据

状态：Evidence baseline（2026-07-15）

本文记录社区通常称为 Phigros Official JSON、PGR、Phi JSON 的 `formatVersion = 1` 和
`formatVersion = 3` 谱面。它是 converter 设计资料，不是 PGR 官方规范，也不规定 FCS 的行为。

相关项目决定见：

- [0001：运行时只有一个物理主时钟](../decisions/0001-single-runtime-clock.md)；
- [0007：外部谱面格式使用版本化 semantic profile](../decisions/0007-versioned-conversion-semantic-profiles.md)。

## 1. 证据范围

主要证据：

| 证据 | 快照 | 相关路径 |
|---|---|---|
| Phira parser | `824e24b97af53c2e14c4e2dfa13ecd36d87c9e06` | `refer/chart/phira/prpr/src/parse/pgr.rs` |
| Phira package/player | 同上 | `refer/chart/phira/prpr/src/scene/game.rs`、`prpr/src/info.rs` |
| Phichain schema/converter | `fe3448449781af86c67a36b97f672b0dbe6c8243` | `refer/chart/phichain/phichain-format/src/official/` |
| Phira Docs | `909a4913d726c13af8ea6904501faea6f91bd2ae` | `refer/chart/phira-docs/src/chart-standard/chart-format/phi/` |

“Official”是这些项目采用的模块名称，不表示本文获得了上游游戏厂商的规范授权或完整版本历史。

## 2. 格式识别与版本

### 2.1 顶层结构

**结构共识**：PGR v1/v3 是 JSON object，核心顶层字段为：

| 字段 | 形态 | 已观察含义 |
|---|---|---|
| `formatVersion` | integer | 主要决定判定线移动坐标编码；已确认值为 1、3 |
| `offset` | number | chart 内建 offset，文档称单位为秒 |
| `judgeLineList` | array | 判定线数组；数组索引可作为来源 line identity |

Phira 和 Phichain 都只对 `formatVersion = 1 | 3` 提供明确转换路径；未知版本被拒绝。Phira Docs
提到还可能出现其他值，但没有给出其语义，因此未知值只能进入未支持 dialect 检测，不能按 v3
静默解析。

### 2.2 v1 与 v3 的核心差别

已确认的结构差别集中在 `judgeLineMoveEvents`：

- v1 把 X、Y 打包进单个 `start`/`end` 数值；
- v3 使用 `start`/`end` 表示 X，`start2`/`end2` 表示 Y；
- 其他核心 Line、Note、speed event 字段形状基本相同。

`formatVersion` 只标识编码形状，不能独自解决 v1 packed Y 的 520/530 分歧、offset 符号、
`floorPosition` 信任策略或 Hold `speed` 等执行语义。

## 3. 判定线与事件 schema

每条 `judgeLineList[i]` 的核心字段为：

| 字段 | 形态 | 作用 |
|---|---|---|
| `bpm` | number | 该 Line 的 source time base |
| `notesAbove` | array | 位于判定线一侧的 Note |
| `notesBelow` | array | 位于判定线另一侧的 Note |
| `speedEvents` | array | line scroll speed/distance 数据 |
| `judgeLineMoveEvents` | array | X/Y 移动 |
| `judgeLineRotateEvents` | array | 旋转 |
| `judgeLineDisappearEvents` | array | alpha/可见度 |

PGR 核心没有 RPE 的 `father`、多 event layer、Bezier、texture、extended storyboard 或
`attachUI`。Importer 不得根据线号、坐标重合或事件相似度猜 parent。

### 3.1 普通数值事件

rotate/disappear event 的公共形状是：

```text
startTime
endTime
start
end
```

move event 还受 `formatVersion` 控制坐标编码。核心 PGR event 不携带 easing ID；已检查实现把
`start` 到 `end` 作为线性变化。来源区间、source order 和精确端点仍应保留，因为 gap、overlap
和相同时间事件的处理不由 JSON 结构自动确定。

### 3.2 speed event

已观察形状：

```text
startTime
endTime
value
floorPosition?  // 某些版本/工具保存的缓存值
```

Phira 只读取 `startTime`、`endTime`、`value`，重新积分 distance；Phichain schema 接受可缺失的
`floorPosition`，导出时重新计算。Phira Docs 也把它描述为便于计算、较高版本可能不存在的字段。

因此 `floorPosition` 更适合作为来源验证点和 fidelity 数据，而不是比 speed events 更高优先级的
第二条 scroll 真值来源。

## 4. 时间、Line BPM 与 offset

### 4.1 raw `T` 不是全局物理时钟

**结构共识**：Line event、Note time 和 Hold duration 的 raw `T` 使用每条 Line 自己的 `bpm`。
Phira Docs 给出的物理单位为：

```text
one raw T unit = 60 / (32 * lineBpm) seconds
```

因此 importer 应先建立：

```text
sourceLineBeat = T / 32
chartTimeSeconds = T * 60 / (32 * lineBpm)
```

`sourceLineBeat` 只是来源坐标。映射到 canonical `chartTime` 后，line BPM 不得作为运行时第二时钟。
如果多个 Line 用不同 BPM 表示同一物理时刻，它们应得到相同、可比较的 canonical time。

如果 raw `T` 是整数，可以把 `T/32` 精确保留为 rational；如果是 JSON decimal，应保留十进制
原文或等价 exact decimal，再显式记录到 binary float/rational 的转换。

### 4.2 不同实现对 line BPM 的处理

| 实现 | 快照行为 |
|---|---|
| Phira | `r = 60 / 32 / line.bpm`，每条 Line 独立把 raw `T` 转为秒 |
| Phichain importer | 全局 BPM 取第一条 Line 的 BPM，所有 Line 使用同一个 `t(x)=x*1.875/60` Beat 映射 |
| Phichain exporter | 先把 canonical BPMList 归一化到一个输出 BPM，再给所有 Line 写相同 BPM |

因此含有不同 line BPM 的输入在 Phira 与当前 Phichain importer 中可能产生不同物理时间。这个行为
必须被报告；不能以“Phichain 能导入”为依据把第一条 Line BPM 解释为全谱 BPM。

### 4.3 offset

Phira Docs 把 PGR `offset` 描述为秒，并用“正值时谱面比音乐快”解释符号。Phira parser 原样保存该
值；Phira player 的快照行为是：

```text
effectiveOffset = chart.offset + info.yml offset + playerConfigOffset
chartTime = playerTime - effectiveOffset
```

自然语言描述与计时公式容易被相反解释。Source profile 必须用明确公式定义 PGR offset 到 FCS
`audioOffset` 的映射，并在 package import 时区分 chart 内建 offset 与外层 `info.yml` offset。

## 5. 坐标

### 5.1 v3 判定线坐标

v3 move 坐标使用归一化 canvas：

```text
source X/Y: 0 at one edge, 0.5 at center, 1 at the opposite edge
```

映射到 FCS 1920×1080 中心原点候选公式为：

```text
lineXpx = (sourceX - 0.5) * 1920
lineYpx = (sourceY - 0.5) * 1080
```

轴方向和 rotation sign 仍须由目标 coordinate convention 明确声明；不能只写 scale。

### 5.2 v1 packed move 坐标

Phira Docs 与 Phira parser 的解释为：

```text
xUnit = trunc(packed / 1000)
yUnit = packed % 1000

xNormalized = xUnit / 880
yNormalized = yUnit / 520
```

之后以 0.5 为屏幕中心映射到目标坐标。

当前 Phichain importer 则使用：

```text
xUnit = round(packed / 1000)
yNormalized = (packed % 1000) / 530
```

这里有两个独立歧义：

1. Y 基数为 520 还是 530；
2. X 的千位部分使用截断/取整还是四舍五入。

第二项不是无害实现细节：当余数大于等于约 500 时，`round(packed/1000)` 可能把 X 增加一格。
Importer 必须由 semantic profile 选择公式。没有 profile 时，strict 模式不能静默选 520 或 530。

### 5.3 Note X

PGR Note `positionX` 是相对于判定线中心的“宽度单位”，不是 v3 move 的 0..1 canvas 坐标。
Phira Docs/Phira runtime 给出：

```text
one note X unit = 0.05625 * render width
```

映射到 1920-wide FCS logical world：

```text
noteXpx = sourcePositionX * 108
```

固定 Phichain importer 则写成：

```text
internalX = sourcePositionX / 18 * CANVAS_WIDTH
```

其 `CANVAS_WIDTH=1350`，播放器再按 viewport width/CANVAS_WIDTH 映射；换到 1920-wide FCS world
等价于：

```text
noteXpx = sourcePositionX * 1920/18
        = sourcePositionX * 320/3
```

因此 Note X 的 `108` 与 `320/3≈106.6667` 也是 profile 分歧，不应只按“都接近 1/18 屏宽”合并。
判定线 move X 与 Note X 仍必须使用不同 rule ID。

## 6. rotation、alpha 与 event interval

- rotate event 值是角度；FCS importer 必须显式记录 clockwise/counterclockwise 与 degree→radian；
- disappear event 通常使用 0..1 alpha，但 schema 本身不声明范围；越界不能静默 clamp；
- move、rotate、disappear 的 `startTime/endTime` 是来源区间；
- gap 是保持前值、回默认值还是由导出器补常量事件，需要指定 runtime/profile 证据；
- overlap 和同时间事件的优先级不能由 JSON object 或数组偶然顺序猜测。

Phira 会删除 `startTime > endTime` 的普通事件并警告；这是明确 Repair。Phichain importer 默认还会
缩短长常量事件、拟合事件并删除冗余常量后缀；这些是内部转换策略，不能反向定义 PGR source
语义，且拟合必须在 ConversionReport 中说明是否等价或近似。

## 7. speed、distance 与 `floorPosition`

常见积分形式为：

```text
distanceDelta = (endT - startT) * speedValue * 60 / (32 * lineBpm)
```

Phichain 输出代码以等价形式计算：

```text
floor += (endT - startT) * value / lineBpm * 1.875
```

但“一个来源 floor 单位等于多少 FCS logical pixel”不能由 `formatVersion` 唯一推出。Importer 应：

1. 从 speed event 精确构造 source distance function；
2. 保存 raw `floorPosition`；
3. 在每个缓存点比较重建值；
4. 报告差异、积分规则和采用的视觉 scale；
5. 只有 profile 明确时才把来源 distance 映射到 FCS scroll coordinate。

Phira 的快照 parser 会在第一条 speed event 不从 0 开始时把其 `startTime` 改成 0；空 speed list
会触及不安全的索引路径。把首事件移到 0 是 Repair，不是通用默认值。缺失、gap、overlap、乱序和
负 speed 都需要明确 profile/diagnostic。

## 8. Note 与 Hold

### 8.1 Note schema

核心字段：

| 字段 | 含义 |
|---|---|
| `type` | `1=Tap`、`2=Drag`、`3=Hold`、`4=Flick` |
| `time` | 开始 raw `T` |
| `holdTime` | Hold duration；非 Hold 通常为 0 |
| `positionX` | 判定线局部横向坐标 |
| `speed` | Note/尾部 scroll 倍率，Hold 解释存在差异 |
| `floorPosition` | Note 时刻的来源 distance/cache |

`notesAbove` 与 `notesBelow` 决定 side。核心 PGR v1/v3 schema 没有 RPE 式的 `above`、`isFake`、
`size`、`visibleTime`、`alpha` 或自定义 hitsound 字段。JSON parser 可能忽略这些未知字段，但这不
代表目标 runtime 执行了它们。

### 8.2 Hold

Hold endpoint 为：

```text
endT = time + holdTime
```

必须在 source time profile 下映射 start/end，并验证 end 严格晚于 start。不能先分别 round 到目标
tick 再假设顺序仍成立。

Phira Docs 把 Hold `speed` 描述为尾部速度，Hold 头倍率恒为 1。Phira parser 对 Hold 直接把 Note
speed 设为 1，并从 Line distance 求 Hold 尾高度。Phichain importer 则根据 Note 时刻的 Line speed
对来源 Hold speed 做一次除法归一化，export 时再乘回。这里存在运行时表示差异；在没有可验证
round-trip probe 前，不得宣称单一 Hold-speed 公式对所有 PGR producer/runtime 等价。

### 8.3 fake 与 presentation

核心 PGR v1/v3 没有 fake Note 表示。导入带未知 `isFake` 字段的 JSON 时，parser 可以保留 raw
unknown field，但 source profile 必须证明某个方言定义了它，才能映射为
`judgment.enabled = false`。导出到普通 PGR 时，FCS fake/presentation 能力需要 target capability
negotiation，不能静默丢弃。

## 9. Package 与资源

PGR JSON 本身只保存 chart 结构和 `offset`，不提供完整的曲名、谱师、音乐、插图和包资源清单。
在 Phira 包中，根目录 `info.yml` 指定 chart、music、illustration 等文件；package metadata 与 PGR
payload 必须分别解析并记录 provenance。

Phira 的格式自动检测把不含 `"META"` 的 JSON 候选视为 PGR。这只是播放器 heuristic：任意不含
该字符串的 JSON 不能因此被认为是合法 PGR。显式 package format、顶层 schema 和
`formatVersion` 证据优先。

## 10. 已确认的实现分歧

| 主题 | Phira | Phichain | 处理要求 |
|---|---|---|---|
| v1 packed Y | `/520` | `/530` | semantic profile |
| v1 packed X | 截断千位 | `round(packed/1000)` | semantic profile/兼容 profile |
| 不同 Line BPM | 每 Line 独立换算 | 实际使用第一 Line BPM | 报告实现差异；strict 不猜 |
| Note X | `source*108px` | `source*1920/18px` | semantic profile |
| 非法普通 event | 丢弃 `start>end` | 后续 sequence/fitting 处理 | Repair 分离 |
| 首 speed event | 可强制改到 0 | 构造内部 event | Repair 分离 |
| `floorPosition` | 忽略并重算 | schema 接受、导出重算 | cache 验证与 provenance |
| Hold speed | Hold 内部倍率固定 1 | 按 Line speed 归一化/反归一化 | profile + round-trip probe |
| 未知 Note 字段 | serde 默认可忽略 | schema 默认可忽略 | 不得声称已执行 |

## 11. Semantic profile 需要控制的维度

以下是 profile 维度，不是最终稳定 ID：

```text
formatVersion: 1 | 3
producer/runtime identity
raw T + line BPM mapping
v1 packed X extraction
v1 packed Y base: 520 | 530 | other-declared
offset sign and package-offset composition
event gap/overlap/source-order rule
speed integration and floor scale
floorPosition trust/validation rule
Hold speed rule
Note X width rule: 108px/unit | full-width/18
unknown/fake extension policy
numeric precision and rounding
```

只有当输入不触及候选 profile 的差异时，自动检测才能把多个 profile 视为 canonical-equivalent。
例如，一个仅含 v3 move、所有 Line BPM 相同、无 Hold 的文件不会触及 v1 Y 或 Hold speed 分歧；
这不意味着这些分歧不存在。

## 12. Parser、compatible 与 Repair 边界

### Parser 应保留

- JSON 数字原文或 exact decimal；
- 字段是否出现；
- unknown fields；
- Line、event 和 Note source order；
- `formatVersion`、Line index、raw BPM/T；
- raw `floorPosition` 和 counters。

### Compatible 可以做但必须报告

- 根据明确 configured default 选择 520 或 530；
- 接受已知 producer 的额外字段；
- 对不影响当前输入的多个 profile 自动合并候选；
- 使用 package manifest 补 metadata/resource 路径。

### Repair 必须 opt-in

- 删除或裁剪 `startTime > endTime`/overlap event；
- 补空 speed event 或把首事件移动到 0；
- clamp alpha/coordinate；
- 重排乱序事件；
- 用重建值替换不一致 `floorPosition`；
- 把非法 Hold endpoint 交换、拉长或降级 Note。

Repair 后不能把整体结果标为 lossless。

## 13. Export 要点

- target 必须明确选择 v1 或 v3，不存在 generic PGR；
- v3 能直接保存分离 X/Y，通常比 packed v1 少一层歧义，但是否作为工具默认值属于 CLI/profile
  决策；
- canonical `chartTime` 必须按目标每-Line BPM 反算 raw `T`；
- 所有 Note/event 边界先作为 exact constraint，再执行目标精度量化；
- speed/distance 与 Note `floorPosition` 应由同一积分规则生成并自检；
- Hold `speed` 必须采用 target runtime profile；
- target 不支持 parent、fake、runtime expression 或 Render 属性时，必须 negotiation、显式近似、
  preserve 或失败；
- 写出后使用同一 target profile 重导入，按 canonical semantics 比较，不能只比较 JSON。

## 14. 当前 examples 状态

### `examples/pgr/simple.pgr.json`

当前不能作为现代 PGR valid conformance fixture：

- Note 缺少 Phira/Phichain schema 要求的 `holdTime` 和 `floorPosition`；
- Note 带有 PGR 核心 schema 不定义的 `size`、`above`；
- 它可以保留为历史 converter input，但必须标成 `legacy-candidate` 或修复后另建 fixture。

### `examples/pgr/features.pgr.json`

同样不是当前严格 schema 的 valid fixture：

- Note 缺少 `holdTime`/`floorPosition`；
- `type = 3` 的条目使用 `visibleTime`，却没有 PGR Hold 所需的 `holdTime`；
- 第一条 Line 缺少 parser schema 要求的 rotate/disappear arrays；
- 混入 `size`、`above` 等 RPE 风格字段。

当前 examples 没有 v1 packed coordinate fixture，也没有能区分 520/530、不同 Line BPM、
`floorPosition` mismatch 和 Hold speed profile 的证据 fixture。

## 15. 证据索引

### Phira

- `refer/chart/phira/prpr/src/parse/pgr.rs`
  - `PgrChart`、`PgrJudgeLine`、`PgrEvent`、`PgrSpeedEvent`、`PgrNote`；
  - `parse_move_events_fv1`：520、千位截断；
  - `parse_judge_line`：每-Line `60/32/bpm`；
  - `parse_speed_events`：首 speed event 修复和 distance 积分；
  - `parse_notes`：Note type、Hold、Note X 和 Hold speed 行为。
- `refer/chart/phira/prpr/src/scene/game.rs`
  - `infer_chart_format`：JSON/PGR heuristic；
  - `offset`：chart/package/player offset 合成。

### Phichain

- `refer/chart/phichain/phichain-format/src/official/schema.rs`：v1/v3 JSON schema；
- `refer/chart/phichain/phichain-format/src/official/into_phichain.rs`
  - 第一 Line BPM；
  - v1 `/530` 和 `round`；
  - Note X 使用 `note.x/18*CANVAS_WIDTH`；
  - event fitting、Hold speed 归一化；
- `refer/chart/phichain/phichain-format/src/official/from_phichain.rs`
  - v3 exporter；
  - single-BPM normalization；
  - speed/floorPosition/Note cache 重建。

### Phira Docs

- `refer/chart/phira-docs/src/chart-standard/chart-format/phi/root.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/phi/judgeLine.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/phi/event.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/phi/note.md`。
