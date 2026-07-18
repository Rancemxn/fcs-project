# 0012：Canonical textual ID 编码与 typed stable ID 冲突策略

状态：Accepted

日期：2026-07-18

## 1. 背景

FCS §17 已要求显式 ID 优先、生成 ID 使用 source/expansion path 与 document order、显式和生成
namespace 分离，但没有固定生成 textual ID 的公开 spelling。该缺口会让同一 canonical chart 在不同
实现中产生不同的 Line/Note identity，也会使 `fcbc.md` §6.2 的 stable u64 hash 没有唯一输入。

用户已接受“保留显式 ID、保留 `generated/` 前缀、使用 collection/template/generator path 与最终
expanded-output order”的推荐方案。本 ADR 记录该决定，并由 FCS §17 承载完整规范性文本。

## 2. 决策

1. 显式 source ID 原样保留；任何以 `generated/` 开头的显式 ID 都是错误。
2. Line/Note 的生成 textual ID 固定为
   `generated/<entity-kind>/<expansion-path>/order/<zero-based-decimal>`。
3. Expansion path 固定为
   `collection/<collection-name>/item/<item-order>`，后接可选的
   `template/<template-name>/call/<call-order>` 与 `generate/<generator-index>`。
4. Source identifier 使用其 ASCII spelling；数字段为零基且无前导零；最终 `order` 是 owning collection
   的 expanded-output 顺序。
5. Line 和 Note 分别使用 `fcs.line` 与 `fcs.note` namespace，并按 FCBC §6.2 计算前 64 little-endian
   bits。ID 为 0、重复 textual ID 和 typed 64-bit collision 都失败，不允许 salting 或自动重命名。

## 3. I3.1 实施边界

I3.1 只建立 `fcs-model` 的 identity/provenance seam、哈希和 collision registry，并绑定 direct、template
和 generator expansion vectors。时间归一化、Track/Line graph、完整 Note lowering、runtime descriptor、
FCBC、Render、Conversion 与 CLI 属于后续路线图 work unit。

该决定不提升任何规范域至 Reviewed 或 Frozen，也不把 Issue/PR/实现测试变为规范权威；后续 canonical
lowering 必须消费本 ADR 与 FCS §17 的同一 contract。
