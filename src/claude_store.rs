use std::{
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use walkdir::WalkDir;

use crate::session_store::{ConversationEntry, Session, SessionKind, sort_sessions};

pub(super) const CLAUDE_PROVIDER_LABEL: &str = "claude";

const META_SCAN_LINE_LIMIT: usize = 400;

/// Read-only snapshot of the local Claude Code configuration, shown on the
/// Providers page. codex-board never writes any Claude config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ClaudeStatus {
    pub(super) email: Option<String>,
    pub(super) organization: Option<String>,
    pub(super) model: Option<String>,
    pub(super) base_url: Option<String>,
}

impl ClaudeStatus {
    pub(super) const fn logged_in(&self) -> bool {
        self.email.is_some()
    }
}

/// Returns `None` when no Claude Code config exists (not installed).
pub(super) fn load_claude_status(config_dir: &Path) -> Option<ClaudeStatus> {
    // `.claude.json` sits inside CLAUDE_CONFIG_DIR when that is set, and next
    // to `~/.claude` in the default layout.
    let account_json = read_json_file(&config_dir.join(".claude.json")).or_else(|| {
        config_dir
            .parent()
            .and_then(|parent| read_json_file(&parent.join(".claude.json")))
    });
    let settings = read_json_file(&config_dir.join("settings.json"));
    if account_json.is_none() && settings.is_none() {
        return None;
    }

    let account = account_json
        .as_ref()
        .and_then(|json| json.get("oauthAccount").cloned());
    let env = settings.as_ref().and_then(|json| json.get("env").cloned());
    Some(ClaudeStatus {
        email: json_string(account.as_ref(), "emailAddress"),
        organization: json_string(account.as_ref(), "organizationName"),
        model: settings
            .as_ref()
            .and_then(|json| json.get("model"))
            .and_then(JsonValue::as_str)
            .map(str::to_string)
            .or_else(|| json_string(env.as_ref(), "ANTHROPIC_MODEL")),
        base_url: json_string(env.as_ref(), "ANTHROPIC_BASE_URL"),
    })
}

fn read_json_file(path: &Path) -> Option<JsonValue> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn json_string(json: Option<&JsonValue>, key: &str) -> Option<String> {
    json?
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

pub(super) fn load_claude_sessions(projects_dir: &Path) -> Result<Vec<Session>> {
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in WalkDir::new(projects_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|s| s.to_str()) != Some("jsonl")
        {
            continue;
        }
        if let Some(session) = parse_claude_session_file(entry.path())? {
            sessions.push(session);
        }
    }

    sort_sessions(&mut sessions);
    Ok(sessions)
}

fn parse_claude_session_file(path: &Path) -> Result<Option<Session>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open session file {}", path.display()))?;
    let reader = io::BufReader::new(file);
    let mut parsed = ParsedClaudeSession::default();

    for line in reader.lines().take(META_SCAN_LINE_LIMIT) {
        let line =
            line.with_context(|| format!("failed to read session file {}", path.display()))?;
        let json: JsonValue = match serde_json::from_str(&line) {
            Ok(json) => json,
            Err(_) => continue,
        };
        parsed.apply_line(&json);
        if parsed.is_sidechain {
            return Ok(None);
        }
        if parsed.is_complete() {
            break;
        }
    }

    Ok(parsed.into_session(path))
}

#[derive(Default)]
struct ParsedClaudeSession {
    id: Option<String>,
    cwd: Option<PathBuf>,
    timestamp: Option<String>,
    model: Option<String>,
    summary: Option<String>,
    title: Option<String>,
    is_sidechain: bool,
}

impl ParsedClaudeSession {
    fn apply_line(&mut self, json: &JsonValue) {
        if self.id.is_none() {
            self.id = json
                .get("sessionId")
                .and_then(JsonValue::as_str)
                .map(str::to_string);
        }
        if self.cwd.is_none() {
            self.cwd = json
                .get("cwd")
                .and_then(JsonValue::as_str)
                .map(PathBuf::from);
        }
        if self.timestamp.is_none() {
            self.timestamp = json
                .get("timestamp")
                .and_then(JsonValue::as_str)
                .map(str::to_string);
        }

        match json.get("type").and_then(JsonValue::as_str) {
            Some("summary") if self.title.is_none() => {
                self.title = json
                    .get("summary")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
            }
            Some("user") => {
                if is_sidechain_line(json) {
                    self.is_sidechain = true;
                    return;
                }
                if self.summary.is_none()
                    && !is_meta_line(json)
                    && let Some(text) = message_text(json)
                {
                    let normalized = normalize_whitespace(&text);
                    if !normalized.is_empty() && !is_noise_user_text(&normalized) {
                        self.summary = Some(normalized);
                    }
                }
            }
            Some("assistant") => {
                if is_sidechain_line(json) {
                    self.is_sidechain = true;
                    return;
                }
                if self.model.is_none() {
                    self.model = json
                        .get("message")
                        .and_then(|message| message.get("model"))
                        .and_then(JsonValue::as_str)
                        .map(str::to_string);
                }
            }
            _ => {}
        }
    }

    const fn is_complete(&self) -> bool {
        self.id.is_some()
            && self.cwd.is_some()
            && self.timestamp.is_some()
            && self.model.is_some()
            && self.summary.is_some()
    }

    fn into_session(self, path: &Path) -> Option<Session> {
        let id = self.id?;
        let cwd = self.cwd?;
        let summary = self
            .summary
            .or(self.title)
            .unwrap_or_else(|| fallback_summary(path, &id));

        Some(Session {
            kind: SessionKind::Claude,
            id,
            cwd,
            provider: CLAUDE_PROVIDER_LABEL.to_string(),
            model: self.model,
            timestamp: self.timestamp.unwrap_or_default(),
            summary,
            file: path.to_path_buf(),
            thread_source: "user".to_string(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            agent_depth: None,
        })
    }
}

pub(super) fn load_claude_conversation(path: &Path) -> Result<Vec<ConversationEntry>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open session file {}", path.display()))?;
    let reader = io::BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line =
            line.with_context(|| format!("failed to read session file {}", path.display()))?;
        let json: JsonValue = match serde_json::from_str(&line) {
            Ok(json) => json,
            Err(_) => continue,
        };
        let Some(role @ ("user" | "assistant")) = json.get("type").and_then(JsonValue::as_str)
        else {
            continue;
        };
        if is_meta_line(&json) || is_sidechain_line(&json) {
            continue;
        }
        let Some(text) = message_text(&json) else {
            continue;
        };
        let normalized = normalize_whitespace(&text);
        if normalized.is_empty() || (role == "user" && is_noise_user_text(&normalized)) {
            continue;
        }

        entries.push(ConversationEntry {
            timestamp: json
                .get("timestamp")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_string(),
            role: role.to_string(),
            text: text.trim().to_string(),
        });
    }

    Ok(entries)
}

fn is_meta_line(json: &JsonValue) -> bool {
    json.get("isMeta").and_then(JsonValue::as_bool) == Some(true)
}

fn is_sidechain_line(json: &JsonValue) -> bool {
    json.get("isSidechain").and_then(JsonValue::as_bool) == Some(true)
}

fn message_text(json: &JsonValue) -> Option<String> {
    let content = json.get("message")?.get("content")?;
    match content {
        JsonValue::String(text) => Some(text.clone()),
        JsonValue::Array(items) => {
            let parts = items
                .iter()
                .filter(|item| item.get("type").and_then(JsonValue::as_str) == Some("text"))
                .filter_map(|item| item.get("text").and_then(JsonValue::as_str))
                .filter(|text| !text.trim_start().starts_with("<system-reminder>"))
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}

fn is_noise_user_text(text: &str) -> bool {
    text.starts_with("Caveat:")
        || text.starts_with("<command-name>")
        || text.starts_with("<local-command")
        || text.starts_with("<system-reminder>")
        || text.starts_with("[Request interrupted")
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fallback_summary(path: &Path, id: &str) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map_or_else(|| id.to_string(), str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn user_line(text: &str) -> String {
        format!(
            r#"{{"type":"user","isSidechain":false,"cwd":"/tmp/project","sessionId":"claude-1","timestamp":"2026-07-01T00:00:01Z","message":{{"role":"user","content":{}}}}}"#,
            serde_json::to_string(text).unwrap()
        )
    }

    fn assistant_line(text: &str) -> String {
        format!(
            r#"{{"type":"assistant","isSidechain":false,"cwd":"/tmp/project","sessionId":"claude-1","timestamp":"2026-07-01T00:00:02Z","message":{{"role":"assistant","model":"claude-fable-5","content":[{{"type":"text","text":{}}}]}}}}"#,
            serde_json::to_string(text).unwrap()
        )
    }

    #[test]
    fn parses_claude_session_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude-1.jsonl");
        fs::write(
            &path,
            user_line("Caveat: The messages below were generated elsewhere.")
                + "\n"
                + &user_line("<command-name>/clear</command-name>")
                + "\n"
                + &user_line("给项目增加 claude 支持")
                + "\n"
                + &assistant_line("好的。")
                + "\n",
        )
        .unwrap();

        let session = parse_claude_session_file(&path).unwrap().unwrap();

        assert_eq!(session.kind, SessionKind::Claude);
        assert_eq!(session.id, "claude-1");
        assert_eq!(session.cwd, PathBuf::from("/tmp/project"));
        assert_eq!(session.provider, "claude");
        assert_eq!(session.model.as_deref(), Some("claude-fable-5"));
        assert_eq!(session.timestamp, "2026-07-01T00:00:01Z");
        assert_eq!(session.summary, "给项目增加 claude 支持");
    }

    #[test]
    fn skips_sidechain_session_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("agent-1.jsonl");
        fs::write(
            &path,
            r#"{"type":"user","isSidechain":true,"cwd":"/tmp/project","sessionId":"agent-1","timestamp":"2026-07-01T00:00:01Z","message":{"role":"user","content":"subagent prompt"}}"#
                .to_string()
                + "\n",
        )
        .unwrap();

        assert_eq!(parse_claude_session_file(&path).unwrap(), None);
    }

    #[test]
    fn falls_back_to_summary_line_when_no_user_text() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude-2.jsonl");
        fs::write(
            &path,
            r#"{"type":"summary","summary":"Prior work on parser","leafUuid":"x"}"#.to_string()
                + "\n"
                + &user_line("Caveat: continuation noise").replace("claude-1", "claude-2"),
        )
        .unwrap();

        let session = parse_claude_session_file(&path).unwrap().unwrap();

        assert_eq!(session.id, "claude-2");
        assert_eq!(session.summary, "Prior work on parser");
    }

    #[test]
    fn loads_claude_conversation_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("claude-1.jsonl");
        fs::write(
            &path,
            user_line("<command-name>/clear</command-name>")
                + "\n"
                + &user_line("实现新功能")
                + "\n"
                + r#"{"type":"assistant","isSidechain":false,"sessionId":"claude-1","timestamp":"2026-07-01T00:00:03Z","message":{"role":"assistant","model":"claude-fable-5","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{}}]}}"#
                + "\n"
                + &assistant_line("已实现。")
                + "\n",
        )
        .unwrap();

        let entries = load_claude_conversation(&path).unwrap();

        assert_eq!(
            entries,
            vec![
                ConversationEntry {
                    timestamp: "2026-07-01T00:00:01Z".to_string(),
                    role: "user".to_string(),
                    text: "实现新功能".to_string(),
                },
                ConversationEntry {
                    timestamp: "2026-07-01T00:00:02Z".to_string(),
                    role: "assistant".to_string(),
                    text: "已实现。".to_string(),
                },
            ]
        );
    }

    #[test]
    fn load_claude_status_reads_account_and_settings() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join(".claude");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            dir.path().join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"user@example.com","organizationName":"Acme"}}"#,
        )
        .unwrap();
        fs::write(
            config_dir.join("settings.json"),
            r#"{"model":"claude-fable-5","env":{"ANTHROPIC_BASE_URL":"https://gateway.example.com"}}"#,
        )
        .unwrap();

        let status = load_claude_status(&config_dir).unwrap();

        assert!(status.logged_in());
        assert_eq!(status.email.as_deref(), Some("user@example.com"));
        assert_eq!(status.organization.as_deref(), Some("Acme"));
        assert_eq!(status.model.as_deref(), Some("claude-fable-5"));
        assert_eq!(
            status.base_url.as_deref(),
            Some("https://gateway.example.com")
        );
    }

    #[test]
    fn load_claude_status_falls_back_to_env_model() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join(".claude");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("settings.json"),
            r#"{"env":{"ANTHROPIC_MODEL":"claude-sonnet-5"}}"#,
        )
        .unwrap();

        let status = load_claude_status(&config_dir).unwrap();

        assert!(!status.logged_in());
        assert_eq!(status.model.as_deref(), Some("claude-sonnet-5"));
    }

    #[test]
    fn load_claude_status_returns_none_without_config() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join(".claude");

        assert_eq!(load_claude_status(&config_dir), None);
    }

    #[test]
    fn load_claude_sessions_scans_project_directories() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("-tmp-project");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(
            project_dir.join("claude-1.jsonl"),
            user_line("第一个请求") + "\n" + &assistant_line("完成") + "\n",
        )
        .unwrap();

        let sessions = load_claude_sessions(dir.path()).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "claude-1");
        assert_eq!(sessions[0].provider, "claude");
    }
}
