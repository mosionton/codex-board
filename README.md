# codex-board

[![CI](https://github.com/mosionton/codex-board/actions/workflows/ci.yml/badge.svg)](https://github.com/mosionton/codex-board/actions/workflows/ci.yml)
[![Release](https://github.com/mosionton/codex-board/actions/workflows/release.yml/badge.svg)](https://github.com/mosionton/codex-board/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#license)

`codex-board` 是 Codex CLI 和 Claude Code 的本地会话恢复面板。

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

Codex 会话读取自：

```text
$CODEX_HOME/sessions
```

Claude Code 数据目录：

- 设置了 `CLAUDE_CONFIG_DIR`：使用 `CLAUDE_CONFIG_DIR`
- 默认：`~/.claude`

Claude Code 会话读取自：

```text
$CLAUDE_CONFIG_DIR/projects
```

Claude Code 会话在列表里的 provider 显示为 `claude`，可以像其他 provider 一样过滤。目录不存在时自动跳过。

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

Claude Code 会话不需要切换 provider，直接选中按 `Enter` 恢复。

恢复前会检查会话原工作目录是否存在。通过检查后按会话类型执行：

```sh
codex resume <session_id> [--yolo] # Codex 会话；确认框按 Space 切换可选参数
claude --resume <session_id> # Claude Code 会话
```

## 功能

### 会话按 provider 和关系展示

Sessions 页面显示当前目录或全部本地会话，同时包含 Codex 和 Claude Code 两种来源。默认按父子关系树形展示会话；`source` 列会用
`●`、`├─`、`└─` 和 `│` 显示父子层级。表格包含时间、agent（`codex` / `claude`）、provider、来源、工作目录和摘要。

Claude Code 会话解析自 `~/.claude/projects` 下的 `.jsonl` 记录，跳过 subagent 侧链记录；
对话查看、搜索、详情和恢复（`claude --resume`）与 Codex 会话一致。

支持：

- 当前目录和全部会话范围切换，当前目录范围会识别软连接等价路径。
- 按 provider 过滤会话。
- 在树形和平铺视图之间切换。
- 搜索会话 id、agent、provider、工作目录、摘要、时间和 subagent 关系信息。
- 查看会话详情，包括 parent、agent、role、agent path 和 depth；兼容 Codex multi-agent v2 会话关系。
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

列表末尾会有一行只读的 `claude` 条目，展示本机 Claude Code 的状态：

- 登录状态和 OAuth 账号（读取 `~/.claude.json` 的 `oauthAccount`）。
- 默认模型（`settings.json` 的 `model` 或 `env.ANTHROPIC_MODEL`）。
- 自定义 `ANTHROPIC_BASE_URL`（如果配置了代理网关）。

这一行仅作展示，`a`/`e`/`d` 对它无效——Claude Code 的配置由它自己管理，codex-board 不会写入任何 Claude 配置文件。未安装 Claude Code 时不显示该行。

### 恢复前查看上下文

Conversation 窗口用于恢复前检查会话内容。

支持：

- 查看用户和助手消息。
- 对话正文会渲染 Markdown，包括标题、列表、引用、链接、表格、任务列表和代码块。
- Mermaid、LaTeX、HTML、图片和结构化文本会以终端安全文本形式显示，保留内容但不执行或图形化渲染。
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
model = "gpt-5.6-sol"
model_reasoning_effort = "max"
plan_mode_reasoning_effort = "high"
auto_compact_percent = 70
auth_mode = "openai"

[providers.local]
base_url = "http://localhost:11434/v1"
wire_api = "chat"
model = "qwen3-coder"
auto_compact_percent = 70
env_key = "LOCAL_MODEL_API_KEY"
auth_mode = "api_key"
```

字段：

| Field | Meaning |
| --- | --- |
| `base_url` | API 地址，必填 |
| `wire_api` | Codex 协议，通常是 `responses` 或 `chat`，必填 |
| `model` | 默认模型 |
| `model_reasoning_effort` | 模型推理强度；候选值和默认值由当前安装的 Codex bundled model catalog 决定 |
| `plan_mode_reasoning_effort` | Plan mode 推理强度；使用所选模型的支持列表 |
| `auto_compact_percent` | 自动历史压缩阈值占模型上下文窗口的百分比；整数 `1..=99`，默认 `70` |
| `auth_mode` | `api_key` 或 `openai` |
| `env_key` | 保存密钥的环境变量名 |
| `api_key` | 明文密钥；优先使用 `env_key` |

当前 Codex 中，GPT-5.6 Sol 和 Terra 支持 `low`、`medium`、`high`、
`xhigh`、`max`、`ultra`；Luna 支持到 `max`。`gpt-5.6` 按 Sol 处理。
未知模型或模型目录不可用时，codex-board 回退到 `low`、`medium`、
`high`、`xhigh`，默认 `medium`，不会阻止 provider 编辑或应用。

应用 provider 时，codex-board 会读取当前 Codex bundled model catalog 的
`context_window`，把 `auto_compact_percent` 换算为顶层
`model_auto_compact_token_limit`，并写入
`model_auto_compact_token_limit_scope = "total"`。GPT-5.6 当前窗口为
`372000`，默认 `70%` 会写入 `260400`；未知模型按 `272000` 计算，写入
`190400`。自动压缩不能阻止单个超大输入或工具输出一次跨过阈值。

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
