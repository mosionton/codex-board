use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::TextField;

pub(super) fn handle_text_input_key(input: &mut TextField, key: KeyEvent) {
    match key.code {
        KeyCode::Left => input.move_cursor_left(),
        KeyCode::Right => input.move_cursor_right(),
        KeyCode::Home => input.move_cursor_home(),
        KeyCode::End => input.move_cursor_to_end(),
        KeyCode::Backspace => input.remove_char_before_cursor(),
        KeyCode::Delete => input.remove_char_at_cursor(),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.clear();
        }
        KeyCode::Char(ch) if text_char_from_key(key).is_some() => {
            input.insert_char(ch);
        }
        _ => {}
    }
}

fn text_char_from_key(key: KeyEvent) -> Option<char> {
    let KeyCode::Char(ch) = key.code else {
        return None;
    };
    let non_text_modifiers = KeyModifiers::CONTROL
        | KeyModifiers::ALT
        | KeyModifiers::SUPER
        | KeyModifiers::HYPER
        | KeyModifiers::META;
    if key.modifiers.intersects(non_text_modifiers) {
        None
    } else {
        Some(ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ProviderEditor, ProviderField};

    #[test]
    fn text_input_inserts_and_deletes_at_cursor() {
        let mut input = TextField::new("ab");
        input.move_cursor_left();

        handle_text_input_key(
            &mut input,
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
        );
        assert_eq!(input.as_str(), "aXb");
        assert_eq!(input.cursor(), 2);

        handle_text_input_key(
            &mut input,
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
        );
        assert_eq!(input.as_str(), "aX");
        assert_eq!(input.cursor(), 2);

        handle_text_input_key(
            &mut input,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        );
        assert_eq!(input.as_str(), "a");
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn text_input_handles_unicode_cursor_positions() {
        let mut input = TextField::new("你b");
        input.move_cursor_left();

        handle_text_input_key(
            &mut input,
            KeyEvent::new(KeyCode::Char('好'), KeyModifiers::NONE),
        );
        assert_eq!(input.as_str(), "你好b");
        assert_eq!(input.cursor(), 2);

        handle_text_input_key(
            &mut input,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        );
        assert_eq!(input.as_str(), "你b");
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn text_input_ignores_shortcut_characters() {
        let mut input = TextField::new("abc");

        handle_text_input_key(
            &mut input,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        );
        handle_text_input_key(
            &mut input,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
        );

        assert_eq!(input.as_str(), "abc");
        assert_eq!(input.cursor(), 3);
    }

    #[test]
    fn provider_editor_text_field_uses_text_input_cursor() {
        let mut editor = ProviderEditor::new();
        editor.id.set_with_cursor("ab", 1);
        editor.active_field = ProviderField::Id;

        handle_text_input_key(
            editor.active_text_mut().unwrap(),
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
        );

        assert_eq!(editor.id.as_str(), "aXb");
        assert_eq!(editor.id.cursor(), 2);
    }
}
