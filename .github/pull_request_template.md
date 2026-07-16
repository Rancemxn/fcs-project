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

## Progress

### YYYY-MM-DD — Initial reviewable checkpoint

- Completed: <!-- meaningful commit/change group and resulting capability -->
- Evidence: <!-- commits, tests, fixtures, review, or inspected output -->
- Decisions: <!-- why this change group exists and why this approach was chosen -->
- Blockers: <!-- exact blocker/owner, or none -->
- Next: <!-- next bounded action or ready-for-review disposition -->

Append a checkpoint after each material push and before marking the PR ready. Keep this narrative aligned with the current diff and commit set; do not substitute a raw commit list.

## Verification

- [ ] Rust/build/dependency/test/executable-fixture change: a full Rust checkpoint is required before ready/merge.
- [ ] Documentation/workflow/metadata-only change: Rust gates are not applicable.

Focused checks actually run:

```text
<!-- commands and results -->
```

Full Rust checkpoint, when applicable:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

List every skipped or unavailable gate with its reason. Do not report a non-applicable gate as passed.

## Risks and follow-up

- Residual risk:
- Follow-up Issues:
- Prohibited shortcuts checked:
