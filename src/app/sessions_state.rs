use std::path::{Path, PathBuf};

use ratatui::widgets::TableState;

use crate::session_store::{Session, matches_search, search_terms};

use super::{ProviderTabs, Scope, SearchState, TableSelection};

pub struct SessionsState {
    pub(super) items: Vec<Session>,
    pub(super) provider_tabs: ProviderTabs,
    pub(super) selection: TableSelection,
    pub(super) visible_indices: Vec<usize>,
    pub(super) search: SearchState,
    pub(super) scope: Scope,
    pub(super) current_dir: PathBuf,
    pub(super) sessions_dir: PathBuf,
}

impl SessionsState {
    pub(crate) fn new(items: Vec<Session>, current_dir: PathBuf, sessions_dir: PathBuf) -> Self {
        let provider_tabs = ProviderTabs::new(&items, Scope::CurrentDir, &current_dir);
        Self {
            items,
            provider_tabs,
            selection: TableSelection::default(),
            visible_indices: Vec::new(),
            search: SearchState::default(),
            scope: Scope::CurrentDir,
            current_dir,
            sessions_dir,
        }
    }

    pub(crate) fn items(&self) -> &[Session] {
        &self.items
    }

    pub(crate) fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    pub(crate) fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    pub(crate) const fn scope(&self) -> Scope {
        self.scope
    }

    pub(crate) const fn provider_tabs(&self) -> &ProviderTabs {
        &self.provider_tabs
    }

    pub(crate) const fn move_provider_tab(&mut self, delta: isize) {
        self.provider_tabs.move_by(delta);
    }

    pub(crate) fn selected_provider_label_owned(&self) -> Option<String> {
        self.provider_tabs.selected_label_owned()
    }

    pub(crate) const fn search(&self) -> &SearchState {
        &self.search
    }

    pub(crate) const fn search_mut(&mut self) -> &mut SearchState {
        &mut self.search
    }

    pub(crate) const fn visible_len(&self) -> usize {
        self.visible_indices.len()
    }

    pub(crate) fn visible_session(&self, visible_index: usize) -> Option<&Session> {
        self.visible_indices
            .get(visible_index)
            .and_then(|index| self.items.get(*index))
    }

    pub(crate) fn selected_session(&self) -> Option<&Session> {
        self.visible_session(self.selection_index())
    }

    pub(crate) fn refresh_visible(&mut self) {
        self.provider_tabs
            .rebuild(&self.items, self.scope, &self.current_dir);

        let selected_provider = self.provider_tabs.selected_provider().map(str::to_string);
        let query_terms = search_terms(self.search.query());
        self.visible_indices = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, session)| match self.scope {
                Scope::CurrentDir => session.cwd == self.current_dir,
                Scope::All => true,
            })
            .filter(|(_, session)| {
                selected_provider
                    .as_deref()
                    .is_none_or(|provider| provider == session.provider.as_str())
            })
            .filter(|(_, session)| matches_search(session, &query_terms))
            .map(|(index, _)| index)
            .collect();

        self.selection.sync_len(self.visible_indices.len());
    }

    pub(crate) fn rebuild_provider_tabs_preserving_label(&mut self, selected_label: Option<&str>) {
        self.provider_tabs.rebuild_preserving_label(
            &self.items,
            self.scope,
            &self.current_dir,
            selected_label,
        );
    }

    pub(crate) fn replace_items(&mut self, items: Vec<Session>) {
        self.items = items;
    }

    pub(crate) fn select_visible_session_by_id(&mut self, session_id: &str) {
        if let Some(index) = self
            .visible_indices
            .iter()
            .position(|session_index| self.items[*session_index].id == session_id)
        {
            self.selection.select(index);
        }
    }

    pub(crate) const fn move_selection(&mut self, delta: isize) {
        self.selection.move_by(self.visible_indices.len(), delta);
    }

    pub(crate) fn page_selection(&mut self, delta: isize) {
        self.selection
            .move_by_clamped(self.visible_indices.len(), delta);
    }

    pub(crate) fn toggle_scope(&mut self) {
        self.scope = match self.scope {
            Scope::CurrentDir => Scope::All,
            Scope::All => Scope::CurrentDir,
        };
        self.selection.reset();
        self.refresh_visible();
    }

    pub(crate) fn clear_search(&mut self) {
        self.search.clear();
        self.selection.reset();
        self.refresh_visible();
    }

    pub(crate) const fn selection_index(&self) -> usize {
        self.selection.index()
    }

    pub(crate) const fn reset_selection(&mut self) {
        self.selection.reset();
    }

    #[cfg(test)]
    pub(crate) const fn select_index(&mut self, index: usize) {
        self.selection.select(index);
    }

    pub(crate) const fn selection_state_mut(&mut self) -> &mut TableState {
        self.selection.state_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session(id: &str, cwd: PathBuf, provider: &str, summary: &str) -> Session {
        Session {
            id: id.to_string(),
            cwd,
            provider: provider.to_string(),
            model: None,
            timestamp: "2026-06-24T00:00:00Z".to_string(),
            summary: summary.to_string(),
            file: PathBuf::from(format!("{id}.jsonl")),
        }
    }

    #[test]
    fn refresh_visible_filters_by_scope_provider_and_search() {
        let current_dir = PathBuf::from("/repo/current");
        let mut state = SessionsState::new(
            vec![
                test_session("1", current_dir.clone(), "alpha", "first request"),
                test_session("2", current_dir.clone(), "beta", "second request"),
                test_session("3", PathBuf::from("/repo/other"), "alpha", "third request"),
            ],
            current_dir,
            PathBuf::from("sessions"),
        );

        state.provider_tabs.set_selected_index(99);
        state.search_mut().set_query("second");
        state.refresh_visible();

        assert_eq!(state.provider_tabs().selected_index(), 0);
        assert_eq!(state.visible_len(), 1);
        assert_eq!(state.selected_session().unwrap().id, "2");
        assert_eq!(state.selection.state().selected(), Some(0));
    }
}
