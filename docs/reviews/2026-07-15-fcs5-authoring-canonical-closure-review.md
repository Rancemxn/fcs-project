# FCS 5.0 Authoring / Canonical Closure Review

日期：2026-07-15

状态：Core delta 已写入；等待 FCBC/Render/Conversion 同步与独立复审，尚未 Frozen

## 1. 授权与范围

用户已接受 ADR 0007–0009，并确认：

- FCS 是 authoring/source 格式；
- 制谱器使用普通目录 workspace 保存 FCS 与资源；
- FCBC 是恰好一个谱面、内嵌全部资源的播放器最小输入；
- 标准 FCS→FCBC 保留 exact runtime expression；
- baking 不是谱师选项或 packager 默认行为，只用于显式 target approximation；
- 播放器 sampled cache 是默认关闭的本地实现配置，不进入 FCS/FCBC。

本 review 只关闭 `fcs.md` 的 authoring、canonical 和 exact execution 边界。FCBC record layout、
Render resource encoding 和 PGR/RPE/PEC profile registry 仍由后续规范同步负责；本轮不开始 I1
Rust 实现。

## 2. 章节级 delta ledger

| 章节 | 原问题 | 已写入的规范决定 | 后续规范依赖 |
|---|---|---|---|
| 1.1–1.4 | Source/Canonical/Fidelity/FCBC 混成三层，播放器输入与 workspace 不清楚 | 固定 SourceDocument→ExpandedSourceDocument→CanonicalCompilation→FCBC；定义 CanonicalChart、CanonicalResourceBundle 和 DistributionMetadata；播放器只需 FCBC | FCBC loader/writer、Render host boundary |
| 4.5、6.8 | 编译期结构“消失”未区分 exact runtime expression | comment/template/generator/local 等 authoring-only 结构消失；typed runtime expression 必须保留 | I2 ExpandedSourceDocument、I4 DAG |
| 7.3 | Resource `source` 是宿主 URI，可能依赖网络、工作目录或 runtime path | 改为 component-normalized workspace member path；禁止 URI/data URI/escape；解析 opaque bytes、验证 SHA-256、生成完整 resource bundle | FCBC ResourceData、Render resource ID |
| 8.1–8.3 | 外部 line BPM/bpmfactor/tick 与唯一 runtime clock 的关系不够明确 | 外部 time base 只参与 import-time decoding；映射后只有 chartTime，原始值只进 provenance/report | Conversion semantic profiles |
| 10.4 | 无闭式积分容易被理解为必须预烘焙 | standard exact path 保存 analytic 或 evaluable integrand；runtime 数值积分不把采样点升级为谱面语义 | ABI Distance/evaluator |
| 12.2–12.4 | judgeShape、soundPolicy、scorePolicy 只有字段名与默认值，没有完整 schema/组合规则 | 定义 lineDefault/rectangle/circle、default/none/resource sound、default/none/custom score、disabled normalization、extension/resource diagnostics；visual position 不反向定义 judge geometry | FCBC NoteRecord、未来 gameplay host policy |
| 13.4 | 默认 lowering 链以 adaptive baking 结束 | exact Constant/Track/Piecewise/DAG 是完整标准链；无法静态化不触发 baking | FCBC PropertyDescriptor/Expression |
| 14.1–14.2 | Adaptive baking 被描述成普通 Core compile 类别 | 改为显式 target approximation；播放器 sampled cache 完全本地且不具 strict exact conformance | ConversionReport、播放器实现（非格式） |
| 15.2 | Preserve raw snapshot 可能流入 CanonicalChart/FCBC | preserve 只服务 authoring round-trip；只能提取非原文 provenance fact，禁止 source snapshot/AST | FCBC 删除 SourceSnapshot |
| 17 | Canonical lowering 把 resource、distribution metadata 和 FCBC serialization 混在一起 | 定义 CanonicalCompilation 三部分，standard writer one-chart/self-contained/exact；明确 canonical equivalence | FCBC/Render/Conversion |
| 18 | Fixture 没有覆盖 authoring equivalence、no-bake、workspace 和 Note policy | 新增 7 个 canonical/error fixture，保留 S14 原 32 项，总数 39 | I1–I5 runner 实现 |

## 3. 新增 conformance 绑定

| Fixture ID | Stage | 关键预期 |
|---|---|---|
| `source.valid.canonical-equivalent-direct` | canonical | 与 template/preserve 版本使用同一 canonical expected |
| `source.valid.canonical-equivalent-template` | canonical | template、local 与 raw preserve payload 不进入 canonical |
| `source.valid.exact-expression-dag` | canonical | `choose`/`sin` lowering 为 ExpressionDAG，禁止 BakedCurve |
| `source.valid.note-policies` | canonical | rectangle/circle、resource sound、disabled normalization、required custom score extension |
| `source.invalid.note-policy-disabled-sound` | canonical | disabled judgment 与 resource sound 组合使用 `schema.non-constructible` |
| `source.invalid.resource-path-escape` | canonical | `..` workspace escape 使用 `resource.unknown-reference` |
| `source.invalid.resource-hash-mismatch` | canonical | 原始 payload SHA-256 不匹配使用 `resource.hash-mismatch` |

资源 fixture 使用 opaque payload：

```text
relative member: assets/opaque-resource.bin
SHA-256: 66eb55e69c42345c65021ea9364fc43c61d2151dde67a89dc02362543b289903
```

Root/FCS conformance manifest schema 升为 2：root 的瞬时状态字段改为 `candidate_baseline`，
`workspace_root` 只为 canonical resource fixture 声明 resolver root，不改变 FCS source grammar。

## 4. 明确不在本轮完成的内容

- FCBC ResourceData record、offset、alignment、hash/checksum 和 loader order；
- `fcbc.md` 中 SourceSnapshot 删除、BakedCurve profile 限制与 NoteRecord 新字段编码；
- Render image/font/texture/path/shader 引用解析到 FCBC payload；
- PGR/RPE/PEC source/target profile ID、mapping registry 和 ConversionReport schema；
- input timing window、设备 gesture sampling、全局 score formula 和播放器 default-hit-sound PCM；
- 播放器 sampled cache 文件格式、持久化和算法；
- I1–I5 Rust semantic implementation。

## 5. 后续交叉规范 gate

Core 可以进入独立 review 前，至少必须：

1. 检查 Appendix B 不因 dotted Note field 或 workspace path 规则产生 grammar 分叉；
2. 强类型加载 39-entry FCS manifest，并验证 workspace root/expected path 都在 conformance tree 内；
3. 验证两个 opaque resource fixture 的 bytes 与声明 SHA-256 一致；
4. 检查 standard exact compile、explicit approximation 和 player sampled cache 三条路径没有交叉；
5. 在 `fcbc.md` 删除外部 package member/SourceSnapshot/default baking 冲突；
6. 在 `fcs-render.md` 绑定 canonical resource ID→FCBC embedded payload；
7. 在 `fcs-conversion.md` 用 versioned profile 完成 import-time source time decoding；
8. 更新 roadmap、implementation matrix 和 I1 precondition；
9. 运行 Clippy、nextest、rustfmt check、document/link audit 和 `git diff --check`；
10. 独立 reviewer 关闭所有 Critical/Important 后，才可把相应规范从 Draft 改为 Reviewed。

该 gate 完成后仍需五版本域的统一 re-freeze review；本文件不能单独授权 I1 开始。

## 6. Core 候选自检证据

本节只锁定当前 authoring/canonical delta 的候选内容，证明它可以进入后续跨规范同步与独立复审；
它不是独立 reviewer 结论，也不把 FCS Core 状态改为 Reviewed 或 Frozen。

### 6.1 Grammar、manifest 与资源

- Core Appendix B：117 个 production，未定义引用 0，直接左递归 0；
- Root/FCS conformance manifest schema 均为 2；FCS manifest 强类型加载 39 entries；
- `workspace_root`、source、expected 和 raster 路径均由 manifest integrity test 验证位于
  conformance tree 内，resource fixture 的 source 位于其声明的 workspace root 内；
- 两份 `assets/opaque-resource.bin` 的原始 bytes 相同，SHA-256 均为
  `66eb55e69c42345c65021ea9364fc43c61d2151dde67a89dc02362543b289903`。

### 6.2 候选内容哈希

单文件哈希按文件原始 bytes 计算：

```text
fcs.md
caafe4c91d46f6015060b83d1a5f82ddf3c435f52cf19756dc1c1d3b6ac4cd7d

conformance/fcs5/manifest.toml
4d8dfb7ba2d636cd94a02f39807c7a0da6185b73213f50dd77e8f2a69c5b25f4

conformance/manifest.toml
2563bb91f450a04075689b87bdb9ce6155cad929fb3468a7b45e32e6eb389c1c
```

Conformance tree 使用相对于 `conformance/`、以 `/` 分隔的 ordinal path order，并对每个文件依次
输入 `UTF-8(path) + NUL + bytes + NUL`：

```text
conformance tree (63 files)
a18e25c38123a3b557740fc76d84c1eed8f00827fa2d66ba7c6204bd0fa1b3e3
```

### 6.3 I0 回归门禁

```text
cargo clippy --workspace --all-targets -- -D warnings  PASS
cargo nextest run --workspace                         PASS 135/135
cargo fmt --all                                      PASS
cargo fmt --all -- --check                           PASS
git diff --check                                     PASS
Markdown UTF-8/NUL/local-link audit                  PASS 27 files
conformance text UTF-8/NUL audit                     PASS 61 files
```

135 项通过只证明 manifest/corpus 没有破坏 I0 workspace 和既有 characterization；39-entry 中新增的
canonical/resource/Note policy 语义仍标为未来 I1–I5 实现，不得把 manifest integrity test 解释为
新 parser、elaborator 或 canonical compiler 已经实现。

## 7. 后续 FCBC closure 对候选树的影响

本 review 写成后，S15 下一依赖节点又把 FCBC corpus 升为 schema 2，增加 required ResourceData、
embedded-resource golden 和 mutation。第 6.2 节的 `fcs.md` 与 FCS manifest hash 仍对应同一 Core
delta；其中 root manifest 与整个 conformance tree hash 只保留“Core closure 完成时”的阶段证据，
不再是当前仓库 tree hash。后续值与 FCBC/ABI delta 见
`2026-07-15-fcbc2-execution-abi-closure-review.md`，最终 re-freeze 仍须统一复算全部版本域。
