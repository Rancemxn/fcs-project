## Goal

Allow Conversion preserve negotiation to succeed when the export path already owns a structured FCBC Fidelity sink, instead of failing every preserve decision after Fidelity section encoding already exists.

## Scope and authority

- Parent: #294. Refs #296 and root #9.
- Authority: `docs/specifications/fcs-conversion.md` sections 6.1-6.3, 7.1, 15.1-15.3, and 17; `docs/specifications/fcbc.md` sections 3.1 and 16.3; roadmap I8.3; closed children #415 (fail-closed without sink) and #434 (source-free Fidelity section encoding).
- Product boundary: `ExportOptions` / export negotiation in `crates/fcs-conversion`, plus the existing `CanonicalCompilation` distribution surface and `fcs_fcbc::write_from_compilation_with_profile(..., ContainerProfile::Fidelity)` path.

## Current defect

- #415 correctly rejects `NegotiationAction::Preserve` when no Fidelity or external-sidecar sink exists.
- #434 makes `ContainerProfile::Fidelity` encode a source-free section 16 payload from `DistributionMetadata`.
- The exporter still hard-fails every preserve decision before write, so a caller that already holds a `CanonicalCompilation` with distribution facts cannot negotiate preserve into that sink. Matrix residual "structured preserve output" remains open for this reason.

## Acceptance

1. `ExportOptions` (or the existing compilation export entry points) can declare an explicit structured Fidelity sink backed by the current restricted distribution/provenance surface; absence of that sink keeps the #415 fail-closed behavior unchanged.
2. When a preserve decision is negotiated and a Fidelity sink is present, negotiation succeeds, the target writer still omits the preserved domain from the external target bytes, and the successful report records `preserved` / `preserved-only` with the preserve capability entries.
3. The same successful path emits or updates a structured Fidelity payload that retains only source-free facts already allowed by section 15.2 (no raw source, token trees, absolute paths, or authoring-only data), and a focused regression proves Core/`load_chart` identity is unchanged when that Fidelity section is stripped.
4. Strict policy still rejects preserve; external sidecar schemas remain unspecified and unavailable; PGR/RPE/PEC writers do not gain a second preserve channel.
5. Capability domains, drop/approximation authorization, canonical comparison, target profile selection, formatter behavior, and registered report categories remain unchanged except for the preserve-with-sink success path.
6. Matrix/roadmap evidence records only this bounded preserve-sink wiring; no Conversion Frozen, Render, CLI/RC, or I10 completion claim.
7. Permitted local fmt/check/Clippy and static diff checks pass; exact-head GitHub Full Gate and Primary Self-Audit pass before merge.

## Non-goals

- No new sidecar file format, authoring workspace snapshot, raw source retention, or generic external-profile expansion.
- No full AST reserialization formatter work, Render product path, CLI assembly, distribution inventory, or five-domain re-freeze.
- No broader #294 parent closure.

## Dependencies and verification

Blocked by: none. #415 and #434 are already on `main` at `c35b4f649599df0bbb0c229f03b2c6e66ce971dd`.

Verification: focused preserve-with-sink success regression plus the existing no-sink fail-closed regression; workspace fmt/check/Clippy development feedback; `git diff --check`; exact-head GitHub Full Gate; fixed-head Primary Self-Audit.

## Initial Progress

- Completed: Frontier Sync after #440/#441 merge on `origin/main` `c35b4f649599df0bbb0c229f03b2c6e66ce971dd`. Traced #415 fail-closed guard, #434 Fidelity writer/loader, `ExportOptions`, and compilation export entry points.
- Evidence: `negotiate_export_with_options` still returns `conversion.capability-mismatch` for any `NegotiationAction::Preserve`; `write_from_compilation_with_profile(..., Fidelity)` already encodes section 16 from `DistributionMetadata`; matrix conversion row still lists structured preserve output as residual.
- Decisions: reuse the existing distribution/Fidelity surface as the only in-tree preserve sink; keep external sidecars out of scope; do not invent a second report schema.
- Blockers: none.
- Next: open the linked implementation branch, add the focused preserve-with-sink regression, and wire the smallest exporter option that satisfies acceptance 1-4.
