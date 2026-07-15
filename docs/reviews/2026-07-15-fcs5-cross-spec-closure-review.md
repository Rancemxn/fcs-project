# FCS 5 S15 Cross-Spec Closure Candidate Review

日期：2026-07-15

状态：四规范联合候选自检完成，仍有明确的 conformance/独立复审阻塞项；五个版本域全部保持
Draft，本文不是 independent review，不授权重新 Frozen，也不授权开始 I1 Rust 实现。

## 1. 审查范围与结论

本轮按依赖顺序联合检查：

```text
FCS Core authoring/canonical semantics
→ FCBC Container 2.0 / Execution ABI 1.0
→ Render Profile resource/runtime binding
→ Conversion Specification semantic profiles/report
→ conformance、governance、implementation matrix 和 I1 gate
```

联合自检关闭了本轮发现的直接术语、profile、feature flag、`Value` projection、resource binding、
profile parameter 和 equivalence 冲突。四份根规范现在对以下边界使用同一候选语义：

- FCS 是普通目录 workspace 中的 authoring source；FCBC 是恰好一个谱面、全部声明资源原始 bytes
  内嵌的自包含分发/执行容器；
- standard FCS→FCBC 完整展开 comment/template/generator/local 等 authoring-only 结构，但保留
  exact Constant/SegmentTrack/Piecewise/Expression DAG，不生成 BakedCurve；
- 播放器 sampled cache 是默认关闭的本地实现配置，不进入 FCS、CanonicalCompilation、FCBC、
  RenderSection、Fidelity、ConversionReport 或 packager output；
- FCBC profile 是 runtime/fidelity/reserved/strict-runtime，value 2 必须拒绝；所有 profile 都拒绝
  descriptor kind 5，并只通过 Resources section 6 与 ResourceData section 20 解析资源；
- Conversion parser dialect、`syntaxMode`、`profileSelectionMode`、typed `ProfileBinding`、Repair、
  target `CapabilitySet` 和 approximation/drop authorization 相互独立；
- PGR/RPE/PEC 不存在无版本、无 runtime 语义的通用默认 profile；歧义只可由声明、显式 binding、
  唯一 direct evidence 或当前输入的 canonical semantic equivalence 关闭。

这说明 S15 的规范 delta 已形成可审计候选，不表示剩余 conformance 已完成。第 7 节的阻塞项关闭、
独立 reviewer 复核并由治理文件重新标记 Frozen 前，任何实现计划都不能把本 review 当作规范冻结。

## 2. 当前候选版本与内容哈希

单文件 hash 按文件原始 bytes 计算 SHA-256：

| 版本域 | 候选版本 | 权威文件 | SHA-256 | 当前状态 |
|---|---:|---|---|---|
| FCS Core Source/Canonical | 5.0.0 | `fcs.md` | `34b47e3a56ed21ef45325ceb38fec2f0e9834ab4e5a1d6e303109e696efe0926` | Draft |
| FCBC Container | 2.0.0 | `fcbc.md` | `e90ac001743a3868b84ff5004d6e0d1d7dd70f50b91ed8f35d3d5bf3f64f8825` | Draft |
| FCS Execution ABI | 1.0.0 | `fcbc.md` | `e90ac001743a3868b84ff5004d6e0d1d7dd70f50b91ed8f35d3d5bf3f64f8825` | Draft |
| FCS Render Profile | 1.0.0 | `fcs-render.md` | `c0b6e47eeb98253e5aa3f02af5e15b3f2c496c0150f8b449b1144a438687dd9f` | Draft |
| FCS Conversion Specification | 1.0.0 | `fcs-conversion.md` | `7f8156af94858e25d453a8b25b7f5af3f9cd58f1651cbc235d7115cc7f4e0d72` | Draft |

Container 与 ABI 继续由同一个 `fcbc.md` 联合版本化和审查，因此共享文件 hash；该事实不表示两个
SemVer 域合并。2026-07-14 Frozen hash 和 S14 范围化 Reviewed hash 只保留历史审计用途，不能替代
本表候选 bytes。

## 3. 本次联合审计关闭的 finding

| Finding | 修正 |
|---|---|
| 最新 7 条 RPE presentation/resource rule 仍使用零 hash，两个 Phira RPE profile registry hash 已失效 | 复算全部 contract/profile raw-byte SHA-256；registry integrity test 重新通过 |
| Conversion 文档与测试仍绑定 49 rule/33 vector 等旧计数 | 统一为 12 profile、7 dialect、56 rule、32 category、38 exact、5 invalid、10 selection |
| FCBC section table 的 Fidelity 文案可被解释为 runtime/strict-runtime 禁止携带 Fidelity | 改为 HAS_FIDELITY↔section 双向约束，fidelity profile 额外强制置位并包含 |
| Header 使用未定义的 `FEATURE_SOURCE_HASH_PRESENT` | 统一为 bit 0 的 `SOURCE_HASH_PRESENT`，并闭合 clear/zero 约束 |
| `USES_REVERSE_SCROLL` 与 Line `ALLOW_REVERSE_SCROLL` 没有一致性规则 | 固定为所有 Line flag 的逻辑或，不能替代逐 Line 授权 |
| Core canonical equivalence 把可剥离 DistributionMetadata 混入 Conversion profile 等价判断 | 拆分 canonical semantic equivalence 与更严格的 distribution equivalence；selector/target reparse 使用前者 |
| FCBC Fidelity 被误写为 FCS `Value(object)` | 统一为 FCBC `Value(object)`；report hash 使用 lowercase hex string，binary header/resource hash 仍为 raw 32 bytes |
| PEC negative-alpha 缺少 parameter 时同时被写成 unsupported 与 parameter-invalid | 缺失/非法/未注册 binding 固定为 `conversion.profile-parameter-invalid`；合法 extension 结果仍是 runtime-only |
| Render Geometry `Value(object)` 内的 u32 descriptor/ref 没有可编码 tag | 固定 descriptor/table ref 为受 u32 范围约束的 `Value(int)`，resource ID 为 `Value(resourceRef)`，并拒绝 Core unknown key |
| `docs/community/pec.md` 仍把 tick2048 描述成当前 Conversion 默认 | 改为早期候选 finding；当前 rule 已删除并作为 forbidden ID |
| PGR Note X 被误写成所有 profile 共用 108px/unit | Phira 固定 108px/unit；Phichain-import characterization 固定 `1920/18=320/3` px/unit |

联合术语审计还确认：

- `fcs-conversion.md` 使用的 32 个 `conversion.*` category 与 diagnostic registry 完全一致；
- `fcbc.md` 使用的 30 个 `fcbc.*` category 全部出现在 stable loader list；
- `fcs-render.md` 使用的 10 个 `render.*` category 全部出现在 stable Render list；
- Conversion 人类索引中的 12 个 versioned profile 与 registry 一一对应，所有显式 versioned
  profile/rule ref 都能解析；
- 标准正向路径没有 `rawSnapshots`、archive/editable profile、默认 tick2048、scroll-only bpmfactor、
  默认 adaptive bake、外部 resource lookup 或可写入格式的 player sampled cache。

## 4. 跨规范不变量

### 4.1 Authoring、canonical 与 distribution

```text
AuthoringWorkspace
├── FCS source
└── declared resource files
    → parse/static/elaborate/resource validation
    → CanonicalChart + CanonicalResourceBundle + DistributionMetadata
    → deterministic one-chart FCBC
```

CanonicalChart 不保存 source AST、comment、template/generator、local、workspace path 或 raw snapshot；
CanonicalResourceBundle 保存每个 canonical resource ID 的 kind/metadata/hash/original bytes。FCBC
Resources record 不保存 source path/URI，ResourceData 按 resource ID、8-byte alignment 和最小零
padding 保存未经解码、重编码、转码或容器级压缩的原始 payload。相同 hash 的不同 resource ID
不得合并。

FCBC 2 不定义加密、混淆、DRM、签名或外层 ZIP/PEZ。传输层可以在容器外压缩整个 byte sequence，
但解包后的 FCBC 不能依赖另一份 archive、文件、URL 或 workspace。

### 4.2 Exact execution 与 approximation

标准 lowering 只使用 descriptor kind 1–4：Constant、SegmentTrack、Piecewise、Expression。
Kind 5 保留编号但所有 FCBC 2 profile 都以 `fcbc.forbidden-descriptor` 拒绝。无法静态化为 Track 或
Piecewise 的合法 Core expression 必须保留为 typed Expression DAG；目标设备性能、shader 复杂度、
帧率和节点数都不是 distribution-time baking 条件。

Approximation 只存在于外部 target 能力不足、用户显式授权 domain/error budget、固定离散边界并在
same-profile target reparse 后验证的转换路径。播放器 sampled cache 不使用 Conversion
ApproximationAuthorization，也不产生 BakedCurve 或格式内 report payload。

### 4.3 Profile、parser 与 Repair

Source 固定阶段为：

```text
bytes/package
→ lossless parsed source
→ parser dialect validation
→ DetectionEvidence
→ typed source ProfileBinding selection
→ SourceSemanticDocument
→ FCS CanonicalChart + CanonicalResourceBundle + provenance
```

Exporter 固定先完成 target ProfileBinding/CapabilitySet negotiation，再 exact rewrite 或获准
approximation/drop，写 target 后用同一 profile reparse 并做 canonical semantic comparison。
Exporter 不得消费 FCS source AST 或外部 parse tree。Repair 只能修改非法、矛盾或缺失到无法构造
合法 semantic document 的输入，不能替用户选择两个都合法但不同的 profile。

### 4.4 Render resource/runtime boundary

Render resource 固定经过 FCS `@resource`→canonical stable ID→FCBC Resources/ResourceData→bounded
immutable view→kind-specific decoder/compiler。RenderSection 不保存 path、URI、hash/offset 副本、
source text、glyph cluster 或 resource payload；ImagePattern、Image、font、shader 等都只能引用同一
FCBC 已验证资源。动态属性只引用 exact descriptor kind 1–4。

## 5. Conformance baseline 与哈希

Tree hash 使用 ordinal relative-path 顺序，对每个文件追加
`UTF-8(path) + NUL + raw file bytes + NUL` 后计算 SHA-256。

| Suite | 当前候选内容 | Manifest SHA-256 | Tree files / SHA-256 |
|---|---|---|---|
| FCS Core | 39-entry source/static/canonical manifest | `4d8dfb7ba2d636cd94a02f39807c7a0da6185b73213f50dd77e8f2a69c5b25f4` | 51 / `e6398947d92e1edc88dfe1e82fadc1069bffebd4f4281b34f416c79780caa4e1` |
| FCBC | 2 schema-2 golden、11 minimal + 2 resource mutation | `86758355543f59767181be50f329368333eb6ef8436f7668aa138fe2ac1e7b31` | 7 / `550d26d1683a753e1c1ba430fae7dc8ef8772961c165fac9f94eb7a34398c7dd` |
| Render | source/semantic/raster entries + 1 opaque embedded-resource binding | `9deafb31ecf7bbd9a992bf7f487364cb274f43e70811013d8f2cc890bfff40aa` | 9 / `329c5409e70efaa66586703fb61f04fbe9fa986767965110e8a4fe45c3f5a998` |
| Conversion | 12 profile、7 dialect、56 rule、32 category、38 exact、5 invalid、10 selection | `28cd000589cf2b3d547abaed1f57bd2453267b511b535c3af8375527aae6c7b2` | 19 / `0d55f3eca638e2188fa8fded1ad1a1afc1865ce0c80f08488aeaf2d69e395b52` |
| 整个 `conformance/` | root schema 2 umbrella | `1378a680d7a593f4156f45646ba263641b5e8198d90f2c79ce3407e9e37eb4ac` | 88 / `a5d4071a61338fcb5990c4fa804b06e6a432c8702ad0f8b651820abb419f1a25` |

Execution ABI 当前继续引用
`conformance/fcs5/expected/numeric-vectors.toml`，其 SHA-256 为
`fd8ea6ac9775484b5c655d9627805f25c35068dfc32e60c8cf39a2218a4ce9d5`。该文件不是非空
Expression/Distance/RenderSection FCBC byte golden，不能用于关闭第 7 节 ABI blocker。

Conversion 细分 registry/vector hash 记录在
`2026-07-15-conversion1-semantic-profile-closure-review.md`，并由 Rust manifest integrity test 对
profile raw-file hash、dialect/rule contract hash、typed parameter 和所有跨引用重新计算。

## 6. 本次验证证据

在当前未暂存工作树上依次执行：

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

结果：

- Clippy 通过，零 warning；
- nextest run `68544f15-f02b-475e-97c9-4912e64346c6`：135/135 passed，0 skipped；
- 定向 `fcs-source::conformance_manifest`：2/2 passed；
- rustfmt check 与 `git diff --check` 通过；
- 排除 `.git`、`target` 和外部 `refer` 后，158 个项目文本文件严格 UTF-8 解码通过，0 个 NUL；
- 31 个项目 Markdown 文件的非 code-fence 本地 link audit：0 个 missing target；
- profile/rule/category/vector count、hash、path containment、direction/format/typed parameter 和引用
  完整性由测试验证。

这些测试只证明 I0 retained implementation 和当前 manifest/hex 完整性。活动 workspace 仍只有
`crates/fcs-source`，没有 canonical model、FCBC writer/loader、Execution ABI evaluator、converter、
RenderSection codec、renderer 或 player；不能据此声称上述规范语义已经实现。

## 7. 重新 Frozen 前的阻塞项

### 7.1 Conversion 真实 round-trip corpus（Important）

当前 Conversion corpus 是公式、registry 和 selection schema 向量。仍需加入可公开的真实 PGR
v1/v3、RPE、PEC source/package fixture，固定 lossless parsed-source evidence、exact ProfileBinding、
canonical golden、resource bundle、target bytes/package、same-profile reparse 与 semantic comparison。
每个内建 profile 还需要 runtime probe；approximation/drop/capability/error-budget 需要可执行边界向量；
FCBC Fidelity/ConversionReport 需要至少一个非空 byte vector。

### 7.2 Execution ABI 非空 byte/evaluation vector（Important）

现有两个 FCBC golden 只覆盖空 Core tables和 ResourceData framing。仍需至少一个非空
ConstantPool/Track/Expression/Distance/Line/Note 文件，执行 typed DAG、lazy And/Or/Choose、直接 seek、
portable analytic/evaluable distance、strict-runtime profile 和 kind-5 rejection。Writer、loader 与
reference evaluator 必须在同一 vector 上闭环；numeric TOML 不能替代二进制 record/引用验证。

### 7.3 RenderSection 完整 binary/raster binding（Important）

现有 solid-rect semantic/raster fixture 没有对应非空 FCBC RenderSection byte golden，opaque resource
binding 又显式不做 decode。独立复审前必须用 byte layout 与 loader vector 固定 Layer/Node/Geometry
以及 Path/Paint/Stroke/Clip/GlyphRun 的完整 record schema、辅助 stable-ID namespace/派生、Stroke 的
paint binding、table ordering/reference 和 resource type。还需内嵌固定 image/font 的 decoder、shaping、
glyph-run 与 reference raster 闭环；不能让未来实现填补当前 prose 中未由 byte vector消除的选择。

### 7.4 Core fixture execution 与独立复审（Important）

39-entry FCS manifest 中 S14/S15 新增 source/canonical fixture 当前只有 path/manifest integrity，活动
I0 parser 并未执行完整 grammar/canonical stage。重新 Frozen gate 必须明确由何种独立 reference
validator 证明这些 vectors 自洽，且不能通过先启动被禁止的 I1 实现来倒置治理顺序。

所有 Critical/Important finding 关闭后，必须由未参与本轮修改的 reviewer 检查四规范全文、上述
byte/round-trip/raster corpus、哈希与治理状态；本文作者的联合自检不能代替该步骤。

## 8. 独立复审必须特别确认的规范选择

PGR 没有独立 global musical tempo。当前 per-Line profile 在各 Line BPM 不同、又无可信 package/
profile global tempo declaration时，生成 `0beat→60bpm` neutral canonical anchor，使
`chartBeat=chartTime/1s`，并报告 `conversion.generated-canonical-tempo`；已映射 Note/event chartTime、
Line motion 和 scroll 不得因此改变。First-Line characterization 则使用第一 Line BPM。

这是本项目为承载“已由每 Line BPM 解码的唯一 chartTime”新增的 canonical 选择，不是 PGR source
本身声明的全局 tempo，也不是固定参考项目共同给出的事实。独立 reviewer 必须确认该选择不会让
global Beat-dependent extension/metadata 获得伪造语义；如不能确认，应在重新 Frozen 前改为要求
显式 global-tempo binding，而不是让实现按多数 BPM 或最小误差猜测。

## 9. Gate 结论

S15 四规范 delta 的联合候选自检与当前 workspace/hash gate已经完成，但第 7 节 Important blocker
仍开放，独立复审尚未发生。因此：

- FCS Core 5.0.0、FCBC Container 2.0.0、Execution ABI 1.0.0、Render Profile 1.0.0 和 Conversion
  Specification 1.0.0 全部保持 Draft；
- 不把本文或 135 个 I0 tests 描述为 Reviewed/Frozen/conforming implementation；
- 不开始 I1 Rust 实现，不创建未来 canonical/runtime/FCBC/converter/render/CLI 空壳 crate；
- 下一规范工作应先补齐第 7.1–7.4 的 fixture/layout/reference validation，再进行独立复审；
- 全部重新 Frozen 后，仍必须由用户重新确认 `docs/plans/i1-source-ast-parser.md`，才能开始 I1。
