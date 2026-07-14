# FCS 5 Conformance Corpus

本目录是 FCS 5 规范冻结所绑定的机器可读测试向量，不属于示例教程。`manifest.toml` 中每个
fixture 使用稳定 ID，并引用权威规范条款、输入文件和预期结果。

Conformance runner 必须：

1. 按 UTF-8 bytes 原样读取 source；
2. 对 success fixture 至少完成声明的 `parse`、`elaborate`、`canonical`、`fcbc-load` 或
   `evaluate` stage；
3. 对 error fixture 比较稳定 diagnostic category，不比较完整人类 message；
4. 比较 Float64 时优先使用预期 hexadecimal bits；
5. 比较 canonical object 时忽略 JSON whitespace，但不忽略 array/source order；
6. 报告 implementation limits，不能把超限伪装成规范 fixture 失败；
7. 不以当前 Rust 实现行为覆盖 fixture 预期。

目录：

```text
fcs5/manifest.toml          Source/static/canonical fixtures
fcs5/source/valid/          应成功的 FCS source
fcs5/source/invalid/        应产生稳定 category 的 FCS source
fcs5/expected/              canonical snapshot 和数值向量
fcbc/minimal-runtime.hex    FCBC 2.0 最小 runtime golden
fcbc/minimal-runtime.toml   golden manifest、offset、CRC 和 SHA-256
fcbc/mutations.toml         基于 golden 的确定性损坏向量
render/manifest.toml        Render semantic/raster fixture manifest
conversion/mapping-vectors.toml  外部格式 exact mapping 与 capability 边界
```

`.hex` 文件只包含 ASCII lowercase hex 和换行。Reader 必须先移除 ASCII whitespace，再每两位
解码一个 byte。
