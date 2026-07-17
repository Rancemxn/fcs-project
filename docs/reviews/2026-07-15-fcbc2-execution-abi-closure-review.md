# FCBC 2.0 / Execution ABI 1.0 Closure Review

日期：2026-07-15

状态：FCBC/ABI delta 已写入并完成候选自检；等待 Render/Conversion 同步、完整跨规范复审与独立
review，尚未 Reviewed/Frozen

## 1. 授权、范围和版本

用户接受 ADR 0008–0009，并确认 FCBC 是恰好一个谱面、内嵌全部资源的播放器最小输入；资源保留
workspace 输入的原始 bytes；FCBC 不保存 comment、template、generator、局部变量、FCS/external
source snapshot 或 source AST；标准发行保留 exact descriptor，播放器 sampled cache 不写入 FCBC。

本轮继续用一个 `fcbc.md` 联合定义 FCBC Container 2.0.0 与 Execution ABI 1.0.0。项目尚未公开，
兼容修改成本为零，因此候选 SemVer 不变，但 2026-07-14 的 804-byte golden 与 Frozen hash 只作
历史证据。Container framing、Resources/ResourceData、Meta、Note、Distance 和 profile contract
发生了实际变化；Expression opcode 1–70、lazy And/Or/Choose、binary64 register type 与 descriptor
kind 1–4 的执行语义保持不变。

## 2. 章节级 delta ledger

| 章节 | 旧候选问题 | 当前候选决定 |
|---|---|---|
| 1–3 | FCBC 可依赖外层包；editable/archive 暗示 authoring backup | FCBC 自身是 one-chart/self-contained input；profile 改为 runtime/fidelity/strict-runtime，value 2 reserved；没有压缩、加密或 source-bearing profile |
| 3.2、5 | HAS_SOURCE_SNAPSHOT/USES_ADAPTIVE_BAKED 与 SourceSnapshot section 仍存在 | bit 5 改为 HAS_DISTRIBUTION_METADATA，bit 7 reserved；section 18 改为 DistributionMetadata；新增 required ResourceData section 20 |
| 4–8 | known section 重复规则含糊；StringTable 禁止 Core 合法 U+0000；ConstantPool 合并 signed zero | FCBC 2 known section 全部 singleton；StringTable slice 是 length-delimited UTF-8，可含 U+0000；`+0/-0` bits 分别保留；固定 stable-ID namespace |
| 9.1 | canonical Artwork 没有编码位置 | Meta payload 增加 `artwork:Value(object)`；没有声明时是空 object |
| 9.4–9.5 | Resources 保存 package-member source，容器不内嵌 bytes | ResourceRecord 删除 source/locator，增加 ResourceData-relative offset/length；ResourceData 按 ID、8-byte alignment、最小零 padding 保存全部原始 bytes，并同时验证 section CRC 与逐资源 SHA-256 |
| 12 | NoteRecord 缺 custom score extension identity 和 disabled normalization | 增加 `scoreExtensionNamespace`；完整绑定 judge shape、sound/score policy、disabled judgment、audio/image/texture resource type |
| 13 | 普通 profile 接受 BakedCurve | descriptor kind 5 保留但所有 FCBC 2 profile 必须以 `fcbc.forbidden-descriptor` 拒绝；播放器 sampled cache/显式 target approximation 都不进入 FCBC |
| 15 | exact integrand 与预采样 distance 没有清楚分离 | 定义 portable-analytic/portable-evaluable/runtime-only-extension；保存 exact integrand、analytic descriptor 或直接积分、boundary 与 ABI error contract；seek 不依赖 frame history |
| 16 | Fidelity/Debug/SourceSnapshot 可以携带 authoring 数据；extension payload 不是 Core typed object | Extension payload 改为 `Value(object)`；Fidelity、ConversionReport、DistributionMetadata、Debug 只能保存结构化非原文 fact，禁止任何编码形式的 source snapshot/AST/authoring graph |
| 17–20 | loader 不验证 resource coverage/hash；golden 仍是 13-section/804-byte | 先结构/CRC，再 ResourceData layout/coverage/hash，再 graph/type/profile；新增 resource limits、exact-only 和 14-section schema 2 golden/mutations |

## 3. ResourceData 唯一布局

Resources 按 stable resource ID 排序。对每个 record，writer 从上一 payload 末尾向上对齐到 8 bytes，
写最小全零 padding，再逐 byte 写 workspace 输入文件；最后一个 payload 后不写 trailing padding。
因此 loader 可以从 Resources 唯一重建期望 offset：任何未引用 byte、非零/额外 padding、range alias、
overlap、外部 locator 或 content dedup 都是无效 FCBC。

Section CRC 覆盖整个 ResourceData payload；ResourceRecord SHA-256 只覆盖自己的原始 payload。Loader
必须先通过 section CRC，再检查布局/coverage，最后核对逐资源 hash，且在完成前不能暴露部分 chart
或 resource slice。媒体 decode、转码和 codec 识别不属于 packager/hash 过程。

## 4. FCBC schema 2 conformance 候选

### 4.1 Golden

```text
minimal-runtime
decoded length: 864
file SHA-256: 504fbdb039b386854f7551dd5ea3edf6f324cadbaaabdb141594efe4ecc7fb19
Resources count: 0
ResourceData: offset 864, length 0, CRC32 00000000

embedded-resource
decoded length: 1021
file SHA-256: a0056fdbd19c05918d8999e89f86c6f59f749ba8d9e52ba00ace5a63a27eccde
resource textual ID: opaque
resource stable ID: ff390959caa06661
ResourceData: offset 992, length 29, CRC32 9afb5c84
resource SHA-256: 66eb55e69c42345c65021ea9364fc43c61d2151dde67a89dc02362543b289903
```

两个文件都声明 `chart_count=1`、exact descriptors only，并包含 section type 1–13 与 required
ResourceData 20。Rust manifest integrity test 解码 hex，核对 header、table、offset、length、stored
CRC、padding、coverage 和 payload bytes；它不是 FCBC loader/runtime implementation。

### 4.2 Mutation

- minimal-runtime：11 项，覆盖 magic、source/FCBC/ABI version、reserved profile、file length、
  alignment、checksum、unknown required、missing ResourceData 和 overlap；
- embedded-resource：2 项；两项都同步修正 Resources section CRC，确保分别到达逐资源 hash mismatch
  与 ResourceData trailing-byte invariant，而不是提前停在 section checksum。

Loader step 6 固定先报告 unknown REQUIRED，再报告 missing known required；step 9 固定先报告
bounds/layout/coverage，再计算 resource hash，使 mutation 的 stable category 唯一。

## 5. 候选内容哈希

单文件按原始 bytes；tree 使用 ordinal relative path order、`UTF-8(path) + NUL + bytes + NUL`：

```text
fcbc.md
957cc9f10702756ea5589c785237528daefd392b038b97db75383536054ad4d6

docs/conformance/manifest.toml
73c7db2974ae0bf1144b792a2ea57457a6d523cfe23029eac57a80660cd5d76e

docs/conformance/fcbc/manifest.toml
86758355543f59767181be50f329368333eb6ef8436f7668aa138fe2ac1e7b31

docs/conformance/fcbc tree (7 files)
550d26d1683a753e1c1ba430fae7dc8ef8772961c165fac9f94eb7a34398c7dd

conformance tree (70 files; includes the subsequent Render binding fixture)
569a417ff3dd5f9c57fdee6820dd313cd1ac9e3b8bd1427ecb9ccb970483a716
```

## 6. 已完成自检与限制

已完成：

- table-driven CRC 与 SHA/section parser 独立复算两个 golden；
- stable resource ID、resource payload/hash 和两项 post-CRC mutation 条件复算；
- FCBC schema 2 强类型加载、路径/hex/table/patch integrity test；
- Clippy 与 targeted nextest manifest test。

尚未完成、不能由本 review 冒充完成：

- RenderSection 中所有 image/font/texture/path/shader ID 到 ResourceData 的绑定；
- Conversion Fidelity/ConversionReport 的最终 schema/profile registry；
- Note/Track/Expression/Distance 的非空 execution golden 和 reference evaluator vector；
- runtime/fidelity/strict-runtime 全 profile byte golden；
- 活动 Rust FCBC writer、loader 或 ABI evaluator（仍属于 I7）；
- 跨四规范 review、独立 reviewer 和 Frozen 状态。

因此当前 FCBC/ABI 仍为 Draft。下一依赖节点是 `fcs-render.md` resource binding；S15 全部完成、
统一 docs/conformance/hash 复算且独立 review 关闭 Critical/Important 前，不开始 I1 Rust 实现。
