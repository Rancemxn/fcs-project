# 01 — Governance and stage-scoped implementation baseline

Type: task

Status: resolved

Blocked by: none

## Question

Can the repository replace the global five-domain-Frozen-before-I1 gate with an auditable stage-scoped baseline
without weakening the final conformance RC gate?

## Acceptance criteria

1. ADR 0010 records the accepted decision and explicitly says the baseline is not a version status.
2. Governance defines objective baseline evidence, invalidation and dependency-closure rules.
3. `AGENTS.md`, roadmap, I1 plan and `loop.md` use one consistent gate and automatic transition rule.
4. I10 retains five-domain Frozen, final joint review and full executable conformance requirements.
5. `docs/agents/domain.md` and `AGENTS.md` point to the actual `docs/decisions/` ADR location.
6. Historical reviews retain their original conclusions and receive dated amendments instead of silent rewrites.
7. Local links, UTF-8, `git diff --check` and relevant wording searches pass.

## Comments

- 2026-07-16: user accepted D0 and authorized automatic execution without another stage approval.

## Answer

Yes. ADR 0010 defines the Reviewed Implementation Baseline as a stage-scoped evidence gate rather than a
version status; governance, `AGENTS.md`, the roadmap, I1 plan and `loop.md` now use that rule, while I10 retains
the five-domain Frozen/final-joint-review/full-executable-conformance gate. Historical review conclusions remain
in place with dated amendments. The repository points only to `docs/decisions/`, and the current local-link,
strict UTF-8/NUL, stale-wording and `git diff --check` audits pass.
