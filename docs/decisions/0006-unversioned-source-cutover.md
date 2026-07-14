# 0006：归档 FCS 4 并将无版本前缀的 source crate 作为唯一主线

状态：Accepted

日期：2026-07-14

## 1. 背景

FCS 尚未对外发布，没有已发布 API、格式或下游使用者需要迁移。当前 workspace 同时包含：

- 根模块中的 FCS 4 AST、parser、compiler、旧 bytecode、VM 和 diagnostics；
- `crates/fcs-core/src/v5/` 中尚未完成的 FCS 5 source front end；
- 直接绑定 FCS 4 AST 的 `fcs-cli` 和 `fcs-converter`；
- 已冻结的 FCS Core 5.0.0、FCBC 2.0.0、Execution ABI 1.0.0、Render Profile
  1.0.0 和 Conversion Specification 1.0.0；
- 尚未提交的 generator AST/parser/test 工作与冻结文档工作。

继续保留两套默认语义会迫使 parser、converter、CLI 和后续 canonical model 维护没有用户价值的
兼容层。当前 `fcs-converter::IrChart` 也不是 Frozen Conversion Specification 要求的 canonical
model，不应作为新工具链的第二语义模型继续演化。

## 2. 决策

I0 立即结束 FCS 4 与 FCS 5 的并行开发。FCS 4 只保存在永久归档分支中；活动 `master`
只保留 Frozen FCS 5 工具链的实现，并且 Rust package、module、test 和普通 example 不使用
`v5` 作为实现版本前缀。

该变更是内部实现和实施路线变更，不改变任何 Frozen 规范文本、合法输入语义、canonical
结果、FCBC layout、Execution ABI、Render 或 Conversion 行为，因此不提升任何规范版本。

## 3. Git 历史与分支拓扑

I0 执行前必须先把当前工作区整理为一个可恢复、可审计的提交。该提交同时包含最新 FCS 4
工具链、当前 FCS 5 候选实现、generator 工作、Frozen 规范、conformance corpus 和文档清理。

在该提交上创建：

```text
archive/fcs4-pre-cutover
```

该分支是精确的 I0 前快照：

- 不删除其中的 FCS 5 候选文件；
- 不重写、squash 或 rebase 其历史；
- 不在该分支继续开发；
- 用记录的 commit SHA 验证其归属；
- 需要查看或恢复旧实现时从该分支读取。

当前功能分支是 `master` 的线性后代。归档建立后，`master` fast-forward 到该快照，所有
I0 代码改动直接发生在 `master`。仓库当前没有 remote；未来增加托管远端时，托管平台的
默认分支也必须设置为 `master`。

## 4. Workspace 与 crate 边界

I0 完成后根 workspace 暂时只有：

```toml
[workspace]
resolver = "2"
members = ["crates/fcs-source"]
```

现有 `crates/fcs-core` 重命名并收缩为 `crates/fcs-source`。该 crate 只负责：

- FCS source value、span 和 AST；
- source lexer 与 parser；
- static semantics 与 compile-time elaboration；
- construction schema；
- source version；
- 稳定、结构化的 source diagnostics。

I0 从活动 `master` 删除：

```text
crates/fcs-cli/
crates/fcs-converter/
crates/fcs-core/src/ast/
crates/fcs-core/src/parser/
crates/fcs-core/src/compiler/
crates/fcs-core/src/bytecode/
crates/fcs-core/src/vm/
crates/fcs-core/src/error/
crates/fcs-core/src/units/
```

`crates/fcs-core/src/v5/` 中经过规范审计的候选实现被提升到无版本前缀的目录；移动完成后
删除 `src/v5/`。不提供 `fcs_core` package 兼容、不提供 `v5` module alias、不提供 FCS 4
feature flag，也不提供兼容 re-export。

目标 crate 布局为：

```text
crates/fcs-source/
├── Cargo.toml
├── src/
│   ├── ast/
│   │   ├── color.rs
│   │   ├── definitions.rs
│   │   ├── entity.rs
│   │   ├── mod.rs
│   │   ├── time.rs
│   │   └── types.rs
│   ├── parser/
│   │   ├── definitions.rs
│   │   ├── document.rs
│   │   ├── entities.rs
│   │   ├── expression.rs
│   │   ├── header.rs
│   │   ├── input.rs
│   │   ├── lexer.rs
│   │   ├── mod.rs
│   │   ├── tempo.rs
│   │   └── token.rs
│   ├── elaborator/
│   │   ├── cycle.rs
│   │   ├── entities.rs
│   │   ├── eval.rs
│   │   ├── mod.rs
│   │   └── scope.rs
│   ├── diagnostic.rs
│   ├── lib.rs
│   ├── schema.rs
│   ├── validation.rs
│   └── version.rs
└── tests/
    ├── diagnostic.rs
    ├── compile_time.rs
    ├── conformance_manifest.rs
    ├── expression.rs
    ├── frontend.rs
    └── workspace_structure.rs
```

公共路径直接使用：

```rust
use fcs_source::ast::Document;
use fcs_source::elaborator::elaborate;
use fcs_source::parser::parse_document;
```

`conformance/fcs5/` 保留版本名，因为它描述格式版本 corpus；Rust 实现路径、测试文件名和
普通 examples 去除 `fcs5` 前缀。FCS 4 examples 从活动 `master` 删除，由归档分支保存。

后续 crate 到对应路线阶段再创建，不在 I0 建立空壳：

```text
fcs-model       canonical chart、Track、Line、Note、metadata 和 fidelity
fcs-runtime     reference evaluator、runtime DAG、数值与 baking
fcs-fcbc        FCBC 2 writer/loader 与 Execution ABI
fcs-converter   PGR/RPE/PEC 与 fcs-model 之间的转换
fcs-render      Render Profile
fcs-cli         组合上述 crate 的命令行入口
```

`fcs-converter` 重建后只能通过 `fcs-model` 交换 FCS 语义，不得直接消费 `fcs-source` AST。

## 5. Parser 技术选择

I0 使用 crates.io 的 Chumsky `0.11.1` 重写 source lexer/parser。`refer/chumsky` 只作为只读
参考资料，不作为 path dependency。参考仓库当前 `main` 是未发布的 0.13 开发线；实现只参考
其 `0.11` tag 中与 crates.io `0.11.1` 对应的稳定 API。

依赖声明为：

```toml
[dependencies]
chumsky = {
    version = "0.11.1",
    default-features = false,
    features = ["std"],
}

[dev-dependencies]
serde = { version = "1", features = ["derive"] }
toml = {
    version = "1.1.2",
    default-features = false,
    features = ["parse", "serde"],
}
```

不引入 Logos：Chumsky 自身完成字符输入到 spanned token stream 的 lexer。不引入 Winnow：
它不能同时替代当前需要的 labelled error 和 recovery。不引入 Ariadne：终端渲染属于未来 CLI。
不在 I0 引入 Proptest：属性生成留给数值、Track 和 evaluator 阶段按需评估。

Chumsky `0.11.1` 的 `pratt` feature 依赖其 `unstable` feature，因此 I0 禁止启用。表达式按稳定
combinator 分层：

```text
primary
postfix/member/call
unary
power
multiplicative
additive
comparison
equality
logical-and
logical-or
```

本地 `0.11` 源码确认 `&str` 输入使用 `usize` cursor，读取字符后按 `char::len_utf8()` 增加
cursor，因此 `SimpleSpan<usize>` 是 UTF-8 byte span。实现仍必须保留 Unicode span 回归测试，
防止依赖升级改变该契约。

禁用默认 `stacker`。FCS parser 必须使用显式 source byte、token count、literal length 和 nesting
budget 拒绝资源滥用，不能依靠动态扩栈接受无界递归。

## 6. Parser 数据流与恢复边界

Parser 使用两阶段数据流：

```text
UTF-8 &str
  -> Chumsky lexer
  -> Vec<(Token, SourceSpan)>
  -> Chumsky token parser
  -> source AST
  -> static semantics / elaborator
  -> ExpandedSourceDocument
```

Lexer 负责 longest-match tokenization、trivia、literal 和精确 span，不创建 AST。Token parser
负责 expression、statement、block 和 document grammar，不执行类型检查或 schema repair。

错误恢复只用于继续扫描并收集更多 diagnostics。Recovery 不得：

- 把非法 source 静默修复为合法 `Document`；
- 插入影响语义的默认值；
- 允许严格编译入口消费含 error diagnostic 的 recovered AST；
- 改变 diagnostic code 的确定性顺序。

## 7. 公共诊断与 parse 输出

Parser 和 elaborator 使用统一的公共诊断数据：

```rust
pub struct Diagnostic {
    code: DiagnosticCode,
    stage: DiagnosticStage,
    severity: DiagnosticSeverity,
    message: String,
    primary_span: SourceSpan,
    labels: Vec<DiagnosticLabel>,
    expansion_trace: Vec<ExpansionTraceFrame>,
    budget: Option<BudgetDetails>,
}

pub struct DiagnosticCode(&'static str);

pub enum DiagnosticStage {
    Decode,
    Parse,
    Elaborate,
    Canonical,
    Evaluate,
    Implementation,
}

pub enum DiagnosticSeverity {
    Error,
    Warning,
}

pub enum ExpansionTraceKind {
    Const,
    Function,
    Template,
    Collection,
    Range,
    Generator,
    Emit,
}

pub struct ExpansionTraceFrame {
    kind: ExpansionTraceKind,
    subject: Option<String>,
    index: Option<usize>,
    emitted_type: Option<String>,
    span: Option<SourceSpan>,
}

pub struct BudgetDetails {
    kind: String,
    limit: usize,
    observed: usize,
}

pub struct DiagnosticLabel {
    span: SourceSpan,
    message: String,
}
```

调用方只能读取 code、stage、severity、message、primary span、labels、有序
`expansion_trace` 和可选 `budget`；诊断构造器保持 crate-private。`ExpansionTraceFrame` 的字段支持规范要求的
function/template/collection/range/generator/index/emit 信息，不把 trace 拼进人类 message。
I0 所有诊断都是 Error，Canonical/Evaluate/Decode 相关 stage/category 先作为稳定公共形状
保留，直到对应路线阶段实际发出。Chumsky `Rich` error 在 parser 边界映射为项目稳定 code，
不能作为公共 API 泄漏。I0 的 parser source/token/comment limits 使用实现定义的
`resource.limit-exceeded`；它不冒充 FCS 6.8 的 `compile-time.budget-exceeded`。

Parser 返回：

```rust
pub struct ParseOutput<T> {
    output: Option<T>,
    diagnostics: Vec<Diagnostic>,
}
```

其契约是：

- 无 error diagnostic 时 `output` 必须是 `Some`；
- 有 error diagnostic 时严格入口 `into_result()` 返回 `Err(Vec<Diagnostic>)`；
- diagnostics 使用 source order 与稳定 tie-breaker 排序；
- recovered output 不能进入 elaborator；
- 当前没有 warning 行为；I0 的 `Diagnostic::new` 默认 Error，未来 warning 必须显式设置
  `DiagnosticSeverity::Warning`，不能通过 message 文本猜测。

Elaborator 返回 `Result<ExpandedSourceDocument, Vec<Diagnostic>>`。I0 内部可以在首个静态错误
后停止，但公共返回类型允许后续在不迁移 API 的情况下累积独立错误。

## 8. Generator 的 I0 边界

I0 保留当前 generator AST/parser 工作，但删除与 Frozen 规范冲突的兼容行为：

```text
start ..< end step value   合法，半开
start ..= end step value   合法，闭区间
start .. end step value    syntax.invalid-token
```

Parser 不再只对字面量 zero step 做特殊检查。所有 step 表达式在 elaborator 求值后统一验证，
确保字面量、const、函数结果和单位类型使用相同规则。

I0 不实现 generator 展开。Elaborator 遇到第一个 generator 时，在产生任何部分输出之前返回：

```text
code: implementation.feature-unavailable
stage: Implementation
message: compile-time generator elaboration is scheduled for I2
primary span: generator 完整 span
```

该临时诊断不得出现在 conformance manifest 的 expected diagnostic 中，不得跳过 generator，
不得把 generator 复制到 `ExpandedSourceDocument`。I2 完成 generator 后必须删除该路径。

## 9. Conformance manifest 基线

I0 的 `tests/conformance_manifest.rs` 使用 Serde 与 `toml::from_str` 强类型加载：

```text
conformance/manifest.toml
conformance/fcs5/manifest.toml
```

测试必须验证：

- suite ID 和 fixture ID 唯一；
- 引用路径位于 `conformance/` 内，不能使用 `..` 逃逸；
- source、expected 和 vector 路径存在且是普通文件；
- stage、expect 和 profile 只使用 manifest schema 允许值；
- error fixture 有 diagnostic，success fixture 没有 diagnostic；
- 每个 fixture 至少关联一个规范条款；
- expected diagnostic 不使用 `implementation.*`；
- 当前 22 个 FCS fixture 全部可反序列化。

I0 只建立 manifest 完整性门，不宣称 22 个 fixture 已通过语义 conformance。I1/I2 按 stage
逐项接入实际 parser/elaborator runner。

## 10. 实施批次

I0 按下列批次执行，每批先建立失败测试，再实现，再运行局部门禁：

1. **I0-A 快照与归档**：提交当前状态、创建 archive、fast-forward master；
2. **I0-B 唯一 crate 切换**：删除 V4/CLI/converter、建立 `fcs-source`、恢复现有 V5 测试；
3. **I0-C 诊断边界**：稳定 code、labels、`ParseOutput` 和诊断排序；
4. **I0-D Chumsky lexer**：token、byte span、trivia、literal 和资源 limits；
5. **I0-E expression parser**：稳定分层 precedence grammar；
6. **I0-F document grammar**：header、format、tempo、definition、template、entity、collection 和 recovery；
7. **I0-G generator/manifest**：range 严格化、阶段诊断和强类型 manifest gate；
8. **I0-H 对账与总门禁**：implementation matrix、文档、结构检查和 workspace gate。

结构切换与 parser 重写必须是不同提交，以便失败时定位是路径迁移还是 grammar 行为变化。

## 11. 实施矩阵与状态词汇

I0 建立 `docs/conformance/fcs5-implementation-matrix.md`。每行必须包含：

```text
规范文件与章节
能力
public API
实现文件
valid fixture
invalid fixture
当前状态
下一阶段
已知偏差
```

状态只允许：

```text
implemented
partial
not-started
blocked-by-I<n>
```

不得使用“基本完成”“应该支持”等不可验证描述。实现文件使用 I0 后目标路径；不存在的未来
文件写明所属阶段，不伪造当前实现位置。

## 12. Roadmap 与冻结审查

现有 roadmap 的 v4/v5 共存策略被本决策取代：

- I0 改为归档 FCS 4、立即完成唯一 source implementation 切换；
- I1 聚焦完整 source AST/grammar，而 parser framework 已由 I0 建立；
- I2 删除 FCS 4 API 无回归要求；
- I10 改为 CLI、发行组合与 conformance release candidate，不再负责删除 FCS 4；
- converter 只依赖未来 canonical `fcs-model`。

Freeze review 中的 roadmap SHA-256 是冻结当时的历史摘要，不能覆盖。Roadmap 修改后在 review
增加“冻结后实施路线修订”附录，记录本决策、修订前后 roadmap hash，并明确四份权威规范、
conformance 语义和五个版本域均未改变。

## 13. 完成条件

I0 只有同时满足以下条件才完成：

- `archive/fcs4-pre-cutover` 指向记录的精确切换前提交；
- 活动开发位于 `master`；
- workspace member 只有 `fcs-source`；
- 活动源码不存在 FCS 4 parser/compiler/VM/bytecode/converter/CLI；
- `src/` 不存在 `v4`、`v5` 或其他实现版本目录；
- Rust import 不存在 `fcs_core::v5` 或 `crate::v5`；
- 普通 examples 不含 `#fcs v4`；
- Chumsky 只使用 `0.11.1` 稳定 API，不启用 `pratt`/`unstable`；
- parser diagnostics 使用 UTF-8 byte span 与稳定 code；
- generator 不接受裸 `..`，未实现展开时明确失败且无部分输出；
- manifest 完整性 gate 通过，且不包含 `implementation.*` expectation；
- implementation matrix 没有空白状态或未登记偏差；
- `cargo clippy --workspace --all-targets -- -D warnings` 通过；
- `cargo nextest run --workspace` 通过；
- `cargo fmt --all -- --check` 通过；
- `git diff --check` 通过。

## 14. 后果

- FCS 4 退出活动主线，但可从永久归档分支完整恢复；
- 实现 API 不再携带尚未发布的 `v5` 迁移前缀；
- source AST 与未来 canonical/runtime/container crate 形成单向依赖边界；
- converter 和 CLI 在其规范依赖准备好后重新建立，不继承旧第二语义模型；
- parser 重写增加 I0 工作量，但在继续扩展 grammar 前消除重复 Cursor、嵌套扫描、错误恢复和
  span 处理；
- I0 允许一个明确、可搜索、不会进入 conformance expectation 的 generator 阶段诊断；
- Freeze 版本不变，实施路线修订通过 review 附录单独审计。
