use std::ops::Deref;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TextField {
    text: String,
    cursor: usize,
}

impl TextField {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        let mut field = Self {
            text: text.into(),
            cursor: 0,
        };
        field.move_cursor_to_end();
        field
    }

    pub(crate) const fn empty() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) const fn cursor(&self) -> usize {
        self.cursor
    }

    pub(crate) fn set(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.move_cursor_to_end();
    }

    #[cfg(test)]
    pub(crate) fn set_with_cursor(&mut self, text: impl Into<String>, cursor: usize) {
        self.text = text.into();
        self.cursor = cursor;
        self.clamp_cursor();
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub(crate) fn move_cursor_left(&mut self) {
        self.clamp_cursor();
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub(crate) fn move_cursor_right(&mut self) {
        self.clamp_cursor();
        self.cursor = (self.cursor + 1).min(char_count(&self.text));
    }

    pub(crate) fn move_cursor_home(&mut self) {
        self.cursor = 0;
    }

    pub(crate) fn move_cursor_to_end(&mut self) {
        self.cursor = char_count(&self.text);
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        self.clamp_cursor();
        let byte_index = self.byte_index_for_cursor(self.cursor);
        self.text.insert(byte_index, ch);
        self.cursor += 1;
    }

    pub(crate) fn remove_char_before_cursor(&mut self) {
        self.clamp_cursor();
        if self.cursor == 0 {
            return;
        }
        let start = self.byte_index_for_cursor(self.cursor - 1);
        let end = self.byte_index_for_cursor(self.cursor);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub(crate) fn remove_char_at_cursor(&mut self) {
        self.clamp_cursor();
        if self.cursor >= char_count(&self.text) {
            return;
        }
        let start = self.byte_index_for_cursor(self.cursor);
        let end = self.byte_index_for_cursor(self.cursor + 1);
        self.text.replace_range(start..end, "");
    }

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(char_count(&self.text));
    }

    fn byte_index_for_cursor(&self, cursor: usize) -> usize {
        self.text
            .char_indices()
            .nth(cursor)
            .map_or(self.text.len(), |(index, _)| index)
    }
}

impl Deref for TextField {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

pub(crate) fn char_count(text: &str) -> usize {
    text.chars().count()
}
