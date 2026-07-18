use crate::session_store::{Session, truncate_chars};

use super::{App, AppAction, Overlay, ResumeOptions, ensure_session_cwd_exists};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationAction {
    ApplyProvider(String),
    DeleteProvider(String),
    ResumeSession {
        session: Box<Session>,
        options: ResumeOptions,
    },
    SaveProvider(String),
}

impl App {
    pub(crate) fn confirm_pending_action(&mut self) {
        let Some(action) = self.confirmation.take() else {
            self.overlay = None;
            return;
        };
        match action {
            ConfirmationAction::ApplyProvider(id) => {
                self.overlay = None;
                self.apply_provider(&id);
            }
            ConfirmationAction::DeleteProvider(id) => {
                self.overlay = None;
                self.delete_provider(&id);
            }
            ConfirmationAction::ResumeSession { session, options } => {
                self.overlay = None;
                if let Err(err) = ensure_session_cwd_exists(&session.cwd) {
                    self.show_error(format!("Cannot resume session: {err}"));
                } else {
                    self.queued_action = Some(AppAction::Resume { session, options });
                }
            }
            ConfirmationAction::SaveProvider(_) => self.save_provider_editor(),
        }
    }

    pub(crate) fn close_overlay(&mut self) {
        if self.overlay == Some(Overlay::ProviderEditor) {
            self.providers.editor = None;
            self.providers.model_fetch_task = None;
        }
        if self.overlay == Some(Overlay::SessionSearch) {
            self.session_state.search.reset_draft();
        }
        if self.overlay == Some(Overlay::ConversationSearch) {
            self.conversation.search_mut().reset_draft();
            self.overlay = Some(Overlay::Conversation);
            self.clear_status();
            return;
        }
        if self.overlay == Some(Overlay::Confirmation) {
            self.confirmation = None;
        }
        self.overlay = None;
        self.clear_status();
    }

    pub(crate) fn confirmation_dialog(&self) -> Option<(&'static str, String)> {
        match self.confirmation.as_ref()? {
            ConfirmationAction::ApplyProvider(id) => Some((
                "Apply Provider",
                format!("Apply provider '{id}' to Codex config?"),
            )),
            ConfirmationAction::DeleteProvider(id) => {
                Some(("Delete Provider", format!("Delete provider '{id}'?")))
            }
            ConfirmationAction::ResumeSession { session, options } => {
                let label = if session.summary.trim().is_empty() {
                    session.id.clone()
                } else {
                    truncate_chars(&session.summary, 72)
                };
                let command = session
                    .kind
                    .resume_command_display(&session.id, options.optional_args(session.kind));
                let optional_args =
                    if matches!(session.kind, crate::session_store::SessionKind::Codex) {
                        format!(
                            "\n\nOptional arguments:\n  [{}] --yolo",
                            if options.yolo { 'x' } else { ' ' }
                        )
                    } else {
                        String::new()
                    };
                Some((
                    "Resume Session",
                    format!(
                        "Resume session '{label}'?\n\n  {command}\n  in {}{optional_args}",
                        session.cwd.display()
                    ),
                ))
            }
            ConfirmationAction::SaveProvider(id) => {
                Some(("Save Provider", format!("Save provider '{id}'?")))
            }
        }
    }

    pub(crate) fn toggle_resume_optional_argument(&mut self) {
        if let Some(ConfirmationAction::ResumeSession { session, options }) =
            self.confirmation.as_mut()
            && matches!(session.kind, crate::session_store::SessionKind::Codex)
        {
            options.yolo = !options.yolo;
        }
    }

    pub(crate) fn confirmation_help(&self) -> &'static str {
        if matches!(
            self.confirmation,
            Some(ConfirmationAction::ResumeSession { ref session, .. })
                if matches!(session.kind, crate::session_store::SessionKind::Codex)
        ) {
            "Space toggles --yolo. Enter/y confirms. Esc/n cancels."
        } else {
            "Enter/y confirms. Esc/n cancels."
        }
    }
}
