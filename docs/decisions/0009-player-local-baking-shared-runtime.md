# 0009：精确表达式默认执行，烘焙仅是播放器本地可选策略

状态：Accepted

日期：2026-07-15

## 1. 背景

FCS 大量使用 runtime property expression。若编译器默认把表达式烘焙为采样曲线，会把本来精确的
谱面语义转成有损近似，而且表达式误差可能经过 transform、scroll integration 或后续计算被放大。
对谱师而言，无损执行是正常预期；为了假设中的低端设备而在分发阶段默认产生有损数据才是异常。

2D 音游的主要性能压力通常更可能来自 shader、资源解码、draw call、geometry、compositing 和
GPU/渲染同步，而不是有限 typed scalar expression 的 CPU 求值。FCBC Expression DAG 又明确禁止
loop、recursion、allocation、IO 和 runtime entity creation，单次求值成本天然有界。

## 2. 本机证据

2026-07-15 使用以下环境做了一次非 conformance、单线程 Rust 微基准：

```text
CPU: Intel Celeron J4125 @ 2.00GHz
cores/logical processors: 4/4
rustc: 1.96.0 x86_64-pc-windows-msvc
build: -O -C target-cpu=native -C codegen-units=1
evaluator: 24-byte node、topological Vec、预分配 scratch、无 JIT/SIMD/cache/culling
```

中位数结果：

| Pattern | Nodes/expression | Time/expression | Expressions/1ms |
|---|---:|---:|---:|
| arithmetic/compare/choose | 64 | 0.379us | 2636 |
| arithmetic/compare/choose | 256 | 1.420us | 704 |
| arithmetic/compare/choose | 1024 | 5.290us | 189 |
| mixed math | 64 | 0.756us | 1323 |
| mixed math | 256 | 2.950us | 339 |
| mixed math | 1024 | 12.749us | 78 |
| mixed math | 4096 | 48.410us | 21 |
| transcendental-heavy | 256 | 7.299us | 137 |
| transcendental-heavy | 1024 | 30.987us | 32 |
| transcendental-heavy | 16384 | 525.966us | 1.9 |
| transcendental-heavy | 65536 | 2108.780us | 0.5 |

该基准使用宿主 `f64` transcendental 实现，不证明 `fcs.md` 正确舍入数学函数的 conformance 或最终
成本；它只说明需要极端节点数和极端数学函数密度，表达式解释才会单独占据明显帧预算。正常实现
还可以使用 descriptor sharing、公共子表达式、按环境分类缓存、可见性裁剪、批量求值、SIMD 或
JIT，因此本证据不支持默认有损烘焙。

## 3. 决策

### 3.1 标准分发与默认播放使用精确表达式

- FCS→FCBC 默认保留 exact Constant、Track、Piecewise 和 Expression DAG；
- 播放器默认直接执行 FCBC 精确 descriptor；
- 谱师无需声明是否烘焙、采样率或设备性能目标；
- 标准发行 FCBC 不因播放器性能配置而改变；
- 优化优先级是缓存、共享、裁剪、批量求值和 evaluator 优化，而不是先丢失语义。

### 3.2 烘焙是播放器全局配置

面向极低端设备，播放器可以提供默认关闭的全局近似模式，例如：

```text
expressionEvaluation = exact

expressionEvaluation = sampled
sampleRate = 1000Hz
```

该能力完全属于播放器本地实现与设备配置；播放器可以选择不实现 sampled mode。它不进入 FCS、
FCBC、制谱器 workspace、packager 参数或单张谱面 metadata，也不形成跨播放器必须互认的配置
协议。若播放器实现该能力，配置应针对该播放器/设备上的所有谱面，而不是写入某一谱面或由谱师
选择。播放器可以据此从精确 FCBC 建立本地派生缓存，但：

- 缓存不得覆盖、重写或替代分发 FCBC；
- 缓存不得写入 FCBC 的 section、resource payload 或 packager 输出；
- 缓存失效时必须能回退到精确 evaluator；
- cache key 至少绑定 FCBC content hash、Execution ABI、runtime/evaluator version 和全部采样参数；
- Note、Hold、tempo、Track point、visibility、Render active 等离散边界必须显式保留，不能只按
  统一采样格点移动；
- sampled mode 是用户选择的性能/兼容模式，不能宣称 strict exact execution conformance；
- 采样率、插值、缓存布局、容量和淘汰算法属于播放器实现配置，不进入 FCS source。

### 3.3 共享 runtime 和 renderer

项目未来的播放器、autoplay 视频渲染器、谱面游玩器/模拟器和制谱器预览必须复用同一底层：

```text
FCBC loader
→ canonical runtime descriptors
→ reference/optimized evaluator
→ shared renderer
```

上层产品只在宿主职责上不同：

- 播放器负责播放、seek 和普通预览；
- autoplay renderer 使用 headless/offscreen backend 和视频编码；
- 模拟器增加输入、判定、计分和平台集成；
- 制谱器增加 FCS 编辑、增量编译、diagnostic 和编辑 UI，再把编译结果交给同一 runtime/renderer。

GPU backend、headless backend 和窗口 backend 可以不同，但都消费同一种 canonical/FCBC display
data。它们不得各自解释 FCS、PGR、RPE 或 PEC，也不得让 renderer 反向定义 gameplay 语义。

当前工作仍优先完成谱面格式、canonical model、FCBC 和 conformance；本决策只固定未来边界，
不要求当前阶段提前创建播放器、编辑器或渲染器空壳。

## 4. 与既有决策的关系

本决策收窄 `0005-error-bounded-numerics.md` 中 baking 的默认适用范围：error-bounded baking 仍是
外部目标能力不足、显式转换策略或播放器 sampled mode 的有效机制，但不再是标准 FCS→FCBC
编译中“表达式不能静态化”时的默认替代。FCBC Expression DAG 是默认运行时表示。

## 5. 后果

- `fcs.md` 与 `fcbc.md` 必须把 exact Expression descriptor 放在默认路径，降低 adaptive baking
  在标准编译叙述中的优先级；
- CLI 的 converter/compiler 不要求谱师提供通用 sampling 参数；
- sampling 配置属于未来播放器的本地 CLI、配置文件或平台设置，不属于跨工具格式规范；
- FCBC conformance 以 exact execution 为基线，本地 sampled cache 不进入 deterministic FCBC golden；
- roadmap 中 baking 的主要消费者改为 target conversion 和未来低端播放器模式；
- 性能测试应分别测 evaluator、renderer、shader、resource decode 和 frame pipeline，不能只凭
  “表达式很多”推断 CPU 瓶颈。

## 6. 不在本决策范围

- 播放器配置文件格式与参数名；
- sampled cache 的文件格式和是否持久化；
- 正确舍入 transcendental evaluator 的最终实现；
- GPU shader、draw batching 和 renderer backend 选择；
- 具体移动端、桌面端和视频编码框架。
