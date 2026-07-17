# FCS Chart Toolchain

This context defines the project-specific language for authoring, compiling, distributing, converting and
executing Phigros community charts without conflating source structure, canonical semantics or release state.

## Language

**FCS source**:
The human-maintained authoring format that can contain compile-time abstractions and exact runtime expressions.
_Avoid_: distribution chart, player package

**Authoring workspace**:
A normal folder containing FCS source and all resources used while editing a single chart project.
_Avoid_: FCBC package, PEZ archive

**Canonical Chart**:
The unique chart semantics remaining after source validation, compile-time expansion and normalization.
_Avoid_: source AST, converter IR

**FCBC**:
A self-contained, exactly one-chart distribution and execution container containing canonical data and every
required original resource payload.
_Avoid_: baked chart, source snapshot, multi-chart pack

**Execution ABI**:
The versioned typed descriptor and evaluation contract used to query canonical runtime properties from FCBC.
_Avoid_: VM bytecode, baked curve format

**Render Profile**:
The separately versioned scene, resource-binding and raster semantics layered on Canonical Chart and FCBC.
_Avoid_: gameplay renderer implementation, editor UI

**Conversion Semantic Profile**:
An explicit, versioned interpretation of an external PGR, RPE or PEC dialect and its ambiguity choices.
_Avoid_: repair mode, guessed compatibility

**Exact expression**:
A runtime property representation whose specified value is preserved rather than replaced by sampled chart data.
_Avoid_: pre-baked animation

**Player-local sampled cache**:
An optional device/player approximation derived from exact FCBC data and never written back into the chart.
_Avoid_: FCBC baking, author-selected sampling

**Reviewed Implementation Baseline**:
A stage-scoped, independently reviewed normative dependency closure that permits I1–I9 implementation without
changing any specification version status.
_Avoid_: Reviewed version, partial freeze, conformance release

**Frozen**:
The repository version state meaning an entire version domain and its bound executable conformance baseline are
stable after independent review.
_Avoid_: implementation baseline, code complete
