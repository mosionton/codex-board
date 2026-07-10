use std::env;

use crate::provider_config::ProviderConfig;

pub fn provider_api_key_display(provider: &ProviderConfig) -> String {
    if provider.auth_mode.requires_openai_auth() {
        return "-".to_string();
    }

    let masked = mask_secret(provider.api_key.as_deref());
    if masked != "missing" {
        return masked;
    }

    provider
        .env_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| {
            env::var(key)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map_or_else(|| format!("env:{key}"), |value| mask_secret(Some(&value)))
        })
        .unwrap_or(masked)
}

#[must_use]
pub const fn provider_auth_mode_display(provider: &ProviderConfig) -> &'static str {
    provider.auth_mode.as_str()
}

fn mask_secret(secret: Option<&str>) -> String {
    let Some(secret) = secret.map(str::trim).filter(|secret| !secret.is_empty()) else {
        return "missing".to_string();
    };

    let chars = secret.chars().collect::<Vec<_>>();
    let len = chars.len();
    if len <= 4 {
        return "****".to_string();
    }

    let (prefix_len, suffix_len) = match len {
        5..=8 => (1, 1),
        9..=12 => (2, 2),
        _ => (4, 4),
    };

    let prefix = chars[..prefix_len].iter().collect::<String>();
    let suffix = chars[len - suffix_len..].iter().collect::<String>();
    format!("{prefix}******{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_config::{DEFAULT_AUTO_COMPACT_PERCENT, ProviderAuthMode};

    #[test]
    fn masks_secret_for_display() {
        assert_eq!(mask_secret(None), "missing");
        assert_eq!(mask_secret(Some("")), "missing");
        assert_eq!(mask_secret(Some("abcd")), "****");
        assert_eq!(mask_secret(Some("abcde")), "a******e");
        assert_eq!(mask_secret(Some("abcdefghij")), "ab******ij");
        assert_eq!(mask_secret(Some("sk-1234567890abcdef")), "sk-1******cdef");
    }

    #[test]
    fn shows_provider_auth_mode_for_openai_auth_provider() {
        let provider = ProviderConfig {
            model: None,
            reasoning_effort: None,
            plan_reasoning_effort: None,
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: Some("sk-1234567890abcdef".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::OpenAi,
        };

        assert_eq!(provider_api_key_display(&provider), "-");
        assert_eq!(provider_auth_mode_display(&provider), "openai");
    }

    #[test]
    fn shows_provider_auth_mode_for_api_key_provider() {
        let provider = ProviderConfig {
            model: None,
            reasoning_effort: None,
            plan_reasoning_effort: None,
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: None,
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        assert_eq!(provider_auth_mode_display(&provider), "api_key");
    }

    #[test]
    fn shows_env_key_as_api_key_source() {
        let provider = ProviderConfig {
            model: None,
            reasoning_effort: None,
            plan_reasoning_effort: None,
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: None,
            env_key: Some("CODEX_SWITCHER_TEST_MISSING_API_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        assert_eq!(
            provider_api_key_display(&provider),
            "env:CODEX_SWITCHER_TEST_MISSING_API_KEY"
        );
    }

    #[test]
    fn shows_masked_api_key_from_env_key() {
        unsafe {
            env::set_var("CODEX_SWITCHER_TEST_DISPLAY_API_KEY", "sk-1234567890abcdef");
        }

        let provider = ProviderConfig {
            model: None,
            reasoning_effort: None,
            plan_reasoning_effort: None,
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: None,
            env_key: Some("CODEX_SWITCHER_TEST_DISPLAY_API_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        assert_eq!(provider_api_key_display(&provider), "sk-1******cdef");
    }
}
