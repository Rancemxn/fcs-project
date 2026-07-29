# Conversion Specification 1 Post-write Evidence Category Amendment

Date: 2026-07-29

Status: governance section 6 change record for Conversion Specification 1.0.0.
The specification remains Draft. This record does not change a version state,
close Issue #294 or #324, or claim I10 completion.

## 1. Trigger

The earlier diagnostic-category amendment registered
`conversion.capability-negotiated` only for deterministic target capability
negotiation before writing. It also recorded two unresolved implementation
sites where `finish_export` reused that category during
`ConversionPhase::ReparseCompare`: an authorized dropped selector and a
verified approximation metric. Issue #407 owns the bounded correction.

## 2. Normative Change

Conversion Specification section 17.2 and the active category registry add
two stable, report-entry-only, cross-domain categories:

- `conversion.drop-applied`: target output omitted a canonical selector under
  the report's matching explicit drop authorization.
- `conversion.approximation-verified`: same-profile target reparse verified an
  explicitly authorized approximation within its recorded `ErrorMetric`
  budget.

`conversion.capability-negotiated` keeps its existing narrow meaning. Failure
diagnostics, conversion phases, status aggregation, authorization objects,
writer behavior, canonical comparison, and `ErrorMetric` fields do not change.

## 3. Governance Section 6 Record

1. Affected authority: `fcs-conversion.md` sections 7.2-7.3 and 17.2, plus
   `docs/conformance/conversion/diagnostic-categories.toml`.
2. Current and proposed behavior: post-write drop and approximation evidence
   currently uses a pre-write negotiation category. The proposed categories
   make the stable machine interface identify the phase-specific fact without
   changing the fact itself.
3. Legal, illegal, and boundary cases: an applied authorized selector drop may
   emit `conversion.drop-applied`; an in-budget `ErrorMetric` produced by
   same-profile reparse may emit `conversion.approximation-verified`. Emitting
   either without its matching authorization is illegal. An unused
   authorization emits neither; over-budget and unverified metrics keep their
   existing failure categories; pre-write decisions keep
   `conversion.capability-negotiated`.
4. Version impact: no SemVer change. Conversion 1.0.0 is still an unpublished
   Draft and the addition is compatible. FCS Core, FCBC, Execution ABI, and
   Render Profile are unaffected.
5. Conformance change: the active category registry grows from 33 to 35, the
   manifest integrity count follows it, and focused exporter checks bind both
   successful post-write categories to their phase, semantic status, selector,
   and `ErrorMetric` evidence.
6. Ordering: this specification, registry, and conformance ledger change is
   committed before the two product emitters are changed.
7. Roadmap and state: Conversion remains Draft with #294, #324, and I10 open.
   The final five-domain Frozen review must include the delivered exact-head
   gate and independent review evidence for this change.

## 4. Historical Records

The 2026-07-15 closure review remains a fixed 32-category snapshot. The
2026-07-26 amendment remains the record that introduced the 33rd category and
identified this residual. Neither historical record is rewritten or re-pinned.
The active authority is the current specification and registry.

## 5. Delivery Boundary

This amendment authorizes only the category split described above. It does not
authorize a capability redesign, new approximation algorithm, report-schema
change, formatter behavior, external-profile interpretation, or release-state
transition. Exact-head Full Gate, Primary Self-Audit, and asynchronous
independent review evidence remain delivery requirements for Issue #407.

Refs #407
Refs #324
Refs #294
