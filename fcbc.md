# FCBC Container Format 2.0.0 and Execution ABI 1.0.0

状态：Draft（2026-07-15；联合候选自检已完成，等待非空 ABI/Render byte vector 与独立复审）

本文定义 FCBC 2.0.0 二进制容器和 FCS Execution ABI 1.0.0。规范治理见
`docs/specification-governance.md`，source/canonical 语义见 `fcs.md`。

FCBC 是经过验证、完成编译期展开、恰好包含一个谱面及其全部资源的确定性执行包。它是正式
播放器必须支持的最小自包含输入，不是 source AST dump、外部资源清单或允许任意程序执行的通用
VM 格式。一个空 fragment 仍是一个合法的空谱面；FCBC 2 不提供零谱面、多谱面、曲包或 catalog
索引。

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

FCBC 2.0 不定义容器级压缩、加密、混淆、DRM 或签名。资源以输入文件的原始 bytes 内嵌；资源
本身已有的 PNG/JPEG/WebP/OGG/MP3 等格式内压缩保持不变。传输系统可以在 FCBC 之外压缩整个
byte sequence，但解包后的 FCBC 不得依赖另一份 archive、workspace、URL 或外部文件才能加载；
FCBC header/section 也没有 compression/encryption flag。

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
- source FCS major 不受 runtime 支持时必须拒绝；未来 minor 只有在 runtime 支持其 canonical Core
  capability 时可以接受，不能把未知 Core 语义猜成当前版本；
- source FCS version 不表示 source snapshot 存在，具体 descriptor opcode/layout 仍只由 ABI version
  决定。

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

`sourceHash` 对去除可选 BOM、将 CRLF 规范为 LF、除此之外不修改的 UTF-8 FCS source bytes
计算。它只是可选的输入 content hash，不授权或要求 FCBC 保存 source。由 PGR/RPE/PEC 等外部
格式直接生成 CanonicalCompilation、或不能声明唯一 FCS source 时，写 32 个零并清除
`SOURCE_HASH_PRESENT`；外部输入 hash 可以进入 DistributionMetadata/ConversionReport。

`sourceFcsVersion` 是该文件采用的 FCS canonical semantic version。即使 CanonicalCompilation
来自外部格式 converter，而没有中间 FCS 文本，也必须写 converter 所针对的 FCS 版本；runtime
不得把它解释为“容器中存在一份该版本 source”。

`compilerIdString` 和 `compilerVersionString` 是 StringTable index；无值时为 `0xFFFFFFFF`。
Deterministic writer 固定 `sectionTableOffset=128`；未来 minor 只有在 headerSize 增长时才可以移动
section table。Reader 必须使用字段值而不是假定 128，并验证 table 不与 header 重叠。

### 3.1 Container Profile

| Value | Name | 含义 |
|---:|---|---|
| 0 | runtime | Core 执行所需 canonical 数据 |
| 1 | fidelity | runtime + 结构化 Fidelity/ConversionReport/DistributionMetadata；仍不是 authoring backup |
| 2 | reserved | FCBC 2.0.0 必须拒绝 |
| 3 | strict-runtime | runtime 且全部值 portable、误差已验证、无 optional runtime extension |

未知或 reserved containerProfile 必须拒绝。每个已定义 profile 都必须自包含恰好一个 chart 和全部
resource payload，并且不得包含 source snapshot、source AST 或 BakedCurve。FCS document profile
不使用该字段，保存在 Meta section。

Profile/feature 约束：

- runtime 可以携带与对应 feature bit 一致的结构化 Fidelity、ConversionReport、
  DistributionMetadata 或 Debug；这些 section 不改变 execution；
- fidelity 必须置 HAS_FIDELITY 并包含 Fidelity，可按事实同时包含 ConversionReport 和
  DistributionMetadata；“fidelity”只表示结构化转换保真信息，不表示可恢复 authoring source；
- strict-runtime 只能使用 Core exact descriptor 或 runtime 已支持且声明 strict-portable 的 required
  extension；不得依赖 optional extension、runtime-only-extension distance 或未通过数值验证的数据；
- containerProfile 不得改变 Note、Track、Expression、Render 或 resource bytes 的 canonical 语义。

### 3.2 Feature flags

| Bit | Name |
|---:|---|
| 0 | SOURCE_HASH_PRESENT |
| 1 | HAS_RENDER |
| 2 | HAS_EXTENSIONS |
| 3 | HAS_FIDELITY |
| 4 | HAS_CONVERSION_REPORT |
| 5 | HAS_DISTRIBUTION_METADATA |
| 6 | HAS_DEBUG |
| 7 | reserved，必须为 0 |
| 8 | USES_REVERSE_SCROLL |

Bits 9–63 在 2.0.0 中保留。写入者必须置零；loader 看到未知置位 bit 必须拒绝，除非未来
FCBC minor 明确将其定义为 safely ignorable。

`SOURCE_HASH_PRESENT` 清除时 `sourceHash` 必须为 32 个零；置位时该字段按第 3 章解释为
SHA-256（理论上的全零 digest 仍由 bit 区分，loader 不凭字节值猜 presence）。
`USES_REVERSE_SCROLL` 必须恰好等于所有 LineRecord `ALLOW_REVERSE_SCROLL` flag 的逻辑或；它是
loader/capability 的快速声明，不替代逐 Line flag，也不自行授权任何负 scrollSpeed。

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
| 1 | PRESERVE | 声称保留未知 optional section 的 FCBC rewriter 必须逐 byte 保留其 payload |

其他 bit 必须为零。未知 optional section 可以跳过；未知 required section 必须拒绝。

FCBC 2.0 定义的 sectionType 全部是 singleton；Extensions/Debug 的多个逻辑记录位于各自 singleton
payload 内，不通过重复 section 表达。未知 future optional type 只有其版本规范明确允许时才能重复。
Section entry 必须按 `(sectionType, offset)` 升序排列；payload 必须按 entry 顺序布局，以最少零
padding 满足 alignment。文件末尾不得有未被 section 或 padding 解释的 bytes。

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
| 16 | Fidelity | feature-required when HAS_FIDELITY；fidelity profile 必须置位并包含 |
| 17 | ConversionReport | feature-required when HAS_CONVERSION_REPORT |
| 18 | DistributionMetadata | feature-required when HAS_DISTRIBUTION_METADATA |
| 19 | Debug | feature-required when HAS_DEBUG |
| 20 | ResourceData | required |

“Required” section 即使 record count 为零也必须存在；因此一个没有 resource declaration 的空 chart
仍包含 length=0 的 ResourceData section。每个 Core section version 为 `1.0.0`。
Section version major 未知按 REQUIRED 规则处理；未来 minor 只有在本 section 自描述长度允许
跳过新增 record/field 时可以接受。

表中的 `feature-required when HAS_*` 是双向约束：对应 bit 置位时 singleton section 必须存在并置
section flag REQUIRED；bit 清除时该 section 不得存在。Fidelity container profile 还必须置
HAS_FIDELITY；其他 profile 只按该双向 feature/presence 约束决定是否携带这些辅助 section。

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

Core namespace byte string 固定为：

```text
fcs.contributor
fcs.credit
fcs.resource
fcs.line
fcs.note
```

`id` 是 canonical textual ID，不是 workspace path、StringTable index 或 display label。显式 ID 和
compiler 生成 ID 的 textual namespace 分离规则由 `fcs.md` 第 17 章决定；进入本哈希前必须已经
形成最终 canonical textual ID。FCBC reader 不从 u64 反推 source ID。

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

`offsets[0]=0`，offset 非递减且最后等于 byte 区长度。每个 slice 必须是有效 UTF-8。U+0000
允许，因为 Core string 可以由 source `\0` escape 合法产生，slice boundary 已由 offsets 明确；
runtime 不得把 StringTable slice 当作 NUL-terminated C string。相同 byte string 只出现一次。
Deterministic writer 按 UTF-8 byte lexicographic order 排序；所有 StringRef 是此排序后的 index。

---

## 8. ConstantPool

Payload 为 `Array<Value>`。只允许 scalar、color、vec2、resourceRef 和 contributorRef；array、
object 留在 metadata section。相同 type 和 bitwise payload 的常量只出现一次，按
`(typeTag,payloadBytes)` lexicographic 排序。`+0.0` 与 `-0.0` 的 payload bits 不同，必须分别
保留和去重；NaN/Inf 禁止。

---

## 9. Meta、Contributors、Credits、Resources、Sync

### 9.1 Meta

Meta payload：

```text
documentProfile:u8
reserved[3]=0
documentFeatureBits:u32
meta:Value(object)
artwork:Value(object)
```

DocumentProfile ID：1 fragment、2 chart、3 playable、4 renderable、5 publishable。Feature bits：
bit0 playable、bit1 renderable；其他 bit 必须为零。Profile/features 必须满足 `fcs.md` 的组合规则。
`meta` object 的标准 key 按以下顺序编码，缺失则省略：

```text
title, subtitle, alternativeTitles, chartVersion, difficulty, level,
description, language, tags, license, documentId, revision, custom
```

`artwork` 使用 `fcs.md` 第 7.3 节的 canonical object；没有 artwork declaration 时编码空 object。
类型必须符合 `fcs.md`。所有 container profile 都必须保留完整 canonical Meta 与 Artwork。

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
flags:u16=0
mediaType:StringRef
hashAlgorithm:u16
reserved:u16=0
dataOffset:u64
dataLength:u64
hash:Bytes
metadata:Value(object)
```

Resource kind ID：1 audio、2 image、3 font、4 texture、5 path、6 shader、7 binary。Record 按
id 升序。SHA-256 algorithm ID=1，hash 长度必须 32。`dataOffset` 相对于 ResourceData section
payload 第一个 byte，`dataLength` 是 authoring workspace 输入普通文件的原始 byte length。
ResourceRecord 不编码 `resource.source`、workspace logical/absolute path、URI、package member 或
外部 archive 引用；`flags` 在 2.0.0 中必须为零。

`metadata` 保存 kind-specific canonical contract，例如 image color space、alpha 和 sampling；它不
授权 packager 解码或验证媒体内容。相同 content hash 的两个 resource ID 仍是两个语义 identity，
不得合并 ResourceRecord 或让一个 ID 替代另一个 ID。

### 9.5 ResourceData

ResourceData sectionType=20，payload 是 Resources 中全部 record 的 opaque 原始 bytes 与最小零
padding，不含 count、record header、codec wrapper 或压缩层。布局由 Resources 的 ID 顺序唯一
决定：

```text
cursor = 0
for resource in Resources ordered by id:
    expectedOffset = alignUp(cursor, 8)
    zeroPad(cursor, expectedOffset)
    resource.dataOffset = expectedOffset
    append resource original bytes exactly
    cursor = expectedOffset + resource.dataLength
section.length = cursor
```

每个 `dataOffset` 必须等于上述 `expectedOffset`，每个 padding byte 必须为零，section 末尾不得有
trailing padding 或未被 ResourceRecord 解释的 byte。非空 resource range 因而不重叠、不别名；
standard writer 即使发现相同 bytes/hash 也不得做 payload deduplication。零长度 resource 可以与
下一个零长度 resource 具有相同 offset，但仍各自保留 record 与空 payload 的 SHA-256。

ResourceData section entry 使用 alignmentLog2=3。Section CRC 覆盖包括内部 padding 在内的整个
payload；每个 ResourceRecord 的 SHA-256 只覆盖它自己的 `[dataOffset,dataOffset+dataLength)` 原始
bytes，不覆盖 padding。Loader 必须检查加法 overflow、section bounds、规范布局、零 padding、
`dataLength` 与 SHA-256，再向 runtime/renderer 暴露 bounded immutable slice。Section checksum
通过不能替代逐资源 hash；media decode error 也不能反向改写 hash 或 resource identity。

Packager 不得解码、重编码、转码、降采样、颜色转换或规范化 payload。所有通过 canonical
validation 的 resource declaration 都必须有一个 ResourceRecord 和对应 ResourceData range，包括
暂未被当前 renderer 直接引用但由 metadata、artwork 或 extension 拥有的资源。没有 resource 时
Resources 编码 `count=0`，required ResourceData section 的 length 必须为 0、CRC 必须为
`0x00000000`。

### 9.6 Sync

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
scoreExtensionNamespace:StringRef or null
reserved:u32=0
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
或 `circle`：lineDefault 不允许额外 geometry；rectangle 保存 line-local `center` 与两个有限正
`halfExtents`；circle 保存 line-local `center` 与有限正 `radius`。SoundPolicy：1 default、2 none、
3 resource；只有 resource 允许且要求 soundResourceId 引用内嵌 audio，其他 policy 必须写 0。
ScorePolicy：1 default、2 none、3 custom；只有 custom 允许且要求非 null
scoreExtensionNamespace，该 namespace 必须匹配 Extensions 中的 required gameplay extension。

当 judgment enabled flag 未置位时，soundPolicy 和 scorePolicy 必须都规范化为 none，
soundResourceId 必须为 0，scoreExtensionNamespace 必须为 null；loader 不得通过 alpha、visibility、
texture 或 Render state 猜测 fake/judgment。Gameplay 数据不能引用 runtime expression。Hold 的 Core
sound intent 只发生在 head；其他 sound behavior 需要 required extension。`textureResourceId` 为 0
或静态引用一个内嵌 image/texture resource；任何 sound/texture resource 的 type、ID、payload hash
或 ResourceData range 不合法都必须在返回 chart 前拒绝。

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
| 5 | reserved | FCBC 2.0.0 所有 profile 必须拒绝 |

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

### 13.3 Reserved approximation descriptor

DescriptorKind 5 是旧候选中的 BakedCurve tag，当前 FCBC 2.0.0 保留该数值以避免被未来 exact
descriptor 复用，但任何 container profile、feature 或 optional section 都不能启用它。Loader
遇到 kind 5 必须使用 `fcbc.forbidden-descriptor` 拒绝文件；standard writer 不生成该 tag。

显式 target approximation 属于 `fcs-conversion.md` 的目标能力协商与 ConversionReport，不修改或
替代标准发行 FCBC。播放器本地 sampled cache 也不写入 FCBC，不能借 Debug、Fidelity、extension
或 unknown optional section 绕过本条。

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
integrandDescriptor:u32
analyticDistanceDescriptor:u32 or null
domainStart:f64
domainEnd:f64
integrationOrigin:f64
initialFloorPosition:f64
maxVelocityError:f64
maxDistanceError:f64
boundaryCount:u32
classification:u8   1 portable-analytic, 2 portable-evaluable, 3 runtime-only-extension
flags:u8            bit0 unbounded-before, bit1 unbounded-after
reserved:u16=0
boundaryTimes:f64[boundaryCount]
```

LineRecord 的 `distanceDescriptor` 是本表 index。`integrandDescriptor` 必须是 exact float descriptor，
表示 `scrollSpeed(s) * scrollBpm(s) / 60`。Boundary 按 binary64 total order 严格递增，包含所有
tempo/Track point、segment endpoint 和 compiler 静态知道的离散积分边界；不得量化到统一时间格。
有界一侧的 domain endpoint 必须有限，unbounded flag 置位的一侧 endpoint 写 0。

`portable-analytic` 必须提供 analyticDistanceDescriptor，runtime 直接查询该 exact descriptor；
`portable-evaluable` 必须把 analyticDistanceDescriptor 写 null，由 Execution ABI 对 exact integrand
从 integrationOrigin 到查询时间做直接、与帧历史无关的确定性积分。两类都不是 BakedCurve，也不
保存预采样 floorPosition。`runtime-only-extension` 需要已声明、已支持的 required ABI extension，
不能出现在 strict-runtime。

`maxVelocityError` 对两种 portable exact 分类必须为 0，因为 integrand 的逐 node binary64 求值由
Expressions ABI 直接定义。`portable-analytic` 的 `maxDistanceError` 必须为 0；
`portable-evaluable` 固定写 ABI 1.0 的 Core absolute floor error `0x1p-32`，runtime 的结果相对于
高精度 reference integral 的允许误差为 `max(0x1p-32, 4 ULP(referenceResult))`。该常量是 ABI
执行约束，不是谱师/packager sampling 参数，也不能因目标设备性能而放宽。积分必须在 boundary
处分段，并保持 floorPosition 连续；无法在实现公开的 evaluation/depth budget 内满足误差时返回
structured execution error，不能退化成 frame accumulation 或写回采样曲线。Strict-runtime
validator 必须在声明的 conformance query domain 验证该 bound；普通 bounded loader 只验证字段、
引用、分类与有限性，不在加载安全路径中重做高精度积分。Seek 总是直接求值 analytic descriptor
或从 integrationOrigin 积分，不依赖此前 frame。

这里的 reference integral 是 `fcs.md` 第 10.2 节连续积分公式：对每个实数积分变量 `u`，先把
EnvS/EnvB/EnvQ 输入按 binary64 roundTiesToEven 形成该点的 ABI environment，再按第 14 章逐 node
语义求 integrand；对所得有界、分段可积函数取实数定积分，最后把高精度 reference result 舍入到
binary64。Boundary 两侧按各 descriptor 的半开/endpoint 规则取值，单点值不改变积分。出现非有限
值、不可积 singularity 或不能建立有限 error enclosure 的 domain 使用 `fcbc.invalid-distance`；
reference validator 应使用足够精度的区间/多精度方法建立误差包络，但其具体算法不进入 FCBC
bytes，也不能成为另一套 chart semantics。

---

## 16. Render、Extensions 和辅助 section

### 16.1 Render

Render payload 由 `fcs-render.md` 的 RenderSection 1.0 定义。HAS_RENDER 置位时该 section 必须
存在且 REQUIRED；否则不得存在。RenderSection 只保存 stable resource ID，不保存 workspace path、
URI、hash/offset 副本或 resource bytes；所有 image/font/texture/path/shader/binary 引用必须在同一
FCBC 的 Resources 中存在，并在 ResourceData 验证完成后按 kind/capability 绑定。Renderer 不得
建立第二套外部 lookup 或让 Render payload 覆盖 ResourceRecord metadata/hash。

### 16.2 Extensions

```text
count:u32
ExtensionRecord[count]

ExtensionRecord:
namespace:StringRef
major:u16 minor:u16 patch:u16
flags:u16              bit0 required, bit1 preserve
payload:Value(object)
```

按 namespace/version 排序。相同 namespace/version 不得重复。Required extension 不受支持时
拒绝执行；optional 可以跳过执行，并由声称 preservation 的 rewriter 按 preserve flag 保留 typed
object。Namespace spelling、ordered object 和 required/optional 语义必须符合 `fcs.md` 第 15.1 节；
FCBC extension 不引入私有 source grammar、任意 executable bytes 或媒体类型分派。

### 16.3 Fidelity

Fidelity 是 Value object，schema 由 `fcs-conversion.md` 定义。Runtime 可以不加载；fidelity
container profile 必须存在该 section，其他 profile 只有 HAS_FIDELITY 置位时才允许存在。它只
保存结构化 source-state/semantic-fidelity fact，不得覆盖 Lines/Notes/Tracks 的 Core 值，也不得
包含 FCS/外部 source raw bytes、token stream、comment、template/generator/local、source AST 或可
恢复这些 authoring-only 数据的编码 snapshot。

### 16.4 ConversionReport

编码 `fcs-conversion.md` 定义的 report Value object。多个历史 report 使用 Value array，按发生
顺序保存。Runtime 不依赖 report。HAS_CONVERSION_REPORT 未置位时不得存在；置位时必须存在。

### 16.5 DistributionMetadata

```text
metadata:Value(object)
```

标准 key 顺序为 `provenance`、`repairRecords`、`inputHashes`、`custom`；缺失则省略。各数组只保存
结构化 typed object，例如 producer/tool ID 与版本、外部 source/profile ID、mapping rule ID、
hash algorithm/digest 和 `fcs.md` 第 15.3 节 repair record。该 section 不具有执行语义，不得覆盖
CanonicalChart 或 ResourceData。

DistributionMetadata、Fidelity、ConversionReport 和 Debug 合称 CanonicalCompilation 的可选
distribution metadata 编码。任何一项都不得包含 FCS/外部 source 文本或 raw snapshot、source
AST、comment/trivia、template/function/generator 声明或调用、局部变量、authoring expansion graph、
workspace absolute path，或仅为规避本限制而 base64/hex/压缩后的等价内容。Input content hash、
stable rule/profile ID 和非原文 provenance fact 允许。需要 byte-exact round-trip 的工具必须保留
原 authoring workspace。

### 16.6 Debug

Debug 是 optional、可剥离、无执行语义的 Value object，只能注释 canonical stable ID、section、
descriptor、runtime validation 或性能事实。它不得保存 source map、authoring symbol/compiler trace
或上一节禁止的数据。未知 key 保留。Deterministic build 在相同 debug mode 下也必须稳定，不得写
wall-clock timestamp、绝对本机路径、线程 ID 或随机 UUID。

---

## 17. Loader 验证顺序

Loader 必须先验证结构再分配由文件控制的大内存：

1. 最小长度、magic、headerSize、fileLength；
2. FCBC/ABI major、containerProfile、numeric model、reserved/feature flags；
3. sectionCount 和实现公开上限；
4. section table bounds、排序、alignment、overlap 和 file coverage；
5. 每个 checksum；
6. required/optional section 集合与 version；同一步先报告 unknown REQUIRED，再报告缺少的 known
   required section，最后检查 feature↔section 对应；
7. StringTable UTF-8、offset 和唯一性；
8. record length、count 乘法和所有 index bounds；
9. Resources↔ResourceData offset/length、完整覆盖、零 padding、逐 resource SHA-256；同一步先检查
   bounds/layout/coverage，再计算 hash；
10. stable ID 唯一性、resource/entity 引用和 parent DAG；
11. tempo/Note/Track/descriptor/expression/distance type invariants；
12. extension feature availability；
13. container/document profile、exact-only 和 strict 数值要求。

任何失败都必须停止加载，不得返回部分可执行 chart 或未验证 resource slice。Loader limit 至少
覆盖 file size、section count、record count、string bytes、单 resource bytes、resource 总 bytes、
expression nodes、descriptor segments、distance boundaries、custom depth 和 Render nodes。媒体
codec 的 decoded dimensions/sample count/glyph/path/shader limits 由消费层在 decode/compile 前另行
检查；ResourceData loader 不为 hash validation 解码媒体。超限是资源错误，不是“损坏”推断。

---

## 18. Deterministic writer

Writer 固定：

- 保留 `+0.0/-0.0` 的 IEEE 754 bits；
- 拒绝非有限 float；
- StringTable 和 ConstantPool 按第 7/8 章排序去重；
- entity record 使用规范排序键；
- ResourceRecord 按 ID，ResourceData 按第 9.5 节写原始 bytes 和唯一最小零 padding；
- section entry 按 type/offset；
- padding 全零且最小；
- custom object 保持语义顺序，standard object 使用规范 field order；
- source hash 使用第 3 章 normalization；
- 不写 BakedCurve、SourceSnapshot、workspace path、外部资源 locator 或 authoring-only 数据；
- 不写 timestamp、本机路径、线程 ID 或随机数据；
- CRC 在最终 payload 上计算。

相同 CanonicalChart、CanonicalResourceBundle 原始 bytes、规范性 DistributionMetadata、source
version、FCBC/ABI version、container profile、feature set 和 debug mode 必须产生完全相同 bytes。
宿主 workspace 的绝对位置、文件枚举顺序和线程调度不得影响输出。Compiler version 不同可以改变
合法 exact sharing/optimization，因此 deterministic 比较的输入条件必须包含 compiler mode；但
任何结果都必须执行语义等价，且资源 payload 必须 byte-identical。

---

## 19. 禁止的运行时能力

FCBC Core 必须不包含：

- source token、comment、template body 或 generator range；
- mutable local、store、jump、通用 branch 或 loop；
- function/template recursive call；
- runtime entity/node creation；
- BakedCurve、预采样 floorPosition 或播放器 sampled cache；
- source snapshot、source AST、authoring symbol/expansion graph；
- workspace/package member/URL/外部 archive resource lookup；
- wall clock、filesystem、network、system font fallback；
- 未固定随机数；
- 依赖此前 frame history 的 floor distance；
- ResourceData 中未引用、重叠、非最小或非零 padding data。

只有新的 runtime 计算能力可以通过独立 namespace、required feature 和新 ABI version 试验，且
不能标记 strict-runtime/Core portable。Source snapshot/AST、authoring graph、外部 resource lookup、
BakedCurve 和播放器 sampled cache 在 FCBC 2 的 extension/Fidelity/Debug 中仍然禁止，不能借
“实验 extension”绕过。

---

## 20. Golden conformance

FCBC conformance suite 至少包含：

1. 空 fragment、空 chart、最小 playable 和“一个文件恰好一个 chart”结构；
2. 每种 primitive/value/record 边界；
3. 多 tempo 同 beat step；
4. parent DAG、每种 Note 和 Hold；
5. constant/segment/piecewise/expression exact descriptor，以及 kind 5 必须拒绝；
6. analytic/evaluable distance direct-seek、boundary 和 error vector；
7. metadata/artwork/credit/resource/sync 完整保留；至少一个 golden 内嵌非空 opaque resource，并逐
   byte 比较 ResourceData、offset、length、CRC 与 SHA-256；
8. optional/required unknown section；
9. checksum、offset、overlap、count overflow、bad UTF-8、dangling reference、resource hidden/trailing
   bytes、resource hash mismatch 和缺少 required ResourceData；
10. deterministic byte golden；
11. source→canonical→FCBC→reference evaluator 闭环；
12. runtime/fidelity/strict-runtime profile 差异、reserved profile 2 rejection；
13. distribution metadata 中没有 source snapshot/AST/authoring-only 数据。

每个 golden 必须同时提供文件 hash、section manifest、成功/失败预期和失败 diagnostic category。

### 20.1 Stable loader diagnostic category

```text
fcbc.bad-magic
fcbc.unsupported-source-version
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
fcbc.invalid-resource-data
fcbc.resource-hash-mismatch
fcbc.duplicate-id
fcbc.dangling-reference
fcbc.parent-cycle
fcbc.invalid-tempo
fcbc.invalid-note
fcbc.invalid-track
fcbc.invalid-expression
fcbc.invalid-distance
fcbc.forbidden-descriptor
fcbc.unsupported-required-extension
fcbc.profile-requirement-missing
```

Loader 可以返回更细 subcategory，但必须保留上述最接近的 stable parent category。结构验证顺序
按第 17 章，因此一个同时损坏多处的文件只要求报告最先遇到的 category。
