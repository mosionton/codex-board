use ratatui::widgets::TableState;

use super::cycle_index;

#[derive(Debug, Clone, Default)]
pub struct TableSelection {
    index: usize,
    state: TableState,
}

impl TableSelection {
    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    pub(crate) const fn state_mut(&mut self) -> &mut TableState {
        &mut self.state
    }

    pub(crate) const fn reset(&mut self) {
        self.index = 0;
        self.state.select(Some(0));
    }

    pub(crate) const fn clear(&mut self) {
        self.index = 0;
        self.state.select(None);
    }

    pub(crate) const fn select(&mut self, index: usize) {
        self.index = index;
        self.state.select(Some(index));
    }

    pub(crate) fn sync_len(&mut self, len: usize) {
        if len == 0 {
            self.clear();
        } else {
            self.select(self.index.min(len - 1));
        }
    }

    pub(crate) const fn move_by(&mut self, len: usize, delta: isize) {
        if len == 0 {
            return;
        }
        self.select(cycle_index(self.index, len, delta));
    }

    pub(crate) fn move_by_clamped(&mut self, len: usize, delta: isize) {
        if len == 0 {
            return;
        }

        let max_index = len - 1;
        let current = self.index.min(max_index);
        let step = delta.unsigned_abs();
        let next = if delta.is_negative() {
            current.saturating_sub(step)
        } else {
            current.saturating_add(step).min(max_index)
        };
        self.select(next);
    }

    #[cfg(test)]
    pub(crate) const fn state(&self) -> &TableState {
        &self.state
    }
}
