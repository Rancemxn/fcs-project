# Conversion Specification 1 Capability Domain Amendment

Date: 2026-07-30

Status: governance section 6 change record for Conversion Specification 1.0.0.
The specification remains Draft. This record does not close Issue #294, #325,
or I10.

## 1. Trigger

Issue #425 found that the product capability descriptor declared four axes as
extra domains: `numeric`, `entity`, `limits`, and `expression`. Conversion
Specification section 7.2 defines no such report domains, so the exporter
silently collapsed three to `profile` and one to `package` when constructing
negotiation and limit entries.

## 2. Normative Change

Conversion sections 6.2 and 7.2 now require capability descriptors and report
entries to share the existing ten-domain keyspace. Numeric precision, entity
identity, limits, and expression/runtime-extension support are capability axes
inside the semantic domain they constrain. Only genuinely target-wide runtime
environment, extension-registry, numeric-policy, or identity-policy facts use
`profile`; resource/package counts and byte closure use their own domains.

Unknown or cross-cutting axes cannot fall back to `profile` or `package`.
An axis affecting multiple semantic domains is declared and negotiated once per
affected domain. Approximation domains and drop-selector domain prefixes reject
unregistered names at their typed construction boundary.

## 3. Governance Section 6 Record

1. Affected authority: `fcs-conversion.md` sections 6.2 and 7.2, the typed
   capability/report model, focused negotiation/report regressions, the
   implementation matrix, and the I10 roadmap ledger.
2. Current and proposed behavior: the current product emits unsupported hidden
   domain names and remaps them; the proposed model uses the existing section
   7.2 domains directly and rejects unknown report-domain strings.
3. Legal, illegal, and boundary cases: a motion entity limit is reported as
   `motion`; a package byte limit is `package`; a target-wide runtime extension
   registry is `profile`; `numeric`, `entity`, `limits`, and `expression` are
   illegal domain keys. A shared axis is repeated under every affected legal
   domain rather than collapsed.
4. Version impact: no SemVer change. Conversion 1.0.0 is an unpublished Draft;
   FCS Core, FCBC, Execution ABI, and Render Profile are unaffected.
5. Conformance change: focused tests require the capability descriptor domain
   inventory to equal `ConversionDomain::ALL`, require package byte-limit
   failures to remain in `package`, and reject unregistered report or loss-
   authorization domains.
6. Ordering: this specification and dated amendment authorize the domain model
   before the product implementation is changed.
7. Roadmap and state: Conversion remains Draft with #294, #325, #425, and I10
   open; final Frozen review must include exact-head gate and independent review
   evidence for this change.

## 4. Delivery Boundary

This amendment changes only capability/report attribution. It does not add a
report domain, choose external-format semantics, expand writer capability,
authorize approximation/drop, complete canonical comparison, or change release
state.

Refs #425
Refs #325
Refs #294
