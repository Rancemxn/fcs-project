# Phigros 社区谱面格式知识库

状态：Evidence baseline（2026-07-15）

本目录记录 FCS converter 需要理解的既有 Phigros 社区谱面格式。当前覆盖：

- [PGR v1/v3](pgr.md)；
- [RPE](rpe.md)；
- [PEC](pec.md)。

这些文档回答“外部文件和既有工具实际上如何表达、解释谱面”，不是 FCS、FCBC 或 Conversion
Specification 的替代品。外部格式本身存在版本漂移、播放器差异、编辑器遗留行为和未版本化
方言；文档必须保留这些分歧，不能为了得到一条简单公式而抹平证据。

## 1. 职责分域与权威

本目录不使用一条简单的“规范 > ADR > community > refer”总排名。FCS 项目的规范行为与外部格式
的事实证据是两个不同问题。

### 1.1 FCS 项目的规范权威

1. `docs/specification-governance.md` 决定候选版本、状态、变更与冻结流程；
2. `fcs.md`、`fcbc.md`、`fcs-render.md`、`fcs-conversion.md` 在各自版本域内决定 FCS/FCBC 的
   规范行为；
3. `docs/decisions/` 中 Accepted ADR 记录已接受的设计约束和规范修订方向，但不直接替代规范
   条款；
4. 当前 parser/converter、测试和 example 只能证明实现状态或 fixture 意图，不能静默定义格式。

Accepted ADR 与现行规范冲突时，必须按治理流程重开并修订规范，不能任选一方直接实现。规范
重新 Frozen 前，ADR 是修订方向而不是可跳过规范文本执行的兼容开关。

### 1.2 外部格式的证据权威

`docs/community/` 是维护后的证据综合、歧义索引和研究入口；固定 commit/hash 的
`refer/chart/` 是特定项目、特定版本行为的一手证据。对“Phira 在 commit X 如何解释某字段”这类
事实，必须回到该固定快照核对；若摘要与其引用的源码不一致，应修订摘要或明确记录分歧。

一手快照只能证明该项目版本的行为，不能单独定义 PGR、RPE 或 PEC 的通用语义。多个可信来源
冲突时，本目录保留来源、路径和双方行为，不以投票或“主流”抹平；Conversion Specification 只在
指定 semantic profile 内作出确定选择。

其中 [0007-versioned-conversion-semantic-profiles.md](../decisions/0007-versioned-conversion-semantic-profiles.md)
决定了外部格式必须通过显式、版本化的 semantic profile 解释。这里记录 profile 需要控制的
维度，但不提前固定最终 profile ID、默认 profile 或完整 mapping registry。

## 2. 证据标签

本文档使用以下标签区分结论性质：

| 标签 | 含义 |
|---|---|
| **结构共识** | 至少一个格式文档和多个独立实现一致，当前没有相反证据 |
| **快照行为** | 指定 commit 下某个工具的可复现行为，只能描述该工具 |
| **歧义** | 两个或更多可信来源会对同一输入产生不同语义 |
| **实现缺口** | 工具明确忽略、近似、丢弃或尚未实现来源能力 |
| **Repair** | 工具修改了非法或矛盾输入，例如裁剪 overlap、补事件或断开 parent |
| **编辑器约束** | 编辑器 UI/导出器限制，不一定是文件语法或播放器限制 |
| **待考证** | 证据不足，不能选择唯一解释 |

“某工具能加载”不等于“输入合法”，“多个工具碰巧给出相同画面”也不自动证明字段语义等价。
凡是会改变 Note time、Hold、motion、scroll distance、presentation 或资源选择的歧义，都必须由
source semantic profile 解决。

## 3. 证据快照

本轮知识基线固定到以下本地参考快照：

| 项目 | Origin | Commit/内容标识 | 主要用途 |
|---|---|---|---|
| Phira | `https://github.com/TeamFlos/phira.git` | `824e24b97af53c2e14c4e2dfa13ecd36d87c9e06` | PGR/RPE/PEC 实际 parser、包加载和播放器行为 |
| Phichain | `https://github.com/Ivan-1F/phichain.git` | `fe3448449781af86c67a36b97f672b0dbe6c8243` | PGR/RPE schema、双向转换和已声明有损边界 |
| Phira Docs | `https://github.com/TeamFlos/phira-docs.git` | `909a4913d726c13af8ea6904501faea6f91bd2ae` | 社区格式与 Phira 包文档；文档内部也可能冲突 |
| sim-phi/extends | `https://github.com/sim-phi/extends.git` | `00a6602e9b837dba4453aa758cdab06cafc79162` | PEC→PGR、RPE→PGR 的兼容转换实现 |
| phispler-ext | 本地副本无 `.git` | `light_utils.py` SHA-256 `89e5c71c76ab011494292b06105e8f008149f6b4720c69eeee5b9eaa7a1204d6` | 独立 PEC→RPE 行为证据；证据等级低于固定 commit |

检查日期为 2026-07-15。更新 `refer/chart` 后，不能只改表中的 commit；必须重新核对受影响的
字段、公式、歧义表和 fixture 结论。

## 4. 三种格式的定位

| 格式 | 文件形态 | 内建版本信息 | 主要时间表示 | 资源/元数据情况 |
|---|---|---|---|---|
| PGR v1/v3 | JSON | `formatVersion`，主要区分移动坐标编码 | 每条 Line 的 raw `T` 与 line BPM | JSON 只有 chart offset；完整包信息通常在外部 |
| RPE | JSON | `META.RPEVersion`，但不能唯一标识真实语义 | exact-looking Beat triple + 全局 `BPMList`，另有 `bpmfactor` 歧义 | JSON 可引用资源并重复保存元数据；包还可有 `info.yml` |
| PEC | 空白分隔的命令文本 | 没有可靠版本字段 | 主流证据把 decimal number 直接当 Beat | 不保存曲名、谱师和资源清单，依赖外部包信息 |

PGR、RPE、PEC 是 chart payload 格式，不等于一个稳定、统一的“社区谱面包格式”。Phira 常用
ZIP/PEZ 风格包并以根目录 `info.yml` 指向 chart、music、illustration 等文件，但其他工具可能采用
不同目录、清单或优先级。包解析、资源解析和 chart semantic profile 必须分层处理。

FCBC 是否使用 ZIP 与这些历史包无关；本项目已经决定 FCBC 自身直接内嵌一个谱面及其全部资源。

## 5. Converter 的统一分层

外部格式 importer 应遵循：

```text
source bytes / package
→ package/resource discovery
→ lossless source-format parse
→ format/dialect detection evidence
→ selected source semantic profile
→ source semantic IR
→ FCS CanonicalChart + provenance + ConversionReport
```

各层不得互相替代：

- parser mode 决定接受哪些语法形态以及如何保留未知字段；
- source semantic profile 决定合法输入的 time、BPM、坐标、event、scroll 和默认值语义；
- repair mode 决定是否修改非法或矛盾输入；
- target profile/capability 决定导出目标能表达什么；
- package policy 决定从哪个清单和路径取得音乐、图片、纹理、字体与 hitsound。

自动检测只能在证据唯一，或所有候选对当前输入产生 canonical-equivalent 结果时静默成功。格式
版本、producer 版本、目标播放器、semantic profile 和 repair mode 必须分别记录。

## 6. 时间与单一运行时时钟

PGR line BPM、RPE `bpmfactor` 和 PEC `bp` 都可以参与 import-time source time 解码，但结果进入
FCS 后必须归一化为唯一 `chartTime`。外部 time base 不得作为第二套隐式运行时时钟继续存在。

Importer 至少应保留：

- source 数值的原始文本或 exact representation；
- source time domain 和单位；
- 参与换算的 BPM/profile 参数；
- 映射后的 canonical `chartTime`；
- rule ID、profile ID/version 和选择证据；
- 任何 rounding、clamp、补点或顺序调整。

## 7. Fixture 使用规则

`examples/pgr`、`examples/rpe`、`examples/pec` 是历史 converter 输入，不自动拥有 conformance
地位。每个未来 fixture 必须声明：

```text
format
producer/dialect evidence
source semantic profile
parser mode
repair mode
expected parse/import status
expected ConversionReport facts
license/provenance
```

没有 profile、预期 runtime 或来源证据的旧文件只能作为 `legacy-candidate`、`characterization` 或
`invalid` 输入。不能因为扩展名是 `.json`、`.pec` 或旧 converter 曾接受，就标成 normative valid。

## 8. 更新规则

修改本目录时必须：

1. 先检查参考仓库自己的 `AGENTS.md` 或贡献规则；
2. 记录准确 commit；无 Git 历史的副本记录相关文件 SHA-256；
3. 同时查看 parser/schema、调用方、文档和至少一个独立实现；
4. 把结构、执行语义、默认值、repair 和实现缺口分开写；
5. 对冲突保留双方路径与行为，不用“主流”“通常”替代证据；
6. 不把 reference 项目的 bug、TODO 或兼容开关写成通用格式规则；
7. 若结论影响 FCS/FCBC 行为，另行修改并审查对应权威规范；
8. 若结论改变 profile 选择，更新 ConversionReport 和 conformance fixture 设计。

## 9. 已知需要进入 Conversion Specification 的修订

本轮证据已经证明当前 `fcs-conversion.md` 至少需要：

- 增加显式 source/target semantic profile 与 ambiguity reporting；
- 把 PGR v1 packed coordinate 的 520/530 和千位提取差异参数化；
- 不把 `RPEVersion` 当成 RPE semantic profile 的唯一选择器；
- 重新定义 `bpmfactor`、`rotateWithFather` 缺失值和 speed easing profile；
- 删除“PEC 默认 `sourceTick/2048`”结论，默认候选改为 decimal Beat；
- 区分 PEC Note X 的线局部相对坐标与判定线 X 的 0..2048 canvas 坐标；
- 参数化 PEC 150ms/175ms offset bias、`cv` scale 和负 alpha 行为；
- 把所有实现特有的 clipping、补首事件、丢 layer 和 parent 修复记录为 compatibility/repair。

具体证据和边界分别见三份格式文档。
