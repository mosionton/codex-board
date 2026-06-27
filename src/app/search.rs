use super::TextField;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchState {
    query: String,
    draft: TextField,
}

impl SearchState {
    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) const fn draft(&self) -> &TextField {
        &self.draft
    }

    pub(crate) const fn draft_mut(&mut self) -> &mut TextField {
        &mut self.draft
    }

    #[cfg(test)]
    pub(crate) fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.reset_draft();
    }

    pub(crate) fn reset_draft(&mut self) {
        self.draft.set(self.query.clone());
    }

    pub(crate) fn apply_draft(&mut self) {
        self.query = self.draft.as_str().to_string();
        self.draft.move_cursor_to_end();
    }

    pub(crate) fn clear(&mut self) {
        self.query.clear();
        self.draft.clear();
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}
