# 05 — Establish the I1 Reviewed Implementation Baseline

Type: task

Status: resolved

Blocked by: 03, 04

## Question

Are the exact source-parser clauses and fixtures stable enough to enter I1 without waiting for unrelated I6–I9
product artifacts?

## Acceptance criteria

1. The I1 plan lists its complete normative dependency closure and explicit exclusions.
2. Candidate hashes and applicable source/diagnostic/limit fixtures are fixed.
3. An independent read-only reviewer reports zero open Critical/Important findings in I1 scope.
4. All out-of-scope S15 blockers have owners and cannot alter I1 public AST/parser behavior.
5. I0/prerequisite Clippy, workspace nextest, rustfmt and repository integrity gates pass.
6. Governance/review/matrix records the I1 baseline without marking any whole version Reviewed/Frozen.
7. Once all criteria pass, I1 is claimed automatically with no new user confirmation.

## Comments

- Full five-domain Frozen remains mandatory for I10 conformance RC, not for this stage baseline.
- The first unrecorded fixture-tree candidate used Windows separators despite declaring forward-slash paths;
  independent review rejected it. The corrected 39-file digest is
  `0b4e67330c0e9963a8998660bef227dcf7374abff8cd8f85ec8b71dbec0f4154`.

## Answer

Yes. `docs/reviews/2026-07-16-i1-source-parser-baseline-review.md` fixes the exact Core/Render-envelope clauses,
single-file hashes, forward-slash fixture-tree algorithm and digest, 39 manifest entries, 38 source paths,
parse/later-stage treatment, architecture ADRs and reviewed I1 plan. The independent reviewer reproduced the
corrected digest, found no undefined Appendix B production or fixture/phase conflict, routed all I2–I10
residuals outside I1, and reported Critical 0, Important 0, Minor 0. The 149-test prerequisite gate passes and no
version status changed, so I1 Task 1 starts automatically.
