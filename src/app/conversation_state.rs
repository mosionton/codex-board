use crate::session_store::ConversationEntry;

use super::{ConversationRoleFilter, ScrollPosition, SearchState};

#[derive(Debug, Clone)]
pub struct ConversationState {
    messages: Vec<ConversationEntry>,
    scroll: ScrollPosition,
    search: SearchState,
    role_filter: ConversationRoleFilter,
}

impl ConversationState {
    pub(crate) fn messages(&self) -> &[ConversationEntry] {
        &self.messages
    }

    pub(crate) fn replace_for_new_session(&mut self, messages: Vec<ConversationEntry>) {
        self.messages = messages;
        self.scroll.reset();
        self.search.clear();
        self.role_filter = ConversationRoleFilter::User;
    }

    pub(crate) fn replace_preserving_filters(&mut self, messages: Vec<ConversationEntry>) {
        self.messages = messages;
        self.scroll.reset();
    }

    pub(crate) const fn scroll(&self) -> ScrollPosition {
        self.scroll
    }

    pub(crate) const fn scroll_mut(&mut self) -> &mut ScrollPosition {
        &mut self.scroll
    }

    pub(crate) const fn search(&self) -> &SearchState {
        &self.search
    }

    pub(crate) const fn search_mut(&mut self) -> &mut SearchState {
        &mut self.search
    }

    pub(crate) const fn role_filter(&self) -> ConversationRoleFilter {
        self.role_filter
    }

    #[cfg(test)]
    pub(crate) const fn set_role_filter(&mut self, role_filter: ConversationRoleFilter) {
        self.role_filter = role_filter;
    }

    pub(crate) fn apply_search(&mut self) {
        self.search.apply_draft();
        self.scroll.reset();
    }

    pub(crate) fn clear_search(&mut self) {
        self.search.clear();
        self.scroll.reset();
    }

    pub(crate) const fn cycle_role_filter(&mut self) {
        self.role_filter = self.role_filter.next();
        self.scroll.reset();
    }

    pub(crate) fn scroll_by(&mut self, delta: i16) {
        self.scroll.scroll_by(delta);
    }
}

impl Default for ConversationState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            scroll: ScrollPosition::default(),
            search: SearchState::default(),
            role_filter: ConversationRoleFilter::User,
        }
    }
}
