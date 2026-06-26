use std::time::{Duration, Instant};

use super::App;

impl App {
    pub(crate) fn show_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_expires_at = None;
    }

    pub(crate) fn show_transient_status(&mut self, message: impl Into<String>, duration: Duration) {
        self.status = message.into();
        self.status_expires_at = Some(Instant::now() + duration);
    }

    pub(crate) fn clear_status(&mut self) {
        self.status.clear();
        self.status_expires_at = None;
    }

    pub(crate) fn show_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
        self.error_expires_at = Some(Instant::now() + Duration::from_secs(3));
    }

    pub(crate) fn clear_expired_status(&mut self) {
        if self
            .status_expires_at
            .is_some_and(|expires_at| Instant::now() >= expires_at)
        {
            self.clear_status();
        }
    }

    pub(crate) fn clear_expired_error(&mut self) {
        if self
            .error_expires_at
            .is_some_and(|expires_at| Instant::now() >= expires_at)
        {
            self.error = None;
            self.error_expires_at = None;
        }
    }
}
