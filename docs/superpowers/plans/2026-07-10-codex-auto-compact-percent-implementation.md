# Codex 自动压缩百分比 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让每个 Codex provider 保存自动压缩百分比，并在应用时按模型上下文窗口换算成 Codex 接受的绝对 token 阈值。

**Architecture:** 在 `ProviderConfig` 中持久化严格校验的整数百分比，由 `ModelCatalog` 统一提供上下文窗口、百分比到 token 阈值的换算以及安全反算。Codex 导入和写回、Provider 编辑器、列表及详情共享这些接口；未知模型使用 `272000` token 兼容窗口。

**Tech Stack:** Rust 2024、Serde/serde_json、TOML/toml_edit、Anyhow、Ratatui、Crossterm、Cargo 内联单元测试。

## Global Constraints

- provider 字段名必须是 `auto_compact_percent`，类型为 `u8`。
- 默认值必须是 `70`，合法范围必须是 `1..=99`。
- 已有 provider 文件缺少字段时必须无迁移加载为 `70`。
- 上下文窗口来源必须是 `codex debug models --bundled` 的 `context_window`。
- 未知模型、缺失窗口、错误类型或窗口为 `0` 时必须使用 `272000`。
- GPT-5.6 当前 `372000 * 70 / 100` 必须写为 `260400`。
- 未知模型当前 `272000 * 70 / 100` 必须写为 `190400`。
- 最终必须写入 `model_auto_compact_token_limit_scope = "total"`。
- 导入反算必须向下取整，重新应用后的绝对阈值不得高于导入值。
- `body_after_prefix`、错误类型和越界阈值导入时必须回退 `70`，不得阻断其他 provider。
- 不增加 provider 级 `model_context_window`，不修改压缩提示词，不改变 Claude Code 配置。
- 不增加新的 Cargo 依赖。
- 所有手工编辑必须使用 `apply_patch`；每个 Rust 行为任务遵循 red-green TDD。
- 修改 Rust 后必须运行 `cargo fmt --all -- --check` 和仓库规定的完整 Clippy 命令。
- 提交使用 Conventional Commits；类型英文，摘要和非平凡正文使用中文。

---

## File Structure

- Modify: `src/provider_config/registry.rs` — provider 百分比字段、默认值和最终范围校验。
- Modify: `src/provider_config/mod.rs` — 导出百分比常量。
- Modify: `src/provider_config/model_catalog.rs` — 上下文窗口解析、兼容窗口及双向换算。
- Modify: `src/provider_config/codex_import.rs` — 从 Codex 顶层绝对阈值安全反算 provider 百分比。
- Modify: `src/provider_config/codex.rs` — Codex 导入回归测试。
- Modify: `src/provider_config/codex_apply.rs` — 写入绝对阈值和 `total` 作用域。
- Modify: `src/app/provider_editor.rs` — 编辑器字段、默认值、范围解析和字段导航。
- Modify: `src/app/providers.rs` — 保存编辑器时持久化百分比并恢复错误状态。
- Modify: `src/ui/provider_editor_input.rs`、`src/ui/input.rs` — 数字输入过滤及交互测试。
- Modify: `src/ui/provider_editor_view.rs` — 编辑器字段渲染和光标位置。
- Modify: `src/ui/details.rs`、`src/ui/tables.rs`、`src/ui/mod.rs` — 列表和详情的 `compact` 显示。
- Modify: `src/app/mod.rs`、`src/app/provider_display.rs` — 更新现有 provider 测试夹具。
- Modify: `README.md` — 用户配置字段、默认值、换算与限制说明。

---

### Task 1: 持久化并校验 provider 自动压缩百分比

**Files:**
- Modify: `src/provider_config/registry.rs:14-215`
- Modify: `src/provider_config/mod.rs:15-20`
- Modify: `src/provider_config/codex_import.rs:140-210`
- Modify: `src/provider_config/codex_apply.rs:175-690`
- Modify: `src/app/provider_editor.rs:423-710`
- Modify: `src/app/providers.rs:354-695`
- Modify: `src/app/mod.rs:497-710`
- Modify: `src/app/provider_display.rs:57-145`
- Modify: `src/ui/input.rs:469-490`
- Modify: `src/ui/mod.rs:247-375`

**Interfaces:**
- Produces: `DEFAULT_AUTO_COMPACT_PERCENT: u8 = 70`
- Produces: `MIN_AUTO_COMPACT_PERCENT: u8 = 1`
- Produces: `MAX_AUTO_COMPACT_PERCENT: u8 = 99`
- Produces: `ProviderConfig::auto_compact_percent: u8`
- Preserves: all existing provider constructors and fixtures compile with an explicit default value.

- [ ] **Step 1: 写失败测试覆盖默认加载、序列化和边界校验**

Add these tests to `src/provider_config/registry.rs`:

```rust
#[test]
fn missing_auto_compact_percent_uses_default() {
    let dir = tempdir().unwrap();
    let path = config_path(dir.path());
    fs::write(
        &path,
        r#"
[providers.switcher]
base_url = "https://example.test/v1"
wire_api = "responses"
"#,
    )
    .unwrap();

    let registry = ProviderRegistry::load(&path).unwrap();

    assert_eq!(
        registry.providers["switcher"].auto_compact_percent,
        DEFAULT_AUTO_COMPACT_PERCENT
    );
}

#[test]
fn saves_auto_compact_percent_explicitly() {
    let dir = tempdir().unwrap();
    let path = config_path(dir.path());
    let mut registry = ProviderRegistry::default();
    registry
        .upsert(
            "switcher",
            ProviderConfig::new("https://example.test/v1", "responses"),
        )
        .unwrap();

    registry.save(&path).unwrap();

    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains("auto_compact_percent = 70"));
}

#[test]
fn rejects_auto_compact_percent_outside_supported_range() {
    for percent in [0, 100] {
        let mut registry = ProviderRegistry::default();
        let mut provider = ProviderConfig::new("https://example.test/v1", "responses");
        provider.auto_compact_percent = percent;

        let error = registry.upsert("switcher", provider).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("auto_compact_percent must be between 1 and 99")
        );
    }
}

#[test]
fn accepts_auto_compact_percent_range_boundaries() {
    for percent in [1, 99] {
        let mut registry = ProviderRegistry::default();
        let mut provider = ProviderConfig::new("https://example.test/v1", "responses");
        provider.auto_compact_percent = percent;

        registry.upsert(format!("switcher-{percent}"), provider).unwrap();
    }
}
```

- [ ] **Step 2: 运行测试并确认字段尚不存在**

Run: `cargo test provider_config::registry::tests::missing_auto_compact_percent_uses_default --all-features --locked`

Expected: FAIL to compile because `ProviderConfig` and `DEFAULT_AUTO_COMPACT_PERCENT` do not yet exist.

- [ ] **Step 3: 增加常量、Serde 默认和最终校验**

Add to `src/provider_config/registry.rs` near the existing config constants:

```rust
pub const DEFAULT_AUTO_COMPACT_PERCENT: u8 = 70;
pub const MIN_AUTO_COMPACT_PERCENT: u8 = 1;
pub const MAX_AUTO_COMPACT_PERCENT: u8 = 99;

const fn default_auto_compact_percent() -> u8 {
    DEFAULT_AUTO_COMPACT_PERCENT
}
```

Add this field to `ProviderConfig` after the two reasoning fields:

```rust
#[serde(default = "default_auto_compact_percent")]
pub auto_compact_percent: u8,
```

Set it in `ProviderConfig::new`:

```rust
auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
```

Add this validation before the existing `base_url` validation:

```rust
if !(MIN_AUTO_COMPACT_PERCENT..=MAX_AUTO_COMPACT_PERCENT)
    .contains(&self.auto_compact_percent)
{
    bail!(
        "auto_compact_percent must be between {MIN_AUTO_COMPACT_PERCENT} and {MAX_AUTO_COMPACT_PERCENT}"
    );
}
```

Export the constants from `src/provider_config/mod.rs`:

```rust
pub use registry::{
    CONFIG_FILE_NAME, DEFAULT_AUTO_COMPACT_PERCENT, MAX_AUTO_COMPACT_PERCENT,
    MIN_AUTO_COMPACT_PERCENT, ProviderAuthMode, ProviderConfig, ProviderRegistry, config_path,
};
```

- [ ] **Step 4: 更新所有现有 `ProviderConfig` 结构体构造**

Run: `rg -l 'ProviderConfig \{' src`

Expected files include `src/provider_config/registry.rs`, `src/provider_config/codex_import.rs`, `src/provider_config/codex_apply.rs`, `src/app/provider_editor.rs`, `src/app/providers.rs`, `src/app/mod.rs`, `src/app/provider_display.rs`, `src/ui/input.rs`, and `src/ui/mod.rs`.

In every existing literal, add this exact field beside the reasoning fields and import the constant from `crate::provider_config` where needed:

```rust
auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
```

For the two production constructors in `src/provider_config/codex_import.rs`, use the same default temporarily; Task 3 will replace it with the imported value.

- [ ] **Step 5: 运行定向测试和完整编译测试**

Run: `cargo test provider_config::registry::tests --all-features --locked`

Expected: PASS, including default, serialization, `1`/`99` acceptance through existing validation paths, and `0`/`100` rejection.

Run: `cargo test --all-features --locked`

Expected: PASS with all existing provider fixtures updated.

- [ ] **Step 6: 提交 provider schema**

```bash
git add src
git commit -m 'feat: 增加供应商自动压缩百分比' -m $'为每个 Codex provider 持久化严格校验的压缩百分比。\n\n- 默认旧配置和新配置为 70%\n- 限制合法范围为 1 到 99'
```

---

### Task 2: 从模型目录解析上下文窗口并统一换算

**Files:**
- Modify: `src/provider_config/model_catalog.rs:1-310`

**Interfaces:**
- Produces: `ReasoningProfile::context_window() -> u64`
- Produces: `ModelCatalog::auto_compact_token_limit(Option<&str>, u8) -> u64`
- Produces: `ModelCatalog::auto_compact_percent(Option<&str>, u64) -> Option<u8>`
- Consumes: `MIN_AUTO_COMPACT_PERCENT` and `MAX_AUTO_COMPACT_PERCENT` from Task 1.

- [ ] **Step 1: 扩展模型目录测试夹具并写失败测试**

Add `"context_window":372000` to the GPT-5.6 Sol, Terra, and Luna entries in `GPT_5_6_CATALOG`, then add:

```rust
#[test]
fn parses_context_window_and_calculates_compaction_values() {
    let catalog = ModelCatalog::from_json(GPT_5_6_CATALOG).unwrap();
    let sol = catalog.profile_for(Some("gpt-5.6-sol"));

    assert_eq!(sol.context_window(), 372_000);
    assert_eq!(catalog.profile_for(Some("gpt-5.6")).context_window(), 372_000);
    assert_eq!(
        catalog.auto_compact_token_limit(Some("gpt-5.6-sol"), 70),
        260_400
    );
    assert_eq!(
        catalog.auto_compact_percent(Some("gpt-5.6-sol"), 260_400),
        Some(70)
    );
    assert_eq!(
        catalog.auto_compact_percent(Some("gpt-5.6-sol"), 260_399),
        Some(69)
    );
}

#[test]
fn invalid_or_missing_context_windows_use_compatibility_value() {
    let catalog = ModelCatalog::from_json(
        r#"{"models":[
          {"slug":"missing","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"medium"}]},
          {"slug":"zero","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"medium"}],"context_window":0},
          {"slug":"wrong-type","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"medium"}],"context_window":"large"}
        ]}"#,
    )
    .unwrap();

    for model in ["missing", "zero", "wrong-type", "unknown"] {
        assert_eq!(catalog.profile_for(Some(model)).context_window(), 272_000);
        assert_eq!(
            catalog.auto_compact_token_limit(Some(model), 70),
            190_400
        );
    }
}

#[test]
fn inverse_compaction_rejects_invalid_thresholds() {
    let catalog = ModelCatalog::from_json(GPT_5_6_CATALOG).unwrap();

    assert_eq!(catalog.auto_compact_percent(Some("gpt-5.6-sol"), 0), None);
    assert_eq!(
        catalog.auto_compact_percent(Some("gpt-5.6-sol"), 372_000),
        None
    );
    assert_eq!(
        catalog.auto_compact_percent(Some("gpt-5.6-sol"), 400_000),
        None
    );
}
```

- [ ] **Step 2: 运行模型目录测试并确认窗口 API 缺失**

Run: `cargo test provider_config::model_catalog::tests --all-features --locked`

Expected: FAIL to compile because `context_window`, `auto_compact_token_limit`, and `auto_compact_percent` are missing.

- [ ] **Step 3: 实现宽松窗口解析和兼容窗口**

Import the range constants:

```rust
use super::registry::{MAX_AUTO_COMPACT_PERCENT, MIN_AUTO_COMPACT_PERCENT};
```

Add the fallback constant and profile field:

```rust
const FALLBACK_CONTEXT_WINDOW: u64 = 272_000;

pub struct ReasoningProfile {
    default_effort: String,
    supported_efforts: Vec<String>,
    context_window: u64,
}
```

Add the getter:

```rust
#[must_use]
pub const fn context_window(&self) -> u64 {
    self.context_window
}
```

Set `context_window: FALLBACK_CONTEXT_WINDOW` in the fallback profile. Extend `CatalogModel` with a wide value so a wrong type does not invalidate the model entry:

```rust
#[serde(default)]
context_window: Option<serde_json::Value>,
```

When constructing each valid profile, derive the window independently of reasoning validation:

```rust
let context_window = model
    .context_window
    .as_ref()
    .and_then(serde_json::Value::as_u64)
    .filter(|window| *window > 0)
    .unwrap_or(FALLBACK_CONTEXT_WINDOW);
```

Add `context_window` to the `ReasoningProfile` inserted into `profiles`.

- [ ] **Step 4: 实现正向和反向换算**

Add these methods to `impl ModelCatalog`:

```rust
#[must_use]
pub fn auto_compact_token_limit(&self, model: Option<&str>, percent: u8) -> u64 {
    let limit = u128::from(self.profile_for(model).context_window())
        * u128::from(percent)
        / 100;
    u64::try_from(limit).unwrap_or(u64::MAX)
}

#[must_use]
pub fn auto_compact_percent(&self, model: Option<&str>, token_limit: u64) -> Option<u8> {
    let context_window = self.profile_for(model).context_window();
    if token_limit == 0 || token_limit >= context_window {
        return None;
    }

    let percent = u128::from(token_limit) * 100 / u128::from(context_window);
    let percent = u8::try_from(percent).ok()?;
    (MIN_AUTO_COMPACT_PERCENT..=MAX_AUTO_COMPACT_PERCENT)
        .contains(&percent)
        .then_some(percent)
}
```

- [ ] **Step 5: 运行模型目录测试**

Run: `cargo test provider_config::model_catalog::tests --all-features --locked`

Expected: PASS; malformed window values retain their valid reasoning profiles and use `272000`.

- [ ] **Step 6: 提交模型窗口换算**

```bash
git add src/provider_config/model_catalog.rs
git commit -m 'feat: 解析模型上下文窗口' -m $'统一计算 Codex 自动压缩的绝对阈值和导入百分比。\n\n- 从 bundled model catalog 读取 context_window\n- 为未知或无效窗口使用 272000 token 回退'
```

---

### Task 3: 从 Codex 配置安全导入自动压缩百分比

**Files:**
- Modify: `src/provider_config/codex_import.rs:10-215`
- Modify: `src/provider_config/codex.rs:20-455`

**Interfaces:**
- Consumes: `ModelCatalog::auto_compact_percent(Option<&str>, u64) -> Option<u8>`
- Produces: every imported `ProviderConfig` receives an explicit safe `auto_compact_percent`.

- [ ] **Step 1: 为已知阈值、向下取整和回退写失败测试**

Update the local `gpt_5_6_catalog()` fixture in `src/provider_config/codex.rs` so every GPT-5.6 entry contains `"context_window":372000`. Add:

```rust
#[test]
fn imports_total_auto_compact_limit_as_percent() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let auth_path = dir.path().join("auth.json");
    fs::write(
        &config_path,
        r#"
model = "gpt-5.6-sol"
model_auto_compact_token_limit = 260400
model_auto_compact_token_limit_scope = "total"

[model_providers.switcher]
base_url = "https://example.test/v1"
wire_api = "responses"
"#,
    )
    .unwrap();

    let registry =
        load_codex_config_providers(&config_path, &auth_path, &gpt_5_6_catalog()).unwrap();

    assert_eq!(registry.providers["switcher"].auto_compact_percent, 70);
}

#[test]
fn imported_auto_compact_percent_rounds_down() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let auth_path = dir.path().join("auth.json");
    fs::write(
        &config_path,
        r#"
model = "gpt-5.6-sol"
model_auto_compact_token_limit = 260399

[model_providers.switcher]
base_url = "https://example.test/v1"
wire_api = "responses"
"#,
    )
    .unwrap();

    let registry =
        load_codex_config_providers(&config_path, &auth_path, &gpt_5_6_catalog()).unwrap();

    assert_eq!(registry.providers["switcher"].auto_compact_percent, 69);
}

#[test]
fn unsafe_auto_compact_imports_use_default_percent() {
    let cases = [
        "model_auto_compact_token_limit = 0",
        "model_auto_compact_token_limit = 372000",
        "model_auto_compact_token_limit = -1",
        "model_auto_compact_token_limit = \"bad\"",
        "model_auto_compact_token_limit = 260400\nmodel_auto_compact_token_limit_scope = \"body_after_prefix\"",
        "model_auto_compact_token_limit = 260400\nmodel_auto_compact_token_limit_scope = 7",
    ];

    for (index, compact_config) in cases.into_iter().enumerate() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(format!("config-{index}.toml"));
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            format!(
                "model = \"gpt-5.6-sol\"\n{compact_config}\n\n[model_providers.switcher]\nbase_url = \"https://example.test/v1\"\nwire_api = \"responses\"\n"
            ),
        )
        .unwrap();

        let registry =
            load_codex_config_providers(&config_path, &auth_path, &gpt_5_6_catalog()).unwrap();

        assert_eq!(
            registry.providers["switcher"].auto_compact_percent,
            DEFAULT_AUTO_COMPACT_PERCENT
        );
    }
}
```

Extend `synthesized_openai_inherits_top_level_model_and_efforts` with a `260400` total threshold and:

```rust
assert_eq!(provider.auto_compact_percent, 70);
```

Also extend `loads_providers_from_codex_config`, whose fixture has no compact threshold, with:

```rust
assert_eq!(
    provider.auto_compact_percent,
    DEFAULT_AUTO_COMPACT_PERCENT
);
```

- [ ] **Step 2: 运行导入测试并确认仍固定为默认值**

Run: `cargo test imports_total_auto_compact_limit_as_percent --all-features --locked`

Expected: FAIL because imported providers still receive `DEFAULT_AUTO_COMPACT_PERCENT` without reading Codex fields.

- [ ] **Step 3: 宽松读取 Codex 顶层压缩字段**

Add to `CodexConfig` in `src/provider_config/codex_import.rs`:

```rust
#[serde(default)]
model_auto_compact_token_limit: Option<toml::Value>,
#[serde(default)]
model_auto_compact_token_limit_scope: Option<toml::Value>,
```

Import `DEFAULT_AUTO_COMPACT_PERCENT`, then add:

```rust
fn imported_auto_compact_percent(
    config: &CodexConfig,
    model_catalog: &ModelCatalog,
) -> u8 {
    if config
        .model_auto_compact_token_limit_scope
        .as_ref()
        .is_some_and(|scope| scope.as_str().map(str::trim) != Some("total"))
    {
        return DEFAULT_AUTO_COMPACT_PERCENT;
    }

    let Some(token_limit) = config
        .model_auto_compact_token_limit
        .as_ref()
        .and_then(toml::Value::as_integer)
        .and_then(|value| u64::try_from(value).ok())
    else {
        return DEFAULT_AUTO_COMPACT_PERCENT;
    };

    model_catalog
        .auto_compact_percent(config.model.as_deref(), token_limit)
        .unwrap_or(DEFAULT_AUTO_COMPACT_PERCENT)
}
```

- [ ] **Step 4: 把导入值传给自定义和内置 OpenAI provider**

Immediately after entering `if let Some(codex_config)`, compute:

```rust
let auto_compact_percent = imported_auto_compact_percent(&codex_config, model_catalog);
```

Add an `auto_compact_percent: u8` parameter to both `imported_provider_config` and `add_openai_provider_for_openai_auth`, then use:

```rust
auto_compact_percent,
```

in both `ProviderConfig` literals. When there is no Codex config, call `add_openai_provider_for_openai_auth` with `DEFAULT_AUTO_COMPACT_PERCENT`.

- [ ] **Step 5: 运行全部 Codex 导入测试**

Run: `cargo test provider_config::codex::tests --all-features --locked`

Expected: PASS, including custom provider import, synthesized OpenAI import, floor rounding, and nonblocking fallback cases.

- [ ] **Step 6: 提交 Codex 导入行为**

```bash
git add src/provider_config/codex_import.rs src/provider_config/codex.rs
git commit -m 'feat: 导入 Codex 自动压缩百分比' -m $'从现有 Codex 顶层 token 阈值恢复 provider 百分比。\n\n- 对 total 作用域执行安全向下反算\n- 对错误类型和不兼容作用域回退 70%'
```

---

### Task 4: 应用 provider 时写入 Codex 绝对压缩阈值

**Files:**
- Modify: `src/provider_config/codex_apply.rs:45-690`

**Interfaces:**
- Consumes: `ProviderConfig::auto_compact_percent`
- Consumes: `ModelCatalog::auto_compact_token_limit(Option<&str>, u8) -> u64`
- Produces: top-level `model_auto_compact_token_limit` integer and `model_auto_compact_token_limit_scope = "total"`.

- [ ] **Step 1: 写失败测试覆盖已知、未知和继承模型窗口**

Add `"context_window":372000` to the Sol and Luna entries in `gpt_5_6_catalog()`. Add:

```rust
#[test]
fn writes_auto_compact_limit_from_model_context_window() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "service_tier = \"default\"\n").unwrap();
    let provider = ProviderConfig {
        model: Some("gpt-5.6-sol".to_string()),
        reasoning_effort: Some("low".to_string()),
        plan_reasoning_effort: Some("low".to_string()),
        auto_compact_percent: 70,
        api_key: Some("sk-test".to_string()),
        env_key: None,
        base_url: "https://example.test/v1".to_string(),
        wire_api: "responses".to_string(),
        auth_mode: ProviderAuthMode::ApiKey,
    };

    apply_provider_to_codex("switcher", &provider, &config_path, &gpt_5_6_catalog()).unwrap();

    let config = fs::read_to_string(config_path).unwrap();
    let doc = toml::from_str::<toml::Value>(&config).unwrap();
    assert_eq!(
        doc.get("model_auto_compact_token_limit")
            .and_then(toml::Value::as_integer),
        Some(260_400)
    );
    assert_eq!(
        doc.get("model_auto_compact_token_limit_scope")
            .and_then(toml::Value::as_str),
        Some("total")
    );
    assert_eq!(
        doc.get("service_tier").and_then(toml::Value::as_str),
        Some("default")
    );
}

#[test]
fn unknown_model_uses_compatibility_auto_compact_limit() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut provider = ProviderConfig::new("https://example.test/v1", "responses")
        .with_model("custom-model");
    provider.api_key = Some("sk-test".to_string());

    apply_with_default_catalog("switcher", &provider, &config_path).unwrap();

    let config = fs::read_to_string(config_path).unwrap();
    let doc = toml::from_str::<toml::Value>(&config).unwrap();
    assert_eq!(
        doc.get("model_auto_compact_token_limit")
            .and_then(toml::Value::as_integer),
        Some(190_400)
    );
}

#[test]
fn empty_provider_model_uses_existing_model_for_auto_compact_limit() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "model = \"gpt-5.6-sol\"\n").unwrap();
    let mut provider = ProviderConfig::new("https://example.test/v1", "responses");
    provider.auto_compact_percent = 69;
    provider.api_key = Some("sk-test".to_string());

    apply_provider_to_codex("switcher", &provider, &config_path, &gpt_5_6_catalog()).unwrap();

    let config = fs::read_to_string(config_path).unwrap();
    let doc = toml::from_str::<toml::Value>(&config).unwrap();
    assert_eq!(
        doc.get("model_auto_compact_token_limit")
            .and_then(toml::Value::as_integer),
        Some(256_680)
    );
}
```

Extend `builtin_openai_provider_does_not_write_custom_provider_table` to assert that the same two top-level compact keys are present.

- [ ] **Step 2: 运行新测试并确认压缩键尚未写入**

Run: `cargo test writes_auto_compact_limit_from_model_context_window --all-features --locked`

Expected: FAIL because `model_auto_compact_token_limit` is absent.

- [ ] **Step 3: 在统一写回边界计算并写入两个顶层键**

In `write_codex_config`, after computing the effective model and normalized efforts, add:

```rust
let auto_compact_token_limit = model_catalog
    .auto_compact_token_limit(effective_model, provider.auto_compact_percent);
let auto_compact_token_limit = i64::try_from(auto_compact_token_limit)
    .context("auto compact token limit exceeds Codex TOML integer range")?;
```

After writing the two reasoning keys, add:

```rust
doc["model_auto_compact_token_limit"] = value(auto_compact_token_limit);
doc["model_auto_compact_token_limit_scope"] = value("total");
```

Keep these assignments before the built-in OpenAI early return so OpenAI and custom providers share the behavior.

- [ ] **Step 4: 运行 Codex 写回测试**

Run: `cargo test provider_config::codex_apply::tests --all-features --locked`

Expected: PASS; unrelated top-level keys remain unchanged and custom-provider table behavior is unchanged.

- [ ] **Step 5: 提交 Codex 写回行为**

```bash
git add src/provider_config/codex_apply.rs
git commit -m 'feat: 写入 Codex 自动压缩阈值' -m $'应用 provider 时把百分比转换成 Codex 顶层 token 配置。\n\n- 按有效模型窗口计算绝对阈值\n- 固定使用完整上下文 total 作用域'
```

---

### Task 5: 在 Provider 编辑器中输入和保存百分比

**Files:**
- Modify: `src/app/provider_editor.rs:1-710`
- Modify: `src/app/providers.rs:286-610`
- Modify: `src/ui/provider_editor_input.rs:1-60`
- Modify: `src/ui/provider_editor_view.rs:1-285`
- Modify: `src/ui/input.rs:444-522`

**Interfaces:**
- Produces: `ProviderField::AutoCompactPercent`
- Produces: `ProviderEditor::parsed_auto_compact_percent() -> anyhow::Result<u8>`
- Produces: `ProviderEditor::auto_compact_percent: TextField`
- Consumes: default/min/max constants from Task 1.

- [ ] **Step 1: 写失败测试覆盖默认值、编辑、清除、导航和范围错误**

Add to `src/app/provider_editor.rs` tests:

```rust
#[test]
fn auto_compact_percent_defaults_loads_and_validates() {
    let editor = ProviderEditor::new();
    assert_eq!(editor.auto_compact_percent.as_str(), "70");
    assert_eq!(editor.parsed_auto_compact_percent().unwrap(), 70);

    let mut provider = api_key_provider();
    provider.auto_compact_percent = 65;
    let mut editor = ProviderEditor::from_provider("switcher", &provider);
    assert_eq!(editor.auto_compact_percent.as_str(), "65");

    editor.auto_compact_percent.set("1");
    assert_eq!(editor.parsed_auto_compact_percent().unwrap(), 1);
    editor.auto_compact_percent.set("99");
    assert_eq!(editor.parsed_auto_compact_percent().unwrap(), 99);

    for invalid in ["", "abc", "0", "100", "999"] {
        editor.auto_compact_percent.set(invalid);
        assert!(editor.parsed_auto_compact_percent().is_err());
    }
}

#[test]
fn auto_compact_field_participates_in_navigation_and_reset() {
    let mut editor = ProviderEditor::new();
    editor.active_field = ProviderField::PlanReasoningEffort;

    editor.next_field();
    assert_eq!(editor.active_field, ProviderField::AutoCompactPercent);
    editor.auto_compact_percent.set("65");
    editor.clear_active_field();
    assert_eq!(editor.auto_compact_percent.as_str(), "70");

    editor.next_field();
    assert_eq!(editor.active_field, ProviderField::Id);
    editor.previous_field();
    assert_eq!(editor.active_field, ProviderField::AutoCompactPercent);
}
```

Add to `src/app/providers.rs` tests:

```rust
fn app_with_registry_and_paths(registry: ProviderRegistry, root: &std::path::Path) -> App {
    App::new(
        Vec::new(),
        PathBuf::from("/repo/current"),
        registry,
        root.join("providers.toml"),
        root.join("config.toml"),
        root.join("sessions"),
    )
}

#[test]
fn saves_auto_compact_percent_from_editor() {
    let dir = tempdir().unwrap();
    let mut app = app_with_registry_and_paths(ProviderRegistry::default(), dir.path());
    let mut editor = ProviderEditor::new();
    editor.id.set("switcher");
    editor.base_url.set("https://example.test/v1");
    editor.api_key.set("sk-test");
    editor.auto_compact_percent.set("65");
    app.providers.editor = Some(editor);
    app.overlay = Some(Overlay::ProviderEditor);

    app.save_provider_editor();

    assert_eq!(
        app.providers.registry.providers["switcher"].auto_compact_percent,
        65
    );
    assert_eq!(app.overlay, None);
}

#[test]
fn invalid_auto_compact_percent_keeps_editor_open() {
    let dir = tempdir().unwrap();
    let mut app = app_with_registry_and_paths(ProviderRegistry::default(), dir.path());
    let mut editor = ProviderEditor::new();
    editor.id.set("switcher");
    editor.base_url.set("https://example.test/v1");
    editor.api_key.set("sk-test");
    editor.auto_compact_percent.set("100");
    app.providers.editor = Some(editor);
    app.overlay = Some(Overlay::ProviderEditor);

    app.save_provider_editor();

    assert!(app.providers.registry.providers.is_empty());
    assert_eq!(app.overlay, Some(Overlay::ProviderEditor));
    assert!(app.providers.editor.is_some());
    assert!(
        app.error
            .as_deref()
            .is_some_and(|error| error.contains("between 1 and 99"))
    );
}
```

- [ ] **Step 2: 运行编辑器测试并确认字段/API 缺失**

Run: `cargo test auto_compact_percent_defaults_loads_and_validates --all-features --locked`

Expected: FAIL to compile because the field, enum variant, and parser are absent.

- [ ] **Step 3: 实现编辑器字段、解析和导航**

Import:

```rust
use anyhow::{Context, Result, bail};

use crate::provider_config::{
    DEFAULT_AUTO_COMPACT_PERCENT, MAX_AUTO_COMPACT_PERCENT, MIN_AUTO_COMPACT_PERCENT,
    ModelCatalog, ProviderAuthMode, ProviderConfig, ReasoningProfile, effective_model,
};
```

Add `AutoCompactPercent` after `PlanReasoningEffort`, and add this public editor field:

```rust
pub auto_compact_percent: TextField,
```

Initialize new editors with:

```rust
auto_compact_percent: TextField::new(DEFAULT_AUTO_COMPACT_PERCENT.to_string()),
```

Initialize existing providers with:

```rust
auto_compact_percent: TextField::new(provider.auto_compact_percent.to_string()),
```

Add:

```rust
pub fn parsed_auto_compact_percent(&self) -> Result<u8> {
    let percent = self
        .auto_compact_percent
        .trim()
        .parse::<u8>()
        .context("auto_compact_percent must be an integer between 1 and 99")?;
    if !(MIN_AUTO_COMPACT_PERCENT..=MAX_AUTO_COMPACT_PERCENT).contains(&percent) {
        bail!(
            "auto_compact_percent must be between {MIN_AUTO_COMPACT_PERCENT} and {MAX_AUTO_COMPACT_PERCENT}"
        );
    }
    Ok(percent)
}
```

Update `is_editable_field`, `clear_active_field`, `active_text_mut`, `text_cursor_for`, and the non-option branch of `cycle_active_option` for the new text field. Use this reset code:

```rust
ProviderField::AutoCompactPercent => {
    self.auto_compact_percent
        .set(DEFAULT_AUTO_COMPACT_PERCENT.to_string());
}
```

Update navigation with this exact tail:

```rust
Self::ReasoningEffort => Self::PlanReasoningEffort,
Self::PlanReasoningEffort => Self::AutoCompactPercent,
Self::AutoCompactPercent => Self::Id,
```

and:

```rust
Self::Id => Self::AutoCompactPercent,
Self::AutoCompactPercent => Self::PlanReasoningEffort,
```

- [ ] **Step 4: 解析后再构造并保存 `ProviderConfig`**

At the start of `save_provider_editor`, after taking the editor, add:

```rust
let auto_compact_percent = match editor.parsed_auto_compact_percent() {
    Ok(percent) => percent,
    Err(err) => {
        self.restore_provider_editor_with_error(editor, format!("Invalid provider: {err}"));
        return;
    }
};
```

Set the field in the saved config:

```rust
auto_compact_percent,
```

- [ ] **Step 5: 过滤非数字输入并渲染文本字段**

In `src/ui/provider_editor_input.rs`, immediately before calling `active_text_mut`, ignore non-ASCII characters for the compact field:

```rust
if editor.active_field == ProviderField::AutoCompactPercent
    && matches!(key.code, KeyCode::Char(ch) if !ch.is_ascii_digit())
{
    return;
}
```

In `src/ui/provider_editor_view.rs`, append this editor line after Plan Reason:

```rust
provider_editor_line(
    editor,
    ProviderField::AutoCompactPercent,
    "auto_compact",
    editor.auto_compact_percent.as_str(),
),
```

Map it to row `8` in `provider_editor_field_row` and return its text/cursor in `provider_editor_active_text`. Make the active-field options line explain the exact unit and range:

```rust
ProviderField::AutoCompactPercent => {
    Some("1..99 percent | default 70".to_string())
}
```

Add a view assertion that activating `AutoCompactPercent` makes `provider_editor_options_line` contain `"1..99 percent | default 70"`.

Add this behavior to `provider_editor_key_routes_text_model_and_option_updates` in `src/ui/input.rs`:

```rust
let editor = app.providers.editor_mut().unwrap();
editor.active_field = ProviderField::AutoCompactPercent;
editor.auto_compact_percent.clear();
handle_provider_editor_key(
    &mut app,
    KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE),
);
handle_provider_editor_key(
    &mut app,
    KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
);
assert_eq!(
    app.providers.editor().unwrap().auto_compact_percent.as_str(),
    "7"
);
```

Extend the cursor test in `src/ui/provider_editor_view.rs` so `AutoCompactPercent` at row `8` returns a cursor position based on the same label width `13`.

- [ ] **Step 6: 运行编辑器、输入和保存测试**

Run: `cargo test app::provider_editor::tests --all-features --locked`

Expected: PASS.

Run: `cargo test app::providers::tests --all-features --locked`

Expected: PASS, including editor restoration on invalid percentage.

Run: `cargo test provider_editor --all-features --locked`

Expected: PASS for app and UI provider-editor tests.

- [ ] **Step 7: 提交编辑器功能**

```bash
git add src/app/provider_editor.rs src/app/providers.rs src/ui/provider_editor_input.rs src/ui/provider_editor_view.rs src/ui/input.rs
git commit -m 'feat: 编辑自动压缩百分比' -m $'在 Provider 编辑器中提供数字百分比输入和严格保存校验。\n\n- 新建和重置字段时使用 70%\n- 无效值保留编辑器并显示范围错误'
```

---

### Task 6: 展示百分比、更新文档并完成全量验证

**Files:**
- Modify: `src/ui/details.rs:158-195`
- Modify: `src/ui/tables.rs:15-225`
- Modify: `src/ui/mod.rs:247-375`
- Modify: `README.md:165-225`

**Interfaces:**
- Produces: provider shared display item `("compact", "70%")`.
- Preserves: Claude row column count and all existing provider display ordering.

- [ ] **Step 1: 写失败测试固定列表和详情显示顺序**

Update `provider_display_items_keep_readable_order` in `src/ui/mod.rs` to use a non-default value and assert the new slot:

```rust
let provider = ProviderConfig {
    model: Some("gpt-5.5".to_string()),
    reasoning_effort: Some("high".to_string()),
    plan_reasoning_effort: None,
    auto_compact_percent: 65,
    api_key: Some("sk-test".to_string()),
    env_key: None,
    base_url: "https://api.example.test/v1".to_string(),
    wire_api: "responses".to_string(),
    auth_mode: ProviderAuthMode::ApiKey,
};

assert_eq!(items[8], ("compact", "65%".to_string()));
assert_eq!(items[9].1, "s******t");
```

Extend `provider_details_preserve_supported_gpt_5_6_effort` with:

```rust
assert!(text.contains("compact    : 70%"));
```

Add a focused table test in `src/ui/tables.rs`:

```rust
#[test]
fn claude_status_row_matches_provider_column_count() {
    let status = ClaudeStatus::default();
    let cells = claude_status_cells(&status);

    assert_eq!(cells.len(), PROVIDER_DISPLAY_LABELS.len());
}
```

- [ ] **Step 2: 运行显示测试并确认数组长度/索引失败**

Run: `cargo test provider_display_items_keep_readable_order --all-features --locked`

Expected: FAIL because `provider_display_items` still returns nine entries and has no `compact` item.

- [ ] **Step 3: 增加共享显示项和表格列**

Change `provider_display_items` to return ten entries and insert before `api_key`:

```rust
let compact_percent = provider.auto_compact_percent;
let compact = format!("{compact_percent}%");

("compact", compact),
```

Change the table labels and widths to:

```rust
pub(super) const PROVIDER_DISPLAY_LABELS: [&str; 10] = [
    "id",
    "status",
    "model",
    "auth_mode",
    "base_url",
    "wire_api",
    "reason",
    "plan_reason",
    "compact",
    "api_key",
];

const PROVIDER_TABLE_WIDTHS: [Constraint; 10] = [
    Constraint::Length(18),
    Constraint::Length(9),
    Constraint::Length(18),
    Constraint::Length(10),
    Constraint::Min(28),
    Constraint::Length(14),
    Constraint::Length(10),
    Constraint::Length(12),
    Constraint::Length(9),
    Constraint::Length(16),
];
```

Refactor the existing Claude row constructor into this fixed-size helper; its final two placeholders represent `compact` and `api_key`:

```rust
fn claude_status_row(status: &ClaudeStatus) -> Row<'static> {
    Row::new(claude_status_cells(status))
}

fn claude_status_cells(status: &ClaudeStatus) -> [Cell<'static>; 10] {
    let dash = || "-".to_string();
    let login = if status.logged_in() {
        Cell::from("login").style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Cell::from(dash())
    };
    [
        Cell::from("claude").style(Style::default().fg(Color::Magenta)),
        login,
        Cell::from(status.model.clone().unwrap_or_else(dash)),
        Cell::from(if status.logged_in() {
            "oauth".to_string()
        } else {
            dash()
        }),
        Cell::from(status.base_url.clone().unwrap_or_else(dash)),
        Cell::from(dash()),
        Cell::from(dash()),
        Cell::from(dash()),
        Cell::from(dash()),
        Cell::from(dash()),
    ]
}
```

- [ ] **Step 4: 更新 README 配置示例和字段说明**

Add the field after `plan_mode_reasoning_effort` in the OpenAI example and after `model` in the local-provider example:

```toml
auto_compact_percent = 70
```

Add this field-table row:

```markdown
| `auto_compact_percent` | 自动历史压缩阈值占模型上下文窗口的百分比；整数 `1..=99`，默认 `70` |
```

After the reasoning-level compatibility paragraph, add:

```markdown
应用 provider 时，codex-board 会读取当前 Codex bundled model catalog 的
`context_window`，把 `auto_compact_percent` 换算为顶层
`model_auto_compact_token_limit`，并写入
`model_auto_compact_token_limit_scope = "total"`。GPT-5.6 当前窗口为
`372000`，默认 `70%` 会写入 `260400`；未知模型按 `272000` 计算，写入
`190400`。自动压缩不能阻止单个超大输入或工具输出一次跨过阈值。
```

- [ ] **Step 5: 运行显示测试和全量测试**

Run: `cargo test provider_display_items_keep_readable_order --all-features --locked`

Expected: PASS with `compact` at index `8` and API key at index `9`.

Run: `cargo test --all-features --locked`

Expected: PASS with zero failed tests.

- [ ] **Step 6: 运行格式和 CI 同款 Clippy**

Run: `cargo fmt --all -- --check`

Expected: exit code `0` and no diff. If it fails, run `cargo fmt --all`, inspect the formatter changes, then rerun the check.

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

Expected: exit code `0` with no warnings.

- [ ] **Step 7: 提交 UI 和文档**

```bash
git add src/ui/details.rs src/ui/tables.rs src/ui/mod.rs README.md
git commit -m 'feat: 展示自动压缩百分比' -m $'在 Provider 列表、详情和文档中公开自动压缩配置。\n\n- 增加 compact 列并保持 Claude 行对齐\n- 说明 70% 默认值和 Codex token 换算规则'
```

- [ ] **Step 8: 提交后执行最终证据检查**

Run: `cargo test --all-features --locked`

Expected: PASS with zero failed tests.

Run: `cargo fmt --all -- --check`

Expected: exit code `0`.

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

Expected: exit code `0` with no warnings.

Run: `git status --short --branch`

Expected: no uncommitted implementation changes; branch remains ahead only by the intended design, plan, and feature commits.
