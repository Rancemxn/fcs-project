# Conversion Specification 1 Diagnostic Category Amendment

日期：2026-07-26

状态：`docs/reviews/2026-07-15-conversion1-semantic-profile-closure-review.md` 的 dated amendment，
并补记 governance 第 6 章对 `conversion.capability-negotiated` 的变更流程。Conversion Specification
1.0.0 保持 Draft；本 amendment 不改变任何版本状态、不重新开启任何 Frozen domain、不授权关闭 #294。

原 2026-07-15 record 的 finding、结论和第 6/7 章数字一律不改写。该 record 记录的是 2026-07-15
所复审的内容，应当继续那样陈述；变更由本文件承接。

## 1. 触发

合并 PR #299（Issue #294）向 `fcs-conversion.md` 第 17.2 节和
`docs/conformance/conversion/diagnostic-categories.toml` 加入 report-entry category
`conversion.capability-negotiated`，registry 总数由 32 变为 33，并把
`docs/specifications/governance.md` 中计数该集合的句子同步改为 33。

该新增本身是合法的：Conversion Specification 处于 Draft（governance 第 2 章第 41 行
「Draft（2026-07-22；typed capabilities and canonical target reparse remain open）」），status 行点名的
正是这项工作；没有 SemVer 债务，集合一致性也成立——registry 恰有 33 条，第 17.1/17.2 节合计命名同样
的 33 个 id，两侧无单边成员。

有缺陷的是记录方式，见第 2、3 节。

## 2. governance 把固定 record 当成了活动计数来源

被改成 33 的那句话把计数归属给一份固定的独立复审 record。该 record 未被 PR #299 触及，第 6 章仍写
`diagnostic/report categories: 32`。于是 governance 指向一份与自己矛盾的文件。

真正的根因不是「record 忘了更新」，而是**归属方向错了**：2026-07-15 record 是一份历史快照，它对
2026-07-15 的树是准确的，此后也应当继续只陈述那一天。任何随实现推进而变化的当前计数都不能把它当作
权威来源。稳定 category 集合的活动权威是 `fcs-conversion.md` 第 17.1/17.2 节与
`docs/conformance/conversion/diagnostic-categories.toml`，后者由 Rust manifest integrity test 执行。

处置：governance 那句话改为按「record 当日 32，现行 33」双陈述，并把活动来源指向规范与 registry，
同时指向本 amendment。原 record 第 6 章不动。

## 3. 内容哈希 ledger 的处置：整体退役为历史 pin，不重新 pin

2026-07-15 record 第 7 章的 hash ledger 按同样的理由处理。逐条实测（2026-07-26，`sha256sum`，
单文件按原始 bytes）：

| ledger 条目 | pin 前缀 | 当前前缀 | 状态 |
|---|---|---|---|
| `fcs-conversion.md` | `7f8156af` | `c8362367` | 已漂移 |
| `docs/conformance/manifest.toml` | `1378a680` | `231f4505` | 已漂移 |
| `docs/conformance/conversion/manifest.toml` | `28cd0005` | `ff89994b` | 已漂移 |
| `docs/conformance/conversion/profile-registry.toml` | `7d33f47b` | `7d33f47b` | 仍解析 |
| `docs/conformance/conversion/parser-dialects.toml` | `46aeb452` | `46aeb452` | 仍解析 |
| `docs/conformance/conversion/mapping-rules.toml` | `47ad0c81` | `47ad0c81` | 仍解析 |
| `docs/conformance/conversion/diagnostic-categories.toml` | `6b216576` | `890a1126` | 已漂移（PR #299） |
| `docs/conformance/conversion/mapping-vectors.toml` | `a65d3ea` | `a65d3ea` | 仍解析 |
| `docs/conformance/conversion/selection-vectors.toml` | `d36eb78c` | `d36eb78c` | 仍解析 |
| `docs/conformance/conversion` tree | 声明 19 files | 实测 35 files | 文件数已变，tree hash 必然不符 |
| conformance tree at this stage | 声明 88 files | 实测 135 files | 同上 |

即：9 条单文件 pin 中 4 条已不解析，两条 tree pin 的文件数都已改变。这比原 finding 描述的范围大——
finding 只指出 `diagnostic-categories.toml` 一条，且正确地把其余漂移标注为早于 PR #299 且不归因于它。
实测确认了这一点：`fcs-conversion.md` 和两个 manifest 的漂移同样早于 PR #299。

处置是**退役而不是重新 pin**。第 7 章整体明确为 2026-07-15 当日的历史 pin：它证明当天复审了哪些
bytes，不再作为对当前树的可解析断言。重新 pin 会把同一个缺陷推迟一次提交——固定 record 里的活动
pin 必然随下一次改动再次失效。当前 bytes 的权威是 Rust manifest integrity test，它在每次 full gate
上对活动树执行，而不是文档里的一行十六进制。

因此本 amendment 不写入新的 hash。原第 7 章不改写；退役由本节声明，并由原 record 末尾新增的
amendment 指针引用。

## 4. governance 第 6 章变更流程补记

第 6 章要求七项。PR #299 当时只满足第 7 项。现补记如下——补记不追认当时的顺序，缺项照实记录。

1. **受影响规范与章节**：`fcs-conversion.md` 第 17.2 节（stable report-entry category）；
   `docs/conformance/conversion/diagnostic-categories.toml`；governance 第 2 章计数句。
2. **当前行为、建议行为、动机**：此前 capability negotiation 的结果没有稳定 report-entry category，
   negotiation 证据无法被机器消费。新增 `conversion.capability-negotiated`，`uses = ["report-entry"]`、
   `domain = "cross-domain"`。
3. **合法、非法、边界案例**：合法——export 成功且每个 target capability domain 已确定性协商；
   非法——把该 id 用作 error diagnostic（它只登记 `report-entry`）；边界——见第 5 节，实现当前
   在两个 registry description 未覆盖的位置发出它，这是未决缺陷而非已授权行为。
4. **版本域**：无。Conversion 处于 Draft，新增 report-entry category 是兼容添加，FCS、FCBC、ABI、
   Render 均不动。
5. **conformance fixture**：**缺**。没有任何 vector 要求该 category，尽管每次成功 export 都会发出它，
   而 public fixture lane 的 `required_categories` key 已支持这种断言。该缺项路由至 #324。
6. **先改规范再改实现**：**未遵守**。第 17.2 节条目、registry 条目与发出它的实现落在同一个 commit。
7. **路线图与版本表状态**：已记录，但叙述方向错误——路线图把该变更写成「run 30112879965 暴露了陈旧的
   32-category 断言，活动分支更新该完整性计数」，即 registry 追赶实现。正确的因果是规范变更授权了
   实现。原叙述不改写（它如实记录了当时发生的事），由本节更正因果归属。

第 6、7 两项是本 amendment 记录的实际违规。它们没有造成语义错误——集合两侧一致，Draft 状态使新增
合法——但顺序倒置正是仓库禁止的「实现静默成为规范来源」模式，这里如实登记。

## 5. 本 amendment 不处理的事项

以下两项曾被建议作为纯文档修正一并处理，此处明确拒绝，理由相同。

**registry description 与实际发出点不符。** 登记含义是「A target capability domain was
deterministically negotiated before writing.」，而 `finish_export` 另在两处发出同一 id：
`ConversionPhase::ReparseCompare` 的 reparse 比较结果，以及 `SemanticStatus::Approximated` 的
per-metric 近似误差证据。两者都不是 capability negotiation。

把 description 改宽以覆盖现有发出点，正是本文件第 4 节第 6 项刚刚登记为违规的那个模式——让实现决定
规范含义。正确处置是收窄实现，或为那两类证据走第 6 章流程取得 category。两者都是代码变更加规范问题，
不属于文档单元。路由至 #324。

**`conversion.report` 未登记。** `crates/fcs-conversion/src/export.rs` 有 9 处以
`ExportError::new("conversion.report", …)` 发出该 id，而它既不在第 17.1 节的 parent 列表中，也不在
registry 里。第 17.1 节允许更细的 subcategory，但 `conversion.report` 是已登记
`conversion.report-limit` 的**前缀**而非其 subcategory，因此这条路走不通。

同样不能靠「往 registry 补一行」解决。两条合法路径：把 9 个位置按各自失败原因映射到既有 parent
（`conversion.approximation-not-authorized`、`conversion.drop-not-authorized`、
`conversion.report-limit` 等，无需改规范），或先走第 6 章变更流程新增 parent。路由至 #324。

## 6. 验收

- governance 的计数句与 2026-07-15 record 不再互相矛盾，且不再把活动计数归属给固定 record；
- 2026-07-15 record 的第 6、7 章原文未改，其 hash ledger 已明确退役为历史 pin；
- `conversion.capability-negotiated` 的第 6 章变更流程已登记，两项缺项照实记录并路由；
- `conversion.report` 与 registry description 两项作为未决缺陷路由至 #324，未以文档变更掩盖。

Refs #326
Refs #324
Refs #299
Refs #294
