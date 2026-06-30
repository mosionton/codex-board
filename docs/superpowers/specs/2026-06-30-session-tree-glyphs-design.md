# Session Tree Glyphs Design

## 背景

Sessions 页面已经默认按父子关系生成树形顺序，并在 `source` 列用缩进展示 subagent。
当前展示主要依赖空格缩进，树形特征不够明显。用户在视觉方案中选择了 A：继续使用现有
`source` 列，但把空格缩进升级为明确的 tree glyph。

## 目标

- 让 Tree 视图的层级关系在表格中一眼可见。
- 不新增表格列，继续使用现有 `source` 列。
- 保持 Flat 视图不显示树线，避免平铺模式被误读为层级结构。
- 保持现有 provider 过滤、搜索、详情和恢复行为不变。

## 展示规则

Tree 视图下，`source` 列前缀使用树形符号：

- 根 user 会话显示 `● user`。
- 有子节点的中间子会话显示 `├─ sub <agent>/<role>`。
- 同级最后一个子会话显示 `└─ sub <agent>/<role>`。
- 多层子会话显示祖先延续线，例如 `│  └─ sub <agent>/<role>`。
- 找不到父会话的 orphan subagent 作为根显示，格式为
  `● sub <agent>/<role> <- <parent-short-id>`。

Flat 视图下，`source` 列不显示 tree glyph：

- user 会话显示 `user`。
- subagent 显示 `sub <agent>/<role>`。
- orphan subagent 仍可显示 parent 短 id，便于识别来源。

## 状态模型

`SessionsState` 当前维护 `visible_indices` 和 `visible_depths`。为了让 UI 准确知道每行是否
同级最后一个节点，以及祖先层级是否需要画 `│` 延续线，状态层增加每个可见行的 tree
prefix 字符串。

建议新增：

- `visible_tree_prefixes: Vec<String>`
- `visible_tree_prefix(visible_index: usize) -> &str`

Tree 构建时，深度优先遍历会传入祖先 continuation 信息，生成每行前缀：

- 根：`● `
- 非最后子节点：`├─ `
- 最后子节点：`└─ `
- 有祖先 continuation 时追加 `│  `
- 无祖先 continuation 时追加三个空格

Flat 构建时，`visible_tree_prefixes` 为等长空字符串。

## UI 行为

`src/ui/tables.rs` 的 `session_source_label` 改为同时接收 view mode 和 tree prefix：

- Tree 视图：`<tree-prefix><source-label>`
- Flat 视图：`<source-label>`

`source` 列宽度保持 24。如果标签超宽，继续使用现有 `truncate_chars` 截断。

## 测试

新增或更新测试覆盖：

- Tree 状态为父子行生成 `● `、`├─ `、`└─ ` 和多层 `│  └─ ` 前缀。
- Flat 状态生成空 tree prefix。
- `session_source_label` 在 Tree 视图显示 tree glyph。
- `session_source_label` 在 Flat 视图不显示 tree glyph。
- UI 相关测试仍通过。

## 非目标

- 不做展开/折叠。
- 不新增独立 `tree` 列。
- 不改变会话排序、搜索和过滤语义。
