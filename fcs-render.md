# FCS Render Profile 1.0.0

状态：Frozen（2026-07-14）

本文定义 FCS Render Profile 1.0.0。它扩展 `fcs.md` 的可选顶级 `render` block，并定义
`fcbc.md` Render section 1.0。Render 使用 FCS 唯一 `chartTime` 和 property descriptor，不能
改变 Note 判定、score、sound、Line scroll 或其他 gameplay 结果。

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

---

## 2. Source 结构

```fcs
render profile 1.0.0 {
    viewport {
        width: 1920px;
        height: 1080px;
        colorSpace: linear-srgb;
    }

    layer background {
        pass: background;
        zOrder: -100;
        space: world;

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
linear RGBA 插值。Paint resource/stop topology 编译期确定；stop color 可以动态。

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

Image 必须引用 `image`/`texture` resource，且 manifest 声明 hash、media type、color space、alpha
和 sampling。Core decoding 支持由 conformance profile 列出的 PNG 8/16-bit 和 lossless WebP；
其他媒体需要 extension feature。

Source rect 在原始 image pixel space，左上原点、X 右、Y 下；destination 通过 node transform
进入 FCS Y-up space。超出 source bounds 是错误。Sampling：nearest 或 linear；mipmap 和
anisotropic 是 realtime quality option，不能改变 reference raster fixture。

Straight alpha decode 后转换到 premultiplied linear RGBA。Embedded ICC/profile 不得覆盖 manifest
声明；冲突是 resource validation error。

---

## 11. Text

Text source content必须编译期确定并绑定 font resource、font face index、size、language、script、
direction 和 shaping feature set。Compiler 在 FCBC lowering 前 shaping，输出：

```text
glyphId
xAdvance/yAdvance
xOffset/yOffset
cluster（仅 editable/debug）
```

FCBC runtime 不调用系统 font fallback。缺字必须在编译期通过显式 fallback font list 解决，
否则错误。Semantic conformance 比较 glyph ID 和 position；reference raster fixture 必须携带固定
font bytes/hash。Text fill/stroke 使用普通 Paint/Stroke。

---

## 12. 动态属性

下列属性可以使用 Track、`choose`、Expression 或 BakedCurve：

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

---

## 13. Effect 和 Shader 扩展

Core 1.0 不内置 blur、shadow、color matrix 或自定义 shader。它们必须由 extension namespace
声明：输入 texture 数、parameter type、采样边界、color space、determinism、resource limits 和
reference fallback。Required effect 不受支持时拒绝 render；optional effect 可以只在节点明确
声明 fallback paint/node 时回退，并记录 capability report。

Shader 不得读取 gameplay state 以外的隐式全局、wall clock、filesystem、network、unbounded
buffer 或先前 frame feedback，除非独立 non-portable profile 明确声明。

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

Payload 中所有 `*Descriptor` value 是 FCBC u32 descriptor index，所有 `*Ref` 是本 RenderSection
table index。标准 key：

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

### 14.4 Path、Paint、Stroke、Clip 和 GlyphRun

这些 table 的每项都是带 stableId 的 Record。Path command ID：1 MoveTo、2 LineTo、
3 QuadraticTo、4 CubicTo、5 Arc、6 EllipseArc、7 Close；command payload 使用本规范第 7 章
顺序的 descriptor index，direction 1 clockwise、2 counterClockwise。Fill rule：1 nonzero、
2 evenodd。

Paint kind：1 Solid、2 LinearGradient、3 RadialGradient、4 ImagePattern；spread：1 pad、2 repeat、
3 reflect；sampling：1 nearest、2 linear。Stroke cap：1 butt、2 round、3 square；join：1 miter、
2 round、3 bevel。所有 array 先写 count，再写对应 descriptor/resource/index。

ClipRecord 引用 GeometryRecord 和 fill rule。GlyphRunRecord 保存 font resource ID、font face、
size descriptor、glyph count，以及按 source order排列的 `(glyphId:u32, xAdvance:f64,
yAdvance:f64, xOffset:f64, yOffset:f64)`。Runtime profile 不保存 cluster；editable/debug 可以在
record 尾部附带等长 u32 cluster array并置 flags bit0。

Path/Paint/Stroke/Clip/GlyphRun record 按本规范字段和 source stable ID 排序；每个变长对象使用
Record length，未知 future minor 尾部可跳过。所有 reference 必须 bounds/type valid，node parent
必须无环，layer root range 必须与 parent=null 节点一致。

---

## 15. Reference raster conformance

每个 raster fixture 固定：

- viewport width/height；
- chartTime；
- resource bytes 与 SHA-256；
- output color space（默认 sRGB RGBA8）；
- transparent black initial target；
- 8×8 规则 subpixel sample grid，sample center 为 `(i+0.5)/8`；
- analytic/flattened path coverage误差不超过 1/1024 logical px；
- linear-light compositing；
- 最后一步 round-to-nearest-even 编码到 8-bit。

Reference image pixel 比较：每 channel absolute difference <=1 且差异 pixel 比例 <=0.1%；
包含 sharp subpixel geometry 的 fixture可以声明更严格 semantic coverage map，而不是放宽全局
容差。Realtime renderer 必须公布其 raster conformance level。

---

## 16. Resource limits 和错误

Compiler/renderer 必须限制 node、path command、point、gradient stop、glyph、clip depth、group depth、
descriptor 和 image dimension/decoded bytes。限制在处理前公开，超限拒绝。

以下是 error：未知 required profile/feature、duplicate ID、parent cycle、无效 attachment/resource、
非法 path、负 geometry、无效 gradient/dash、缺字、动态 topology、runtime node creation、非法
composite、descriptor 类型错误和非有限值。

Render 错误不能通过隐藏节点静默恢复。显式 repair/fallback 必须记录在 ConversionReport 或
repair record。

---

## 17. Conformance fixture

至少覆盖：

1. 每种 node/geometry/path command；
2. parent transform、opacity、clip 和 isolate；
3. 所有 Core paint/stroke/composite；
4. world/screen/line/note attachment；
5. active/visibility 边界和 deterministic sorting；
6. image color/alpha/sampling；
7. fixed-font glyph run；
8. Track/choose/baked dynamic property；
9. generator 完全展开；
10. malformed graph/resource/reference；
11. semantic scene snapshot；
12. reference raster output。
