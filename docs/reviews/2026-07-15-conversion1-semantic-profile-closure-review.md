# Conversion Specification 1.0 Semantic Profile Closure Review

日期：2026-07-15

状态：semantic profile/report delta 已写入并通过 S15 联合候选自检；等待真实 source/target
round-trip fixture 与独立复审，尚未 Reviewed/Frozen

## 1. 范围

本轮把 ADR 0007 与 `docs/community/` 的 PGR v1/v3、RPE、PEC evidence baseline 写回
`fcs-conversion.md`，关闭旧候选中把 parser 容错、语义解释、Repair 和 exporter approximation 混在
一起的问题。主要范围为：

- 固定 source bytes/package→lossless parse→dialect evidence→profile selection→source semantic IR→
  FCS canonical lowering；
- 分离 `syntaxMode`、`profileSelectionMode`、`SemanticProfileRef/ProfileBinding`、format/producer/runtime
  version、Repair mode 和 target CapabilitySet；
- 为 source/target profile 固定 ID、exact SemVer、descriptor content SHA-256、typed parameters 与
  selection evidence；
- 固定 strict/compatible 自动选择规则、canonical-equivalent representative、configured default
  报告和 Repair 不得替用户消歧；
- 重写 PGR/RPE/PEC 时间、坐标、offset、speed/distance、layer/default/Hold/package mapping；
- 删除默认 `pec.time.tick2048`，拆分 PEC Note X 与 Line X；
- 把 approximation 限定为“目标能力不足 + 用户显式授权 + error budget + target reparse”，标准
  FCS→FCBC 和播放器 sampled cache 均不使用该路径；
- 删除 FCBC Fidelity 中 raw snapshot 能力，只允许 structured non-source facts；完整 source
  preservation 只留在 authoring workspace/外部 sidecar。

本轮没有实现 PGR/RPE/PEC parser、converter、target writer、FCBC report codec 或 canonical
comparison engine。

## 2. Profile selection contract

自动选择只允许：

1. package/manifest 声明 exact profile；
2. direct evidence 唯一确定；
3. 所有剩余 candidate 对当前输入的 timing/gameplay/motion/scroll/presentation/resource/metadata
   canonical semantics 经证明相同。

其他情况：

- strict profile selection 使用 `conversion.ambiguous-source-semantics` 失败；
- compatible profile selection 只能使用调用方预配置 default，并报告 candidates/chosen/reason/impact；
- Repair 只修改非法/矛盾 source，不能选择两个都合法但不同的解释；
- external strict target 必须显式选择 target profile；不存在 generic PGR/RPE/PEC。

Parser 接受兼容 JSON/token 形状不授权 compatible semantic selection；这两个 mode 已在规范和
selection vector 中作为独立字段绑定。

## 3. 内建 Profile registry

| 格式 | Profile | Class / strict |
|---|---|---|
| PGR | `pgr.phira.v1`、`pgr.phira.v3` | semantic / eligible |
| PGR | `pgr.phichain-import.v1`、`pgr.phichain-import.v3` | characterization / not eligible |
| RPE | `rpe.community.divide-bpmfactor` | evidence / eligible；speedMode 参数 |
| RPE | `rpe.docs-example.multiply-bpmfactor` | evidence / eligible；speedMode 参数 |
| RPE | `rpe.phira.legacy-speed`、`rpe.phira.rpe170-speed` | semantic / eligible |
| RPE | `rpe.phichain-import` | characterization / not eligible；first-layer/presentation loss |
| PEC | `pec.phira` | semantic / eligible |
| PEC | `pec.extends`、`pec.phispler` | characterization / not eligible |

Registry 不定义永久默认。每个 descriptor 是独立 UTF-8 TOML 文件；registry 保存其原始 bytes
SHA-256。Parser dialect 与 mapping rule 使用 `SHA-256(UTF-8(contract))`，使 rule/dialect 的
ID/version 不能在内容变化后静默复用。

## 4. 格式闭合结果

### 4.1 PGR

- `sourceLineBeat=T/32` 只作为来源坐标，不再误称 canonical chartBeat；
- Phira profile 使用每 Line BPM，Phichain-import characterization 使用第一 Line BPM；
- v1 精确分为 trunc/520 和 ties-away-round/530；v3 使用 split normalized XY；
- Note X 与 Line coordinate 分离；Phira 使用 108px/unit，Phichain-import characterization 使用
  `1920/18=320/3` px/unit；
- raw `floorPosition` 是 cache validation point，strict scroll presentation 需要显式 `floorScalePx`；
- Phira Hold head=1/tail-from-distance 与 Phichain Hold-speed normalization 分 profile；
- per-Line profile 在 Line BPM 不同、又无可信 global tempo declaration 时生成 `0beat→60bpm` 的
  neutral canonical anchor并报告；first-Line characterization 则使用第一 Line BPM。该选择不改变
  已解码 chartTime，列为独立复审重点。

### 4.2 RPE

- Beat strict rule 要求 denominator>0；`[a,0,0]→a` 仅属于 Phichain compatibility profile；
- `bpmfactor` 明确分为 divide/multiply/ignore，且作用于该 Line 的 Note、Hold、普通 event、speed
  等全部 Beat-timed runtime value，不再声称“只影响 scroll”；
- event layers 的合法语义是 additive；first-only 是带 mandatory loss report 的 characterization；
- `rotateWithFather` 缺失 false/true 分 profile；
- speed 分 legacy-player、pre-170 derivative、170+ direct-eased，并同时约束瞬时 speed 与 distance；
- Phira profile 另固定 fake nonzero、side==1、visibleTime 秒边界、u16 alpha saturation、uniform size、
  `yOffset×speed` 与 package-root hitsound 七项 presentation/resource rule；
- Phichain fake/presentation/parent/layer 缺口不能 strict/lossless。

### 4.3 PEC

- 三份固定证据均使用 direct decimal Beat；`pec.time.tick2048` 从 registry 删除并作为 forbidden ID；
- offset 分 150ms/175ms；`cv` 分 `/5.85`、`/7` 和 RPE-height exact chain；
- Note X=`x*1920/2048`，Line X=`(x/2048-0.5)*1920`，不再共用错误 rule；
- line-command、global token-stream 和 global suffix zip 是 parser dialect，不是 semantic/Repair mode；
- overlap clipping、从 0 猜 interpolation、补首 speed、未知 easing→linear 和整数步采样分别归入
  Repair 或显式 target approximation；
- Phira negative alpha 使用 typed `negativeAlphaExtension` parameter 绑定已注册 required runtime
  extension；缺失/非法 binding 使用 `conversion.profile-parameter-invalid`，其他 profile 不静默 clamp。

## 5. ConversionReport、Fidelity 与 FCBC

`ConversionReport` 现在必须保存 source/target descriptor、InterpretationDecision、candidate/chosen
profile、evidence/reason/ambiguity impact、rule refs、Repair/approximation/drop authorization、ordered
entries、aggregation status 和 output hash。

FCBC Fidelity 的标准 object 只保存 restricted SourceDescriptor、profile binding、entity mapping、
必要的 field fact、mapping rule 和 semantic loss。以下在 runtime/fidelity/strict-runtime profile 中
全部禁止：source text/bytes/AST/object tree、exhaustive field dump、comment/template/generator/local、
workspace path、resource payload副本、BakedCurve 和 player sampled cache。FCBC 仍恰好一个 chart，
资源只通过 Resources/ResourceData 原样内嵌。

## 6. Conformance 候选

```text
semantic profiles: 12
parser dialects: 7
mapping rules: 56
diagnostic/report categories: 32
exact mapping vectors: 38
invalid mapping vectors: 5
selection/ambiguity vectors: 10
conversion tree files: 19
```

Selection vectors至少绑定：PGR v1 formatVersion 不足、PGR v3 当前输入 canonical-equivalent、RPE
explicit/ambiguous/configured-default/unit-factor-equivalent、Repair 不得消歧、PEC ambiguity/package
declaration和 strict external target profile required。

Rust `conformance_manifest` test 强类型加载 umbrella/registry/vector，复算所有 profile raw-file hash
和 dialect/rule contract hash，验证 path containment、ID/version/direction/class、profile→dialect/rule、
vector→rule 和 selection→profile 引用。它不执行公式或 converter semantics。

## 7. 候选内容哈希

单文件按原始 bytes；tree 使用 ordinal relative path order、
`UTF-8(path) + NUL + bytes + NUL`：

```text
fcs-conversion.md
7f8156af94858e25d453a8b25b7f5af3f9cd58f1651cbc235d7115cc7f4e0d72

conformance/manifest.toml
1378a680d7a593f4156f45646ba263641b5e8198d90f2c79ce3407e9e37eb4ac

conformance/conversion/manifest.toml
28cd000589cf2b3d547abaed1f57bd2453267b511b535c3af8375527aae6c7b2

conformance/conversion/profile-registry.toml
7d33f47b5f2722ecd8e1db510d86b4c285a05c6016e6268044e4ef80805c1b52

conformance/conversion/parser-dialects.toml
46aeb45232719b34b2f0f51deaf46a9cb5543315d5784bf43a47cee353d46845

conformance/conversion/mapping-rules.toml
47ad0c81c44de3968e4959b8e74ed9f51a1d380773159c24ea2ce9eda856711b

conformance/conversion/diagnostic-categories.toml
6b216576778e0bf00ffc8c4562f4d22f75ba6771e2228b48c0c9190998f8fd8f

conformance/conversion/mapping-vectors.toml
a65d3eae8f710cb79b667ef205ebddbf8d91fba456084ac8b358294c08844831

conformance/conversion/selection-vectors.toml
d36eb78c7246df77c49918226858cb33f908ec03c9b97791d08fd78e1a724b25

conformance/conversion tree (19 files)
0d55f3eca638e2188fa8fded1ad1a1afc1865ce0c80f08488aeaf2d69e395b52

conformance tree at this stage (88 files)
a5d4071a61338fcb5990c4fa804b06e6a432c8702ad0f8b651820abb419f1a25
```

## 8. 已完成自检与剩余 gate

已完成：

- fixed reference/community evidence 与 profile contract 对账；
- TOML schema/hash/cross-reference 强类型加载；
- `cargo fmt --all`；
- `cargo clippy --workspace --all-targets -- -D warnings`；
- `cargo nextest run -p fcs-source --test conformance_manifest`：2/2 passed；
- Conversion stale term audit：旧 rawSnapshots/archive profile/default tick2048/scroll-only bpmfactor 已删除；
- 四规范术语、profile/diagnostic/resource/Value/hash 联合候选审计及完整 workspace gate，最终证据见
  `2026-07-15-fcs5-cross-spec-closure-review.md`。

仍需：

- 真实、可公开 PGR/RPE/PEC source fixture 与 exact parsed-source/canonical golden；
- 每个 profile 的 runtime probe、package/resource fixture 和 target reparse round-trip；
- capability/approximation/error-budget executable fixture；
- FCBC Fidelity/ConversionReport nonempty byte vector；
- 独立 reviewer 关闭 Critical/Important finding。

因此 Conversion Specification 1.0.0 保持 Draft。本 review 不授权开始 I1；最终 gate 由
`docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md` 与治理文件统一记录。
