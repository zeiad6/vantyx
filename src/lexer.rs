//! Indentation-aware lexer.
//!
//! The lexer never panics on arbitrary UTF-8 input. It reports diagnostics and
//! keeps scanning so later stages can recover and report more than one problem.

use crate::diagnostic::Diagnostics;
use crate::span::Span;
use crate::token::{Token, TokenKind};

pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Diagnostics,
}

pub fn lex(source: &str) -> LexOutput {
    Lexer::new(source, 0, 0).run()
}

/// Lex one file in a multi-file compilation.
pub fn lex_file(source: &str, file: u32) -> LexOutput {
    Lexer::new(source, 0, file).run()
}

/// Lex a fragment (used for string interpolation) with spans offset by `base`.
pub fn lex_fragment(source: &str, base: usize) -> LexOutput {
    Lexer::new(source, base, 0).run()
}

/// Lex an interpolation fragment belonging to `file`.
pub fn lex_fragment_in_file(source: &str, base: usize, file: u32) -> LexOutput {
    Lexer::new(source, base, file).run()
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    base: usize,
    file: u32,
    tokens: Vec<Token>,
    diagnostics: Diagnostics,
    indents: Vec<usize>,
    at_line_start: bool,
    brackets: Vec<(char, Span)>,
}

enum LineStart {
    Eof,
    /// A blank or comment-only line was consumed; restart at the next line.
    Again,
    /// Indentation was processed; the caller may now lex tokens.
    Ready,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, base: usize, file: u32) -> Self {
        Self {
            src,
            pos: 0,
            base,
            file,
            tokens: Vec::new(),
            diagnostics: Diagnostics::new(),
            indents: vec![0],
            at_line_start: true,
            brackets: Vec::new(),
        }
    }

    fn run(mut self) -> LexOutput {
        loop {
            if self.at_line_start && self.brackets.is_empty() {
                match self.handle_line_start() {
                    LineStart::Eof => break,
                    LineStart::Again => continue,
                    LineStart::Ready => {}
                }
            }
            self.skip_trivia();
            let Some(ch) = self.peek() else { break };
            match ch {
                '\n' => {
                    let start = self.pos;
                    self.bump();
                    if self.brackets.is_empty() {
                        self.push(TokenKind::Newline, start, self.pos);
                        self.at_line_start = true;
                    }
                }
                '#' => self.skip_comment(),
                'a'..='z' | 'A'..='Z' | '_' => self.lex_ident(),
                '0'..='9' => self.lex_number(),
                '"' => self.lex_string(),
                '(' | '[' | '{' => {
                    let open = ch;
                    let start = self.pos;
                    self.bump();
                    self.brackets.push((open, self.span(start, self.pos)));
                    self.push(
                        match open {
                            '(' => TokenKind::LParen,
                            '[' => TokenKind::LBracket,
                            _ => TokenKind::LBrace,
                        },
                        start,
                        self.pos,
                    );
                }
                ')' | ']' | '}' => self.lex_close(ch),
                ',' => self.single(TokenKind::Comma),
                ':' => self.single(TokenKind::Colon),
                '?' => self.single(TokenKind::Question),
                '.' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('.') {
                        if self.eat('=') {
                            self.push(TokenKind::DotDotEq, start, self.pos);
                        } else {
                            self.push(TokenKind::DotDot, start, self.pos);
                        }
                    } else {
                        self.push(TokenKind::Dot, start, self.pos);
                    }
                }
                '+' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('=') {
                        self.push(TokenKind::PlusEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Plus, start, self.pos);
                    }
                }
                '-' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('>') {
                        self.push(TokenKind::Arrow, start, self.pos);
                    } else if self.eat('=') {
                        self.push(TokenKind::MinusEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Minus, start, self.pos);
                    }
                }
                '*' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('*') {
                        self.push(TokenKind::StarStar, start, self.pos);
                    } else if self.eat('=') {
                        self.push(TokenKind::StarEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Star, start, self.pos);
                    }
                }
                '/' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('/') {
                        if self.eat('=') {
                            self.push(TokenKind::SlashSlashEq, start, self.pos);
                        } else {
                            self.push(TokenKind::SlashSlash, start, self.pos);
                        }
                    } else if self.eat('=') {
                        self.push(TokenKind::SlashEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Slash, start, self.pos);
                    }
                }
                '%' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('=') {
                        self.push(TokenKind::PercentEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Percent, start, self.pos);
                    }
                }
                '=' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('=') {
                        self.push(TokenKind::EqEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Eq, start, self.pos);
                    }
                }
                '<' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('=') {
                        self.push(TokenKind::LtEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Lt, start, self.pos);
                    }
                }
                '>' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('=') {
                        self.push(TokenKind::GtEq, start, self.pos);
                    } else {
                        self.push(TokenKind::Gt, start, self.pos);
                    }
                }
                '!' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('=') {
                        self.push(TokenKind::NotEq, start, self.pos);
                    } else {
                        let span = self.span(start, self.pos);
                        self.diagnostics.push(
                            crate::diagnostic::Diagnostic::error(
                                "E0006",
                                "`!` is not an operator in Vantyx",
                                span,
                            )
                            .with_help("use `not` for logical negation"),
                        );
                    }
                }
                '&' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('&') {
                        let span = self.span(start, self.pos);
                        self.diagnostics.push(
                            crate::diagnostic::Diagnostic::error(
                                "E0006",
                                "`&&` is not an operator in Vantyx",
                                span,
                            )
                            .with_help("use `and`"),
                        );
                    } else {
                        let span = self.span(start, self.pos);
                        self.diagnostics.push(
                            crate::diagnostic::Diagnostic::error(
                                "E0001",
                                "unexpected character `&`",
                                span,
                            )
                            .with_help("bitwise operators are not implemented yet"),
                        );
                    }
                }
                '|' => {
                    let start = self.pos;
                    self.bump();
                    if self.eat('|') {
                        let span = self.span(start, self.pos);
                        self.diagnostics.push(
                            crate::diagnostic::Diagnostic::error(
                                "E0006",
                                "`||` is not an operator in Vantyx",
                                span,
                            )
                            .with_help("use `or`"),
                        );
                    } else {
                        self.push(TokenKind::Pipe, start, self.pos);
                    }
                }
                '\'' => {
                    let start = self.pos;
                    self.bump();
                    let span = self.span(start, self.pos);
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "E0001",
                            "unexpected character `'`",
                            span,
                        )
                        .with_help("strings use double quotes, for example \"text\""),
                    );
                }
                other => {
                    let start = self.pos;
                    self.bump();
                    let span = self.span(start, self.pos);
                    self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                        "E0001",
                        format!("unexpected character `{other}`"),
                        span,
                    ));
                }
            }
        }
        self.finish();
        LexOutput {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    // ---- line and trivia handling ----

    /// Handles leading indentation. Returns false at EOF.
    fn handle_line_start(&mut self) -> LineStart {
        let start = self.pos;
        let mut indent = 0usize;
        loop {
            match self.peek() {
                Some(' ') => {
                    self.bump();
                    indent += 1;
                }
                Some('\t') => {
                    let span = self.span(self.pos, self.pos + 1);
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "E0002",
                            "tab character in indentation",
                            span,
                        )
                        .with_help("use spaces for indentation (4 spaces per level)"),
                    );
                    self.bump();
                    indent += 4;
                }
                _ => break,
            }
        }

        match self.peek() {
            None => return LineStart::Eof,
            Some('\n') => {
                self.bump();
                return LineStart::Again;
            }
            Some('\r') => {
                self.bump();
                if self.peek() == Some('\n') {
                    self.bump();
                }
                return LineStart::Again;
            }
            Some('#') => {
                self.skip_comment();
                if self.peek() == Some('\n') {
                    self.bump();
                } else if self.peek() == Some('\r') {
                    self.bump();
                    if self.peek() == Some('\n') {
                        self.bump();
                    }
                }
                return LineStart::Again;
            }
            _ => {}
        }

        let current = *self.indents.last().unwrap_or(&0);
        if indent > current {
            self.indents.push(indent);
            let span = self.span(self.pos, self.pos);
            self.tokens.push(Token::new(TokenKind::Indent, span));
        } else if indent < current {
            while self.indents.len() > 1 && *self.indents.last().unwrap() > indent {
                self.indents.pop();
                let span = self.span(self.pos, self.pos);
                self.tokens.push(Token::new(TokenKind::Dedent, span));
            }
            if *self.indents.last().unwrap() != indent {
                let span = self.span(self.pos, self.pos);
                self.diagnostics.push(
                    crate::diagnostic::Diagnostic::error("E0001", "inconsistent indentation", span)
                        .with_help("align this line with an enclosing block"),
                );
                // Recover by treating this as a new level.
                self.indents.push(indent);
            }
        }

        let _ = start;
        self.at_line_start = false;
        LineStart::Ready
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\r') => {
                    self.bump();
                }
                Some('\n') if !self.brackets.is_empty() => {
                    self.bump();
                }
                _ => break,
            }
        }
    }

    fn skip_comment(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.bump();
        }
    }

    // ---- token lexers ----

    fn lex_ident(&mut self) {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.bump();
            } else {
                break;
            }
        }
        let text = &self.src[start..self.pos];
        let kind = match text {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "var" => TokenKind::Var,
            "if" => TokenKind::If,
            "elif" => TokenKind::Elif,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "import" => TokenKind::Import,
            "from" => TokenKind::From,
            "match" => TokenKind::Match,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            _ => TokenKind::Ident(text.to_string()),
        };
        self.push(kind, start, self.pos);
    }

    fn lex_number(&mut self) {
        let start = self.pos;
        let mut is_float = false;

        if self.peek() == Some('0')
            && let Some(prefix) = self.peek_n(1)
        {
            let radix = match prefix {
                'x' | 'X' => Some(16),
                'b' | 'B' => Some(2),
                'o' | 'O' => Some(8),
                _ => None,
            };
            if let Some(radix) = radix {
                self.bump();
                self.bump();
                let digits_start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        self.bump();
                    } else {
                        break;
                    }
                }
                let raw = &self.src[digits_start..self.pos];
                let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
                if cleaned.is_empty() {
                    let span = self.span(start, self.pos);
                    self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                        "E0004",
                        "missing digits after base prefix",
                        span,
                    ));
                    self.tokens.push(Token::new(TokenKind::Int(0), span));
                } else {
                    match i64::from_str_radix(&cleaned, radix) {
                        Ok(value) => self.push(TokenKind::Int(value), start, self.pos),
                        Err(_) => {
                            let span = self.span(start, self.pos);
                            self.diagnostics.push(
                                crate::diagnostic::Diagnostic::error(
                                    "E0004",
                                    format!("invalid base-{radix} integer literal"),
                                    span,
                                )
                                .with_help("the value must fit in a signed 64-bit integer"),
                            );
                            self.tokens.push(Token::new(TokenKind::Int(0), span));
                        }
                    }
                }
                return;
            }
        }

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                self.bump();
            } else {
                break;
            }
        }

        if self.peek() == Some('.')
            && let Some(next) = self.peek_n(1)
            && next.is_ascii_digit()
        {
            is_float = true;
            self.bump();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() || ch == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
        }

        if matches!(self.peek(), Some('e') | Some('E')) && self.looks_like_exponent() {
            is_float = true;
            self.bump(); // e/E
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.bump();
            }
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() || ch == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
        }

        // A number directly followed by identifier characters is malformed.
        if let Some(ch) = self.peek()
            && (ch.is_ascii_alphabetic() || ch == '_')
        {
            while let Some(c) = self.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
            let span = self.span(start, self.pos);
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error("E0004", "invalid numeric literal", span)
                    .with_help("remove the trailing characters or add an operator"),
            );
            self.tokens.push(Token::new(TokenKind::Int(0), span));
            return;
        }

        let raw = &self.src[start..self.pos];
        let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
        if is_float {
            match cleaned.parse::<f64>() {
                Ok(value) => self.push(TokenKind::Float(value), start, self.pos),
                Err(_) => {
                    let span = self.span(start, self.pos);
                    self.diagnostics.push(crate::diagnostic::Diagnostic::error(
                        "E0004",
                        "invalid float literal",
                        span,
                    ));
                    self.tokens.push(Token::new(TokenKind::Float(0.0), span));
                }
            }
        } else {
            match cleaned.parse::<i64>() {
                Ok(value) => self.push(TokenKind::Int(value), start, self.pos),
                Err(_) => {
                    let span = self.span(start, self.pos);
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "E0004",
                            "integer literal is too large",
                            span,
                        )
                        .with_help("the maximum Int is 9223372036854775807"),
                    );
                    self.tokens.push(Token::new(TokenKind::Int(0), span));
                }
            }
        }
    }

    fn looks_like_exponent(&self) -> bool {
        let mut chars = self.src[self.pos..].chars();
        chars.next(); // e/E
        match chars.next() {
            Some('+') | Some('-') => matches!(chars.next(), Some(c) if c.is_ascii_digit()),
            Some(c) => c.is_ascii_digit(),
            None => false,
        }
    }

    fn lex_string(&mut self) {
        let start = self.pos;
        self.bump(); // opening quote
        let content_start = self.pos;
        loop {
            match self.peek() {
                None => {
                    let span = self.span(start, self.pos);
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "E0003",
                            "unterminated string literal",
                            span,
                        )
                        .with_help("add a closing `\"`"),
                    );
                    let raw = self.src[content_start..self.pos].to_string();
                    self.tokens.push(Token::new(TokenKind::Str(raw), span));
                    return;
                }
                Some('"') => {
                    let raw = self.src[content_start..self.pos].to_string();
                    self.bump();
                    self.push(TokenKind::Str(raw), start, self.pos);
                    return;
                }
                Some('\n') => {
                    let span = self.span(start, self.pos);
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "E0003",
                            "unterminated string literal",
                            span,
                        )
                        .with_help("strings cannot span lines; add a closing `\"`"),
                    );
                    let raw = self.src[content_start..self.pos].to_string();
                    self.tokens.push(Token::new(TokenKind::Str(raw), span));
                    return;
                }
                Some('\\') => {
                    self.bump();
                    if self.peek().is_some() {
                        self.bump();
                    }
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    fn lex_close(&mut self, ch: char) {
        let start = self.pos;
        self.bump();
        let span = self.span(start, self.pos);
        let expected = match ch {
            ')' => '(',
            ']' => '[',
            _ => '{',
        };
        match self.brackets.pop() {
            Some((open, _)) if open == expected => {}
            Some((open, open_span)) => {
                self.diagnostics.push(
                    crate::diagnostic::Diagnostic::error(
                        "E0005",
                        format!("mismatched closing bracket `{ch}`"),
                        span,
                    )
                    .with_help(format!(
                        "the bracket opened as `{open}` at byte {}",
                        open_span.start
                    )),
                );
            }
            None => {
                self.diagnostics.push(
                    crate::diagnostic::Diagnostic::error(
                        "E0005",
                        format!("unexpected closing bracket `{ch}`"),
                        span,
                    )
                    .with_help("remove the bracket or add an opening bracket"),
                );
            }
        }
        let kind = match ch {
            ')' => TokenKind::RParen,
            ']' => TokenKind::RBracket,
            _ => TokenKind::RBrace,
        };
        self.push(kind, start, self.pos);
    }

    fn finish(&mut self) {
        for (open, span) in self.brackets.drain(..) {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error("E0005", format!("unclosed `{open}`"), span)
                    .with_help("add the matching closing bracket"),
            );
        }

        let needs_newline = match self.tokens.last() {
            None => false,
            Some(token) => !matches!(
                token.kind,
                TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
            ),
        };
        if needs_newline {
            let span = self.span(self.pos, self.pos);
            self.tokens.push(Token::new(TokenKind::Newline, span));
        }
        while self.indents.len() > 1 {
            self.indents.pop();
            let span = self.span(self.pos, self.pos);
            self.tokens.push(Token::new(TokenKind::Dedent, span));
        }
        let span = self.span(self.pos, self.pos);
        self.tokens.push(Token::new(TokenKind::Eof, span));
    }

    // ---- helpers ----

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::in_file(self.file, self.base + start, self.base + end)
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        let span = self.span(start, end);
        self.tokens.push(Token::new(kind, span));
    }

    fn single(&mut self, kind: TokenKind) {
        let start = self.pos;
        self.bump();
        self.push(kind, start, self.pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        let out = lex(source);
        assert!(
            !out.diagnostics.has_errors(),
            "unexpected diagnostics: {:?}",
            out.diagnostics
        );
        out.tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn lexes_a_function_with_indentation() {
        let got = kinds("fn main():\n    let x = 1\n");
        assert_eq!(
            got,
            vec![
                TokenKind::Fn,
                TokenKind::Ident("main".into()),
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::Colon,
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Let,
                TokenKind::Ident("x".into()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn handles_blank_lines_and_comments() {
        let got = kinds("# header\n\nfn f():\n    # body\n    return 1\n\n");
        assert_eq!(
            got,
            vec![
                TokenKind::Fn,
                TokenKind::Ident("f".into()),
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::Colon,
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Return,
                TokenKind::Int(1),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn ignores_newlines_inside_brackets() {
        let got = kinds("let xs = [\n  1,\n  2,\n]\n");
        assert!(got.contains(&TokenKind::LBracket));
        assert_eq!(got.iter().filter(|k| **k == TokenKind::Newline).count(), 1);
    }

    #[test]
    fn lexes_operators_greedily() {
        let got = kinds("a //= b ** c -> d >= e != f\n");
        assert_eq!(
            got,
            vec![
                TokenKind::Ident("a".into()),
                TokenKind::SlashSlashEq,
                TokenKind::Ident("b".into()),
                TokenKind::StarStar,
                TokenKind::Ident("c".into()),
                TokenKind::Arrow,
                TokenKind::Ident("d".into()),
                TokenKind::GtEq,
                TokenKind::Ident("e".into()),
                TokenKind::NotEq,
                TokenKind::Ident("f".into()),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_numbers() {
        let got = kinds("1 2.5 1e9 0xFF 0b101 1_000\n");
        assert_eq!(
            got,
            vec![
                TokenKind::Int(1),
                TokenKind::Float(2.5),
                TokenKind::Float(1e9),
                TokenKind::Int(255),
                TokenKind::Int(5),
                TokenKind::Int(1000),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn rejects_tab_indentation() {
        let out = lex("fn f():\n\treturn 1\n");
        assert!(out.diagnostics.has_errors());
        assert!(out.diagnostics.items.iter().any(|d| d.code == "E0002"));
    }

    #[test]
    fn reports_unterminated_string_without_panicking() {
        let out = lex("let x = \"abc\n");
        assert!(out.diagnostics.items.iter().any(|d| d.code == "E0003"));
    }

    #[test]
    fn reports_inconsistent_dedent() {
        let out = lex("if x:\n    if y:\n        z = 1\n  w = 2\n");
        assert!(out.diagnostics.items.iter().any(|d| d.code == "E0001"));
    }

    #[test]
    fn does_not_emit_newline_for_empty_source() {
        let out = lex("");
        assert_eq!(out.tokens.len(), 1);
        assert_eq!(out.tokens[0].kind, TokenKind::Eof);
    }
}
