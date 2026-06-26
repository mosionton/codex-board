use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

/// Fetches model ids from a provider's `/models` endpoint.
///
/// # Errors
///
/// Returns an error if inputs are empty, the HTTP request fails, the provider
/// returns a non-success status, or the response cannot be parsed.
pub fn fetch_provider_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
    let base_url = base_url.trim();
    if base_url.is_empty() {
        bail!("base_url is required to fetch models");
    }
    let api_key = api_key.trim();
    if api_key.is_empty() {
        bail!("api_key is required to fetch models");
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .context("failed to initialize HTTP client")?;
    let response = client
        .get(models_url(base_url))
        .bearer_auth(api_key)
        .send()
        .context("failed to request provider models")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read provider models response")?;

    if !status.is_success() {
        bail!(
            "model list request failed with status {status}: {}",
            truncate_error_body(&body)
        );
    }

    parse_model_ids(&body)
}

fn models_url(base_url: &str) -> String {
    format!("{}/models", base_url.trim_end_matches('/'))
}

fn parse_model_ids(body: &str) -> Result<Vec<String>> {
    let response =
        serde_json::from_str::<ModelsResponse>(body).context("failed to parse models response")?;
    let mut models = response
        .data
        .into_iter()
        .map(|model| model.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    if models.is_empty() {
        bail!("provider returned no models");
    }
    Ok(models)
}

fn truncate_error_body(body: &str) -> String {
    let text = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= 160 {
        return text;
    }
    let mut truncated = text.chars().take(159).collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_models_url_from_base_url() {
        assert_eq!(
            models_url("https://api.example.test/v1/"),
            "https://api.example.test/v1/models"
        );
    }

    #[test]
    fn parses_model_ids_sorted_and_deduplicated() {
        let models = parse_model_ids(
            r#"{"object":"list","data":[{"id":"z-model"},{"id":"a-model"},{"id":"a-model"},{"id":"  "}]} "#,
        )
        .unwrap();

        assert_eq!(models, vec!["a-model".to_string(), "z-model".to_string()]);
    }

    #[test]
    fn parse_model_ids_rejects_invalid_json() {
        let error = parse_model_ids(r#"{"data":"not-a-list"}"#).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to parse models response")
        );
    }

    #[test]
    fn parse_model_ids_rejects_empty_model_list() {
        let error = parse_model_ids(r#"{"data":[{"id":"  "}]}"#).unwrap_err();

        assert!(error.to_string().contains("provider returned no models"));
    }

    #[test]
    fn truncate_error_body_normalizes_short_whitespace() {
        assert_eq!(
            truncate_error_body("  provider \n returned\tbad request  "),
            "provider returned bad request"
        );
    }

    #[test]
    fn truncate_error_body_truncates_long_text() {
        let text = "0123456789 ".repeat(20);
        let truncated = truncate_error_body(&text);

        assert_eq!(truncated.chars().count(), 160);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn fetch_provider_models_requires_non_empty_inputs() {
        let error = fetch_provider_models("   ", "sk-test").unwrap_err();
        assert!(error.to_string().contains("base_url is required"));

        let error = fetch_provider_models("https://api.example.test/v1", " ").unwrap_err();
        assert!(error.to_string().contains("api_key is required"));
    }
}
