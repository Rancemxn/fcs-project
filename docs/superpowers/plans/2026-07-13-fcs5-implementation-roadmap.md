# FCS 5.0 Implementation Roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the approved FCS 5.0 source language, canonical semantic model, FCBC v2 runtime format, low-loss converters, metadata/resource pipeline, and Render Profile without leaving the workspace unbuildable between phases.

**Architecture:** Introduce a temporary `fcs_core::v5` namespace beside the existing v4 implementation, build and test each FCS 5 subsystem behind explicit APIs, then migrate the CLI and converter and remove v4 in a final cutover. Each phase has its own detailed plan and must finish with workspace Clippy and nextest green.

**Tech Stack:** Rust 2024 workspace, `nom` 8 parser combinators, `serde`, `thiserror`, `bytemuck`, Cargo Clippy, cargo-nextest.

---

## Phase order

| Phase | Deliverable | Depends on | Status |
|---:|---|---|---|
| 1 | Versioned FCS 5 front-end foundation: versions, profiles, exact beat keys, tempo map, minimal document parser | approved design | Implemented; workspace validation gates pending |
| 2 | Compile-time language: typed values, immutable bindings, pure functions, entity templates, `generate/emit`, expansion budgets | Phase 1 | Not started |
| 3 | Canonical chart semantics: tracks, Note gameplay/presentation, transform graph, scroll tempo/speed/distance | Phases 1–2 | Not started |
| 4 | Expression DAG, finite piecewise values, adaptive baking, Float64 curve representation, reference evaluator | Phase 3 | Not started |
| 5 | FCBC v2 container, section table, version triplet, Execution ABI, deterministic serialization and loader | Phase 4 | Not started |
| 6 | Metadata, contributors/credits, resources, artwork, sync, fidelity and conversion reports | Phase 5 | Not started |
| 7 | FCS Render Profile, retained scene graph, paths/paint/images/text, RenderSection and raster fixtures | Phases 2, 4–6 | Not started |
| 8 | PGR/RPE/PEC importer migration into the FCS 5 canonical model | Phases 3–6 | Not started |
| 9 | FCS 5 exporters, loss profiles, capability negotiation and round-trip conformance | Phase 8 | Not started |
| 10 | CLI cutover, normative `fcs.md` finalization, v4 removal, full workspace and copyright-chart validation | Phases 1–9 | Not started |

## Cross-phase rules

- Every phase starts from an isolated Git branch. A separate worktree is optional and should be used only when concurrent work or filesystem isolation is useful.
- [ ] Every behavior change begins with a failing test.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings` before any nextest command.
- [ ] Run `cargo nextest run --workspace` before completing each phase.
- [ ] Run `cargo fmt --all -- --check` before completing each phase; use `cargo fmt --all` only when formatting is required.
- [ ] Do not silently repair invalid source data; diagnostics and explicit repair records follow the approved design.
- [ ] Do not introduce runtime jump, loop, recursion, mutable local, `generate`, or `emit` instructions into FCBC.
- [ ] Preserve unrelated dirty-worktree changes and stage only files belonging to the active phase.
- [ ] Update the normative spec and executable fixtures in the same phase as each stabilized semantic feature.

## Detailed plan files

- Phase 1: `docs/superpowers/plans/2026-07-13-fcs5-frontend-foundation.md`
- Phases 2–10: create one detailed plan per phase after the preceding phase passes and its public interfaces are known. Each plan must cite this roadmap and the approved design at `docs/superpowers/specs/2026-07-13-fcs5-spec-redesign-design.md`.

## Final cutover criteria

- [ ] `fcs_core::v5` is promoted to the default public API and the temporary namespace is removed or re-exported without duplication.
- [ ] All v4-only AST, parser, VM and FCBC code is deleted.
- [ ] Root `fcs.md` describes only FCS 5 and records the FCS, FCBC, Execution ABI and Render Profile versions.
- [ ] Every example in `fcs.md` has an executable fixture.
- [ ] PGR v1/v3, RPE and PEC fixtures pass semantic comparison and emit machine-readable loss reports.
- [ ] FCBC runtime, editable and archive profiles preserve the required canonical metadata and optional fidelity/source sections.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, and `cargo fmt --all -- --check` all pass.
