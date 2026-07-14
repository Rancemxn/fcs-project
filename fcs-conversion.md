# FCS Conversion Specification 1.0.0

状态：Frozen（2026-07-14）

本文定义 FCS 与 PGR、RPE、PEC 等外部谱面格式之间的 canonical 转换、保真保存、能力协商、
repair 和机器可读 ConversionReport。它不改变 `fcs.md` 的 Core 执行语义。

---

## 1. 原则

转换器必须：

1. 先解析来源格式自身语义，再 lowering 到 FCS canonical model；
2. 区分原始值、解释后的值和 FCS canonical 值；
3. 不把“成功解析”误报为“无损转换”；
4. 不静默丢字段、猜 parent、填 speed gap、交换非法区间或 clamp 值；
5. 在目标能力不足时选择等价映射、可验证烘焙、仅保存、显式丢弃或失败；
6. 用 canonical semantics 比较 round-trip，不以文本/JSON 顺序相等代替；
7. 保留 source format/version/hash 和每个重要映射的 provenance；
8. 让用户显式修改的 FCS 语义优先于陈旧 raw snapshot。

---

## 2. Source descriptor 和 provenance

每次 import 建立：

```text
SourceDescriptor {
    format: fcs | pgr | rpe | pec | extension-id
    version: string
    hashAlgorithm: sha256
    hash: bytes
    parserMode: strict | compatible | repair
    parserId/version
}
```

TargetDescriptor：

```text
TargetDescriptor {
    format: fcs | fcbc | pgr | rpe | pec | extension-id
    version: string
    profile: optional string
    capabilitySetHash: sha256
    writerId/version
}
```

CapabilitySet 必须 canonical serialization 后计算 hash，使 report 能证明 negotiation 使用了哪一组
目标能力；writerVersion 只用于复现，不覆盖 capability 内容。

每个 canonical field 可以携带：

```text
sourcePath
sourceValue
sourceUnit
sourceOrder
sourceEntityId
mappingRule
semanticStatus
originState
```

OriginState：

```text
unset
explicit-default
explicit-value
inherited
imported
generated
user-modified
```

不得通过值是否等于默认值推断 originState。

---

## 3. Semantic status

每项映射使用：

| Status | 含义 |
|---|---|
| native | 来源字段与 FCS Core 语义直接相同 |
| mapped | 通过确定且可逆/等价公式映射 |
| equivalent | 表示不同但执行语义经证明相同 |
| approximated | 有声明误差界的近似 |
| preserved | 仅保存在 fidelity/extension，不由 Core 执行 |
| runtime-only | 依赖已声明非 Core runtime feature |
| repaired | 非法/矛盾来源经显式 repair 修改 |
| dropped | 用户允许后未写入目标 |
| unsupported | 无可接受表示，转换失败或部分失败 |

`preserved` 不等于 `lossless`。只有目标重新导出可恢复且执行语义也不丢失时，整体才能是
lossless。

---

## 4. Conversion 策略

### 4.1 Semantic

以当前 FCS canonical 语义为准。可以烘焙或报告目标损失；不会为了还原来源 raw field 而覆盖
用户修改。

### 4.2 Roundtrip

若 canonical field 仍为 imported 且其依赖未修改，可以使用已验证 provenance/raw payload
回写。任何 user-modified field 使受影响来源 payload 失效，必须重新导出并报告变化。

### 4.3 Strict

任何 approximated、preserved-only、runtime-only、repaired、dropped、unsupported 或无法证明
equivalent 的行为都使转换失败。失败仍必须输出 report，不输出伪成功目标文件。

### 4.4 Repair

Repair 是独立 opt-in mode，不是默认 strategy。每个 repair 必须先记录原 diagnostic、source
path、old/new value、rule ID 和 semantic impact。Repair 后整体状态至少为 repaired。

---

## 5. Capability negotiation

Target exporter 在转换前发布 CapabilitySet：

```text
format/version
note kinds and fields
time domains and precision
tempo/line-local tempo
Track interpolation/easing/overlap
parent/inherit/transform
scroll speed/distance/reverse
runtime expression environments
resources/metadata/credits
render/effect
extensions
numeric limits
entity/event limits
```

每个 canonical feature 选择：

```text
direct
equivalent rewrite
adaptive bake
preserve only
drop with authorization
unsupported
```

Exporter 不得先丢数据再生成报告；negotiation 和 lowering decision 必须先完成。Adaptive bake
使用 `fcs.md` 属性误差和强制精确边界，目标采样率不能覆盖 Note/Hold/tempo/event 精确时刻。

---

## 6. ConversionReport

### 6.1 顶级结构

```text
ConversionReport {
    specificationVersion: "1.0.0"
    operationId: stable string
    source: SourceDescriptor
    target: TargetDescriptor
    strategy: semantic | roundtrip | strict
    repairMode: bool
    status: lossless | equivalent | approximate | preserved-only |
            repaired | unsupported | failed
    entries: ordered array<ConversionEntry>
    summary: counts by severity/status/category
    outputHash: optional sha256
}
```

整体 status 按最严重 entry 决定，严重度顺序：

```text
lossless < equivalent < approximate < preserved-only < repaired < unsupported < failed
```

若 conversion 本身成功但含 repaired，status=repaired；没有生成目标 bytes 时 status 至少 failed。

### 6.2 Entry

```text
ConversionEntry {
    id: stable string
    category: stable enum/string
    severity: info | warning | error
    semanticStatus
    sourcePath: optional string
    targetPath: optional string
    entityId: optional stable ID
    sourceValue: optional typed value
    canonicalValue: optional typed value
    targetValue: optional typed value
    ruleId: stable mapping rule
    message: human text
    errorMetric: optional descriptor
    dependencies: array<entry id>
}
```

Entry 按 conversion traversal 的 canonical entity order，再按 field schema order；不能依赖哈希表。
Message 不稳定，category/ruleId/status 是机器接口。

### 6.3 Error metric

Approximation 必须记录 domain、metric、declared maximum、observed/verified maximum、validation
method、sample/segment count、强制边界和 source expression hash。只写“已采样”不够。

---

## 7. Raw snapshot 和回写失效

Raw snapshot 可以保存完整 source bytes/hash，也可以保存 typed source payload。精确回写的必要条件：

- source hash 和 payload 未损坏；
- 所有影响该 payload 的 canonical field origin 仍为 imported/inherited；
- mapping rule/version 与 importer 相同或兼容；
- target format/version 与来源一致；
- exporter 可以证明 raw payload 与当前 canonical semantics 一致。

任何 user-modified dependency 使相关 payload stale。Exporter 可以保留无关 raw 区域，但必须
重新生成受影响区域，不能以整个 snapshot 覆盖用户修改。

---

## 8. 时间、单位和坐标的公共映射规则

### 8.1 时间

所有来源 time 先保留 exact source representation，再映射到 canonical chartTime。来源 line-local
BPM 可以用于解析来源 tick，但结果一旦进入 canonical Note time 就不再是独立运行时 clock。

目标格式只支持离散 tick 时：

- tempo/Note/Hold/event boundary 必须先作为 exact constraint；
- 选择最近可表示 tick 并记录 time error；
- Hold end 必须保持严格大于 start，否则 strict 失败，repair 需要显式记录；
- 不得用全局统一 rounding 后假设顺序仍有效。

### 8.2 坐标

转换明确记录 source space、axis、origin、scale、aspect policy 和 target space。FCS logical world
是 1920×1080、中心原点、Y-up。映射公式必须进入 ruleId；不能只存转换后数字。

### 8.3 角度和 alpha

角度必须记录 clockwise/counterclockwise 和零方向。Alpha source range/encoding 显式转换到 FCS
linear scalar；越界默认错误，不静默 clamp。

### 8.4 Core mapping rule registry

Conversion 1.0 固定以下 rule ID 和公式；变量使用 source 原始数值，结果使用 FCS canonical unit：

| Rule ID | 公式 |
|---|---|
| `pgr.time.t32` | `chartBeat = T / 32` |
| `pgr.note-x.unit108` | `positionXpx = sourceX * 108` |
| `pgr.line-x.normalized` | `lineXpx = (sourceX - 0.5) * 1920` |
| `pgr.line-y.normalized` | `lineYpx = (sourceY - 0.5) * 1080` |
| `pgr.rotation.clockwise-deg` | `fcsAngle = -sourceDegrees * π/180` |
| `pgr.offset.seconds` | `audioOffset = sourceOffset * 1s` |
| `rpe.beat.abc` | `beat = a + (c==0 && b==0 ? 0 : b/c)`；`c==0 && b!=0` 非法 |
| `rpe.x.canvas1350` | `xPx = sourceX * 1920/1350` |
| `rpe.y.canvas900` | `yPx = sourceY * 1080/900` |
| `rpe.alpha.byte255` | `alpha = sourceAlpha/255` |
| `rpe.offset.milliseconds` | `audioOffset = sourceOffset * 1ms` |
| `rpe.speed.scale4_5` | `canonicalSpeed = sourceSpeed/4.5` |
| `pec.time.tick2048` | `beat = sourceTick/2048` |
| `pec.x.canvas2048` | `xPx = (sourceX/2048 - 0.5) * 1920` |
| `pec.y.canvas1400` | `yPx = (sourceY/1400 - 0.5) * 1080` |
| `pec.offset.bias150ms` | `audioOffset = (sourceOffset-150) * 1ms` |

逆向导出使用公式的数学逆，再按目标字段规则量化。任何 rounding 必须单独记录 rule ID 后缀
`.round-nearest-even`、量化前后值和误差，不能把 rounding 并入上述 exact mapping 而仍标记 mapped。

PGR `floorPosition` 的画面 length scale 不由 JSON formatVersion 唯一确定，不进入无参数 Core rule。
Importer 必须从声明的 engine capability profile 得到 `floorScalePx`；缺失时可以保留无量纲
floorPosition 并验证 speed 积分，但视觉 distance 状态至少是 `preserved`，strict conversion 失败。
不得在 648px、1080px 或宿主 viewport 间静默选择。

---

## 9. PGR Import

本规范中的 PGR 包括社区常见 PGR v1/v3 JSON。具体版本探测必须来自格式字段和结构，不仅靠
文件扩展名。

### 9.1 Time 和 BPM

- 使用 `pgr.time.t32` 得到 exact chartBeat，再用每条 PGR line 的 BPM 把其整数/实数 `T`
  映射到 canonical chartTime；
- 原始 T、line BPM、版本和换算结果进入 provenance；
- 不把 line BPM 建成 FCS 第二物理 clock；
- 同一音频时刻来自不同 line 的 Note 必须得到相同 chartTime 比较语义；
- 能重建的公共音乐 tempo 可进入 global tempoMap；不能证明公共 tempo 时使用 canonical time
  并保存 line-local source mapping。

### 9.2 Line event

- PGR v1/v3 move、rotate、alpha event 使用 `replace` Track；来源 overlap 若依赖特定引擎覆盖
  行为，必须拆成带明确 priority 的 Track 或标记 compatibility runtime，不能用“通常”推断；
- event 区间、easing 和 source order 保留；
- PGR normalized coordinate 按版本明确映射到 1920×1080；
- 非法 overlap 不按最后文本项静默覆盖，compatible mode 必须记录 chosen rule；
- father/层级若格式版本不支持则不猜测。

### 9.3 Speed 和 floorPosition

- speed event 映射 scrollSpeed；
- source floorPosition 作为验证点和 fidelity；
- canonical distance 与 source floorPosition 不一致时记录 category `distance-mismatch`；
- strict import 失败；compatible 可以选择 source engine-compatible rule并记录 runtime fidelity；
- 固定 120Hz 历史积分不能冒充 Core exact，可以作为 source compatibility evaluator。
- floorPosition 画面 scale 必须遵守第 8.4 节 capability 要求。

### 9.4 Note

- tap/hold/flick/drag 映射 Core kind；
- above/below 映射 gameplay.side；
- positionX 映射 line-local length；
- speed 映射 Note scrollFactor；
- fake/非判定 source flag 映射 judgment.enabled=false，不改变 kind；
- Hold time/endTime 必须验证；
- source floorPosition、type code 和顺序保留。

---

## 10. RPE Import

### 10.1 BPMList、Beat 和 bpmfactor

- RPE Beat 通过全局 `BPMList` 映射 chartTime；
- Beat triplet/decimal 保留为 exact rational；
- `bpmfactor` 只影响 line scroll coordinate/speed mapping，不修改 Note 判定 chartTime；
- 非法 BPM、乱序和重复 point 按 strict/repair 规则处理；
- 同 beat step 使用 FCS tempoMap 同 beat source order 语义。

### 10.2 Event layers

- 每个 event layer index 保存在 provenance；
- moveX/moveY/rotate/alpha/scale 根据来源引擎语义映射 replace/add Track；
- 任何 add/replace 选择必须有 versioned ruleId；
- 多层冲突通过显式 FCS blend/priority 表达，不依赖 JSON array 偶然顺序；
- easing、Bezier 参数、start/end 和 easingLeft/Right 精确保留；
- 无法直接表达的曲线进入 adaptive bake并记录误差。

### 10.3 Parent 和 transform

- father 映射 line.parent；
- `rotateWithFather` 映射 inherit.rotation；
- 其他 inherit 仅在来源字段/版本明确定义时开启；
- parent 不存在、self/cycle 默认错误；repair 断开边必须记录被移除关系；
- source anchor、texture scale、zOrder 和 cover 分别映射，不能混入一个矩阵。

### 10.4 Note 和 presentation

- type 映射 Core kind；
- startTime/endTime 映射 gameplay time；
- positionX、speed、alpha、size、visibleTime、yOffset、tint/texture 映射 presentation；
- visibleTime 转为绝对 visible interval并保存换算 rule；
- fake 映射 judgment.enabled=false；
- above 映射 side；
- 不把 alpha=0 推断为 fake；
- controls/effects 若无 Core 对应，进入 Render/extension 或 preserved，并报告。

---

## 11. PEC Import

### 11.1 Tick、bp 和 offset

- PEC integer/decimal tick 和 `bp` 列表先构建 exact source tempo mapping；
- 映射到 canonical chartTime/global chartBeat；
- 原始 tick、bp segment 和 parser compatibility mode 保留；
- PEC offset 必须通过 versioned rule 映射到 FCS `audioOffset`，报告中同时记录来源符号解释和
  最终 `audioTime = chartTime + audioOffset` 值；
- 不允许使用“音乐快/慢”的自然语言猜符号。

### 11.2 Commands

- `n1/n2/n3/n4` 等 note command 映射 Core kind/side/time/position；
- Hold endpoint 验证严格大于 start；
- `cp/cd/ca/cm/cr/cf` 等 line command 合并成显式 Track；
- `cv` 映射 scrollSpeed，source engine scale 和积分规则进入 ruleId；
- 点 command 是 point/step，不生成零长普通 segment；
- fake、宽度、alpha、motion/easing 扩展按对应 Core/Render/fidelity 处理；
- 同时刻 command 保留 source order，但 canonical conflict 必须显式解决或报错。

---

## 12. Export 到 PGR/RPE/PEC

Exporter 必须使用目标明确版本的 CapabilitySet，不存在“generic PGR/RPE/PEC”。

公共顺序：

1. 验证 canonical chart；
2. 选择目标版本和 strategy；
3. negotiation 全部 feature；
4. 固定 exact event boundaries；
5. 对不直接支持的 Track/expression 做等价 partition 或 adaptive bake；
6. 量化目标 time/coordinate 并测量误差；
7. 验证 target constraints 和 Hold/event order；
8. 写目标；
9. 重新 parse 目标并转换为 canonical comparison model；
10. 完成 report 和 output hash。

如果第 9 步结果超过声明误差，输出必须标记 failed，除非用户显式请求保留失败产物用于诊断。

目标不支持 Render、credit、resource、extension 或 dynamic property 时，不得只忽略；根据 strategy
选择 sidecar/preserve、烘焙、drop authorization 或失败。

---

## 13. Canonical semantic comparison

Round-trip 比较至少覆盖：

- tempo 的 beat/time mapping；
- Note count、kind、line、time/endTime、side 和 judgment；
- Line parent DAG 和 world transform test times；
- Track boundary、value 和 derivative（适用时）；
- scroll velocity 和累计 distance；
- Note presentation sample/forced boundaries；
- resource identity/hash；
- metadata/credits 的 required preservation；
- Render semantic scene（目标支持时）。

离散 identity 可以通过 provenance mapping 对齐；不能只按 array index。Continuous property 使用
规范属性误差；event boundary 必须 exact 或单独记录 time quantization error。

整体 `equivalent` 要求 gameplay exact 且 presentation 在目标声明容差内、无 preserved-only/dropped；
`lossless` 还要求所有规范和来源回写信息可恢复。

---

## 14. FCBC 编码

FCBC Fidelity section 保存 ordered Value object：

```text
sources: array<SourceDescriptor>
entities: object keyed by canonical stable ID
fields: ordered provenance records
rawSnapshots: optional references/payload
mappingRuleVersions: object
```

ConversionReport section保存一个或多个第 6 章对象。所有 runtime/editable/archive profile 都保留
canonical Meta/Contributors/Credits/Resources/Sync；Fidelity 可以从 runtime 剥离，archive 必须保留
source snapshot（若输入提供且许可允许）。剥离 fidelity 不得改变 Core execution。

---

## 15. Copyright、隐私和安全

Raw source、作者 identifier、URI 和本机 path 可能含隐私或版权内容。Archive/sidecar 写入必须：

- 不默认嵌入绝对本机 path；
- 不在 report 中复制无关大 payload；
- 遵守资源 license 和用户选择；
- hash 不代表分发许可；
- 测试中私有版权 fixture 必须 opt-in，缺失时不伪装为已验证。

Parser 必须限制 JSON/object 深度、string、entity、event 和 decompressed resource 大小；URI resolver
不得允许未授权网络或目录逃逸。

---

## 16. Conformance fixture

至少包含：

1. PGR v1/v3 的 tempo、speed、floor、四种 Note 和边界；
2. RPE 多 BPM、bpmfactor、多 event layer、father、Bezier、visibleTime、fake；
3. PEC 多 bp、offset、cv、point/easing、fake 和 Hold；
4. 每个 semantic status；
5. semantic/roundtrip/strict/repair 四种模式；
6. stale raw payload；
7. capability negotiation 与 adaptive error；
8. target reparse 后 canonical comparison；
9. deterministic report ordering/hash；
10. private copyright fixture opt-in lane。

每个 fixture 必须记录 source version、target version、strategy、expected status、entry category、
canonical snapshot 和允许误差。
