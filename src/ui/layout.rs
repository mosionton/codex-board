use std::path::Path;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn details_dialog_height(line_count: usize, area_height: u16) -> u16 {
    let line_count = u16::try_from(line_count).unwrap_or(u16::MAX);
    let desired = line_count.saturating_add(2).max(3);
    let max_height = area_height.saturating_sub(2).max(area_height.min(3));
    desired.min(max_height).min(area_height)
}

pub(super) fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    for segment in text.split('\n') {
        let mut current = String::new();
        for word in segment.split_whitespace() {
            let word_len = UnicodeWidthStr::width(word);
            let current_len = UnicodeWidthStr::width(current.as_str());
            let needed = if current.is_empty() {
                word_len
            } else {
                current_len + 1 + word_len
            };
            if needed > width && !current.is_empty() {
                lines.push(current);
                current = String::new();
            }
            if word_len > width {
                if !current.is_empty() {
                    lines.push(current);
                    current = String::new();
                }
                lines.extend(split_word_by_width(word, width));
                continue;
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(super) fn split_word_by_width(word: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for ch in word.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if !current.is_empty() && current_width + ch_width > width {
            lines.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub(super) fn compact_path(path: &Path) -> String {
    const MAX: usize = 52;
    let text = path.to_string_lossy();
    if UnicodeWidthStr::width(text.as_ref()) <= MAX {
        return text.into_owned();
    }

    let tail_width = MAX.saturating_sub(UnicodeWidthChar::width('…').unwrap_or(1));
    let mut width = 0;
    let mut tail_chars = Vec::new();
    for ch in text.chars().rev() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > tail_width {
            break;
        }
        tail_chars.push(ch);
        width += ch_width;
    }
    let tail = tail_chars.into_iter().rev().collect::<String>();
    format!("…{tail}")
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

pub(super) const fn centered_rect_size(width: u16, height: u16, area: Rect) -> Rect {
    let width = if width > area.width {
        area.width
    } else {
        width
    };
    let height = if height > area.height {
        area.height
    } else {
        height
    };
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub(super) fn percent_len(total: u16, percent: u16) -> u16 {
    u16::try_from((u32::from(total) * u32::from(percent)) / 100).unwrap_or(u16::MAX)
}
