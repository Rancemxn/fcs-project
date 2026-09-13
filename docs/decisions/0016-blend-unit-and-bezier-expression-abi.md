# 0016：unit-typed blend 与 blend 内 cubicBezier 的 Expression ABI 编码

状态：Accepted

日期：2026-09-13

## 1. 背景

PR #628 把 Replace/Add/Multiply blend 精确组合为逐区域的 Expression DAG，但仅覆盖 float 与
vec2-float target。`crates/fcs-fcbc/src/writer.rs` 的 `native_blended_fixture` 对两类贡献保持
governed rejection（拒绝写出而不是近似）：

1. **unit-typed target（Position=vec2-length、Rotation=angle）。** Core runtime 的 blend
   `combine`（`crates/fcs-runtime/src/track.rs`）对 Angle 与 Vec2Length 的 add/multiply 已按
   payload/component 逐项 binary64 运算、每项一次 roundTiesToEven 精确支持；Expression ABI 的
   Mul 表只有 `U,int/float → U` 与 `vec2-U,int/float → vec2-U` 行，multiply fold 的
   `U,U → U` 组合无法表达（replace/add fold 本可表达）。
2. **blend 内的 cubicBezier segment。** 非 blend SegmentTrack 已原生编码 cubicBezier
   （interpolation=4 加四个 raw control bits），但 blend 区域的 Expression DAG 没有参数化的
   正确舍入 cubic Bezier progress opcode；`Easing` opcode 只覆盖 31 个固定 ID。

FCS Execution ABI 当前为 1.0.0 **Draft**（governance §2），因此本决策是 Draft 域修订，以 dated
ADR 记录，不触发版本号变更；按 ADR 0004，本修订只影响 Execution ABI 一个兼容域。

## 2. 决策

### 2.1 Mul 表扩展

`fcbc.md` §14 的 Mul 唯一组合表新增两行：

```text
Mul U,U（同一 U）            → U
Mul vec2-U,vec2-U（同一 U）  → vec2-U
```

语义按 payload/component 逐项 binary64 乘法、每项单独 roundTiesToEven 定义，与 Core runtime
blend `combine` 的 multiply 语义逐操作一致，因此 writer 镜像编码后 product 查询与 canonical
runtime bit for bit 相同。

### 2.2 CubicBezier opcode

新增 Expression opcode `64 CubicBezier`：arity 3，operandA 是 clamp 到 `[0,1]` 的 scalar
progress（float），operandB 是 `(x1,y1)`、operandC 是 `(x2,y2)` 两个 vec2-float 常量，result
为 float，immediate=0。求值语义与 `fcs.md` §9.4 的 cubicBezier 完全一致：把 p 与四个 binary64
control value 解释为精确实数，取满足 cubic x(t)=p 的唯一 `t∈[0,1]`，再把实数 cubic y(t) 正确
舍入一次为 binary64。Loader 在 load 时以 `fcbc.invalid-expression` 拒绝非有限 control value、
`x1`/`x2` 不在 `[0,1]` 或 x 曲线不可单值反解的节点。

`CubicBezier` 复用既有 StructuralKey 规则（operand 递归嵌入、Constant node 嵌入被引 Value
canonical bytes、immediate=0 不另产生可变 key），不引入新的 key 规则。

## 3. 与既有决策的关系

- ADR 0004：单一兼容域（Execution ABI）的 Draft 期修订，不涉及 FCS source、FCBC container
  framing、Render Profile 或 Conversion Specification。
- ADR 0009：扩展的是精确 Expression DAG 的表达范围，不引入烘焙、采样或近似路径。
- ADR 0005：cubicBezier 的正确舍入求解沿用既有 canonical certified evaluator 语义，不放宽误差
  界。

## 4. 后果

- `fcs-fcbc` writer/loader/evaluator 与独立 reference evaluator 必须同步实现两个 Mul 组合与
  opcode 64；
- `native_blended_fixture` 的 unit-typed target 与 blend 内 cubicBezier governed rejection
  解除，Position/Rotation blend 与 Bezier-in-blend 进入精确组合路径；
- I10 evidence ledger 的对应 residual 移至已关闭列；
- 在所有实现同步前，旧 reader 读到 opcode 64 按未知 opcode 以 `fcbc.invalid-expression`
  拒绝，不产生部分解释。

## 5. 不在本决策范围

- Position/Rotation 以外新增 Track target；
- blend 内 color target 的表达；
- Render 侧 governed rejection 的其他条目（error fill、unresolved hold、multi-Track target
  等），它们由后续 ABI 决策分别处理。
