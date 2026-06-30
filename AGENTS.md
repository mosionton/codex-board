# AGENTS.md

## 代码检查

- 修改 Rust 代码后，按 CI lint 配置检查代码。
- 格式检查：

```sh
cargo fmt --all -- --check
```

- Clippy 检查：

```sh
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

## 提交

- 使用 Conventional Commits。
- 常用类型：
  - `feat`：用户可见功能
  - `fix`：缺陷修复
  - `test`：测试
  - `chore`：发布、版本号或维护
- 类型保持英文，冒号后的摘要写中文。
- 摘要用祈使句，保持短句。

示例：

```text
feat: 初始化 codex-board 项目
test: 增加应用和界面流程覆盖
chore: 发布 1.0.0
```

## 提交正文

- 非平凡提交写正文。
- 正文说明改了什么、为什么改。
- 摘要段落后写 2 到 5 条中文 bullet。

格式：

```text
type: 中文摘要

说明这次变更的目的和影响。

- 主要变更
- 主要变更
```

## 标签

- 发布标签只使用 `vX.Y.Z`。
- 不创建临时发布标签。
- 发布标签必须指向最终发布提交。
- 重写、合并或重排发布历史后，删除并重建对应发布标签。

## 标签消息

- 使用 annotated tag。
- tag 消息写中文。
- 第一行写版本号。
- 后续小节使用：
  - `发布详情`
  - `功能`
  - `缺陷修复`

格式：

```text
vX.Y.Z

发布详情

本次发布的简短说明。

功能

- 功能

缺陷修复

- 修复
```
