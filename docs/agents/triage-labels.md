# GitHub Triage Labels

Workflow state is represented by exactly one of these GitHub labels on each open Issue:

| Label | Meaning | Normal exit |
|---|---|---|
| `needs-triage` | Maintainer must classify scope, authority, readiness, and owner | another state label |
| `needs-info` | Work is blocked on information from the reporter or an external owner | `needs-triage` |
| `ready-for-agent` | Scope, authority inputs, acceptance criteria, and verification are sufficient for an agent | linked PR or re-triage |
| `ready-for-human` | A human decision, credential, environment, or implementation is required | `needs-triage` or linked PR |
| `wontfix` | The request will not be actioned | close Issue with rationale |

Type labels such as `bug`, `documentation`, `enhancement`, and `question` are orthogonal and may coexist with one state label.

## Orthogonal taxonomy

The repository maintains these additional labels for cross-cutting routing:

| Label family | Labels | Use |
|---|---|---|
| Domain | `specification`, `conformance` | Identify normative or executable-conformance work without changing workflow state. |
| Delivery | `workflow` | Identify repository collaboration and delivery policy work. |
| Review | `review-finding` | Identify an Issue created from an independent review finding. |
| Severity | `severity:critical`, `severity:important`, `severity:minor` | Record the supported impact of a review finding; use at most one severity label per finding. |

These labels are additive metadata, not alternative workflow states. An open Issue still has exactly one
`needs-*`/`ready-*`/`wontfix` state label. Reviewers may apply existing domain, review, and severity labels to finding
Issues they create, but they must not silently redefine the label taxonomy.

## Milestones

Milestones group work by stage or repository workflow and do not replace a state label, an owner, or a dependency
relationship. The initial milestones are `I2 Static Semantics` and `Repository Workflow`; they intentionally have no
due dates. Create later stage milestones only when work for that stage exists.

The primary session owns the milestone and taxonomy for primary Issues. An independent reviewer may assign an existing
milestone to a finding Issue it created and may propose a new milestone or taxonomy change in a separate Issue/comment;
the primary session makes the global change and may adjust the primary Issue's milestone.

Do not add an `in-progress` state. For `ready-for-agent`, the assignee and linked branch/PR record that work has started. Keep the state label until the Issue closes through merge or returns to triage.

## State changes

Use `gh issue edit` to remove the old state and add the new one atomically in one command:

```text
gh issue edit 42 --remove-label needs-triage --add-label ready-for-agent
gh issue edit 42 --remove-label needs-info --add-label needs-triage
```

Before `ready-for-agent`, verify that the Issue identifies:

- the owning specification or confirms the work is internal-only;
- relevant Accepted ADRs, conformance artifacts, reviews, and stage baseline;
- acceptance criteria, non-goals, dependencies, and verification commands;
- any action that still requires human authority.

When using `needs-info`, comment with the exact missing facts and the consequence of each possible answer. When using `wontfix`, comment with the durable reason and close the Issue; do not modify specifications or historical reviews merely to match the triage decision.
