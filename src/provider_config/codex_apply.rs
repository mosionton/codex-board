use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use toml_edit::{DocumentMut, Item, Table, value};

use super::{
    ProviderConfig,
    auth::{load_env_key_value, normalize_env_key},
    file_io::write_file_atomic,
    normalize_reasoning_effort, validate_provider_definition,
};

const OPENAI_PROVIDER_ID: &str = "openai";
#[cfg(test)]
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
#[cfg(test)]
const RESPONSES_WIRE_API: &str = "responses";

/// Applies a provider to the Codex configuration file.
///
/// # Errors
///
/// Returns an error if the provider is invalid, required API-key credentials
/// are missing, or the Codex config cannot be read, parsed, or written.
pub fn apply_provider_to_codex(
    id: &str,
    provider: &ProviderConfig,
    config_path: &Path,
) -> Result<()> {
    validate_provider_definition(id, provider)?;
    if !provider.auth_mode.requires_openai_auth() {
        let env_key = normalize_env_key(provider.env_key.as_deref());
        if provider
            .api_key
            .as_deref()
            .is_none_or(|key| key.trim().is_empty())
        {
            if let Some(env_key) = env_key.as_deref() {
                load_env_key_value(env_key)
                    .ok_or_else(|| anyhow!("env_key '{env_key}' is not set or empty"))?;
            } else {
                return Err(anyhow!(
                    "api_key or env_key is required to apply provider '{id}'"
                ));
            }
        }
    }

    write_codex_config(id, provider, config_path)
}

fn write_codex_config(id: &str, provider: &ProviderConfig, path: &Path) -> Result<()> {
    let text = if path.exists() {
        fs::read_to_string(path)
            .with_context(|| format!("failed to read Codex config {}", path.display()))?
    } else {
        String::new()
    };
    let mut doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse Codex config {}", path.display()))?;

    doc["model_provider"] = value(id);
    if let Some(model) = provider
        .model
        .as_deref()
        .filter(|model| !model.trim().is_empty())
    {
        doc["model"] = value(model);
    }
    doc["model_reasoning_effort"] = value(normalize_reasoning_effort(
        provider.reasoning_effort.as_deref(),
    ));
    doc["plan_mode_reasoning_effort"] = value(normalize_reasoning_effort(
        provider.plan_reasoning_effort.as_deref(),
    ));

    if id == OPENAI_PROVIDER_ID {
        remove_model_providers_table(&mut doc)?;
        write_config_file(path, &doc)?;
        return Ok(());
    }

    write_custom_provider_table(&mut doc, id, provider)?;
    write_config_file(path, &doc)
}

fn write_custom_provider_table(
    doc: &mut DocumentMut,
    id: &str,
    provider: &ProviderConfig,
) -> Result<()> {
    replace_model_providers_table(doc)?;
    if !doc["model_providers"].is_table_like() {
        bail!("Codex config key `model_providers` is not a table");
    }
    if doc["model_providers"].get(id).is_none() {
        doc["model_providers"][id] = Item::Table(Table::new());
    }
    if !doc["model_providers"][id].is_table_like() {
        bail!("Codex config key `model_providers.{id}` is not a table");
    }

    let provider_table = doc["model_providers"][id]
        .as_table_like_mut()
        .ok_or_else(|| anyhow!("Codex config key `model_providers.{id}` is not a table"))?;
    provider_table.insert("name", value(id));
    provider_table.insert("base_url", value(provider.base_url.as_str()));
    provider_table.insert("wire_api", value(provider.wire_api.as_str()));
    provider_table.insert(
        "requires_openai_auth",
        value(provider.auth_mode.requires_openai_auth()),
    );
    provider_table.remove("env_key_instructions");
    provider_table.remove("auth");

    if provider.auth_mode.requires_openai_auth() {
        provider_table.remove("env_key");
        provider_table.remove("experimental_bearer_token");
    } else if let Some(api_key) = provider
        .api_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
    {
        provider_table.remove("env_key");
        provider_table.insert("experimental_bearer_token", value(api_key));
    } else if let Some(env_key) = normalize_env_key(provider.env_key.as_deref()) {
        provider_table.remove("experimental_bearer_token");
        provider_table.insert("env_key", value(env_key));
    }

    Ok(())
}

fn replace_model_providers_table(doc: &mut DocumentMut) -> Result<()> {
    if let Some(model_providers) = doc.as_table().get("model_providers")
        && !model_providers.is_table_like()
    {
        bail!("Codex config key `model_providers` is not a table");
    }
    doc["model_providers"] = Item::Table(Table::new());
    Ok(())
}

fn remove_model_providers_table(doc: &mut DocumentMut) -> Result<()> {
    let Some(model_providers) = doc.as_table().get("model_providers") else {
        return Ok(());
    };
    if !model_providers.is_table_like() {
        bail!("Codex config key `model_providers` is not a table");
    }
    doc.as_table_mut().remove("model_providers");
    Ok(())
}

fn write_config_file(path: &Path, doc: &DocumentMut) -> Result<()> {
    let text = doc.to_string();
    write_file_atomic(path, text.as_bytes())
        .with_context(|| format!("failed to write Codex config {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::*;
    use crate::provider_config::ProviderAuthMode;
    use tempfile::tempdir;

    #[test]
    fn writes_config_and_auth() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "service_tier = \"default\"\n").unwrap();

        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: Some("high".to_string()),
            plan_reasoning_effort: Some("medium".to_string()),
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        apply_provider_to_codex("switcher", &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        assert_eq!(
            doc.get("service_tier").and_then(toml::Value::as_str),
            Some("default")
        );
        assert_eq!(
            doc.get("model_provider").and_then(toml::Value::as_str),
            Some("switcher")
        );
        assert_eq!(
            doc.get("model").and_then(toml::Value::as_str),
            Some("gpt-5.5")
        );
        assert_eq!(
            doc.get("model_reasoning_effort")
                .and_then(toml::Value::as_str),
            Some("high")
        );
        assert_eq!(
            doc.get("plan_mode_reasoning_effort")
                .and_then(toml::Value::as_str),
            Some("medium")
        );
        let provider_doc = doc
            .get("model_providers")
            .and_then(|item| item.get("switcher"))
            .unwrap();
        assert_eq!(
            provider_doc.get("name").and_then(toml::Value::as_str),
            Some("switcher")
        );
        assert_eq!(
            provider_doc.get("base_url").and_then(toml::Value::as_str),
            Some("https://api.example.test/v1")
        );
        assert_eq!(
            provider_doc.get("wire_api").and_then(toml::Value::as_str),
            Some("responses")
        );
        assert_eq!(
            provider_doc
                .get("requires_openai_auth")
                .and_then(toml::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            provider_doc
                .get("experimental_bearer_token")
                .and_then(toml::Value::as_str),
            Some("sk-test")
        );
    }

    #[test]
    fn keeps_only_applied_custom_provider() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
service_tier = "default"
model_provider = "old"

[model_providers.old]
name = "old"
base_url = "https://old.example.test/v1"
wire_api = "chat"

[model_providers.other]
name = "other"
base_url = "https://other.example.test/v1"
wire_api = "responses"
"#,
        )
        .unwrap();
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: Some("high".to_string()),
            plan_reasoning_effort: Some("medium".to_string()),
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        apply_provider_to_codex("switcher", &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        let providers = doc
            .get("model_providers")
            .and_then(toml::Value::as_table)
            .unwrap();

        assert_eq!(providers.len(), 1);
        assert!(providers.contains_key("switcher"));
        assert!(!providers.contains_key("old"));
        assert!(!providers.contains_key("other"));
        assert_eq!(
            doc.get("service_tier").and_then(toml::Value::as_str),
            Some("default")
        );
    }

    #[test]
    fn defaults_reasoning_to_medium() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
model_reasoning_effort = "high"
plan_mode_reasoning_effort = "xhigh"
"#,
        )
        .unwrap();

        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: Some("none".to_string()),
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        apply_provider_to_codex("switcher", &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        assert_eq!(
            doc.get("model_reasoning_effort")
                .and_then(toml::Value::as_str),
            Some("medium")
        );
        assert_eq!(
            doc.get("plan_mode_reasoning_effort")
                .and_then(toml::Value::as_str),
            Some("medium")
        );
    }

    #[test]
    fn requires_api_key() {
        let dir = tempdir().unwrap();
        let provider = ProviderConfig::new("https://api.example.test/v1", "responses");
        let err = apply_provider_to_codex("switcher", &provider, &dir.path().join("config.toml"))
            .unwrap_err();

        assert!(err.to_string().contains("api_key or env_key is required"));
    }

    #[test]
    fn rejects_openai_id_for_api_key_provider() {
        let dir = tempdir().unwrap();
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        let err = apply_provider_to_codex(
            OPENAI_PROVIDER_ID,
            &provider,
            &dir.path().join("config.toml"),
        )
        .unwrap_err();

        assert!(err.to_string().contains("reserved for OpenAI auth"));
        assert!(!dir.path().join("config.toml").exists());
    }

    #[test]
    fn builtin_openai_provider_does_not_write_custom_provider_table() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
model_provider = "switcher"

[model_providers.openai]
name = "stale"
base_url = "https://stale.example.test/v1"
wire_api = "chat"

[model_providers.switcher]
name = "switcher"
base_url = "https://api.example.test/v1"
wire_api = "responses"
"#,
        )
        .unwrap();
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: Some("high".to_string()),
            plan_reasoning_effort: Some("medium".to_string()),
            api_key: None,
            env_key: None,
            base_url: OPENAI_BASE_URL.to_string(),
            wire_api: RESPONSES_WIRE_API.to_string(),
            auth_mode: ProviderAuthMode::OpenAi,
        };

        apply_provider_to_codex(OPENAI_PROVIDER_ID, &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        assert_eq!(
            doc.get("model_provider").and_then(toml::Value::as_str),
            Some(OPENAI_PROVIDER_ID)
        );
        assert_eq!(
            doc.get("model").and_then(toml::Value::as_str),
            Some("gpt-5.5")
        );
        assert!(doc.get("model_providers").is_none());
    }

    #[test]
    fn allows_openai_auth_without_api_key() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: None,
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::OpenAi,
        };

        apply_provider_to_codex("switcher", &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        let provider_doc = doc
            .get("model_providers")
            .and_then(|item| item.get("switcher"))
            .unwrap();

        assert_eq!(
            provider_doc
                .get("requires_openai_auth")
                .and_then(toml::Value::as_bool),
            Some(true)
        );
        assert!(provider_doc.get("experimental_bearer_token").is_none());
        assert!(provider_doc.get("env_key").is_none());
    }

    #[test]
    fn does_not_write_token_with_openai_auth() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: Some("sk-login-auth".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::OpenAi,
        };

        apply_provider_to_codex("switcher", &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        let provider_doc = doc
            .get("model_providers")
            .and_then(|item| item.get("switcher"))
            .unwrap();

        assert_eq!(
            provider_doc
                .get("requires_openai_auth")
                .and_then(toml::Value::as_bool),
            Some(true)
        );
        assert!(provider_doc.get("experimental_bearer_token").is_none());
        assert!(provider_doc.get("env_key").is_none());
    }

    #[test]
    fn prefers_api_key_over_env_key() {
        unsafe {
            env::set_var("CODEX_SWITCHER_TEST_PROVIDER_API_KEY", "sk-env-provider");
        }

        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: Some("sk-env-provider".to_string()),
            env_key: Some("CODEX_SWITCHER_TEST_PROVIDER_API_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        apply_provider_to_codex("switcher", &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        let provider_doc = doc
            .get("model_providers")
            .and_then(|item| item.get("switcher"))
            .unwrap();

        assert_eq!(
            provider_doc
                .get("experimental_bearer_token")
                .and_then(toml::Value::as_str),
            Some("sk-env-provider")
        );
        assert!(provider_doc.get("env_key").is_none());
    }

    #[test]
    fn writes_env_key_when_api_key_is_missing() {
        unsafe {
            env::set_var("CODEX_SWITCHER_TEST_PROVIDER_ENV_ONLY", "sk-env-only");
        }

        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: None,
            env_key: Some("CODEX_SWITCHER_TEST_PROVIDER_ENV_ONLY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        apply_provider_to_codex("switcher", &provider, &config_path).unwrap();

        let config = fs::read_to_string(&config_path).unwrap();
        let doc = toml::from_str::<toml::Value>(&config).unwrap();
        let provider_doc = doc
            .get("model_providers")
            .and_then(|item| item.get("switcher"))
            .unwrap();

        assert!(provider_doc.get("experimental_bearer_token").is_none());
        assert_eq!(
            provider_doc.get("env_key").and_then(toml::Value::as_str),
            Some("CODEX_SWITCHER_TEST_PROVIDER_ENV_ONLY")
        );
    }

    #[test]
    fn requires_env_key_to_be_set() {
        let dir = tempdir().unwrap();
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: None,
            env_key: Some("CODEX_SWITCHER_TEST_MISSING_PROVIDER_API_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        let err = apply_provider_to_codex("switcher", &provider, &dir.path().join("config.toml"))
            .unwrap_err();

        assert!(err.to_string().contains("env_key"));
        assert!(err.to_string().contains("not set or empty"));
    }
}
