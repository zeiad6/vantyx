//! Diagnostics and source rendering.

use crate::span::{SourceMap, SourceRegistry, Span, expand_tabs};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub span: Span,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.notes.push(format!("help: {}", help.into()));
        self
    }

    pub fn render(&self, source_name: &str, source: &str) -> String {
        let map = SourceMap::new(source_name, source);
        let (line, col) = map.line_col(self.span.start);
        let line_text = map.line_text(line);
        let expanded = expand_tabs(line_text);
        let line_start = map
            .line_col(self.span.start)
            .0
            .checked_sub(1)
            .map(|_| {
                // Recover the byte offset of the line start from the span and column.
                let prefix_chars = col.saturating_sub(1);
                let mut bytes = 0usize;
                for ch in line_text.chars().take(prefix_chars) {
                    bytes += ch.len_utf8();
                }
                bytes
            })
            .unwrap_or(0);

        // Caret position in the expanded line.
        let prefix = &line_text[..line_start.min(line_text.len())];
        let caret_col = expand_tabs(prefix).chars().count();
        let span_len = (self.span.end.saturating_sub(self.span.start) as usize)
            .max(1)
            .min(expanded.chars().count().saturating_sub(caret_col).max(1));

        let number = line.to_string();
        let gutter = " ".repeat(number.len());
        let mut out = String::new();
        out.push_str(&format!(
            "{}[{}]: {}\n",
            self.severity, self.code, self.message
        ));
        out.push_str(&format!("{}--> {}:{}:{}\n", gutter, source_name, line, col));
        out.push_str(&format!("{} |\n", gutter));
        out.push_str(&format!("{} | {}\n", number, expanded));
        out.push_str(&format!(
            "{} | {}{}\n",
            gutter,
            " ".repeat(caret_col),
            "^".repeat(span_len)
        ));
        for note in &self.notes {
            out.push_str(&format!("{} = {}\n", gutter, note));
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub fn error(&mut self, code: impl Into<String>, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::error(code, message, span));
    }

    pub fn warning(&mut self, code: impl Into<String>, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::warning(code, message, span));
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn extend(&mut self, other: Diagnostics) {
        self.items.extend(other.items);
    }

    pub fn render(&self, source_name: &str, source: &str) -> String {
        let mut out = String::new();
        for diagnostic in &self.items {
            out.push_str(&diagnostic.render(source_name, source));
            out.push('\n');
        }
        out
    }

    /// Renders every diagnostic against the source file named by its span.
    pub fn render_all(&self, sources: &SourceRegistry) -> String {
        let mut out = String::new();
        for diagnostic in &self.items {
            let file = sources
                .get(diagnostic.span.file)
                .or_else(|| sources.iter().next());
            match file {
                Some(file) => out.push_str(&diagnostic.render(&file.name, &file.source)),
                None => out.push_str(&format!(
                    "{}[{}]: {}\n",
                    diagnostic.severity, diagnostic.code, diagnostic.message
                )),
            }
            out.push('\n');
        }
        out
    }

    /// Keeps only errors, dropping warnings.
    pub fn errors_only(&self) -> Diagnostics {
        Diagnostics {
            items: self
                .items
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .cloned()
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_caret_under_the_span() {
        let src = "fn main():\n    let x = 1\n";
        let span = Span::new(23, 24);
        let d = Diagnostic::error("E0101", "unexpected token", span).with_help("remove the token");
        let rendered = d.render("test.vtx", src);
        assert!(rendered.contains("error[E0101]: unexpected token"));
        assert!(rendered.contains("test.vtx:2:13"));
        assert!(rendered.contains("^"));
        assert!(rendered.contains("help: remove the token"));
    }

    #[test]
    fn tabs_do_not_break_alignment() {
        let src = "\tlet x = 1\n";
        let d = Diagnostic::error("E0002", "tab", Span::new(1, 4));
        let rendered = d.render("t.vtx", src);
        assert!(rendered.contains("    let x"));
    }

    #[test]
    fn empty_source_is_safe() {
        let d = Diagnostic::error("E0001", "eof", Span::point(0));
        let rendered = d.render("empty.vtx", "");
        assert!(rendered.contains("empty.vtx:1:1"));
    }
}
