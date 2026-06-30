# codex-board

[![CI](https://github.com/mosionton/codex-board/actions/workflows/ci.yml/badge.svg)](https://github.com/mosionton/codex-board/actions/workflows/ci.yml)
[![Release](https://github.com/mosionton/codex-board/actions/workflows/release.yml/badge.svg)](https://github.com/mosionton/codex-board/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#license)

`codex-board` 是 Codex CLI 的本地会话恢复面板。

它处理的主要场景是：同一个 workspace 里用过多个 provider 跑 Codex，会话记录里保存了各自的 provider，但当前 Codex 配置只会指向一个 provider。恢复旧会话前，需要先知道这条会话属于哪个 provider，并把 Codex 切回对应配置。

`cboard` 把这几步放在一个 TUI 里：

1. 在当前 workspace 找到历史会话。
2. 看清会话使用的 provider。
3. 应用对应 provider 到 Codex 配置。
4. 从会话原目录执行 `codex resume`。

## 安装

从 GitHub Releases 下载对应平台的压缩包，把 `cboard` 或 `cboard.exe` 放到 `PATH`。

从源码安装：

```sh
cargo install --git https://github.com/mosionton/codex-board --locked --bin cboard
```

本地开发安装：

```sh
cargo install --path . --locked
```

## 启动

```sh
cboard
```

Codex 数据目录：

- 设置了 `CODEX_HOME`：使用 `CODEX_HOME`
- Unix/macOS 默认：`~/.codex`
- Windows 默认：`%USERPROFILE%\.codex`

会话读取自：

```text
$CODEX_HOME/sessions
```

供应商配置保存在：

```text
$CODEX_HOME/switcher-providers.toml
```

应用 provider 时写入：

```text
$CODEX_HOME/config.toml
```

## 恢复流程

1. 进入 Sessions 页面。
2. 用 `a` 选择当前目录或全部会话。
3. 用 `Tab` / `Shift+Tab` 按 provider 过滤。
4. 选中要恢复的会话，确认列表里的 `provider`。
5. 按 `t` 进入 Providers 页面。
6. 选中同名 provider，按 `a` 应用到 Codex 配置。
7. 回到 Sessions 页面，选中会话，按 `Enter` 恢复。

恢复前会检查会话原工作目录是否存在。通过检查后执行：

```sh
codex resume <session_id>
```

## 功能

### 会话按 provider 和关系展示

Sessions 页面显示当前目录或全部本地会话。默认按父子关系树形展示会话；`source` 列会用
`●`、`├─`、`└─` 和 `│` 显示父子层级。表格包含时间、provider、来源、工作目录和摘要。

支持：

- 当前目录和全部会话范围切换。
- 按 provider 过滤会话。
- 在树形和平铺视图之间切换。
- 搜索会话 id、provider、工作目录、摘要、时间和 subagent 关系信息。
- 查看会话详情，包括 parent、agent、role 和 depth。
- 打开会话对话。
- 从会话原目录恢复。

### provider 写回 Codex 配置

Providers 页面维护可以应用到 Codex 的 provider。列表会标记当前已应用的 provider。

支持：

- 新建、编辑、删除 provider。
- 将选中 provider 写入 `$CODEX_HOME/config.toml`。
- 从 Codex `config.toml` 导入已有 provider。
- 从 Codex `auth.json` 识别 OpenAI 登录状态。
- 从 provider `/models` 端点拉取模型列表。

自定义 provider 会写入 Codex 的 `[model_providers]`。内置 `openai` 使用 Codex 保留配置，并清理自定义 provider 表。

### 恢复前查看上下文

Conversation 窗口用于恢复前检查会话内容。

支持：

- 查看用户和助手消息。
- 搜索对话内容。
- 按全部、用户消息、助手消息过滤。
- 重新加载当前对话。

## 快捷键

### Sessions

| Key | Action |
| --- | --- |
| `Up` / `Down` | 移动选择 |
| `PageUp` / `PageDown` | 按页移动 |
| `Tab` / `Shift+Tab` | 切换 provider 过滤 |
| `/` | 搜索 |
| `a` | 切换当前目录/全部会话 |
| `c` | 打开对话，或清除搜索 |
| `i` | 查看详情 |
| `r` | 重新加载 |
| `v` | 切换树形/平铺视图 |
| `Enter` | 恢复会话 |
| `t` | 切到 Providers |
| `q` / `Esc` | 退出，或清除搜索 |

### Providers

| Key | Action |
| --- | --- |
| `Up` / `Down` | 移动选择 |
| `PageUp` / `PageDown` | 按页移动 |
| `n` | 新建 |
| `e` | 编辑 |
| `a` | 应用到 Codex 配置 |
| `d` | 删除 |
| `i` | 查看详情 |
| `t` | 切到 Sessions |
| `q` | 退出 |

### Provider Editor

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | 切换字段 |
| `Left` / `Right` | 切换选项 |
| `F5` | 拉取模型列表 |
| `Ctrl+U` | 清除或重置当前字段 |
| `Enter` | 保存 |
| `Esc` | 取消 |

### Conversation

| Key | Action |
| --- | --- |
| `Up` / `Down` | 滚动 |
| `PageUp` / `PageDown` | 按页滚动 |
| `/` | 搜索 |
| `Tab` | 切换角色过滤 |
| `Ctrl+U` | 清除搜索 |
| `r` | 重新加载 |
| `Esc` | 关闭 |

## 供应商配置

示例：

```toml
[providers.openai]
base_url = "https://api.openai.com/v1"
wire_api = "responses"
model = "gpt-5.5"
model_reasoning_effort = "medium"
plan_mode_reasoning_effort = "medium"
auth_mode = "openai"

[providers.local]
base_url = "http://localhost:11434/v1"
wire_api = "chat"
model = "qwen3-coder"
env_key = "LOCAL_MODEL_API_KEY"
auth_mode = "api_key"
```

字段：

| Field | Meaning |
| --- | --- |
| `base_url` | API 地址，必填 |
| `wire_api` | Codex 协议，通常是 `responses` 或 `chat`，必填 |
| `model` | 默认模型 |
| `model_reasoning_effort` | 模型推理强度：`low`、`medium`、`high`、`xhigh` |
| `plan_mode_reasoning_effort` | plan mode 推理强度：`low`、`medium`、`high`、`xhigh` |
| `auth_mode` | `api_key` 或 `openai` |
| `env_key` | 保存密钥的环境变量名 |
| `api_key` | 明文密钥；优先使用 `env_key` |

认证规则：

- `auth_mode = "openai"`：使用 Codex 的 OpenAI 登录状态，不写入密钥字段。
- `auth_mode = "api_key"` 加 `env_key`：Codex 从环境变量读取密钥。
- `auth_mode = "api_key"` 加 `api_key`：写入 Codex 使用的 `experimental_bearer_token`。
- `openai` 是 Codex 保留 id，自定义 API key provider 不能使用。

## 平台

Release 包覆盖：

| Platform | x86_64 | ARM64 | Packaging |
| --- | --- | --- | --- |
| Linux | `x86_64-unknown-linux-musl` | `aarch64-unknown-linux-musl` | 静态 musl 二进制 |
| macOS | `x86_64-apple-darwin` | `aarch64-apple-darwin` | 原生二进制 |
| Windows | `x86_64-pc-windows-msvc` | `aarch64-pc-windows-msvc` | 静态 CRT 二进制 |

每个 artifact 都有对应 SHA-256 校验文件。tag 发布包含合并后的 `SHA256SUMS`。

## 排查

- 没有会话：检查 `CODEX_HOME` 和 `$CODEX_HOME/sessions`。
- resume 到错误 provider：先在 Sessions 页面确认目标会话的 provider，再到 Providers 页面应用同名 provider。
- 不能恢复：检查原工作目录是否存在，并确认 `codex` 在 `PATH` 中。
- provider 不生效：检查 `$CODEX_HOME/config.toml` 和相关环境变量。
- `F5` 拉取失败：检查 `base_url`、认证方式和网络。

## 开发

```sh
cargo fmt --all -- --check
cargo test --all-features --locked
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
actionlint
cargo package --allow-dirty --locked
```

## License

MIT, as declared in `Cargo.toml`.
