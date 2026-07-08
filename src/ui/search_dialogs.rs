use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    app::{App, CurrentDirMatcher, Scope},
    session_store::{matches_search, search_terms},
};

use super::{
    conversation::{conversation_matches_role, conversation_matches_search},
    input_view::{input_cursor_position, input_line},
    layout::centered_rect,
};

pub(super) fn draw_session_search_dialog(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let popup = centered_rect(58, 24, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        input_line(
            "Query: ",
            app.session_state.search().draft().as_str(),
            app.session_state.search().draft().cursor(),
        ),
        Line::raw(""),
        Line::from(format!(
            "Current matches: {} (applies after Enter)",
            session_search_match_count(app, app.session_state.search().draft().as_str())
        )),
        Line::raw(""),
        Line::styled(
            "Left/Right moves. Home/End jumps. Backspace/Delete edits. Ctrl+U clears. Enter applies. Esc cancels.",
            Style::default().fg(Color::Gray),
        ),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("Search").borders(Borders::ALL)),
        popup,
    );
    frame.set_cursor_position(input_cursor_position(
        popup,
        "Query: ",
        app.session_state.search().draft().as_str(),
        app.session_state.search().draft().cursor(),
    ));
}

fn session_search_match_count(app: &App, query: &str) -> usize {
    let selected_provider = app.session_state.provider_tabs().selected_provider();
    let terms = search_terms(query);
    let current_dir_matcher = match app.session_state.scope() {
        Scope::CurrentDir => Some(CurrentDirMatcher::new(app.session_state.current_dir())),
        Scope::All => None,
    };
    app.session_state
        .items()
        .iter()
        .filter(|session| {
            current_dir_matcher
                .as_ref()
                .is_none_or(|matcher| matcher.matches(&session.cwd))
        })
        .filter(|session| {
            selected_provider.is_none_or(|provider| provider == session.provider.as_str())
        })
        .filter(|session| matches_search(session, &terms))
        .count()
}

pub(super) fn draw_conversation_search_dialog(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: Rect,
) {
    let popup = centered_rect(58, 24, area);
    frame.render_widget(Clear, popup);

    let terms = search_terms(app.conversation.search().draft().as_str());
    let matches = app
        .conversation
        .messages()
        .iter()
        .filter(|message| conversation_matches_role(message, app.conversation.role_filter()))
        .filter(|message| conversation_matches_search(message, &terms))
        .count();
    let lines = vec![
        input_line(
            "Query: ",
            app.conversation.search().draft().as_str(),
            app.conversation.search().draft().cursor(),
        ),
        Line::raw(""),
        Line::from(format!("Current matches: {matches} (applies after Enter)")),
        Line::raw(""),
        Line::styled(
            "Left/Right moves. Home/End jumps. Backspace/Delete edits. Ctrl+U clears. Enter applies. Esc cancels.",
            Style::default().fg(Color::Gray),
        ),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title("Conversation Search")
                .borders(Borders::ALL),
        ),
        popup,
    );
    frame.set_cursor_position(input_cursor_position(
        popup,
        "Query: ",
        app.conversation.search().draft().as_str(),
        app.conversation.search().draft().cursor(),
    ));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{provider_config::ProviderRegistry, session_store::Session};
    use tempfile::tempdir;

    fn test_session(id: &str, cwd: PathBuf, provider: &str, summary: &str) -> Session {
        Session {
            kind: crate::session_store::SessionKind::Codex,
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
    fn session_search_match_count_respects_scope_provider_and_terms() {
        let current_dir = PathBuf::from("/repo/current");
        let sessions = vec![
            test_session("1", current_dir.clone(), "alpha", "first request"),
            test_session("2", current_dir.clone(), "beta", "second request"),
            test_session("3", PathBuf::from("/repo/other"), "alpha", "third request"),
        ];
        let mut app = app_with_sessions(sessions, current_dir);

        assert_eq!(session_search_match_count(&app, "request"), 2);
        app.switch_provider_tab(1);
        assert_eq!(session_search_match_count(&app, "request"), 1);
        app.toggle_scope();
        assert_eq!(session_search_match_count(&app, "third"), 1);
    }

    #[cfg(unix)]
    #[test]
    fn session_search_match_count_matches_symlink_equivalent_current_dir() {
        let dir = tempdir().unwrap();
        let real_project = dir.path().join("real-project");
        let linked_project = dir.path().join("linked-project");
        std::fs::create_dir(&real_project).unwrap();
        std::os::unix::fs::symlink(&real_project, &linked_project).unwrap();
        let sessions = vec![test_session("1", real_project, "alpha", "first request")];
        let app = app_with_sessions(sessions, linked_project);

        assert_eq!(session_search_match_count(&app, "request"), 1);
    }

    #[test]
    fn session_search_match_count_excludes_different_missing_paths() {
        let dir = tempdir().unwrap();
        let current_dir = dir.path().join("missing");
        let session_cwd = current_dir.join(".");
        let sessions = vec![test_session("1", session_cwd, "alpha", "first request")];
        let app = app_with_sessions(sessions, current_dir);

        assert_eq!(session_search_match_count(&app, "request"), 0);
    }
}
