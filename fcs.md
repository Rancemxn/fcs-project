 # FCS (Functional Chart Specification) — 谱面格式规范 v4.1.0

 > FCS 是面向新一代社区谱面编辑器的声明式谱面格式。
 > 核心哲学：**一切动态属性皆为独立变量（时间或空间）的连续函数**。

---

 ## 1. 文件类型与编译管线

 | 后缀 | 名称 | 性质 | 用途 |
 |------|------|------|------|
 | `.fcs` | FCS Source | 纯文本 UTF-8 | 供谱师编写、人类阅读 |
 | `.fcbc` | FCS Bytecode | 二进制 | 供引擎运行时**唯一**读取 |

 - 打包发布时 AOT Compiler 将 `.fcs` 编译为 `.fcbc`。
 - 编译单向且不可逆：提取所有数学表达式预编译为底层操作码（Opcodes），抹除符号名与注释。
 - **零代价抽象**：原型继承、模板调用等抽象在编译期内联展平（Flatten），运行时无递归查找开销。
 - 引擎运行时仅加载 `.fcbc`，不解析 `.fcs` 源文件。

---

 ## 2. 词法与基础语法

 ### 2.1 字符集与编码

 - 源文件编码: **UTF-8**（必须，无 BOM）。

 ### 2.2 标识符

 ```
 ident := [a-zA-Z_][a-zA-Z0-9_]*
 ```

 命名约定：属性与字段使用 **lowerCamelCase**；结构块关键字使用 **lower_case**；类型名在文档中使用 **PascalCase**。

 ### 2.3 比较表达式

 支持**链式比较**语法 `a < b < c`，语义等价于 `a < b` 且 `b < c`。不引入 `and`/`or` 逻辑运算符，多条件分支通过嵌套 `if...else` 实现。

 基本比较运算符: `<`, `>`, `<=`, `>=`, `==`, `!=`。

 ### 2.4 字符串

 - 仅支持双引号 `"..."`。
 - 转义序列：`\n`, `\t`, `\\`, `\"`, `\u{XXXXXX}` (Unicode 码点，1~6 位十六进制)。
 - 遇到单引号 `'...'` 抛出 `SyntaxError` (Fatal)。

 ### 2.5 注释

 - 单行: `// ...`
 - 多行: `/* ... */`

 ### 2.6 数值字面量

 | 类型 | 示例 | 说明 |
 |------|------|------|
 | 整数 | `42` | i64 |
 | 浮点 | `3.14`, `1e-3` | IEEE 754 Float64 |
 | 带单位数值 | `120ms`, `0.5vw`, `45deg` | 数值 + 单位标识符，中间无空格 |

 ### 2.7 结构块

 - 所有声明使用 C-style 大括号 `{}` 界定作用域。
 - 所有语句以分号 `;` 结尾。
 - 允许尾随逗号。

---

 ## 3. 严格单位系统 (Strict Unit System)

 任何具有物理/几何意义的数值**必须**显式附带单位。禁止隐式转换。

### 3.1 时间单位

| 单位 | 含义 | 换算与解析上下文 |
|------|------|------------------|
| `ms` | 毫秒 | 1s = 1000ms |
| `s` | 秒 | 基准 |
| `b` | Beat (节拍) | 依据上下文解析（见下文） |

**时间戳解析唯一性规则**：
1. **Note 的 `time` 与 `endTime` 属性**：若使用 `b` 单位，**必须且只能**依据全局 `masterTimeline` 解析为绝对物理秒数 `s`。
2. **运动区间 `[startBeat => endBeat]` 与 VM 环境变量 `b`**：依据该判定线局部的 `bpmTimeline` 解析。
3. **编译期转换**：AOT 编译器在编译为 `.fcbc` 时，会将所有 Note 的 `timeBeat` 与 `endTimeBeat` 统一换算为基于局部 `bpmTimeline` 的节拍数值，以确保运行时 VM 变量 `b` 的自洽性。

 ### 3.2 空间/长度单位

 | 单位 | 含义 |
 |------|------|
 | `px` | 逻辑像素 |
 | `vw` | 视口宽百分比。1vw = 视口宽度的 1% |
 | `vh` | 视口高百分比。1vh = 视口高度的 1% |

 **逻辑坐标系定义**:

 引擎内部建立一个正交投影矩阵 (Orthographic Projection)，逻辑坐标系固定为:

 - **宽度**: 1920 单位
 - **高度**: 1080 单位
 - **中心**: `(0, 0)`
 - **Y 轴向上为正**, X 轴向右为正

 在此坐标系中:
 - `1px` = 逻辑坐标系的 1 个 X 轴单位。
 - `1vw` = 逻辑坐标系的 19.2 个 X 轴单位（即 1920 / 100）。
 - `1vh` = 逻辑坐标系的 10.8 个 Y 轴单位（即 1080 / 100）。

 引擎通过此正交投影矩阵直接完成逻辑坐标到屏幕像素的变换，无需额外换算。

 ### 3.3 角度单位

 | 单位 | 含义 |
 |------|------|
 | `deg` | 度 (°) |
 | `rad` | 弧度 |

 ### 3.4 无量纲倍率

 用于 `speed`, `scaleX`, `scaleY`, `alpha` 等属性的纯数字，**不写单位**。

 ### 3.5 颜色

 仅支持 HEX 格式:
 - `#RRGGBB` (8-bit sRGB)
 - `#RRGGBBAA` (8-bit sRGB + Alpha)

 引擎内部采用**线性色彩空间 (Linear Color Space)** 进行混合与渲染。

 ### 3.6 类型检查异常

 | 场景 | 异常 |
 |------|------|
 | 期望长度的字段被赋予无量纲数字 (如 `positionX: 100;`) | `TypeError` — 编译失败 |
 | 期望无量纲的字段被赋予带单位值 (如 `speed: 2.0px;`) | `TypeError` — 编译失败 |
 | 角度字段被赋予长度值 | `TypeError` — 编译失败 |
---

 ## 4. 运行时数学引擎 (Math Engine VM)

 ### 4.1 内置环境变量 (State Variables)

 引擎在计算表达式时自动注入以下变量：

 | 变量 | 类型 | 含义 |
 |------|------|------|
 | `s` | Float64 (秒) | 受主时间轴影响的绝对物理秒数 |
 | `b` | Float64 (拍) | 受**当前判定线局部 BPM 轴**影响的节拍数 |
 | `d` | Float64 (px) | **仅 Note 控制内有效**。该 Note 距离判定线中心的 Y 轴像素距离。`d = floorPosition − currentFloorPosition` |

 ### 4.2 数学常量

 | 常量 | 值 | 精度 |
 |------|-----|------|
 | `pi` | π | IEEE 754 Float64 |
 | `e` | e | IEEE 754 Float64 |

 ### 4.3 内置函数

 ```
 sin(x), cos(x), tan(x)
 asin(x), acos(x), atan(x), atan2(y, x)
 abs(x), exp(x), ln(x), log2(x), log10(x)
 sqrt(x), pow(base, exp)
 floor(x), ceil(x), round(x)
 min(a, b), max(a, b), clamp(x, lo, hi)
 lerp(a, b, t)
 ```

 ### 4.4 缓存函数 (Easing)

 详见 [§7 缓存函数](#7-缓存函数-easing)。

---

 ## 5. 文档结构定义

 ### 5.1 顶层结构 (必知)

 `.fcs` 文件必须按以下**顺序**包含块:

 ```
 fcsDocument := MetaBlock MasterTimelineBlock TemplatesBlock? JudgelinesBlock ShadersBlock?
 ```

 | 块 | 必需 | 出现次数 | 说明 |
 |----|------|---------|------|
 | `meta { }` | **是** | 1 | 谱面元信息 |
 | `masterTimeline { }` | **是** | 1 | 全局主时间轴 |
 | `templates { }` | 否 | 0~1 | 可复用数学逻辑模板 |
 | `judgelines { }` | **是** | 1 | 所有判定线定义 |
 | `shaders { }` | 否 | 0~1 | 自定义着色器绑定 |

 所有块之外的内容（包括空行和注释）允许但不产生语义效果。

---

 ### 5.2 `meta` 块

 ```
 meta {
     name: "Chart Name";
     artists: ["Artist A", "Artist B"];
     charters: ["Mapper X"];
     offset: -120ms;
     version: "2.1.0";
     illustration: "Artist Name";
     level: "IN Lv.15";
     customField: "any value";
     customId: 42;
 }
 ```

 **必需字段**:

 | 字段 | 类型 | 说明 |
 |------|------|------|
 | `name` | `string` | 谱面名称 |
 | `artists` | `string \| string[]` | 曲师 |
 | `charters` | `string \| string[]` | 谱师 |
 | `offset` | 时间单位 | 音乐偏移。正数=谱面比音乐快，负数=谱面比音乐慢 |
 | `version` | `string` | FCS 格式版本 |

 **开放扩展**: `meta` 允许任意额外键值对，类型限定为 `string`, `int`, `float`, `bool`, `string[]`。编译器对其不理解的键发出 `Warning` 但不影响编译。**注意**: `.fcbc` 字节码仅保留预定义的必需字段；自定义字段在编译时被剥离。

 #### 5.2.1 RPE 预定义扩展元信息

 FCS 转谱器在将 RPE 谱面转换为 FCS 格式时，会将以下 RPE 独有的 META 字段作为**预定义扩展键**（以 `rpe` 前缀标识）写入 `meta` 块。这些键在 FCS 引擎中**完全无语义**（no-op），仅供转谱器在 FCS → RPE 方向上**无损还原**原始 RPE 元信息。

 | 扩展键 | 类型 | 说明 | RPE 原始字段 |
 |--------|------|------|-------------|
 | `rpeVersion` | `int` | RPE 版本号 (100–160) | `META.RPEVersion` |
 | `rpeId` | `string` | 谱面 ID | `META.id` |
 | `rpeChartTime` | `float` | 谱面编辑时长，单位秒 | `chartTime` |
 | `rpeSong` | `string` | 音乐文件相对路径 | `META.song` |

 **冲突规则**: FCS 原生 `version` / `offset` 等字段优先于任何 RPE 扩展键。转谱器处理冲突时遵循 §5.6.4.1 优先级规则。

---

 ### 5.3 `masterTimeline` 块

 定义全局基准 BPM 时间轴。

 ```
 masterTimeline {
     0.0b   -> 180.0;
     16.0b  -> 90.0;
     32.0b  -> 180.0;
 }
 ```

 **BPM 阶跃 (Step)**: 若同一 beat 处出现**连续两个条目**，表示该点发生阶跃突变——BPM 在前一条目的值瞬间跳变至后一条目的值:

 ```
 masterTimeline {
     0.0b   -> 180.0;
     16.0b  -> 180.0;   // 阶跃前值
     16.0b  -> 90.0;    // 阶跃后值（瞬时跳变）
     32.0b  -> 90.0;
 }
 ```

 **规则**:
 - 第一个条目必须从 `0.0b` 开始。
 - 条目必须按 beat 非降序排列。
 - 非阶跃的相邻条目之间 BPM 线性插值；阶跃 beat 处瞬时跳变。
 - 最后一条目之后 BPM 保持终值不变。
 - 至少需要 1 个条目。

---

 ### 5.4 `templates` 块 (可选)

 定义可复用的数学逻辑片段。编译器在编译期内联展开——**零运行时开销**。

 模板参数支持**类型标注**，允许在条件中引用 `line` / `note` 的属性进行分支:

 ```
 templates {
     def adaptiveX(note: Note, line: Line) {
         if (line.alpha < 0.5) { return 0px; }
         if (note.positionX > 100px) { return note.positionX * 1.5; }
         else { return note.positionX; }
     }
 
     def scaledSpeed(note: Note, factor: float) {
         return note.speed * factor;
     }
 }
 ```

 **参数类型**: `Note`, `Line`, `float`, `int`, `bool`, `time`, `length`, `angle`。属性访问路径如 `note.positionX`, `line.alpha`。

 **规则**:
 - 模板内可使用 `if...else` 控制流与链式比较，但**禁止循环** (模板必须保证 O(1) 求值)。
 - 模板可调用其他已定义的模板 (前向引用禁止)。
 - 模板仅存在于 `.fcs` 源文件，编译时内联展开到调用点。
---

 ### 5.5 `judgelines` 块

 ```
 judgelines {
     line <name> { ... }
     line <name> { ... }
 }
 ```

 #### 5.5.1 `line` 完整结构

 ```
 line lineName {
     texture: "path/to/texture.png";
     textureAnchor: (0.5, 0.5);
     zOrder: 0;
     color: #FFFFFFFF;
     parent: parentLineName;
     inherit: [position, rotation];
 
     bpmTimeline {
         0.0b  -> 180.0;
         16.0b -> 180.0;   // 阶跃前值
         16.0b -> 90.0;    // 阶跃后值（瞬时跳变）
     }
 
     motion {
         layer {
             positionX { ... }
             positionY { ... }
             rotation { ... }
             alpha { ... }
             scaleX { ... }
             scaleY { ... }
             speed { ... }
         }
     }
 
     notes { ... }
 }
 ```

 **`line` 属性定义**:

 | 属性 | 类型 | 默认值 | 必需 | 说明 |
 |------|------|--------|------|------|
 | `texture` | `string` | (引擎默认纹理) | 否 | 判定线纹理路径 |
 | `textureAnchor` | `(float, float)` | `(0.5, 0.5)` | 否 | 纹理锚点，范围 [0, 1] |
 | `zOrder` | `int` | `0` | 否 | 图层顺序，值越大越靠前 |
 | `color` | `color` | `#FFFFFFFF` | 否 | 线的颜色 (顶点颜色乘法) |
 | `parent` | `ident` | (无) | 否 | 父线名称 |
 | `inherit` | `ident[]` | `[position]` | 否 | 继承列表 |
 | `bpmTimeline` | 块 | — | **是** | 局部 BPM 轴，支持阶跃 |
 | `motion` | 块 | (无) | 否 | 运动定义 |
 | `notes` | 块 | (无) | 否 | Note 定义 |

 **`inherit` 可选项**: `position`, `rotation`, `scale`, `alpha`。默认 `[position]`。

* 其运行期求值规则：
      - 静态属性（如无 motion 块的常数位置/角度）由编译器在编译期进行展平合并。
      - 动态属性（含有表达式或关键帧变换）：编译器不进行展平，而是在 `.fcbc` 中保留 `parentLineIndex` 的树状拓扑关系。
      - 执行栈顺序：引擎在每帧渲染前，必须根据依赖关系对所有判定线进行**拓扑排序**，自顶向下计算矩阵变换。子线的全局矩阵 $M_{child} = M_{parent} \times T_{local}$。

 **父线允许嵌套**。编译器在编译时展平继承链。

 **BPM 阶跃**: `bpmTimeline` 同样支持阶跃语法——同一 beat 处连续两条目即表示瞬时跳变，规则与 `masterTimeline` 一致。

 #### 5.5.1.1 RPE 预定义扩展属性

 FCS 转谱器在导入 RPE 谱面时，会将以下 RPE 独有的判定线字段作为**预定义扩展键**（以 `rpe` 前缀标识）写入 `line` 块。这些键在 FCS 引擎中**完全无语义**（no-op），仅供转谱器在 FCS → RPE 方向上**无损还原**原始 RPE 判定线配置。

 | 扩展键 | 类型 | 默认值 | 说明 | RPE 原始字段 |
 |--------|------|--------|------|-------------|
 | `rpeGroup` | `int` | `0` | 判定线所属组索引 | `Group` |
 | `rpeIsCover` | `int` | `1` | 是否遮罩 (1=遮罩, 0=不遮罩) | `isCover` |
 | `rpeBpmFactor` | `float` | `1.0` | BPM 因子 (实际 BPM = nowBpm / factor) | `bpmfactor` |
 | `rpeAttachUi` | `string` | — | UI 绑定标识 | `attachUI` |
 | `rpeIsGif` | `bool` | `false` | 纹理是否为 GIF | `isGif` |
 | `rpeRotateWithFather` | `bool` | `true` | 子线是否继承父线旋转角 | `rotateWithFather` |

 **Note**: `textureAnchor` 在 FCS 中已有 `textureAnchor: (0.5, 0.5)` 等效字段，不设扩展键。`father` / `zOrder` 在 FCS 中已有 `parent` / `zOrder` 等效字段。

 ##### 5.5.1.2 RPE Extended 事件层预定义扩展属性

 RPE 的第五事件层（`extended`）包含判定线的特殊事件（故事板），其中部分属性在 FCS 中有等效机制，部分为 RPE 独有。FCS 转谱器按以下规则处理：

 | RPE Extended 事件 | FCS 等效 | 转换策略 |
 |-------------------|----------|----------|
 | `colorEvents` | `color: #RRGGBBAA` (静态) | RPE→FCS: 保留为 `rpeExtendedColorEvents` 扩展键。FCS→RPE: 如 FCS motion 中有动态 alpha → 采样烘焙为 colorEvents；否则使用 line 静态 `color` 或扩展键回退 |
 | `scaleXEvents` | `motion.layer.scaleX` | RPE→FCS: 保留为 `rpeExtendedScaleXEvents` 扩展键。FCS→RPE: FCS 优先，见 §5.6.4.3 |
 | `scaleYEvents` | `motion.layer.scaleY` | 同上 |
 | `textEvents` | 无等效 | RPE→FCS: 保留为 `rpeExtendedTextEvents` 扩展键。FCS→RPE: 扩展键回退 (FCS 无文字事件) |
 | `inclineEvents` | 无等效 (RPE 已弃用) | 作为 `rpeExtendedInclineEvents` 原样保留 |
 | `gifEvents` | 无等效 | 作为 `rpeExtendedGifEvents` 原样保留 |

 ##### 5.5.1.3 Event 公共扩展字段

 RPE 事件公有的 `linkgroup` 字段在 FCS 中无语义，作为 `linkgroup` 扩展键直接保留在事件的区间语法中。例如：

 ```
 positionX {
     [0.0b => 16.0b]: easeOutCubic(b, 0.0b, 16.0b, 0px, 200px, 0.0, 1.0);
     linkgroup: 0;
 }
 ```

 #### 5.5.2 `motion` 块与 `layer`

在进行多图层混合渲染时，各 `layer` 对应属性值按以下规则叠加：
 - **加性叠加**：`positionX`, `positionY`, `rotation` 以及 **`speed`** 采用相加计算。
 - **乘性叠加：`scaleX`, `scaleY`, `alpha` 采用相乘计算。

* **数学保证**：
  由于 `speed` 采用加性叠加，判定线的综合流速为 $speed_{total}(b) = \sum speed_i(b)$。其位置积分函数同样满足加性展开：
  $$floorPosition_{total}(b) = \sum \int_{0}^{b} speed_i(u) \, du$$

 ```
 line LDemo {
     bpmTimeline { 0.0b -> 120.0; }
 
     motion {
         layer {
             positionX {
                 [0.0b => 16.0b]: easeOutCubic(b, 0.0b, 16.0b, 0px, 200px, 0.0, 1.0);
                 [16.0b => 999.0b]: 200px;
             }
             positionY { /* 默认 0px */ }
             rotation {
                 [0.0b => 8.0b]: easeInSine(b, 0.0b, 8.0b, 0deg, 45deg, 0.0, 1.0);
             }
         }
         layer {
             rotation {
                 [16.0b => 32.0b]: easeOutElastic(b, 16.0b, 32.0b, 0deg, 360deg, 0.0, 1.0);
             }
             alpha {
                 [0.0b => 4.0b]: easeInQuad(b, 0.0b, 4.0b, 0.0, 1.0, 0.0, 1.0);
             }
         }
     }
 }
 ```

 **`layer` 内支持的子块**:

 | 子块 | 类型 | 默认值 | 说明 |
 |------|------|--------|------|
 | `positionX` | 长度单位 | `0px` | 线 X 位移 |
 | `positionY` | 长度单位 | `0px` | 线 Y 位移 |
 | `rotation` | 角度单位 | `0deg` | 线旋转角 |
 | `alpha` | 无量纲 | `1.0` | 线不透明度 (0=全透明, 1=不透明) |
 | `scaleX` | 无量纲 | `1.0` | 线 X 缩放 |
 | `scaleY` | 无量纲 | `1.0` | 线 Y 缩放 |
 | `speed` | 无量纲 | `1.0` | Note 下落速度倍率 |

 每个子块内使用**区间语法**:
 ```
 section := "[" startBeat "=>" endBeat "]" ":" expression ";"
          | "[" startBeat "=>" endBeat ")" ":" expression ";"
 ```
 - `[a => b]`: 闭区间。
 - `[a => b)`: 左闭右开，b 时刻回退到前一个区间。

#### 5.5.3 `speed` 属性与相关算法

任意 Note 在局部节拍 $b$ 处相对判定线中心的纵向逻辑距离 $Y(b)$ 定义为：

$$Y(b) = (floorPosition(b_{\text{note}}) - floorPosition(b)) \times \text{基准流速} \times note.speed$$

其中 $floorPosition(b)$ 是关于节拍 $b$ 的绝对连续函数（$C^0$ 连续），定义为流速倍率函数 $speed(u)$ 对节拍 $u$ 的黎曼积分：

$$floorPosition(b) = \int_{0}^{b} speed(u) \, du$$

为在各种复杂的数学表达式下保证运行时的高确定性与 $O(1)$ 或 $O(\log N)$ 求值效率，规定采用三级渐进式积分架构进行求值降级分流：

##### Tier 1: 编译器符号代数积分（编译期 AOT）
*   **适用条件**：`speed` 的 `Kind=2 Expr` 仅包含自变量 $b$ 且在数学上存在初等解析原函数（如多项式、三角函数、指数函数组合）。
*   **处理规范**：AOT 编译器在编译期对表达式进行符号积分推导。原属性在 `.fcbc` 中被解耦并降级，其位置积分求值退化为对原函数表达式的直接求值。
*   **性能**：$O(1)$ 复杂度

##### Tier 2: 编译期数值积分烘焙（编译期 AOT）
*   **适用条件**：`speed` 的 `Kind=2 Expr` 仅包含自变量 $b$，但在数学上无法求出解析原函数。
*   **处理规范**：
    1. AOT 编译器在编译期以固定步长 $\Delta b = 0.001\text{b}$，使用RK4对该表达式进行高精度预积分。
    2. 积分结果将被拟合为分段贝塞尔曲线，或高密度查找表（LUT），并作为 `Kind=1 Easing` 关键帧表写入 `.fcbc`。
    3. 运行时求值 $floorPosition(b)$ 时，引擎对该 LUT 进行 $O(\log N)$ 二分查找，并进行三次 Hermite 样条插值。
*   **性能**：运行时 $O(\log N)$ 复杂度。

##### Tier 3: 带检查点的逻辑帧物理积分（运行时）
*   **适用条件**：`speed` 表达式引入了运行时动态因变量，无法预计算。
*   **处理规范**：
    
    1. 引擎禁止在渲染循环中进行积分累加，必须在受音频时钟驱动的固定逻辑更新循环（TICK_RATE = 120Hz）中，以恒定时间步长 $\Delta t$ 运行欧拉积分：$$F(t + \Delta t) = F(t) + speed(t) \times \Delta t$$
    
    2. 为防止因非线性进度跳转（Seeking）导致计算复杂度退化为 $O(N)$，引擎在运行时会以固定节拍间隔（默认 $4.0\text{b}$）在内存中自动保存积分快照。
    
    3. 当发生 Seeking 行为跳转到目标节拍 $b_{\text{target}}$ 时，引擎二分查找最近的、低于该节拍的检查点，恢复其 $floorPosition$ 状态，并仅对剩余的极小区间（$\le 4.0\text{b}$）进行快速迭代累加，从而在 $O(1)$ 时间内完成状态恢复。

 - speed 的区间之间**不允许有空隙**——编译时自动填充。
---

 ### 5.6 Note 定义

 Note 定义在 `notes { }` 块内。

 #### 5.6.1 Note 种类 (Kind)

 | Kind | 标识符 | 判定行为 | 渲染行为 |
 |------|--------|----------|----------|
 | Tap | `tap` | 点击瞬间判定 | 正常渲染 |
 | Hold | `hold` | 需持续按住，松开时判定 | 正常渲染 |
 | Flick | `flick` | 滑动经过判定 | 正常渲染 |
 | Drag | `drag` | 经过即判定，无方向限制 | 正常渲染 |
 | Fake | `fake` | 无判定、无打击特效、无音效、不计分、不计物量 | 见下方 |

 **`kind: fake` 与 `fake: true` 的语义**:

| 属性组合 | 渲染行为 | 判定行为 |
|----------|----------|----------|
| `kind: fake` | 引擎在渲染管线中**完全跳过**该音符的绘制（等同于完全透明/不可见）。其底层 `alpha` 等属性值保持不变，允许将 `kind: fake` 作为隐形的轨道控制器或位置锚点，并正常读取其属性 | 无判定、无音效、不计分、不计物量 |
| `kind: tap` (或 hold/flick/drag) + `fake: true` | 引擎**正常渲染**该音符，其外观、颜色、不透明度完全遵循其原本 `kind` 和 `alpha` 等属性 | 屏蔽判定逻辑。玩家点击不产生任何 Hit/Miss 判定，不计入 Combo 和总物量，无打击音效、无打击特效 |

 #### 5.6.2 Note 必需属性

 | 属性 | 类型 | 说明 |
 |------|------|------|
 | `kind` | `ident` | `tap \| hold \| flick \| drag \| fake` |
 | `time` | 时间单位 | 打击时间。对 Hold 为开始时间 |
 | `positionX` | 长度单位 | X 轴位置 |

 #### 5.6.3 Note 可选属性 (含默认值)

 | 属性 | 类型 | 默认值 | 说明 |
 |------|------|--------|------|
 | `above` | `bool` | `true` | `true`=正面下落，`false`=背面下落 |
 | `endTime` | 时间单位 | = `time` | **仅 Hold**。Hold 的结束时间 (绝对时间) |
 | `speed` | 无量纲 | `1.0` | 流速倍率 (Hold 头尾使用相同倍率) |
 | `scaleX` | 无量纲 | `1.0` | 横向缩放倍率 |
 | `scaleY` | 无量纲 | `1.0` | 纵向缩放倍率 |
 | `xOffset` | 长度单位 | `0px` | 横向偏移量，叠加在 positionX 之上 |
 | `yOffset` | 长度单位 | `0px` | 纵向偏移量 (正=向上)，同时偏移打击特效位置 |
 | `alpha` | 无量纲 | `1.0` | 不透明度 (0=全透明, 1=不透明) |
 | `fake` | `bool` | `false` | 是否为可见诱导音符 (见 §5.6.1 语义表) |
 | `color` | `color` | `#FFFFFFFF` | 音符颜色 (顶点颜色乘法) |
 | `judgeShape` | `JudgeShape` | `infiniteY(1.0)` | 判定区域形状 (见 §5.6.6) |

#### 5.6.3.1 Hold 音符的动态环境变量 `d` 评估规范

当在 Note 的动态表达式（如 `alpha`, `color`）中引用环境变量 `d`（像素距离）时，针对 Hold类型音符，遵循以下渲染评估规范：

1. 对于 Hold 身体网格上的任意顶点 $V$，其对应的自变量 $d_V$ 定义为该顶点当前在屏幕空间中距离判定线的实际像素距离。
   $$\alpha_V = f(d_V)$$
   在渲染 Hold 身体时，须在顶点着色器中对每个顶点独立评估表达式，或在 CPU 端分段计算后填充顶点色。
   
2. 对于 `scaleX`, `scaleY`, `xOffset`, `yOffset` 等几何属性，Hold 身体所有顶点**统一使用 Hold 头部（即打击点）的距离 $d_{head}$** 进行评估。

 #### 5.6.4 开放扩展

 Note 允许任意额外属性，类型必须为 `string | int | float | bool`。编译器对未识别的属性发出 `Warning`，但保留在 `.fcs` 源中。**`.fcbc` 字节码会剥离所有未识别的扩展属性。**

 ##### 5.6.4.1 RPE 预定义扩展属性

 FCS 转谱器在导入 RPE 谱面时，会将以下 RPE 独有的 Note 字段作为**预定义扩展键**（以 `rpe` 前缀标识）写入 Note 定义。这些键在 FCS 引擎中**完全无语义**（no-op），仅供转谱器在 FCS → RPE 方向上**无损还原**原始 RPE Note 配置。

 | 扩展键 | 类型 | 默认值 | 说明 | RPE 原始字段 |
 |--------|------|--------|------|-------------|
 | `rpeIsFake` | `int` | `0` | 是否为可见诱导音符 (1=是, 0=否)。**语义不同于 FCS `kind: fake`** — 见下文 §5.6.4.2 | `isFake` |
 | `rpeSize` | `float` | `1.0` | 音符宽度倍率 | `size` |
 | `rpeVisibleTime` | `float` | `999999.0` | 音符可见时间，单位秒 | `visibleTime` |
 | `rpeHitsound` | `string` | — | 自定义打击音文件相对路径 | `hitsound` |
 | `rpeJudgeArea` | `float` | `1.0` | 判定区域宽度倍率 (RPE 170+) | `judgeArea` |
 | `rpeTint` | `int[3]` | `[255,255,255]` | 音符顶点颜色乘法 `[R,G,B]`，范围 0–255 | `tint` / `color` |
 | `rpeTintHitEffects` | `int[3]` | — | 打击特效顶点颜色乘法 `[R,G,B]` | `tintHitEffects` |
 | `rpeAlphaControl` | `object[]` | 默认 | alpha 距离关键帧表 | `alphaControl` |
 | `rpePosControl` | `object[]` | 默认 | positionX 倍率距离关键帧表 | `posControl` |
 | `rpeSizeControl` | `object[]` | 默认 | 大小距离关键帧表 | `sizeControl` |
 | `rpeYControl` | `object[]` | 默认 | Y 偏移距离关键帧表 | `yControl` |
 | `rpeSkewControl` | `object[]` | 默认 | 倾斜距离关键帧表 | `skewControl` |

 ##### 5.6.4.2 `kind: fake` 与 `rpeIsFake` 的语义区分

 | 属性 | 渲染行为 | 判定行为 | 可见性 |
 |------|----------|----------|--------|
 | FCS `kind: fake` | 引擎完全跳过绘制 | 无判定、无音效、不计分 | **不可见** |
 | RPE `isFake: 1` | 引擎正常渲染 | 无判定、无音效、不计分 | **可见** |
 | FCS `fake: true` | 引擎正常渲染 | 无判定、无音效、不计分 | **可见** |

 **转换规则**:
 - FCS `kind: fake` → RPE: **丢弃该 Note**。前提是转谱器已事先解析原型继承链，将被该 fake note 所影响的子 Note 属性在编译期展平。
 - FCS `fake: true` → RPE: 映射为 `isFake: 1`。
 - RPE `isFake: 1` → FCS: 保留为 `rpeIsFake: 1` 扩展键；同时设置 `fake: true`。
 - RPE 无 `kind: fake` 等效概念 → 转回 RPE 时不做特殊处理。

 ##### 5.6.4.3 扩展属性冲突解决规则

 当同一 Note 同时存在 FCS 原生属性与 RPE 预定义扩展键时，转谱器按以下优先级处理：

 1. **FCS 原生优先 (Native Wins)**：若 FCS 原生属性被显式设置（非默认值），转谱器 MUST 使用 FCS 原生属性的求值结果。对应 RPE 扩展键被忽略，同时 MUST 发出 Warning: `"RPE extension key '<key>' overridden by native FCS property '<prop>'"`。
 2. **扩展键回退 (Extension Fallback)**：若 FCS 原生属性为默认值（谱师未显式设置），转谱器 SHOULD 使用 RPE 扩展键中的值。
 3. **保留被覆盖的键**：转谱器 SHOULD 在输出中保留被覆盖的扩展键（不删除），以便后续审查能发现冲突。

 **语义不兼容的特殊处理**:
 - `rpePosControl`（倍率）与 `positionX`（绝对值）语义不同。转谱器在 FCS 优先规则下，忽略 `rpePosControl`，仅使用 `positionX` 的求值结果。

 #### 5.6.5 原型继承 (Prototype Inheritance)

 编译器在编译时自动完成属性展平——**零代价抽象**。

 ```
 notes {
     tap templateGhost {
         above: true;
         speed: 1.0;
         alpha: easeInSine(d, 0px, 200px, 0.0, 1.0, 0.0, 1.0);
     }
 
     tap note1 : templateGhost {
         time: 4.0b;
         positionX: -150px;
     }
 
     hold holdDemo {
         time: 8.0b;
         endTime: 12.0b;
         positionX: 0px;
         speed: 1.5;
     }
 }
 ```

 **规则**:
 - `kind name : parentName { ... }` 语法继承父模板的全部属性。
 - 子实例可以覆写任意属性。
 - 只有有 `time` 属性的 Note 实例才被渲染。
 - 模板嵌套继承允许。

 #### 5.6.6 `judgeShape` — 判定区域形状

 判定区域以 note 中心为原点，在判定线本地坐标系 (判定线为 X 轴) 中定义。支持三种形状:

 | 语法 | 参数 | 说明 |
 |------|------|------|
 | `infiniteY(w)` | `w`: 无量纲 | 沿 Y 轴向两端无限延伸的带状区域，宽度为 `w × 原始判定宽度` |
 | `rectangle(w, h)` | `w`, `h`: 长度单位 | 以 note 中心为几何中心的矩形碰撞盒，`w` 为沿判定线方向的宽度，`h` 为垂直于判定线方向的高度 |
 | `circle(r)` | `r`: 长度单位 | 以 note 中心为圆心、`r` 为半径的圆形判定区域 |

 示例:
 ```
 judgeShape: infiniteY(1.5);
 judgeShape: rectangle(120px, 80px);
 judgeShape: circle(75px);
 ```

---

判定区域的碰撞检测是在判定线的本地坐标系中进行的。

 ### 5.7 `shaders` 块 (可选)

 ```
 shaders {
     shader glitchEffect {
         vertex: "shd/glitch.vert";
         fragment: "shd/glitch.frag";
         binds {
             uIntensity: easeOutQuad(b, 0.0b, 4.0b, 1.0, 0.0, 0.0, 1.0);
             uTime: s;
         }
     }
 }
 ```

 | 子字段 | 类型 | 必需 | 说明 |
 |--------|------|------|------|
 | `vertex` | `string` | 是 | 顶点着色器路径 |
 | `fragment` | `string` | 是 | 片段着色器路径 |
 | `binds` | 块 | 否 | uniform 绑定至表达式 |

---

 ## 6. 属性类型定义总表

 ### 6.1 基本类型

 | 类型 | 语法 | 示例 | 说明 |
 |------|------|------|------|
 | `int` | 整数 | `42` | 64-bit 有符号整数 |
 | `float` | 浮点数 | `3.14`, `1e-3` | IEEE 754 Float64 |
 | `bool` | `true` / `false` | `true` | 布尔值 |
 | `string` | `"..."` | `"hello"` | UTF-8 字符串 |
 | `color` | `#RRGGBB[AA]` | `#FF0000` | HEX sRGB 颜色 |
 | `time` | `float + unit` | `120ms`, `4.0b` | 时间 (ms/s/min/b) |
 | `length` | `float + unit` | `200px`, `0.5vw` | 长度 (px/vw/vh) |
 | `angle` | `float + unit` | `45deg`, `0.785rad` | 角度 (deg/rad) |
 | `dimensionless` | `float` | `1.0` | 无量纲倍率 |
 | `JudgeShape` | 函数调用 | `circle(75px)` | 判定形状 |

 ### 6.2 复合类型

 | 类型 | 语法 | 示例 |
 |------|------|------|
 | `T[]` | `[elem, elem, ...]` | `["A", "B"]` |
 | `(T, T)` | `(val, val)` | `(0.5, 0.5)` |
---

 ## 7. 缓存函数 (Easing)

 ### 7.1 签名格式

 ```
 ease<Dir><Type>(t, t0, t1, v0, v1, clampLeft, clampRight)
 ```

 | 参数 | 类型 | 说明 |
 |------|------|------|
 | `t` | Float64 | 当前自变量 (通常为 `b` 或 `s`) |
 | `t0` | 时间单位 | 区间起始时间 |
 | `t1` | 时间单位 | 区间结束时间 |
 | `v0` | 与 v1 同类型 | 起始值 |
 | `v1` | 与 v0 同类型 | 结束值 |
 | `clampLeft` | Float64 (0~1) | 缓动左边界。默认 `0.0` |
 | `clampRight` | Float64 (0~1) | 缓动右边界。默认 `1.0` |

 **Clamp 行为**:
- 如果 `t < t0`：返回 `v0`。
- 如果 `t > t1`：返回 `v1`。
- 如果 `t0 ≤ t ≤ t1`：令 `p = (t − t0) / (t1 − t0)`，然后 `p = clampLeft + p × (clampRight − clampLeft)`，再 `p = clamp(p, 0.0, 1.0)`，最后 `result = f(p) × (v1 − v0) + v0`。

### 7.2 29种缓动函数完整定义

令 `t ∈ [0, 1]` 表示经过钳制边界调整后的归一化进度。

| ID | 函数名 | 数学定义 f(t) |
|----|---------------|------------------------------|
| 1 | `easeLinear` | t |
| 2 | `easeOutSine` | sin(t × π / 2) |
| 3 | `easeInSine` | 1 − cos(t × π / 2) |
| 4 | `easeOutQuad` | 1 − (1 − t)² |
| 5 | `easeInQuad` | t² |
| 6 | `easeInOutSine` | −(cos(π t) − 1) / 2 |
| 7 | `easeInOutQuad` | t < 0.5 时为 2t²；t ≥ 0.5 时为 1 − (−2t + 2)² / 2 |
| 8 | `easeOutCubic` | 1 − (1 − t)³ |
| 9 | `easeInCubic` | t³ |
| 10 | `easeOutQuart` | 1 − (1 − t)⁴ |
| 11 | `easeInQuart` | t⁴ |
| 12 | `easeInOutCubic` | t < 0.5 时为 4t³；t ≥ 0.5 时为 1 − (−2t + 2)³ / 2 |
| 13 | `easeInOutQuart` | t < 0.5 时为 8t⁴；t ≥ 0.5 时为 1 − (−2t + 2)⁴ / 2 |
| 14 | `easeOutQuint` | 1 − (1 − t)⁵ |
| 15 | `easeInQuint` | t⁵ |
| 16 | `easeOutExpo` | t = 1 时为 1，否则为 1 − 2^(−10t) |
| 17 | `easeInExpo` | t = 0 时为 0，否则为 2^(10t − 10) |
| 18 | `easeOutCirc` | √(1 − (t − 1)²) |
| 19 | `easeInCirc` | 1 − √(1 − t²) |
| 20 | `easeOutBack` | 1 + c₃(t − 1)³ + c₁(t − 1)²，其中 c₁ = 1.70158，c₃ = c₁ + 1 |
| 21 | `easeInBack` | c₃ t³ − c₁ t²，其中 c₁ = 1.70158，c₃ = c₁ + 1 |
| 22 | `easeInOutCirc` | t < 0.5 时为 (1 − √(1 − (2t)²)) / 2；t ≥ 0.5 时为 (√(1 − (−2t + 2)²) + 1) / 2 |
| 23 | `easeInOutBack` | t < 0.5 时为 ((2t)²((c₁ + 1)2t − c₁)) / 2；t ≥ 0.5 时为 ((2t − 2)²((c₁ + 1)(2t − 2) + c₁) + 2) / 2，其中 c₁ = 2.5949095 |
| 24 | `easeOutElastic` | t = 0 时为 0；t = 1 时为 1；否则为 2^(−10t) sin((10t − 0.75) × 2π/3) + 1 |
| 25 | `easeInElastic` | t = 0 时为 0；t = 1 时为 1；否则为 −2^(10t − 10) sin((10t − 10.75) × 2π/3) |
| 26 | `easeOutBounce` | n₁ = 7.5625，d₁ = 2.75；t < 1/d₁ 时 f(t) = n₁ t²；t < 2/d₁ 时为 n₁(t − 1.5/d₁)² + 0.75；t < 2.5/d₁ 时为 n₁(t − 2.25/d₁)² + 0.9375；否则为 n₁(t − 2.625/d₁)² + 0.984375 |
| 27 | `easeInBounce` | 1 − easeOutBounce(1 − t) |
| 28 | `easeInOutBounce` | t < 0.5 时为 (1 − easeOutBounce(1 − 2t)) / 2；t ≥ 0.5 时为 (1 + easeOutBounce(2t − 1)) / 2 |
| 29 | `easeInOutElastic` | t = 0 时为 0；t = 1 时为 1；t < 0.5 时为 (−2^(20t−10) sin((20t−11.125) × 2π/4.5)) / 2；t ≥ 0.5 时为 (2^(−20t+10) sin((20t−11.125) × 2π/4.5)) / 2 + 1 |

### 7.3 贝塞尔曲线缓动

```
easeBezier(t, t0, t1, v0, v1, cx1, cy1, cx2, cy2, clampLeft, clampRight)
```

| 额外参数 | 允许范围 | 说明 |
|-----------------|---------------|-------------|
| `cx1, cy1` | [0, 1] | 第一个控制点 |
| `cx2, cy2` | [0, 1] | 第二个控制点 |

使用三次贝塞尔曲线进行进度映射，语义上等同于 CSS 的 `cubic-bezier()`

---

 ## 8. 异常处理规范

 ### 8.1 编译时错误 (Fatal — 终止编译)

 | 错误码 | 场景 | 说明 |
 |--------|------|------|
 | `E001` | 未闭合的字符串/块 | 语法错误 |
 | `E002` | 单引号字符串 `'...'` | 仅允许双引号 `"..."` |
 | `E003` | 类型不匹配 | 期望长度的字段赋为无量纲值；或反之 |
 | `E004` | 未知单位 | 使用了未定义的单位标识符 |
 | `E005` | 未定义的模板引用 | `note x : nonexistentTemplate` |
 | `E006` | 循环模板依赖 | A 调用 B, B 调用 A |
 | `E007` | 模板内含循环/递归 | 模板必须 O(1) 求值 |
 | `E008` | `masterTimeline` 非零起始 | 第一项必须从 `0.0b` 开始 |
 | `E009` | `masterTimeline` BPM ≤ 0 | BPM 必须为正数 |
 | `E010` | `bpmTimeline` 非零起始 | 每条线的 BPM 轴必须从 `0.0b` 开始 |
 | `E011` | `bpmTimeline` BPM ≤ 0 | BPM 必须为正数 |
 | `E012` | Beat 分母为 0 | Beat 字面量 `[int, num, 0]` 不允许 |
 | `E013` | 父线引用不存在的线 | `parent: nonexistentLine` |
 | `E014` | 必需块缺失 | 缺少 `meta`, `masterTimeline`, 或 `judgelines` |
 | `E015` | `name`, `artists`, `charters`, `offset`, `version` 缺失 | meta 必需字段缺失 |
 | `E016` | `judgeShape` 语法错误 | 未知形状或参数类型错误 |
 | `E017` | Hold 结束时间异常 | `endTime < time` 导致时间轴倒流 |

 ### 8.2 编译时警告 (Warning — 不阻止编译)

 | 警告码 | 场景 | 说明 |
 |--------|------|------|
 | `W001` | motion 区间重叠 (同一 layer 同一属性) | **后声明的区间在重叠部分优先** |
 | `W002` | motion 区间之间存在空隙 (非 speed) | 间隙期间该属性使用默认值 |
 | `W003` | 未识别的 meta 自定义键 | 保留在 `.fcs` 中 |
 | `W004` | 未识别的 Note 扩展属性 | 保留在 `.fcs` 中 |
 | `W005` | 模板定义但未被使用 | 不影响编译结果 |
 | `W006` | line 无 notes 且无 motion | 可能为有意为之 |
 | `W007` | 事件 startTime > endTime | 运行时自动 swap |

 ### 8.3 运行时异常处理

 | 场景 | 行为 |
 |------|------|
 | 除零 | 该实体在当前帧被**视锥剔除**（静默隐藏），不崩溃 |
 | 表达式产生 `NaN` 或 `±Inf` | 同上，该帧剔除 |
 | 纹理文件不存在 | 替换为**紫黑棋盘格** fallback 纹理；控制台 Warning |
 | `father` 索引越界 (`.fcbc` 中) | 视为无父线 |
 | Beat 超出 BPM 轴范围 | 使用最后一个 BPM 值外推 |
 | 时间戳超出所有事件范围 | position/rotation/alpha/scale: 使用最近边界值；speed: 使用最后一个 speed 值 |

 ### 8.4 时间轴冲突解决规则

 1. 同一 layer 同一属性时间重叠 → **后声明优先**。
 2. 不同 layer → 加性叠加，不构成冲突。
 3. speed 区间 → 编译时自动填充空隙；重叠时 W001 + 后声明优先。
 4. Note 原型链 → 子覆写父，最后覆写优先。

 ### 8.5 建议验证规则 (非强制)

 - 判定线条数建议 ≤ 256。
 - 单线 Note 数量建议 ≤ 10000。
 - `offset` 建议范围: `[−5000ms, 5000ms]`。
---

 ## 9. 字节码规范 (.fcbc)

 ### 9.1 概述

 `.fcbc` (FCS Bytecode) 是 FCS 谱面的 AOT 编译产物，为引擎运行时唯一读取的二进制格式。

 #### 9.1.1 设计原则

 | 原则 | 说明 |
 |------|------|
 | 零字符串解析 | 所有标识符、路径编译为字符串表偏移量；引擎不做文本解析 |
 | 双轨求值 | 常量属性走快路径 (Fast-Path)，动态表达式走 VM 求值 (VM-Path) |
 | 最小体积 | 仅保留引擎运行时必需属性；扩展属性全部剥离 |
 | 预计算 | BPM LUT、原型展平、模板内联均在编译时完成 |

 #### 9.1.2 顶层结构

 ```
 FcbcFile := Header StringTable ConstantPool MetaSection
             MasterTimelineSection LineSection
             ExpressionSection? ShaderSection?
 ```

 ### 9.2 文件头 (File Header)

 | 偏移 | 大小 | 类型 | 字段 | 说明 |
 |--------|------|------|-------|-------------|
 | 0 | 4 | u8[4] | magic | `0x46 0x43 0x53 0x42` ("FCSB") |
 | 4 | 4 | u32 LE | version | 字节码格式版本 (= 1) |
 | 8 | 4 | u32 LE | flags | bit 0: 含着色器; bit 1: 含表达式 |
 | 12 | 4 | u32 LE | stringTableOffset | 字符串表的字节偏移 |
 | 16 | 4 | u32 LE | stringTableSize | 字符串表的字节大小 |
 | 20 | 4 | u32 LE | constantPoolOffset | 常量池的字节偏移 |
 | 24 | 4 | u32 LE | constantPoolSize | 常量池的字节大小 |

 文件头总计: 28 字节。

### 9.3 字符串表 (String Table)

以空字符结尾的 UTF-8 字符串的连续序列。通过从本节起始位置的 `u32` 字节偏移量进行引用。

### 9.4 常量池 (Constant Pool)

IEEE 754 Float64 值的连续序列（每个 8 字节）。通过 `u16` 索引进行引用。编译器会对相同的常量进行去重。

### 9.5 属性描述符 (Property Descriptor)

**核心概念**：在 `.fcbc` 中，每个动态属性（位置、缩放、透明度、速度等）均存储为属性描述符而非静态的 `f32`，实现了双路径求值模型。

```
PropertyDescriptor := Kind: u8  Reserved: u24  Payload: u32
```

总计：8 字节。

| Kind 值 | 名称 | Payload 含义 | 性能 |
|------------|------|----------------------|-------------|
| 0 | Const | Payload 即为 IEEE 754 Float32 值（直接内联） | 极速：零解引用 |
| 1 | Easing | Payload 为所属实体的关键帧表中的 `u32` 字节偏移量 | 快速：直接查找 |
| 2 | Expr | Payload 为表达式段中的 `u32` 字节偏移量，指向 Opcode 流 | 灵活：VM 求值 |

### 9.6 元信息段 (Meta Section)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 (stOff) | name | 着色器名称 | 线名称 | 谱面名称 |
| 4 | 4 | u32 | artistCount | 曲师数量 |
| 8 | 4 × N | u32[] | artistStOff | 曲师名称的字符串表偏移 |
| ... | 4 | u32 | charterCount | 谱师数量 |
| ... | 4 × N | u32[] | charterStOff | 谱师名称的字符串表偏移 |
| ... | 8 | f64 | offset | 音乐偏移（秒，有符号） |

自定义 `.fcs` 元字段（`illustration`、`level` 等）**不**存储在 `.fcbc` 中。

### 9.7 主时间轴段 (Master Timeline Section)

编译时由 BPM 积分预计算 LUT。运行时：O(log N) 二分查找。

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 | entryCount | 条目数量 |
| 4 | 24 × N | struct[] | entries | 条目数组 |

**条目结构**（24 字节）：

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 8 | f64 | beat | 节拍值 |
| 8 | 8 | f64 | accumulatedSec | 累积秒数 |
| 16 | 8 | f64 | bpm | BPM 值 |

阶跃条目（同一 beat、连续两行）编码瞬时 BPM 跳变。

### 9.8 判定线段 (Line Section)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 | lineCount | 判定线数量 |
| 4 | 可变 | Line[] | lines | 判定线数组 |

#### 9.8.1 判定线头部 (Line Header)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 (stOff) | name | 着色器名称 | 线名称 | 谱面名称 |
| 4 | 4 | u32 (stOff) | texture | 纹理路径 |
| 8 | 8 | f32, f32 | textureAnchor | 纹理锚点 |
| 16 | 4 | i32 | zOrder | 图层顺序 |
| 20 | 4 | u8×4 | color | 颜色 (RGBA) |
| 24 | 4 | i32 | parentLineIndex | 父线索引（−1 = 无） |
| 28 | 1 | u8 | inheritFlags | 继承标志（bit 0=位移，1=旋转，2=缩放，3=透明度） |
| 29 | 3 | u24 | reserved | 保留 |
| 32 | 4 | u32 | bpmLutOffset | BPM LUT 偏移 |
| 36 | 4 | u32 | bpmLutEntryCount | BPM LUT 条目数 |
| 40 | 4 | u32 | motionLayerCount | 运动层数量 |
| 44 | 4 | u32 | noteCount | 音符数量 |

头部总计：48 字节。

#### 9.8.2 判定线 BPM 查找表 (Line BPM LUT)

结构与主时间轴 LUT（§9.7）相同。阶跃条目编码瞬时跳变。

#### 9.8.3 运动关键帧表 (Motion Keyframe Table)

每个运动属性（positionX / positionY / rotation / alpha / scaleX / scaleY / speed）编码为独立的关键帧表。

**Keyframe Table 头部**：

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 | keyframeCount | 关键帧数量 |
| 4 | 可变 | Keyframe[] | keyframes | 关键帧数组 |

 **Keyframe 结构**（40 字节，32-bit 对齐）：

 | Offset | Size | Type | Field | 说明 |
 |--------|------|------|-------|-------------|
 | 0 | 8 | f64 | timeBeat | 节拍时间 |
 | 8 | 1 | u8 | easingId | 缓动 ID（1–29，30 = bezier） |
 | 9 | 3 | u24 | reserved | 填充对齐（固定为 `0`） |
 | 12 | 4 | f32 | clampLeft | 缓动左边界 |
 | 16 | 4 | f32 | clampRight | 缓动右边界 |
 | 20 | 16 | f32[4] | bezierParams | Bezier 控制点（仅 easingId = 30 时有效） |
 | 36 | 4 | f32 | targetValue | 目标值 |

多通道属性（position = X+Y；scale = X+Y）使用两个独立的关键帧表。

##### 9.8.3.1 `speed` 属性的双轨二进制描述规范

由于 `speed` 属性在求值时具有双轨特征（需要同时获取瞬时流速 $v(b)$ 进行视觉拉伸，以及累加位置 $floorPosition(b)$ 进行坐标计算），因此在 `.fcbc` 中，`speed` 属性的 `PropertyDescriptor` 采用以下特殊双轨载荷映射：

| 字段 | 大小 | 类型 | 说明 |
| :--- | :--- | :--- | :--- |
| `Kind` | 1 字节 | `u8` | `0`: Const; `1`: LUT (Tier 2); `2`: Dynamic (Tier 1 & 3) |
| `Reserved` | 3 字节 | `u24` | 填充对齐 |
| `Payload` | 4 字节 | `u32` | 依据 `Kind` 的不同具有以下特化定义： |

*   **当 `Kind = 1` (LUT / Tier 2 降级时)**：
    *   `Payload` 指向关键帧表中的偏移量。该表存储 AOT 编译器预烘焙的分段样条参数，供引擎进行快速插值。
*   **当 `Kind = 2` (Dynamic / Tier 1 或 Tier 3 时)**：
    *   `Payload` 指向表达式段中的一个特化双轨表达式描述符：
        
        ```cpp
        struct DualExprDescriptor {
            u32 velocityExprOffset;  // 指向瞬时流速 v(b) 表达式字节码的偏移量
            u32 integralExprOffset;  // 指向累计积分 F(b) 表达式字节码的偏移量
        };
        ```
    *   若该属性被降级为 **Tier 3** 运行时积分，则其 `integralExprOffset` 固定填为 `0xFFFFFFFF`。引擎在解析此标识时，将自动激活运行时固定步长积分与 `Checkpoint` 状态机。

#### 9.8.4 音符编码 (Note Encoding)

| Offset | Size | Type | Field | 说明 |
|--------|------|------|-------|-------------|
| 0 | 1 | u8 | kind | 种类（0=Tap，1=Drag，2=Hold，3=Flick，4=Fake） |
| 1 | 1 | u8 | flags | 标志位（bit 0=正面，bit 1=诱导） |
| 2 | 1 | u8 | judgeShapeKind| 判定形状（0=infiniteY，1=rectangle，2=circle） |
| 3 | 5 | u8[5] | padding1 | 显式填充对齐（固定为 0） |
| 8 | 8 | f64 | timeBeat | 节拍时间（对齐至 8 字节边界） |
| 16 | 8 | f64 | endTimeBeat | 结束节拍（非 Hold 为 0） |
| 24 | 8 | PropertyDescriptor | positionX | X 轴位置 |
| 32 | 8 | PropertyDescriptor | speed | 流速倍率 |
| 40 | 8 | PropertyDescriptor | scaleX | 横向缩放 |
| 48 | 8 | PropertyDescriptor | scaleY | 纵向缩放 |
| 56 | 8 | PropertyDescriptor | xOffset | 横向偏移 |
| 64 | 8 | PropertyDescriptor | yOffset | 纵向偏移 |
| 72 | 8 | PropertyDescriptor | alpha | 不透明度 |
| 80 | 8 | PropertyDescriptor | judgeShapeParam1| 判定参数1 |
| 88 | 8 | PropertyDescriptor | judgeShapeParam2| 判定参数2 |
| 96 | 4 | u8×4 | color | 颜色 RGBA |
| 100 | 4 | u8[4] | padding2 | 尾部填充，使整体大小为 8 的倍数 |

**结构体总计：104 字节（固定，严格 8 字节对齐）**。

所有音符按 `timeBeat` 升序排列。扩展属性（例如 `customId`）**不**存储。

`judgeShapeKind = 1`（rectangle）使用两个参数槽；`judgeShapeKind = 0、2` 仅使用 param1。

### 9.9 表达式段 (Expression Section)

仅当 Flags bit 1 置位时存在。存储所有 Kind=2 (Expr) PropertyDescriptor 引用的 Opcode 流。

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 | exprCount | 表达式数量 |
| 4 | 可变 | Expression[] | expressions | 表达式数组 |

**表达式条目**：

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 | bytecodeSize | 字节码大小 |
| 4 | 可变 | u8[] | bytecode | Opcode 流 |

#### 9.9.1 VM 模型

Float64 栈机模型。每条 Opcode 占 1 字节，可选后接操作数。执行完毕后栈顶留下一个 f64 结果。

VM 运行时最大栈深度限制为 **256** 层。若执行中发生栈溢出，VM 抛出 `RuntimeWarning`，强行清空该求值上下文，该帧返回值强制设为 `0.0`，并执行视锥体剔除。

静态分析阶段，若检测到表达式内存在无法在 O(1) 内退出的跳转逻辑，编译器将抛出 `E007` 编译错误。

#### 9.9.2 栈操作

| Opcode | 助记符 | 操作数 | 栈效果 |
|--------|----------|----------|-------------|
| `0x00` | NOP | — | — |
| `0x01` | PUSH_CONST | u16 index | push constPool[index] |
| `0x02` | POP | — | pop 1 |
| `0x03` | DUP | — | push top |
| `0x04` | SWAP | — | swap top two |

#### 9.9.3 环境变量访问

| Opcode | 助记符 | 栈效果 |
|--------|----------|-------------|
| `0x10` | PUSH_VAR_S | push s (seconds) |
| `0x11` | PUSH_VAR_B | push b (beats) |
| `0x12` | PUSH_VAR_D | push d (pixel distance) |
| `0x13` | PUSH_VAR_T | push normalized interval progress (0–1) |

#### 9.9.4 算术运算（二元：弹 2 压 1；一元：弹 1 压 1）

| Opcode | 助记符 | 操作 |
|--------|----------|-----------|
| `0x20` | ADD | a + b |
| `0x21` | SUB | a − b |
| `0x22` | MUL | a × b |
| `0x23` | DIV | a / b |
| `0x24` | MOD | a % b |
| `0x25` | NEG | −a (unary) |
| `0x26` | POW | a^b |

#### 9.9.5 数学函数（一元：弹 1 压 1）

| Opcode | 助记符 | Opcode | 助记符 |
|--------|----------|--------|----------|
| `0x30` | SIN | `0x38` | EXP |
| `0x31` | COS | `0x39` | LN |
| `0x32` | TAN | `0x3A` | LOG2 |
| `0x33` | ASIN | `0x3B` | LOG10 |
| `0x34` | ACOS | `0x3C` | SQRT |
| `0x35` | ATAN | `0x3D` | FLOOR |
| `0x36` | ATAN2 | `0x3E` | CEIL |
| `0x37` | ABS | `0x3F` | ROUND |

#### 9.9.6 比较与分支

| Opcode | 助记符 | 操作数 | 栈效果 |
|--------|----------|----------|-------------|
| `0x50` | CMP_LT | — | pop 2，push a < b ? 1.0 : 0.0 |
| `0x51` | CMP_LE | — | pop 2，push a ≤ b ? 1.0 : 0.0 |
| `0x52` | CMP_GT | — | pop 2，push a > b ? 1.0 : 0.0 |
| `0x53` | CMP_GE | — | pop 2，push a ≥ b ? 1.0 : 0.0 |
| `0x54` | CMP_EQ | — | pop 2，push a = b ? 1.0 : 0.0 |
| `0x55` | CMP_NE | — | pop 2，push a ≠ b ? 1.0 : 0.0 |
| `0x56` | JMP_IF_FALSE | i16 offset | pop 1；若 = 0.0，则相对跳转 |
| `0x57` | JMP | i16 offset | 无条件相对跳转 |

#### 9.9.7 缓动调度

| Opcode | 助记符 | 操作数 | 栈效果 |
|--------|----------|----------|-------------|
| `0x60` | EASE | u8 easingId | pop 7 (t, t0, t1, v0, v1, cl, cr)；求值缓动；push 结果 |

VM 保存当前 opcode 位置，压入新栈帧，用 7 个参数求值缓动函数，然后将结果压入调用者的栈。缓动 ID 1–29 对应 §7.2；ID 30 调度 `easeBezier`。

以下是为 **FCS v4.0.0 规范** 定制编写的更新文档。其排版格式、术语命名（如 `lowerCamelCase`、`easingId`、`clampLeft` 等）以及严谨的规范语气，均与你提供的谱面格式规范上下文完全对齐。

你可以直接将此部分内容替换或插入到原规范的 **`#### 9.9.7 缓动调度`** 章节中。

---

#### 9.9.7 缓动调度

| Opcode | 助记符 | 操作数 | 栈效果 |
|--------|----------|----------|-------------|
| `0x60` | EASE | `u8 easingId` | 弹出 7 个参数（自右向左），求值缓动函数后将 `Float64` 结果压入栈顶 |

VM 执行 `EASE` 指令时，将挂起当前执行流，弹出栈顶的 7 个参数进行缓动计算，然后将结果压回。`easingId` 1–29 对应 [§7.2 缓动函数定义](#72-29种缓动函数完整定义)；`easingId = 30` 调度 `easeBezier`（[§7.3 贝塞尔曲线缓动](#73-贝塞尔曲线缓动)）。

##### 9.9.7.1 栈流协议

**1. 编译器参数求值与发射顺序**：
AOT 编译器在解析缓动表达式 `easeDirType(t, t0, t1, v0, v1, clampLeft, clampRight)` 时，**必须且只能**按照自左向右的顺序发射求值指令。

**2. 虚拟机 LIFO 弹出绑定顺序**：
由于 VM 栈的后进先出特性，在执行 `0x60 EASE` 指令时，VM 弹出参数的顺序与编译器求值顺序相反。VM 从栈顶依次弹出 7 个操作数，并将其严格按以下顺序绑定到缓动引擎的输入参数中：

| 弹栈次序 | 栈位置 | 对应变量 | 备注 |
| :---: | :--- | :--- | :--- |
| **1st Pop** | 栈顶 (TOS) | `clampRight` | 缓动右边界 |
| **2nd Pop** | 栈顶 − 1 | `clampLeft` | 缓动左边界 |
| **3rd Pop** | 栈顶 − 2 | `v1` | 目标结束值 |
| **4th Pop** | 栈顶 − 3 | `v0` | 初始起始值 |
| **5th Pop** | 栈顶 − 4 | `t1` | 区间结束时间 |
| **6th Pop** | 栈顶 − 5 | `t0` | 区间起始时间 |
| **7th Pop** | 栈顶 − 6 | `t` | 当前自变量（时间/空间变量） |

##### 9.9.7.2 运行时安全与栈下溢检查

1. **栈深校验**：
   在执行 `0x60 EASE` 指令之前，VM 验证当前栈帧中的元素数量。若当前栈深度 $< 7$，VM 立即中断求值流程。
2. **容错保护**：
   触发下溢保护后，该求值上下文被强制清空，该帧返回值设为 `0.0`，并对该属性所属实体执行视锥体剔除。

##### 9.9.7.3 编译与执行对照状态机示例

若 `.fcs` 源文件中包含如下属性声明：
```css
alpha: easeOutSine(b, 0.0b, 4.0b, 0.0, 1.0, 0.0, 1.0);
```

其编译产生的 `.fcbc` 字节码序列和 VM 栈状态演变符合下表：

| 步序 | Opcode | 栈状态 (左侧为栈底，右侧为栈顶 TOS) | 说明 |
| :--- | :--- | :--- | :--- |
| 0 | — | `[]` | 初始空栈 |
| 1 | `0x11` (PUSH_VAR_B) | `[ b ]` | 评估 `t`，压栈 |
| 2 | `0x01` (PUSH_CONST [0]) | `[ b, 0.0 ]` | 评估 `t0`，压栈 |
| 3 | `0x01` (PUSH_CONST [1]) | `[ b, 0.0, 4.0 ]` | 评估 `t1`，压栈 |
| 4 | `0x01` (PUSH_CONST [2]) | `[ b, 0.0, 4.0, 0.0 ]` | 评估 `v0`，压栈 |
| 5 | `0x01` (PUSH_CONST [3]) | `[ b, 0.0, 4.0, 0.0, 1.0 ]` | 评估 `v1`，压栈 |
| 6 | `0x01` (PUSH_CONST [4]) | `[ b, 0.0, 4.0, 0.0, 1.0, 0.0 ]` | 评估 `clampLeft`挂载，压栈 |
| 7 | `0x01` (PUSH_CONST [5]) | `[ b, 0.0, 4.0, 0.0, 1.0, 0.0, 1.0 ]` | 评估 `clampRight`挂载，压栈 |
| 8 | `0x60` (EASE [2]) | `[ result ]` | 弹出 7 个值，计算后压回结果 |
| 9 | `0xFF` (RET) | `[]` | 退出表达式求值，返回 `result` |

#### 9.9.8 特殊操作

| Opcode | 助记符 | 栈效果 |
|--------|----------|-------------|
| `0x70` | CLAMP | pop x, lo, hi；push clamp(x, lo, hi) |
| `0x71` | LERP | pop a, b, t；push lerp(a, b, t) |
| `0x72` | SELECT | pop cond, ifTrue, ifFalse；push cond≠0 ? ifTrue : ifFalse |
| `0x73` | MIN | pop a, b；push min(a, b) |
| `0x74` | MAX | pop a, b；push max(a, b) |

#### 9.9.9 终止

| Opcode | 助记符 | 栈效果 |
|--------|----------|-------------|
| `0xFF` | RET | 表达式完成；栈顶即为结果 |

#### 9.9.10 编译示例

源码：`positionX: 200px * sin(b * pi);`

字节码：
```
PUSH_CONST [0]   // constPool[0] = 200.0
PUSH_VAR_B
PUSH_CONST [1]   // constPool[1] = pi
MUL
SIN
MUL
RET
```

源码：`scaleX: d > 200px ? 1.5 : 1.0;`

字节码：
```
PUSH_VAR_D
PUSH_CONST [0]    // constPool[0] = 200.0
CMP_GT
JMP_IF_FALSE +8
PUSH_CONST [1]    // constPool[1] = 1.5
JMP +5
PUSH_CONST [2]    // constPool[2] = 1.0
RET
```

### 9.10 着色器段 (Shader Section)

仅当 Flags bit 0 置位时存在。

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 | shaderCount | 着色器数量 |
| 4 | 可变 | Shader[] | shaders | 着色器数组 |

**着色器条目**：

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 (stOff) | name | 着色器名称 | 线名称 | 谱面名称 |
| 4 | 4 | u32 (stOff) | vertexPath | 顶点着色器路径 |
| 8 | 4 | u32 (stOff) | fragmentPath | 片段着色器路径 |
| 12 | 4 | u32 | bindCount | 绑定数量 |
| 16 | 可变 | Bind[] | binds | 绑定数组 |

**绑定条目**：

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 4 | u32 (stOff) | uniformName | uniform 名称 |
| 4 | 8 | PropertyDescriptor | value | 绑定的值 |

---

 ## 10. 附录：RPE ↔ FCS 转谱对照总表

 ### 10.1 Note 类型映射

 | FCS `kind` | PGR `type` | RPE `type` | 说明 |
 |------------|-----------|-----------|------|
 | `tap` | 1 | 1 | Tap |
 | `drag` | 2 | **4** | Drag (注意 RPE 与 PGR 编号不同) |
 | `hold` | 3 | **2** | Hold |
 | `flick` | 4 | **3** | Flick |
 | `fake` | 0 | — | RPE/PGR 无不可见音符等效 (见 §5.6.4.2) |

 ### 10.2 Note 属性映射

 | FCS 属性 | PGR 字段 | RPE 字段 | 方向 |
 |----------|---------|---------|------|
 | `time` | `time` (T 单位) | `startTime` (Beat `[a,b,c]`) | ↔ |
 | `endTime` (Hold) | `holdTime` (T 单位) | `endTime` (Beat `[a,b,c]`) | ↔ |
 | `positionX` | `positionX` (X 单位 或 V1 编码) | `positionX` (float) | ↔ |
 | `speed` | `speed` | `speed` | ↔ |
 | `above` | — (notesAbove/notesBelow 分离) | `above` (0/1) | ↔ |
 | `alpha` (0.0–1.0) | — | `alpha` (0–255) | ↔ 需范围转换 |
 | `scaleX` | — | `size` | → FCS `scaleX` ↔ RPE `size` |
 | `yOffset` | — | `yOffset` | ↔ |
 | `fake` | — | `isFake` | → (见 §5.6.4.2) |
 | `color` | — | `tint` / `color` | ↔ 格式不同 |
 | `judgeShape` | — | `judgeArea` (RPE 170+) | → 部分映射 |

 ### 10.3 判定线属性映射

 | FCS 属性 | PGR 字段 | RPE 字段 | 方向 |
 |----------|---------|---------|------|
 | `name` | — | `Name` | ↔ |
 | `texture` | — | `Texture` | ↔ |
 | `textureAnchor` | — | `anchor` | ↔ |
 | `zOrder` | — | `zOrder` | ↔ |
 | `parent` | — | `father` (索引) | ↔ (名称→索引转换) |
 | `bpmTimeline` | `bpm` (标量) | `BPMList` + `bpmfactor` | → 展开 |
 | `motion.layer.positionX` | `judgeLineMoveEvents` (start/start2) | `moveXEvents` | ↔ |
 | `motion.layer.positionY` | `judgeLineMoveEvents` (end/end2) | `moveYEvents` | ↔ |
 | `motion.layer.rotation` | `judgeLineRotateEvents` | `rotateEvents` | ↔ 方向取反 |
 | `motion.layer.alpha` | `judgeLineDisappearEvents` | `alphaEvents` | ↔ 范围转换 |
 | `motion.layer.speed` | `speedEvents` (value) | `speedEvents` | ↔ |
 | `motion.layer.scaleX` | — | `extended.scaleXEvents` | → |
 | `motion.layer.scaleY` | — | `extended.scaleYEvents` | → |
 | `color` | — | `extended.colorEvents` | → |

 ### 10.4 坐标系统对照

 | | FCS | PGR | RPE | PEC |
 |---|---|---|---|---|
 | 画布尺寸 | 1920×1080 逻辑坐标 | — (屏幕百分比) | 1350×900 | — (2048/1400 编码) |
 | X 中心 | 0 | 0.5 (屏幕比例) | 0 | 1024 |
 | Y 中心 | 0 | 0.5 (屏幕比例) | 0 | 700 |
 | X 范围 | [-960, 960] | [0, 1] | [-675, 675] | [0, 2048) |
 | Y 范围 | [-540, 540] | [0, 1] | [-450, 450] | [0, 1400) |
 | 1 单位 | 1px = 1 逻辑单位 | 1X = 108px (@1920w) | 1 单位 = 1/1350 屏宽 | 1 单位 = 1/2048 屏宽 |
 | 时间单位 | beat (b) | T (1T = 1.875/BPM s) | Beat `[a,b,c]` | beat×2048 (int) |
 | 缓动 | 29 种 + bezier | 无 (仅线性) | 1–28 + bezier | 无 (控制点线性) |

 ### 10.5 缓动编号对照

 | ID | FCS 函数名 | RPE easingType | 说明 |
 |----|-----------|---------------|------|
 | 1 | `easeLinear` | 1 | 线性 |
 | 2 | `easeOutSine` | 2 | — |
 | 3 | `easeInSine` | 3 | — |
 | ... | ... | ... | 1–28 完全对应 |
 | 28 | `easeInOutBounce` | 28 | — |
 | 29 | `easeInOutElastic` | **不支持** | RPE 降级为 1 (linear) |
 | — | `easeBezier` | 0 (bezier=1) | 控制点存 `bezierPoints` |

 ### 10.6 转换方向与特性保留矩阵

 | 特性 | FCS→PGR | FCS→RPE | FCS→PEC | RPE→FCS | PGR→FCS |
 |------|---------|---------|---------|---------|---------|
 | 模板 (templates) | AOT 展平 | AOT 展平 | AOT 展平 | — | — |
 | 原型继承 (prototype) | AOT 展平 | AOT 展平 | AOT 展平 | — | — |
 | 父子线 (parent) | AOT 展平 | 保留 father | AOT 展平 | 保留 parent | — |
 | 数学表达式 | 采样求值 | 采样求值或直接映射 | 采样求值 | 保留为表达式 | 保留为表达式 |
 | 缓动函数 | 展开为线性密集段 | 直接映射 easingType | 展开为线性段 | 直接映射 | 展开为线性段 |
 | `kind: fake` | 丢弃 | 丢弃 | 丢弃 | — | — |
 | `shaders` | 丢弃 + Warning | 丢弃 + Warning | 丢弃 + Warning | — | — |
 | RPE Controls | — | 从 FCS 表达式采样烘焙 | — | 保留为扩展键 | — |
 | RPE Extended | — | 扩展键回退 | — | 保留为扩展键 | — |
 | RPE 独有 meta/line/note 字段 | — | 扩展键回退 | — | 保留为扩展键 | — |

---