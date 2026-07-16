# Render Profile 1 Normative Amendment Review

日期：2026-07-16

状态：首个固定快照独立复审失败（Critical 2、Important 8、Minor 0）；修订后的 candidate hash 已由
未参与修改的只读 reviewer 复审通过（Critical 0、Important 0、Minor 0），`RNR-C01`–`RNR-I08`
normative gate 已关闭。Render Profile、FCBC Container 和 Execution ABI 因 I7/I9 artifact、其余
cross-spec blocker 与最终联合复审仍保持 Draft。

## 1. 触发与范围

在准备 RenderSection executable vector 时，对 `fcs-render.md` 的 Path 状态机、Paint/Stroke/Clip/
composite 非法值和 runtime descriptor error 做了只读审计。审计发现 2026-07-16
`render1-binary-raster-closure-review` 只读复审没有覆盖的三个 Important finding。它们会让独立
writer、loader、semantic evaluator 或 rasterizer 对同一合法/非法 bytes 做出不同选择，因此必须在
生成 static golden 前先修订规范。

本 amendment 只处理 Render Profile 1.0 的公开语义与其 FCBC 交叉引用；不修改外部 PGR/RPE/PEC
证据、不创建产品 renderer、不把 test-only harness 变成规范来源，也不改变候选 SemVer。

## 2. Finding ledger

| ID | Severity | 复现边界 | 处置 |
|---|---|---|---|
| REN-I08 | Important | 当前点不等于 Arc/EllipseArc 参数曲线 start 点时，现行文本没有规定隐式 line、跳点或拒绝；fill、stroke、dash 和 raster 因此不唯一。 | Arc/EllipseArc 总是先概念性追加当前点到参数 start 点的 LineTo，再绘制参数曲线；零长度连接线仍属于路径但不编码额外 command。 | 
| REN-I09 | Important | Core Line/Note direct root 的 exact path、owner、type、domain 与 `d/q` environment 只有 schema 片段，不能唯一生成 descriptor StructuralKey。 | 已在 `fcbc.md` 第 13/14 章加入 16 项 exact matrix；共享 root 使用所有 direct root environment 的交集，`p` 只在自身 Segment 内可见。 |
| REN-I10 | Important | Paint spread/stop、Stroke cap/join/miter/dash、Clip fillRule、composite/isolate 以及 descriptor execution failure 与 owner-invalid value 没有稳定 category。 | 增加 `render.invalid-paint`、`render.invalid-stroke`、`render.invalid-clip`、`render.invalid-composite`；descriptor evaluator failure 固定为 `render.invalid-descriptor`，成功但违反 owner field domain 时使用 owner-specific category。 |

REN-I09 的修订是由现有 FCBC schema、canonical traversal 和 reviewed loader 唯一推出的补全；REN-I08
和 REN-I10 是公开行为选择，已按用户确认的“表达优先、连接线参与完整 path 语义、稳定 owner
diagnostic”决定写入候选规范。

## 3. 规范性 delta

### 3.1 Path

`fcs-render.md` 第 7、14.4、15.2 章现在固定 Arc/EllipseArc 的参数点公式、角度单位、半径非负
约束、当前点到 start 的概念性 LineTo、zero sweep 与 multi-turn 行为。该线参与 fill、stroke、
dash 弧长和 flatten，但不改变 source command count、FCBC PathCommandRecord layout 或 topology。

### 3.2 Descriptor root

`fcbc.md` 新增 Core Line/Note 16-entry exact path matrix，并固定 owner stable ID、property type、
双向 unbounded domain、direct environment 和 shared-root 交集规则。Render root 仍受
`fcs-render.md` 第 14.8 节约束，kind 5 仍被拒绝。

### 3.3 Diagnostics

`fcs-render.md` 第 16 章现在把 framing/reference/resource、descriptor 执行失败和 owner field 值域
失败分层。完整 Record 但 `spread=4` 是 `render.invalid-paint`；descriptor 除零是
`render.invalid-descriptor`；成功返回 opacity `1.5` 是 `render.invalid-composite`。验证顺序按
FCBC structural/checksum/hash、Render section/record/graph/reference、owner tables、descriptor roots、
resource capability/decode、query-time evaluation 固定；更早 category 不被后续 category 覆盖。

## 4. Conformance additions

Render manifest/golden 必须增加以下可执行边界：

1. Arc/EllipseArc 的不同 start、相同 start、zero sweep with non-zero connector 和 multi-turn；
2. Paint/Stroke/Clip/composite 的非法 enum、count、值域及 isolate 组合；
3. descriptor evaluator error 与成功返回 owner-invalid value 的成对 mutation；
4. 每个新 stable category 至少一个同步 CRC 的 deep mutation，并保留未同步 CRC 的
   `fcbc.section-checksum` precedence case。

Static FCBC、独立 loader/evaluator、decoder/shaper、semantic draw list、reference raster 和 mutation
corpus 仍是跨规范 review 第 7.3 项的后续 gate。它们不得读取本文件或任何 expected 文件生成实际
结果。

## 5. Version and status

这些是尚未公开的 5.0.0/2.0.0/1.0.0 候选的规范性修订；按照治理流程保持五个版本域为 Draft，
直到本 amendment 的独立复审、Render executable artifact、Conversion round-trip、Core fixture
validation 和最终联合复审全部通过。旧 review 中固定 hash 与“0 finding”的结论只保留为其当时
快照的历史审计事实，不能用于当前 Frozen gate。

## 6. Independent review ledger

独立 reviewer 的固定快照、复现范围和失败 finding 记录在第 8 节。第 8 节的 hash 只标识该次审查
输入；后续修订不得改写其 finding 数量或把修订后的 bytes 冒充为已审快照。全部 Critical/Important
finding 在规范、fixture、实现和新的独立复审中关闭前，Render normative gate 保持 open。

## 7. Follow-up findings added before re-review

首轮独立复审又发现以下与 Core/ABI 交叉边界直接相关的 Important finding；它们已在本次候选修订中
一并处理，必须在新的固定 hash 上重新复核：

| ID | 复现边界 | 修订 disposition |
|---|---|---|
| REN-I11 | `fcs.md` Text schema 的 `size: length > 0` 与 Render GlyphRun 的“0 合法”矛盾。 | 统一为 `>0`；零/负成功值是 `render.invalid-geometry`。 |
| REN-I12 | Note source 只有 compile-time `visibleFrom/visibleUntil`，但 FCBC visibility root 缺少 lowering/environment。 | `fcs.md` 固定 interval→Constant/Piecewise bool；FCBC root 只依赖 `s`，source 不提供第二套动态 visibility object。 |
| REN-I13 | ABI 有 EnvP opcode，但 Piece context、nested binding 与无 context rejection 未定义。 | Piece 选择 inner descriptor 时绑定规范 `p`；所有 EnvP→root 路径必须跨 Piecewise，否则 `fcbc.invalid-expression`；direct root 不隐式提供 p。 |
| REN-I14 | Path open subpath 的 fill 是否隐式闭合未定义。 | fill 概念性追加 closing line；只参与 fill，显式 Close 才参与 stroke/dash；增加边界 fixture。 |
| REN-I15 | RadialGradient start/end radius 的负值语义未定义。 | start/end radius 必须 finite、非负；动态负值使用 `render.invalid-paint`。 |
| REN-I16 | Note/Line attachment 求值失败未与 Render descriptor error 对齐。 | FCBC structural error 仍用 `fcbc.*`；已加载 chart 的 transitive query-time Core failure 按 dependency order 使用 `render.invalid-descriptor`。 |

这些 follow-up 修订没有改变 REN-I08 的 connector 选择，也没有新增 FCBC source snapshot、baking 或
外部 resource fallback 语义。由于它们改变了 `fcs.md`/`fcbc.md` candidate bytes，旧首轮 hash 不能
继续作为当前审计输入。

## 8. 2026-07-16 fixed-snapshot independent review：FAIL

未参与第 1–7 节候选修订的只读 reviewer 固定并复核了以下输入：

| 文件 | SHA-256 |
|---|---|
| `fcs.md` | `37CFF422B6CFBA64E7A0E5541A366A7E93D2E89A2EB18F069186AB28452D41F5` |
| `fcbc.md` | `BDC9C19D4C4F4C7EDB2A57A38A45B91244F9E07D24B168CB9ED6FC92CB5708BB` |
| `fcs-render.md` | `B066E7F403B9B04761261AB254C46A39462700DA9E221D1C42C76001D4357FC8` |

审查覆盖 Core scroll dependency、Render active/visibility 查询顺序、FCBC generic/profile-specific
validation precedence、resource decode 与 geometry 分类、attachment 变换与依赖、stroke 退化边界，
以及 shared descriptor 的完整 owner/environment 交集。结论为 **FAIL**：Critical 2、Important 8、
Minor 0。reviewer 摘要曾误写 Important 7；逐项 ledger 明确包含 `RNR-I01` 至 `RNR-I08` 共 8 项，
本记录以逐项 ledger 为准并保留该计数勘误。

| ID | Severity | Finding | Candidate disposition before re-review |
|---|---|---|---|
| RNR-C01 | Critical | `line.scrollTempo` 可依赖 `q`，而 `q` 又由 scroll integration 产生，形成跨语义自循环。 | 候选修订已解决，等待新固定 hash 独立复审：`fcs.md` 第 10、12、13 章和 `fcbc.md` 第 13、14 章固定 `scrollTempo -> s,b`、`scrollSpeed -> s,b,q`，并拒绝到 EnvQ/EnvD 的 direct/transitive path。 |
| RNR-C02 | Critical | inactive Node 的“无需求值”规则与先求 attachment/visibility descriptor 的顺序冲突，无法确定哪些 query-time 错误可被跳过。 | 候选修订已解决，等待新固定 hash 独立复审：`fcs-render.md` 第 3.3、4、15、16 章固定 active→Note gate→最小 attachment dependency→Node visibility→剩余 root，并区分不可隐藏的 load-time failure。 |
| RNR-I01 | Important | Render owner 的 environment 违规既可落入 `fcbc.invalid-expression`，也可落入 `render.invalid-descriptor`，分类不唯一。 | 候选修订已解决，等待新固定 hash 独立复审：`fcbc.md` 第 13、14、16、17 章与 `fcs-render.md` 第 14.8、16 章把 intrinsic ABI/Core owner failure 与仅由 Render owner 收窄产生的 failure 分层。 |
| RNR-I02 | Important | FCBC 通用 section/reference validation 可先于 Render graph/reference validation 抢占错误分类。 | 候选修订已解决，等待新固定 hash 独立复审：`fcbc.md` 第 13、14、16、17 章与 `fcs-render.md` 第 16 章固定 generic loader/profile validator seam、完整 ownership 建立时机和 precedence。 |
| RNR-I03 | Important | TrueType composite glyph limit 的失败可同时解释为 resource decode failure 与 limit failure。 | 候选修订已解决，等待新固定 hash 独立复审：`fcs-render.md` 第 11、14.7、16 章区分 malformed index/cycle、已解码 glyph 语义值和公开 resource/work limit。 |
| RNR-I04 | Important | viewport、Layer 与 Node 的非法数值缺少稳定 parent category。 | 候选修订已解决，等待新固定 hash 独立复审：`fcs-render.md` 第 14、16 章为 viewport、Layer、Node、attachment 和 query-time owner 值固定 stable parent 与联合验证顺序。 |
| RNR-I05 | Important | GlyphRun 对 glyph 0 的规则与 TrueType `.notdef` 及测试 loader 的接受范围矛盾。 | 候选修订已解决，等待新固定 hash 独立复审：`fcs-render.md` 第 11、14.7、16 章只接受 `1 <= glyphId < numGlyphs`；glyph 0、越界 glyph 和 `faceIndex != 0` 使用 `render.invalid-geometry`。 |
| RNR-I06 | Important | attachment matrix、Core dependency order、visibility gate 与样式继承均未闭合。 | 候选修订已解决，等待新固定 hash 独立复审：`fcs-render.md` 第 3.3、4 章固定四种 attachment matrix、Note gate、`followHiddenAttachment` 与“只继承几何、不继承样式”。 |
| RNR-I07 | Important | stroke 中零长度 segment 对 cap、join、dash phase 和 raster coverage 的影响未定义。 | 候选修订已解决，等待新固定 hash 独立复审：`fcs-render.md` 第 7、15.2 章固定 topology/arclength、coverage/cap/join/tangent 与 dash phase 行为。 |
| RNR-I08 | Important | shared descriptor 的完整 owner/environment intersection 何时验证、失败归谁尚未闭合。 | 候选修订已解决，等待新固定 hash 独立复审：`fcbc.md` 第 13、14、16、17 章与 `fcs-render.md` 第 14.8、16 章要求完整 Core+Render+required-extension owner 集合和 environment 交集。 |

本次失败不回退已经独立关闭的 Execution ABI 非空 artifact，也不表示第 1–7 节的修订方向被撤销。
后续先在新的 candidate bytes 中逐项关闭 `RNR-C01`–`RNR-I08`，补对应 fixture/loader assertions，再由
未参与修订的 reviewer 固定新 hash 复审。旧 hash 永久保留为失败快照，不能用于 Reviewed、Frozen
或 implementation-baseline gate。

截至 2026-07-16，本表“候选修订已解决”只表示 normative candidate 已给出单值答案，不把 finding
冒充为独立关闭。当前 test-only evidence 已覆盖 scrollTempo/scrollSpeed environment、Render-owned
Node/attachment category、font 解码后的 glyph/face 边界与 CRC precedence；active/visibility query、
attachment matrix、zero-length stroke 和完整 shared-owner intersection 的 executable semantic/raster
artifact 仍由 I9 交付。新的只读 reviewer 必须同时判断：这些后期 executable artifact 是否影响 I1
source AST/parser dependency closure；若不影响，只将其保留为 I9 blocker，不得恢复五域 Frozen 的
I1 全局门。

## 9. Pre-stage test-only artifact boundary

当前 Render manifest 不再声明不存在的 `nonempty-render.toml`、binary vector、mutation、semantic JSON
或 raster expected；`binary_fixture` 计数固定为 0。项目自制 PNG/WebP/TTF、deterministic generator、
restricted decoder/shaper、test-only writer/independent loader 与 CRC-aware focused mutation仍保留，完整
static golden→semantic evaluator→reference raster→mutation corpus 由 I9 交付。

为保留这些 test-only 证据，`fcs-source` 提前以 dev-dependency 激活 `image` 和 `serde_json`。这是
pre-stage conformance test-only exception，不改变 I3/I6/I9 的产品 dependency ownership，也不创建
Render 产品 API。

另有一个已显式路由的历史 artifact 差异：旧 nonempty Execution ABI golden 的
`note.presentation.visibility` expression 使用 EnvB，而当前 `fcs.md`/`fcbc.md` 要求该 root 只依赖
`s`。当前 test-only Core loader 为保留已复审 bytes 暂不对该单一 root 执行 `s`-only 特例；它不是完整
conforming product loader。I7 baseline 前必须重建对应 golden/vector/mutations 并独立复审；由于该
差异不改变 I1 lexer/AST/parser/parse-stage diagnostic，它不是 I1 blocker。

## 10. 2026-07-16 candidate snapshot：等待独立复审

完成上述候选修订、focused mutation 和诚实 manifest baseline 后固定当前规范 bytes：

| 文件 | Candidate SHA-256 |
|---|---|
| `fcs.md` | `2A2882E60AEEF4D96FDB9F7C3CD65B143EA9D9C61F971ECE24A8B2791627DC58` |
| `fcbc.md` | `245E1E41EBB20C94BAF644D284994820487C3CC2DF7C75463F37C3DB04B6F99A` |
| `fcs-render.md` | `848D9AB581529179FC91047737B846A04EE0469AFFAAB43377F47C66C20DB4C3` |

本 snapshot 上的执行证据为：Clippy PASS；`conformance_manifest`、`fcbc_execution_abi`、
`fcbc_render_section` 共 16 项 nextest PASS；`git diff --check`、项目文本 UTF-8/NUL audit 和本地
Markdown link audit PASS；随后全 workspace nextest 149/149、`cargo fmt --all -- --check` PASS。该证据
只说明候选输入可复现，不构成独立 review。未参与修订的只读 reviewer 必须从上述三个 hash 开始
复核 `RNR-C01`–`RNR-I08`，并分别判断剩余 I7/I9 artifact blocker 是否会改变 I1 source parser 的
公开 contract。在 reviewer 记录结果前，本文件状态仍是 FAIL 后的 candidate remediation，Render
版本域仍为 Draft。

## 11. 2026-07-16 fixed-candidate independent re-review：PASS

一名未参与第 8–10 节规范、fixture、loader 或文档修改的只读 reviewer 首先复算并确认第 10 节三个
candidate hash 完全一致，然后直接复核固定 bytes；test-only writer/loader/tests 只作为证据，不作为
规范来源。

逐项结果：

| ID | Independent disposition | Owning closure |
|---|---|---|
| RNR-C01 | Closed | `fcs.md` 第 10、12、13 章与 `fcbc.md` 第 13、14、18 章固定 scrollTempo 因果 root 与 EnvQ/EnvD 禁止路径。 |
| RNR-C02 | Closed | `fcs-render.md` 第 3.3、4、16 章固定 active→Note gate→最小 attachment dependency→visibility→remaining root，并禁止隐藏 load-time failure。 |
| RNR-I01 | Closed | `fcbc.md` descriptor/Expression owner validation 与 `fcs-render.md` 第 14.8、16 章唯一分层 intrinsic/Core 和 Render-only environment failure。 |
| RNR-I02 | Closed | Generic FCBC 与 Render profile validator 的职责、完整 owner 建立时机和 category precedence 已单值化。 |
| RNR-I03 | Closed | `fcs-render.md` 第 11、14.7、16 章区分 malformed font、公开 work/shape limit 和已解码 glyph semantic failure。 |
| RNR-I04 | Closed | `fcs-render.md` 第 16 章联合顺序为 viewport、Layer、Node、attachment 与 owner value 提供稳定 parent。 |
| RNR-I05 | Closed | `1 <= glyphId < numGlyphs`、`faceIndex=0` 与 glyph 0 禁止规则一致，语义非法值稳定为 `render.invalid-geometry`。 |
| RNR-I06 | Closed | `fcs-render.md` 第 3.3、4 章完整固定 attachment matrix、Note gate、follow-hidden 和 geometry-only/no-style inheritance。 |
| RNR-I07 | Closed | `fcs-render.md` 第 7、15.2 章固定零长度 segment 的 topology/arclength、coverage/cap/join/tangent 与 dash phase。 |
| RNR-I08 | Closed | `fcbc.md` 与 `fcs-render.md` 第 14.8、16 章要求完整 Core+Render+required-extension owner/environment 交集，禁止 first-owner/visible-only shortcut。 |

Finding ledger：

| Severity | Open findings |
|---|---:|
| Critical | 0 |
| Important | 0 |
| Minor | 0 |

Reviewer 另确认第 9 节记录的 legacy visibility EnvB golden 不符合当前 `s`-only owner rule，但该差异
被诚实限定为历史 test-only artifact，必须在 I7 baseline 前重建并复审；它不改变 I1 source
lexer/AST/parser/parse-stage diagnostic。完整 Render semantic/raster/mutation artifact 同样保持 I9 blocker。

因此本次只关闭 `RNR-C01`–`RNR-I08` 的 normative gate，不关闭 I7/I9 executable artifact，不提升
任何版本域为 Reviewed/Frozen，也不授权完整 FCBC/ABI/Render conformance 声明。
