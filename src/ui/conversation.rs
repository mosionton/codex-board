use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    app::{App, ConversationRoleFilter},
    session_store::{ConversationEntry, search_terms},
};

use super::layout::{centered_rect_size, details_dialog_height, percent_len, wrap_text};

pub(super) fn draw_conversation_dialog(frame: &mut ratatui::Frame<'_>, app: &mut App, area: Rect) {
    let popup_width = percent_len(area.width, 90)
        .max(area.width.min(48))
        .min(area.width);
    let filtered = filtered_conversation(
        app.conversation.messages(),
        app.conversation.search().query(),
        app.conversation.role_filter(),
    );
    let lines = conversation_lines(&filtered, popup_width.saturating_sub(4) as usize);
    let popup_height = details_dialog_height(lines.len(), area.height);
    let popup = centered_rect_size(popup_width, popup_height, area);
    let content_height = popup.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(content_height);
    app.conversation.scroll_mut().clamp_to(max_scroll);
    let scroll_offset = app.conversation.scroll().offset();
    let visible_lines = lines
        .iter()
        .skip(scroll_offset)
        .take(content_height)
        .cloned()
        .collect::<Vec<_>>();
    let title = if max_scroll == 0 || content_height == 0 {
        conversation_title(
            "Conversation - / search | Tab role | r reload | Esc/Enter closes",
            app,
        )
    } else {
        let start = scroll_offset + 1;
        let end = (scroll_offset + content_height).min(lines.len());
        conversation_title(
            &format!(
                "Conversation {start}-{end}/{} - Up/Down/Page scroll | / search | Tab role | r reload | Esc/Enter closes",
                lines.len()
            ),
            app,
        )
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(visible_lines).block(Block::default().title(title).borders(Borders::ALL)),
        popup,
    );
}

pub(super) fn filtered_conversation<'a>(
    messages: &'a [ConversationEntry],
    query: &str,
    role_filter: ConversationRoleFilter,
) -> Vec<&'a ConversationEntry> {
    let terms = search_terms(query);
    messages
        .iter()
        .filter(|message| conversation_matches_role(message, role_filter))
        .filter(|message| conversation_matches_search(message, &terms))
        .collect()
}

pub(super) fn conversation_matches_role(
    message: &ConversationEntry,
    role_filter: ConversationRoleFilter,
) -> bool {
    role_filter.matches(&message.role)
}

pub(super) fn conversation_matches_search(message: &ConversationEntry, terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }
    let haystack =
        format!("{} {} {}", message.role, message.timestamp, message.text).to_lowercase();
    terms.iter().all(|term| haystack.contains(term))
}

pub(super) fn conversation_title(base: &str, app: &App) -> String {
    let role = format!("role: {}", app.conversation.role_filter().as_str());
    if app.conversation.search().is_empty() {
        format!("{base} | {role}")
    } else {
        format!(
            "{base} | {role} | Ctrl+U clears search | search: {}",
            app.conversation.search().query()
        )
    }
}

pub(super) fn conversation_lines(
    messages: &[&ConversationEntry],
    width: usize,
) -> Vec<Line<'static>> {
    if messages.is_empty() {
        return vec![Line::raw("No conversation messages found.")];
    }

    let mut lines = Vec::new();
    let text_width = width.saturating_sub(4).max(16);
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>3}. ", index + 1),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                message.role.as_str().to_string(),
                conversation_role_style(&message.role),
            ),
            Span::raw(format!("  {}", message.timestamp)),
        ]));
        for part in wrap_text(&message.text, text_width) {
            lines.push(Line::from(vec![Span::raw("    "), Span::raw(part)]));
        }
    }
    lines
}

fn conversation_role_style(role: &str) -> Style {
    match role {
        "user" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "assistant" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        _ => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    }
}
