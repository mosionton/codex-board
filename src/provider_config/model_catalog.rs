use std::{collections::BTreeMap, process::Command};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const FALLBACK_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const WARNING: &str = "Codex model catalog unavailable; using compatibility reasoning options.";

#[must_use]
pub fn effective_model<'a>(
    provider_model: Option<&'a str>,
    current_codex_model: Option<&'a str>,
) -> Option<&'a str> {
    provider_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .or_else(|| {
            current_codex_model
                .map(str::trim)
                .filter(|model| !model.is_empty())
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningProfile {
    default_effort: String,
    supported_efforts: Vec<String>,
}

impl ReasoningProfile {
    #[must_use]
    pub fn default_effort(&self) -> &str {
        &self.default_effort
    }

    #[must_use]
    pub fn supported_efforts(&self) -> &[String] {
        &self.supported_efforts
    }

    #[must_use]
    pub fn supports(&self, effort: &str) -> bool {
        self.supported_efforts
            .iter()
            .any(|supported| supported == effort)
    }

    pub fn normalize(&self, value: Option<&str>) -> String {
        value
            .map(str::trim)
            .filter(|value| self.supports(value))
            .unwrap_or(&self.default_effort)
            .to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    profiles: BTreeMap<String, ReasoningProfile>,
    fallback: ReasoningProfile,
}

impl Default for ModelCatalog {
    fn default() -> Self {
        Self {
            profiles: BTreeMap::new(),
            fallback: ReasoningProfile {
                default_effort: "medium".to_string(),
                supported_efforts: FALLBACK_EFFORTS.iter().map(ToString::to_string).collect(),
            },
        }
    }
}

impl ModelCatalog {
    #[must_use]
    pub fn load_bundled() -> ModelCatalogLoad {
        match Command::new("codex")
            .args(["debug", "models", "--bundled"])
            .output()
        {
            Ok(output) => Self::from_command_result(output.status.success(), &output.stdout),
            Err(_) => ModelCatalogLoad {
                catalog: Self::default(),
                warning: Some(WARNING.to_string()),
            },
        }
    }

    /// Parses a bundled Codex model catalog response.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is invalid JSON or contains no valid models.
    pub fn from_json(json: &str) -> Result<Self> {
        let response = serde_json::from_str::<CatalogResponse>(json)
            .context("failed to parse Codex model catalog")?;
        let mut profiles = BTreeMap::new();

        for raw_model in response.models {
            let Ok(model) = serde_json::from_value::<CatalogModel>(raw_model) else {
                continue;
            };
            let slug = model.slug.trim();
            let default_effort = model.default_reasoning_level.trim();
            let mut supported_efforts = Vec::new();

            for level in model.supported_reasoning_levels {
                let effort = level.effort.trim();
                if !effort.is_empty()
                    && !supported_efforts
                        .iter()
                        .any(|supported| supported == effort)
                {
                    supported_efforts.push(effort.to_string());
                }
            }

            if slug.is_empty()
                || default_effort.is_empty()
                || supported_efforts.is_empty()
                || !supported_efforts
                    .iter()
                    .any(|supported| supported == default_effort)
            {
                continue;
            }

            profiles.insert(
                slug.to_string(),
                ReasoningProfile {
                    default_effort: default_effort.to_string(),
                    supported_efforts,
                },
            );
        }

        if profiles.is_empty() {
            bail!("Codex model catalog contains no valid models");
        }

        Ok(Self {
            profiles,
            ..Self::default()
        })
    }

    pub fn profile_for(&self, model: Option<&str>) -> &ReasoningProfile {
        let model = model.map(str::trim);
        let model = match model {
            Some("gpt-5.6") => Some("gpt-5.6-sol"),
            other => other,
        };

        model
            .and_then(|model| self.profiles.get(model))
            .unwrap_or(&self.fallback)
    }

    #[must_use]
    pub fn normalize_effort(&self, model: Option<&str>, effort: Option<&str>) -> String {
        self.profile_for(model).normalize(effort)
    }

    fn from_command_result(success: bool, stdout: &[u8]) -> ModelCatalogLoad {
        if success
            && let Ok(text) = std::str::from_utf8(stdout)
            && let Ok(catalog) = Self::from_json(text)
        {
            return ModelCatalogLoad {
                catalog,
                warning: None,
            };
        }
        ModelCatalogLoad {
            catalog: Self::default(),
            warning: Some(WARNING.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalogLoad {
    pub catalog: ModelCatalog,
    pub warning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CatalogResponse {
    models: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CatalogModel {
    slug: String,
    default_reasoning_level: String,
    #[serde(default)]
    supported_reasoning_levels: Vec<CatalogLevel>,
}

#[derive(Debug, Deserialize)]
struct CatalogLevel {
    effort: String,
}

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
        assert_eq!(
            sol.supported_efforts(),
            ["low", "medium", "high", "xhigh", "max", "ultra"]
        );
        assert_eq!(catalog.profile_for(Some("gpt-5.6")), sol);
        assert!(catalog.profile_for(Some("gpt-5.6-terra")).supports("ultra"));
        assert!(!catalog.profile_for(Some("gpt-5.6-luna")).supports("ultra"));
    }

    #[test]
    fn unknown_model_uses_compatibility_profile() {
        let catalog = ModelCatalog::from_json(GPT_5_6_CATALOG).unwrap();
        let profile = catalog.profile_for(Some("custom-model"));
        assert_eq!(profile.default_effort(), "medium");
        assert_eq!(
            profile.supported_efforts(),
            ["low", "medium", "high", "xhigh"]
        );
        assert_eq!(
            catalog.normalize_effort(Some("custom-model"), Some("ultra")),
            "medium"
        );
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
    fn keeps_valid_siblings_when_entries_are_structurally_malformed() {
        let catalog = ModelCatalog::from_json(
            r#"{"models":[
              {"slug":"missing-default","supported_reasoning_levels":[{"effort":"medium"}]},
              {"slug":"wrong-type","default_reasoning_level":7,"supported_reasoning_levels":"medium"},
              {"slug":"valid","default_reasoning_level":"high","supported_reasoning_levels":[{"effort":"low"},{"effort":"high"}]}
            ]}"#,
        )
        .unwrap();

        let profile = catalog.profile_for(Some("valid"));
        assert_eq!(profile.default_effort(), "high");
        assert_eq!(profile.supported_efforts(), ["low", "high"]);
    }

    #[test]
    fn command_failures_return_warning_and_fallback_catalog() {
        let loaded = ModelCatalog::from_command_result(false, b"");
        assert!(loaded.warning.is_some());
        assert_eq!(
            loaded
                .catalog
                .profile_for(Some("gpt-5.6-sol"))
                .supported_efforts(),
            ["low", "medium", "high", "xhigh"]
        );
    }
}
