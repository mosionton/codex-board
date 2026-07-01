use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn markdown_lines(text: &str, width: usize) -> Vec<Line<'static>> {
    wrap_spans(vec![Span::raw(clean_control_chars(text))], width)
}

fn clean_control_chars(text: &str) -> String {
    text.chars()
        .filter(|ch| *ch == '\n' || *ch == '\t' || !ch.is_control())
        .collect()
}

fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from(spans)];
    }

    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0;

    for token in split_preserving_spaces(&spans) {
        match token {
            Token::Newline => {
                if !current.is_empty() {
                    push_wrapped_line(&mut lines, &mut current, &mut current_width);
                }
            }
            Token::Word {
                spans,
                preceded_by_space,
                space_style,
            } => {
                push_split_token(
                    &mut lines,
                    &mut current,
                    &mut current_width,
                    spans,
                    preceded_by_space,
                    space_style,
                    width,
                );
            }
        }
    }

    if !current.is_empty() {
        push_wrapped_line(&mut lines, &mut current, &mut current_width);
    }

    if lines.is_empty() {
        lines.push(Line::from(Vec::new()));
    }

    lines
}

#[derive(Debug)]
enum Token {
    Word {
        spans: Vec<Span<'static>>,
        preceded_by_space: bool,
        space_style: Style,
    },
    Newline,
}

fn split_preserving_spaces(spans: &[Span<'static>]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut buffer = String::new();
    let mut buffer_style = Style::default();
    let mut word_spans = Vec::new();
    let mut preceded_by_space = false;
    let mut pending_space = false;
    let mut pending_space_style = Style::default();

    let flush_buffer =
        |word_spans: &mut Vec<Span<'static>>, buffer: &mut String, buffer_style: Style| {
            if !buffer.is_empty() {
                word_spans.push(Span::styled(std::mem::take(buffer), buffer_style));
            }
        };

    for span in spans {
        for ch in span.content.as_ref().chars() {
            match ch {
                '\n' => {
                    flush_buffer(&mut word_spans, &mut buffer, buffer_style);
                    if !word_spans.is_empty() {
                        tokens.push(Token::Word {
                            spans: std::mem::take(&mut word_spans),
                            preceded_by_space,
                            space_style: pending_space_style,
                        });
                    }
                    tokens.push(Token::Newline);
                    preceded_by_space = false;
                    pending_space = false;
                    pending_space_style = Style::default();
                }
                ch if ch.is_whitespace() => {
                    flush_buffer(&mut word_spans, &mut buffer, buffer_style);
                    if !word_spans.is_empty() {
                        tokens.push(Token::Word {
                            spans: std::mem::take(&mut word_spans),
                            preceded_by_space,
                            space_style: pending_space_style,
                        });
                        preceded_by_space = false;
                    }
                    pending_space = true;
                    pending_space_style = span.style;
                }
                _ => {
                    if buffer.is_empty() {
                        buffer_style = span.style;
                        preceded_by_space = pending_space;
                        pending_space = false;
                    } else if buffer_style != span.style {
                        flush_buffer(&mut word_spans, &mut buffer, buffer_style);
                        buffer_style = span.style;
                    }
                    buffer.push(ch);
                }
            }
        }
    }

    flush_buffer(&mut word_spans, &mut buffer, buffer_style);
    if !word_spans.is_empty() {
        tokens.push(Token::Word {
            spans: word_spans,
            preceded_by_space,
            space_style: pending_space_style,
        });
    }

    tokens
}

fn push_split_token(
    lines: &mut Vec<Line<'static>>,
    current: &mut Vec<Span<'static>>,
    current_width: &mut usize,
    spans: Vec<Span<'static>>,
    preceded_by_space: bool,
    space_style: Style,
    width: usize,
) {
    let word_width = spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum::<usize>();
    if word_width > width {
        if !current.is_empty() {
            push_wrapped_line(lines, current, current_width);
        }
        push_split_word(lines, spans, width);
        return;
    }

    let needs_space = preceded_by_space && !current.is_empty();
    if *current_width + usize::from(needs_space) + word_width > width && !current.is_empty() {
        push_wrapped_line(lines, current, current_width);
    }

    if preceded_by_space && !current.is_empty() {
        current.push(Span::styled(" ", space_style));
        *current_width += 1;
    }

    *current_width += word_width;
    current.extend(spans);
}

fn push_split_word(lines: &mut Vec<Line<'static>>, spans: Vec<Span<'static>>, width: usize) {
    let mut current = Vec::new();
    let mut current_width = 0;

    for span in spans {
        for ch in span.content.as_ref().chars() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width + ch_width > width && !current.is_empty() {
                lines.push(Line::from(std::mem::take(&mut current)));
                current_width = 0;
            }
            push_char(&mut current, ch, span.style);
            current_width += ch_width;
        }
    }

    if !current.is_empty() {
        lines.push(Line::from(current));
    }
}

fn push_char(line: &mut Vec<Span<'static>>, ch: char, style: Style) {
    if let Some(last) = line.last_mut()
        && last.style == style
    {
        last.content.to_mut().push(ch);
        return;
    }

    line.push(Span::styled(ch.to_string(), style));
}

fn push_wrapped_line(
    lines: &mut Vec<Line<'static>>,
    current: &mut Vec<Span<'static>>,
    current_width: &mut usize,
) {
    lines.push(Line::from(std::mem::take(current)));
    *current_width = 0;
}

#[cfg(test)]
mod tests {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::Span,
    };
    use unicode_width::UnicodeWidthStr;

    use ratatui::text::Line;

    use super::{markdown_lines, wrap_spans};

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn line_has_modifier(line: &Line<'_>, modifier: Modifier) -> bool {
        line.spans
            .iter()
            .any(|span| span.style.add_modifier.contains(modifier))
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

    #[test]
    fn markdown_lines_match_existing_wrap_text_newline_semantics() {
        let lines = markdown_lines("a\n\nb\n", 20);

        assert_eq!(lines.iter().map(line_text).collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn wrap_styled_spans_width_zero_returns_original_spans() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let lines = wrap_spans(vec![Span::styled("alpha beta", bold)], 0);

        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "alpha beta");
        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn wrap_styled_spans_preserves_style_across_wrapped_lines() {
        let style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let lines = wrap_spans(vec![Span::styled("alpha beta gamma", style)], 8);

        assert_eq!(
            lines.iter().map(line_text).collect::<Vec<_>>(),
            ["alpha", "beta", "gamma"]
        );
        assert!(
            lines
                .iter()
                .all(|line| line_has_modifier(line, Modifier::BOLD))
        );
    }

    #[test]
    fn wrap_styled_spans_preserves_adjacent_styles_without_whitespace() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let italic = Style::default().add_modifier(Modifier::ITALIC);
        let lines = wrap_spans(
            vec![Span::styled("ab", bold), Span::styled("cd", italic)],
            10,
        );

        assert_eq!(lines.iter().map(line_text).collect::<Vec<_>>(), ["abcd"]);
        assert_eq!(lines[0].spans.len(), 2);
        assert_eq!(lines[0].spans[0].content.as_ref(), "ab");
        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(lines[0].spans[1].content.as_ref(), "cd");
        assert!(
            lines[0].spans[1]
                .style
                .add_modifier
                .contains(Modifier::ITALIC)
        );
    }

    #[test]
    fn wrap_styled_spans_preserves_space_style_before_styled_word() {
        let italic = Style::default().add_modifier(Modifier::ITALIC);
        let lines = wrap_spans(vec![Span::raw("ab "), Span::styled("cd", italic)], 20);

        assert_eq!(lines.iter().map(line_text).collect::<Vec<_>>(), ["ab cd"]);
        assert_eq!(lines[0].spans.len(), 3);
        assert_eq!(lines[0].spans[0].content.as_ref(), "ab");
        assert!(
            !lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::ITALIC)
        );
        assert_eq!(lines[0].spans[1].content.as_ref(), " ");
        assert!(
            !lines[0].spans[1]
                .style
                .add_modifier
                .contains(Modifier::ITALIC)
        );
        assert_eq!(lines[0].spans[2].content.as_ref(), "cd");
        assert!(
            lines[0].spans[2]
                .style
                .add_modifier
                .contains(Modifier::ITALIC)
        );
    }

    #[test]
    fn wrap_styled_spans_preserves_styles_when_splitting_adjacent_spans() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let italic = Style::default().add_modifier(Modifier::ITALIC);
        let lines = wrap_spans(
            vec![Span::styled("你好", bold), Span::styled("ab", italic)],
            4,
        );

        assert_eq!(
            lines.iter().map(line_text).collect::<Vec<_>>(),
            ["你好", "ab"]
        );
        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert!(
            lines[1].spans[0]
                .style
                .add_modifier
                .contains(Modifier::ITALIC)
        );
    }

    #[test]
    fn wrap_styled_spans_splits_wide_unicode_words() {
        let lines = wrap_spans(vec![Span::raw("你好abcdef")], 4);

        assert!(lines.len() > 1);
        assert_eq!(
            lines.iter().map(line_text).collect::<String>(),
            "你好abcdef"
        );
        assert!(
            lines
                .iter()
                .all(|line| UnicodeWidthStr::width(line_text(line).as_str()) <= 4)
        );
    }
}
