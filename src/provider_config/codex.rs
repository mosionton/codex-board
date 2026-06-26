use std::path::{Path, PathBuf};

pub use super::codex_import::{load_applied_model_provider, load_codex_config_providers};

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
    use std::{env, fs};

    use super::*;
    use crate::provider_config::ProviderAuthMode;
    use tempfile::tempdir;

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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
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

        let registry = load_codex_config_providers(&config_path, &auth_path).unwrap();
        let provider = registry.providers.get("switcher").unwrap();

        assert_eq!(provider.api_key, None);
        assert_eq!(provider.env_key, None);
        assert_eq!(provider.auth_mode, ProviderAuthMode::OpenAi);
    }
}
