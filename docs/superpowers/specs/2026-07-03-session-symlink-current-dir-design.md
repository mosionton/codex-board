# Session Symlink Current Dir Design

## 背景

Sessions 页面在当前目录范围下用字面路径比较判断会话是否属于当前 workspace。
如果用户通过软连接路径进入项目，而 Codex session 记录的是真实路径，或反过来，就会导致
当前目录范围下看不到本应匹配的会话。Provider tabs 和搜索弹窗匹配计数也各自使用同样的
字面比较，因此需要统一修正。

## 目标

- 当前目录范围能识别软连接等价路径。
- Sessions 列表过滤、provider tabs 和搜索弹窗匹配计数使用一致的目录匹配语义。
- 保持表格中显示的 `cwd` 为 session 原始记录路径。
- 保持恢复会话时的执行目录为 session 原始 `cwd`。

## 非目标

- 不重写 session 文件中的 `cwd`。
- 不把 UI 展示路径改成当前软连接路径。
- 不改变全部会话范围、搜索字段、树形排序和 resume 流程。

## 目录匹配规则

新增共享的目录等价判断，例如 `session_matches_current_dir(session_cwd, current_dir)`：

- 如果 `session_cwd == current_dir`，直接匹配。
- 否则尝试对两边执行 `std::fs::canonicalize`。
- 两边都能 canonicalize 时，比较 canonical path。
- 任一边 canonicalize 失败时回退为不匹配，保持旧的严格行为。

这个规则避免不存在的 session 路径被误判为当前目录，同时让软连接路径和真实路径互相匹配。

## 接入点

需要替换当前直接比较 `session.cwd == current_dir` 的地方：

- `SessionsState::refresh_visible` 的当前目录范围过滤。
- `ProviderTabs` 构建当前目录 provider 标签时的过滤。
- `session_search_match_count` 统计当前搜索草稿匹配数时的过滤。

`Session` 结构不新增字段，避免扩大测试构造和 UI 展示的改动面。

## 测试

新增或更新测试覆盖：

- `SessionsState` 在当前目录为软连接、session cwd 为真实路径时仍显示该会话。
- `ProviderTabs` 在当前目录范围下包含软连接等价 session 的 provider。
- 搜索弹窗匹配计数在软连接等价路径下统计该会话。
- 不存在路径仍不会被软连接逻辑误判为当前目录。

## 风险

`canonicalize` 需要路径存在。对已经删除的 session cwd，行为仍与当前实现一致：当前目录范围不匹配，
恢复前也继续由现有目录存在性检查拦截。路径比较只用于过滤和计数，不影响显示和 resume。
