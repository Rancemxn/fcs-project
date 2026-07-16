## Summary

- <!-- concise summary -->

## Linked Issue

Closes #

## Scope and non-goals

- Included:
- Excluded:

## Authority and evidence impact

- [ ] Internal-only; no normative behavior changes.
- [ ] Relevant specification clauses and Accepted ADRs are identified.
- [ ] Conformance fixtures/manifests/expected outputs are updated or confirmed unaffected.
- [ ] Implementation matrix, plans, and dated reviews are updated when required.
- [ ] Any reopened baseline or version-state gate is recorded.

Explain the checked items and name affected files or gates.

## Verification

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
```

List additional focused tests, fixture/hash checks, and any command not run with its reason.

## Risks and follow-up

- Residual risk:
- Follow-up Issues:
- Prohibited shortcuts checked:
