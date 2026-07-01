use ratatui::text::{Line, Span};

use super::layout::wrap_text;

pub(super) fn markdown_lines(text: &str, width: usize) -> Vec<Line<'static>> {
    wrap_text(&clean_control_chars(text), width)
        .into_iter()
        .map(|line| Line::from(vec![Span::raw(line)]))
        .collect()
}

fn clean_control_chars(text: &str) -> String {
    text.chars()
        .filter(|ch| *ch == '\n' || *ch == '\t' || !ch.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::text::Line;
    use unicode_width::UnicodeWidthStr;

    use super::markdown_lines;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn plain_text_markdown_lines_wrap_by_display_width() {
        let lines = markdown_lines("alpha beta gamma delta", 12);

        assert!(lines.len() > 1);
        assert_eq!(
            lines.iter().map(line_text).collect::<Vec<_>>().join(" "),
            "alpha beta gamma delta"
        );
        assert!(
            lines
                .iter()
                .all(|line| UnicodeWidthStr::width(line_text(line).as_str()) <= 12)
        );
    }

    #[test]
    fn empty_markdown_lines_returns_one_empty_line() {
        let lines = markdown_lines("", 20);

        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "");
    }
}
