# 07 — I1.2 complete grammar AST and expression/type parser

Type: task

Status: claimed

Blocked by: 06

## Question

Can the source AST and the existing recursive Chumsky parser represent every Appendix B expression, type and
entity-expression production without leaking token internals or performing I2 static semantics?

## Acceptance criteria

1. Failing public AST-shape tests cover arrays, ordered objects, references, index postfix, `choose`, `with`,
   nested generic/Track types, constructible boundaries, source order and complete half-open spans.
2. Failing parser tests cover every primary/postfix/type production, trailing-comma boundaries, duplicate object
   keys retained syntactically, right-associative power, comparison chains, keyword fields and complete-input
   rejection.
3. Source-only nodes stay separate from typed/elaborated values; ordered data is not lowered into hash maps and
   token/Chumsky types remain crate-private.
4. The parser reuses the single spanned-token stream, shared nesting limits and Chumsky `stacker`; it adds no Pratt,
   cursor, token-slice re-parser, fixed thread or premature schema/type/name validation.
5. Existing I0 elaborator behavior either remains green through explicit adapters or returns the established
   later-phase boundary without panic; Clippy, targeted nextest, workspace nextest, rustfmt and repository audits
   pass before Task 2 is resolved.

## Comments

- Baseline: `docs/reviews/2026-07-16-i1-source-parser-baseline-review.md`.
- Task semantics are defined by `docs/plans/i1-source-ast-parser.md` Task 2 and the fixed `fcs.md` clauses listed
  there; this issue does not authorize static/canonical behavior or fixture rewrites.
