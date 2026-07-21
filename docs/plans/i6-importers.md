# I6 Importer 实施边界

本计划记录 I6 产品 importer 的分阶段交付边界。它不定义外部格式语义；PGR、RPE、PEC 的解释仍以
`docs/specifications/fcs-conversion.md`、版本化 semantic profile 和固定外部证据为准。

## 权威与阶段顺序

- Conversion §2 固定 `SourceArtifactSet`、lossless parse、`ParsedSourceDocument`、dialect validation、
  profile binding、source semantic IR、canonical lowering 的顺序；每个箭头都是可失败边界。
- Conversion §3.1 禁止 parser dialect 直接定义 timing、coordinate、speed/distance、parent transform
  或 runtime semantics；§8.1 要求 source number lexeme 在 canonical Float64 conversion 前保持精确。
- Accepted ADR 0001、0002、0005、0007 约束 profile、canonical、provenance、report 和 ambiguity 边界。
  `docs/community/` 与 `refer/chart/` 提供外部格式证据，不能替代 Conversion Specification。

## I6.1 已交付边界

- `fcs-conversion` 拥有 `SourceArtifact`、`SourceArtifactSet` 和公开 `ImportLimits`。artifact 保留
  logical ID、role、exact bytes、byte length 和 computed SHA-256；set 保留输入顺序并拒绝重复 ID、
  超量 artifact、单体 bytes 和总 bytes。
- `parse_json_document` 只接受 PGR/RPE JSON，显式拒绝非法 UTF-8、JSON 和 PEC；输出
  `ParsedSourceDocument`，不读取或解释任何格式字段。
- `LosslessJsonValue` 保留 object member/array order、duplicate keys、decoded/raw string spelling 和
  exact number lexeme，包括 signed zero、exponent spelling 与超出机器整数范围的文本。
- JSON grammar 由固定 `serde_json` 1.0.150 的 `raw_value` feature 承载；项目递归 materialize 自有 IR。
  不启用 `arbitrary_precision`，不自写第二套 JSON grammar，也不从 `refer/` 建立 path dependency。
- I6.1 不创建 `ProfileBinding`、`SourceSemanticDocument`、Repair、canonical lowering、package/ZIP 或
  exporter surface。通过 parse 只证明 source shape 合法，不证明来源语义或转换等价。

## 验收证据

- `crates/fcs-conversion/src/lib.rs` 单元测试覆盖 duplicate/order、raw key/value string、number lexeme、
  artifact bytes/hash/order/duplicate ID、三类 artifact limit、非法 UTF-8 和 PEC JSON rejection。
- 本地只运行格式、diff 和文档静态检查。Rust compile、Clippy、nextest、bounded fuzz、dependency 和
  clean-worktree gate 必须由 `.github/workflows/full-gate.yml` 的 exact PR head run 证明。
- Primary Self-Audit 必须固定 Issue/PR、head SHA、scope、commands、full-gate evidence 和 acceptance
  gate；无未解决 Critical/Important finding 后才能 Ready/merge。

## 后续单元

- I6.2：PGR v1/v3 dialect validation、typed source semantic IR 与 canonical lowering。
- I6.3：RPE dialect validation、typed source semantic IR 与 canonical lowering。
- I6.4：PEC command parser、source order 和版本化 offset/cv profile。
- I6.5：content-hash-bound profile registry、detection evidence 与无猜测 selection。
- I6.6：strict-invalid 和显式 Repair execution/report。
- I6.7：公开 fixture、canonical snapshot、expected report 与 opt-in copyright lane。

I6 只有在上述单元共同交付真实 source-to-canonical execution 与 round-trip 证据后才完成；I6.1 只是
共享的 lossless trust boundary。
