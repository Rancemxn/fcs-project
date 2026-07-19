# FCS 5 规范与参考实现路线图

## 1. 目标

本路线图取代此前按日期拆分的 FCS 5 计划。目标是先闭合每个阶段可独立实现的规范输入，以
Reviewed Implementation Baseline 驱动 Rust 参考实现，再在完整 executable conformance 上冻结五个
版本域，最终完成 FCS、FCBC、播放器接口、转换器、Render Profile 与 CLI 的同一条确定性工具链。

## 2. 全局规则

- 规范性语义只能写入根目录四份权威规范；计划和 decision record 不能覆盖它们。
- FCS 5 尚未公开，不为未发布的临时语法保留兼容别名。
- 每项行为先有规范条款和失败 fixture，再实现，再通过质量门。
- 编译期能力不能进入 FCBC；FCBC Core 不包含 jump、loop、recursion、mutable local、
  `template`、`generate` 或 `emit`。
- 运行时只有一个物理主时钟；判定时间与滚动坐标分离，但不建立 line-local 物理时钟。
- 不静默修复非法谱面；可选 repair 必须生成机器可读记录。
- I0-A 用 `archive/fcs4-pre-cutover` 保存完整旧工具链；I0 完成后活动 `main` 只保留一套
  无版本实现前缀的 FCS source API，不建立 FCS 4 兼容层。
- 每个实施阶段结束前依次运行 Clippy、nextest、rustfmt check 和 diff check。
- generator 实现持续以权威编译期语言为准；I0 的 parser/feature-unavailable 边界不反向定义语法。
- I1–I9 使用 ADR 0010 的阶段范围化 Reviewed Implementation Baseline：完整 normative dependency
  closure、绑定 fixture/hash、独立复审无未关闭 Critical/Important、阶段计划一致且前置质量门通过。
  条件满足后自动进入下一阶段，无需逐阶段取得用户确认；该 baseline 不改变版本状态。
- I10 conformance RC 仍要求五个版本域全部 Frozen、最终联合独立复审和完整 executable
  conformance 通过。

## 3. 版本基线

| 组件 | 候选版本 | 冻结条件 |
|---|---:|---|
| FCS Core | 5.0.0 | Core 全章节 Reviewed，source/canonical fixtures 可执行 |
| FCBC | 2.0.0 | 二进制布局、loader 规则和 golden files 完成 |
| Execution ABI | 1.0.0 | descriptor/evaluator test vectors 完成 |
| Render Profile | 1.0.0 | semantic 与 reference raster fixtures 完成 |
| Conversion Specification | 1.0.0 | PGR/RPE/PEC mapping 与 loss report fixtures 完成 |

五个版本曾于 2026-07-14 标记 Frozen。2026-07-15 用户确认尚未公开且兼容成本为零后，首先因
Source grammar 未闭合而重开 FCS Core 5.0.0 与 Render Profile 1.0.0，并完成 S14 Reviewed
证据。随后用户接受 ADR 0007–0009 以及 `docs/community/` evidence baseline，确认显式版本化
conversion profile、FCS authoring source、自包含单谱面 FCBC、内嵌原始资源和 exact-first runtime
expression 边界；因此 Core、FCBC、ABI、Render 与 Conversion 当前均按
`docs/specifications/governance.md` 进入 Draft/联合重审状态。

历史冻结记录见 `docs/reviews/2026-07-14-fcs5-freeze-review.md`；S14 grammar closure 的 Reviewed
hash 与独立复审范围见 `docs/reviews/2026-07-15-fcs5-source-grammar-closure-review.md`。这些记录只
证明各自审查时点和范围，不覆盖当前状态。当前绑定 corpus 入口为 `docs/conformance/manifest.toml`。

## 4. Part A：规范冻结

### S0：治理、术语和迁移索引

交付：

- `docs/specifications/governance.md`；
- 本路线图；
- 唯一权威文档列表；
- 版本状态表；
- 旧设计章节到新规范章节的迁移检查。

完成条件：设计稿不再新增语义；每个已确认决策有且只有一个规范目标章节。

### S1：文档、词法、版本和 profile

交付：FCS 文件头、UTF-8、token、literal、注释、顶级块、重复/顺序规则、profile、
版本兼容和 source span 规范。

验证：header/profile 合法、非法和未来版本 fixture；BOM、UTF-8、换行与重复块边界。

### S2：类型、单位和表达式

交付：完整类型集合、literal 映射、operator matrix、显式转换、内建函数、字段访问、
Float64/Beat 规则、静态与运行时表达式边界。

验证：每个运算符的合法与非法类型组合；非有限值、除零、精确 beat 和 Unicode span。

### S3：编译期语言

交付：`const`、`let`、纯 `fn`、typed template、constructor、`with`、statement `if`、
`generate`、range、`emit`、作用域、环检测、六类 budget 和 expansion trace。

必须关闭的现有分歧：

- 半开 range 只使用 `..<`，裸 `..` 为语法错误；
- template body 与 generator body 支持显式类型的局部 `let`；
- template 允许编译期 statement `if`；
- generator 当前值按 `start + index * step` 精确计算；
- budget 在单次 elaboration 中共享并在工作开始前计数。

验证：展开后不存在任何编译期节点；generator/template/function cycle 与每类预算均有 fixture。

### S4：时间和同步

交付：唯一 `chartTime`、全局 `chartBeat`、tempo map、beat/time 双向映射、audio offset、
seeking、负时间、判定时间和 `lineScrollCoordinate`。

验证：边界 BPM step、offset 符号、seek 与顺序播放一致；不存在第二物理时钟。

### S5：Track、滚动、speed 和 distance

交付：`Track<T>`、半开 segment、point/step、blend/priority、fill/extrapolation、插值、
scroll tempo、speed multiplier、floor scale、积分域和误差。

验证：重叠、缺口、零时长、反向滚动、积分连续性和 frame-rate independence。

### S6：坐标、变换和 line graph

交付：1920×1080 logical space、world/line/note/scroll space、矩阵顺序、pivot、anchor、
父线 DAG、inherit、排序与 stable ID。

验证：矩阵 test vectors、父线环、未知父线、不同声明顺序的确定性。

### S7：Note gameplay 与 presentation

交付：Note identity、tap/hold/flick/drag、判定、side、judge shape、Hold 边界、呈现属性、
fake 的替代模型、稳定排序和动态值限制。

验证：Hold 约束、不可见但可判定、可见但不可判定、presentation 不反向改变 gameplay。

### S8：Runtime expression 和数值精度

交付：`s/b/q/d/p`、`choose`、typed DAG、piecewise、exact-first lowering、强制精确边界、
显式 target approximation 的属性级误差指标、Float64 和资源预算。

验证：无通用控制流、无循环依赖、标准 compile 不产生 BakedCurve、显式 approximation error、
1ms 最大验证间隔不是固定输出采样率。

### S9：Metadata、credits、resources 和 sync

交付：meta、contributors、credits、custom role、workspace resource manifest、artwork、audio、font、
logical member path、opaque bytes/hash、typed custom data、profile 最低要求和 FCBC 保留规则。

验证：自由角色、引用完整性、offset 公式、runtime/fidelity/strict-runtime 保留矩阵，以及所有
profile 都没有 source snapshot/authoring-only 数据。

### S10：Fidelity 和 Conversion

交付：namespace/schema、来源状态、语义状态、authoring-workspace source preservation 边界、
versioned parser/profile、repair、capabilities、ConversionReport、semantic/roundtrip/strict 策略和
PGR/RPE/PEC mapping。

验证：每个丢失、近似和仅保留行为均机器可读；canonical semantic round-trip fixture。

### S11：FCBC 2.0 与 Execution ABI 1.0

交付：header、section table、primitive encoding、section schema、descriptor、profile、hash、
checksum、loader validation、deterministic ordering 和 golden bytes。

验证：第三方仅按文档可实现 reader；未知 optional/required section；损坏文件 corpus。

### S12：Render Profile 1.0

交付：retained scene graph、node/path/paint/stroke/clip/image/text、attachment、compositing、
RenderSection、FCBC stable resource ID→ResourceData binding、semantic conformance 和 reference raster
conformance。

验证：固定内嵌资源与 glyph、无 workspace/URL/system-font fallback、exact descriptor only、无 Canvas
mutable state、无 pixel IO、render 不影响 gameplay。

### S13：全规范审查和冻结

任务：

1. 搜索并消除 `TODO`、`TBD`、“有待商榷”和未定义核心行为；
2. 检查四份规范的术语、单位、版本和交叉引用；
3. 为所有 normative example 建立 fixture 清单；
4. 独立审查 time/scroll、Track、Note、FCBC 和 conversion mapping；
5. 将各规范状态从 Draft 改为 Frozen；
6. 发布 conformance manifest。

完成条件：实施代码不需要再做语言或容器设计，只需实现规范。

### S14：FCS 5.0 Source grammar closure 与重新冻结

2026-07-15 在准备 I1 时发现 Appendix B 引用了未定义顶级 production，closed enum 与 keyword
grammar 冲突，extension/preserve/render envelope 不闭合，并且 parser/static diagnostic ownership
不明确。用户确认格式尚未公开且兼容修改成本为零，因此在原 5.0.0/Render 1.0.0 候选版本上
修正，不保留临时语法兼容层。

交付：

- 闭合 Core lexical、literal、format、metadata/resource/sync/tempo、extension、preserve 和完整
  Appendix B EBNF；
- closed enum 统一为 string literal，移除 mixed-Beat 临时语法；
- Core 只平衡保存 Render payload，`fcs-render.md` 定义完整内部 source grammar；
- 明确 decode/parse/source-structure/static/canonical/evaluate ownership；
- 将 conformance baseline 扩展到 32 个 FCS fixture，并增加 grammar closure 边界；
- 独立复审后把 FCS Core 5.0.0 与 Render Profile 1.0.0 重新标记 Frozen。

完成条件：规范 grammar 无未定义 nonterminal，全部 source 示例与 fixture 使用同一语法，
实现无需猜测 enum、extension payload、generator diagnostic stage 或 tempo validation phase。

### S15：Authoring、Canonical 与分发边界闭合

2026-07-15 用户接受 ADR 0007–0009，确认 FCS 是 workspace authoring source、FCBC 是单谱面
自包含分发物、标准运行时保留 exact expression，baking 只属于显式 target approximation，播放器
sampled cache 不进入格式。S15 按依赖顺序回写四份规范：

1. FCS Core：SourceDocument→ExpandedSourceDocument→CanonicalCompilation、workspace resource、
   Note policy、唯一 chartTime、exact DAG、preserve/source snapshot 消除；
2. FCBC/ABI：one-chart、ResourceData、原始 payload、无外部 package、无 SourceSnapshot、exact
   descriptor 与 loader/golden；
3. Render：resource ID 最终解析到 FCBC embedded payload；
4. Conversion：versioned source/target profile、ambiguity evidence、PGR/RPE/PEC mapping 与 report；
5. 更新 conformance、cross-spec review 和五版本域 hash，再独立复审并重新 Frozen。

Core、FCBC/ABI、Render 与 Conversion 的候选 delta 已依序写入；阶段范围分别见
`docs/reviews/2026-07-15-fcs5-authoring-canonical-closure-review.md`、
`docs/reviews/2026-07-15-fcbc2-execution-abi-closure-review.md`、
`docs/reviews/2026-07-15-render1-resource-binding-closure-review.md` 和
`docs/reviews/2026-07-15-conversion1-semantic-profile-closure-review.md`。Conversion corpus 当前绑定
12 个 profile、7 个 parser dialect、56 个 mapping rule、32 个 diagnostic/report category、38 个
exact vector、5 个 invalid vector 与 10 个 selection vector。非空 Execution ABI
writer→static bytes→independent loader/evaluator、bits/trace/direct-seek 与 mutation corpus 已完成并
通过独立复审，见 `docs/reviews/2026-07-16-fcbc2-execution-abi-nonempty-review.md`。RenderSection
layout、decoder/shaping、semantic/raster 和 diagnostic 规范文字曾独立复审，见
`docs/reviews/2026-07-16-render1-binary-raster-closure-review.md`；随后 executable-vector 审计发现
REN-I08–I16，修订与重新复审入口为
`docs/reviews/2026-07-16-render1-normative-amendment-review.md`。其后固定快照复审以 Critical 2、
Important 8 失败；先关闭 `RNR-C01`–`RNR-I08` 并恢复诚实的 pre-I1 Render manifest baseline。完整
RenderSection semantic/raster/mutation artifact 仍由 I9 交付，Conversion 真实 round-trip 与 Core
canonical fixture execution 由各自后续阶段交付，不再作为 I1 的全局前置门。I1 在 source parser
dependency closure 建立 Reviewed Implementation Baseline 且 I0 质量门通过后自动开始；五域完整
Frozen 保留给 I10 conformance RC。

当前 `fcs-source` 为保存上述规范闭合的 test-only 证据，提前在 `[dev-dependencies]` 激活了
`image` 和 `serde_json`。这是显式的 **pre-stage conformance test-only exception**：`image` 只用于
生成/解码项目自制 PNG、lossless WebP 与固定资源断言，`serde_json` 只保留给测试 artifact 数据；
二者都没有进入 `fcs-source` 产品 API。产品 dependency ownership 不变：human-readable canonical
snapshot/import report 仍由 I3/I6 激活，Render codec/raster 能力仍由 I9 的 owning crate 激活。该例外
不得被解释为 I3、I6 或 I9 已开始，也不得把 test-only writer/loader/decoder 当作产品实现。

## 5. Part B：参考实现

### I0：健康基线与规范对账

当前进度：I0.1–I0.9 已完成；活动树只有 `fcs-source`，Chumsky token parser、稳定诊断、
byte/property robustness、精确 generator parser 边界和强类型 manifest gate 已落地。
I0 的 135-test 历史证据与 test-only ABI/Render closure 的 175-test checkpoint 保留为审计事实；
I1.1–I1.8 已交付 source AST/parser、fixture runner、production ledger、parser limits、deterministic
properties 和独立 fuzz lane，当前 root workspace gate 为 218 tests，I1 Task 9 正在进行最终治理和独立复审。

- 提交完整切换前快照并建立永久 `archive/fcs4-pre-cutover`；
- fast-forward `main`，删除活动主线中的 FCS 4、旧 converter 和旧 CLI；
- 将 `fcs-core/src/v5` 提升为无版本前缀的独立 `fcs-source` crate；
- 用 Chumsky 0.11.2 + `stacker` 建立 token/span/parser 和结构化诊断基线；
- 建立 byte decode API 和固定 seed/case 的 Proptest parser robustness gate；
- 在 workspace catalog 预登记后续阶段依赖，但只允许当前 owning crate 激活；
- 保存并审计 generator 工作，严格拒绝裸 `..`，未展开时返回明确阶段诊断；
- 建立“条款—实现—测试”矩阵和强类型 conformance manifest gate。

完成条件：活动 workspace 只有 `fcs-source`；没有 FCS 4 或 `v5` 实现路径；所有已知偏差
进入矩阵；Clippy、nextest、rustfmt check 和 diff check 全绿。

### I1：完整 Source AST 与 parser

- 在 I0 的 Chumsky/token/diagnostic 基线上实现全部顶级块和 source node；
- 完成 Appendix B grammar、span、diagnostic category 和 parser limits；
- 加载所有 S1/S2 语法 fixture；
- parser 不做 semantic repair。

完成条件：所有合法 source 可解析，所有非法 source 产生确定类别和位置。

### I2：Static semantics 与编译期展开

- 完成类型检查、作用域、函数、template statement、generator 和 `emit`；
- 使用单个 elaboration budget context；
- 生成不含编译期结构的 `ExpandedSourceDocument`。

完成条件：S2/S3 conformance 全绿；当前 Phase 2 才可标记完成。

### I3：Canonical chart lowering

- 实现 canonical IDs、tempo、Line、Note、Track、parent graph、transform、scroll 和 distance；
- 执行跨字段、拓扑和区间验证；
- Phase 3 以后只消费 canonical model。

完成条件：source 宏、模板和 generator 不泄漏到 canonical/runtime 层。

### I4：Reference evaluator 与数值系统

- 实现 Track、DAG、piecewise、easing、exact integration descriptor 和 distance；
- 建立未复用生产优化路径的 reference evaluator；
- 验证 seek、误差、边界、标准路径无 BakedCurve 和跨平台确定性。

完成条件：运行结果与帧率无关，所有 portable 值满足规范误差。

### I5：Metadata、resources、sync 与 fidelity

- 实现 canonical metadata/credit/resource；
- 实现 workspace member resolver、opaque resource bytes/SHA-256 与 CanonicalResourceBundle；
- 实现 provenance、extension、repair 和 ConversionReport 数据结构；
- 建立 profile 保留矩阵。

完成条件：后续 converter/FCBC 不需要旁路字段保存规范数据。

### I6：PGR/RPE/PEC importer

- 将 PGR v1/v3、RPE、PEC 导入统一 canonical model；
- 保存原始路径、版本和值；
- 输出 loss/repair report；
- 用公开 fixture 和可选版权 fixture 验证。

完成条件：真实外部谱面能检验 canonical model，而非由 converter 自建第二语义模型。

### I7：FCBC writer、loader 与 ABI

- 实现所有 Core section、profile 和 deterministic writer；
- 实现 bounds/checksum/reference/feature validation；
- 完成 golden bytes、malformed corpus 和 evaluator 闭环。

完成条件：canonical → FCBC → execution 与 reference evaluator 一致。

### I8：FCS 和外部格式 exporter

- deterministic FCS formatter；
- PGR/RPE/PEC exporter；
- capability negotiation 和用户显式授权的 target approximation；
- semantic/roundtrip/strict 策略。

完成条件：所有不可逆行为进入 ConversionReport，round-trip 按 canonical semantics 比较。

### I9：Render Profile

- source schema、canonical scene graph、RenderSection；
- software reference renderer 和 raster fixtures；
- GPU backend interface，但不让 backend 行为成为规范。

完成条件：semantic 与 raster conformance 通过，Render 不影响 gameplay。

### I10：CLI、发行组合与 Conformance Release Candidate

- 重建只组合新领域 crate 的 CLI，使用 FCS 5/FCBC 2；
- 提供 check、format、compile、inspect、convert 和 report；
- 完成 JSON/text diagnostic、profile/resource/capability/budget 参数和稳定 exit category；
- 闭合 examples、normative fixtures 和公开 API 文档；
- 完成 workspace、fuzz、property 和版权 fixture 验证。

完成条件：CLI 只消费 `fcs-source`、`fcs-model`、`fcs-runtime`、`fcs-fcbc`、
`fcs-converter` 和 `fcs-render` 的规范边界；四份权威规范与工具输出版本一致。

## 6. 实现任务账本

以下 task ID 是后续 current-phase plan、commit 和交付报告的稳定引用。一个 task 只有规范条款、
失败测试、实现、review 和本阶段 gate 都完成后才可标记完成。

### I0：健康基线与规范对账

- **I0.1 快照与归档（已完成）**：提交当前 generator、当时 Frozen 的文档、conformance 和项目
  workflow 工作；创建指向精确切换前提交 `148936d17b671bb34968c88969ab748c818f9fc0` 的
  `archive/fcs4-pre-cutover`，然后 fast-forward `main`。后续 Trellis bookkeeping commits
  不移动归档指针。
- **I0.2 Generator staging（已完成）**：只接受 `..<`/`..=`，拒绝裸 `..`；在 I2 完成展开前
  返回临时 `FeatureUnavailable { feature: "compile-time-generator", .. }`；已消除
  non-exhaustive match 和部分输出路径。稳定 `implementation.feature-unavailable` 公共
  diagnostic code 仍属于后续诊断边界任务。
- **I0.3 唯一 crate 切换（已完成）**：删除活动 FCS 4 core、旧 CLI、旧 converter 和第二 IR；将候选
  source front end 提升为 `crates/fcs-source`，不提供兼容 re-export。
- **I0.4 诊断边界（已完成）**：建立稳定 `DiagnosticCode`、UTF-8 byte span、labels、确定性多诊断
  `ParseOutput<T>`；Chumsky error 不泄漏为 public API。
- **I0.5 Chumsky parser 基线（已完成）**：使用 0.11.2 稳定 token/span/recovery API 和 `stacker`，禁用
  alpha、Pratt 和 unstable feature；迁移当时 Frozen-conforming 的 expression/document subset，
  删除 raw-text Cursor、token clone 和固定 64 MiB parser thread。
- **I0.6 Decode/robustness gate（已完成）**：新增 `parse_document_bytes`、`decode.invalid-utf8` 和固定
  seed/case 的 Proptest gate；验证任意 bounded bytes/string 不 panic、span 有界、结果确定且 limits
  在递归/分配前生效。
- **I0.7 Manifest gate（已完成）**：Serde/TOML 强类型加载根 manifest 和 22 个 FCS fixture，验证路径、
  stage、expect、diagnostic、clauses 与 `implementation.*` 禁止规则。
- **I0.8 条款矩阵（已完成）**：维护 `docs/conformance/fcs5-implementation-matrix.md`，每行含规范章节、
  public API、实现文件、valid/invalid fixture、状态、下一阶段和已知偏差。
- **I0.9 基线 gate（已完成）**：workspace 只有 `fcs-source`；结构搜索、依赖/feature 审计、Clippy、
  nextest、fmt、diff 和
  独立 review 全绿，记录准确 test count 与 archive/main SHA。

交付：精确旧工具链归档、绿色 `main`、唯一 `fcs-source`、条款矩阵、偏差清单和强类型
manifest gate。逐步执行见 `docs/plans/i0-source-cutover.md`。

### I1：完整 Source AST 与 parser

- **I1.1 Lexer 完整性（2026-07-16 完成）**：补齐 I0 Chumsky lexer 的全部 Appendix B token、UTF-8/BOM、nested
  comment、重开后的 keyword、string escape/NUL/noncharacter、Color、unsigned decimal/unit 和
  byte span conformance；负号只作为 unary token。
- **I1.2 Grammar AST（已完成）**：按附录 B 建立 document、definition、statement、schema block、entity、
  Track、metadata/resource/sync/extension source node；source node 保留完整半开 span。表达式/type
  AST 与 parser evidence 已合并。
- **I1.3 顶级 parser（已完成）**：强制 format，拒绝 duplicate/unknown block，允许规范声明的顺序无关和
  source-order collection；全部 39 个 FCS source manifest entry 已在 parser boundary 执行。
- **I1.4 Definitions parser（已完成）**：统一 `const/fn/template` 到 definitions，template/function 使用
  不同 statement 类型或一个不能表示非法语句的 typed source enum。
- **I1.5 Generator parser（已完成）**：保持 I0 的 `..<`/`..=`、裸 `..` 拒绝和 local let；补齐
  nested/misplaced generate 的 Frozen category 以及 segment/track owner grammar；zero step
  统一留给 elaborator 求值。
- **I1.6 Schema parser（已完成）**：解析 Line/Note/Track/meta/credits/resources/sync、ordered extension
  object、preserve envelope 和 balanced Render payload；closed enum 的直接 spelling 是 string，
  bare identifier 仍作为名称 expression 保留给 static phase，parser 只识别结构。
- **I1.7 Diagnostic（已完成）**：所有 parser error 映射附录 C category，保留 primary/related span；不得
  panic 或接受 trailing garbage。
- **I1.8 Robustness 扩展（已完成）**：在 I0 byte/property gate 上覆盖全部 grammar production、comment、
  delimiter、expression precedence 和长合法输入，并增加独立 fuzz lane。

测试：每个 EBNF production 至少一个 valid/invalid fixture；所有 `fcs.md` source example parse。
I1.8 的生产覆盖 ledger、parser limits、deterministic property 与独立 fuzz smoke 证据分别见
`docs/conformance/fcs5-source-production-coverage.md` 和 `docs/conformance/fcs5-fuzz-lane.md`。

交付：完整、带 span 的 Core source AST，Appendix B parser、parse-stage conformance runner、
production coverage ledger、独立 fuzz lane 和 I1 条款矩阵证据。逐步执行草案见
`docs/plans/i1-source-ast-parser.md`；I1 只有在其完整 normative dependency closure、绑定 fixture/hash
建立 Reviewed Implementation Baseline，独立复审无未关闭的 Critical/Important finding，该计划与
绑定规范一致，且 I0 质量门通过后才自动开始 Rust 实现。该 gate 已通过，固定输入与 0/0/0 独立复审见
`docs/reviews/2026-07-16-i1-source-parser-baseline-review.md`；I1.1–I1.8 的实现与证据已合并，当前
进入 Task 9 governance、最终门禁和独立复审；I2.1–I2.9 的实现与独立交付证据已合并，当前由
I2.10 公共 conformance fixture 与阶段门禁收尾。I2.1–I2.10 及其 corrective chain 已合并到
`origin/main` `2d24494354b4b10fe8dbd30cdadf8d1f5d22f4c8`；主自审和适用 Rust gate 已通过。I3.1–I3.4
的 canonical implementation units 已合并，I3.3 metadata/resource/sync 与 I3.4 Line span corrective
residual 均已路由完成；当前 bounded frontier 转入 I3.5 Note lowering。

### I2：Static semantics 与编译期展开

- **I2.1 Type matrix**：逐项实现第 3/4 章 operator、conversion、builtin、finite/domain rule；
  generated table test 防止 operator 遗漏。
- **I2.2 Scope**：全局、参数、local、branch、generator scope；禁止 ancestor shadowing，允许
  sibling branch 同名；forward reference 在完整 symbol collection 后解析。
- **I2.3 Dependency graph**：const/function/template 独立节点和跨种类合法 edge；在 evaluation
  前报告确定性的最短 cycle 与完整 related spans。
- **I2.4 Function elaboration**：每条可达路径 return、exact return type、纯调用和 operation/node
  budget；拒绝 template/emit/generate。
- **I2.5 Template statement**：typed parameter/local、compile-time if、entity return、template
  composition、`with`、schema field/required/default 和 recursion trace。
- **I2.6 Generator range**：exact int/Beat count、正负 step、空 range、inclusive reachability、
  overflow；每次使用 `start + index*step`，不做重复 Float64 加法。
- **I2.7 Generator body**：local let、condition、typed emit、collection type、source-order output；
  禁止 nested/runtime dependency。
- **I2.8 Unified budget**：单个 elaboration context 计六类预算，开始工作前计数；错误含 kind、
  limit、observed、span、function/template/collection/range/index/emit trace。
- **I2.9 Expanded output**：私有构造器保证只含 concrete typed field/entity；public read-only API；
  invariant test 遍历确认无 source compile-time node。
- **I2.10 Public fixture**：definitions、template `with`、template if、beat/int generator、emit 和
  limits 的一份可执行 source。

测试：S2/S3 valid/invalid/budget/cycle 全部；展开结果不含任何 source compile-time node。

### I3：Canonical chart lowering

- **I3.1 Canonical IDs**：显式 ID、生成 ID、namespace、collision 和 deterministic order；禁止
  hash-map order 泄漏。
- **I3.2 Time normalization**：exact Beat/tempo 到 Float64 chartTime，保留 provenance；同 beat
  step、负 beat 和 inverse mapping test vectors。
- **I3.3 Metadata graph**：meta/contributor/credit/resource/sync typed canonical object 和引用验证。
- **I3.4 Line graph**：Line schema、parent existence、DAG、stable topo、inherit、base transform 和
  scroll 配置。
- **I3.5 Note lowering**：四 kind、required gameplay、Hold constraint、presentation defaults、line
  binding 和 stable sort。
- **I3.6 Track normalization**：target schema、segment/point、half-open boundary、blend/priority、
  fill/extrapolation、conflict 和 gap validation。
- **I3.7 Scroll model**：global/line scroll tempo、speed multiplier、reverse capability、floorScale、
  integration origin 和 initial distance。
- **I3.8 Canonical API**：Phase 4+ 只能依赖 immutable CanonicalChart；禁止引用 parser/template AST。
- **I3.9 Snapshot**：用 cataloged `serde_json` 建立仅测试使用的 human-readable canonical snapshot
  serializer，排序与 stable ID 固定；不能让 JSON representation 成为 canonical API。

测试：tempo/Track/graph/Note 每个边界，source declaration reorder 的语义不变性和 snapshot golden。

当前交付状态：I3.1–I3.8 已实现并合并；I3.8 已建立 immutable `CanonicalChart` aggregation 和单一
source compiler seam。I3.9 implementation branch 增加测试专用的显式 `serde_json::Value` projection、
pretty snapshot、direct/template byte equality 与单一 golden，并验证 snapshot 不保留 source/workspace/
authoring/preserve 状态；其交付仍受 full gate 与 Issue/PR workflow 约束。I3.9 不建立产品或规范性 JSON
格式。其后 residual 是 I4 runtime visibility/descriptor 与 I5 resource bytes/hash resolution。

### I4：Reference evaluator 与数值系统

- **I4.1 Easing**：附录公式和 31 ID 的端点、中点、overshoot test vector；binary64 finite check。
- **I4.2 Track evaluator**：constant/step/linear/easing/Bezier、point boundary、fill/extrapolation、
  layered blend 和 exact selected descriptor。
- **I4.3 Transform evaluator**：column-vector matrix、pivot、parent inherit 和 world transform vectors；
  可在 production 私有实现激活 cataloged `nalgebra` 的 `f64` fixed-size types，但 independent
  reference path 不得复用该实现或暴露 nalgebra public type。
- **I4.4 Scroll evaluator**：q、velocity、analytic/piecewise integral、distance continuity、negative/zero
  speed 和 direct seek。
- **I4.5 Expression DAG**：typed acyclic nodes、`s/b/q/d/p` availability、lazy choose、invalid math 和
  dependency-cycle analysis。
- **I4.6 Piecewise lowering**：强制边界 partition、descriptor sharing 和无 overlap/gap invariant；
  无法静态化的合法 Core expression 保留 typed DAG，不进入默认 baking。
- **I4.7 Exact integration validation**：对 portable-evaluable integrand 实现确定性求值/积分误差界、
  direct-seek 与 analytic cross-check；不输出采样曲线或 BakedCurve。
- **I4.8 Independent reference path**：reference evaluator 不复用 production caching/SIMD shortcut；
  production 与 reference cross-check。
- **I4.9 Determinism/property**：seek=sequential、partition invariance、frame-rate independence、
  randomized legal Track 与 error bound property tests。

### I5：Metadata、resources、sync 与 fidelity

- **I5.1 Profile validator**：fragment/chart/playable/renderable/publishable + features 正交约束。
- **I5.2 Contributors/credits**：标准/custom role、display order、identifier 和 typed reference。
- **I5.3 Resource manifest**：激活 cataloged `sha2`，实现 component-normalized workspace member
  path、root escape/symlink 防护、opaque bytes、SHA-256、media/color/alpha/sampling/font contract
  和 CanonicalResourceBundle；不执行媒体转码。
- **I5.4 Sync**：唯一 offset 公式、preview audio domain 和 player/converter shared test vector。
- **I5.5 Typed custom**：ordered object、duplicate key、depth/count/string/byte limits和 FCBC-compatible
  value restrictions。
- **I5.6 Provenance**：originState、sourcePath/value/order、mapping rule 和 stale dependency graph。
- **I5.7 Report/repair model**：stable category/rule ID、status aggregation、deterministic entries 和
  old/new typed values。

### I6：PGR/RPE/PEC importer

- **I6.1 Shared import IR boundary**：激活 cataloged `serde`/`serde_json`，source parser 先输出
  未经 FCS 猜测的 lossless parsed source；parser dialect、profile selection、Repair 分层后才得到
  source semantic IR，lowering 才产生 CanonicalChart/provenance/report。只有明确纳入 package input
  surface 后才为 owning converter crate 激活 defaults-disabled `zip`。
- **I6.2 PGR v1/v3**：line BPM/time、event、coordinate、speed/floor validation、Note/fake/side/Hold。
- **I6.3 RPE**：BPMList/exact Beat、bpmfactor、multi-layer blend、father/rotateWithFather、Bezier、
  controls、visibleTime、resource/metadata。
- **I6.4 PEC**：direct decimal Beat/bp、150/175ms offset profile、Note/Line X、line point/easing、
  cv profile/integration、four Note、fake/Hold 和 command source order；不得恢复无证据的默认
  `tick2048`。
- **I6.5 Profile registry/selection**：实现 content-hash-bound typed selector、detection evidence、
  candidate/equivalence/configured-default decision 和 `--source-profile`/`--target-profile` 公共 API；
  历史 source engine 行为与 Core semantic 分离，Repair 不得选择合法但歧义的 profile。
- **I6.6 Invalid source**：strict 默认失败；repair mode 每条修改有 record；验证脚本必须与实际
  parser/evaluator 逻辑一致。
- **I6.7 Fixtures**：公开最小/feature/extreme fixture + opt-in copyright lane；每个有 canonical
  snapshot 和 expected report。

### I7：FCBC writer、loader 与 ABI

- **I7.1 Primitive codec**：使用标准库 `to_le_bytes`/`from_le_bytes` 实现 little-endian、Value、
  Record、StringTable、ConstantPool 和 canonical zero；不引入 `byteorder`。
- **I7.2 Container writer**：激活 cataloged `crc` 的 CRC-32/ISO-HDLC 与 `sha2`，实现 128-byte
  header、40-byte entries、alignment/padding、checksum、source hash 和 deterministic sections。
- **I7.3 Core sections**：Meta through Distance、required ResourceData 的 record writer/reader、原始
  resource bytes/hash/coverage 和 version dispatch。
- **I7.4 Descriptor ABI**：PropertyDescriptor、Segment/Piecewise/Expression topological nodes、
  environment/type validation、analytic/evaluable Distance 和 descriptor kind 5 rejection；FCBC 2
  所有 profile 都是 exact-only，显式 target approximation 不写入 standard FCBC。
- **I7.5 Profiles**：runtime/fidelity/strict-runtime section/feature 保留矩阵，reserved profile 2
  rejection；所有 profile 都是 one-chart/self-contained/no-source-snapshot。
- **I7.6 Loader hardening**：先 bounds 再 allocation，count overflow、overlap、checksum、UTF-8、ID、
  DAG/reference/type、unknown optional/required 和公开 limits。
- **I7.7 Golden files**：在 S15 的 864-byte empty 与 1021-byte embedded-resource schema 2 baseline
  上，补齐每种 section/descriptor/profile 的 byte fixture、manifest、CRC 和 SHA-256。
- **I7.8 Malformed corpus/fuzz**：header、section、record、index、graph 和 descriptor mutations；loader
  不 panic、不 OOM、不返回部分 chart。
- **I7.9 Execution closure**：source→canonical→FCBC→load→reference execution 数值一致。

### I8：FCS 和外部格式 exporter

- **I8.1 Canonical FCS formatter**：唯一 whitespace/order/literal/unit policy，parse-format-parse
  semantic idempotence。
- **I8.2 CapabilitySet**：每个目标版本声明 time/Note/Track/graph/expression/resource/limit 能力。
- **I8.3 Negotiation**：direct/equivalent/bake/preserve/drop/unsupported 在写目标前完成并入 report。
- **I8.4 Boundary-preserving bake**：先固定 tempo/Note/Hold/event，再优化 continuous segment；量化
  误差按目标和属性记录。
- **I8.5 PGR exporter**：明确 v1/v3，line-local time/speed/floor、coordinate 和 Note encoding。
- **I8.6 RPE exporter**：BPMList/bpmfactor、event layer、father/inherit、easing/Bezier 和 extension。
- **I8.7 PEC exporter**：bp/tick/offset、command ordering、cv 和 integer limits。
- **I8.8 Target reparse**：每次 export 重新 import 目标并做 canonical comparison；超过报告误差则
  failed，不能只信 writer。
- **I8.9 Strategy tests**：semantic/roundtrip/strict/repair、stale raw payload、drop authorization 和
  deterministic report。

### I9：Render Profile

- **I9.1 Render source/schema**：profile/version、Layer/Node/Geometry/Path/Paint/Stroke/Text，以及只允许
  FCS `@resource` 的静态 resource source schema。
- **I9.2 Compile-time construction**：RenderNode template/generator/emit，topology/resource 编译期固定。
- **I9.3 Canonical scene**：stable forest、pass/sort、attachment dependency、dynamic property descriptor。
- **I9.4 RenderSection codec**：全部 table/enum/reference、exact descriptor kind 1–4 与 FCBC
  version/feature/Resources/ResourceData validation；section 不保存 path/payload/source text/cluster。
- **I9.5 Semantic evaluator**：transform/opacity/active/visibility/clip/attachment/composite draw list。
- **I9.6 Reference rasterizer**：为 render crate 激活 defaults-disabled `image`，只开启规范/fixture
  需要的 codec并设置 dimension/allocation limits；实现 fixed resources、path coverage、linear
  compositing、image sampling、embedded-font glyph run 和 RGBA8 output，不能让 image crate behavior
  定义规范或访问外部 asset/system font。
- **I9.7 Raster fixtures**：每种 node/paint/clip/composite、line/note attachment、dynamic boundary 和
  malformed resource；在 S15 semantic binding fixture 上增加可解码 image/font FCBC payload 并比较
  规范容差。
- **I9.8 Backend interface**：GPU/realtime backend只消费 canonical display data，不能反向影响 chart。

### I10：CLI、发行组合与 Conformance Release Candidate

- **I10.1 CLI surface**：在 CLI crate 激活 cataloged `clap`，提供 `check`、`format`、`compile`、
  `inspect`、`convert`、`report`，输出稳定 exit category 和可选 JSON diagnostic/report。
- **I10.2 Profile/resource options**：FCBC profile、strict/repair、resolver root、source/target
  semantic profile 和 target capability；approximation budget 只属于显式 target conversion，不能
  成为标准 FCS→FCBC compile 参数。
- **I10.3 Public API assembly**：只组合无版本前缀的领域 crate，不建立第二默认 AST 或兼容 facade。
- **I10.4 Converter assembly**：CLI 只通过 canonical/provenance API 调用 converter，不直接消费
  source AST。
- **I10.5 Distribution metadata**：package metadata、license、version output、feature/profile 清单和
  release artifact 内容与 Frozen 版本表一致。
- **I10.6 文档/fixture闭环**：四规范每个 normative example 可执行，CLI `--version` 同版本表。
- **I10.7 最终验证**：workspace gates、all profiles/goldens/round-trips/rasters、property/fuzz smoke、
  optional copyright fixtures；发布 conformance manifest。

## 7. 每个实施阶段的质量门

必须按顺序执行：

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
git diff --check
```

涉及二进制、转换或渲染的阶段还必须运行对应 golden/round-trip/raster lane。Clippy 失败后
不得把旧 nextest 结果当作当前工作树证据。

## 8. 当前状态

- S0：完成；权威文档、治理、路线图、版本表和旧设计迁移已落地；
- S1–S12：原章节语义审查完成；其中 FCS Source/Render source grammar 由 S14 重开闭合；
- S13：2026-07-14 历史冻结记录保留；其 FCS/Render Source grammar 完整性结论已由 S14 纠正；
- S14：FCS Core 5.0.0 与 Render Profile 1.0.0 grammar closure 已写入规范和 32-entry FCS
  conformance baseline，其 Reviewed hash 作为该范围的历史审计证据保留；后续 ADR 0007–0009
  又使五个版本域进入 Draft/联合重审，不能把 S14 hash 当作当前完整规范 hash；
- S15：FCS Core、FCBC/ABI、Render 与 Conversion 候选 delta 已依依赖顺序写入；FCS manifest 为
  39 项，FCBC 有三个 schema 2 golden/17 个 mutation，Render 有 embedded-resource binding，
  Conversion 有 12-profile/7-dialect/56-rule/32-category/38-valid/5-invalid/10-selection registry。统一
  cross-spec/hash/test 候选自检见 `docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md`；非空 ABI
  artifact 已由 `docs/reviews/2026-07-16-fcbc2-execution-abi-nonempty-review.md` 独立复审关闭；Render
  binary/raster 的历史 closure 已被 executable-vector 审计发现的 REN-I08–I16 重新打开；随后固定
  快照复审又以 `RNR-C01`–`RNR-I08`（Critical 2、Important 8）失败，当前 ledger 见
  `docs/reviews/2026-07-16-render1-normative-amendment-review.md`；修订后的 candidate hash 已由该文件
  第 11 节以 0/0/0 独立复审关闭 normative RNR gate。真实 Conversion round-trip、完整
  RenderSection executable binary/raster artifact、Core canonical fixture validation 与最终联合独立
  复审仍是各自阶段/RC blocker，全部版本域保持 Draft；
- I0.1–I0.9 已完成；I0 baseline 加 test-only ABI/Render closure 的 149-test 历史证据仍保留，当前
  I1.1–I1.8 implementation/evidence 已合并，root workspace 的最终 I1 gate 为 218 个 tests 通过；
  结构/依赖/归档拓扑和既有 I0 独立复审 gate 不回退；
- ADR 0010 已将 I1–I9 改为阶段范围化 Reviewed Implementation Baseline；Render `RNR-*` normative
  gate、诚实 manifest、I1 source-parser fixed hash/fixture tree、计划一致性与 I0 质量门均已通过。
  `docs/reviews/2026-07-16-i1-source-parser-baseline-review.md` 建立 I1 baseline；I1.1–I1.8
  implementation/evidence 已合并；I2 的 implementation closure review 现已独立通过，当前 frontier
  转入 I3.1；
- retained source subset 已提升为唯一 `fcs-source`；I1.1–I1.8 已交付完整 Source AST/grammar
  parser boundary，后续 static/canonical/runtime 语义仍由 I2+ owning stages 负责；
- 当前 generator AST/parser 只接受 `int|beat`、`..<|..=` 和 `let|if|emit`，拒绝裸 `..`、
  `return` 与 nested generator；zero-step 语法保留给 I2，elaborator 在 I1 不展开并返回稳定
  `implementation.feature-unavailable`，且不产生部分输出；
- I2.1–I2.10 已完成并合并；I2.10 的三份 valid fixture、六份 elaborate-error/budget fixture、矩阵
  更新和完整 Rust delivery gate 已记录。后续 corrective chain 为 PR #92/#96/#97/#98/#102，当前
  `origin/main` 为 `f15a6595ebdf8c01dfe77424cbceda1a9f018fe1`；主自审和适用 Rust gate 均通过。
  独立 merged-SHA Audit 已对 #92、#96、#98、#102 给出 `pass`，#97 需在 #99 修复后重审。因而 I2
  closure 的实现交付已修正但 stage claim 保持 provisional；五个规范域仍为 Draft。
- I3.1–I3.7 已实现并合并；I3.7 scroll model 的计划与边界记录在
  `docs/plans/i3-scroll-model.md`。I3.8 implementation branch 已聚合 immutable `CanonicalChart`，其
  计划记录在 `docs/plans/i3-canonical-chart.md`，但仍等待交付门禁。后续 residual 为 I3.9 snapshot、
  I4 runtime descriptor/evaluator 和 I5 resource bytes/hash resolution；I3.9–I10 尚未完成。I3 可以
  建立自己的 canonical model/owning crate，但不得把阶段性 canonical seams 误报为完整
  CanonicalCompilation/runtime/FCBC/Render/Conversion/CLI 完成或版本 Frozen。
