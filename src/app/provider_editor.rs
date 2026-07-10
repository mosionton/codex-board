use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::provider_config::{
    DEFAULT_AUTO_COMPACT_PERCENT, MAX_AUTO_COMPACT_PERCENT, MIN_AUTO_COMPACT_PERCENT, ModelCatalog,
    ProviderAuthMode, ProviderConfig, ReasoningProfile, effective_model,
};

use super::{TextField, cycle_index};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderField {
    Id,
    ApiKey,
    BaseUrl,
    WireApi,
    Auth,
    Model,
    ReasoningEffort,
    PlanReasoningEffort,
    AutoCompactPercent,
}

pub struct ProviderEditor {
    pub original_id: Option<String>,
    pub active_field: ProviderField,
    pub id: TextField,
    pub model: TextField,
    pub model_options: Vec<String>,
    pub reasoning_effort: String,
    pub reasoning_effort_options: Vec<String>,
    pub plan_reasoning_effort: String,
    pub plan_reasoning_effort_options: Vec<String>,
    pub auto_compact_percent: TextField,
    pub api_key: TextField,
    pub base_url: TextField,
    pub wire_api: String,
    pub auth_mode: ProviderAuthMode,
    model_catalog: Arc<ModelCatalog>,
    current_codex_model: Option<String>,
    reasoning_effort_explicit: bool,
    plan_reasoning_effort_explicit: bool,
}

pub const WIRE_API_OPTIONS: &[&str] = &["responses", "chat"];

impl ProviderEditor {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "kept as a compatibility constructor")
    )]
    pub fn new() -> Self {
        Self::new_with_catalog(Arc::new(ModelCatalog::default()))
    }

    pub fn new_with_catalog(model_catalog: Arc<ModelCatalog>) -> Self {
        Self::new_with_catalog_and_current_model(model_catalog, None)
    }

    pub fn new_with_catalog_and_current_model(
        model_catalog: Arc<ModelCatalog>,
        current_codex_model: Option<String>,
    ) -> Self {
        let profile = model_catalog.profile_for(current_codex_model.as_deref());
        Self {
            original_id: None,
            active_field: ProviderField::Id,
            id: TextField::empty(),
            model: TextField::empty(),
            model_options: Vec::new(),
            reasoning_effort: profile.default_effort().to_string(),
            reasoning_effort_options: profile.supported_efforts().to_vec(),
            plan_reasoning_effort: profile.default_effort().to_string(),
            plan_reasoning_effort_options: profile.supported_efforts().to_vec(),
            auto_compact_percent: TextField::new(DEFAULT_AUTO_COMPACT_PERCENT.to_string()),
            api_key: TextField::empty(),
            base_url: TextField::empty(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
            model_catalog,
            current_codex_model,
            reasoning_effort_explicit: false,
            plan_reasoning_effort_explicit: false,
        }
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "kept as a compatibility constructor")
    )]
    pub fn from_provider(id: &str, provider: &ProviderConfig) -> Self {
        Self::from_provider_with_catalog(id, provider, Arc::new(ModelCatalog::default()))
    }

    pub fn from_provider_with_catalog(
        id: &str,
        provider: &ProviderConfig,
        model_catalog: Arc<ModelCatalog>,
    ) -> Self {
        Self::from_provider_with_catalog_and_current_model(id, provider, None, model_catalog)
    }

    pub fn from_provider_with_catalog_and_current_model(
        id: &str,
        provider: &ProviderConfig,
        current_codex_model: Option<&str>,
        model_catalog: Arc<ModelCatalog>,
    ) -> Self {
        let model = provider.model.clone().unwrap_or_default();
        let profile = model_catalog.profile_for(effective_model(Some(&model), current_codex_model));
        let (reasoning_effort, reasoning_effort_explicit) =
            initial_effort(profile, provider.reasoning_effort.as_deref());
        let (plan_reasoning_effort, plan_reasoning_effort_explicit) =
            initial_effort(profile, provider.plan_reasoning_effort.as_deref());
        let api_key = if provider.auth_mode.requires_openai_auth() {
            String::new()
        } else {
            provider.api_key.clone().unwrap_or_default()
        };
        let base_url = provider.base_url.clone();
        Self {
            original_id: Some(id.to_string()),
            active_field: ProviderField::Id,
            id: TextField::new(id),
            model: TextField::new(model),
            model_options: Vec::new(),
            reasoning_effort,
            reasoning_effort_options: profile.supported_efforts().to_vec(),
            plan_reasoning_effort,
            plan_reasoning_effort_options: profile.supported_efforts().to_vec(),
            auto_compact_percent: TextField::new(provider.auto_compact_percent.to_string()),
            api_key: TextField::new(api_key),
            base_url: TextField::new(base_url),
            wire_api: provider.wire_api.clone(),
            auth_mode: provider.auth_mode,
            model_catalog,
            current_codex_model: current_codex_model.map(ToString::to_string),
            reasoning_effort_explicit,
            plan_reasoning_effort_explicit,
        }
    }

    pub fn next_field(&mut self) {
        let old_field = self.active_field;
        loop {
            self.active_field = self.active_field.next();
            if self.is_editable_field(self.active_field) {
                break;
            }
        }
        if old_field == ProviderField::Model {
            self.commit_model_change();
        }
    }

    pub fn previous_field(&mut self) {
        let old_field = self.active_field;
        loop {
            self.active_field = self.active_field.previous();
            if self.is_editable_field(self.active_field) {
                break;
            }
        }
        if old_field == ProviderField::Model {
            self.commit_model_change();
        }
    }

    pub const fn is_editable_field(&self, field: ProviderField) -> bool {
        match field {
            ProviderField::Auth => false,
            ProviderField::ApiKey => !self.auth_mode.requires_openai_auth(),
            ProviderField::Id
            | ProviderField::BaseUrl
            | ProviderField::WireApi
            | ProviderField::Model
            | ProviderField::ReasoningEffort
            | ProviderField::PlanReasoningEffort
            | ProviderField::AutoCompactPercent => true,
        }
    }

    pub fn clear_active_field(&mut self) {
        match self.active_field {
            ProviderField::Id => {
                self.id.clear();
            }
            ProviderField::Model => {
                self.model.clear();
                self.commit_model_change();
            }
            ProviderField::ReasoningEffort => {
                let profile = self.current_profile();
                self.reasoning_effort = profile.default_effort().to_string();
                self.reasoning_effort_explicit = false;
            }
            ProviderField::PlanReasoningEffort => {
                let profile = self.current_profile();
                self.plan_reasoning_effort = profile.default_effort().to_string();
                self.plan_reasoning_effort_explicit = false;
            }
            ProviderField::AutoCompactPercent => {
                self.auto_compact_percent
                    .set(DEFAULT_AUTO_COMPACT_PERCENT.to_string());
            }
            ProviderField::ApiKey => {
                self.api_key.clear();
            }
            ProviderField::BaseUrl => {
                self.base_url.clear();
            }
            ProviderField::WireApi => self.wire_api = default_wire_api().to_string(),
            ProviderField::Auth => {}
        }
    }

    pub const fn active_text_mut(&mut self) -> Option<&mut TextField> {
        match self.active_field {
            ProviderField::Id => Some(&mut self.id),
            ProviderField::Model => Some(&mut self.model),
            ProviderField::ApiKey if !self.auth_mode.requires_openai_auth() => {
                Some(&mut self.api_key)
            }
            ProviderField::BaseUrl => Some(&mut self.base_url),
            ProviderField::AutoCompactPercent => Some(&mut self.auto_compact_percent),
            ProviderField::ApiKey
            | ProviderField::ReasoningEffort
            | ProviderField::PlanReasoningEffort
            | ProviderField::WireApi
            | ProviderField::Auth => None,
        }
    }

    pub fn apply_model_options(&mut self, models: Vec<String>) {
        self.model_options = models;
        if self.model_options.is_empty() {
            return;
        }
        if self.model.trim().is_empty()
            || !self
                .model_options
                .iter()
                .any(|model| model == self.model.trim())
        {
            self.model.set(self.model_options[0].clone());
        } else {
            self.model.move_cursor_to_end();
        }
        self.active_field = ProviderField::Model;
        self.commit_model_change();
    }

    pub fn cycle_model_option(&mut self, delta: isize) -> bool {
        if self.model_options.is_empty() {
            return false;
        }
        let current = self
            .model_options
            .iter()
            .position(|model| model == self.model.trim())
            .unwrap_or(0);
        let next = cycle_index(current, self.model_options.len(), delta);
        self.model.set(self.model_options[next].clone());
        self.commit_model_change();
        true
    }

    pub fn commit_model_change(&mut self) {
        let profile = self.current_profile().clone();
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

    pub fn parsed_auto_compact_percent(&self) -> Result<u8> {
        let percent = self
            .auto_compact_percent
            .trim()
            .parse::<u8>()
            .context("auto_compact_percent must be an integer between 1 and 99")?;
        if !(MIN_AUTO_COMPACT_PERCENT..=MAX_AUTO_COMPACT_PERCENT).contains(&percent) {
            bail!(
                "auto_compact_percent must be between {MIN_AUTO_COMPACT_PERCENT} and {MAX_AUTO_COMPACT_PERCENT}"
            );
        }
        Ok(percent)
    }

    pub const fn text_cursor_for(&self, field: ProviderField) -> Option<usize> {
        match field {
            ProviderField::Id => Some(self.id.cursor()),
            ProviderField::Model => Some(self.model.cursor()),
            ProviderField::ApiKey => Some(self.api_key.cursor()),
            ProviderField::BaseUrl => Some(self.base_url.cursor()),
            ProviderField::AutoCompactPercent => Some(self.auto_compact_percent.cursor()),
            ProviderField::ReasoningEffort
            | ProviderField::PlanReasoningEffort
            | ProviderField::WireApi
            | ProviderField::Auth => None,
        }
    }

    pub fn cycle_active_option(&mut self, delta: isize) -> bool {
        match self.active_field {
            ProviderField::ReasoningEffort => {
                let cycled = cycle_owned_string_option(
                    &mut self.reasoning_effort,
                    &self.reasoning_effort_options,
                    delta,
                );
                self.reasoning_effort_explicit |= cycled;
                cycled
            }
            ProviderField::PlanReasoningEffort => {
                let cycled = cycle_owned_string_option(
                    &mut self.plan_reasoning_effort,
                    &self.plan_reasoning_effort_options,
                    delta,
                );
                self.plan_reasoning_effort_explicit |= cycled;
                cycled
            }
            ProviderField::WireApi => {
                cycle_string_option(&mut self.wire_api, WIRE_API_OPTIONS, delta)
            }
            ProviderField::Id
            | ProviderField::Model
            | ProviderField::ApiKey
            | ProviderField::BaseUrl
            | ProviderField::AutoCompactPercent
            | ProviderField::Auth => false,
        }
    }

    pub const fn auth_mode_display(&self) -> &'static str {
        self.auth_mode.as_str()
    }

    fn current_profile(&self) -> &ReasoningProfile {
        self.model_catalog.profile_for(effective_model(
            Some(self.model.as_str()),
            self.current_codex_model.as_deref(),
        ))
    }
}

impl ProviderField {
    pub const fn next(self) -> Self {
        match self {
            Self::Id => Self::BaseUrl,
            Self::BaseUrl => Self::ApiKey,
            Self::ApiKey => Self::WireApi,
            Self::WireApi | Self::Auth => Self::Model,
            Self::Model => Self::ReasoningEffort,
            Self::ReasoningEffort => Self::PlanReasoningEffort,
            Self::PlanReasoningEffort => Self::AutoCompactPercent,
            Self::AutoCompactPercent => Self::Id,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Id => Self::AutoCompactPercent,
            Self::BaseUrl => Self::Id,
            Self::ApiKey => Self::BaseUrl,
            Self::WireApi => Self::ApiKey,
            Self::Auth | Self::Model => Self::WireApi,
            Self::ReasoningEffort => Self::Model,
            Self::PlanReasoningEffort => Self::ReasoningEffort,
            Self::AutoCompactPercent => Self::PlanReasoningEffort,
        }
    }
}

pub(super) fn empty_to_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn editor_credentials(
    editor: &ProviderEditor,
    original_provider: Option<&ProviderConfig>,
) -> (Option<String>, Option<String>) {
    let api_key = empty_to_none(editor.api_key.as_str());
    let Some(original_provider) = original_provider else {
        return (api_key, None);
    };
    let Some(env_key) = original_provider.env_key.clone() else {
        return (api_key, None);
    };

    if api_key.is_none() || api_key.as_deref() == original_provider.api_key.as_deref() {
        (None, Some(env_key))
    } else {
        (api_key, None)
    }
}

fn default_wire_api() -> &'static str {
    WIRE_API_OPTIONS[0]
}

#[expect(
    clippy::option_if_let_else,
    reason = "the branches make explicit-choice state directly auditable"
)]
fn initial_effort(profile: &ReasoningProfile, value: Option<&str>) -> (String, bool) {
    match value.map(str::trim).filter(|value| profile.supports(value)) {
        Some(value) => (value.to_string(), true),
        None => (profile.default_effort().to_string(), false),
    }
}

fn cycle_string_option(current: &mut String, options: &[&str], delta: isize) -> bool {
    if options.is_empty() {
        return false;
    }

    let index = options
        .iter()
        .position(|option| *option == current.as_str())
        .unwrap_or(0);
    let next = cycle_index(index, options.len(), delta);
    *current = options[next].to_string();
    true
}

fn cycle_owned_string_option(current: &mut String, options: &[String], delta: isize) -> bool {
    if options.is_empty() {
        return false;
    }

    let index = options
        .iter()
        .position(|option| option == current)
        .unwrap_or(0);
    let next = cycle_index(index, options.len(), delta);
    current.clone_from(&options[next]);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::provider_config::{DEFAULT_AUTO_COMPACT_PERCENT, ModelCatalog};

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

    fn api_key_provider() -> ProviderConfig {
        ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: Some("invalid".to_string()),
            plan_reasoning_effort: Some("high".to_string()),
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: Some("sk-test".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        }
    }

    #[test]
    fn new_editor_uses_selected_model_default() {
        let mut editor = ProviderEditor::new_with_catalog(gpt_5_6_catalog());
        editor.model.set("gpt-5.6-sol");
        editor.commit_model_change();
        assert_eq!(editor.reasoning_effort, "low");
        assert_eq!(editor.plan_reasoning_effort, "low");
        assert_eq!(
            editor.reasoning_effort_options,
            ["low", "medium", "high", "xhigh", "max", "ultra"]
        );
    }

    #[test]
    fn preserves_supported_explicit_effort_and_replaces_unsupported_effort() {
        let provider = ProviderConfig {
            model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: Some("xhigh".to_string()),
            plan_reasoning_effort: Some("ultra".to_string()),
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };
        let mut editor =
            ProviderEditor::from_provider_with_catalog("switcher", &provider, gpt_5_6_catalog());
        editor.model.set("gpt-5.6-luna");
        editor.commit_model_change();
        assert_eq!(editor.reasoning_effort, "xhigh");
        assert_eq!(editor.plan_reasoning_effort, "medium");
    }

    #[test]
    fn empty_provider_model_uses_current_sol_profile_without_persisting_model() {
        let provider = ProviderConfig {
            model: None,
            reasoning_effort: Some("ultra".to_string()),
            plan_reasoning_effort: Some("max".to_string()),
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        let editor = ProviderEditor::from_provider_with_catalog_and_current_model(
            "switcher",
            &provider,
            Some("gpt-5.6-sol"),
            gpt_5_6_catalog(),
        );

        assert_eq!(editor.model.as_str(), "");
        assert_eq!(editor.reasoning_effort, "ultra");
        assert_eq!(editor.plan_reasoning_effort, "max");
        assert!(
            editor
                .reasoning_effort_options
                .contains(&"ultra".to_string())
        );
    }

    #[test]
    fn empty_provider_model_uses_current_luna_profile_per_effort() {
        let provider = ProviderConfig {
            model: None,
            reasoning_effort: Some("ultra".to_string()),
            plan_reasoning_effort: Some("max".to_string()),
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        let editor = ProviderEditor::from_provider_with_catalog_and_current_model(
            "switcher",
            &provider,
            Some("gpt-5.6-luna"),
            gpt_5_6_catalog(),
        );

        assert_eq!(editor.model.as_str(), "");
        assert_eq!(editor.reasoning_effort, "medium");
        assert_eq!(editor.plan_reasoning_effort, "max");
        assert!(
            !editor
                .reasoning_effort_options
                .contains(&"ultra".to_string())
        );
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

    #[test]
    fn from_provider_populates_fields_and_normalizes_reasoning() {
        let provider = api_key_provider();

        let editor = ProviderEditor::from_provider("switcher", &provider);

        assert_eq!(editor.original_id.as_deref(), Some("switcher"));
        assert_eq!(editor.id.as_str(), "switcher");
        assert_eq!(editor.id.cursor(), "switcher".chars().count());
        assert_eq!(editor.model.as_str(), "gpt-5.5");
        assert_eq!(editor.model.cursor(), "gpt-5.5".chars().count());
        assert_eq!(editor.api_key.as_str(), "sk-test");
        assert_eq!(editor.api_key.cursor(), "sk-test".chars().count());
        assert_eq!(editor.base_url.as_str(), "https://api.example.test/v1");
        assert_eq!(
            editor.base_url.cursor(),
            "https://api.example.test/v1".chars().count()
        );
        assert_eq!(editor.reasoning_effort, "medium");
        assert_eq!(editor.plan_reasoning_effort, "high");
    }

    #[test]
    fn field_navigation_skips_non_editable_fields_for_openai_auth() {
        let mut editor = ProviderEditor::new();
        editor.auth_mode = ProviderAuthMode::OpenAi;
        editor.active_field = ProviderField::BaseUrl;

        editor.next_field();
        assert_eq!(editor.active_field, ProviderField::WireApi);

        editor.previous_field();
        assert_eq!(editor.active_field, ProviderField::BaseUrl);
    }

    #[test]
    fn clear_active_field_resets_text_and_defaults() {
        let mut editor = ProviderEditor::new();
        editor.id.set_with_cursor("switcher", 3);
        editor.active_field = ProviderField::Id;
        editor.clear_active_field();
        assert_eq!(editor.id.as_str(), "");
        assert_eq!(editor.id.cursor(), 0);

        editor
            .base_url
            .set_with_cursor("https://api.example.test/v1", 5);
        editor.active_field = ProviderField::BaseUrl;
        editor.clear_active_field();
        assert_eq!(editor.base_url.as_str(), "");
        assert_eq!(editor.base_url.cursor(), 0);

        editor.reasoning_effort = "xhigh".to_string();
        editor.active_field = ProviderField::ReasoningEffort;
        editor.clear_active_field();
        assert_eq!(editor.reasoning_effort, "medium");

        editor.wire_api = "chat".to_string();
        editor.active_field = ProviderField::WireApi;
        editor.clear_active_field();
        assert_eq!(editor.wire_api, default_wire_api());
    }

    #[test]
    fn active_text_mut_and_text_cursor_follow_editable_fields() {
        let mut editor = ProviderEditor::new();
        editor.active_field = ProviderField::Id;

        let text = editor.active_text_mut().unwrap();
        text.set("provider");
        assert_eq!(editor.id.as_str(), "provider");
        assert_eq!(editor.text_cursor_for(ProviderField::Id), Some(8));
        assert_eq!(editor.text_cursor_for(ProviderField::ReasoningEffort), None);

        editor.auth_mode = ProviderAuthMode::OpenAi;
        editor.active_field = ProviderField::ApiKey;
        assert!(editor.active_text_mut().is_none());
    }

    #[test]
    fn apply_model_options_preserves_matching_model_and_handles_empty_options() {
        let mut editor = ProviderEditor::new();
        editor.active_field = ProviderField::BaseUrl;
        editor.model.set_with_cursor("gpt-5.5", 2);

        editor.apply_model_options(Vec::new());
        assert_eq!(editor.active_field, ProviderField::BaseUrl);
        assert_eq!(editor.model.as_str(), "gpt-5.5");
        assert_eq!(editor.model.cursor(), 2);

        editor.apply_model_options(vec!["gpt-5-mini".to_string(), "gpt-5.5".to_string()]);
        assert_eq!(editor.active_field, ProviderField::Model);
        assert_eq!(editor.model.as_str(), "gpt-5.5");
        assert_eq!(editor.model.cursor(), "gpt-5.5".chars().count());
    }

    #[test]
    fn cycle_active_option_wraps_supported_fields() {
        let mut editor = ProviderEditor::new();

        editor.active_field = ProviderField::ReasoningEffort;
        assert!(editor.cycle_active_option(1));
        assert_eq!(editor.reasoning_effort, "high");

        editor.active_field = ProviderField::PlanReasoningEffort;
        assert!(editor.cycle_active_option(-1));
        assert_eq!(editor.plan_reasoning_effort, "low");

        editor.active_field = ProviderField::WireApi;
        assert!(editor.cycle_active_option(1));
        assert_eq!(editor.wire_api, "chat");

        editor.active_field = ProviderField::Id;
        assert!(!editor.cycle_active_option(1));
    }

    #[test]
    fn empty_to_none_trims_non_empty_values() {
        assert_eq!(empty_to_none(""), None);
        assert_eq!(empty_to_none("   "), None);
        assert_eq!(empty_to_none("  gpt-5.5 "), Some("gpt-5.5".to_string()));
    }

    #[test]
    fn auto_compact_percent_defaults_loads_and_validates() {
        let editor = ProviderEditor::new();
        assert_eq!(editor.auto_compact_percent.as_str(), "70");
        assert_eq!(editor.parsed_auto_compact_percent().unwrap(), 70);

        let mut provider = api_key_provider();
        provider.auto_compact_percent = 65;
        let mut editor = ProviderEditor::from_provider("switcher", &provider);
        assert_eq!(editor.auto_compact_percent.as_str(), "65");

        editor.auto_compact_percent.set("1");
        assert_eq!(editor.parsed_auto_compact_percent().unwrap(), 1);
        editor.auto_compact_percent.set("99");
        assert_eq!(editor.parsed_auto_compact_percent().unwrap(), 99);

        for invalid in ["", "abc", "0", "100", "999"] {
            editor.auto_compact_percent.set(invalid);
            assert!(editor.parsed_auto_compact_percent().is_err());
        }
    }

    #[test]
    fn auto_compact_field_participates_in_navigation_and_reset() {
        let mut editor = ProviderEditor::new();
        editor.active_field = ProviderField::PlanReasoningEffort;

        editor.next_field();
        assert_eq!(editor.active_field, ProviderField::AutoCompactPercent);
        editor.auto_compact_percent.set("65");
        editor.clear_active_field();
        assert_eq!(editor.auto_compact_percent.as_str(), "70");

        editor.next_field();
        assert_eq!(editor.active_field, ProviderField::Id);
        editor.previous_field();
        assert_eq!(editor.active_field, ProviderField::AutoCompactPercent);
    }

    #[test]
    fn editor_credentials_keep_env_key_until_api_key_changes() {
        let original_provider = ProviderConfig {
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: Some("sk-original".to_string()),
            ..api_key_provider()
        };
        let mut editor = ProviderEditor::from_provider("switcher", &original_provider);

        editor.api_key.clear();
        assert_eq!(
            editor_credentials(&editor, Some(&original_provider)),
            (None, Some("OPENAI_API_KEY".to_string()))
        );

        editor.api_key.set("sk-original");
        assert_eq!(
            editor_credentials(&editor, Some(&original_provider)),
            (None, Some("OPENAI_API_KEY".to_string()))
        );

        editor.api_key.set("sk-new");
        assert_eq!(
            editor_credentials(&editor, Some(&original_provider)),
            (Some("sk-new".to_string()), None)
        );
    }
}
