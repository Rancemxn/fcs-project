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

## I6.2a PGR typed source 与 semantic IR

I6.2a 是 I6.2 的第一个可独立验收单元；它固定 PGR v1/v3 的 profile-bound source semantic 输入，暂不
装配 canonical model。

- `ExactDecimal` 保留原始 JSON number lexeme，并在 bounded `num-bigint`/`num-rational` 表示中保持 exact
  value；digit/exponent、entity count 和最终 finite Float64 参数边界均显式失败。
- `parse_pgr_document` 要求明确的 v1/v3 root 和 typed Line/event/speed/Note shape，拒绝 duplicate known
  fields，保留 root/Line/event/Note unknown members 的 source order；v3 要求 split `start2`/`end2`，v1
  packed extensions 仍作为 unknown data。
- `PgrProfileBinding` 只接受四个显式 profile：`pgr.phira.v1`、`pgr.phira.v3`、
  `pgr.phichain-import.v1`、`pgr.phichain-import.v3`，并要求 finite-positive `floorScalePx`；不存在
  automatic profile/default。
- `interpret_pgr` 生成 exact source Line Beat、chart time、offset、move/rotation/alpha、speed distance
  和 Note/Side/Hold semantic IR。它固定 per-Line BPM 与 first-Line BPM、v1 trunc/520 与 round/530、v3
  normalized XY、Note X 108 与 320/3、clockwise rotation、Phira/Phichain Hold speed 差异，以及从 0
  连续 speed integration 对 raw `floorPosition` 的 exact cache 检查。
- 不隐式排序、裁剪、填充、交换、clamp、重建 cache 或执行 Repair；反向/重叠 interval、非法 alpha、负/越界
  packed coordinate、invalid Hold、缺失/gap/overlap/negative speed coverage 和 cache mismatch 直接产生
  typed `PgrError`。

验收证据：`crates/fcs-conversion/src/{exact.rs,pgr.rs}` 的 focused unit tests 执行 checked-in
`mapping-vectors.toml` 的 12 个 PGR source-semantic vectors，并覆盖四 profile divergence、v3 shape、
unknown order、limits 和 invalid paths。Rust compile/Clippy/nextest、locked dependency、bounded fuzz、diff
与 clean-worktree 仍只由 exact-head GitHub full gate 提供；I6.2a 不包含 CanonicalChart、ConversionReport、
profile registry/selection、Repair、package/ZIP、RPE/PEC 或 round-trip fixture。

## I6.2b PGR canonical assembly

I6.2b 只消费 I6.2a 已绑定 profile 的 `PgrSemanticDocument`，不重新解析 source 或选择 profile。

- `lower_pgr_to_canonical` 要求匹配的 chart `SourceArtifact` identity，复用既有 `fcs-model` 构造器输出
  `CanonicalCompilation` 和 `ConversionReport`；chart-only PGR 使用空 resource bundle 与 `chart` profile。
- 一个 canonical chart clock 承载 offset、Line、Track、scroll 和 Note/Hold 时间。Line 使用 60 BPM
  scroll coordinate 保留 speed×seconds 距离，PGR speed 进入 Line `scrollSpeed` Track。
- Line/Note array order 通过 FCS generated textual ID 固定；Note 按 Line、`notesAbove`、`notesBelow`
  source traversal 获得 identity，再由 canonical Note set 执行规范排序。
- restricted provenance 记录 artifact/hash、logical locator、source order、profile、mapping rule、origin 和
  dependency closure；不保留 raw JSON、parser tree 或 workspace path。
- Phira exact lowering 报告 `equivalent`；不同 per-Line BPM 时记录 generated 60 BPM canonical anchor。
  Phichain compatibility profile 报告 `preserved-only`。该边界不执行 Repair、approximation 或 source mutation。

验收证据：`crates/fcs-conversion/src/pgr_canonical.rs` 的 fixture-driven tests 执行
`docs/conformance/conversion/pgr-canonical-vectors.toml`，覆盖 minimal chart、motion/scroll、Hold、offset、
side/source order、profile divergence、multi-Line BPM anchor、重复装配和 artifact identity failure。另有
focused failure tests 覆盖未定义 ordinary-event gap、unsupported zero-duration semantic 与 exact interval
Float64 collapse。I6.2 至此完成 PGR source-to-canonical 产品边界；profile selection、RPE/PEC、package、
真实 round-trip 与 exporter
仍由后续单元交付。

## 验收证据

- `crates/fcs-conversion/src/lib.rs` 单元测试覆盖 duplicate/order、raw key/value string、number lexeme、
  artifact bytes/hash/order/duplicate ID、三类 artifact limit、非法 UTF-8 和 PEC JSON rejection。
- 本地只运行格式、diff 和文档静态检查。Rust compile、Clippy、nextest、bounded fuzz、dependency 和
  clean-worktree gate 必须由 `.github/workflows/full-gate.yml` 的 exact PR head run 证明。
- Primary Self-Audit 必须固定 Issue/PR、head SHA、scope、commands、full-gate evidence 和 acceptance
  gate；无未解决 Critical/Important finding 后才能 Ready/merge。

## 后续单元

- I6.3：RPE dialect validation、typed source semantic IR 与 canonical lowering。
- I6.4：PEC command parser、source order 和版本化 offset/cv profile。
- I6.5：content-hash-bound profile registry、detection evidence 与无猜测 selection。
- I6.6：strict-invalid 和显式 Repair execution/report。
- I6.7：公开 fixture、canonical snapshot、expected report 与 opt-in copyright lane。

I6 只有在上述单元共同交付真实 source-to-canonical execution 与 round-trip 证据后才完成；I6.1 只是
共享的 lossless trust boundary。
