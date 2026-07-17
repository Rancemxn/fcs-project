# FCS Conversion Specification 1.0.0

状态：Draft（2026-07-15；semantic profile closure 与联合候选自检已完成，等待真实 round-trip fixture 与独立复审）

本文定义 FCS 与 PGR v1/v3、RPE、PEC 及扩展外部格式之间的解析边界、版本化语义 Profile、
canonical 转换、能力协商、保真事实、Repair 和机器可读 `ConversionReport`。本文不改变
`fcs.md` 的 Core 执行语义，也不把任一社区播放器的当前实现提升为未版本化的格式真值。

---

## 1. 范围、权威关系与原则

转换器必须：

1. 先无损解析来源格式的结构，再按已解析的 source semantic profile 解释来源语义；
2. 把 parser dialect、format version、producer version、intended runtime、semantic profile、
   syntax mode、Repair mode 和 target capability 作为互相独立的维度；
3. 先得到 FCS `CanonicalChart`、`CanonicalResourceBundle` 和结构化 provenance，再由 exporter
   消费 canonical model；exporter 不得直接消费 FCS source AST 或外部 source parse tree；
4. 区分原始 source value、profile 解释后的 source semantic value、FCS canonical value 和
   target value；
5. 不把“成功解析”或“播放器可以打开”误报为无损/等价转换；
6. 不静默猜 profile、丢字段、选择 parent、填 speed gap、裁剪 overlap、交换非法区间、clamp
   数值或修改未知 easing；
7. 在目标能力不足时，按 exact direct、exact rewrite、显式 runtime extension、结构化 preserve、
   用户授权 approximation、用户授权 drop、unsupported 的顺序作决定；
8. 用 canonical execution semantics 比较 round-trip，不以 JSON/TOML/text 顺序或字节相等代替；
9. 保留输入 hash、profile/rule/tool identity、选择证据、歧义影响、Repair 和 approximation 事实；
10. 让用户修改后的 canonical 语义优先于外部 source workspace 中的陈旧回写信息。

`docs/community/` 是外部格式 evidence baseline，`refer/chart/` 是固定快照证据；二者不覆盖本文。
证据冲突必须在不同 profile 中显式存在，不能通过修改 FCS Core 或在 runtime 加隐式兼容开关解决。

---

## 2. 固定转换阶段

Importer 的阶段顺序固定为：

```text
source bytes / source package
→ SourceArtifactSet
→ lossless source-format parse
→ ParsedSourceDocument
→ syntax/dialect validation
→ DetectionEvidence[]
→ source ProfileBinding selection
→ SourceSemanticDocument
→ FCS canonical lowering
→ CanonicalChart + CanonicalResourceBundle + ConversionProvenance
```

Exporter 的阶段顺序固定为：

```text
CanonicalChart + CanonicalResourceBundle
→ target ProfileBinding + CapabilitySet
→ complete capability negotiation
→ exact target IR, or explicitly authorized approximation/drop decision
→ target bytes / target package
→ reparse with the same target ProfileBinding
→ canonical semantic comparison
→ ConversionReport + output hash
```

每个箭头都是可失败边界。Parser 不得在构造 `ParsedSourceDocument` 时按 FCS 字段含义解释
`bpmfactor`、PGR Line BPM、PEC command time 或 offset bias；profile selection 也不得修改非法来源。
Repair 发生在原始 diagnostic 已建立之后，并产生新的、可审计的 repaired source semantic input。

`ParsedSourceDocument` 和完整 `ConversionProvenance` 可以存在于 converter/制谱器的活动 workspace，
但不是 `CanonicalChart`、标准 FCBC 或发行 metadata 的组成部分。

---

## 3. Parser、Profile 与 Repair 的类型边界

### 3.1 Parser dialect 与 syntax mode

```text
ParserDialectRef {
    id: stable dotted identifier
    version: exact SemVer
    contentHash: sha256-lower-hex string
}

syntaxMode: strict | compatible

profileSelectionMode: strict | compatible
```

Parser dialect 只定义 token/JSON shape、字段别名、换行边界、unknown field retention 和遗留语法
接受范围。它不得定义 Note 物理时刻、坐标单位、speed/distance、parent transform 或 runtime 行为。
Conversion 1.0 内建 dialect 位于 `docs/conformance/conversion/parser-dialects.toml`；其
`contentHash=SHA-256(UTF-8(contract))`，不包含 TOML 引号或换行。

- `strict` syntax mode 只接受 dialect 明确列出的规范形状；
- `compatible` syntax mode 可以接受已登记的别名、number/string version、missing/null/sparse 形状，
  但必须保存实际输入形态并报告 compatibility rule；
- syntax mode 不得修复非法值，也不得选择两个合法但语义不同的 profile。

`profileSelectionMode` 只控制第 4.4 节的歧义处理；它与 parser `syntaxMode` 独立。Strict
`conversionPolicy` 要求 `profileSelectionMode=strict`，但 semantic/roundtrip policy 可以由调用方
选择 strict 或 compatible profile selection。接受兼容 JSON/token 形状不授权使用 configured
semantic default，反之亦然。

### 3.2 Semantic profile reference 与 binding

```text
SemanticProfileRef {
    id: stable dotted identifier
    version: exact SemVer
    contentHash: sha256-lower-hex string
}

ProfileBinding {
    profile: SemanticProfileRef
    direction: source | target
    parameters: ordered typed object
}
```

`ProfileBinding` identity 包含全部 parameter value，不只包含 profile ref。Parameter key 按 descriptor
中的 `parameters` 顺序编码；省略未触发的 conditional parameter，不写 null placeholder。Baseline
parameter 的确定性比较顺序为：finite positive length 按 canonical binary64 IEEE-754 `totalOrder`，
string enum 按 UTF-8 bytes lexical，extension ref 按 `(namespace, version, contentHash)` lexical。
Future parameter type 必须在引入它的 profile version 中同时定义 canonical ordering；否则不能进入
自动 candidate set。

Profile 是版本化机器接口。它至少固定：

- format family、适用 direction、format version/producer/runtime evidence constraint；
- time/BPM/offset、坐标/角度/alpha、event layer/order/gap/overlap；
- speed、distance、Hold、parent/inherit、fake/judgment、presentation 和 resource/package 行为；
- 缺失字段默认值、合法数值域、mapping rule/version；
- 必需参数、已知实现限制和 strict eligibility。

Profile 参数只能填充 descriptor 明确声明的有限 schema，例如 PGR 的 `floorScalePx`。未知参数、
缺少 required 参数、类型错误或非有限值使用 `conversion.profile-parameter-invalid`。参数不能覆盖
descriptor 中已经固定的公式，也不能用一个自由字符串建立未登记的兼容模式。

Registry descriptor 的 `parameters` 是结构化数组，每项固定 `name`、`value_type`、
`required_when`、`constraint` 和 `allowed_values`；不能用 colon-delimited 人类字符串替代 typed
schema。`format_version_policy` 只能是 `exact`、`evidence-only` 或 `absent`：PGR baseline 为 exact
1/3，RPE 的 `RPEVersion` 只作 evidence，PEC 没有内建 version。Profile implementation 必须按这些
字段验证 binding，不能解析 `contract` 人类文本来猜参数类型。Descriptor 的 `report_categories`
列出该 profile 可能按触发条件产生的稳定 category，不表示每次转换都必须无条件生成全部 entry。

内建 profile registry 位于 `docs/conformance/conversion/profile-registry.toml`。每个 entry 指向一个
UTF-8 descriptor 文件，`contentHash` 是该文件原始 bytes 的 SHA-256；BOM、换行和空白变化都会
改变 hash。Resolved `ProfileBinding` 必须保存完整 ID、exact version 和 content hash。Selector
只写 ID 时，仅当当前 registry 中恰好有一个可用版本才可解析；否则必须要求 `id@version`。

自定义 profile 必须使用调用方控制的反向域名 ID、提供同样的 version/hash/descriptor，并在 report
中标为 `custom`；它不能声称通过内建 profile 的 conformance fixture。

### 3.3 Repair mode

```text
RepairMode {
    enabled: bool
    authorizedRules: ordered array<rule-id[@version]>
}
```

Repair 只修改非法、矛盾或缺失且无法构造合法 source semantic document 的输入。每项 Repair 必须
先产生原 diagnostic，再记录 source locator、old/new typed value、rule ref、reason 和 semantic
impact。启用 Repair 不授权：

- 在两个都合法但结果不同的 semantic profile 之间选择；
- 丢掉目标不支持的 canonical feature；
- 近似 runtime expression/Track/easing；
- 忽略资源、版权或 package containment 错误。

未列入 `authorizedRules` 的 Repair 不得执行。Repair 后整体 conversion status 至少为 `repaired`。

---

## 4. Descriptor、证据和 Profile 选择

### 4.1 Source 与 Target descriptor

```text
CapabilitySetRef {
    id: stable dotted identifier
    version: exact SemVer
    descriptorMediaType: nonempty ASCII string
    contentHash: sha256-lower-hex string
}

SourceDescriptor {
    format: fcs | pgr | rpe | pec | extension-id
    formatVersion: optional exact source value
    producer: optional { id, version }
    intendedRuntime: optional { id, version, optionsHash: sha256-lower-hex string }
    artifacts: ordered array<ArtifactDigest>
    parserDialect: ParserDialectRef
    syntaxMode: strict | compatible
    semanticProfile: optional ProfileBinding
    profileSelectionMode: strict | compatible
    repairMode: RepairMode
}

TargetDescriptor {
    format: fcs | fcbc | pgr | rpe | pec | extension-id
    formatVersion: optional exact target format value
    producer: { id, version }
    intendedRuntime: optional { id, version, optionsHash: sha256-lower-hex string }
    semanticProfile: optional ProfileBinding
    capabilitySet: CapabilitySetRef
}

ArtifactDigest {
    role: chart | manifest | metadata | audio | image | font | other
    logicalId: stable string
    hashAlgorithm: sha256
    hash: sha256-lower-hex string
    byteLength: nonnegative int
}
```

PGR/RPE/PEC 和外部 extension target/source 必须有 semantic profile。FCS source profile、FCBC
container profile 和 Execution ABI 已由各自权威规范定义，不在 Conversion registry 中伪造第二份
profile；它们仍须在 descriptor/capability 中记录 exact version。Artifact 不保存绝对本机路径、
URI credential 或 source bytes。

ConversionReport/Fidelity 使用 FCBC `Value` 编码，因此其中所有 SHA-256 digest 都是恰好 64 个
lowercase ASCII hex digit 的 string，所有 byte length 必须在 `0..=i64::MAX`，不能假设存在任意
`bytes` Value tag。FCBC header `sourceHash`、Resources record `hash:Bytes` 等由 `fcbc.md` 单独定义的
binary field 仍保存 32 raw bytes，不受本投影规则改变。

`CapabilitySetRef.contentHash` 对由 `descriptorMediaType` 标识的 exact descriptor bytes 计算；同一
binding 不能在 hash 后做换行或 object-order normalization。Capability descriptor 定义 writer/runtime
实际支持的字段、限制和 extension，不能用 writer version 替代内容。Target
semantic profile 定义目标格式/runtime 的语义，CapabilitySet 只在其上进一步收窄当前工具能力；
二者冲突时取交集，不能由 capability 静默扩大 profile。

### 4.2 DetectionEvidence

```text
DetectionEvidence {
    id: stable string
    kind: package-declaration | format-field | producer-field |
          structural-shape | runtime-option | explicit-selector |
          configured-default | canonical-equivalence
    sourceLocator: optional logical locator
    observedValue: optional typed value
    supports: ordered array<SemanticProfileRef>
    excludes: ordered array<SemanticProfileRef>
    parameterBindings: ordered array<{
        profile: SemanticProfileRef,
        name: string,
        value: typed value
    }>
    reliability: declaration | direct | structural | heuristic | configured
}
```

File extension、文件名和“JSON 不含某字符串”等 heuristic 只能产生 `heuristic` evidence，不能单独
证明 profile。缺失 `RPEVersion` 后 parser 填 160、补第一条 speed event、排序 BPM 或 event，也
不能作为 source evidence。

### 4.3 InterpretationDecision

```text
InterpretationDecision {
    candidates: ordered array<ProfileBinding>
    chosen: optional ProfileBinding
    reason: explicit | declared | unique-evidence |
            canonical-equivalent | configured-default | unresolved
    evidenceIds: ordered array<DetectionEvidence.id>
    ambiguityImpacts: ordered array<gameplay | timing | motion | scroll |
                                      presentation | resource | metadata>
    equivalenceProof: optional {
        verificationMethod: { id, version }
        candidateResults: ordered array<{
            binding: ProfileBinding,
            result: equivalent | different,
            comparisonEntryIds: ordered array<ConversionEntry.id>
        }>
        comparedDomains: ordered array<domain>
    }
}
```

Candidate 先按 `(profile.id, version, contentHash)`，再按第 3.2 节 parameter tuple 排序；evidence
按 `evidence.id` 排序。`chosen` 不是省略审计信息的快捷字段；report 必须同时保存原 bindings、
reason 和 evidence。

### 4.4 Source 自动选择算法

自动选择只能在以下情况成功：

1. 受支持的 package/manifest 明确声明一个 exact profile；
2. 未冲突的 direct evidence 唯一留下一个 profile；
3. 所有剩余 candidate 对当前输入的 timing、gameplay、motion、scroll、presentation、resource 和
   metadata 结果均经 canonical lowering 证明相同。

进入上述步骤前必须先由 explicit input、package/runtime declaration 或 configured binding 解析全部
触发的 required parameter。缺失/非法 parameter 使用 `conversion.profile-parameter-invalid`，不能把
它降级为 profile ambiguity，也不能让 equivalence test 替参数选择值。

第 3 项只证明“本输入未触发差异”，不合并或删除 profile。选择 canonical-equivalent representative
时先排除仅用于 compatibility-characterization 且 `strictEligible=false` 的 candidate；仍有 candidate
时取 `(id, version, contentHash)` lexical 最小项。若只剩 characterization profile，则按 lexical
最小项选择但整体不能 strict success。两种情况都保存全部 candidate 和逐 candidate comparison
result/entry；不能依赖未定义的 canonical object byte hash 冒充语义比较。

若合法 candidate 产生不同结果：

- explicit selector：使用用户指定且适用的 exact profile，并报告冲突 evidence；
- strict profile selection：`conversion.ambiguous-source-semantics`，不生成 canonical 成功结果；
- compatible profile selection：只有调用方预先配置了 default `ProfileBinding` 时才能采用，reason
  为 `configured-default`，并报告 candidates 和全部受影响 domain；
- Repair：不得消除该歧义，也不得把选择记录为 repaired。

Profile selection 不允许根据“转换后更像 FCS”、数值更小、字段更多、当前播放器更流行或 importer
恰好没有报错来打分。

### 4.5 Target 选择与 CLI/API

外部 target 不存在 generic PGR/RPE/PEC。Strict export 必须显式绑定 target profile；compatible
export 可以使用应用预配置 default，但仍要写入 report。公共 CLI 概念参数必须保留：

```text
--source-profile <profile-id[@version]>
--target-profile <profile-id[@version]>
```

库 API 必须接受 typed selector/binding，不能依赖进程全局变量。CLI 具体子命令拼写由 I10 决定，
但不得把这两个参数合并为 format version、parser mode 或 `--compatible`。CLI/API 还必须能为
selected profile 提供 typed source/target parameter binding；最终 flag spelling 可以后定，但不能
只支持 profile ID 而让 required parameter 隐式取宿主默认。

---

## 5. Source semantic IR、provenance 与语义状态

### 5.1 Source semantic IR

`SourceSemanticDocument` 必须已经由唯一 `ProfileBinding` 解释，但尚未套用 FCS canonical defaults。
它保留 source entity identity、source order、exact decimal/rational、字段出现状态和 profile rule ref。
PGR Line BPM、RPE `bpmfactor` 和 PEC command state 在这里可以存在；lowering 后它们只能作为
provenance fact，不得成为 FCS runtime 第二套时钟或隐式 evaluator。

### 5.2 OriginState

```text
unset
explicit-default
explicit-value
inherited
imported
generated
user-modified
```

不得通过值是否等于 default 推断 origin。Canonical edit 必须把受影响字段改为 `user-modified`，并
沿 provenance dependency graph 使对应 authoring-workspace round-trip handle stale。

### 5.3 ConversionProvenance

活动 workspace 中的完整 provenance 可以为每个 canonical field 保存：

```text
sourceArtifactId
sourceLocator / sourceSpan
sourceValue / sourceUnit / sourceOrder / sourceEntityId
profileBinding
mappingRuleRef
semanticStatus
originState
dependencies
```

完整 span、unknown field、token tree 和 raw representation 只属于活动 workspace。发行用 Fidelity
是该数据的受限投影，见第 15 章；不能通过“每个字段一个 record”重建完整 source document。

### 5.4 SemanticStatus

| Status | 含义 |
|---|---|
| `native` | 来源与 FCS/目标的规范语义直接相同 |
| `mapped` | 通过确定、精确、可审计公式映射 |
| `equivalent` | 表示不同但 canonical execution 经证明相同 |
| `approximated` | 用户授权且有误差界的目标近似 |
| `preserved` | 只保留结构化事实，不由目标执行 |
| `runtime-only` | 依赖已声明、非 Core 的 target runtime extension |
| `repaired` | 非法/矛盾 source 经授权修改 |
| `dropped` | 用户明确授权后未写入目标 |
| `unsupported` | 没有获准的表示，转换不能成功 |

`preserved` 不等于 lossless；raw source hash 也不证明语义已保留。`runtime-only` 在声明 runtime
上可以 execution-equivalent，但 strict portable conversion 仍失败。

---

## 6. Conversion policy、能力协商与近似授权

### 6.1 Policy

```text
conversionPolicy: semantic | roundtrip | strict
```

- `semantic`：以当前 canonical semantics 为准；外部 workspace 中旧 source value 不覆盖用户修改；
- `roundtrip`：仅在原 source workspace 仍可用、hash 匹配、依赖未 stale、profile/rule compatible
  时复用 source representation；否则从 canonical 重建并报告 representation change；
- `strict`：禁止 approximated、preserved-only、runtime-only、repaired、dropped、unsupported 和无法
  证明 equivalent 的行为。失败仍输出 report，不输出伪成功目标。

Roundtrip 不要求 FCBC 保存 source。FCBC 重新导出外部格式总是 canonical semantic export，不能
承诺 byte-exact source round-trip。

### 6.2 CapabilitySet

Target capability 至少声明：

```text
format/version and semantic profile
note kinds, judgment, sound and score fields
time domains, exactness, precision and numeric limits
tempo and line-local source time capability
Track interpolation/easing/blend/overlap
parent/inherit/transform
scroll speed/distance/reverse/Hold geometry
runtime expression environment and extensions
metadata/credits/resource/package/render/effect
entity/event/resource/byte limits
```

Negotiation 必须在写目标 bytes 前完成。每个 canonical feature 按以下优先顺序得到一个 decision：

```text
direct exact
equivalent exact rewrite
declared runtime extension
structured preserve / external sidecar
authorized sampled target approximation
authorized drop
unsupported
```

Exporter 不得先丢字段或采样，再把结果倒推成 report。

### 6.3 ApproximationAuthorization

```text
ApproximationAuthorization {
    enabled: bool
    targetDomains: ordered array<stable property/entity selector string>
    errorBudgets: ordered object<metric, finite nonnegative bound>
    maximumSegments: positive integer
    algorithm: { id, version }
}

DropAuthorization {
    enabled: bool
    targetDomains: ordered array<stable property/entity selector string>
    reason: nonempty string
}
```

Approximation 只在目标格式无法表示 exact canonical feature，且用户显式授权对应 domain/budget 时
可用。不存在全局默认“adaptive bake”。算法必须：

- 固定 Note、Hold、tempo、point、Track/event boundary 和 discontinuity 为 exact forced boundary；
- 使用 `fcs.md` 对应属性 error metric；
- 记录 source expression/descriptor hash、domain、declared/verified maximum、验证方法和 segment count；
- 写出后重导入并重新测量；超预算则 conversion failed；
- 不把采样结果写成来源语义等价或 `mapped`。

标准 FCS→FCBC 编译不接受 ApproximationAuthorization，不生成 descriptor kind 5 或 BakedCurve。
播放器为低端设备生成的 sampled cache 是播放器本地实现配置，对 FCS、FCBC、Fidelity、Report 和
RenderSection 均不可见。

Drop 需要独立 `DropAuthorization`，并列出 domain/entity/field；Approximation 授权不隐含 drop，
Repair 授权也不隐含二者。授权 object 进入 report，不得只保存无法审计的人类摘要。

---

## 7. ConversionReport

### 7.1 顶级结构

```text
ConversionReport {
    specificationVersion: "1.0.0"
    operationId: stable string
    source: SourceDescriptor
    target: TargetDescriptor
    interpretation: InterpretationDecision
    conversionPolicy: semantic | roundtrip | strict
    repairMode: RepairMode
    approximationAuthorization: optional ApproximationAuthorization
    dropAuthorization: optional DropAuthorization
    status: lossless | equivalent | approximate | preserved-only |
            repaired | unsupported | failed
    entries: ordered array<ConversionEntry>
    repairs: ordered array<RepairRecord>
    summary: counts by severity/status/category/domain
    outputHash: optional sha256-lower-hex string
}
```

Aggregation 顺序为：

1. 没有生成获准目标、target reparse 失败或比较超预算：`failed`；
2. 存在未解决 required feature：`unsupported`；
3. 执行过 Repair：`repaired`；
4. 只有 preserve 才保住某项 required source fact/semantics：`preserved-only`；
5. 存在获准 approximation/drop：`approximate`；
6. 所有 runtime semantics exact，但 representation 不同：`equivalent`；
7. 所有声明语义和 required round-trip fact 均精确可逆：`lossless`。

`dropped` 使结果至多 `approximate`，并必须在 summary 单独计数，不能被“在容差内”隐藏。一个
profile 本身具有 implementation loss 时不能得到 `lossless` 或 strict success。

### 7.2 Entry

```text
ConversionEntry {
    id: stable string
    category: stable diagnostic/category
    domain: timing | gameplay | motion | scroll | presentation |
            resource | metadata | syntax | profile | package
    severity: info | warning | error
    semanticStatus: SemanticStatus
    sourceLocator: optional logical string
    targetLocator: optional logical string
    entityId: optional canonical stable ID
    sourceValue: optional typed value
    interpretedValue: optional typed value
    canonicalValue: optional typed value
    targetValue: optional typed value
    profile: optional SemanticProfileRef
    rule: optional { id, version, contentHash }
    message: human text
    errorMetric: optional ErrorMetric
    dependencies: ordered array<entry id>
}
```

Entry 先按 conversion phase，再按 canonical entity stable order、field schema order、rule ID 排序。
Message 不是稳定 API；category/domain/status/profile/rule 是机器接口。大型 source/object/resource
payload 不得复制进 entry。

### 7.3 ErrorMetric

```text
ErrorMetric {
    domain
    metric
    declaredMaximum: finite nonnegative value in metric unit
    verifiedMaximum: finite nonnegative value in metric unit
    verificationMethod
    sampleCount: nonnegative int
    segmentCount: nonnegative int
    forcedBoundaries: ordered array<time in canonical chartTime domain>
    sourceDescriptorHash: sha256-lower-hex string
}
```

只写“已采样”或采样率不足以证明误差。验证器必须覆盖每段的解析界或可靠上界；只能数值抽样时，
report 必须标记方法能力，不能声称数学证明。

### 7.4 Determinism

相同 source artifact bytes、profile registry、ProfileBinding、CapabilitySet、policy、授权、Repair
allowlist 和 tool algorithm version 必须产生 byte-identical report canonical encoding。Operation ID
不得使用 timestamp/random/thread ID；推荐由上述输入 hash 派生。

---

## 8. 公共时间、单位与 rule registry

### 8.1 Exact source number

JSON/text decimal 先按十进制原文构造 exact rational；Beat triple 先做整数/rational 运算。只有进入
FCS canonical Float64 或目标字段时才按 `fcs.md` 数值规则转换，并检查 finite/range。不得先读成
`f32` 再把结果标记 exact。

所有 source time 最终映射为 canonical `chartTime`。Source-local beat 可以保存在 provenance，
但不自动等于 FCS `chartBeat`。Canonical global tempo map 只定义 FCS chartBeat↔chartTime；Line BPM、
`bpmfactor` 和 PEC command state 在 lowering 后消失。

目标只有离散 time 时：先固定所有 exact boundary，再逐个量化并验证 strict order。Hold end 必须
仍严格大于 start；统一 round 后碰撞不是合法结果，Repair 或 target approximation 都要单独授权。

### 8.2 坐标、角度与 alpha

每个坐标 rule 必须声明 source space、local/world、axis、origin、scale、aspect policy 和 FCS target
space。FCS logical world 为 1920×1080、中心原点、Y-up；Note local X 与 Line canvas X 不能共用
rule。角度声明正方向和零方向。Alpha 显式映射为 linear scalar，越界默认 diagnostic，不 clamp。

### 8.3 Stable rule reference

```text
MappingRuleRef {
    id: stable dotted identifier
    version: exact SemVer
    contentHash: sha256-lower-hex string
}
```

本版本 rule 的 normative descriptor 位于 `docs/conformance/conversion/mapping-rules.toml`，其中
`contentHash=SHA-256(UTF-8(contract))`，不包含 TOML 引号或换行。Rule 公式变化必须改变 version
或 content hash。Report 不得只保存人类可读公式。

### 8.4 PGR exact rules

以下公式中的 `T`、BPM 和坐标都是 source exact value：

| Rule ID | Version | 公式/约束 |
|---|---|---|
| `pgr.time.source-line-beat-t32` | 1.0.0 | `sourceLineBeat = T / 32`；不得命名为 canonical chartBeat |
| `pgr.time.per-line-bpm` | 1.0.0 | `chartTime = T * 60s / (32 * currentLineBpm)` |
| `pgr.time.first-line-bpm` | 1.0.0 | `chartTime = T * 60s / (32 * firstLineBpm)`；仅对应声明的兼容 profile |
| `pgr.note-x.unit108` | 1.0.0 | `positionXpx = sourceX * 108px` |
| `pgr.note-x.unit320_3` | 1.0.0 | `positionXpx = sourceX * 1920/18px = sourceX * 320/3px`；Phichain characterization |
| `pgr.line-x.normalized` | 1.0.0 | `lineXpx = (sourceX - 0.5) * 1920px` |
| `pgr.line-y.normalized` | 1.0.0 | `lineYpx = (sourceY - 0.5) * 1080px` |
| `pgr.v1-move-x.trunc1000-div880` | 1.0.0 | `x=(truncTowardZero(packed/1000)/880-0.5)*1920px` |
| `pgr.v1-move-x.round1000-div880` | 1.0.0 | `x=(roundTiesAwayFromZero(packed/1000)/880-0.5)*1920px` |
| `pgr.v1-move-y.mod1000-div520` | 1.0.0 | `y=(truncRemainder(packed,1000)/520-0.5)*1080px` |
| `pgr.v1-move-y.mod1000-div530` | 1.0.0 | `y=(truncRemainder(packed,1000)/530-0.5)*1080px` |
| `pgr.rotation.clockwise-deg` | 1.0.0 | `angle = -sourceDegrees * π/180` |
| `pgr.offset.seconds` | 1.0.0 | `audioOffset = sourceOffset * 1s` |

Baseline v1 profile 要求 packed value 非负且 remainder 在声明 canvas 范围；负 packed 或越界值没有
已确认语义，strict 失败。Profile 可以通过新版本扩展，不能复用上述 ID/hash。
本表的 `truncTowardZero(x)` 是朝 0 取整数，`roundTiesAwayFromZero(x)` 是最近整数且恰好半值时远离
0，`truncRemainder(x,m)=x-truncTowardZero(x/m)*m`；不得换成宿主语言未声明的 `%`/rounding 规则。

### 8.5 RPE exact rules

| Rule ID | Version | 公式/约束 |
|---|---|---|
| `rpe.beat.abc-strict` | 1.0.0 | `beat=a+b/c`，baseline 要求 `c>0`；否则 invalid |
| `rpe.beat.abc-zero-zero-integer` | 1.0.0 | `[a,0,0]→a`；其他 `c=0` invalid，`c>0` 时同 strict |
| `rpe.time.bpmfactor-divide` | 1.0.0 | 每 BPM segment 使用 `effectiveBpm=BPM/factor`，因此 `dt=db*60*factor/BPM` |
| `rpe.time.bpmfactor-multiply` | 1.0.0 | `effectiveBpm=BPM*factor`，因此 `dt=db*60/(BPM*factor)` |
| `rpe.time.bpmfactor-ignore` | 1.0.0 | `dt=db*60/BPM`；仍保存 raw factor provenance |
| `rpe.x.canvas1350` | 1.0.0 | `xPx=sourceX*1920/1350` |
| `rpe.y.canvas900` | 1.0.0 | `yPx=sourceY*1080/900` |
| `rpe.alpha.byte255` | 1.0.0 | `alpha=sourceAlpha/255`，baseline input 0..255 |
| `rpe.offset.milliseconds` | 1.0.0 | `audioOffset=sourceOffset*1ms` |
| `rpe.rotation.clockwise-deg` | 1.0.0 | `angle=-sourceDegrees*π/180` |
| `rpe.speed.scale4_5` | 1.0.0 | `canonicalSpeed=sourceSpeed/4.5`，distance 仍按 profile speed interpolation 积分 |
| `rpe.note-fake.nonzero` | 1.0.0 | `judgment.enabled=(isFake==0)` |
| `rpe.note-side.equals1` | 1.0.0 | `above==1` 映射 above，其他已接受值映射 below |
| `rpe.note-visible-time.phira` | 1.0.0 | `visibleTime>=noteTimeSeconds` 时无 lower bound，否则 `visibleFrom=noteTime-visibleTime` |
| `rpe.note-alpha.phira-u16` | 1.0.0 | `alpha=min(sourceAlpha,255)/255`；只用于 Phira Note profile |
| `rpe.note-size.phira-uniform` | 1.0.0 | `scale=(sourceSize,sourceSize)` |
| `rpe.note-y-offset.phira-speed` | 1.0.0 | `offsetYpx=sourceYOffset*1080/900*sourceSpeed` |
| `rpe.hitsound.phira-resource-root` | 1.0.0 | 三个 built-in 文件名映射默认音效，其他值解析为 package-root audio resource |

`bpmfactor` rule 作用于该 Line 的 Note、Hold endpoint、普通 event、speed event 和其他以 Line Beat
表达的 runtime 值。它不是“只影响 scroll”的字段。映射出的 chartTime 再通过 canonical global
tempo map 的 inverse 得到可选 chartBeat；factor 不进入 runtime。

### 8.6 PEC exact rules

| Rule ID | Version | 公式/约束 |
|---|---|---|
| `pec.time.direct-beat` | 1.0.0 | `source decimal number` 直接成为 exact source Beat |
| `pec.note-x.relative2048` | 1.0.0 | `noteXpx=sourceX*1920/2048`；0 是 Line 中心 |
| `pec.line-x.canvas2048` | 1.0.0 | `lineXpx=(sourceX/2048-0.5)*1920px` |
| `pec.line-y.canvas1400` | 1.0.0 | `lineYpx=(sourceY/1400-0.5)*1080px` |
| `pec.rotation.clockwise-deg` | 1.0.0 | `angle=-sourceDegrees*π/180` |
| `pec.alpha.byte255` | 1.0.0 | 非负 `alpha=sourceAlpha/255` |
| `pec.offset.bias150ms` | 1.0.0 | `audioOffset=(rawOffset-150)*1ms` |
| `pec.offset.bias175ms` | 1.0.0 | `audioOffset=(rawOffset-175)*1ms` |
| `pec.cv.scale5_85` | 1.0.0 | `sourceVelocity=rawCv/5.85` |
| `pec.cv.scale7` | 1.0.0 | `sourceVelocity=rawCv/7` |
| `pec.cv.rpe-height900` | 1.0.0 | `rpeSpeed=rawCv*900/1400`，再经 `rpe.speed.scale4_5`，合成为 `rawCv/7` |

Conversion 1.0.0 没有 `pec.time.tick2048`。没有 producer/runtime declaration 的大整数不能成为
tick evidence；未来找到可复现证据时必须用新的 custom/built-in profile 和 rule ID 引入。

### 8.7 逆公式与量化

Exporter 使用所选 target profile 中 rule 的数学逆。任何 rounding/quantization 都使用独立 rule
ref，保存量化前后值和 error；不得给 exact rule 加一个未登记的 `.round` 字符串后仍标记 mapped。

---

## 9. Conversion 1.0 内建 Profile registry

本表是 registry 的人类可读索引；机器权威 ID/version/hash/contract 在 conformance descriptor 中。
“Strict eligible”只表示 profile 本身可以用于 strict conversion；仍要求输入合法、required 参数
完整、target 能力充分且没有 approximation/Repair/loss。

| Profile ID | Direction | Class | Strict eligible | 核心分歧轴 |
|---|---|---|---:|---|
| `pgr.phira.v1@1.0.0` | source,target | semantic | yes | per-Line BPM；v1 trunc/520；Phira Hold-speed；需要 `floorScalePx` |
| `pgr.phira.v3@1.0.0` | source,target | semantic | yes | per-Line BPM；v3 normalized XY；Phira Hold-speed；需要 `floorScalePx` |
| `pgr.phichain-import.v1@1.0.0` | source | compatibility-characterization | no | first-Line BPM；Note X=width/18；v1 round/530；Hold normalization；tool fitting/loss 必须另报 |
| `pgr.phichain-import.v3@1.0.0` | source | compatibility-characterization | no | first-Line BPM；Note X=width/18；v3 XY；Hold normalization；tool fitting/loss 必须另报 |
| `rpe.community.divide-bpmfactor@1.0.0` | source,target | evidence-profile | yes | factor divide；additive layers；missing rotate=false；speed mode 参数必填 |
| `rpe.docs-example.multiply-bpmfactor@1.0.0` | source,target | evidence-profile | yes | factor multiply；additive layers；missing rotate=true；speed mode 参数必填 |
| `rpe.phira.legacy-speed@1.0.0` | source,target | semantic | yes | factor ignored；additive layers；missing rotate=false；player legacy speed path |
| `rpe.phira.rpe170-speed@1.0.0` | source,target | semantic | yes | factor ignored；additive layers；missing rotate=false；version-branched derivative/modern speed |
| `rpe.phichain-import@1.0.0` | source | compatibility-characterization | no | `[a,0,0]` integer；factor ignored；first layer only；missing rotate=true；presentation/fake loss |
| `pec.phira@1.0.0` | source,target | semantic | yes | direct Beat；150ms；cv/5.85；line/last-note dialect；negative-alpha extension |
| `pec.extends@1.0.0` | source | compatibility-characterization | no | direct Beat；175ms；cv/7；token-stream；integer-step easing 是 approximation |
| `pec.phispler@1.0.0` | source | compatibility-characterization | no | direct Beat；150ms；RPE-height cv；global suffix zip 有 association loss |

Registry 不指定永久默认 profile。一个项目可以配置 compatible default，但该选择必须出现在每份
report；不能把“Phira style”或任一工具 silently 设为规范默认。

---

## 10. PGR v1/v3 Import

### 10.1 Detection 与 format version

PGR source 必须是满足 profile parser dialect 的 JSON object，`formatVersion` 明确为 1 或 3，且有
`judgeLineList`。未知 version 使用 `conversion.unsupported-format-version`。文件扩展名或“不含 META”
只能是 heuristic。

`formatVersion` 只选择 v1 packed/v3 split shape，不能选择 Line BPM、v1 520/530、Hold speed、
floor scale、offset/package composition 或 event overlap policy。

### 10.2 Time、Line BPM 与 canonical tempo

- 每个 raw `T` 先通过 `pgr.time.source-line-beat-t32` 保存 source Line Beat；
- Phira profile 使用当前 Line BPM；Phichain-import profile 使用第一 Line BPM；
- BPM 必须 finite 且大于 0；Note start/end 和 event boundary 全部使用相同 profile time rule；
- 结果直接成为 canonical chartTime，Line BPM 不成为 FCS Line field或第二 clock。

PGR 没有独立 global musical tempo。Per-Line profile 使用
`pgr.tempo.per-line-canonical-anchor@1.0.0`：所有 Line BPM 相同时采用共同 BPM；否则优先使用
package/profile 的可信 global tempo declaration；仍不存在时生成 `0beat→60bpm` 的 identity tempo
anchor，使 `chartBeat=chartTime/1s`，并记录 `conversion.generated-canonical-tempo`。First-Line
compatibility profile 使用 `pgr.tempo.first-line-anchor@1.0.0`，把第一 Line BPM 建为 global tempo，
因为这正是该 profile 的时间解释。Generated/first-line anchor 都不允许再次改变已经映射的
chartTime、Line motion 或 scroll；Importer 必须显式构造每 Line scroll model，不得从多数 Line 或
最小误差猜 tempo。

### 10.3 Coordinate 与 Line event

- v3 move 使用 normalized X/Y rules；
- v1 profile 成对选择 trunc/520 或 round/530，不能交叉拼接；
- Phira profile 的 Note X 使用 `pgr.note-x.unit108`；Phichain-import characterization 使用
  `pgr.note-x.unit320_3`；二者都不能使用 Line normalized rule；
- rotate/disappear/move 是 source linear interval；point 与普通 segment 分离；
- gap/overlap/source order 只有 profile 明确定义时才能 lowering；非法 overlap 默认失败，裁剪、排序
  或最后项覆盖均是 Repair/compatibility decision；
- PGR 核心没有 parent，Importer 不根据线号/几何猜 parent。

### 10.4 Speed、floorPosition 与 Hold

Speed event 按 selected time rule 构造 exact piecewise velocity/distance。Raw `floorPosition` 是缓存
验证点，不是第二真值：

1. 从 speed 重建每个 cache point；
2. 保存 raw/cache/reconstructed/error；
3. mismatch 使用 `conversion.distance-mismatch`；strict 失败，替换 cache 是 Repair；
4. 首 speed 不在 0、空 list、gap、overlap、负 speed 都按 profile/diagnostic 处理，不能自动补。

PGR visual floor unit 未由 `formatVersion` 唯一确定。Strict profile binding 必须提供 finite positive
`floorScalePx`；缺失时 gameplay time 仍可转换，但 scroll presentation 至少 `preserved-only`，strict
conversion 失败。不得在 648、1080、FCS default 120px 或宿主 viewport 间静默选择。

Hold end 是 `time+holdTime`，必须晚于 start。`pgr.phira.*` 把 Hold head scrollFactor 固定为 1，
tail geometry 来自 Line distance；`pgr.phichain-import.*` 按 Note 时刻 Line speed 对 raw Hold speed
归一化。两者有歧义时必须 profile-select，不能称为同一公式。

### 10.5 Note、offset 与 package

Type 1/2/3/4 分别映射 Tap/Drag/Hold/Flick；`notesAbove/Below` 映射 gameplay side。核心 PGR 没有
fake/presentation extension；unknown `isFake` 只有 declared dialect/profile 才能执行，否则仅留活动
workspace unknown field。

Chart `offset` 使用 `pgr.offset.seconds`。Package manifest 的 music/illustration/info offset 是单独
artifact/evidence；若 runtime 定义 chart+package offset 相加，分别记录两项和最终 FCS audioOffset，
不把外层 offset 改写成 chart raw value。

---

## 11. RPE Import

### 11.1 Detection、RPEVersion 与 Beat

Importer 必须分别保存 `META.RPEVersion` 的 JSON type/raw value、producer/editor、package runtime
option、实际字段和 profile。缺失/非法 version 后 parser 猜 160 只属于兼容 parser fact，不能成为
profile evidence。

Baseline strict Beat 使用 `rpe.beat.abc-strict`。只有 `rpe.phichain-import` 接受 `[a,0,0]` 作为整数；
把其他 denominator 0 改 1 是 Repair。Decimal/Beat triple 均保持 exact，BPMList 必须 finite positive、
非递减并保留 same-Beat source order；排序、去重和补 Beat 0 是 Repair。

### 11.2 bpmfactor 与唯一 chartTime

Profile 必须选择 divide、multiply 或 ignore rule，并应用于该 Line 的全部 Beat-timed runtime value。
Factor 必须 finite positive；缺失使用 profile 明确 default（baseline 为 1）。

Global canonical tempo map 来自 BPMList 本身。每条 Line 的 source Beat 按 factor rule 映射到
chartTime；factor 非 1 时，该 raw Line Beat 不等于同数值 canonical chartBeat。Lowering 后 factor
只留 provenance，不能在播放器端再次改变 Note time 或 scroll。

### 11.3 Event layer、easing 与 speed era

合法 RPE 普通 event layer 对同属性按 layer index 加法组合。每层 source order、start/end、
Bezier、easingType、easingLeft/Right 必须保留并 lowering 为 FCS add Track 或 exact expression。
“只取第一层”是 `rpe.phichain-import` 的已知有损 characterization，不能 strict success，也不能
作为普通 compatible optimization。

Speed profile 必须同时定义 instantaneous speed 和累计 distance：

- `phira legacy player path`：`useRpe170Speed=false`，忽略 speed easing shape，按 layer-additive
  piecewise linear raw speed 和 `rpe.speed.scale4_5` 积分；
- `phira RPE170 option`：`useRpe170Speed=true`；raw `RPEVersion>=170` 时 easing 直接插值 speed，
  更早 era 使用 easing derivative 表示 speed，使 distance 跟随 easing；
- community divide/multiply profile 必须绑定 `speedMode=legacy-linear|legacy-derivative|modern-eased`；
- 其中 `legacy-linear`、`legacy-derivative`、`modern-eased` 分别选择
  `rpe.speed.phira-legacy-player@1.0.0`、`rpe.speed.phira-legacy-derivative@1.0.0` 和
  `rpe.speed.phira-modern-eased@1.0.0`；
- Phichain-import 只保存其支持的 linear/first-layer projection，丢失 easing/layer 必须报告。

遇到无法由 Core easing/Expression exact 表示的合法 source curve，先尝试 exact typed expression 或
runtime extension；只有外部 target export 且得到 ApproximationAuthorization 才能采样。

### 11.4 Parent、default 与 presentation

`father` 映射 Line parent；`rotateWithFather` 映射 inherit.rotation。显式字段优先于 profile default：

- Phira/community baseline 缺失为 false；
- docs-example/Phichain baseline 缺失为 true。

不存在 parent、self/cycle、强制继承、提升 root 或断边默认错误；修改是 Repair。Position 与 rotation
inherit 分开建模。

Note type/time/end/positionX/side/fake 映射 Core gameplay；alpha/size/visibleTime/yOffset/tint/texture/
hitsound 映射 presentation/resource。`visibleTime` 是秒而不是 Beat。Phira profile 使用第 8.5 节
七个 Note/presentation rule：任意非零 `isFake`、`above==1`、conditional absolute visibleFrom、u16
Note alpha saturation、uniform XY size、`yOffset×speed` 和 package-root hitsound。Fake 不由 alpha=0
推断。Community/docs evidence profile 若没有另行绑定这些 presentation 轴，遇到对应非默认字段时
必须 `profile-not-applicable` 或使用更完整 custom profile，不能静默继承 Phira。Phichain-import 对
fake/presentation 的删除或忽略是已知 loss，不能宣称 lossless。

META chart offset 与 package info offset 分别通过 provenance 组合；display metadata owner 与 runtime
offset composition 是两项 decision。Texture/GIF/font/hitsound 必须经受控 package resolver 读取为
opaque raw resource bytes，再进入 `CanonicalResourceBundle`。

---

## 12. PEC Import

### 12.1 Parser dialect 与 direct Beat

PEC parser 必须保留 first-line offset、每行/token/span、decimal 原文、全局 command order、Note 与
`#`/`&` 的物理邻接以及 unknown/extra token。

- Phira dialect：第一物理行 offset，之后每行一个 command，suffix 关联最近 Note；
- extends dialect：全文件 whitespace token stream；
- phispler dialect：按行分类后 global zip，具有已知 association loss，不能 strict。

所有内建 profile 使用 `pec.time.direct-beat`。`bp <beat> <bpm>`、Note/event command time 共用 source
Beat；BPM segment 使用 `dt=db*60/BPM`。Phira 的 late `bp` rejection 是 dialect constraint；先收集
排序 late BPM 是不同 dialect/Repair，不能静默互换。

### 12.2 Offset、Note 与坐标

Profile 明确选择 150ms 或 175ms bias；PEC 没有版本字段自动消除歧义。Report 同时保存 raw offset、
bias、chart offset、package offset 和最终 `audioTime=chartTime+audioOffset`。

`n1/n2/n3/n4` 映射 Tap/Hold/Flick/Drag。Note X 使用 `pec.note-x.relative2048`；Line `cp/cm` X
使用 `pec.line-x.canvas2048`，Y 使用 canvas1400。Side/fake 合法域按 profile 验证；Hold end 必须
大于 start。`#` speed 与 `&` width 只作用于 dialect 明确关联的最近 Note，不能跨 unknown command
猜测。

### 12.3 Stateful Line command

`cv/cp/cd/ca` 是 point/step；`cm/cr/cf` 是区间 easing，起始值来自同一 Line 在 start Beat 的当前
状态。Importer 必须按全局 source order 建状态机，然后生成显式 FCS point/segment；不得先按 command
kind 分组后猜最近 end。

第一个 interpolation 前没有 concrete value、同 Beat conflict、overlap、未知 easing 和反向区间
默认失败。以下都不是 profile 合法语义：从 0 猜起点、裁剪 overlap、禁用 event、把 easing 改
linear、按整数 Beat/T 采样或交换 endpoint；它们分别需要 Repair 或 target approximation 授权。

### 12.4 cv、distance 与 negative alpha

Profile 选择 cv/5.85、cv/7 或 RPE-height chain，并用同一 rule 构造 speed、distance 与 Hold geometry。
缺少首 cv 时不能自动补 0/10；profile 若没有明确 default 则失败。Raw/final speed、积分边界和
floorScale 参数进入 provenance。

普通 alpha 使用 `pec.alpha.byte255`。Phira profile 遇到负 alpha 时要求
`negativeAlphaExtension` parameter，值必须是已注册 required extension 的
`{namespace, version, contentHash}` typed object；Conversion 1.0 不凭社区实现名发明默认 namespace。
缺少、非法或未注册的 parameter binding 使用 `conversion.profile-parameter-invalid`，转换不能成功；
即使 binding 有效，该结果仍是 `runtime-only`，不能 strict portable success。其他 profile 未声明
该语义时是 unsupported。把负值 clamp 到 0 是 Repair/drop，不是 exact mapping。

### 12.5 Package

PEC payload 不含完整音乐、插图、metadata。Package importer 从受控 manifest/resource root 解析，
chart offset 与 package offset 分开保存。历史 PEZ/ZIP 不是 PEC source semantics，也不改变 FCBC
自包含容器规则。

---

## 13. Export 到 PGR/RPE/PEC

公共顺序：

1. 验证 canonical chart/resource；
2. 解析 exact target ProfileBinding 与 CapabilitySet；
3. 对全部 canonical feature 完成 negotiation；
4. 固定 Note/Hold/tempo/point/Track/discontinuity exact boundary；
5. 先做 direct/exact rewrite；只对获准 domain 做 approximation/drop；
6. 按 target rule 的逆公式转换 time/coordinate/offset/speed；
7. 量化并验证 target numeric/entity/package limit；
8. 写 target bytes/package；
9. 使用同一个 parser dialect、target profile 和 runtime option 重新导入；
10. 做 canonical semantic comparison，完成 report/output hash。

Target-specific 要求：

- PGR 必须选择 v1/v3、Line BPM、packed variant、Hold speed 和 floor scale；v3 不自动成为默认；
- RPE 必须选择 BPMfactor、Beat zero denominator、layer、speed era、rotate default、fake/presentation
  和 package resource 行为；只写 `RPEVersion` 不足；反过来，选择 version-branched target profile
  时必须写出或在 target descriptor 中绑定明确 `RPEVersion`，不得依赖接收方 parser 的缺失默认；
- PEC 必须选择 direct-Beat runtime、offset bias、suffix dialect、cv/negative-alpha/easing 行为；默认
  不把 Beat 乘 2048；
- 不支持的 Render、credit、resource、parent、extension 或 expression 不能只忽略；
- target reparse 超过声明误差时 status=failed，失败产物只有用户显式要求诊断输出时才可保留，且
  不得作为成功 artifact。

---

## 14. Canonical semantic comparison

Round-trip comparison 至少覆盖：

- global tempo 的 beat/time mapping 与每个 imported exact chartTime；
- Note count、kind、line、start/end、side、judgment、judge/sound/score policy；
- Line parent DAG、inherit 和强制 test times 的 world transform；
- Track point/segment boundary、value、适用 derivative、blend/priority/fill；
- scroll velocity、累计 distance、floorScale、Note/Hold geometry；
- presentation visibility、position、alpha、scale、texture/hitsound；
- resource stable identity、kind/media type、raw byte hash；
- metadata/credits/sync required preservation；
- Render semantic scene（目标支持时）；
- required runtime extension identity/version/payload semantics。

Entity 使用 stable provenance mapping 对齐，不按 array index。Discrete field 必须 exact。Continuous
property 使用 `fcs.md` 指标；event boundary exact，除非 target time quantization 单独获准并报告。

`equivalent` 要求 gameplay/timing exact，且其余声明执行语义 exact 或在获准误差内、没有
preserved-only/unauthorized drop。`lossless` 还要求 profile 定义的 required source semantic fact 和
target round-trip fact 可恢复；不要求恢复 whitespace/comment/raw source bytes。

---

## 15. Fidelity、FCBC 与 source preservation 边界

### 15.1 Authoring workspace 与 source round-trip

完整 source bytes、token/trivia、comment、unknown field、原始对象顺序、external source AST 和
byte-exact round-trip handle 只能留在制谱器/converter 的活动 authoring workspace或用户管理的外部
sidecar。它们不进入 CanonicalCompilation 的可分发部分。

### 15.2 FCBC Fidelity schema

FCBC Fidelity section 是 ordered FCBC `Value(object)`：

```text
specificationVersion: "1.0.0"
sources: array<restricted SourceDescriptor>
profileBindings: array<ProfileBinding>
entityMappings: array<{ canonicalId, sourceArtifactId, sourceEntityId }>
fieldFacts: array<restricted provenance fact>
mappingRules: array<MappingRuleRef>
semanticLosses: array<{ domain, status, category, entityId? }>
custom: ordered object
```

Standard key 顺序如上。Restricted descriptor/fact 可以保存 input hash、logical source locator、
producer/runtime/profile/rule、origin state、source unit、必要的单个 numeric/enum mapping fact和 stable
entity mapping；不得保存：

- FCS/PGR/RPE/PEC source bytes、text、token、AST、JSON/object tree 或等价 base64/hex/compressed data；
- comment、template、generator、局部变量、compile-time branch/expansion graph；
- 可以按 source traversal 完整恢复原文/结构的穷举 field dump；
- workspace absolute path、URI credential、外部 archive locator；
- resource payload/hash 副本（资源由 FCBC Resources/ResourceData 唯一保存）；
- BakedCurve、sampled player cache 或 target approximation payload。

### 15.3 FCBC profile/section matrix

- `runtime`：可按 feature bit 携带结构化 Fidelity/ConversionReport/DistributionMetadata；
- `fidelity`：必须有 Fidelity，可同时有 Report/DistributionMetadata；仍不是 authoring backup；
- profile value 2 reserved，FCBC 2 没有 archive/editable/source-snapshot profile；
- `strict-runtime`：只允许 portable exact runtime 数据；可以保存不改变执行的结构化 report fact；
- 所有 profile 恰好一个 chart、全部资源 raw bytes 内嵌、没有 external lookup；
- ConversionReport section 保存一份 report object或按发生顺序的 report array；
- DistributionMetadata 只保存 producer/profile/rule/input hash/repair/package ownership 等非原文事实；
- 剥离 Fidelity/Report/DistributionMetadata 不得改变 Core、Render 或 resource execution。

FCS→FCBC standard compile 不通过 intermediate external source snapshot。来自 PGR/RPE/PEC 的 canonical
chart 可以写 FCBC，header `sourceHash` 按 `fcbc.md` 置零；外部 artifact hashes 写 structured report/
DistributionMetadata。

---

## 16. Resource、package、版权与安全

Importer 的 package resolver 必须：

- 以显式 workspace/package root 解析 logical relative member；
- 拒绝 URI、绝对路径、反斜杠、`.`/`..`、escape 和 symlink 越界；
- 对输入文件原始 bytes 计算 SHA-256，不在 converter 中 decode/transcode；
- 分开记录 chart/manifest/package metadata offset 与 resource provenance；
- 把全部 runtime-required resource 放入 `CanonicalResourceBundle`，由 FCBC 原样内嵌。

Report/Fidelity 中的 source identifier、logical path 和 metadata 仍可能含隐私或版权信息。Writer
不得复制无关大 payload或 credential；hash 不代表分发许可。私有版权 fixture 必须 opt-in，缺失时
报告 skipped，不能伪装已验证。

Parser/Converter limit 至少覆盖 input bytes、JSON/TOML/text nesting、string、number digits、entity、
event、profile candidate、report entry、resource count、single/total resource bytes、approximation
segment 和 reparse budget。超过 limit 使用稳定 diagnostic，不返回部分可执行 chart。

---

## 17. Conformance

Conversion conformance corpus 必须包含：

1. Profile/dialect/rule/diagnostic registry 的 ID/version/hash/direction/class/strict eligibility 与
   cross-reference 完整性；
2. source selection：explicit、declared、unique evidence、canonical-equivalent、configured default 和
   unresolved ambiguity；
3. PGR v1 trunc/520 与 round/530、v3、per-Line/first-Line BPM、floor cache、Hold speed；
4. RPE strict/legacy Beat、divide/multiply/ignore bpmfactor、additive/first layer、speed era、rotate
   default、fake/presentation/resource；
5. PEC direct Beat、150/175 offset、Note/Line X、三种 cv chain、suffix dialect、negative alpha；
6. 每种 SemanticStatus、policy、Repair allowlist、approximation/drop authorization；
7. target reparse/canonical compare、deterministic report order/hash；
8. FCBC Fidelity/ConversionReport 中没有 raw snapshot/authoring-only data；
9. public minimal/feature/extreme fixture 和 opt-in private copyright lane。

每个 format fixture 必须声明 parser dialect、format/producer/runtime evidence、source/target profile
ID/version/hash、profile parameters、policy、expected status/category、canonical snapshot 和容差。只写
`format="rpe"` 的旧 example 不能成为 normative valid fixture。

当前 mapping/profile/selection manifest 是规范 schema/公式闭合向量，不表示活动 Rust workspace 已有
PGR/RPE/PEC converter。实现阶段必须另外加入真实 source fixture、canonical golden 和 reparse
round-trip。

### 17.1 Stable diagnostic category

实现至少保留以下 parent category：

```text
conversion.unsupported-format
conversion.unsupported-format-version
conversion.parser-dialect-unsupported
conversion.source-invalid
conversion.profile-required
conversion.profile-not-found
conversion.profile-version-unsupported
conversion.profile-hash-mismatch
conversion.profile-not-applicable
conversion.profile-parameter-invalid
conversion.ambiguous-source-semantics
conversion.target-profile-required
conversion.capability-mismatch
conversion.repair-not-authorized
conversion.approximation-not-authorized
conversion.approximation-budget-exceeded
conversion.drop-not-authorized
conversion.distance-mismatch
conversion.resource-missing
conversion.package-escape
conversion.roundtrip-mismatch
conversion.report-limit
```

实现可以返回更细 subcategory，但不得把 profile ambiguity 报成 syntax error、把 Repair 报成 mapped，
或把 target capability loss 报成 source invalid。

### 17.2 Stable report-entry category

下列非 error category 仍是 `ConversionReport.entries[].category` 的稳定机器接口：

```text
conversion.generated-canonical-tempo
conversion.compatibility-characterization
conversion.tool-rewrite
conversion.profile-evidence-conflict
conversion.ignored-source-semantic-field
conversion.runtime-option-bound
conversion.layer-loss
conversion.presentation-loss
conversion.runtime-extension
conversion.suffix-association-loss
```

`conversion.capability-mismatch`、`conversion.approximation-not-authorized`、
`conversion.approximation-budget-exceeded`、`conversion.drop-not-authorized`、
`conversion.distance-mismatch`、`conversion.resource-missing` 和 `conversion.roundtrip-mismatch` 可以
同时作为失败 diagnostic 与 report entry。完整 usage/domain 绑定在
`docs/conformance/conversion/diagnostic-categories.toml`；profile/vector 不得引用未登记 category。
Registry 的 `cross-domain` 表示该 category 可以和 entry 自己声明的任一第 7.2 节具体 domain 组合，
不是 `ConversionEntry.domain` 的新取值。
