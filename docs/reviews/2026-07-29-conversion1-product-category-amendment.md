# Conversion Specification 1 Product Category Registry Amendment

Date: 2026-07-29

Status: governance section 6 change record for Conversion Specification 1.0.0.
The specification remains Draft. This record does not close Issue #294 or #324,
or claim I10 completion.

## 1. Trigger

Issue #419 found two `conversion.*` identifiers emitted by the active
`fcs-conversion` product that were absent from the active category registry:
`conversion.internal` and `conversion.profile-registry-integrity`. The existing
manifest integrity test checked registry shape and cross-references but did not
check the product-emitted set.

## 2. Normative Change

Conversion Specification section 17.1 and the active registry add two stable
diagnostic parent categories:

- `conversion.profile-registry-integrity`: the installed profile registry cannot
  be read, parsed, or verified against its descriptor hashes.
- `conversion.internal`: an unexpected product boundary failure prevents a valid
  conversion artifact from being serialized.

These categories classify existing failure boundaries. They do not change target
serialization, profile selection, canonical semantics, repair, tolerance, or
external-profile behavior.

## 3. Governance Section 6 Record

1. Affected authority: `fcs-conversion.md` section 17.1,
   `docs/conformance/conversion/diagnostic-categories.toml`, the manifest
   integrity test, and the Conversion implementation matrix/roadmap count.
2. Current and proposed behavior: product code already emits the two IDs;
   the proposed registry entries make both parent diagnostics authoritative and
   add a product-to-registry subset check to prevent future drift.
3. Legal, illegal, and boundary cases: registry read/parse/descriptor/hash
   failures use `conversion.profile-registry-integrity`; unexpected target
   serialization or generated identity/provenance failures use
   `conversion.internal`; neither category is a report-entry category, and
   unrelated format/profile/capability failures retain their existing IDs.
4. Version impact: no SemVer change. Conversion 1.0.0 is an unpublished Draft;
   FCS Core, FCBC, Execution ABI, and Render Profile are unaffected.
5. Conformance change: the active registry and manifest integrity count grow
   from 35 to 37. A focused product inventory test requires every emitted
   `conversion.*` category to be registered.
6. Ordering: this specification, registry, and dated amendment authorize the
   category set before the product inventory test is delivered.
7. Roadmap and state: Conversion remains Draft with #294, #324, and I10 open;
   the final five-domain Frozen review must include exact-head gate and
   independent review evidence for this change.

## 4. Delivery Boundary

This amendment authorizes only registry coverage and drift detection. It does not
authorize a new diagnostic algorithm, category remapping, writer behavior,
canonical model change, or release-state transition. The product inventory is
deliberately a subset check because the registry may contain future categories
before their owning product path is implemented.

Refs #419
Refs #324
Refs #294
