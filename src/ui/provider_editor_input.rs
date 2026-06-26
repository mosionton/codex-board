use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, ProviderField};

use super::text_input::handle_text_input_key;

pub(super) fn handle_provider_editor_key(app: &mut App, key: KeyEvent) {
    match (key.code, key.modifiers) {
        (KeyCode::Enter, _) => app.prompt_save_provider_editor(),
        (KeyCode::Esc, _) => app.close_overlay(),
        (KeyCode::F(5), _) => app.fetch_provider_models_for_editor(),
        (KeyCode::Tab, KeyModifiers::NONE) => {
            if let Some(editor) = app.providers.editor_mut() {
                editor.next_field();
            }
        }
        (KeyCode::BackTab, _) => {
            if let Some(editor) = app.providers.editor_mut() {
                editor.previous_field();
            }
        }
        _ => {
            if let Some(editor) = app.providers.editor_mut() {
                if editor.active_field == ProviderField::Model {
                    match key.code {
                        KeyCode::Up => {
                            editor.cycle_model_option(-1);
                            return;
                        }
                        KeyCode::Down => {
                            editor.cycle_model_option(1);
                            return;
                        }
                        _ => {}
                    }
                }

                if let Some(input) = editor.active_text_mut() {
                    handle_text_input_key(input, key);
                    return;
                }

                match (key.code, key.modifiers) {
                    (KeyCode::Left, _) => {
                        editor.cycle_active_option(-1);
                    }
                    (KeyCode::Right, _) => {
                        editor.cycle_active_option(1);
                    }
                    (KeyCode::Char('u'), modifiers)
                        if modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        editor.clear_active_field();
                    }
                    _ => {}
                }
            }
        }
    }
}
