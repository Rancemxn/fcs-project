## Summary

- <!-- concise summary -->

## Linked work

- Issue: #
- Relationship keyword (plain text, not code): Closes #<n> only when merge should close the Issue; otherwise use Refs #<n>.

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

## Independent review handoff

- Review required: <!-- yes for non-mechanical implementation; no only with a reason -->
- Requested scope: <!-- fixed files/behavior/acceptance boundary -->
- Required commands: <!-- focused/full commands and expected evidence -->
- Fixed head SHA at request time: <!-- fill in the Review requested comment; do not rewrite history -->
- Re-review triggers: <!-- any push, scope, command, dependency, or acceptance change -->

The independent reviewer records the verdict in append-only comments on the PR (and associated Issue). Do not
repeatedly edit this section; follow `docs/loops/review-loop.md`.

## Progress

### Initial reviewable checkpoint

- Completed: <!-- meaningful commit/change group and resulting capability -->
- Evidence: <!-- commits, tests, fixtures, review, or inspected output -->
- Decisions: <!-- why this change group exists and why this approach was chosen -->
- Blockers: <!-- exact blocker/owner, or none -->
- Next: <!-- next bounded action or ready-for-review disposition -->

Keep this initial checkpoint in the body. After each material push, blocker change, and before marking the PR ready, send a new PR comment with the same five fields. Do not append to or repeatedly edit this message; use a new explicitly superseding comment for corrections. Use an event- or state-only heading without a manually written date; GitHub supplies the timestamp. The latest message must match the current diff and commit set, and a raw commit list is not progress.

## Verification

- [ ] Rust/build/dependency/test/executable-fixture change: a full Rust checkpoint is required before ready/merge.
- [ ] Documentation or workflow-policy-metadata-only change (not `.github/workflows/` implementation): Rust gates are not applicable.

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

List every skipped or unavailable gate with its reason. Do not report a non-applicable gate as passed. The exact
review/merge gate is defined by `docs/agents/issue-tracker.md` and `docs/loops/loop.md`.

## Risks and follow-up

- Residual risk:
- Follow-up Issues:
- Prohibited shortcuts checked:
