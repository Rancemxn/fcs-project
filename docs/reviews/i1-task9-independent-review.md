# I1 Task 9 Independent Read-only Review

Status: **PASS**

Reviewed snapshot: `eaea6a0676aaf81e7cf3abfce90a8b3a90b85f68` on the pushed
`codex/40-i1-governance` branch. The snapshot was fixed before this review; the
reviewer did not edit it. This review covers I1 governance and parser-boundary
evidence only. It does not authorize I2 or promote any specification domain to
Frozen.

## Scope and inputs

The review covered the complete I1 diff from `origin/main`, the I1 plan Task 9,
the roadmap, the implementation matrix, the production-coverage ledger, the
fuzz-lane contract, the I1 baseline review, `fcs.md` Appendix B, the bound FCS
manifest and fixtures, and the active `fcs-source` parser/tests.

The snapshot changes only `AGENTS.md`, the implementation matrix, the I1 plan,
the roadmap, and I1 production-coverage evidence. A direct diff check confirmed
no change to `fcs.md`, `fcbc.md`, `fcs-render.md`, `fcs-conversion.md`, or the
bound FCS source/expected fixtures.

## Evidence review

| Area | Evidence and disposition |
|---|---|
| Appendix B and fixtures | The 117-production ledger is bound to the source-AST, expression, diagnostic, robustness, and manifest tests. The manifest audit reports 39 entries: 3 parse-success, 9 parse-error, and 27 later-stage syntax-acceptance entries. The focused fixture-runner test passes. |
| Parser/static boundary | Source syntax is retained in the source AST; profile, schema, resource, Track, generator, runtime, and canonical semantic checks remain assigned to later stages. The complete-source and later-phase acceptance tests pass, and no I2 product crate or canonical model is present. |
| AST, spans, recovery | Source-order nodes, typed owner bodies, references/index/postfix, ordered object entries, extension/preserve payloads, and Track intervals are covered by source-AST/expression tests. Diagnostic tests cover deterministic ordering, bounded primary/related spans, recovery progress, complete consumption, and no partial output. |
| Limits and robustness | The six published parser limits have exact boundary/no-partial-output tests. The deterministic lane has 12 fixed-seed properties. The bounded smoke loads the 42-seed corpus into `document_bytes`, `document_utf8`, and `expression` and completes successfully. The unbounded lane remains explicitly local-only. |
| Matrix transitions | `blocked-by-I1` occurs zero times. Parser-complete rows are `implemented`; mixed syntax/semantic rows remain `partial` with I2/I3/I4/I5/I6 owners. Future product rows remain blocked by their owning stage rather than being promoted by parser evidence. |
| Public/dependency boundary | `cargo metadata --no-deps` reports one workspace package, `fcs-source` 0.2.0. Normal dependencies contain only Chumsky; test-only roots remain in the dev tree, and fuzz tooling is isolated in the separate `fuzz/` workspace. Token, span, and Chumsky types are crate-private; no `refer/` path dependency or legacy v4/v5 implementation path exists. |
| Rust and repository gates | Clippy passed with `-D warnings`; workspace nextest passed 218/218. `cargo fmt --all`, fmt check, both dependency-tree audits, `git diff --check`, and clean status checks passed. The later changes in the reviewed snapshot are Markdown-only and do not invalidate the Rust gate. |

## Findings

Two governance-text discrepancies were found before this fixed review snapshot:

1. The production ledger still described the deterministic/fuzz audit as a
   future I1.8 child. It was corrected to point to the merged I1.8c evidence.
2. The roadmap still described the active branch, test count, and I1 frontier
   using historical `master`/135/175/Task-2 wording. It was reconciled with
   `main`, the 218-test gate, and the Task 9 frontier.

Both corrections were pushed before the reviewed snapshot. No unresolved
Critical or Important finding remains.

| Severity | Open | Closed before snapshot |
|---|---:|---:|
| Critical | 0 | 0 |
| Important | 0 | 2 |
| Minor | 0 | 0 |

## Disposition

The fixed snapshot satisfies the I1 Task 9 review scope. The I1 implementation
baseline and baseline-bound specification/fixture inputs remain unchanged;
I1 can be marked complete only after this evidence, the final gate, and the
delivery merge are recorded. I2 remains a separate stage requiring its own
Reviewed Implementation Baseline.
