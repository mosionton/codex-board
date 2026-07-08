use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    app::{App, Page, provider_api_key_display, provider_auth_mode_display},
    claude_store::ClaudeStatus,
    provider_config::{ProviderConfig, normalize_reasoning_effort},
};

use super::layout::{centered_rect_size, details_dialog_height, percent_len, wrap_text};

pub(super) fn draw_details_dialog(frame: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let popup_width = percent_len(area.width, 78)
        .max(area.width.min(32))
        .min(area.width);
    let lines = match app.page {
        Page::Sessions => selected_session_details(app, popup_width.saturating_sub(4) as usize),
        Page::Providers => selected_provider_details(app, popup_width.saturating_sub(4) as usize),
    };
    let popup_height = details_dialog_height(lines.len(), area.height);
    let popup = centered_rect_size(popup_width, popup_height, area);
    let content_height = popup.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(content_height);
    app.details_scroll.clamp_to(max_scroll);
    let scroll_offset = app.details_scroll.offset();
    let visible_lines = lines
        .iter()
        .skip(scroll_offset)
        .take(content_height)
        .cloned()
        .collect::<Vec<_>>();
    let title = if max_scroll == 0 || content_height == 0 {
        "Details - Esc/Enter closes".to_string()
    } else {
        let start = scroll_offset + 1;
        let end = (scroll_offset + content_height).min(lines.len());
        format!(
            "Details {start}-{end}/{} - Up/Down/Page scroll | Esc/Enter closes",
            lines.len()
        )
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(visible_lines).block(Block::default().title(title).borders(Borders::ALL)),
        popup,
    );
}

pub(super) fn selected_session_details(app: &App, width: usize) -> Vec<Line<'static>> {
    let Some(session) = app.selected_session() else {
        return vec![Line::raw("No session selected.")];
    };
    detail_lines(
        [
            ("time", session.timestamp.clone()),
            ("provider", session.provider.clone()),
            (
                "model",
                session.model.clone().unwrap_or_else(|| "-".to_string()),
            ),
            ("source", session.thread_source.clone()),
            (
                "parent",
                session
                    .parent_thread_id
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ),
            (
                "agent",
                session
                    .agent_nickname
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ),
            (
                "role",
                session
                    .agent_role
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ),
            (
                "depth",
                session
                    .agent_depth
                    .map_or_else(|| "-".to_string(), |depth| depth.to_string()),
            ),
            ("cwd", session.cwd.display().to_string()),
            ("summary", session.summary.clone()),
            ("session_id", session.id.clone()),
        ],
        width,
    )
}

pub(super) fn selected_provider_details(app: &App, width: usize) -> Vec<Line<'static>> {
    if app.is_claude_row_selected() {
        let Some(status) = app.providers.claude_status() else {
            return vec![Line::raw("No provider selected.")];
        };
        return claude_status_details(status, width);
    }
    let Some(id) = app.selected_provider_id() else {
        return vec![Line::raw("No provider selected.")];
    };
    let Some(provider) = app.providers.provider(&id) else {
        return vec![Line::raw("No provider selected.")];
    };
    let is_applied = app.providers.is_applied(&id);
    detail_lines(provider_display_items(&id, provider, is_applied), width)
}

fn claude_status_details(status: &ClaudeStatus, width: usize) -> Vec<Line<'static>> {
    let dash = || "-".to_string();
    detail_lines(
        [
            ("id", "claude".to_string()),
            (
                "status",
                if status.logged_in() {
                    "login".to_string()
                } else {
                    "not logged in".to_string()
                },
            ),
            ("account", status.email.clone().unwrap_or_else(dash)),
            (
                "organization",
                status.organization.clone().unwrap_or_else(dash),
            ),
            ("model", status.model.clone().unwrap_or_else(dash)),
            ("base_url", status.base_url.clone().unwrap_or_else(dash)),
            (
                "note",
                "read-only; managed by Claude Code itself".to_string(),
            ),
        ],
        width,
    )
}

pub(super) fn provider_display_items(
    id: &str,
    provider: &ProviderConfig,
    is_applied: bool,
) -> [(&'static str, String); 9] {
    [
        ("id", id.to_string()),
        (
            "status",
            if is_applied {
                "applied".to_string()
            } else {
                "-".to_string()
            },
        ),
        (
            "model",
            provider.model.clone().unwrap_or_else(|| "-".to_string()),
        ),
        (
            "auth_mode",
            provider_auth_mode_display(provider).to_string(),
        ),
        ("base_url", provider.base_url.clone()),
        ("wire_api", provider.wire_api.clone()),
        (
            "reason",
            normalize_reasoning_effort(provider.reasoning_effort.as_deref()).to_string(),
        ),
        (
            "plan_reason",
            normalize_reasoning_effort(provider.plan_reasoning_effort.as_deref()).to_string(),
        ),
        ("api_key", provider_api_key_display(provider)),
    ]
}

pub(super) fn detail_lines<const N: usize>(
    items: [(&'static str, String); N],
    width: usize,
) -> Vec<Line<'static>> {
    let label_width = items
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    let label_prefix_width = label_width + 2;
    let value_width = width.saturating_sub(label_prefix_width).max(16);
    let mut lines = Vec::new();
    for (label, value) in items {
        let wrapped = wrap_text(&value, value_width);
        for (index, part) in wrapped.into_iter().enumerate() {
            let label_text = if index == 0 {
                format!("{label:<label_width$}: ")
            } else {
                " ".repeat(label_prefix_width)
            };
            lines.push(Line::from(vec![
                Span::styled(label_text, Style::default().fg(Color::Yellow)),
                Span::raw(part),
            ]));
        }
    }
    lines
}
