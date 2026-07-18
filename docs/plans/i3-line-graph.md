# I3.4 Line Graph Plan

## Normative closure

This work unit lowers the source `lines` declarations into an immutable
canonical Line graph. Its authority is FCS Core §§10.1–10.4 and §§11.1–11.5,
the canonical-lowering pipeline in §17, ADR 0001, ADR 0002, ADR 0010, and the
I3 roadmap.

The closure covers:

- stable Line identity and deterministic ID-keyed storage;
- base transform, alpha, texture anchor, floor/integration, reverse-scroll,
  and z-order defaults and static validation;
- parent existence, self-parent rejection, DAG validation, stable topological
  order, and component-wise inherit flags;
- source-order-independent graph lowering and the immutable parent composition
  boundary;
- declared line scroll-tempo key domain/order/zero-origin validation while
  retaining the single global `chartTime` clock.

Track segment lowering, runtime property descriptors, exact scroll integration,
Note lowering, and FCBC/runtime handoff remain later bounded units.

## Owned surface

- `crates/fcs-model`: canonical Line value types, graph/topology validation, and
  immutable parent/inherit/base-transform objects.
- `crates/fcs-source`: typed lowering from `LinesBlock` and the current Line AST,
  static field evaluation, graph/reference diagnostics, and bounded scroll-map
  declaration validation.
- `crates/fcs-source/tests`: focused valid/invalid Line graph, reorder,
  default, transform, parent, inherit, and scroll-boundary evidence.

The source adapter must not expose parser AST or source spans in the canonical
Line object and must not read resources or introduce a second clock.

## Explicit non-goals

This unit does not implement Track interval/overlap/fill semantics, Note
lowering, runtime evaluator or expression DAG, scroll integration/floor
distance, FCBC/ABI, Render, Conversion, CLI, resource resolution, or release
behavior.

## Acceptance evidence

1. Identity Line defaults lower deterministically; explicit and generated Line
   IDs use the existing canonical ID foundation.
2. Parent references resolve, cycles/self-parent fail with stable diagnostics,
   and the returned topology is stable under declaration reorder and stable-ID
   tie-breaks.
3. Base transform and inherit fields enforce finite/unit/range constraints and
   preserve the specified component composition boundary.
4. Scroll tempo declarations accept one key domain with a zero first key and
   non-decreasing order, reject mixed/invalid domains, and do not become a
   second runtime clock.
5. Focused tests pass before the repository full gate. The completed work unit
   includes the full gate, `git diff --check`, Primary Self-Audit, immutable
   review handoff, and merged-SHA review request.
