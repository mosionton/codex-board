use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, Page, Scope, SessionViewMode};

use super::layout::{centered_rect, centered_rect_size, percent_len, wrap_text};

pub(super) fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    if app.page == Page::Providers {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Codex Board", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(
                    " | Providers | {} configured | file: {}",
                    app.providers.provider_count(),
                    app.providers.config_path().display()
                )),
            ]))
            .block(Block::default().title("Providers").borders(Borders::ALL)),
            area,
        );
        return;
    }

    let provider_spans = app
        .session_state
        .provider_tabs()
        .labels()
        .iter()
        .enumerate()
        .flat_map(|(index, provider)| {
            let style = if index == app.session_state.provider_tabs().selected_index() {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            [
                Span::raw(" "),
                Span::styled(provider.as_str(), style),
                Span::raw(" "),
            ]
        });
    let mut line = vec![Span::styled(
        "Provider Filter ",
        Style::default().add_modifier(Modifier::BOLD),
    )];
    line.extend(provider_spans);

    let scope = match app.session_state.scope() {
        Scope::CurrentDir => "current directory",
        Scope::All => "all sessions",
    };
    let view = match app.session_state.view_mode() {
        SessionViewMode::Tree => "tree",
        SessionViewMode::Flat => "flat",
    };
    let title = format!("Sessions - {scope} - {view}");

    frame.render_widget(
        Paragraph::new(Line::from(line)).block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}

pub(super) fn draw_page_list(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let sessions_style = page_item_style(app.page == Page::Sessions);
    let providers_style = page_item_style(app.page == Page::Providers);
    let lines = vec![
        Line::styled("1 Sessions", sessions_style),
        Line::styled("2 Providers", providers_style),
        Line::raw(""),
        Line::styled("t toggles", Style::default().fg(Color::Gray)),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("Pages").borders(Borders::ALL)),
        area,
    );
}

pub(super) fn draw_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let search = if app.session_state.search().is_empty() {
        "none"
    } else {
        app.session_state.search().query()
    };
    let status = if app.status.is_empty() {
        match app.page {
            Page::Sessions => {
                "Enter resume | Space expand | i details | c conversation | r reload | v view | t pages | Tab provider | a scope | / search | q quit"
            }
            Page::Providers => {
                "a apply | i details | t toggle pages | n new | e edit | d delete | q quit"
            }
        }
    } else {
        app.status.as_str()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(
                "FILTER",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  search: {}  visible: {}",
                search,
                app.session_state.visible_len()
            )),
        ]),
        Line::raw(status),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("Controls").borders(Borders::ALL)),
        area,
    );
}

pub(super) fn draw_empty_sessions_message(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    if app.page != Page::Sessions || app.session_state.visible_len() != 0 {
        return;
    }

    let popup = centered_rect(64, 20, area);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(if app.session_state.search().is_empty() {
            "No sessions match the current directory or provider."
        } else {
            "No sessions match the search filter. Press Esc to clear search."
        })
        .block(Block::default().title("Empty").borders(Borders::ALL)),
        popup,
    );
}

pub(super) fn draw_error_dialog(frame: &mut ratatui::Frame<'_>, error: &str, area: Rect) {
    let popup = centered_rect(58, 18, area);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                error.to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::styled(
                "This message closes automatically.",
                Style::default().fg(Color::Gray),
            ),
        ])
        .block(Block::default().title("Error").borders(Borders::ALL)),
        popup,
    );
}

pub(super) fn draw_confirmation_dialog(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let Some((title, message)) = app.confirmation_dialog() else {
        return;
    };

    let help = app.confirmation_help();
    let popup_width = percent_len(area.width, 58)
        .max(area.width.min(36))
        .min(area.width);
    let inner_width = usize::from(popup_width.saturating_sub(2));
    let content_height = message
        .lines()
        .map(|line| wrap_text(line, inner_width).len())
        .sum::<usize>()
        .saturating_add(1)
        .saturating_add(wrap_text(help, inner_width).len());
    let popup_height = u16::try_from(content_height)
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .max(7);
    let popup = centered_rect_size(popup_width, popup_height, area);
    let mut lines = message
        .lines()
        .map(|line| Line::raw(line.to_string()))
        .collect::<Vec<_>>();
    lines.push(Line::raw(""));
    lines.push(Line::styled(help, Style::default().fg(Color::Gray)));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().title(title).borders(Borders::ALL)),
        popup,
    );
}

fn page_item_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}
