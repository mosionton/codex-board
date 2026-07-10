use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{
    ModelCatalog, ProviderAuthMode, ProviderConfig, ProviderRegistry,
    auth::{CodexAuth, load_codex_auth, load_env_key_value, normalize_env_key},
};

const OPENAI_PROVIDER_ID: &str = "openai";
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const RESPONSES_WIRE_API: &str = "responses";

#[derive(Debug, Deserialize)]
struct CodexConfig {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    model_provider: Option<String>,
    #[serde(default)]
    model_reasoning_effort: Option<String>,
    #[serde(default)]
    plan_mode_reasoning_effort: Option<String>,
    #[serde(default)]
    model_providers: BTreeMap<String, CodexProviderConfig>,
}

#[derive(Debug, Deserialize)]
struct CodexProviderConfig {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    wire_api: Option<String>,
    #[serde(default)]
    experimental_bearer_token: Option<String>,
    #[serde(default)]
    env_key: Option<String>,
    #[serde(default)]
    requires_openai_auth: Option<bool>,
}

/// Loads the provider id applied in Codex config.
///
/// # Errors
///
/// Returns an error if the Codex config cannot be read or parsed.
pub fn load_applied_model_provider(config_path: &Path) -> Result<Option<String>> {
    let Some(config) = load_codex_config(config_path)? else {
        return Ok(None);
    };

    Ok(config
        .model_provider
        .map(|provider| provider.trim().to_string())
        .filter(|provider| !provider.is_empty()))
}

/// Loads provider definitions from Codex config and auth files.
///
/// # Errors
///
/// Returns an error if the Codex config or auth files cannot be read or parsed,
/// or if imported provider definitions are invalid.
pub fn load_codex_config_providers(
    config_path: &Path,
    auth_path: &Path,
    model_catalog: &ModelCatalog,
) -> Result<ProviderRegistry> {
    let codex_auth = load_codex_auth(auth_path)?;
    let codex_config = load_codex_config(config_path)?;

    let mut registry = ProviderRegistry::default();
    if let Some(codex_config) = codex_config {
        let model = codex_config.model.clone();
        let reasoning_effort = model_catalog.normalize_effort(
            model.as_deref(),
            codex_config.model_reasoning_effort.as_deref(),
        );
        let plan_reasoning_effort = model_catalog.normalize_effort(
            model.as_deref(),
            codex_config.plan_mode_reasoning_effort.as_deref(),
        );
        for (id, provider) in codex_config.model_providers {
            if provider.base_url.is_none() || provider.wire_api.is_none() {
                continue;
            }
            registry.upsert(
                id,
                imported_provider_config(
                    model.clone(),
                    reasoning_effort.clone(),
                    plan_reasoning_effort.clone(),
                    provider,
                ),
            )?;
        }
        add_openai_provider_for_openai_auth(
            &mut registry,
            &codex_auth,
            model,
            reasoning_effort,
            plan_reasoning_effort,
        )?;
    } else {
        add_openai_provider_for_openai_auth(
            &mut registry,
            &codex_auth,
            None,
            model_catalog.normalize_effort(None, None),
            model_catalog.normalize_effort(None, None),
        )?;
    }

    Ok(registry)
}

fn load_codex_config(config_path: &Path) -> Result<Option<CodexConfig>> {
    if !config_path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read Codex config {}", config_path.display()))?;
    toml::from_str::<CodexConfig>(&text)
        .map(Some)
        .with_context(|| format!("failed to parse Codex config {}", config_path.display()))
}

fn imported_provider_config(
    model: Option<String>,
    reasoning_effort: String,
    plan_reasoning_effort: String,
    provider: CodexProviderConfig,
) -> ProviderConfig {
    let requires_openai_auth = provider.requires_openai_auth.unwrap_or(false);
    let env_key = if requires_openai_auth {
        None
    } else {
        normalize_env_key(provider.env_key.as_deref())
    };
    let api_key = if requires_openai_auth {
        None
    } else {
        provider
            .experimental_bearer_token
            .clone()
            .or_else(|| env_key.as_deref().and_then(load_env_key_value))
    };

    ProviderConfig {
        model,
        reasoning_effort: Some(reasoning_effort),
        plan_reasoning_effort: Some(plan_reasoning_effort),
        api_key,
        env_key,
        base_url: provider.base_url.unwrap_or_default(),
        wire_api: provider.wire_api.unwrap_or_default(),
        auth_mode: if requires_openai_auth {
            ProviderAuthMode::OpenAi
        } else {
            ProviderAuthMode::ApiKey
        },
    }
}

fn add_openai_provider_for_openai_auth(
    registry: &mut ProviderRegistry,
    auth: &CodexAuth,
    model: Option<String>,
    reasoning_effort: String,
    plan_reasoning_effort: String,
) -> Result<()> {
    if !auth.has_openai_auth {
        return Ok(());
    }
    if registry.providers.contains_key(OPENAI_PROVIDER_ID) {
        return Ok(());
    }

    registry.upsert(
        OPENAI_PROVIDER_ID,
        ProviderConfig {
            model,
            reasoning_effort: Some(reasoning_effort),
            plan_reasoning_effort: Some(plan_reasoning_effort),
            api_key: None,
            env_key: None,
            base_url: OPENAI_BASE_URL.to_string(),
            wire_api: RESPONSES_WIRE_API.to_string(),
            auth_mode: ProviderAuthMode::OpenAi,
        },
    )
}
