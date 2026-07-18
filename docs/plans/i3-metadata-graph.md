# I3.3 Metadata Graph Plan

## Normative closure

This work unit lowers the FCS Core metadata surface into deterministic canonical
objects at the source-to-canonical boundary. Its authority is FCS Core §§7.1–7.5
and §17, together with ADR 0001, ADR 0002, ADR 0008, ADR 0009, and ADR 0010.

The closure covers:

- optional `meta` fields and ordered typed `custom` data;
- contributor declarations and contributor references;
- ordered credits, standard/custom role validation, and contributor references;
- logical resource declarations, declared metadata, and resource references;
- artwork primary-resource binding;
- single-clock `sync`, primary audio, preview, and `audioOffset`.

Canonical resource identity is the declared logical ID. Source paths are checked
as authoring-workspace member paths but are not retained in the canonical
descriptor. This unit does not read the filesystem, resolve symlinks, calculate
hashes, or construct `CanonicalResourceBundle`; those belong to I5.

## Owned surface

- `crates/fcs-model`: immutable canonical metadata/value/resource/sync types and
  validation primitives that do not depend on source AST or a filesystem.
- `crates/fcs-source`: typed lowering from the parsed metadata AST, static value
  evaluation at the metadata boundary, reference/type validation, and diagnostics.
- `crates/fcs-source/tests`: fixture and focused behavior evidence for valid and
  invalid metadata graphs, ordering, optional-block determinism, and path rules.

The public source boundary is `Document::canonical_metadata`; it returns the
canonical graph or canonical-stage diagnostics without exposing source AST nodes
to downstream canonical consumers.

## Explicit non-goals

This unit does not add Track or Line graphs, complete Note policy, runtime
descriptors, FCBC/container or ABI behavior, Render, Conversion, CLI, release
packaging, workspace resource resolution, opaque resource bytes, computed
SHA-256, or source snapshot/provenance output.

## Acceptance evidence

1. The valid metadata fixture lowers to immutable canonical objects, including
   ordered credits/custom values and the signed affine audio offset.
2. Duplicate/unknown/type-mismatched fields, duplicate IDs, unknown references,
   unsupported roles, invalid resource paths, invalid hash syntax, artwork type
   mismatches, and invalid sync intervals produce stable canonical diagnostics;
   declared-hash-versus-file mismatches remain deferred to I5.
3. Reordering declarations changes only explicitly ordered semantic arrays; the
   canonical representation is independent of top-level block order and absence
   of optional blocks is deterministic.
4. No lowering path opens a path or computes a resource digest; declared hashes
   are syntax/metadata constraints only until I5.
5. Focused tests pass before the workspace full gate. The completed work unit is
   delivered with the repository-required full gate, `git diff --check`, a
   Primary Self-Audit, and an immutable review handoff.
