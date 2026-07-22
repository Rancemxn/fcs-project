# FCBC Container Format 2.0.0 and Execution ABI 1.0.0

状态：Draft（2026-07-22；native FCBC/Execution ABI closure remains open；见 `docs/specifications/governance.md`）

本文定义 FCBC 2.0.0 二进制容器和 FCS Execution ABI 1.0.0。规范治理见
`docs/specifications/governance.md`，source/canonical 语义见 `fcs.md`。

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
- section payload 默认 8-byte 对齐，声明更高 alignment 时必须满足声明；
- 本文列出的结构字段按出现顺序紧密编码，不插入宿主 ABI padding；只有本文明确写出的
  reserved/padding/alignment byte 才存在，Array element 之间也不另加隐式 padding。

本文所有 `ULP(x)` 对有限 binary64 x 统一定义为相邻有限 representable value 的较大间距：同时
存在 finite nextDown/nextUp 时取 `max(|x-nextDown|,|nextUp-x|)`；只有一侧相邻值有限时使用该有限
侧（覆盖 `±MAX_FINITE`）；零与 subnormal 使用最小正 subnormal 间距。该定义在 2 的幂边界和
正负值上唯一且取较保守的一侧。

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
  extension；不得依赖 optional extension、ABI 1.0 保留拒绝的 distance classification 3 或未通过
  数值验证的数据；
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

`alignmentLog2` 在 0–20；offset 必须是 `2^alignmentLog2` 的倍数。FCBC 2.0 已定义的 sectionType
1–20 固定 `alignmentLog2=3`，因此 reader 也必须拒绝这些 known section 使用 0–2 或 4–20；这与
第 1 章的 8-byte Core layout 是同一约束。未来版本定义的新 section type 只有其规范明文给出其他
值时才可使用 0–20 中的其他 alignment。
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
Section version major 未知按 REQUIRED 规则处理；Render type 14 是已知 owning profile，其不受支持的
section/profile version 使用 `render.unsupported-profile`。未来 minor 只有在本 section 自描述长度允许
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
Record := byteLength:u32, recordVersion:u16, recordFlags:u16, payload[byteLength-8]
```

Record `byteLength` 包含 8-byte prefix，至少为 8，且是 4 的倍数。本文明确称为
`ContributorRecord`、`CreditRecord`、`ResourceRecord`、`LineRecord`、`NoteRecord`、
`DistanceDescriptor`、`ExtensionRecord`、`PropertyDescriptor` 或 singleton `Record` 的结构，都使用
该 prefix；随后列出的字段是 `payload`，不是另一个嵌套 prefix。只称为 `TempoPoint`、`Segment`、
`Piece` 或 `ExpressionNode` 的结构是对应章节给出的固定宽度裸结构，不使用 Record prefix。

FCBC 2.0/ABI 1.0 的所有已定义 Record 固定 `recordVersion=1`、`recordFlags=0`。未知
`recordVersion` 或非零 `recordFlags` 使用 `fcbc.invalid-record` 拒绝。Reader 必须按 `byteLength`
逐个前进。默认可以跳过 `recordVersion=1` record 末尾位于 record boundary 内的未知扩展 bytes；
但 owning section specification 可以为了 no-source-snapshot、安全或 canonical 唯一性把某个 section
minor/Record 标为 exact-length。此时该版本的 reader 必须拒绝未知 tail，只有显式支持定义该 tail
的未来 section minor 才能接受。RenderSection 1.0 的全部 Record 使用此 exact-length 规则。Writer
2.0.0 不生成未登记扩展尾部，任何为 4-byte 对齐加入的尾部 byte 都写零。Record 的内部 `flags` 字段
（例如 Line/Note/descriptor flags）属于各自 payload，不是 `recordFlags`。

一个 known section 的 count/Record/Value/固定宽度裸结构以及该 section 明确定义的内部 padding
必须恰好消费 section payload；最后一个元素结束后不允许 section-level trailing bytes。只有位于
某个合法 Record `byteLength` 内、在该 Record 的 ABI 1.0 count-derived known payload 后面的 bytes
才是可跳过 record tail。`Segment`、`Piece`、`ExpressionNode` 等裸结构没有 element tail；singleton
Record 的 boundary 必须恰好等于 section end。声称保留未知数据的 rewriter 必须逐 byte 保留接受的
record tail，否则不得声称 lossless preservation。

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
fcs.render.layer
fcs.render.node
fcs.render.geometry
fcs.render.path
fcs.render.paint
fcs.render.stroke
fcs.render.clip
fcs.render.glyph-run
```

`id` 是 canonical textual ID，不是 workspace path、StringTable index 或 display label。显式 ID 和
compiler 生成 ID 的 textual namespace 分离规则由 `fcs.md` 第 17 章决定；进入本哈希前必须已经
形成最终 canonical textual ID；Render auxiliary 的 exact derivation 由 `fcs-render.md` 第 14 章固定。
FCBC reader 不从 u64 反推 source ID。

### 6.3 Value type

| Tag | Type | Payload |
|---:|---|---|
| 0 | null | none |
| 1 | bool | u8 value（0=false，1=true）+ 7 zero bytes |
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
空 array 仍保存 schema 声明的 elementTag。Bool 首 byte 只能是 0 或 1；其他值即使为非零也不能
解释为 true，必须使用 `fcbc.invalid-record` 拒绝。

Fixed-size tag 的 `payloadLength` 必须精确为：null=0；bool/int/float/string/time/length/angle/
resourceRef/contributorRef=8；beat=16；color=32。String payload 是 u32 StringRef 后跟 4 个 zero byte。
Color 四个 component 必须有限且位于 `[0,1]`。Vec2 `elementType` 使用本表 Value tag，只允许
int(2)、float(3)、time(5)、beat(6)、length(7)、angle(8)；两个 component 紧随 8-byte element
header，使用相应 Value 的裸 payload而不再嵌套 Value header，因此非 beat vec2 payloadLength=24、
beat vec2=40。所有 scalar/vector float component 必须有限，所有 beat denominator 必须为正。
Standalone Value 为 8-byte 对齐加入的 padding 必须为零；未知 tag、错误 payloadLength、非零
flags/reserved/padding 或非法 elementType 使用 `fcbc.invalid-record`。

### 6.4 Property type

Execution ABI property type 使用：

```text
1 bool, 2 int, 3 float, 4 time, 5 beat, 6 length,
7 angle, 8 color, 9 vec2-float, 10 vec2-length
```

String、array、object 和 entity reference 不能作为 runtime varying property。
`PropertyDescriptor.propertyType` 只能使用 1–10。`ExpressionNode.resultType` 还允许以下只用于
DAG 中间值的 ABI value type；它们不得成为 PropertyDescriptor root type：

```text
11 vec2-int, 12 vec2-time, 13 vec2-beat, 14 vec2-angle
```

这些 node-only type 使 Core `vec2<T>` 构造、分量访问和中间运算能够精确 lowering，而不把不存在的
runtime-varying schema property 加入 FCBC。后文统称 1–14 为 ABI value type，统称 9–14 为
vector type。

Constant descriptor 引用的 ConstantPool Value 必须按下表精确匹配；不得仅凭 payload 宽度进行
隐式转换：

| Property type | ConstantPool Value |
|---|---|
| bool | bool |
| int | int |
| float | float |
| time | time |
| beat | beat |
| length | length |
| angle | angle |
| color | color |
| vec2-float | vec2 且 elementType=float |
| vec2-length | vec2 且 elementType=length |

Expression Constant node 还可以引用 node-only vector type；其 ConstantPool Value 的 elementType
必须分别为 int、time、beat 或 angle。Expression descriptor 的 root 仍必须回到外层
PropertyDescriptor 的 1–10 type。

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

当 HAS_RENDER 置位且资源被 Core Render owner引用时，image/texture/font metadata 必须使用
`fcs-render.md` 第 10/11 章的 exact object key、Value tag、canonical order 和已物化 default；Render
section 不保存第二份 metadata。Audio/path/shader/binary 或 required extension 的 metadata 由其 owning
规范定义。Unknown generic metadata 不能改变 Core Render decode。

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
单调性和相邻映射的一致性。对不同 beat 的相邻 point，先从前一 point 的 exact beat、存储
chartTime 与生效 BPM 按 Core 公式计算并正确舍入 `referenceChartTime`；存储值允许的绝对误差为
`2 * ULP(referenceChartTime)`，ULP 使用第 1 章定义，不使用待验证 stored value 的 ULP。同 beat
step 的 chartTime 必须 bitwise 相同，不应用该 tolerance。

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

Line 字段的引用类型固定如下：

| Field | Referenced table/type |
|---|---|
| positionDescriptor | Tracks / vec2-length |
| rotationDescriptor | Tracks / angle |
| scaleDescriptor | Tracks / vec2-float |
| alphaDescriptor | Tracks / float |
| transformOriginConstant | ConstantPool / vec2-length |
| textureAnchorConstant | ConstantPool / vec2-float |
| scrollTempoTrack | Tracks / float |
| scrollSpeedTrack | Tracks / float |
| distanceDescriptor | Distance；目标 record 的 lineId 必须等于本 Line id |

`scrollTempoTrack` 的每个可达值必须有限且大于零；`scrollSpeedTrack` 的负值仍受
`ALLOW_REVERSE_SCROLL` 约束。`distanceDescriptor` 不是 Tracks index。

`inheritFlags` bit4 只控制 query-time scroll composition。每个 Line 的 `scrollTempoTrack`、
`scrollSpeedTrack` 和 `distanceDescriptor` 都描述该 Line-local q、velocity 和 floor；bit4=0 时
`lineScrollDistance` 只返回该 local Distance，bit4=1 且存在 parent 时按第 15 章把 parent 的
effective Distance 加入。该 bit 不合并 q、Track environment、integration origin、initial floor
position、floorScale、Line transform 或 Note scrollFactor，也不授权 writer 增加另一份 absolute-floor
descriptor。

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

Note 的 runtime property 引用类型固定如下：

| Field | Tracks property type |
|---|---|
| positionXDescriptor | length |
| scrollFactorDescriptor | float |
| xOffsetDescriptor | length |
| yOffsetDescriptor | length |
| alphaDescriptor | float |
| scaleXDescriptor | float |
| scaleYDescriptor | float |
| rotationDescriptor | angle |
| colorDescriptor | color |
| visibilityDescriptor | bool |

这些 index 都必须存在且类型精确匹配；静态 resource 字段不是 descriptor，不能用 Expression 动态
替代。FCS source 的 `visibleFrom`/`visibleUntil` 在 canonical lowering 时变为
`visibilityDescriptor` 的 Constant 或 Piecewise bool：它只依赖 chartTime `s`，在半开可见区间内为
true，区间外为 false；source 没有可直接赋值的 `presentation.visibility` 对象。任意其他 visibility
动态语义必须通过已版本化 required extension 声明，不能由 FCBC writer 猜测。

---

## 13. Tracks 和 PropertyDescriptor

Tracks section：

```text
descriptorCount:u32
PropertyDescriptor[descriptorCount]
```

每个 descriptor 是 `PropertyDescriptor` Record。`Record` prefix 后的 ABI 1.0 known payload 为：

```text
propertyType:u8
descriptorKind:u8
flags:u16
domainStart:f64
domainEnd:f64
payload...
```

`flags` bit0=`UNBOUNDED_BEFORE`、bit1=`UNBOUNDED_AFTER`，其他 bit 必须为零；它不是通用
`recordFlags`。未置对应 unbounded bit 的 endpoint 必须是有限 binary64，置位时对应
`domainStart`/`domainEnd` 必须写 `+0.0`。把 unbounded 端分别解释为负/正无穷后必须满足
`domainStart <= domainEnd`。Domain 使用有限 binary64 chartTime `s`，声明 descriptor 可以被查询的
闭区间；对 domain 外的直接 query 必须返回 structured execution error，不得隐式 clamp、hold 或
extrapolate。

LineRecord 的 position/rotation/scale/alpha/scrollTempo/scrollSpeed root、NoteRecord 的全部 runtime
property root，以及 Distance 使用的 Line scroll root 都必须同时置 `UNBOUNDED_BEFORE` 和
`UNBOUNDED_AFTER`，从而对每个有限 chartTime 都有唯一值。FCBC 2 没有另一个隐式 chart/play
interval，Note time、最后一个 note 或媒体长度都不得缩小该 required domain。Piecewise 引用的
内部 descriptor 可以使用较小 domain，但引用 interval 必须被覆盖。Strict-runtime validator 对
root 的结构/domain 检查因此不依赖文件外的“conformance query domain”声明。Strict conformance
不要求在 load-time 枚举并证明所有可能 environment 都返回 value；对规范 domain error/overflow
必须返回同一 structured execution error，对 conformance vector 中拥有有限 reference result 的
query 则必须满足逐 bit 或本 ABI 明列的误差界。

DescriptorKind：

| Value | Kind | Payload |
|---:|---|---|
| 1 | Constant | constantPoolIndex:u32 |
| 2 | SegmentTrack | segmentCount:u32 + Segment[segmentCount] |
| 3 | Piecewise | pieceCount:u32 + Piece[pieceCount] |
| 4 | Expression | expressionRoot:u32 |
| 5 | reserved | FCBC 2.0.0 所有 profile 必须拒绝 |

Kind payload 必须精确按下列方式编码；count 的乘法和 payload end 必须在 Record boundary 内：

| Kind | ABI 1.0 known payload after common fields | Writer `byteLength` |
|---|---|---:|
| Constant | `constantPoolIndex:u32` | 32 |
| SegmentTrack | `segmentCount:u32, Segment[segmentCount]` | `32 + 64*segmentCount` |
| Piecewise | `pieceCount:u32, Piece[pieceCount]` | `32 + 24*pieceCount` |
| Expression | `expressionRoot:u32` | 32 |

`byteLength` 包含 8-byte Record prefix。Writer 不在 kind payload 后加入另一层 `Bytes`、Record
prefix 或私有 lookup table。Reader 可以按第 6.1 节跳过未来同 record version 的尾部扩展，但 ABI
1.0 evaluator 不得把未知尾部解释为 sampling、fallback 或另一种 descriptor。

Constant index 必须存在且 Value 与 `propertyType` 按第 6.4 节精确匹配。Segment 的两个 constant、
Piece 引用的 descriptor，以及 Expression root 的 result type 也必须精确匹配外层
`propertyType`。Piecewise descriptor 依赖图必须无环。Kind 5 只有在 Record `byteLength>=28`、完整 common payload 已位于
boundary 内且 propertyType/flags/domain 均可合法解析后，才在读取任何假定 kind payload 前按
第 13.3 节拒绝；更短/truncated common payload 先使用 `fcbc.invalid-record`。

SegmentTrack/Piecewise 的 count 必须大于零。Constant 和 Expression 可以使用任意合法
bounded/unbounded domain；SegmentTrack/Piecewise 的 unbounded 端按下两节的显式 first/last element
规则编码，不能由 evaluator 猜 source fill/extrapolation。

Tracks 不允许为 conformance vector、debug 或未来用途保留 unowned descriptor。以 Lines、Notes 和
RenderSection 中每个规范 descriptor field 作为 direct root；Distance 的 speed 是对应 Line root 的
bitwise alias，不另造 root。每个 descriptor 都必须从至少一个 direct root 经 Piecewise edge 可达，
否则使用 `fcbc.invalid-track` 拒绝。

Core Line/Note direct root 的 exact ASCII target path、owner、property type、domain 和 environment
matrix 固定如下；表中 spelling 是 deterministic traversal input，不是示例或 implementation label：

| Canonical target path | Owner stable ID | Property type | Required root domain | Environment |
|---|---|---:|---|---|
| `line.alpha` | owning Line ID | float | 双向 unbounded | `s,b,q`；禁止 `d` |
| `line.position` | owning Line ID | vec2-length | 双向 unbounded | `s,b,q`；禁止 `d` |
| `line.rotation` | owning Line ID | angle | 双向 unbounded | `s,b,q`；禁止 `d` |
| `line.scale` | owning Line ID | vec2-float | 双向 unbounded | `s,b,q`；禁止 `d` |
| `line.scrollSpeed` | owning Line ID | float | 双向 unbounded | `s,b,q`；禁止 `d` |
| `line.scrollTempo` | owning Line ID | float | 双向 unbounded | `s,b`；禁止 `q,d` |
| `note.presentation.positionX` | owning Note ID | length | 双向 unbounded | `s,b,q,d` |
| `note.presentation.scrollFactor` | owning Note ID | float | 双向 unbounded | `s,b,q`；禁止 `d` |
| `note.presentation.xOffset` | owning Note ID | length | 双向 unbounded | `s,b,q,d` |
| `note.presentation.yOffset` | owning Note ID | length | 双向 unbounded | `s,b,q,d` |
| `note.presentation.alpha` | owning Note ID | float | 双向 unbounded | `s,b,q,d` |
| `note.presentation.scaleX` | owning Note ID | float | 双向 unbounded | `s,b,q,d` |
| `note.presentation.scaleY` | owning Note ID | float | 双向 unbounded | `s,b,q,d` |
| `note.presentation.rotation` | owning Note ID | angle | 双向 unbounded | `s,b,q,d` |
| `note.presentation.color` | owning Note ID | color | 双向 unbounded | `s,b,q,d` |
| `note.presentation.visibility` | owning Note ID | bool | 双向 unbounded | `s` |

`p` 只在第 13.2 节 Piece context 中由选中的 inner descriptor 使用，不是 direct-root 调用方额外
注入的环境。
`line.scrollTempo` 的 `q/d` 禁止是因果约束：`q` 由 scrollTempo 积分产生，`d` 又依赖 scroll
distance；任何到 EnvQ/EnvD 的 direct 或 transitive path 都使用 `fcbc.invalid-expression` 拒绝。

共享 descriptor 的环境是所有引用它的 direct root 的交集；例如同时被 Line 与 Note引用的 root 即使
Note允许 `d`，也不能包含 EnvD。完整 direct-root 集合包含 Core、Render 和已加载 required extension
登记的全部 root；loader 不得在 Render graph/ownership 尚未验证时把一个只由 Render 引用的 descriptor
误判为 unowned，也不得只按第一个访问者验证 environment。若 intrinsic ABI graph/type 本身非法，或
任一 Core owner 的 schema 不接受该 dependency，使用对应 `fcbc.*` category；Core 全部接受、但
Render owner/attachment 进一步收窄环境时使用 `render.invalid-descriptor`。若两类 owner 同时拒绝，
Core/ABI validation 的 `fcbc.*` failure 先发生。除本表 16 项、`fcs-render.md` 第 14.8 节登记的 Render
root 和已版本化 required extension 登记的 root 外，FCBC 2/ABI 1 writer/loader 不得发明其他 target
path。`p` 不出现在 direct-root environment matrix；它只由第 13.2 节 Piece context 临时绑定。

Deterministic writer 在分配 index 前执行以下 canonicalization：

1. 为 descriptor 计算与 table index 无关的 `StructuralKey`：按顺序编码 propertyType、descriptor
   flags、domainStart raw binary64 bits、domainEnd raw bits 和 kind。Constant 加入被引 Value 的
   canonical bytes。SegmentTrack 对每项只编码 start/end raw bits、interpolation、easing、flags、
   startValue/endValue canonical bytes 和四个 Bezier raw bits，**不编码** startConstant/endConstant
   u32。Piecewise 对每项只编码 start/end raw bits、flags 和 child StructuralKey，**不编码**
   descriptorIndex。Expression 加入第 14 章 root node key。所有 variable bytes 前置 u32 byteLength，
   array 前置 u32 count，因此拼接无歧义；
2. StructuralKey 完全相同的 descriptor 全局 intern 为一个对象；SHA/hash 可以加速比较，但碰撞时
   必须比较完整 length-prefixed key bytes；
3. 把 direct root reference 按 `(UTF-8 canonical target path bytes, owner stable u64 ID,
   root StructuralKey bytes)` 排序。依次从每个 root 深度优先访问 descriptor；Piecewise child 按 Piece
   array 顺序访问，child 全部完成后才访问当前 descriptor。已访问的 interned descriptor 跳过，在
   postorder 返回时分配下一个 index；
4. 本节上述 reachability 要求保证遍历结束后没有剩余 descriptor。跨 owner sharing 总在
   lexicographically first root 首次访问，Piecewise child/root 的次序也由 postorder 唯一确定。

Canonical target path 使用 Core/Render schema 的完整路径（例如 `line.position`、
`note.presentation.alpha`），array/table field 必须包含 zero-based ordinal（例如
`render.geometry.radiiDescriptors[2]`），不是 source spelling、局部 Track ID 或 workspace path。
Render 1.0 的完整 exact path、owner type、property type、domain 和 environment matrix 位于
`fcs-render.md` 14.8；writer/loader 不得从 Geometry object key临时发明另一种 path spelling。
StructuralKey 是最终 tie-break；相同 key 已 intern，因此 direct-root key 是全序。该规则同时
消除跨 owner sharing、Piecewise root/child 和 descriptor interning 时的排序歧义。Environment
availability 仍使用所有 direct/transitive owner root 的去重集合，不因“首次访问者”而缩小。

### 13.1 Segment

`Segment` 是固定 64-byte 裸结构，不使用 Record prefix：

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

所有时间与 Bezier 参数必须有限。`flags` bit0=`POINT`，其他 bit 必须为零。普通 segment 固定
`flags=0`、`start < end`，在 `[start,end)` 生效；point 固定 `start=end`、`interpolation=1`、
`easing=0`、四个 Bezier 参数为 `+0.0`，且 start/end ConstantPool Value 必须 bitwise 相同。Point
从其时刻开始保持该值，直到同一 Track 的下一 point 或普通 segment 开始；同一时刻普通 segment
优先，且其 start value 必须与 point 值 bitwise 相同。Segment 按 `(start, POINT-before-segment)`
排序，普通 segment 不重叠，point 不得位于普通 segment 内。Canonical SegmentTrack 必须在其
PropertyDescriptor domain 的每个查询时刻产生唯一值；source fill、extrapolation、blend 和
priority 必须在写 FCBC 前已经精确 lowering，ABI 不从缺口猜这些 authoring policy。

若外层置 `UNBOUNDED_BEFORE`，排序后的第一个 element 必须是 point；该 point 值还覆盖所有早于
其时间的有限 chartTime。若外层置 `UNBOUNDED_AFTER`，最后一个 element 必须是 point；该 point 值
覆盖所有晚于其时间的有限 chartTime。未置对应 flag 时，第一个/最后一个 element 必须精确覆盖
bounded domain endpoint。这样一个在起止处各有 point 的有限 Track 可以把 before/after fill 明确
物化为全时域函数，而不在 bytes 中保存 authoring policy 名称。

Interpolation：1 step、2 linear、3 easing、4 cubicBezier。Step 在普通 segment 内返回
`startConstant`；其他 interpolation 按 `fcs.md` 第 9.4 节从 start value 到 end value 求值。
Step 允许全部 property type；linear/easing/cubicBezier 只允许 float、time、beat、length、angle、
color、vec2-float 和 vec2-length，并对 color/vector 逐分量使用同一 scalar progress。非 Bezier
参数必须为 `+0.0`；cubicBezier 的 x1/x2 必须在 `[0,1]` 且 x 曲线可单值反解。

Easing ID：0 linear；1–3 Sine in/out/inOut；4–6 Quad；7–9 Cubic；10–12 Quart；13–15 Quint；
16–18 Expo；19–21 Circ；22–24 Back；25–27 Elastic；28–30 Bounce。每组三项固定为 in、out、
inOut，公式见 `fcs.md`。Interpolation=3 时 easing 必须是 1–30；其他 interpolation 的 easing
写 0。普通 segment 的 start/end constant 必须与 PropertyDescriptor type 精确匹配。

### 13.2 Piecewise

`Piece` 是固定 24-byte 裸结构，不使用 Record prefix：

```text
start:f64
end:f64
descriptorIndex:u32
flags:u32
```

`flags` bit0=`END_INCLUSIVE`、bit1=`UNBOUNDED_BEFORE`、bit2=`UNBOUNDED_AFTER`，其他 bit 必须为零。
Bit1 只允许第一个 Piece 且必须与外层同名 flag 一致；其 `start` 固定写 `+0.0` 并解释为负无穷。
Bit2 只允许最后一个 Piece 且必须与外层同名 flag 一致；其 `end` 固定写 `+0.0` 并解释为正无穷。
其他 start/end 必须有限；把这两个特殊端点替换为无穷后，每个 Piece 必须 `start < end`。Bit0 只
允许 bounded 最后一项；unbounded-after 已包含全部有限 endpoint，不得再置 bit0。

Piece 按解释后的 start 严格递增，通常表示 `[start,end)`，置 bit0 的最后一项表示 `[start,end]`；
它们不得重叠，并且必须无缺口覆盖外层 PropertyDescriptor 的整个 domain。一个同时置 bit1/bit2、
start/end 都写 `+0.0` 的单 Piece 因而表示整个有限 chartTime 域。引用 descriptor 的 type 必须与
外层一致，其 domain 必须覆盖该 Piece interval。Piece 选择 inner descriptor 时，把
`p=(chartTime-start)/(end-start)` 作为该次 inner query 的 EnvP；双向有限 Piece 使用该公式，只有
unbounded-before 的 Piece 在其无界侧固定 `p=0.0`，只有 unbounded-after 的 Piece 在其无界侧固定
`p=1.0`，双向无界的 Piece 固定 `p=0.0`。末尾置 bit0 的 bounded end endpoint 精确为 `1.0`。
嵌套 Piece 以内层绑定覆盖外层 EnvP；Piece 之外的 direct-root query 没有 EnvP。依赖图必须无环。

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

Node 按拓扑顺序，operand index 必须小于当前 node index。`ExpressionNode` 是固定 20-byte 裸结构，
不使用 Record prefix：

```text
opcode:u16
resultType:u8
arity:u8
operandA:u32
operandB:u32
operandC:u32
immediate:u32
```

未使用 operand 固定写 `0xFFFFFFFF`。除 Constant、Easing 外 `immediate` 必须为 0。ABI 1.0
opcode、arity 和类型固定如下；表中的 `T` 要求所有出现位置是完全相同的 ABI value type：

| Opcode | Arity / operands | Result and immediate |
|---|---|---|
| Constant | 0；A/B/C unused | resultType 精确匹配 `ConstantPool[immediate]` |
| EnvS / EnvB / EnvQ / EnvD / EnvP | 0；A/B/C unused | 固定分别为 time / beat / float / length / float；immediate=0 |
| Neg | 1；A:T，T∈int/float/time/beat/length/angle | T |
| Not | 1；A:bool | bool |
| Add / Sub | 2；A:T、B:T，T∈int/float/time/beat/length/angle 或任一 vector type | T |
| Mod | 2；A:int、B:int | int |
| Pow | 2；两侧同为 int 或同为 float | 同 operand type |
| Eq / Ne | 2；A:T、B:T，T 为任一 ABI value type | bool |
| Lt / Le / Gt / Ge | 2；A:T、B:T，T∈int/float/time/beat/length/angle | bool |
| And / Or | 2；A:bool、B:bool | bool |
| ApproxEq | 3；A:float、B:float、C:float tolerance | bool |
| Abs | 1；A:T，T∈int/float/time/beat/length/angle | T |
| Min / Max | 2；A:T、B:T，T∈int/float/time/beat/length/angle | T |
| Clamp | 3；A:T、B:T、C:T，分别为 value/lo/hi | T；T∈int/float/time/beat/length/angle |
| Floor / Ceil / Round / Sqrt / Exp / Ln / Sin / Cos / Tan / Asin / Acos / Atan | 1；A:float | float |
| Atan2 | 2；A:float、B:float | float |
| Easing | 1；A:float | float；immediate 是 0–30 easing ID |
| ToFloat | 1；A:int | float |
| Seconds | 1；A:time | float |
| Radians | 1；A:angle | float |
| Choose | 3；A:bool、B:T、C:T | T |
| Vec2 | 2；A:T、B:T，T∈int/float/time/beat/length/angle | 对应 vec2-T |
| Vec2X / Vec2Y | 1；A:任一 vector type | 对应 element type |

Mul/Div 因 result type 不总与两侧相同，使用以下唯一组合；`U` 是 time/beat/length/angle：

| Opcode | Operand types | Result |
|---|---|---|
| Mul | int,int | int |
| Mul | float,float | float |
| Mul | U,int 或 int,U | U |
| Mul | U,float 或 float,U | U |
| Mul | vec2-int,int 或 int,vec2-int | vec2-int |
| Mul | vec2-float,float 或 float,vec2-float | vec2-float |
| Mul | vec2-U,int/float 或 int/float,vec2-U | vec2-U |
| Div | int,int | int |
| Div | float,float | float |
| Div | U,int/float | U |
| Div | U,U（同一 U） | float |
| Div | vec2-int,int | vec2-int |
| Div | vec2-float,float | vec2-float |
| Div | vec2-U,int/float | vec2-U |

除上述组合外全部使用 `fcbc.invalid-expression` 拒绝。Int 运算使用 checked i64；除法向零截断，
除零、负 int exponent、overflow 和 `i64::MIN / -1` 是 execution error。Int scalar 参与 U/vector-U
运算时先按 roundTiesToEven 转为 binary64，再逐 node 舍入；float/U/vector 运算、比较和函数遵守
`fcs.md` 第 4、13、14 章，vector/color equality 逐分量，`+0.0 == -0.0` 为 true。

`ToFloat` 将 i64 正确舍入为 binary64；`Seconds`/`Radians` 保持 payload binary64 bits，只改变
静态 type。`ApproxEq` 先要求 tolerance 有限且非负，再按两个输入的 binary64 subtraction、Abs、
Le 三个逐步 roundTiesToEven 操作的结果定义；任何中间非有限值产生 execution error。Vec2 按 A、
B 顺序求值并组合两个同类型 component，Vec2X/Vec2Y 返回相应 component。Easing ID 0 是 linear，
1–30 与 Segment 表一致。

ABI 1.0 的 numeric opcode number 为：

```text
1 Constant       immediate=ConstantPool index
2 EnvS           3 EnvB           4 EnvQ           5 EnvD           6 EnvP
10 Neg           11 Not
20 Add           21 Sub           22 Mul            23 Div           24 Mod
25 Pow
30 Eq            31 Ne            32 Lt             33 Le            34 Gt
35 Ge            36 And           37 Or             38 ApproxEq
40 Abs           41 Min           42 Max            43 Clamp
44 Floor         45 Ceil          46 Round          47 Sqrt
48 Exp           49 Ln            50 Sin            51 Cos           52 Tan
53 Asin          54 Acos          55 Atan           56 Atan2
60 Easing        immediate=easing ID
61 ToFloat       62 Seconds        63 Radians
70 Choose        operandA=predicate, B=true value, C=false value/next Choose
80 Vec2          81 Vec2X         82 Vec2Y
```

Expression node 的 `StructuralKey` 与最终 index 无关：按 opcode、resultType、arity 以及 A、B、C
顺序递归嵌入 used operand key；Constant node 嵌入被引 Value canonical bytes而不编码 immediate
index，Easing 嵌入 easing ID，其他 opcode 的 zero immediate 不另产生可变 key。Unused slot 不进入
递归但仍在 bytes 中写 null。每段 variable bytes 都 length-prefix。完全相同 key 的纯 node全局
intern。Writer 先按第 13 章得到最终 descriptor 顺序，再依该顺序访问每个 Expression
root，对 operand A→B→C 作 postorder depth-first traversal；已 emitted 的 interned node 跳过，在返回
当前 node 时分配下一个 index。这样每个 operand index 必然更小，shared subgraph 只出现一次，且
不存在任意 topological tie-break。

Expressions section 中每个 node 都必须从至少一个最终可达的 Expression PropertyDescriptor root
transitively reachable；unreachable node 或 unowned Expression descriptor 使用
`fcbc.invalid-expression`/`fcbc.invalid-track` 分别拒绝。该 reachability 只能在 Core、Render 和已加载
required extension 的完整 direct-root 集合建立后最终判定。一个 Expression descriptor 的
environment availability 取全部 direct/transitive owner root 对应 target schema 的交集；该集合不会
为空。

Expression graph 不含 jump、store、call、loop、recursion、random、IO、emit 或 allocation。
Choose chain 必须有限且最终 else 可以是任何同类型 ordinary value node；嵌套 Choose 只是 operandC
引用另一个较小 index node，不引入控制流 index。Loader 必须重新 type-check 每个 node、精确 arity、
unused operand、immediate 和整个 DAG；这些不依赖 owner 的 intrinsic failure 使用
`fcbc.invalid-expression`。Unknown opcode 使用 `fcbc.invalid-expression`。每个 PropertyDescriptor root
的 resultType 必须与其 propertyType 相同；node-only vector type 只能出现在 root 之前。

Owner-dependent environment availability 在完整 direct-root ownership 建立后验证。Env dependency
必须满足所有引用该 descriptor 的 target schema；共享 descriptor 使用这些 schema 允许环境的交集。
Core owner failure 使用 `fcbc.invalid-expression`；仅由 Render owner/attachment 收窄产生的 failure 使用
`render.invalid-descriptor`，详见 `fcs-render.md` 第 14.8、16 章。Runtime 对 invalid math 返回
structured execution error。Strict-runtime load-time 验证结构、type、root domain 和 portable
capability；它不尝试穷举所有 environment 证明永不发生 domain error，有限 reference query 的
逐 bit/误差验证由 conformance vector 执行。EnvP node 的每一条到最终 PropertyDescriptor root 的
依赖路径都必须跨过一个 Piecewise descriptor；若存在没有 Piece context 的 direct-root path，loader
使用 `fcbc.invalid-expression` 拒绝。SegmentTrack 的内置 interpolation 可以使用规范 `p` 计算，
但它不向没有 Piece context 的 Expression DAG 暴露 EnvP。

Evaluator 可以递归/按需读取拓扑表，不能假定所有较小 index 都必须先求值。除 lazy opcode 外，
operand 按 A、B、C 顺序求值。And、Or 和 Choose 遵守 `fcs.md` 的 lazy semantics：And 在 A=false
时不求值 B，Or 在 A=true 时不求值 B，Choose 只求值 predicate 和被选 result。未选 node 可以被
其他 root 使用，因此 loader 仍对整个 DAG 执行结构和类型验证。

---

## 15. Distance

```text
count:u32
DistanceDescriptor[count]
```

DistanceDescriptor 按 `lineId` u64 升序排列；LineRecord 的 table index 必须指向该排序后的项。
相同 lineId 重复使用 `fcbc.invalid-distance` 拒绝。

每项是 `DistanceDescriptor` Record。ABI 1.0 known payload 为：

```text
lineId:u64
scrollSpeedDescriptor:u32
reservedAnalyticDescriptor:u32 = null
domainStart:f64
domainEnd:f64
integrationOrigin:f64
initialFloorPosition:f64
maxVelocityError:f64
maxDistanceError:f64
boundaryCount:u32
classification:u8   1 portable-analytic, 2 portable-evaluable, 3 reserved (legacy runtime-only-extension)
flags:u8            bit0 unbounded-before, bit1 unbounded-after
reserved:u16=0
boundaryTimes:f64[boundaryCount]
```

`byteLength` 包含 Record prefix，Writer 固定写 `80 + 8*boundaryCount`。Known payload 在最后一个
boundaryTime 后结束，不含 sample count、floorPosition table、interpolation 或 frame-state seed；
Reader 跳过的未来 Record 尾部不得被 ABI 1.0 evaluator 解释为这些数据。`flags` bit0/bit1 分别是
unbounded-before/after，其他 bit 必须为零。每个被 LineRecord 引用的 Distance 必须同时置这两个
bit，`domainStart=domainEnd=+0.0`，因而 floor 对所有有限 chartTime 都有确定的 value 或 structured
invalid-value error；FCBC 2 不从 note/audio 时长推断更小区间。其余所有 f64 field 和 boundary
必须有限。

LineRecord 的 `distanceDescriptor` 是本表 index。每个 Distance `lineId` 唯一且恰好被同 ID 的
LineRecord 引用一次；`scrollSpeedDescriptor` 必须 bitwise 等于该 LineRecord 的
`scrollSpeedTrack`，而 `reservedAnalyticDescriptor` 在 ABI 1.0 固定为 null。LineRecord 的
`scrollTempoTrack` 与这个 scrollSpeed descriptor 都必须是 unbounded exact float descriptor。
这两个 Line-owned descriptor 是该 Line-local scroll 的唯一规范真相；Distance 不保存第二份
integrand DAG、继承后 absolute-floor function 或可与它们矛盾的 parent cache。

在积分变量 u 处，ABI 依次求值 `scrollSpeedDescriptor(u)` 与 Line `scrollTempoTrack(u)`，再执行
两个独立 binary64 node 等价操作：`speed * scrollBpm`，随后 `/ 60.0`。所得 float 是该点 integrand。
Scroll BPM 必须有限且大于零；负 speed 仍受 Line `ALLOW_REVERSE_SCROLL` 约束。Boundary 按
binary64 total order 严格递增，固定为以下 finite value 的 totalOrder 去重并集：integrationOrigin；
两个 root 的 reachable SegmentTrack point/start/end 与 Piecewise bounded start/end；以及它们依赖的
global TempoMap/EnvB point time。scrollTempo 禁止依赖 EnvQ/EnvD，因此不存在由 q/d 反向产生的
scrollTempo boundary。除此之外不得加入“有助于某积分器”的任意点或统一采样格；Expression 内的
连续 predicate threshold 不因 compiler 猜测而成为
离散 boundary，数值积分器必须自行建立误差 enclosure。相同 canonical descriptor graph 因而产生
唯一 boundary list。

LineRecord 中重复保存的 `integrationOrigin`/`initialFloorPosition` 必须与 Distance record bitwise
相同。Writer 包含全部已知有限 boundary，不得额外写统一采样格。

ABI 1.0 的 `portable-analytic` 只允许 scrollTempoTrack 和 scrollSpeedDescriptor 都是 Constant kind。
Loader 因而能只凭结构验证 classification；runtime 先按上一段得到 constant binary64 integrand c，
再对精确实数 `initialFloorPosition + c * (queryTime-integrationOrigin)` 正确舍入一次为 binary64。
`portable-evaluable` 用于其他 exact kind，由 Execution ABI 从 integrationOrigin 到 query time 做直接、
与帧历史无关的确定性积分，再在高精度域加 initialFloorPosition。两类都不读取
reservedAnalyticDescriptor，不是 BakedCurve，也不保存预采样 floorPosition。

若 queryTime 与 integrationOrigin 按 IEEE equality 相等（包括 `+0.0/-0.0`），两类都必须直接返回
`initialFloorPosition` 原始 bits。其他 query 的高精度 absolute floor 恰好为实数零时固定舍入为
`+0.0`；不得因积分方向、临时 accumulator 或宿主加法保留任意 zero sign。

上述结果是一个 Line-local Distance。`lineScrollCoordinate(lineId,t)` 始终只返回该 Line 的 local
q。`lineScrollDistance(lineId,t)` 先沿 Line parent chain 建立从 root 到目标 Line 的有限 ancestry，
再按每个 child 的 `inheritFlags` bit4 决定是否继续包含 parent；bit4=0 在该 child 处终止 ancestry。
查询不得访问不在这条 ancestry 上的 root、sibling 或其他 Line。

对包含的每个 Line，runtime 使用其自己的 q、scroll tempo、speed、integration origin、initial floor
position 和 boundary 求 local reference floor。Effective reference floor 是这些 local reference floor
在高精度域中的实数和，最终只舍入一次为 binary64。若 ancestry 只有目标 Line，则直接使用上段的
local result 和 signed-zero 规则；若加入任一 parent contribution，实数和恰好为零时结果固定为
`+0.0`。Effective velocity 则按 root→target 顺序，把每个 Line 按本章定义的 binary64 local
integrand 用 binary64 roundTiesToEven 逐项相加；任何非有限 local 值或中间和返回 structured
execution error。

每个 Line 的 `ALLOW_REVERSE_SCROLL` 只验证该 Line-local `scrollSpeedTrack`。一个显式允许 reverse
的祖先可以使 descendant effective velocity 为负，不要求 descendant 重复置位；header
`USES_REVERSE_SCROLL` 仍是所有 Line local flags 的逻辑或。Parent/local descriptor 的 domain、积分或
数值错误只沿实际 ancestry 传播；无关 Line 的错误不得使目标 query 失败。Direct seek 必须独立查询
每个 local Distance 并组合，不得依赖此前 frame、顺序播放历史或隐藏 sampled parent cache。

Classification value 3 保留旧候选的 `runtime-only-extension` 名称，但 ABI 1.0 record 没有足以唯一
绑定 extension namespace/version/payload 的字段；因此 FCBC 2.0/ABI 1.0 的所有 profile 都必须以
`fcbc.invalid-distance` 拒绝 value 3。未来 ABI 只有在定义明确 extension binding 后才能启用该编号。
其他未知 classification 同样拒绝，不能按 portable 类猜测。

`maxVelocityError` 对两种 portable exact 分类必须为 0，因为 integrand 的逐 node binary64 求值由
Expressions ABI 直接定义。`portable-analytic` 的 `maxDistanceError` 必须为 0；
`portable-evaluable` 固定写 ABI 1.0 的 Core absolute floor error `0x1p-32`，runtime 的结果相对于
高精度 reference floor 的允许误差为 `max(0x1p-32, 4 ULP(referenceResult))`。该常量是 ABI
执行约束，不是谱师/packager sampling 参数，也不能因目标设备性能而放宽。积分必须在 boundary
处分段，并保持 floorPosition 连续；无法在实现公开的 evaluation/depth budget 内满足误差时返回
structured execution error，不能退化成 frame accumulation 或写回采样曲线。Strict-runtime
conformance 对每个 vector 声明的有限 query 验证该 bound；文件本身不携带第二个隐藏 validation
domain。普通 bounded loader 只验证字段、引用、分类与有限性，不在加载安全路径中重做高精度积分。
Seek 总是直接使用 constant analytic formula 或从 integrationOrigin 积分，不依赖此前 frame。

Inherited effective-floor query 的 Core absolute integration error 是被包含 ancestry 上
`maxDistanceError` 的高精度和；runtime 必须在同一个高精度 accumulator 中组合各 local enclosure，
并使最终 binary64 结果相对于 effective reference floor 的误差不超过
`max(sum(maxDistanceError), 4 ULP(referenceResult))`。Portable-analytic ancestry 的和仍是
bit-exact round-once 查询；任一 portable-evaluable member 都使该 effective query 使用派生的
portable-evaluable bound。这个派生 bound 不写入第二个 Distance record，也不能被播放器性能配置放宽。

这里的 local reference floor 是 `fcs.md` 第 10.2 节连续积分公式：对每个实数积分变量 `u`，先把
EnvS/EnvB/EnvQ 输入按 binary64 roundTiesToEven 形成该点的 ABI environment，再按第 14 章逐 node
语义求 integrand；对所得有界、分段可积函数取实数定积分，在高精度域中与
`initialFloorPosition` 相加，再把这一绝对 floor 结果舍入到 binary64。不得先把 integral 单独舍入
后再把该两步舍入结果当成 reference。Boundary 两侧按各 descriptor 的半开/endpoint 规则取值，
单点值不改变积分。出现非有限
值、不可积 singularity 或不能建立有限 error enclosure 的 domain 使用 `fcbc.invalid-distance`；
reference validator 应使用足够精度的区间/多精度方法建立误差包络，但其具体算法不进入 FCBC
bytes，也不能成为另一套 chart semantics。

Inherited effective reference floor 再按本节 ancestry 规则组合这些 local reference floor；它不改变
任何 local integrand、boundary list、classification 或 Distance bytes。

本节的 `ULP(referenceResult)` 使用第 1 章的全局定义。

---

## 16. Render、Extensions 和辅助 section

### 16.1 Render

Render payload 由 `fcs-render.md` 的 RenderSection 1.0 定义。HAS_RENDER 置位时该 section 必须
存在且 REQUIRED；否则不得存在。RenderSection 只保存 stable resource ID，不保存 workspace path、
URI、hash/offset 副本或 resource bytes；所有 image/font/texture/path/shader/binary 引用必须在同一
FCBC 的 Resources 中存在，并在 ResourceData 验证完成后按 kind/capability 绑定。Renderer 不得
建立第二套外部 lookup 或让 Render payload 覆盖 ResourceRecord metadata/hash。

RenderSection 1.0 是 exact-length singleton Record，包含 viewport 与八张 typed table；所有 nested
Record、stable ID namespace、canonical order、ownership/reachability、descriptor direct root、image/
font decode 和 stable `render.*` precedence 均以 `fcs-render.md` 第 14–16 章为准。FCBC structural、
section CRC 和 ResourceData layout/hash 失败仍先返回本规范的 `fcbc.*` category。

Render direct root 的 descriptor index/type/domain/environment 或 evaluator 本身失败使用
`render.invalid-descriptor`；descriptor 成功返回 typed finite value 后，owner field 的值域失败分别使用
`render.invalid-geometry`、`render.invalid-paint`、`render.invalid-stroke` 或
`render.invalid-composite`。Clip 的静态 fillRule/geometry 一致性使用 `render.invalid-clip`。FCBC loader
不得把这些 owner-specific semantic failure 折叠为 `fcbc.invalid-record`，也不得让更深 Render
category 越过本规范先发生的 framing/checksum/hash failure。Intrinsic Expression opcode/arity/type/DAG
或 Core owner environment failure 仍先使用 `fcbc.invalid-expression`；只有这些检查通过后，由 Render
owner/attachment environment intersection 产生的失败才是 `render.invalid-descriptor`。Render
note/line attachment 在 query-time
读取的 Core Line、Distance 或 Note descriptor 若发生 execution/domain/非有限错误，也统一使用
`render.invalid-descriptor`；只有 FCBC structural loader 已经报告的 malformed expression/distance
才使用对应 `fcbc.*` category。该传递性 query 按 `fcs-render.md` 第 16 章的 dependency order 执行。

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
   required section，最后检查 feature↔section 对应；已知 Render type 14 的不受支持 version按
   `fcs-render.md` 返回 `render.unsupported-profile`；
7. StringTable UTF-8、offset 和唯一性；
8. record length、count 乘法、byte range 和通用 FCBC index bounds；Render payload 中 profile-owned
   u32/u64 reference 只验证其编码宽度与 Record boundary，不在本步解引用或决定 Render category；
9. Resources↔ResourceData offset/length、完整覆盖、零 padding、逐 resource SHA-256；同一步先检查
   bounds/layout/coverage，再计算 hash；
10. 非 Render stable ID 唯一性、Core resource/entity 引用和 parent DAG；来自 RenderSection 的
    resource/entity/table reference 延迟到第 12 步；
11. tempo/Note/Track/descriptor/expression/distance intrinsic invariants 与 Core direct-root
    type/domain/environment；本步不以不完整 root 集合执行最终 unowned/shared-environment 判定；
12. Render singleton/Record/table/ID/order/reachability、所有 Render-owned reference、resource
    kind/media/capability binding与 Render direct-root type/domain/environment；随后以 Core+Render+
    required-extension 完整 root 集合完成 descriptor/expression reachability、canonical order和共享
    environment intersection。Render 内部 precedence 由 `fcs-render.md` 第 16 章固定；
13. extension feature availability；
14. container/document profile、exact-only 和 strict 数值要求。

任何失败都必须停止加载，不得返回部分可执行 chart 或未验证 resource slice。Loader limit 至少
覆盖 file size、section count、record count、string bytes、单 resource bytes、resource 总 bytes、
expression nodes、descriptor segments、distance boundaries、custom depth 和 Render nodes。媒体
codec 的 decoded dimensions/sample count/glyph/path/shader limits 由消费层在 decode/compile 前另行
检查；ResourceData loader 不为 hash validation 解码媒体。超限是资源错误，不是“损坏”推断。

ABI mutation 的 stable parent category 固定如下；更细 subcategory 不改变该 parent：

| Failure surface | Stable parent |
|---|---|
| Record byteLength/version/recordFlags、count-derived truncation、known section 未恰好消费 | `fcbc.invalid-record` |
| 通用 entity/table/ConstantPool index 越界或不存在（Expression operand 与 Constant-node immediate 除外） | `fcbc.dangling-reference` |
| PropertyDescriptor flags/domain/kind payload、Segment/Piece ordering/coverage/type/interpolation | `fcbc.invalid-track` |
| Descriptor kind 5，在 byteLength≥28 且完整 common payload/type/flags/domain 合法后 | `fcbc.forbidden-descriptor` |
| Expression opcode/arity/operand/ConstantPool immediate/result type/DAG/env dependency | `fcbc.invalid-expression` |
| Intrinsic Expression/Core owner validation 通过，但 Render owner/attachment environment 不接受 dependency | `render.invalid-descriptor` |
| Distance.lineId 不存在，或其通用 table/entity reference 不存在 | `fcbc.dangling-reference` |
| Distance lineId 存在但反向 distanceDescriptor/唯一绑定/重复 bits 不一致，或 descriptor type/domain/classification/error field/boundary 无效 | `fcbc.invalid-distance` |
| Line 自身 property reference type 或 parent/scroll invariant | `fcbc.invalid-track` |
| Note 自身 property/resource reference type 或 gameplay/presentation invariant | `fcbc.invalid-note` |

验证仍按上面的全局顺序进行：例如 mutation 若没有同步修正 section CRC，必须先返回
`fcbc.section-checksum`，不能越过 checksum 去报告更深的 kind/expression/distance category。

---

## 18. Deterministic writer

Writer 固定：

- 保留 `+0.0/-0.0` 的 IEEE 754 bits；
- 拒绝非有限 float；
- StringTable 和 ConstantPool 按第 7/8 章排序去重；
- entity record 使用规范排序键；
- Render auxiliary ID、Layer/Node root partition、typed table、owner/reachability 和 descriptor root
  使用 `fcs-render.md` 第 14 章 canonical order；
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
14. HAS_RENDER、viewport、八张 Render table、image/WebP/font resource、semantic draw-list、reference
    raster、exact descriptor root、no-source-text GlyphRun，以及 Geometry/Paint/Stroke/Clip/composite 与
    descriptor-evaluation 分层的 `render.*` mutation precedence。

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
