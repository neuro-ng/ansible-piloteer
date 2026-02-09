use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

#[derive(Debug)]
pub struct SyntaxHighlighter {
    ps: SyntaxSet,
    ts: ThemeSet,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
        }
    }

    pub fn highlight<'a>(&self, code: &'a str, extension: &str) -> Text<'a> {
        let syntax = self
            .ps
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.ps.find_syntax_plain_text());

        let mut h = HighlightLines::new(syntax, &self.ts.themes["base16-ocean.dark"]);
        let mut lines = Vec::new();

        for line in LinesWithEndings::from(code) {
            let ranges: Vec<(syntect::highlighting::Style, &str)> =
                h.highlight_line(line, &self.ps).unwrap_or_default();
            let spans: Vec<Span> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let fg = style.foreground;
                    let color = Color::Rgb(fg.r, fg.g, fg.b);
                    Span::styled(text.to_string(), Style::default().fg(color))
                })
                .collect();
            lines.push(Line::from(spans));
        }

        Text::from(lines)
    }
}

// Global instance or lazy_static?
// For now, initializing it every frame is expensive.
// We should put it in App state or minimal lazy_static.
// Putting in App is better.
