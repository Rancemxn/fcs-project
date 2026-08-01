# FCS 5 Conformance Corpus

本目录是 FCS 5 规范冻结所绑定的机器可读测试向量，不属于示例教程。`manifest.toml` 中每个
fixture 使用稳定 ID，并引用权威规范条款、输入文件和预期结果。

2026-07-15 的 Source grammar closure baseline 保留 FCS 5.0.0 版本并将 FCS source fixture 扩展到
32 项；这些条目闭合完整顶级 envelope、escaped NUL、header whitespace/leading-zero、duplicate
block、generator placement、extension delimiter、mixed-Beat rejection 和 unresolved schema enum。

后续 authoring/canonical closure 最初增加 7 项：canonical-equivalent
direct/template、exact Expression DAG、Note policy、disabled sound policy、workspace path escape 和
resource hash mismatch。Root/FCS manifest schema 升为 2：root 使用 `candidate_baseline`，resource
fixture 可以声明 `workspace_root`，resolver 只能在该目录内读取 opaque member bytes。原 32 项
grammar baseline 继续作为 Reviewed 范围的历史证据；后续 I3–I5 stage-scoped fixture 持续追加，当前
manifest 共 53 项。新增的 `source.valid.appendix-a-minimal-complete` 以 canonical stage 覆盖 FCS Appendix A；I5.3 新增 `source.invalid.resource-missing-member`，并把 existing valid resource、
path escape 与 hash mismatch 向量接到显式 workspace bundle resolver；I5.4 新增 sync preview invalid fixtures 与 shared formula vectors，并执行 existing offset equation。当前 53 项
candidate 仍随五个 Draft/联合重审版本域审查，不是 Frozen baseline。当前状态和后续 re-freeze gate
以 `docs/specifications/governance.md`、路线图与
`docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md` 为准；后者是联合候选自检，不是独立
review 或 Frozen 声明。

I4.4 scroll-composition closure added the evaluate-stage
`source.valid.scroll-inheritance` vector and its literal expected output to the
active corpus. The product evaluator executes this vector; exact-head full-gate
and Primary Self-Audit evidence remain required before a new binding supports a
stage claim.

I4.9 adds the bounded deterministic runtime-property lane documented in
`fcs5-runtime-properties.md`. It uses the pinned dev-only `proptest` source and
does not add manifest fixtures.

S15 FCBC/Execution ABI closure 把 root `candidate_baseline` 更新为
`2026-07-15-s15-cross-spec-closure`，并引入 FCBC manifest schema 2。当前三个 FCBC golden 都固定
14 个 required section（原 1–13 加 ResourceData 20）和 `chart_count=1`：

- `minimal-runtime`：864 bytes、Resources count=0、ResourceData length=0；
- `embedded-resource`：1021 bytes，内嵌 29-byte opaque binary resource，固定 resource ID、原始
  payload、offset/length、CRC-32/ISO-HDLC 和 SHA-256；
- `nonempty-execution`：3432 bytes、strict-runtime profile，带固定 execution vector。

最小向量有 11 个 header/section/profile mutation，resource 向量有 2 个在重新计算 section CRC 后
才能到达的 resource hash/coverage mutation。它们仍是规范候选 corpus，不代表 Frozen；当前
`fcs-fcbc` loader/mutation tests 和 CLI `inspect` product lane 分别提供 domain 与产品入口证据，
其中 CLI 对三个 golden 分别执行声明的 Core success/rejection contract。最终同 SHA Full Gate、独立复审和五域 re-freeze 仍是
I10 门槛。

Conformance runner 必须：

1. 按 UTF-8 bytes 原样读取 source；
2. 对 success fixture 至少完成声明的 `parse`、`elaborate`、`canonical`、`fcbc-load` 或
   `evaluate` stage；
3. 对 error fixture 比较稳定 diagnostic category，不比较完整人类 message；
4. 比较 Float64 时优先使用预期 hexadecimal bits；
5. 比较 canonical object 时忽略 JSON whitespace，但不忽略 array/source order；
6. 报告 implementation limits，不能把超限伪装成规范 fixture 失败；
7. 对 Conversion profile descriptor 使用原始文件 bytes SHA-256，对 parser dialect/mapping rule 使用
   UTF-8 contract SHA-256，并拒绝 dangling ID/version/hash；
8. 不以当前 Rust 实现行为覆盖 fixture 预期。

目录：

```text
fcs5/manifest.toml          Source/static/canonical fixtures
fcs5/source/valid/          应成功的 FCS source
fcs5/source/invalid/        应产生稳定 category 的 FCS source
fcs5/expected/              canonical snapshot 和数值向量
fcbc/manifest.toml          FCBC schema 2 fixture index
fcbc/minimal-runtime.*      空资源 FCBC 2.0 runtime golden 与 manifest
fcbc/embedded-resource.*    内嵌原始资源的 FCBC 2.0 golden、manifest 与 mutations
fcbc/mutations.toml         基于 minimal-runtime 的确定性损坏向量
render/manifest.toml        Render source/semantic/raster fixture manifest
conversion/manifest.toml    Conversion schema 2 registry/vector 入口
conversion/profile-registry.toml  12 个版本化 semantic profile 与 descriptor hash
conversion/parser-dialects.toml   7 个 parser dialect contract
conversion/mapping-rules.toml     56 个 versioned mapping rule contract
conversion/diagnostic-categories.toml 33 个 diagnostic/report category
conversion/mapping-vectors.toml   38 个 exact mapping 与 5 个 invalid boundary
conversion/selection-vectors.toml 10 个 evidence/profile ambiguity decision
conversion/public-fixtures/ I6.7 可执行公开 importer fixtures + expected report keys
conversion/profiles/        各 profile 的完整、逐文件 hash-bound descriptor
```

Render manifest schema 3 在原 semantic/raster `fixture` 之外增加 `source_fixture`，其
`expect`/`diagnostic` 规则与 FCS source manifest 一致，并增加 semantic-only `binding_fixture`。
`fcs-render` domain tests 执行 source-to-Render/semantic/raster 与 resource-boundary 证据；CLI
product lane 通过 `check`、`compile`、`inspect --render` 覆盖声明的 source、compile 和 binding
入口，但不替代 manifest 的 semantic/raster oracle。

S15 resource closure 又在同一候选 schema 2 增加一个 semantic-only `binding_fixture`：source 与
29-byte opaque asset 必须位于声明的 workspace root，预期固定 canonical resource ID/hash、FCBC
Resources section 6 与 ResourceData section 20。该向量显式 `decode=false`，只验证绑定边界，不用
不可解码的 opaque bytes 冒充 raster fixture。

S15 Conversion closure 把 root conversion suite 从单个旧 mapping 文件提升为 schema 2 umbrella。
Registry 明确分离 parser dialect、profile selection mode、semantic profile 与 Repair，固定 PGR
per-Line/first-Line BPM 和 v1 520/530、RPE divide/multiply/ignore bpmfactor 与 speed/layer/default、PEC
direct Beat/150–175ms/cv/Note-X/Line-X；`pec.time.tick2048` 被列为 forbidden rule。Selection vectors
覆盖 explicit、declared、canonical-equivalent、configured-default、Repair 不得消歧和 strict target
profile required。Registry/schema tests 负责契约完整性；`fcs-conversion::fixture_lane` 执行六个公开
import fixture 及声明的 export/reparse target，CLI product lane 再通过 `report`/`convert` 复现公开
入口。它们仍是候选证据，不替代同 SHA Full Gate、独立复审或 Frozen。

`.hex` 文件只包含 ASCII lowercase hex 和换行。Reader 必须先移除 ASCII whitespace，再每两位
解码一个 byte。

I10 product-entry, domain, property, fuzz, CLI, and distribution evidence is
indexed in [`fcs5-i10-evidence-ledger.md`](fcs5-i10-evidence-ledger.md). The
unpublished distribution inventory at `fcs5/distribution.toml` is checked against
Cargo package listings and UTF-8 inputs by the CLI distribution test; this remains
candidate evidence pending the exact final-SHA Full Gate, independent review, and
five-domain re-freeze.
