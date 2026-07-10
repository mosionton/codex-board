use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::file_io::write_file_atomic;

pub const CONFIG_FILE_NAME: &str = "switcher-providers.toml";
pub const OPENAI_PROVIDER_ID: &str = "openai";
pub const DEFAULT_AUTO_COMPACT_PERCENT: u8 = 70;
pub const MIN_AUTO_COMPACT_PERCENT: u8 = 1;
pub const MAX_AUTO_COMPACT_PERCENT: u8 = 99;

const fn default_auto_compact_percent() -> u8 {
    DEFAULT_AUTO_COMPACT_PERCENT
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRegistry {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(
        default,
        rename = "model_reasoning_effort",
        alias = "reasoning_effort",
        skip_serializing_if = "Option::is_none"
    )]
    pub reasoning_effort: Option<String>,
    #[serde(
        default,
        rename = "plan_mode_reasoning_effort",
        alias = "plan_reasoning_effort",
        skip_serializing_if = "Option::is_none"
    )]
    pub plan_reasoning_effort: Option<String>,
    #[serde(default = "default_auto_compact_percent")]
    pub auto_compact_percent: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    pub base_url: String,
    pub wire_api: String,
    #[serde(default)]
    pub auth_mode: ProviderAuthMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthMode {
    #[default]
    ApiKey,
    #[serde(rename = "openai", alias = "chatgpt", alias = "chat_gpt")]
    OpenAi,
}

impl ProviderAuthMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::OpenAi => "openai",
        }
    }

    #[must_use]
    pub const fn requires_openai_auth(self) -> bool {
        matches!(self, Self::OpenAi)
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::ApiKey => Self::OpenAi,
            Self::OpenAi => Self::ApiKey,
        }
    }
}

impl ProviderRegistry {
    /// Loads provider definitions from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, parsed, or validated.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read provider config {}", path.display()))?;
        let registry = toml::from_str::<Self>(&text)
            .with_context(|| format!("failed to parse provider config {}", path.display()))?;
        registry.validate()?;
        Ok(registry)
    }

    /// Saves provider definitions to a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if validation, serialization, directory creation, or writing fails.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;

        let text = toml::to_string_pretty(self).context("failed to serialize provider config")?;
        write_file_atomic(path, text.as_bytes())
            .with_context(|| format!("failed to write provider config {}", path.display()))?;
        Ok(())
    }

    /// Inserts or replaces a provider definition.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider id or config is invalid.
    pub fn upsert(&mut self, id: impl Into<String>, config: ProviderConfig) -> Result<()> {
        let id = id.into();
        validate_provider_definition(&id, &config)?;
        self.providers.insert(id, config);
        Ok(())
    }

    #[must_use]
    pub fn remove(&mut self, id: &str) -> Option<ProviderConfig> {
        self.providers.remove(id)
    }

    /// Validates all provider definitions.
    ///
    /// # Errors
    ///
    /// Returns an error if any provider id or provider config is invalid.
    pub fn validate(&self) -> Result<()> {
        for (id, provider) in &self.providers {
            validate_provider_definition(id, provider)
                .with_context(|| format!("invalid provider '{id}'"))?;
        }
        Ok(())
    }

    pub fn merge_missing(&mut self, other: Self) {
        for (id, provider) in other.providers {
            self.providers.entry(id).or_insert(provider);
        }
    }

    pub fn merge_defaults(&mut self, other: Self) {
        for (id, provider) in other.providers {
            if let Some(current) = self.providers.get_mut(&id) {
                let missing_api_key = current
                    .api_key
                    .as_deref()
                    .is_none_or(|api_key| api_key.trim().is_empty());
                if missing_api_key {
                    current.api_key.clone_from(&provider.api_key);
                }
                let missing_env_key = current
                    .env_key
                    .as_deref()
                    .is_none_or(|env_key| env_key.trim().is_empty());
                if current.auth_mode == ProviderAuthMode::ApiKey
                    && provider.auth_mode.requires_openai_auth()
                    && missing_api_key
                    && missing_env_key
                {
                    current.auth_mode = provider.auth_mode;
                }
                if missing_env_key {
                    current.env_key.clone_from(&provider.env_key);
                }
            } else {
                self.providers.insert(id, provider);
            }
        }
    }
}

impl ProviderConfig {
    #[must_use]
    pub fn new(base_url: impl Into<String>, wire_api: impl Into<String>) -> Self {
        Self {
            model: None,
            reasoning_effort: None,
            plan_reasoning_effort: None,
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: None,
            env_key: None,
            base_url: base_url.into(),
            wire_api: wire_api.into(),
            auth_mode: ProviderAuthMode::ApiKey,
        }
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Validates this provider definition.
    ///
    /// # Errors
    ///
    /// Returns an error if the auto-compact percentage is outside the supported range or required
    /// fields are empty.
    pub fn validate(&self) -> Result<()> {
        if !(MIN_AUTO_COMPACT_PERCENT..=MAX_AUTO_COMPACT_PERCENT)
            .contains(&self.auto_compact_percent)
        {
            bail!(
                "auto_compact_percent must be between {MIN_AUTO_COMPACT_PERCENT} and {MAX_AUTO_COMPACT_PERCENT}"
            );
        }
        if self.base_url.trim().is_empty() {
            bail!("base_url is required");
        }
        if self.wire_api.trim().is_empty() {
            bail!("wire_api is required");
        }
        Ok(())
    }
}

#[must_use]
pub fn config_path(codex_home: impl AsRef<Path>) -> PathBuf {
    codex_home.as_ref().join(CONFIG_FILE_NAME)
}

pub(super) fn validate_provider_id(id: &str) -> Result<()> {
    if id.trim().is_empty() {
        bail!("provider id is required");
    }
    if id.contains('.') || id.contains('[') || id.contains(']') {
        return Err(anyhow!("provider id '{id}' cannot contain '.', '[' or ']'"));
    }
    Ok(())
}

pub(super) fn validate_provider_definition(id: &str, provider: &ProviderConfig) -> Result<()> {
    validate_provider_id(id)?;
    if id == OPENAI_PROVIDER_ID && provider.auth_mode == ProviderAuthMode::ApiKey {
        bail!(
            "provider id '{OPENAI_PROVIDER_ID}' is reserved for OpenAI auth; api_key providers must use a different id"
        );
    }
    provider.validate()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_provider_registry() {
        let dir = tempdir().unwrap();
        let path = config_path(dir.path());
        let mut registry = ProviderRegistry::default();
        let mut provider =
            ProviderConfig::new("https://example.test/v1", "responses").with_model("gpt-5.5");
        provider.reasoning_effort = Some("high".to_string());
        provider.plan_reasoning_effort = Some("low".to_string());
        provider.api_key = Some("sk-test".to_string());
        provider.env_key = None;
        provider.auth_mode = ProviderAuthMode::OpenAi;
        registry.upsert("switcher", provider).unwrap();

        registry.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#"auth_mode = "openai""#));
        let loaded = ProviderRegistry::load(&path).unwrap();

        assert_eq!(loaded.providers.len(), 1);
        let provider = loaded.providers.get("switcher").unwrap();
        assert_eq!(provider.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(provider.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(provider.plan_reasoning_effort.as_deref(), Some("low"));
        assert_eq!(provider.api_key.as_deref(), Some("sk-test"));
        assert_eq!(provider.env_key.as_deref(), None);
        assert_eq!(provider.base_url, "https://example.test/v1");
        assert_eq!(provider.wire_api, "responses");
        assert_eq!(provider.auth_mode, ProviderAuthMode::OpenAi);
    }

    #[test]
    fn missing_provider_registry_loads_as_empty() {
        let dir = tempdir().unwrap();
        let loaded = ProviderRegistry::load(&config_path(dir.path())).unwrap();
        assert!(loaded.providers.is_empty());
        assert!(config_path(dir.path()).ends_with("switcher-providers.toml"));
    }

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

            registry
                .upsert(format!("switcher-{percent}"), provider)
                .unwrap();
        }
    }

    #[test]
    fn rejects_invalid_provider_config() {
        let mut registry = ProviderRegistry::default();
        assert!(
            registry
                .upsert("bad.name", ProviderConfig::new("x", "responses"))
                .is_err()
        );
        assert!(
            registry
                .upsert("ok", ProviderConfig::new("", "responses"))
                .is_err()
        );
    }

    #[test]
    fn rejects_openai_id_for_api_key_provider() {
        let mut registry = ProviderRegistry::default();
        let err = registry
            .upsert(
                OPENAI_PROVIDER_ID,
                ProviderConfig::new("https://api.example.test/v1", "responses"),
            )
            .unwrap_err();

        assert!(err.to_string().contains("reserved for OpenAI auth"));
        assert!(err.to_string().contains("api_key providers"));
    }

    #[test]
    fn allows_openai_id_for_openai_auth_provider() {
        let mut registry = ProviderRegistry::default();
        let mut provider = ProviderConfig::new("https://api.openai.com/v1", "responses");
        provider.auth_mode = ProviderAuthMode::OpenAi;

        registry.upsert(OPENAI_PROVIDER_ID, provider).unwrap();

        assert!(registry.providers.contains_key(OPENAI_PROVIDER_ID));
    }

    #[test]
    fn merge_defaults_preserves_empty_model_context_and_fills_credentials() {
        let mut current = ProviderRegistry::default();
        current
            .upsert(
                "switcher",
                ProviderConfig {
                    model: None,
                    reasoning_effort: None,
                    plan_reasoning_effort: None,
                    auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
                    api_key: None,
                    env_key: None,
                    base_url: "https://local.example/v1".to_string(),
                    wire_api: "responses".to_string(),
                    auth_mode: ProviderAuthMode::ApiKey,
                },
            )
            .unwrap();

        let mut imported = ProviderRegistry::default();
        imported
            .upsert(
                "switcher",
                ProviderConfig {
                    model: Some("gpt-5.5".to_string()),
                    reasoning_effort: Some("low".to_string()),
                    plan_reasoning_effort: Some("xhigh".to_string()),
                    auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
                    api_key: Some("sk-test".to_string()),
                    env_key: Some("OPENAI_API_KEY".to_string()),
                    base_url: "https://remote.example/v1".to_string(),
                    wire_api: "chat".to_string(),
                    auth_mode: ProviderAuthMode::ApiKey,
                },
            )
            .unwrap();

        current.merge_defaults(imported);
        let provider = current.providers.get("switcher").unwrap();

        assert_eq!(provider.model, None);
        assert_eq!(provider.reasoning_effort, None);
        assert_eq!(provider.plan_reasoning_effort, None);
        assert_eq!(provider.api_key.as_deref(), Some("sk-test"));
        assert_eq!(provider.env_key.as_deref(), Some("OPENAI_API_KEY"));
        assert_eq!(provider.base_url, "https://local.example/v1");
        assert_eq!(provider.wire_api, "responses");
        assert_eq!(provider.auth_mode, ProviderAuthMode::ApiKey);
    }
}
