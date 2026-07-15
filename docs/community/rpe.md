# RPE 格式与语义证据

状态：Evidence baseline（2026-07-15）

本文记录 Re:PhiEdit（RPE）JSON 谱面及其在 Phira、Phichain 和社区文档中的已观察行为。RPE
字段丰富、版本演进快，且 `META.RPEVersion`、编辑器版本和播放器执行语义并非一一对应。因此
“RPE”必须被视为格式族，而不是一套由单个整数完整决定的语义。

相关项目决定见：

- [0001：运行时只有一个物理主时钟](../decisions/0001-single-runtime-clock.md)；
- [0007：外部谱面格式使用版本化 semantic profile](../decisions/0007-versioned-conversion-semantic-profiles.md)。

## 1. 证据范围

| 证据 | 快照 | 相关路径 |
|---|---|---|
| Phira RPE parser/player | `824e24b97af53c2e14c4e2dfa13ecd36d87c9e06` | `refer/chart/phira/prpr/src/parse/rpe.rs`、`scene/game.rs` |
| Phichain RPE schema/converter | `fe3448449781af86c67a36b97f672b0dbe6c8243` | `refer/chart/phichain/phichain-format/src/rpe/` |
| Phira Docs | `909a4913d726c13af8ea6904501faea6f91bd2ae` | `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/` |
| sim-phi/extends RPE converter | `00a6602e9b837dba4453aa758cdab06cafc79162` | `refer/chart/extends/src/pec/rpe2json.ts` |

Phira Docs 是高价值社区资料，但其中已经确认存在同页公式矛盾和默认值矛盾；引用文档不能替代
版本化 runtime probe。

## 2. 格式族与版本识别

### 2.1 顶层结构

现代 RPE JSON 的主要顶层字段是：

| 字段 | 形态 | 作用 |
|---|---|---|
| `BPMList` | array | 全局 BPM point 列表 |
| `META` | object | RPEVersion、offset 和重复的包元数据 |
| `judgeLineList` | array | 判定线数组，索引也被 `father` 引用 |

还可出现 `chartTime`、`judgeLineGroup`、`multiLineString`、`multiScale`、`timeTags`、`xybind` 等
编辑器状态。除非目标明确需要编辑器 round-trip，这些字段不应被误当成 gameplay/runtime 语义；
即使不进入 canonical model，也应在 source representation/provenance 中保留。

### 2.2 `RPEVersion` 不是 semantic profile

Phira Docs 明确记录：

- RPE 1.5.0 到 1.6.0 前的一段版本仍写 `RPEVersion = 150`；
- RPE 1.6.1 仍可能写 `RPEVersion = 160`；
- 不同字段在 142、150、162、163、170 等时期加入或改变行为。

Phira parser 还兼容 number/string 两种 `RPEVersion`，缺失或无法解析时默认为 160。这是 parser
兼容行为，不是文件自身声明了 160 语义。

因此检测证据至少需要分别记录：

```text
META.RPEVersion raw value
producer/editor identity and version, if known
package/player compatibility options
fields actually present
selected source semantic profile
```

仅凭 `RPEVersion >= 170`、文件扩展名或是否存在 `META` 不能确定全部语义。

## 3. `META`、包信息和资源根

常见 `META` 字段：

| 字段 | 已观察含义 |
|---|---|
| `RPEVersion` | 格式/编辑器候选版本标记，但会停滞或复用 |
| `offset` | chart 内建 offset，整数毫秒 |
| `name`、`level` | 曲名、难度显示文本 |
| `charter`、`composer`、`illustration` | 创作者信息 |
| `song`、`background` | 相对于谱面资源根的路径候选 |
| `id` | 编辑器标识；文档指出实际可能是任意字符串 |

Phira 谱面包还在根目录使用 `info.yml` 指定 chart、music、illustration 和显示元数据。Phira Docs
明确建议包信息以 `info.yml` 为准，以避免与 RPE `META` 重复后不一致。

但 offset 不能简单说“完全由 info.yml 覆盖”。Phira 快照中：

```text
chartOffset = META.offset / 1000 seconds
effectiveOffset = chartOffset + info.yml offset + playerConfigOffset
chartTime = playerTime - effectiveOffset
```

因此 package importer 必须分别保存 chart offset、package offset 和播放器校准；metadata merge
policy 与 runtime offset composition 是两个问题。

RPE 的自定义 Line texture、GIF、font 和 Note hitsound 都可能引用资源根内文件。仅解析 JSON 而不
解析 package/resource root，不能得到可播放的完整语义。

## 4. Beat、BPMList 与唯一 `chartTime`

### 4.1 Beat triple

RPE 时间通常写成：

```json
[whole, numerator, denominator]
```

一般解释为：

```text
beat = whole + numerator / denominator
```

Importer 应保留 exact rational，而不是先转成 `f32`。负数、非约分形式和等价分数可以保留 raw
representation，再在 source semantic IR 中使用约分后的 exact value。

### 4.2 denominator 为 0

存在实现分歧：

| 输入 | Phichain | Phira |
|---|---|---|
| `[a, 0, 0]` | 特判为整数 `a`，等价 denominator=1 | `a + 0/0`，产生非有限值并可能在后续失败 |
| `[a, b, 0]`, `b != 0` | 明确报错 | 浮点除零产生非有限值 |

`[a,0,0]` 是否是遗留整数编码必须由 dialect/profile 证明。Strict parser 可以保留该形状并在 semantic
阶段要求 profile；把 denominator 从 0 改成 1 属于 compatibility interpretation 或 Repair，不能
伪装成普通规范化。

### 4.3 BPMList

每个 point 常见形状为：

```text
startTime: Beat
bpm: number
```

在没有 `bpmfactor` 的普通解释下，分段换算为：

```text
seconds += beatDelta * 60 / segmentBpm
```

Importer 必须验证 BPM 有限且大于 0、point 顺序、同 Beat 多 point 的来源顺序和第一个有效区间。
排序、去重、插入 Beat 0 point 或 clamp BPM 都属于 Repair，不能在 parser 中静默完成。

## 5. `bpmfactor` 是已确认歧义

`bpmfactor` 位于每条判定线，文档称默认 1.0 且编辑器中不可编辑。已检查来源存在三种行为：

| 证据 | 行为 |
|---|---|
| Phira Docs `beat.md` 与 `judgeLine.md` 正文 | `effectiveBpm = BPMList.bpm / bpmfactor` |
| 同一 `judgeLine.md` 后附 Python 示例 | `effectiveBpm = BPMList.bpm * bpmfactor` |
| Phira parser | `RPEJudgeLine` 注释 `TODO bpmfactor`，字段未建模，实际忽略 |
| Phichain RPE schema/importer | 字段未建模，实际忽略 |
| sim-phi/extends | 缺失补 1，非 1 时警告未兼容，实际不执行其语义 |

这不是小数误差：乘、除、忽略会改变该 Line 的 Note、event、Hold endpoint 和 scroll 的物理时刻。
Source profile 必须明确：

```text
effective BPM formula
which line-local values the factor affects
how those values map to canonical chartTime
```

映射完成后，`bpmfactor` 不得作为 FCS runtime 的第二时钟继续存在。当前
`fcs-conversion.md` 中“只影响 scroll、不修改 Note 判定时间”的旧结论缺乏上述证据支持，需要在
规范修订时替换为 profile-based mapping。

## 6. JudgeLine schema

现代 RPE Line 常见字段：

| 字段 | 含义/边界 |
|---|---|
| `Group`、`Name` | 编辑器分组和显示名称 |
| `Texture` | 默认 `line.png`，否则为资源相对路径 |
| `anchor` | 纹理锚点，文档称 142 加入 |
| `eventLayers` | 普通事件层；可见实现处理 missing、`null`、稀疏 layer 的能力不同 |
| `extended` | color/text/scale/GIF/paint/incline 等特殊事件 |
| `father` | parent Line 数组索引，`-1` 表示无 parent |
| `rotateWithFather` | 是否继承 parent rotation；缺失默认值有冲突 |
| `isCover` | 1 表示遮罩背面 Note，其他值表示不遮罩 |
| `notes`、`numOfNotes` | Note 数据和编辑器/缓存计数 |
| `zOrder` | 渲染顺序候选；文档给出的范围仍标注待验证 |
| `attachUI` | UI 控制线标记，不是普通 gameplay Line |
| `isGif` | texture 是否为 GIF |
| `bpmfactor` | line time-base 因子，语义有冲突 |
| `posControl` 等 | 按 Note 距离控制 presentation 的关键帧 |

`numOfNotes` 是可重建字段，且文档称包含 fake、不包含 Hold；它不能替代实际 Note 数组。Mismatch
应作为 source validation/provenance 信息，而不是删 Note 以匹配计数。

### 6.1 `eventLayers` 的结构兼容

Phira Docs 指出空 layer 在旧文件中可能为 `null`，新文件中可能省略 event array 或整个
`eventLayers`。Phichain schema 专门把 missing/null/sparse layer 归一化为空列表；Phira 当前 parser
的顶层 `event_layers: Vec<Option<_>>` 对 missing/null 的接受边界更窄。

接受这些形状属于 parser/dialect compatibility。它与“多层如何组合”的 semantic rule 是两个
独立问题。

## 7. 普通事件与 event layer

### 7.1 五类普通事件

每个 layer 可包含：

- `moveXEvents`；
- `moveYEvents`；
- `rotateEvents`；
- `alphaEvents`；
- `speedEvents`。

非 speed common event 的主要字段为：

```text
startTime / endTime: Beat
start / end
easingType
easingLeft / easingRight
bezier
bezierPoints[4]
linkgroup
```

`bezier = 1` 时使用 cubic Bezier 控制点；否则使用 RPE easing table，并可通过
`easingLeft/easingRight` 截取区间。未知 easing ID、非法 Bezier、`left >= right` 的处理在工具间
不同，必须由 profile/diagnostic 定义。

### 7.2 event layer 是加法层

**结构共识**：多个普通 event layer 对同一属性按值相加，而不是“后层覆盖前层”。

证据：

- Phira Docs 的 `GetPos`/`GetState` 示例逐层累加 move、alpha、rotate；
- Phira `Anim::chain` 的 `now_opt` 对各 layer 的当前值调用 `Tweenable::add`；
- Phichain 源码注释明确写出 RPE layers additive，但因为内部模型不支持，只保留第一层。

因此：

- Phira 会把多个 layer 的 move/alpha/rotate/speed 组合；
- Phichain importer 明确丢弃第二层及以后并发出 warning；
- “只取第一层”是有损实现缺口，不是 RPE profile 的正常简化。

FCS importer 应把每层 index、事件和默认值保存在 provenance，并用 FCS Track blend/add 或等价
表达式重建总值。

### 7.3 gap、overlap 与默认值

社区文档示例会在 event gap 中补“保持上一 end 值”的常量事件；不同 parser 可能依赖动画容器的
默认值或排序。Importer 不得在未声明 profile 时：

- 把 gap 自动回零；
- 让最后 JSON item 偶然覆盖 overlap；
- 排序后丢失同 Beat source order；
- 把零长 point 当普通闭区间 segment。

## 8. speed easing 的版本和播放器开关

RPE speed event 的历史至少有三套候选行为：

1. 早期只处理 start/end，按线性 speed 变化；
2. 约 162 起，非 1 easing 曾被描述为“速度形状对应 easing 导函数，使 floorPosition 遵循 easing”；
3. 1.7.0 起，文档称恢复为直接用 easing 插值 speed。

Phira 快照还引入 package/player 配置 `info.yml.useRpe170Speed`：

- 配置关闭时走 legacy speed parser；
- 配置开启时才使用新的积分路径；
- 在新路径中，`RPEVersion >= 170` 选择 Modern，否则选择 Legacy derivative 模式。

Phichain 的 `RpeSpeedEvent` schema 不保存 easing 字段，并统一按 linear transition 导入。

所以 speed 语义至少由 `RPEVersion`、字段、producer 和目标播放器配置共同决定。Importer profile
必须同时固定 speed interpolation 与累计 distance；只匹配瞬时 speed 或只匹配 Note 位置都不足以
证明等价。

## 9. Parent、继承和 `rotateWithFather`

`father` 是 Line 数组索引，`-1` 表示 root。父线可以嵌套。位置继承和 rotation 继承应分别建模，
不能把 parent 关系压成一个未说明约定的 transform matrix。

### 9.1 缺失默认值冲突

| 证据 | `rotateWithFather` 缺失时 |
|---|---|
| Phira Docs 字段表 | 写默认 `true` |
| 同页说明正文 | 写应视为 `false`，以兼容 163 以前版本 |
| Phira parser | `Option<bool>.unwrap_or(false)` |
| Phichain `#[serde(default)]` schema | `RpeJudgeLine::default()` 为 `true` |
| Phichain exporter | 总是写 `true` |

该默认值必须进入 source semantic profile。显式字段值优先于 profile default。

### 9.2 非法 parent

- Phira 检查 parent cycle 并失败；
- Phichain 对非法索引、broken chain 或 cycle 发 warning，把不可达 Line 提升为 root 或构造可达树；
- Phichain 目前不能表示 `rotateWithFather = false`，会警告后仍继承 rotation。

提升 root、断边或强制继承都是 Repair/有损转换。Strict import 应保留并验证原始 parent graph，
不能把修复后的树误报为来源图。

## 10. 坐标、角度、alpha 和 cover

RPE 常用逻辑 canvas 为 1350×900、中心原点：

```text
x source range candidate: -675 .. 675
y source range candidate: -450 .. 450
```

映射到 FCS 1920×1080 的 scale 候选：

```text
xPx = sourceX * 1920 / 1350
yPx = sourceY * 1080 / 900
```

轴方向必须显式记录；Phira Docs 的屏幕归一化示例对 Y 使用 `1 - (...)`，不能只看数值范围推断
最终 Y-up/Y-down。

Phira 与 Phichain 都在导入 RPE rotation 时取负，说明来源 rotation 与其内部角度约定相反。FCS
rule 必须明确 source clockwise/counterclockwise 和 degree→radian。

alpha event 通常以 0..255 表示，各 layer 相加后再映射。负 alpha 在社区 runtime 中还可能触发
“隐藏 Line 及其 Note”的废弃扩展；clamp 到 0 会丢失该行为。`isCover = 1` 表示遮挡 Line 背面的
Note，不能与 alpha、visibility 或 fake 混为一个字段。

## 11. Note

### 11.1 核心字段

| 字段 | 语义 |
|---|---|
| `type` | `1=Tap`、`2=Hold`、`3=Flick`、`4=Drag`；未知值的 fallback 依实现而异 |
| `startTime`、`endTime` | Beat；Hold 使用区间，其他 Note 通常相等 |
| `positionX` | 相对 Line 中心的 X，使用 1350-wide source unit |
| `above` | 1 为一侧，其他值为另一侧 |
| `isFake` | 1/非零通常表示无判定 fake，但精确合法域需 profile |
| `speed` | Note scroll multiplier |
| `alpha` | 0..255 presentation alpha；野外文件可出现 256 |
| `size` | RPE 实际常表现为宽度倍率，不一定是统一二维 scale |
| `visibleTime` | 秒，不是 Beat；表示 Note 提前可见时长 |
| `yOffset` | Line 局部 Y/presentation offset |
| `hitsound` | 相对资源根的自定义音效路径 |
| `judgeArea` | 判定区域宽度倍率，文档称 170 加入 |
| `tint`/历史 `color` | Note RGB tint 字段名发生过迁移 |
| `tintHitEffects` | hit effect tint |

### 11.2 已观察实现差异

- Phira 保留 fake，并把任意非零 `isFake` 视为 fake；
- Phichain 可选择删除 `isFake == 1`，否则把 fake 当真实 Note，二者都不能保持 fake gameplay；
- Phira 将 `visibleTime` 与 Note 物理秒时间比较，生成绝对 visibility boundary；
- Phira 的 `yOffset` 映射还乘以 Note `speed`；该耦合必须由 profile 记录；
- Phira 把 Note `size` 同时写入 X/Y scale，而 Phira Docs 说明编辑器中的该字段实际只控制宽度；
- Phira parser 读取 `tint`，而文档要求兼容历史 `color` 别名；只支持一个字段会丢 presentation；
- Phichain RPE schema 当前忽略 alpha、size、visibleTime、yOffset 等 presentation 字段。

Hold 必须验证 `endTime > startTime`。Fake 不等于 alpha=0；alpha=0 的真实 Note 仍可能参与判定。

## 12. Controls、extended 与 Render 边界

Line 还可包含：

- `alphaControl`、`sizeControl`、`posControl`、`yControl`、`skewControl`；
- `extended.colorEvents`；
- `scaleXEvents`、`scaleYEvents`；
- `textEvents` 与可选 font；
- `gifEvents`；
- `paintEvents`；
- `inclineEvents`；
- custom `Texture`、`isGif`；
- `attachUI`。

Controls 的 `x` 常表示 Note 与判定线的 scroll/distance 坐标，而不是 chartTime。它们可以与 Note
自身 alpha/size/position 相乘或组合，且部分 control 对 Hold 无效。Importer 需要同时保留 source
distance dependency、easing 和适用 Note kind，不能把它们压成单个静态 Note 字段。

这些能力可能分别映射到 FCS Core presentation、runtime expression、Render Profile 或声明的
extension。没有 Core 对应不等于可以丢弃；应使用 `preserved`、runtime extension、显式 approximation
或失败。Phichain 当前未建模的大部分 extended/controls 是实现缺口，不是字段无效的证据。

## 13. 已确认的实现分歧

| 主题 | 分歧 | 语义影响 |
|---|---|---|
| `RPEVersion` | 会停滞；Phira 缺失时猜 160 | profile 选择 |
| Beat `[a,0,0]` | Phichain 当整数；Phira 非有限 | 所有时间 |
| `bpmfactor` | 除、乘、忽略 | Note/event/Hold/scroll time |
| event layers | Phira/文档相加；Phichain只取第一层 | motion/alpha/speed |
| speed easing | 早期、162 derivative、170 direct + 播放器开关 | speed 和累计 distance |
| `rotateWithFather` 缺失 | false 与 true | parent transform |
| invalid parent | 失败、提升 root、强制继承 | transform graph |
| empty layers | missing/null/sparse 接受边界不同 | parser compatibility |
| fake | 保留、删除、当真实 Note | gameplay/score |
| Note `size` | 文档称宽度；Phira按统一二维 scale | presentation |
| presentation | Phira 支持较多；Phichain忽略较多 | 渲染保真 |
| META vs `info.yml` | display metadata 外层优先；offset 实际相加 | metadata/sync |

## 14. Semantic profile 需要控制的维度

以下是维度，不是最终 profile ID：

```text
producer/editor/runtime identity
RPEVersion raw value and version-era evidence
Beat zero-denominator compatibility
BPMList validation and same-Beat ordering
bpmfactor formula and affected fields
event layer additive/default/gap behavior
easing table, clipping and Bezier behavior
speed easing era + player useRpe170Speed option
parent repair policy
rotateWithFather missing default
coordinate axes and rotation sign
negative alpha/isCover behavior
Note fake/presentation/control behavior
META/info.yml merge and offset composition
resource root and missing-resource policy
```

一个不含 `bpmfactor`、多 layer、parent、speed easing 和 extended 字段的简单输入，可能在多个候选
profile 下 canonical-equivalent；只有这种输入相关等价被证明时，才允许静默自动选择。

## 15. Parser、compatible 与 Repair 边界

### Parser 应保留

- JSON 字段出现/缺失/null 的区别；
- Beat 三元组原值；
- `RPEVersion` 的 JSON type 和原文；
- unknown/editor fields；
- event layer index 和 source order；
- parent index、resource path、META 与 package metadata 各自 provenance；
- easing/Bezier/control 的全部 raw 参数。

### Compatible interpretation

- 接受 number/string `RPEVersion`，但不能伪造来源版本；
- 接受已知时代的 missing/null/sparse layer；
- 对 `[a,0,0]` 采用明确的 legacy-integer profile；
- 使用 package manifest 定位资源；
- 使用 configured default profile 解释缺失 `rotateWithFather`，并报告选择。

### Repair 必须 opt-in

- 排序/去重 BPM point 或 event；
- 补 Beat 0 BPM；
- 把非法 denominator 改为 1；
- clamp easing、alpha、coordinate；
- 删除多余 event layer；
- 删除 fake/UI Line；
- 断开 cycle、提升 orphan Line；
- 把 `rotateWithFather=false` 强制改成 true；
- 修正 Hold endpoint 或缺失资源。

“目标工具不支持”不等于 Repair 授权；丢字段还需要 target capability negotiation。

## 16. Export 要点

- target 必须选择明确 producer/runtime profile，不能只写 `RPEVersion`；
- Beat 应按目标允许的 exact rational 写出，rounding 单独报告；
- `bpmfactor` 只有在目标 profile 明确定义时才可生成非 1 值；
- event layer additive semantics 必须保持；若目标工具只接受一层，需要先证明 flatten 等价；
- speed event 同时验证瞬时 speed、累计 distance、Note/Hold 几何；
- parent/rotation inherit 必须用目标缺失默认值验证 round-trip；
- fake、controls、extended、textures、fonts、GIF 和 hitsound 都属于 capability negotiation；
- `META` 与 package manifest 的重复字段使用明确 owner，不能产生两份相互矛盾的值；
- 写出后用同一 target profile 重导入并比较 canonical semantics。

## 17. 当前 examples 状态

### `examples/rpe/simple.rpe.json`

它不是现代 RPE valid fixture：

- 缺少顶层 `BPMList`；
- 使用旧式/非当前 schema 的 Line `bpm` 和 `eventList`；
- 缺少现代 `eventLayers`、`Name`、`Texture`、`isCover` 等 parser 所需结构；
- Note 使用 `[a,0,0]` Beat；
- `type = 3` 同时出现 `holdTime`，但现代 RPE type 3 是 Flick，Hold 是 type 2。

### `examples/rpe/extremes.rpe.json`

同样是未识别 legacy candidate：

- 缺少 `BPMList` 和可靠 `RPEVersion`；
- 使用 `eventList` 而非现代 `eventLayers`；
- 大时间值和 `[a,0,0]` 未声明 dialect；
- 不能作为 modern RPE parser 或 canonical conformance 的 expected-valid 输入。

两者可保留用于历史 converter characterization，但需要明确 `legacy-candidate` metadata；现代 fixture
应另外覆盖 BPMList、exact Beat、eventLayers、bpmfactor profile、parent/default、speed 162/170、
fake、controls/extended 和 package resources。

## 18. 证据索引

### Phira

- `refer/chart/phira/prpr/src/parse/rpe.rs`
  - `RPEChart`、`RPEMetadata`、`RPEJudgeLine`、`RPEEventLayer`、`RPEEvent`、`RPENote`；
  - `parse_rpe`：RPEVersion/speed mode、BPMList、parent cycle；
  - `events_with_factor` 与 `Anim::chain`：layer additive；
  - `parse_speed_events`/`parse_speed_events_legacy`：speed easing 分支；
  - `parse_notes`：Note presentation、fake、hitsound；
  - `rotate_with_father.unwrap_or(false)`：缺失默认。
- `refer/chart/phira/prpr/src/core.rs`：`Triple::beats`、`BpmList`；
- `refer/chart/phira/prpr/src/core/anim.rs`：`Anim::chain` 的加法组合；
- `refer/chart/phira/prpr/src/info.rs`：Phira package 配置和 `use_rpe_170_speed`；
- `refer/chart/phira/prpr/src/scene/game.rs`：format detection、resource load、offset composition。

### Phichain

- `refer/chart/phichain/phichain-format/src/rpe/schema.rs`
  - Beat denominator compatibility；
  - missing/null layer normalization；
  - `rotateWithFather=true` default；
- `refer/chart/phichain/phichain-format/src/rpe/into_phichain.rs`
  - 只取第一 event layer；
  - fake/UI options；
  - parent promotion 和 rotation-inherit 缺口；
- `refer/chart/phichain/phichain-format/src/rpe/from_phichain.rs`：单 layer RPE export。

### Phira Docs

- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/root.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/beat.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/judgeLine.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/event.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/note.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/controls.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/extend.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/rpe/extendEvent.md`；
- `refer/chart/phira-docs/src/chart-standard/index.md`、`chartinfo.md`：Phira package/`info.yml`。

### extends

- `refer/chart/extends/src/pec/rpe2json.ts`：`bpmfactor` 未兼容 warning、RPE→PGR 降级行为。
