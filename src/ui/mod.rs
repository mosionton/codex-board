use std::io;

use anyhow::{Context, Result};
use crossterm::{
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
#[cfg(test)]
use ratatui::{layout::Rect, text::Line};
#[cfg(test)]
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
use crate::app::ConversationRoleFilter;
use crate::app::{App, AppAction, Overlay, Page};

mod chrome;
mod conversation;
mod details;
mod input;
mod input_view;
mod layout;
mod overlay_input;
mod page_input;
mod provider_editor_input;
mod provider_editor_view;
mod search_dialogs;
mod tables;
mod text_input;

use chrome::{
    draw_confirmation_dialog, draw_empty_sessions_message, draw_error_dialog, draw_footer,
    draw_header, draw_page_list,
};
use conversation::draw_conversation_dialog;
#[cfg(test)]
use conversation::{
    conversation_lines, conversation_matches_role, conversation_matches_search, conversation_title,
    filtered_conversation,
};
use details::draw_details_dialog;
#[cfg(test)]
use details::{
    detail_lines, provider_display_items, selected_provider_details, selected_session_details,
};
use input::event_loop;
#[cfg(test)]
use layout::{
    centered_rect, centered_rect_size, compact_path, details_dialog_height, percent_len,
    split_word_by_width, wrap_text,
};
use provider_editor_view::draw_provider_editor;
use search_dialogs::{draw_conversation_search_dialog, draw_session_search_dialog};
#[cfg(test)]
use tables::PROVIDER_DISPLAY_LABELS;
use tables::{draw_providers, draw_sessions};

pub(super) fn run_tui(app: &mut App) -> Result<AppAction> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;
    let keyboard_enhancement_enabled = enable_keyboard_enhancement(terminal.backend_mut());

    let result = event_loop(&mut terminal, app);

    if keyboard_enhancement_enabled {
        disable_keyboard_enhancement(terminal.backend_mut());
    }
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

fn enable_keyboard_enhancement(backend: &mut CrosstermBackend<io::Stdout>) -> bool {
    execute!(
        backend,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
    .is_ok()
}

fn disable_keyboard_enhancement(backend: &mut CrosstermBackend<io::Stdout>) {
    execute!(backend, PopKeyboardEnhancementFlags).ok();
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(4),
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(20)])
        .split(chunks[1]);

    draw_page_list(frame, app, body[0]);
    match app.page {
        Page::Sessions => draw_sessions(frame, app, body[1]),
        Page::Providers => draw_providers(frame, app, body[1]),
    }
    draw_footer(frame, app, chunks[2]);

    draw_empty_sessions_message(frame, app, area);

    if app.overlay == Some(Overlay::SessionSearch) {
        draw_session_search_dialog(frame, app, area);
    }
    if matches!(
        app.overlay,
        Some(Overlay::Conversation | Overlay::ConversationSearch)
    ) {
        draw_conversation_dialog(frame, app, area);
    }
    if app.overlay == Some(Overlay::ConversationSearch) {
        draw_conversation_search_dialog(frame, app, area);
    }
    if app.overlay == Some(Overlay::ProviderEditor)
        && let Some(editor) = app.providers.editor()
    {
        draw_provider_editor(frame, editor, area);
    }
    if app.overlay == Some(Overlay::Confirmation) {
        draw_confirmation_dialog(frame, app, area);
    }
    if app.overlay == Some(Overlay::Details) {
        draw_details_dialog(frame, app, area);
    }
    if let Some(error) = app.error.as_deref() {
        draw_error_dialog(frame, error, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        provider_config::{ProviderAuthMode, ProviderConfig, ProviderRegistry},
        session_store::{ConversationEntry, Session},
    };
    use std::path::{Path, PathBuf};

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

    fn test_message(role: &str, timestamp: &str, text: &str) -> ConversationEntry {
        ConversationEntry {
            timestamp: timestamp.to_string(),
            role: role.to_string(),
            text: text.to_string(),
        }
    }

    fn app_with_sessions_and_registry(
        sessions: Vec<Session>,
        current_dir: PathBuf,
        provider_registry: ProviderRegistry,
    ) -> App {
        App::new(
            sessions,
            current_dir,
            provider_registry,
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        )
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn details_dialog_height_tracks_content_lines() {
        assert_eq!(details_dialog_height(1, 40), 3);
        assert_eq!(details_dialog_height(8, 40), 10);
    }

    #[test]
    fn details_dialog_height_is_capped_to_available_area() {
        assert_eq!(details_dialog_height(100, 40), 38);
        assert_eq!(details_dialog_height(100, 2), 2);
    }

    #[test]
    fn wrap_text_uses_terminal_display_width() {
        let text = "1.实现tui界面展示当前目录下不同提供商的sessions使codex resume到对应的session上. 2.实现codex切换不同的供应商配置,确保能切换到login auth方式";
        let lines = wrap_text(text, 48);

        assert!(lines.len() > 1);
        assert_eq!(strip_whitespace(&lines.join("")), strip_whitespace(text));
        assert!(
            lines
                .iter()
                .all(|line| UnicodeWidthStr::width(line.as_str()) <= 48)
        );
    }

    fn strip_whitespace(text: &str) -> String {
        text.chars().filter(|ch| !ch.is_whitespace()).collect()
    }

    #[test]
    fn compact_path_limits_display_width() {
        let path = Path::new("/backup/codes/workspaces/很长很长很长很长很长很长很长/project");
        let compact = compact_path(path);

        assert!(compact.starts_with('…'));
        assert!(UnicodeWidthStr::width(compact.as_str()) <= 52);
    }

    #[test]
    fn provider_display_items_keep_readable_order() {
        let provider = ProviderConfig {
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: Some("high".to_string()),
            plan_reasoning_effort: None,
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };

        let items = provider_display_items("switcher", &provider, true);
        let labels = items.iter().map(|(label, _)| *label).collect::<Vec<_>>();

        assert_eq!(labels, PROVIDER_DISPLAY_LABELS);
        assert_eq!(items[0].1, "switcher");
        assert_eq!(items[1].1, "applied");
        assert_eq!(items[2].1, "gpt-5.5");
        assert_eq!(items[3].1, "api_key");
        assert_eq!(items[8].1, "s******t");
    }

    #[test]
    fn filtered_conversation_honors_role_and_search_terms() {
        let messages = vec![
            test_message("user", "2026-06-24T00:00:01Z", "first request"),
            test_message("assistant", "2026-06-24T00:00:02Z", "second reply"),
            test_message("assistant", "2026-06-24T00:00:03Z", "third reply"),
        ];

        let filtered =
            filtered_conversation(&messages, "second reply", ConversationRoleFilter::Assistant);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].timestamp, "2026-06-24T00:00:02Z");
        assert!(conversation_matches_role(
            filtered[0],
            ConversationRoleFilter::Assistant
        ));
        assert!(conversation_matches_search(
            filtered[0],
            &["second".to_string(), "reply".to_string()]
        ));
    }

    #[test]
    fn conversation_title_and_lines_include_search_and_wrapped_messages() {
        let current_dir = PathBuf::from("/repo/current");
        let mut app =
            app_with_sessions_and_registry(Vec::new(), current_dir, ProviderRegistry::default());
        app.conversation
            .set_role_filter(ConversationRoleFilter::Assistant);
        app.conversation.search_mut().set_query("needle");

        assert_eq!(
            conversation_title("Conversation", &app),
            "Conversation | role: assistant | Ctrl+U clears search | search: needle"
        );

        let empty = conversation_lines(&[], 20);
        assert_eq!(line_text(&empty[0]), "No conversation messages found.");

        let message = test_message(
            "user",
            "2026-06-24T00:00:01Z",
            "one two three four five six seven",
        );
        let messages = vec![&message];
        let lines = conversation_lines(&messages, 18);

        assert!(line_text(&lines[0]).contains("1. user"));
        assert!(
            lines
                .iter()
                .skip(1)
                .all(|line| line_text(line).starts_with("    "))
        );
        assert!(lines.len() > 2);
    }

    #[test]
    fn selected_details_render_fallbacks_and_provider_values() {
        let current_dir = PathBuf::from("/repo/current");
        let app = app_with_sessions_and_registry(
            Vec::new(),
            current_dir.clone(),
            ProviderRegistry::default(),
        );
        assert_eq!(
            line_text(&selected_session_details(&app, 40)[0]),
            "No session selected."
        );
        assert_eq!(
            line_text(&selected_provider_details(&app, 40)[0]),
            "No provider selected."
        );

        let mut session =
            test_session("session-1", current_dir.clone(), "switcher", "summary text");
        session.thread_source = "subagent".to_string();
        session.parent_thread_id = Some("parent-1".to_string());
        session.agent_nickname = Some("Boole".to_string());
        session.agent_role = Some("worker".to_string());
        session.agent_depth = Some(1);
        let sessions = vec![session];
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig::new("https://api.example.test/v1", "responses")
                    .with_model("gpt-5.5"),
            )
            .unwrap();
        let app = app_with_sessions_and_registry(sessions, current_dir, registry);
        let session_lines = selected_session_details(&app, 40);
        let provider_lines = selected_provider_details(&app, 40);

        assert!(
            session_lines
                .iter()
                .any(|line| line_text(line).contains("session-1"))
        );
        let session_detail_text = session_lines
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(session_detail_text.contains("source"));
        assert!(session_detail_text.contains("subagent"));
        assert!(session_detail_text.contains("parent"));
        assert!(session_detail_text.contains("parent-1"));
        assert!(session_detail_text.contains("agent"));
        assert!(session_detail_text.contains("Boole"));
        assert!(session_detail_text.contains("role"));
        assert!(session_detail_text.contains("worker"));
        assert!(session_detail_text.contains("depth"));
        assert!(session_detail_text.contains("1"));
        assert!(
            provider_lines
                .iter()
                .any(|line| line_text(line).contains("gpt-5.5"))
        );
    }

    #[test]
    fn detail_lines_and_split_word_by_width_wrap_cleanly() {
        let lines = detail_lines([("label", "alpha beta gamma delta".to_string())], 18);

        assert!(lines.len() > 1);
        assert!(line_text(&lines[0]).starts_with("label: "));
        assert!(line_text(&lines[1]).starts_with("       "));

        let split = split_word_by_width("你好abcdef", 4);
        assert!(split.len() > 1);
        assert_eq!(split.join(""), "你好abcdef");
        assert!(
            split
                .iter()
                .all(|part| UnicodeWidthStr::width(part.as_str()) <= 4)
        );
    }

    #[test]
    fn layout_helpers_respect_bounds() {
        assert_eq!(percent_len(40, 25), 10);
        assert_eq!(
            centered_rect_size(50, 30, Rect::new(10, 5, 20, 8)),
            Rect::new(10, 5, 20, 8)
        );
        assert_eq!(
            centered_rect(50, 50, Rect::new(0, 0, 100, 40)),
            Rect::new(25, 10, 50, 20)
        );
    }
}
