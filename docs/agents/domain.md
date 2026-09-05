# Domain Docs

Use this reference when a task needs domain vocabulary, design context, or an authority decision.
It is not a prerequisite for every file edit or repository search.

## Read the relevant context

- Consult [CONTEXT.md](../CONTEXT.md) for terms used by the affected behavior.
- Consult the owning specification clauses and related Accepted ADRs in
  [decisions/](../decisions/README.md) when design or public behavior is affected.
- Follow the task routes in [AGENTS.md](../../AGENTS.md) for fixtures and external-format evidence.
  Read the needed sections; reuse context that has not changed.

Use existing project terminology. A missing term may be a documentation gap; it does not require creating a
new domain model or invoking a particular skill. Optional context can be absent without blocking work.
Missing normative inputs needed to decide behavior must be reported, and only dependent work should pause.

## Resolve authority conflicts

The repository has one ADR directory, `docs/decisions/`. Do not create a second `docs/adr/` tree.

An Accepted ADR constrains design direction; it does not replace normative grammar, binary layout, or execution
semantics. If a substantive conflict exists, identify the exact clauses and follow
[specification governance](../specifications/governance.md). Reopen only the affected baseline and dependencies.
Ordinary implementation choices and documentation corrections do not require a new specification version.
