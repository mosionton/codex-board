# 模型感知推理等级 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 codex-board 根据当前安装的 Codex bundled model catalog 动态选择模型默认推理等级和候选项，完整支持 GPT-5.6 Sol、Terra、Luna。

**Architecture:** 新增只负责加载、解析和查询 Codex 模型目录的 `ModelCatalog`，应用启动时加载一次并通过 `ProvidersState` 共享。Provider 编辑器、Codex 配置导入、详情显示和最终写入统一使用该目录；目录不可用或模型未知时使用现有四档兼容 profile。

**Tech Stack:** Rust 2024、Serde/serde_json、Anyhow、Ratatui、Crossterm、TOML、Cargo 内联单元测试。

## Global Constraints

- 模型能力来源必须是 `codex debug models --bundled`，不得读取 `~/.codex/models_cache.json`。
- `gpt-5.6` 必须解析为 `gpt-5.6-sol`。
- 未知模型或目录加载失败时，默认值为 `medium`，候选项为 `low`、`medium`、`high`、`xhigh`。
- Sol 和 Terra 支持到 `ultra`；Luna 支持到 `max`。
- 普通模式和 Plan 模式共享模型目录的支持列表，但保留独立当前值和显式选择状态。
- 已有且被新模型支持的显式值必须保留；缺失、非显式或不支持的值回退到模型默认。
- 保持 `model_reasoning_effort`、`plan_mode_reasoning_effort` 及其旧别名的 TOML 兼容性。
- 所有手工编辑必须使用 `apply_patch`；每个 Rust 任务遵循 red-green TDD。
- 最终必须运行 `cargo test`、`cargo fmt --all -- --check` 和仓库要求的完整 Clippy 命令。
- 提交使用 Conventional Commits，类型为英文，摘要和非平凡正文为中文。

---

## File Structure

- Create: `src/provider_config/model_catalog.rs` — 加载、解析、校验和查询 bundled model catalog。
- Modify: `src/provider_config/mod.rs` — 导出目录类型。
- Modify: `src/provider_config/registry.rs` — 删除固定四档 normalizer。
- Modify: `src/provider_config/codex_import.rs`、`src/provider_config/codex.rs` — 模型感知导入及测试。
- Modify: `src/provider_config/codex_apply.rs` — 最终写入校验。
- Modify: `src/app/providers_state.rs`、`src/app/runtime.rs` — 启动加载和共享目录。
- Modify: `src/app/provider_editor.rs`、`src/app/providers.rs` — 编辑器动态 profile 和保存/应用边界。
- Modify: `src/ui/provider_editor_input.rs`、`src/ui/provider_editor_view.rs`、`src/ui/details.rs` — 交互和显示。
- Modify: `src/ui/input.rs`、`src/ui/mod.rs`、`src/app/mod.rs` — 更新集成测试。
- Modify: `README.md` — 用户可见配置说明。

---

### Task 1: 实现 Codex 模型目录解析与兼容回退

**Files:**
- Create: `src/provider_config/model_catalog.rs`
- Modify: `src/provider_config/mod.rs:1-18`

**Interfaces:**
- Produces: `ModelCatalog::load_bundled() -> ModelCatalogLoad`
- Produces: `ModelCatalog::from_json(&str) -> anyhow::Result<ModelCatalog>`
- Produces: `ModelCatalog::profile_for(Option<&str>) -> &ReasoningProfile`
- Produces: `ModelCatalog::normalize_effort(Option<&str>, Option<&str>) -> String`
- Produces: `ReasoningProfile::{default_effort, supported_efforts, supports}`

- [ ] **Step 1: 写失败测试覆盖 GPT-5.6、别名、未知模型和坏目录**

Create `src/provider_config/model_catalog.rs` with this fixture and test contract:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const GPT_5_6_CATALOG: &str = r#"{
      "models": [
        {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}],"ignored":true},
        {"slug":"gpt-5.6-terra","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
        {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
      ]
    }"#;

    #[test]
    fn parses_gpt_5_6_profiles_and_alias() {
        let catalog = ModelCatalog::from_json(GPT_5_6_CATALOG).unwrap();
        let sol = catalog.profile_for(Some("gpt-5.6-sol"));
        assert_eq!(sol.default_effort(), "low");
        assert_eq!(sol.supported_efforts(), ["low", "medium", "high", "xhigh", "max", "ultra"]);
        assert_eq!(catalog.profile_for(Some("gpt-5.6")), sol);
        assert!(catalog.profile_for(Some("gpt-5.6-terra")).supports("ultra"));
        assert!(!catalog.profile_for(Some("gpt-5.6-luna")).supports("ultra"));
    }

    #[test]
    fn unknown_model_uses_compatibility_profile() {
        let catalog = ModelCatalog::from_json(GPT_5_6_CATALOG).unwrap();
        let profile = catalog.profile_for(Some("custom-model"));
        assert_eq!(profile.default_effort(), "medium");
        assert_eq!(profile.supported_efforts(), ["low", "medium", "high", "xhigh"]);
        assert_eq!(catalog.normalize_effort(Some("custom-model"), Some("ultra")), "medium");
    }

    #[test]
    fn ignores_invalid_entries_and_rejects_empty_catalogs() {
        let catalog = ModelCatalog::from_json(
            r#"{"models":[{"slug":"","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"medium"}]},{"slug":"valid","default_reasoning_level":"high","supported_reasoning_levels":[{"effort":"low"},{"effort":"high"}]}]}"#,
        )
        .unwrap();
        assert_eq!(catalog.profile_for(Some("valid")).default_effort(), "high");
        assert!(ModelCatalog::from_json(r#"{"models":[]}"#).is_err());
    }

    #[test]
    fn command_failures_return_warning_and_fallback_catalog() {
        let loaded = ModelCatalog::from_command_result(false, b"");
        assert!(loaded.warning.is_some());
        assert_eq!(loaded.catalog.profile_for(Some("gpt-5.6-sol")).supported_efforts(), ["low", "medium", "high", "xhigh"]);
    }
}
```

- [ ] **Step 2: 运行测试并确认目录 API 缺失**

Run: `cargo test provider_config::model_catalog::tests -- --nocapture`

Expected: FAIL with unresolved `ModelCatalog`, `ReasoningProfile`, or missing methods.

- [ ] **Step 3: 实现目录类型、解析、校验和命令加载**

Implement these production types and methods above the tests:

```rust
use std::{collections::BTreeMap, process::Command};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const FALLBACK_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const WARNING: &str = "Codex model catalog unavailable; using compatibility reasoning options.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningProfile {
    default_effort: String,
    supported_efforts: Vec<String>,
}

impl ReasoningProfile {
    pub fn default_effort(&self) -> &str { &self.default_effort }
    pub fn supported_efforts(&self) -> &[String] { &self.supported_efforts }
    pub fn supports(&self, effort: &str) -> bool {
        self.supported_efforts.iter().any(|supported| supported == effort)
    }
    pub fn normalize(&self, value: Option<&str>) -> String {
        value.map(str::trim).filter(|value| self.supports(value)).unwrap_or(&self.default_effort).to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    profiles: BTreeMap<String, ReasoningProfile>,
    fallback: ReasoningProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalogLoad {
    pub catalog: ModelCatalog,
    pub warning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CatalogResponse { models: Vec<CatalogModel> }

#[derive(Debug, Deserialize)]
struct CatalogModel {
    slug: String,
    default_reasoning_level: String,
    #[serde(default)]
    supported_reasoning_levels: Vec<CatalogLevel>,
}

#[derive(Debug, Deserialize)]
struct CatalogLevel { effort: String }
```

`Default` must create an empty exact-match map and fallback profile `medium` plus the four compatibility efforts. `from_json` must deserialize with context, trim and deduplicate efforts while preserving order, ignore invalid entries, and `bail!("Codex model catalog contains no valid models")` if none remain. `profile_for` must map exact `gpt-5.6` to `gpt-5.6-sol`; all other misses return the fallback reference.

Implement command loading with this exact nonfatal shape:

```rust
pub fn load_bundled() -> ModelCatalogLoad {
    match Command::new("codex").args(["debug", "models", "--bundled"]).output() {
        Ok(output) => Self::from_command_result(output.status.success(), &output.stdout),
        Err(_) => ModelCatalogLoad {
            catalog: Self::default(),
            warning: Some(WARNING.to_string()),
        },
    }
}

fn from_command_result(success: bool, stdout: &[u8]) -> ModelCatalogLoad {
    if success
        && let Ok(text) = std::str::from_utf8(stdout)
        && let Ok(catalog) = Self::from_json(text)
    {
        return ModelCatalogLoad { catalog, warning: None };
    }
    ModelCatalogLoad {
        catalog: Self::default(),
        warning: Some(WARNING.to_string()),
    }
}
```

Export the three public types from `src/provider_config/mod.rs`.

- [ ] **Step 4: 运行目录测试和格式检查**

Run:

```sh
cargo test provider_config::model_catalog::tests -- --nocapture
cargo fmt --all -- --check
```

Expected: 4 tests PASS; format exits 0.

- [ ] **Step 5: 提交目录模块**

```sh
git add src/provider_config/model_catalog.rs src/provider_config/mod.rs
git commit -m "feat: 增加 Codex 模型目录解析" -m "从当前 Codex 二进制读取 bundled model catalog，并为未知模型提供兼容回退。\n\n- 解析模型默认推理等级和支持列表\n- 支持 GPT-5.6 系列别名\n- 在命令或目录失败时返回非阻断警告"
```

---

### Task 2: 启动时加载并共享模型目录

**Files:**
- Modify: `src/app/providers_state.rs:1-75`
- Modify: `src/app/runtime.rs:17-48`

**Interfaces:**
- Consumes: `ModelCatalog::load_bundled()`
- Produces: `ProvidersState::model_catalog() -> Arc<ModelCatalog>`
- Produces: `ProvidersState::set_model_catalog(ModelCatalog)`

- [ ] **Step 1: 写失败测试覆盖共享目录状态**

Add to `src/app/providers_state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_shared_model_catalog() {
        let mut state = ProvidersState::new(
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
        );
        let catalog = ModelCatalog::from_json(
            r#"{"models":[{"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"max"}]}]}"#,
        )
        .unwrap();
        state.set_model_catalog(catalog);
        assert_eq!(state.model_catalog().profile_for(Some("gpt-5.6-sol")).default_effort(), "low");
    }
}
```

- [ ] **Step 2: 运行测试并确认状态 API 缺失**

Run: `cargo test app::providers_state::tests::stores_shared_model_catalog -- --exact`

Expected: FAIL because the model catalog field and methods do not exist.

- [ ] **Step 3: 在 ProvidersState 中保存 Arc<ModelCatalog>**

Import `std::sync::Arc` and `ModelCatalog`, add:

```rust
pub(super) model_catalog: Arc<ModelCatalog>,
```

Initialize it with:

```rust
model_catalog: Arc::new(ModelCatalog::default()),
```

Add:

```rust
pub(crate) fn model_catalog(&self) -> Arc<ModelCatalog> {
    Arc::clone(&self.model_catalog)
}

pub(crate) fn set_model_catalog(&mut self, catalog: ModelCatalog) {
    self.model_catalog = Arc::new(catalog);
}
```

- [ ] **Step 4: 在 runtime 中安装目录和一次性警告**

After resolving Codex paths, load:

```rust
let model_catalog_load = provider_config::ModelCatalog::load_bundled();
```

After `App::new`, install:

```rust
app.providers.set_model_catalog(model_catalog_load.catalog);
if let Some(warning) = model_catalog_load.warning {
    app.show_status(warning);
}
```

Do not propagate catalog failure through `run() -> Result<()>`.

- [ ] **Step 5: 运行状态测试并提交**

Run:

```sh
cargo test app::providers_state::tests -- --nocapture
cargo fmt --all -- --check
```

Expected: PASS and format exit 0.

Commit:

```sh
git add src/app/providers_state.rs src/app/runtime.rs
git commit -m "feat: 启动时加载模型能力目录" -m "在应用启动阶段读取一次 Codex bundled catalog，并通过 provider 状态共享。\n\n- 使用 Arc 共享目录\n- 目录失败时保留兼容 profile\n- 在 TUI 中显示一次非阻断提示"
```

---

### Task 3: 让 Codex 配置导入按模型归一化

**Files:**
- Modify: `src/provider_config/codex_import.rs:1-175`
- Modify: `src/provider_config/codex.rs:20-290`
- Modify: `src/app/runtime.rs:20-29`

**Interfaces:**
- Consumes: `ModelCatalog::normalize_effort(model, value)`
- Changes: `load_codex_config_providers(config_path, auth_path, catalog) -> Result<ProviderRegistry>`
- Produces: synthesized `openai` provider with inherited top-level model and efforts

- [ ] **Step 1: 写失败测试覆盖 GPT-5.6 导入和 OpenAI 继承**

Add this helper in `src/provider_config/codex.rs` tests:

```rust
fn gpt_5_6_catalog() -> ModelCatalog {
    ModelCatalog::from_json(
        r#"{"models":[
          {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
          {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
        ]}"#,
    )
    .unwrap()
}
```

In the `src/provider_config/codex.rs` test module, extend the provider-config import and add a compatibility helper for the pre-existing tests:

```rust
use crate::provider_config::{ModelCatalog, ProviderAuthMode, ProviderRegistry};

fn load_with_default_catalog(
    config_path: &Path,
    auth_path: &Path,
) -> anyhow::Result<ProviderRegistry> {
    load_codex_config_providers(config_path, auth_path, &ModelCatalog::default())
}
```

Replace every pre-existing two-argument `load_codex_config_providers(&config_path, &auth_path)` call in this test module with `load_with_default_catalog(&config_path, &auth_path)`. Then add these two complete tests:

```rust
#[test]
fn imports_supported_gpt_5_6_efforts_for_custom_provider() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let auth_path = dir.path().join("auth.json");
    fs::write(
        &config_path,
        r#"
model = "gpt-5.6-sol"
model_reasoning_effort = "ultra"
plan_mode_reasoning_effort = "max"

[model_providers.switcher]
base_url = "https://example.test/v1"
wire_api = "responses"
"#,
    )
    .unwrap();

    let registry =
        load_codex_config_providers(&config_path, &auth_path, &gpt_5_6_catalog()).unwrap();
    let provider = registry.providers.get("switcher").unwrap();

    assert_eq!(provider.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(provider.reasoning_effort.as_deref(), Some("ultra"));
    assert_eq!(provider.plan_reasoning_effort.as_deref(), Some("max"));
}

#[test]
fn synthesized_openai_inherits_top_level_model_and_efforts() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let auth_path = dir.path().join("auth.json");
    fs::write(
        &config_path,
        r#"
model = "gpt-5.6-sol"
model_reasoning_effort = "ultra"
plan_mode_reasoning_effort = "max"
"#,
    )
    .unwrap();
    fs::write(
        &auth_path,
        r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"access_token":"chatgpt-access-token"}}"#,
    )
    .unwrap();

    let registry =
        load_codex_config_providers(&config_path, &auth_path, &gpt_5_6_catalog()).unwrap();
    let provider = registry.providers.get("openai").unwrap();

    assert_eq!(provider.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(provider.reasoning_effort.as_deref(), Some("ultra"));
    assert_eq!(provider.plan_reasoning_effort.as_deref(), Some("max"));
    assert_eq!(provider.auth_mode, ProviderAuthMode::OpenAi);
}
```

- [ ] **Step 2: 运行导入测试并确认旧签名或固定 medium 行为失败**

Run: `cargo test provider_config::codex::tests -- --nocapture`

Expected: FAIL because `load_codex_config_providers` lacks a catalog parameter and synthesized OpenAI has no model.

- [ ] **Step 3: 修改导入签名并统一归一化**

Change the signature:

```rust
pub fn load_codex_config_providers(
    config_path: &Path,
    auth_path: &Path,
    model_catalog: &ModelCatalog,
) -> Result<ProviderRegistry>
```

For a parsed Codex config, calculate:

```rust
let model = codex_config.model.clone();
let reasoning_effort = model_catalog.normalize_effort(
    model.as_deref(),
    codex_config.model_reasoning_effort.as_deref(),
);
let plan_reasoning_effort = model_catalog.normalize_effort(
    model.as_deref(),
    codex_config.plan_mode_reasoning_effort.as_deref(),
);
```

Pass clones to every imported provider. Change `add_openai_provider_for_openai_auth` to accept:

```rust
fn add_openai_provider_for_openai_auth(
    registry: &mut ProviderRegistry,
    auth: &CodexAuth,
    model: Option<String>,
    reasoning_effort: String,
    plan_reasoning_effort: String,
) -> Result<()>
```

Construct synthesized OpenAI with:

```rust
ProviderConfig {
    model,
    reasoning_effort: Some(reasoning_effort),
    plan_reasoning_effort: Some(plan_reasoning_effort),
    api_key: None,
    env_key: None,
    base_url: OPENAI_BASE_URL.to_string(),
    wire_api: RESPONSES_WIRE_API.to_string(),
    auth_mode: ProviderAuthMode::OpenAi,
}
```

When config is absent, pass `None` plus `model_catalog.normalize_effort(None, None)` for both efforts.

- [ ] **Step 4: 更新 runtime 调用点**

In `runtime::run`, load the catalog before import and pass `&model_catalog_load.catalog`. The test call sites already use the `load_with_default_catalog` helper from Step 1, while the two GPT-5.6 tests pass `&gpt_5_6_catalog()` directly.

Use this exact runtime call:

```rust
provider_registry.merge_defaults(provider_config::load_codex_config_providers(
    &codex_config_path,
    &codex_auth_path,
    &model_catalog_load.catalog,
)?);
```

- [ ] **Step 5: 运行导入测试、格式检查并提交**

Run:

```sh
cargo test provider_config::codex::tests -- --nocapture
cargo fmt --all -- --check
```

Expected: all import tests PASS; format exits 0.

Commit:

```sh
git add src/provider_config/codex_import.rs src/provider_config/codex.rs src/app/runtime.rs
git commit -m "feat: 按模型归一化 Codex 配置" -m "让配置导入使用模型目录校验推理等级，并修正内置 OpenAI provider 的默认继承。\n\n- 保留 GPT-5.6 支持的 max 和 ultra\n- 不支持的历史值回退到模型默认\n- 合成 OpenAI provider 时继承顶层模型设置"
```

---

### Task 4: 让 ProviderEditor 动态跟随模型

**Files:**
- Modify: `src/app/provider_editor.rs:1-460`
- Modify: `src/app/providers.rs:60-105,130-280`
- Modify: `src/ui/provider_editor_input.rs:1-65`
- Modify: `src/ui/input.rs:445-475`
- Modify: `src/app/mod.rs:290-500`

**Interfaces:**
- Consumes: `Arc<ModelCatalog>` and `ReasoningProfile`
- Produces: `ProviderEditor::new_with_catalog(Arc<ModelCatalog>)`
- Produces: `ProviderEditor::from_provider_with_catalog(id, provider, Arc<ModelCatalog>)`
- Produces: `ProviderEditor::commit_model_change()`
- Produces: dynamic option vectors and explicit-choice flags

- [ ] **Step 1: 写失败测试覆盖默认、保留、回退和 Ctrl+U**

Add this complete local fixture to the `src/app/provider_editor.rs` test module, then add the assertions below it:

```rust
fn gpt_5_6_catalog() -> Arc<ModelCatalog> {
    Arc::new(
        ModelCatalog::from_json(
            r#"{"models":[
              {"slug":"gpt-5.5","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"}]},
              {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
              {"slug":"gpt-5.6-terra","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
              {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
            ]}"#,
        )
        .unwrap(),
    )
}
```

Import `std::sync::Arc` and `crate::provider_config::ModelCatalog` in the test module. Add these tests:

```rust
#[test]
fn new_editor_uses_selected_model_default() {
    let mut editor = ProviderEditor::new_with_catalog(gpt_5_6_catalog());
    editor.model.set("gpt-5.6-sol");
    editor.commit_model_change();
    assert_eq!(editor.reasoning_effort, "low");
    assert_eq!(editor.plan_reasoning_effort, "low");
    assert_eq!(editor.reasoning_effort_options, ["low", "medium", "high", "xhigh", "max", "ultra"]);
}

#[test]
fn preserves_supported_explicit_effort_and_replaces_unsupported_effort() {
    let provider = ProviderConfig {
        model: Some("gpt-5.6-sol".to_string()),
        reasoning_effort: Some("xhigh".to_string()),
        plan_reasoning_effort: Some("ultra".to_string()),
        api_key: Some("sk-test".to_string()),
        env_key: None,
        base_url: "https://example.test/v1".to_string(),
        wire_api: "responses".to_string(),
        auth_mode: ProviderAuthMode::ApiKey,
    };
    let mut editor = ProviderEditor::from_provider_with_catalog("switcher", &provider, gpt_5_6_catalog());
    editor.model.set("gpt-5.6-luna");
    editor.commit_model_change();
    assert_eq!(editor.reasoning_effort, "xhigh");
    assert_eq!(editor.plan_reasoning_effort, "medium");
}

#[test]
fn clearing_reasoning_restores_current_model_default() {
    let mut editor = ProviderEditor::new_with_catalog(gpt_5_6_catalog());
    editor.model.set("gpt-5.6-sol");
    editor.commit_model_change();
    editor.active_field = ProviderField::ReasoningEffort;
    assert!(editor.cycle_active_option(1));
    assert_eq!(editor.reasoning_effort, "medium");
    editor.clear_active_field();
    assert_eq!(editor.reasoning_effort, "low");
}
```

- [ ] **Step 2: 运行编辑器测试并确认动态 API 缺失**

Run: `cargo test app::provider_editor::tests -- --nocapture`

Expected: FAIL because catalog-aware constructors, option vectors, and `commit_model_change` are missing.

- [ ] **Step 3: 增加编辑器目录状态和构造器**

Add fields:

```rust
model_catalog: Arc<ModelCatalog>,
pub reasoning_effort_options: Vec<String>,
pub plan_reasoning_effort_options: Vec<String>,
reasoning_effort_explicit: bool,
plan_reasoning_effort_explicit: bool,
```

Keep compatibility wrappers:

```rust
pub fn new() -> Self {
    Self::new_with_catalog(Arc::new(ModelCatalog::default()))
}

pub fn from_provider(id: &str, provider: &ProviderConfig) -> Self {
    Self::from_provider_with_catalog(id, provider, Arc::new(ModelCatalog::default()))
}
```

Use this initializer for each saved effort:

```rust
fn initial_effort(profile: &ReasoningProfile, value: Option<&str>) -> (String, bool) {
    match value.map(str::trim).filter(|value| profile.supports(value)) {
        Some(value) => (value.to_string(), true),
        None => (profile.default_effort().to_string(), false),
    }
}
```

- [ ] **Step 4: 实现模型提交和显式选择规则**

Add:

```rust
pub fn commit_model_change(&mut self) {
    let profile = self.model_catalog.profile_for(Some(self.model.as_str())).clone();
    self.reasoning_effort_options = profile.supported_efforts().to_vec();
    self.plan_reasoning_effort_options = profile.supported_efforts().to_vec();
    if !self.reasoning_effort_explicit || !profile.supports(&self.reasoning_effort) {
        self.reasoning_effort = profile.default_effort().to_string();
        self.reasoning_effort_explicit = false;
    }
    if !self.plan_reasoning_effort_explicit || !profile.supports(&self.plan_reasoning_effort) {
        self.plan_reasoning_effort = profile.default_effort().to_string();
        self.plan_reasoning_effort_explicit = false;
    }
}
```

Use `&[String]` for dynamic reasoning option cycling. Mark the matching flag `true` after Left/Right. In `clear_active_field`, restore the current profile default and set the matching flag `false`. Call `commit_model_change` after F5 auto-selection, Up/Down selection, and clearing Model.

- [ ] **Step 5: 在离开或保存 Model 字段时提交手输值**

Change `next_field` and `previous_field` from `const fn` to `fn`; when the old field is Model, call `commit_model_change` after navigation.

Create editors in `src/app/providers.rs` with:

```rust
let model_catalog = self.providers.model_catalog();
self.providers.editor = Some(ProviderEditor::new_with_catalog(model_catalog));
```

and:

```rust
let model_catalog = self.providers.model_catalog();
self.providers.editor = Some(ProviderEditor::from_provider_with_catalog(&id, provider, model_catalog));
```

At the start of `prompt_save_provider_editor`:

```rust
if let Some(editor) = self.providers.editor.as_mut() {
    editor.commit_model_change();
}
```

- [ ] **Step 6: 更新交互测试并运行**

In `src/ui/input.rs`, import `std::sync::Arc` plus `ModelCatalog` and `ProviderAuthMode`. Append this setup and assertion to `provider_editor_key_routes_text_model_and_option_updates` after the existing model Down-key assertion:

```rust
let catalog = ModelCatalog::from_json(
    r#"{"models":[
      {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
      {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
    ]}"#,
)
.unwrap();
let provider = ProviderConfig {
    model: Some("gpt-5.6-sol".to_string()),
    reasoning_effort: Some("ultra".to_string()),
    plan_reasoning_effort: Some("max".to_string()),
    api_key: Some("sk-test".to_string()),
    env_key: None,
    base_url: "https://example.test/v1".to_string(),
    wire_api: "responses".to_string(),
    auth_mode: ProviderAuthMode::ApiKey,
};
let mut editor = ProviderEditor::from_provider_with_catalog(
    "switcher",
    &provider,
    Arc::new(catalog),
);
editor.active_field = ProviderField::Model;
editor.model_options = vec!["gpt-5.6-sol".to_string(), "gpt-5.6-luna".to_string()];
app.providers.set_editor(Some(editor));

handle_provider_editor_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

let editor = app.providers.editor().unwrap();
assert_eq!(editor.model.as_str(), "gpt-5.6-luna");
assert_eq!(editor.reasoning_effort, "medium");
assert_eq!(editor.plan_reasoning_effort, "max");
```

In `src/app/mod.rs`, replace `reasoning_options_match_gpt_5_3_and_later_strengths` and its static-constant imports with this catalog-aware test; import `std::sync::Arc` and `ModelCatalog`:

```rust
#[test]
fn reasoning_options_follow_editor_catalog() {
    let catalog = Arc::new(
        ModelCatalog::from_json(
            r#"{"models":[
              {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]}
            ]}"#,
        )
        .unwrap(),
    );
    let mut editor = ProviderEditor::new_with_catalog(catalog);
    assert_eq!(
        editor.reasoning_effort_options,
        ["low", "medium", "high", "xhigh"]
    );

    editor.model.set("gpt-5.6-sol");
    editor.commit_model_change();

    assert_eq!(
        editor.reasoning_effort_options,
        ["low", "medium", "high", "xhigh", "max", "ultra"]
    );
    assert_eq!(editor.plan_reasoning_effort_options, editor.reasoning_effort_options);
}
```

Run:

```sh
cargo test app::provider_editor::tests -- --nocapture
cargo test ui::input::tests::provider_editor_key_routes_text_model_and_option_updates -- --exact
cargo test app::tests -- --nocapture
```

Expected: all selected tests PASS.

- [ ] **Step 7: 提交编辑器联动**

```sh
git add src/app/provider_editor.rs src/app/providers.rs src/ui/provider_editor_input.rs src/ui/input.rs src/app/mod.rs
git commit -m "feat: 让推理等级跟随模型" -m "Provider 编辑器改用模型目录提供的默认值和动态候选项。\n\n- 保留新模型仍支持的显式选择\n- 不兼容值回退到模型默认\n- 覆盖拉取、方向键、手输和 Ctrl+U 流程"
```

---

### Task 5: 更新动态 UI、详情显示和最终 Codex 写入

**Files:**
- Modify: `src/ui/provider_editor_view.rs:1-255`
- Modify: `src/ui/details.rs:1-185`
- Modify: `src/ui/mod.rs:180-440`
- Modify: `src/provider_config/codex_apply.rs:20-590`
- Modify: `src/app/providers.rs:230-250`
- Modify: `src/provider_config/registry.rs:13-16,252-260`
- Modify: `src/provider_config/mod.rs:12-18`

**Interfaces:**
- Consumes: editor dynamic option vectors
- Changes: `provider_display_items(id, provider, is_applied, catalog)`
- Changes: `apply_provider_to_codex(id, provider, config_path, catalog)`
- Removes: static reasoning option constants and `normalize_reasoning_effort`

- [ ] **Step 1: 写失败测试覆盖动态选项和详情值**

In `src/ui/provider_editor_view.rs`, create a catalog-aware editor and assert:

```rust
#[test]
fn reasoning_options_follow_selected_model() {
    let catalog = Arc::new(ModelCatalog::from_json(
        r#"{"models":[
          {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
          {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
        ]}"#,
    ).unwrap());
    let mut editor = ProviderEditor::new_with_catalog(catalog);
    editor.model.set("gpt-5.6-sol");
    editor.commit_model_change();
    editor.active_field = ProviderField::ReasoningEffort;
    assert!(line_text(&provider_editor_options_line(&editor).unwrap()).contains("max | ultra"));
    editor.model.set("gpt-5.6-luna");
    editor.commit_model_change();
    assert!(!line_text(&provider_editor_options_line(&editor).unwrap()).contains("ultra"));
}
```

In `src/ui/mod.rs`, extend the provider-config test import with `ModelCatalog`, then add:

```rust
#[test]
fn provider_details_preserve_supported_gpt_5_6_effort() {
    let catalog = ModelCatalog::from_json(
        r#"{"models":[
          {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]}
        ]}"#,
    )
    .unwrap();
    let mut registry = ProviderRegistry::default();
    registry
        .upsert(
            "switcher",
            ProviderConfig {
                model: Some("gpt-5.6-sol".to_string()),
                reasoning_effort: Some("ultra".to_string()),
                plan_reasoning_effort: Some("max".to_string()),
                api_key: None,
                env_key: None,
                base_url: "https://example.test/v1".to_string(),
                wire_api: "responses".to_string(),
                auth_mode: ProviderAuthMode::ApiKey,
            },
        )
        .unwrap();
    let mut app = app_with_sessions_and_registry(
        Vec::new(),
        PathBuf::from("/repo/current"),
        registry,
    );
    app.providers.set_model_catalog(catalog);

    let text = selected_provider_details(&app, 80)
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("reason     : ultra"));
    assert!(text.contains("plan_reason: max"));
    assert!(!text.contains("reason     : medium"));
}
```

- [ ] **Step 2: 写失败测试覆盖最终写入和空模型**

In the `src/provider_config/codex_apply.rs` test module, import `ModelCatalog` and add these helpers:

```rust
fn gpt_5_6_catalog() -> ModelCatalog {
    ModelCatalog::from_json(
        r#"{"models":[
          {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
          {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
        ]}"#,
    )
    .unwrap()
}

fn apply_with_default_catalog(
    id: &str,
    provider: &ProviderConfig,
    config_path: &Path,
) -> anyhow::Result<()> {
    apply_provider_to_codex(id, provider, config_path, &ModelCatalog::default())
}
```

Replace every pre-existing three-argument `apply_provider_to_codex` call in this test module with `apply_with_default_catalog`. Then add:

```rust
#[test]
fn writes_supported_gpt_5_6_efforts() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let provider = ProviderConfig {
        model: Some("gpt-5.6-sol".to_string()),
        reasoning_effort: Some("ultra".to_string()),
        plan_reasoning_effort: Some("max".to_string()),
        api_key: Some("sk-test".to_string()),
        env_key: None,
        base_url: "https://example.test/v1".to_string(),
        wire_api: "responses".to_string(),
        auth_mode: ProviderAuthMode::ApiKey,
    };
    apply_provider_to_codex("switcher", &provider, &config_path, &gpt_5_6_catalog()).unwrap();
    let text = fs::read_to_string(&config_path).unwrap();
    assert!(text.contains("model_reasoning_effort = \"ultra\""));
    assert!(text.contains("plan_mode_reasoning_effort = \"max\""));
}

#[test]
fn empty_provider_model_uses_existing_codex_model_for_validation() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "model = \"gpt-5.6-luna\"\n").unwrap();
    let provider = ProviderConfig {
        model: None,
        reasoning_effort: Some("ultra".to_string()),
        plan_reasoning_effort: Some("max".to_string()),
        api_key: Some("sk-test".to_string()),
        env_key: None,
        base_url: "https://example.test/v1".to_string(),
        wire_api: "responses".to_string(),
        auth_mode: ProviderAuthMode::ApiKey,
    };
    apply_provider_to_codex("switcher", &provider, &config_path, &gpt_5_6_catalog()).unwrap();
    let text = fs::read_to_string(&config_path).unwrap();
    assert!(text.contains("model_reasoning_effort = \"medium\""));
    assert!(text.contains("plan_mode_reasoning_effort = \"max\""));
}
```

- [ ] **Step 3: 运行测试并确认静态 UI 和固定 normalizer 失败**

Run:

```sh
cargo test ui::provider_editor_view::tests::reasoning_options_follow_selected_model -- --exact
cargo test ui::tests::provider_details_preserve_supported_gpt_5_6_effort -- --exact
cargo test provider_config::codex_apply::tests::writes_supported_gpt_5_6_efforts -- --exact
cargo test provider_config::codex_apply::tests::empty_provider_model_uses_existing_codex_model_for_validation -- --exact
```

Expected: FAIL because UI still reads static arrays and apply still uses the four-value normalizer.

- [ ] **Step 4: 渲染编辑器动态候选项**

Remove static reasoning imports and replace the old option lookup with:

```rust
fn provider_field_options_text(editor: &ProviderEditor) -> Option<String> {
    match editor.active_field {
        ProviderField::ReasoningEffort => Some(editor.reasoning_effort_options.join(" | ")),
        ProviderField::PlanReasoningEffort => Some(editor.plan_reasoning_effort_options.join(" | ")),
        ProviderField::WireApi => Some(WIRE_API_OPTIONS.join(" | ")),
        ProviderField::Id
        | ProviderField::Model
        | ProviderField::ApiKey
        | ProviderField::BaseUrl
        | ProviderField::Auth => None,
    }
}
```

Use it in `provider_editor_options_line`:

```rust
let text = provider_field_options_text(editor)?;
Some(Line::styled(format!("Options: {text}"), Style::default().fg(Color::Gray)))
```

- [ ] **Step 5: 让详情显示使用共享目录**

Change the signature:

```rust
pub(super) fn provider_display_items(
    id: &str,
    provider: &ProviderConfig,
    is_applied: bool,
    model_catalog: &ModelCatalog,
) -> [(&'static str, String); 9]
```

Resolve values with:

```rust
let model = provider.model.as_deref();
let reasoning_effort = model_catalog.normalize_effort(model, provider.reasoning_effort.as_deref());
let plan_reasoning_effort = model_catalog.normalize_effort(model, provider.plan_reasoning_effort.as_deref());
```

Use those strings in the array. In `selected_provider_details`, clone the Arc from `app.providers.model_catalog()` and pass `model_catalog.as_ref()`. Change the existing `provider_display_items_keep_readable_order` call to:

```rust
let items = provider_display_items(
    "switcher",
    &provider,
    true,
    &ModelCatalog::default(),
);
```

- [ ] **Step 6: 让最终 Codex 写入使用有效模型 profile**

Change signatures:

```rust
pub fn apply_provider_to_codex(
    id: &str,
    provider: &ProviderConfig,
    config_path: &Path,
    model_catalog: &ModelCatalog,
) -> Result<()>
```

```rust
fn write_codex_config(
    id: &str,
    provider: &ProviderConfig,
    path: &Path,
    model_catalog: &ModelCatalog,
) -> Result<()>
```

Before overwriting `model`, determine the effective model and efforts:

```rust
let provider_model = provider.model.as_deref().map(str::trim).filter(|model| !model.is_empty());
let existing_model = doc.get("model").and_then(toml::Value::as_str);
let effective_model = provider_model.or(existing_model);
let reasoning_effort = model_catalog.normalize_effort(
    effective_model,
    provider.reasoning_effort.as_deref(),
);
let plan_reasoning_effort = model_catalog.normalize_effort(
    effective_model,
    provider.plan_reasoning_effort.as_deref(),
);
```

Write the resolved strings. In `src/app/providers.rs`, clone the shared Arc and pass `model_catalog.as_ref()` when applying. The pre-existing `codex_apply.rs` tests must call the `apply_with_default_catalog` helper defined in Step 2; the two GPT-5.6 tests continue to pass `&gpt_5_6_catalog()` directly.

- [ ] **Step 7: 删除旧全局推理常量和 normalizer**

Remove from `registry.rs` and `provider_config/mod.rs`:

```rust
pub const DEFAULT_REASONING_EFFORT: &str = "medium";
pub const REASONING_EFFORT_OPTIONS: &[&str] = &["low", "medium", "high", "xhigh"];
pub const PLAN_REASONING_EFFORT_OPTIONS: &[&str] = &["low", "medium", "high", "xhigh"];
pub fn normalize_reasoning_effort(value: Option<&str>) -> &'static str
```

Delete the old static-array assertion test. All default and support decisions must come from `ModelCatalog` or `ReasoningProfile`.

- [ ] **Step 8: 运行 UI、应用和全量测试**

Run:

```sh
cargo test ui::provider_editor_view::tests -- --nocapture
cargo test provider_config::codex_apply::tests -- --nocapture
cargo test ui::tests -- --nocapture
cargo test
```

Expected: all commands PASS with zero failed tests.

- [ ] **Step 9: 提交 UI 和最终写入集成**

```sh
git add src/ui/provider_editor_view.rs src/ui/details.rs src/ui/mod.rs src/provider_config/codex_apply.rs src/provider_config/registry.rs src/provider_config/mod.rs src/app/providers.rs
git commit -m "feat: 展示并写入模型支持的推理等级" -m "统一编辑器、详情页和 Codex 配置写入的模型能力判断。\n\n- 动态展示 GPT-5.6 的 max 和 ultra\n- 空模型按现有 Codex 模型校验\n- 移除固定四档全局 normalizer"
```

---

### Task 6: 更新 README 并执行完整 CI 验证

**Files:**
- Modify: `README.md:207-246`

**Interfaces:**
- Consumes: Tasks 1-5 completed behavior
- Produces: documented configuration contract and final verification evidence

- [ ] **Step 1: 更新 README 配置说明和示例**

Replace the fixed-value descriptions with:

```markdown
| `model_reasoning_effort` | 模型推理强度；候选值和默认值由当前安装的 Codex bundled model catalog 决定 |
| `plan_mode_reasoning_effort` | Plan mode 推理强度；使用所选模型的支持列表 |

当前 Codex 中，GPT-5.6 Sol 和 Terra 支持 `low`、`medium`、`high`、
`xhigh`、`max`、`ultra`；Luna 支持到 `max`。`gpt-5.6` 按 Sol 处理。
未知模型或模型目录不可用时，codex-board 回退到 `low`、`medium`、
`high`、`xhigh`，默认 `medium`，不会阻止 provider 编辑或应用。
```

Update the nearby TOML example to:

```toml
model = "gpt-5.6-sol"
model_reasoning_effort = "max"
plan_mode_reasoning_effort = "high"
```

- [ ] **Step 2: 运行完整测试**

Run: `cargo test`

Expected: exit 0, zero failed tests.

- [ ] **Step 3: 运行格式检查**

Run: `cargo fmt --all -- --check`

Expected: exit 0 with no formatting diff.

- [ ] **Step 4: 运行完整 Clippy**

Run:

```sh
cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
```

Expected: exit 0 with no warnings or errors.

- [ ] **Step 5: 检查差异并提交 README**

Run:

```sh
git diff --check
git status --short
git diff --stat HEAD
```

Expected: whitespace check exits 0; only intended README or formatter-required in-scope changes remain.

Commit:

```sh
git add README.md
git commit -m "docs: 说明模型感知推理等级" -m "更新 provider 配置文档，说明 GPT-5.6 支持范围和兼容回退。\n\n- 列出 Sol、Terra、Luna 的推理等级\n- 说明 gpt-5.6 别名行为\n- 记录目录不可用时的四档回退"
```

- [ ] **Step 6: 验证提交和干净工作树**

Run:

```sh
git log -6 --oneline --decorate
git status --short
```

Expected: six implementation commits are visible after the design commits, and `git status --short` prints nothing.
