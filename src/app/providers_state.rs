use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use ratatui::widgets::TableState;

use crate::claude_store::ClaudeStatus;
use crate::provider_config::{ModelCatalog, ProviderConfig, ProviderRegistry};

use super::{ProviderEditor, TableSelection, model_fetch::ModelFetchTask};

pub struct ProvidersState {
    pub(super) registry: ProviderRegistry,
    pub(super) applied_provider_id: Option<String>,
    pub(super) config_path: PathBuf,
    pub(super) codex_config_path: PathBuf,
    pub(super) selection: TableSelection,
    pub(super) editor: Option<ProviderEditor>,
    pub(super) model_fetch_task: Option<ModelFetchTask>,
    pub(super) claude_status: Option<ClaudeStatus>,
    pub(super) model_catalog: Arc<ModelCatalog>,
}

impl ProvidersState {
    pub(crate) fn new(
        registry: ProviderRegistry,
        config_path: PathBuf,
        codex_config_path: PathBuf,
    ) -> Self {
        Self {
            registry,
            applied_provider_id: None,
            config_path,
            codex_config_path,
            selection: TableSelection::default(),
            editor: None,
            model_fetch_task: None,
            claude_status: None,
            model_catalog: Arc::new(ModelCatalog::default()),
        }
    }

    pub(crate) fn set_claude_status(&mut self, status: Option<ClaudeStatus>) {
        self.claude_status = status;
    }

    pub(crate) const fn claude_status(&self) -> Option<&ClaudeStatus> {
        self.claude_status.as_ref()
    }

    pub(crate) fn model_catalog(&self) -> Arc<ModelCatalog> {
        Arc::clone(&self.model_catalog)
    }

    pub(crate) fn set_model_catalog(&mut self, catalog: ModelCatalog) {
        self.model_catalog = Arc::new(catalog);
    }

    pub(crate) const fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    pub(crate) fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub(super) fn codex_config_path(&self) -> &Path {
        &self.codex_config_path
    }

    pub(crate) fn provider_count(&self) -> usize {
        self.registry.providers.len()
    }

    pub(crate) fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.registry.providers.get(id)
    }

    pub(crate) fn is_applied(&self, id: &str) -> bool {
        self.applied_provider_id.as_deref() == Some(id)
    }

    pub(crate) const fn selection_state_mut(&mut self) -> &mut TableState {
        self.selection.state_mut()
    }

    pub(crate) const fn editor(&self) -> Option<&ProviderEditor> {
        self.editor.as_ref()
    }

    pub(crate) const fn editor_mut(&mut self) -> Option<&mut ProviderEditor> {
        self.editor.as_mut()
    }

    #[cfg(test)]
    pub(crate) fn set_editor(&mut self, editor: Option<ProviderEditor>) {
        self.editor = editor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_shared_model_catalog() {
        let mut state = ProvidersState::new(
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
        );
        let catalog = ModelCatalog::from_json(
            r#"{"models":[{"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"max"}]}]}"#,
        )
        .unwrap();
        state.set_model_catalog(catalog);
        assert_eq!(
            state
                .model_catalog()
                .profile_for(Some("gpt-5.6-sol"))
                .default_effort(),
            "low"
        );
    }
}
