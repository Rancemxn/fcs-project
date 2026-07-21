# I10 Local Conformance RC Freeze Evidence

**Status:** Primary-session RC freeze evidence for the local (unpublished) FCS 5
conformance release candidate.

**Scope:** Five version domains Frozen for local RC only; product surfaces on
`main` for source/canonical/runtime/FCBC/conversion/Render/CLI.

## Product surfaces on main

| Surface | Product location |
|---|---|
| Source/static/canonical | `crates/fcs-source`, `crates/fcs-model` |
| Runtime | `crates/fcs-runtime` |
| FCBC / Execution ABI | `crates/fcs-fcbc` |
| Conversion importers/export | `crates/fcs-conversion` |
| Render | `crates/fcs-render` |
| CLI | `crates/fcs-cli` (`fcs` binary) |

## Stage evidence (selected)

- I6.7 public importer fixtures: PR #273
- I7 product FCBC framing/Core load/writer/mutations: PRs #275, #277, #279, #281
- I10.1 CLI: PR #283
- I8/I9 residual + governance Frozen: this closeout unit (Issue #284)

## Gates

- Rust compile/test evidence is only accepted from GitHub Full gate on exact PR
  head SHAs.
- Primary Self-Audit Verdict `pass` is required before Ready/merge.
- Async reviewer second-pass is not a per-PR merge wait gate for this RC
  acceleration policy.
- Open Critical/Important `review-finding` Issues at freeze: none required to
  remain open for RC.

## Non-goals retained

- No public tag, GitHub Release, crates.io publish, or public conformance bundle.
- Post-I10 advisory / `ready-for-human` Issues remain open and non-blocking.

## Frozen decision

Per user-authorized RC acceleration and product evidence above, the five version
domains listed in `docs/specifications/governance.md` §2 are marked **Frozen**
for the local conformance RC only. Any later normative change must reopen the
affected domain under governance versioning rules.
