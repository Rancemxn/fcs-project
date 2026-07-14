# Agent Skills for Context Engineering 集成设计

## 目标

将现有的 `Agent-Skills-for-Context-Engineering-main` 上游副本整理为仓库级可发现的 Agent Skills，并在根 `AGENTS.md` 中记录明确、可执行的调用时机。

## 方案

- 完整上游项目放在 `.agents/vendor/Agent-Skills-for-Context-Engineering/`，保留其入口文档、许可证、插件清单、研究框架和示例，作为可审阅的来源副本。
- 上游 `skills/` 下的 17 个 skill 目录逐一安装到 `.agents/skills/<skill-name>/`。每个目录保持自己的 `SKILL.md`、`references/`、`scripts/` 和 `tests/`，不使用符号链接或扁平化文件布局。
- 根 `AGENTS.md` 增加 Context Engineering skill 路由章节：说明渐进式加载、按主题选择 skill、与 FCS 权威规范及现有 Trellis 规则的优先级，并为每个 skill 写出正向触发条件和相邻任务边界。

## 边界与兼容性

- 本次不把该项目的研究脚本、示例项目或插件元数据复制进 `.agents/skills/`；它们只保留在 vendor 副本中。
- 本次不修改 Rust 源码、Cargo 配置或 FCS 格式规范，因此不运行 Rust workspace 测试。
- 安装使用实际目录复制，兼容 Windows，并避免仓库级 skill 目录依赖上游路径或符号链接。

## 验收标准

1. `.agents/vendor/Agent-Skills-for-Context-Engineering/` 存在完整上游副本，原根目录临时副本不再存在。
2. `.agents/skills/` 下存在 17 个新 skill 目录，每个目录包含有效的 `SKILL.md` 和合法 YAML frontmatter。
3. 安装副本的内容与 vendor 副本的对应 `skills/<name>/` 内容一致。
4. 根 `AGENTS.md` 保留已有未提交内容，并说明 17 个 skill 的调用时机、边界和优先级。
5. 安装布局检查和上游确定性校验命令返回成功。
