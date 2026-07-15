# 0007：外部谱面格式使用显式、版本化的语义 Profile 解释

状态：Accepted

日期：2026-07-15

## 1. 背景

FCS 是正在开发的 Phigros 新社区谱面规范。PGR v1/v3、RPE 和 PEC 是 converter 需要导入、
导出的既有社区格式，而不是 FCS Core 语义的权威来源。

这些格式不能只靠格式名或单个版本字段得到唯一执行语义：

- PGR v1/v3 有 line-local BPM、缓存的 `floorPosition`、Hold `speed` 和版本化坐标编码；不同
  社区实现对 PGR v1 packed Y 坐标可见 520/530 等差异；
- RPE 的 `RPEVersion` 不能唯一标识实际编辑器版本；`bpmfactor` 可见除法、乘法和忽略三种
  解释，`rotateWithFather` 缺失默认值和 speed easing 也随版本、播放器选项而变化；
- PEC 没有可靠的内建版本字段。已检查的主流实现把 command time 当作 decimal beat，而不是
  `1/2048` tick；offset 可见 150ms/175ms bias，`cv` scale 也存在实现差异；
- parser 的容错行为不等于格式语义。忽略非法 event、裁剪 overlap、补默认 speed、断开 parent
  或丢弃不支持的 event layer 都是具体工具的 compatibility/repair policy。

`refer/chart/` 中的实现和文档是重要证据，但任何单个项目都可能包含 TODO、兼容开关、缺失
功能或静默 repair，不能单独成为 Conversion Specification 的规范来源。

## 2. 决策

### 2.1 语法解析与语义解释分离

每个外部格式 importer 固定分为：

```text
source bytes / package
→ lossless source-format parse
→ format/dialect detection evidence
→ selected source semantic profile
→ source semantic IR
→ FCS CanonicalChart + provenance + ConversionReport
```

Source parser 必须尽量保留原始数值表示、字段是否出现、source order、未知字段、原始路径和
span/path。Parser 不得根据 FCS 字段含义提前猜测来源语义，也不得把 repair 后的值伪装成原值。

Source semantic profile 是版本化的机器接口。它定义特定 producer/runtime/dialect 下的：

- time、BPM、tick/beat 和 offset 映射；
- 坐标、角度、alpha 和 aspect policy；
- event overlap、gap、layer、easing 和 source-order 语义；
- scroll speed、floor/distance 和 Hold 几何；
- Note kind、side、fake、visibility、hitsound 和 presentation；
- 缺失字段默认值、废弃扩展和播放器兼容行为。

Profile 的稳定 ID、版本和 mapping rule 必须写入 provenance 与 ConversionReport。Format version、
producer version、intended runtime、semantic profile 和 parser/repair mode 是不同维度，不得压缩为
一个含糊的 `version` 或 `compatible` bool。

### 2.2 CLI 必须允许显式选择语义

上层 CLI 必须提供显式 source semantic profile 选择能力。公共概念参数为：

```text
--source-profile <profile-id[@version]>
--target-profile <profile-id[@version]>
```

最终参数拼写可以在 CLI 阶段按命令结构细化，但不得删除显式选择能力。库 API 同样必须接受
typed profile selector，不能只依赖进程全局设置或文件扩展名。

自动检测只有在以下情况可以不询问用户：

1. package/manifest 明确声明且声明受支持；
2. 证据唯一确定一个 profile；
3. 多个候选 profile 对当前输入产生可证明相同的 canonical semantics。

若候选解释产生不同 gameplay、motion、scroll、presentation 或 resource 结果：

- strict mode 必须要求显式 profile 或失败；
- compatible mode 可以使用用户配置的默认 profile，但必须记录 candidate、chosen profile、
  selection reason 和受影响语义；
- repair mode 只用于修改非法/矛盾 source，不能用于替用户选择一个合法但有歧义的解释。

### 2.3 Profile 不改变 FCS Core

Source semantic profile 只决定外部格式如何映射到 FCS canonical semantics。映射完成后：

- runtime 只执行 FCS canonical chart；
- PGR line BPM、RPE bpmfactor 或 PEC command state 不作为第二套隐式 FCS 运行时规则继续存在；
- 可以精确表示的兼容行为应 lowering 为普通 FCS Track、Expression、Line、Note 或 Render 数据；
- Core/Render 无法表示的行为使用声明的 runtime extension、preserve、approximation 或失败；
- converter 不得建立长期存在、与 `fcs-model` 并列的第二套 FCS 语义模型。

### 2.4 导出同样使用目标 Profile

PGR、RPE、PEC exporter 不存在无版本、无 runtime 假设的 generic target。Target profile 必须声明
字段能力、数值限制、时间精度、event/layer 语义、资源/package 约束和目标播放器行为。

Exporter 完成写出后必须使用同一 target profile 重新导入，并按 canonical semantics 比较；仅文本
或 JSON 字段相似不能证明转换等价。

## 3. 证据与权威顺序

外部格式行为的证据按下列顺序记录，而不是静默覆盖：

1. 明确的 producer/package/runtime 声明；
2. 对应版本的格式文档与编辑器输出；
3. 可复现的原始编辑器/目标播放器行为 probe；
4. 多个独立社区实现；
5. 用户显式选择；
6. configured compatibility default。

若证据互相冲突，community 文档必须保留冲突和来源；Conversion Specification 只在指定 profile
内作出确定选择。实现现状不得静默成为未版本化的新规范。

## 4. 后果

- `fcs-conversion.md` 必须增加 source/target semantic profile、detection evidence、interpretation
  decision 和 ambiguity report；
- I6 importer 必须先建立未经 FCS 猜测的 source representation，再按 profile lowering；
- I10 CLI 必须暴露 profile 参数，并把最终选择写入 report；
- conformance fixture 必须声明 producer/dialect/profile，不能只写 `format = "rpe"`；
- `examples/` 中没有版本与预期 runtime 的旧文件只能作为 legacy candidate/characterization，不能
  自动成为 normative valid fixture；
- `docs/community/` 负责保存 PGR/RPE/PEC 的字段、版本、已知歧义和证据索引，但不覆盖四份
  权威规范。

### 4.1 2026-07-15 治理补充

用户确认采用本决策和 `docs/community/` evidence baseline 后，Conversion Specification 1.0.0
不能继续使用 2026-07-14 的 Frozen 状态。该候选版本尚未发布且兼容成本为零，因此撤回旧
Frozen 结论，在相同 SemVer 上修订 profile、mapping registry 和 ConversionReport，再经完整
复审重新 Frozen。旧 hash 只保留历史审计用途。

## 5. 不在本决策范围

- 每个 profile 的最终稳定 ID 和完整 mapping registry；
- CLI 子命令和参数的最终拼写；
- 私有版权 fixture 的分发许可；
- 为某个社区播放器指定永久默认 profile。
