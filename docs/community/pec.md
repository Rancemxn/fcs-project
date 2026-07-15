# PEC 格式与语义证据

状态：Evidence baseline（2026-07-15）

PEC（PhiEditer Chart）是空白分隔的命令文本格式。它没有可靠的内建版本字段，编辑器已经停止
更新，但播放器和转换器保留了多套兼容行为。已检查主流证据把命令时间直接解释为 decimal Beat，
而不是固定 `1/2048` tick；offset bias、`cv` scale、负 alpha 和 event repair 仍存在分歧。

相关项目决定见：

- [0001：运行时只有一个物理主时钟](../decisions/0001-single-runtime-clock.md)；
- [0007：外部谱面格式使用版本化 semantic profile](../decisions/0007-versioned-conversion-semantic-profiles.md)。

## 1. 证据范围

| 证据 | 快照 | 相关路径 |
|---|---|---|
| Phira PEC parser | `824e24b97af53c2e14c4e2dfa13ecd36d87c9e06` | `refer/chart/phira/prpr/src/parse/pec.rs` |
| Phira package/player | 同上 | `refer/chart/phira/prpr/src/scene/game.rs`、`prpr/src/info.rs` |
| Phira Docs | `909a4913d726c13af8ea6904501faea6f91bd2ae` | `refer/chart/phira-docs/src/chart-standard/chart-format/pe/` |
| sim-phi/extends PEC converter | `00a6602e9b837dba4453aa758cdab06cafc79162` | `refer/chart/extends/src/pec/` |
| phispler-ext PEC converter | 无 Git 历史 | `light_utils.py` SHA-256 `89e5c71c76ab011494292b06105e8f008149f6b4720c69eeee5b9eaa7a1204d6` |

Phira Docs 明确声明其 PEC 文档只介绍结构、不保证行为准确；因此执行语义必须同时参考多个实现。

## 2. 文件结构与词法

### 2.1 基本结构

典型 PEC：

```text
<offset-ms>
bp <beat> <bpm>
<commands...>
```

命令和参数用 ASCII whitespace 分隔，数值通常允许 decimal。没有已确认的 comment、string、JSON
object 或内建 metadata 语法。

保守的 lossless parser 应保留：

- 原始行和 token span；
- 数字原文；
- command source order；
- Note 与后续 `#`/`&` 的邻接关系；
- 未识别 command/多余 token；
- first token/first line 的 offset 形态。

### 2.2 line-based 与 token-stream parser

实现接受边界不同：

| 实现 | 解析边界 |
|---|---|
| Phira | 第一物理行读取 offset，之后逐行解析一个 command；普通 command 拒绝多余 token，但 Note suffix 探测会误吞特定多余 token |
| extends | 对整个文件按 whitespace 切 token，command 不严格受换行约束 |
| phispler-ext | 先按行分类，再把 Note、`#`、`&` 三个列表按位置 `zip` |

因此换行不是可以完全忽略的细节。尤其是缺少某个 `#`/`&` 时，phispler 的全局 zip 可能错配或
丢掉后续 Note，而 last-note parser 会产生另一结果。Parser profile 必须定义允许 inline suffix、
独立 suffix line 和缺失 suffix 的行为。

## 3. PEC 不保存完整包信息

PEC 只保存 chart offset、BPM、Line command 和 Note，不保存曲名、谱师、音乐、插图等完整元数据。
Phira 风格包通常由根目录 `info.yml` 指定 `.pec`、music、illustration 和显示信息；其他历史工具可
使用不同外层清单。

因此：

- PEC chart parser 不应猜资源文件名；
- package importer 负责 metadata/resource discovery；
- chart offset 与 package offset 分别进入 provenance；
- ZIP/PEZ 的历史用法不成为 PEC payload 语义，也不成为 FCBC 的容器设计依据。

PhiEditer UI 文档称最多创建 30 条 Line。已检查 parser 会按最大 line ID 动态扩容，因此“30”目前
只能标记为编辑器约束；若 target profile 要兼容原编辑器，exporter 再执行该限制。

## 4. offset bias 是已确认歧义

文件开头数值单位为毫秒，但实现还减去固定 bias：

| 证据 | chart offset 候选公式 |
|---|---|
| Phira parser | `(rawOffset - 150) ms` |
| phispler-ext | `META.offset = rawOffset - 150` ms |
| Phira Docs | `(rawOffset - 175) ms` |
| sim-phi/extends | `rawOffset / 1000 - 0.175` seconds |

PEC 没有可靠版本字段来选择 150 或 175。Importer 不得用“当前主流”静默决定；source profile
必须声明 bias、符号和最终 FCS 公式，例如记录：

```text
rawOffset
bias
interpreted chart offset
package offset
final audioTime/chartTime relation
```

150↔175 不是 Repair：两者都可能是合法 dialect/runtime 解释。只有用户明确选择或 package
producer 证据才能消除歧义。

## 5. `bp` 与时间单位

### 5.1 `bp` command

```text
bp <startBeat> <bpm>
```

`startBeat` 和所有 Note/event time 共享同一 source Beat 坐标。BPM 分段的物理时间为：

```text
seconds += beatDelta * 60 / segmentBpm
```

Importer 应保留 decimal Beat 的 exact representation，验证 BPM、顺序、同 Beat point 和第一个有效
区间。排序、去重、把负 start clamp 到 0 或补默认 BPM 都属于 Repair。

Phira 在第一个需要 time mapping 的非-`bp` command 出现时冻结 BPMList；之后再出现 `bp` 会报错。
extends 和 phispler 会先收集、排序全部 `bp`。这是 parser/dialect 边界，不能靠重排输入掩盖。

### 5.2 主流证据不是 `tick/2048`

三份独立实现都直接把 PEC 时间 number 当 Beat：

- Phira：`take_f32()` 后直接调用 `BpmList::time_beats(value)`；
- phispler-ext：写成 RPE `[value, 0, 1]`；
- extends：`BpmList.calc(beat)` 直接以输入 number 做 BPM 分段。

因此早期 Conversion 候选中的默认 `pec.time.tick2048: beat = sourceTick/2048` 与证据不符。该规则
现已从 `fcs-conversion.md` 与 mapping registry 删除，并在 conformance 中列为 forbidden rule ID；
只有未来取得明确 producer/runtime 证据后，才能作为独立 versioned legacy profile 重新引入。

如果某个旧文件大量使用 2048、4096、8192，它可能来自 tick 方言或错误 fixture，但数值大小本身
不足以自动判定；strict mode 必须要求 profile，compatible mode 也必须报告候选和选择理由。

## 6. Note commands

### 6.1 command 形状

| Command | 参数 |
|---|---|
| `n1` | `line startBeat x side fake`（Tap） |
| `n2` | `line startBeat endBeat x side fake`（Hold） |
| `n3` | `line startBeat x side fake`（Flick） |
| `n4` | `line startBeat x side fake`（Drag） |

Note 后可跟：

```text
# <speedMultiplier>
& <widthMultiplier>
```

Phira 和 extends 在缺失时使用 1；phispler-ext 的 zip 实现实际上假设每个 Note 都有对应 speed/size
行。缺失值默认与 suffix 关联算法必须分开记录。

Phira 的 Note 行实现会先无条件读取一个 token检查是否为 `#`，再读取一个 token检查是否为 `&`。
因此 inline 只有 `&` 时可能被吞掉，某些任意尾随 token也可能被误吞；独立下一行的 `#`/`&` 则
走 last-note command。该行为是 parser 缺陷/兼容证据，不是 PEC grammar。

### 6.2 side、fake 与 Hold

- `side = 1` 被已检查实现视为一侧/above；其他值被视为另一侧；
- 文档常用 `side = 2` 表示 below；Phira strict parser 只对 fake 强制 0/1，side 的其他整数会落到
  below；
- `fake = 1` 表示无判定 fake，`0` 表示真实 Note；
- Hold endpoint 是 `n2` 的第二个 Beat，必须验证 `endBeat > startBeat`；
- `#` 和 `&` 只修改最近 Note，不能跨未知 command 猜关联。

### 6.3 Note X 与 Line X 是不同坐标域

PEC Note X 是相对于判定线中心的局部坐标，已检查实现共同支持约 `-1024..1024`：

```text
noteXNormalized = sourceX / 1024
noteXpx = sourceX * 1920 / 2048
```

证据：

- Phira 直接使用 `sourceX / 1024`；
- extends 先使用 `sourceX / 115.2` 生成 PGR Note unit，乘 PGR 的 108px/unit 后等于
  `sourceX * 1920/2048`；
- phispler 写为 RPE `sourceX/2048*1350`，再映射到 1920 也得到相同结果。

Line `cp/cm` X 则是绝对 canvas 坐标，0..2048、中心 1024：

```text
lineXpx = (sourceX / 2048 - 0.5) * 1920
```

当前 Conversion rule 必须拆成 `pec.note-x.relative2048` 与 `pec.line-x.canvas2048`；共用一个
`pec.x.canvas2048` 会把 Note 0 错误映射到屏幕左边，而不是 Line 中心。

## 7. Line commands

### 7.1 瞬时 command

| Command | 参数 | 作用 |
|---|---|---|
| `cv` | `line beat speed` | 立即设置 scroll speed |
| `cp` | `line beat x y` | 立即设置 Line position |
| `cd` | `line beat angle` | 立即设置 Line rotation |
| `ca` | `line beat alpha` | 立即设置 Line alpha/扩展状态 |

瞬时 command 是 point/step 赋值，不应被 lowering 成普通零长插值 segment。

### 7.2 缓动 command

| Command | 参数 | 作用 |
|---|---|---|
| `cm` | `line startBeat endBeat targetX targetY easing` | 从当前 position 缓动到目标 |
| `cr` | `line startBeat endBeat targetAngle easing` | 从当前 rotation 缓动到目标 |
| `cf` | `line startBeat endBeat targetAlpha` | 从当前 alpha 线性变化到目标 |

`cm/cr` easing 编号与 RPE easing table 有历史关联，但具体表、未知 ID 和端点行为仍需 profile。
`cf` 没有 easing 参数，按线性处理。

缓动 command 只保存目标值，起始值来自该 Line 在 `startBeat` 的当前状态。Command source order、gap、
同 Beat point 和前一事件的端点会影响结果。

实现分歧：

- Phira 要求第一个插值事件之前已经有 concrete value，否则失败；
- extends 从 0 初始化 position/rotation/alpha，并把 easing 按每个 PGR raw T 单位采样成大量线性段；
- phispler 通过“距离 start time 最近的已有 event end”猜 start value。

后两种转换不自动等价于原编辑器执行语义；尤其 extends 的整数步采样属于 approximation，不应被
报告为精确 event preservation。

## 8. Line 坐标、rotation 与 alpha

### 8.1 Line position

文档和实现共同使用 2048×1400 canvas：

```text
lineXpx = (sourceX / 2048 - 0.5) * 1920
lineYpx = (sourceY / 1400 - 0.5) * 1080
```

源文件中心为 `(1024, 700)`。最终 Y-up/Y-down 仍要由 coordinate profile 明确；不要只记录 scale。

### 8.2 rotation

Phira Docs 描述源文件中顺时针为正。Phira 和 extends 都在导入时取负；phispler 先保持数值写入
RPE，而 RPE importer 通常再取负。映射到 FCS 时应显式使用：

```text
fcsAngle = -sourceDegrees * pi / 180
```

并在 provenance 中记录 source sign，而不是仅保存转换后的 radians。

### 8.3 alpha 与负值扩展

普通 alpha 以 0..255 表示。文档还记录 `-1` 会隐藏 Line 上所有 Note，并提示存在其他未考证
负值行为。

- Phira 对非负值除以 255，对负值保留，并启用 `pe_alpha_extension`；
- extends 把负 alpha clamp 到 0 并 warning；
- 直接 clamp 会丢失来源 runtime extension。

负 alpha 必须由 source profile 映射到明确 visibility/extension；未知负值 strict 失败或 preserve，
不能静默当 0。

## 9. `cv`、speed 与 distance

`cv` 的 raw value scale 没有统一实现：

| 证据 | 处理 |
|---|---|
| Phira | `sourceCv / 5.85` 后积分为内部 height |
| extends | `sourceCv / 7.0` 后生成 PGR speed/floorPosition |
| phispler-ext | `sourceCv / 1400 * RPE_HEIGHT` 写入 RPE speed event |
| Phira Docs | 只写默认值约 10，没有给出跨 runtime canonical scale |

这些公式不能只靠不同内部单位相互比较。Source profile 必须通过可复现 probe 固定：

- raw `cv` 到 source scroll velocity 的单位；
- 与目标 logical height 的 scale；
- BPM segment 下的 distance 积分；
- Hold 头尾 geometry；
- 负 speed 和零 speed 行为。

Phira 在某 Line 没有 speed event 时会进入不安全的首元素访问路径，并在首 speed time 大于 0 时补
`(0,0)`。这说明该 parser 依赖至少一个 `cv`，并带有首事件 Repair；它不证明 PEC 规范要求默认
speed=0。文档称默认 speed=10，也不能在缺少 profile 时直接替代输入。

## 10. event 顺序、overlap 和 Repair

PEC 是 stateful command stream。同一时间 command 的 source order、之前的 point 和当前值都可能
改变结果。Lossless parser 不得先按 command kind 分组后丢掉全局顺序。

已观察 Repair/近似：

- Phira 按 `(endTime,startTime)` 排序事件，并把 overlap 后一事件的 start 裁到前一 end；
- extends 对 `startTime > endTime` 禁用 event并 warning；
- extends 把未知 easing 改为 linear；
- extends 按整数 raw T 采样 easing；
- phispler 按 command kind 独立排序，使用近邻 event 猜缓动 start；
- 多数实现会 clamp、忽略或默认处理越界 coordinate/alpha/fake。

这些都必须进入 ConversionReport。Parser mode、semantic profile 和 repair mode不能合并成一个
“PEC compatible”开关。

## 11. 已确认的实现分歧

| 主题 | 分歧 | 影响 |
|---|---|---|
| time unit | 主流 direct decimal Beat；旧 fixture 暗示 tick2048 | 全部 Note/event time |
| offset bias | 150ms / 175ms | 音频同步 |
| `cv` scale | `/5.85`、`/7`、RPE scale | scroll/distance/Hold |
| Note X vs Line X | 相对中心 / 绝对 canvas | Note 横向位置 |
| command boundary | 每行 / 全 token / 分类 zip | suffix 与 source order |
| late `bp` | Phira 拒绝；其他实现先收集排序 | tempo mapping |
| first interpolation | 失败 / 从 0 / 猜最近 end | Line motion |
| overlap | clip / disable / 分组重排 | motion/visibility |
| negative alpha | 扩展执行 / clamp | Line 与 Note visibility |
| easing | 精确 runtime / 整数步采样 / fallback | motion fidelity |
| line count 30 | 编辑器限制；parser 可更多 | export capability |

## 12. Semantic profile 需要控制的维度

以下是维度，不是最终 profile ID：

```text
producer/editor/runtime identity
time domain: decimal Beat | declared legacy tick dialect
BPM command ordering/negative-beat behavior
offset bias: 150ms | 175ms | other-declared
Note suffix association and defaults
side/fake accepted domain
Note X relative coordinate rule
Line canvas axes and rotation sign
easing table and interpolation start-value rule
event gap/overlap/source-order rule
negative alpha extension
cv scale and distance integration
missing-first-speed behavior
package metadata/resource policy
```

只有当一个输入没有触及候选差异，且转换结果经证明 canonical-equivalent 时，才能自动选择。
例如 offset raw value 即使为 0，150/175 仍产生不同结果，不能因文件简单而忽略 profile。

## 13. Parser、compatible 与 Repair 边界

### Parser 应保留

- first-line offset 原文；
- 每个 command、参数和行/span；
- decimal 数字原文；
- 全局 source order；
- `#`/`&` 与 Note 的物理邻接；
- unknown command 和 extra token；
- raw side/fake/alpha/easing/cv。

### Compatible interpretation

- 接受 suffix inline 或独立行，但记录 parser dialect；
- 根据明确 profile 选择 150/175；
- 根据 producer 声明选择 direct Beat 或 legacy tick；
- 使用 package manifest 补 metadata/resource root；
- 对不影响当前输入的多个 profile 合并候选。

### Repair 必须 opt-in

- 排序/移动 `bp`；
- 补默认 BPM、speed、point value；
- 交换/裁剪 `startBeat > endBeat`；
- 裁剪 overlap；
- clamp coordinate/alpha；
- 未知 easing 改 linear；
- 丢 unknown command；
- 猜失配的 `#`/`&` 应属于哪个 Note；
- 修正非法 Hold endpoint、side 或 fake。

## 14. Export 要点

- target 必须声明 PEC producer/runtime profile，不能只写 `format = pec`；
- 默认不得把 Beat 乘 2048；只有 target profile 明确要求 tick 方言时才量化；
- raw offset 应按 target bias 的数学逆写出，并记录 package offset；
- BPM、Note、Line event 的 exact boundaries 先固定，再按目标数值能力量化；
- Note X 与 Line X 使用不同逆公式；
- stateful command order必须构造得使每个 `cm/cr/cf` 起始值明确；
- 每个 Note 的 `#`/`&` 输出策略要匹配目标 parser；
- `cv` 使用 target runtime scale，并同时验证累计 distance/Hold；
- PEC 无法表达的 parent、multi-layer、Render、runtime expression、custom resource 字段必须显式
  negotiation；
- 写出后用同一 target profile 重导入并比较 canonical semantics。

## 15. 当前 examples 状态

### `examples/pec/all-notes.pec`

不能作为默认 decimal-Beat PEC valid fixture：

- 大量时间为 2048/4096/6144/8192，明显依赖未声明的 tick 假设；按已检查 parser 会直接成为巨大
  Beat；
- `n2 0 2048.00 2 ...` 的 Hold end=2 早于 start=2048；
- `n3 ... 1 0 2048.00` 在 fake 参数后还有不属于 Note grammar 的额外数值；Phira快照会在 suffix
  探测时意外吞掉它，而 extends 会把它当未知 command，因此不能据此扩展合法语法；
- 没有 `cv`，Phira当前 Line speed 路径不能安全处理；
- 因此不能用它证明 `source/2048` 是 PEC 规范。

### `examples/pec/simple.pec`

同样是 legacy/invalid candidate：

- 使用 4096/8192 时间而未声明 dialect；
- Hold start=4096、end=0，区间反向；
- `cp` 的 Y=1789 超出文档 0..1400 canvas；
- 没有 `cv`；
- 不能作为现代 strict PEC 的 expected-valid fixture。

未来至少需要分别建立：

- direct decimal-Beat、150ms profile fixture；
- direct decimal-Beat、175ms profile fixture；
- 若找到 producer 证据，再建立 tick2048 fixture；
- `cv` scale probe；
- Note X/Line X 区分；
- negative alpha、easing、same-Beat order、suffix 关联和 invalid Hold fixture。

## 16. 证据索引

### Phira

- `refer/chart/phira/prpr/src/parse/pec.rs`
  - `Take::take_time`：number 直接作为 Beat；
  - `parse_pec`：150ms、command grammar、Note suffix、`cv / 5.85`；
  - `parse_events`/`sanitize_events`：start value、排序与 overlap clipping；
  - `parse_speed_events`：首 speed event repair；
  - `parse_judge_line`：Line coordinate、negative alpha 和 distance；
- `refer/chart/phira/prpr/src/scene/game.rs`：package format detection、offset composition；
- `refer/chart/phira/prpr/src/info.rs`：Phira `info.yml` 字段。

### Phira Docs

- `refer/chart/phira-docs/src/chart-standard/chart-format/pe/index.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/pe/basic.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/pe/event.md`；
- `refer/chart/phira-docs/src/chart-standard/chart-format/pe/note.md`；
- `refer/chart/phira-docs/src/chart-standard/index.md`、`chartinfo.md`：package/metadata。

### extends

- `refer/chart/extends/src/pec/pec2json.ts`
  - 175ms、whitespace token parser、Note/Line coordinate、`cv / 7`；
- `refer/chart/extends/src/pec/BpmList.ts`：direct Beat→PGR T；
- `refer/chart/extends/src/pec/LinePec.ts`：speed/floorPosition、Hold 和整数步 easing 展开；
- `refer/chart/extends/src/pec/easing.js`：easing table。

### phispler-ext

- `refer/chart/phispler-ext/src/light_utils.py`
  - `pec2rpe`：150ms、direct Beat、Note/Line coordinate、suffix zip；
  - 该目录没有 `.git`，引用时必须同时使用本页记录的文件 SHA-256。
