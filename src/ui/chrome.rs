use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{App, Page, Scope, SessionViewMode};

use super::layout::centered_rect;

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
                "Enter resume | i details | c conversation | r reload | v view | t toggle pages | Tab provider | a scope | / search | q quit"
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

    let popup = centered_rect(58, 22, area);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(message),
            Line::raw(""),
            Line::styled(
                "Enter/y confirms. Esc/n cancels.",
                Style::default().fg(Color::Gray),
            ),
        ])
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
