# FCS Core Source Specification 5.0.0

状态：Draft（2026-07-15；authoring/canonical closure 与联合候选自检已完成，等待完整 fixture validation 与独立复审）

本文是 FCS（Functional Chart Specification）5.0.0 Core 的规范性定义。规范用语、
版本治理和冻结条件见 `docs/specifications/governance.md`。FCBC、Render Profile 和格式转换
分别由 `fcbc.md`、`fcs-render.md` 和 `fcs-conversion.md` 定义。

FCS 5 的目标是用严格、确定、可验证的 canonical semantic model 同时服务制谱、播放、
游玩、渲染和低损失格式转换。FCS source 是谱师和开发工具使用的人类可编辑 authoring 格式，
通常与资源文件共同位于普通目录形式的 authoring workspace。发行播放器必须支持的最小输入是
FCBC，而不是 FCS source；运行时不得直接执行 source 宏、template 或 generator。

---

## 1. 模型和 Conformance

### 1.1 Authoring 到 runtime 的阶段模型

标准 FCS 工具链固定区分以下对象：

```text
AuthoringWorkspace
├── FCS source file
└── declared resource files
    ↓ parse / static semantics / elaboration
SourceDocument
    ↓ 展开全部 authoring-only 结构
ExpandedSourceDocument
    ↓ resource resolution / canonical validation / exact lowering
CanonicalCompilation
├── CanonicalChart
├── CanonicalResourceBundle
└── DistributionMetadata
    ↓ FCBC deterministic writer
one-chart self-contained FCBC
```

- **SourceDocument** 保留 source AST、原始 byte span、comment/trivia 位置、声明结构和诊断所需的
  authoring 信息；
- **ExpandedSourceDocument** 只包含已实例化的 concrete entity/Track/metadata 结构和 typed
  runtime expression；`const`、局部绑定、纯函数、template、compile-time `if`、generator、
  `emit` 和 `with` 已经求值或展开，但实现可以继续保留 source mapping 供诊断与编辑器使用；
- **CanonicalChart** 定义 Core gameplay、time、Line、Note、Track、runtime property 与逻辑资源
  identity 的唯一执行语义；它不包含 source AST、comment、template/generator 定义、局部变量、
  workspace path 或 raw source snapshot；
- **CanonicalResourceBundle** 为每个 canonical resource ID 保存 kind、media type、content hash 和
  workspace 输入文件的原始 bytes；资源 payload 不是 runtime expression，也不进入 gameplay
  canonical comparison；
- **DistributionMetadata** 可以保存不会改变执行结果的 provenance、repair/ConversionReport、
  compiler identity 和输入 content hash，但不得保存 FCS 文本、source AST 或用于恢复
  comment/template/generator 的 source snapshot；
- **Execution** 是 CanonicalChart 在唯一物理时钟下对 exact descriptor 的确定性求值；
- **FCBC** 是独立版本化、恰好包含一个 chart 与其全部资源的编译分发物，不是 FCS source AST
  的二进制转储。其 record layout、资源 payload 和 loader contract 由 `fcbc.md` 定义。

Provenance/Fidelity 是与上述阶段正交的数据域：它可以说明来源和转换事实，但不能反向改变 Core
语义。需要 byte-exact 回写 FCS 或外部 source 时，工具必须保留原始 authoring workspace；标准
FCBC 不承担 source round-trip。

本规范所称 FCS→FCBC **语义无损**，只表示 CanonicalChart 的 execution/Render 语义与所有资源
bytes 都被保留，不表示 source 文本或 authoring abstraction 可逆。两个 source 可以使用完全不同
的 template、generator、局部名称、comment 和排版，只要它们产生相同 CanonicalChart、资源内容
hash 与规范性 distribution metadata，就具有相同的分发语义。

### 1.2 实现类别

- **FCS parser**：实现第 2–7 章、相关 schema source 章节和 Appendix B 的 Core syntax，并为
  profile payload 提供有界、可完整消费的解析入口；
- **FCS compiler**：实现 static semantics、canonical lowering 和全部 portable validation；
- **FCS packager**：从声明的 workspace root 解析资源，生成 CanonicalCompilation，并按
  `fcbc.md` 写出 one-chart self-contained FCBC；
- **FCS runtime**：从已验证的 CanonicalChart/FCBC descriptor 实现第 8–14 章 execution
  semantics，不解析 FCS source；
- **FCS tool**：若修改 source，必须保持语义或报告修改；
- **FCS Core conforming chart**：不依赖未声明 extension 即可执行；
- **FCS portable chart**：只使用 Core 或已声明 portable profile 能力。

发行播放器、移动端模拟器和 headless renderer 无需实现 FCS parser/compiler。制谱器预览 FCS
时，必须调用与发行打包相同的 parse/static/elaborate/canonical 管线，再把内存中的 canonical/FCBC
descriptor 交给共享 runtime；不得建立一套“预览专用 FCS 语义”。

实现可以设置资源限制，但必须在处理前公开限制，并用结构化 diagnostic 拒绝超限输入，
不得产生不完整而自称成功的结果。

### 1.3 确定性

在相同 source bytes、FCS 版本、编译 profile、workspace logical path→resource bytes 映射、资源
resolver policy 和 extension 版本下：

- 名称解析、类型检查和展开顺序必须确定；
- canonical ID 和排序必须确定；
- runtime 查询结果必须在规定数值容差内一致；
- FCBC deterministic profile 必须 byte-for-byte 一致；
- workspace 的宿主绝对路径不得改变 canonical ID、resource identity 或 deterministic FCBC bytes；
- 哈希表迭代顺序、线程调度和渲染帧率不得改变 gameplay 语义。

### 1.4 前端阶段边界

FCS 前端固定区分以下责任；后续阶段不得为了复用现有实现而把错误提前到 parser：

| 阶段 | 责任 | 不属于该阶段的行为 |
|---|---|---|
| decode | UTF-8、BOM、原始 byte span | token、类型或 schema 判断 |
| header/lex/parse | version、token、delimiter、Appendix B grammar、完整输入消费 | profile capability requirement、名称、类型、schema、tempo/Track/graph 数值合法性 |
| source structure | required format block/field、duplicate format/top-level block、statement/generator placement | profile capability requirement、preserve schema completeness、range 求值、zero step、collection emit 类型 |
| static/elaborate | 名称、类型、schema、作用域、const/function/template/generator 展开 | canonical graph、tempo/Track overlap、runtime 数值求值 |
| resource resolve | 规范化 logical workspace path、读取 opaque bytes、验证声明 hash | 媒体解码/重编码、网络获取或 runtime 外部文件查找 |
| canonical | profile requirement、tempo、ID、resource、Track、Line、Note、graph validation 与 exact descriptor lowering | parser repair、默认 baking 或运行时采样 |
| evaluate | 已验证 descriptor 的确定性运行时求值 | 改变 canonical 结构 |

Conformance manifest 的 `parse` stage 包含 decode、header/lex/parse 和 source-structure 检查。
因此 duplicate block、nested generator 和 misplaced generator 可以在 `parse` fixture 中使用其
稳定专用 category；这不授权 parser 执行类型、schema 或数值语义。除这些明确的结构检查外，
语法正确但语义非法的 source 必须仍能产生完整 source AST。

---

## 2. 编码、词法和文件头

### 2.1 编码

FCS source 必须是有效 UTF-8。文件可以带一个 UTF-8 BOM；parser 必须忽略文件起始 BOM，
其他位置的 U+FEFF 是普通字符且不得出现在标识符中。换行可以是 LF 或 CRLF；source span
使用原始 UTF-8 bytes 的半开区间 `[start,end)`，BOM 若存在计入原始 byte offset。

实现必须拒绝无效 UTF-8、原始 source 中的 U+0000、孤立 surrogate escape 和 Unicode
noncharacter。String escape `\0` 明确允许并产生一个 U+0000 string value；它不等于在 source
bytes 中直接写入 NUL。`\u{...}` 的结果除必须是 Unicode scalar value 外，也不得是
noncharacter。标识符和 string 按解码后的 Unicode code point 比较，不执行隐式大小写折叠或
Unicode normalization。

本规范的 Unicode noncharacter 是 `U+FDD0..U+FDEF`，以及每个 Unicode plane 末尾的
`U+FFFE/U+FFFF` 对应码点（至 `U+10FFFE/U+10FFFF`）。

### 2.2 文件头

除 BOM 外，文件第一个 token 必须是：

```fcs
#fcs 5.0.0
```

`#fcs` 与版本之间必须恰有一个 ASCII space。版本必须有 major、minor、patch 三段十进制
无符号整数；除单独的 `0` 外，各段不得有前导零。Appendix B 的 `semver` 是一个连续的复合
lexeme：三段数字和两个 `.` 之间不得出现空白或 comment，lexer 必须先于 `floatMagnitude` 按完整
三段形式 longest-match。Header 中除这一个 ASCII space 外不得含其他 trivia，version 后必须立即
出现 newline 或文件结束；extension/Render version 中 `semver` 两侧可以有 trivia，但其内部仍
不得有。FCS 5 compiler：

- 必须拒绝非 5 major；
- 可以接受不高于自身支持 minor 的 5.x source；
- 必须接受同 major/minor 下更高 patch，因为 patch 不改变有效输入语义；
- 必须拒绝自身未知的未来 minor，除非实现声明对应 feature-level conformance；
- 不得根据 `meta.chartVersion` 判断格式兼容性。

文件头后必须有换行或文件结束。文件头不得重复，也不得由注释代替。
除 BOM 后的输入若不以 `#fcs` header introducer 开始，使用 `version.missing-header`；若已出现
`#fcs` 但 space、semver、换行或尾随 header token 不合法，使用 `version.invalid`。完整 semver
合法但版本不受支持时使用 `version.unsupported`。

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
以下词是 Core 保留词：

```text
format profile features fragment chart playable renderable publishable
meta contributors person credits credit resources artwork sync
definitions const let fn template return if else choose when
generate emit in step with true false null
tempoMap lines line collections notes judgelines tracks track segments segment keyframe point using
scrollTempoMap cubicBezier
render extensions extension required optional preserve source payload
audio image font texture path shader binary
bool int float string time beat length angle color vec2 array
Note Line RenderNode Track TrackSegment Keyframe
tap hold flick drag
```

保留词不能作为未转义标识符。FCS 5.0 不提供 escaped identifier。Appendix B 的 `fieldName`
在 field-path 位置可以使用 identifier 或保留词；这只允许诸如 `line:`、`render.enabled:` 和
`source:` 的 schema field，不会把保留词变成普通绑定名。

### 2.5 数值 literal

```text
uintMagnitude = 0 | [1-9][0-9]*
floatMagnitude = ((0|[1-9][0-9]*)\.[0-9]+ | [0-9]+[eE][+-]?[0-9]+ |
                  (0|[1-9][0-9]*)\.[0-9]+[eE][+-]?[0-9]+)
```

前导 `-` 始终是独立 unary operator，不属于 numeric token；因此 `a-1` 与 `a - 1` 的 token
序列相同。整数语义范围为有符号 64 bit。Parser 必须保留足够精度，使直接 unary minus 的
`9223372036854775808` 可以表示 `i64::MIN`；其他超范围 integer 在 static numeric validation
时报 `numeric.overflow`。Float 是 IEEE 754 binary64，但 source decimal 必须先按十进制精确
读取，再正确舍入到最近偶数。Source 不允许 NaN、Infinity、十六进制 float、前导 `+` 或数字
分隔符。

### 2.6 String

String 只使用双引号，支持：

```text
\n \r \t \\ \" \0 \u{H...H}
```

Unicode escape 含 1–6 个十六进制数字，结果必须是有效且非 noncharacter 的 Unicode scalar
value。未定义 escape、裸换行和未闭合 string 是语法错误。

### 2.7 Color

Color literal 使用 `#RRGGBB` 或 `#RRGGBBAA`，十六进制数字允许 `0-9A-Fa-f`，每个分量为
两位；省略 alpha 时等于 `FF`。canonical color 是四个 `[0,1]` linear RGBA Float64 分量。
literal 的 RGB 字节按 sRGB EOTF 转换到 linear；alpha 线性映射。颜色插值使用非预乘 linear
RGBA，compositing 前转换为预乘形式。

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
尾随逗号。Schema block 使用 `fieldPath: schemaValue;`，不能与 custom object 语法混用。

文档内稳定引用写 `@identifier`，其静态类型由使用 schema 决定；不存在或类型不匹配是错误。
Interval 是 schema syntax，不是一等 value。FCS Core 5.0 唯一 interval spelling 是半开
`[start,end)`；`[start,end]` 始终是两元素 array，不会由 field schema 静默重解释为闭区间。
需要 endpoint-inclusive interval 的未来 profile/extension 必须定义不同且无歧义的 source
spelling。`vec2(a,b)` 是内建构造，两个分量类型必须相同。

### 2.10 Closed enum value

Schema 中的闭集枚举静态类型是 `string`。直接写值时使用普通 UTF-8 string literal，不使用 bare
identifier 或保留词。例如：

```fcs
gameplay.side: "above";
blend: "replace";
colorSpace: "srgb";
```

`profile: chart;`、resource kind、Note variant、statement keyword 和 Render node kind 是 grammar
结构，不是 schema enum value，因此仍使用对应的裸 terminal。未加引号的普通 identifier 始终是
名称引用；它可以引用一个 compile-time `string` const，但 compiler 不得根据 field schema 把
unresolved identifier 猜成 string enum。

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

上述 generic argument 约束属于 static type validation。Source parser 按 Appendix B 保留完整
type syntax；例如 `vec2<bool>` 或 `array<Note>` 必须产生 type AST，再由 static phase 使用
`type.mismatch` 拒绝，不得伪装成 `syntax.invalid-token`。

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

相邻的 decimal 与已知 suffix 必须作为一个 unit/bpm literal 进行 longest match。Decimal 后若
立即跟随 ASCII identifier continuation，但整体不是当前 context 允许的 suffix，必须报
`syntax.invalid-token`，不得拆成 number 与 identifier 两个 token。

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

- `int`：同类型 `+ - * / % **`；整数除法向零截断，`a % b` 固定为
  `a - trunc(a/b)*b`（非零 remainder 与 a 同号）；除零错误；负 exponent 错误；
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
`approxEq(value,expected,tolerance)` 固定依次执行 binary64 subtraction `value-expected`、Abs 和 Le；
每步按第 14.1 节舍入，任何中间非有限值立即成为 invalid-value error。它不是使用宿主相对误差、
ULP heuristic 或高精度整式一次比较的别名。

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
note.presentation.visibleFrom
currentLine.transform.inherit.rotation
```

字段不存在、访问 non-object 或访问当前 phase 不可用 schema 都是静态错误。实体值只能在
编译期读取；runtime expression 只能读取该属性允许的环境和 descriptor 输入。

### 4.5 Runtime value 边界

编译期 expression 不得读取第 13 章的 runtime 环境变量。依赖 runtime 的值不能决定：

- entity type 或 Note kind；
- field 是否存在；
- collection 数量；
- `emit` 是否发生；
- gameplay time、judgment、side、judge/sound/score policy、resource identity、parent 或 inherit；
- Render node 数量、类型、parent、path topology 或 resource identity。

允许动态的 field 必须使用 Track、`choose` 或可编译为 PropertyDescriptor 的 expression。

---

## 5. 文档结构和 Profile

### 5.1 顶级结构

文件头后必须立即出现唯一的 `format` block。随后可以出现：

```text
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

`format` 不得延后或重复。其余每类顶级 block 最多一个；一个 `extensions` block 可以包含
多个 namespace 不同的 extension declaration，但 `extensions` block 本身同样最多一个。
`format` 之后各类顶级 block 的文本顺序不改变名称解析或 execution semantics；其中 collection
item、credit、extension declaration、Render sibling 和显式 document order 按源顺序赋予稳定
顺序。

未知顶级 block 是 `syntax.invalid-token`。`format` 延后、顶级 block 出现在嵌套 context 或其他
已知 block 位于错误 context 使用 `syntax.misplaced-block`。Duplicate top-level block 使用
`name.duplicate` 并将第一处声明作为 related span。Extension 必须位于 `extensions` 并按第 15 章
声明 namespace、版本和 requirement。

若完整输入中根本没有 `format` block，使用 `profile.requirement-missing`，primary span 是 header
之后的第一个 non-trivia token；若已经到 EOF，则使用 EOF 的零长度 span。若 `format` 存在但被
已知顶级 block 延后，优先使用上述 `syntax.misplaced-block` 指向先出现的 block。

### 5.2 Format

每个文档必须有：

```fcs
format {
    profile: chart;
}
```

`format` 内 field 顺序无语义。`profile` 必须恰好出现一次；`features` 最多出现一次，使用
专用 profile-feature list grammar，而不是普通 expression array。重复 field 使用
`name.duplicate`，第二个 field name 是 primary span、首个同名 field name 是 related span；缺失
`profile` 是 `profile.requirement-missing`，primary span 是 `format` block 的 closing `}`。Profile/
feature 组合是否合法以及所需的 tempo、resource、metadata 和 scene 是否存在属于 canonical
profile validation，不是 parser failure。

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

`features` 只接受 `playable` 和 `renderable`，不得包含 primary-only 的 `fragment`、`chart` 或
`publishable`。`fragment` 不得声明任何 feature。`chart`、`playable`、`renderable` 和
`publishable` 都包含 chart 的 tempo/time model 约束。

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
    line: @main;
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
    emit ghostTap(at, 0px, @main);
} else {
    emit normalTap(at, 0px, @main);
}
```

Condition 必须是 compile-time `bool`。所有分支都必须完成名称解析、类型检查、schema 检查
和 return-path 检查；只对被选分支执行值求值、template 实例化和 `emit`。因此未选分支中的
除零等仅在求值时发生的错误不触发，但未知名称、类型错误和缺失 required field 始终触发。
结构条件不得降级成 runtime `choose`。

### 6.6 Generator

唯一 range 语法：

```fcs
generate at: beat in 20beat..<80beat step 10beat { }
generate i: int in 4..=0 step -1 { }
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
和 index 必须在 canonical semantic lowering 前消失。Comment、trivia、局部绑定名、template/
generator 声明和未选择的 compile-time branch 同样不能进入 CanonicalChart 或 FCBC。FCBC 不为
它们分配 runtime slot，也不携带供播放器延迟展开的 source program。

ExpandedSourceDocument 可以为编辑器诊断保留 source span、expansion trace 和 authoring provenance，
但其中每个输出实体必须已经是 concrete entity，collection 数量与稳定 source order 已确定。
依赖第 13 章 runtime 环境的 typed property expression 不是 authoring-only 结构：它必须作为 exact
expression 保留到 canonical lowering，不能因为 template/generator 已展开而一并求值、采样或删除。

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
        role: "composer";
        label: "作曲";
        contributors: [@alice];
    }
    credit {
        role: "chart-effects";
        label: "特效谱面";
        contributors: [@alice];
    }
}
```

Contributor ID 在文档内唯一。`role` 是 string。标准 role string 包括 `"composer"`、
`"arranger"`、`"lyricist"`、`"vocalist"`、`"instrumentalist"`、`"mixer"`、
`"mastering"`、`"charter"`、`"illustrator"`、`"designer"`、`"programmer"` 和
`"publisher"`。其他 role 必须是非空 ASCII custom ID，不使用上下文相关的 `custom(...)`
特殊语法。Label 是自由 UTF-8 展示文本。Credit 顺序有展示语义。来源含糊的 `"artist"`
不得自动解释为 `"composer"`。

### 7.3 Resources 和 Artwork

```fcs
resources {
    audio song {
        source: "song.ogg";
        hash: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        mediaType: "audio/ogg";
    }
    image cover {
        source: "cover.png";
        hash: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        mediaType: "image/png";
        colorSpace: "srgb";
        alpha: "straight";
        sampling: "linear";
    }
}

artwork {
    primary: @cover;
}
```

Resource type 包括 audio、image、font、texture、path、shader 和 binary。Resource ID 是文档内
唯一、稳定的逻辑 identity；source path 和 content hash 都不能替代 ID，多个 ID 可以有相同 bytes。

`source` 是 authoring-only workspace member path，不是 runtime URI。标准 spelling 必须满足：

- 非空 UTF-8 string，component 之间只使用 `/`；
- 不以 `/`、`\`、drive prefix 或 URI scheme 开始；
- 不含 `\`、NUL、空 component、`.` 或 `..` component；
- 按解码后的 Unicode code point 和大小写 byte-exact 比较，不执行 percent decode、Unicode
  normalization 或宿主大小写折叠。

标准 Core 不允许 `http:`、`https:`、`file:`、`data:` 或其他 URI scheme。需要网络或生成式资源
的工具必须在进入标准 canonical compilation 前，把结果实体化为 workspace member，并记录工具
provenance；播放器不得重新执行获取过程。

Compiler/packager 必须由宿主显式取得 workspace root。它把 logical path 映射为一个普通文件，并
在解析 `.`/`..`、平台路径别名和 symlink 后确认最终对象仍位于 workspace root 内；目录、缺失
文件、逃逸路径和非普通文件使用 `resource.unknown-reference`。Source parser 与纯 static checker
不得为了识别语法而访问文件系统。

Hash 语法为 `<algorithm>:<lowercase-hex>`；Core 必须支持 SHA-256。Publishable resource 必须声明
SHA-256，resource resolution 必须对输入文件原始 bytes 计算 hash 并以
`resource.hash-mismatch` 拒绝不一致。非 publishable source 可以省略声明 hash，但
CanonicalResourceBundle 仍必须包含 compiler 计算的 SHA-256。

Packager 把资源当作 opaque byte sequence：不得为了编译而解码、重新编码、转码、降采样、颜色
转换或规范化。`mediaType`、image color space、alpha 和 sampling 是声明的 decoder/renderer
contract，不证明 bytes 本身可解码；实际 codec validation 与 decode error 属于消费该资源的
runtime/renderer。标准 FCS→FCBC 必须把每个通过 canonical validation 的 resource declaration
及其原始 payload 都写入同一 FCBC，不得留下 workspace path、URL 或外部 archive dependency。

Canonical resource descriptor 保存 stable resource ID、kind、media type、content hash 和类型特有
metadata，但不保存用于 runtime resolution 的 `source`。实现可以把 logical source path 保留在
authoring diagnostic/provenance 中；它不得参与 gameplay identity，且 deterministic distribution
profile 必须排除会泄露宿主绝对路径的 provenance。

所有 resource reference 必须存在且类型匹配。Resource identity 必须在 static/canonical phase
确定，runtime expression、Render frame 或播放器资源搜索顺序都不能改变引用目标。

Text 渲染必须引用 font resource，不能依赖系统 fallback。Image 的 color space、alpha 和
sampling 必须显式或使用 schema 默认；默认 string 分别是 `"srgb"`、`"straight"`、
`"linear"`。

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

Custom/extension object 的 value position 使用普通 expression grammar；expression 必须在 static
phase 求值为上述允许的 typed value。`null` 可用于 custom/extension/preserve data，以及 schema
明确声明 nullable 的 reference/field（例如无 parent）；它不是可用于普通算术或 Track value 的
Core compile-time type。

---

## 8. 唯一时间模型

### 8.1 Chart time

FCS runtime 只有一个物理主时钟 `chartTime`，单位秒。它驱动音频同步、Note 判定、Hold、
line motion、transform、visibility、Render、shader、speed 和 distance 查询。Line 不拥有可
独立暂停、快进、倒放或推进的物理时钟。

负 chartTime 合法，用于 pre-roll。Pause 保持 chartTime 不变；resume 从同值继续。Seek 到
任意有限 chartTime 必须产生与从初始状态顺序求值到该点相同的 Core 结果。

PGR line BPM、RPE `bpmfactor`、PEC command/BPM state 和其他外部 time base 只允许存在于
converter 的 import-time source decoding。Importer 必须先按显式、版本化的 source semantic
profile，把原始 time 数值映射为 FCS `beat`/`time` 或直接映射为 canonical chartTime；映射完成后，
外部 time base 不得作为 Line field、隐式 Track 参数、第二 runtime clock 或播放器兼容开关继续
存在。

Provenance/ConversionReport 可以保存原始数值表示、source time domain、BPM/profile 参数、rule ID、
rounding 和映射后的 chartTime，但这些字段只用于审计与回写，不参与 runtime 求值。无法唯一确定
source time 解释时，converter 必须要求 semantic profile 或失败，不能让 Core runtime 猜测。

### 8.2 Tempo map 和 chart beat

```fcs
tempoMap {
    0beat  -> 180bpm;
    64beat -> 200bpm;
}
```

每个左侧是必须在 static phase 求值为 `beat` 的 compile-time expression；右侧是只在 tempo
schema 中合法的 signed decimal `bpm` literal。数字与 `bpm` 之间不得有 trivia。空 block、负值、
零值、非有限值和非单调 key 在语法上可表示，以便 canonical validation 分别产生
`tempo.invalid` 或 `tempo.non-monotonic`；parser 不得提前拒绝。FCS 5.0 不存在
`[whole,numerator,denominator]beat` mixed-number syntax，精确 Beat 使用普通十进制 `beat` literal
和 compile-time rational arithmetic。

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
归一化到 chartTime，同时可以保留 exact beat provenance。FCS source 不提供 line-local beat、
line-local BPM 或 producer-specific tick literal；converter 若输出 FCS source，必须已经完成这些
来源坐标的解码。

### 8.3 判定时间与滚动坐标

Note 判定只由 canonical chartTime 决定。每条 line 可以有：

```text
lineScrollCoordinate_i = q_i(chartTime)
```

`q_i` 是 chartTime 的纯函数，用于滚动和 floor distance，不是独立 clock。Line 的 BPM、
RPE `bpmfactor` 或外部局部 tick 不得隐式改变 Note 判定时间，也不得以 fidelity/preserve 字段
反向覆盖已确定的 canonical time。对 canonical comparison，source beat provenance 不同但最终
chartTime、tempo 与其余语义相同的 Note 具有相同判定时间。

---

## 9. Track 模型

### 9.1 Track schema

```fcs
tracks {
  track slide -> positionX: length {
    blend: "replace";
    priority: 0;
    fill: "base";
    extrapolateBefore: "base";
    extrapolateAfter: "holdAfter";

    segments {
        [0s, 1s): 0px -> 100px using "easeInOutSine";
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
后统一为 chartTime。Direct segment 的 `using` 接受编译期 `string` expression，其值可以是
`"step"`、`"linear"` 或 Core easing 名；也可以直接写 `cubicBezier(x1,y1,x2,y2)`。
`cubicBezier(...)` 是 direct `using` 和 schema `interpolation` field 共享的 schema-only value，
不是可以赋给局部变量或传给普通函数的一等 expression。

`segments` collection 可以生成结构：

```fcs
segments {
    generate at: beat in 0beat..<8beat step 1beat {
        emit segment {
            start: at;
            end: at + 1beat;
            startValue: 0px;
            endValue: 20px;
            interpolation: "linear";
        };
    }
}
```

在 Track<T> 中，`segment` 构造 `TrackSegment<T>`，required field 是 start、end、startValue、
endValue、interpolation；`keyframe` 构造 `Keyframe<T>`，required field 是 time、value，等价于
direct `point`。类型 `T` 从 enclosing Track 固定，emit 其他类型是 collection mismatch。

Track setting 只允许 blend、priority、fill、extrapolateBefore、extrapolateAfter。Blend 默认
`"replace"`，priority 默认 0。Fill 默认随 blend 为 replace→base、add→zero、multiply→one；
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

Core interpolation 支持 step、linear、cubic Bezier 和附录 A easing。Bezier 的四个参数必须在
static phase 求值为有限 `float`，顺序为 `[x1,y1,x2,y2]`；`x1,x2` 必须在 `[0,1]`，曲线 x
必须可单值反解。Y 可以 overshoot，但 target schema 可以禁止超界。

Interpolation/type matrix 是 static semantics，不由 FCBC writer 补猜：

| Runtime property type | step | linear / Core easing / cubicBezier |
|---|---|---|
| bool、int | allowed | `type.mismatch` |
| float、time、beat、length、angle | allowed | allowed |
| color | allowed | allowed，在线性 RGBA 中逐分量 |
| vec2<float>、vec2<length> | allowed | allowed，逐分量使用同一 progress |

其他 compile-time-only `vec2<T>` 不能成为 Core runtime property target。Step 在 segment 内返回
start value；非 step 使用 `start + (end-start)*p'`，每个 subtraction、multiplication、addition 和
每个 component 都按第 14.1 节独立 roundTiesToEven，不允许 FMA 或结合律重排。

对 cubicBezier，先按第 14.1 节得到 binary64 `p`。把 p 和四个 binary64 control value 解释为精确
实数，取满足 cubic x(t)=p 的唯一 `t∈[0,1]`，再把实数 cubic y(t) 正确舍入一次为 binary64 `p'`。
这是规范结果；Newton iteration 次数、查表分辨率或宿主图形库近似都不能改变 bits。不能建立唯一
解或正确舍入 enclosure 时 strict-runtime validation/execution 失败，不能退化为固定采样曲线。

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

`scrollBpm_i(t)` 只能依赖 canonical `s`、global `b` 和常量，不能依赖该 Line 的 `q` 或任何 Note
distance `d`。`q_i` 正是由 `scrollBpm_i` 积分得到；允许 `scrollTempo -> q` 会把定义变成无初始
固定点规则的跨语义自循环。Compiler 必须在 exact lowering 前拒绝任何直接或 transitive
`scrollTempo -> EnvQ/EnvD` dependency。

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

`scrollSpeed` 是相对于 q 的无量纲 multiplier，可以依赖 `s/b/q` 和常量，但不能依赖 Note `d`：

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

- `portable-analytic`：所选 Execution ABI 明确定义结构可验证、bit-exact direct formula 的
  descriptor 组合；常量是基础候选，线性、已知 easing 等只有该 ABI 也定义相应公式时才属于本类；
- `portable-evaluable`：保存 exact integrand/Expression DAG 和规范化 integration boundary，由
  reference/optimized evaluator 按第 14 章数值要求查询；
- `explicit-approximation`：仅用于外部 target capability 不足且用户明确允许 approximation 的
  转换结果，必须同时携带 velocity、distance、domain 和 max error；
- `runtime-only-extension`：需要独立 required feature/ABI，不能冒充 Core portable。

标准 FCS→CanonicalChart→FCBC 路径必须选择 `portable-analytic` 或 `portable-evaluable`，不能因为
积分没有简单闭式就预先采样 floorPosition。Canonical scroll semantics 是上式定义的连续积分；
runtime 数值算法可以近似计算该数学值，但不得把实现采样点升级为谱面语义或写回标准 FCBC 的
BakedCurve。

分类选择不得改变数学语义。一个 Core integrand 即使在数学上存在闭式，只要目标 ABI 没有定义
loader 可验证的 analytic contract，就必须精确降为 `portable-evaluable`，这不构成 approximation 或
fidelity loss。Execution ABI 1.0 当前只把 constant scrollTempo×constant scrollSpeed 组合列为
portable-analytic；其他 Core exact combination 使用 portable-evaluable。

Core 禁止把固定 120Hz、1000Hz 或渲染帧累加作为 floorPosition 真相。Seek 必须直接查询
descriptor 或确定性积分，不依赖此前渲染帧历史。播放器本地 sampled cache 若存在，完全属于
实现配置，不改变该分类、CanonicalChart、FCBC bytes 或 strict exact conformance。

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
inherit.position: true;
inherit.rotation: true;
inherit.scale: true;
inherit.alpha: true;
inherit.scroll: false;
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
    gameplay.side: "above";
    gameplay.judgment.enabled: true;
    gameplay.judgeShape.kind: "lineDefault";
    gameplay.soundPolicy: "default";
    gameplay.scorePolicy: "default";
}
```

Source required：line、time；`id` 可选。缺失 ID 时 compiler 按第 17 章使用规范化 source path、
template call path、generator index 和 document order 生成 canonical stable ID。默认 side=`"above"`
且 `judgment.enabled=true`。当 judgment enabled 时，默认 judge shape、sound 和 score policy 分别为
`{"kind":"lineDefault"}`、`"default"`、`"default"`；当 judgment disabled 时，默认 sound/score
policy 都是 `"none"`。

这些直接枚举值遵循第 2.10 节的 string spelling。Gameplay kind、time、endTime、side、judgment、
judgeShape、soundPolicy、soundResource、scorePolicy、scoreExtension 和 line 必须在 canonical
lowering 前确定，不能依赖 runtime expression、Render 或输入设备状态。

`gameplay.judgeShape` 是 closed typed object。Core 5.0 支持：

| `kind` | Field | Canonical 语义 |
|---|---|---|
| `"lineDefault"` | 不允许额外 geometry field | 不增加空间 hitbox 约束；只保留所属 Line、Note kind、side 和时间/gesture intent |
| `"rectangle"` | `center: vec2<length>`，默认 `(0px,0px)`；`halfExtents: vec2<length>` required | line-local、轴对齐闭矩形，两个 half extent 必须有限且大于 0px |
| `"circle"` | `center: vec2<length>`，默认 `(0px,0px)`；`radius: length` required | line-local 闭圆，radius 必须有限且大于 0px |

Rectangle/circle geometry 在 Note `gameplay.time` 使用所属 Line 的 gameplay world transform；它不
读取 `presentation.positionX`、alpha、scale、texture 或 Render geometry。`lineDefault` 是精确的
“无附加空间约束”descriptor，不允许播放器为 canonical comparison 猜测隐藏 width/radius。
Input sampling、timing window、手势识别和同一输入如何匹配多个候选 Note 属于 gameplay host
policy，不属于 FCS Core chart semantics；Core reference API 返回 canonical judgment descriptor，
而不是依赖设备的最终命中结果。

`gameplay.soundPolicy` 是：

- `"default"`：在成功 judgment intent 上发出按 Note kind 区分的 host default-hit-sound intent；
- `"none"`：不发出 Note hit-sound intent；
- `"resource"`：发出 `gameplay.soundResource` 指向的 audio resource，且该 field 必须存在。

只有 `"resource"` 允许 `gameplay.soundResource`；引用必须静态确定并指向 audio。`"default"` 是
runtime 内建 policy intent，不引用 chart resource；要求 byte-exact 自定义声音时必须使用
`"resource"`，其 bytes 随 FCBC 内嵌。Hold 的 Core sound intent 只在 head judgment 发出；需要
tail/continuous sound 的格式必须使用声明的 required gameplay extension。

`gameplay.scorePolicy` 是：

- `"default"`：发出一个 standard score-eligible judgment intent；
- `"none"`：不发出 score intent；
- `"custom"`：把 judgment intent 交给 `gameplay.scoreExtension` 指定的 required extension。

只有 `"custom"` 允许且要求非空 `gameplay.scoreExtension` namespace；该 namespace 必须在文档
`extensions` 中以 `required` 声明。FCS Core 不规定设备 timing window、全局计分公式、rank 或
combo 权重；它规范化的是 Note 是否可判定、是否产生 score/sound intent 以及 extension dispatch。
Canonical comparison 必须比较这些 policy descriptor，不能用某个播放器的最终分数代替。

若 `gameplay.judgment.enabled=false`，Note 不接受 input judgment，也不产生 Core score/sound
intent；source 必须省略 sound/score policy 或显式写 `"none"`，并且不得设置 soundResource 或
scoreExtension。Converter 的 fake/非判定 Note 映射到该状态，不能通过 alpha、visibility 或
texture 推断。

缺失 shape/resource/extension 所需 field 使用 `schema.missing-required-field`；禁止的 field 组合或
非正 geometry 使用 `schema.non-constructible`；soundResource 类型错误使用
`resource.type-mismatch`；custom score namespace 未声明或不是 required extension 使用
`extension.unsupported-required`。Parser 必须接受语法正确的组合并把这些检查留给
static/canonical phase。

Hold 必须有 `endTime` 且 `endTime > time`；非 Hold 不得设置 endTime。Time 可以用 beat 或
time source value，canonical 后必须是 chartTime。Judge shape 在 line-local gameplay 坐标中
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
texture      null
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
scrollFactor。Line transform 和 scrollSpeed 可以依赖 `s/b/q` 但不得依赖 Note `d`；scrollTempo
只能依赖 `s/b`，不得依赖 `q/d`。

Canonical Note 仍有一个 runtime `presentation.visibility: bool` property，供 FCBC/Render 查询；它
不是可由 source 直接赋值的第二套 field。Compiler 将 `visibleFrom`/`visibleUntil` 的 compile-time
interval 精确 lowering 为该 property 的 Constant 或 Piecewise descriptor：在半开区间
`[visibleFrom, visibleUntil)` 内为 `true`，区间外为 `false`，缺失 boundary 表示对应方向无界。
该 descriptor 只依赖 canonical chartTime `s`，不依赖 `b/q/d/p`；因此 visibility boundary 不会被
Note distance 或渲染采样改变。

`presentation.texture` 是 null 或静态 image/texture resource reference；resource identity 不能
动态变化。缺失/null 表示使用 Render/game host 对该 Note kind 的默认 visual，不产生外部资源
依赖。`presentation.positionX` 和其他 visual property 不参与 judgment geometry；若谱面需要显式
空间 hitbox，必须使用 `gameplay.judgeShape.center`/geometry，而不能依靠视觉位置反推 gameplay。

### 12.4 Position 和 Hold

```text
sideSign = gameplay.side == "above" ? +1 : -1
d = sideSign
    * (floor(noteTime)-floor(now))
    * floorScale
    * scrollFactor

localX = positionX + xOffset
localY = d + yOffset
```

Runtime 环境变量 `d` 精确等于上式、不包含 `yOffset`。Above/below 由 sideSign 表达，不通过负
alpha 或 texture flip 表达。Hold head 位于 time，tail 位于 endTime，body 覆盖
`[time,endTime]` 的几何连接；Core judgment descriptor 与 gameplay host policy 都不得依赖渲染
采样。动态 presentation 可以按 `d` 求值。

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

Core 不定义多义 `t` 别名。`p` 只在一个 Track segment/Canonical Piece 已经选中、并且该 Piece
正在求值其 inner descriptor 时存在；它等于该 Piece 的 normalized progress。普通 direct-root
查询没有隐式 `p`。Compiler 必须把 source 中依赖 `p` 的表达式放入相应 Segment/Piece context；
没有此 context 的 `p` 依赖是 static/capability error，不能让 runtime 以 `0` 或上一帧值猜测。
环境可用性由 target schema 限制：scrollTempo 只能依赖 `s/b` 和常量，不能依赖 `q/d`；scrollSpeed
只能依赖 `s/b/q` 和常量，不能依赖 Note `d`、Render 或外部输入。Gameplay 结构 field 不允许
runtime expression。共享 descriptor 必须满足所有 direct/transitive target schema 的环境交集，不能
因另一个 owner 允许更多环境而放宽限制。

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
typed Expression DAG
```

当 exact lowering 选择 Piecewise/SegmentTrack 时，selected Piece 向其 inner descriptor 传递上述
`p`；嵌套 Piece 以最内层选中的 Piece 重新绑定 `p`。离开 Piece context 后 `p` 不会泄漏到 direct
root、Line/Note/Render attachment 或另一个 sibling descriptor。

该列表只选择语义等价的 exact representation，不是逐级降低精度的 fallback。任何合法 Core runtime
expression 都必须能够保留为 typed Expression DAG；表达式不能静态化为 Track/Piecewise 不是错误，
更不是默认 baking 条件。标准 FCS→CanonicalChart→FCBC 编译不得因为性能猜测、目标设备或节点数
把 exact expression 改写为 BakedCurve，也不接受谱师提供的通用 sample rate。

Compiler 可以进行 constant folding、公共子表达式共享、descriptor interning 和可证明等价的
代数优化，但必须保持第 14 章规定的逐 node binary64 bits、短路/`choose` 行为、error timing 和
离散边界。若某个 required extension expression 无法由所选 ABI 精确表示，标准编译必须报告
capability/version error；不得静默采样后宣称 exact conformance。

FCBC Core 表达式是 typed DAG/descriptor，不包含 jump、store、loop、recursive call、emit 或
generate。Portable expression 不得依赖 IO、未固定随机数、wall clock 或宿主未定义行为。

每个 DAG node 在 binary64 roundTiesToEven 下独立舍入；不得使用未声明 FMA、扩展精度寄存器
或重排结合律。`+0/-0` 按 IEEE 754 运算保留，NaN/Infinity 在产生该 node 时立即成为 execution
error。Compiler constant folding 必须产生与相同 node 求值完全一致的 bits。

---

## 14. 精度、显式近似和 Easing

### 14.1 数值模型

Canonical runtime time、position、angle、speed、distance、Bezier、easing 和 transform 使用
Float64。Source/canonical 尽量保留 decimal 和 rational beat。非有限结果无效。

Core `sqrt/exp/ln/sin/cos/tan/asin/acos/atan/atan2/pow` 的结果定义为对应实函数正确舍入到
binary64 roundTiesToEven；argument reduction 也是该定义的一部分。实现不能直接继承宿主
`libm` 的未声明误差。Conformance vectors 提供困难输入的期望 bits；不能正确舍入的实现不满足
strict-runtime ABI 1.0。

非 transcendental scalar operation 每个 source/ABI node 也独立舍入，禁止隐式 FMA、扩展精度和
重排。涉及 IEEE signed zero 时固定：

- `abs` 清除 sign bit，因而 `abs(-0.0)=+0.0`；
- `min(a,b)`/`max(a,b)` 先作规范比较；若两者数值相等（包括 `+0.0/-0.0`）返回第一个 operand 的
  原始 bits；
- `clamp(x,lo,hi)` 依次检查 `lo<=hi`、`x<lo`、`x>hi`，否则返回 x 原始 bits，不用宿主
  `fmin/fmax` 替换；
- `floor`、`ceil` 和 ties-to-even `round` 使用 IEEE 754 对应 round-to-integral operation，有限输入
  得到零时保留输入 zero sign；
- equality 把 `+0.0` 与 `-0.0` 视为相等；ConstantPool、constant folding 和 deterministic bytes
  仍逐 bit 区分两者。

Int arithmetic 使用 checked i64；overflow、除零、`i64::MIN/-1` 和负 int exponent 是该 node 的
invalid-value error；int `0**0` 固定返回 1。Int→binary64 使用正确舍入 roundTiesToEven。FCBC opcode 的完整 type/arity 表
由 `fcbc.md` 的独立 ABI version 定义，但不得改变本章的 source runtime bits 和 error timing。

Float special case 也属于规范结果：`sqrt(-0.0)=-0.0`，其他负输入是 invalid domain。
`atan2(y,x)` 使用完整 signed-zero axis rule：y 为 `±0` 且 x 为正数或 `+0` 时返回同 sign 的
`±0`；y 为 `±0` 且 x 为负数或 `-0` 时返回同 sign 的 `±π`；x 为任一 zero 且 y 非零时返回与 y
同 sign 的 `±π/2`；其他 finite pair 使用主值区间 `[-π,π]` 的对应实函数。Float `pow(0,0)=1`；零
base 与负 exponent 是 invalid domain；负 base 只允许 exponent 是 binary64 中精确表示的整数，结果
sign 由整数奇偶决定；正 base 使用对应实函数。`-0` 的正整数 power 仅在奇 exponent 时保留负号。
`sin(-0)`、`tan(-0)`、`asin(-0)` 和 `atan(-0)` 均保留 `-0`；对应 `+0` 返回 `+0`，
`cos(±0)=exp(±0)=1`。所有分支最终仍要求 finite、正确舍入的 binary64 result。

属性分类：

```text
Exact Constant
Exact SegmentTrack
Exact Piecewise
Exact Expression DAG
Explicit Approximate BakedCurve
```

Tempo、Track、Note、Hold、visibility、Render active 和 emit 的所有离散时间点都是强制精确
边界，不得量化到采样网格。前四类属于标准 exact CanonicalChart；`Explicit Approximate
BakedCurve` 不属于标准 FCS→FCBC 编译的 fallback。

### 14.2 显式 approximation 与播放器本地 sampled mode

只有外部 target format/runtime 无法执行 exact descriptor、用户明确允许 approximation，且转换
策略与 target profile 都声明该能力时，converter 才能生成 BakedCurve。该操作产生一个有损目标
表示和机器可读 ConversionReport；它不修改 FCS source、CanonicalChart 或标准发行 FCBC，也不
允许 packager 把 sample rate 写入 chart metadata、workspace manifest 或 FCBC section。

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
wall-time budget 内满足声明误差时，显式 strict approximation 转换失败；标准 exact 编译不会因
该失败改走另一条近似路径。

播放器可以从 exact FCBC 在本地建立默认关闭的 sampled cache，但该缓存是设备/播放器实现配置：

- 不得覆盖、重写或替代分发 FCBC；
- 不得写回 FCS、CanonicalChart、FCBC、packager output 或单谱面 metadata；
- cache miss/失效时必须能够回退 exact evaluator；
- cache key 至少绑定 FCBC content hash、Execution ABI、evaluator version 和全部采样参数；
- 必须显式保留 Note/Hold/tempo/Track point/visibility/Render active 等离散边界；
- sampled playback 不能宣称 strict exact execution conformance。

具体 sample rate、插值、容量、淘汰和持久化格式不属于 FCS Core。播放器可以完全不实现 sampled
mode；谱师和标准 compiler/packager 不需要提供设备性能目标。

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

上述公式是逐操作 binary64 算法，不是“把整式/实函数高精度算完后只舍入一次”：decimal 常量和
π 先按 roundTiesToEven 转成 binary64；先检查 x=0/x=1 端点钉扎；随后严格按括号、通常运算优先级
和从左到右的 operand 顺序执行，每个 `+ - * /`、comparison 和 intermediate result 都独立
roundTiesToEven，`sqrt/sin/cos/pow` 使用第 14.1 节正确舍入结果。Out/InOut/Bounce 的变换也按同一
规则逐步求值并把上一步 binary64 结果传给下一步。不得重结合、使用未声明 FMA、把 easing 整体
替换为宿主近似函数，或以 LUT/sample rate 改变 expected bits。

---

## 15. Extension、Fidelity 和 Repair

### 15.1 Extension

Extension 使用唯一的 Core envelope：

```fcs
extensions {
    extension("org.example.chart-effects", 1.0.0) optional {
        "quality": "portable",
        "strength": 0.5,
    }
}
```

一个文档最多有一个 `extensions` block。每个 declaration 包含全球唯一、非空的 namespace
string、三段 schema version、`required` 或 `optional` requirement，以及一个 ordered typed object
payload。同一 namespace 在文档中最多声明一次。Namespace 必须是 lowercase ASCII、以 `.` 分隔
的非空 component；component 只能含 `a-z0-9-`，必须以字母或数字开头/结尾，建议使用
reverse-DNS。比较是 byte-exact，不执行 normalization 或大小写折叠。

Namespace schema 只能约束 object key、value type、required field、默认值和语义，不能增加 Core
lexer token、delimiter 或任意私有 source grammar。这样未知 optional extension 仍可由 Core parser
完整保存为有序 source AST，并在不执行其语义时重新序列化。未知 required extension 必须在
static/canonical validation 以 `extension.unsupported-required` 拒绝 compile/play；未知 optional
extension 可以保留并忽略执行，但 capability/provenance report 必须记录未执行状态。

### 15.2 Preserve

```fcs
preserve {
    source {
        format: "rpe";
        version: "170";
        hash: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    }
    payload: extension("org.phigros.rpe", 1.0.0) {
        "encoding": "base64",
        "raw": "",
    };
}
```

Preserve 数据不自动改变 Core 语义。Raw snapshot 只保证 source 未被语义编辑时可精确回写。
来源状态至少区分 unset、explicit-default、explicit-value、inherited、imported、generated 和
user-modified；不得用“值等于默认值”猜测来源状态。

`preserve` 最多一个；其中 `source` schema block 和 `payload` 必须各出现一次，顺序无语义。
`payload` 复用 extension namespace/version header 和 ordered typed object，但没有
required/optional requirement，因为 preserve payload 从不获得执行能力。需要 byte-exact snapshot
时，payload 必须显式声明 encoding 并将 bytes 编码为 string；Core 不提供隐式原始 token blob。

`preserve` 是 authoring workspace 中的回写辅助数据。ExpandedSourceDocument 可以保留它，converter
也可以用它恢复未编辑的外部 source；CanonicalChart 不得包含 preserve raw payload。标准 FCBC
同样不得保存 FCS/external source snapshot、编码后的 raw source、source AST 或足以恢复
comment/template/generator/局部变量的等价数据。

Compiler 可以从 preserve 中提取非原文的 provenance fact，例如 source format/profile ID、输入
content hash、mapping rule 和工具版本，并按 container profile 写入 DistributionMetadata；这些 fact
不得包含 raw source bytes，也不能在 runtime 反向覆盖 Core semantics。需要继续制谱或 byte-exact
round-trip 的用户必须保留原始 workspace，而不是把 FCBC 当作 authoring backup。

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
- Hold `endTime <= time`，或 Note judge/sound/score policy 组合无效；
- resource workspace path、reference、type、declared hash 或 profile requirement 无效；
- runtime expression 循环依赖或 dynamic gameplay structure；
- 不支持的 required extension 或版本 major。

显式 approximation workflow 无法满足 error budget 时使用
`baking.error-budget-unsatisfied`，但它不是标准 exact FCS compile 的 fallback error。播放器本地
sampled cache 的失败不得使 chart invalid，必须回退 exact evaluator。

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
→ resolve workspace resources and verify declared hashes
→ normalize beat/time and stable IDs
→ validate tempo/Track/Line/Note/resource/extension graphs
→ exact-lower runtime properties
→ build CanonicalCompilation
→ hand off to the separately versioned FCBC writer
```

`CanonicalCompilation` 固定分为：

```text
CanonicalChart
CanonicalResourceBundle
DistributionMetadata
```

CanonicalChart 至少包含：source version、profile/features、tempo map、metadata、contributors、
credits、logical resources、sync、stable Line/Note/Track、exact runtime property descriptor、required
execution extensions，以及会改变 canonical semantics 的 repair decision。它不得包含 source
comment/trivia、template、function/generator 声明或调用、`emit`、局部变量、statement `if`、
parser-only token、workspace path、raw preserve payload 或 source snapshot。

CanonicalResourceBundle 必须为每个 canonical resource ID 保存声明 kind/metadata、计算得到的
content hash 和 workspace 输入文件的原始 bytes。标准编译不得删除未被当前 renderer 直接引用但
仍由合法 resource declaration、metadata、artwork 或 extension 拥有的资源，也不得通过媒体转码
改变 payload。Resource ID 决定引用 identity；content hash 用于完整性和 deterministic output，
不能自动合并两个语义上不同的 ID。

DistributionMetadata 可以包含不会改变执行结果的 provenance、ConversionReport、repair record、
compiler ID/version 和输入 content hash，但不得包含 FCS 文本、外部 source raw snapshot、source
AST 或 authoring-only expansion graph。Container profile 可以省略 optional distribution metadata；
省略不得改变 CanonicalChart 或资源 payload。

Runtime property lowering 只允许 exact Constant、SegmentTrack、Piecewise 和 typed Expression DAG。
选择不同 descriptor sharing、constant folding 或等价 partition 只有在逐 node 数值、错误行为、
离散边界和所有查询结果保持规范等价时才合法。标准 FCS compiler/FCBC writer 不生成 BakedCurve；
第 14.2 节的 BakedCurve 只属于显式有损 target conversion。

Canonical ID 由显式 ID 优先；缺失允许 ID 的 schema 使用规范化 source path 和 document order
生成稳定 ID。显式 ID 与生成 ID 的 namespace 分离，避免碰撞。Publishable Note/Line/Resource
必须最终拥有稳定 ID。

这里的“规范化 source path”是 AST 内的声明/expansion path，不是 workspace 文件系统绝对路径或
resource `source`。相同 source、resource bytes 和 compilation profile 位于不同宿主目录时，必须
得到相同 canonical IDs。

对于 Line 和 Note，生成 textual ID 的精确编码固定为：

```text
generated/<entity-kind>/<expansion-path>/order/<zero-based-decimal>
```

其中 `<entity-kind>` 当前只能是小写 ASCII `line` 或 `note`；`<expansion-path>` 依次包含：

```text
collection/<collection-name>/item/<item-order>
[/template/<template-name>/call/<call-order>]
[/generate/<generator-index>]
```

`collection-name` 和 `template-name` 使用其 source identifier 的 ASCII spelling，不做大小写、Unicode
或其他规范化；因此它们不能包含 `/`。所有 order/index 组件都是零基、无前导零的十进制整数。
`item-order` 是 collection 中 source item 的零基顺序；`call-order` 是同一次 deterministic expansion
traversal 中 template call 的零基顺序；`generator-index` 是该 generator 的零基 iteration index。
最终 `order` 是实体完成 template/generator expansion 后在其 owning collection 中的零基 expanded-output
顺序。条件分支只贡献被选中的输出，但不重编号 source item；同一 canonical input 的不同宿主目录、
comment、trivia 或 authoring-only local name 不得改变这些 ID。

显式 source ID 保留原始 UTF-8 字节，不得以 `generated/` 开头；该前缀由 compiler 生成 ID 保留，违反时
必须在 canonical ID validation 失败。Line 使用 `fcs.line`、Note 使用 `fcs.note` typed namespace，
并按 `fcbc.md` §6.2 对最终 textual ID 计算 `SHA-256(namespace || 0x00 || UTF-8(textual ID))` 的前
64 little-endian bits。ID 为 0、重复 textual ID 或同一 typed namespace 中的任意 64-bit collision 都是
错误；compiler 不得加盐、重命名或依赖 map/host traversal order 恢复。

标准 FCBC writer 必须消费整个 CanonicalCompilation，把恰好一个 CanonicalChart 和全部
CanonicalResourceBundle payload 写入一个自包含 FCBC。播放器只从 FCBC loader 得到已验证的
runtime descriptors；它不重新解析 FCS、解析 workspace path、执行 authoring structure 或决定
是否在分发时 baking。

**Canonical semantic equivalence** 至少比较 CanonicalChart 的全部规范字段、required extension
identity、resource ID→kind/content-hash 映射和会改变 execution/Render 的 extension payload；资源
payload bytes 由对应 hash 绑定。Comment、trivia、未进入输出的局部名称、source span、workspace
absolute path、ConversionReport、Fidelity 和其他 non-semantic provenance 不参与该关系；
template/generator 组织只有在展开结果、stable ID 与规范性顺序也相同时才等价。Conversion profile
自动选择与 target reparse 使用该关系，不能因为候选 profile 产生不同的审计记录就误判语义不同。

**Distribution equivalence** 在 canonical semantic equivalence 之上，还要求 CanonicalResourceBundle
原始 bytes、规范性 DistributionMetadata、container profile/feature 与所选 optional distribution
section 一致。它用于 deterministic writer/build artifact 比较，不得反向改变 canonical semantics。

---

## 18. Conformance 测试

规范 fixture 必须覆盖：

1. 版本、编码、词法、类型和单位；
2. const/fn/template/generate、作用域、环、预算和 authoring-only 结构完全消失；
3. 两种 authoring 组织在显式 identity 相同时产生 canonical-semantic-equivalent 结果；
4. tempo、time/beat、audio offset、seek 和外部 source time base 不进入 runtime；
5. Track 区间、blend、gap、easing、speed、continuous distance 和 exact integration descriptor；
6. 坐标、矩阵、父线 DAG、Note/Hold 和完整 judge/sound/score policy；
7. runtime expression、Piecewise、Expression DAG，以及标准 compile 不产生 BakedCurve；
8. workspace-relative resource resolution、path escape、declared hash、opaque payload 与 stable ID；
9. metadata、credits、sync、custom、provenance 和 preserve/source-snapshot 消除边界；
10. source→expanded→canonical compilation→FCBC→reference execution 闭环；
11. PGR/RPE/PEC semantic profile round-trip 与 approximation report；
12. parser/compiler/loader fuzz、property tests 和资源上限。

每个重要规范示例必须对应可执行 fixture，记录预期成功、diagnostic category、canonical snapshot
或数值 test vector。Reference evaluator 至少公开：

2026-07-15 Source grammar closure 的原始 baseline 含 32 个 manifest entry；后续 corpus 增长时仍必须
保留完整顶级 envelope、escaped NUL、header whitespace/leading-zero、duplicate block、nested/
misplaced generator、unclosed extension payload、mixed-Beat rejection 和 unresolved bare schema
enum 的独立 fixture，不得删除这些边界来迁就 parser。

Authoring/canonical closure 还必须绑定以下可执行事实：

- template/generator/preserve source 与直接 concrete source 的 canonical comparison；
- runtime transcendental/`choose` expression lowering 为 Expression DAG 而不是 BakedCurve；
- `judgment.enabled=false` 的 sound/score normalization，以及 resource/custom policy 的合法和非法
  组合；
- workspace logical path 合法化、escape rejection、SHA-256 mismatch 与原始 payload hash；
- CanonicalChart/FCBC snapshot 中不存在 source text、workspace path、template/generator/local 或
  raw preserve payload。

```text
lineTransform(lineId, chartTime)
lineScrollCoordinate(lineId, chartTime)
lineScrollDistance(lineId, chartTime)
notePresentation(noteId, chartTime)
noteJudgmentDescriptor(noteId)
```

必须验证 seek 与顺序求值等价、编译期结构完全消失、source-time provenance 不改变 runtime、
standard FCBC 无通用控制流/BakedCurve/source snapshot/external resource dependency、同输入
deterministic。

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
            gameplay.side: "above";
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

以下 EBNF 定义完整 Core source syntax。具体 schema 允许的 field、field type、required/default
规则仍由正文决定，属于 static/canonical validation。除 `header` 行、`semver` 内部、numeric 与
unit suffix 的相邻关系以及 string/color token 内部外，`trivia` 可以出现在任意 token 之间，此处
省略。

```text
document       = bom?, header, formatBlock, topLevelBlock* ;
header         = "#fcs", asciiSpace, semver, newline ;
semver         = uintMagnitude, ".", uintMagnitude, ".", uintMagnitude ;

topLevelBlock  = metaBlock | contributorsBlock | creditsBlock | resourcesBlock
               | artworkBlock | syncBlock | definitionsBlock | tempoMapBlock
               | linesBlock | collectionsBlock | renderBlock
               | extensionsBlock | preserveBlock ;

formatBlock    = "format", "{", formatField*, "}" ;
formatField    = ("profile", ":", profile
               | "features", ":", featureArray), ";" ;
profile        = "fragment" | "chart" | "playable" | "renderable" | "publishable" ;
featureArray   = "[", (profileFeature, (",", profileFeature)*, ","?)?, "]" ;
profileFeature = "playable" | "renderable" ;

metaBlock         = "meta", schemaBlock ;
contributorsBlock = "contributors", "{", contributorDecl*, "}" ;
contributorDecl    = "person", identifier, schemaBlock ;
creditsBlock       = "credits", "{", creditDecl*, "}" ;
creditDecl         = "credit", schemaBlock ;
resourcesBlock     = "resources", "{", resourceDecl*, "}" ;
resourceDecl       = resourceKind, identifier, schemaBlock ;
resourceKind       = "audio" | "image" | "font" | "texture"
                   | "path" | "shader" | "binary" ;
artworkBlock       = "artwork", schemaBlock ;
syncBlock          = "sync", schemaBlock ;

tempoMapBlock = "tempoMap", "{", tempoPoint*, "}" ;
tempoPoint     = expression, "->", bpmLiteral, ";" ;
bpmLiteral     = "-"?, numberLiteral, "bpm" ;

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
collection       = collectionName, "{", collectionItem*, "}" ;
collectionName   = identifier | "notes" | "judgelines" ;
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

entityExpression = entityPrimary, ("with", schemaBlock)* ;
entityPrimary    = entityConstructor
                 | identifier, "(", arguments?, ")" ;
entityConstructor = noteVariant, entityBlock
                  | "Line", entityBlock
                  | "RenderNode", entityBlock
                  | "segment", schemaBlock
                  | "keyframe", schemaBlock ;
noteVariant      = "tap" | "hold" | "flick" | "drag" ;
schemaBlock      = "{", schemaField*, "}" ;
entityBlock      = "{", (schemaField | tracksBlock | scrollTempoMapBlock)*, "}" ;
schemaField      = fieldPath, ":", schemaValue, ";" ;
schemaValue      = halfOpenInterval | cubicBezierValue | expression ;
fieldPath        = fieldName, (".", fieldName)* ;
fieldName        = identifier | keyword ;

tracksBlock      = "tracks", "{", trackDecl*, "}" ;
trackDecl        = "track", identifier, "->", fieldPath, ":", type,
                   "{", trackSetting*, segmentsBlock, "}" ;
trackSetting     = identifier, ":", expression, ";" ;
segmentsBlock    = "segments", "{", segmentItem*, "}" ;
segmentItem      = directSegment | directPoint | generator | segmentIf ;
segmentIf        = "if", expression, "{", segmentItem*, "}",
                   ("else", "{", segmentItem*, "}")? ;
directSegment    = halfOpenInterval, ":", expression, "->", expression,
                   "using", interpolation, ";" ;
directPoint      = "point", expression, ":", expression, ";" ;
halfOpenInterval = "[", expression, ",", expression, ")" ;
interpolation    = cubicBezierValue | expression ;
cubicBezierValue = "cubicBezier", "(", expression, ",", expression,
                   ",", expression, ",", expression, ")" ;
scrollTempoMapBlock = "scrollTempoMap", "{", scrollTempoPoint*, "}" ;
scrollTempoPoint = expression, "->", bpmLiteral, ";" ;

extensionsBlock     = "extensions", "{", extensionDecl*, "}" ;
extensionDecl       = extensionHeader, extensionRequirement, object ;
extensionHeader     = "extension", "(", stringLiteral, ",", semver, ")" ;
extensionRequirement = "required" | "optional" ;

preserveBlock       = "preserve", "{", preserveItem*, "}" ;
preserveItem        = preserveSource | preservePayload ;
preserveSource      = "source", schemaBlock ;
preservePayload     = "payload", ":", extensionHeader, object, ";" ;

renderBlock          = "render", "profile", semver, balancedTokenBlock ;
balancedTokenBlock   = "{", balancedToken*, "}" ;
balancedToken        = nonDelimiterToken | balancedTokenBlock
                     | balancedParenGroup | balancedBracketGroup ;
balancedParenGroup   = "(", balancedToken*, ")" ;
balancedBracketGroup = "[", balancedToken*, "]" ;
nonDelimiterToken    = any Core token except "{", "}", "(", ")", "[" and "]" ;

expression      = logicalOr ;
logicalOr       = logicalAnd, ("||", logicalAnd)* ;
logicalAnd      = equality, ("&&", equality)* ;
equality        = ordering, (("==" | "!="), ordering)* ;
ordering        = sum, (("<" | "<=" | ">" | ">="), sum)* ;
sum             = product, (("+" | "-"), product)* ;
product         = power, (("*" | "/" | "%"), power)* ;
power           = unary, ("**", power)? ;
unary           = ("!" | "-"), unary | postfix ;
postfix         = primary, (("(", arguments?, ")") | (".", fieldName)
                | ("[", expression, "]"))* ;
primary         = literal | identifier | reference | array | object
                | vec2Constructor | chooseExpression | "(", expression, ")" ;
literal         = booleanLiteral | nullLiteral | numberLiteral | unitLiteral
                | stringLiteral | colorLiteral ;
booleanLiteral  = "true" | "false" ;
nullLiteral     = "null" ;
numberLiteral   = uintMagnitude | floatMagnitude ;
unitLiteral     = numberLiteral, unitSuffix ;
unitSuffix      = "ns" | "us" | "ms" | "s" | "min" | "beat"
                | "px" | "deg" | "rad" | "turn" ;
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

`bom`、`asciiSpace`、`newline`、`identifier`、`keyword`、`uintMagnitude`、`floatMagnitude`、
`stringLiteral` 和 `colorLiteral` 的 lexical grammar 见第 2 章。`semver` 的三段与点必须相邻，
并在与 decimal 重叠时优先识别完整三段 lexeme；`unitLiteral` 与 `bpmLiteral` 的 number/suffix
之间不得有 trivia。`fieldName` 中的 `keyword` 是第 2.4 节完整 Core keyword 集合，仅在
field-path context 生效。

Core parser 对 `renderBlock` 验证 version envelope、Core tokenization、三类 delimiter 的正确嵌套和
完整输入，并将 `balancedTokenBlock` 按顺序保存在 source AST；string/comment token 内的 delimiter
不参与平衡。其内部 grammar 与 semantic validation 由 `fcs-render.md` 定义。Extension payload
不再拥有 namespace 私有 grammar，只能使用本附录的 ordered `object`，因此未知 optional
extension 不会逃逸 Core lexer/parser limits。

Lexer 必须对 `..<` 和 `..=` 执行 longest match，单独 `..` 无 token。Parser 在禁止 generator
的 context 遇到可识别的 `generate` production 时，必须优先返回
`compile-time.nested-generator` 或 `compile-time.misplaced-generator`，而不是降级为普通
`syntax.invalid-token`。这两项和 duplicate top-level block 是第 1.4 节定义的 source-structure
检查。

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
resource.limit-exceeded
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

Decode/header/lex/grammar 失败使用最具体的 decode/version/syntax category。第 1.4 节明确的
source-structure 检查例外使用 `name.duplicate`、`compile-time.nested-generator` 或
`compile-time.misplaced-generator`；缺失必需的 `format.profile` 使用
`profile.requirement-missing`；整个 `format` block 缺失时也使用该 category，即使实现把这些检查
与 parser API 合并。Static/canonical 阶段
不得用通用 `syntax.invalid-token` 代替类型、schema、tempo、graph 或数值错误。Expansion trace
是 `compile-time.budget-exceeded` 和递归/cycle diagnostic 的结构化字段。

`resource.limit-exceeded` 用于实现公开的 decode/parser/compiler 输入资源限制，不表示 chart 中的
`resources` block 引用错误。它必须携带 limit kind、limit、observed count 和 source span；检查
必须发生在受限工作或分配之前。编译期语言第 6.8 节的六类共享 budget 仍使用
`compile-time.budget-exceeded`。

`resource.unknown-reference` 同时覆盖未知 resource ID，以及 workspace member path 缺失、逃逸、
指向目录或非普通文件；`resource.hash-mismatch` 只用于已经安全读取的普通文件 bytes 与声明 hash
不一致。第 12.2 节 Note policy 的禁止组合使用 `schema.non-constructible`，缺失条件 field 使用
`schema.missing-required-field`，不得降级为 parser syntax error。
