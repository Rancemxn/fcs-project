# 0005：数值正确性由误差界而非固定采样率定义

状态：Accepted

## 决策

常量、线性、已知 easing 和可解析 piecewise 使用精确 descriptor；不能精确 lowering 的属性
使用自适应烘焙并记录定义域、属性级最大误差、验证 profile 和 source hash。Strict profile
中的 1ms 是无法解析证明时的最大验证间隔，不是强制 1000Hz 输出采样率。

## 理由

固定 120Hz 或 1000Hz 对高曲率、长时间积分和离散事件都不能自动保证正确，同时会对简单
属性浪费空间。属性级误差和强制精确边界可以直接验证视觉与 gameplay 需求。

## 后果

- Note/Hold/tempo/Track 边界不能量化到统一网格；
- Speed 同时验证 velocity 和累计 distance；
- Runtime frame rate 与编译烘焙精度分离；
- 无法在预算内满足误差时 strict 编译失败。
