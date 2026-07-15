# FCBC 2 / Execution ABI 1 Nonempty Artifact Review

日期：2026-07-16

范围：`fcbc.md` 第 13–15 章及其绑定的非空 FCBC/Execution ABI conformance artifact

结论：非空 Execution ABI byte/evaluation blocker 已关闭；FCBC 2.0.0 与 Execution ABI 1.0.0
仍保持 Draft，等待 Render、Conversion、Core blocker 和最终跨规范独立复审

## 1. 审查边界

本轮只证明一条机器可执行的测试参考链已经闭合：

```text
固定声明式 fixture
  → test-only reference writer
  → checked-in static FCBC bytes
  → independent test-only loader
  → independent test-only evaluator
  → bits/trace/direct-seek vector 与 malformed mutation corpus
```

三个 reference support 文件都位于 `crates/fcs-source/tests/support/`，仅由
`tests/fcbc_execution_abi.rs` 通过 `#[path]` 加入测试 binary。它们没有进入产品 `src/`，也不构成
I7 的通用 FCBC writer、loader 或 runtime：

- writer 从固定声明式 chart 构造 bytes，不读取 static golden、manifest 或产品实现；
- loader 只消费传入 bytes，拥有独立 cursor、record、section、CRC、graph 与 canonical validation，
  不调用 writer；
- evaluator 只消费 loader 的 `DecodedChart`，不调用 writer，也不依赖 frame history、sample cache
  或产品 evaluator。

Writer 是固定 fixture 构造器，不是通用 canonical sorter。该限制不是 blocker，因为最终 static
bytes 的 owner、reachability、StructuralKey interning、descriptor/node canonical order 和 Distance
order 都由独立 loader 重新验证；不得用该 writer 宣称 I7 产品能力已经实现。

## 2. 规范快照

本 artifact 基于以下原始文件 bytes：

| 文件 | SHA-256 |
|---|---|
| `fcs.md` | `594ee2a13272a8501eec6a866e3dc09a233ff64699e9a8af5c69fd8aa9d11ddc` |
| `fcbc.md` | `1090fb88c2cc3805dc9c4b1e91eb3247d7773e17f1cf764add8f70e109ea4b78` |

对 `fcbc.md` 第 13–15 章规范文字的前置独立复审未发现 Critical、Important 或 Minor finding；本轮
不重新解释规范，而是验证这些条款能被一个非空 binary artifact 唯一实现和独立读取。

## 3. Static golden 与 manifest

`conformance/fcbc/nonempty-execution.hex` 是 lowercase textual hex；解码后：

```text
length  = 3432 bytes
SHA-256 = ffcd8f24bd792c406a584fea26c753b62fafaa7d8f91ff0886b697c67dbfea61
profile = strict-runtime / chart
```

原始 textual `.hex` 文件 SHA-256 为
`0f7494a73a908a1981b2bbf36c2e48d7863df30caa7cd0e666489ab20710d53a`。Reference writer 与解码后的
static bytes 做全文件 byte-for-byte 比较，不允许 loader 在测试时重新调用 writer 生成自己的期望值。

| Section | Offset | Length | CRC-32/ISO-HDLC |
|---:|---:|---:|---|
| 1 StringTable | 688 | 32 | `6d4e92fa` |
| 2 ConstantPool | 720 | 300 | `3e38eac6` |
| 3 Meta | 1024 | 40 | `e31c05b5` |
| 4 Contributors | 1064 | 4 | `2144df1c` |
| 5 Credits | 1072 | 4 | `2144df1c` |
| 6 Resources | 1080 | 4 | `2144df1c` |
| 7 Sync | 1088 | 48 | `6ea5c831` |
| 8 TempoMap | 1136 | 44 | `06376fc0` |
| 9 Lines | 1184 | 236 | `e249a0aa` |
| 10 Notes | 1424 | 332 | `ea954ed2` |
| 11 Tracks | 1760 | 668 | `964b15a5` |
| 12 Expressions | 2432 | 804 | `363aa343` |
| 13 Distance | 3240 | 188 | `5d120888` |
| 20 ResourceData | 3432 | 0 | `00000000` |

Manifest 绑定 14 constants、14 descriptors、40 expression nodes、2 distances、2 lines 和 2 notes。
Descriptor kinds 覆盖 Constant、SegmentTrack、Piecewise 与 Expression；Distance table 按 `lineId`
排序，依次为 portable-evaluable 和 portable-analytic。

相关文件 hash：

| 文件 | SHA-256 |
|---|---|
| `conformance/fcbc/manifest.toml` | `e435c8d452ec991c6a023cab15ce2a2f819ae0febe68ae7b044af288ba5f8c63` |
| `nonempty-execution.toml` | `1c5d208133085a88bb0387f7730712191f5e6670a8770569515919e279c40a74` |
| `nonempty-execution-vector.toml` | `c5231aad620469b0a654bbc022bb5986e27ff060bde74cdb6f8a1d7b3b8a09e3` |
| `nonempty-execution-mutations.toml` | `ff6576269a306cab9d7a6a4fdc95fa65dffeb82fcc68a8cd42f87fcdfb56f2b6` |

按 ordinal relative path、逐文件输入 `UTF-8(path) + NUL + raw bytes + NUL` 复算：

```text
conformance/fcbc/: 11 files
tree SHA-256: 1cedf8f6830a2d36a1839c78c9c53c5658bd01d88f1ef44968b35836185ef9c3

conformance/: 92 files
tree SHA-256: 6b40d46536ec1cc7c5b6d4642eae17c8547e8696fca9f95d1f981899dbde4090
```

## 4. Loader 与执行覆盖

Loader 实际验证 header/version/profile、14 个 known section、8-byte alignment、minimal padding、
coverage、CRC、Record envelope、StringTable/ConstantPool/Value、descriptor type/domain/kind、Segment
与 Piece encoding、Piecewise acyclic、Expression signature/DAG、Line/Note owner 与 type、stable reference、
descriptor/node 可达性、StructuralKey interning、canonical traversal order，以及 Distance 的 line alias、
排序、classification、domain、error field 与 deterministic boundary。

声明式 execution vector 有 10 个 descriptor query 和 7 个 distance query，覆盖：

- `Seconds`、`Radians`、`ToFloat`、typed `Vec2`、`Vec2X`、`Vec2Y` 与 `ApproxEq`；
- Line transform root 不得读取 Note-only `d`，Note presentation root 可以读取 `d`；
- SegmentTrack 的 before fill、linear interior 和 after fill；
- 双向 unbounded Piecewise；
- lazy And、Or、Choose 的完整 recursive-entry trace；Choose 的未选 node 由 `trace_excludes` 明确证明
  没有执行；
- portable-evaluable 与 portable-analytic distance 在 origin 前、origin、segment interior、boundary
  和 boundary 后的 expected binary64 bits；
- 同一组 distance query 以声明顺序和完全反向顺序执行，结果逐项相同，证明不依赖前一 frame/query。

Vector 的 `max_absolute_error_bits` 与对应 Distance record 的 `maxDistanceError` 原始 bits 逐 query
绑定；portable-analytic 固定为零，portable-evaluable 固定为 `0x1p-32`。

## 5. Mutation corpus

四个 mutation 都实际修改 static golden，再由同一个独立 loader 分类：

| Mutation | CRC 处理 | 期望并实际得到的 category |
|---|---|---|
| descriptor kind `5` | 同步更新 Tracks section CRC | `fcbc.forbidden-descriptor` |
| expression opcode `999` | 同步更新 Expressions section CRC | `fcbc.invalid-expression` |
| distance classification `3` | 同步更新 Distance section CRC | `fcbc.invalid-distance` |
| StringTable payload raw corruption | 故意不更新 CRC | `fcbc.section-checksum` |

前三项不会被错误地提前归类为 checksum failure；第四项证明 checksum gate 先于深层 payload parse。

## 6. 验证证据

当前未暂存工作树上执行：

```text
cargo clippy --workspace --all-targets -- -D warnings
PASS

cargo nextest run -p fcs-source --test fcbc_execution_abi
6 passed, 0 skipped

cargo nextest run -p fcs-source --test conformance_manifest
2 passed, 0 skipped

cargo nextest run --workspace
141 passed, 0 skipped

cargo fmt --all -- --check
PASS

git diff --check
PASS
```

一个未参与 writer、loader、evaluator、golden、vector 或 mutation 修改的只读 reviewer 独立检查
规范、文件依赖、bytes、hash/CRC、canonical/reachability、bits/trace/direct-seek 和 stable diagnostic，
并复跑定向 Clippy 与 6-test nextest lane。Finding ledger：

| Severity | Open findings |
|---|---:|
| Critical | 0 |
| Important | 0 |
| Minor | 0 |

## 7. Disposition

`docs/reviews/2026-07-15-fcs5-cross-spec-closure-review.md` 第 7.2 项的“Execution ABI 非空
byte/evaluation vector” blocker 由本 artifact 和独立复审关闭。这个结论不关闭：

- RenderSection binary/decoder/shaping/raster blocker；
- Conversion 真实 source→canonical→target→reparse blocker；
- Core 39-entry fixture 的独立 execution/validation blocker；
- 五版本域的最终联合独立复审和 Frozen 状态迁移；
- I7 通用产品 writer、loader、runtime 与 canonical→FCBC→execution 实现。

因此当前可以自动进入 S15 的 RenderSection blocker，但不能开始 I1，也不能把 test-only harness
描述为 FCBC/ABI 产品实现或完整 conformance implementation。
