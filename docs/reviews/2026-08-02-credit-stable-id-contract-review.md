# Credit Stable Identity Contract Review

Status: implementation candidate; Full Gate and Primary Self-Audit pending.

Scope: Issue #477, the canonical identity of FCS `Credit` records and its FCBC
section 5 representation.

Decision: use an explicit non-empty source-text `id`. Duplicate IDs, reserved
generated-ID spellings, zero stable IDs, and typed stable-ID collisions are
errors. Different entity kinds retain separate namespaces. Credit declaration
order is display order and is not used to derive identity. Reordering records
therefore changes display order without changing their stable IDs.

Normative closure:

- FCS `Credit.id` is required and is preserved as UTF-8 text.
- The stable ID uses the existing `fcs.credit` namespace.
- Role, label, contributors, and ordinal position do not participate in ID
  derivation.
- FCBC section 5 keeps the canonical display order. An omitted FCS label is
  encoded as the empty StringTable string; a repeated contributor reference is
  `fcbc.invalid-record` and a missing contributor reference is
  `fcbc.dangling-reference`.

Evidence:

- `crates/fcs-source/src/canonical.rs` and `crates/fcs-model/src/metadata.rs`
  implement explicit Credit identity and namespace validation.
- `crates/fcs-source/tests/metadata_graph.rs` covers identity preservation,
  duplicate rejection, and reorder behavior.
- `docs/conformance/fcs5/source/invalid/credit-missing-id.fcs` and
  `docs/conformance/fcs5/source/invalid/credit-duplicate-id.fcs` cover the new
  source failures.
- `crates/fcs-fcbc/src/writer.rs`, `crates/fcs-fcbc/src/loader.rs`, and
  `crates/fcs-fcbc/src/writer_compilation_tests.rs` cover section 5 emission,
  loading, custom roles, omitted labels, contributor references, and display
  order.

Verification completed on the worktree: `cargo fmt --all -- --check`,
`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, and `git diff --check`. Local tests were not run
because the repository workflow reserves test execution for the exact-head
Codespace Full Gate.

Remaining gate: run the complete Full Gate on the final pushed head SHA, then
record Primary Self-Audit and reviewer evidence. No Frozen claim is made by
this record.
