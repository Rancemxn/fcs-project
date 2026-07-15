# 0008：FCS 是制谱源格式，FCBC 是自包含分发与执行容器

状态：Accepted

日期：2026-07-15

## 1. 背景

项目目标是建立以 FCS 为中心的 Phigros 社区工具链。谱师需要可读、可编辑、适合抽象复用的
source 格式；播放器、移动端模拟器和 headless 渲染器需要有限、可验证、快速加载且不直接暴露
完整制谱 source 结构的分发格式。

FCS source 包含 `const`、纯函数、template、compile-time `if`、generator、`emit` 和表达式。
其中结构生成能力必须在编译期消失，但 runtime property expression 本身是谱面精确语义的一部分，
不应为了生成二进制文件而默认近似或丢失。

本决策作出前的 FCBC 2.0 候选已经定义 Constant、SegmentTrack、Piecewise、Expression 和
BakedCurve descriptor，以及 typed Expression DAG。因此 FCBC 不必是“全部预计算/烘焙的谱面”，
可以保存完成 static/canonical validation 后的精确运行时表达；本决策落实后，descriptor kind 5
只保留编号并在 FCBC 2 的所有 profile 中拒绝。

同一决策前候选的 FCBC Resources section 只保存资源清单，并明确不内嵌资源 bytes。这不足以成为
播放器唯一需要接收的自包含分发物；当前规范已按本 ADR 增加 required ResourceData。

## 2. 决策

### 2.1 FCS 的职责

FCS 是谱师和开发工具使用的权威制谱 source 格式：

- 表达人类可维护的谱面结构、metadata、resource 引用、编译期抽象和精确动态属性；
- 由 parser、static semantics、elaborator 和 canonical compiler 完整验证；
- 可以作为制谱器工程、版本控制和跨工具交换输入；
- 不作为发行播放器必须支持的输入格式。

制谱器工作时直接把一个普通文件夹作为 authoring workspace。该文件夹保存 FCS source 及其引用的
音频、图片、字体、纹理、shader 和其他资源文件，适合编辑器增量写入、外部资源工具修改和版本控制。
workspace 不是播放器输入，也不是另一种分发容器；打包时，packager 从 workspace 解析 FCS 与资源
引用，完成验证和编译，再把一个谱面及其全部资源整合为一个 FCBC。

播放器无需提供 FCS fallback parser/compiler。谱师测试 FCS 时，由制谱器、CLI 或开发宿主先调用
同一编译管线生成内存中的 canonical/FCBC 数据，再交给共享 runtime 和 renderer。

### 2.2 FCBC 的职责

FCBC 是默认分发格式、确定性执行 ABI 容器和最小自包含播放器输入。每个 FCBC 必须恰好包含一个
谱面；多难度、曲包或 catalog 由 FCBC 之外的未来上层机制组织，不在一个 FCBC 内建立多个 chart
entry。

一个可独立分发、可播放或可渲染的 FCBC 必须同时提供：

- 完成编译期展开和 canonical validation 的 chart semantics；
- Tempo、Line、Note、Track、Expression、Distance 和需要的 Render 数据；
- metadata、credits、sync 和 resource manifest；
- 执行、渲染所需的 audio、image、font、texture、path、shader、binary 等资源 bytes；
- 完整性校验、版本、feature/profile 和 loader resource limits；
- 按 container profile 可选的 fidelity、ConversionReport、debug 和 provenance metadata，但不包含
  FCS source snapshot 或 source AST。

FCBC 不再依赖一个未标准化的 PEZ 风格 ZIP 才能成为最小播放器输入。资源目录、payload、offset、
length、hash、alignment、随机访问和解码限制必须由 FCBC Container Specification 自身定义。所有
被谱面引用、且执行或渲染需要的资源都必须有内嵌 payload；播放器不得依赖 workspace 路径、外部
文件、URL 或另一份 archive 才能加载标准 FCBC。

FCBC 中保存的是编译后的 canonical/execution 表示，不是 FCS source AST dump。FCBC 2 不保存
comment、template 定义、generator 定义或调用、局部变量和其他只服务于制谱的 source 结构。
provenance 可以记录输入标识、内容 hash 和工具版本，但不得借此嵌入 FCS 文本、AST 或上述
authoring-only 结构。

### 2.3 资源按原始 bytes 内嵌

packager 必须把 workspace 中的资源文件作为 opaque byte sequence 原样写入 FCBC：

- FCBC 2 不对资源 payload 施加额外的容器级压缩；
- packager 不得为了打包而解码、重新编码、转码、降采样或规范化资源；
- PNG、JPEG、WebP、OGG、MP3 等资源已有的格式内压缩保持不变，内嵌 payload 与输入文件 bytes
  一致；
- 资源类型、长度和 hash 等目录信息可以单独编码，但不得改变 payload；
- codec 识别与资源解码由播放器实现承担，不属于 FCBC packager 的语义转换。

该规则只约束资源 payload。谱面内容仍按 Execution ABI 的结构化二进制表示编码，不要求把 FCS
文本原样放入容器。

### 2.4 标准打包必须展开 FCS 并保持语义

FCS→FCBC 的默认编译顺序为：

```text
workspace 中的 FCS source + resource files
→ parse/static/elaborate
→ 展开 template/generator/局部绑定等 authoring-only 结构
→ canonical validation
→ exact Constant / SegmentTrack / Piecewise / Expression DAG
→ 内嵌原始 resource bytes
→ one-chart self-contained FCBC
```

标准 packager 必须完成展开；FCBC 不提供让播放器延迟执行 template、generator 或名称解析的模式。
展开只消除 authoring-time 结构，不等于对 runtime expression 采样或烘焙。默认编译必须优先保存
精确 runtime semantics：

- 能表示为 Constant、Track、Piecewise 或 Expression DAG 的属性不得自动变成 BakedCurve；
- runtime expression 可以直接写入 FCBC Expressions section；
- template、generator、局部绑定和 source comment 不进入 FCBC，但其展开结果必须精确；
- 编译器优化可以改变内部 descriptor sharing、constant folding 或等价表示，但不得改变执行结果；
- “生成 FCBC”本身不构成 approximation，也不要求 ConversionReport 降级。

FCS source 与 FCBC 有意不提供文本或 authoring 结构可逆性。需要继续制谱或精确回写 source 时，
必须保留原始 workspace，而不是从 FCBC 反编译 template、generator、局部变量或 comment。这里的
无损指 canonical/execution semantics 和资源 bytes 不因编译与分发而损失。

### 2.5 播放端只依赖 FCBC

正式播放器、移动端模拟器和 headless autoplay renderer 的输入边界统一为 FCBC：

```text
FCBC bytes
→ bounded loader validation
→ canonical/runtime descriptors
→ shared evaluator + renderer
```

这样播放器不需要携带 source lexer、parser、名称解析、类型检查、template/generator 展开和 FCS
错误恢复；移动端加载成本、攻击面和实现分叉相应减少。

## 3. 当前版本不提供加密或混淆

FCBC 编译表示避免直接分发人类可读 FCS source，可以提高普通复制和修改的门槛，但它不是
密码学 DRM：播放器能够执行的内容最终也可以被有能力的客户端分析。

FCBC 2 不定义或引入资源/谱面加密、混淆、DRM、密钥管理或访问控制。标准 FCBC 的 chart section
和 resource payload 必须能够由 conforming loader 直接解析，不得要求私有解密或反混淆步骤。
完整性 hash/checksum 和有界 loader 仍然是格式正确性与安全解析要求，不属于加密或版权保护。

如果以后需要加密、混淆或相关分发能力，应在后续 FCBC 版本中重新设计、审查和版本化；它不能
静默扩展 FCBC 2，也不能反向改变 canonical semantics。

## 4. 后果

- `fcbc.md` 的 Resources/section layout 必须增加真正的资源 payload，而不只保留 package-member
  引用；
- `fcbc.md` 必须规定每个容器恰好一个 chart，并拒绝零 chart 或多 chart 容器；
- Resource payload 必须逐字节保留 workspace 输入，不增加容器级压缩或媒体转码；
- FCBC Container 2.0.0 的旧 Frozen 状态必须撤回并重新审查；项目尚未公开且兼容成本为零，
  可以保留候选 SemVer，但旧冻结 hash 只作历史审计；
- FCS Core 必须明确 exact expression 是默认 FCBC lowering，而 baking 不是分发前置条件；
- Render resource 引用必须最终解析到 FCBC 内嵌资源；
- 制谱器和 packager 必须保留“文件夹 workspace”与“单文件 FCBC 分发物”的边界；
- 路线图中的 renderer/player/CLI 不能各自建立另一套 chart parser 或运行时模型；
- `zip` 仍可用于导入既有社区谱面包或其他工具工作流，但不因此成为 FCBC 的强制外层格式。

### 4.1 2026-07-15 治理补充

用户确认 `fcbc.md` 继续联合定义 FCBC Container 与 Execution ABI，不为本轮修订拆分规范文件。
旧冻结审查只对整个 `fcbc.md` 保存一个文件级 hash，而 ResourceData、required section 与 loader
contract 的修改会改变该联合文件，因此 FCBC 2.0.0 和 Execution ABI 1.0.0 一起重开并重新审查，
候选 SemVer 均保持不变。这不预先断言 Expression DAG 指令语义已经变化；联合复审必须明确记录
实际变化与保持不变的 ABI 条款。

## 5. 不在本决策范围

- ResourceData 的最终 record layout、alignment 和流式加载 API；
- authoring workspace 的最终目录命名约定与编辑器私有状态格式；
- 后续 FCBC 版本可能采用的签名、加密或混淆设计；
- 商店分发、跨谱面资源去重、曲包、多难度合集和 catalog 格式。
