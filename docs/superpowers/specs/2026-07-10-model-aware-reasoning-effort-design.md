# 模型感知推理等级设计

## 背景

codex-board 当前为所有模型固定提供 `low`、`medium`、`high`、`xhigh`
四个推理等级，并把缺失或未知值统一归一为 `medium`。模型输入、F5 模型拉取和
Up/Down 模型切换均不会更新普通模式或 Plan 模式的推理等级。

这与当前 Codex 模型能力不一致。尤其是 GPT-5.6 系列，Codex 自带模型目录给出的
能力如下：

| 模型 | 默认等级 | 支持等级 |
| --- | --- | --- |
| `gpt-5.6-sol` | `low` | `low`, `medium`, `high`, `xhigh`, `max`, `ultra` |
| `gpt-5.6-terra` | `medium` | `low`, `medium`, `high`, `xhigh`, `max`, `ultra` |
| `gpt-5.6-luna` | `medium` | `low`, `medium`, `high`, `xhigh`, `max` |

OpenAI API 文档还说明 `gpt-5.6` 是 Sol 的系列别名。API 本身支持的等级与 Codex
产品目录并不完全相同，例如 API 包含 `none`，而 Codex 目录包含产品级的
`ultra`。由于 codex-board 最终写入 Codex 配置，本功能以当前安装的 Codex bundled
model catalog 为运行时依据，而不是把 API 参数全集直接暴露给用户。

官方参考：

- <https://developers.openai.com/api/docs/guides/latest-model>
- <https://developers.openai.com/api/docs/guides/reasoning>
- <https://developers.openai.com/codex/models>

## 目标

- 推理等级候选项、默认值和最终写入值由当前模型决定。
- 支持 GPT-5.6 Sol、Terra、Luna 的 `max` 和适用的 `ultra`。
- 随已安装 Codex 的 bundled model catalog 更新，不在 codex-board 中维护完整模型表。
- 保留已有且被新模型支持的显式选择，只在不兼容时回退。
- 对未知模型、自定义 provider 和旧 Codex 版本保持可用。
- 保持现有 provider TOML 字段和序列化格式兼容。

## 非目标

- 不升级或替换用户当前选择的模型。
- 不修改会话恢复逻辑，也不从历史会话恢复推理等级。
- 不为同一 provider 按模型分别记忆多个推理等级偏好。
- 不从 provider `/models` 响应推断能力；该端点当前只提供模型 ID。
- 不加入 GPT-5.6 Pro、持久化推理或其他 API 请求字段。

## 方案选择

### 采用：Codex bundled model catalog

应用启动时执行：

```sh
codex debug models --bundled
```

该命令不需要网络或 API 认证，返回当前 Codex 二进制自带的模型目录，包括模型
slug、默认推理等级和支持等级。它能让 codex-board 跟随本机 Codex 版本，并避免
维护易过期的硬编码模型表。

### 未采用：读取 `models_cache.json`

缓存可能不存在或长期未刷新。调查时本机缓存仍只包含 GPT-5.5 和 GPT-5.4 Mini，
无法作为 GPT-5.6 的可靠来源。

### 未采用：硬编码完整模型映射

硬编码实现简单，但每次 Codex 增加或调整模型都需要发布新版 codex-board，与
“推理等级跟着模型走”的目标冲突。

## 组件设计

### ModelCatalog

新增 `src/provider_config/model_catalog.rs`，提供以下职责：

- 启动 `codex debug models --bundled` 并读取标准输出。
- 用 Serde 解析所需字段，忽略目录中的其他字段。
- 把有效模型转换为按 slug 查询的目录。
- 将官方别名 `gpt-5.6` 解析为 `gpt-5.6-sol`。
- 为未知模型返回兼容 profile。
- 为配置值提供统一的支持性判断和回退操作。

核心概念为：

```text
ModelCatalog
  model slug -> ReasoningProfile

ReasoningProfile
  default_effort
  supported_efforts
```

等级继续使用字符串，而不是封闭枚举。这样 Codex 将来增加新等级时，只要 bundled
catalog 包含该字符串，codex-board 就能展示和保存它。

兼容 profile 固定为：

```text
default: medium
supported: low, medium, high, xhigh
```

### 生命周期与所有权

`runtime::run` 在读取和合并 provider 配置前加载一次目录，并把目录存入
`ProvidersState`。目录规模很小，可使用共享所有权或低成本克隆，避免在每次打开
编辑器或渲染帧时启动子进程。

模型目录加载结果包含目录和可选警告。目录失败时应用继续启动，警告在 TUI 中显示
一次；后续逻辑只看到可用的兼容目录，不需要到处处理加载错误。

### ProviderEditor

编辑器增加动态状态：

- 当前普通模式支持等级。
- 当前 Plan 模式支持等级。
- 当前模型默认等级。
- 普通模式值是否由用户显式选择。
- Plan 模式值是否由用户显式选择。

“显式选择”状态仅属于当前编辑会话，不改变 provider TOML 格式：

- 新建 provider 或原字段缺失时，状态为非显式。
- 原字段存在且受当前模型支持时，状态为显式。
- 原字段非法或不受支持时，使用模型默认并视为非显式。
- Left/Right 修改等级后，状态变为显式。
- Ctrl+U 将字段恢复为当前模型默认，并视为非显式。

保存时仍按现有格式写出解析后的字符串。该设计保持序列化兼容，同时保证本次编辑
过程中先选模型、再选推理等级的流程正确。

## 联动规则

每次提交一个模型变化时，普通模式和 Plan 模式分别执行：

1. 查询新模型的 `ReasoningProfile`。
2. 如果当前值是显式选择且新模型仍支持，则保留。
3. 如果当前值不是显式选择，或新模型不支持，则改为新模型默认值。
4. 更新 UI 使用的动态候选项。

模型变化的提交时机为：

- F5 拉取结果自动选择模型时。
- Up/Down 切换已拉取模型时。
- 手动编辑 Model 字段并离开该字段时。
- 用户直接保存、但尚未离开 Model 字段时。

手动输入不会在每个字符变化时立即归一化，避免从一个完整 slug 输入到另一个 slug
的过程中，临时未知值覆盖用户选择。

示例：

- GPT-5.5 的显式 `xhigh` 切到 Sol 后仍为 `xhigh`。
- Sol 的显式 `ultra` 切到 Terra 后仍为 `ultra`。
- Sol 的显式 `ultra` 切到 Luna 后回退为 Luna 默认的 `medium`。
- 新建 provider 先选择 Sol 时，非显式的初始值更新为 Sol 默认的 `low`。
- 未知模型使用兼容 profile；不在四档内的值回退为 `medium`。

普通模式和 Plan 模式都使用 Codex 模型目录提供的单一支持等级列表。当前 Codex
目录没有分别提供 Plan 能力表，因此两者共享模型能力判断，但仍保留独立的当前值和
显式选择状态。

## 导入、显示与应用

### Codex 配置导入

`load_codex_config_providers` 使用目录按 provider 的有效模型归一化两个推理字段。
已有且受支持的值保留，缺失或不支持的值使用模型默认。

通过 OpenAI 认证自动生成内置 `openai` provider 时，应继承 Codex 顶层当前模型和
两个推理字段。这样编辑内置 provider 时看到的是当前有效组合，而不是
`model = None` 加固定 `medium`。

### 详情显示

Provider 详情使用同一目录解析实际显示值，不再调用只认识四档的全局 normalizer。
因此 `max` 和 `ultra` 不会在详情页被错误显示成 `medium`。

### 应用到 Codex

应用 provider 前再次解析有效模型和两个等级：

- provider 模型非空时，以该模型为准。
- provider 模型为空且现有 Codex 配置包含模型时，以现有模型为准。
- 没有可识别模型时使用兼容 profile。

最终写入值必须在 profile 支持列表中；否则写模型默认值。该校验是最后一道边界，
防止手工编辑的旧配置或未来调用路径绕过 TUI 校验。

现有字段名保持不变：

- `model_reasoning_effort`
- `plan_mode_reasoning_effort`

## UI 行为

- Reason 和 Plan Reason 的 Options 行改为读取编辑器动态候选项。
- GPT-5.6 Sol/Terra 显示 `low | medium | high | xhigh | max | ultra`。
- GPT-5.6 Luna 显示 `low | medium | high | xhigh | max`。
- 未知模型显示现有四档。
- Ctrl+U 帮助文案语义改为恢复当前模型默认，而不是固定恢复 `medium`。
- 模型目录加载失败只显示一次非阻断状态，不新增弹窗或确认步骤。

## 错误处理

以下情况不会阻止 codex-board 启动：

- 找不到 `codex` 可执行文件。
- 当前 Codex 版本不支持 `debug models --bundled`。
- 命令退出失败。
- 标准输出不是合法 JSON。
- 目录为空或没有任何有效模型。

单个目录项满足以下任一条件时忽略该项，其余有效项继续使用：

- slug 为空。
- 默认等级为空。
- 支持等级为空。
- 默认等级不在支持等级中。

如果没有剩余有效项，则使用兼容目录。警告不包含认证信息、完整环境或其他敏感
数据，只说明模型目录不可用及已启用兼容回退。

## 测试设计

### ModelCatalog 单元测试

- 解析包含额外字段的真实形状 JSON。
- 验证 Sol、Terra、Luna 的默认值和支持列表。
- 验证 `gpt-5.6` 别名解析到 Sol。
- 验证未知模型使用兼容 profile。
- 验证无效单项被忽略。
- 验证无效 JSON、失败状态和空目录触发整体回退。
- 命令执行与 JSON 解析分离，单元测试不依赖本机安装的 Codex。

### ProviderEditor 测试

- 新建 provider 选择 Sol 后使用 `low`。
- 已有受支持的显式值在模型切换后保留。
- `ultra` 从 Sol 切到 Luna 后回退为 `medium`。
- F5 自动选择、Up/Down 切换和离开手动输入字段都会同步 profile。
- Left/Right 将当前字段标记为显式。
- Ctrl+U 恢复当前模型默认。
- 普通模式与 Plan 模式分别覆盖。

### UI 测试

- Options 行展示编辑器动态列表。
- Sol/Terra 包含 `max` 和 `ultra`。
- Luna 包含 `max` 但不包含 `ultra`。
- 未知模型继续显示四档。

### 配置测试

- 导入 GPT-5.6 的 `max` 或适用的 `ultra` 时不再降为 `medium`。
- 内置 OpenAI provider 继承当前顶层模型和等级。
- 应用前保留受支持值并修正不支持值。
- provider 模型为空时按现有 Codex 模型校验。
- TOML 字段名和旧别名保持兼容。

### 验证命令

实现完成后运行：

```sh
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

## 文档更新

README 的 provider 配置说明改为：

- 推理等级候选值由当前安装的 Codex 模型目录决定。
- 列出 GPT-5.6 Sol、Terra、Luna 的当前示例。
- 说明未知模型回退到 `low`、`medium`、`high`、`xhigh`。
- 说明目录加载失败时应用仍可用。

## 验收标准

- 选择 GPT-5.6 Sol/Terra 时可选 `max` 和 `ultra`。
- 选择 GPT-5.6 Luna 时可选 `max`，不可选 `ultra`。
- 新建或缺失等级使用模型目录默认值。
- 已有合法显式值在切换模型后保留，不合法值回退为新模型默认。
- 未知模型和模型目录加载失败时仍可编辑、保存和应用 provider。
- UI、详情页、导入逻辑和最终 Codex TOML 对同一组合给出一致结果。
- 所有测试、格式检查和 CI Clippy 配置通过。

