# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root, or
- **`CONTEXT-MAP.md`** at the repo root if it exists — it points at one `CONTEXT.md` per context. Read each one relevant to the topic.
- **`docs/decisions/`** — read Accepted ADRs that touch the area you're about to work in. This is the
  repository's only ADR directory; do not create a parallel `docs/adr/` tree.

If any of these files don't exist, proceed silently. Don't flag their absence or suggest creating them upfront; the domain-modeling skill creates them lazily when terms or decisions are actually resolved.

## File structure

Single-context repo:

```text
/
├── CONTEXT.md
├── docs/decisions/
│   ├── 0001-single-runtime-clock.md
│   └── 0010-stage-scoped-implementation-baselines.md
└── crates/
```

## Use the glossary's vocabulary

When your output names a domain concept, use the term as defined in `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that signals either that you're inventing language the project doesn't use or that there's a real gap; note it for domain modeling.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR 0009 (exact expressions by default) — reopen the affected specification before
> implementation._

An Accepted ADR constrains design direction but does not replace normative source grammar, binary layout, or
execution semantics. If it conflicts with a current root specification, follow `AGENTS.md` and
`docs/specification-governance.md`: reopen and revise the affected specification and conformance evidence before
resuming the impacted implementation baseline.
