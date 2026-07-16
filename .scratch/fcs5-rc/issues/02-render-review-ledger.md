# 02 — Record failed Render fixed-snapshot review

Type: task

Status: resolved

Blocked by: none

## Question

Is the first independent review after REN-I08–I16 preserved as a reproducible failed snapshot?

## Acceptance criteria

1. The exact SHA-256 of `fcs.md`, `fcbc.md` and `fcs-render.md` is recorded.
2. The ledger records Critical 2, Important 8, Minor 0 and corrects the summary's Important 7 typo.
3. `RNR-C01`–`RNR-I08` each have severity, impact and disposition state.
4. Governance and cross-spec review point at the failed ledger without claiming closure.

## Answer

Yes. `docs/reviews/2026-07-16-render1-normative-amendment-review.md` section 8 fixes the three hashes and
the complete FAIL ledger; governance and the cross-spec dated amendment reference it. Findings remain open.

## Comments

- 2026-07-16: recorded before modifying the reviewed specification bytes.
