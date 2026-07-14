# 0003：Template 和 Generator 只存在于编译期

状态：Accepted

## 决策

`const`、`let`、纯函数、typed template、compile-time `if`、`with`、`generate` 和 `emit` 必须
在 canonical lowering 前完全展开。FCBC Core 不提供 loop、jump、recursive call、mutable local
或 runtime entity creation。

## 理由

制谱需要高层抽象，但播放器需要有限、可验证、可预算和确定的执行模型。把结构生成限制在
编译期可以同时满足两者，并避免 runtime 宏导致资源不可控。

## 后果

- Generator range 必须有限并受共享预算限制；
- Template/function/generator 环必须静态拒绝；
- 动态值选择使用无副作用 `choose`/descriptor，而不是结构分支；
- FCBC loader 可以在不执行 source 程序的情况下验证资源上限。
