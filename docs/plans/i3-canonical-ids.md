# I3.1 Canonical IDs

Status: implementation work unit; the normative contract is FCS §17 and ADR 0012.

## Objective

Create the owning `fcs-model` identity seam for deterministic Line/Note canonical textual IDs and typed
FCBC stable IDs. Preserve explicit source bytes, make generated expansion paths host-independent, and fail
reserved-prefix, zero-ID, duplicate, and typed 64-bit collision cases.

## Boundaries

In scope: `EntityKind`, `ExpansionPath`, canonical textual-ID construction, `SHA-256(namespace || 0x00 ||
UTF-8 textual ID)` derivation, typed collision registry, and direct/template/generator vectors.

Out of scope: Beat/tempo normalization, metadata/resource graph, parent DAG/transform, Track and Note semantic
lowering, runtime descriptors, FCBC, Render, Conversion, CLI, and snapshot serialization.

## Authority and evidence

- FCS §17, §§11.2/11.5, §§12.1/12.2/12.5;
- FCBC §6.2;
- ADR 0010 stage-scoped Reviewed Implementation Baseline;
- ADR 0012 and the I3.1 fixture vectors under `docs/conformance/fcs5/`.

The path uses source identifier spellings, zero-based decimal order, and final expanded-output order. The
implementation must not use filesystem paths, hash-map traversal, comments, trivia, or authoring-local names.

## Acceptance gate

1. `fcs-model` exposes immutable identity types and no source-AST dependency.
2. Explicit IDs remain byte-exact and reject `generated/`; generated IDs have one exact spelling.
3. Line/Note namespaces are separated and use the FCBC §6.2 little-endian hash vector.
4. Registry tests reject duplicate textual IDs, zero IDs, and typed collisions without salting.
5. Direct, template, and generator vectors assert deterministic textual IDs and remain independent of host path.
6. Full Rust gates pass at this public workspace/dependency checkpoint; later I3 units remain unclaimed.
