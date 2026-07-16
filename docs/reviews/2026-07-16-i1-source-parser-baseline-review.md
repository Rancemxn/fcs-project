# I1 Source AST/Parser Reviewed Implementation Baseline

日期：2026-07-16

状态：**PASS**。I1 normative dependency closure 的独立只读复审为 Critical 0、Important 0、Minor 0；
本记录建立 ADR 0010 定义的 I1 Reviewed Implementation Baseline，并自动授权进入 I1 Task 1。该
baseline 不是版本状态，不把 FCS Core、Render Profile 或任何其他版本域提升为 Reviewed/Frozen，
也不授权发布或完整 conformance 声明。

## 1. 阶段产物与边界

I1 只交付完整、带 half-open UTF-8 byte span 的 FCS 5 Core source AST、Appendix B parser、parse-stage
diagnostic/recovery/limit、parse conformance runner、production coverage 和独立 fuzz lane。活动 workspace
仍只有 `crates/fcs-source`；source AST 不是 CanonicalChart，converter、runtime、FCBC、Render 产品实现
和 CLI 均不在 I1 中创建。

本 baseline 的 normative dependency closure 包含：

- `fcs.md` 中 I1 实际读取的 source lexical、literal、type/expression grammar、document/top-level/schema/
  generator/Track/metadata/extension/preserve/render envelope、parse-stage diagnostic、parser limit、
  parse-conformance 条款，以及 Appendix B 和适用的 Appendix C category；
- `fcs-render.md` 第 2 章，且仅用于 Core `renderBlock` 的 balanced-token envelope/ownership seam；
- `conformance/fcs5/manifest.toml` 的全部 source input、stage、expect、diagnostic 和 clauses binding；
- ADR 0006、0008、0010 的 unversioned source、authoring/canonical separation、FCBC distribution boundary
  和 stage-scoped baseline 约束；
- `docs/plans/i1-source-ast-parser.md` 的任务顺序、phase separation、质量门和完成条件。

I1 明确不读取 Render runtime/raster/resource/attachment 语义、FCBC binary/ABI owner validation、Conversion
round-trip/profile lowering 或 Core canonical/runtime evaluation 来决定 source AST/parser 行为。

## 2. 固定输入

单文件 hash 为原始 bytes 的 SHA-256：

| Input | SHA-256 | Baseline role |
|---|---|---|
| `fcs.md` | `2A2882E60AEEF4D96FDB9F7C3CD65B143EA9D9C61F971ECE24A8B2791627DC58` | Core source grammar、parser category/limit/conformance |
| `fcs-render.md` | `848D9AB581529179FC91047737B846A04EE0469AFFAAB43377F47C66C20DB4C3` | 仅第 2 章 balanced Render envelope |
| `conformance/manifest.toml` | `231F4505DE29C854201057F97706295756109A98DCC7AC99F08CA21CD3F96FE8` | root suite binding |
| `conformance/fcs5/manifest.toml` | `4D8DFB7BA2D636CD94A02F39807C7A0DA6185B73213F50DD77E8F2A69C5B25F4` | 39-entry FCS source manifest |
| `docs/decisions/0006-unversioned-source-cutover.md` | `9E9261FFF2D5CDB9E7E29229E7FB3F6F683A71AA66792474F24E2BE5D2DA62C2` | one unversioned source crate/no compatibility path |
| `docs/decisions/0008-fcs-authoring-fcbc-distribution-boundary.md` | `853B5CC765917A8A103B8027DD66541D1DD1D4C1AA64C4BE617BE2917BA72E9B` | source AST/canonical/FCBC separation |
| `docs/decisions/0010-stage-scoped-implementation-baselines.md` | `FEC11690D4F4AB1AE7EDAD86C8AD6E4DBFA9BE3BC86E711AD72046171D011205` | baseline establishment/invalidation rules |
| `docs/plans/i1-source-ast-parser.md` | `3DFBD4F4405097667A349E39A4EFA54B3CA94C1889B05694DA2A8718EAD0DA03` | independently reviewed implementation plan snapshot |

Fixture tree 固定为 `conformance/fcs5` 下的 `manifest.toml` 加全部 38 个 `**/*.fcs`，共 39 files。
路径必须相对 `conformance/fcs5`、按 ordinal byte order 排序，并把每个 Windows separator 规范化为
ASCII `/`。Hash stream 对每项依次追加：

```text
UTF-8(forward-slash relative path) + NUL + raw file bytes + NUL
```

得到：

```text
0b4e67330c0e9963a8998660bef227dcf7374abff8cd8f85ec8b71dbec0f4154
```

首次本地预计算曾给出未写入 baseline 的 `0655c501...`；独立 reviewer 发现它不能按声明算法复现。
根因是旧 PowerShell 兼容脚本匹配了两个反斜杠，实际 hash 仍使用 Windows separator。该值被拒绝，
脚本改为逐个 `\`→`/` 后由执行者和独立 reviewer 分别复算出上面的同一 digest。这个勘误发生在
baseline 建立前，不得把旧值用于后续 invalidation 或审计。

## 3. Appendix B closure

独立 reviewer 直接检查固定 `fcs.md`，确认：

- `document`、`header`、`semver` 在 Appendix B 有定义（当前第 1962–1964 行）；
- `topLevelBlock` 的完整 alternatives 已定义（当前第 1966–1968 行）；
- `renderBlock`、`balancedTokenBlock`、parenthesis/bracket group 均有闭合 production（当前第
  2081–2086 行）；
- Core 对 Render payload 的职责只包含 version envelope、Core tokenization、delimiter nesting 和
  complete-input consumption；Render semantic grammar 明确委托给 Render-aware parser（当前第
  2139–2143 行）；
- I1 closure 中没有 undefined、nonproductive 或彼此矛盾的 nonterminal，也没有要求 parser 以实现
  猜测填补的 terminal ambiguity；longest-match、semver/float、range/operator 和 keyword field context
  均由规范与计划显式覆盖。

Appendix B 的 closure 只证明 source grammar 可实现，不证明 static/canonical/runtime semantics 已完成。

## 4. Fixture 与 phase ownership

FCS manifest 有 39 entries、38 distinct source paths：

| Stage / expectation | Count | I1 treatment |
|---|---:|---|
| parse / success | 3 | 必须 parse success |
| parse / error | 9 | 必须返回绑定 category |
| elaborate / success | 3 | I1 必须 syntax-accept，不执行 expected elaboration |
| elaborate / error | 6 | I1 必须 syntax-accept，不提前返回 later-stage category |
| canonical / success | 8 | I1 必须 syntax-accept，不生成 CanonicalChart |
| canonical / error | 8 | I1 必须 syntax-accept，不执行 canonical validation |
| evaluate / success | 2 | I1 必须 syntax-accept，不执行 runtime query |

九个 parse-error binding 分别覆盖 missing/invalid header、duplicate top-level block、nested/misplaced
generator、unclosed extension payload、mixed Beat 和 bare range；category 为 `version.missing-header`、
`version.invalid`、`name.duplicate`、`compile-time.nested-generator`、
`compile-time.misplaced-generator` 或 `syntax.invalid-token`。三项 parse-success 覆盖 minimal fragment、
complete source grammar 和 escaped NUL。

以下合法 source shape 必须在 I1 保留而不得提前做语义拒绝：zero generator step、unknown resource
reference/path/hash、duplicate custom key、Track overlap、Hold end、runtime gameplay dependency、unresolved
schema enum、shadowing、required-field、graph cycle 和其他 elaborate/canonical/evaluate fixture。计划第
148–163 行的 explicit exclusions 与 manifest stage 一致。

## 5. Out-of-scope blocker routing

| Residual | Owner | Why it does not change I1 public output |
|---|---|---|
| name/type/schema/static expansion、zero-step、shadowing | I2 | I1 只保留 source nodes/spans并区分 placement grammar |
| resource resolution、canonical Note/Track/graph/time | I3 | reference existence、value legality与 canonical identity不参与 parsing |
| exact expression DAG/runtime evaluation | I4 | I1 只表示 source expression和 `choose` syntax |
| provenance/fidelity/preserve elimination | I5 | I1 只保留 source envelope/order/spans |
| PGR/RPE/PEC profile parsing/conversion | I6/I8 | 外部格式不能定义 FCS Core source grammar |
| FCBC/Execution ABI、legacy visibility EnvB golden | I7 | binary owner validation不改变 source token/AST/category/span/limit |
| Render semantic evaluator/raster/resource/attachment | I9 | I1 只接受 `fcs-render.md` 第 2 章 balanced Core envelope |
| full cross-spec executable conformance/Frozen | I10 | 是发布门，不是 I1 implementation input |

`docs/reviews/2026-07-16-render1-normative-amendment-review.md` 第 11 节已经独立确认 Render RNR
normative gate 关闭，并把 legacy visibility EnvB 与完整 Render executable artifact 分别路由到 I7/I9。
若后续任何残留 finding 能改变 I1 AST、parser category/span/limit 或 envelope，则 ADR 0010 要求立即
使本 baseline 失效并重新复审，不能继续按“域外”处理。

## 6. Plan consistency

独立 reviewer 确认 `docs/plans/i1-source-ast-parser.md`：

- Task 1–8 覆盖 lexer、完整 AST/grammar、document、definitions、generator placement、Core schema/
  extension/render envelope、diagnostic/recovery、fixture/limit/property/fuzz；Task 9 只做治理和最终 gate；
- 明确先补失败测试、Clippy-before-nextest、无 raw-text second parser、无 Chumsky public leakage、无
  `refer/` path dependency；
- 不用计划创造 source semantic，不把 later-stage invalid fixture 改写成 parse failure；
- completion condition、expected matrix transition、I2 separation 和 independent review 足够详细；
- 与本 baseline 的条款、fixture、架构约束一致，没有未定义的实现选择。

计划 hash 绑定的是复审时 snapshot。后续仅更新“baseline 已建立/当前 task”这类运行状态时，必须在
dated status note 中明确不改变 task semantics；任何 task scope、category、fixture treatment 或 public
invariant 变化都使该 plan-consistency evidence 失效并要求重新复审。

## 7. Quality evidence

在固定规范和 fixture bytes 上执行：

```text
cargo clippy --workspace --all-targets -- -D warnings  PASS
cargo nextest run --workspace                         PASS 149/149
cargo fmt --all -- --check                            PASS
git diff --check                                      PASS
strict UTF-8/NUL audit                                PASS 198 project text files
local Markdown link audit                             PASS 10 links / 47 files
cargo tree --workspace --edges normal,build,dev        PASS; registry sources only
Cargo path dependency into refer/                     none
```

当前 `image`/`serde_json` 只是在 `fcs-source` dev-dependencies 中记录过的 pre-stage conformance
exception，不进入 I1 产品 API；I1 不得再激活未拥有的 catalog dependency。

## 8. Independent review result

Reviewer 未参与规范、fixture、plan 或 baseline 修改。首次审查正确拒绝了不可复现的预计算 tree hash；
在 forward-slash path rule 和 corrected digest 固定后，reviewer 独立复算一致并完成全范围复审。

| Severity | Open findings |
|---|---:|
| Critical | 0 |
| Important | 0 |
| Minor | 0 |

结论：I1 source AST/parser dependency closure、fixture binding、phase ownership 和计划已满足 ADR 0010。
I0/prerequisite quality gate通过，I1 自动进入实施，无需再次取得用户确认。下一工作单元是 I1 Task 1
lexer completeness；不得在同一阶段提前实现 I2 static semantics。
