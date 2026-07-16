---
name: agent-loop
description: "Design a focused Markdown loop.md contract for agent/automation loops: goal, termination, progress invariant, approval gates, measurement, residual routing, and optional subagent policy. Use for loop design only; output loop.md; never execute the loop, emit runtime code, or prescribe execution mechanics."
---

# Agent Loop

## Role

Design one artifact: `loop.md`. It fixes the decisions a loop must settle and
leaves execution to the user's external goal mode. Do not run the loop, write
executors, generate launcher code, or prescribe mechanics such as `while`
blocks, phases, tool dispatch, or state layout.

Use `assets/loop-template.md` as the blank output shape when drafting a new
`loop.md`. Use `references/review-checklist.md` during review, whenever a
residual appears, or when subagent policy is in scope.

## Output Shape

`loop.md` is a design contract. Include only sections that are needed for the
task, but keep the spine:

1. **Goal & Success Signal** - one goal and an observable completion test.
   Avoid vague signals such as "done", "looks good", or "task complete".
2. **Termination Conditions** - max iterations or budget, the concrete shape
   of no progress, and the goal-achievement check.
3. **Progress Invariant** - the bounded quantity every path advances toward an
   exit. A path that neither terminates nor advances is undeliverable.
4. **Approval Gates** - only for irreversible actions. State the gated action,
   grant consequence, and deny consequence. Do not gate separable safe work.
5. **Measurement Domain** - match verification to the output domain: code uses
   tests, visual work uses renders/screenshots, interactive work is driven, and
   data work uses queries or checks.
6. **Residual Routing** - route iteration failures to LOCAL (retry in loop),
   PLANNER (re-scope), or HUMAN (escalate).
7. **Subagent Using Policy** *(only if dispatching subagents)* - define the
   dispatch contract, not the dispatch mechanism: trigger, role capability,
   input contract, output contract and acceptance check, concurrency, failure
   routing, and sub-task termination.

For the exact blank document, copy the shape from `assets/loop-template.md`.

## Workflow

```
compile goal -> survey role capability -> grill -> draft -> review -> user diff -> accept
```

1. **Compile goal** - extract task, success signal, risk surface, and output
   domains.
2. **Survey role capability** - before grilling subagent selection, inspect the
   current platform's available role capabilities and tool boundaries. Use this
   as the strawman for any subagent dispatch contract. Skip this only when the
   loop will not dispatch subagents.
3. **Grill** - resolve one design branch at a time with 2-3 related questions.
   Each question includes a recommended answer for the user to accept or
   correct. Push back on vague answers with a sharper strawman.
4. **Draft** - write Goal, Termination, and Progress Invariant first; then fill
   the needed optional sections from the decision log.
5. **Review** - read `references/review-checklist.md`; fix LOCAL residuals in
   place, re-scope PLANNER residuals, and escalate HUMAN residuals.
6. **User diff** - change only the sections the user names.
7. **Accept** - deliver when the user accepts and no residuals remain.

Bound drafting to the initial draft plus 2 review/rewrite rounds. If the same
residual is present in the initial draft and both rewrites, stop and ask.

## Grill Discipline

The grill turns a vague goal into a design sharp enough to write. Never skip it
unless the user explicitly asks to draft with open questions.

Rules:

1. Ask about one design branch at a time, with 2-3 related questions per turn.
2. Include a recommended answer with every question; the user accepts or
   corrects it.
3. Work depth-first. Exhaust one branch before switching to the next.
4. Do your own homework first. If code or docs can answer a question, read them
   and present the conclusion for confirmation.
5. Push back on vague answers such as "make it work" or "good enough"; name the
   vagueness, offer a sharper strawman, and ask again.

Resolve these branches before drafting, roughly in order:

- observable success signal and failure signal
- output domains and matching measurements
- termination bounds and no-progress definition
- progress invariant
- irreversible actions and approval gates
- residual routing for likely failures
- subagent dispatch points, role capabilities, contracts, concurrency, and
  sub-task termination
- worst case and Plan B

The grill ends when every branch is resolved, the user explicitly says to draft
with open questions, or 6 turns pass without convergence.

## Subagent Policy Notes

Use capability semantics as the contract, not platform names. For example,
"read-only explorer" or "full-capability executor" may mention a concrete
platform agent only parenthetically. Every dispatched sub-task must carry its
own termination and no-progress bound.

## Output Rule

Deliver one `loop.md`. Keep the grill log, drafting notes, and review notes
internal unless the user asks for reasoning or a diff.
