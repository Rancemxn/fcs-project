# Render Profile 1.0 Resource Binding Closure Review

日期：2026-07-15

状态：Render resource/exact-runtime delta 已写入并完成候选自检；等待 Conversion 同步、完整跨规范
review 与独立复审，尚未 Reviewed/Frozen

## 1. 范围

本轮在 S14 已闭合的 Render source envelope/EBNF 上，关闭 ADR 0008–0009 引出的运行时输入边界：

- FCS source `@resource` 在 authoring workspace 解析；
- canonical Render scene 只保存 stable resource ID/kind/metadata/content hash；
- FCBC Resources section 6 保存目录，ResourceData section 20 保存原始 bytes；
- renderer 只接收 bounded loader 验证后的 immutable resource view；
- RenderSection 不保存路径、URI、payload/hash 副本、source text/cluster 或外部 fallback；
- 动态 Render 属性只引用 FCBC exact descriptor kind 1–4，BakedCurve/sample cache 不进入格式。

本轮不实现 Render-aware parser、decoder、shaper、rasterizer 或 GPU backend，也不把 opaque binding
fixture 冒充可解码图片 fixture。

## 2. 章节级 delta

| 章节 | 决定 |
|---|---|
| 1–2 | 固定 FCS `@identifier`→canonical stable ID→FCBC Resources/ResourceData→RenderResourceView 流程；Render source 不接受 path/URI/data URI/system-font lookup |
| 8、10 | Image/ImagePattern identity 编译期固定；定义 Image source field；RenderSection 只写 ID/descriptor；FCBC CRC/layout/SHA 先于 image kind/media/decode；Core decoder set 明确为 static PNG 与 static lossless WebP 边界 |
| 11 | Shaping 使用内嵌 font bytes；fallback font 必须显式内嵌；标准 GlyphRun 不保存 source text/cluster/authoring cursor mapping |
| 12 | Track/Piecewise/choose/Expression 只映射 descriptor kind 1–4；kind 5 使用 `fcbc.forbidden-descriptor`；播放器 sampled cache 对 RenderSection 不可见 |
| 13 | Shader/path/binary/texture extension 输入必须列出 stable ID/kind/media/profile/limit；include/sidecar/网络/文件查找禁止 |
| 14 | RenderSection flags=0，没有 resource table；Image、ImagePattern、GlyphRun 等 ID 必须解析到同一 FCBC 的 image/texture/font payload；无 fallback search path |
| 15–17 | Raster fixture 绑定 FCBC ID/kind/metadata/raw bytes/hash；增加 decoder/renderer limit 与稳定 category；新增 semantic-only resource binding fixture |

`fcbc.md` 第 16.1 节同步写入相同约束，因此 FCBC loader 完成 resource validation 后才验证/暴露
Render resource reference，Render payload 不得覆盖 ResourceRecord metadata/hash。

## 3. Binding fixture

`render.binding.embedded-image-resource` 使用普通目录 `docs/conformance/render/binding/` 作为 fixture
workspace：

```text
source: binding/resource-image.fcs
workspace member: binding/assets/opaque-image.bin
payload length: 29
payload SHA-256: 66eb55e69c42345c65021ea9364fc43c61d2151dde67a89dc02362543b289903
canonical textual ID: sprite
stable resource ID: 43b6b71ca2ee7437
kind/media type: image / image/png
FCBC directory/payload section: 6 / 20
decode: false
```

Opaque bytes 故意不是 PNG。该 fixture 只验证 workspace path/hash、canonical ID 和 FCBC binding，
不进入 decoder；若把它交给 PNG decoder，预期应是 `render.resource-decode-failed`，不能成功 raster。
实际 image/font decoder 与 reference raster 仍需独立固定资源向量。

## 4. 候选内容哈希

```text
fcs-render.md
ff225be16c41a1c38bf05009855659ebabadc4c047f4583d29c6a40cf84f32c4

fcbc.md（同步 Render binding 后）
957cc9f10702756ea5589c785237528daefd392b038b97db75383536054ad4d6

docs/conformance/render/manifest.toml
9deafb31ecf7bbd9a992bf7f487364cb274f43e70811013d8f2cc890bfff40aa

docs/conformance/render tree (9 files)
947706cc1ea3ead7c12d9de022dedf55b7aad31b8a66ed1f449fd4e33b0d8f4c

conformance tree (70 files)
569a417ff3dd5f9c57fdee6820dd313cd1ac9e3b8bd1427ecb9ccb970483a716
```

Tree hash 使用相对于对应目录、以 `/` 分隔的 ordinal path order，逐文件输入
`UTF-8(path) + NUL + bytes + NUL`。最终 Conversion/cross-spec closure 后必须再次统一复算，不能把
这里的阶段 hash 当作最终 Frozen hash。

## 5. 自检与剩余 gate

已完成：

- binding asset length/SHA-256、stable ID、source/expected section 6/20 mapping 独立复算；
- Render manifest schema 2 强类型加载 binding fixture，验证 workspace containment、path、asset
  length、expected ID/hash；
- Clippy 与 targeted manifest nextest 通过；
- stale editable/archive cluster 与 dynamic BakedCurve 正向能力已移除，只剩明确禁止语句。

仍需：

- 可解码 PNG/WebP 与固定 font FCBC resource fixture；
- Image/ImagePattern/GlyphRun/extension resource 的 RenderSection byte vector；
- kind/media/decode/limit failure vectors；
- exact dynamic property 与 descriptor kind 5 rejection vector；
- Render-aware parser/semantic/raster implementation（I9）；
- Conversion 规范同步、四规范交叉审计和独立复审。

因此 Render Profile 1.0.0 保持 Draft。下一依赖节点是 `fcs-conversion.md` 的版本化 semantic profile、
PGR/RPE/PEC time/resource/ambiguity/report closure。
