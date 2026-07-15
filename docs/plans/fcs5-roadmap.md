# FCS 5 规范与参考实现路线图

## 1. 目标

本路线图取代此前按日期拆分的 FCS 5 计划。目标是先冻结可独立实现的 FCS 5 规范，
再让 Rust workspace 成为规范的参考实现，最终完成 FCS、FCBC、播放器接口、转换器、
Render Profile 与 CLI 的同一条确定性工具链。

## 2. 全局规则

- 规范性语义只能写入根目录四份权威规范；计划和 decision record 不能覆盖它们。
- FCS 5 尚未公开，不为未发布的临时语法保留兼容别名。
- 每项行为先有规范条款和失败 fixture，再实现，再通过质量门。
- 编译期能力不能进入 FCBC；FCBC Core 不包含 jump、loop、recursion、mutable local、
  `template`、`generate` 或 `emit`。
- 运行时只有一个物理主时钟；判定时间与滚动坐标分离，但不建立 line-local 物理时钟。
- 不静默修复非法谱面；可选 repair 必须生成机器可读记录。
- I0-A 用 `archive/fcs4-pre-cutover` 保存完整旧工具链；I0 完成后活动 `master` 只保留一套
  无版本实现前缀的 FCS source API，不建立 FCS 4 兼容层。
- 每个实施阶段结束前依次运行 Clippy、nextest、rustfmt check 和 diff check。
- 当前未提交 generator 工作在编译期语言规范冻结后重新审计，不以现状反向定义语法。

## 3. 版本基线

| 组件 | 候选版本 | 冻结条件 |
|---|---:|---|
| FCS Core | 5.0.0 | Core 全章节 Reviewed，source/canonical fixtures 可执行 |
| FCBC | 2.0.0 | 二进制布局、loader 规则和 golden files 完成 |
| Execution ABI | 1.0.0 | descriptor/evaluator test vectors 完成 |
| Render Profile | 1.0.0 | semantic 与 reference raster fixtures 完成 |
| Conversion Specification | 1.0.0 | PGR/RPE/PEC mapping 与 loss report fixtures 完成 |

上述五个版本已于 2026-07-14 达到冻结条件；冻结记录见
`docs/reviews/2026-07-14-fcs5-freeze-review.md`，绑定 corpus 入口为 `conformance/manifest.toml`。

## 4. Part A：规范冻结

### S0：治理、术语和迁移索引

交付：

- `docs/specification-governance.md`；
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

交付：`s/b/q/d/p`、`choose`、typed DAG、piecewise、adaptive baking、强制精确边界、
属性级误差指标、Float64 和资源预算。

验证：无通用控制流、无循环依赖、baked error、1ms 最大验证间隔的正确含义。

### S9：Metadata、credits、resources 和 sync

交付：meta、contributors、credits、custom role、resource manifest、artwork、audio、font、
hash、typed custom data、profile 最低要求和 FCBC 保留规则。

验证：自由角色、引用完整性、offset 公式、runtime/editable/archive 保留矩阵。

### S10：Fidelity 和 Conversion

交付：namespace/schema、来源状态、语义状态、preserve payload、repair、capabilities、
ConversionReport、semantic/roundtrip/strict 策略和 PGR/RPE/PEC mapping。

验证：每个丢失、近似和仅保留行为均机器可读；canonical semantic round-trip fixture。

### S11：FCBC 2.0 与 Execution ABI 1.0

交付：header、section table、primitive encoding、section schema、descriptor、profile、hash、
checksum、loader validation、deterministic ordering 和 golden bytes。

验证：第三方仅按文档可实现 reader；未知 optional/required section；损坏文件 corpus。

### S12：Render Profile 1.0

交付：retained scene graph、node/path/paint/stroke/clip/image/text、attachment、compositing、
RenderSection、semantic conformance 和 reference raster conformance。

验证：固定资源与 glyph、无 Canvas mutable state、无 pixel IO、render 不影响 gameplay。

### S13：全规范审查和冻结

任务：

1. 搜索并消除 `TODO`、`TBD`、“有待商榷”和未定义核心行为；
2. 检查四份规范的术语、单位、版本和交叉引用；
3. 为所有 normative example 建立 fixture 清单；
4. 独立审查 time/scroll、Track、Note、FCBC 和 conversion mapping；
5. 将各规范状态从 Draft 改为 Frozen；
6. 发布 conformance manifest。

完成条件：实施代码不需要再做语言或容器设计，只需实现规范。

## 5. Part B：参考实现

### I0：健康基线与规范对账

当前进度：I0-A（快照、归档和 `master` 分支切换）、I0.2 generator staging、I0.3 唯一
crate 切换和 I0.4 诊断边界已完成；I0.5 parser 重建和 conformance gate 尚未开始。活动树现已
只有 `fcs-source`；generator 仍按 I0 边界不展开，不能将局部 source candidate 测试通过误记
为完整 source implementation 已完成。

- 提交完整切换前快照并建立永久 `archive/fcs4-pre-cutover`；
- fast-forward `master`，删除活动主线中的 FCS 4、旧 converter 和旧 CLI；
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

- 实现 Track、DAG、piecewise、easing、积分、distance 和 adaptive baking；
- 建立未复用生产优化路径的 reference evaluator；
- 验证 seek、误差、边界和跨平台确定性。

完成条件：运行结果与帧率无关，所有 portable 值满足规范误差。

### I5：Metadata、resources、sync 与 fidelity

- 实现 canonical metadata/credit/resource；
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
- capability negotiation 和 target baking；
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

- **I0.1 快照与归档（已完成）**：提交当前 generator、Frozen 文档、conformance 和项目
  workflow 工作；创建指向精确切换前提交 `148936d17b671bb34968c88969ab748c818f9fc0` 的
  `archive/fcs4-pre-cutover`，然后 fast-forward `master`。后续 Trellis bookkeeping commits
  不移动归档指针。
- **I0.2 Generator staging（已完成）**：只接受 `..<`/`..=`，拒绝裸 `..`；在 I2 完成展开前
  返回临时 `FeatureUnavailable { feature: "compile-time-generator", .. }`；已消除
  non-exhaustive match 和部分输出路径。稳定 `implementation.feature-unavailable` 公共
  diagnostic code 仍属于后续诊断边界任务。
- **I0.3 唯一 crate 切换**：删除活动 FCS 4 core、旧 CLI、旧 converter 和第二 IR；将候选
  source front end 提升为 `crates/fcs-source`，不提供兼容 re-export。
- **I0.4 诊断边界**：建立稳定 `DiagnosticCode`、UTF-8 byte span、labels、确定性多诊断
  `ParseOutput<T>`；Chumsky error 不泄漏为 public API。
- **I0.5 Chumsky parser 基线**：使用 0.11.2 稳定 token/span/recovery API 和 `stacker`，禁用
  alpha、Pratt 和 unstable feature；迁移当前 Frozen-conforming expression/document subset，
  删除 raw-text Cursor、token clone 和固定 64 MiB parser thread。
- **I0.6 Decode/robustness gate**：新增 `parse_document_bytes`、`decode.invalid-utf8` 和固定
  seed/case 的 Proptest gate；验证任意 bounded bytes/string 不 panic、span 有界、结果确定且 limits
  在递归/分配前生效。
- **I0.7 Manifest gate**：Serde/TOML 强类型加载根 manifest 和 22 个 FCS fixture，验证路径、
  stage、expect、diagnostic、clauses 与 `implementation.*` 禁止规则。
- **I0.8 条款矩阵**：维护 `docs/conformance/fcs5-implementation-matrix.md`，每行含规范章节、
  public API、实现文件、valid/invalid fixture、状态、下一阶段和已知偏差。
- **I0.9 基线 gate**：workspace 只有 `fcs-source`；结构搜索、依赖/feature 审计、Clippy、
  nextest、fmt、diff 和
  独立 review 全绿，记录准确 test count 与 archive/master SHA。

交付：精确旧工具链归档、绿色 `master`、唯一 `fcs-source`、条款矩阵、偏差清单和强类型
manifest gate。逐步执行见 `docs/plans/i0-source-cutover.md`。

### I1：完整 Source AST 与 parser

- **I1.1 Lexer 完整性**：补齐 I0 Chumsky lexer 的全部 Appendix B token、UTF-8/BOM、nested
  comment、keyword、string escape、Color、decimal/unit 和 byte span conformance。
- **I1.2 Grammar AST**：按附录 B 建立 document、definition、statement、schema block、entity、
  Track、metadata/resource/sync/extension source node；source node 保留完整半开 span。
- **I1.3 顶级 parser**：强制 format，拒绝 duplicate/unknown block，允许规范声明的顺序无关和
  source-order collection。
- **I1.4 Definitions parser**：统一 `const/fn/template` 到 definitions，template/function 使用
 不同 statement 类型或一个不能表示非法语句的 typed source enum。
- **I1.5 Generator parser**：保持 I0 的 `..<`/`..=` 与裸 `..` 拒绝；补齐 local let、
  nested/misplaced generate 和 segment/track owner grammar；zero step 统一留给 elaborator 求值。
- **I1.6 Schema parser**：解析 Line/Note/Track/meta/credits/resources/sync；parser 只识别结构，
  schema 合法性留给 static phase。
- **I1.7 Diagnostic**：所有 parser error 映射附录 C category，保留 primary/related span；不得
  panic 或接受 trailing garbage。
- **I1.8 Robustness 扩展**：在 I0 byte/property gate 上覆盖全部 grammar production、comment、
  delimiter、expression precedence 和长合法输入，并增加独立 fuzz lane。

测试：每个 EBNF production 至少一个 valid/invalid fixture；所有 `fcs.md` source example parse。

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
- **I4.6 Piecewise lowering**：强制边界 partition、descriptor sharing 和无 overlap/gap invariant。
- **I4.7 Adaptive baking**：recursive fit、property metric、velocity+distance dual validation、1ms
  maximum validation interval和 compile budgets；输出 BakedCurve provenance。
- **I4.8 Independent reference path**：reference evaluator 不复用 production caching/SIMD shortcut；
  production 与 reference cross-check。
- **I4.9 Determinism/property**：seek=sequential、partition invariance、frame-rate independence、
  randomized legal Track 与 error bound property tests。

### I5：Metadata、resources、sync 与 fidelity

- **I5.1 Profile validator**：fragment/chart/playable/renderable/publishable + features 正交约束。
- **I5.2 Contributors/credits**：标准/custom role、display order、identifier 和 typed reference。
- **I5.3 Resource manifest**：激活 cataloged `sha2`，实现 kind、URI policy、SHA-256、
  media/color/alpha/sampling/font validation。
- **I5.4 Sync**：唯一 offset 公式、preview audio domain 和 player/converter shared test vector。
- **I5.5 Typed custom**：ordered object、duplicate key、depth/count/string/byte limits和 FCBC-compatible
  value restrictions。
- **I5.6 Provenance**：originState、sourcePath/value/order、mapping rule 和 stale dependency graph。
- **I5.7 Report/repair model**：stable category/rule ID、status aggregation、deterministic entries 和
  old/new typed values。

### I6：PGR/RPE/PEC importer

- **I6.1 Shared import IR boundary**：激活 cataloged `serde`/`serde_json`，source parser输出版本化、
  未经 FCS 猜测的 source semantic IR；lowering 才产生 CanonicalChart/provenance/report。只有明确
  纳入 package input surface 后才为 owning converter crate 激活 defaults-disabled `zip`。
- **I6.2 PGR v1/v3**：line BPM/time、event、coordinate、speed/floor validation、Note/fake/side/Hold。
- **I6.3 RPE**：BPMList/exact Beat、bpmfactor、multi-layer blend、father/rotateWithFather、Bezier、
  controls、visibleTime、resource/metadata。
- **I6.4 PEC**：tick/bp、offset sign、line point/easing、cv/integration、four Note、fake/Hold 和 command
  source order。
- **I6.5 Compatibility evaluator**：历史 source engine 行为与 Core semantic 分离；选择兼容路径时
  记录 runtime-only/approximation，不能污染 Core evaluator。
- **I6.6 Invalid source**：strict 默认失败；repair mode 每条修改有 record；验证脚本必须与实际
  parser/evaluator 逻辑一致。
- **I6.7 Fixtures**：公开最小/feature/extreme fixture + opt-in copyright lane；每个有 canonical
  snapshot 和 expected report。

### I7：FCBC writer、loader 与 ABI

- **I7.1 Primitive codec**：使用标准库 `to_le_bytes`/`from_le_bytes` 实现 little-endian、Value、
  Record、StringTable、ConstantPool 和 canonical zero；不引入 `byteorder`。
- **I7.2 Container writer**：激活 cataloged `crc` 的 CRC-32/ISO-HDLC 与 `sha2`，实现 128-byte
  header、40-byte entries、alignment/padding、checksum、source hash 和 deterministic sections。
- **I7.3 Core sections**：Meta through Distance 的 record writer/reader 和 version dispatch。
- **I7.4 Descriptor ABI**：PropertyDescriptor、Segment/Piecewise/Baked、Expression topological nodes、
  environment/type validation。
- **I7.5 Profiles**：runtime/editable/archive/strict-runtime section/feature 保留矩阵。
- **I7.6 Loader hardening**：先 bounds 再 allocation，count overflow、overlap、checksum、UTF-8、ID、
  DAG/reference/type、unknown optional/required 和公开 limits。
- **I7.7 Golden files**：每种 section/descriptor/profile 的 byte fixture、manifest 和 SHA-256。
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

- **I9.1 Render source/schema**：profile/version、Layer/Node/Geometry/Path/Paint/Stroke/Text 和 resource。
- **I9.2 Compile-time construction**：RenderNode template/generator/emit，topology/resource 编译期固定。
- **I9.3 Canonical scene**：stable forest、pass/sort、attachment dependency、dynamic property descriptor。
- **I9.4 RenderSection codec**：全部 table/enum/reference 与 FCBC version/feature validation。
- **I9.5 Semantic evaluator**：transform/opacity/active/visibility/clip/attachment/composite draw list。
- **I9.6 Reference rasterizer**：为 render crate 激活 defaults-disabled `image`，只开启规范/fixture
  需要的 codec并设置 dimension/allocation limits；实现 fixed resources、path coverage、linear
  compositing、image sampling、glyph run 和 RGBA8 output，不能让 image crate behavior 定义规范。
- **I9.7 Raster fixtures**：每种 node/paint/clip/composite、line/note attachment、dynamic boundary 和
  malformed resource；比较规范容差。
- **I9.8 Backend interface**：GPU/realtime backend只消费 canonical display data，不能反向影响 chart。

### I10：CLI、发行组合与 Conformance Release Candidate

- **I10.1 CLI surface**：在 CLI crate 激活 cataloged `clap`，提供 `check`、`format`、`compile`、
  `inspect`、`convert`、`report`，输出稳定 exit category 和可选 JSON diagnostic/report。
- **I10.2 Profile/resource options**：FCBC profile、strict/repair、resolver root、target capability 和
  bake budget显式参数。
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
- S1–S12：完成；逐章审查和首批机器可读 conformance fixture 已落地；
- S13：完成；FCS 5.0.0、FCBC 2.0.0、ABI 1.0.0、Render 1.0.0、Conversion 1.0.0
  已于 2026-07-14 Frozen；
- I0-A 快照与归档、I0.2 generator staging、I0.3 唯一 crate 切换和 I0.4 稳定诊断边界已完成；
  100 个 candidate tests 通过；I0.5 及后续任务尚未执行；
- 已有 Phase 1/2 source candidate：已提升为唯一 `fcs-source`，仍需按 S1–S4 对账；
- 当前 generator AST/parser：已拒绝裸 `..`，zero-step 语法保留给 I2；elaborator 在 I0
  暂不展开 generator 并返回临时 feature-unavailable 诊断；稳定诊断 API 已在 I0.4 建立；
- I3–I10：未开始。
