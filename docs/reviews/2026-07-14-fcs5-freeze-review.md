# FCS 5 规范冻结审查

日期：2026-07-14
结论：Accepted — Frozen

## 冻结版本

| 规范 | 版本 |
|---|---:|
| FCS Core Source Specification | 5.0.0 |
| FCBC Container Format | 2.0.0 |
| FCS Execution ABI | 1.0.0 |
| FCS Render Profile | 1.0.0 |
| FCS Conversion Specification | 1.0.0 |

## 审查范围与关闭项

### S1–S3：Source、类型和编译期语言

- 完整文件头、UTF-8、token、literal、reference、array/object、interval 和 EBNF；
- `array<T>` 是 immutable homogeneous compile-time value，空 array 由上下文定型；
- template required field 在 return 时验证，不能由调用后的 `with` 延迟补齐；
- compile-time `if` 全分支静态检查、只对选中分支执行值求值与结构展开；
- `..<`/`..=` 是唯一 range operator，裸 `..` 非法；
- generator 支持 exact int/Beat、正负 step、空 range、local let、if 和 typed emit；
- Track 的 `segments` 是可 generator/emit 的局部 collection；
- 六类 budget 在单次 elaboration 中共享并在工作开始前计数；
- 编译期结构在 canonical lowering 前完全消失。

### S4–S8：时间、Track、Line、Note 和数值

- 唯一物理 `chartTime`，全局 chartBeat 与 lineScrollCoordinate 不是独立 clock；
- q 原点固定为 `q(0s)=0`，scrollTempo source/边界和 floor integral 已定义；
- Track 有 owner-local ID、target、blend/priority、fill/extrapolation、segments 和 point 冲突规则；
- Line inherit 使用声明 component 递推，不依赖矩阵数值分解；
- Note source ID 可选、canonical ID 必须存在；
- `d`、sideSign、yOffset 和 Hold body anchor 明确定义；
- runtime dependency 白名单防止 scrollFactor/d 循环；
- And/Or/Choose lazy，整个 DAG 仍结构/type validated；
- strict-runtime 使用 binary64 roundTiesToEven，constant folding 与 runtime bits 一致；
- 1ms 是不能解析证明时的最大验证间隔，不是固定 1000Hz 输出。

### S9–S12：Metadata、Conversion、FCBC 和 Render

- document profile 与 FCBC container profile 分离编码；
- typed custom array、contributors/credits/resources/sync 和 offset 公式闭合；
- ConversionReport、TargetDescriptor 和 capability hash 已定义；
- PGR/RPE/PEC 基础 exact mapping rule 有固定 ID/vector；
- PGR floor visual scale 的社区歧义不被猜测，必须由 engine capability 提供；
- FCBC header/section/value/descriptor/loader/deterministic rules由 golden bytes 验证；
- loader 安全验证与独立数值 conformance validator 责任分离；
- Render scene、geometry table、attachment、paint/text 和 semantic/raster vector 已定义。

## 绑定 Conformance Baseline

入口：`conformance/manifest.toml`

初始 corpus 包含：

- 22 个 FCS source/static/canonical fixture 条目；
- compile-time、time/scroll、parent transform、metadata 和 stable diagnostic expectations；
- binary64/numeric/easing vectors；
- 804-byte FCBC runtime golden，13 个 required Core section；
- 8 个 FCBC deterministic mutation/error vector；
- 1 个 Render semantic + RGBA8 raster vector；
- PGR/RPE/PEC exact mapping 和 capability-boundary vectors。

FCBC golden：

```text
decoded length: 804
SHA-256: 461b6bf2a1cf5e99ad0e81fb01d4621f149dd0a54f8eff403bf4924dda837f6c
```

## 冻结内容哈希

哈希对当时文件原始 UTF-8 bytes 计算：

```text
fcs.md
72e984e6a1f098ebc014b9772d4d108115f6e51e8c572c6437c5e21c9188bb1c

fcbc.md
54f819097ae10685d74c6a86e3fba41aff7236d2c700ab66c5e9615b39794eb0

fcs-render.md
bbd44462ce837e81e068bb97308539f5df9d3cec192755e75ef71e4207b635d6

fcs-conversion.md
3228471b8c77c01fa9f540264dcf3aeefecfe41d1f84ead36811f81c7cca2565

docs/specification-governance.md
e58a1254277c46b5adf7881b653fe17662297acfd6278eef2d69a8dc052f5005

docs/plans/fcs5-roadmap.md
96c0398165c280c9c923c424c49e6c5e1f4512290f349846908ef6aada7edbf5

conformance tree (39 files; paths relative to conformance/ with / separators; ordinal path order;
UTF-8(path) + NUL + bytes + NUL)
b35a6f0c8c2efbe3136fd07a30e8fe715f6d730fac053017328123964c91b98a
```

该哈希是审计记录，不是规范版本兼容机制。冻结后允许新增不改变语义的 fixture；修改规范
语义必须按治理文件提升对应版本，不能仅更新本记录中的哈希。

## 实现状态声明

规范 Frozen 不表示当前 Rust workspace 已通过 FCS 5 conformance。现有 Phase 1/2 代码仍是候选
实现，必须从路线图 I0 开始按“条款—实现—fixture”对账。冻结时已知当前工作树中的 generator
AST/parser 尚未接入 elaborator，并且临时接受裸 `..`；这些是实现偏差，不是规范兼容行为。

## 后续规则

- 纯勘误且不改变任何有效输入语义：提升对应 patch；
- 保持现有语义的可选新增：提升 minor；
- 改变语法、canonical、runtime、binary layout 或转换意义：提升相应 major；
- 实现阶段不得修改 Frozen 语义来迁就当前代码；发现规范缺陷必须先走规范变更流程。

## 冻结后实施路线修订

日期：2026-07-14

用户在规范冻结后确认项目未公开且迁移成本为零，因此参考实现不再保留 FCS 4 共存期：

- I0 执行前创建 `archive/fcs4-pre-cutover`，保存精确的切换前工作树；
- `master` 作为唯一活动开发和默认分支；
- 活动 workspace 删除 FCS 4、旧 converter 和旧 CLI；
- `crates/fcs-core/src/v5` 提升为无版本前缀的 `crates/fcs-source`；
- source parser 使用 Chumsky 0.11.1 稳定 API；
- canonical/runtime/FCBC/converter/render/CLI crate 按路线阶段重新建立。

设计记录为 `docs/decisions/0006-unversioned-source-cutover.md`，详细执行计划为
`docs/plans/i0-source-cutover.md`。该决定只改变内部结构和实施顺序，不改变四份权威规范、
conformance 语义或任何版本域。

冻结时 roadmap 历史 SHA-256 保留为：

```text
96c0398165c280c9c923c424c49e6c5e1f4512290f349846908ef6aada7edbf5
```

实施路线修订后的 roadmap SHA-256 为：

```text
f501ad13ea81c643a92be5b38cbfc54b42106e60cc0079c8b867558df9c85b0e
```

复核时六份规范/治理文件和 39-file conformance tree 均保持冻结记录中的 bytes 与哈希。

### 2026-07-15 I0 依赖基线修订

用户确认可以修改 I0 计划并引入经审计的依赖。该修订只替换上一节的 parser/dependency
实施基线，不改变 source、canonical、execution、FCBC、Render 或 Conversion 语义：

- Chumsky 从 0.11.1 升级到稳定补丁 0.11.2，并启用 `std`/`stacker`；
- I0 新增严格 UTF-8 byte entry 和固定 seed/case 的 Proptest robustness gate；
- Serde/TOML 仍只用于 test manifest，接受 TOML `parse` feature 的 Winnow 传递依赖；
- 根 workspace 预登记 `serde_json`、`nalgebra`、`sha2`、`crc`、`image`、`clap`、`zip`，但仅由
  I3-I10 的 owning crate 在对应阶段激活；
- `fcs-source` 的 direct normal dependency 仍只有 Chumsky，未来阶段候选不进入其依赖树。

上一版冻结后实施路线 SHA-256 保留为：

```text
f501ad13ea81c643a92be5b38cbfc54b42106e60cc0079c8b867558df9c85b0e
```

本次依赖基线修订后的 roadmap SHA-256 为：

```text
ebc374dd128e7272d06eb4491564770f8b7b218ff0e58df39f627abd73d72222
```

四份权威规范、规范治理文件和 conformance tree 未修改，因此冻结版本与原始哈希记录保持
不变。

### 2026-07-15 I0 实施证据同步

I0.1–I0.8 已按实际 workspace 复核并同步到 roadmap 与 implementation matrix：

- `archive/fcs4-pre-cutover` 仍固定在
  `148936d17b671bb34968c88969ab748c818f9fc0`，并且是当前 `master` 的祖先；
- 活动 workspace 只有无版本前缀的 `crates/fcs-source`，旧 FCS 4 core、converter、CLI、VM
  和 bytecode 只保留在归档分支；
- source subset 已迁移到 Chumsky 0.11.2 的单一 spanned-token 数据流，严格 byte decode、
  固定配置 Proptest robustness 和强类型 conformance manifest 完整性门已建立；
- generator parser 只接受 `int`/`beat`、`..<`/`..=` 和 `let`/`if`/`emit`，I0 elaborator 在
  产生任何部分输出前返回 `implementation.feature-unavailable`；
- I0.8 同步时 workspace 有 134 个通过的测试；I0.9 最终结构、依赖、质量和独立审查 gate
  仍需单独执行，不能据此声明完整 FCS 5 conformance。

上一版依赖基线修订后的 roadmap SHA-256 保留为：

```text
ebc374dd128e7272d06eb4491564770f8b7b218ff0e58df39f627abd73d72222
```

本次 I0 实施证据同步后的 roadmap SHA-256 为：

```text
8ea7db4a25fc06808d114cd247e9d83c087a1c8bcd450cdee8b1d2ea920de374
```

本次同步只修改实施路线、协作指南、状态矩阵、审查附录和 decision 中的 Chumsky 参考仓库
路径，并删除已被正式决策与计划取代的旧设计/计划副本。四份权威规范、规范治理文件、
conformance corpus 和全部 Frozen 版本均未修改。
