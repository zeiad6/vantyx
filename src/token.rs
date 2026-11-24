//! Token definitions.

use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // Literals and names
    Int(i64),
    Float(f64),
    /// Raw string content between the quotes, with escapes still encoded.
    Str(String),
    Ident(String),

    // Keywords
    Fn,
    Let,
    Var,
    If,
    Elif,
    Else,
    While,
    For,
    In,
    Return,
    Break,
    Continue,
    Struct,
    Enum,
    Match,
    True,
    False,
    And,
    Or,
    Not,

    // Punctuation
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    Arrow,
    Plus,
    Minus,
    Star,
    Slash,
    SlashSlash,
    Percent,
    StarStar,
    Eq,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    SlashSlashEq,
    PercentEq,

    // Structure
    Newline,
    Indent,
    Dedent,
    Eof,
}

impl TokenKind {
    /// Human-readable description used in parser diagnostics.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Int(_) => "integer literal".to_string(),
            TokenKind::Float(_) => "float literal".to_string(),
            TokenKind::Str(_) => "string literal".to_string(),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::Fn => "`fn`".to_string(),
            TokenKind::Let => "`let`".to_string(),
            TokenKind::Var => "`var`".to_string(),
            TokenKind::If => "`if`".to_string(),
            TokenKind::Elif => "`elif`".to_string(),
            TokenKind::Else => "`else`".to_string(),
            TokenKind::While => "`while`".to_string(),
            TokenKind::For => "`for`".to_string(),
            TokenKind::In => "`in`".to_string(),
            TokenKind::Return => "`return`".to_string(),
            TokenKind::Break => "`break`".to_string(),
            TokenKind::Continue => "`continue`".to_string(),
            TokenKind::Struct => "`struct`".to_string(),
            TokenKind::Enum => "`enum`".to_string(),
            TokenKind::Match => "`match`".to_string(),
            TokenKind::True => "`true`".to_string(),
            TokenKind::False => "`false`".to_string(),
            TokenKind::And => "`and`".to_string(),
            TokenKind::Or => "`or`".to_string(),
            TokenKind::Not => "`not`".to_string(),
            TokenKind::LParen => "`(`".to_string(),
            TokenKind::RParen => "`)`".to_string(),
            TokenKind::LBracket => "`[`".to_string(),
            TokenKind::RBracket => "`]`".to_string(),
            TokenKind::LBrace => "`{`".to_string(),
            TokenKind::RBrace => "`}`".to_string(),
            TokenKind::Comma => "`,`".to_string(),
            TokenKind::Colon => "`:`".to_string(),
            TokenKind::Dot => "`.`".to_string(),
            TokenKind::Arrow => "`->`".to_string(),
            TokenKind::Plus => "`+`".to_string(),
            TokenKind::Minus => "`-`".to_string(),
            TokenKind::Star => "`*`".to_string(),
            TokenKind::Slash => "`/`".to_string(),
            TokenKind::SlashSlash => "`//`".to_string(),
            TokenKind::Percent => "`%`".to_string(),
            TokenKind::StarStar => "`**`".to_string(),
            TokenKind::Eq => "`=`".to_string(),
            TokenKind::EqEq => "`==`".to_string(),
            TokenKind::NotEq => "`!=`".to_string(),
            TokenKind::Lt => "`<`".to_string(),
            TokenKind::LtEq => "`<=`".to_string(),
            TokenKind::Gt => "`>`".to_string(),
            TokenKind::GtEq => "`>=`".to_string(),
            TokenKind::PlusEq => "`+=`".to_string(),
            TokenKind::MinusEq => "`-=`".to_string(),
            TokenKind::StarEq => "`*=`".to_string(),
            TokenKind::SlashEq => "`/=`".to_string(),
            TokenKind::SlashSlashEq => "`//=`".to_string(),
            TokenKind::PercentEq => "`%=`".to_string(),
            TokenKind::Newline => "end of line".to_string(),
            TokenKind::Indent => "indentation".to_string(),
            TokenKind::Dedent => "dedentation".to_string(),
            TokenKind::Eof => "end of file".to_string(),
        }
    }

    pub fn same_variant(&self, other: &TokenKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
