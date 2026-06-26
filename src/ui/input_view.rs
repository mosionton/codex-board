use ratatui::{
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::app::char_count;

pub(super) fn input_line(label: &'static str, text: &str, cursor: usize) -> Line<'static> {
    let mut spans = vec![Span::styled(
        label,
        Style::default().add_modifier(Modifier::BOLD),
    )];
    let cursor = cursor.min(char_count(text));

    if text.is_empty() {
        spans.extend(text_with_cursor_gap_spans(text, cursor, Some("<empty>")));
        return Line::from(spans);
    }

    spans.extend(text_with_cursor_gap_spans(text, cursor, None));
    Line::from(spans)
}

pub(super) fn text_with_cursor_gap_spans(
    text: &str,
    cursor: usize,
    empty_placeholder: Option<&'static str>,
) -> Vec<Span<'static>> {
    let cursor = cursor.min(char_count(text));
    let mut spans = Vec::new();
    let before = text.chars().take(cursor).collect::<String>();
    let after = text.chars().skip(cursor).collect::<String>();

    if !before.is_empty() {
        spans.push(Span::raw(before));
    }
    spans.push(Span::raw(" "));
    if text.is_empty()
        && let Some(placeholder) = empty_placeholder
    {
        spans.push(Span::styled(
            placeholder,
            Style::default().fg(Color::DarkGray),
        ));
    } else if !after.is_empty() {
        spans.push(Span::raw(after));
    }
    spans
}

pub(super) fn input_cursor_position(
    area: Rect,
    label: &str,
    text: &str,
    cursor: usize,
) -> Position {
    input_cursor_position_at(area, 0, UnicodeWidthStr::width(label), text, cursor)
}

pub(super) fn input_cursor_position_at(
    area: Rect,
    row: u16,
    prefix_width: usize,
    text: &str,
    cursor: usize,
) -> Position {
    let inner_x = area.x.saturating_add(1);
    let inner_y = area.y.saturating_add(1).saturating_add(row);
    let inner_width = area.width.saturating_sub(2);
    let cursor = cursor.min(char_count(text));
    let text_before_cursor = text.chars().take(cursor).collect::<String>();
    let offset = prefix_width + UnicodeWidthStr::width(text_before_cursor.as_str());
    let offset = u16::try_from(offset)
        .unwrap_or(u16::MAX)
        .min(inner_width.saturating_sub(1));
    Position::new(inner_x.saturating_add(offset), inner_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn cursor_position_uses_display_width() {
        let area = Rect {
            x: 10,
            y: 4,
            width: 40,
            height: 8,
        };

        assert_eq!(
            input_cursor_position(area, "Query: ", "你a", 1),
            Position::new(20, 5)
        );
        assert_eq!(
            input_cursor_position(area, "Query: ", "你a", 2),
            Position::new(21, 5)
        );
    }

    #[test]
    fn input_helpers_show_placeholder_and_cursor_gap() {
        let empty = input_line("Query: ", "", 0);
        assert!(line_text(&empty).contains("<empty>"));
        assert_eq!(empty.spans[2].style.fg, Some(Color::DarkGray));

        let filled = input_line("Query: ", "abc", 2);
        assert_eq!(line_text(&filled), "Query: ab c");

        let spans = text_with_cursor_gap_spans("abc", 1, None);
        let text = spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(text, "a bc");
    }
}
