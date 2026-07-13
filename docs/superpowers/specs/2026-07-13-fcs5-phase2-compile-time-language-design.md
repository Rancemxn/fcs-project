# FCS 5 Phase 2 编译期语言设计

## 状态

已确认。本文定义 FCS 5 Phase 2 的编译期语言、construction schema 与 expansion 边界。它依赖已经完成的 Phase 1 front end，并在不实现 Phase 3 runtime chart semantics 的前提下交付 typed template、`generate` 和 `emit`。

## 目标

Phase 2 将 FCS 5 从最小 document front end 扩展为有限、强类型、纯函数式的编译期语言。它必须提供：

- 显式类型的 `const`、`let`、纯 `fn`；
- 基础值、单位值、`vec2<T>` 与静态表达式类型检查；
- 不可变作用域、禁止 shadowing、函数/常量/template 调用图环检测；
- construction schema 驱动的 typed entity template 与 `with`；
- collection 内有限 `generate`、`emit`、编译期 statement-level `if`；
- 可配置的 expansion budget 与包含调用链、generator index 的诊断；
- 不含任何编译期结构的 `ExpandedSourceDocument`，作为 Phase 3 canonical lowering 的唯一输入。

## 非目标

- 不计算 Note 判定、Hold 约束、line transform、scroll distance、track overlap 或父线图；这些是 Phase 3 的责任。
- 不构建 expression DAG、runtime `choose`、adaptive baking 或 reference evaluator；这些是 Phase 4 的责任。
- 不构建 FCBC、runtime loop、runtime local slot、递归调用或 `emit`/`generate` 指令。
- 不实现 RenderNode/Track 的运行时或 source schema；它们分别由 Phase 7 与 Phase 3 注册。
- 不迁移 v4 parser、converter 或 CLI 默认入口。

## 总体架构

Phase 2 分为四个内部层次，并要求各层单向依赖：

```text
source parser
    ↓
P2.1 typed language kernel
    ↓
P2.2 construction schema validation
    ↓
P2.3 template / generate expansion
    ↓
P2.4 ExpandedSourceDocument
    ↓
Phase 3 canonical semantic lowering
```

`ExpandedSourceDocument` 是 source-level、静态类型正确的构造结果，不是 runtime chart。它可以包含 Note 与 Line source entity，但不包含 track 求值、tempo-to-time 归一化、transform 或任何 FCBC 数据。

## P2.1：typed language kernel

### 类型

Phase 2 定义下列语言类型：

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
Note
Line
RenderNode
TrackSegment<T>
Keyframe<T>
```

其中 `time` 与 `beat` 是不同类型，不允许隐式转换。单位值按其语义进入 `time`、`beat`、`length` 或 `angle`；裸整数字面量为 `int`，裸小数字面量为 `float`。`vec2<T>` 仅在 `T` 为可构造纯值类型时有效。

实体类型不能参与 equality、排序、算术或隐式序列化。`RenderNode`、`TrackSegment<T>` 和 `Keyframe<T>` 在 Phase 2 中可作为类型符号出现在诊断与 schema API 中，但没有 constructible source schema；对它们的 constructor、template 实例化或 `emit` 必须报出“schema unavailable”错误。

### 值与表达式

语言 kernel 定义 `TypedValue`、`TypedExpression` 与 `Type`。每个 expression 在 type checking 后必须有唯一 `Type`，并将 source span 保留在节点上。

支持的静态运算规则：

- `int` 与 `float`：同类算术；混合运算仅允许显式 `toFloat(int)`；
- `time`、`beat`、`length`、`angle`：仅与同类型量做加减、比较；可与 `int`/`float` 标量相乘或相除；同类型非零量相除返回 `float`，以支持 `beat / beat` 等无量纲比值；
- `bool`：逻辑运算与 equality；
- `string`、`color`：仅 equality；
- `vec2<T>`：构造、同类型 equality、分量访问；
- comparison 两侧类型必须相同且实现对应比较；
- Float equality 是精确 equality；近似比较只能调用 `approxEq(value, expected, tolerance)`。

Phase 2 不引入 runtime 环境变量 `s/b/q/d/p` 或 runtime `choose`。任何依赖 runtime 值的表达式在 Phase 2 的 compile-time 位置必须得到诊断，而不是退化成未类型化节点。

内建 compile-time 常量 `pi: float` 与内建纯函数（包括 `sin`、`cos`、`approxEq`、`toFloat`）按固定签名参加类型检查与 operation budget 计数；它们不访问 runtime 状态。

### 绑定、作用域与函数

`const` 只能出现在顶层 `definitions`；`let` 只能出现在函数、template、generator 或 compile-time `if` 的局部作用域。两者都必须显式标注类型并初始化，且初始化值必须可在编译期求值。

作用域以声明时的符号表为准：同一作用域或任何嵌套作用域中同名声明均为错误，禁止 shadowing。不存在 `var`、赋值、`++`、`+=`、可变 collection 或全局可变状态。

`fn` 的每个参数与返回类型必须显式声明。函数体只允许纯 `let`、纯 compile-time `if`、`return` 和对其他纯函数的调用；不能调用 template、`generate` 或 `emit`。常量、函数与 template 各自建立有向依赖图；任意环立即报错，不以深度预算掩盖递归。

## P2.2：construction schema

### 责任边界

construction schema 是 source constructor 的静态契约，不是 runtime semantic model。它定义：

```text
entity type
constructible variants
field path
field type
required / optional
allowed collection
```

它不定义 field 的运行时含义、跨字段约束或时间求值规则。Phase 3 会将 `ExpandedSourceDocument` lowering 为 canonical chart，并处理 Hold `endTime > time`、line parent 图、track/transform/scroll 语义等规则。

schema 由 `ConstructionSchema` 注册表表示，核心接口概念为：

```text
EntitySchema { entity_type, variants, fields, allowed_collections }
FieldSchema { path, type, required }
CollectionSchema { collection_name, emitted_entity_type }
```

模板与 generator elaborator 接收不可变 `ConstructionSchema`；不存在“未知字段先保留、以后再检查”的回退模式。

### Phase 2 bootstrap schema

Phase 2 注册下列 constructible schema：

```text
notes      → Note
judgelines → Line
```

`Note` 的 source construction schema 包含以下稳定字段：

```text
variant                              : tap | hold | flick | drag
gameplay.time                        : beat       (required)
gameplay.endTime                     : beat       (optional)
gameplay.side                        : above | below
gameplay.judgment.enabled            : bool
render.enabled                       : bool
presentation.positionX               : length
presentation.scrollFactor            : float
presentation.xOffset                 : length
presentation.yOffset                 : length
presentation.alpha                   : float
presentation.scaleX                  : float
presentation.scaleY                  : float
presentation.color                   : color
presentation.texture                 : string
presentation.visibleFrom             : beat
presentation.visibleUntil            : beat
```

Phase 2 只检查字段名、值类型、重复字段和 required `gameplay.time`。例如 `gameplay.endTime` 是否只属于 Hold、范围之间的关系、判定/可见性/texture 的实际语义均由 Phase 3 或后续资源阶段验证。

`Line` 在 Phase 2 只允许 identity construction：

```text
id     : string (required)
zOrder : int
```

line transform、inheritance、track、scroll 与父线字段不属于 Phase 2 schema。`RenderNode`、`TrackSegment<T>`、`Keyframe<T>` 保留类型身份，但等其所属 Phase 注册 schema 后才可构造。

schema 扩展必须单调：后续 Phase 可以为自己的实体注册新 schema，或为已有实体注册新字段；不得改变 Phase 2 已注册 field path 的类型或 collection 归属。

## P2.3：template、with、generate 与 emit

### Typed template

模板语法保持：

```fcs
template Note ghostTap(hitTime: beat, x: length) {
    return tap {
        gameplay.time: hitTime;
        gameplay.judgment.enabled: false;
        presentation.positionX: x;
    };
}
```

每个 template 明确返回一个实体类型，且 return expression 必须是该实体类型的 constructor 或同类型 `with` 结果。Template 参数、局部 `let` 与 return value 都要经过 language kernel type checking。Template 调用只能引用 schema 已提供的 constructible entity type，调用图必须无环。

`with` 产生不可变的新实体：

```fcs
return base with {
    presentation.scaleX: 1.25;
    presentation.color: #FFAA00FF;
};
```

base 必须是 entity value；每个 path 必须在该 entity schema 中存在，赋值类型必须精确匹配，且同一 `with` block 不得重复修改同一 path。`with` 不会在运行时变更对象，也不会保留 prototype inheritance。

statement-level `if` 仅在 condition 为 compile-time `bool` 时可出现在 template 中。若条件影响 entity variant、field set 或 emit，任何 runtime-only value 都必须报错；动态 presentation 选择留给 Phase 4 的 runtime expression model。

### Generator

`generate` 只允许出现在注册 collection 中：

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

range start/end/step 必须类型相同、编译期可求值，step 不得为零。range 只支持 `int` 与 `beat`：

```text
start..<end  半开区间
start..=end  包含终点
```

每次迭代精确计算 `start + index × step`；`beat` 使用既有精确有理数 `Beat`，禁止通过浮点重复加法生成值。Generator scope 提供用户指定变量、`index: int`、`range.start`、`range.end`、`range.step`、`range.count`。

`emit` expression 的类型必须等于 collection schema 的 emitted entity type：`notes` 只能 emit `Note`，`judgelines` 只能 emit `Line`。Generator 不可嵌套、不可位于 `fn` 或 template 中、不可写外部状态、不可依赖 runtime-only value。

## P2.4：expansion、budgets 与输出 IR

每次 elaboration 使用不可变 `CompileTimeLimits`：

```text
maxExpansionDepth
maxGeneratedNodes
maxGeneratorIterations
maxTemplateInstances
maxCompileTimeOperations
maxExpressionNodes
```

budgets 用于限制已证明无环的合法展开链；环检测独立且优先执行。每个预算错误必须携带 expansion trace：函数/template 调用路径、当前 generator 的 collection、`index`、range 和待 emit 的实体类型。

成功输出：

```text
ExpandedSourceDocument
  source_version
  profile
  tempo_map
  collections: Vec<ExpandedCollection>

ExpandedCollection
  collection_kind
  entities: Vec<ExpandedEntity>

ExpandedEntity
  entity_type
  variant
  typed fields
  source provenance / span
```

此输出中不得存在 `const`、`let`、`fn`、template、`generate`、`emit`、statement-level `if`、range 或 generator index。它也不得包含 runtime bytecode、mutable local、loop、recursion 或 FCBC instruction。

## 模块边界

建议在 `crates/fcs-core/src/v5/` 中新增：

```text
ast/types.rs          Type、typed AST、source spans
ast/definitions.rs    definitions、bindings、functions、templates
ast/entity.rs         source constructors、with、collections、expanded IR
schema.rs              ConstructionSchema 与 Phase 2 bootstrap registry
parser/definitions.rs  definitions grammar
parser/entity.rs       constructors、collections、generate/emit grammar
parser/expression.rs   Phase 2 typed-expression source AST parser
elaborator/            scope、type-check、evaluator、cycle、expansion、budgets
```

parser 只生成带 span 的 source AST；elaborator 是唯一进行名称解析、type checking、schema validation、evaluation 和 expansion 的组件。`parser::parse_document` 在 Phase 2 后仍返回 source document；新的公开 `v5::elaborator::elaborate` 显式返回 `ExpandedSourceDocument` 或结构化 diagnostic。

## 诊断与测试

Phase 2 诊断至少区分：

```text
duplicate binding / shadowing
missing type annotation
type mismatch
unknown name
unknown entity field
field type mismatch
non-constructible entity type
wrong collection emit type
non-constant structural condition
recursive const / fn / template
invalid range / zero step
nested or misplaced generator
budget exceeded with expansion trace
```

测试必须从失败用例开始，并覆盖：

1. 值/单位/比较的合法与非法组合；
2. const/let/fn 作用域、shadowing 与函数环；
3. Note/Line constructor、`with`、未知 field 与 type mismatch；
4. template composition、template 环与 compile-time `if`；
5. int/beat range 的端点、方向、零 step 与精确 index 计算；
6. `emit` collection type mismatch、嵌套 generator 与各项 budget；
7. expanded output 中不存在全部 compile-time node；
8. 现有 Phase 1 public fixture 与 v4 API 不回归。

每个 Phase 2 行为提交前运行 workspace Clippy、nextest 与 rustfmt；Phase 2 完成时新增公开 FCS 5 fixture，至少覆盖 definitions、template `with` 与 beat generator。

## 完成条件

Phase 2 完成时：

- `fcs_core::v5` 可解析并 elaborate definitions、typed templates 和有限 generators；
- construction schema 对 Phase 2 constructible entity 的 field path、类型与 collection 归属完全生效；
- 展开后不保留任何 compile-time language node；
- 所有环、预算和结构性 runtime 条件都得到确定性诊断；
- 不引入 runtime mutable local、jump、loop、recursion、`generate` 或 `emit`；
- 新旧测试、workspace Clippy、nextest 与 rustfmt 全部通过；
- Phase 3 可以只接收 `ExpandedSourceDocument`，无需重新处理宏、纯函数或 generator 语义。
