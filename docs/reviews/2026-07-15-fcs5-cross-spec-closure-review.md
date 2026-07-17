# FCS 5 S15 Cross-Spec Closure Candidate Review

日期：2026-07-15

状态：四规范联合候选自检完成，仍有明确的 docs/conformance/独立复审阻塞项；五个版本域全部保持
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
| 整个 `docs/conformance/` | root schema 2 umbrella | `1378a680d7a593f4156f45646ba263641b5e8198d90f2c79ce3407e9e37eb4ac` | 88 / `a5d4071a61338fcb5990c4fa804b06e6a432c8702ad0f8b651820abb419f1a25` |

Execution ABI 当前继续引用
`docs/conformance/fcs5/expected/numeric-vectors.toml`，其 SHA-256 为
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

## 10. 2026-07-16 dated amendment：Execution ABI blocker closure

本 amendment 只更新第 7.2 项和由它直接影响的 hash/test/blocker ledger；前文保留 2026-07-15
联合候选自检发生时的审计事实，不静默改写历史 hash、test count 或当时的 gate 结论。I1 的旧人工
确认门已由 2026-07-16 治理修订取消，当前客观阶段门以 `docs/specifications/governance.md` 为准。

第 7.2 项现由 `docs/reviews/2026-07-16-fcbc2-execution-abi-nonempty-review.md` 关闭。新增 artifact：

- 3432-byte decoded strict-runtime/chart static FCBC，SHA-256
  `ffcd8f24bd792c406a584fea26c753b62fafaa7d8f91ff0886b697c67dbfea61`；
- 14 constants、14 descriptors、40 reachable expression nodes、2 lines、2 notes、2 distances；
- fixed test-only writer→static golden→independent test-only loader→independent evaluator；
- 10 descriptor bits/trace、7 direct-seek distance queries与 4 个实际执行的 deep/checksum mutation；
- writer/static bytes、length/SHA、14 section offset/length/CRC、owner/reachability/canonical order、lazy
  trace、analytic/evaluable error bits 和 stable diagnostic 均由测试闭合。

当前 hash 增量：

| Item | SHA-256 |
|---|---|
| `fcs.md` | `594ee2a13272a8501eec6a866e3dc09a233ff64699e9a8af5c69fd8aa9d11ddc` |
| `fcbc.md` | `1090fb88c2cc3805dc9c4b1e91eb3247d7773e17f1cf764add8f70e109ea4b78` |
| `docs/conformance/manifest.toml` | `231f4505de29c854201057f97706295756109a98dcc7ac99f08ca21cd3f96fe8` |
| `docs/conformance/fcbc/manifest.toml` | `e435c8d452ec991c6a023cab15ce2a2f819ae0febe68ae7b044af288ba5f8c63` |
| `docs/conformance/fcbc/` tree（11 files） | `1cedf8f6830a2d36a1839c78c9c53c5658bd01d88f1ef44968b35836185ef9c3` |
| `docs/conformance/` tree（92 files） | `6b40d46536ec1cc7c5b6d4642eae17c8547e8696fca9f95d1f981899dbde4090` |

当前验证为 workspace Clippy `-D warnings` PASS、Execution ABI 6/6、manifest 2/2、workspace
nextest 141/141、rustfmt check 与 `git diff --check` PASS。未参与 artifact 修改的只读 reviewer 复跑
定向 gate 并检查 independence、bytes/hash/CRC、canonical/reachability、bits/trace/direct-seek 和
mutation category，finding ledger 为 Critical 0、Important 0、Minor 0。

因此第 7.2 项关闭；第 7.1 Conversion、第 7.3 Render 和第 7.4 Core 仍开放。FCBC Container 2.0.0、
Execution ABI 1.0.0 以及其他三个版本域继续保持 Draft，直到剩余 blocker 全部关闭并完成最终联合
独立复审。该 test-only harness 不构成 I7 产品实现。

## 11. 2026-07-16 dated amendment：Render normative closure

本 amendment 只记录第 7.3 项的规范文字前置 gate，不静默改写 2026-07-15 的历史 hash、test count
或当时结论。Render binary/raster 初始只读审计发现 1 个 Critical、6 个 Important 规范空白：viewport
未编码、typed Record layout不完整、stable ID/order/ownership不唯一、descriptor direct root不完整、
image/resource decode与sampling不唯一、font/shaping不唯一，以及 semantic/raster/diagnostic precedence
不唯一。

`docs/reviews/2026-07-16-render1-binary-raster-closure-review.md` 记录了 `REN-C01`、`REN-I02–I07`
的逐项 closure。最终固定规范快照为：

| 文件 | SHA-256 |
|---|---|
| `fcs-render.md` | `95685f44fae88c26126e4dc34a13793499fd65b6eb61b021ca1f56156470cad1` |
| `fcbc.md` | `fc1bc9b8032d7ac88d16068e08cb3d8907a25b83fc752db85a7679e1ebed1c33` |

未参与修改的 reviewer 在首尾 hash 一致的只读复审中复核 RenderSection 与全部 nested Record长度、
root-only attachment、isolate、ownership/collision、descriptor roots、PNG/WebP/font、shaping、sampling、
viewport/output、RadialGradient退化和 stable diagnostics，finding ledger 为 Critical 0、Important 0、
Minor 0。

这只证明第 7.3 项不再要求未来实现填补 prose 选择；它没有生成或验证 static FCBC、independent
loader/evaluator、decoder/shaper、semantic/raster expected 或 mutation corpus。因此第 7.3 项仍开放，
Render Profile 1.0.0 和其他四个版本域仍为 Draft。下一 Render gate 是在上述固定规范上实现并独立
复审 executable binary/decoder/shaping/raster artifact。

## 12. 2026-07-16 dated amendment：Render normative gate reopened

在实现第 7.3 项 executable vector 时，新一轮只读审计发现 `REN-I08–I10` 三个 Important finding：

- Arc/EllipseArc 在此前当前点不同于参数起点时缺少连接语义；
- Core Line/Note descriptor root 缺少完整 exact path/owner/type/domain/environment matrix；
- Paint/Stroke/Clip/composite 非法值，以及 descriptor evaluator failure 与 owner-invalid value，缺少
  稳定、互斥的 parent category。

用户已确认采用概念性连接 LineTo 和 owner-specific diagnostic 分层；`fcbc.md` 的 16-entry Core
descriptor root matrix 由现有 schema/canonical traversal 补齐。规范 delta 与当前复审入口见
`docs/reviews/2026-07-16-render1-normative-amendment-review.md`。第 11 节记录的旧 hash、10-entry
Render category count 和 0-finding ledger 均继续作为当时快照的历史事实，但不再证明当前规范文字
gate 闭合。

因此第 7.3 项仍开放，且下一顺序恢复为：先对 REN-I08–I10 的新规范 bytes完成独立只读复审，再
继续 static FCBC、independent loader/evaluator、decoder/shaper、semantic/raster 和 mutation artifact。
五个版本域均保持 Draft；该重开不回退已经独立复审关闭的第 7.2 Execution ABI artifact。

## 13. 2026-07-16 dated amendment：Render/Core follow-up closure delta

重新审计又发现并已写入候选规范的 `REN-I11–I16`：Text GlyphRun size 与 Core `>0` domain 矛盾、
Note visible interval 到 visibility descriptor 的 lowering 缺失、EnvP 的 Piece context 缺失、Path
open-subpath fill closure 缺失、RadialGradient 负 radius 缺失，以及 Note/Line attachment 的
transitive Core query error category 缺失。相应修订同时涉及 `fcs.md`、`fcbc.md` 与 `fcs-render.md`，
因此当前候选 hash 必须重新计算，旧第 11/12 节 hash 只保留历史审计事实。

当前治理结论不变：五个版本域全部保持 Draft；在新 hash 上完成未参与修改 reviewer 的全量复审、
Render executable binary/semantic/raster/mutation artifact、Conversion round-trip、Core fixture
validator 和最终联合复审前，不得启动 I1 或声称 Render prose closure。

## 14. 2026-07-16 dated amendment：Render fixed-snapshot review failed

第 13 节修订后的独立只读复审固定了以下输入：

| 文件 | SHA-256 |
|---|---|
| `fcs.md` | `37CFF422B6CFBA64E7A0E5541A366A7E93D2E89A2EB18F069186AB28452D41F5` |
| `fcbc.md` | `BDC9C19D4C4F4C7EDB2A57A38A45B91244F9E07D24B168CB9ED6FC92CB5708BB` |
| `fcs-render.md` | `B066E7F403B9B04761261AB254C46A39462700DA9E221D1C42C76001D4357FC8` |

结果为 **FAIL**：Critical 2、Important 8、Minor 0。Critical finding 是 `line.scrollTempo` 的
`q` dependency 自循环和 inactive Node query order 矛盾；Important finding 涉及 Render/FCBC
diagnostic ownership、generic/profile validation precedence、TrueType limit、viewport/Layer/Node parent
category、glyph 0、attachment matrix/style、零长度 stroke segment，以及 shared descriptor 的完整
owner/environment intersection。reviewer 摘要中的 Important 7 是计数笔误；逐项 `RNR-I01`–`RNR-I08`
共 8 项。完整 ledger 与用户已确认的 disposition 见
`docs/reviews/2026-07-16-render1-normative-amendment-review.md` 第 8 节。

因此 Render normative gate 继续开放，且旧第 11–13 节的任何“closure delta”都不能单独证明当前
candidate 已闭合。该失败不回退第 7.2 项 Execution ABI 非空 artifact，但在新的规范、fixture、实现
修订和独立复审关闭全部 `RNR-*` finding 前，不得把 Render 相关阶段标记 Reviewed/Frozen。

## 15. 2026-07-16 dated amendment：stage-scoped implementation baseline

第 9、13 节中“全部五域重新 Frozen 前不得启动 I1”的文字是其写入时的治理结论。用户随后接受
ADR 0010，明确区分阶段实现输入与完整发布冻结：I1–I9 只需其完整 normative dependency closure
建立 Reviewed Implementation Baseline；I10 conformance RC 仍要求五个版本域全部 Frozen、所有
executable blocker 与最终联合独立复审关闭。

该变更不把本文、第 7.2 项 artifact、S14 grammar review 或任何 Draft 条款提升为 Reviewed/Frozen，
也不关闭第 7.1、7.3、7.4 项。I1 只有在 source syntax/AST/parser/diagnostic/limit 及其实际读取的
profile envelope 范围固定 hash、绑定 fixture、独立复审无 Critical/Important、计划一致且 I0 质量门
通过后才能自动开始。Render raster、Conversion round-trip 和 Core canonical execution 等域外
artifact 保留其后续 owner；若其中 finding 能改变 I1 的公开 AST/parser 行为，则必须重新纳入 I1
dependency closure。

## 16. 2026-07-16 dated amendment：RNR candidate remediation 与诚实 manifest

第 14 节失败后，`RNR-C01`–`RNR-I08` 已在新的候选规范中逐项得到单值 disposition；这只表示
resolved-by-candidate，尚未独立关闭。当前固定候选输入为：

| 文件 | Candidate SHA-256 |
|---|---|
| `fcs.md` | `2A2882E60AEEF4D96FDB9F7C3CD65B143EA9D9C61F971ECE24A8B2791627DC58` |
| `fcbc.md` | `245E1E41EBB20C94BAF644D284994820487C3CC2DF7C75463F37C3DB04B6F99A` |
| `fcs-render.md` | `848D9AB581529179FC91047737B846A04EE0469AFFAAB43377F47C66C20DB4C3` |

Render manifest 已撤下不存在的 nonempty binary/vector/mutation/semantic/raster paths，当前
`binary_fixture` 为 0；项目自制 PNG/WebP/TTF、test-only writer/independent loader、restricted
decoder/shaper 和 focused CRC-aware mutation仍保留。完整 Render executable artifact 明确归 I9，
不再作为 I1 的全局前置门。`image`/`serde_json` 的提前激活只是 dev-only conformance 例外，不改变
I3/I6/I9 产品 ownership。

另一个已知残留是历史 nonempty ABI golden 的 `note.presentation.visibility` expression 仍读取 EnvB，
而当前规范要求该 root 只依赖 `s`。当前 test-only loader 为保留已复审 bytes 明确不把该单一特例
当作完整 owner validator；I7 前必须重建 golden/vector/mutations 并独立复审。该残留不改变 I1
source lexer/AST/parser/parse-stage diagnostic contract。

本候选上 Clippy、focused 16-test lane、全 workspace nextest 149/149、rustfmt、diff、UTF-8/NUL、
本地 Markdown link 与 dependency topology audit 均通过。新的独立 reviewer 仍须固定上述 hash，复核
全部 RNR finding 及其 I1 dependency-closure 影响；在该记录完成前，第 7.3 项和 Render normative gate
仍开放，五个版本域保持 Draft。

随后未参与修订的只读 reviewer 复算上述三个 hash 并逐项复核 `RNR-C01`–`RNR-I08`，结果为
Critical 0、Important 0、Minor 0；Render normative RNR gate 由
`docs/reviews/2026-07-16-render1-normative-amendment-review.md` 第 11 节关闭。Reviewer 同时确认 legacy
visibility EnvB golden 仍是 I7/RC artifact blocker，完整 Render semantic/raster/mutation artifact 仍是
I9 blocker，二者均不改变 I1 source parser contract。第 7.3 项的完整 executable blocker和五域 Draft
状态不因此关闭或提升。
