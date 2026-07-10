use std::path::{Path, PathBuf};

pub use super::codex_import::{
    load_applied_model_provider, load_codex_config_providers, load_current_codex_model,
};

#[must_use]
pub fn codex_config_path(codex_home: impl AsRef<Path>) -> PathBuf {
    codex_home.as_ref().join("config.toml")
}

#[must_use]
pub fn codex_auth_path(codex_home: impl AsRef<Path>) -> PathBuf {
    codex_home.as_ref().join("auth.json")
}

#[cfg(test)]
mod tests {
    use std::{env, fs, sync::Arc};

    use super::*;
    use crate::{
        app::ProviderEditor,
        provider_config::{ModelCatalog, ProviderAuthMode, ProviderRegistry},
    };
    use tempfile::tempdir;

    fn load_with_default_catalog(
        config_path: &Path,
        auth_path: &Path,
    ) -> anyhow::Result<ProviderRegistry> {
        load_codex_config_providers(config_path, auth_path, &ModelCatalog::default())
    }

    fn gpt_5_6_catalog() -> ModelCatalog {
        ModelCatalog::from_json(
            r#"{"models":[
          {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
          {"slug":"gpt-5.6-terra","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
          {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
        ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn loads_providers_from_codex_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.5"
model_reasoning_effort = "medium"
plan_mode_reasoning_effort = "low"
model_provider = "switcher"

[model_providers.switcher]
name = "switcher"
base_url = "https://api.example.test/v1"
wire_api = "responses"
experimental_bearer_token = "sk-test"
requires_openai_auth = false
"#,
        )
        .unwrap();
        fs::write(&auth_path, r#"{"OPENAI_API_KEY":"sk-global"}"#).unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("switcher").unwrap();

        assert_eq!(provider.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(provider.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(provider.plan_reasoning_effort.as_deref(), Some("low"));
        assert_eq!(provider.api_key.as_deref(), Some("sk-test"));
        assert_eq!(provider.env_key.as_deref(), None);
        assert_eq!(provider.base_url, "https://api.example.test/v1");
        assert_eq!(provider.wire_api, "responses");
        assert_eq!(provider.auth_mode, ProviderAuthMode::ApiKey);
    }

    #[test]
    fn loads_applied_model_provider_from_codex_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "model_provider = \" switcher \"\n").unwrap();

        let applied = load_applied_model_provider(&config_path).unwrap();

        assert_eq!(applied.as_deref(), Some("switcher"));
        assert_eq!(
            load_applied_model_provider(&dir.path().join("missing.toml")).unwrap(),
            None
        );
    }

    #[test]
    fn loads_current_codex_model_from_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "model = \" gpt-5.6-sol \"\n").unwrap();

        assert_eq!(
            load_current_codex_model(&config_path).unwrap().as_deref(),
            Some("gpt-5.6-sol")
        );
        assert_eq!(
            load_current_codex_model(&dir.path().join("missing.toml")).unwrap(),
            None
        );
    }

    #[test]
    fn loads_openai_auth_without_exposing_chatgpt_access_token() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.5"

[model_providers.switcher]
name = "switcher"
base_url = "https://api.example.test/v1"
wire_api = "responses"
requires_openai_auth = true
"#,
        )
        .unwrap();
        fs::write(
            &auth_path,
            r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"access_token":"chatgpt-access-token"}}"#,
        )
        .unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("switcher").unwrap();

        assert_eq!(provider.api_key, None);
        assert_eq!(provider.env_key, None);
        assert_eq!(provider.auth_mode, ProviderAuthMode::OpenAi);
    }

    #[test]
    fn adds_openai_provider_when_auth_is_chatgpt_login() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("missing-config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &auth_path,
            r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"access_token":"chatgpt-access-token"}}"#,
        )
        .unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("openai").unwrap();

        assert_eq!(registry.providers.len(), 1);
        assert_eq!(provider.api_key, None);
        assert_eq!(provider.env_key, None);
        assert_eq!(provider.base_url, "https://api.openai.com/v1");
        assert_eq!(provider.wire_api, "responses");
        assert_eq!(provider.auth_mode, ProviderAuthMode::OpenAi);
    }

    #[test]
    fn adds_openai_provider_when_auth_is_api_key_login() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("missing-config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &auth_path,
            r#"{"auth_mode":"api_key","OPENAI_API_KEY":"sk-auth-only"}"#,
        )
        .unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("openai").unwrap();

        assert_eq!(registry.providers.len(), 1);
        assert_eq!(provider.api_key, None);
        assert_eq!(provider.env_key, None);
        assert_eq!(provider.base_url, "https://api.openai.com/v1");
        assert_eq!(provider.wire_api, "responses");
        assert_eq!(provider.auth_mode, ProviderAuthMode::OpenAi);
    }

    #[test]
    fn loads_api_key_from_env_key_when_provider_token_is_missing() {
        unsafe {
            env::set_var("CODEX_SWITCHER_TEST_LOADED_API_KEY", "sk-env-loaded");
        }

        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.5"

[model_providers.switcher]
name = "switcher"
base_url = "https://api.example.test/v1"
wire_api = "responses"
env_key = "CODEX_SWITCHER_TEST_LOADED_API_KEY"
requires_openai_auth = false
"#,
        )
        .unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("switcher").unwrap();

        assert_eq!(provider.api_key.as_deref(), Some("sk-env-loaded"));
        assert_eq!(
            provider.env_key.as_deref(),
            Some("CODEX_SWITCHER_TEST_LOADED_API_KEY")
        );
        assert_eq!(provider.auth_mode, ProviderAuthMode::ApiKey);
    }

    #[test]
    fn provider_env_key_takes_precedence_over_codex_auth() {
        unsafe {
            env::set_var(
                "CODEX_SWITCHER_TEST_PROVIDER_SPECIFIC_KEY",
                "sk-provider-env",
            );
        }

        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.5"

[model_providers.switcher]
name = "switcher"
base_url = "https://api.example.test/v1"
wire_api = "responses"
env_key = "CODEX_SWITCHER_TEST_PROVIDER_SPECIFIC_KEY"
requires_openai_auth = false
"#,
        )
        .unwrap();
        fs::write(&auth_path, r#"{"OPENAI_API_KEY":"sk-global-auth"}"#).unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("switcher").unwrap();

        assert_eq!(provider.api_key.as_deref(), Some("sk-provider-env"));
        assert_eq!(
            provider.env_key.as_deref(),
            Some("CODEX_SWITCHER_TEST_PROVIDER_SPECIFIC_KEY")
        );
        assert_eq!(provider.auth_mode, ProviderAuthMode::ApiKey);
    }

    #[test]
    fn skips_codex_auth_for_providers_without_openai_auth_requirement() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.5"

[model_providers.switcher]
name = "switcher"
base_url = "https://api.example.test/v1"
wire_api = "responses"
requires_openai_auth = false
"#,
        )
        .unwrap();
        fs::write(&auth_path, r#"{"OPENAI_API_KEY":"sk-global-auth"}"#).unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("switcher").unwrap();

        assert_eq!(provider.api_key, None);
        assert_eq!(provider.auth_mode, ProviderAuthMode::ApiKey);
    }

    #[test]
    fn loads_openai_api_key_login_without_exposing_key() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.5"

[model_providers.switcher]
name = "switcher"
base_url = "https://api.example.test/v1"
wire_api = "responses"
requires_openai_auth = true
"#,
        )
        .unwrap();
        fs::write(
            &auth_path,
            r#"{"auth_mode":"api_key","OPENAI_API_KEY":"sk-auth-only","tokens":{"access_token":"ignored-access-token"}}"#,
        )
        .unwrap();

        let registry = load_with_default_catalog(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("switcher").unwrap();

        assert_eq!(provider.api_key, None);
        assert_eq!(provider.env_key, None);
        assert_eq!(provider.auth_mode, ProviderAuthMode::OpenAi);
    }

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
    fn imported_missing_efforts_follow_new_model_defaults_in_editor() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.6-sol"

[model_providers.switcher]
base_url = "https://example.test/v1"
wire_api = "responses"
"#,
        )
        .unwrap();
        let catalog = Arc::new(gpt_5_6_catalog());

        let registry =
            load_codex_config_providers(&config_path, &auth_path, catalog.as_ref()).unwrap();
        let provider = registry.providers.get("switcher").unwrap();
        assert_eq!(provider.reasoning_effort, None);
        assert_eq!(provider.plan_reasoning_effort, None);

        let mut editor = ProviderEditor::from_provider_with_catalog("switcher", provider, catalog);
        assert_eq!(editor.reasoning_effort, "low");
        assert_eq!(editor.plan_reasoning_effort, "low");
        editor.model.set("gpt-5.6-terra");
        editor.commit_model_change();

        assert_eq!(editor.reasoning_effort, "medium");
        assert_eq!(editor.plan_reasoning_effort, "medium");
    }

    #[test]
    fn imported_explicit_and_invalid_efforts_remain_independent() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let auth_path = dir.path().join("auth.json");
        fs::write(
            &config_path,
            r#"
model = "gpt-5.6-sol"
model_reasoning_effort = "low"
plan_mode_reasoning_effort = "unsupported"

[model_providers.switcher]
base_url = "https://example.test/v1"
wire_api = "responses"
"#,
        )
        .unwrap();
        let catalog = Arc::new(gpt_5_6_catalog());

        let registry =
            load_codex_config_providers(&config_path, &auth_path, catalog.as_ref()).unwrap();
        let provider = registry.providers.get("switcher").unwrap();
        assert_eq!(provider.reasoning_effort.as_deref(), Some("low"));
        assert_eq!(provider.plan_reasoning_effort, None);

        let mut editor = ProviderEditor::from_provider_with_catalog("switcher", provider, catalog);
        editor.model.set("gpt-5.6-terra");
        editor.commit_model_change();

        assert_eq!(editor.reasoning_effort, "low");
        assert_eq!(editor.plan_reasoning_effort, "medium");
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
}
