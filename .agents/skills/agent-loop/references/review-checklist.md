# Review Checklist

Use this checklist before delivering `loop.md`, after user edits, and whenever
a residual repeats. Fix LOCAL residuals in place, re-scope PLANNER residuals,
and escalate HUMAN residuals.

## Residual Classification

| Residual | Class | Action |
|---|---|---|
| `goal_vague`, `term_missing` | LOCAL | Fix the relevant section. |
| `progress_invariant_missing`, `unbounded_path` | LOCAL | Add the bounded quantity or exit. |
| `skipped_grill`, `vague_answer_accepted` | LOCAL | Return to grill or re-ask with a sharper strawman. |
| `subagent_policy_missing` | LOCAL | Add Subagent Using Policy. |
| `subagent_unbounded` | LOCAL | Give the sub-task its own termination. |
| `skipped_capability_survey` | LOCAL | Survey capability before grilling subagent selection. |
| `platform_leak` | LOCAL | Replace platform names with capability semantics; keep examples parenthetical only. |
| `measurement_domain_mismatch` | PLANNER | Re-scope the measurement domains. |
| `subagent_role_unavailable` | HUMAN | Ask whether to change the loop or provide the missing role. |
| `ambiguous_success` | HUMAN | Ask the user for an observable success signal. |

## Hard Rules

- Design only: no loop execution, runtime code, executors, launchers, or tool-dispatch mechanics.
- Do not prescribe execution shape: no `while`, phase machine, state layout, or dispatch pseudocode.
- Do not deliver a loop with an unbounded path.
- Do not draft against an un-grilled goal unless the user explicitly accepts open questions.
- Do not grill subagent selection before surveying available role capabilities.
- Do not use platform subagent names as the contract; use role capability and tool boundary.
- Do not accept reviewer claims or chain-of-thought as validation; require a domain-matched artifact.
- Every dispatched sub-task needs its own termination and no-progress bound.

## Common Mistakes

| Mistake | Fix |
|---|---|
| Success signal is "done" or "good enough". | State an observable check. |
| One loop contains several goals. | Split or reduce to one goal. |
| `loop.md` describes a while loop, phases, or dispatch code. | Keep only design decisions and contracts. |
| Source inspection is used to verify visual or interactive work. | Require render, screenshot, or driven interaction artifact. |
| Grill asks fill-in-the-blank questions. | Ship a recommended answer each time. |
| Vague answers are accepted politely. | Name the vagueness and offer a sharper strawman. |
| Subagent policy says "use general-purpose subagent". | Write the role capability, tool boundary, and acceptance check. |
| Subagent task has no termination. | Add max-iterations, budget, or no-progress bound. |
