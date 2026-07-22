# I10 Local Conformance RC Freeze Evidence

> **Superseded:** This primary-session freeze claim was withdrawn on 2026-07-22 after the
> loop success signal and the independent PR #290 audit were reconciled with product evidence.
> The historical evidence below is retained for audit; it is not current Frozen or I10 completion
> authority. Current status is tracked by root Issue #9 and `docs/specifications/governance.md`.

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

## Known residuals (honest after #288/#289)

- Native `write_from_compilation` product-asserts `load_container` framing; full Core `load_chart` after native compile remains best-effort (descriptor scaffold).
- FCBC matrix status is `partial` for that residual while goldens still Core-load.
- CapabilitySet is a bounded negotiation surface, not full Conversion §6.2 descriptor.
- No public tag/Release/crate publish.

## Superseding withdrawal

The five-domain Frozen decision above is withdrawn because the documented residuals affect the
I10 success signal rather than a post-RC enhancement. Native `CanonicalCompilation` output does
not yet guarantee Core `load_chart` or embed ResourceData, Conversion capability and semantic
target-reparse evidence are incomplete, and the Render/CLI surfaces are not the full executable
products required by the roadmap. No version domain may return to Frozen until the corrective
Issues under root #9 close and a new exact-snapshot freeze review satisfies governance section 7.
