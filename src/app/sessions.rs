use std::time::Duration;

use crate::session_store::{Session, load_sessions};

use super::{App, ConfirmationAction, Overlay, ensure_session_cwd_exists};

impl App {
    pub(crate) fn refresh_visible(&mut self) {
        self.session_state.refresh_visible();
    }

    pub(crate) fn select_visible_session_by_id(&mut self, session_id: &str) {
        self.session_state.select_visible_session_by_id(session_id);
    }

    pub(crate) fn selected_session(&self) -> Option<&Session> {
        self.session_state.selected_session()
    }

    pub(crate) const fn move_selection(&mut self, delta: isize) {
        self.session_state.move_selection(delta);
    }

    pub(crate) fn reload_sessions(&mut self) {
        let selected_id = self.selected_session().map(|session| session.id.clone());
        let selected_provider = self.session_state.selected_provider_label_owned();

        match load_sessions(self.session_state.sessions_dir()) {
            Ok(sessions) => {
                self.session_state.replace_items(sessions);
                self.session_state
                    .rebuild_provider_tabs_preserving_label(selected_provider.as_deref());
                self.session_state.reset_selection();
                self.refresh_visible();
                if let Some(session_id) = selected_id {
                    self.select_visible_session_by_id(&session_id);
                }
                self.show_transient_status(
                    format!("Reloaded {} sessions.", self.session_state.items.len()),
                    Duration::from_secs(1),
                );
            }
            Err(err) => self.show_error(format!("Failed to reload sessions: {err}")),
        }
    }

    pub(crate) fn page_selection(&mut self, delta: isize) {
        self.session_state.page_selection(delta * 10);
    }

    pub(crate) fn prompt_resume_selected_session(&mut self) {
        let Some(session) = self.selected_session().cloned() else {
            self.show_error("No session selected.");
            return;
        };
        if let Err(err) = ensure_session_cwd_exists(&session.cwd) {
            self.show_error(format!("Cannot resume session: {err}"));
            return;
        }
        self.confirmation = Some(ConfirmationAction::ResumeSession(session));
        self.overlay = Some(Overlay::Confirmation);
        self.clear_status();
    }

    pub(crate) fn switch_provider_tab(&mut self, delta: isize) {
        self.session_state.move_provider_tab(delta);
        self.session_state.reset_selection();
        self.refresh_visible();
    }

    pub(crate) fn toggle_scope(&mut self) {
        self.session_state.toggle_scope();
    }

    pub(crate) fn toggle_session_view_mode(&mut self) {
        self.session_state.toggle_view_mode();
        self.session_state.reset_selection();
        self.refresh_visible();
    }

    pub(crate) fn clear_session_search(&mut self) {
        self.session_state.clear_search();
        self.clear_status();
    }

    pub(crate) fn open_session_search(&mut self) {
        self.session_state.search.reset_draft();
        self.overlay = Some(Overlay::SessionSearch);
        self.clear_status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Scope, char_count};
    use std::path::PathBuf;

    use crate::provider_config::ProviderRegistry;

    fn test_session(id: &str, cwd: PathBuf, provider: &str, summary: &str) -> Session {
        Session {
            id: id.to_string(),
            cwd,
            provider: provider.to_string(),
            model: None,
            timestamp: "2026-06-24T00:00:00Z".to_string(),
            summary: summary.to_string(),
            file: PathBuf::from(format!("{id}.jsonl")),
            thread_source: "user".to_string(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_depth: None,
        }
    }

    fn app_with_sessions(sessions: Vec<Session>, current_dir: PathBuf) -> App {
        App::new(
            sessions,
            current_dir,
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        )
    }

    #[test]
    fn refresh_visible_filters_by_scope_provider_and_search() {
        let current_dir = PathBuf::from("/repo/current");
        let sessions = vec![
            test_session("1", current_dir.clone(), "alpha", "first request"),
            test_session("2", current_dir.clone(), "beta", "second request"),
            test_session("3", PathBuf::from("/repo/other"), "alpha", "third request"),
        ];
        let mut app = app_with_sessions(sessions, current_dir);

        app.session_state.provider_tabs.set_selected_index(99);
        app.session_state.search.set_query("second");
        app.refresh_visible();

        assert_eq!(app.session_state.provider_tabs.selected_index(), 0);
        assert_eq!(app.session_state.visible_indices.len(), 1);
        assert_eq!(app.selected_session().unwrap().id, "2");
        assert_eq!(app.session_state.selection.state().selected(), Some(0));
    }

    #[test]
    fn select_visible_session_by_id_updates_selection_and_ignores_missing_ids() {
        let current_dir = PathBuf::from("/repo/current");
        let sessions = vec![
            test_session("1", current_dir.clone(), "alpha", "first"),
            test_session("2", current_dir.clone(), "alpha", "second"),
        ];
        let mut app = app_with_sessions(sessions, current_dir);

        app.select_visible_session_by_id("2");
        assert_eq!(app.session_state.selection.index(), 1);
        assert_eq!(app.session_state.selection.state().selected(), Some(1));

        app.select_visible_session_by_id("missing");
        assert_eq!(app.session_state.selection.index(), 1);
    }

    #[test]
    fn move_selection_switch_provider_tab_and_toggle_scope_wrap_and_reset_selection() {
        let current_dir = PathBuf::from("/repo/current");
        let sessions = vec![
            test_session("1", current_dir.clone(), "alpha", "first"),
            test_session("2", current_dir.clone(), "beta", "second"),
            test_session("3", PathBuf::from("/repo/other"), "alpha", "third"),
        ];
        let mut app = app_with_sessions(sessions, current_dir);

        app.move_selection(-1);
        assert_eq!(app.selected_session().unwrap().id, "2");

        app.switch_provider_tab(1);
        assert_eq!(app.session_state.provider_tabs.selected_index(), 1);
        assert_eq!(app.session_state.selection.index(), 0);
        assert_eq!(app.session_state.visible_indices.len(), 1);
        assert_eq!(app.selected_session().unwrap().provider, "alpha");

        app.toggle_scope();
        assert_eq!(app.session_state.scope, Scope::All);
        assert_eq!(app.session_state.selection.index(), 0);
        assert_eq!(app.session_state.visible_indices.len(), 2);
    }

    #[test]
    fn page_selection_clamps_at_visible_boundaries() {
        let current_dir = PathBuf::from("/repo/current");
        let sessions = (0..12)
            .map(|index| {
                test_session(
                    &format!("session-{index:02}"),
                    current_dir.clone(),
                    "alpha",
                    "summary",
                )
            })
            .collect();
        let mut app = app_with_sessions(sessions, current_dir);

        app.page_selection(-1);
        assert_eq!(app.session_state.selection.index(), 0);

        app.page_selection(1);
        assert_eq!(app.session_state.selection.index(), 10);

        app.page_selection(1);
        assert_eq!(app.session_state.selection.index(), 11);

        app.page_selection(-1);
        assert_eq!(app.session_state.selection.index(), 1);

        app.page_selection(-1);
        assert_eq!(app.session_state.selection.index(), 0);
    }

    #[test]
    fn session_search_workflow_resets_state_and_overlay() {
        let current_dir = PathBuf::from("/repo/current");
        let mut app = app_with_sessions(
            vec![test_session(
                "1",
                current_dir.clone(),
                "alpha",
                "first request",
            )],
            current_dir,
        );
        app.status = "busy".to_string();
        app.session_state.search.set_query("first");

        app.open_session_search();
        assert_eq!(app.overlay, Some(Overlay::SessionSearch));
        assert_eq!(app.session_state.search.draft().as_str(), "first");
        assert_eq!(
            app.session_state.search.draft().cursor(),
            char_count("first")
        );
        assert_eq!(app.status, "");

        app.clear_session_search();
        assert_eq!(app.session_state.search.query(), "");
        assert_eq!(app.session_state.search.draft().as_str(), "");
        assert_eq!(app.session_state.search.draft().cursor(), 0);
        assert_eq!(app.session_state.selection.index(), 0);
    }
}
