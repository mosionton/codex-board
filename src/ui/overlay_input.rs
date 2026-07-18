use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Overlay};

use super::{provider_editor_input::handle_provider_editor_key, text_input::handle_text_input_key};

pub(super) fn handle_overlay_key(app: &mut App, overlay: Overlay, key: KeyEvent) {
    match overlay {
        Overlay::SessionSearch => handle_session_search_key(app, key),
        Overlay::ConversationSearch => handle_conversation_search_key(app, key),
        Overlay::ProviderEditor => handle_provider_editor_key(app, key),
        Overlay::Confirmation => handle_confirmation_key(app, key),
        Overlay::Details => match key.code {
            KeyCode::Esc => app.close_overlay(),
            KeyCode::Up => app.scroll_details(-1),
            KeyCode::Down => app.scroll_details(1),
            KeyCode::PageUp => app.scroll_details(-10),
            KeyCode::PageDown => app.scroll_details(10),
            KeyCode::Home => app.details_scroll.reset(),
            KeyCode::End => app.details_scroll.jump_to_end(),
            _ => {}
        },
        Overlay::Conversation => handle_conversation_overlay_key(app, key),
    }
}

pub(super) fn handle_conversation_overlay_key(app: &mut App, key: KeyEvent) {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => app.close_overlay(),
        (KeyCode::Char('/'), KeyModifiers::NONE) => app.open_conversation_search(),
        (KeyCode::Char('r'), KeyModifiers::NONE) => app.reload_conversation(),
        (KeyCode::Tab, KeyModifiers::NONE) => app.cycle_conversation_role_filter(),
        (KeyCode::Char('u'), modifiers)
            if modifiers.contains(KeyModifiers::CONTROL)
                && !app.conversation.search().is_empty() =>
        {
            app.clear_conversation_search();
        }
        (KeyCode::Up, _) => app.scroll_conversation(-1),
        (KeyCode::Down, _) => app.scroll_conversation(1),
        (KeyCode::PageUp, _) => app.scroll_conversation(-10),
        (KeyCode::PageDown, _) => app.scroll_conversation(10),
        (KeyCode::Home, _) => app.conversation.scroll_mut().reset(),
        (KeyCode::End, _) => app.conversation.scroll_mut().jump_to_end(),
        _ => {}
    }
}

pub(super) fn handle_session_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            app.session_state.search_mut().apply_draft();
            app.overlay = None;
            app.session_state.reset_selection();
            app.clear_status();
            app.refresh_visible();
        }
        KeyCode::Esc => app.close_overlay(),
        _ => handle_text_input_key(app.session_state.search_mut().draft_mut(), key),
    }
}

pub(super) fn handle_conversation_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => app.apply_conversation_search(),
        KeyCode::Esc => app.close_overlay(),
        _ => handle_text_input_key(app.conversation.search_mut().draft_mut(), key),
    }
}

pub(super) fn handle_confirmation_key(app: &mut App, key: KeyEvent) {
    match (key.code, key.modifiers) {
        (KeyCode::Char(' '), KeyModifiers::NONE) => app.toggle_resume_optional_argument(),
        (KeyCode::Enter, _) | (KeyCode::Char('y'), KeyModifiers::NONE) => {
            app.confirm_pending_action();
        }
        (KeyCode::Esc, _) | (KeyCode::Char('n'), KeyModifiers::NONE) => app.close_overlay(),
        _ => {}
    }
}
