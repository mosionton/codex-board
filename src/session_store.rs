use std::{
    cmp::Reverse,
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Session {
    pub(super) id: String,
    pub(super) cwd: PathBuf,
    pub(super) provider: String,
    pub(super) model: Option<String>,
    pub(super) timestamp: String,
    pub(super) summary: String,
    pub(super) file: PathBuf,
    pub(super) thread_source: String,
    pub(super) parent_thread_id: Option<String>,
    pub(super) agent_nickname: Option<String>,
    pub(super) agent_role: Option<String>,
    pub(super) agent_depth: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConversationEntry {
    pub(super) timestamp: String,
    pub(super) role: String,
    pub(super) text: String,
}

pub(super) fn load_sessions(sessions_dir: &Path) -> Result<Vec<Session>> {
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in WalkDir::new(sessions_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|s| s.to_str()) != Some("jsonl")
        {
            continue;
        }
        if let Some(session) = parse_session_file(entry.path())? {
            sessions.push(session);
        }
    }

    sessions.sort_by_key(|session| Reverse(session.timestamp.clone()));
    Ok(sessions)
}

fn parse_session_file(path: &Path) -> Result<Option<Session>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open session file {}", path.display()))?;
    let reader = io::BufReader::new(file);

    let mut id = None;
    let mut cwd = None;
    let mut provider = None;
    let mut timestamp = None;
    let mut model = None;
    let mut summary = None;
    let mut thread_source = None;
    let mut parent_thread_id = None;
    let mut agent_nickname = None;
    let mut agent_role = None;
    let mut agent_depth = None;

    for line in reader.lines().take(160) {
        let line =
            line.with_context(|| format!("failed to read session file {}", path.display()))?;
        let json: JsonValue = match serde_json::from_str(&line) {
            Ok(json) => json,
            Err(_) => continue,
        };
        let Some(payload) = json.get("payload") else {
            continue;
        };

        match json.get("type").and_then(JsonValue::as_str) {
            Some("session_meta") => {
                let spawn = subagent_spawn(payload);
                id = payload
                    .get("id")
                    .or_else(|| payload.get("session_id"))
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
                cwd = payload
                    .get("cwd")
                    .and_then(JsonValue::as_str)
                    .map(PathBuf::from);
                provider = payload
                    .get("model_provider")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
                timestamp = payload
                    .get("timestamp")
                    .and_then(JsonValue::as_str)
                    .or_else(|| json.get("timestamp").and_then(JsonValue::as_str))
                    .map(str::to_string);
                thread_source = payload
                    .get("thread_source")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
                parent_thread_id = optional_string_from(
                    payload.get("parent_thread_id"),
                    spawn.and_then(|spawn| spawn.get("parent_thread_id")),
                );
                agent_nickname = optional_string_from(
                    payload.get("agent_nickname"),
                    spawn.and_then(|spawn| spawn.get("agent_nickname")),
                );
                agent_role = optional_string_from(
                    payload.get("agent_role"),
                    spawn.and_then(|spawn| spawn.get("agent_role")),
                );
                agent_depth = spawn
                    .and_then(|spawn| spawn.get("depth"))
                    .and_then(JsonValue::as_u64)
                    .and_then(|depth| u32::try_from(depth).ok());
            }
            Some("turn_context") if model.is_none() => {
                model = payload
                    .get("model")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
            }
            Some("response_item") if summary.is_none() => {
                summary = extract_user_summary(payload);
            }
            _ => {}
        }

        if id.is_some()
            && cwd.is_some()
            && provider.is_some()
            && timestamp.is_some()
            && model.is_some()
            && summary.is_some()
        {
            break;
        }
    }

    let Some(id) = id else {
        return Ok(None);
    };
    let Some(cwd) = cwd else {
        return Ok(None);
    };
    let Some(provider) = provider else {
        return Ok(None);
    };

    let summary = summary.unwrap_or_else(|| fallback_summary(path, &id));

    Ok(Some(Session {
        id,
        cwd,
        provider,
        model,
        timestamp: timestamp.unwrap_or_default(),
        summary,
        file: path.to_path_buf(),
        thread_source: thread_source.unwrap_or_else(|| "user".to_string()),
        parent_thread_id,
        agent_nickname,
        agent_role,
        agent_depth,
    }))
}

fn subagent_spawn(payload: &JsonValue) -> Option<&JsonValue> {
    payload.get("source")?.get("subagent")?.get("thread_spawn")
}

fn optional_string_from(
    primary: Option<&JsonValue>,
    fallback: Option<&JsonValue>,
) -> Option<String> {
    primary
        .and_then(JsonValue::as_str)
        .or_else(|| fallback.and_then(JsonValue::as_str))
        .map(str::to_string)
}

pub(super) fn load_session_conversation(path: &Path) -> Result<Vec<ConversationEntry>> {
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
        if json.get("type").and_then(JsonValue::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = json.get("payload") else {
            continue;
        };
        if let Some(entry) =
            extract_conversation_entry(payload, json.get("timestamp").and_then(JsonValue::as_str))
        {
            entries.push(entry);
        }
    }

    Ok(entries)
}

pub(super) fn search_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|term| !term.is_empty())
        .collect()
}

pub(super) fn matches_search(session: &Session, terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }

    let haystack = session_search_text(session);
    terms.iter().all(|term| haystack.contains(term))
}

fn session_search_text(session: &Session) -> String {
    [
        session.id.as_str(),
        session.provider.as_str(),
        session.cwd.to_str().unwrap_or_default(),
        session.summary.as_str(),
        session.timestamp.as_str(),
        session.thread_source.as_str(),
        session.parent_thread_id.as_deref().unwrap_or_default(),
        session.agent_nickname.as_deref().unwrap_or_default(),
        session.agent_role.as_deref().unwrap_or_default(),
    ]
    .join(" ")
    .to_lowercase()
}

fn extract_user_summary(payload: &JsonValue) -> Option<String> {
    let message = payload.get("item").unwrap_or(payload);
    let role = message
        .get("role")
        .or_else(|| payload.get("role"))
        .and_then(JsonValue::as_str)?;
    if role != "user" {
        return None;
    }

    let text = text_from_content(message.get("content").or_else(|| payload.get("content"))?)?;
    let summary = normalize_summary(&text);
    if is_bootstrap_user_message(&summary) {
        None
    } else {
        Some(summary)
    }
}

fn extract_conversation_entry(
    payload: &JsonValue,
    fallback_timestamp: Option<&str>,
) -> Option<ConversationEntry> {
    let message = payload.get("item").unwrap_or(payload);
    let role = message
        .get("role")
        .or_else(|| payload.get("role"))
        .and_then(JsonValue::as_str)?;
    if role != "user" && role != "assistant" {
        return None;
    }

    let text = text_from_content(message.get("content").or_else(|| payload.get("content"))?)?;
    let normalized_text = normalize_summary(&text);
    if normalized_text.is_empty() || (role == "user" && is_bootstrap_user_message(&normalized_text))
    {
        return None;
    }
    let text = text.trim().to_string();

    let timestamp = message
        .get("timestamp")
        .or_else(|| payload.get("timestamp"))
        .and_then(JsonValue::as_str)
        .or(fallback_timestamp)
        .unwrap_or_default()
        .to_string();

    Some(ConversationEntry {
        timestamp,
        role: role.to_string(),
        text,
    })
}

fn text_from_content(content: &JsonValue) -> Option<String> {
    match content {
        JsonValue::String(text) => Some(text.clone()),
        JsonValue::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(JsonValue::as_str)
                        .or_else(|| item.get("content").and_then(JsonValue::as_str))
                })
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        JsonValue::Object(obj) => obj
            .get("text")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn normalize_summary(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_bootstrap_user_message(text: &str) -> bool {
    text.starts_with("# AGENTS.md instructions")
        || text.starts_with("<environment_context>")
        || text.contains("<environment_context>")
}

fn fallback_summary(path: &Path, id: &str) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map_or_else(|| truncate_chars(id, 96), |name| truncate_chars(name, 96))
}

pub(super) fn truncate_chars(text: &str, max: usize) -> String {
    if UnicodeWidthStr::width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }

    let mut truncated = String::new();
    let mut width = 0;
    let max_text_width = max.saturating_sub(UnicodeWidthChar::width('…').unwrap_or(1));
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_text_width {
            break;
        }
        truncated.push(ch);
        width += ch_width;
    }
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_session_metadata_and_turn_context() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"timestamp":"2026-06-23T00:00:01Z","type":"session_meta","payload":{"id":"abc-123","timestamp":"2026-06-23T00:00:00Z","cwd":"/tmp/project","model_provider":"switcher"}}"#
                .to_string()
                + "\n"
                + r#"{"timestamp":"2026-06-23T00:00:02Z","type":"turn_context","payload":{"model":"gpt-5.5"}}"#
                + "\n"
                + r##"{"timestamp":"2026-06-23T00:00:03Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions\n\n<environment_context>skip</environment_context>"}]}}"##
                + "\n"
                + r#"{"timestamp":"2026-06-23T00:00:04Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"实现 TUI 摘要展示"}]}}"#
                + "\n",
        )
        .unwrap();

        let session = parse_session_file(&path).unwrap().unwrap();
        assert_eq!(session.id, "abc-123");
        assert_eq!(session.cwd, PathBuf::from("/tmp/project"));
        assert_eq!(session.provider, "switcher");
        assert_eq!(session.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(session.summary, "实现 TUI 摘要展示");
    }

    #[test]
    fn parses_nested_response_item_summary() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"timestamp":"2026-06-23T00:00:01Z","type":"session_meta","payload":{"id":"abc-123","timestamp":"2026-06-23T00:00:00Z","cwd":"/tmp/project","model_provider":"switcher"}}"#
                .to_string()
                + "\n"
                + r#"{"timestamp":"2026-06-23T00:00:02Z","type":"response_item","payload":{"type":"response_item","item":{"type":"message","role":"user","content":[{"type":"input_text","text":"nested user request"}]}}}"#
                + "\n",
        )
        .unwrap();

        let session = parse_session_file(&path).unwrap().unwrap();

        assert_eq!(session.summary, "nested user request");
    }

    #[test]
    fn parses_subagent_metadata_from_session_meta() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r#"{"timestamp":"2026-06-29T11:34:49Z","type":"session_meta","payload":{"id":"child","session_id":"parent","parent_thread_id":"parent","timestamp":"2026-06-29T11:34:49Z","cwd":"/tmp/project","model_provider":"switcher","thread_source":"subagent","agent_nickname":"Boole","agent_role":"worker","source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent","depth":1,"agent_nickname":"Boole","agent_role":"worker"}}}}}"#
                .to_string()
                + "\n",
        )
        .unwrap();

        let session = parse_session_file(&path).unwrap().unwrap();

        assert_eq!(session.id, "child");
        assert_eq!(session.thread_source, "subagent");
        assert_eq!(session.parent_thread_id.as_deref(), Some("parent"));
        assert_eq!(session.agent_nickname.as_deref(), Some("Boole"));
        assert_eq!(session.agent_role.as_deref(), Some("worker"));
        assert_eq!(session.agent_depth, Some(1));
    }

    #[test]
    fn loads_user_and_assistant_conversation_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        fs::write(
            &path,
            r##"{"timestamp":"2026-06-23T00:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions\n\n<environment_context>skip</environment_context>"}]}}"##
                .to_string()
                + "\n"
                + r#"{"timestamp":"2026-06-23T00:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"实现 TUI 摘要展示"}]}}"#
                + "\n"
                + r#"{"timestamp":"2026-06-23T00:00:03Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"已实现。"}]}}"#
                + "\n",
        )
        .unwrap();

        let entries = load_session_conversation(&path).unwrap();

        assert_eq!(
            entries,
            vec![
                ConversationEntry {
                    timestamp: "2026-06-23T00:00:02Z".to_string(),
                    role: "user".to_string(),
                    text: "实现 TUI 摘要展示".to_string(),
                },
                ConversationEntry {
                    timestamp: "2026-06-23T00:00:03Z".to_string(),
                    role: "assistant".to_string(),
                    text: "已实现。".to_string(),
                },
            ]
        );
    }

    #[test]
    fn search_matches_all_terms_across_fields() {
        let session = Session {
            id: "abc-123".into(),
            cwd: PathBuf::from("/repo/current"),
            provider: "switcher".into(),
            model: None,
            timestamp: "2026-06-23T00:00:00Z".into(),
            summary: "实现 TUI 摘要展示".into(),
            file: PathBuf::from("1"),
            thread_source: "user".into(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_depth: None,
        };

        assert!(matches_search(&session, &search_terms("switcher 摘要")));
        assert!(matches_search(&session, &search_terms("current tui")));
        assert!(!matches_search(&session, &search_terms("switcher missing")));
        assert!(matches_search(&session, &search_terms("   ")));
    }

    #[test]
    fn search_matches_session_relationship_metadata() {
        let session = Session {
            id: "abc-123".into(),
            cwd: PathBuf::from("/repo/current"),
            provider: "switcher".into(),
            model: None,
            timestamp: "2026-06-23T00:00:00Z".into(),
            summary: "实现 TUI 摘要展示".into(),
            file: PathBuf::from("1"),
            thread_source: "subagent".into(),
            parent_thread_id: Some("parent-123".into()),
            agent_nickname: Some("Boole".into()),
            agent_role: Some("worker".into()),
            agent_depth: Some(1),
        };

        assert!(matches_search(&session, &search_terms("parent-123 boole")));
        assert!(matches_search(&session, &search_terms("subagent worker")));
    }

    #[test]
    fn truncate_chars_limits_display_width() {
        let truncated = truncate_chars("确保能切换到login auth方式", 12);

        assert_eq!(truncated, "确保能切换…");
        assert!(UnicodeWidthStr::width(truncated.as_str()) <= 12);
    }
}
