# 0005：数值正确性由误差界而非固定采样率定义

状态：Accepted（默认 baking 范围由 0009 收窄）

## 决策

常量、线性、已知 easing、可解析 piecewise 和 portable runtime expression 使用精确 descriptor
或 typed Expression DAG。只有目标格式/运行时不能执行精确表示、并且用户显式允许 approximation
时，才使用自适应烘焙，并记录定义域、属性级最大误差、验证 profile 和 source hash。Strict
approximation profile 中的 1ms 是无法解析证明时的最大验证间隔，不是强制 1000Hz 输出采样率。

2026-07-15 的后续架构决定明确：标准 FCS→FCBC 编译优先保留 typed Expression DAG，不因表达式
无法静态化为 Track/Piecewise 就默认烘焙。上述自适应烘焙规则主要用于目标格式能力不足的显式
转换和用户选择的 approximation。播放器端默认关闭的 sampled cache 是从 exact FCBC 派生的本地
实现策略，不生成 BakedCurve，也不使用或写入 Conversion approximation payload。详见
`0009-player-local-baking-shared-runtime.md`。

## 理由

固定 120Hz 或 1000Hz 对高曲率、长时间积分和离散事件都不能自动保证正确，同时会对简单
属性浪费空间。属性级误差和强制精确边界可以直接验证视觉与 gameplay 需求。

## 后果

- Note/Hold/tempo/Track 边界不能量化到统一网格；
- Speed 同时验证 velocity 和累计 distance；
- Runtime frame rate 与编译烘焙精度分离；
- 已显式授权的 strict target approximation 无法在预算内满足误差时转换失败；标准 exact
  FCS→FCBC 编译不走该路径。
