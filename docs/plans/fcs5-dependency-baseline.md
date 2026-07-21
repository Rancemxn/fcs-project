# FCS 5 直接依赖基线

本文件固定 FCS 5 初步实现采用的直接第三方 Rust 依赖、crates.io 发布源码 commit 和用途边界。
它是实施计划与 API 阅读入口，不是 FCS、FCBC、Render 或 Conversion 语义来源。

## 固定规则

- 根 `Cargo.toml` 的直接第三方依赖使用精确 `=version`；`Cargo.lock` 决定传递依赖闭包。
- `refer/dependencies/<repo>` 必须 detached checkout 到对应发布包 `.cargo_vcs_info.json` 的
  `git.sha1`。monorepo package 同时核对 `path_in_vcs`，不能用最新 branch HEAD、近似 tag 或仓库根
  package 版本替代。
- Cargo 仍从 registry 解析依赖；`refer/` 不得成为 path dependency、patch source 或兼容层。
- 版本、feature 或直接依赖用途改变时，同一 work-unit 更新 `Cargo.toml`、`Cargo.lock`、本表、参考
  checkout 和适用 gate evidence。只有 catalog entry 而尚未被 crate 激活的依赖不会进入 lockfile。
- 依赖实现可以承载通用机制，不能决定规范枚举、舍入顺序、错误 precedence、资源限制、修复策略或
  canonical ordering；这些仍由权威规范和项目测试固定。

## 版本与用途

| Cargo package | 精确版本 | 发布 Git SHA | 计划用途 |
|---|---:|---|---|
| `astro-float` | 0.9.5 | `6f76df1844b9af432b398e3e2e408bda894b2973` | I4.8 dev-only 高精度独立数值 oracle；不进入 production evaluator |
| `bitflags` | 2.13.1 | `f92a2921b41644b02ca5d50a6ace542e309e6a6f` | I7/I9 checked wire flag types；未知位仍由 loader/profile 明确拒绝或保留 |
| `chumsky` | 0.11.2 | `47eb15575a90d2fe40ae5450b49e484bafe138d9` | I0-I2 source lexer/parser/error recovery；不定义 grammar |
| `clap` | 4.6.1 | `ac5fda6a799e4c640d671edd1111d4a5e723dc1a` | I10 CLI 参数结构；stable exit/diagnostic schema 由项目定义 |
| `crc` | 3.4.0 | `2c8fd9615d620b5a5f8c9556b79a4ca173d6d401` | conformance 与 I7 CRC-32/ISO-HDLC |
| `image` | 0.25.10 | `76e57184f22772dad1138e96954e57945406b15e` | fixture decode 与 I9 受限 PNG/lossless WebP decode；不定义 sampling/raster |
| `nalgebra` | 0.35.0 | `5f927f6c821d0ced07a0e086e5f37251cab2a8c3` | I4 production 私有 fixed-size matrix storage；运算顺序和 public type 仍归项目 |
| `num_enum` | 0.7.6 | `f11d81c6cb644489f8a0fc24f9eea9a53d66f92e` | I7/I9 closed wire enum checked conversion；不生成 catch-all 语义 |
| `proptest` | 1.11.0 | `7f1367f9a4dc8440c47b93166a38ed064f63ea8c` | parser/runtime/codec property 与 shrinking |
| `ryu` | 1.0.23 | `f0b52bb194befe6fd242154f2182fafd43a819b8` | I8 finite f64 shortest-roundtrip decimal 候选；FCS canonical policy 再归一化 |
| `serde` | 1.0.228 | `a866b336f14aa57a07f0d0be9f8762746e64ecb4` | typed manifest/report/import data boundary |
| `serde_json` | 1.0.150 | `a1ae73ac6a6940a4a57c673aebaa13ed4dfe3e8c` | conformance、I6 JSON source 和 I10 JSON output；lossless order/duplicate handling由 owning parser 固定 |
| `sha2` | 0.11.0 | `ffe093984c004769747e998f77da8ff7c0e7a765` | canonical ID、resource、FCBC 和 manifest SHA-256 |
| `tempfile` | 3.27.0 | `5c8fa12eb584931b4f1bccfde87eb72fbfa7dc61` | I5 workspace resolver filesystem/symlink tests与 I10 同目录原子输出；resolver policy 仍由项目检查 |
| `thiserror` | 2.0.19 | `e13a7854338e446f184725fc8b9c9e86a0a537d2` | 新建或实质修改的 typed error boilerplate；不替代稳定 category/rule ID |
| `toml` | 1.1.3 | `eb251609a333580bf005cae27df853bb63dffdbe` | conformance manifest 和可执行测试 corpus |
| `ttf-parser` | 0.25.1 | `56c33b910b03ca152f78363ec471c5dfd97c3069` | I9 defaults-disabled bounded TrueType table/outline decode；不使用其 shaping/raster policy |
| `zip` | 8.6.0 | `771dfc534d2614158af5497ea3dff4d4208d7db1` | I6 明确纳入的 package input，defaults disabled；不改变 source semantics |

## 新采用依赖的审计边界

下表中的 MSRV 取自固定发布源码的 `rust-version`；`astro-float` 0.9.5 未在 manifest 声明
`rust-version`，因此只记录其同一固定 commit README 声明的 Rust 1.62.1。项目仍以 workspace
toolchain 和 full gate 为实际兼容性证据。

| Package | 激活 feature | License | 上游声明 MSRV | 允许的直接传递闭包 |
|---|---|---|---:|---|
| `astro-float` 0.9.5 | `default-features = false`, `std` | MIT | README: 1.62.1 | `astro-float-num` 0.3.6、`astro-float-macro` 0.4.5 及其纯 Rust arithmetic/proc-macro 依赖；不得激活 `random`、`serde`、Rug、GMP 或 MPFR |
| `bitflags` 2.13.1 | defaults only；owning stage 激活前不得增加 `serde`/`bytemuck` | MIT OR Apache-2.0 | 1.56.0 | 无必需运行时依赖 |
| `num_enum` 0.7.6 | `default-features = false` | BSD-3-Clause OR MIT OR Apache-2.0 | 1.70.0 | `num_enum_derive` 0.7.6 及其 proc-macro 闭包；不激活 `std` 或 `complex-expressions` |
| `ryu` 1.0.23 | defaults only | Apache-2.0 OR BSL-1.0 | 1.71 | 无运行时传递依赖 |
| `serde` 1.0.228 | default `std` + workspace `derive` | MIT OR Apache-2.0 | 1.56 | `serde_core` 1.0.228 与 `serde_derive` 1.0.228 proc-macro closure；derive 不定义项目 schema 或 diagnostic |
| `serde_json` 1.0.150 | default `std` + I6.1 `raw_value` | MIT OR Apache-2.0 | 1.71 | `itoa`、`memchr`、`serde`、`serde_core`、`zmij`；不启用 `arbitrary_precision`、`float_roundtrip`、`preserve_order` 或 `unbounded_depth` |
| `tempfile` 3.27.0 | default `getrandom`；只在 I5/I10 激活 | MIT OR Apache-2.0 | 1.63 | 仅平台 temporary-file、randomness 和 syscall 闭包；不得把系统临时目录替代 I10 同目录原子 rename policy |
| `ttf-parser` 0.25.1 | `default-features = false` | MIT OR Apache-2.0 | 1.63.0 | 无必需运行时依赖；不得隐式激活 OpenType/Apple layout、variable-font、glyph-name 或 std profile |
| `thiserror` 2.0.19 | default `std` | MIT OR Apache-2.0 | 1.71 | `thiserror-impl` 2.0.19 及其 proc-macro 闭包；derive output 不能改变稳定 diagnostic surface |

`astro-float` 在 I4.8、`proptest` 在 I4.9 进入 `fcs-runtime` dev graph；两者都不进入 production
dependency tree。I5.3 复用 `fcs-model` 已激活的 `sha2` 0.11.0 计算 exact resource bytes digest，并将
`tempfile` 3.27.0 仅加入 `fcs-source` dev graph，以隔离 workspace/symlink/非普通文件测试；resolver
production graph 不依赖 `tempfile`。其余五项先作为精确版本的 workspace catalog，分别到 owning
stage 才写入 crate manifest 和 lockfile；catalog entry 本身不能被描述为已实现能力。

I6.1 将 `serde` 1.0.228、`serde_json` 1.0.150、既有 `sha2` 0.11.0 与 `fcs-model` 激活到
`fcs-conversion` 产品 graph。`serde_json` 只增加 `raw_value` feature，用于让上游 parser 验证 JSON
grammar，同时由项目 IR 保留 member/array order、duplicate keys、raw string spelling 和 number
lexeme；不激活 `preserve_order` 或 `arbitrary_precision`。cataloged `zip` 仍未激活，直到 I6 明确交付
package input surface。

## 已完成代码的迁移边界

- 保留 project-owned ordered object、stable ID、canonical enum、descriptor、binary64 evaluator 和错误
  category。`indexmap`、`ordered-float` 或依赖的默认错误字符串不替换这些领域类型。
- I4 及以前的新增依赖限于 I4.8 dev-only `astro-float` 与 I4.9 dev-only `proptest`。现有 `EasingId::try_from`、
  canonical opcode、feature list 和 handwritten finite checks较短且携带领域错误，不改为
  `num_enum`、`bitflags` 或通用 numeric wrapper。
- `thiserror` 只在一个 error enum 正被功能性修改时用于保持等价的 `Display`、`source` 与 public
  shape；不单独发起全仓机械迁移。
- `tempfile` 已随 I5.3 resource resolver 的 dev-only integration tests 激活；I10 可继续用于同目录
  原子输出测试，并可在触及现有 filesystem tests 时替换手写 temporary path cleanup；不会为单纯
  依赖迁移重开已完成阶段。
