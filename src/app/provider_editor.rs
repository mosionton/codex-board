use crate::provider_config::{
    DEFAULT_REASONING_EFFORT, PLAN_REASONING_EFFORT_OPTIONS, ProviderAuthMode, ProviderConfig,
    REASONING_EFFORT_OPTIONS, normalize_reasoning_effort,
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
}

pub struct ProviderEditor {
    pub original_id: Option<String>,
    pub active_field: ProviderField,
    pub id: TextField,
    pub model: TextField,
    pub model_options: Vec<String>,
    pub reasoning_effort: String,
    pub plan_reasoning_effort: String,
    pub api_key: TextField,
    pub base_url: TextField,
    pub wire_api: String,
    pub auth_mode: ProviderAuthMode,
}

pub const WIRE_API_OPTIONS: &[&str] = &["responses", "chat"];

impl ProviderEditor {
    pub fn new() -> Self {
        Self {
            original_id: None,
            active_field: ProviderField::Id,
            id: TextField::empty(),
            model: TextField::empty(),
            model_options: Vec::new(),
            reasoning_effort: DEFAULT_REASONING_EFFORT.to_string(),
            plan_reasoning_effort: DEFAULT_REASONING_EFFORT.to_string(),
            api_key: TextField::empty(),
            base_url: TextField::empty(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        }
    }

    pub fn from_provider(id: &str, provider: &ProviderConfig) -> Self {
        let model = provider.model.clone().unwrap_or_default();
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
            reasoning_effort: normalize_reasoning_effort(provider.reasoning_effort.as_deref())
                .to_string(),
            plan_reasoning_effort: normalize_reasoning_effort(
                provider.plan_reasoning_effort.as_deref(),
            )
            .to_string(),
            api_key: TextField::new(api_key),
            base_url: TextField::new(base_url),
            wire_api: provider.wire_api.clone(),
            auth_mode: provider.auth_mode,
        }
    }

    pub const fn next_field(&mut self) {
        loop {
            self.active_field = self.active_field.next();
            if self.is_editable_field(self.active_field) {
                break;
            }
        }
    }

    pub const fn previous_field(&mut self) {
        loop {
            self.active_field = self.active_field.previous();
            if self.is_editable_field(self.active_field) {
                break;
            }
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
            | ProviderField::PlanReasoningEffort => true,
        }
    }

    pub fn clear_active_field(&mut self) {
        match self.active_field {
            ProviderField::Id => {
                self.id.clear();
            }
            ProviderField::Model => {
                self.model.clear();
            }
            ProviderField::ReasoningEffort => {
                self.reasoning_effort = DEFAULT_REASONING_EFFORT.to_string();
            }
            ProviderField::PlanReasoningEffort => {
                self.plan_reasoning_effort = DEFAULT_REASONING_EFFORT.to_string();
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
        true
    }

    pub const fn text_cursor_for(&self, field: ProviderField) -> Option<usize> {
        match field {
            ProviderField::Id => Some(self.id.cursor()),
            ProviderField::Model => Some(self.model.cursor()),
            ProviderField::ApiKey => Some(self.api_key.cursor()),
            ProviderField::BaseUrl => Some(self.base_url.cursor()),
            ProviderField::ReasoningEffort
            | ProviderField::PlanReasoningEffort
            | ProviderField::WireApi
            | ProviderField::Auth => None,
        }
    }

    pub fn cycle_active_option(&mut self, delta: isize) -> bool {
        match self.active_field {
            ProviderField::ReasoningEffort => {
                cycle_string_option(&mut self.reasoning_effort, REASONING_EFFORT_OPTIONS, delta)
            }
            ProviderField::PlanReasoningEffort => cycle_string_option(
                &mut self.plan_reasoning_effort,
                PLAN_REASONING_EFFORT_OPTIONS,
                delta,
            ),
            ProviderField::WireApi => {
                cycle_string_option(&mut self.wire_api, WIRE_API_OPTIONS, delta)
            }
            ProviderField::Id
            | ProviderField::Model
            | ProviderField::ApiKey
            | ProviderField::BaseUrl
            | ProviderField::Auth => false,
        }
    }

    pub const fn auth_mode_display(&self) -> &'static str {
        self.auth_mode.as_str()
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
            Self::PlanReasoningEffort => Self::Id,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Id => Self::PlanReasoningEffort,
            Self::BaseUrl => Self::Id,
            Self::ApiKey => Self::BaseUrl,
            Self::WireApi => Self::ApiKey,
            Self::Auth | Self::Model => Self::WireApi,
            Self::ReasoningEffort => Self::Model,
            Self::PlanReasoningEffort => Self::ReasoningEffort,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn api_key_provider() -> ProviderConfig {
        ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: Some("invalid".to_string()),
            plan_reasoning_effort: Some("high".to_string()),
            api_key: Some("sk-test".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        }
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
        assert_eq!(editor.reasoning_effort, DEFAULT_REASONING_EFFORT);
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
        assert_eq!(editor.reasoning_effort, DEFAULT_REASONING_EFFORT);

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
    fn editor_credentials_keep_env_key_until_api_key_changes() {
        let original_provider = ProviderConfig {
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
