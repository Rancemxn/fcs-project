# FCS 5 RC execution map

Goal: 按权威规范、ADR 0010 和 `docs/plans/fcs5-roadmap.md`，以有限、可审计的工作单元推进当前
Render closure、建立 I1 implementation baseline，并继续到本地 FCS 5 conformance release candidate。

Iteration: 5 / 160

Active work unit: `07-i1-expression-type-ast`

## Notes

- 本目录是执行 ledger，不是规范、ADR 或计划的替代品。
- 工作区含用户已有 Render/ABI scaffold；不得删除、回退或误纳入无关 `.agents/`、`skills-lock.json`。
- 当前失败复审固定 hash：
  - `fcs.md`: `37CFF422B6CFBA64E7A0E5541A366A7E93D2E89A2EB18F069186AB28452D41F5`
  - `fcbc.md`: `BDC9C19D4C4F4C7EDB2A57A38A45B91244F9E07D24B168CB9ED6FC92CB5708BB`
  - `fcs-render.md`: `B066E7F403B9B04761261AB254C46A39462700DA9E221D1C42C76001D4357FC8`
- 历史 Render review gate：FAIL，Critical 2、Important 8、Minor 0；修订后的固定 candidate 独立复审
  PASS，Critical 0、Important 0、Minor 0。
- 当前 workspace quality evidence：Clippy PASS；nextest 175/175；rustfmt check、diff check、UTF-8/NUL、
  local Markdown links 和 dependency topology PASS。
- 已独立复审的 Render candidate hash：
  - `fcs.md`: `2A2882E60AEEF4D96FDB9F7C3CD65B143EA9D9C61F971ECE24A8B2791627DC58`
  - `fcbc.md`: `245E1E41EBB20C94BAF644D284994820487C3CC2DF7C75463F37C3DB04B6F99A`
  - `fcs-render.md`: `848D9AB581529179FC91047737B846A04EE0469AFFAAB43377F47C66C20DB4C3`
- I1 fixture tree：39 files，forward-slash relative path，
  `0b4e67330c0e9963a8998660bef227dcf7374abff8cd8f85ec8b71dbec0f4154`；I1 baseline review 0/0/0。

## Decisions so far

- D0：I1–I9 使用阶段范围化 Reviewed Implementation Baseline；I10 RC 仍要求五域 Frozen。
- D1：`line.scrollTempo` 只允许 `s,b`；`line.scrollSpeed` 允许 `s,b,q`。
- D2：Node query 顺序为 active → attachment gate/minimal dependency → visibility → remaining descriptors。
- D3：attachment 只提供几何和 gate，不继承 Line/Note presentation style。
- D4：TrueType glyph 仅接受 `1 <= glyphId < numGlyphs`，非法值为 `render.invalid-geometry`。
- D5：PGR 无可信 global tempo 时生成 `0beat→60bpm` neutral canonical anchor并报告 provenance。
- Governance baseline 已闭合：阶段 baseline 不改变版本状态，I10 最终门不削弱；证据见
  [work unit 01](issues/01-governance-baseline.md)。
- Render RNR normative gate 与诚实 manifest baseline 已闭合：证据见
  [work unit 03](issues/03-render-normative-fixes.md) 和 [work unit 04](issues/04-render-manifest-baseline.md)。
- I1 source-parser baseline 已闭合并自动进入 I1.1：证据见
  [work unit 05](issues/05-i1-readiness-review.md)。
- I1.1 lexer completeness passed 175/175 workspace tests with exact terminal/span/limit evidence and no raw-text
  or fixed-thread parser path; evidence is recorded in [work unit 06](issues/06-i1-lexer-completeness.md).

## Work units

| ID | Work unit | State | Blocked by | Acceptance pointer |
|---|---|---|---|---|
| 01 | Governance and stage baseline | resolved | — | `issues/01-governance-baseline.md` |
| 02 | Record failed Render review | resolved | — | `issues/02-render-review-ledger.md` |
| 03 | Close Render normative findings | resolved | 01 | `issues/03-render-normative-fixes.md` |
| 04 | Restore executable Render manifest baseline | resolved | 03 | `issues/04-render-manifest-baseline.md` |
| 05 | I1 scoped readiness review | resolved | 03, 04 | `issues/05-i1-readiness-review.md` |
| 06 | I1.1 lexer completeness | resolved | 05 | `issues/06-i1-lexer-completeness.md` |
| 07 | I1.2 expression/type source AST | claimed | 06 | `issues/07-i1-expression-type-ast.md` |

## Frontier

Execute I1 Task 2 test-first from the fixed baseline: add public AST-shape tracer bullets for arrays, ordered
objects, references, index postfix, `choose`, `with` and recursive/Track types, then extend only the source syntax
model and the existing recursive Chumsky parser.

## Residuals

- `RNR-C01`–`RNR-I08` are independently closed on the candidate hashes; Render executable coverage remains I9.
- Full Render semantic/raster/mutation artifacts remain owned by I9 after the pre-I1 normative/manifest baseline.
- The historical nonempty ABI visibility EnvB descriptor is an explicit I7/RC residual; current test-only loading
  does not prove the current `s`-only visibility owner rule.
- Conversion round-trip and Core canonical fixture execution remain later-stage blockers and do not block I1 unless
  their findings enter I1's normative dependency closure.
- FCS source semver components are unbounded `uintMagnitude`; FCBC 2 stores version components as `u16`. I1
  preserves exact source values without truncation. I7 must define or reject non-representable writer inputs before
  a product FCBC writer can claim conformance.
