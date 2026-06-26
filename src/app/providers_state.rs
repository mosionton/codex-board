use std::path::{Path, PathBuf};

use ratatui::widgets::TableState;

use crate::provider_config::{ProviderConfig, ProviderRegistry};

use super::{ProviderEditor, TableSelection, model_fetch::ModelFetchTask};

pub(crate) struct ProvidersState {
    pub(super) registry: ProviderRegistry,
    pub(super) applied_provider_id: Option<String>,
    pub(super) config_path: PathBuf,
    pub(super) codex_config_path: PathBuf,
    pub(super) selection: TableSelection,
    pub(super) editor: Option<ProviderEditor>,
    pub(super) model_fetch_task: Option<ModelFetchTask>,
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
        }
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

    pub(crate) fn selection_state_mut(&mut self) -> &mut TableState {
        self.selection.state_mut()
    }

    pub(crate) fn editor(&self) -> Option<&ProviderEditor> {
        self.editor.as_ref()
    }

    pub(crate) fn editor_mut(&mut self) -> Option<&mut ProviderEditor> {
        self.editor.as_mut()
    }

    #[cfg(test)]
    pub(crate) fn set_editor(&mut self, editor: Option<ProviderEditor>) {
        self.editor = editor;
    }
}
