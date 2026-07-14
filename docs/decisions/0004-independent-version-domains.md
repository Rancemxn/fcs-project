# 0004：FCS、FCBC、Execution ABI 和 Render 独立版本化

状态：Accepted

## 决策

FCS source、FCBC container、Execution ABI、Render Profile、Conversion Specification、extension
schema 和 chart 自身版本分别记录。Compiler version 只用于复现和诊断。

## 理由

Source 语法新增不一定改变 runtime；容器 framing 改变不一定改变 chart 语义；Render 能力也
不应迫使 Core source major 升级。单一 `version` 字段无法表达这些兼容域。

## 后果

- FCBC header 同时记录 source FCS、FCBC 和 ABI 版本；
- `meta.chartVersion` 不参与格式兼容；
- 未知 optional section 可以跳过，未知 required section 必须拒绝；
- 版本升级必须指出受影响兼容域。
