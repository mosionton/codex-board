use std::{path::PathBuf, time::Instant};

use crate::provider_config::ProviderRegistry;
use crate::session_store::Session;

mod conversation_flow;
mod conversation_state;
mod model_fetch;
mod overlays;
mod provider_display;
mod provider_editor;
mod provider_tabs;
mod providers;
mod providers_state;
mod runtime;
mod scroll;
mod search;
mod selection;
mod sessions;
mod sessions_state;
mod status;
mod text_field;

pub(crate) use conversation_state::ConversationState;
pub(crate) use overlays::ConfirmationAction;
pub(crate) use provider_display::{provider_api_key_display, provider_auth_mode_display};
pub(crate) use provider_editor::{ProviderEditor, ProviderField, WIRE_API_OPTIONS};
pub(crate) use provider_tabs::ProviderTabs;
pub(crate) use providers_state::ProvidersState;
use runtime::ensure_session_cwd_exists;
#[cfg(test)]
use runtime::exec_codex_resume;
pub(crate) use runtime::run;
pub(crate) use scroll::ScrollPosition;
pub(crate) use search::SearchState;
pub(crate) use selection::TableSelection;
pub(crate) use sessions_state::{SessionViewMode, SessionsState};
pub(crate) use text_field::{TextField, char_count};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    CurrentDir,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Sessions,
    Providers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Overlay {
    SessionSearch,
    ConversationSearch,
    ProviderEditor,
    Details,
    Conversation,
    Confirmation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationRoleFilter {
    All,
    User,
    Assistant,
}

pub(crate) struct App {
    pub(crate) session_state: SessionsState,
    pub(crate) page: Page,
    pub(crate) providers: ProvidersState,
    pub(crate) confirmation: Option<ConfirmationAction>,
    pub(crate) queued_action: Option<AppAction>,
    pub(crate) overlay: Option<Overlay>,
    pub(crate) details_scroll: ScrollPosition,
    pub(crate) conversation: ConversationState,
    pub(crate) status: String,
    pub(crate) status_expires_at: Option<Instant>,
    pub(crate) error: Option<String>,
    pub(crate) error_expires_at: Option<Instant>,
}

pub(crate) enum AppAction {
    Resume(Box<Session>),
    Quit,
}

impl App {
    pub(crate) fn new(
        sessions: Vec<Session>,
        current_dir: PathBuf,
        provider_registry: ProviderRegistry,
        provider_config_path: PathBuf,
        codex_config_path: PathBuf,
        sessions_dir: PathBuf,
    ) -> Self {
        let mut app = Self {
            session_state: SessionsState::new(sessions, current_dir, sessions_dir),
            page: Page::Sessions,
            providers: ProvidersState::new(
                provider_registry,
                provider_config_path,
                codex_config_path,
            ),
            confirmation: None,
            queued_action: None,
            overlay: None,
            details_scroll: ScrollPosition::default(),
            conversation: ConversationState::default(),
            status: String::new(),
            status_expires_at: None,
            error: None,
            error_expires_at: None,
        };
        app.refresh_visible();
        app.refresh_provider_selection();
        app
    }

    pub(crate) fn switch_page(&mut self) {
        self.set_page(match self.page {
            Page::Sessions => Page::Providers,
            Page::Providers => Page::Sessions,
        });
    }

    pub(crate) fn set_page(&mut self, page: Page) {
        self.page = page;
        self.clear_status();
    }

    pub(crate) const fn take_queued_action(&mut self) -> Option<AppAction> {
        self.queued_action.take()
    }
}

impl ConversationRoleFilter {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::All => Self::Assistant,
            Self::User => Self::All,
            Self::Assistant => Self::User,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    pub(crate) fn matches(self, role: &str) -> bool {
        match self {
            Self::All => true,
            Self::User => role == "user",
            Self::Assistant => role == "assistant",
        }
    }
}

const fn cycle_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let step = delta.unsigned_abs() % len;
    if delta.is_negative() {
        (current + len - step) % len
    } else {
        (current + step) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_config::{
        PLAN_REASONING_EFFORT_OPTIONS, ProviderAuthMode, ProviderConfig, REASONING_EFFORT_OPTIONS,
    };
    use std::path::Path;
    use std::time::Duration;
    use tempfile::tempdir;

    fn test_session(cwd: PathBuf) -> Session {
        Session {
            id: "session-1".into(),
            cwd,
            provider: "switcher".into(),
            model: None,
            timestamp: "2026-06-23T00:00:00Z".into(),
            summary: "test session".into(),
            file: PathBuf::from("session-1.jsonl"),
            thread_source: "user".into(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_depth: None,
        }
    }

    fn test_session_with_file(cwd: PathBuf, file: PathBuf) -> Session {
        Session {
            file,
            ..test_session(cwd)
        }
    }

    fn app_with_sessions_dir(
        sessions: Vec<Session>,
        current_dir: PathBuf,
        sessions_dir: PathBuf,
    ) -> App {
        App::new(
            sessions,
            current_dir,
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            sessions_dir,
        )
    }

    fn write_session_meta(path: &Path, id: &str, cwd: &Path, summary: &str) {
        let cwd_json = serde_json::to_string(&cwd.to_string_lossy()).unwrap();
        std::fs::write(
            path,
            format!(
                r#"{{"timestamp":"2026-06-23T00:00:01Z","type":"session_meta","payload":{{"id":"{id}","timestamp":"2026-06-23T00:00:00Z","cwd":{cwd_json},"model_provider":"switcher"}}}}"#
            )
            + "\n"
            + &format!(
                r#"{{"timestamp":"2026-06-23T00:00:02Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"{summary}"}}]}}}}"#
            )
            + "\n",
        )
        .unwrap();
    }

    #[test]
    fn filters_provider_tabs_by_scope() {
        let current_dir = PathBuf::from("/repo/current");
        let sessions = vec![
            Session {
                id: "1".into(),
                cwd: current_dir.clone(),
                provider: "a".into(),
                model: None,
                timestamp: "2".into(),
                summary: "first".into(),
                file: PathBuf::from("1"),
                thread_source: "user".into(),
                parent_thread_id: None,
                agent_nickname: None,
                agent_role: None,
                agent_depth: None,
            },
            Session {
                id: "2".into(),
                cwd: PathBuf::from("/repo/other"),
                provider: "b".into(),
                model: None,
                timestamp: "1".into(),
                summary: "second".into(),
                file: PathBuf::from("2"),
                thread_source: "user".into(),
                parent_thread_id: None,
                agent_nickname: None,
                agent_role: None,
                agent_depth: None,
            },
        ];

        assert_eq!(
            ProviderTabs::new(&sessions, Scope::CurrentDir, &current_dir).labels(),
            vec!["All".to_string(), "a".to_string()]
        );
        assert_eq!(
            ProviderTabs::new(&sessions, Scope::All, &current_dir).labels(),
            vec!["All".to_string(), "a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn reasoning_options_match_gpt_5_3_and_later_strengths() {
        assert_eq!(REASONING_EFFORT_OPTIONS, ["low", "medium", "high", "xhigh"]);
        assert_eq!(
            PLAN_REASONING_EFFORT_OPTIONS,
            ["low", "medium", "high", "xhigh"]
        );
    }

    #[test]
    fn prompt_resume_requires_existing_session_directory() {
        let missing_dir = tempdir().unwrap().path().join("missing");
        let mut app = app_with_sessions_dir(
            vec![test_session(missing_dir.clone())],
            missing_dir,
            PathBuf::from("sessions"),
        );

        app.prompt_resume_selected_session();

        assert_eq!(app.overlay, None);
        assert_eq!(app.confirmation, None);
        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("session directory does not exist"))
        );
    }

    #[test]
    fn confirm_resume_rechecks_session_directory() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("project");
        std::fs::create_dir(&session_dir).unwrap();
        let mut app = app_with_sessions_dir(
            vec![test_session(session_dir.clone())],
            session_dir.clone(),
            dir.path().join("sessions"),
        );

        app.prompt_resume_selected_session();
        std::fs::remove_dir(&session_dir).unwrap();
        app.confirm_pending_action();

        assert!(app.queued_action.is_none());
        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("session directory does not exist"))
        );
    }

    #[test]
    fn exec_resume_rejects_missing_session_directory() {
        let missing_dir = tempdir().unwrap().path().join("missing");

        let error = exec_codex_resume("session-1", &missing_dir).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("session directory does not exist")
        );
    }

    #[test]
    fn conversation_role_filter_defaults_to_user() {
        let app = app_with_sessions_dir(
            Vec::new(),
            PathBuf::from("/repo/current"),
            PathBuf::from("sessions"),
        );

        assert_eq!(app.conversation.role_filter(), ConversationRoleFilter::User);
        assert!(app.conversation.role_filter().matches("user"));
        assert!(!app.conversation.role_filter().matches("assistant"));
    }

    #[test]
    fn conversation_role_filter_cycles_from_user_to_all() {
        let mut filter = ConversationRoleFilter::User;

        filter = filter.next();
        assert_eq!(filter, ConversationRoleFilter::All);
        assert!(filter.matches("user"));
        assert!(filter.matches("assistant"));

        filter = filter.next();
        assert_eq!(filter, ConversationRoleFilter::Assistant);
        assert!(!filter.matches("user"));
        assert!(filter.matches("assistant"));
    }

    #[test]
    fn provider_editor_applies_and_cycles_model_options() {
        let mut editor = ProviderEditor::new();
        editor.model.clear();

        editor.apply_model_options(vec![
            "gpt-5-mini".to_string(),
            "gpt-5.5".to_string(),
            "o4".to_string(),
        ]);

        assert_eq!(editor.active_field, ProviderField::Model);
        assert_eq!(editor.model.as_str(), "gpt-5-mini");
        assert_eq!(editor.model.cursor(), "gpt-5-mini".chars().count());

        assert!(editor.cycle_model_option(1));
        assert_eq!(editor.model.as_str(), "gpt-5.5");
        assert!(editor.cycle_model_option(-1));
        assert_eq!(editor.model.as_str(), "gpt-5-mini");
    }

    #[test]
    fn provider_editor_keeps_auth_mode_read_only() {
        let provider = ProviderConfig {
            model: None,
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: Some("sk-auth-mode-must-not-display".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::OpenAi,
        };
        let mut editor = ProviderEditor::from_provider("openai-proxy", &provider);

        assert_eq!(editor.api_key.as_str(), "");
        assert_eq!(editor.auth_mode_display(), "openai");
        assert!(!editor.is_editable_field(ProviderField::Auth));
        assert!(!editor.is_editable_field(ProviderField::ApiKey));

        editor.active_field = ProviderField::Auth;
        assert!(editor.active_text_mut().is_none());
        assert!(!editor.cycle_active_option(1));
        editor.next_field();
        assert_eq!(editor.active_field, ProviderField::Model);
    }

    #[test]
    fn provider_ids_put_openai_auth_providers_first() {
        let dir = tempdir().unwrap();
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "aaa-api-key",
                ProviderConfig {
                    model: None,
                    reasoning_effort: None,
                    plan_reasoning_effort: None,
                    api_key: Some("sk-test".to_string()),
                    env_key: None,
                    base_url: "https://api.example.test/v1".to_string(),
                    wire_api: "responses".to_string(),
                    auth_mode: ProviderAuthMode::ApiKey,
                },
            )
            .unwrap();
        registry
            .upsert(
                "mmm-openai-auth",
                ProviderConfig {
                    model: None,
                    reasoning_effort: None,
                    plan_reasoning_effort: None,
                    api_key: None,
                    env_key: None,
                    base_url: "https://api.openai.com/v1".to_string(),
                    wire_api: "responses".to_string(),
                    auth_mode: ProviderAuthMode::OpenAi,
                },
            )
            .unwrap();
        registry
            .upsert(
                "zzz-openai-auth",
                ProviderConfig {
                    model: None,
                    reasoning_effort: None,
                    plan_reasoning_effort: None,
                    api_key: None,
                    env_key: None,
                    base_url: "https://api.openai.com/v1".to_string(),
                    wire_api: "responses".to_string(),
                    auth_mode: ProviderAuthMode::OpenAi,
                },
            )
            .unwrap();
        let app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            dir.path().join("providers.toml"),
            dir.path().join("config.toml"),
            dir.path().join("sessions"),
        );

        assert_eq!(
            app.provider_ids(),
            ["mmm-openai-auth", "zzz-openai-auth", "aaa-api-key"]
        );
        assert_eq!(
            app.selected_provider_id().as_deref(),
            Some("mmm-openai-auth")
        );
    }

    #[test]
    fn app_groups_provider_state() {
        let mut registry = ProviderRegistry::default();
        registry
            .upsert(
                "switcher",
                ProviderConfig::new("https://api.example.test/v1", "responses"),
            )
            .unwrap();

        let app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            PathBuf::from("sessions"),
        );

        assert!(app.providers.registry().providers.contains_key("switcher"));
        assert_eq!(app.providers.config_path(), PathBuf::from("providers.toml"));
        assert_eq!(
            app.providers.codex_config_path(),
            PathBuf::from("config.toml")
        );
    }

    #[test]
    fn app_groups_session_state() {
        let current_dir = PathBuf::from("/repo/current");
        let sessions_dir = PathBuf::from("sessions");
        let app = App::new(
            vec![test_session(current_dir.clone())],
            current_dir.clone(),
            ProviderRegistry::default(),
            PathBuf::from("providers.toml"),
            PathBuf::from("config.toml"),
            sessions_dir.clone(),
        );

        assert_eq!(app.session_state.items().len(), 1);
        assert_eq!(app.session_state.current_dir(), current_dir.as_path());
        assert_eq!(app.session_state.sessions_dir(), sessions_dir.as_path());
        assert_eq!(app.session_state.scope(), Scope::CurrentDir);
    }

    #[test]
    fn invalid_provider_rename_keeps_editor_and_original_provider() {
        let dir = tempdir().unwrap();
        let provider = ProviderConfig::new("https://api.example.test/v1", "responses");
        let mut registry = ProviderRegistry::default();
        registry.upsert("old-provider", provider.clone()).unwrap();
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            dir.path().join("providers.toml"),
            dir.path().join("config.toml"),
            dir.path().join("sessions"),
        );
        app.providers.editor = Some(ProviderEditor::from_provider("old-provider", &provider));
        if let Some(editor) = app.providers.editor.as_mut() {
            editor.id.set("bad.provider");
        }
        app.confirmation = Some(ConfirmationAction::SaveProvider("bad.provider".to_string()));
        app.overlay = Some(Overlay::Confirmation);

        app.confirm_pending_action();

        assert!(
            app.providers
                .registry
                .providers
                .contains_key("old-provider")
        );
        assert!(
            !app.providers
                .registry
                .providers
                .contains_key("bad.provider")
        );
        assert_eq!(app.overlay, Some(Overlay::ProviderEditor));
        assert!(app.providers.editor.is_some());
        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("Invalid provider"))
        );
    }

    #[test]
    fn saving_env_key_provider_preserves_env_key_without_storing_api_key() {
        let dir = tempdir().unwrap();
        let provider = ProviderConfig {
            model: None,
            reasoning_effort: None,
            plan_reasoning_effort: None,
            api_key: Some("sk-loaded-from-env".to_string()),
            env_key: Some("CODEX_SWITCHER_TEST_PROVIDER_KEY".to_string()),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: "responses".to_string(),
            auth_mode: ProviderAuthMode::ApiKey,
        };
        let mut registry = ProviderRegistry::default();
        registry.upsert("env-provider", provider.clone()).unwrap();
        let mut app = App::new(
            Vec::new(),
            PathBuf::from("/repo/current"),
            registry,
            dir.path().join("providers.toml"),
            dir.path().join("config.toml"),
            dir.path().join("sessions"),
        );
        app.providers.editor = Some(ProviderEditor::from_provider("env-provider", &provider));
        if let Some(editor) = app.providers.editor.as_mut() {
            editor.model.set("gpt-5.5");
        }
        app.overlay = Some(Overlay::ProviderEditor);

        app.save_provider_editor();

        let saved = app
            .providers
            .registry
            .providers
            .get("env-provider")
            .unwrap();
        assert_eq!(
            saved.env_key.as_deref(),
            Some("CODEX_SWITCHER_TEST_PROVIDER_KEY")
        );
        assert_eq!(saved.api_key, None);
        assert_eq!(saved.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(app.overlay, None);
    }

    #[test]
    fn reload_sessions_scans_sessions_directory() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        let project_dir = dir.path().join("project");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        std::fs::create_dir(&project_dir).unwrap();
        write_session_meta(
            &sessions_dir.join("session.jsonl"),
            "session-1",
            &project_dir,
            "new request",
        );
        let mut app = app_with_sessions_dir(Vec::new(), project_dir, sessions_dir);

        app.reload_sessions();

        assert_eq!(app.session_state.items.len(), 1);
        assert_eq!(app.session_state.visible_indices.len(), 1);
        assert_eq!(app.session_state.items[0].id, "session-1");
        assert_eq!(app.status, "Reloaded 1 sessions.");
    }

    #[test]
    fn reload_conversation_preserves_filters() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("project");
        let session_file = dir.path().join("session.jsonl");
        std::fs::create_dir(&project_dir).unwrap();
        std::fs::write(
            &session_file,
            r#"{"timestamp":"2026-06-23T00:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"first request"}]}}"#
                .to_string()
                + "\n"
                + r#"{"timestamp":"2026-06-23T00:00:03Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"first response"}]}}"#
                + "\n",
        )
        .unwrap();
        let session = test_session_with_file(project_dir.clone(), session_file);
        let mut app =
            app_with_sessions_dir(vec![session], project_dir, dir.path().join("sessions"));
        app.conversation.search_mut().set_query("first");
        app.conversation.search_mut().draft_mut().set("draft");
        app.conversation
            .set_role_filter(ConversationRoleFilter::Assistant);

        app.reload_conversation();

        assert_eq!(app.conversation.messages().len(), 2);
        assert_eq!(app.conversation.search().query(), "first");
        assert_eq!(app.conversation.search().draft().as_str(), "draft");
        assert_eq!(
            app.conversation.role_filter(),
            ConversationRoleFilter::Assistant
        );
        assert_eq!(app.status, "Reloaded 2 conversation messages.");
    }

    #[test]
    fn transient_status_expires() {
        let mut app = app_with_sessions_dir(
            Vec::new(),
            PathBuf::from("/repo/current"),
            PathBuf::from("sessions"),
        );
        app.show_transient_status("Reloaded 7 sessions.", Duration::from_secs(1));

        assert_eq!(app.status, "Reloaded 7 sessions.");
        assert!(app.status_expires_at.is_some());

        app.status_expires_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
        );
        app.clear_expired_status();

        assert!(app.status.is_empty());
        assert!(app.status_expires_at.is_none());
    }
}
