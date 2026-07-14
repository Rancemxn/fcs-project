# FCBC Container Format 2.0.0 and Execution ABI 1.0.0

状态：Frozen（2026-07-14）

本文定义 FCBC 2.0.0 二进制容器和 FCS Execution ABI 1.0.0。规范治理见
`docs/specification-governance.md`，source/canonical 语义见 `fcs.md`。

FCBC 是经过验证、完成编译期展开的确定性执行包。它不是 source AST dump，也不是允许
任意程序执行的通用 VM 格式。

---

## 1. 基本约定

- 所有多字节整数和 Float64 使用 little-endian；
- 所有 offset 从文件第一个 byte 起算；
- 所有长度单位为 byte；
- `u8/u16/u32/u64` 是对应宽度无符号整数，`i64` 是二补码；
- `f64` 是 IEEE 754 binary64，文件中禁止 NaN 和 Infinity；
- 未明确允许的 reserved field 必须写零，loader 必须拒绝非零；
- 文件必须是随机访问的有限 byte sequence；
- offset+length 的运算必须检查 unsigned overflow；
- section 不得重叠 header、section table 或其他 section；
- section payload 默认 8-byte 对齐，声明更高 alignment 时必须满足声明。

FCBC 2.0 不定义压缩或加密。需要压缩时由外层包格式处理；未知 compression flag 必须拒绝。

---

## 2. 版本和兼容性

Header 同时保存：

```text
sourceFcsVersion
fcbcFormatVersion
executionAbiVersion
```

- FCBC major 不同：loader 必须拒绝；
- FCBC 同 major 的未来 minor：只有 loader 能跳过全部未知 optional section/flag 时可以接受；
- FCBC patch：不得改变布局或已有有效文件语义；
- ABI major 不同：runtime 必须拒绝执行；
- ABI 未来 minor：runtime 只有在 feature flags 和 required section 均受支持时可以执行；
- source FCS version 用于诊断和 provenance，不允许 runtime 据此猜测 descriptor 语义。

Compiler ID/version 仅用于复现和 bug 追踪，不参与兼容判断。

---

## 3. Header

Header 固定 128 bytes：

| Offset | Size | Type | Field |
|---:|---:|---|---|
| 0 | 4 | bytes | magic = `46 43 53 42` (`FCSB`) |
| 4 | 2 | u16 | headerSize = 128 |
| 6 | 2 | u16 | headerFlags = 0 |
| 8 | 2 | u16 | sourceFcsMajor |
| 10 | 2 | u16 | sourceFcsMinor |
| 12 | 2 | u16 | sourceFcsPatch |
| 14 | 2 | u16 | fcbcMajor = 2 |
| 16 | 2 | u16 | fcbcMinor = 0 |
| 18 | 2 | u16 | fcbcPatch = 0 |
| 20 | 2 | u16 | abiMajor = 1 |
| 22 | 2 | u16 | abiMinor = 0 |
| 24 | 2 | u16 | abiPatch = 0 |
| 26 | 1 | u8 | containerProfile |
| 27 | 1 | u8 | numericModel = 1 (`binary64`) |
| 28 | 8 | u64 | featureFlags |
| 36 | 4 | u32 | sectionCount |
| 40 | 8 | u64 | sectionTableOffset |
| 48 | 8 | u64 | fileLength |
| 56 | 32 | bytes | sourceHash (SHA-256) |
| 88 | 4 | u32 | compilerIdString |
| 92 | 4 | u32 | compilerVersionString |
| 96 | 32 | bytes | reserved = zero |

`sourceHash` 对去除可选 BOM、将 CRLF 规范为 LF、除此之外不修改的 UTF-8 source bytes 计算。
如果 FCBC 不是直接由 source 产生而没有 source snapshot，写 32 个零并清除
`FEATURE_SOURCE_HASH_PRESENT`。

`compilerIdString` 和 `compilerVersionString` 是 StringTable index；无值时为 `0xFFFFFFFF`。
Deterministic writer 固定 `sectionTableOffset=128`；未来 minor 只有在 headerSize 增长时才可以移动
section table。Reader 必须使用字段值而不是假定 128，并验证 table 不与 header 重叠。

### 3.1 Container Profile

| Value | Name | 含义 |
|---:|---|---|
| 0 | runtime | Core 执行所需 canonical 数据 |
| 1 | editable | runtime + 编辑/高质量回写所需 fidelity |
| 2 | archive | editable + source snapshot 和完整 provenance |
| 3 | strict-runtime | runtime 且全部值 portable、误差已验证、无 optional runtime extension |

未知 containerProfile 必须拒绝。FCS document profile 不使用该字段，保存在 Meta section。

### 3.2 Feature flags

| Bit | Name |
|---:|---|
| 0 | SOURCE_HASH_PRESENT |
| 1 | HAS_RENDER |
| 2 | HAS_EXTENSIONS |
| 3 | HAS_FIDELITY |
| 4 | HAS_CONVERSION_REPORT |
| 5 | HAS_SOURCE_SNAPSHOT |
| 6 | HAS_DEBUG |
| 7 | USES_ADAPTIVE_BAKED |
| 8 | USES_REVERSE_SCROLL |

Bits 9–63 在 2.0.0 中保留。写入者必须置零；loader 看到未知置位 bit 必须拒绝，除非未来
FCBC minor 明确将其定义为 safely ignorable。

---

## 4. Section table

Section table 位于 `sectionTableOffset`，包含 `sectionCount` 个 40-byte entry：

| Offset | Size | Type | Field |
|---:|---:|---|---|
| 0 | 4 | u32 | sectionType |
| 4 | 2 | u16 | sectionMajor |
| 6 | 2 | u16 | sectionMinor |
| 8 | 2 | u16 | sectionPatch |
| 10 | 2 | u16 | sectionFlags |
| 12 | 1 | u8 | alignmentLog2 |
| 13 | 3 | bytes | reserved = zero |
| 16 | 8 | u64 | offset |
| 24 | 8 | u64 | length |
| 32 | 4 | u32 | checksum |
| 36 | 4 | u32 | reserved = zero |

`alignmentLog2` 在 0–20；offset 必须是 `2^alignmentLog2` 的倍数。Core writer 使用 3（8-byte）。
`checksum` 是 section payload 的 CRC-32/ISO-HDLC（poly `0x04C11DB7`、reflected、init/final
`0xFFFFFFFF`）；length=0 时仍计算空 payload checksum `0x00000000`。

Section flags：

| Bit | Name | 行为 |
|---:|---|---|
| 0 | REQUIRED | 未知 type/version 时拒绝整个文件 |
| 1 | PRESERVE | 重写 editable/archive 文件时应原样保留未知 optional payload |

其他 bit 必须为零。未知 optional section 可以跳过；未知 required section 必须拒绝。

同一 singleton sectionType 不得重复。允许重复的 extension/debug section 必须在对应章节明确。
Section entry 必须按 `(sectionType, offset)` 升序排列；payload 必须按 entry 顺序布局，以最少
零 padding 满足 alignment。文件末尾不得有未被 section 或 padding 解释的 bytes。

---

## 5. Section 类型

| ID | Name | Core profile |
|---:|---|---|
| 1 | StringTable | required |
| 2 | ConstantPool | required |
| 3 | Meta | required |
| 4 | Contributors | required |
| 5 | Credits | required |
| 6 | Resources | required |
| 7 | Sync | required |
| 8 | TempoMap | required |
| 9 | Lines | required |
| 10 | Notes | required |
| 11 | Tracks | required |
| 12 | Expressions | required |
| 13 | Distance | required |
| 14 | Render | feature-required when HAS_RENDER |
| 15 | Extensions | feature-required when HAS_EXTENSIONS |
| 16 | Fidelity | optional, editable/archive |
| 17 | ConversionReport | optional |
| 18 | SourceSnapshot | archive only |
| 19 | Debug | optional |

“Required” section 即使 record count 为零也必须存在。每个 Core section version 为 `1.0.0`。
Section version major 未知按 REQUIRED 规则处理；未来 minor 只有在本 section 自描述长度允许
跳过新增 record/field 时可以接受。

---

## 6. 通用编码

### 6.1 Counted bytes 和 array

```text
Bytes  := byteLength:u32, data[byteLength], zero padding to 4 bytes
Array<T> := count:u32, T[count]
Record := byteLength:u32, recordVersion:u16, flags:u16, payload[byteLength-8]
```

Record `byteLength` 包含 8-byte prefix，至少为 8，且是 4 的倍数。Reader 必须跳过已知 major
下 record 末尾未知 bytes，但必须验证它们位于 record boundary 内。Writer 2.0.0 将未定义
尾部写零。

### 6.2 Index 和 ID

- Table index 使用 u32，`0xFFFFFFFF` 表示 optional null；
- Stable entity/resource/contributor ID 使用 u64；0 保留为 null；
- ID 由 compiler 对 canonical textual ID 计算 SipHash-2-4 不可接受，因为 key 不稳定；
  Core 使用 SHA-256(`namespace || 0x00 || UTF-8 id`) 的前 64 little-endian bits；
- 如果发生 64-bit collision，compiler 必须报错，不能换盐导致不确定输出。

### 6.3 Value type

| Tag | Type | Payload |
|---:|---|---|
| 0 | null | none |
| 1 | bool | u8 value + 7 zero bytes |
| 2 | int | i64 |
| 3 | float | f64 |
| 4 | string | u32 StringRef + 4 zero bytes |
| 5 | time | f64 seconds |
| 6 | beat | i64 numerator + i64 positive denominator |
| 7 | length | f64 logical px |
| 8 | angle | f64 radians |
| 9 | color | four f64 linear RGBA |
| 10 | vec2 | elementType:u8 + 7 zero bytes + two element payloads |
| 11 | resourceRef | u64 ID |
| 12 | contributorRef | u64 ID |
| 13 | array | elementTag:u8 + 3 zero bytes + elementCount:u32 + encoded Value records |
| 14 | object | fieldCount:u32 + `(StringRef, Value)` pairs |

Standalone Value 编码为 `tag:u8, flags:u8=0, reserved:u16=0, payloadLength:u32, payload`，总长
padding 到 8 bytes。Object key 不得重复，按 source insertion order 保存；standard metadata
writer 使用规范 field order。Array elementTag 不得为 null，所有 element 的 tag 必须相同；
空 array 仍保存 schema 声明的 elementTag。

### 6.4 Property type

Execution ABI property type 使用：

```text
1 bool, 2 int, 3 float, 4 time, 5 beat, 6 length,
7 angle, 8 color, 9 vec2-float, 10 vec2-length
```

String、array、object 和 entity reference 不能作为 runtime varying property。

Execution ABI 的 runtime register 对 float、time、beat、length 和 angle 都保存一个 Float64，单位
分别是 scalar、秒、beat、logical px 和 radian；静态类型 tag 始终保留，禁止跨类型使用。同一
Beat source constant 在 ConstantPool 中保留 exact rational，载入 runtime register 时正确舍入为
binary64。TempoMap 的 exact beat key 仍用于强制边界和 source provenance，不能被该转换替代。

---

## 7. StringTable

Payload：

```text
count:u32
offsets:u32[count+1]
utf8Bytes:u8[offsets[count]]
padding to 8
```

`offsets[0]=0`，offset 非递减且最后等于 byte 区长度。每个 slice 必须是有效 UTF-8，不含
NUL。相同 byte string 只出现一次。Deterministic writer 按 UTF-8 byte lexicographic order
排序；所有 StringRef 是此排序后的 index。

---

## 8. ConstantPool

Payload 为 `Array<Value>`。只允许 scalar、color、vec2、resourceRef 和 contributorRef；array、
object 留在 metadata section。相同 type 和 bitwise payload 的常量只出现一次，按
`(typeTag,payloadBytes)` lexicographic 排序。`-0.0` canonicalize 为 `+0.0`；NaN/Inf 禁止。

---

## 9. Meta、Contributors、Credits、Resources、Sync

### 9.1 Meta

Meta payload：

```text
documentProfile:u8
reserved[3]=0
documentFeatureBits:u32
meta:Value(object)
```

DocumentProfile ID：1 fragment、2 chart、3 playable、4 renderable、5 publishable。Feature bits：
bit0 playable、bit1 renderable；其他 bit 必须为零。Profile/features 必须满足 `fcs.md` 的组合规则。
`meta` object 的标准 key 按以下顺序编码，缺失则省略：

```text
title, subtitle, alternativeTitles, chartVersion, difficulty, level,
description, language, tags, license, documentId, revision, custom
```

类型必须符合 `fcs.md`。运行时、editable、archive profile 都必须保留完整 canonical Meta。

### 9.2 Contributors

```text
count:u32
ContributorRecord[count]

ContributorRecord payload:
id:u64
name:StringRef
aliasCount:u32
aliases:StringRef[aliasCount]
identifiers:Value(object)
custom:Value(object)
```

Record 按 id 升序。Name 不得为空；所有引用唯一且有效。

### 9.3 Credits

```text
count:u32
CreditRecord[count]

CreditRecord payload:
stableId:u64
roleKind:u16
reserved:u16=0
customRole:StringRef or null
label:StringRef
contributorCount:u32
contributors:u64[contributorCount]
custom:Value(object)
```

Credit 保持 canonical display order，不按 ID 重排。`roleKind=0` 表示 custom 且 customRole 必须
存在。标准 ID：1 composer、2 arranger、3 lyricist、4 vocalist、5 instrumentalist、6 mixer、
7 mastering、8 charter、9 illustrator、10 designer、11 programmer、12 publisher。

### 9.4 Resources

```text
count:u32
ResourceRecord[count]

ResourceRecord payload:
id:u64
kind:u16
flags:u16
source:StringRef
mediaType:StringRef
hashAlgorithm:u16
reserved:u16=0
hash:Bytes
metadata:Value(object)
```

Resource kind ID：1 audio、2 image、3 font、4 texture、5 path、6 shader、7 binary。Record 按
id。SHA-256 algorithm ID=1，hash 长度必须 32。Embedded resource flag bit0 表示 source 指向
package member；FCBC 2.0 不在本 section 内嵌任意资源 bytes。

### 9.5 Sync

Singleton Record：

```text
primaryAudioId:u64 or 0
audioOffset:f64 seconds
hasPreview:u8
reserved[7]=0
previewStart:f64 audio seconds
previewEnd:f64 audio seconds
```

无 preview 时 start/end 写 0。Offset 符号严格使用 `fcs.md` 公式。

---

## 10. TempoMap

```text
count:u32
TempoPoint[count]

TempoPoint:
beatNumerator:i64
beatDenominator:i64
chartTime:f64
bpm:f64
sourceOrder:u32
reserved:u32=0
```

Denominator 正，point 按 `(exact beat, sourceOrder)` 排序，第一 exact beat 为 0。ChartTime 是
该 point beat 的 canonical time；同 beat point 的 chartTime 相同。Loader 必须重新验证 BPM、
单调性和相邻映射的一致性，允许的 Float64 误差为 2 ULP。

---

## 11. Lines

```text
count:u32
LineRecord[count]
```

LineRecord 按 stable ID：

```text
id:u64
parentId:u64 or 0
documentOrder:u32
zOrder:i32
inheritFlags:u32
lineFlags:u32
positionDescriptor:u32
rotationDescriptor:u32
scaleDescriptor:u32
alphaDescriptor:u32
transformOriginConstant:u32
textureAnchorConstant:u32
scrollTempoTrack:u32
scrollSpeedTrack:u32
distanceDescriptor:u32
floorScale:f64
integrationOrigin:f64
initialFloorPosition:f64
custom:Value(object)
```

Descriptor index 引用 Tracks/Distance。Identity/default property 也必须有显式 constant descriptor，
避免 runtime 猜默认。LineFlags bit0=`ALLOW_REVERSE_SCROLL`。Inherit bits 0–4 对应 position、
rotation、scale、alpha、scroll。Loader 必须验证 parent DAG 和所有引用类型。

---

## 12. Notes

```text
count:u32
NoteRecord[count]
```

按 `(time,lineId,documentOrder,id)`：

```text
id:u64
lineId:u64
documentOrder:u32
kind:u8              1 tap, 2 hold, 3 flick, 4 drag
side:u8              1 above, 2 below
flags:u16
time:f64
endTime:f64
judgeShape:Value(object)
soundPolicy:u16
scorePolicy:u16
soundResourceId:u64 or 0
positionXDescriptor:u32
scrollFactorDescriptor:u32
xOffsetDescriptor:u32
yOffsetDescriptor:u32
alphaDescriptor:u32
scaleXDescriptor:u32
scaleYDescriptor:u32
rotationDescriptor:u32
colorDescriptor:u32
visibilityDescriptor:u32
textureResourceId:u64 or 0
custom:Value(object)
```

Flags bit0 judgment enabled、bit1 render enabled、bit2 has endTime。非 Hold 不得置 bit2 且
endTime 写 0；Hold 必须置 bit2 且 endTime>time。`judgeShape.kind` 是 `lineDefault`、`rectangle`
或 `circle`，后两者必须提供有限正 geometry。SoundPolicy：1 default、2 none、3 resource；
resource 时 soundResourceId 必须引用 audio。ScorePolicy：1 default、2 none、3 custom，custom
规则必须由 required gameplay extension 提供。Gameplay 数据不能引用 runtime expression。

---

## 13. Tracks 和 PropertyDescriptor

Tracks section：

```text
descriptorCount:u32
PropertyDescriptor[descriptorCount]
```

每个 descriptor 是 Record：

```text
propertyType:u8
descriptorKind:u8
flags:u16
domainStart:f64
domainEnd:f64
payload...
```

DescriptorKind：

| Value | Kind | Payload |
|---:|---|---|
| 1 | Constant | constantPoolIndex:u32 |
| 2 | SegmentTrack | segmentCount:u32 + Segment records |
| 3 | Piecewise | pieceCount:u32 + Piece records |
| 4 | Expression | expressionRoot:u32 |
| 5 | BakedCurve | baked payload |

Domain 可以使用 finite `[start,end]`；flags bit0 表示 unbounded before，bit1 unbounded after。
Descriptor index 按 canonical target path 和 owner ID 的排序分配；引用必须匹配 property type。

### 13.1 Segment

```text
start:f64
end:f64
interpolation:u16
easing:u16
flags:u32
startConstant:u32
endConstant:u32
bezierX1:f64
bezierY1:f64
bezierX2:f64
bezierY2:f64
```

Interpolation：1 step、2 linear、3 easing、4 cubicBezier。非 Bezier 参数写零。Segment 按 start
排序、半开且不重叠。Point step 由 start=end 的专用 flags bit0 表示；普通 segment 禁止相等。

Easing ID：0 linear；1–3 Sine in/out/inOut；4–6 Quad；7–9 Cubic；10–12 Quart；13–15 Quint；
16–18 Expo；19–21 Circ；22–24 Back；25–27 Elastic；28–30 Bounce。每组三项固定为 in、out、
inOut，公式见 `fcs.md`。Interpolation=3 时 easing 必须非零且有效；其他 interpolation 的 easing
写 0。

### 13.2 Piecewise

Piece 是 `(start:f64,end:f64,descriptorIndex:u32,flags:u32)`，按半开区间排列且 descriptor
依赖图必须无环。Piecewise 最后一段可用 endpoint-inclusive flag bit0。

### 13.3 BakedCurve

```text
sourceExpressionHash[32]
declaredMaxError:f64
validationInterval:f64
segmentCount:u32
validationProfile:u16
reserved:u16=0
BakedSegment[segmentCount]
```

BakedSegment 支持 constant、step、linear、cubic Hermite、cubic Bezier 和 dense LUT。每项为
Record，含 start/end、kind、value payload 和 kind-specific coefficient。Dense LUT 必须包含
sampleCount>=2、uniform step 和 sample constant indices；它不允许跨越强制精确边界。

---

## 14. Expressions ABI

```text
nodeCount:u32
ExpressionNode[nodeCount]
```

Node 按拓扑顺序，operand index 必须小于当前 node index。Node 固定 header：

```text
opcode:u16
resultType:u8
arity:u8
operandA:u32
operandB:u32
operandC:u32
immediate:u32
```

未使用 operand 写 `0xFFFFFFFF`。ABI 1.0 opcode：

```text
1 Constant       immediate=ConstantPool index
2 EnvS           3 EnvB           4 EnvQ           5 EnvD           6 EnvP
10 Neg           11 Not
20 Add           21 Sub           22 Mul            23 Div           24 Mod
25 Pow
30 Eq            31 Ne            32 Lt             33 Le            34 Gt
35 Ge            36 And           37 Or
40 Abs           41 Min           42 Max            43 Clamp
44 Floor         45 Ceil          46 Round          47 Sqrt
48 Exp           49 Ln            50 Sin            51 Cos           52 Tan
53 Asin          54 Acos          55 Atan           56 Atan2
60 Easing        immediate=easing ID
70 Choose        operandA=predicate, B=true value, C=false/next choose
```

Expression graph 不含 jump、store、call、loop、recursion、random、IO、emit 或 allocation。
Choose graph 必须有限且最终 else 是普通 value node。Loader 必须重新 type-check node、arity、
environment availability 和 DAG。Runtime 对 invalid math 返回 structured execution error；
strict-runtime 文件在加载前必须证明或验证有效 domain。

Evaluator 可以递归/按需读取拓扑表，不能假定所有较小 index 都必须先求值。And、Or 和 Choose
遵守 `fcs.md` 的 lazy semantics：And 在 A=false 时不求值 B，Or 在 A=true 时不求值 B，Choose
只求值 predicate 和被选 result。未选 node 可以被其他 root 使用，因此 loader 仍对整个 DAG
执行结构和类型验证。

---

## 15. Distance

```text
count:u32
DistanceDescriptor[count]
```

Record：

```text
lineId:u64
velocityDescriptor:u32
distanceDescriptor:u32
domainStart:f64
domainEnd:f64
integrationOrigin:f64
initialFloorPosition:f64
maxVelocityError:f64
maxDistanceError:f64
classification:u8   1 exact, 2 baked, 3 extension-runtime
reserved[7]=0
```

Core runtime query distanceDescriptor，不允许按 frame 累加 velocity。Loader 必须验证 descriptor
引用、domain、classification 和 error 均合法、有限且 error 非负；它不需要在安全加载路径中
重新证明 compiler 的数值误差声明。独立 conformance validator 必须按 BakedCurve validation
profile 重算 velocity/distance 误差。Strict-runtime 只能包含已通过该验证的 descriptor。Seek
直接求值 distance。

---

## 16. Render、Extensions 和辅助 section

### 16.1 Render

Render payload 由 `fcs-render.md` 的 RenderSection 1.0 定义。HAS_RENDER 置位时该 section 必须
存在且 REQUIRED；否则不得存在。

### 16.2 Extensions

```text
count:u32
ExtensionRecord[count]

ExtensionRecord:
namespace:StringRef
major:u16 minor:u16 patch:u16
flags:u16              bit0 required, bit1 preserve
mediaType:StringRef
payload:Bytes
```

按 namespace/version 排序。相同 namespace/version 不得重复。Required extension 不受支持时
拒绝执行；optional 可以跳过并按 preserve flag 保留。

### 16.3 Fidelity

Fidelity 是 Value object，schema 由 `fcs-conversion.md` 定义。Runtime 可以不加载，但 editable
和 archive writer 必须原样或语义等价保留已知记录。它不得覆盖 Lines/Notes/Tracks 的 Core 值。

### 16.4 ConversionReport

编码 `fcs-conversion.md` 定义的 report Value object。多个历史 report 使用 Value array，按发生
顺序保存。Runtime 不依赖 report。

### 16.5 SourceSnapshot

```text
encoding:u16 = 1 UTF-8
normalization:u16 = 0 raw
sourceHash[32]
source:Bytes
```

只允许 archive profile。Source bytes 必须有效 UTF-8，hash 按 bytes 原样计算并匹配。

### 16.6 Debug

Debug 可以包含 source map、symbol 和 compiler trace。它是 optional、可剥离、无执行语义。
2.0.0 使用 Value object；未知 key 保留。Deterministic build 在相同 debug mode 下也必须稳定，
不得写 wall-clock timestamp、绝对本机路径或随机 UUID。

---

## 17. Loader 验证顺序

Loader 必须先验证结构再分配由文件控制的大内存：

1. 最小长度、magic、headerSize、fileLength；
2. FCBC/ABI major、containerProfile、numeric model、reserved/feature flags；
3. sectionCount 和实现公开上限；
4. section table bounds、排序、alignment、overlap 和 file coverage；
5. 每个 checksum；
6. required/optional section 集合与 version；
7. StringTable UTF-8、offset 和唯一性；
8. record length、count 乘法和所有 index bounds；
9. stable ID 唯一性、引用和 parent DAG；
10. tempo/Note/Track/descriptor/expression type invariants；
11. extension feature availability；
12. container/document profile 和数值误差要求。

任何失败都必须停止加载，不得返回部分可执行 chart。Loader limit 至少覆盖 file size、section
count、record count、string bytes、expression nodes、descriptor segments、custom depth 和 Render
nodes。超限是资源错误，不是“损坏”推断。

---

## 18. Deterministic writer

Writer 固定：

- canonicalize `-0.0`；
- 拒绝非有限 float；
- StringTable 和 ConstantPool 按第 7/8 章排序去重；
- entity record 使用规范排序键；
- section entry 按 type/offset；
- padding 全零且最小；
- custom object 保持语义顺序，standard object 使用规范 field order；
- source hash 使用第 3 章 normalization；
- 不写 timestamp、本机路径、线程 ID 或随机数据；
- CRC 在最终 payload 上计算。

相同 canonical chart、source version、FCBC/ABI version、container profile、feature set 和
debug mode 必须产生完全相同 bytes。Compiler version 不同可以改变合法优化结果，因此 deterministic 比较的
输入条件必须包含 compiler mode；但任何结果都必须执行语义等价。

---

## 19. 禁止的运行时能力

FCBC Core 必须不包含：

- source token、comment、template body 或 generator range；
- mutable local、store、jump、通用 branch 或 loop；
- function/template recursive call；
- runtime entity/node creation；
- wall clock、filesystem、network、system font fallback；
- 未固定随机数；
- 依赖此前 frame history 的 floor distance；
- 未声明误差的 baked curve。

需要上述能力的实验 extension 必须使用独立 namespace、required feature 和 ABI version，且
不能标记 strict-runtime/Core portable。

---

## 20. Golden conformance

FCBC conformance suite 至少包含：

1. 空 fragment、空 chart 和最小 playable；
2. 每种 primitive/value/record 边界；
3. 多 tempo 同 beat step；
4. parent DAG、每种 Note 和 Hold；
5. constant/segment/piecewise/expression/baked descriptor；
6. distance seek test vector；
7. metadata/credit/resource/sync 完整保留；
8. optional/required unknown section；
9. checksum、offset、overlap、count overflow、bad UTF-8 和 dangling reference；
10. deterministic byte golden；
11. source→canonical→FCBC→reference evaluator 闭环；
12. runtime/editable/archive/strict-runtime profile 差异。

每个 golden 必须同时提供文件 hash、section manifest、成功/失败预期和失败 diagnostic category。

### 20.1 Stable loader diagnostic category

```text
fcbc.bad-magic
fcbc.unsupported-container-version
fcbc.unsupported-abi-version
fcbc.unsupported-profile
fcbc.invalid-header
fcbc.file-length-mismatch
fcbc.section-table-bounds
fcbc.section-order
fcbc.section-overlap
fcbc.section-alignment
fcbc.section-checksum
fcbc.missing-required-section
fcbc.unknown-required-section
fcbc.invalid-string-table
fcbc.invalid-record
fcbc.limit-exceeded
fcbc.duplicate-id
fcbc.dangling-reference
fcbc.parent-cycle
fcbc.invalid-tempo
fcbc.invalid-note
fcbc.invalid-track
fcbc.invalid-expression
fcbc.unsupported-required-extension
fcbc.profile-requirement-missing
```

Loader 可以返回更细 subcategory，但必须保留上述最接近的 stable parent category。结构验证顺序
按第 17 章，因此一个同时损坏多处的文件只要求报告最先遇到的 category。
