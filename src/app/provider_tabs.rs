use std::{collections::BTreeSet, path::Path};

use crate::session_store::Session;

use super::{Scope, cycle_index};

const ALL_PROVIDERS_LABEL: &str = "All";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTabs {
    labels: Vec<String>,
    selected_index: usize,
}

impl ProviderTabs {
    pub(crate) fn new(sessions: &[Session], scope: Scope, current_dir: &Path) -> Self {
        let labels = build_labels(sessions, scope, current_dir);
        Self {
            labels,
            selected_index: 0,
        }
    }

    pub(crate) fn rebuild(&mut self, sessions: &[Session], scope: Scope, current_dir: &Path) {
        self.labels = build_labels(sessions, scope, current_dir);
        self.sync_selection();
    }

    pub(crate) fn rebuild_preserving_label(
        &mut self,
        sessions: &[Session],
        scope: Scope,
        current_dir: &Path,
        selected_label: Option<&str>,
    ) {
        self.labels = build_labels(sessions, scope, current_dir);
        self.selected_index = selected_label
            .and_then(|label| self.labels.iter().position(|item| item == label))
            .unwrap_or(0);
        self.sync_selection();
    }

    pub(crate) fn labels(&self) -> &[String] {
        &self.labels
    }

    pub(crate) const fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub(crate) fn selected_label(&self) -> Option<&str> {
        self.labels
            .get(self.selected_index)
            .map(std::string::String::as_str)
    }

    pub(crate) fn selected_label_owned(&self) -> Option<String> {
        self.labels.get(self.selected_index).cloned()
    }

    pub(crate) fn selected_provider(&self) -> Option<&str> {
        self.selected_label()
            .filter(|label| *label != ALL_PROVIDERS_LABEL)
    }

    pub(crate) const fn move_by(&mut self, delta: isize) {
        if self.labels.is_empty() {
            return;
        }
        self.selected_index = cycle_index(self.selected_index, self.labels.len(), delta);
    }

    #[cfg(test)]
    pub(crate) const fn set_selected_index(&mut self, index: usize) {
        self.selected_index = index;
    }

    const fn sync_selection(&mut self) {
        if self.selected_index >= self.labels.len() {
            self.selected_index = 0;
        }
    }
}

fn build_labels(sessions: &[Session], scope: Scope, current_dir: &Path) -> Vec<String> {
    let mut provider_tabs = BTreeSet::new();
    for session in sessions {
        if scope == Scope::CurrentDir && session.cwd != current_dir {
            continue;
        }
        provider_tabs.insert(session.provider.clone());
    }
    let mut tabs = vec![ALL_PROVIDERS_LABEL.to_string()];
    tabs.extend(provider_tabs);
    tabs
}
