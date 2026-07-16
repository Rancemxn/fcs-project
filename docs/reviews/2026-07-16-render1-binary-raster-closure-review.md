# Render Profile 1 Binary/Raster Normative Closure Review

日期：2026-07-16

范围：`fcs-render.md` 的 RenderSection 1.0、resource decode/shaping、semantic scene 与 reference
raster 规范文字，以及 `fcbc.md` 中与 Render owning section、resource binding 和 diagnostic
precedence 直接相交的条款

结论：Render binary/raster 的规范选择已闭合并通过独立只读复审；完整 static FCBC、decoder、
shaping、semantic、raster 和 mutation artifact 尚未实现，因此 Render Profile 1.0.0 仍为 Draft，
本文不关闭跨规范 review 第 7.3 项，也不授权 Frozen。

## 1. 审查边界

现有 Render source、opaque resource binding、semantic JSON 和 solid-rect RGBA fixture 不能回答
“第三方只按规范能否生成并读取唯一 RenderSection bytes、得到相同 draw list 与 reference pixels”。
本轮先关闭会迫使 writer、loader、decoder 或 rasterizer自行选择语义的规范空白：

```text
source/canonical scene
  → stable Render ownership graph
  → exact RenderSection records
  → FCBC descriptor/resource binding
  → deterministic decode/shaping
  → semantic draw list
  → reference raster/output
```

本轮没有编写或接受 future test-only writer/loader/rasterizer，没有生成 static golden，也没有使用
未来实现的输出反向决定规范。外部 PGR/RPE/PEC 行为和 `refer/` 不属于本审查范围。

## 2. 初始 finding ledger 与处置

初始只读审计得到 1 个 Critical、6 个 Important finding。最终 disposition 如下：

| ID | Severity | 原问题与可复现分歧 | 最终 disposition |
|---|---|---|---|
| REN-C01 | Critical | RenderSection 没有 viewport width、height 或 color space；仅持有 FCBC 的播放器无法恢复 source viewport。 | **Closed**。singleton header 固定三项 viewport 字段、范围、enum、逻辑坐标到 output pixel 的唯一映射。 |
| REN-I02 | Important | Path、Paint、Stroke、Clip、GlyphRun 只有叙述字段；Record prefix、字段宽度、array 和 byteLength 不唯一，Stroke 甚至没有 paint binding。 | **Closed**。八张 table 和全部 variant 已逐字段固定，Record 使用 exact-length；Stroke 明确 owns Paint，GlyphRun 禁止 source-text/cluster tail。 |
| REN-I03 | Important | Layer/Node/auxiliary stable ID、table order、root partition、ownership、sharing、orphan 与 reachability 不完整。 | **Closed**。八个 namespace、textual ID、documentOrder、table order、root range、single-owner graph、全局 auxiliary collision 与 no-orphan 规则完整固定。 |
| REN-I04 | Important | Render descriptor direct root 缺少 exact path、owner、property type、domain 与 environment，无法唯一排序、type-check 或判定 `d/q`。 | **Closed**。第 14.8 节枚举所有 path 与 ordinal，绑定 owning stable ID、property type、active domain、attachment environment 及 kind 1–4；kind 5 仍稳定拒绝。 |
| REN-I05 | Important | Image/texture metadata、PNG/WebP capability、color/alpha conversion、texel coordinate、edge/repeat 与 sampling 不唯一。 | **Closed**。metadata exact object、PNG/static-lossless-WebP profile、transfer/alpha 顺序、local inverse mapping、texel center、nearest/linear 和 Image/ImagePattern edge/repeat 均已固定。 |
| REN-I06 | Important | Font container、cmap、shaping、fallback、metric、outline 与 glyph raster 不唯一，且通用 future Record tail 可绕过 no-source-text。 | **Closed**。`truetype-glyf-1` 与 `simple-ltr-1` 固定 single-face TTF、table/cmap、无 hinting/kerning/ligature、fallback split、em metric、outline/raster；Render 1.0 Record 全部 exact-length。 |
| REN-I07 | Important | Flatten/isolate/clip/pass、paint/composite、viewport/sample/output 与 `fcbc.*`/`render.*` precedence 不唯一。 | **Closed**。层级 traversal、root-only attachment、isolated offscreen、coverage/flatten、paint/composite、8×8 raster、RGBA8 output 和 13 层 diagnostic precedence 已固定。 |

最终没有把 implementation convenience、宿主 image/font service 或外部文档行为提升为规范真相。

## 3. 固定规范快照

独立复审开始和结束均核对了原始文件 bytes；两个 hash 在复审期间保持不变：

| 文件 | SHA-256 |
|---|---|
| `fcs-render.md` | `95685f44fae88c26126e4dc34a13793499fd65b6eb61b021ca1f56156470cad1` |
| `fcbc.md` | `fc1bc9b8032d7ac88d16068e08cb3d8907a25b83fc752db85a7679e1ebed1c33` |

`fcbc.md` 的 hash 已不同于 2026-07-16 Execution ABI artifact review：本轮只增加/澄清 Render owning
section、Render namespace、exact-length、resource metadata 与联合 validation/diagnostic 规则，不重写
非 Render Execution ABI golden 的 payload 语义。最终跨规范独立复审仍须在届时完整候选 hash 上
复核两部分，不能把两个 dated review 的旧 hash 拼接成 Frozen 证据。

## 4. Binary layout 与 canonical graph

RenderSection type 14 固定为 required、8-byte aligned、version 1.0.0 的 exact-length singleton。
header 编码 profile、flags、viewport 和八张 table count。Layer 是组织记录而不是 Node；Node table
显式分为按 Layer 排列的 root partition 与 parent-before-child descendant partition。

独立 reviewer 人工复算的 Record 总长度为：

| Record | byteLength |
|---|---:|
| Layer | `36` |
| Node | `108 + encodedCustomValueLength`；empty object 时 `124` |
| Geometry | `20 + encodedPayloadValueLength` |
| Path | `24 + sum(PathCommand.byteLength)` |
| MoveTo / LineTo | `16 / 16` |
| QuadraticTo / CubicTo | `20 / 24` |
| Arc / EllipseArc / Close | `32 / 40 / 12` |
| Solid / LinearGradient / RadialGradient / ImagePattern | `24 / (36+16n) / (44+16n) / 48` |
| Stroke | `48 + 8*dashCount` |
| Clip | `24` |
| GlyphRun | `60 + 40*glyphCount` |

Stable ID 对 Layer、Node、Geometry、Path、Paint、Stroke、Clip 与 GlyphRun 使用独立 namespace。
Auxiliary textual ID 由 exact owner edge、field name 和 ordinal 派生；Render auxiliary stable ID 在整个
section 内全局非零且无 collision。每个 auxiliary record 恰好一个 owner并从 Layer root 可达；禁止
orphan、cross-owner sharing 和 hidden payload。Resource identity仍只来自 FCBC ResourceRecord，允许
多个 Render owner引用同一个 immutable resource。

## 5. Scene、resource 与 execution closure

复审确认下列边界没有第二套合法解释：

- attachment 只允许 root 真正建立并只应用一次；descendant保存 effective attachment用于 environment，
  但 world matrix 不重复乘 attachment；
- inactive 或 visibility-false node 移除整个 subtree；non-isolated Group 原位展开，isolated Group 在
  相同 viewport/sample grid/premultiplied buffer 的透明 offscreen绘制，并只在回合成时应用一次 group
  opacity/composite；
- pass anchor、Layer order、递归 sibling order 与 fill→stroke→children 顺序固定；
- Clip Path 的 fillRule 必须等于 PathRecord；Arc/EllipseArc 使用 Y-up 有符号 sweep；
- Render direct root 的 target path、owner、type、domain、environment 与 FCBC StructuralKey traversal
  完整绑定；
- Image/ImagePattern 在 geometry/paint local coordinate采样；PNG、static lossless WebP、sRGB/
  linear-sRGB、straight/premultiplied 与 alpha-zero边界固定；
- Core font只接受 fixed TrueType profile；cmap、fallback split、glyph metrics、outline和 8×8 coverage
  固定，FCBC GlyphRun 不保存 input text、cluster 或 cursor；
- RadialGradient 显式固定 `A/B/C`、discriminant、linear/constant degeneration、infinite-root case
  `t=0`、逐 binary64 operation 和最大有限合法 root；
- viewport→pixel、curve flatten、stroke/dash、paint/composite、pixel accumulation、unpremultiply、
  output transfer 和 ties-to-even RGBA8 编码均有唯一规则。

## 6. Stable diagnostic precedence

`render.invalid-section` 与 `render.invalid-record` 分离 section envelope 和 nested Record/Value framing。
联合 loader 先处理 FCBC framing/checksum/ResourceData hash，再处理 Render profile、section、record、
graph、reference、geometry、descriptor、resource identity/type/capability/decode，最后处理 evaluated value。
未同步 Render section CRC 的 deep mutation必须停在 `fcbc.section-checksum`；同步 CRC 后 descriptor
kind 5仍返回 `fcbc.forbidden-descriptor`。

## 7. 独立复审结果

最终 reviewer 未参与本轮规范修改，按固定 hash 只读复核：

- root-only attachment、isolate、ownership/collision、straight→premultiplied 与 local image sampling；
- Clip fill rule、pass anchor、Arc direction、cmap selection、viewport/output；
- RadialGradient 的全部退化与逐操作公式；
- 第 14 章全部 Record 长度公式；
- 初始 REN finding 的逐项 disposition。

Finding ledger：

| Severity | Open findings |
|---|---:|
| Critical | 0 |
| Important | 0 |
| Minor | 0 |

## 8. Gate 与下一工作单元

这份 review 只关闭“Render binary/raster prose 是否仍要求实现自行猜测”的前置 gate。跨规范 review
第 7.3 项继续开放，直到完成并独立执行：

```text
fixed declaration
  → test-only writer
  → checked-in static FCBC
  → independent loader
  → semantic evaluator
  → PNG/WebP/font decoder + shaping
  → reference raster
  → deep/checksum mutation corpus
```

Artifact 必须覆盖八张非空 table、所有 Core geometry/path/paint/stroke/clip/GlyphRun variant、descriptor
kinds 1–4 与 kind 5 rejection、项目自制内嵌资源、no-source-text 和 stable diagnostic precedence。
完成该 artifact 及其独立复审前，Render Profile、FCBC Container 和其他版本域均保持 Draft；I1 gate
不因此开启。

## 9. 2026-07-16 dated amendment：executable-vector audit 重新打开规范 gate

后续 executable-vector 准备没有改变本文第 1–8 节在固定 hash 时的历史审计事实，但发现本文独立
复审未覆盖的 `REN-I08–I10`：Arc/EllipseArc 与此前当前点的连接语义、Core Line/Note exact
descriptor root matrix，以及 Paint/Stroke/Clip/composite 与 descriptor execution 的稳定诊断分层。
这些都是第三方 writer/loader/evaluator/rasterizer 不能自行选择的公开 contract。

因此本文第 9–11 行“规范选择已闭合”和第 140 行“只关闭 prose 前置 gate”的结论从当前 gate 角度
撤回；旧 hash 和 finding ledger 仅保留为当时快照证据。新修订、用户确认的语义选择、conformance
增量和独立复审入口统一记录在
`docs/reviews/2026-07-16-render1-normative-amendment-review.md`。该 amendment 完成独立复审前，不得
在旧固定 hash 上继续生成 normative executable golden，也不得把本文的 0-finding ledger用于 Frozen。
