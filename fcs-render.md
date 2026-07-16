# FCS Render Profile 1.0.0

状态：Draft（2026-07-16；Source grammar/resource binding closure 已完成，RenderSection/font/raster
closure 正在联合复审，等待完整 binary/decoder/raster vector 与最终独立复审）

本文定义 FCS Render Profile 1.0.0。它扩展 `fcs.md` 的可选顶级 `render` block，并定义
`fcbc.md` Render section 1.0。Render 使用 FCS 唯一 `chartTime` 和 property descriptor，不能
改变 Note 判定、score、sound、Line scroll 或其他 gameplay 结果。

正式 renderer 的输入是已通过 FCBC bounded loader 的 one-chart CanonicalChart、RenderSection
和 resource view。Renderer 不解析 FCS、workspace path、URL 或外部 archive；Render resource
identity 最终只通过 FCBC stable resource ID 解析到 Resources directory 与 ResourceData 原始 bytes。

---

## 1. Conformance 和能力边界

Render Profile 分为：

- **semantic conformance**：scene graph、属性、排序、attachment、resource 和 compositing 一致；
- **reference raster conformance**：在规定 viewport、time、resource 和采样规则下像素在容差内；
- **realtime renderer**：可以使用 GPU 优化，但必须满足 semantic conformance 和声明的 raster
  tolerance。

Render 不是 Canvas 2D 状态机。Core 明确禁止：

- mutable drawing context；
- `save()` / `restore()`；
- runtime path command append/remove；
- runtime node creation/destruction；
- get/put image data 或其他 pixel readback；
- 未声明系统字体 fallback；
- Render 数据反向写 gameplay。

节点数量、类型、ID、parent、path topology、text content、glyph sequence、resource identity 和
blend mode 必须编译期确定。允许动态的是已注册数值/颜色属性。

Resource binding 固定经过：

```text
FCS source @resource reference
→ canonical stable resource ID/kind/metadata/content hash
→ FCBC Resources record + ResourceData range
→ bounded loader 验证 CRC/layout/SHA-256
→ RenderResourceView(id, kind, metadata, immutable raw bytes)
→ kind-specific decoder/compiler
```

RenderSection 不保存 workspace path、URI、package member、resource bytes 的第二份副本或外部查找
候选。Hash 相同的不同 resource ID 仍是不同 identity；renderer 不能按 hash/filename/media type
自动合并或替换引用。

---

## 2. Source 结构

```fcs
render profile 1.0.0 {
    viewport {
        width: 1920px;
        height: 1080px;
        colorSpace: "linear-srgb";
    }

    layer background {
        pass: "background";
        zOrder: -100;
        space: "world";

        children {
            group pulse {
                active: [8beat, 16beat);
                opacity: 0.8;

                children {
                    circle ring {
                        center: vec2(0px, 0px);
                        radius: 100px;
                        fill: solid(#FF4444FF);
                    }
                }
            }
        }
    }
}
```

Render Profile 导入 `fcs.md` Appendix B 的 `semver`、`identifier`、`schemaBlock`、`schemaField`、
`tracksBlock`、`entityExpression`、`expression` 和 `generator`。其 payload grammar 为：

```text
renderProfileBlock = "render", "profile", semver, "{",
                     viewportBlock, layerDecl*, "}" ;
viewportBlock      = "viewport", schemaBlock ;
layerDecl          = "layer", identifier, renderBody ;
renderBody         = "{", renderMember*, "}" ;
renderMember       = schemaField | tracksBlock | childrenBlock ;

childrenBlock      = "children", "{", renderItem*, "}" ;
renderItem         = renderNodeDecl
                   | entityExpression, ";"
                   | renderIf
                   | generator ;
renderIf           = "if", expression, "{", renderItem*, "}",
                     ("else", "{", renderItem*, "}")? ;
renderNodeDecl     = renderNodeKind, identifier, renderBody ;
renderNodeKind     = "group" | "clipGroup"
                   | "rect" | "roundedRect" | "circle" | "ellipse" | "line"
                   | "polyline" | "polygon" | "path" | "image" | "text" ;
```

除已经属于 Core keyword 的 terminal 外，以上固定单词是 Render payload 内的 contextual keyword，
在 Core token stream 中仍可表示为普通 ASCII identifier。Core parser 只负责保存平衡的 payload；
Render-aware parser 必须按本 grammar 完整消费它。`viewport` 语法上恰好一个且位于 layer 之前；
layer 可以为空，profile requirement 和 scene completeness 在 Render static/canonical validation
检查。

Render Profile source 只能组合 FCS Core token；未来 Render 版本若需要 Core lexer 不认识的新
delimiter/token，必须同时声明相容的 FCS source feature/minor，不能让 Core parser 按原始 bytes
猜测跳过。

Render source 中的 image/font/texture/path/shader/binary reference 必须使用已声明 FCS resource 的
`@identifier`，并遵守 `fcs.md` 第 7.3 节 workspace member/hash/type 规则；Render payload 不接受路径
string、URI、data URI、系统字体名称或“在运行时搜索同名文件”的替代 spelling。

所有 pass、space、color space、node option、paint/stroke/composite 等闭集枚举 field 都遵循
`fcs.md` 2.10，直接值使用 string literal；已经完成名称解析的 compile-time string expression
同样合法，但 Render parser 不得把 unresolved bare identifier 猜成 enum。Node kind、`layer`、
`viewport` 和 `children` 是结构 terminal，不加引号。

Render profile version 必须完整声明。未知 major 拒绝；未来 minor 只有全部 required feature 受
支持时才可加载。

---

## 3. Scene graph

### 3.1 Node 类型

Core scene object：

```text
Layer Group ClipGroup
Rect RoundedRect Circle Ellipse Line
Polyline Polygon Path Image Text
```

Layer 是纯组织记录，不是 Node：它只保存 stable ID、pass、zOrder、documentOrder、root node 和
root 的默认 attachment space。Layer 不拥有 transform、opacity、active、visibility、geometry、
paint 或 composite；需要这些能力时必须在 layer 下显式建立 Group。Canonical lowering 把 layer
的 `space` 复制为每个未显式声明 attachment 的 root node attachment；LayerRecord 不再保存第二份
space 真相。

Viewport schema 固定为：`width:length` 与 `height:length` required、compile-time、有限且大于 0；
`colorSpace` optional，默认 `"linear-srgb"`，只允许 `"linear-srgb" | "srgb"`。Layer schema
固定为：`pass` required；`zOrder:int` 默认 0；`space` 默认 `"world"`，允许 `"world"`、
`"screen"`、`line(lineId)` 或 `note(noteId)`；`children` 缺失等价于空 collection。

每个 Node 具有唯一 stable ID、documentOrder、parent、local transform、opacity、active interval、
visibility、zOrder 和 effective attachment。Parent 必须存在且 graph 为 forest。Node 通用默认值为：

```text
position = vec2(0px, 0px)       origin = vec2(0px, 0px)
rotation = 0rad                 scale = vec2(1.0, 1.0)
opacity = 1.0                   visibility = true
active = 双向无界               zOrder = 0
composite = "sourceOver"        isolate = false
followHiddenAttachment = false
```

Opacity descriptor 必须成功返回位于 `[0,1]` 的 finite float；0 仍执行透明 composite，1 表示不额外
衰减。超出该范围不是隐式 clamp。

Root node 未声明 attachment 时使用 layer space；非 root 未声明时继承 parent effective attachment。
Core 1.0 只允许 root 显式覆盖 attachment；非 root 若显式声明，必须与 parent effective attachment
完全相同，否则 static error。NodeRecord 为每个 node 物化 effective attachment以固定 descriptor
environment，但 descendant 不重复应用 attachment matrix。Group 的 `geometryRef` 必须为 null；
ClipGroup 的 `geometryRef` 也为 null 且 `clipRef` 必须存在。其余 node 必须引用 kind-compatible
GeometryRecord。

### 3.2 Children 和 generator

`children` 是 `RenderNode` collection，可以使用 FCS Core template、compile-time `if` 和
generator：

```fcs
children {
    generate i: int in 0..<8 step 1 {
        emit tickMark(i);
    }
}
```

这里仍使用 FCS Core 唯一 range 语法；裸 `..` 非法。所有结构在 canonical scene graph 前
展开，FCBC 不包含 generator/emit/template。

### 3.3 Active 和 visibility

`active` 是 chartTime/beat source interval，canonical 后为 chartTime 半开区间。无 active
表示全时域。每个 Node query 必须按以下顺序执行；同一层级不得提前求值后续项：

1. 用已验证的 compile-time active interval检查 chartTime；inactive 时立即跳过整个 subtree；
2. 对 Note attachment 执行第 4 章的 static/render-enabled/Note visibility gate；gate 关闭时跳过 subtree；
3. 只求第 4 章构造 attachment matrix 所需的最小 Core Line/Distance/Note dependency；
4. 求当前 Node 的 visibility descriptor；false 时跳过剩余 Node descriptor 和整个 subtree；
5. 按 canonical direct-root order 求 local transform、geometry、paint、stroke、clip、opacity/composite，
   再按第 5 章遍历 children。

因此 inactive 可以隐藏本次 query 原本会发生的 attachment/visibility/geometry descriptor execution
error；Note gate 关闭可以隐藏 attachment geometry 与 Node descriptor execution error；Node visibility
false 可以隐藏其后的 Render descriptor execution error。任何 load-time framing、Record/reference、
graph/DAG/type/environment、resource decode/capability/limit 错误都必须在 query 前对完整 FCBC 验证，
永远不能被 inactive、Note gate 或 visibility 隐藏。

Visibility false 使该 node 及其 subtree 不参与绘制，但不改变 sibling；这使 Group/ClipGroup 的
visibility 只有一种解释。Opacity 0 仍是存在的透明绘制，可能影响 `copy` composite。

### 3.4 Transform

Render 使用 `fcs.md` 列向量和矩阵顺序。每个 node：

```text
M_local = T(position) * T(origin) * R(rotation) * S(scale) * T(-origin)
M_world(root)  = M_attachment * M_local
M_world(child) = M_parent * M_local
```

Opacity 沿 non-isolated parent chain 相乘；isolated group 的 opacity 不乘入内部 draw，而在 offscreen
合成时恰好应用一次。Clip 按 world transform 后的 geometry 求交。Scale 可以为负或零；
零 scale 产生零面积 geometry，不是错误。

---

## 4. Attachment 和空间

支持：

```text
world
screen
line(lineId)
note(noteId)
```

- world：FCS 1920×1080 logical world；
- screen：viewport 中心原点，单位仍是 logical px，不受 chart line camera transform；
- line：在同一 chartTime 使用 line world matrix；
- note：只使用 Note 当前 presentation geometry 和所属 line matrix。

令 `s` 为本次 query chartTime，列向量矩阵固定为：

```text
M_attachment(world)  = Identity
M_attachment(screen) = Identity  // viewport-centered logical coordinates
M_attachment(line L) = M_line_world(L, s)
M_attachment(note N) = M_line_world(line(N), s)
                       * T(positionX(N,s) + xOffset(N,s),
                           d(N,s)         + yOffset(N,s))
```

`d(N,s)` 是 Core 定义的 Note 到判定线的有符号 logical distance。Root node 恰好应用一次
`M_attachment`；descendant 只通过 `M_parent * M_local` 继承结果，不重复应用 attachment。

Attachment 只提供几何与 visibility gate，不提供 presentation style inheritance。Line attachment 不
继承 Line alpha/texture/style；Note attachment 不继承 Note alpha、color、rotation、scale、texture 或
任何 built-in Note paint。Render Node 的 opacity、paint、stroke、composite 和 local transform 始终只
来自 Render scene 自身。

Attachment reference 必须存在。Render 可以读取 chart transform/presentation，chart/gameplay
不能读取 Render node。对 Note attachment 且 `followHiddenAttachment=false`，先读取 compile-time
`presentation.render.enabled`；false 时 gate 关闭。随后查询只依赖 `s` 的
`note.presentation.visibility`；false 时同样关闭。`followHiddenAttachment=true` 必须只用于 Note
attachment，并忽略这两个 gate，但仍按上式查询完整 Note geometry；它不忽略 active、不继承 Note
style，也不隐藏 attachment geometry 的 query-time error。World/screen/line attachment 的该 flag 必须
为 false。

---

## 5. 排序和 Render pass

标准 pass 顺序：

```text
background
behindLines
lines
notes
aboveNotes
overlay
```

相对于宿主 Core draw，anchor 固定为：background 最早；behindLines 紧邻且早于全部 built-in Line；
lines 紧邻且晚于全部 built-in Line、早于 built-in Note；notes 紧邻且晚于全部 built-in Note；
aboveNotes 在 notes 后；overlay 最后。没有宿主 Line/Note draw（例如 isolated conformance raster）时
仍按这六个 pass 顺序连接。Render node 不进入 built-in Line/Note 自身的内部排序。

同一 pass 的 Layer sibling 排序键：

```text
(layer.zOrder, layer.documentOrder, layer.stableId)
```

每个 parent 下的 Node sibling 排序键固定为
`(node.zOrder,node.documentOrder,node.stableId)`。Renderer 按 pass 顺序、Layer 顺序和递归 sibling
顺序遍历；drawable node 固定先发出 fill draw op、再发出 stroke draw op，然后遍历 children。
Non-isolated Group 自身不发出 draw op，只把 children 原位展开；ClipGroup 对整个 subtree 追加一个
clip。因而不存在把所有 descendant 再按全局 node key 重排的第二套算法。

Group 不自动建立离屏 stacking context。`isolate=true` 时先在透明背景上按上述规则渲染完整 subtree，
offscreen 使用与当前 target 相同 viewport、sample grid 和每-sample premultiplied buffer；再把结果
作为一个原子 draw op，以 group opacity/composite 合成到 parent target。非隔离
Group/ClipGroup 的 composite 必须为 sourceOver；其 opacity 与 ancestor opacity逐层乘入 descendant
source。需要非 sourceOver group composite 的 source 必须同时声明 `isolate=true`。普通 drawable
node 的 composite 只应用于自己的 fill/stroke；children 随后在其递归位置继续绘制。

---

## 6. Geometry

所有 geometry 参数是 length/property descriptor 且必须有限。

- Rect：origin、size；negative width/height 非法；
- RoundedRect：Rect + 四角 scalar radius。令
  `f=min(1,w/(rTL+rTR),w/(rBL+rBR),h/(rTL+rBL),h/(rTR+rBR))`，分母 0 的 ratio 视为
  `+Infinity`；所有 radius 乘同一 f。运算按书写顺序逐 binary64 舍入；
- Circle：center、radius，radius >= 0；
- Ellipse：center、radiusX/Y、local rotation，radius >= 0；
- Line：start、end；
- Polyline：至少 2 个编译期 point；
- Polygon：至少 3 个编译期 point，隐式 close；
- Path：不可变 command sequence；
- Image：destination rect、可选 source rect；
- Text：编译期 glyph run 和 origin。

零 radius/零面积合法但覆盖率为零。非有限、负 radius 或非法 source rect 是错误。

---

## 7. Path

Path command：

```text
MoveTo(x,y)
LineTo(x,y)
QuadraticTo(cx,cy,x,y)
CubicTo(c1x,c1y,c2x,c2y,x,y)
Arc(center,radius,startAngle,endAngle,direction)
EllipseArc(center,rx,ry,rotation,startAngle,endAngle,direction)
Close
```

第一条 drawing command 前必须有 MoveTo。MoveTo 建立新 subpath，并把它的参数同时设为 subpath
起点和当前点；其他 drawing command 把当前点更新为自己的终点。Close 连接当前点到 subpath 起点，
再把当前点设为该起点。

Arc/EllipseArc 的参数曲线点固定为下式；`theta/phi` 分别是 angle 的弧度值，sin/cos 和每个显示的
运算按 `fcs.md` 的 binary64 规则从左到右执行：

```text
Arc(theta):
  x = center.x + radius * cos(theta)
  y = center.y + radius * sin(theta)

EllipseArc(theta), phi = rotation:
  x = center.x + (radiusX * cos(theta)) * cos(phi)
               - (radiusY * sin(theta)) * sin(phi)
  y = center.y + (radiusX * cos(theta)) * sin(phi)
               + (radiusY * sin(theta)) * cos(phi)
```

执行 Arc/EllipseArc 时，路径总是先从此前当前点向参数曲线的 startAngle 点追加一条概念性直线，
再追加参数曲线，并把当前点更新为 endAngle 点。即使两个端点相等，这条概念性直线仍存在但长度为
零；它不增加 PathCommandRecord，也不改变编译期 command topology，但必须像显式 LineTo 一样参与
fill、stroke、dash 弧长和 reference flatten。start=end 只让参数曲线部分成为零 arc，不删除可能
非零的连接线，也不表示 full circle；full circle 必须显式相差一整 turn。

Arc radius 与 EllipseArc radiusX/radiusY 都必须非负。Direction 是 clockwise/counterClockwise，并按
FCS Y-up 坐标解释；除零 sweep 外，clockwise 要求 `endAngle-startAngle<0`，counterClockwise 要求
`>0`。Runtime 可以改变 command 的数值参数，不能改变 command 数量和类型。

Fill rule：`nonzero` 或 `evenodd`。每个 open subpath 在 fill 时概念性追加从当前点到 subpath 起点的
closing line；该 line 参与 fill winding，但不产生 PathCommandRecord，也不参与 stroke 或 dash。显式
Close 仍按上文更新当前点，并且它的 closing segment 参与 stroke、dash 和 fill。Stroke 对
open/closed subpath 分别处理。

零长度 drawing segment 在 command/topology/source order 中仍存在，弧长精确为 0，但不产生 stroke
coverage、cap、join 或 tangent。求 join tangent 时跳过连续零长度 segment，使用同一 vertex 两侧最近
的非零 segment；open subpath 的 cap 只取首/末非零 on-segment 的 tangent。一个 subpath 若没有任何
非零 on-segment，则不产生 stroke coverage，不因 round/square cap 变成点或方块。Dash phase 不因
零长度 path segment 前进；dash array 中的零长度 element 按 array order在同一 arclength 依次消费，
直到到达正长度 element，整个 array 总长大于 0 保证该过程有限。Arc/EllipseArc 的零长度概念性
connector 遵守同一规则：保留有序边界但不制造额外 cap/join/coverage。

---

## 8. Paint 和 Stroke

### 8.1 Paint

```text
Solid(color)
LinearGradient(start,end,stops,spread)
RadialGradient(startCenter,startRadius,endCenter,endRadius,stops,spread)
ImagePattern(resource,transform,repeat,sampling)
```

Gradient stop 数量至少 2；offset compile-time 确定、有限、位于 `[0,1]` 且非递减。RadialGradient
的 startRadius/endRadius 必须是 finite 且非负；动态 descriptor 成功返回负值时使用
`render.invalid-paint`，不能把它解释成反向或隐式取绝对值。同 offset
连续 stop 表示精确 color step，右侧使用后一 stop。Spread：pad、repeat、reflect。颜色使用
linear RGBA 插值。Paint resource/stop topology 编译期确定；stop color 可以动态。ImagePattern 的
resource 必须是静态 image/texture stable ID，并与 Image node 走同一 FCBC ResourceData binding、
type/hash/decode/limit contract；不能在 paint evaluator 中打开文件或切换 resource identity。
ImagePattern 的 `transform` 不是宿主 matrix blob，而是固定四字段
`position:vec2<length>`、`origin:vec2<length>`、`rotation:angle`、`scale:vec2<float>`，默认值与
Node transform 相同；这些字段可以是 exact descriptor。`repeat` 是 compile-time enum
`"none" | "x" | "y" | "both"`，默认 `"both"`；`sampling` 与 Image 使用同一 enum和像素约定。

### 8.2 Stroke

```text
width: length >= 0
cap: butt | round | square
join: miter | round | bevel
miterLimit: float >= 1
dash: compile-time array<length >= 0>
dashOffset: length
```

Stroke 必须显式绑定一个 Paint。`width` 和 `dashOffset` 可以是 exact descriptor；`cap`、`join`、
`miterLimit` 与 dash element 必须 compile-time 确定。Dash 总长度必须大于 0；奇数长度数组在
canonical lowering 时复制一次成为偶数，因此 RenderSection 只保存偶数 count。Width=0 表示不
绘制 stroke，不表示 device hairline。Dash phase 先以总长度做
`offset - floor(offset / total) * total` 归一化到 `[0,total)`；零长度 dash element 合法，但整个数组
不能全零。负 width、dash element、非有限值或 miterLimit<1 拒绝。

---

## 9. Compositing、颜色和 Clip

Core composite modes：

```text
sourceOver copy add multiply screen
```

所有合成在 premultiplied linear RGBA 中进行。输入 color 先乘 node/group opacity，再合成。
输出到 sRGB target 时最后编码。Add 每通道 clamp 到 `[0,1]`；multiply/screen 使用第 15.3 节的
premultiplied source-over blend 公式。未知 mode 是 required feature error，
不得静默退回 sourceOver。

Clip 是几何 coverage mask，多个祖先 clip 相乘。ClipGroup clip 可以用 fill rule，但没有 paint、
stroke 或 composite。空 clip 隐藏 subtree。

---

## 10. Image

Image source schema 固定为：

```text
resource: image/texture resource reference                    required, static
destination.origin: vec2<length>                              required
destination.size: vec2<length>                                required, components >= 0
sourceRect.origin: vec2<float>                                optional, pixel space
sourceRect.size: vec2<float>                                  optional, components >= 0
sampling: "nearest" | "linear"                              optional
```

`sourceRect.origin` 与 `.size` 必须同时存在或同时缺失；缺失表示完整 decoded image。`sampling`
缺失时使用 resource metadata 的 canonical default。Resource reference 必须在 canonical lowering 前
解析为 image/texture stable ID；destination/source 数值可以使用 exact property descriptor，但 ID、
sourceRect 是否存在和 sampling mode 都不能动态。

FCBC RenderSection 的 Image GeometryRecord 只保存该 stable ID 和 descriptor index。Loader 必须先按
`fcbc.md` 验证 Resources record、ResourceData range、section CRC 与逐资源 SHA-256，再把唯一
immutable slice 交给 decoder。Render Profile 1.0 Core image decoder set 固定为：

- `image/png`：单幅、非 APNG 的 PNG；decoded channel precision 为 8 或 16 bit，palette/`tRNS` 在
  color/alpha conversion 前确定性展开；
- `image/webp`：单幅 static lossless WebP；animated 或 lossy WebP 需要显式 extension feature。

其他 media type/codec 需要显式 extension feature。Resource kind、media type 或 decoder capability
不匹配不能按文件扩展名、magic 或 decode success 猜另一种 resource；动画不能静默只取第一帧。

Image/texture ResourceRecord 的 `metadata:Value(object)` 在 Render 1.0 必须恰好包含以下 canonical
key，顺序固定且 source 缺失的默认值已经物化：

```text
colorSpace : Value(string) = "srgb" | "linear-srgb"
alpha      : Value(string) = "straight" | "premultiplied"
sampling   : Value(string) = "nearest" | "linear"
```

Unknown、duplicate、错误 tag 或错误顺序使用 `render.resource-decode-failed`。Image/texture 必须包含
三项；RenderSection 的 Image/ImagePattern sampling 是已经应用 source override 后的最终 enum，
不得在 runtime 再猜 metadata default。Resource metadata 不保存 width/height；dimensions 只由受限
decoder 从原始 payload 得到，避免目录与 payload 两份真相。

Core PNG 接受标准 PNG color type 0、2、3、4、6 及其规范 bit depth，确定性展开 palette 和 `tRNS`，
并把 1/2/4-bit sample 扩展为其精确整数比例。`acTL`、`fcTL` 或 `fdAT` 表示 APNG，使用
`render.resource-capability-missing` 拒绝。`iCCP` 在 Core 1.0 也需要显式 color-management extension。
可选 color chunk 只有以下 exact 一致组合可接受；它们从不覆盖 ResourceRecord metadata：

| Metadata | `sRGB` | `gAMA` integer | `cHRM` integer tuple |
|---|---|---:|---|
| `srgb` | absent 或 present | absent 或 45455 | absent 或 `(31270,32900,64000,33000,30000,60000,15000,6000)` |
| `linear-srgb` | 必须 absent | absent 或 100000 | absent 或同一 tuple |

出现但不匹配使用 `render.resource-decode-failed`。Core lossless WebP 接受直接 `VP8L`，或没有
`ANIM/ANMF/VP8/ICCP` 且图像 payload 为 `VP8L` 的 static `VP8X`；lossy、animated 或 ICC profile
使用 `render.resource-capability-missing`。EXIF/XMP 不参与 Render 语义。WebP sample 固定解释为
8-bit sRGB straight alpha，因此 Resource metadata 必须为 `srgb/straight`。

整数 channel `n` bit 的 encoded scalar 固定为 `integer/(2^n-1)`，按一次 correctly-rounded
binary64 division。`linear-srgb` RGB 直接使用该值；`srgb` 逐 channel 使用 IEC 61966-2-1：

```text
linear(c) = c / 12.92                                      if c <= 0.04045
            pow((c + 0.055) / 1.055, 2.4)                 otherwise
```

每个加法、除法和 `pow` 分别按 `fcs.md` 第 14.1 节 binary64 规则舍入。Alpha 不做 transfer。
Straight alpha 先 transfer RGB 再乘 alpha；premultiplied encoded sample 在 alpha=0 时要求 RGB 全零，
否则先以 encoded RGB/alpha 得到 straight encoded RGB、做 transfer，再重新乘 alpha。Decoder 输出
始终是 premultiplied linear RGBA binary64。

Source rect 在原始 image pixel space，左上原点、X 右、Y 下；destination 通过 node transform
进入 FCS Y-up space。超出 source bounds 是错误。Sampling：nearest 或 linear；mipmap 和
anisotropic 是 realtime quality option，不能改变 reference raster fixture。

Decoded texel `(i,j)` 的 center 是 `(i+0.5,j+0.5)`。对 local destination rect
`[dx,dx+dw) × [dy,dy+dh)` 内的 sample `P=(x,y)`，`dw/dh` 为零时 coverage 为零，否则：

```text
u  = (x - dx) / dw
v  = (y - dy) / dh
sx = sourceX + u       * sourceWidth
sy = sourceY + (1 - v) * sourceHeight
```

所有操作逐 binary64 舍入。Nearest 使用 `floor(sx),floor(sy)`，因此恰在相邻 texel center 中点时
选择较大 index。Linear 令 `fx=sx-0.5, fy=sy-0.5`，以 `floor(fx/fy)` 得到四 tap 并先 X 后 Y
做 linear interpolation；每个 tap clamp 到 source rect 内最接近的 texel center，不从相邻 atlas
区域 bleed。Source rect 必须完全位于 decoded bounds、width/height 非负；fractional rect 合法，
但正面积 rect 必须覆盖至少一个 texel center，否则 `render.invalid-geometry`。

ImagePattern 先用第 8.1 节 transform 的逆矩阵把 owning geometry local sample 映射到 pattern pixel
space；singular
transform 产生零 coverage。`none/x/y/both` 分别决定对应轴越界时透明或以
`coordinate - floor(coordinate/extent)*extent` wrap。之后使用与 Image 相同的 texel-center和
nearest/linear 规则；repeat 在选择 tap 前应用，non-repeat linear tap 越界时为透明黑而不是 clamp。

上述 decode/color/alpha 规则是唯一 Core conversion；codec chunk、magic 或宿主 color manager 不得
覆盖 ResourceRecord metadata，也不得调用平台 ICC service 得到另一组像素。

Renderer 不得为了“找到可用图片”访问 workspace、相对路径、URL、系统 asset catalog 或另一个
archive，也不得使用同 hash/同 filename 的其他 resource ID 替代。Decoder 在读取 dimensions、
chunk/table count 和分配 decoded buffer 前应用公开的 image dimension/decoded-byte/metadata limit；
非法或超限 payload 分别使用 `render.resource-decode-failed` 或 `render.limit-exceeded`。这些错误不
修改已验证的原始 bytes/hash，且不能通过隐藏 Image node静默忽略 required resource。

---

## 11. Text

Text source content必须编译期确定并绑定 font resource、font face index、size 和版本化 shaping
profile。Render 1.0 Core 只定义 `simple-ltr-1`：

```text
content: string                                      required, static
font: font resource reference                        required, static
fallbackFonts: compile-time array<font reference>    default []
faceIndex: int                                        fixed 0
size: length > 0                                     exact descriptor
shapingProfile: "simple-ltr-1"                      required/default
language: "und"                                     fixed
script: "Latn"                                      fixed
direction: "ltr"                                    fixed
features: compile-time empty array                   fixed
```

其他 language/script/direction、normalization、bidi、GSUB/GPOS、kerning 或 feature set 需要显式
required shaping extension；Core loader 不能按宿主 shaper 的默认值接受。`simple-ltr-1` 不做 Unicode
normalization，按 UTF-8 解码后的 Unicode scalar source order逐个处理；除普通 space U+0020 外，C0/C1
control、line/paragraph separator 与 bidi control 必须拒绝或由 extension处理。

每个 scalar 依次在 primary font、再按声明顺序在 fallbackFonts 查找 cmap glyph。Glyph ID 0 表示
missing，不算命中；全部 missing 时 static error。连续使用同一 font 的 scalar 组成一个 GlyphRun，
font 切换时新 run 的 `runOffset` 等于此前所有 run 的累计 advance。每个 scalar 恰好产生一个 glyph，
不做 ligature、reordering 或 kerning：

```text
glyphId  = selected font cmap result
xAdvance = selected hmtx advanceWidth / unitsPerEm
yAdvance = 0
xOffset  = 0
yOffset  = 0
```

四个 metric 和 runOffset 都以 em-normalized binary64 保存，每次整数除法独立正确舍入；runtime 以
`sizeDescriptor` 的 length 乘它们得到 logical px。Font 坐标和 Render 坐标都是 Y-up。Text
Geometry 可以按 source order引用一个或多个 GlyphRun；多个 run 使用同一个 text origin 和 size
descriptor，各自 runOffset 已包含 fallback split 前的累计 pen。

Render 1.0 Core font ResourceRecord 必须为 kind `font`、media type `font/ttf`，metadata object 恰好按
以下顺序编码：

```text
fontProfile    : Value(string) = "truetype-glyf-1"
shapingProfile : Value(string) = "simple-ltr-1"
faceCount      : Value(int)    = 1
```

`truetype-glyf-1` 是单 face sfnt version `0x00010000`，要求 bounded `head/maxp/hhea/hmtx/cmap/loca/glyf`
table、有效 checksum、unitsPerEm 和 glyph/metric bounds；不接受 TTC/OTC、CFF/CFF2、variation、color
glyph、bitmap strike 或 SVG。`cmap` 只使用 platform 3 format 4/12，format 12 优先，其次 format 4；
encoding ID 只允许 1 (Unicode BMP) 或 10 (Unicode full repertoire)，format 12 必须使用10。多个同
优先级 subtable 按 encoding ID、再 table offset升序选择第一个。Glyph outline允许 TrueType
simple glyph和不形成 component cycle 的 composite glyph；所有 hinting instruction 在 reference
raster 中跳过，不执行 grid fitting、stem darkening、LCD/subpixel hinting 或平台 font service。

Runtime 用 GlyphRun 中的 glyphId 直接取 outline，不重新 cmap 或 shape。TrueType consecutive
off-curve point 中间插入隐含 on-curve midpoint；quadratic outline 按 nonzero fill rule、8×8 sample
grid raster，font units 先乘 `size/unitsPerEm`，再加 runOffset、pen、glyph offset 和 Text origin，最后
应用 Node world matrix。Composite transform 按 sfnt 2×2/translation字段逐 binary64 运算。Component
glyph index 越界、component cycle、table/flag/transform malformed 是 font bytes 无效，使用
`render.resource-decode-failed`；在展开前检查的公开 composite depth、component count、point/contour
count 或 glyph-work budget 超限使用 `render.limit-exceeded`，不能伪装为 malformed font。

标准 RenderSection 不保存 source text、UTF code point→glyph cluster、authoring cursor mapping、cmap
输入或字体文件路径。编辑器需要的 text/cluster mapping 留在 authoring workspace；Fidelity/Debug
也不能用等价编码把 source text 带回 FCBC。Shaping conformance 的 input text 只存在于外部
conformance vector，用来与 FCBC GlyphRun 比较；它不是 container payload。Text fill/stroke 使用
普通 Paint/Stroke。

---

## 12. 动态属性

下列属性可以使用 FCBC exact Constant、SegmentTrack、Piecewise、`choose`/Expression DAG：

```text
position, origin, rotation, scale, opacity, visibility
geometry numeric parameters
paint colors and gradient geometry
stroke width/dashOffset
image destination/source numeric rectangle
glyph-run size and Text origin
material/effect parameters declared portable by extension
```

下列必须编译期确定：

```text
node type/ID/parent
pass/layer topology
attachment target kind and ID
path command topology
gradient stop count/order
gradient stop offset, stroke miterLimit and dash elements
resource and font identity
glyph sequence/text content/shaping profile/run metrics
composite mode
effect/shader schema
```

Render expression 可以读 `s/b/q/d/p`，但 `d` 只在 note attachment context 可用，`q` 只在
line/note attachment context 可用。非法环境读取是 static error。

RenderSection 中每个 PropertyDescriptor index 必须引用 FCBC descriptor kind 1–4，并匹配属性
类型/domain；kind 5/BakedCurve 在所有 FCBC 2 profile 都是 `fcbc.forbidden-descriptor`。标准 Render
compile 不因节点数、shader、帧率或目标设备把 exact expression 烘焙。播放器本地 sampled cache
如果存在，只是 exact descriptor evaluator 的私有派生缓存，不改变 RenderSection bytes、resource
binding、离散 active/visibility boundary 或 strict raster conformance 声明。

---

## 13. Effect 和 Shader 扩展

Core 1.0 不内置 blur、shadow、color matrix 或自定义 shader。它们必须由 extension namespace
声明：输入 texture 数、parameter type、采样边界、color space、determinism、resource limits 和
reference fallback。Required effect 不受支持时拒绝 render；optional effect 可以只在节点明确
声明 fallback paint/node 时回退，并记录 capability report。

Extension 使用 shader/path/binary/texture resource 时，schema 必须把每个输入固定为 canonical
stable resource ID，并声明允许的 resource kind/media type、entry point、编译 profile 和 limit。
对应 bytes 必须来自当前 FCBC Resources/ResourceData；shader include、sidecar、相对路径和运行时
网络获取一律禁止。需要多个 module/include 时必须各自成为声明并内嵌的 resource，由 extension
object 显式列出 ID 和顺序。

Shader 不得读取 gameplay state 以外的隐式全局、wall clock、filesystem、network、unbounded
buffer 或先前 frame feedback，除非独立 non-portable profile 明确声明。即使 non-portable profile
允许额外执行能力，也不能放宽 FCBC 2 的自包含 resource、hash、无外部 lookup 和无 source snapshot
规则。

---

## 14. FCBC RenderSection 1.0

Section type 14 的 section version 固定 `1.0.0`、REQUIRED、`alignmentLog2=3`。Payload 是一个
`fcbc.md` 第 6.1 节通用 prefix 包裹的 singleton `RenderSectionRecord`；不存在“裸 header”解释：

```text
RenderSectionRecord payload:
renderProfileMajor:u16 = 1
renderProfileMinor:u16 = 0
renderProfilePatch:u16 = 0
flags:u16 = 0
viewportWidth:f64
viewportHeight:f64
viewportColorSpace:u16
reserved:u16 = 0
layerCount:u32
nodeCount:u32
geometryCount:u32
pathCount:u32
paintCount:u32
strokeCount:u32
clipCount:u32
glyphRunCount:u32
LayerRecord[layerCount]
NodeRecord[nodeCount]
GeometryRecord[geometryCount]
PathRecord[pathCount]
PaintRecord[paintCount]
StrokeRecord[strokeCount]
ClipRecord[clipCount]
GlyphRunRecord[glyphRunCount]
```

Viewport color space ID：1 linear-sRGB、2 sRGB；它是默认 output surface encoding，内部 paint与
compositing 始终使用 linear RGBA。Conformance fixture 可以显式请求两种已定义 output 之一，但不
能让该选择改变 scene semantics。Width/height 必须有限且大于 0。Header 的 known
payload 是 60 bytes，连同 Record prefix 和各 table 后，`RenderSectionRecord.byteLength` 必须恰好
等于 section length；count、nested Record 和 Value 必须恰好消费它。Render section minor 0 不允许
singleton、Layer/Node/Geometry/Path/Paint/Stroke/Clip/GlyphRun 或 PathCommand 的未知 Record tail；
future minor 只有明确登记 tail、声明 safely ignorable 规则且 loader 支持该 minor 时才能接受，不能
利用通用 tail 携带 source text/cluster。

RenderSection 不含 resource path、hash、offset、length 或 payload table；所有 `resourceId` 都是
FCBC `fcs.resource` stable u64，唯一解析目标是同一文件的 Resources record 与 ResourceData range。
Loader 按 `fcbc.md` 第 17 章先完成 resource directory/data 验证，再验证本 section 的每个
ID/kind/capability reference。

Render auxiliary stable ID namespace 固定为：

```text
fcs.render.layer
fcs.render.node
fcs.render.geometry
fcs.render.path
fcs.render.paint
fcs.render.stroke
fcs.render.clip
fcs.render.glyph-run
```

算法与 `fcbc.md` 6.2 相同。Expanded source 的 layer textual ID 是 `layer/<layerIdentifier>`；Node
textual ID 是其 layer/node ancestry 中 ASCII identifier segment 以 `/` 连接的
`layer/<layer>/node/<node>/.../node/<node>`。同一 parent 的 expanded child identifier 必须唯一；
template/generator 必须在展开时产生最终 identifier，不能把内存地址、线程顺序、workspace path
或 hash-map iteration 拼入 ID。

Layer documentOrder 是展开后 layer declaration 的 zero-based source order；Node documentOrder 是
展开后同一 parent（或同一 layer root collection）中的 zero-based emit order。它们必须从0连续且
同一 collection 不重复；table sorting 不改写该值。

没有独立 source declaration 的 auxiliary object 使用
`owner/<ownerStableId-lowercase-16-hex>/field/<fixed-field-name>/ordinal/<zero-based-decimal>`；没有
array ordinal 的 field 固定 ordinal 0，十进制不写前导零。每类 object 用自身 namespace hash 该
textual ID。Stable ID 0、任意两项 Render auxiliary record 的 u64 collision（即使 namespace/table
不同）或同一 ID 重复必须拒绝；resource/line/note 仍在各自 typed namespace验证。

Layer table 按 `(pass,zOrder,documentOrder,id)` 升序。Node table 分两段：先按 Layer table顺序写
每层全部 root，root 在层内按 `(zOrder,documentOrder,id)`；随后写非 root，key 是
`(layerIndex, root-to-node sibling-key sequence)` 的 ordinal lexicographic order，其中 sibling key 同样
为 `(zOrder,documentOrder,id)`，较短 prefix 先于其 extension。这样 parent index 总小于 child，且
每个 Layer 的 root 是一个唯一连续 range。Geometry、Path、Paint、Stroke、Clip 和 GlyphRun 各自
按 stable ID 升序。

### 14.1 LayerRecord

LayerRecord 使用通用 Record prefix，known `byteLength=36`：

```text
id:u64
pass:u16
flags:u16=0
zOrder:i32
documentOrder:u32
firstRootNode:u32
rootNodeCount:u32
```

Pass ID：1 background、2 behindLines、3 lines、4 notes、5 aboveNotes、6 overlay。
`firstRootNode/rootNodeCount` 是 Node table 首段中的连续 range；空 layer 写
`firstRootNode=0xFFFFFFFF,rootNodeCount=0`，非空时 first 不得为 null。所有 root range 按 Layer
顺序首尾相接并恰好覆盖 Node table 的 root partition，不得遗漏、重叠或指向 non-root。

### 14.2 NodeRecord

NodeRecord 使用通用 Record prefix；从 `id` 到第二个 reserved 的 fixed payload 是 100 bytes，随后
恰好一个 standalone `custom:Value(object)`，所以
`byteLength = 108 + encodedCustomValueLength`，空 object 的总 byteLength 为 124：

```text
id:u64
kind:u16
flags:u16
parentNode:u32 or null
layerIndex:u32
documentOrder:u32
zOrder:i32
attachmentKind:u16
reserved:u16=0
attachmentId:u64 or 0
activeStart:f64
activeEnd:f64
positionDescriptor:u32
originDescriptor:u32
rotationDescriptor:u32
scaleDescriptor:u32
opacityDescriptor:u32
visibilityDescriptor:u32
geometryRef:u32 or null
fillPaint:u32 or null
strokeRef:u32 or null
clipRef:u32 or null
compositeMode:u16
reserved:u16=0
custom:Value(object)
```

Node flags bit0 unbounded active before、bit1 unbounded after、bit2 isolate、bit3 follow hidden
attachment；其他 bit 为零。Bit0 置位时 activeStart 必须为 canonical `+0.0` bits，否则是 inclusive
finite start；bit1 对 activeEnd 同理，否则是 exclusive finite end。两个 bounded endpoint 必须
`start<=end`，相等表示永不 active。GeometryRef 对 Group/ClipGroup 必须 null，对其他 kind 必须
非 null并指向同 kind GeometryRecord。内部所有动态数值均引用 FCBC PropertyDescriptor。

Node kind ID：1 Group、2 ClipGroup、3 Rect、4 RoundedRect、5 Circle、6 Ellipse、7 Line、
8 Polyline、9 Polygon、10 Path、11 Image、12 Text。Layer 不编码为 Node。Attachment kind：
1 world、2 screen、3 line、4 note。Composite ID：1 sourceOver、2 copy、3 add、4 multiply、
5 screen。

Parent 为 null 的 node 必须出现在所属 Layer root range；非 null parent 必须小于当前 node index、与
child 使用相同 layerIndex，且 child attachmentKind/attachmentId 必须与 parent完全相同。World/screen
attachmentId 必须为 0；line/note 必须为非零且存在于同一 FCBC Core table。`isolate` 只对
Group/ClipGroup 合法；non-isolated Group/ClipGroup composite 必须
sourceOver。ClipGroup clipRef required；Group fill/stroke/clip 均 null；ClipGroup fill/stroke null；
Image fill/stroke null；Line stroke required 且 fill null；Text 与其他 fillable geometry至少有 fillPaint
或 strokeRef。`follow hidden attachment` bit 只允许 attachmentKind=note；其他 attachment kind 置位使用
`render.invalid-reference`。任何 node 的 clipRef 都在自身 draw和 subtree 上生效。

`custom` 只允许无执行语义的 typed metadata；Core writer 使用空 object。Required runtime custom
行为必须进入版本化 extension，不能由 unknown custom key改变绘制。Custom 仍受 FCBC
no-source-snapshot约束，不得保存 source text、cluster、workspace path、template/generator/local 或其
编码等价物。

### 14.3 GeometryRecord

GeometryRecord 使用通用 Record；fixed payload 12 bytes 后恰好一个 standalone object，因此
`byteLength = 20 + encodedPayloadValueLength`：

```text
kind:u16
flags:u16
stableId:u64
payload:Value(object)
```

`flags` 在 1.0 必须为 0。

Payload 使用 FCBC `Value(object)` 编码。所有 `*Descriptor` value 与非 resource 的 `*Ref` value
必须使用 `Value(int)` tag，并限制在 `0..=u32::MAX`；它们分别解释为 FCBC descriptor index 与本
RenderSection table index。`resourceId` 必须使用 `Value(resourceRef)` tag；boolean 使用
`Value(bool)`；index list 使用 elementTag=int 的 `Value(array)`，每项同样限制为 u32。不能把这些
字段编码成 float、string 或裸的未加 tag u32。RenderSection 1.0 的标准 key：

```text
Rect          originDescriptor, sizeDescriptor
RoundedRect   originDescriptor, sizeDescriptor, radiiDescriptors
Circle        centerDescriptor, radiusDescriptor
Ellipse       centerDescriptor, radiusXDescriptor, radiusYDescriptor, rotationDescriptor
Line          startDescriptor, endDescriptor
Polyline      pointDescriptors
Polygon       pointDescriptors
Path          pathRef
Image         resourceId, destinationDescriptors, sourceDescriptors?, sampling
Text          glyphRunRefs, originDescriptor
```

以上也是 object key 的唯一 spelling 与 canonical order。`radiiDescriptors`、
`destinationDescriptors`、`sourceDescriptors` 使用 elementTag=int，长度固定 4；顺序分别是
topLeft/topRight/bottomRight/bottomLeft radius 和 x/y/width/height。`pointDescriptors` 使用 int array，
Polyline 至少 2、Polygon 至少 3；`glyphRunRefs` 使用 int array且至少 1。Path/采样/其他 table ref
使用 int；sampling ID 1 nearest、2 linear。Polyline/Polygon 的普通 fill 固定 nonzero；Path 使用
PathRecord.fillRule。Source rect 不存在时唯一编码是省略
`sourceDescriptors` key，不允许 null 或空 array作为第二种 spelling。

Descriptor type 固定为：origin/size/center/start/end/point/Text origin=`vec2-length`；radius/radiusX/
radiusY/radii=`length`；Ellipse rotation=`angle`；Image destination x/y/width/height=`length`、source
x/y/width/height=`float`。Kind 必须与引用它的 Node kind一致。Point 数量和 array 长度在加载时按
第 6 章验证。
Image `resourceId` 必须引用 image/texture；0、dangling ID 或其他 kind 使用
`render.resource-type-mismatch`/`render.resource-not-found`，不得回退默认图片。
每种 Core Geometry kind 的 object 必须恰好使用上表为该 kind 定义的 key；Group/ClipGroup 没有
GeometryRecord。Duplicate、missing 或
unknown key 使用 `render.invalid-geometry`。Future Render minor 或已登记 required extension 可以
定义新 key，但 Core 1.0 loader 不得把未知 key 当作可忽略 authoring metadata。

Geometry 不做跨 owner sharing：每项必须恰好由一个 non-Group Node 的 geometryRef，或一个
ClipRecord 的 geometryRef拥有。Path geometry 再拥有恰好一个 PathRecord；Text geometry按 array
顺序拥有其全部 GlyphRun。相同结构若服务于不同 owner，仍写不同 stable ID record。

### 14.4 PathRecord 和 PathCommandRecord

PathRecord 使用通用 Record：

```text
id:u64
flags:u16 = 0
fillRule:u16
commandCount:u32
PathCommandRecord[commandCount]
```

`byteLength = 24 + sum(command.byteLength)`。Fill rule：1 nonzero、2 evenodd。每个 command 也使用
通用 Record prefix，payload 起始均为 `kind:u16,flags:u16=0`：

| Kind | ID | 后续 payload（均为 u32 descriptor index，最后的 direction 除外） | byteLength |
|---|---:|---|---:|
| MoveTo | 1 | `point` | 16 |
| LineTo | 2 | `point` | 16 |
| QuadraticTo | 3 | `control,end` | 20 |
| CubicTo | 4 | `control1,control2,end` | 24 |
| Arc | 5 | `center,radius,startAngle,endAngle,direction:u16,reserved:u16=0` | 32 |
| EllipseArc | 6 | `center,radiusX,radiusY,rotation,startAngle,endAngle,direction:u16,reserved:u16=0` | 40 |
| Close | 7 | 无 | 12 |

Direction：1 clockwise、2 counterClockwise。Point/control/center descriptor=`vec2-length`；radius=
`length`；rotation/start/end=`angle`。Command sequence 必须满足第 7 章状态机：drawing command前已有
MoveTo；Close 后当前点回到 subpath start，再次 drawing 合法；重复 Close 或空 subpath Close 非法。
Arc/EllipseArc 的 angle difference 使用保存的有符号差并必须与 direction 满足第 7 章；每项在此前
当前点与参数曲线 startAngle 点之间隐含第 7 章定义的概念性 LineTo，但不编码额外 command record。
start=end 只令曲线部分为零 arc；绝对差大于一整 turn 时保留全部 turns，不按 modulo丢失。

### 14.5 PaintRecord

每项使用通用 Record，common payload 为：

```text
id:u64
kind:u16
flags:u16 = 0
variant payload...
```

Paint kind：1 Solid、2 LinearGradient、3 RadialGradient、4 ImagePattern；spread：1 pad、2 repeat、
3 reflect；sampling：1 nearest、2 linear；pattern repeat：1 none、2 x、3 y、4 both。

```text
Solid:
colorDescriptor:u32
byteLength = 24

LinearGradient:
startDescriptor:u32             // vec2-length
endDescriptor:u32               // vec2-length
spread:u16
reserved:u16 = 0
stopCount:u32
GradientStop[stopCount]
byteLength = 36 + 16*stopCount

RadialGradient:
startCenterDescriptor:u32       // vec2-length
startRadiusDescriptor:u32       // length
endCenterDescriptor:u32         // vec2-length
endRadiusDescriptor:u32         // length
spread:u16
reserved:u16 = 0
stopCount:u32
GradientStop[stopCount]
byteLength = 44 + 16*stopCount

ImagePattern:
resourceId:u64
positionDescriptor:u32          // vec2-length
originDescriptor:u32            // vec2-length
rotationDescriptor:u32          // angle
scaleDescriptor:u32             // vec2-float
repeat:u16
sampling:u16
byteLength = 48

GradientStop (bare 16 bytes, no Record prefix):
offset:f64
colorDescriptor:u32             // color
reserved:u32 = 0
```

Stop count至少 2，offset finite、`0<=offset<=1` 且 raw numeric order非递减；相同 offset 保持 source
order。ImagePattern resource 遵守 Image 的 image/texture binding，不允许 0 或 fallback。

### 14.6 StrokeRecord 和 ClipRecord

StrokeRecord 使用通用 Record：

```text
id:u64
flags:u16 = 0
reserved:u16 = 0
paintRef:u32
widthDescriptor:u32             // length
cap:u16
join:u16
miterLimit:f64
dashOffsetDescriptor:u32        // length
dashCount:u32
dash:f64[dashCount]
byteLength = 48 + 8*dashCount
```

Cap：1 butt、2 round、3 square；join：1 miter、2 round、3 bevel。PaintRef required。Dash count 0
表示 solid，否则必须为偶数；元素 finite且非负，总和大于 0。MiterLimit finite且至少 1。

ClipRecord 使用通用 Record且 fixed `byteLength=24`：

```text
id:u64
flags:u16 = 0
fillRule:u16                     // 1 nonzero, 2 evenodd
geometryRef:u32
```

Clip geometry 只允许 Rect、RoundedRect、Circle、Ellipse、Polygon 或 Path；不允许 Group、ClipGroup、
Line、Polyline、Image 或 Text。每个 Clip 恰好由一个 Node.clipRef拥有，其 Geometry 又恰好由该 Clip
拥有；这一所有权边不形成另一个 clipRef，因此 clip graph 只有 Node ancestor chain，不存在独立
ClipRecord cycle。Path clip 的 fillRule 必须等于其 PathRecord.fillRule；其他允许 geometry 由
ClipRecord 唯一提供 fill rule。Clip Geometry 使用 owning Node 的 world matrix和effective attachment；
ClipRecord 不保存第二份 transform。

### 14.7 GlyphRunRecord

GlyphRun 使用通用 Record：

```text
id:u64
fontResourceId:u64
faceIndex:u32
flags:u16 = 0
shapingProfile:u16 = 1           // simple-ltr-1
sizeDescriptor:u32               // length
runOffsetX:f64                    // em-normalized, X right
runOffsetY:f64                    // em-normalized, Y up
glyphCount:u32
reserved:u32 = 0
GlyphPlacement[glyphCount]
byteLength = 60 + 40*glyphCount

GlyphPlacement (bare 40 bytes, no Record prefix):
glyphId:u32
reserved:u32 = 0
xAdvance:f64
yAdvance:f64
xOffset:f64
yOffset:f64
```

所有 metric finite。Font resource 必须满足第 11 章；faceIndex 在 Core profile 必须 0，每个 placement
必须满足 `1 <= glyphId < font.numGlyphs`。Glyph 0 是 shaping 阶段的 missing sentinel，不是可绘制的
`.notdef` fallback；faceIndex 非 0、glyph 0 或越界 glyph 在 font 成功解码后使用
`render.invalid-geometry`。Size descriptor 必须成功返回大于 0 的 finite length；0 或负值使用
`render.invalid-geometry`，不表示 device font fallback 或最小字号。Glyph count 可以为 0，仅用于空
source content。RenderSection minor 0 的
`byteLength` 必须精确等于公式，禁止 cluster/source-text/cursor tail；发现额外 tail 使用
`render.invalid-record`。

### 14.8 Descriptor root、ownership 和 reachability

Render direct root 必须纳入 `fcbc.md` 第 13/14/18 章的 descriptor/node canonical traversal。Target
path 是下列 exact ASCII spelling；array 使用 zero-based decimal ordinal，无前导零：

```text
render.node.position
render.node.origin
render.node.rotation
render.node.scale
render.node.opacity
render.node.visibility

render.geometry.origin
render.geometry.size
render.geometry.center
render.geometry.radius
render.geometry.radiusX
render.geometry.radiusY
render.geometry.rotation
render.geometry.start
render.geometry.end
render.geometry.radiiDescriptors[i]
render.geometry.pointDescriptors[i]
render.geometry.destinationDescriptors[i]
render.geometry.sourceDescriptors[i]
render.geometry.originDescriptor

render.path.command[i].point
render.path.command[i].control
render.path.command[i].control1
render.path.command[i].control2
render.path.command[i].end
render.path.command[i].center
render.path.command[i].radius
render.path.command[i].radiusX
render.path.command[i].radiusY
render.path.command[i].rotation
render.path.command[i].startAngle
render.path.command[i].endAngle

render.paint.color
render.paint.start
render.paint.end
render.paint.startCenter
render.paint.startRadius
render.paint.endCenter
render.paint.endRadius
render.paint.stop[i].color
render.paint.position
render.paint.origin
render.paint.rotation
render.paint.scale

render.stroke.width
render.stroke.dashOffset
render.glyphRun.size
```

Owner stable ID 分别是引用字段所在的 Node、Geometry、Path、Paint、Stroke 或 GlyphRun ID；同一
path/owner 若有多个 array element使用 ordinal 区分。Property type 按 14.2–14.7 的注释与 14.3
matrix。所有 root 只允许 descriptor kind 1–4；domain 必须覆盖 owning Node active domain，Node
双向无界时 root 也双向无界。所有 auxiliary record只有一个规范 owner，因此环境由 owner Node 的
effective attachment 唯一决定：`d` 仅 note，`q` 仅 line/note，其他 attachment读取它们使用
`render.invalid-descriptor`。`p` 只在 `fcs.md`/`fcbc.md` 定义的 Piece context 中由选中的 inner
descriptor 使用，不形成 Render direct-root 的外部隐式 context。

Loader 必须先完成全部 Render reference、graph 和 auxiliary ownership，收集每个 descriptor 的全部
Core/Render/required-extension direct/transitive owner，再验证共享 descriptor 的 environment 交集；
不得按首个 owner、首个 table record 或当前可见 Node 提前接受。Intrinsic ABI opcode/arity/type/DAG
或 Core owner environment 失败使用 `fcbc.invalid-expression`；这些检查通过、但任一 Render owner 的
effective attachment 不允许 EnvQ/EnvD 时使用 `render.invalid-descriptor`。该 load-time 检查遍历全部
Node，包括永不 active 或 runtime visibility 恒 false 的 Node。

每个 Layer/Node/auxiliary record 必须从某个 Layer root沿 parent、geometry/fill/stroke/clip/path/
glyphRun ownership edge 可达；每个 record 恰好有上文规定的一个 owner，禁止 orphan、跨 owner sharing
和隐藏 payload。Exact ownership edge 固定为：kind 3–12 drawable Node owns Geometry；Node.fillPaint owns
Paint；Node.strokeRef owns Stroke 且 Stroke.paintRef owns另一个 Paint；Node.clipRef owns Clip 且 Clip
owns Geometry；Path Geometry owns Path；Text Geometry 的 glyphRunRefs 按 ordinal分别 owns GlyphRun。
这些 fixed field name/ordinal 同时用于 auxiliary textual ID derivation。Resource 可以由多个 Render
owner引用，但 identity始终是 ResourceRecord。所有
table ref 必须 bounds/kind valid，所有 stable ID必须唯一，所有 Node parent无环。RenderSection
验证成功只产生 resource ID 到已验证 immutable slice/decoded handle 的显式映射；不得构造 fallback
search path。

---

## 15. Reference raster conformance

### 15.1 Semantic draw list

在 raster 前，reference semantic evaluator 必须在给定 chartTime 从已加载的 FCBC 生成唯一 draw
list，不读取 source fixture。它严格按第 3.3、4 章执行 active → Note attachment gate → minimal Core
attachment geometry → Node visibility → remaining Render roots，再按第 5 章递归 flatten；不能为了批量
求值而观察一个本应被前置 gate 跳过的 query-time error。Inactive/gate-closed/visibility-false parent
移除 subtree；每个 local/attachment/parent/world 3×3 matrix、effective opacity、clip chain 和 evaluated
property 都使用 `fcs.md` binary64逐操作规则。Loader 在此之前已对全部 Node 完成 framing、reference、
graph、DAG/type/environment、resource decode/capability 和 limit validation。

每个 draw op 至少保存以下可比较事实：pass/layer/node/geometry stable ID、fill 或 stroke kind、完整
sort/ancestry key、world matrix raw bits、effective opacity raw bits、composite、clip chain；Image 再保存
resource ID、source/destination rect bits和sampling；Text 保存 font ID、face、size、runOffset、glyph ID
与最终 glyph origin bits。Semantic conformance 比较这些 typed value和raw bits，不比较 loader 无法
反推的 source path/label。以任意 query 顺序或从任意此前 frame开始必须得到同一 draw list。

### 15.2 Viewport、sample grid 和 coverage

每个 raster fixture 固定 FCBC RenderSection viewport、chartTime bits、output pixel width/height、resource
bytes/hash、output color space 与 RGBA8。Output pixel `(px,py)` 按 top-to-bottom row-major 保存；其
8×8 sample `(sx,sy)` 的 logical coordinate固定为：

```text
deviceX = px + (sx + 0.5) / 8
deviceY = py + (sy + 0.5) / 8
logicalX = (deviceX / outputWidth  - 0.5) * viewportWidth
logicalY = (0.5 - deviceY / outputHeight) * viewportHeight
```

不存在隐式 aspect preserve、letterbox、crop 或 devicePixelRatio；不同 output aspect 对 X/Y独立缩放。
每个 sample 初始 premultiplied linear transparent black，按 draw list逐项执行。Fill 使用数学 path 的
nonzero/evenodd rule；sample 恰在 boundary 上算 inside。Clip 对每个 sample 是 0/1 coverage，ancestor
clip 逻辑相乘。每个 geometry/clip 先用其 world matrix 的逆把 logical sample映射到 local space；
singular matrix 的 coverage 为0。Geometry coverage 与 clip coverage为 1 时才在同一 local point执行
paint/composite。

Rect、Circle、Ellipse 和 line segment 按其数学闭集；zero-area fill没有 interior。RoundedRect 先按
第 6 章 CSS corner scaling，再用四段 quarter ellipse。Polyline fill 以隐式 closing segment计算但
stroke 保持 open；Polygon fill/stroke 都 closed。Path 的每个 open subpath 与第 7 章相同，在 fill
时使用隐式 closing segment，但 stroke/dash 保持 open；显式 Close 的 segment 同时参与 fill、stroke
和 dash。Path 的 quadratic/cubic/arc/ellipse arc 语义是精确
参数曲线；reference 实现可以按 source order递归二分，直到 control hull/arc sagitta 到 chord 的最大
距离不超过 `1/1024` logical px，最大 depth 32，left half先于right half。超过 depth仍不满足是
`render.limit-exceeded`，不能放宽 tolerance。

Stroke 是路径中心线以 `width/2` 扩张的闭集。Butt cap 在 endpoint 截断；square 沿切线再扩
`width/2`；round 加半圆。Bevel 连接两个外侧 offset endpoint；round 使用以 vertex为中心的扇形；
miter 使用两条外侧 offset line交点，miter length/halfWidth 大于 miterLimit 时退化 bevel。Dash 从每个
subpath起点重新开始，按第 8.2 节归一化 phase、沿 flatten 后弧长交替 on/off；Close 的 closing
segment参与 dash。第 7 章的零长度 segment 不产生 sample coverage、cap、join 或 tangent，也不推进
dash phase；join/cap 使用最近的非零 on-segment。Sample 恰在 stroke boundary 上算 inside。

### 15.3 Paint 与 compositing

Solid 直接返回 descriptor color。LinearGradient 对 sample `P` 计算
`t=dot(P-start,end-start)/dot(end-start,end-start)`；zero-length gradient 固定 `t=0`。RadialGradient
对 `C(t)=C0+t(C1-C0), r(t)=r0+t(r1-r0)` 解
`|P-C(t)|^2=r(t)^2`；在所有 finite real root 中选择 `r(t)>=0` 的最大 t，没有候选则 transparent。
令 `ux=P.x-C0.x`、`uy=P.y-C0.y`、`vx=C1.x-C0.x`、`vy=C1.y-C0.y`、
`dr=r1-r0`，并按以下括号逐 binary64 operation 计算：

```text
A = ((vx*vx) + (vy*vy)) - (dr*dr)
B = -2.0 * (((ux*vx) + (uy*vy)) + (r0*dr))
C = ((ux*ux) + (uy*uy)) - (r0*r0)
```

这给出 `A*t^2+B*t+C=0`。任一 coefficient 非有限时没有候选。`A=0,B!=0` 时唯一候选为
`-C/B`；`A=B=0,C!=0` 时没有候选；`A=B=C=0` 时固定候选 `t=0`，不得把无限根解释为任意最大值。
`A!=0` 时先按 `D=(B*B)-((4.0*A)*C)` 计算 discriminant；`D` 非有限或小于 0 时没有候选，
`D=0` 时唯一候选为 `(-B)/(2.0*A)`，`D>0` 时按此顺序计算
`rootMinus=((-B)-sqrt(D))/(2.0*A)` 与 `rootPlus=((-B)+sqrt(D))/(2.0*A)`。`sqrt` 使用 Core 的
correctly-rounded binary64 规则。Candidate 只有在 `t` finite，且按 `r0+(t*dr)` 计算的半径 finite、
`>=0` 时才合法；最后从合法 candidate 中选择数值最大的 `t`，没有候选则 transparent。

Spread pad clamp到 `[0,1]`；repeat 使用 `t-floor(t)`；reflect 先以 period 2归一化，结果大于1时用
`2-result`。Stop 查找固定为：t 小于首 offset用首色，大于末 offset用末色；否则 left 是最后一个
`offset<=t` 的 stop，right 是其后第一个更大 offset，若不存在用 left；相同 offset 因此在其右侧
精确选择该 offset 的最后一个 stop。不同 offset 之间按 linear RGBA逐 channel插值。Image 与
ImagePattern 使用第 10 章规则。

Color descriptor 与 gradient stop 是 straight linear RGBA；gradient 先按 straight component插值，再把
RGB乘结果 alpha。Image/ImagePattern decoder 已返回 premultiplied linear RGBA。两条路径随后把全部
premultiplied component乘 shape/image/glyph coverage、clip coverage 和 effective opacity，再
compositing。`S`、`D` 都是 premultiplied linear RGBA。SourceOver 与 copy：

```text
sourceOver.rgb = S.rgb + D.rgb * (1 - S.a)
sourceOver.a   = S.a   + D.a   * (1 - S.a)
copy           = S
add            = clamp(S + D, 0, 1) per component
```

Multiply/screen 使用统一 source-over blend：alpha 为 `Sa+Da-Sa*Da`；令 alpha=0 的 straight RGB 为0，
否则 `Cs=S.rgb/Sa,Cd=D.rgb/Da`，`Bmul=Cs*Cd`，`Bscreen=Cs+Cd-Cs*Cd`，输出 RGB 为
`S.rgb*(1-Da)+D.rgb*(1-Sa)+Sa*Da*B`。所有表达式按书写顺序逐 binary64 operation舍入并在最终有限
结果 clamp `[0,1]`。

### 15.4 Glyph 与 image

Image sample 由第 10 章 decoder和sampling产生。Glyph 由第 11 章 TrueType outline产生；glyph pen
先加 runOffset，再按 source order对每项使用当前 pen+glyph offset，绘制后增加 advance。Outline
coverage使用本章同一 8×8 grid和nonzero rule，不执行 hinting。Text fill先绘 glyph fill，stroke 后绘
glyph outline stroke；两者使用普通 Paint/Stroke和Node composite顺序。

### 15.5 Pixel aggregation 和 comparison

对每个 pixel，按 `sy=0..7` 外层、`sx=0..7` 内层顺序逐 component binary64相加 64 个最终 sample
color，再除以 64。结果仍是 premultiplied linear。Alpha 为0时输出 straight RGB 固定0；否则先
`rgb/alpha` unpremultiply并 clamp。Output `linear-srgb` 直接编码该 linear straight RGB；sRGB 使用：

```text
encoded(c) = 12.92*c                                      if c <= 0.0031308
             1.055*pow(c,1/2.4) - 0.055                  otherwise
```

每步独立舍入。RGB/alpha clamp `[0,1]`，乘255 后 roundTiesToEven 到 u8；输出是 straight RGBA8。

每个 raster fixture 至少固定：

- viewport width/height；
- chartTime；
- FCBC resource stable ID、kind/metadata、ResourceData 原始 bytes 与 SHA-256；
- output color space（默认 sRGB RGBA8）；
- transparent black initial target；
- 8×8 规则 subpixel sample grid；
- analytic/flattened path coverage误差不超过 1/1024 logical px；
- linear-light compositing；
- 最后一步 round-to-nearest-even 编码到 8-bit。

Reference image pixel 比较：每 channel absolute difference <=1 且差异 pixel 比例 <=0.1%；
比例必须以整数检查 `differentPixels*1000 <= totalPixels`，不得使用 binary float近似 0.1%。
包含 sharp subpixel geometry 的 fixture可以声明更严格 semantic coverage map，而不是放宽全局
容差。Fixture 不得通过测试进程工作目录或系统字体寻找未内嵌 asset。Realtime renderer 必须公布
其 raster conformance level。

---

## 16. Resource limits 和错误

Compiler/renderer 必须限制 node、path command、point、gradient stop、glyph、clip depth、group depth、
descriptor、单 resource bytes、resource 总 bytes、image dimension/decoded bytes、font table/glyph、
shader module/instruction/texture binding。限制在读取受文件控制的 count 或分配/编译前公开并检查，
超限拒绝。

以下是 error：未知 required profile/feature、duplicate ID、parent cycle、无效 attachment/resource、
非法 path、负 geometry、无效 gradient/dash、缺字、动态 topology、runtime node creation、非法
composite、descriptor 类型错误和非有限值。

Load-time Render 错误不能通过 inactive 或隐藏节点静默恢复：framing、Record/reference、graph、
DAG/type/environment、resource decode/capability 和 limit 必须验证完整 FCBC。仅 query-time descriptor
execution error 可以按第 3.3 章的 active/Note gate/visibility 顺序被更早 gate 跳过。显式
repair/fallback 必须记录在 ConversionReport 或 repair record。

稳定 Render resource/runtime category：

```text
render.unsupported-profile
render.invalid-section
render.invalid-record
render.resource-not-found
render.resource-type-mismatch
render.resource-decode-failed
render.resource-capability-missing
render.invalid-reference
render.invalid-geometry
render.invalid-paint
render.invalid-stroke
render.invalid-clip
render.invalid-composite
render.invalid-graph
render.invalid-descriptor
render.limit-exceeded
```

FCBC framing、ResourceData bounds/coverage/checksum/hash 先使用 `fcbc.*` category；只有 bounded FCBC
resource view 已验证后发生的 Render kind/media/decode/capability 问题才使用 `render.*`。同一失败不
得根据 renderer 是否尝试外部 fallback 而改变 category，因为外部 fallback 本身被禁止。

Generic FCBC validation 与 Render profile validation 的 seam 固定如下：generic loader 先验证 container/
ABI intrinsic framing、Record boundary、opcode/arity/type/DAG 和 Core owner invariants，但不得提前
解引用 Render-owned table/resource/entity reference，也不得用尚不完整的 owner 集合完成 descriptor
reachability/environment。Render validator 随后验证 scene graph、reference、ownership 和 resource
binding，建立完整 Core+Render+required-extension root 集合，再完成 shared descriptor
reachability/environment intersection。Intrinsic ABI 或 Core owner failure 使用 `fcbc.*`；只有前者通过
而 Render owner/attachment 不接受 dependency 时使用 `render.invalid-descriptor`。因此 generic
`fcbc.dangling-reference`/`fcbc.invalid-expression` 不能抢占本表为 Render graph/reference/owner 明确
指定的 category。

Render query 严格执行第 3.3 章顺序：active interval → Note static/visibility gate → attachment 的最小
Core dependency → Node visibility → remaining Render direct roots。任何已通过 FCBC load-time
validation、但在被实际查询的 transitive Core root（line transform、scroll coordinate、distance 或
note presentation）求值时发生的 execution/domain/非有限错误，统一使用
`render.invalid-descriptor`；它不能退回 generic `fcbc.invalid-expression`，也不能因 attachment owner
改类。多个实际发生的 query-time failure 按该 dependency/direct-root order 取首个 category；被更早
gate 跳过的 root 不算 failure。

只要 descriptor direct root 的 index、property type、domain 和 environment 已通过结构验证，后续失败
必须区分“descriptor 没有成功返回一个 typed finite value”和“成功返回的值违反 owner field 约束”：

- expression/track evaluator 的 checked arithmetic、除零、domain、求值 budget、NaN/Infinity、结果 type
  或其他执行失败统一使用 `render.invalid-descriptor`，不能按 owner 改类；
- descriptor 成功返回后，Geometry/Path/Image/Text/Glyph 布局字段违反第 6、7、10、11 章值域时使用
  `render.invalid-geometry`；
- Paint 字段违反 paint kind、spread、repeat、sampling、gradient stop/radius/color 值域时使用
  `render.invalid-paint`；
- Stroke 字段违反 width、cap、join、miterLimit、dash/dashOffset 约束时使用
  `render.invalid-stroke`；
- Clip fillRule、允许的 geometry kind 或 Path fillRule 一致性失败使用 `render.invalid-clip`；
- Node/Group opacity、composite enum 或 isolate/composite 组合失败使用
  `render.invalid-composite`。

RenderSection header 的 viewport width/height/colorSpace/reserved 非法使用 `render.invalid-section`；
Layer pass/order/range 与 Node active interval/parent/layer/order 非法使用 `render.invalid-graph`；Node 或
Geometry kind 非法使用 `render.invalid-geometry`；attachment kind/ID、follow-hidden 适用性和其他
Render-owned reference 非法使用 `render.invalid-reference`。Font 成功解码后，GlyphRun
`faceIndex != 0`、glyph 0 或 `glyphId >= numGlyphs` 使用 `render.invalid-geometry`，不回退
`render.resource-decode-failed`。

这一分类不改变 reference 类别：dangling table/resource 仍先使用 reference/resource category，Record/
Value framing 仍先使用 `render.invalid-record`。例如 PaintRecord 已完整 framed 但 `spread=4` 是
`render.invalid-paint`，不是 `render.invalid-record`；opacity descriptor 自身除零是
`render.invalid-descriptor`，而成功返回 `1.5` 是 `render.invalid-composite`。Runtime query 按
`fcbc.md` 第 13、18 章的 canonical Core dependency/direct-root traversal求值，每个 root 成功后立即验证 owner field，
因此同时存在多个 query-time 失败时仍有唯一首个 category。

联合验证顺序和 stable parent 固定为：

| 顺序 | Failure surface | Stable parent |
|---:|---|---|
| 1 | Header/section table/alignment/coverage/checksum、Resources/ResourceData layout/hash | `fcbc.*` 最接近 category |
| 2 | Render section/profile major/minor/flags 不受支持 | `render.unsupported-profile` |
| 3 | Singleton/count/table boundary/known payload 未恰好消费，或 viewport width/height/colorSpace/reserved 非法 | `render.invalid-section` |
| 4 | Render nested Record byteLength/version/flags/tail、Value framing/tag/padding | `render.invalid-record` |
| 5 | Duplicate/zero ID、table canonical order、Layer pass/root range、Node active/parent/layer/order、cycle、orphan ownership | `render.invalid-graph` |
| 6 | Render-owned table ref越界、nullability/kind incompatibility、attachment target/kind/ID、非法 follow-hidden；resource stable ID 是否存在留到第 13 步 | `render.invalid-reference` |
| 7 | Node/Geometry kind、Geometry object key/tag/count、path enum/状态、compile-time geometry值域 | `render.invalid-geometry` |
| 8 | Paint kind/spread/repeat/sampling、stop count/order或 compile-time paint值域 | `render.invalid-paint` |
| 9 | Stroke cap/join/miter/dash结构或 compile-time stroke值域 | `render.invalid-stroke` |
| 10 | Clip fillRule、允许 geometry kind、Path fillRule一致性 | `render.invalid-clip` |
| 11 | Composite enum、isolate适用性或 non-isolated Group/ClipGroup组合 | `render.invalid-composite` |
| 12 | Intrinsic ABI/Core owner validation通过后，Render direct-root index/type/domain/environment或完整 shared-owner intersection错误 | `render.invalid-descriptor`；descriptor kind 5 仍是 `fcbc.forbidden-descriptor` |
| 13 | Render resource ID 不存在 | `render.resource-not-found` |
| 14 | Resource kind 或已声明 media type与 owner不符 | `render.resource-type-mismatch` |
| 15 | 已知 kind/media但 Core codec/shaping/profile feature不支持 | `render.resource-capability-missing` |
| 16 | 已支持 codec/font profile 的 bytes/metadata/table/outline malformed，包括 composite index/cycle | `render.resource-decode-failed` |
| 17 | 已解码 font 对应 GlyphRun 的 faceIndex/glyph 0/glyphId range 非法 | `render.invalid-geometry` |
| 18 | Active 后实际查询的 Core Line/Distance/Note attachment dependency execution/domain failure | `render.invalid-descriptor` |
| 19 | Note gate通过后实际查询的 Node visibility evaluator 未成功返回 bool | `render.invalid-descriptor` |
| 20 | Visibility 为 true 后其余 Render descriptor evaluator 未成功返回 typed finite value | `render.invalid-descriptor` |
| 21 | 成功返回的 descriptor value 违反 Geometry/Path/Image/Text/Glyph 布局值域 | `render.invalid-geometry` |
| 22 | 成功返回的 descriptor value 违反 Paint 值域 | `render.invalid-paint` |
| 23 | 成功返回的 descriptor value 违反 Stroke 值域 | `render.invalid-stroke` |
| 24 | 成功返回的 opacity value 不在 `[0,1]` | `render.invalid-composite` |

任何 limit 在对应步骤读取 file-controlled count、dimension、table 或分配/递归展开前检查，并覆盖
该步骤的其他语义错误，返回 `render.limit-exceeded`；这包括 image decoded bytes、font table/glyph
count、composite depth/component/point/contour 和 glyph-work budget。Malformed component index/cycle
不是 limit，仍用 `render.resource-decode-failed`。更早的 FCBC structural/checksum/hash 仍优先。一个
mutation 若未同步 section CRC，必须停在 `fcbc.section-checksum`。

---

## 17. Conformance fixture

至少覆盖：

1. 第 2 章 grammar 的合法完整 block、缺失 viewport、非法 node kind、unbalanced/trailing payload；
2. 每种 node/geometry/path command；Arc/EllipseArc 必须覆盖当前点不同于参数起点、二者相等、
   start=end 但连接线非零和多 turn；Path open subpath fill 的隐式 closing line 与显式 Close 的
   stroke/dash 差异也必须覆盖；零长度 leading/interior/trailing/only segment 必须覆盖 cap、join、dash
   phase 和零 coverage；
3. parent transform、opacity、clip 和 isolate；
4. 所有 Core paint/stroke/composite；
5. world/screen/line/note attachment 的 exact matrix、无 Line/Note style inheritance、Note gate 与
   followHiddenAttachment；
6. active → Note gate → attachment dependency → Node visibility → remaining root 的惰性顺序、每个 gate
   可跳过的 query-time error、永远不可隐藏的 load-time error，以及 deterministic sorting；
7. canonical resource ID→FCBC Resources/ResourceData binding、missing/type/hash/decode boundary，及
   image color/alpha/sampling；
8. embedded fixed-font glyph run、glyph 0/numGlyphs/faceIndex mutation、composite malformed 与
   composite limit 分层，且 RenderSection 无 source text/cluster；
9. Track/Piecewise/choose/Expression exact dynamic property、完整 shared-owner environment intersection、
   generic FCBC 与 Render validation precedence，以及 descriptor kind 5 rejection；
10. generator 完全展开；
11. Paint/Stroke/Clip/composite 的非法 compile-time enum/value（包括负 radial radius），以及 descriptor
    执行失败与成功返回 owner-invalid value 的 stable category 分层；Core attachment transitive error
    必须映射为 `render.invalid-descriptor`；
12. Note visibleFrom/visibleUntil→visibility Piecewise、Piece-context EnvP 与无 context EnvP rejection；
13. malformed graph/resource/reference，包括 viewport、Layer、Node 各类非法值的稳定 parent；
14. semantic scene snapshot；
15. reference raster output。

Source grammar closure baseline 至少绑定一个合法完整 block、missing viewport 和 unknown node kind
三项 `source_fixture`；Render-aware parser 对后两项返回 `syntax.invalid-token`。Core I1 parser 只需
确认它们的 version envelope 与 brace/parenthesis/bracket balance，不能据此声称 Render source
conformance。

Resource binding closure 另绑定一个 semantic-only `binding_fixture`：它必须证明合法 workspace
resource reference 产生固定 canonical ID/kind/hash，FCBC writer 只在 Resources section 6 保存目录并
在 ResourceData section 20 保存原始 bytes，Render Geometry 只保存该 ID。该 fixture 不执行媒体
decode；实际 PNG/WebP/font decode 与 raster 由独立固定资源 fixture 覆盖。

Render binary closure 还必须至少绑定一个非空 static FCBC golden，包含 section 1–14、20、八张
非空 Render table、全部 Core geometry/path command/paint/stroke/clip/GlyphRun record variant、kind
1–4 descriptor direct root 与 kind 5 rejection。Golden manifest 必须记录 decoded file SHA-256、每个
section offset/length/CRC、每个 resource ID/kind/media/metadata/range/SHA、table count 和 expected
success；writer 生成 bytes 与 static golden逐 byte比较，独立 loader不得调用 writer。

固定 resource 至少包括项目自制 RGBA PNG、static lossless WebP 和 TrueType font；conformance
vector在 FCBC 外保存 shaping input text/profile和 expected glyph/metric bits，FCBC 内只保存 GlyphRun。
Semantic vector 必须从 static bytes 经 loader/evaluator得到 draw list；raster expected 保存 decoded
RGBA8 length/SHA和容差。Deep mutation 在同步 section CRC 后至少覆盖 Record/tail、ID/order/root range、
reference/kind、descriptor type/kind5、resource missing/type/capability/decode、graph cycle、Geometry key
和 GlyphRun source-text tail，以及 Paint/Stroke/Clip/composite owner-specific category；另有不修 CRC 的
checksum case。Fixture asset 必须有项目内 provenance/license，测试不得读取系统字体、外部 URL 或
workspace fallback。
