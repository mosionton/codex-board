# Final Review Fixes Report

## Status

三个 Important 集成缺陷均已完成系统化定位、RED→GREEN 回归、跨层一致性修复和全量验证。

## Important 1：导入缺失/非法 effort 被误判为显式

### 根因证据

- `load_codex_config_providers` 先调用 `ModelCatalog::normalize_effort`，把缺失、空白和不支持值全部变为具体默认字符串。
- `imported_provider_config` 随后无条件保存 `Some(default)`。
- `ProviderEditor::from_provider_with_catalog` 将所有受支持的 `Some` 视为显式值，因此 Sol 缺失字段被伪装成显式 `low`，切到 Terra 后不会采用 `medium` 默认值。

### RED

命令：

```sh
cargo test imported_ -- --nocapture
```

失败证据：2 个测试失败；两处均为期望 `None`、实际 `Some("low")`。

- `imported_missing_efforts_follow_new_model_defaults_in_editor`
- `imported_explicit_and_invalid_efforts_remain_independent`

### GREEN

命令：

```sh
cargo test imported_ -- --nocapture
cargo test synthesized_openai_inherits_top_level_model_and_efforts -- --nocapture
```

结果：前者 2 passed，后者 1 passed。

### 修复

- 导入层仅把源中明确存在、去空白后受当前 profile 支持的值保存为 `Some`。
- 缺失、空白或不支持值保存为 `None`，显示/编辑时仍由目录计算当前模型默认值。
- 普通与 Plan 字段分别判断，互不影响。
- 合成 OpenAI provider 继续继承明确存在的 `ultra`/`max`；缺失/非法值保持非显式。
- 启动 `merge_defaults` 不再把本地 provider 的空 model/effort 用 Codex 顶层值回填，只补认证信息，避免保存时静默持久化 fallback。

## Important 2：空 provider model 的 UI/editor 与 apply 使用不同有效模型

### 根因证据

- 编辑器和详情/表格只把 `provider.model` 传给目录；空值直接落到兼容 profile。
- apply 从 Codex TOML 读取现有顶层 `model`，所以同一 provider 可能在 UI 显示 `medium`，最终却保留 `ultra` 或 `max`。
- `ProvidersState` 原先没有当前 Codex 顶层 model，渲染路径无法共享 apply 使用的模型上下文。
- 启动合并还会把空 model 写入运行时 provider，破坏“保存仍为空”的要求。

### RED

命令：

```sh
cargo test empty_provider_model_uses_current_ -- --nocapture
cargo test current_codex_model -- --nocapture
cargo test merge_defaults_preserves_empty_model_context_and_fills_credentials -- --nocapture
```

失败证据：

- 第一条因不存在 current-model-aware editor 构造入口而编译失败。
- 第二条因 `ProvidersState` 没有 current Codex model getter/setter、导入层没有 current model loader 而编译失败。
- 第三条行为失败：期望 model 保持 `None`，实际被合并为 `Some("gpt-5.5")`。

### GREEN

命令：

```sh
cargo test current_codex_model -- --nocapture
cargo test empty_provider_model -- --nocapture
cargo test saving_empty_provider_model_keeps_it_empty -- --nocapture
cargo test merge_defaults_preserves_empty_model_context_and_fills_credentials -- --nocapture
```

结果：分别 4 passed、5 passed、1 passed、1 passed。

### 修复

- 新增统一 `effective_model` 优先级：非空 provider model → 当前 Codex model → 兼容 profile。
- 启动时读取一次 Codex 顶层 model，规范化后缓存到 `ProvidersState`；渲染期间不读配置文件。
- 新建/编辑 provider、表格、详情均使用同一当前模型上下文。
- apply 使用同一 `effective_model` helper 解析 provider model 与现有 Codex model。
- 成功应用非空 provider model 后同步更新应用状态中的当前模型。
- editor 仅用 fallback model 选择 profile，保存时原空 model 仍写为 `None`。
- Sol 空 model 的 `ultra`/`max` 在 editor、UI 和 apply 中保留；Luna 将 `ultra` 归一为 `medium`、保留 `max`。

## Important 3：一个结构损坏目录项拒绝有效 sibling

### 根因证据

- `CatalogResponse` 使用 `Vec<CatalogModel>`，Serde 在进入逐项语义过滤前就对整个数组做强类型反序列化。
- 任一条目缺少 `default_reasoning_level` 或字段类型错误会令整个 `from_json` 返回 parse error。

### RED

命令：

```sh
cargo test keeps_valid_siblings_when_entries_are_structurally_malformed -- --nocapture
```

失败证据：测试在 `unwrap()` 处失败，错误链为 `failed to parse Codex model catalog`，原因是 malformed sibling 缺少 `default_reasoning_level`。

### GREEN

命令：

```sh
cargo test keeps_valid_siblings_when_entries_are_structurally_malformed -- --nocapture
cargo test provider_config::model_catalog::tests -- --nocapture
```

结果：分别 1 passed、5 passed。

### 修复

- 外层 `models` 先解析为 JSON value 列表。
- 每个条目独立反序列化为 `CatalogModel`；缺字段、错误类型或非对象条目只丢弃自身。
- 原有空 slug、空默认值、空支持列表、默认值不在支持列表中的语义过滤保持不变。
- 没有任何有效条目时仍返回原有 whole-catalog error，并由加载层启用兼容 fallback。

## 修改文件

- `src/provider_config/codex_import.rs`：导入显式性与当前 model 加载。
- `src/provider_config/model_catalog.rs`：统一有效模型 helper、逐项目录解析。
- `src/provider_config/codex_apply.rs`：apply 使用统一有效模型优先级及 Sol 回归测试。
- `src/provider_config/registry.rs`：默认合并不再覆盖本地 model/effort。
- `src/provider_config/codex.rs`：导入、OpenAI 继承和 current model 回归测试。
- `src/provider_config/mod.rs`：导出新 helper/loader。
- `src/app/providers_state.rs`：缓存当前 Codex model。
- `src/app/runtime.rs`：启动时加载当前 model。
- `src/app/provider_editor.rs`：current-model-aware profile 与 Sol/Luna 测试。
- `src/app/providers.rs`：editor/apply 状态接线与空 model 保存测试。
- `src/ui/details.rs`、`src/ui/tables.rs`、`src/ui/mod.rs`：表格/详情统一 profile 和 UI 回归测试。
- `docs/superpowers/plans/2026-07-10-final-review-fixes.md`：执行计划。

## 完整验证

```text
cargo test
test result: ok. 218 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo fmt --all -- --check
exit 0

cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
Finished `dev` profile; exit 0

git diff --check
exit 0
```

## 自审

- Provider TOML schema、字段名和旧 alias 未修改。
- 未新增模型能力表，也未在生产代码硬编码 GPT-5.6 能力。
- `gpt-5.6 -> gpt-5.6-sol`、bundled catalog、兼容 profile、认证和既有测试保持通过。
- 空 provider model 只参与有效 profile 解析，不会被 editor/save 或启动合并写回 provider。
- 普通和 Plan effort 的值及显式状态始终独立。
- 目录结构错误和语义错误均按单项隔离；全无有效项仍保留原错误语义。

## Concerns

无阻断 concern。当前启动阶段为 provider 导入、applied provider 和 current model 分别读取 Codex 配置；均只发生一次且不在渲染路径，后续如需可合并为单次解析，但不影响本次正确性。
