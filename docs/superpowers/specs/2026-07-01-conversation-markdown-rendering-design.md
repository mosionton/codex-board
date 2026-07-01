# Conversation Markdown Rendering Design

## 背景

`codex-board` 的 Conversation 弹窗当前把会话消息当纯文本处理，只按终端宽度包行。
实际会话内容经常包含 Markdown，以及代码块中的 JSON、TOML、YAML、XML、diff、日志、
Mermaid、LaTeX、HTML 片段和图片链接等格式。用户希望会话支持完整渲染这些内容。

终端 TUI 无法像浏览器一样执行 HTML/CSS、绘制 Mermaid 图、排版 LaTeX 公式或显示图片。
本设计采用“终端级完整支持”：识别常见格式，尽可能转成 ratatui 样式；无法图形化表达的
内容保留原文和语义标记，保证不丢内容、不破坏排版。

## 目标

- Conversation 弹窗使用 Markdown 解析结果渲染消息正文。
- 支持 CommonMark 和常见 GFM 结构，包括标题、段落、列表、引用、强调、删除线、行内代码、
  代码块、链接、图片、表格和任务列表。
- 对 Mermaid、LaTeX、HTML、JSON、TOML、YAML、XML、diff、日志等混合内容提供稳定的终端
  显示方式。
- 保持现有滚动、搜索、角色过滤和会话加载流程不变。
- 在无法表达完整视觉效果时保留内容和类型信息，而不是静默丢弃。

## 非目标

- 不执行 HTML、CSS 或 JavaScript。
- 不把 Mermaid 渲染成图形。
- 不把 LaTeX 渲染成公式版式。
- 不显示远程或本地图片。
- 不做代码语法高亮；第一版只保留语言标签和代码块样式。
- 不改变 `ConversationEntry` 的存储结构或搜索语义。

## 架构

新增一个 UI 层渲染模块，例如 `src/ui/markdown.rs`，负责把原始消息文本转换为
`Vec<Line<'static>>`。`src/ui/conversation.rs` 继续负责弹窗布局、标题、滚动和过滤，只把正文
渲染委托给 Markdown 渲染模块。

推荐依赖：

- `pulldown-cmark`：解析 Markdown，并启用表格、任务列表、删除线等选项。

渲染模块提供小而稳定的接口：

```rust
pub(super) fn markdown_lines(text: &str, width: usize) -> Vec<Line<'static>>;
```

`conversation_lines` 保持现有消息头逻辑，然后对每条消息正文调用 `markdown_lines`，并给正文
行加上现有四空格缩进。这样角色、时间戳、滚动高度计算和空消息提示都不需要重写。

## Markdown 映射

基础结构映射到 ratatui 文本样式：

- 标题：加粗，使用醒目颜色，并保留 `#` 层级提示。
- 段落：按终端宽度包行。
- 引用：每行前缀 `> `，使用灰色样式。
- 无序列表：显示 `- ` 并缩进嵌套层级。
- 有序列表：显示 `1. `、`2. ` 等编号，并缩进嵌套层级。
- 任务列表：显示 `[x]` 或 `[ ]`。
- 粗体、斜体、删除线：映射到 ratatui modifier。
- 行内代码：使用代码样式，不额外换行。
- 代码块：保留换行和缩进，显示语言标签，使用代码块样式。
- 链接：显示链接文本，并追加 `<url>`，避免目标地址丢失。
- 图片：显示 `![alt] <url>`；没有 alt 时显示 `<image: url>`。
- 表格：按终端等宽文本表格渲染，使用 Unicode 宽度计算列宽。

## 混合格式处理

代码块根据语言标签做终端级处理：

- `json`、`toml`、`yaml`、`xml`：作为结构化代码块显示，保留缩进。
- `diff`：作为代码块显示，后续可为 `+`、`-` 行加颜色。
- `log`、`text`、未知语言：作为普通代码块显示。
- `mermaid`：显示为带 `mermaid` 标签的代码块，保留源码。
- `latex`、`tex`、`math`：显示为公式代码块，保留源码。

Markdown 行内或块级 HTML 不执行。HTML 事件按文本显示，保留标签内容；控制字符需要清理，避免
破坏终端输出。ANSI escape 序列不解释为终端控制码，第一版按安全文本显示或去除控制字符。

## 包行和样式

现有 `wrap_text` 返回纯字符串，会丢失 span 样式。Markdown 渲染需要新增 span 级包行 helper：

- 输入为带样式的 span 序列。
- 根据 `unicode-width` 计算显示宽度。
- 在空白处优先换行，长单词按显示宽度拆分。
- 拆分 span 时保留原样式。

代码块按原始换行处理，每行仍需要按可用宽度拆分，避免宽行撑破弹窗。表格列宽也使用
`unicode-width` 计算，保证中文和宽字符对齐。

## 数据流

1. `load_session_conversation` 继续读取原始文本，生成 `ConversationEntry`。
2. 搜索和角色过滤继续基于 `ConversationEntry.text` 原文运行。
3. `conversation_lines` 生成消息头。
4. `markdown_lines` 把正文渲染为 styled lines。
5. Conversation 弹窗按渲染后的 line count 计算滚动范围。

## 错误处理

- Markdown 解析遇到未知事件时保留其文本内容。
- 空消息仍显示现有空状态。
- 不支持的格式必须降级为可读文本或代码块，不返回空内容。
- 超宽内容必须包行或拆分，不能导致 UI 布局异常。
- 控制字符必须安全处理，避免影响终端状态。

## 测试

按 TDD 添加单元测试，先验证失败，再实现：

- 基础 Markdown：标题、粗体、斜体、删除线和行内代码保留文本并产生样式。
- 列表和引用：无序列表、有序列表、嵌套缩进和引用前缀正确。
- 任务列表：`[x]` 和 `[ ]` 显示正确。
- 链接和图片：链接 URL、图片 alt 和 URL 不丢失。
- 代码块：语言标签、缩进和多行内容保留。
- 表格：列内容完整，宽字符列宽不会错位。
- Mermaid、LaTeX、HTML：降级显示时保留源码或标签文本。
- 包行：styled spans 拆分后不丢样式，行宽不超过限制。
- Conversation 集成：消息头、正文缩进、滚动 line count 和空消息行为保持稳定。

## 发布说明影响

README 的功能说明需要增加：

- Conversation 弹窗支持 Markdown 和常见代码/结构化文本的终端渲染。
- Mermaid、LaTeX、HTML 和图片会以终端安全形式显示，不执行或图形化渲染。
