# FCS Core Source Specification 5.0.0

状态：Frozen（2026-07-14）

本文是 FCS（Functional Chart Specification）5.0.0 Core 的规范性定义。规范用语、
版本治理和冻结条件见 `docs/specification-governance.md`。FCBC、Render Profile 和格式转换
分别由 `fcbc.md`、`fcs-render.md` 和 `fcs-conversion.md` 定义。

FCS 5 的目标是用严格、确定、可验证的 canonical semantic model 同时服务制谱、播放、
游玩、渲染和低损失格式转换。FCS source 是人类可编辑输入；运行时不得直接执行 source
宏、template 或 generator。

---

## 1. 模型和 Conformance

### 1.1 三层模型

FCS 文档包含三个概念层：

```text
FCS Source
├── Canonical Semantic Layer
├── Provenance / Fidelity Layer
└── Deterministic Execution Model
    └── FCBC Runtime Representation
```

- Canonical 层定义 FCS Core 真正执行的跨格式语义；
- Fidelity 层保存来源、原始字段和回写信息，但不自动获得 Core 执行语义；
- Execution 层是 canonical chart 在唯一物理时钟下的确定性求值；
- FCBC 是独立版本化的编译产物，不是 FCS source AST 的二进制转储。

### 1.2 实现类别

- **FCS parser**：实现第 2–5 章 source syntax；
- **FCS compiler**：实现 static semantics、canonical lowering 和全部 portable validation；
- **FCS runtime**：实现第 8–14 章 execution semantics；
- **FCS tool**：若修改 source，必须保持语义或报告修改；
- **FCS Core conforming chart**：不依赖未声明 extension 即可执行；
- **FCS portable chart**：只使用 Core 或已声明 portable profile 能力。

实现可以设置资源限制，但必须在处理前公开限制，并用结构化 diagnostic 拒绝超限输入，
不得产生不完整而自称成功的结果。

### 1.3 确定性

在相同 source bytes、FCS 版本、编译 profile、资源内容和 extension 版本下：

- 名称解析、类型检查和展开顺序必须确定；
- canonical ID 和排序必须确定；
- runtime 查询结果必须在规定数值容差内一致；
- FCBC deterministic profile 必须 byte-for-byte 一致；
- 哈希表迭代顺序、线程调度和渲染帧率不得改变 gameplay 语义。

---

## 2. 编码、词法和文件头

### 2.1 编码

FCS source 必须是有效 UTF-8。文件可以带一个 UTF-8 BOM；parser 必须忽略文件起始 BOM，
其他位置的 U+FEFF 是普通字符且不得出现在标识符中。换行可以是 LF 或 CRLF；source span
使用原始 UTF-8 bytes 的半开区间 `[start,end)`，BOM 若存在计入原始 byte offset。

实现必须拒绝无效 UTF-8、孤立 surrogate escape、NUL 和非字符码点。标识符和 string
按原始 Unicode code point 比较，不执行隐式大小写折叠或 Unicode normalization。

### 2.2 文件头

除 BOM 外，文件第一个 token 必须是：

```fcs
#fcs 5.0.0
```

版本必须有 major、minor、patch 三段十进制无符号整数。FCS 5 compiler：

- 必须拒绝非 5 major；
- 可以接受不高于自身支持 minor 的 5.x source；
- 必须接受同 major/minor 下更高 patch，因为 patch 不改变有效输入语义；
- 必须拒绝自身未知的未来 minor，除非实现声明对应 feature-level conformance；
- 不得根据 `meta.chartVersion` 判断格式兼容性。

文件头后必须有换行或文件结束。文件头不得重复，也不得由注释代替。

### 2.3 空白和注释

空格、Tab、CR 和 LF 是 trivia。支持：

```fcs
// 单行注释
/* 可嵌套的块注释 */
```

块注释必须正确闭合并允许嵌套。string 内的注释标记没有注释语义。

### 2.4 标识符和关键字

普通标识符语法：

```text
identifier = [A-Za-z_][A-Za-z0-9_]*
```

FCS 5 Core 标识符只使用 ASCII，以保证跨工具稳定。人类语言内容放入 UTF-8 string。
以下词保留：

```text
format fragment chart playable renderable publishable
meta contributors credits resources artwork sync custom
definitions const let fn template return if else choose when
generate emit in step with true false null
lines line collections notes judgelines tracks track segments segment keyframe point using
render extensions preserve
bool int float string time beat length angle color vec2 array
Note Line RenderNode Track TrackSegment Keyframe
tap hold flick drag above below
replace add multiply base zero one error holdBefore holdAfter
```

保留词不能作为未转义标识符。FCS 5.0 不提供 escaped identifier。

### 2.5 数值 literal

```text
integer = -?(0|[1-9][0-9]*)
float   = -?((0|[1-9][0-9]*)\.[0-9]+|[0-9]+[eE][+-]?[0-9]+|
           (0|[1-9][0-9]*)\.[0-9]+[eE][+-]?[0-9]+)
```

整数语义范围为有符号 64 bit。Float 是 IEEE 754 binary64，但 source decimal 必须先按
十进制精确读取，再正确舍入到最近偶数。source 不允许 NaN、Infinity、十六进制 float、
前导 `+` 或数字分隔符。

### 2.6 String

String 只使用双引号，支持：

```text
\n \r \t \\ \" \0 \u{H...H}
```

Unicode escape 含 1–6 个十六进制数字，结果必须是有效 Unicode scalar value。未定义
escape、裸换行和未闭合 string 是语法错误。

### 2.7 Color

Color literal 使用 `#RRGGBB` 或 `#RRGGBBAA`，每个分量为两位十六进制；省略 alpha 时
等于 `FF`。canonical color 是四个 `[0,1]` linear RGBA Float64 分量。literal 的 RGB
字节按 sRGB EOTF 转换到 linear；alpha 线性映射。颜色插值使用非预乘 linear RGBA，
compositing 前转换为预乘形式。

### 2.8 分隔符

声明和 field 以 `;` 结束。最后一个 array/object 元素允许尾随逗号。Block 结束 `}` 后
不要求分号，除非该 block 本身是 expression statement 的一部分。parser 必须拒绝未消费
的非 trivia 输入。

### 2.9 Array、Object、Reference 和 Interval

Array 使用方括号、逗号分隔，允许空 array 和尾随逗号：

```fcs
[1, 2, 3]
[]
```

Typed custom object 使用 `{ "key": value, ... }`，key 必须是 string literal，允许空 object 和
尾随逗号。Schema block 使用 `identifier: expression;`，不能与 custom object 语法混用。

文档内稳定引用写 `@identifier`，其静态类型由使用 schema 决定；不存在或类型不匹配是错误。
Interval 是 schema syntax，不是一等 value：`[start,end)` 表示半开，`[start,end]` 表示闭区间。
Core Track/active/visibility 默认只接受半开 interval；闭区间只在明确声明 endpoint-inclusive
的 schema 中允许。`vec2(a,b)` 是内建构造，两个分量类型必须相同。

---

## 3. 单位和基础类型

### 3.1 类型集合

Core compile-time value 类型：

```text
bool int float string time beat length angle color vec2<T> array<T>
```

实体和结构类型：

```text
Note Line RenderNode Track<T> TrackSegment<T> Keyframe<T>
```

`vec2<T>` 只允许 `T` 为 `int`、`float`、`time`、`beat`、`length` 或 `angle`。`array<T>` 的 T
只能是 compile-time pure value type，不能是 Note、Line、RenderNode、Track 或 segment entity。
`array<T>`
是 immutable homogeneous sequence；空 array 必须由 field、参数或显式类型上下文确定 `T`，
非空 array 的每个元素必须具有完全相同类型。Array 支持 `.length:int` 和按编译期 int 常量
索引读取，不支持拼接、修改或 runtime indexing。实体不能参与 equality、ordering、算术或
隐式序列化。

### 3.2 单位 literal

数字与单位之间不得有空白。Core 单位：

| 类型 | suffix | canonical 转换 |
|---|---|---|
| time | `ns`, `us`, `ms`, `s`, `min` | 秒 |
| beat | `beat` | 精确有理 beat |
| length | `px` | FCS logical pixel |
| angle | `deg`, `rad`, `turn` | radian |
| tempo | `bpm` | beats per minute |

`1min = 60s`，`1ms = 0.001s`，`1turn = 2πrad`，`180deg = πrad`。`bpm` 只用于
tempo schema，不是一等基础类型。time、beat、length 和 angle 彼此不同，禁止隐式转换。

Beat literal 的十进制必须保留为精确有理数，例如 `0.1beat = 1/10 beat`。运算结果若
超出实现声明的 rational numerator/denominator 限制，编译失败，不得转为 Float64 继续。

### 3.3 默认值

只有 schema 明确列出的 field 才有默认值。类型本身不提供隐式默认；缺少 required field
是错误。`0` 不是 time/beat/length/angle，必须写 `0s`、`0beat`、`0px`、`0deg`。

### 3.4 显式转换

Core 提供：

```text
toFloat(int) -> float
seconds(time) -> float
radians(angle) -> float
```

time 与 beat 的转换只能通过 document `tempoMap` 在 canonical lowering 中完成，不提供
不带 tempo context 的普通纯函数。

---

## 4. 表达式和静态类型规则

### 4.1 运算优先级

从高到低：

```text
postfix: call, field access
unary:   !, unary -
power:   **                 右结合
product: *, /, %
sum:     +, -
order:   <, <=, >, >=
equal:   ==, !=
logical: &&
logical: ||
```

括号显式覆盖优先级。comparison chain `a < b <= c` 等价于 `a < b && b <= c`，中间
expression 只求值一次。

### 4.2 运算矩阵

- `int`：同类型 `+ - * / % **`；整数除法向零截断；除零错误；负 exponent 错误；
- `float`：同类型 `+ - * / **`；不提供 float `%`；任何非有限结果错误；
- unit value：同类型 `+ -` 和 ordering；可与 `int`/`float` scalar 相乘或相除；
- 两个同类型 unit value 相除返回 `float`；
- `bool`：`! && || == !=`；
- `string`、`color`：只支持 `== !=`；
- `vec2<T>`：同类型 equality、同类型向量加减、与 scalar 乘除和 `.x/.y`；
- entity：不支持上述操作。

`time`、`beat`、`length`、`angle` 的 ordering 只允许同类型。比较两侧类型必须完全相同。
Float equality 是精确 binary64 equality；近似比较必须调用：

```text
approxEq(value: float, expected: float, tolerance: float) -> bool
```

Tolerance 必须有限且不小于零。

`&&` 和 `||` 使用从左到右 short-circuit：`false && rhs` 与 `true || rhs` 不求值 rhs。名称和
类型检查仍覆盖 rhs；只有求值期 numeric/domain error 可以因 short-circuit 不发生。Array index
必须是编译期 int 且满足 `0 <= index < length`，越界是 static error。

### 4.3 内建常量和纯函数

```text
pi: float
tau: float
abs, min, max, clamp
floor, ceil, round
sqrt, exp, ln, pow
sin, cos, tan, asin, acos, atan, atan2
approxEq, toFloat, seconds, radians
```

`abs` 接受 int、float 和 unit scalar；`min/max/clamp` 接受同一 ordered scalar type；
`floor/ceil/round/sqrt/exp/ln/sin/cos/tan/asin/acos/atan` 接受 float；`atan2` 接受两个 float；
`pow` 接受两个 float。`round` 使用 ties-to-even。`clamp(x,lo,hi)` 要求三者同类型且
`lo <= hi`。`pi` 和 `tau` 是对应实数正确舍入到 binary64 nearest-even 的常量。
非法 domain、除零、overflow、NaN 或 Infinity 是编译错误或 runtime invalid-value error，
不得静默 clamp、隐藏对象或传播成未定义行为。

### 4.4 字段访问

字段访问可以逐层读取静态 schema 中存在的字段：

```fcs
note.presentation.visibility.from
line.transform.inherit.rotation
```

字段不存在、访问 non-object 或访问当前 phase 不可用 schema 都是静态错误。实体值只能在
编译期读取；runtime expression 只能读取该属性允许的环境和 descriptor 输入。

### 4.5 Runtime value 边界

编译期 expression 不得读取第 13 章的 runtime 环境变量。依赖 runtime 的值不能决定：

- entity type 或 Note kind；
- field 是否存在；
- collection 数量；
- `emit` 是否发生；
- gameplay time、judgment、side、parent 或 inherit；
- Render node 数量、类型、parent、path topology 或 resource identity。

允许动态的 field 必须使用 Track、`choose` 或可编译为 PropertyDescriptor 的 expression。

---

## 5. 文档结构和 Profile

### 5.1 顶级结构

文件头后可以出现：

```text
format
meta
contributors
credits
resources
artwork
sync
definitions
tempoMap
lines
collections
render
extensions
preserve
```

除 `extensions` 中不同 namespace 外，每类顶级 block 最多一个。顶级 block 的文本顺序
不改变名称解析或 execution semantics；其中 collection item、credit、Render sibling 和
显式 document order 按源顺序赋予稳定顺序。

未知顶级 block 是错误。Extension 必须位于 `extensions` 并声明 namespace 和版本。

### 5.2 Format

每个文档必须有：

```fcs
format {
    profile: chart;
}
```

Profile：

- `fragment`：允许缺少 tempo、line、Note、audio 和发布 metadata；
- `chart`：必须有合法 tempo/time model，可以为空谱面；
- `playable`：扩展 chart，要求 primary audio、sync 和至少一个 gameplay line；
- `renderable`：扩展 chart，要求 Render scene 及所有被引用渲染资源；
- `publishable`：扩展 chart，要求 title、documentId、chartVersion、license、至少一个 credit、
  所有外部资源的 cryptographic hash，并且至少声明 `playable` 或 `renderable` feature。

`playable` 与 `renderable` 是正交能力，二者互不包含。`features` 可以在 primary profile 的
最低约束上增加另一项能力；同时需要两者时使用：

```fcs
format { profile: publishable; features: [playable, renderable]; }
```

`fragment` 不得声明 `playable`、`renderable` 或 `publishable` feature。`chart`、`playable`、
`renderable` 和 `publishable` 都包含 chart 的 tempo/time model 约束。

### 5.3 Definitions

`definitions` 统一包含 `const`、`fn` 和 typed entity template。FCS 5 不使用独立顶级
`templates` block。

### 5.4 Collections

标准 collection：

```text
notes       -> Note
judgelines  -> Line
```

Line 可以在 `lines` 声明，也可以从 `judgelines` collection emit；二者进入同一 ID 空间，
重复 ID 是错误。

Track 不使用全局 collection。每个 Line、Note 或 Render node 在自己的 `tracks` block 声明
Track；每个 Track 的 `segments` 是注册为 `TrackSegment<T>`/`Keyframe<T>` 的局部 collection，
因此可以使用 compile-time generator。

---

## 6. 编译期语言

### 6.1 绑定和作用域

```fcs
definitions {
    const NOTE_SPACING: length = 120px;
}
```

- `const` 只允许在 `definitions`；
- `let` 允许在 `fn`、template、generator 和它们的 compile-time `if` block；
- `const`、`let`、参数和 generator variable 必须显式类型并初始化；
- 所有绑定不可变；Core 没有 `var`、赋值、`++`、`+=` 或 mutable collection；
- 同一作用域和任何嵌套作用域都禁止 shadowing；
- sibling branch 可以声明同名局部，因为作用域互不嵌套；
- 名称解析允许同一 definitions block 内 forward reference；
- const/function/template 依赖图必须无环。

### 6.2 纯函数

```fcs
fn wave(at: beat, period: beat, amplitude: length) -> length {
    let phase: float = at / period;
    return sin(phase * tau) * amplitude;
}
```

函数 body 只包含显式类型 `let`、compile-time `if` 和 `return`。每条可达路径必须 return
一个与声明完全相同的类型。函数可以调用纯函数，不得调用 template、`generate` 或 `emit`。
函数参数可以是 pure value 或只读 entity，用于读取静态 field；返回类型必须是 pure value，
不能返回 entity/Track。函数无 overload；用户声明不得覆盖 builtin。

### 6.3 Typed template

```fcs
template Note ghostTap(hitTime: beat, x: length, whichLine: Line) {
    let hidden: bool = true;
    if hidden {
        return tap {
            line: whichLine;
            gameplay.time: hitTime;
            gameplay.judgment.enabled: false;
            presentation.positionX: x;
        };
    } else {
        return tap {
            line: whichLine;
            gameplay.time: hitTime;
            presentation.positionX: x;
        };
    }
}
```

Template 参数和返回实体类型必须显式声明。Body 使用与函数相同的局部 `let` 和编译期
`if`，但 return 必须是声明实体类型。Template 可以调用纯函数和其他 template；template
调用图必须无环。Template 每次返回一个实体，不隐式返回 collection。

影响 entity variant、field set 或结构的条件必须在实例化时是 compile-time bool。Template
return 的实体必须已经满足全部 required field；调用者不能用后续 `with` 把不完整实体补成有效
实体。

### 6.4 Constructor 和 `with`

```fcs
tap {
    id: "intro-1";
    gameplay.time: 4beat;
    presentation.positionX: -150px;
}
```

Constructor field path 必须存在，类型必须精确匹配，同一 constructor 不得重复 field。
Required field 必须在每个 constructor 或 template return 完成时存在。`with` 可以覆盖或显式
设置 schema 中的 optional/default field，但不能延迟 required-field validation。

```fcs
ghostTap(4beat, 0px, @main) with {
    presentation.alpha: 0.5;
}
```

`with` 产生新的不可变实体；不能增加 schema 未定义 field、改变 entity type 或改变 Note
variant。同一 `with` block 不得重复 path。嵌套 `with` 从内到外应用。FCS 5 不提供独立
prototype inheritance。

### 6.5 Compile-time `if`

```fcs
if ENABLE_GHOSTS {
    emit ghostTap(at, 0px);
} else {
    emit normalTap(at, 0px);
}
```

Condition 必须是 compile-time `bool`。所有分支都必须完成名称解析、类型检查、schema 检查
和 return-path 检查；只对被选分支执行值求值、template 实例化和 `emit`。因此未选分支中的
除零等仅在求值时发生的错误不触发，但未知名称、类型错误和缺失 required field 始终触发。
结构条件不得降级成 runtime `choose`。

### 6.6 Generator

唯一 range 语法：

```fcs
generate at: beat in 20beat..<80beat step 10beat { ... }
generate i: int in 4..=0 step -1 { ... }
```

- `..<` 是半开区间；
- `..=` 是包含终点区间；
- 裸 `..` 在 FCS 5.0 中是语法错误；
- start、end、step 必须是相同类型的 compile-time value；
- 类型只能是 `int` 或 `beat`；
- step 不得为零；
- 正 step 且 start 大于 end，或负 step 且 start 小于 end，产生空 range，不是错误；
- 当前值必须按 `start + index * step` 计算，禁止 Float64 重复累加；
- inclusive range 只包含恰好由该公式到达的 end，不额外调整最后一个值。

`range.count` 是满足区间条件的迭代次数。对半开 ascending range，它是最小非负 `n`，
使 `start + n*step >= end`；descending 使用对称定义。Inclusive range 在可达 end 时比对应
半开 range 多一次。计算必须检查 integer/rational overflow。

Generator scope 提供只读绑定：

```text
用户变量      声明的 int 或 beat
index         int，从 0 开始
range.start   与用户变量同类型
range.end     与用户变量同类型
range.step    与用户变量同类型
range.count   int
```

Generator body 允许显式类型 `let`、compile-time `if` 和 `emit`。Generator 不可嵌套，
不可出现在 `fn` 或 template 内，不得修改外部状态或读取 runtime-only value。

### 6.7 Emit

`emit expression;` 只允许在 generator body，expression 必须产生当前 collection 注册的
实体类型。Collection 顶层可以直接写 constructor/template expression，无需 `emit`。
输出严格保持 collection 和 iteration source order。

### 6.8 展开预算

每次完整 elaboration 使用一个共享 budget context：

```text
maxExpansionDepth          默认 128
maxGeneratedNodes          默认 100000
maxGeneratorIterations     默认 100000
maxTemplateInstances       默认 10000
maxCompileTimeOperations   默认 1000000
maxExpressionNodes         默认 100000
```

每项工作开始前递增对应计数，若新值超过 limit 则不执行该工作并报错。嵌套 elaboration 不得
重新创建预算。环检测先于 depth budget；不得通过“递归到上限”接受递归程序。

预算错误必须包含 budget kind、limit、observed count、source span 和有序 expansion trace。
Trace 至少包含函数/template 调用、collection、range、generator index 和待 emit 类型。

所有 `const`、`let`、`fn`、template、`with`、compile-time `if`、`generate`、`emit`、range
和 index 必须在 canonical semantic lowering 前消失。FCBC 不为它们分配 runtime slot。

---

## 7. Metadata、人员、资源和 Sync

### 7.1 Meta

所有字段可选，profile 可以增加 required 约束：

```fcs
meta {
    title: "Example";
    subtitle: "";
    alternativeTitles: ["示例"];
    chartVersion: "1.2";
    difficulty: "Hard";
    level: 12.5;
    description: "";
    language: "zh-Hans";
    tags: ["tech"];
    license: "CC-BY-4.0";
    documentId: "org.example.chart";
    revision: 3;
    custom: { "editor": "example" };
}
```

FCS 格式版本不得存入 meta。`revision` 是非负 int。`level` 必须有限。普通 meta field
顺序无语义，array 顺序保留。

### 7.2 Contributors 和 Credits

```fcs
contributors {
    person alice {
        name: "Alice";
        aliases: ["AliceP"];
        identifiers: { "musicbrainz": "..." };
    }
}

credits {
    credit {
        role: composer;
        label: "作曲";
        contributors: [@alice];
    }
    credit {
        role: custom("chart-effects");
        label: "特效谱面";
        contributors: [@alice];
    }
}
```

Contributor ID 在文档内唯一。标准 role 包括 composer、arranger、lyricist、vocalist、
instrumentalist、mixer、mastering、charter、illustrator、designer、programmer 和 publisher。
其他 role 必须使用非空 ASCII custom ID。Label 是自由 UTF-8 展示文本。Credit 顺序有展示
语义。来源含糊的 `artist` 不得自动解释为 composer。

### 7.3 Resources 和 Artwork

```fcs
resources {
    audio song {
        source: "song.ogg";
        hash: "sha256:...";
        mediaType: "audio/ogg";
    }
    image cover {
        source: "cover.png";
        hash: "sha256:...";
        mediaType: "image/png";
        colorSpace: srgb;
        alpha: straight;
        sampling: linear;
    }
}

artwork {
    primary: @cover;
}
```

Resource type 包括 audio、image、font、texture、path、shader 和 binary。Resource ID 唯一。
Hash 语法为 `<algorithm>:<lowercase-hex>`；Core 必须支持 SHA-256。Publishable 外部资源必须
有 hash。引用必须存在且类型匹配。Resource source 是 URI reference，不允许隐式工作目录
逃逸；具体 resolver 由宿主声明。

Text 渲染必须引用 font resource，不能依赖系统 fallback。Image 的 color space、alpha 和
sampling 必须显式或使用 schema 默认；默认是 `srgb`、`straight`、`linear`。

### 7.4 Sync

```fcs
sync {
    primaryAudio: @song;
    audioOffset: 100ms;
    preview: [30s, 45s);
}
```

唯一符号定义：

```text
audioTime = chartTime + audioOffset
chartTime = audioTime - audioOffset
```

因此 `audioOffset = +100ms` 表示 `chartTime=1.0s` 时读取 audio `1.1s`。Preview 是 audio
time 域的半开区间，必须满足 `end > start >= 0s`。

### 7.5 Typed custom data

Custom data 是保持插入顺序的 typed value：null、bool、int64、float64、string、time、beat、
color、resource/contributor reference、homogeneous array 或 ordered object。Array 的每个元素必须
具有相同 value type；空 array 必须由所属 custom schema 声明 element type。Object key 是 string
且不得重复。非有限 float 和无效引用是错误。Compiler profile 必须限制深度、字段数、string
长度和总 bytes。

---

## 8. 唯一时间模型

### 8.1 Chart time

FCS runtime 只有一个物理主时钟 `chartTime`，单位秒。它驱动音频同步、Note 判定、Hold、
line motion、transform、visibility、Render、shader、speed 和 distance 查询。Line 不拥有可
独立暂停、快进、倒放或推进的物理时钟。

负 chartTime 合法，用于 pre-roll。Pause 保持 chartTime 不变；resume 从同值继续。Seek 到
任意有限 chartTime 必须产生与从初始状态顺序求值到该点相同的 Core 结果。

### 8.2 Tempo map 和 chart beat

```fcs
tempoMap {
    0beat  -> 180bpm;
    64beat -> 200bpm;
}
```

`chartBeat` 是全局音乐坐标，不是第二物理时钟。Tempo point beat 非递减，第一项必须在
`0beat`。BPM 必须有限且大于零。同一 beat 的连续 point 表示瞬时阶跃；该 beat 之后使用
文本中最后一个 point 的 BPM，零长度前置 point 仅保留 provenance。

在相邻不同 beat point `[b0,b1)` 内 BPM 为 b0 最后 point 的常量值：

```text
chartTime(b) = chartTime(b0) + (b-b0) * 60 / bpm(b0)
```

0beat 对应 0s；audio offset 不改变此映射。负 beat 使用 0beat 生效 BPM 向前外推；最后
point 后保持最后 BPM。映射严格单调，time→beat 取唯一逆。

普通 source `beat` 总是全局 chartBeat。Compiler 必须把 gameplay 和 motion event time
归一化到 chartTime，同时可以保留 exact beat provenance。

### 8.3 判定时间与滚动坐标

Note 判定只由 canonical chartTime 决定。每条 line 可以有：

```text
lineScrollCoordinate_i = q_i(chartTime)
```

`q_i` 是 chartTime 的纯函数，用于滚动和 floor distance，不是独立 clock。Line 的 BPM、
RPE `bpmfactor` 或外部局部 tick 不得隐式改变 Note 判定时间。

---

## 9. Track 模型

### 9.1 Track schema

```fcs
tracks {
  track slide -> positionX: length {
    blend: replace;
    priority: 0;
    fill: base;
    extrapolateBefore: base;
    extrapolateAfter: holdAfter;

    segments {
        [0s, 1s): 0px -> 100px using easeInOutSine;
        point 1s: 100px;
    }
  }
}
```

Track ID（示例中的 `slide`）在 owner 内唯一，用于 stable ordering 和 provenance；target path
（示例中的 `positionX`）必须是 owner schema 允许动态化的 field。每个 Track 有 value type、
property target、blend、priority、fill 和 extrapolation。Segment
使用 chartTime 半开区间 `[start,end)` 且必须满足 `end > start`。瞬时 step 使用 point，
不得伪装成零长度 easing segment。

Segment 的 start/end 可以写 time 或 beat，但同一 segment 两端必须同类型；canonical lowering
后统一为 chartTime。Direct segment 的 `using` 可以是 `step`、`linear`、Core easing 名或
`cubicBezier(x1,y1,x2,y2)`。

`segments` collection 可以生成结构：

```fcs
segments {
    generate at: beat in 0beat..<8beat step 1beat {
        emit segment {
            start: at;
            end: at + 1beat;
            startValue: 0px;
            endValue: 20px;
            interpolation: linear;
        };
    }
}
```

在 Track<T> 中，`segment` 构造 `TrackSegment<T>`，required field 是 start、end、startValue、
endValue、interpolation；`keyframe` 构造 `Keyframe<T>`，required field 是 time、value，等价于
direct `point`。类型 `T` 从 enclosing Track 固定，emit 其他类型是 collection mismatch。

Track setting 只允许 blend、priority、fill、extrapolateBefore、extrapolateAfter。Blend 默认
`replace`，priority 默认 0。Fill 默认随 blend 为 replace→base、add→zero、multiply→one；
before/after extrapolation 默认等于 fill。`segments` 必须非空。Owner 的 static field value 是
base；省略 static field 时使用该 field 的 schema default，required 且无默认 field 不能只靠
Track 在部分时间补齐。

### 9.2 Blend 和优先级

固定合成顺序：

```text
base value
→ 生效的最高 priority replace
→ 所有 add 按 (priority, stableId) 求和
→ 所有 multiply 按 (priority, stableId) 求积
```

同一 target、同一最高 priority 的多个 replace 在同一时刻生效是错误。Add/multiply 只允许
schema 声明可组合的数值类型。文本重排不能改变结果；stableId 是最后 tie-breaker。

### 9.3 重叠、缺口和外推

同一 Track 的普通 segment 不得重叠。需要 layered overlap 时必须使用独立 Track 和显式
blend/priority。缺口行为：

- `base`：target schema base value；
- `zero`：类型加法单位元；
- `one`：类型乘法单位元；
- `holdBefore`：最近后继 segment 的 start value；
- `holdAfter`：最近前驱 segment 的 end limit；
- `error`：存在查询缺口时 chart 非 portable。

Replace 默认 `base`，add 默认 `zero`，multiply 默认 `one`；scroll speed 的内部描述必须
覆盖积分域，未覆盖且无显式 fill 时错误。

### 9.4 插值

Segment 内：

```text
p  = (chartTime-start)/(end-start)
p' = easing(clamp(p,0,1))
v  = interpolate(startValue,endValue,p')
```

Core interpolation 支持 step、linear、cubic Bezier 和附录 A easing。Bezier 进度参数为
`[x1,y1,x2,y2]`；`x1,x2` 必须在 `[0,1]`，曲线 x 必须可单值反解。Y 可以 overshoot，
但 target schema 可以禁止超界。

### 9.5 Segment endpoint

在 segment end 的精确时刻，该 segment 不再生效；相邻 `[end,nextEnd)` segment 生效。
若无相邻 segment，按 fill/extrapolateAfter。Point event 在其时刻及之后生效，直到后续
segment/point 或策略覆盖。多个同 target 同时刻 point 由 priority 决定；同 priority 冲突。

同一 Track 中 point 与 segment 在同一时刻开始时，segment 负责该时刻及之后；point value 必须
与 segment startValue 完全相同，否则是 `track.replace-conflict`。Point 落在普通 segment 内是
overlap error，不能临时切断 segment。

---

## 10. Scroll speed 和 distance

### 10.1 Scroll coordinate

每条 line 的 `scrollTempoMap` 定义：

```text
dq_i/dt = scrollBpm_i(t) / 60
```

规范固定 `q_i(0s)=0`。负时间通过首项 scroll BPM 向前积分，正时间按 point 分段积分。Q 的
单位是无量纲 scroll beat；它不等于全局 chartBeat，除非两张 tempo map 恰好相同。

Scroll BPM 必须有限且大于零。默认 `scrollTempoMap` 等于 global tempoMap 在 chartTime 域
的 BPM，但 line 可以显式覆盖。覆盖只改变 q，不改变 chartBeat 或判定时间。

Source override 使用：

```fcs
scrollTempoMap {
    0beat  -> 180bpm;
    32beat -> 240bpm;
}
```

同一 scrollTempoMap 的 key 必须全为 beat 或全为 time，非递减且第一项为对应类型的零。Beat
key 先通过 global tempoMap 映射 chartTime；time key 直接使用。相邻 point 间 BPM 分段常量，
同 key 连续 point 是右侧最后一项生效的瞬时 step；首项向负时间外推，末项向后外推。

### 10.2 Speed multiplier

`scrollSpeed` 是相对于 q 的无量纲 multiplier：

```text
floor_i(t) = initialFloorPosition
           + integral[integrationOrigin,t](scrollSpeed_i(u) dq_i(u))
```

等价于：

```text
integral scrollSpeed_i(u) * scrollBpm_i(u)/60 du
```

默认 scrollSpeed 为 `1.0`。负值只有 line 显式声明 `allowReverseScroll: true` 时合法；零值
停止滚动但不停止 chartTime。Speed 可以在边界跳变，floorPosition 必须连续。

### 10.3 Floor 和 Note Y

`floorPosition` 是无量纲累计距离；`floorScale` 是每 floor unit 对应的 logical length。

```text
localY(note,t) =
    (floor(note.gameplay.time)-floor(t))
    * floorScale
    * note.presentation.scrollFactor
    + note.presentation.yOffset
```

Line 使用 `scrollSpeed`，Note 使用 `scrollFactor`，二者不共享含糊的 `speed` field。

### 10.4 积分和 portable 分类

- `portable-exact`：常量、线性、已知 easing 或可证明精确的分段积分；
- `portable-baked`：同时携带 velocity、distance、domain 和 max error；
- `runtime-only`：需要独立 feature 与 ABI，不得冒充 Core portable。

Core 禁止把固定 120Hz、1000Hz 或渲染帧累加作为 floorPosition 真相。Seek 必须直接查询
descriptor 或确定性积分，不依赖此前渲染帧历史。

---

## 11. 坐标、变换和 Line

### 11.1 坐标空间

FCS logical canvas 是 `1920px × 1080px`，中心原点，X 向右，Y 向上。明确区分：

```text
world space
line-local space
note-local space
scroll space
screen space（仅 Render Profile）
```

Note `positionX` 默认位于所属 line 的 line-local X。

### 11.2 Line schema

```fcs
line main {
    parent: null;
    position: vec2(0px, 0px);
    rotation: 0deg;
    scale: vec2(1.0, 1.0);
    alpha: 1.0;
    transformOrigin: vec2(0px, 0px);
    textureAnchor: vec2(0.5, 0.5);
    floorScale: 120px;
    integrationOrigin: 0s;
    initialFloorPosition: 0.0;
    allowReverseScroll: false;
    zOrder: 0;
}
```

Position、rotation、scale 和 alpha 可以是 Track target。Scale 分量必须有限；零 scale
允许渲染但使 inverse gameplay geometry 不可用，若 judge shape 需要 inverse 则错误。

Line ID 来自 `line <identifier>` 或 `Line { id: ... }`。除 ID 外的默认值与示例相同：parent
null、position/origin 为零、rotation 0deg、scale/alpha 为一、textureAnchor `(0.5,0.5)`、
floorScale 120px、integrationOrigin 0s、initialFloorPosition 0、allowReverseScroll false、zOrder 0。
Inherit 默认 position/rotation/scale/alpha=true、scroll=false。ScrollTempo 默认 global tempo，
scrollSpeed 默认 1，因此 `line main {}` 是有效 identity Line。

### 11.3 矩阵

列向量、右乘局部点，固定顺序：

```text
M_local(t) = T(position(t))
           * T(transformOrigin)
           * R(rotation(t))
           * S(scale(t))
           * T(-transformOrigin)

M_world(child,t) = M_world(parent,t) * M_local(child,t)
```

正 rotation 为逆时针。Core 不支持 shear；外部 shear 只能保留、烘焙为允许表示或报告损失。
`textureAnchor` 使用 `[0,1]²` texture 坐标且不进入几何矩阵。

### 11.4 Parent 和 inherit

```fcs
inherit {
    position: true;
    rotation: true;
    scale: true;
    alpha: true;
    scroll: false;
}
```

Parent 必须存在，不能是自身，所有 line 构成 DAG。Compiler 使用 stable ID tie-break 的稳定
拓扑序。`scroll` 默认 false；父线 transform 不自动把父 floor distance 加给子线。禁用某项
inherit 时，子线 world transform 对应分量从 world identity/base 开始，而非先继承后抵消。
为避免矩阵分解歧义，每条 Line 还维护由声明分量递归得到的 world component state：

```text
parentPosition = inherit.position ? parent.worldOrigin : vec2(0px,0px)
parentRotation = inherit.rotation ? parent.worldRotation : 0rad
parentScale    = inherit.scale    ? parent.worldScale    : vec2(1,1)

M_inherited = T(parentPosition) * R(parentRotation) * S(parentScale)
M_world     = M_inherited * M_local
worldOrigin = M_world * vec3(0,0,1)
worldRotation = parentRotation + localRotation
worldScale    = parentScale componentMultiply localScale
worldAlpha    = (inherit.alpha ? parent.worldAlpha : 1) * localAlpha
```

`worldRotation/worldScale` 来自声明分量递推，不从 `M_world` 数值分解。非均匀 scale 与 rotation
组合可以使最终矩阵包含几何 shear，但 source 仍未声明独立 shear property。`inherit.scroll`
只控制明确的 scroll descriptor composition，不进入几何 component state。

### 11.5 排序

Gameplay line ID 与渲染排序无关。默认视觉排序键：

```text
(zOrder, documentOrder, stableId)
```

Cover、UI attachment 和 Render pass 不进入 line 几何矩阵。

---

## 12. Note 模型

### 12.1 Identity、gameplay、presentation

每个 Note 具有文档内唯一稳定 ID，并分为：

```text
identity
gameplay
presentation
```

Core kind 只有 `tap | hold | flick | drag`。Fake 不是 kind。

### 12.2 Gameplay schema

```fcs
tap {
    id: "n1";
    line: @main;
    gameplay.time: 4beat;
    gameplay.side: above;
    gameplay.judgment.enabled: true;
    gameplay.judgeShape: lineDefault;
    gameplay.soundPolicy: default;
    gameplay.scorePolicy: default;
}
```

Source required：line、time；`id` 可选。缺失 ID 时 compiler 按第 17 章使用规范化 source path、
template call path、generator index 和 document order 生成 canonical stable ID。默认 side=`above`、
judgment.enabled=true。Gameplay kind、time、
endTime、side、judgment、judgeShape、soundPolicy、scorePolicy 和 line 必须编译期确定。

Hold 必须有 `endTime` 且 `endTime > time`；非 Hold 不得设置 endTime。Time 可以用 beat 或
time source value，canonical 后必须是 chartTime。Judge shape 在 line/note gameplay 坐标中
定义，不能依赖 Render frame。

### 12.3 Presentation schema

默认：

```text
positionX    0px
scrollFactor 1.0
xOffset      0px
yOffset      0px
alpha        1.0
scaleX/Y     1.0
rotation     0deg
color        #FFFFFFFF
render.enabled true
visibleFrom  -infinity chart time
visibleUntil +infinity chart time
```

Portable source 不写无穷 literal；缺失 visibility boundary 表示无界。`alpha`、visibility 和
`render.enabled` 相互独立，且都不自动关闭 judgment。可见但不可判定的 Note 使用：

```fcs
gameplay.judgment.enabled: false;
presentation.render.enabled: true;
```

`gameplay.judgment.enabled`、`presentation.render.enabled`、`visibleFrom` 和 `visibleUntil` 必须
编译期确定；visibility interval 必须满足 end>start。Position、offset、alpha、scale、rotation、
color 可以动态。`scrollFactor` 可以依赖常量及 `s/b/q`，不得依赖 `d`，因为 `d` 本身使用
scrollFactor。Line transform、scrollTempo 和 scrollSpeed 同样不得依赖 Note `d`。

### 12.4 Position 和 Hold

```text
sideSign = gameplay.side == above ? +1 : -1
d = sideSign
    * (floor(noteTime)-floor(now))
    * floorScale
    * scrollFactor

localX = positionX + xOffset
localY = d + yOffset
```

Runtime 环境变量 `d` 精确等于上式、不包含 `yOffset`。Above/below 由 sideSign 表达，不通过负
alpha 或 texture flip 表达。Hold head 位于 time，tail 位于 endTime，body 覆盖
`[time,endTime]` 的几何连接；判定
事件仍由 gameplay profile 定义且不依赖渲染采样。动态 presentation 可以按 `d` 求值。

普通 Note 和 Hold head 的 `d` 使用 gameplay.time。Hold tail 使用 endTime。对 Hold body 的连续
几何点，令 body parameter `h∈[0,1]`，其 anchor time 是 `time+h*(endTime-time)`，该点求值时
`d` 使用 anchor time；`h` 不作为 Core expression 环境暴露。Renderer 的 tessellation 只能采样
这个已定义连续函数，不能改变函数或 gameplay 结果。

### 12.5 排序

Canonical Note 排序键：

```text
(gameplay.time, lineStableId, documentOrder, noteStableId)
```

同一时刻 Note 合法。Stable ID 冲突是错误，不得由容器迭代顺序自动重命名。

---

## 13. Runtime expression 与 `choose`

### 13.1 环境变量

```text
s: time    当前 chartTime
b: beat    当前全局 chartBeat
q: float   当前 lineScrollCoordinate
d: length  Note 到判定线的有符号 logical distance
p: float   当前 Track segment normalized progress，范围 [0,1]
```

Core 不定义多义 `t` 别名。环境可用性由 target schema 限制：scrollSpeed 只能依赖 `s/b/q`
和常量，不能依赖 Note `d`、Render 或外部输入。Gameplay 结构 field 不允许 runtime expression。

### 13.2 Expression 分类

```text
constant
chart-time
line-scroll
note-presentation
runtime-only-extension
```

Compiler 必须拒绝 speed、distance、transform、q、d 或 attachment 之间的循环依赖。

### 13.3 Choose

```fcs
presentation.alpha: choose {
    when d < 50px  => 1.0;
    when d < 200px => 0.5;
    else           => 0.0;
};
```

Predicate 必须 bool；所有 result 类型完全相同；必须有 else；按声明顺序选择第一个 true；
未选 result 不求值。Choose 只返回值，不能 emit、创建实体、修改状态或改变 gameplay 结构。

### 13.4 Lowering

Compiler 依次尝试：

```text
constant folding
exact Track partition
analytic boundary lowering
finite PiecewiseDescriptor
adaptive baking
```

FCBC Core 表达式是 typed DAG/descriptor，不包含 jump、store、loop、recursive call、emit 或
generate。Portable expression 不得依赖 IO、未固定随机数、wall clock 或宿主未定义行为。

每个 DAG node 在 binary64 roundTiesToEven 下独立舍入；不得使用未声明 FMA、扩展精度寄存器
或重排结合律。`+0/-0` 按 IEEE 754 运算保留，NaN/Infinity 在产生该 node 时立即成为 execution
error。Compiler constant folding 必须产生与相同 node 求值完全一致的 bits。

---

## 14. 精度、自适应烘焙和 Easing

### 14.1 数值模型

Canonical runtime time、position、angle、speed、distance、Bezier、easing 和 transform 使用
Float64。Source/canonical 尽量保留 decimal 和 rational beat。非有限结果无效。

Core `sqrt/exp/ln/sin/cos/tan/asin/acos/atan/atan2/pow` 的结果定义为对应实函数正确舍入到
binary64 roundTiesToEven；argument reduction 也是该定义的一部分。实现不能直接继承宿主
`libm` 的未声明误差。Conformance vectors 提供困难输入的期望 bits；不能正确舍入的实现不满足
strict-runtime ABI 1.0。

属性分类：

```text
Exact Constant
Exact Segment
Exact Piecewise
Adaptive Baked
```

Tempo、Track、Note、Hold、visibility、Render active 和 emit 的所有离散时间点都是强制精确
边界，不得量化到采样网格。

### 14.2 Adaptive baked

BakedCurve 必须记录 domain、value type、interpolation、segments、declared max error、
validation profile 和 source expression hash。实现可以使用任意确定性算法，但必须在每个
segment 内证明或按 profile 验证误差。Strict profile 在无法解析证明时的验证采样间隔最多
1ms；这不是强制输出 1000Hz LUT。

误差指标：

| 属性 | 指标 |
|---|---|
| position/offset/geometry | logical px absolute error |
| rotation | 最短角距离 |
| alpha/scale | scalar absolute error |
| color | linear RGBA channel max error |
| scrollSpeed | absolute 与 relative error 较严格者 |
| scrollDistance | floor absolute error |

Speed 必须同时验证瞬时 velocity 和累计 distance。无法在 segment、evaluation、depth 或
wall-time budget 内满足声明误差时，strict 编译失败。

### 14.3 Core easing

令输入 `x∈[0,1]`。所有 easing 满足端点精确钉扎：x=0 返回 0，x=1 返回 1。Core 名称：

```text
linear
easeIn/Out/InOutSine
easeIn/Out/InOutQuad
easeIn/Out/InOutCubic
easeIn/Out/InOutQuart
easeIn/Out/InOutQuint
easeIn/Out/InOutExpo
easeIn/Out/InOutCirc
easeIn/Out/InOutBack
easeIn/Out/InOutElastic
easeIn/Out/InOutBounce
```

Base ease-in 函数：

```text
Sine:    1-cos(πx/2)
Quad:    x²
Cubic:   x³
Quart:   x⁴
Quint:   x⁵
Expo:    x=0 ? 0 : 2^(10x-10)
Circ:    1-sqrt(1-x²)
Back:    c3*x³-c1*x², c1=1.70158, c3=c1+1
Elastic: x=0?0 : x=1?1 : -2^(10x-10)*sin((10x-10.75)*2π/3)
```

Out 和 InOut 由任意 base `f` 唯一定义：

```text
out_f(x)   = 1-f(1-x)
inout_f(x) = x<0.5 ? f(2x)/2 : 1-f(2-2x)/2
```

Bounce 先定义 `outBounce(x)`，`n1=7.5625`、`d1=2.75`：

```text
x < 1/d1      : n1*x²
x < 2/d1      : n1*(x-1.5/d1)² + 0.75
x < 2.5/d1    : n1*(x-2.25/d1)² + 0.9375
otherwise     : n1*(x-2.625/d1)² + 0.984375
```

`easeInBounce(x)=1-outBounce(1-x)`；InOut 按上述 inout 结构使用 easeInBounce。实现必须使用
binary64 常量和这些公式，不得用名称相同但参数不同的宿主 easing。

---

## 15. Extension、Fidelity 和 Repair

### 15.1 Extension

Extension 必须声明全球唯一 namespace、schema version、required/optional 和 typed payload。
未知 optional extension 可以保留并忽略执行；未知 required extension 必须拒绝 compile/play。

### 15.2 Preserve

```fcs
preserve {
    source {
        format: "rpe";
        version: "170";
        hash: "sha256:...";
    }
    payload: extension("org.phigros.rpe", 1.0.0) { ... };
}
```

Preserve 数据不自动改变 Core 语义。Raw snapshot 只保证 source 未被语义编辑时可精确回写。
来源状态至少区分 unset、explicit-default、explicit-value、inherited、imported、generated 和
user-modified；不得用“值等于默认值”猜测来源状态。

### 15.3 Repair

默认编译不得静默执行：NaN→隐藏、未知 parent→无 parent、speed gap→猜填、非法区间→交换、
alpha→clamp。显式 repair mode 每次修改必须记录 source path、diagnostic、action、old value、
new value 和语义影响。Repair 后的结果不能标记 lossless。

---

## 16. Diagnostic 和错误

Diagnostic 至少包含稳定 category、severity、primary span、相关 span、message 和 expansion
trace。人类 message 文本不属于兼容 API，category 属于。

Portable Core 中下列情况是 error：

- 无效 UTF-8、版本、语法或单位；
- duplicate/shadowed binding、未知名称、类型错误或调用环；
- 非法/嵌套 generator、zero step 或 budget exceeded；
- NaN、Infinity、除零和非法数学 domain；
- BPM 不大于零、tempo map 非法；
- Track overlap/conflict、未声明 speed gap 或非法 easing；
- parent 不存在、自环或 DAG cycle；
- Hold `endTime <= time`；
- resource/reference/hash/profile requirement 无效；
- runtime expression 循环依赖或 dynamic gameplay structure；
- adaptive baking 无法满足预算内误差；
- 不支持的 required extension 或版本 major。

Warning 只能用于不影响 Core 成功语义的可疑情况。任何近似、保留但不执行或丢弃必须由
ConversionReport 记录，不能只打印 warning。

---

## 17. Canonical lowering

Compiler 固定执行阶段：

```text
decode/header
→ parse source AST
→ resolve names and versions
→ type/schema check
→ reject dependency cycles
→ evaluate const/fn/template/generator
→ produce ExpandedSourceDocument
→ normalize beat/time and IDs
→ validate tempo/Track/Line/Note/resource graphs
→ build CanonicalChart
→ classify/lower/bake runtime properties
→ serialize FCBC profile
```

CanonicalChart 至少包含：source version、profile/features、tempo map、metadata、contributors、
credits、resources、sync、stable Line/Note/Track、runtime property descriptor、extensions、
provenance 和 conversion/repair records。它不得包含 source comment、template、generate、emit、
局部变量、statement if 或 parser-only token。

Canonical ID 由显式 ID 优先；缺失允许 ID 的 schema 使用规范化 source path 和 document order
生成稳定 ID。显式 ID 与生成 ID 的 namespace 分离，避免碰撞。Publishable Note/Line/Resource
必须最终拥有稳定 ID。

---

## 18. Conformance 测试

规范 fixture 必须覆盖：

1. 版本、编码、词法、类型和单位；
2. const/fn/template/generate、作用域、环和预算；
3. tempo、time/beat、audio offset 和 seek；
4. Track 区间、blend、gap、easing、speed 和 distance；
5. 坐标、矩阵、父线 DAG 和 Note/Hold；
6. runtime expression、piecewise 和 adaptive error；
7. metadata、credits、resources、sync 和 custom data；
8. source→expanded→canonical→FCBC→reference execution 闭环；
9. PGR/RPE/PEC semantic round-trip；
10. parser/compiler fuzz、property tests 和资源上限。

每个重要规范示例必须对应可执行 fixture，记录预期成功、diagnostic category、canonical snapshot
或数值 test vector。Reference evaluator 至少公开：

```text
lineTransform(lineId, chartTime)
lineScrollCoordinate(lineId, chartTime)
lineScrollDistance(lineId, chartTime)
notePresentation(noteId, chartTime)
noteJudgmentGeometry(noteId)
```

必须验证 seek 与顺序求值等价、编译期结构完全消失、FCBC 无通用控制流、同输入 deterministic。

---

## 附录 A：最小完整示例

```fcs
#fcs 5.0.0

format { profile: chart; }

meta {
    title: "FCS 5 Example";
    chartVersion: "1";
}

tempoMap {
    0beat -> 180bpm;
}

definitions {
    const STEP: beat = 1beat;

    template Note normalTap(at: beat, x: length, whichLine: Line) {
        return tap {
            line: whichLine;
            gameplay.time: at;
            gameplay.side: above;
            presentation.positionX: x;
        };
    }
}

lines {
    line main {
        position: vec2(0px, 0px);
        rotation: 0deg;
        floorScale: 120px;
    }
}

collections {
    notes {
        generate at: beat in 0beat..<4beat step STEP {
            let x: length = sin(toFloat(index) * pi) * 100px;
            emit normalTap(at, x, @main);
        }
    }
}
```

展开后产生 4 个 Note，time 精确为 0、1、2、3 beat；FCBC 中不出现 `STEP`、template、
generator、index、`let`、`emit` 或 source-level `with`。

---

## 附录 B：Core Source Grammar

以下 EBNF 定义 Core 语法骨架；具体 block 可用 field 及类型由正文 schema 决定。`trivia` 可以
出现在任意 token 之间，此处省略。

```text
document       = bom?, header, formatBlock, topLevelBlock* ;
header         = "#fcs", semver, newline ;
semver         = uint, ".", uint, ".", uint ;

topLevelBlock  = metaBlock | contributorsBlock | creditsBlock | resourcesBlock
               | artworkBlock | syncBlock | definitionsBlock | tempoMapBlock
               | linesBlock | collectionsBlock | renderBlock
               | extensionsBlock | preserveBlock ;

formatBlock    = "format", "{", formatField+, "}" ;
formatField    = ("profile", ":", profile
               | "features", ":", array), ";" ;
profile        = "fragment" | "chart" | "playable" | "renderable" | "publishable" ;

definitionsBlock = "definitions", "{", definition*, "}" ;
definition       = constDecl | functionDecl | templateDecl ;
constDecl        = "const", identifier, ":", type, "=", expression, ";" ;
functionDecl     = "fn", identifier, "(", parameters?, ")", "->", type,
                   statementBlock ;
templateDecl     = "template", constructibleType, identifier, "(", parameters?, ")",
                   templateBlock ;
parameters       = parameter, (",", parameter)*, ","? ;
parameter        = identifier, ":", type ;

statementBlock   = "{", functionStatement*, "}" ;
templateBlock    = "{", templateStatement*, "}" ;
functionStatement = letDecl | functionIf | returnValue ;
templateStatement = letDecl | templateIf | returnEntity ;
letDecl          = "let", identifier, ":", type, "=", expression, ";" ;
functionIf       = "if", expression, statementBlock,
                   ("else", (statementBlock | functionIf))? ;
templateIf       = "if", expression, templateBlock,
                   ("else", (templateBlock | templateIf))? ;
returnValue      = "return", expression, ";" ;
returnEntity     = "return", entityExpression, ";" ;

linesBlock       = "lines", "{", lineDecl*, "}" ;
lineDecl         = "line", identifier, entityBlock ;
collectionsBlock = "collections", "{", collection* , "}" ;
collection       = identifier, "{", collectionItem*, "}" ;
collectionItem   = entityExpression, ";"
                 | collectionIf
                 | generator ;
collectionIf     = "if", expression, "{", collectionItem*, "}",
                   ("else", "{", collectionItem*, "}")? ;

generator        = "generate", identifier, ":", rangeType, "in",
                   expression, rangeOperator, expression, "step", expression,
                   "{", generatorStatement*, "}" ;
rangeType        = "int" | "beat" ;
rangeOperator    = "..<" | "..=" ;
generatorStatement = letDecl | generatorIf | emitStatement ;
generatorIf      = "if", expression, "{", generatorStatement*, "}",
                   ("else", "{", generatorStatement*, "}")? ;
emitStatement    = "emit", entityExpression, ";" ;

entityExpression = entityConstructor
                 | identifier, "(", arguments?, ")"
                 | entityExpression, "with", schemaBlock ;
entityConstructor = noteVariant, entityBlock
                  | "Line", entityBlock
                  | "RenderNode", entityBlock
                  | "segment", schemaBlock
                  | "keyframe", schemaBlock ;
noteVariant      = "tap" | "hold" | "flick" | "drag" ;
schemaBlock      = "{", schemaField*, "}" ;
entityBlock      = "{", (schemaField | tracksBlock | scrollTempoMapBlock)*, "}" ;
schemaField      = fieldPath, ":", expression, ";" ;
fieldPath        = identifier, (".", identifier)* ;

tracksBlock      = "tracks", "{", trackDecl*, "}" ;
trackDecl        = "track", identifier, "->", fieldPath, ":", type,
                   "{", trackSetting*, segmentsBlock, "}" ;
trackSetting     = identifier, ":", expression, ";" ;
segmentsBlock    = "segments", "{", segmentItem*, "}" ;
segmentItem      = directSegment | directPoint | generator | segmentIf ;
segmentIf        = "if", expression, "{", segmentItem*, "}",
                   ("else", "{", segmentItem*, "}")? ;
directSegment    = interval, ":", expression, "->", expression,
                   "using", interpolation, ";" ;
directPoint      = "point", expression, ":", expression, ";" ;
interval         = "[", expression, ",", expression, ")" ;
interpolation    = identifier | "cubicBezier", "(", expression, ",", expression,
                   ",", expression, ",", expression, ")" ;
scrollTempoMapBlock = "scrollTempoMap", "{", scrollTempoPoint+, "}" ;
scrollTempoPoint = expression, "->", unitLiteralBpm, ";" ;

expression      = logicalOr ;
logicalOr       = logicalAnd, ("||", logicalAnd)* ;
logicalAnd      = equality, ("&&", equality)* ;
equality        = ordering, (("==" | "!="), ordering)* ;
ordering        = sum, (("<" | "<=" | ">" | ">="), sum)* ;
sum             = product, (("+" | "-"), product)* ;
product         = power, (("*" | "/" | "%"), power)* ;
power           = unary, ("**", power)? ;
unary           = ("!" | "-"), unary | postfix ;
postfix         = primary, (("(", arguments?, ")") | (".", identifier)
                | ("[", expression, "]"))* ;
primary         = literal | identifier | reference | array | object
                | vec2Constructor | chooseExpression | "(", expression, ")" ;
vec2Constructor = "vec2", "(", expression, ",", expression, ")" ;
arguments       = expression, (",", expression)*, ","? ;
reference       = "@", identifier ;
array           = "[", (expression, (",", expression)*, ","?)?, "]" ;
object          = "{", (stringLiteral, ":", expression,
                   (",", stringLiteral, ":", expression)*, ","?)?, "}" ;
chooseExpression = "choose", "{", chooseArm+, elseArm, "}" ;
chooseArm       = "when", expression, "=>", expression, ";" ;
elseArm         = "else", "=>", expression, ";" ;

type            = scalarType | "vec2", "<", scalarType, ">"
                | "array", "<", type, ">" | entityType
                | "Track", "<", type, ">"
                | "TrackSegment", "<", type, ">"
                | "Keyframe", "<", type, ">" ;
scalarType      = "bool" | "int" | "float" | "string" | "time" | "beat"
                | "length" | "angle" | "color" ;
entityType      = "Note" | "Line" | "RenderNode" ;
constructibleType = entityType
                  | "TrackSegment", "<", type, ">"
                  | "Keyframe", "<", type, ">" ;
```

`renderBlock` 的内部 grammar 由 `fcs-render.md` 扩展；extension payload 的 grammar 由对应
namespace schema 定义。Lexer 必须对 `..<` 和 `..=` 执行 longest match，单独 `..` 无 token。

---

## 附录 C：稳定 Diagnostic Category

实现可以增加更细 subcategory，但以下 category 名称及含义稳定：

```text
decode.invalid-utf8
version.missing-header
version.invalid
version.unsupported
syntax.invalid-token
syntax.unclosed-comment
syntax.unclosed-string
syntax.trailing-input
syntax.misplaced-block
name.unknown
name.duplicate
name.shadowed
name.cycle
type.mismatch
type.invalid-operation
type.invalid-conversion
schema.unknown-field
schema.duplicate-field
schema.missing-required-field
schema.non-constructible
schema.collection-type-mismatch
schema.dynamic-field-forbidden
compile-time.non-constant-condition
compile-time.invalid-range
compile-time.zero-step
compile-time.nested-generator
compile-time.misplaced-generator
compile-time.budget-exceeded
numeric.non-finite
numeric.divide-by-zero
numeric.domain
numeric.overflow
tempo.invalid
tempo.non-monotonic
track.invalid-interval
track.overlap
track.replace-conflict
track.gap
track.invalid-easing
graph.unknown-parent
graph.cycle
note.invalid-hold
resource.unknown-reference
resource.type-mismatch
resource.hash-mismatch
expression.cycle
expression.environment-unavailable
baking.error-budget-unsatisfied
extension.unsupported-required
profile.requirement-missing
repair.applied
```

Parser 失败使用最具体 syntax/version/decode category；static/canonical 阶段不得用通用
`syntax.invalid-token` 代替类型、schema、graph 或数值错误。Expansion trace 是
`compile-time.budget-exceeded` 和递归/cycle diagnostic 的结构化字段。
