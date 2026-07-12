# FCS 5.0 谱面规范重设计

## 状态

设计已在本次协作中逐节确认。本文定义 FCS 5.0 的语义方向和实现约束；它不直接修改现有 FCS 4.x 文档或代码。

## 背景与目标

FCS 的目标不是只成为一种可以被引擎播放的谱面语言，也要成为 PGR、RPE、PEC 等格式之间的低损失交换格式。现有设计已经包含表达式、模板、motion、speed integral、父线和扩展字段，但部分语义仍然隐式或相互重叠：

- Note 判定时间与判定线局部 BPM 轴的职责不够清楚；
- motion 的加法、乘法、覆盖和缺口行为依赖属性名称或文本顺序；
- `speed` 同时充当速度倍率和 floor position 积分结果的来源；
- 父线继承、pivot、texture anchor、滚动距离和几何变换没有完全分离；
- fake、alpha、可见性和判定语义混在 Note 属性中；
- `rpe*` 扩展键能够保存部分字段，但不能明确区分保存与执行；
- `.fcbc` 当前更偏向最小运行时包，无法同时服务编辑、回写和归档；
- VM 环境变量 `s`、`b`、`d`、`t` 的语义与新的时间模型不一致。

FCS 5.0 的目标是：

1. 用严格、可验证的 canonical semantic model 表达跨格式共同语义；
2. 最大限度保留 PGR/RPE/PEC 的源格式信息和原始结构；
3. 保证运行时只有一个物理主时钟，避免判定与渲染脱节；
4. 让 speed、distance、父线、事件层和 Note 的边界行为可确定；
5. 明确区分原生执行、近似烘焙、仅保存和不支持；
6. 让 FCBC、编译器、参考求值器和转换器可以独立验证。

## 总体架构

FCS 5.0 采用三层模型：

```text
FCS Source
├── Canonical Semantic Layer
│   └── FCS 真正定义的跨格式谱面语义
├── Provenance / Fidelity Layer
│   └── 来源格式、原始值、无法执行的扩展、回写信息
└── Compilation
    └── Deterministic Execution Model
        └── FCBC Runtime Sections
```

### Canonical Semantic Layer

包含：

- `chartTime` 和全局 `chartBeat`；
- `tempoMap`；
- line transform graph；
- 显式 blend mode 的 motion tracks；
- `scrollTempoMap`、`scrollSpeed` 和 `scrollDistance`；
- Note gameplay 和 presentation；
- portable/runtime-only 表达式分类；
- 严格的单位、边界和错误规则。

### Provenance / Fidelity Layer

包含：

- source format 和 source version；
- 原始时间、坐标、BPM、speed 和字段路径；
- typed source payload；
- 可选 raw source snapshot；
- 字段映射、覆盖、近似和回写提示；
- conversion report。

保真层不自动获得 FCS 原生运行时语义。保存了一个源字段，不代表 FCS 引擎会执行它。

### Deterministic Execution Model

编译器负责：

- 解析和验证单位；
- 解析全局时间；
- 构建父线拓扑；
- 合成和规范化 tracks；
- 生成可查询的 scroll distance；
- 分类或烘焙表达式；
- 生成固定数值模型的 FCBC。

## 时间模型

### 唯一运行时主时钟

FCS 运行时只有一个物理主时钟：

```text
chartTime
```

它是绝对物理时间，单位为秒，用于：

- 音频同步；
- Note 判定；
- Hold 开始和结束；
- 可见性；
- line motion 事件调度；
- transform graph 求值；
- shader 时间；
- speed 和 distance 查询。

每条线不拥有可以独立暂停、快进、倒放或推进的运行时 clock。

### 全局 chartBeat

`chartBeat` 是全局音乐节拍坐标，不是第二个运行时钟。全局 `tempoMap` 负责：

```text
chartBeat ↔ chartTime
```

规则：

- 第一项从 `0.0beat` 开始；
- BPM 必须是有限且大于零的数；
- beat 节点必须非递减；
- 同一个 beat 的连续条目表示瞬时 BPM 阶跃；
- 普通 Note 和 motion 的 `beat` 默认永远指向全局 `chartBeat`；
- 编译后事件归一化到 `chartTime`。

示意：

```text
tempoMap {
    0.0beat  -> 180bpm;
    64.0beat -> 200bpm;
}
```

### lineScrollCoordinate

每条线可以有自己的滚动坐标，但它必须是 `chartTime` 的函数：

```text
lineScrollCoordinate_i = S_i(chartTime)
```

它不是独立的播放时钟。判定时间和滚动坐标分工如下：

```text
Note 判定、音频同步       → chartTime
判定线速度和 floor 距离   → lineScrollCoordinate / scrollDistance
```

当前 `bpmTimeline` 的语义应在 FCS 5.0 中重定义为 `scrollTempoMap` 或等价的滚动速率定义，不能继续表示一个隐含的独立时间轴。

### 外部格式导入

- RPE Beat 通过全局 `BPMList` 映射到 `chartTime`；`bpmfactor` 影响 line scroll 语义，不隐式修改 Note 判定时间。
- PGR 的 `T` 使用该 line 的 BPM 解析到 `chartTime`；原始 T、line BPM 和 floorPosition 保存在 fidelity 层。
- PEC 的整数时间和 `bp` 列表解析到 `chartTime`；原始 tick、BPM 分段和 offset 兼容策略保存在 fidelity 层。

## Motion Track 模型

### 显式合成

每个属性由显式 track 描述：

```text
track {
    property: positionX;
    blend: replace | add | multiply;
    priority: 0;
}
```

不再根据属性名称隐式决定加法或乘法。默认合成顺序为：

```text
base
→ 最高优先级 replace
→ 所有 add 求和
→ 所有 multiply 求积
```

同优先级的多个 replace track 在同一时刻产生冲突，应报错。

### 区间

所有连续事件统一使用左闭右开：

```text
[start, end)
```

瞬时修改使用 point/step 事件，不使用零长度普通 easing 区间。

同一 track 内的区间重叠默认是编译错误。只有显式允许 overlap 时，才可以依靠 priority 合成。文本声明顺序、格式化和编译器重排不得改变语义。

### 缺口与外推

track 必须声明缺口和首尾外推策略：

```text
fill: base | hold | zero | one | error
extrapolateBefore: ...
extrapolateAfter: ...
```

建议默认：

- replace 使用 base；
- add 使用 zero；
- multiply 使用 one；
- speed/distance 缺口使用 error。

speed 不得由编译器隐式猜测填充。

### 插值

在 `[start,end)` 内：

```text
p = (chartTime - start) / (end - start)
p' = easing(p)
value = interpolate(startValue, endValue, p')
```

默认 `p` 限制在 `[0,1]`。overshoot、异常 Bezier 控制点和 `start == end` 必须显式声明或报错。Bezier 使用四个控制参数 `[x1,y1,x2,y2]`，统一作为进度函数处理。

### 外部事件映射

- PGR 绝对移动、旋转、alpha 事件通常映射为 `replace` track；
- RPE 多事件层保留 layer index，并根据源语义映射为 `add` 或 `replace`；
- PEC 点事件和插值事件合并后通常映射为 `replace` track，原始命令边界进入 fidelity 层。

## Speed 与 Distance

### 滚动坐标

每条线的 `scrollTempoMap` 生成局部滚动坐标 `q_i(t)`：

```text
dq_i / dt = scrollBpm_i(t) / 60
```

### Speed multiplier

`scrollSpeed` 是相对于 `q_i` 的无量纲倍率。最终 floor position：

```text
floor_i(t)
    = floor_i(t0)
    + ∫ scrollSpeed_i(u) d q_i(u)
```

等价形式：

```text
floor_i(t)
    = floor_i(t0)
    + ∫ scrollSpeed_i(u)
          × scrollBpm_i(u) / 60
          du
```

### 显式 floor 单位

`floorPosition` 是无量纲累计距离；`floorScale` 是一个 floor 单位对应的 FCS logical length。它取代未定义的“基准流速”。

Note 的纵向位置遵循：

```text
Y(t)
    = (floor_i(note.chartTime) - floor_i(t))
      × floorScale_i
      × note.scrollFactor
      + note.yOffset
```

line 使用 `scrollSpeed`，Note 使用 `scrollFactor`，不再让二者共用 `speed` 名称和含义。

### 边界和积分

- `scrollTempoMap.bpm` 必须大于零；
- `scrollSpeed` 默认是 `1.0`；
- 负 speed 必须显式声明反向滚动能力；
- `floorPosition` 必须连续；
- speed 可以在事件边界跳变；
- `integrationOrigin` 和 `initialFloorPosition` 必须明确；
- speed 缺口默认编译错误；
- `seek(t)` 与顺序播放到 `t` 的结果必须一致。

FCS 5 Core 删除固定 120Hz 欧拉积分这一规范路径。固定步长运行时累加不能成为 Core `floorPosition` 的唯一真相；portable chart 必须具备：

```text
portable-exact
    可验证的精确或分段精确积分

portable-baked
    带最大误差上限的 velocity/distance track

runtime-only
    只能由声明独立 feature 和 Execution ABI 的扩展实现，
    不能冒充 Core portable 语义
```

FCBC 应同时保存 velocity descriptor、distance descriptor、积分域和误差上限。

## 坐标和变换

### 坐标空间

FCS 保留固定逻辑坐标：

```text
1920 × 1080
中心原点
X 向右为正
Y 向上为正
```

规范显式区分：

```text
world space
line-local space
note-local space
scroll space
```

Note 的横向位置默认是所属 line 的 line-local X，不是全局屏幕 X。line transform 将其转换到 world space。

### 变换矩阵

局部变换按固定顺序定义：

```text
M_local(t)
    = T(position(t))
    × T(pivot)
    × R(rotation(t))
    × S(scale(t))
    × T(-pivot)
```

父子变换在同一个 `chartTime` 求值：

```text
M_world(child,t)
    = M_world(parent,t) × M_local(child,t)
```

FCS 原生核心暂不支持不可稳定分解的 shear；外部 shear 可以作为 fidelity-only 数据保存或在 strict 转换中报错。

### Anchor 和继承

`transformOrigin` 与 `textureAnchor` 分离：

- `transformOrigin`：几何变换 pivot，使用 line-local length；
- `textureAnchor`：纹理绘制锚点，使用 0..1 坐标。

父线继承按属性声明：

```text
inherit {
    position: true;
    rotation: false;
    scale: false;
    alpha: true;
    scroll: false;
}
```

`scroll` 默认不继承。父线移动子线不应自动把父线 floor distance 与子线 floor distance 相加。RPE `rotateWithFather` 映射为 `inherit.rotation`。

父线必须构成无环有向图；不存在的父线、自环和循环依赖都是编译错误。编译器生成稳定拓扑顺序。

`zOrder`、cover 和 UI attachment 不混入几何矩阵。渲染排序至少使用：

```text
(zOrder, documentOrder, stableId)
```

## Note 模型

### 三部分结构

Note 分为：

```text
identity
gameplay
presentation
```

### Gameplay

`kind` 只包含：

```text
tap | hold | flick | drag
```

Fake 不再是 kind。可见但不可判定的 Note 使用：

```text
render.enabled = true
gameplay.judgment.enabled = false
```

Gameplay 至少包含：

```text
kind
time
endTime
side: above | below
judgment
judgeShape
soundPolicy
scorePolicy
```

规则：

- `time` 和 `endTime` 最终归一化为 `chartTime`；
- Hold 必须满足 `endTime > time`；
- 非 Hold 不得隐式使用 `endTime`；
- gameplay 属性必须编译期可确定；
- `judgeShape` 在 line-local/note-local gameplay 坐标中定义；
- `alpha=0`、不可见和 fake 不自动关闭判定。

### Presentation

Presentation 至少包含：

```text
positionX
scrollFactor
xOffset
yOffset
alpha
scaleX
scaleY
color
texture
visibleFrom
visibleUntil
```

可见性、alpha 和 render enabled 是三种独立语义。`visibleTime` 等源字段必须转换成明确的绝对显示区间，并保存原始字段和换算规则。

Note 的横向位置和纵向位置：

```text
localX = positionX + xOffset
localY(t) = (
    scrollDistance(note.chartTime)
    - scrollDistance(t)
)
× floorScale
× scrollFactor
+ yOffset
```

Hold 的头、身和尾具有明确的 start/end 语义；渲染属性可以按身体顶点的 `d` 求值，但 gameplay 结果不能依赖渲染帧率。

每个具体 Note 具有稳定唯一 ID。输出排序使用确定性键，不依赖哈希表迭代顺序。

## 编译期元编程

FCS 5.0 采用分层、有限、纯函数式的元编程模型：

```text
const
    顶层编译期常量

let
    局部不可变绑定

fn
    返回强类型纯值

template T
    构造一个 T 类型实体

generate
    在 collection block 中进行有限编译期迭代

emit
    向当前 collection 输出一个实体

if
    编译期结构选择

choose
    运行时纯值选择
```

顶层 `definitions` 块统一容纳 `const`、`fn` 和 typed entity template：

```fcs
definitions {
    const NOTE_SPACING: length = 120px;

    fn wave(at: beat, period: beat, amplitude: length) -> length {
        let phase: float = at / period;
        return sin(phase * 2pi) * amplitude;
    }

    template Note ghostTap(hitTime: beat, x: length) {
        return tap {
            gameplay {
                time: hitTime;
                judgment.enabled: false;
            }
            presentation {
                positionX: x;
            }
        };
    }
}
```

### 类型和变量

元编程基础类型至少包含：

```text
bool
int
float
string
time
beat
length
angle
color
vec2<T>
```

实体类型至少包含：

```text
Note
Line
RenderNode
TrackSegment<T>
Keyframe<T>
```

`time` 表示物理时间，`beat` 表示全局 `chartBeat`，二者不得隐式互换。需要转换时必须通过 `tempoMap` 的显式映射。

所有 `const` 和 `let` 必须显式标注类型并在定义时初始化。它们均不可变，不提供 `var`、赋值、`++`、`+=`、可变 collection 或全局可变状态。同一作用域以及嵌套作用域均禁止 shadowing。所有局部绑定在 FCBC lowering 前被常量折叠、内联或转换成 expression DAG，不产生运行时 variable slot。

### 纯函数

`fn` 的参数和返回类型必须显式声明。函数无副作用，只能返回纯值，可以调用其他 `fn`，但不能调用 template、`generate` 或 `emit`。函数调用图必须无环。

属性访问允许逐层读取静态类型中存在的字段，例如：

```text
note.presentation.visibility.from
line.transform.inherit.rotation
```

比较两侧必须类型相同并支持对应运算。数字、time、beat、length、angle 支持有序比较；string、bool、color 仅支持 equality；实体不支持 equality。Float equality 使用精确语义，近似比较必须显式使用 `approxEq(value, expected, tolerance)`。

### Typed entity template

`template T` 每次只返回一个明确类型的实体，不隐式返回列表。template 可以调用 `fn` 和其他 template，但调用图必须无环。不可变实体修改使用：

```fcs
return base with {
    presentation.scaleX: 1.25;
    presentation.color: #FFAA00FF;
};
```

独立的 Note prototype inheritance 在 FCS 5.0 中删除，由 typed template composition 和 `with` 统一替代。

Template 中允许 statement-level `if`，但凡影响实体结构、Note kind、字段存在性或 emit 行为的条件，都必须在实例化时编译期可求值。动态 presentation 值使用 `choose`，不能用 template `if` 动态生成不同结构。

### Generate 与 emit

推荐语法：

```fcs
notes {
    generate at: beat in 20beat..<80beat step 10beat {
        let phase: float = (at - range.start) / 10beat;

        if index % 2 == 0 {
            emit normalTap(at, sin(phase * 2pi) * 150px);
        } else {
            emit ghostTap(at, cos(phase * 2pi) * 150px);
        }
    }
}
```

Generator 上下文包含：

```text
用户命名的迭代变量
index: int
range.start
range.end
range.step
range.count
```

不使用 `ct`、`st`、`et` 等缩写。`start..<end` 表示半开区间，`start..=end` 表示包含终点。`start`、`end`、`step` 必须类型相同、编译期可求值且 step 不为零。当前值按 `start + index × step` 精确计算，禁止 Float64 重复累加。

`generate` 只能出现在 collection block 中：

```text
notes                  → emit Note
judgelines             → emit Line
render children/layer  → emit RenderNode
track<T>               → emit TrackSegment<T> 或 Keyframe<T>
```

Generator 不能嵌套，不能出现在 `fn` 或 template 中，不能修改外部状态，不能依赖 runtime-only 值。允许编译期常量 `if` 决定是否实例化一个 generator。

### 展开限制

编译 profile 必须定义：

```text
maxExpansionDepth
maxGeneratedNodes
maxGeneratorIterations
maxTemplateInstances
maxCompileTimeOperations
maxExpressionNodes
```

函数、template 和 const 依赖图中出现环时立即报错。最大深度只保护合法无环调用链，不允许“运行递归直到达到深度上限”。错误必须包含 generator index、template 和函数调用组成的 expansion trace。

所有 `const`、`let`、`fn`、template、`generate`、`emit`、statement-level `if`、range 和 index 必须在 canonical semantic lowering 前完全消失。

## VM、条件值与表达式

公共环境变量：

```text
s = chartTime，秒
b = chartBeat，全局 beat
q = lineScrollCoordinate
d = Note 到判定线的有符号 logical distance
p = 当前 track 区间的 normalized progress，[0,1]
```

`b` 不再表示 line-local beat。`t` 不再承担多重含义；如果保留兼容别名，只能定义为 `t = p`。

表达式分类：

```text
constant
chart-time
line-scroll
note-presentation
runtime-only
```

FCBC 没有通用控制流：禁止 jump、loop、recursion、mutable local 和运行时结构生成。所有 source-level `if` 必须在编译期消失。

动态属性允许有限、强类型、无副作用的纯值选择：

```fcs
alpha: choose {
    when d < 50px  => 1.0;
    when d < 200px => 0.5;
    else           => 0.0;
};
```

`choose` 的 predicate 必须是 bool；所有结果分支类型和单位完全相同；必须有 `else`；predicate 按声明顺序匹配；只有首个匹配分支的结果具有求值语义，未选择分支不求值。`choose` 只能返回值，不能 emit、创建实体、修改状态或改变 gameplay 结构。

编译器依次尝试：

```text
constant folding
→ exact track partition
→ analytic boundary lowering
→ finite PiecewiseDescriptor
→ target export adaptive baking
```

Gameplay 的 kind、time、endTime、judgment、side、judgeShape、parent 和 inherit 禁止动态 `choose`。Presentation 和 render 参数可以使用。`scrollSpeed` 的条件只能依赖 `s`、`b`、`q` 和常量，不能依赖 `d`、Note 状态、render 状态或外部输入。

FCBC 表达式表示应是 typed expression DAG 或没有 jump 的受限 stack expression，节点仅包含 constant、environment、unary、binary、compare、math、easing 和 piecewise；不得包含 jump、store、recursive call、emit 或 generate。

portable 表达式不得依赖外部 IO、未固定随机数、不受限递归或宿主语言未定义行为。编译器必须拒绝 speed、distance、`d`、transform 等之间的循环依赖。NaN、Inf、除零和非法数学结果不能静默变成视锥剔除作为 portable 语义。

## 精度与自适应烘焙

FCS 5.0 不规定所有属性按固定 1000Hz 存入 FCBC。正确性由误差上限定义；strict profile 将 `1ms` 作为无法通过解析方法证明时的最大验证间隔，而不是强制输出采样间隔。

每个属性依次归类为：

```text
Exact Constant
Exact Segment
Exact Piecewise
Adaptive Baked
```

常量、step、线性、已知 easing、Bezier 和可解析 piecewise 使用精确表示。只有不能精确 lowering 的表达式进入 adaptive baked。事件、BPM、speed、Note、Hold、visibility、render active 和 generator emit 的时间点始终是强制精确边界，不能量化到 1ms 网格。

Adaptive baked 通过递归细分、拟合、误差检查和相邻 segment 合并生成 constant、linear、step、cubic Hermite、cubic Bezier 或必要的 dense LUT segment。输出 `BakedCurve` 必须记录定义域、value type、interpolation、segments、declared max error、validation profile 和 source expression hash。

误差按属性分别定义，不使用单一 epsilon：

```text
position / offset   logical px
rotation            angle distance
alpha / scale       scalar absolute error
color               linear RGBA channel error
scrollSpeed         scalar absolute/relative error
scrollDistance      floor absolute error
render geometry     logical px
shader parameter    declared parameter tolerance
```

Speed 同时验证瞬时 velocity 和累计 distance。积分和误差检查在 `chartTime` 域进行，不再规定固定 `0.001beat` RK4。实现可以使用解析积分、自适应 Gauss–Kronrod、Simpson 或高阶方法，只要满足声明误差。

FCBC 核心时间、位置、角度、speed、distance、Bezier、easing 和 transform 均使用 Float64，不再让 const 或 keyframe 默认降为 Float32。Source/canonical 阶段尽量保留 decimal 和 rational beat；generator 以精确 `start + index × step` 计算。

编译 profile 必须限制 baked segment、验证求值、细分深度和烘焙时间。无法在预算内满足误差时，strict 编译失败，不得输出误差未知结果。编译烘焙精度、实时渲染帧率和离线视频帧率是三个独立概念。

## FCS Render Profile

`render` 是 FCS Core 的可选顶级块，具体能力以独立的 FCS Render Profile 版本化。Render 借鉴 Canvas 2D 的 geometry、path、paint、clip、image 和 compositing 能力，但不复制其可变 context、state stack 或 pixel IO。

Source 使用 retained scene graph，FCBC 保存固定 node graph 或已展平 display list：

```fcs
render {
    layer background {
        zOrder: -100;
        space: world;

        group pulse {
            active: [8beat, 16beat);
            opacity: 0.8;

            children {
                circle ring {
                    center: (0px, 0px);
                    radius: 100px;
                    fill: solid(#FF4444FF);
                }
            }
        }
    }
}
```

核心节点包括 Layer、Group、ClipGroup、Rect、RoundedRect、Circle、Ellipse、Line、Polyline、Polygon、Path、Image 和 Text。节点数量、类型、parent、path topology、font 和 blend mode 必须编译期确定；position、rotation、scale、opacity、color、geometry parameter、visibility 和 material parameter 可以使用 track、`choose` 或 baked curve。

Group 使用词法 scoped transform、opacity、clip 和 paint，不暴露 `save()` / `restore()`。Path 是不可变编译期资源，支持 move、line、quadratic、cubic、arc、ellipse arc 和 close；不允许运行时改变 command 数量。

Paint 使用结构化 Solid、LinearGradient、RadialGradient 和 ImagePattern；stroke 明确 width、cap、join、miter、dash。颜色在线性空间插值，合成使用 premultiplied linear RGBA。核心 composite modes 限定为 sourceOver、copy、add、multiply 和 screen；更多模式由 feature flag 扩展，不能静默回退。

Render 支持 world、screen、line(lineId) 和 note(noteId) attachment。依赖只能从 chart/gameplay 指向 render，render 不得反向改变判定。排序使用 render pass、layer zOrder、node zOrder、document order 和 stable ID。

Image 必须引用带 hash、media type、color space、alpha 和 sampling 声明的资源。Text 必须绑定字体资源，内容编译期确定，并在编译期 shaping 为 glyph IDs 和 positions；FCBC 不依赖系统字体 fallback。Pixel API（get/put image data、逐像素读写）明确排除，像素效果使用 typed effect 或 shader profile。

Render conformance 分为 semantic conformance 和 reference raster conformance。实时 GPU renderer 可以在规范像素容差内不同；项目提供软件 reference renderer 生成 canonical fixture。FCBC Render Section 包含 layer、node、hierarchy、path、paint、stroke、clip、glyph run、image reference 和 property descriptor table，不包含 generate、emit、template、save/restore 或 runtime node creation。

## Metadata、Credits、Resources 与 Sync

FCS Core 不强制标题、曲师、谱师、封面或发布信息，但通过 document profile 规定最低能力：`fragment` 不要求完整 chart；`chart` 要求有效 tempo/time model；`playable` 额外要求 primary audio、sync 和 gameplay 资源；`renderable` 要求 render/chart visual 及全部渲染资源；`publishable` 额外要求 title、documentId、chartVersion、至少一条 credit、license 声明和资源 hash。

`meta` 只保存文档描述，例如 title、subtitle、alternativeTitles、chartVersion、difficulty、level、description、language、tags、license、documentId、revision 和 `custom`。FCS 格式版本不再存入 `meta.version`。

人员和展示角色使用独立结构：

```fcs
contributors {
    person alice {
        name: "Alice";
        aliases: ["AliceP"];
    }
}

credits {
    credit {
        role: composer;
        label: "作曲";
        contributors: [@alice];
    }
}
```

Credit 同时保存机器可读的标准 role 和人类展示 label，并允许 `custom("role-id")`。旧 `artists`、`charters` 和含义混杂的 `illustration` 被删除；来源不明确的 artist 不得擅自认定为 composer。图片资源、图片用途和插画作者分别进入 resources、artwork 和 illustrator credit；不引入含义不清的 `er`。

`resources` 是标准顶级块，支持有稳定 ID、source、hash、media type 和 custom data 的 audio、image、font、texture、path、shader 等资源。`artwork` 引用图片资源。音频 offset 从 meta 移入 `sync`，并固定定义：

```text
audioTime = chartTime + audioOffset
chartTime = audioTime - audioOffset
```

因此 `audioOffset = +100ms` 表示在 `chartTime = 1.0s` 求值时读取音频 `1.1s`；所有 parser、player、converter 和 fixture 必须使用这一公式，不得再依据“谱面比音乐快”等自然语言自行解释符号。

所有 FCBC profile 都保留完整 canonical MetaSection、ContributorSection、CreditSection、ResourceManifestSection 和 SyncSection。可剥离的是源码注释、格式、未使用 definitions、调试 span、原始外部格式 snapshot 和编辑器状态；archive profile 可用 SourceSnapshotSection 保存原始 UTF-8 source。

Custom metadata 使用有深度、字段数、字符串长度和总字节限制的 typed ordered object，支持 null、bool、int64、float64、string、time、beat、color、resource/contributor reference、array 和 ordered object。重复 key、非有限 float 和无效 reference 是错误。Credits 顺序有展示语义；普通标准 meta 字段顺序无语义。

## Fidelity、扩展和转换报告

### 命名空间和来源

extension 必须有 namespace、schema/version 和类型。推荐使用结构化 preserve 数据：

```text
preserve {
    source {
        format: "rpe";
        version: 170;
        hash: "sha256:...";
    }
    payload { ... }
}
```

可选 raw snapshot 只保证“源数据未被修改时”的精确回写。源数据被 FCS 原生语义覆盖后，转换器必须重新判断可回写字段。

来源状态至少区分：

```text
unset
explicit-default
explicit-value
inherited
imported
generated
user-modified
```

不能通过“值是否等于默认值”猜测用户是否明确设置了字段。

### 语义状态

字段映射必须能标记：

```text
native
mapped
preserved
approximated
runtime-only
dropped
```

提供：

```text
semantic
roundtrip
strict
```

三种导出策略：

- `semantic`：以当前 FCS 原生语义为准，允许报告损失；
- `roundtrip`：尽量使用来源数据回写，用户显式修改覆盖来源字段；
- `strict`：任何无法证明等价的近似、仅保存、丢弃或冲突都失败。

转换结果必须有机器可读的 ConversionReport，整体状态至少包括：

```text
lossless
equivalent
approximate
preserved-only
unsupported
failed
```

目标格式必须声明 capabilities，转换前先判断直接支持、需要烘焙、只能保存还是不支持。

## 严格错误和修复

portable profile 中以下情况默认是错误：

- NaN、Infinity；
- 非法单位；
- BPM 小于等于零；
- 时间轴非法；
- parent 循环或不存在；
- Hold `endTime <= time`；
- 同一 track 重叠；
- speed 缺口；
- 非法 easing 或 Bezier；
- 坐标、索引或 section 溢出；
- portable 表达式出现循环依赖；
- const、fn 或 template 调用图存在环；
- generator range 非有限、step 为零或超出展开预算；
- `choose` 缺失 else、predicate 非 bool 或分支类型不同；
- baked curve 无法在预算内满足误差；
- Render resource、font、attachment 或 clip 无效；
- FCBC/FCS/ABI major version 不受支持。

不得默认静默执行：

```text
NaN → 隐藏
非法 father → 无父线
speed gap → 猜测填充
非法事件顺序 → 自动交换
alpha → 静默 clamp
```

如果提供 `--repair`，每次修复都必须生成 repair record，包含来源路径、动作、原值和新值。

## 版本和 FCBC

明确分离：

```text
sourceFcsVersion
fcbcFormatVersion
executionAbiVersion
sectionVersion
renderProfileVersion
documentVersion / meta.chartVersion
compilerVersion
sourceFormatVersion
```

每个 FCS source 必须用文件头或 format block 声明完整 FCS 规范版本，例如：

```fcs
#fcs 5.0.0
```

它决定源码语法、类型、时间、元编程、track、Note、Render 接口和诊断语义。谱师自己的版本使用 `meta.chartVersion`，不得参与格式兼容判断。

每个 FCBC header 必须同时记录来源 FCS 规范版本、FCBC 容器格式版本和 Execution ABI 版本。FCBC header 至少表达：

```text
magic
source FCS major/minor/patch
FCBC format major/minor/patch
execution ABI major/minor/patch
profile
numeric model
feature flags
section table
source hash
compiler id/version（仅诊断）
```

`fcbcFormatVersion` 决定 header、section framing、offset、length、string table 和 constant pool 的二进制布局。`executionAbiVersion` 决定 PropertyDescriptor、Expression DAG、PiecewiseDescriptor、RenderNode、velocity/distance descriptor 和数值求值接口。新增一个完全在编译期消失的 FCS 语法功能可以只提升 source FCS minor，而不改变 FCBC 或 ABI。

FCBC 使用有类型、独立版本和长度的 section table：

```text
SectionEntry {
    sectionType;
    sectionVersion;
    flags;
    offset;
    length;
}
```

未知 optional section 可安全跳过；未知 required section 必须拒绝加载。Render Profile、extension schema 和被导入的 source format 分别记录自己的版本。Compiler version 仅用于复现和 bug 追踪，运行时不得根据 compiler version 猜测语义。

所有版本遵循：major 表示不兼容变化，minor 表示可兼容新增，patch 表示不改变有效输入语义的澄清或修正。推荐 FCBC profile：

```text
runtime
editable
archive
strict-runtime
```

同一输入、规范版本和 profile 应生成确定性的 FCBC：字符串表、常量池、line、note、track、section、padding 和 source hash 都必须有稳定规则。

## 验证体系

建立独立 reference evaluator，不直接复用最终引擎的优化路径。至少提供：

```text
lineTransform(t)
lineScrollCoordinate(t)
lineScrollDistance(t)
notePresentation(t)
noteJudgmentGeometry
```

测试分为：

1. 语法、类型、单位和版本测试；
2. const/fn/template/generate 展开、环检测和预算测试；
3. 时间、区间、speed、父线、Note 和 Piecewise 边界测试；
4. adaptive baked 属性误差、1ms 最大验证间隔和 Float64 精度测试；
5. Render scene graph、资源、glyph run、排序和 reference raster 测试；
6. Meta、credits、resources、sync 和 FCBC 完整保留测试；
7. FCS → AST → semantic → FCBC → execution 的闭环测试；
8. PGR v1/v3、RPE 多 BPM/bpmfactor/多层/father/Bezier、PEC 多 bp/offset/cv/fake/Hold 的跨格式 fixture；
9. 属性测试和模糊测试。

必须验证：

- `seek(t)` 等于顺序播放到 `t`；
- 所有 portable 结果在规范容差内一致；
- chartTime、position、rotation、alpha、scrollDistance 分别使用独立误差指标；
- 所有元编程结构在 canonical semantic AST 中完全消失；
- FCBC 不包含 jump、loop、recursive call、emit 或 generate；
- 同一 input/spec/profile/compiler mode 产生确定性 FCBC；
- FCBC header 同时记录来源 FCS、FCBC format 和 Execution ABI 版本；
- `fcs.md` 中的重要示例都是可执行 fixture，并带预期诊断和语义快照。

## 非目标

本文不规定：

- 最终 parser 使用哪一种 Rust parser 组合；
- FCBC 每个 section 的最终字节偏移布局；
- shader 语言的完整规范；
- Render Profile 高级 effect、完整 blend mode 和 pixel-exact GPU 实现；
- PGR/RPE/PEC 引擎实现中尚未解决的历史兼容差异；
- 当前工作区已有转换器代码的迁移顺序。

这些事项必须遵守本文的语义约束，但可以在后续实现计划中分别细化。
