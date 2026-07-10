use ratatui::{
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{ProviderEditor, ProviderField, WIRE_API_OPTIONS};

use super::{
    input_view::{input_cursor_position_at, text_with_cursor_gap_spans},
    layout::centered_rect,
};

pub(super) fn draw_provider_editor(
    frame: &mut ratatui::Frame<'_>,
    editor: &ProviderEditor,
    area: Rect,
) {
    let popup = centered_rect(70, 46, area);
    frame.render_widget(Clear, popup);

    let mut lines = vec![
        provider_editor_line(
            editor,
            ProviderField::Auth,
            "auth_mode",
            editor.auth_mode_display(),
        ),
        provider_editor_line(editor, ProviderField::Id, "id", editor.id.as_str()),
        provider_editor_line(
            editor,
            ProviderField::BaseUrl,
            "base_url",
            editor.base_url.as_str(),
        ),
        provider_editor_line(
            editor,
            ProviderField::ApiKey,
            "api_key",
            editor.api_key.as_str(),
        ),
        provider_editor_line(editor, ProviderField::WireApi, "wire_api", &editor.wire_api),
        provider_editor_line(editor, ProviderField::Model, "model", editor.model.as_str()),
        provider_editor_line(
            editor,
            ProviderField::ReasoningEffort,
            "reason",
            empty_display(&editor.reasoning_effort),
        ),
        provider_editor_line(
            editor,
            ProviderField::PlanReasoningEffort,
            "plan_reason",
            empty_display(&editor.plan_reasoning_effort),
        ),
        Line::raw(""),
    ];
    if let Some(options_line) = provider_editor_options_line(editor) {
        lines.push(options_line);
    }
    lines.push(Line::styled(
        "Tab/Shift+Tab field | F5 fetch models | Left/Right cycles option fields | Ctrl+U clears | Enter saves | Esc cancels",
        Style::default().fg(Color::Gray),
    ));

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title("Edit Provider")
                .borders(Borders::ALL),
        ),
        popup,
    );
    if let Some(position) = provider_editor_cursor_position(popup, editor) {
        frame.set_cursor_position(position);
    }
}

fn provider_editor_line<'a>(
    editor: &ProviderEditor,
    field: ProviderField,
    label: &'a str,
    value: &'a str,
) -> Line<'static> {
    let active = editor.active_field == field;
    let label_style = if active {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let mut spans = vec![
        Span::styled(format!("{label:<12}"), label_style),
        Span::raw(" "),
    ];
    if active && let Some(cursor) = editor.text_cursor_for(field) {
        spans.extend(text_with_cursor_gap_spans(value, cursor, None));
    } else {
        spans.push(Span::raw(empty_display(value).to_string()));
    }
    Line::from(spans)
}

fn provider_editor_cursor_position(popup: Rect, editor: &ProviderEditor) -> Option<Position> {
    let row = provider_editor_field_row(editor.active_field)?;
    let (value, cursor) = provider_editor_active_text(editor)?;
    let label_width = 13;
    Some(input_cursor_position_at(
        popup,
        row,
        label_width,
        value,
        cursor,
    ))
}

const fn provider_editor_field_row(field: ProviderField) -> Option<u16> {
    match field {
        ProviderField::Id => Some(1),
        ProviderField::BaseUrl => Some(2),
        ProviderField::ApiKey => Some(3),
        ProviderField::Model => Some(5),
        ProviderField::WireApi
        | ProviderField::Auth
        | ProviderField::ReasoningEffort
        | ProviderField::PlanReasoningEffort => None,
    }
}

fn provider_editor_active_text(editor: &ProviderEditor) -> Option<(&str, usize)> {
    match editor.active_field {
        ProviderField::Id => Some((editor.id.as_str(), editor.id.cursor())),
        ProviderField::BaseUrl => Some((editor.base_url.as_str(), editor.base_url.cursor())),
        ProviderField::ApiKey => Some((editor.api_key.as_str(), editor.api_key.cursor())),
        ProviderField::Model => Some((editor.model.as_str(), editor.model.cursor())),
        ProviderField::WireApi
        | ProviderField::Auth
        | ProviderField::ReasoningEffort
        | ProviderField::PlanReasoningEffort => None,
    }
}

const fn empty_display(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}

fn provider_editor_options_line(editor: &ProviderEditor) -> Option<Line<'static>> {
    if editor.active_field == ProviderField::Model && !editor.model_options.is_empty() {
        let current = editor
            .model_options
            .iter()
            .position(|model| model == editor.model.trim())
            .map_or(1, |index| index + 1);
        return Some(Line::styled(
            format!(
                "Models: {current}/{} | Up/Down cycles fetched models | F5 reloads",
                editor.model_options.len()
            ),
            Style::default().fg(Color::Gray),
        ));
    }

    let text = provider_field_options_text(editor)?;
    Some(Line::styled(
        format!("Options: {text}"),
        Style::default().fg(Color::Gray),
    ))
}

fn provider_field_options_text(editor: &ProviderEditor) -> Option<String> {
    match editor.active_field {
        ProviderField::ReasoningEffort => Some(editor.reasoning_effort_options.join(" | ")),
        ProviderField::PlanReasoningEffort => {
            Some(editor.plan_reasoning_effort_options.join(" | "))
        }
        ProviderField::WireApi => Some(WIRE_API_OPTIONS.join(" | ")),
        ProviderField::Id
        | ProviderField::Model
        | ProviderField::ApiKey
        | ProviderField::BaseUrl
        | ProviderField::Auth => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::provider_config::ModelCatalog;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn cursor_position_tracks_active_text_field() {
        let mut editor = ProviderEditor::new();
        editor.base_url.set_with_cursor("你a", 1);
        editor.active_field = ProviderField::BaseUrl;
        let popup = Rect {
            x: 10,
            y: 4,
            width: 60,
            height: 20,
        };

        assert_eq!(
            provider_editor_cursor_position(popup, &editor),
            Some(Position::new(26, 7))
        );
    }

    #[test]
    fn option_and_cursor_helpers_cover_text_and_option_fields() {
        let mut editor = ProviderEditor::new();
        editor.active_field = ProviderField::Model;
        editor.model.set("gpt-5.5");
        editor.model_options = vec![
            "gpt-5-mini".to_string(),
            "gpt-5.5".to_string(),
            "o4".to_string(),
        ];
        let model_line = provider_editor_options_line(&editor).unwrap();
        assert!(line_text(&model_line).contains("Models: 2/3"));
        assert_eq!(
            provider_editor_cursor_position(
                Rect {
                    x: 0,
                    y: 0,
                    width: 40,
                    height: 10
                },
                &editor
            ),
            Some(Position::new(21, 6))
        );

        editor.active_field = ProviderField::WireApi;
        let options_line = provider_editor_options_line(&editor).unwrap();
        assert_eq!(line_text(&options_line), "Options: responses | chat");
        assert_eq!(
            provider_editor_cursor_position(Rect::new(0, 0, 40, 10), &editor),
            None
        );
        assert_eq!(provider_editor_active_text(&editor), None);
    }

    #[test]
    fn reasoning_options_follow_selected_model() {
        let catalog = Arc::new(ModelCatalog::from_json(
            r#"{"models":[
              {"slug":"gpt-5.6-sol","default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"},{"effort":"ultra"}]},
              {"slug":"gpt-5.6-luna","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"},{"effort":"max"}]}
            ]}"#,
        ).unwrap());
        let mut editor = ProviderEditor::new_with_catalog(catalog);
        editor.model.set("gpt-5.6-sol");
        editor.commit_model_change();
        editor.active_field = ProviderField::ReasoningEffort;
        assert!(line_text(&provider_editor_options_line(&editor).unwrap()).contains("max | ultra"));
        editor.model.set("gpt-5.6-luna");
        editor.commit_model_change();
        assert!(!line_text(&provider_editor_options_line(&editor).unwrap()).contains("ultra"));
    }

    #[test]
    fn empty_display_uses_dash_fallback() {
        assert_eq!(empty_display(""), "-");
        assert_eq!(empty_display("value"), "value");
    }
}
