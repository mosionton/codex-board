use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const QUOTE_PREFIX: &str = "> ";

pub(super) fn markdown_lines(text: &str, width: usize) -> Vec<Line<'static>> {
    let text = clean_control_chars(text);
    if text.is_empty() {
        return vec![Line::raw("")];
    }

    let mut renderer = MarkdownRenderer::new(width);
    for event in Parser::new_ext(&text, markdown_options()) {
        renderer.push_event(event);
    }
    renderer.finish()
}

fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_GFM);
    options
}

fn heading_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

fn quote_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn inline_code_style() -> Style {
    Style::default().fg(Color::Magenta)
}

fn link_style() -> Style {
    Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::UNDERLINED)
}

fn math_style() -> Style {
    Style::default().fg(Color::Cyan)
}

fn html_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

#[derive(Clone, Copy, Default)]
struct TextStyle {
    style: Style,
}

struct ListState {
    next_number: Option<u64>,
}

struct ItemPrefixState {
    marker: String,
    continuation: String,
    pending_marker: bool,
}

enum BlockPrefixState {
    Quote,
    Item(ItemPrefixState),
}

#[derive(Default)]
struct CodeBlockState {
    language: String,
    buffer: String,
}

#[derive(Default)]
struct TableState {
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    in_cell: bool,
}

struct ImageState {
    url: String,
    has_alt: bool,
}

struct MarkdownRenderer {
    width: usize,
    lines: Vec<Line<'static>>,
    current: Vec<Span<'static>>,
    styles: Vec<TextStyle>,
    lists: Vec<ListState>,
    block_prefixes: Vec<BlockPrefixState>,
    link_stack: Vec<String>,
    image_stack: Vec<ImageState>,
    code_block: Option<CodeBlockState>,
    table: Option<TableState>,
}

impl MarkdownRenderer {
    const fn new(width: usize) -> Self {
        Self {
            width,
            lines: Vec::new(),
            current: Vec::new(),
            styles: Vec::new(),
            lists: Vec::new(),
            block_prefixes: Vec::new(),
            link_stack: Vec::new(),
            image_stack: Vec::new(),
            code_block: None,
            table: None,
        }
    }

    fn push_event(&mut self, event: Event<'_>) {
        if self.push_code_block_event(&event) {
            return;
        }
        if self.push_table_event(&event) {
            return;
        }

        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.push_text(text.as_ref()),
            Event::Code(text) => self.push_styled_text(text.as_ref(), inline_code_style()),
            Event::InlineMath(text) => {
                self.push_styled_text(format!("${}$", text.as_ref()), math_style());
            }
            Event::DisplayMath(text) => {
                self.push_styled_text(format!("$${}$$", text.as_ref()), math_style());
            }
            Event::Html(text) | Event::InlineHtml(text) => {
                self.push_styled_text(text.as_ref(), html_style());
            }
            Event::FootnoteReference(label) => {
                self.push_text(format!("[^{}]", label.as_ref()));
            }
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.flush_block(),
            Event::Rule => self.push_rule(),
            Event::TaskListMarker(checked) => self.push_task_list_marker(checked),
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Subscript
            | Tag::Superscript
            | Tag::MetadataBlock(_)
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell => {}
            Tag::Link { dest_url, .. } => self.link_stack.push(dest_url.to_string()),
            Tag::Image { dest_url, .. } => {
                self.image_stack.push(ImageState {
                    url: dest_url.to_string(),
                    has_alt: false,
                });
            }
            Tag::Heading { level, .. } => {
                self.flush_block();
                self.push_style(heading_style());
                self.push_styled_text(format!("{} ", heading_marker(level)), heading_style());
            }
            Tag::BlockQuote(_) => self.push_quote_prefix_state(),
            Tag::List(start) => self.lists.push(ListState { next_number: start }),
            Tag::Item => {
                self.flush_block();
                self.push_item_prefix_state();
            }
            Tag::Emphasis => self.push_style(Style::default().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.push_style(Style::default().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => {
                self.push_style(Style::default().add_modifier(Modifier::CROSSED_OUT));
            }
            Tag::CodeBlock(kind) => self.start_code_block(kind),
            Tag::HtmlBlock => {
                self.flush_block();
                self.push_style(html_style());
            }
            Tag::FootnoteDefinition(label) => {
                self.flush_block();
                self.push_text(format!("[^{}]: ", label.as_ref()));
            }
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::FootnoteDefinition => self.flush_block(),
            TagEnd::Item => {
                self.flush_block();
                self.pop_block_prefix(|prefix| matches!(prefix, BlockPrefixState::Item(_)));
            }
            TagEnd::Heading(_) | TagEnd::HtmlBlock => {
                self.flush_block();
                let _ = self.styles.pop();
            }
            TagEnd::CodeBlock => self.flush_code_block(),
            TagEnd::BlockQuote(_) => {
                self.flush_block();
                self.pop_block_prefix(|prefix| matches!(prefix, BlockPrefixState::Quote));
            }
            TagEnd::List(_) => {
                self.flush_block();
                let _ = self.lists.pop();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                let _ = self.styles.pop();
            }
            TagEnd::Link => {
                if let Some(url) = self.link_stack.pop() {
                    self.push_styled_text(format!(" <{url}>"), link_style());
                }
            }
            TagEnd::Image => {
                if let Some(image) = self.image_stack.pop() {
                    if image.has_alt {
                        self.push_text("]");
                        self.push_styled_text(format!(" <{}>", image.url), link_style());
                    } else {
                        self.push_styled_text(format!("<image: {}>", image.url), link_style());
                    }
                }
            }
            TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::MetadataBlock(_)
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Subscript
            | TagEnd::Superscript => {}
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush_block();
        if self.lines.is_empty() {
            return vec![Line::raw("")];
        }
        self.lines
    }

    fn push_style(&mut self, style: Style) {
        self.styles.push(TextStyle { style });
    }

    fn current_style(&self) -> Style {
        self.styles
            .iter()
            .fold(Style::default(), |style, entry| style.patch(entry.style))
    }

    fn push_text<T>(&mut self, text: T)
    where
        T: Into<String>,
    {
        let style = self.current_style();
        self.push_span_text(text, style);
    }

    fn push_styled_text<T>(&mut self, text: T, style: Style)
    where
        T: Into<String>,
    {
        self.push_span_text(text, self.current_style().patch(style));
    }

    fn push_span_text<T>(&mut self, text: T, style: Style)
    where
        T: Into<String>,
    {
        let text = text.into();
        if text.is_empty() {
            return;
        }

        if let Some(image) = self.image_stack.last_mut()
            && !image.has_alt
        {
            self.current.push(Span::raw("!["));
            image.has_alt = true;
        }
        self.current.push(Span::styled(text, style));
    }

    fn flush_block(&mut self) {
        if self.current.is_empty() {
            return;
        }

        let first_prefix = self.first_line_prefix_spans();
        let continuation_prefix = self.continuation_prefix_spans();
        self.lines.extend(wrap_spans_with_prefix(
            std::mem::take(&mut self.current),
            &first_prefix,
            &continuation_prefix,
            self.width,
        ));
    }

    fn push_code_block_event(&mut self, event: &Event<'_>) -> bool {
        let Some(code_block) = self.code_block.as_mut() else {
            return false;
        };

        match event {
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                code_block.buffer.push_str(text.as_ref());
            }
            Event::SoftBreak | Event::HardBreak => code_block.buffer.push('\n'),
            Event::End(TagEnd::CodeBlock) => {
                self.flush_code_block();
            }
            Event::End(_) | Event::Start(_) => {}
            Event::InlineMath(text) => {
                code_block.buffer.push('$');
                code_block.buffer.push_str(text.as_ref());
                code_block.buffer.push('$');
            }
            Event::DisplayMath(text) => {
                code_block.buffer.push_str("$$");
                code_block.buffer.push_str(text.as_ref());
                code_block.buffer.push_str("$$");
            }
            Event::FootnoteReference(label) => {
                code_block.buffer.push_str("[^");
                code_block.buffer.push_str(label.as_ref());
                code_block.buffer.push(']');
            }
            Event::Rule => code_block.buffer.push_str("---"),
            Event::TaskListMarker(checked) => {
                code_block
                    .buffer
                    .push_str(if *checked { "[x]" } else { "[ ]" });
            }
        }

        true
    }

    fn push_table_event(&mut self, event: &Event<'_>) -> bool {
        match event {
            Event::Start(Tag::Table(_)) => {
                self.flush_block();
                self.table = Some(TableState::default());
                true
            }
            Event::End(TagEnd::Table) => {
                self.flush_table();
                true
            }
            Event::Start(Tag::Link { dest_url, .. })
                if self.table.as_ref().is_some_and(|table| table.in_cell) =>
            {
                self.link_stack.push(dest_url.to_string());
                true
            }
            Event::End(TagEnd::Link) if self.table.as_ref().is_some_and(|table| table.in_cell) => {
                if let Some(url) = self.link_stack.pop() {
                    self.push_table_text(format!(" <{url}>"));
                }
                true
            }
            Event::Start(Tag::Image { dest_url, .. })
                if self.table.as_ref().is_some_and(|table| table.in_cell) =>
            {
                self.image_stack.push(ImageState {
                    url: dest_url.to_string(),
                    has_alt: false,
                });
                true
            }
            Event::End(TagEnd::Image) if self.table.as_ref().is_some_and(|table| table.in_cell) => {
                if let Some(image) = self.image_stack.pop() {
                    if image.has_alt {
                        self.push_table_text(format!("] <{}>", image.url));
                    } else {
                        self.push_table_text(format!("<image: {}>", image.url));
                    }
                }
                true
            }
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                self.start_table_row();
                true
            }
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                self.end_table_row();
                true
            }
            Event::Start(Tag::TableCell) => {
                self.start_table_cell();
                true
            }
            Event::End(TagEnd::TableCell) => {
                self.end_table_cell();
                true
            }
            Event::Text(text) if self.table.as_ref().is_some_and(|table| table.in_cell) => {
                self.push_table_text(text.as_ref());
                true
            }
            Event::Code(text) if self.table.as_ref().is_some_and(|table| table.in_cell) => {
                self.push_table_text(text.as_ref());
                true
            }
            Event::InlineMath(text) if self.table.as_ref().is_some_and(|table| table.in_cell) => {
                self.push_table_text(format!("${}$", text.as_ref()));
                true
            }
            Event::DisplayMath(text) if self.table.as_ref().is_some_and(|table| table.in_cell) => {
                self.push_table_text(format!("$${}$$", text.as_ref()));
                true
            }
            Event::Html(text) | Event::InlineHtml(text)
                if self.table.as_ref().is_some_and(|table| table.in_cell) =>
            {
                self.push_table_text(text.as_ref());
                true
            }
            Event::SoftBreak | Event::HardBreak
                if self.table.as_ref().is_some_and(|table| table.in_cell) =>
            {
                self.push_table_text(" ");
                true
            }
            Event::Start(_) | Event::End(_) if self.table.is_some() => true,
            _ => false,
        }
    }

    fn start_code_block(&mut self, kind: CodeBlockKind<'_>) {
        self.flush_block();
        let language = match kind {
            CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_string(),
            CodeBlockKind::Indented => String::new(),
        };
        self.code_block = Some(CodeBlockState {
            language,
            buffer: String::new(),
        });
    }

    fn flush_code_block(&mut self) {
        let Some(code_block) = self.code_block.take() else {
            return;
        };

        let style = inline_code_style();
        let fence = if code_block.language.is_empty() {
            "```".to_string()
        } else {
            format!("```{}", code_block.language)
        };
        self.push_preformatted_line(&fence, style);
        for line in code_block.buffer.split('\n') {
            self.push_preformatted_line(line, style);
        }
        self.push_preformatted_line("```", style);
    }

    fn first_line_prefix_spans(&mut self) -> Vec<Span<'static>> {
        let mut prefix = Vec::new();
        for block_prefix in &mut self.block_prefixes {
            match block_prefix {
                BlockPrefixState::Quote => prefix.push(Span::styled(QUOTE_PREFIX, quote_style())),
                BlockPrefixState::Item(item_prefix) => {
                    let segment = if item_prefix.pending_marker {
                        item_prefix.pending_marker = false;
                        item_prefix.marker.clone()
                    } else {
                        item_prefix.continuation.clone()
                    };
                    prefix.push(Span::raw(segment));
                }
            }
        }
        prefix
    }

    fn continuation_prefix_spans(&self) -> Vec<Span<'static>> {
        let mut prefix = Vec::new();
        for block_prefix in &self.block_prefixes {
            match block_prefix {
                BlockPrefixState::Quote => prefix.push(Span::styled(QUOTE_PREFIX, quote_style())),
                BlockPrefixState::Item(item_prefix) => {
                    prefix.push(Span::raw(item_prefix.continuation.clone()));
                }
            }
        }
        prefix
    }

    fn push_rule(&mut self) {
        self.flush_block();
        self.push_preformatted_line("---", quote_style().add_modifier(Modifier::DIM));
    }

    fn push_task_list_marker(&mut self, checked: bool) {
        let marker = if checked { "[x] " } else { "[ ] " };
        let marker_width = UnicodeWidthStr::width(marker);
        if let Some(item_prefix) = self.current_item_prefix_mut() {
            item_prefix.continuation.reserve(marker_width);
            for _ in 0..marker_width {
                item_prefix.continuation.push(' ');
            }
        }
        self.push_text(marker);
    }

    fn next_item_prefix(&mut self) -> String {
        self.lists
            .last_mut()
            .and_then(|state| state.next_number.as_mut())
            .map_or_else(
                || "- ".to_string(),
                |next_number| {
                    let prefix = format!("{next_number}. ");
                    *next_number += 1;
                    prefix
                },
            )
    }

    fn push_item_prefix_state(&mut self) {
        let marker = self.next_item_prefix();
        let continuation_width = UnicodeWidthStr::width(marker.as_str());
        self.block_prefixes
            .push(BlockPrefixState::Item(ItemPrefixState {
                marker,
                continuation: " ".repeat(continuation_width),
                pending_marker: true,
            }));
    }

    fn push_quote_prefix_state(&mut self) {
        self.flush_block();
        self.block_prefixes.push(BlockPrefixState::Quote);
    }

    fn pop_block_prefix(&mut self, mut matches: impl FnMut(&BlockPrefixState) -> bool) {
        if let Some(index) = self.block_prefixes.iter().rposition(&mut matches) {
            let _ = self.block_prefixes.remove(index);
        }
    }

    fn current_item_prefix_mut(&mut self) -> Option<&mut ItemPrefixState> {
        self.block_prefixes
            .iter_mut()
            .rev()
            .find_map(|prefix| match prefix {
                BlockPrefixState::Quote => None,
                BlockPrefixState::Item(item_prefix) => Some(item_prefix),
            })
    }

    fn start_table_row(&mut self) {
        if let Some(table) = &mut self.table {
            table.current_row.clear();
        }
    }

    fn end_table_row(&mut self) {
        if let Some(table) = &mut self.table {
            table.rows.push(std::mem::take(&mut table.current_row));
        }
    }

    fn start_table_cell(&mut self) {
        if let Some(table) = &mut self.table {
            table.in_cell = true;
            table.current_cell.clear();
        }
    }

    fn end_table_cell(&mut self) {
        if let Some(table) = &mut self.table {
            table.in_cell = false;
            table
                .current_row
                .push(table.current_cell.trim().to_string());
        }
    }

    fn push_table_text<T>(&mut self, text: T)
    where
        T: AsRef<str>,
    {
        if let Some(table) = &mut self.table {
            if let Some(image) = self.image_stack.last_mut()
                && !image.has_alt
            {
                table.current_cell.push_str("![");
                image.has_alt = true;
            }
            table.current_cell.push_str(text.as_ref());
        }
    }

    fn flush_table(&mut self) {
        let Some(table) = self.table.take() else {
            return;
        };

        let column_count = table.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![3; column_count];
        for row in &table.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(escaped_table_cell_width(cell));
            }
        }

        for (index, row) in table.rows.iter().enumerate() {
            self.push_preformatted_line(&render_table_row(row, &widths), Style::default());
            if index == 0 {
                let separator = widths
                    .iter()
                    .map(|width| "-".repeat(*width))
                    .collect::<Vec<_>>();
                self.push_preformatted_line(
                    &render_table_row(&separator, &widths),
                    Style::default(),
                );
            }
        }
    }

    fn push_preformatted_line(&mut self, text: &str, style: Style) {
        let first_prefix = self.first_line_prefix_spans();
        let continuation_prefix = self.continuation_prefix_spans();
        self.lines.extend(wrap_preformatted_line_with_prefix(
            text,
            style,
            &first_prefix,
            &continuation_prefix,
            self.width,
        ));
    }
}

fn render_table_row(row: &[String], widths: &[usize]) -> String {
    let mut output = String::from("|");
    for (index, width) in widths.iter().enumerate() {
        let cell = row.get(index).map_or("", String::as_str);
        let cell = escape_table_cell(cell);
        let padding = width.saturating_sub(UnicodeWidthStr::width(cell.as_str()));
        output.push(' ');
        output.push_str(&cell);
        output.push_str(&" ".repeat(padding));
        output.push(' ');
        output.push('|');
    }
    output
}

fn escape_table_cell(cell: &str) -> String {
    cell.replace('|', "\\|")
}

fn escaped_table_cell_width(cell: &str) -> usize {
    UnicodeWidthStr::width(escape_table_cell(cell).as_str())
}

const fn heading_marker(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "#",
        HeadingLevel::H2 => "##",
        HeadingLevel::H3 => "###",
        HeadingLevel::H4 => "####",
        HeadingLevel::H5 => "#####",
        HeadingLevel::H6 => "######",
    }
}

fn clean_control_chars(text: &str) -> String {
    text.chars()
        .filter(|ch| *ch == '\n' || *ch == '\t' || !ch.is_control())
        .collect()
}

fn wrap_preformatted_line_with_prefix(
    text: &str,
    style: Style,
    first_prefix: &[Span<'static>],
    continuation_prefix: &[Span<'static>],
    width: usize,
) -> Vec<Line<'static>> {
    if width == 0 {
        let mut line = first_prefix.to_vec();
        line.push(Span::styled(text.to_string(), style));
        return vec![Line::from(line)];
    }

    let mut lines = Vec::new();
    let mut current = first_prefix.to_vec();
    let mut current_width = spans_width(&current);
    let mut prefix_width = current_width;

    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > width && current_width > prefix_width {
            lines.push(Line::from(std::mem::take(&mut current)));
            current = continuation_prefix.to_vec();
            current_width = spans_width(&current);
            prefix_width = current_width;
        }
        push_char(&mut current, ch, style);
        current_width += ch_width;
    }

    lines.push(Line::from(current));
    lines
}

fn wrap_spans_with_prefix(
    spans: Vec<Span<'static>>,
    first_prefix: &[Span<'static>],
    continuation_prefix: &[Span<'static>],
    width: usize,
) -> Vec<Line<'static>> {
    if width == 0 {
        let mut line = first_prefix.to_vec();
        line.extend(spans);
        return vec![Line::from(line)];
    }

    let mut lines = Vec::new();
    let mut state = WrapState::new(first_prefix);

    for token in split_preserving_spaces(&spans) {
        match token {
            Token::Newline => {
                if state.has_text {
                    push_prefixed_wrapped_line(
                        &mut lines,
                        &mut state.current,
                        &mut state.current_width,
                        continuation_prefix,
                    );
                    state.has_text = false;
                }
            }
            Token::Word {
                spans,
                preceded_by_space,
                space_style,
            } => {
                push_split_token_with_prefix(
                    &mut lines,
                    &mut state,
                    spans,
                    preceded_by_space,
                    space_style,
                    width,
                    continuation_prefix,
                );
            }
        }
    }

    if state.has_text {
        push_wrapped_line(&mut lines, &mut state.current, &mut state.current_width);
    }

    if lines.is_empty() {
        lines.push(Line::from(first_prefix.to_vec()));
    }

    lines
}

#[cfg(test)]
fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    wrap_spans_with_prefix(spans, &[], &[], width)
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

struct WrapState {
    current: Vec<Span<'static>>,
    current_width: usize,
    has_text: bool,
}

impl WrapState {
    fn new(prefix: &[Span<'static>]) -> Self {
        let current = prefix.to_vec();
        let current_width = spans_width(&current);

        Self {
            current,
            current_width,
            has_text: false,
        }
    }
}

fn push_split_token_with_prefix(
    lines: &mut Vec<Line<'static>>,
    state: &mut WrapState,
    spans: Vec<Span<'static>>,
    preceded_by_space: bool,
    space_style: Style,
    width: usize,
    continuation_prefix: &[Span<'static>],
) {
    let word_width = spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum::<usize>();
    let needs_space = preceded_by_space && state.has_text;
    if state.current_width + usize::from(needs_space) + word_width > width && state.has_text {
        push_prefixed_wrapped_line(
            lines,
            &mut state.current,
            &mut state.current_width,
            continuation_prefix,
        );
        state.has_text = false;
    }

    if state.current_width + word_width > width {
        push_split_word(lines, state, spans, width, continuation_prefix);
        return;
    }

    if preceded_by_space && state.has_text {
        state.current.push(Span::styled(" ", space_style));
        state.current_width += 1;
    }

    state.current_width += word_width;
    state.current.extend(spans);
    state.has_text = true;
}

fn push_split_word(
    lines: &mut Vec<Line<'static>>,
    state: &mut WrapState,
    spans: Vec<Span<'static>>,
    width: usize,
    continuation_prefix: &[Span<'static>],
) {
    let mut prefix_width = state.current_width;

    for span in spans {
        for ch in span.content.as_ref().chars() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if state.current_width + ch_width > width && state.current_width > prefix_width {
                push_prefixed_wrapped_line(
                    lines,
                    &mut state.current,
                    &mut state.current_width,
                    continuation_prefix,
                );
                prefix_width = state.current_width;
            }
            push_char(&mut state.current, ch, span.style);
            state.current_width += ch_width;
            state.has_text = true;
        }
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

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

fn push_prefixed_wrapped_line(
    lines: &mut Vec<Line<'static>>,
    current: &mut Vec<Span<'static>>,
    current_width: &mut usize,
    continuation_prefix: &[Span<'static>],
) {
    push_wrapped_line(lines, current, current_width);
    current.extend_from_slice(continuation_prefix);
    *current_width = spans_width(current);
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
    fn markdown_lines_render_headings_emphasis_and_inline_code() {
        let lines = markdown_lines("# Title\n\nA **bold** _em_ `code` ~~gone~~.", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("# Title"));
        assert!(text.contains("A bold em code gone."));
        assert!(
            lines
                .iter()
                .any(|line| line_has_modifier(line, Modifier::BOLD))
        );
        assert!(
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(
                    |span| span.content.as_ref() == "code" && span.style.fg == Some(Color::Magenta)
                )
        );
        assert!(
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.as_ref() == "em"
                    && span.style.add_modifier.contains(Modifier::ITALIC))
        );
        assert!(
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.as_ref() == "gone"
                    && span.style.add_modifier.contains(Modifier::CROSSED_OUT))
        );
    }

    #[test]
    fn markdown_lines_preserve_active_modifier_on_inline_code() {
        let lines = markdown_lines("**`code`**", 80);

        assert!(lines.iter().flat_map(|line| line.spans.iter()).any(|span| {
            span.content.as_ref() == "code"
                && span.style.fg == Some(Color::Magenta)
                && span.style.add_modifier.contains(Modifier::BOLD)
        }));
    }

    #[test]
    fn markdown_lines_parse_emphasis_markers_instead_of_echoing_them() {
        let lines = markdown_lines("A **bold** _em_ `code` ~~gone~~.", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert_eq!(text, "A bold em code gone.");
        assert!(!text.contains("**"));
        assert!(!text.contains("_em_"));
        assert!(!text.contains("~~"));
        assert!(
            lines
                .iter()
                .any(|line| line_has_modifier(line, Modifier::BOLD))
        );
        assert!(
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(
                    |span| span.content.as_ref() == "code" && span.style.fg == Some(Color::Magenta)
                )
        );
    }

    #[test]
    fn markdown_lines_render_lists_quotes_and_task_markers() {
        let lines = markdown_lines("> quoted\n\n- [x] done\n- [ ] open\n1. first", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("> quoted"));
        assert!(text.contains("- [x] done"));
        assert!(text.contains("- [ ] open"));
        assert!(text.contains("1. first"));
    }

    #[test]
    fn markdown_lines_preserve_quote_prefix_on_wrapped_lines() {
        let lines = markdown_lines("> alpha beta gamma delta epsilon", 14);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.len() > 1);
        assert!(text.iter().all(|line| line.starts_with("> ")));
    }

    #[test]
    fn markdown_lines_split_first_word_that_overflows_prefixed_line() {
        let lines = markdown_lines("> abcdefg", 8);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.len() > 1);
        assert!(text.iter().all(|line| line.starts_with("> ")));
        assert!(
            text.iter()
                .all(|line| UnicodeWidthStr::width(line.as_str()) <= 8)
        );
    }

    #[test]
    fn markdown_lines_preserve_list_prefix_on_wrapped_lines() {
        let lines = markdown_lines("- alpha beta gamma delta epsilon", 14);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.len() > 1);
        assert!(text[0].starts_with("- "));
        assert!(text.iter().skip(1).all(|line| line.starts_with("  ")));
    }

    #[test]
    fn markdown_lines_preserve_mixed_quote_list_prefix_order() {
        let quoted_list = markdown_lines("> - alpha beta gamma", 12);
        let quoted_list_text = quoted_list.iter().map(line_text).collect::<Vec<_>>();

        assert!(quoted_list_text.len() > 1);
        assert!(quoted_list_text[0].starts_with("> - "));
        assert!(
            quoted_list_text
                .iter()
                .skip(1)
                .all(|line| line.starts_with(">   "))
        );

        let list_quote = markdown_lines("- > alpha beta gamma", 12);
        let list_quote_text = list_quote.iter().map(line_text).collect::<Vec<_>>();

        assert!(list_quote_text.len() > 1);
        assert!(list_quote_text[0].starts_with("- > "));
        assert!(
            list_quote_text
                .iter()
                .skip(1)
                .all(|line| line.starts_with("  > "))
        );
    }

    #[test]
    fn markdown_lines_preserve_list_prefix_when_first_word_is_split() {
        let lines = markdown_lines("- supercalifragilistic", 8);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.len() > 1);
        assert!(text[0].starts_with("- "));
        assert!(text.iter().skip(1).all(|line| line.starts_with("  ")));
    }

    #[test]
    fn markdown_lines_preserve_task_list_continuation_indent() {
        let lines = markdown_lines("- [x] alpha beta gamma delta", 14);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.len() > 1);
        assert!(text[0].starts_with("- [x] "));
        assert!(text.iter().skip(1).all(|line| line.starts_with("      ")));
    }

    #[test]
    fn markdown_lines_preserve_nested_list_indentation() {
        let lines = markdown_lines("- outer\n  - inner alpha beta gamma delta", 18);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.iter().any(|line| line.starts_with("- outer")));
        assert!(text.iter().any(|line| line.starts_with("  - inner")));
        assert!(
            text.iter()
                .filter(|line| line.contains("gamma") || line.contains("delta"))
                .all(|line| line.starts_with("    "))
        );
    }

    #[test]
    fn markdown_lines_preserve_code_block_spacing_as_safe_fallback() {
        let lines = markdown_lines("```text\nlet  x = 1;\n    indented\n```", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("let  x = 1;"));
        assert!(text.contains("    indented"));
    }

    #[test]
    fn markdown_lines_split_long_code_block_lines_by_display_width() {
        let lines = markdown_lines("```\nabcdefghi\n```", 5);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.len() > 1);
        assert!(text.join("").contains("abcdefghi"));
        assert!(
            text.iter()
                .all(|line| UnicodeWidthStr::width(line.as_str()) <= 5)
        );
    }

    #[test]
    fn markdown_lines_prefix_code_block_inside_quote() {
        let lines = markdown_lines("> ```text\n> let  x = 1;\n> ```", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.iter().any(|line| line == "> let  x = 1;"));
        assert!(text.iter().all(|line| line.starts_with("> ")));
    }

    #[test]
    fn markdown_lines_prefix_rule_inside_list_item() {
        let lines = markdown_lines("- before\n\n  ---", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();

        assert!(text.iter().any(|line| line.starts_with("  ---")));
    }

    #[test]
    fn markdown_lines_keep_links_and_images_targets_visible() {
        let lines = markdown_lines(
            "[site](https://example.test) ![logo](https://img.test/a.png)",
            100,
        );
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("site <https://example.test>"));
        assert!(text.contains("![logo] <https://img.test/a.png>"));
    }

    #[test]
    fn markdown_lines_render_empty_alt_image_as_image_fallback() {
        let lines = markdown_lines("![](https://img.test/a.png)", 100);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("<image: https://img.test/a.png>"));
    }

    #[test]
    fn markdown_lines_keep_styled_image_alt_as_non_empty() {
        let lines = markdown_lines("![`logo` text](https://img.test/a.png)", 100);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("![logo text] <https://img.test/a.png>"));
        assert!(!text.contains("<image: https://img.test/a.png>"));
    }

    #[test]
    fn markdown_lines_render_fenced_code_with_language_label() {
        let lines = markdown_lines("```json\n{\"ok\": true}\n```", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("```json"));
        assert!(text.contains("{\"ok\": true}"));
        assert!(text.contains("```"));
    }

    #[test]
    fn markdown_lines_render_tables_as_terminal_text() {
        let lines = markdown_lines("| 名称 | value |\n| --- | ---: |\n| 宽字符 | 42 |", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("名称"));
        assert!(text.contains("value"));
        assert!(text.contains("宽字符"));
        assert!(text.contains("42"));
        assert!(text.lines().any(|line| line.contains("---")));
    }

    #[test]
    fn markdown_lines_keep_table_cell_link_and_image_targets_visible() {
        let lines = markdown_lines(
            "| link | image |\n| --- | --- |\n| [site](https://example.test) | ![logo](https://img.test/a.png) |",
            120,
        );
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("site <https://example.test>"));
        assert!(text.contains("![logo] <https://img.test/a.png>"));
    }

    #[test]
    fn markdown_lines_escape_pipe_characters_inside_table_cells() {
        let lines = markdown_lines("| value |\n| --- |\n| `a\\|b` |", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("a\\|b"));
    }

    #[test]
    fn markdown_lines_size_table_columns_after_escaping_pipes() {
        let lines = markdown_lines("| value |\n| --- |\n| `a\\|b\\|c` |", 80);
        let text = lines.iter().map(line_text).collect::<Vec<_>>();
        let separator = text.iter().find(|line| line.contains("---")).unwrap();
        let value = text.iter().find(|line| line.contains("a\\|b\\|c")).unwrap();

        assert!(
            UnicodeWidthStr::width(separator.as_str()) >= UnicodeWidthStr::width(value.as_str())
        );
    }

    #[test]
    fn markdown_lines_preserve_mermaid_latex_and_html_as_safe_text() {
        let lines = markdown_lines(
            "```mermaid\ngraph TD\nA-->B\n```\n\n$$x^2$$\n\n<div>safe</div>",
            80,
        );
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("```mermaid"));
        assert!(text.contains("graph TD"));
        assert!(text.contains("$$x^2$$"));
        assert!(text.contains("<div>safe</div>"));
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
