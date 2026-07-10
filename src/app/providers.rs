use std::{cmp::Ordering, time::Duration};

use crate::provider_config::{self, ProviderConfig};

use super::{
    App, ConfirmationAction, Overlay, ProviderEditor,
    model_fetch::{ModelFetchStatus, ModelFetchTask},
    provider_editor::{editor_credentials, empty_to_none},
};

impl App {
    pub(crate) fn provider_ids(&self) -> Vec<String> {
        let mut providers = self
            .providers
            .registry()
            .providers
            .iter()
            .collect::<Vec<_>>();
        providers.sort_by(|(left_id, left), (right_id, right)| {
            match (
                is_openai_auth_provider(left),
                is_openai_auth_provider(right),
            ) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => left_id.cmp(right_id),
            }
        });
        providers
            .into_iter()
            .map(|(id, _provider)| id.clone())
            .collect()
    }

    pub(crate) fn selected_provider_id(&self) -> Option<String> {
        self.provider_ids()
            .get(self.providers.selection.index())
            .cloned()
    }

    pub(crate) fn provider_row_count(&self) -> usize {
        self.providers.registry.providers.len()
            + usize::from(self.providers.claude_status.is_some())
    }

    pub(crate) fn is_claude_row_selected(&self) -> bool {
        self.providers.claude_status.is_some()
            && self.providers.selection.index() == self.providers.registry.providers.len()
    }

    pub(crate) fn refresh_provider_selection(&mut self) {
        let row_count = self.provider_row_count();
        self.providers.selection.sync_len(row_count);
    }

    pub(crate) fn move_provider_selection(&mut self, delta: isize) {
        let row_count = self.provider_row_count();
        self.providers.selection.move_by(row_count, delta);
    }

    pub(crate) fn page_provider_selection(&mut self, delta: isize) {
        let row_count = self.provider_row_count();
        self.providers
            .selection
            .move_by_clamped(row_count, delta * 10);
    }

    pub(crate) fn start_new_provider(&mut self) {
        self.providers.model_fetch_task = None;
        let model_catalog = self.providers.model_catalog();
        let current_codex_model = self
            .providers
            .current_codex_model()
            .map(ToString::to_string);
        self.providers.editor = Some(ProviderEditor::new_with_catalog_and_current_model(
            model_catalog,
            current_codex_model,
        ));
        self.overlay = Some(Overlay::ProviderEditor);
        self.clear_status();
    }

    fn reject_claude_row_action(&mut self) -> bool {
        if self.is_claude_row_selected() {
            self.show_status("claude is read-only; it is managed by Claude Code itself.");
            return true;
        }
        false
    }

    pub(crate) fn start_edit_provider(&mut self) {
        if self.reject_claude_row_action() {
            return;
        }
        let Some(id) = self.selected_provider_id() else {
            self.show_error("No provider selected.");
            return;
        };
        let Some(provider) = self.providers.registry.providers.get(&id) else {
            return;
        };
        self.providers.model_fetch_task = None;
        let model_catalog = self.providers.model_catalog();
        let current_codex_model = self
            .providers
            .current_codex_model()
            .map(ToString::to_string);
        self.providers.editor = Some(
            ProviderEditor::from_provider_with_catalog_and_current_model(
                &id,
                provider,
                current_codex_model.as_deref(),
                model_catalog,
            ),
        );
        self.overlay = Some(Overlay::ProviderEditor);
        self.clear_status();
    }

    pub(crate) fn prompt_delete_selected_provider(&mut self) {
        if self.reject_claude_row_action() {
            return;
        }
        let Some(id) = self.selected_provider_id() else {
            self.show_error("No provider selected.");
            return;
        };
        self.confirmation = Some(ConfirmationAction::DeleteProvider(id));
        self.overlay = Some(Overlay::Confirmation);
        self.clear_status();
    }

    pub(crate) fn delete_provider(&mut self, id: &str) {
        let _ = self.providers.registry.remove(id);
        if let Err(err) = self.providers.registry.save(&self.providers.config_path) {
            self.show_error(format!("Failed to save providers: {err}"));
            return;
        }
        self.refresh_provider_selection();
        self.show_status(format!("Deleted provider '{id}'."));
    }

    pub(crate) fn prompt_apply_selected_provider(&mut self) {
        if self.reject_claude_row_action() {
            return;
        }
        let Some(id) = self.selected_provider_id() else {
            self.show_error("No provider selected.");
            return;
        };
        self.confirmation = Some(ConfirmationAction::ApplyProvider(id));
        self.overlay = Some(Overlay::Confirmation);
        self.clear_status();
    }

    pub(crate) fn prompt_save_provider_editor(&mut self) {
        if let Some(editor) = self.providers.editor.as_mut() {
            editor.commit_model_change();
        }
        let Some(editor) = self.providers.editor.as_ref() else {
            return;
        };
        let label = if !editor.id.trim().is_empty() {
            editor.id.trim().to_string()
        } else if let Some(original_id) = editor.original_id.as_deref() {
            original_id.to_string()
        } else {
            "<new provider>".to_string()
        };
        self.confirmation = Some(ConfirmationAction::SaveProvider(label));
        self.overlay = Some(Overlay::Confirmation);
        self.clear_status();
    }

    pub(crate) fn fetch_provider_models_for_editor(&mut self) {
        let Some(editor) = self.providers.editor.as_ref() else {
            return;
        };
        if self.providers.model_fetch_task.is_some() {
            self.show_status("Model fetch already in progress.");
            return;
        }
        if editor.auth_mode.requires_openai_auth() {
            self.show_error("Cannot fetch models: auth_mode=openai uses Codex credentials.");
            return;
        }
        let base_url = editor.base_url.trim().to_string();
        let api_key = editor.api_key.trim().to_string();
        if base_url.is_empty() {
            self.show_error("Cannot fetch models: base_url is empty.");
            return;
        }
        if api_key.is_empty() {
            self.show_error("Cannot fetch models: api_key is empty.");
            return;
        }

        match ModelFetchTask::spawn(base_url, api_key) {
            Ok(task) => {
                self.providers.model_fetch_task = Some(task);
                self.show_status("Fetching models...");
            }
            Err(err) => self.show_error(format!("Failed to start model fetch: {err}")),
        }
    }

    pub(crate) fn poll_model_fetch(&mut self) {
        let Some(status) = self
            .providers
            .model_fetch_task
            .as_ref()
            .map(ModelFetchTask::poll)
        else {
            return;
        };

        match status {
            ModelFetchStatus::Finished { base_url, result } => {
                self.providers.model_fetch_task = None;
                self.apply_model_fetch_result(&base_url, result);
            }
            ModelFetchStatus::Pending => {}
            ModelFetchStatus::Disconnected => {
                self.providers.model_fetch_task = None;
                self.show_error("Failed to fetch models: background task ended unexpectedly.");
            }
        }
    }

    fn apply_model_fetch_result(&mut self, base_url: &str, result: Result<Vec<String>, String>) {
        match result {
            Ok(models) => {
                let count = models.len();
                let Some(editor) = self.providers.editor.as_mut() else {
                    self.show_transient_status(
                        format!("Fetched {count} models after the provider editor closed."),
                        Duration::from_secs(2),
                    );
                    return;
                };
                if editor.base_url.trim() != base_url {
                    self.show_transient_status(
                        "Ignored fetched models because base_url changed.",
                        Duration::from_secs(2),
                    );
                    return;
                }
                editor.apply_model_options(models);
                self.show_transient_status(
                    format!("Fetched {count} models."),
                    Duration::from_secs(2),
                );
            }
            Err(err) => self.show_error(format!("Failed to fetch models: {err}")),
        }
    }

    pub(crate) fn apply_provider(&mut self, id: &str) {
        let model_catalog = self.providers.model_catalog();
        let Some(provider) = self.providers.registry.providers.get(id) else {
            self.show_error("No provider selected.");
            return;
        };
        if let Err(err) = provider_config::apply_provider_to_codex(
            id,
            provider,
            self.providers.codex_config_path(),
            model_catalog.as_ref(),
        ) {
            self.show_error(format!("Failed to apply provider: {err}"));
            return;
        }
        self.providers.applied_provider_id = Some(id.to_string());
        if let Some(model) = provider
            .model
            .as_deref()
            .map(str::trim)
            .filter(|model| !model.is_empty())
        {
            self.providers
                .set_current_codex_model(Some(model.to_string()));
        }
        self.show_status(format!("Applied provider '{id}' to Codex config."));
    }

    pub(crate) fn save_provider_editor(&mut self) {
        self.providers.model_fetch_task = None;
        let Some(editor) = self.providers.editor.take() else {
            return;
        };
        let id = editor.id.trim().to_string();
        let original_provider = editor
            .original_id
            .as_deref()
            .and_then(|original_id| self.providers.registry.providers.get(original_id));
        let (api_key, env_key) = if editor.auth_mode.requires_openai_auth() {
            (None, None)
        } else {
            editor_credentials(&editor, original_provider)
        };
        let config = ProviderConfig {
            model: empty_to_none(editor.model.as_str()),
            reasoning_effort: empty_to_none(&editor.reasoning_effort),
            plan_reasoning_effort: empty_to_none(&editor.plan_reasoning_effort),
            api_key,
            env_key,
            base_url: editor.base_url.trim().to_string(),
            wire_api: editor.wire_api.trim().to_string(),
            auth_mode: editor.auth_mode,
        };

        let mut provider_registry = self.providers.registry.clone();
        if let Some(original_id) = editor.original_id.as_deref()
            && original_id != id
        {
            let _ = provider_registry.remove(original_id);
        }
        if let Err(err) = provider_registry.upsert(id.clone(), config) {
            self.restore_provider_editor_with_error(editor, format!("Invalid provider: {err}"));
            return;
        }
        if let Err(err) = provider_registry.save(&self.providers.config_path) {
            self.restore_provider_editor_with_error(
                editor,
                format!("Failed to save providers: {err}"),
            );
            return;
        }
        self.providers.registry = provider_registry;
        let ids = self.provider_ids();
        if let Some(index) = ids.iter().position(|provider_id| provider_id == &id) {
            self.providers.selection.select(index);
        }
        self.refresh_provider_selection();
        self.overlay = None;
        self.show_status(format!("Saved provider '{id}'."));
    }

    pub(crate) fn restore_provider_editor_with_error(
        &mut self,
        editor: ProviderEditor,
        message: impl Into<String>,
    ) {
        self.show_error(message);
        self.providers.editor = Some(editor);
        self.overlay = Some(Overlay::ProviderEditor);
    }
}

const fn is_openai_auth_provider(provider: &ProviderConfig) -> bool {
    provider.auth_mode.requires_openai_auth()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::mpsc};

    use crate::provider_config::{ModelCatalog, ProviderAuthMode, ProviderRegistry};
    use tempfile::tempdir;

    fn test_provider() -> ProviderConfig {
        ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        }
    }

    fn model_catalog() -> ModelCatalog {
        ModelCatalog::from_json(
            r#"{"models":[
              {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn provider_editors_use_shared_model_catalog() {
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig {
                    model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: None,
                    plan_reasoning_effort: None,
                    ..test_provider()
                },
            )
            .unwrap();
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        );
        app.providers.set_model_catalog(model_catalog());

        app.start_edit_provider();

        let editor = app.providers.editor().unwrap();
        assert_eq!(editor.reasoning_effort, "low");
        assert_eq!(editor.plan_reasoning_effort, "low");
    }

    #[test]
    fn provider_editor_uses_current_codex_model_when_provider_model_is_empty() {
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig {
                    model: None,
                    reasoning_effort: Some("ultra".to_string()),
                    plan_reasoning_effort: Some("max".to_string()),
                    ..test_provider()
                },
            )
            .unwrap();
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        );
        app.providers.set_model_catalog(model_catalog());
        app.providers
            .set_current_codex_model(Some("gpt-5.6-sol".to_string()));

        app.start_edit_provider();

        let editor = app.providers.editor().unwrap();
        assert_eq!(editor.model.as_str(), "");
        assert_eq!(editor.reasoning_effort, "ultra");
        assert_eq!(editor.plan_reasoning_effort, "max");
    }

    #[test]
    fn saving_empty_provider_model_keeps_it_empty() {
        let dir = tempdir().unwrap();
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig {
                    model: None,
                    reasoning_effort: Some("ultra".to_string()),
                    plan_reasoning_effort: Some("max".to_string()),
                    ..test_provider()
                },
            )
            .unwrap();
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            dir.path().join("providers.toml"),
            dir.path().join("config.toml"),
            dir.path().join("sessions"),
        );
        app.providers.set_model_catalog(model_catalog());
        app.providers
            .set_current_codex_model(Some("gpt-5.6-sol".to_string()));
        app.start_edit_provider();

        app.save_provider_editor();

        assert_eq!(app.providers.provider("switcher").unwrap().model, None);
    }

    #[test]
    fn prompting_save_commits_manually_entered_model() {
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        );
        app.providers.set_model_catalog(model_catalog());
        app.start_new_provider();
        let editor = app.providers.editor_mut().unwrap();
        editor.id.set("switcher");
        editor.model.set("gpt-5.6-sol");

        app.prompt_save_provider_editor();

        let editor = app.providers.editor().unwrap();
        assert_eq!(editor.reasoning_effort, "low");
        assert_eq!(editor.plan_reasoning_effort, "low");
    }

    fn app_with_provider_count(count: usize) -> App {
        let mut registry = ProviderRegistry::default();
        for index in 0..count {
            registry
                .upsert(format!("provider-{index:02}"), test_provider())
                .unwrap();
        }
        App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        )
    }

    #[test]
    fn poll_model_fetch_applies_completed_models_to_matching_editor() {
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        );
        let mut editor = ProviderEditor::new();
        editor.base_url.set("https://api.example.test/v1");
        app.providers.editor = Some(editor);
        app.overlay = Some(Overlay::ProviderEditor);
        let (sender, receiver) = mpsc::channel();
        app.providers.model_fetch_task = Some(ModelFetchTask::from_receiver(
            "https://api.example.test/v1".to_string(),
            receiver,
        ));

        sender
            .send(Ok(vec!["gpt-5-mini".to_string(), "gpt-5.5".to_string()]))
            .unwrap();
        app.poll_model_fetch();

        let editor = app.providers.editor.as_ref().unwrap();
        assert!(app.providers.model_fetch_task.is_none());
        assert_eq!(editor.model.as_str(), "gpt-5-mini");
        assert_eq!(
            editor.model_options,
            vec!["gpt-5-mini".to_string(), "gpt-5.5".to_string()]
        );
        assert_eq!(app.status, "Fetched 2 models.");
    }

    #[test]
    fn poll_model_fetch_ignores_completed_models_when_base_url_changes() {
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        );
        let mut editor = ProviderEditor::new();
        editor.base_url.set("https://changed.example.test/v1");
        app.providers.editor = Some(editor);
        app.overlay = Some(Overlay::ProviderEditor);
        let (sender, receiver) = mpsc::channel();
        app.providers.model_fetch_task = Some(ModelFetchTask::from_receiver(
            "https://api.example.test/v1".to_string(),
            receiver,
        ));

        sender.send(Ok(vec!["gpt-5-mini".to_string()])).unwrap();
        app.poll_model_fetch();

        let editor = app.providers.editor.as_ref().unwrap();
        assert!(app.providers.model_fetch_task.is_none());
        assert!(editor.model_options.is_empty());
        assert_eq!(
            app.status,
            "Ignored fetched models because base_url changed."
        );
    }

    #[test]
    fn page_provider_selection_clamps_at_provider_boundaries() {
        let mut app = app_with_provider_count(12);

        app.page_provider_selection(-1);
        assert_eq!(app.providers.selection.index(), 0);
        assert_eq!(app.selected_provider_id().as_deref(), Some("provider-00"));

        app.page_provider_selection(1);
        assert_eq!(app.providers.selection.index(), 10);
        assert_eq!(app.selected_provider_id().as_deref(), Some("provider-10"));

        app.page_provider_selection(1);
        assert_eq!(app.providers.selection.index(), 11);
        assert_eq!(app.selected_provider_id().as_deref(), Some("provider-11"));

        app.page_provider_selection(-1);
        assert_eq!(app.providers.selection.index(), 1);
        assert_eq!(app.selected_provider_id().as_deref(), Some("provider-01"));

        app.page_provider_selection(-1);
        assert_eq!(app.providers.selection.index(), 0);
        assert_eq!(app.selected_provider_id().as_deref(), Some("provider-00"));
    }

    #[test]
    fn apply_provider_updates_applied_provider_id() {
        let dir = tempdir().unwrap();
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig {
                    model: Some("gpt-5.5".to_string()),
                    reasoning_effort: None,
                    plan_reasoning_effort: None,
                    api_key: Some("sk-test".to_string()),
                    env_key: None,
                    base_url: "https://api.example.test/v1".to_string(),
                    wire_api: "responses".to_string(),
                    auth_mode: ProviderAuthMode::ApiKey,
                },
            )
            .unwrap();
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            dir.path().join("providers.toml"),
            dir.path().join("config.toml"),
            dir.path().join("sessions"),
        );
        app.providers
            .set_current_codex_model(Some("gpt-5.6-sol".to_string()));

        app.apply_provider("switcher");

        assert_eq!(
            app.providers.applied_provider_id.as_deref(),
            Some("switcher")
        );
        assert_eq!(app.providers.current_codex_model(), Some("gpt-5.5"));
        assert_eq!(app.status, "Applied provider 'switcher' to Codex config.");
    }

    #[test]
    fn apply_provider_with_empty_model_preserves_current_codex_model() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "model = \"gpt-5.6-sol\"\n").unwrap();
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig {
                    model: None,
                    reasoning_effort: Some("ultra".to_string()),
                    plan_reasoning_effort: Some("max".to_string()),
                    api_key: Some("sk-test".to_string()),
                    env_key: None,
                    base_url: "https://api.example.test/v1".to_string(),
                    wire_api: "responses".to_string(),
                    auth_mode: ProviderAuthMode::ApiKey,
                },
            )
            .unwrap();
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            dir.path().join("providers.toml"),
            config_path,
            dir.path().join("sessions"),
        );
        app.providers
            .set_current_codex_model(Some("gpt-5.6-sol".to_string()));
        app.providers.set_model_catalog(model_catalog());

        app.apply_provider("switcher");

        assert_eq!(app.providers.current_codex_model(), Some("gpt-5.6-sol"));
        assert_eq!(
            app.providers.applied_provider_id.as_deref(),
            Some("switcher")
        );
    }
}
