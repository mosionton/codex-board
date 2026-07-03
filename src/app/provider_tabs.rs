use std::{collections::BTreeSet, path::Path};

use crate::session_store::Session;

use super::{CurrentDirMatcher, Scope, cycle_index};

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
    match scope {
        Scope::CurrentDir => {
            let current_dir_matcher = CurrentDirMatcher::new(current_dir);
            for session in sessions {
                if current_dir_matcher.matches(&session.cwd) {
                    provider_tabs.insert(session.provider.clone());
                }
            }
        }
        Scope::All => {
            for session in sessions {
                provider_tabs.insert(session.provider.clone());
            }
        }
    }
    let mut tabs = vec![ALL_PROVIDERS_LABEL.to_string()];
    tabs.extend(provider_tabs);
    tabs
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::*;

    fn test_session(id: &str, cwd: PathBuf, provider: &str) -> Session {
        Session {
            id: id.to_string(),
            cwd,
            provider: provider.to_string(),
            model: None,
            timestamp: "2026-06-24T00:00:00Z".to_string(),
            summary: "summary".to_string(),
            file: PathBuf::from(format!("{id}.jsonl")),
            thread_source: "user".to_string(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_depth: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn current_dir_tabs_include_symlink_equivalent_session_provider() {
        let dir = tempdir().unwrap();
        let real_project = dir.path().join("real-project");
        let linked_project = dir.path().join("linked-project");
        let other_project = dir.path().join("other");
        std::fs::create_dir(&real_project).unwrap();
        std::fs::create_dir(&other_project).unwrap();
        std::os::unix::fs::symlink(&real_project, &linked_project).unwrap();
        let sessions = vec![
            test_session("1", real_project, "alpha"),
            test_session("2", other_project, "beta"),
        ];

        assert_eq!(
            ProviderTabs::new(&sessions, Scope::CurrentDir, &linked_project).labels(),
            vec!["All".to_string(), "alpha".to_string()]
        );
    }

    #[test]
    fn current_dir_tabs_exclude_different_missing_paths() {
        let dir = tempdir().unwrap();
        let current_dir = dir.path().join("missing");
        let session_cwd = current_dir.join(".");
        let sessions = vec![test_session("1", session_cwd, "alpha")];

        assert_eq!(
            ProviderTabs::new(&sessions, Scope::CurrentDir, &current_dir).labels(),
            vec!["All".to_string()]
        );
    }
}
