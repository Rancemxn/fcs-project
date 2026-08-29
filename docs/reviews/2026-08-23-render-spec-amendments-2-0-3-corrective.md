# Render 2.0.3 corrective review record

- **Date:** 2026-08-23
- **Target:** PR #534, corrective candidate after merge commit `4c1faa598f1d82d6c654d48eaf309f3db50d962d`
- **Scope:** Render §§8.1, 8.2, 15.2; governance §6; Render conformance contract
- **Decision source:** independent research conclusion supplied for this corrective unit
- **Review status:** candidate correction; not an independent implementation pass
- **Implementation status:** pending source/canonical ImagePattern lowering and empty-dash activation

This record is append-only evidence for the corrective candidate. It supersedes the conflicting empty-dash cap sentence preserved in governance amendment 2.0.2 without rewriting that historical entry. It does not claim that the product implementation, active manifest, Full Gate, Primary Self-Audit, or I10 re-freeze has passed.

## Findings

| Finding | Correction | Evidence |
|---|---|---|
| I-1 | Empty dash directly means a complete solid stroke. Only non-empty dash arrays perform total validation, phase normalization, and element traversal. Open/closed cap and join remain geometric rules. | `docs/specifications/fcs-render.md` §§8.2 and 15.2; `docs/conformance/render/image-pattern-sibling-fields.contract.md` item 1 |
| I-2 | One source-level `pattern*` configuration is copied into each independent ImagePattern Paint record. Each record keeps its own resource from its own `imagePattern(@id)`. The source cannot express two pattern configurations. | `docs/specifications/fcs-render.md` §8.1; contract item 2 |
| I-3 | Omitted `patternSampling` reads each referenced resource's canonical sampling metadata independently. Missing or invalid metadata has stable category `render.resource-decode-failed`; no cross-resource fallback is allowed. | `docs/specifications/fcs-render.md` §§8.1 and 10; contract items 3–4 |
| I-4 | A normative exact closed-form arclength is mandatory for dash placement when present. Other parametric curves use `1/1024`, depth-32 flattening. Exact and flattening are not interchangeable conforming options. | `docs/specifications/fcs-render.md` §15.2; contract item 5 |
| I-5 | The source and expected contract is checked in without active-manifest registration. Activation belongs to the I9 source/canonical ImagePattern-lowering unit and must add machine expected output in the same change. | `docs/conformance/render/image-pattern-sibling-fields.fcs`, `.contract.md`, and `manifest.toml` omission |
| M-1 | The source example is complete FCS source, with resources, viewport, layer, node, paints, stroke fields, and semicolon-terminated sibling fields. | `docs/conformance/render/image-pattern-sibling-fields.fcs` |

## Governance §6 contract

1. **Affected sections:** Render §§8.1, 8.2, 10, 15.2, 17; governance §6; Render conformance matrix and fixture contract.
2. **Current/proposed/motivation:** the prior 2.0.3 candidate allowed empty-dash processing to be inferred, allowed exact and flatten arclength as alternatives, and described shared pattern fields without fully fixing copying/default ownership. This correction makes each rule single-valued.
3. **Boundary cases:** empty versus non-empty dash; one versus two ImagePattern Paints; per-resource sampling metadata; exact versus non-exact geometry; valid source versus not-yet-activated expected output.
4. **Version impact:** FCS Render Profile 1.0.0 remains Draft; no Frozen or I10 claim changes.
5. **Fixture/expected:** the checked-in source and contract are the minimum pre-activation artifact. Machine expected output is intentionally activation-owned because the current implementation cannot pass it.
6. **Implementation ordering:** specification and contract precede the I9 implementation unit.
7. **Status:** Render remains Draft and the activation owner is explicitly recorded.

No `Closes` claim is made by this record. Issue/PR status must not be advanced to Ready, merged, or closed on this documentation-only correction.
