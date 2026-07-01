use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

use crate::{
    app::{App, SessionViewMode},
    session_store::{Session, truncate_chars},
};

use super::{details::provider_display_items, layout::centered_rect, layout::compact_path};

pub(super) const PROVIDER_DISPLAY_LABELS: [&str; 9] = [
    "id",
    "status",
    "model",
    "auth_mode",
    "base_url",
    "wire_api",
    "reason",
    "plan_reason",
    "api_key",
];

const PROVIDER_TABLE_WIDTHS: [Constraint; 9] = [
    Constraint::Length(18),
    Constraint::Length(9),
    Constraint::Length(18),
    Constraint::Length(10),
    Constraint::Min(28),
    Constraint::Length(14),
    Constraint::Length(10),
    Constraint::Length(12),
    Constraint::Length(16),
];

pub(super) fn draw_sessions(frame: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let rows = (0..app.session_state.visible_len())
        .filter_map(|index| {
            let session = app.session_state.visible_session(index)?;
            let source = session_source_label(
                session,
                app.session_state.view_mode(),
                app.session_state.visible_tree_prefix(index),
                app.session_state.visible_parent_link(index),
            );
            let provider_style = Style::default().fg(Color::Cyan);
            Some(Row::new([
                Cell::from(session.timestamp.as_str().to_string()),
                Cell::from(session.provider.clone()).style(provider_style),
                Cell::from(truncate_chars(&source, 24)),
                Cell::from(compact_path(&session.cwd)),
                Cell::from(truncate_chars(&session.summary, 96)),
            ]))
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Length(25),
            Constraint::Length(18),
            Constraint::Length(24),
            Constraint::Length(32),
            Constraint::Min(24),
        ],
    )
    .header(
        Row::new(["time", "provider", "source", "cwd", "summary"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));

    frame.render_stateful_widget(table, area, app.session_state.selection_state_mut());
}

fn session_source_label(
    session: &Session,
    view_mode: SessionViewMode,
    tree_prefix: &str,
    show_parent_link: bool,
) -> String {
    let is_subagent = session.thread_source == "subagent" || session.parent_thread_id.is_some();
    let mut label = if is_subagent {
        subagent_source_label(session)
    } else {
        session.thread_source.clone()
    };
    if show_parent_link && let Some(parent) = session.parent_thread_id.as_deref() {
        label.push_str(" <- ");
        label.push_str(&short_session_id(parent));
    }

    match view_mode {
        SessionViewMode::Tree => format!("{tree_prefix}{label}"),
        SessionViewMode::Flat => label,
    }
}

fn subagent_source_label(session: &Session) -> String {
    let mut label = session
        .agent_nickname
        .as_deref()
        .filter(|nickname| !nickname.trim().is_empty())
        .map_or_else(
            || "subagent".to_string(),
            |nickname| format!("sub {nickname}"),
        );
    if let Some(role) = session
        .agent_role
        .as_deref()
        .filter(|role| !role.trim().is_empty())
    {
        label.push('/');
        label.push_str(role);
    }
    label
}

fn short_session_id(session_id: &str) -> String {
    session_id.chars().take(8).collect()
}

pub(super) fn draw_providers(frame: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let ids = app.provider_ids();
    let rows = ids.iter().map(|id| {
        let provider = app.providers.provider(id).expect("provider id exists");
        let is_applied = app.providers.is_applied(id);
        Row::new(
            provider_display_items(id, provider, is_applied)
                .into_iter()
                .enumerate()
                .map(|(index, (_, value))| {
                    if index == 0 {
                        Cell::from(value).style(Style::default().fg(Color::Cyan))
                    } else if index == 1 && is_applied {
                        Cell::from(value).style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Cell::from(value)
                    }
                }),
        )
    });

    let table = Table::new(rows, PROVIDER_TABLE_WIDTHS)
        .header(
            Row::new(PROVIDER_DISPLAY_LABELS).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));

    frame.render_stateful_widget(table, area, app.providers.selection_state_mut());

    if ids.is_empty() {
        let popup = centered_rect(58, 20, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new("No providers configured. Press n to add one.")
                .block(Block::default().title("Providers").borders(Borders::ALL)),
            popup,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app::SessionViewMode;

    use super::*;

    fn test_session(id: &str, cwd: PathBuf, provider: &str, summary: &str) -> Session {
        Session {
            id: id.to_string(),
            cwd,
            provider: provider.to_string(),
            model: None,
            timestamp: "2026-06-24T00:00:00Z".to_string(),
            summary: summary.to_string(),
            file: PathBuf::from(format!("{id}.jsonl")),
            thread_source: "user".to_string(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            agent_depth: None,
        }
    }

    #[test]
    fn session_source_label_shows_tree_glyphs_only_in_tree_view() {
        let cwd = PathBuf::from("/repo/current");
        let mut child = test_session("child", cwd, "switcher", "summary");
        child.thread_source = "subagent".to_string();
        child.parent_thread_id = Some("parent".to_string());
        child.agent_nickname = Some("Boole".to_string());
        child.agent_role = Some("worker".to_string());

        assert_eq!(
            session_source_label(&child, SessionViewMode::Tree, "├─ ", false),
            "├─ sub Boole/worker"
        );
        assert_eq!(
            session_source_label(&child, SessionViewMode::Flat, "├─ ", false),
            "sub Boole/worker"
        );
    }

    #[test]
    fn orphan_source_label_keeps_parent_id_without_tree_depth() {
        let cwd = PathBuf::from("/repo/current");
        let mut child = test_session("child", cwd, "switcher", "summary");
        child.thread_source = "subagent".to_string();
        child.parent_thread_id = Some("019f1067-10b5-7d02-8176-093dbc9170fa".to_string());
        child.agent_nickname = Some("Boole".to_string());

        assert_eq!(
            session_source_label(&child, SessionViewMode::Tree, "● ", true),
            "● sub Boole <- 019f1067"
        );
    }

    #[test]
    fn flat_orphan_source_label_keeps_parent_id() {
        let cwd = PathBuf::from("/repo/current");
        let mut child = test_session("child", cwd, "switcher", "summary");
        child.thread_source = "subagent".to_string();
        child.parent_thread_id = Some("019f1067-10b5-7d02-8176-093dbc9170fa".to_string());
        child.agent_nickname = Some("Boole".to_string());

        assert_eq!(
            session_source_label(&child, SessionViewMode::Flat, "", true),
            "sub Boole <- 019f1067"
        );
    }
}
