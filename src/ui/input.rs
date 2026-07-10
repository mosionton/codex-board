use std::{io, time::Duration};

use anyhow::Result;
use crossterm::event::{self, Event};
#[cfg(test)]
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};

#[cfg(test)]
use crate::app::char_count;
use crate::app::{App, AppAction, Page};

use super::{
    draw,
    overlay_input::handle_overlay_key,
    page_input::{handle_providers_page_key, handle_sessions_page_key},
};
#[cfg(test)]
use super::{
    overlay_input::{
        handle_confirmation_key, handle_conversation_overlay_key, handle_conversation_search_key,
        handle_session_search_key,
    },
    provider_editor_input::handle_provider_editor_key,
};

pub(super) fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<AppAction> {
    loop {
        app.poll_model_fetch();
        app.clear_expired_status();
        app.clear_expired_error();
        terminal.draw(|frame| draw(frame, app))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if let Some(overlay) = app.overlay {
                handle_overlay_key(app, overlay, key);
                if let Some(action) = app.take_queued_action() {
                    return Ok(action);
                }
                continue;
            }

            match app.page {
                Page::Sessions => {
                    if let Some(action) = handle_sessions_page_key(app, key) {
                        return Ok(action);
                    }
                }
                Page::Providers => {
                    if let Some(action) = handle_providers_page_key(app, key) {
                        return Ok(action);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    use crate::{
        app::{
            ConfirmationAction, ConversationRoleFilter, Overlay, ProviderEditor, ProviderField,
            SessionViewMode,
        },
        provider_config::{
            DEFAULT_AUTO_COMPACT_PERCENT, ModelCatalog, ProviderAuthMode, ProviderConfig,
            ProviderRegistry,
        },
        session_store::Session,
    };
    use tempfile::tempdir;

    #[test]
    fn reload_key_is_plain_lowercase_r_only() {
        let mut app = app_with_registry(ProviderRegistry::default());

        handle_conversation_overlay_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.error, None);

        handle_conversation_overlay_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.error, None);
    }

    #[test]
    fn c_key_does_not_clear_session_search() {
        let mut app = app_with_registry(ProviderRegistry::default());
        app.session_state.search_mut().set_query("needle");

        let action = handle_sessions_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );

        assert!(action.is_none());
        assert_eq!(app.session_state.search().query(), "needle");
        assert_eq!(app.session_state.search().draft().as_str(), "needle");
        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("No session selected"))
        );
    }

    #[test]
    fn ctrl_i_does_not_act_like_tab_or_plain_i_when_disambiguated() {
        let mut app = app_with_registry(ProviderRegistry::default());

        let action = handle_sessions_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
        );

        assert!(action.is_none());
        assert_eq!(app.overlay, None);
    }

    #[test]
    fn t_key_switches_pages_without_number_aliases() {
        let mut app = app_with_registry(ProviderRegistry::default());

        let action = handle_sessions_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE),
        );

        assert!(action.is_none());
        assert_eq!(app.page, Page::Sessions);

        let action = handle_sessions_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        );

        assert!(action.is_none());
        assert_eq!(app.page, Page::Providers);

        let action = handle_providers_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE),
        );

        assert!(action.is_none());
        assert_eq!(app.page, Page::Providers);

        let action = handle_providers_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        );

        assert!(action.is_none());
        assert_eq!(app.page, Page::Sessions);
    }

    #[test]
    fn v_key_toggles_session_view_mode() {
        let mut app = app_with_registry(ProviderRegistry::default());
        assert_eq!(app.session_state.view_mode(), SessionViewMode::Tree);

        let action = handle_sessions_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
        );

        assert!(action.is_none());
        assert_eq!(app.session_state.view_mode(), SessionViewMode::Flat);
    }

    #[test]
    fn e_key_edits_provider() {
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig::new("https://api.example.test/v1", "responses"),
            )
            .unwrap();
        let mut app = app_with_registry(registry);
        app.set_page(Page::Providers);

        let action = handle_providers_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        );

        assert!(action.is_none());
        assert_eq!(app.overlay, Some(Overlay::ProviderEditor));
        assert_eq!(app.providers.editor().unwrap().id.as_str(), "switcher");
    }

    #[test]
    fn dialog_enter_and_i_no_longer_close_details() {
        let mut app = app_with_registry(ProviderRegistry::default());
        app.overlay = Some(Overlay::Details);

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert_eq!(app.overlay, Some(Overlay::Details));

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
        );
        assert_eq!(app.overlay, Some(Overlay::Details));

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert_eq!(app.overlay, None);
    }

    #[test]
    fn space_no_longer_cycles_provider_editor_options() {
        let mut app = app_with_registry(ProviderRegistry::default());
        let mut editor = ProviderEditor::new();
        editor.active_field = ProviderField::WireApi;
        let original_wire_api = editor.wire_api.clone();
        app.providers.set_editor(Some(editor));
        app.overlay = Some(Overlay::ProviderEditor);

        handle_provider_editor_key(
            &mut app,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );

        let editor = app.providers.editor().unwrap();
        assert_eq!(editor.wire_api, original_wire_api);
    }

    #[test]
    fn ctrl_i_does_not_advance_provider_editor_field_when_disambiguated() {
        let mut app = app_with_registry(ProviderRegistry::default());
        let editor = ProviderEditor::new();
        let original_field = editor.active_field;
        app.providers.set_editor(Some(editor));
        app.overlay = Some(Overlay::ProviderEditor);

        handle_provider_editor_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
        );

        let editor = app.providers.editor().unwrap();
        assert_eq!(editor.active_field, original_field);
    }

    #[test]
    fn esc_quits_without_search_and_clears_existing_search() {
        let mut app = app_with_registry(ProviderRegistry::default());

        assert!(matches!(
            handle_sessions_page_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(AppAction::Quit)
        ));

        app.session_state.search_mut().set_query("needle");

        let action =
            handle_sessions_page_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(action.is_none());
        assert_eq!(app.session_state.search().query(), "");
        assert_eq!(app.session_state.search().draft().as_str(), "");
        assert_eq!(app.session_state.search().draft().cursor(), 0);
    }

    #[test]
    fn details_overlay_navigation_updates_scroll_position() {
        let mut app = app_with_registry(ProviderRegistry::default());
        app.overlay = Some(Overlay::Details);
        app.details_scroll.set(5);

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        );
        assert_eq!(app.details_scroll.offset(), 4);

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        );
        assert_eq!(app.details_scroll.offset(), 0);

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        );
        assert_eq!(app.details_scroll.offset(), 1);

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        );
        assert_eq!(app.details_scroll.offset(), usize::MAX);

        handle_overlay_key(
            &mut app,
            Overlay::Details,
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
        );
        assert_eq!(app.details_scroll.offset(), 0);
    }

    #[test]
    fn session_search_enter_applies_draft_and_filters_visible_sessions() {
        let current_dir = PathBuf::from("/repo/current");
        let mut app = app_with_sessions(
            vec![
                test_session("session-1", current_dir.clone(), "alpha request"),
                test_session("session-2", current_dir.clone(), "beta request"),
            ],
            current_dir,
        );
        app.overlay = Some(Overlay::SessionSearch);
        app.session_state.search_mut().draft_mut().set("beta");
        app.session_state.select_index(1);

        handle_session_search_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.overlay, None);
        assert_eq!(app.session_state.search().query(), "beta");
        assert_eq!(
            app.session_state.search().draft().cursor(),
            char_count("beta")
        );
        assert_eq!(app.session_state.selection_index(), 0);
        assert_eq!(app.session_state.visible_len(), 1);
        assert_eq!(app.selected_session().unwrap().id, "session-2");
    }

    #[test]
    fn conversation_search_keys_apply_and_cancel_drafts() {
        let mut app = app_with_registry(ProviderRegistry::default());
        app.overlay = Some(Overlay::ConversationSearch);
        app.conversation.search_mut().set_query("old");
        app.conversation.search_mut().draft_mut().set("assistant");
        app.conversation.scroll_mut().set(8);

        handle_conversation_search_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.overlay, Some(Overlay::Conversation));
        assert_eq!(app.conversation.search().query(), "assistant");
        assert_eq!(app.conversation.scroll().offset(), 0);

        app.overlay = Some(Overlay::ConversationSearch);
        app.conversation.search_mut().draft_mut().set("ignored");

        handle_conversation_search_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.overlay, Some(Overlay::Conversation));
        assert_eq!(app.conversation.search().draft().as_str(), "assistant");
        assert_eq!(
            app.conversation.search().draft().cursor(),
            char_count("assistant")
        );
    }

    #[test]
    fn providers_page_shortcuts_open_editor_and_confirmation() {
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig::new("https://api.example.test/v1", "responses"),
            )
            .unwrap();
        let mut app = app_with_registry(registry);
        app.set_page(Page::Providers);

        let action = handle_providers_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        );
        assert!(action.is_none());
        assert_eq!(app.overlay, Some(Overlay::ProviderEditor));
        assert!(app.providers.editor().is_some());

        app.close_overlay();
        let action = handle_providers_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        );
        assert!(action.is_none());
        assert_eq!(app.overlay, Some(Overlay::ProviderEditor));
        assert_eq!(app.providers.editor().unwrap().id.as_str(), "switcher");

        app.close_overlay();
        let action = handle_providers_page_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        assert!(action.is_none());
        assert_eq!(app.overlay, Some(Overlay::Confirmation));
        assert_eq!(
            app.confirmation,
            Some(ConfirmationAction::ApplyProvider("switcher".to_string()))
        );
    }

    #[test]
    fn enter_no_longer_edits_provider() {
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig::new("https://api.example.test/v1", "responses"),
            )
            .unwrap();
        let mut app = app_with_registry(registry);
        app.set_page(Page::Providers);

        let action =
            handle_providers_page_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(action.is_none());
        assert_eq!(app.overlay, None);
        assert!(app.providers.editor().is_none());
    }

    #[test]
    fn provider_editor_key_routes_text_model_and_option_updates() {
        let mut app = app_with_registry(ProviderRegistry::default());
        let mut editor = ProviderEditor::new();
        editor.active_field = ProviderField::BaseUrl;
        app.providers.set_editor(Some(editor));

        handle_provider_editor_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        assert_eq!(app.providers.editor().unwrap().base_url.as_str(), "x");

        let editor = app.providers.editor_mut().unwrap();
        editor.active_field = ProviderField::WireApi;
        handle_provider_editor_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.providers.editor().unwrap().wire_api, "chat");

        let editor = app.providers.editor_mut().unwrap();
        editor.active_field = ProviderField::Model;
        editor.model.set("gpt-5-mini");
        editor.model_options = vec!["gpt-5-mini".to_string(), "gpt-5.5".to_string()];
        handle_provider_editor_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.providers.editor().unwrap().model.as_str(), "gpt-5.5");

        let catalog = ModelCatalog::from_json(
            r#"{"models":[
              {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
              {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
            ]}"#,
        )
        .unwrap();
        let provider = ProviderConfig {
            model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: Some("ultra".to_string()),
            plan_reasoning_effort: Some("max".to_string()),
            auto_compact_percent: DEFAULT_AUTO_COMPACT_PERCENT,
            api_key: Some("sk-test".to_string()),
            env_key: None,
            base_url: "https://example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };
        let mut editor =
            ProviderEditor::from_provider_with_catalog("switcher", &provider, Arc::new(catalog));
        editor.active_field = ProviderField::Model;
        editor.model_options = vec!["gpt-5.6-sol".to_string(), "gpt-5.6-luna".to_string()];
        app.providers.set_editor(Some(editor));

        handle_provider_editor_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        let editor = app.providers.editor().unwrap();
        assert_eq!(editor.model.as_str(), "gpt-5.6-luna");
        assert_eq!(editor.reasoning_effort, "medium");
        assert_eq!(editor.plan_reasoning_effort, "max");

        let editor = app.providers.editor_mut().unwrap();
        editor.active_field = ProviderField::Model;
        handle_provider_editor_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        let editor = app.providers.editor().unwrap();
        assert_eq!(editor.model.as_str(), "");
        assert_eq!(editor.reasoning_effort, "medium");
        assert_eq!(
            editor.reasoning_effort_options,
            ["low", "medium", "high", "xhigh"]
        );

        let editor = app.providers.editor_mut().unwrap();
        editor.active_field = ProviderField::BaseUrl;
        handle_provider_editor_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.providers.editor().unwrap().base_url.as_str(), "");

        let editor = app.providers.editor_mut().unwrap();
        editor.active_field = ProviderField::AutoCompactPercent;
        editor.auto_compact_percent.clear();
        handle_provider_editor_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE),
        );
        handle_provider_editor_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        assert_eq!(
            app.providers
                .editor()
                .unwrap()
                .auto_compact_percent
                .as_str(),
            "7"
        );
    }

    #[test]
    fn confirmation_keys_confirm_and_cancel_actions() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("project");
        std::fs::create_dir(&session_dir).unwrap();
        let mut app = app_with_registry(ProviderRegistry::default());
        app.confirmation = Some(ConfirmationAction::ResumeSession(Box::new(test_session(
            "session-1",
            session_dir,
            "resume request",
        ))));
        app.overlay = Some(Overlay::Confirmation);

        handle_confirmation_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        );

        match app.take_queued_action() {
            Some(AppAction::Resume(session)) => assert_eq!(session.id, "session-1"),
            _ => panic!("expected queued resume action"),
        }
        assert_eq!(app.overlay, None);

        app.confirmation = Some(ConfirmationAction::ApplyProvider("switcher".to_string()));
        app.overlay = Some(Overlay::Confirmation);
        handle_confirmation_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        );
        assert_eq!(app.overlay, None);
        assert_eq!(app.confirmation, None);
    }

    #[test]
    fn conversation_overlay_shortcuts_manage_search_filter_and_scroll() {
        let mut app = app_with_registry(ProviderRegistry::default());
        app.conversation.search_mut().set_query("needle");
        app.conversation.scroll_mut().set(5);
        app.conversation
            .set_role_filter(ConversationRoleFilter::User);

        handle_conversation_overlay_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.conversation.role_filter(), ConversationRoleFilter::All);

        handle_conversation_overlay_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.conversation.search().query(), "");
        assert_eq!(app.conversation.search().draft().as_str(), "");
        assert_eq!(app.conversation.scroll().offset(), 0);

        handle_conversation_overlay_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        );
        assert_eq!(app.overlay, Some(Overlay::ConversationSearch));

        handle_conversation_overlay_key(&mut app, KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.conversation.scroll().offset(), usize::MAX);

        handle_conversation_overlay_key(&mut app, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(app.conversation.scroll().offset(), 0);
    }

    fn app_with_registry(provider_registry: ProviderRegistry) -> App {
        App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            provider_registry,
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        )
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

    fn test_session(id: &str, cwd: PathBuf, summary: &str) -> Session {
        Session {
            kind: crate::session_store::SessionKind::Codex,
            id: id.to_string(),
            cwd,
            provider: "switcher".to_string(),
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
}
