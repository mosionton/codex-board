use std::{env, fs, path::Path};

use anyhow::{Context, Result, bail};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CodexAuth {
    pub(super) has_openai_auth: bool,
}

pub(super) fn load_codex_auth(path: &Path) -> Result<CodexAuth> {
    if !path.exists() {
        return Ok(CodexAuth::default());
    }

    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read Codex auth {}", path.display()))?;
    let auth = serde_json::from_str::<JsonValue>(&text)
        .with_context(|| format!("failed to parse Codex auth {}", path.display()))?;
    let Some(auth) = auth.as_object() else {
        bail!("Codex auth {} is not a JSON object", path.display());
    };

    let auth_mode = auth.get("auth_mode").and_then(JsonValue::as_str);
    let openai_api_key = auth
        .get("OPENAI_API_KEY")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let has_openai_auth = matches!(auth_mode, Some("chatgpt" | "api_key")) || openai_api_key;

    Ok(CodexAuth { has_openai_auth })
}

pub(super) fn normalize_env_key(env_key: Option<&str>) -> Option<String> {
    env_key
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
}

pub(super) fn load_env_key_value(env_key: &str) -> Option<String> {
    env::var(env_key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
