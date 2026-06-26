use std::time::Duration;

use crate::session_store::load_session_conversation;

use super::{App, Overlay};

impl App {
    pub(crate) fn open_conversation_search(&mut self) {
        self.conversation.search_mut().reset_draft();
        self.overlay = Some(Overlay::ConversationSearch);
        self.clear_status();
    }

    pub(crate) fn apply_conversation_search(&mut self) {
        self.conversation.apply_search();
        self.overlay = Some(Overlay::Conversation);
        self.clear_status();
    }

    pub(crate) fn clear_conversation_search(&mut self) {
        self.conversation.clear_search();
        self.clear_status();
    }

    pub(crate) fn cycle_conversation_role_filter(&mut self) {
        self.conversation.cycle_role_filter();
        self.clear_status();
    }

    pub(crate) fn open_details(&mut self) {
        self.details_scroll.reset();
        self.overlay = Some(Overlay::Details);
    }

    pub(crate) fn open_conversation(&mut self) {
        let Some(session) = self.selected_session() else {
            self.show_error("No session selected.");
            return;
        };
        let path = session.file.clone();
        match load_session_conversation(&path) {
            Ok(conversation) => {
                self.conversation.replace_for_new_session(conversation);
                self.overlay = Some(Overlay::Conversation);
                self.clear_status();
            }
            Err(err) => {
                self.show_error(format!("Failed to load conversation: {err}"));
            }
        }
    }

    pub(crate) fn reload_conversation(&mut self) {
        let Some(session) = self.selected_session() else {
            self.show_error("No session selected.");
            return;
        };
        let path = session.file.clone();
        match load_session_conversation(&path) {
            Ok(conversation) => {
                let count = conversation.len();
                self.conversation.replace_preserving_filters(conversation);
                self.show_transient_status(
                    format!("Reloaded {count} conversation messages."),
                    Duration::from_secs(1),
                );
            }
            Err(err) => {
                self.show_error(format!("Failed to reload conversation: {err}"));
            }
        }
    }

    pub(crate) fn scroll_details(&mut self, delta: i16) {
        self.details_scroll.scroll_by(delta);
    }

    pub(crate) fn scroll_conversation(&mut self, delta: i16) {
        self.conversation.scroll_by(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{ConversationRoleFilter, char_count},
        provider_config::ProviderRegistry,
        session_store::Session,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

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

    fn test_session_with_file(
        id: &str,
        cwd: PathBuf,
        provider: &str,
        summary: &str,
        file: PathBuf,
    ) -> Session {
        Session {
            file,
            ..test_session(id, cwd, provider, summary)
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

    fn write_conversation_file(path: &std::path::Path) {
        std::fs::write(
            path,
            r#"{"timestamp":"2026-06-24T00:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"first request"}]}}"#
                .to_string()
                + "\n"
                + r#"{"timestamp":"2026-06-24T00:00:03Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"first response"}]}}"#
                + "\n",
        )
        .unwrap();
    }

    #[test]
    fn conversation_search_and_role_filter_workflow_reset_scroll() {
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
        app.conversation.search_mut().set_query("first");
        app.conversation.search_mut().draft_mut().set("draft");
        app.conversation.scroll_mut().set(7);
        app.status = "busy".to_string();

        app.open_conversation_search();
        assert_eq!(app.overlay, Some(Overlay::ConversationSearch));
        assert_eq!(app.conversation.search().draft().as_str(), "first");
        assert_eq!(
            app.conversation.search().draft().cursor(),
            char_count("first")
        );
        assert_eq!(app.status, "");

        app.conversation.search_mut().draft_mut().set("assistant");
        app.apply_conversation_search();
        assert_eq!(app.conversation.search().query(), "assistant");
        assert_eq!(
            app.conversation.search().draft().cursor(),
            char_count("assistant")
        );
        assert_eq!(app.conversation.scroll().offset(), 0);
        assert_eq!(app.overlay, Some(Overlay::Conversation));

        app.conversation.scroll_mut().set(5);
        app.cycle_conversation_role_filter();
        assert_eq!(app.conversation.role_filter(), ConversationRoleFilter::All);
        assert_eq!(app.conversation.scroll().offset(), 0);

        app.conversation.search_mut().draft_mut().set("stale");
        app.clear_conversation_search();
        assert_eq!(app.conversation.search().query(), "");
        assert_eq!(app.conversation.search().draft().as_str(), "");
        assert_eq!(app.conversation.search().draft().cursor(), 0);
    }

    #[test]
    fn open_details_resets_scroll_and_sets_overlay() {
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
        app.details_scroll.set(9);

        app.open_details();

        assert_eq!(app.details_scroll.offset(), 0);
        assert_eq!(app.overlay, Some(Overlay::Details));
    }

    #[test]
    fn open_conversation_loads_messages_and_resets_filters() {
        let dir = tempdir().unwrap();
        let current_dir = dir.path().join("project");
        let session_file = dir.path().join("session.jsonl");
        std::fs::create_dir(&current_dir).unwrap();
        write_conversation_file(&session_file);
        let session = test_session_with_file(
            "1",
            current_dir.clone(),
            "alpha",
            "first request",
            session_file,
        );
        let mut app = app_with_sessions(vec![session], current_dir);
        app.conversation.search_mut().set_query("keep");
        app.conversation
            .search_mut()
            .draft_mut()
            .set_with_cursor("draft", 4);
        app.conversation
            .set_role_filter(ConversationRoleFilter::Assistant);
        app.conversation.scroll_mut().set(7);
        app.status = "busy".to_string();

        app.open_conversation();

        assert_eq!(app.conversation.messages().len(), 2);
        assert_eq!(app.conversation.scroll().offset(), 0);
        assert_eq!(app.conversation.search().query(), "");
        assert_eq!(app.conversation.search().draft().as_str(), "");
        assert_eq!(app.conversation.search().draft().cursor(), 0);
        assert_eq!(app.conversation.role_filter(), ConversationRoleFilter::User);
        assert_eq!(app.overlay, Some(Overlay::Conversation));
        assert_eq!(app.status, "");
    }

    #[test]
    fn open_conversation_reports_load_errors() {
        let current_dir = PathBuf::from("/repo/current");
        let session = test_session_with_file(
            "1",
            current_dir.clone(),
            "alpha",
            "first request",
            PathBuf::from("/definitely/missing.jsonl"),
        );
        let mut app = app_with_sessions(vec![session], current_dir);

        app.open_conversation();

        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("Failed to load conversation"))
        );
    }

    #[test]
    fn scroll_helpers_saturate_in_both_directions() {
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

        app.scroll_details(-10);
        assert_eq!(app.details_scroll.offset(), 0);
        app.scroll_details(3);
        assert_eq!(app.details_scroll.offset(), 3);

        app.scroll_conversation(-4);
        assert_eq!(app.conversation.scroll().offset(), 0);
        app.scroll_conversation(6);
        assert_eq!(app.conversation.scroll().offset(), 6);
    }
}
