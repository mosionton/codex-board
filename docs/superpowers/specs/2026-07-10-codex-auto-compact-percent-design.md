# Codex 自动压缩百分比设计

## 背景

codex-board 当前可以为每个 provider 保存模型和推理等级，并在应用 provider 时把这些
字段写入 `$CODEX_HOME/config.toml`。它尚未管理 Codex 的自动历史压缩阈值，因此
provider 切换后仍会沿用原配置中的全局阈值或 Codex 模型默认值。

Codex 配置只接受绝对 token 阈值：

```toml
model_auto_compact_token_limit = 260400
model_auto_compact_token_limit_scope = "total"
```

它没有可直接写入的百分比字段。当前 Codex bundled model catalog 提供每个模型的
`context_window`，所以 codex-board 可以让用户按百分比配置，并在应用 provider 时
换算成 Codex 接受的绝对 token 数。

本功能默认使用 `70%`。以当前 GPT-5.6 的 `372000` token 上下文窗口为例：

```text
372000 * 70 / 100 = 260400
```

这会在 272000 token 之前触发压缩，并预留约 11600 token 的缓冲。未知模型统一按
`272000` token 的兼容窗口计算，默认阈值为 `190400`。

官方参考：

- <https://learn.chatgpt.com/docs/config-file/config-reference#configtoml>
- `model_auto_compact_token_limit` 是触发自动历史压缩的 token 阈值。
- `model_auto_compact_token_limit_scope = "total"` 按完整活动上下文计数。

外部实现参考：

- <https://github.com/bingfengfeifei/switcher/commit/aebf9b5f>

该项目只为 Claude Code 实现了 `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`，没有实现 Codex
自动压缩。codex-board 借鉴其默认 `70%`、数字输入和旧配置补默认值的交互，但不会
复制其环境变量写法、字符串弱校验或允许 `100%` 的范围。

## 目标

- 每个 Codex provider 独立保存自动压缩百分比。
- 新建和旧版 provider 默认使用 `70%`。
- 使用当前安装的 Codex bundled model catalog 获取模型上下文窗口。
- 未知模型或缺失窗口时按 `272000` token 计算。
- 应用 provider 时写入绝对 token 阈值和 `total` 作用域。
- 从现有 Codex 配置导入 provider 时尽可能反算百分比。
- 在 Provider 编辑器、列表和详情中展示该配置。
- 保留 Codex 配置中的所有无关顶层字段和表。

## 非目标

- 不修改 Codex 的压缩提示词。
- 不管理 `compact_prompt` 或 `experimental_compact_prompt_file`。
- 不支持 `body_after_prefix` 作为 provider 选项。
- 不增加 provider 级 `model_context_window` 字段。
- 不保证单个超大用户输入或工具输出永远不会一次跨过压缩阈值。
- 不改变 Claude Code 配置；本功能只管理 Codex provider。

## 方案选择

### 采用：保存百分比，应用时换算

provider 配置保存用户意图：

```toml
[providers.switcher]
auto_compact_percent = 70
```

应用 provider 时根据有效模型的上下文窗口换算绝对阈值。这让同一个百分比可以随
模型窗口变化，并保持 provider 文件可读。

### 未采用：直接保存绝对 token 阈值

该方案不依赖模型目录，但用户切换不同上下文窗口的模型时需要手动重算，也不符合
按百分比配置的目标。

### 未采用：同时保存百分比和上下文窗口

显式窗口可以覆盖自定义模型，但会引入两个需要保持一致的输入。已确认未知模型使用
`272000` 兼容窗口，因此当前范围不增加第二个配置字段。

## Provider 配置模型

`ProviderConfig` 新增：

```text
auto_compact_percent: u8
```

序列化名称为 `auto_compact_percent`。使用自定义 Serde 默认函数让缺失字段得到 `70`，
从而兼容已有 `switcher-providers.toml`。保存 registry 时显式写出该字段。

合法范围固定为：

```text
1..=99
```

不允许 `0`，因为它不能表达有效压缩策略；不允许 `100`，因为它不会为压缩操作和
下一次输入预留空间。`ProviderConfig::validate` 负责最终校验，所有加载、保存、导入和
应用路径都经过该边界。

`ProviderConfig::new` 和所有显式结构体构造都使用默认 `70`。`merge_defaults` 继续以
本地 provider 为准；只有不存在本地 provider 时才采用从 Codex 导入的百分比。这与
模型、推理等级和端点字段的现有合并语义一致。

## 模型目录

`CatalogModel` 额外解析 `context_window`。该字段先保留为宽松的 JSON 值，再单独提取
正整数；缺失或类型错误不应让整个模型条目失效，也不应影响推理等级解析。

`ReasoningProfile` 增加有效上下文窗口：

```text
context_window
```

规则如下：

- bundled catalog 提供正整数时使用该值。
- 字段缺失、类型不匹配或值为 `0` 时使用 `272000`。
- 未知模型使用 fallback profile 的 `272000`。
- `gpt-5.6` 继续映射到 `gpt-5.6-sol`，推理等级和窗口使用同一个 profile。

`ModelCatalog` 提供统一换算方法，避免导入、显示和应用各自复制公式：

```text
token_limit = context_window * percent / 100
```

计算使用足够宽的无符号整数并采用整数除法向下取整。向下取整保证结果不会超过用户
选择的百分比。

## Codex 配置导入

`CodexConfig` 增加可选字段：

```text
model_auto_compact_token_limit
model_auto_compact_token_limit_scope
```

这两个字段使用宽松的 TOML 值承载，再分别提取整数和字符串。这样错误类型只会让
百分比回退到 `70`，不会导致整个 Codex 配置和其中的 provider 无法导入。

导入百分比时先按顶层 `model` 查找有效上下文窗口，然后应用以下规则：

1. 作用域缺失或等于 `total` 时允许反算。
2. 阈值必须为正数并且小于上下文窗口。
3. 使用 `threshold * 100 / context_window` 向下取整。
4. 结果落在 `1..=99` 时使用反算值。
5. 作用域为 `body_after_prefix`、字段类型错误、阈值越界或结果无效时使用 `70`。

顶层压缩配置与顶层模型、推理等级一样应用到本次导入产生的所有 provider，包括通过
OpenAI 登录状态补出的内置 `openai` provider。

示例：

```text
context_window = 372000
token_limit = 260400
percent = 260400 * 100 / 372000 = 70
```

成功反算出 `1..=99` 合法百分比时也向下取整，因此重新应用 provider 后生成的阈值
不会高于导入时的绝对阈值。无法安全反算的阈值仍按上述规则回退 `70`。

## 应用到 Codex

`write_codex_config` 继续使用现有有效模型规则：

- provider 的 `model` 非空时使用该模型。
- provider 模型为空时使用现有 Codex 顶层 `model`。
- 两者都不可用时使用 fallback profile。

在推理等级归一化后，通过 `ModelCatalog` 把 provider 百分比换算成绝对阈值，并写入：

```toml
model_auto_compact_token_limit = <calculated integer>
model_auto_compact_token_limit_scope = "total"
```

这两个键是 Codex 顶层模型设置，不写入 `[model_providers.<id>]`。内置 `openai` 和
自定义 provider 使用完全相同的换算逻辑。

现有 `toml_edit::DocumentMut` 和原子写入流程继续负责保留其他顶层键、MCP 配置、
features、hooks 和未知未来字段。应用 provider 会覆盖旧的压缩阈值和作用域，因为这
两个值现在由所选 provider 管理。

## Provider 编辑器

`ProviderField` 增加 `AutoCompactPercent`，位于 Plan Reason 之后。`ProviderEditor`
增加一个 `TextField` 保存百分比文本，并按以下规则工作：

- 新建 provider 初始显示 `70`。
- 编辑 provider 显示已保存值。
- 键盘输入只接受 ASCII 数字。
- Backspace 和光标移动沿用通用文本输入行为。
- `Ctrl+U` 重置为 `70`。
- Tab 和 Shift+Tab 把该字段纳入循环。
- 保存前解析为整数并校验 `1..=99`。
- 空值、非数字和越界值在状态栏显示明确错误，不保存 registry。

编辑器标签使用 `auto_compact`，值显示为纯数字，避免把 `%` 符号写入输入缓冲区。
帮助文案说明该字段是百分比。

## 列表与详情

Provider 列表和详情的共享显示项增加 `compact`：

```text
70%
```

列表增加固定宽度列，Claude 只读行在该列显示 `-`。详情页通过现有统一显示项自动
展示同一值，避免列表和详情产生不同格式。

## 错误处理

- 手写 provider TOML 中的非整数值会产生配置解析错误；整数越界会产生明确的
  `1..=99` 范围错误。
- 编辑器无效输入只阻止当前保存，不关闭编辑器或修改磁盘。
- bundled model catalog 缺少窗口时只回退窗口，不产生新的启动警告。
- 整个模型目录加载失败时继续使用现有 fallback profile，其窗口为 `272000`。
- Codex 配置中的压缩字段无法安全反算时使用 `70`，不阻止其他 provider 导入。
- 应用前仍执行 provider 最终校验，防止非 UI 调用绕过范围检查。

## 测试策略

### Provider registry

- 缺失 `auto_compact_percent` 时加载为 `70`。
- 保存后显式包含 `auto_compact_percent = 70`。
- 接受 `1` 和 `99`，拒绝 `0` 和 `100`。
- `ProviderConfig::new` 使用 `70`。

### Model catalog

- 解析 GPT-5.6 的 `372000` 上下文窗口。
- `gpt-5.6` 别名使用 Sol 的窗口。
- 缺失或为 `0` 的窗口回退到 `272000`，但保留有效推理等级。
- 未知模型使用 `272000`。
- `372000 * 70%` 得到 `260400`。
- `272000 * 70%` 得到 `190400`。

### Codex 导入

- `260400` 与 `372000` 反算为 `70`。
- 不能整除时向下取整。
- 缺失阈值使用 `70`。
- `body_after_prefix` 使用 `70`。
- 负数、零、等于或超过窗口的阈值使用 `70`。
- 自定义 provider 和内置 OpenAI provider 获得相同百分比。

### Codex 应用

- 已知模型写入正确绝对阈值和 `total` 作用域。
- 未知模型使用 `190400` 默认阈值。
- 非默认百分比按同一公式换算。
- provider 模型为空时使用现有 Codex 模型窗口。
- 内置 OpenAI provider 同样写入压缩配置。
- 无关顶层键和表保持不变。

### Provider 编辑器和 UI

- 新建、编辑和保存正确传递百分比。
- 字段顺序、文本光标、数字过滤和 `Ctrl+U` 正确。
- 空值、非数字和越界值显示保存错误。
- 列表和详情显示 `70%`。
- Claude 行在新列显示 `-`。

## 文档

README 的 provider 示例增加：

```toml
auto_compact_percent = 70
```

字段表说明：

- 默认值为 `70`。
- 合法范围为 `1..=99`。
- 已知模型按 bundled catalog 的 `context_window` 换算。
- 未知模型按 `272000` 换算。
- GPT-5.6 当前 `70%` 会写入 `260400`。
- 自动压缩不能对单个超大输入提供绝对保证。

## 验收标准

- 旧 provider 文件无需手工迁移并默认得到 `70%`。
- 用户可以在 TUI 中查看、编辑和保存自动压缩百分比。
- 应用 GPT-5.6 provider 后 Codex 配置包含 `260400` 和 `total`。
- 应用未知模型 provider 后默认阈值为 `190400`。
- 导入现有合法 Codex 阈值时能恢复安全的不增大百分比。
- 无效百分比无法进入持久化配置或最终 Codex 写回。
- 原有模型、推理等级、认证、provider 切换和配置保留行为不回归。
- Rust 测试、格式检查和 CI 同款 Clippy 检查全部通过。
