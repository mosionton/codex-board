use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppAction};

pub(super) fn handle_sessions_page_key(app: &mut App, key: KeyEvent) -> Option<AppAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(AppAction::Quit),
        (KeyCode::Esc, _) => {
            if app.session_state.search().is_empty() {
                Some(AppAction::Quit)
            } else {
                app.clear_session_search();
                None
            }
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            app.open_conversation();
            None
        }
        (KeyCode::Char('i'), KeyModifiers::NONE) => {
            app.open_details();
            None
        }
        (KeyCode::Char('t'), KeyModifiers::NONE) => {
            app.switch_page();
            None
        }
        (KeyCode::Char('a'), KeyModifiers::NONE) => {
            app.toggle_scope();
            None
        }
        (KeyCode::Char('v'), KeyModifiers::NONE) => {
            app.toggle_session_view_mode();
            None
        }
        (KeyCode::Char(' '), KeyModifiers::NONE) => {
            app.toggle_selected_session_expansion();
            None
        }
        (KeyCode::Char('/'), KeyModifiers::NONE) => {
            app.open_session_search();
            None
        }
        (KeyCode::Char('r'), KeyModifiers::NONE) => {
            app.reload_sessions();
            None
        }
        (KeyCode::Up, _) => {
            app.move_selection(-1);
            None
        }
        (KeyCode::Down, _) => {
            app.move_selection(1);
            None
        }
        (KeyCode::PageUp, _) => {
            app.page_selection(-1);
            None
        }
        (KeyCode::PageDown, _) => {
            app.page_selection(1);
            None
        }
        (KeyCode::BackTab, _) => {
            app.switch_provider_tab(-1);
            None
        }
        (KeyCode::Tab, KeyModifiers::NONE) => {
            app.switch_provider_tab(1);
            None
        }
        (KeyCode::Enter, _) => {
            app.prompt_resume_selected_session();
            None
        }
        _ => None,
    }
}

pub(super) fn handle_providers_page_key(app: &mut App, key: KeyEvent) -> Option<AppAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(AppAction::Quit),
        (KeyCode::Char('t'), KeyModifiers::NONE) => {
            app.switch_page();
            None
        }
        (KeyCode::Char('i'), KeyModifiers::NONE) => {
            app.open_details();
            None
        }
        (KeyCode::Up, _) => {
            app.move_provider_selection(-1);
            None
        }
        (KeyCode::Down, _) => {
            app.move_provider_selection(1);
            None
        }
        (KeyCode::PageUp, _) => {
            app.page_provider_selection(-1);
            None
        }
        (KeyCode::PageDown, _) => {
            app.page_provider_selection(1);
            None
        }
        (KeyCode::Char('a'), KeyModifiers::NONE) => {
            app.prompt_apply_selected_provider();
            None
        }
        (KeyCode::Char('n'), KeyModifiers::NONE) => {
            app.start_new_provider();
            None
        }
        (KeyCode::Char('e'), KeyModifiers::NONE) => {
            app.start_edit_provider();
            None
        }
        (KeyCode::Char('d'), KeyModifiers::NONE) => {
            app.prompt_delete_selected_provider();
            None
        }
        _ => None,
    }
}
