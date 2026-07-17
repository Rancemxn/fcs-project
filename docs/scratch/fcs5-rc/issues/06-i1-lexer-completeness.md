# 06 — I1.1 lexer completeness

Type: task

Status: resolved

Blocked by: 05

## Question

Can the existing single Chumsky token path implement every Appendix B terminal and lexical boundary without
reintroducing a raw-text pre-parser or performing static/canonical validation?

## Acceptance criteria

1. Failing table-driven tests cover every reserved word, punctuation/operator, literal/unit suffix, delimiter,
   longest-match pair and contextual keyword-field boundary named by I1 Task 1.
2. Invalid tests cover BOM/NUL/noncharacter/identifier/semver/numeric/unit/Beat/Color/string/comment and standalone
   punctuation boundaries with baseline-bound category/span behavior.
3. Lexer/token changes preserve one spanned Chumsky stream, leading-minus-as-unary, deterministic byte spans and
   all existing I0 invariants.
4. Source/token/comment/nesting/literal limits fail before bounded work or allocation.
5. Clippy runs before targeted nextest; lexer, diagnostic, robustness and workspace-structure lanes pass, followed
   by rustfmt and `git diff --check`.

## Comments

- Baseline: `docs/reviews/2026-07-16-i1-source-parser-baseline-review.md`.
- Add failing tests before implementation. Do not edit baseline-bound `fcs.md`, `fcs-render.md`, FCS manifest or
  source fixtures without invalidating and reopening the baseline.

## Answer

Yes. The existing Chumsky path now tokenizes every Task 1 Core terminal and contextual boundary without a raw-text
pre-parser, re-lex path or fixed parser thread. Table-driven tests cover the full keyword/operator/unit set,
semver-versus-float priority, field-name keyword tokens and Render contextual identifiers. Invalid evidence covers
extra BOM, NUL/noncharacters, non-ASCII identifiers, leading-zero and arbitrary-length semver, malformed/huge
numeric candidates, unit adjacency, mixed Beat, Color, escapes, raw-newline/unclosed string/comment, comment depth,
bare range and standalone punctuation with deterministic half-open byte spans.

The lexer now uses crate-private sentinels for invalid semver/numeric/Color/punctuation and resource limits so
Chumsky choice competition cannot shorten or recategorize a complete lexeme. `Version` preserves unbounded decimal
components; integer magnitudes retain enough precision for direct `i64::MIN`; source BPM is represented separately
from positive canonical `Bpm`; and `max_token_bytes` bounds identifier/header/semver/number/Color/string payloads
before the relevant allocation. All parser resource diagnostics carry kind/limit/observed/span.

Gate evidence: Clippy passed before nextest; the workspace passed 175/175 tests; focused lexer, diagnostic,
robustness and workspace-structure lanes passed; strict UTF-8/NUL, local Markdown link, dependency topology,
rustfmt and diff checks passed. The fixed `fcs.md`, `fcs-render.md`, FCS manifest and 39-file fixture-tree hashes
reproduced the I1 baseline exactly. FCBC's later `u16` representability for source semver components above 65535 is
an I7 residual; I1 preserves the exact source value and performs no truncation.
