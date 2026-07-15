# FCS Render Profile 1.0.0

状态：Draft（2026-07-15；Source grammar/resource binding closure 与联合候选自检已完成，等待完整 RenderSection/decoder/raster vector 与独立复审）

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

Core node：

```text
Layer Group ClipGroup
Rect RoundedRect Circle Ellipse Line
Polyline Polygon Path Image Text
```

每个 node 具有唯一稳定 ID、documentOrder、parent、local transform、opacity、active interval、
visibility、zOrder 和 attachment。Parent 必须存在且 graph 为 forest；Layer 是 root，不能有
parent。ClipGroup 必须引用一个 clip geometry。

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
表示全时域。Inactive node 及其 subtree 不参与布局、clip 或绘制。Visibility false 使该 node
不绘制，但不改变 sibling；opacity 0 仍是存在的透明绘制，可能影响 `copy` composite。

### 3.4 Transform

Render 使用 `fcs.md` 列向量和矩阵顺序。每个 node：

```text
M_local = T(position) * T(origin) * R(rotation) * S(scale) * T(-origin)
M_world = M_parent * M_attachment * M_local
```

Opacity 沿 parent chain 相乘。Clip 按 world transform 后的 geometry 求交。Scale 可以为负或零；
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
- note：使用 Note 当前 presentation position 和所属 line matrix。

Attachment reference 必须存在。Render 可以读取 chart transform/presentation，chart/gameplay
不能读取 Render node。Note 不可见或 render disabled 时，note attachment 默认也不可见；node
可以显式 `followHiddenAttachment: true` 仅跟随几何而忽略 Note visibility。

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

同一 pass 的排序键：

```text
(layer.zOrder, layer.documentOrder,
 node.zOrder, node.documentOrder, node.stableId)
```

Group 不自动建立离屏 stacking context；只有 `isolate: true` 或需要 group composite/filter 时
建立。Isolated group 先按透明背景渲染 subtree，再以 group opacity/composite 合成一次。

---

## 6. Geometry

所有 geometry 参数是 length/property descriptor 且必须有限。

- Rect：origin、size；negative width/height 非法；
- RoundedRect：Rect + 四角 radius，radius clamp 规则采用 CSS corner scaling：若相邻 radius
  和超过边长，所有 radius 乘以满足约束的最小比例；
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

第一条 drawing command 前必须有 MoveTo。Close 连接当前点到 subpath 起点。Arc radius 非负；
start=end 表示零 arc，不表示 full circle；full circle 必须显式相差一整 turn。Direction 是
clockwise/counterClockwise，并按 FCS Y-up 坐标解释。Runtime 可以改变 command 的数值参数，
不能改变 command 数量和类型。

Fill rule：`nonzero` 或 `evenodd`。Stroke 对 open/closed subpath 分别处理。

---

## 8. Paint 和 Stroke

### 8.1 Paint

```text
Solid(color)
LinearGradient(start,end,stops,spread)
RadialGradient(startCenter,startRadius,endCenter,endRadius,stops,spread)
ImagePattern(resource,transform,repeat,sampling)
```

Gradient stop 数量至少 2；offset compile-time 确定、有限、位于 `[0,1]` 且非递减。同 offset
连续 stop 表示精确 color step，右侧使用后一 stop。Spread：pad、repeat、reflect。颜色使用
linear RGBA 插值。Paint resource/stop topology 编译期确定；stop color 可以动态。ImagePattern 的
resource 必须是静态 image/texture stable ID，并与 Image node 走同一 FCBC ResourceData binding、
type/hash/decode/limit contract；不能在 paint evaluator 中打开文件或切换 resource identity。

### 8.2 Stroke

```text
width: length >= 0
cap: butt | round | square
join: miter | round | bevel
miterLimit: float >= 1
dash: compile-time array<length >= 0>
dashOffset: length
```

Dash 总长度必须大于 0；奇数长度数组复制一次成为偶数。Width=0 表示不绘制 stroke，不表示
device hairline。

---

## 9. Compositing、颜色和 Clip

Core composite modes：

```text
sourceOver copy add multiply screen
```

所有合成在 premultiplied linear RGBA 中进行。输入 color 先乘 node/group opacity，再合成。
输出到 sRGB target 时最后编码。Add 每通道 clamp 到 `[0,1]`；multiply/screen 使用 W3C
Compositing 的 premultiplied source-over blend 定义。未知 mode 是 required feature error，
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

Source rect 在原始 image pixel space，左上原点、X 右、Y 下；destination 通过 node transform
进入 FCS Y-up space。超出 source bounds 是错误。Sampling：nearest 或 linear；mipmap 和
anisotropic 是 realtime quality option，不能改变 reference raster fixture。

Straight alpha decode 后转换到 premultiplied linear RGBA。Embedded ICC/profile 不得覆盖
ResourceRecord metadata 声明；冲突是 resource validation error。

Renderer 不得为了“找到可用图片”访问 workspace、相对路径、URL、系统 asset catalog 或另一个
archive，也不得使用同 hash/同 filename 的其他 resource ID 替代。Decoder 在读取 dimensions、
chunk/table count 和分配 decoded buffer 前应用公开的 image dimension/decoded-byte/metadata limit；
非法或超限 payload 分别使用 `render.resource-decode-failed` 或 `render.limit-exceeded`。这些错误不
修改已验证的原始 bytes/hash，且不能通过隐藏 Image node静默忽略 required resource。

---

## 11. Text

Text source content必须编译期确定并绑定 font resource、font face index、size、language、script、
direction 和 shaping feature set。Compiler 在 FCBC lowering 前 shaping，输出：

```text
glyphId
xAdvance/yAdvance
xOffset/yOffset
```

每个 GlyphRun 绑定一个静态 font stable ID；font ResourceRecord kind 必须为 font，字体原始 bytes
只能来自同一 FCBC ResourceData。Compiler shaping 与 runtime glyph rasterization 必须使用相同
resource bytes、face index 和声明的 shaping profile。FCBC runtime 不调用系统 font fallback；缺字
必须在编译期通过显式、已内嵌的 fallback font resource list 拆成相应 GlyphRun，否则错误。

标准 RenderSection 不保存 source text、UTF code point→glyph cluster、authoring cursor mapping 或字体
文件路径。编辑器需要的 text/cluster mapping 留在 authoring workspace；Fidelity/Debug 也不能用等价
编码把 source text 带回 FCBC。Semantic conformance 比较 font resource ID、face、glyph ID 和
position；reference raster fixture 必须携带固定 FCBC font bytes/hash。Text fill/stroke 使用普通
Paint/Stroke。

---

## 12. 动态属性

下列属性可以使用 FCBC exact Constant、SegmentTrack、Piecewise、`choose`/Expression DAG：

```text
position, origin, rotation, scale, opacity, visibility
geometry numeric parameters
paint colors and gradient geometry
stroke width/dashOffset
image destination/source numeric rectangle
glyph-run origin
material/effect parameters declared portable by extension
```

下列必须编译期确定：

```text
node type/ID/parent
pass/layer topology
attachment target kind and ID
path command topology
gradient stop count/order
resource and font identity
glyph sequence/text content
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

Render section 使用 `fcbc.md` 通用 Record/Value/StringRef/ID 编码：

```text
renderProfileMajor:u16
renderProfileMinor:u16
renderProfilePatch:u16
flags:u16
layerCount:u32
nodeCount:u32
geometryCount:u32
pathCount:u32
paintCount:u32
strokeCount:u32
clipCount:u32
glyphRunCount:u32
layers...
nodes...
geometries...
paths...
paints...
strokes...
clips...
glyphRuns...
```

`flags` 在 RenderSection 1.0 必须为 0。RenderSection 不含 resource path、hash、offset、length 或
payload table；所有 `resourceId` 都是 FCBC `fcs.resource` stable u64，唯一解析目标是同一文件的
Resources record 与 ResourceData range。Loader 按 `fcbc.md` 第 17 章先完成 resource directory/data
验证，再验证本 section 的每个 ID/kind/capability reference。

### 14.1 LayerRecord

```text
id:u64
pass:u16
reserved:u16=0
zOrder:i32
documentOrder:u32
firstRootNode:u32
rootNodeCount:u32
```

Pass ID：1 background、2 behindLines、3 lines、4 notes、5 aboveNotes、6 overlay。

### 14.2 NodeRecord

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
geometryRef:u32
fillPaint:u32 or null
strokeRef:u32 or null
clipRef:u32 or null
compositeMode:u16
reserved:u16=0
custom:Value(object)
```

Node flags bit0 unbounded active before、bit1 unbounded after、bit2 isolate、bit3 follow hidden
attachment。GeometryRef 指向 kind-specific Record，内部所有动态数值均引用 FCBC PropertyDescriptor。

Node kind ID：1 Group、2 ClipGroup、3 Rect、4 RoundedRect、5 Circle、6 Ellipse、7 Line、
8 Polyline、9 Polygon、10 Path、11 Image、12 Text。Layer 不编码为 Node。Attachment kind：
1 world、2 screen、3 line、4 note。Composite ID：1 sourceOver、2 copy、3 add、4 multiply、
5 screen。

### 14.3 GeometryRecord

GeometryRecord 是通用 Record：

```text
kind:u16
flags:u16
stableId:u64
payload:Value(object)
```

Payload 使用 FCBC `Value(object)` 编码。所有 `*Descriptor` value 与非 resource 的 `*Ref` value
必须使用 `Value(int)` tag，并限制在 `0..=u32::MAX`；它们分别解释为 FCBC descriptor index 与本
RenderSection table index。`resourceId` 必须使用 `Value(resourceRef)` tag；boolean 使用
`Value(bool)`；index list 使用 elementTag=int 的 `Value(array)`，每项同样限制为 u32。不能把这些
字段编码成 float、string 或裸的未加 tag u32。RenderSection 1.0 的标准 key：

```text
Rect          originDescriptor, sizeDescriptor
RoundedRect   originDescriptor, sizeDescriptor, radiiDescriptor[4]
Circle        centerDescriptor, radiusDescriptor
Ellipse       centerDescriptor, radiusXDescriptor, radiusYDescriptor, rotationDescriptor
Line          startDescriptor, endDescriptor
Polyline      pointDescriptorRefs[], closed=false
Polygon       pointDescriptorRefs[], closed=true
Path          pathRef
Image         resourceId, destinationRectDescriptors[4], optionalSourceRectDescriptors[4]
Text          glyphRunRef, originDescriptor
Group         empty object
ClipGroup     clipRef
```

Kind 必须与引用它的 Node kind一致。Point 数量和 array 长度在加载时按第 6 章验证。
Image `resourceId` 必须引用 image/texture；0、dangling ID 或其他 kind 使用
`render.resource-type-mismatch`/`render.resource-not-found`，不得回退默认图片。
每种 Core Geometry kind 的 object 必须恰好使用上表为该 kind 定义的 key；duplicate、missing 或
unknown key 使用 `render.invalid-geometry`。Future Render minor 或已登记 required extension 可以
定义新 key，但 Core 1.0 loader 不得把未知 key 当作可忽略 authoring metadata。

### 14.4 Path、Paint、Stroke、Clip 和 GlyphRun

这些 table 的每项都是带 stableId 的 Record。Path command ID：1 MoveTo、2 LineTo、
3 QuadraticTo、4 CubicTo、5 Arc、6 EllipseArc、7 Close；command payload 使用本规范第 7 章
顺序的 descriptor index，direction 1 clockwise、2 counterClockwise。Fill rule：1 nonzero、
2 evenodd。

Paint kind：1 Solid、2 LinearGradient、3 RadialGradient、4 ImagePattern；spread：1 pad、2 repeat、
3 reflect；sampling：1 nearest、2 linear。Stroke cap：1 butt、2 round、3 square；join：1 miter、
2 round、3 bevel。所有 array 先写 count，再写对应 descriptor/resource/index。ImagePattern resource
ID 与 Image Geometry 使用同一 image/texture binding rule。

ClipRecord 引用 GeometryRecord 和 fill rule。GlyphRunRecord 保存 font resource ID、font face、
size descriptor、glyph count，以及按 source order排列的 `(glyphId:u32, xAdvance:f64,
yAdvance:f64, xOffset:f64, yOffset:f64)`。Font resource ID 必须引用已验证的内嵌 font。FCBC 2 的
GlyphRun flags 必须为 0，record 尾部不得附带 cluster/source-text array；future Render minor 若增加
与 authoring source 无关的 runtime field，必须独立版本化且仍遵守 FCBC no-source-snapshot 约束。

Path/Paint/Stroke/Clip/GlyphRun record 按本规范字段和 source stable ID 排序；每个变长对象使用
Record length，未知 future minor 尾部可跳过。所有 reference 必须 bounds/type valid，node parent
必须无环，layer root range 必须与 parent=null 节点一致。RenderSection 验证成功只产生 resource ID
到已验证 immutable slice/decoded handle 的显式映射；不得构造 fallback search path。

---

## 15. Reference raster conformance

每个 raster fixture 固定：

- viewport width/height；
- chartTime；
- FCBC resource stable ID、kind/metadata、ResourceData 原始 bytes 与 SHA-256；
- output color space（默认 sRGB RGBA8）；
- transparent black initial target；
- 8×8 规则 subpixel sample grid，sample center 为 `(i+0.5)/8`；
- analytic/flattened path coverage误差不超过 1/1024 logical px；
- linear-light compositing；
- 最后一步 round-to-nearest-even 编码到 8-bit。

Reference image pixel 比较：每 channel absolute difference <=1 且差异 pixel 比例 <=0.1%；
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

Render 错误不能通过隐藏节点静默恢复。显式 repair/fallback 必须记录在 ConversionReport 或
repair record。

稳定 Render resource/runtime category：

```text
render.unsupported-profile
render.resource-not-found
render.resource-type-mismatch
render.resource-decode-failed
render.resource-capability-missing
render.invalid-reference
render.invalid-geometry
render.invalid-graph
render.invalid-descriptor
render.limit-exceeded
```

FCBC framing、ResourceData bounds/coverage/checksum/hash 先使用 `fcbc.*` category；只有 bounded FCBC
resource view 已验证后发生的 Render kind/media/decode/capability 问题才使用 `render.*`。同一失败不
得根据 renderer 是否尝试外部 fallback 而改变 category，因为外部 fallback 本身被禁止。

---

## 17. Conformance fixture

至少覆盖：

1. 第 2 章 grammar 的合法完整 block、缺失 viewport、非法 node kind、unbalanced/trailing payload；
2. 每种 node/geometry/path command；
3. parent transform、opacity、clip 和 isolate；
4. 所有 Core paint/stroke/composite；
5. world/screen/line/note attachment；
6. active/visibility 边界和 deterministic sorting；
7. canonical resource ID→FCBC Resources/ResourceData binding、missing/type/hash/decode boundary，及
   image color/alpha/sampling；
8. embedded fixed-font glyph run，且 RenderSection 无 source text/cluster；
9. Track/Piecewise/choose/Expression exact dynamic property与 descriptor kind 5 rejection；
10. generator 完全展开；
11. malformed graph/resource/reference；
12. semantic scene snapshot；
13. reference raster output。

Source grammar closure baseline 至少绑定一个合法完整 block、missing viewport 和 unknown node kind
三项 `source_fixture`；Render-aware parser 对后两项返回 `syntax.invalid-token`。Core I1 parser 只需
确认它们的 version envelope 与 brace/parenthesis/bracket balance，不能据此声称 Render source
conformance。

Resource binding closure 另绑定一个 semantic-only `binding_fixture`：它必须证明合法 workspace
resource reference 产生固定 canonical ID/kind/hash，FCBC writer 只在 Resources section 6 保存目录并
在 ResourceData section 20 保存原始 bytes，Render Geometry 只保存该 ID。该 fixture 不执行媒体
decode；实际 PNG/WebP/font decode 与 raster 由独立固定资源 fixture 覆盖。
