#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrollPosition {
    offset: usize,
}

impl ScrollPosition {
    pub(crate) const fn offset(self) -> usize {
        self.offset
    }

    pub(crate) const fn reset(&mut self) {
        self.offset = 0;
    }

    pub(crate) const fn jump_to_end(&mut self) {
        self.offset = usize::MAX;
    }

    pub(crate) fn scroll_by(&mut self, delta: i16) {
        if delta.is_negative() {
            self.offset = self
                .offset
                .saturating_sub(usize::from(delta.unsigned_abs()));
        } else {
            self.offset = self
                .offset
                .saturating_add(usize::from(delta.unsigned_abs()));
        }
    }

    pub(crate) fn clamp_to(&mut self, max: usize) {
        self.offset = self.offset.min(max);
    }

    #[cfg(test)]
    pub(crate) const fn set(&mut self, offset: usize) {
        self.offset = offset;
    }
}
