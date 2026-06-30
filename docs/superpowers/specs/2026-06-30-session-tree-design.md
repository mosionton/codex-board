# Session Tree Design

## 背景

`codex-board` 目前在 Sessions 页面以平铺表格展示会话，只解析 `session_meta`
中的 `id`、`cwd`、`model_provider`、`timestamp` 等基础字段。Codex 的 subagent
会话文件已经包含父子关系元数据，例如：

- `thread_source = "subagent"`
- `parent_thread_id`
- `source.subagent.thread_spawn.depth`
- `source.subagent.thread_spawn.agent_nickname`
- `source.subagent.thread_spawn.agent_role`

用户需要在 Sessions 页面直接看出 subagent 与父 session 的关系，并保留平铺视图作为
切换选项。

## 目标

- Sessions 页面默认使用树形结构展示会话关系。
- 增加快捷键在树形和平铺视图之间切换。
- 在详情、搜索和列表中暴露 subagent 关系信息。
- 保持现有恢复会话、provider 过滤、当前目录/全部会话范围切换和搜索流程可用。

## 非目标

- 第一版不做展开/折叠子树。
- 第一版不做跳转到父 session 或按 parent 聚合的独立页面。
- 不从对话文本中推断关系，只使用结构化 `session_meta` 字段。

## 数据模型

在 `Session` 中增加关系元数据：

- `thread_source: String`，来自 `session_meta.payload.thread_source`，缺失时默认为
  `user`。
- `parent_thread_id: Option<String>`，来自 `session_meta.payload.parent_thread_id` 或
  `source.subagent.thread_spawn.parent_thread_id`。
- `agent_nickname: Option<String>`，来自 `session_meta.payload.agent_nickname` 或
  `source.subagent.thread_spawn.agent_nickname`。
- `agent_role: Option<String>`，来自 `session_meta.payload.agent_role` 或
  `source.subagent.thread_spawn.agent_role`。
- `agent_depth: Option<u32>`，来自 `source.subagent.thread_spawn.depth`。

`thread_source == "subagent"` 或存在 `parent_thread_id` 时，把会话视为子会话。父子关系
以 `parent_thread_id == parent.id` 连接。

## 树形排序

Sessions 状态增加视图模式：

- `Tree`：默认模式。
- `Flat`：保留当前平铺行为。

当前 scope、provider tab 和搜索过滤仍先产生候选会话集合。之后根据视图模式生成
`visible_indices`：

- `Flat` 视图保持现有按 timestamp 倒序的列表。
- `Tree` 视图先找顶层节点，再深度优先输出子节点。
- 顶层节点包括普通用户会话，以及父会话不在候选集合中的子会话。
- 每个兄弟层级继续按 timestamp 倒序排列。
- 如果子会话匹配搜索但父会话没有进入候选集合，子会话仍显示为顶层 orphan，避免搜索
  结果被隐藏。

为支持缩进展示，Sessions 状态同步维护每个可见行的 `depth`。正常子节点使用计算出的
树深度；orphan 子会话以深度 0 显示，同时在来源文本中展示 parent id。

## UI 行为

Sessions 表格增加 `source` 列，展示会话来源：

- 普通用户会话显示 `user`。
- subagent 显示缩进后的 `sub <nickname>`；没有 nickname 时显示 `subagent`。
- 有 role 时可显示为 `sub <nickname>/<role>`，宽度不足时按现有截断规则截断。
- orphan subagent 显示 parent id 的短格式，例如 `sub Boole <- 019f1067`。

表格列调整为：

```text
time | provider | source | cwd | summary
```

详情弹窗增加关系字段：

- `source`
- `parent`
- `agent`
- `role`
- `depth`

没有值的字段显示 `-`。

## 快捷键

Sessions 页面新增 `v`：

- 在 `Tree` 和 `Flat` 之间切换。
- 切换后重建可见行并重置选择。
- footer 和 README 的 Sessions 快捷键表同步更新。

## 搜索与过滤

搜索文本扩展包含：

- `thread_source`
- `parent_thread_id`
- `agent_nickname`
- `agent_role`

provider tab、当前目录/全部会话范围和搜索仍只决定哪些会话可见。树形关系只负责这些候选
会话的排列和缩进，不把未匹配的父会话强行带入搜索结果。

## 错误处理

- 缺少关系字段时按普通 user session 处理。
- `source` 字段既可能是字符串，也可能是对象。解析 subagent 元数据时只读取对象形式的
  `source.subagent.thread_spawn`，其他形式忽略。
- `depth` 无法解析时显示 `-`，树形缩进仍可由 parent 链计算。
- 出现循环父子关系时，树构建必须避免无限递归；已访问节点不重复输出。

## 测试

新增或更新单元测试覆盖：

- 解析 subagent session metadata。
- 搜索可以匹配 parent id、nickname 和 role。
- 默认视图为 `Tree`。
- 树形可见行按父子关系排序，并给子会话正确 depth。
- 父会话被过滤掉时，匹配的子会话仍可见。
- `v` 切换 Tree/Flat 后重建可见行并重置选择。
- 详情页包含 source、parent、agent、role、depth 字段。

## 发布说明影响

README 的功能和快捷键章节需要说明：

- Sessions 默认按父子关系树形展示。
- 可按 `v` 切换树形和平铺视图。
- 详情页可查看 subagent 派发关系。
