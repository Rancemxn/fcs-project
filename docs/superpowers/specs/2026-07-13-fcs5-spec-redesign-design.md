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

当前 120Hz 欧拉积分只能作为 runtime-only 实现，不能作为 portable chart 的规范结果。portable chart 必须具备：

```text
portable-exact
    可验证的精确或分段精确积分

portable-baked
    带最大误差上限的 velocity/distance track

runtime-only
    依赖运行时输入，不能保证静态格式导出
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

## VM 和表达式

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

portable 表达式不得依赖外部 IO、未固定随机数、不受限递归或宿主语言未定义行为。编译器必须拒绝 speed、distance、`d`、transform 等之间的循环依赖。

NaN、Inf、除零和非法数学结果不能静默变成视锥剔除作为 portable 语义。runtime-only profile 可以有引擎 fallback，但必须生成 runtime diagnostic。

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
- portable 表达式出现循环依赖。

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
specVersion
documentVersion
bytecodeAbi
sourceFormatVersion
```

FCS 5.0 使用明确格式声明，不通过 `meta.version` 推断语义。FCBC header 至少表达：

```text
magic
spec version
bytecode ABI
profile
numeric model
feature flags
section table
source hash
```

FCBC 使用有类型、版本和长度的 section table。未知 optional section 可安全跳过；未知 required section 必须拒绝加载。推荐 profile：

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

1. 语法和单位测试；
2. 时间、区间、speed、父线和 Note 边界测试；
3. FCS → AST → semantic → FCBC → execution 的闭环测试；
4. PGR v1/v3、RPE 多 BPM/bpmfactor/多层/father/Bezier、PEC 多 bp/offset/cv/fake/Hold 的跨格式 fixture；
5. 属性测试和模糊测试。

必须验证：

- `seek(t)` 等于顺序播放到 `t`；
- 所有 portable 结果在规范容差内一致；
- chartTime、position、rotation、alpha、scrollDistance 分别使用独立误差指标；
- `fcs.md` 中的重要示例都是可执行 fixture，并带预期诊断和语义快照。

## 非目标

本文不规定：

- 最终 parser 使用哪一种 Rust parser 组合；
- FCBC 每个 section 的最终字节偏移布局；
- shader 语言的完整规范；
- PGR/RPE/PEC 引擎实现中尚未解决的历史兼容差异；
- 当前工作区已有转换器代码的迁移顺序。

这些事项必须遵守本文的语义约束，但可以在后续实现计划中分别细化。
