//! Recursive-descent parser with Pratt expressions.

use crate::ast::*;
use crate::diagnostic::{Diagnostic, Diagnostics};
use crate::lexer::lex_fragment;
use crate::span::Span;
use crate::token::{Token, TokenKind};

const MAX_EXPR_DEPTH: usize = 256;
/// Maximum height of an expression tree. Pratt parsing builds left-deep
/// chains iteratively, so the nesting counter alone cannot bound the AST.
const MAX_AST_DEPTH: usize = 256;
/// Maximum nested indented blocks. Each level recurses through the parser,
/// checker, and interpreter.
const MAX_BLOCK_DEPTH: usize = 256;
/// Maximum nested type constructors such as `List[List[...]]`.
const MAX_TYPE_DEPTH: usize = 64;
/// Maximum nested string interpolations such as `"{ "{...}" }"`.
const MAX_INTERP_DEPTH: usize = 32;
const UNARY_BP: u8 = 60;

pub fn parse(source: &str, tokens: Vec<Token>) -> (Program, Diagnostics) {
    let tokens = if tokens.is_empty() {
        vec![Token::new(TokenKind::Eof, Span::point(0))]
    } else {
        tokens
    };
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        diagnostics: Diagnostics::new(),
        next_id: 0,
        depth: 0,
        block_depth: 0,
        type_depth: 0,
        interp_depth: 0,
        depth_reported: false,
    };
    let program = parser.parse_program();
    (program, parser.diagnostics)
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Diagnostics,
    next_id: u32,
    depth: usize,
    block_depth: usize,
    type_depth: usize,
    interp_depth: usize,
    depth_reported: bool,
}

#[derive(Clone, Copy)]
enum AssignOp {
    Plain,
    Compound(BinOp),
}

impl<'a> Parser<'a> {
    // ---- token helpers ----

    fn peek(&self) -> &Token {
        let idx = self.pos.min(self.tokens.len() - 1);
        &self.tokens[idx]
    }

    fn peek_kind_n(&self, n: usize) -> &TokenKind {
        let idx = (self.pos + n).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek().kind.same_variant(kind)
    }

    fn advance(&mut self) -> Token {
        let idx = self.pos.min(self.tokens.len() - 1);
        let token = self.tokens[idx].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, context: &str) -> Result<Token, ()> {
        if self.check(&kind) {
            Ok(self.advance())
        } else {
            let span = self.peek().span;
            let context = if context.is_empty() {
                String::new()
            } else if context.starts_with(' ') {
                context.to_string()
            } else {
                format!(" {context}")
            };
            let message = format!(
                "expected {}{context}, found {}",
                kind.describe(),
                self.peek().kind.describe()
            );
            self.diagnostics.push(Diagnostic::error("E0102", message, span));
            Err(())
        }
    }

    fn expect_ident(&mut self, context: &str) -> Result<(String, Span), ()> {
        if let TokenKind::Ident(name) = self.peek().kind.clone() {
            let token = self.advance();
            Ok((name, token.span))
        } else {
            let span = self.peek().span;
            let message = format!(
                "expected an identifier for {context}, found {}",
                self.peek().kind.describe()
            );
            self.diagnostics.push(Diagnostic::error("E0102", message, span));
            Err(())
        }
    }

    fn error(&mut self, code: &str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }

    fn new_expr(&mut self, kind: ExprKind, span: Span) -> Expr {
        let id = self.next_id;
        self.next_id += 1;
        let depth = expr_depth(&kind);
        if depth > MAX_AST_DEPTH {
            if !self.depth_reported {
                self.depth_reported = true;
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0108",
                        "expression is too complex",
                        span,
                    )
                    .with_help(format!(
                        "split it into smaller expressions (maximum tree depth is {MAX_AST_DEPTH})"
                    )),
                );
            }
            // Keep the tree bounded so later passes cannot overflow the stack.
            // The error above guarantees that compilation stops here.
            return Expr {
                id,
                kind: ExprKind::Int(0),
                span,
                depth: 1,
            };
        }
        Expr {
            id,
            kind,
            span,
            depth,
        }
    }

    fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn synchronize(&mut self) {
        while !matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof
        ) {
            self.advance();
        }
        if self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn skip_indented_block(&mut self) {
        if !self.check(&TokenKind::Indent) {
            return;
        }
        self.advance();
        let mut depth = 1usize;
        while depth > 0 && !self.check(&TokenKind::Eof) {
            match self.peek().kind {
                TokenKind::Indent => {
                    depth += 1;
                    self.advance();
                }
                TokenKind::Dedent => {
                    depth -= 1;
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn expect_stmt_end(&mut self) -> Result<(), ()> {
        if self.check(&TokenKind::Newline) {
            self.advance();
            Ok(())
        } else if self.check(&TokenKind::Dedent) || self.check(&TokenKind::Eof) {
            Ok(())
        } else if self.pos > 0
            && matches!(self.tokens[self.pos - 1].kind, TokenKind::Dedent)
        {
            // A block-shaped expression such as `match` consumed the newline
            // and dedent that closed its arms; the statement is already
            // terminated at that boundary.
            Ok(())
        } else {
            let span = self.peek().span;
            let message = format!(
                "expected end of line, found {}",
                self.peek().kind.describe()
            );
            self.diagnostics.push(
                Diagnostic::error("E0101", message, span)
                    .with_help("statements must be separated by a newline"),
            );
            Err(())
        }
    }

    // ---- program ----

    fn parse_program(&mut self) -> Program {
        let start_span = self.peek().span;
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind.clone() {
                TokenKind::Eof => break,
                TokenKind::Dedent => {
                    // Stray dedent after a recovered error.
                    self.advance();
                }
                TokenKind::Indent => {
                    let span = self.advance().span;
                    self.error("E0104", "unexpected indentation", span);
                    self.skip_indented_block();
                }
                TokenKind::Fn => match self.parse_fn() {
                    Ok(function) => items.push(Item::Fn(function)),
                    Err(()) => self.synchronize(),
                },
                TokenKind::Struct => match self.parse_struct() {
                    Ok(decl) => items.push(Item::Struct(decl)),
                    Err(()) => self.synchronize(),
                },
                TokenKind::Enum => match self.parse_enum() {
                    Ok(decl) => items.push(Item::Enum(decl)),
                    Err(()) => self.synchronize(),
                },
                _ => match self.parse_stmt() {
                    Ok(stmt) => items.push(Item::Stmt(stmt)),
                    Err(()) => self.synchronize(),
                },
            }
        }
        let end_span = self.peek().span;
        Program {
            items,
            span: start_span.merge(end_span),
        }
    }

    fn parse_fn(&mut self) -> Result<FnDecl, ()> {
        let start = self.expect(TokenKind::Fn, " to start a function")?.span;
        let (name, name_span) = self.expect_ident("a function name")?;
        self.expect(TokenKind::LParen, " after the function name")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_start = self.peek().span;
                let (param_name, param_name_span) = self.expect_ident("a parameter")?;
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let span = param_start.merge(
                    ty.as_ref()
                        .map(|t| t.span)
                        .unwrap_or(param_name_span),
                );
                params.push(Param {
                    name: param_name,
                    name_span: param_name_span,
                    ty,
                    span,
                });
                if self.eat(&TokenKind::Comma) {
                    if self.check(&TokenKind::RParen) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen, " to close the parameter list")?;
        let ret = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_suite("after the function signature")?;
        let span = start.merge(body.span);
        Ok(FnDecl {
            name,
            name_span,
            params,
            ret,
            body,
            span,
        })
    }

    fn parse_struct(&mut self) -> Result<StructDecl, ()> {
        let start = self.expect(TokenKind::Struct, " to start a struct")?.span;
        let (name, name_span) = self.expect_ident("a struct name")?;
        self.expect(TokenKind::Colon, " after the struct name")?;
        self.expect(TokenKind::Newline, " after the struct header")?;
        self.expect(TokenKind::Indent, " to start the field list")?;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) {
                self.advance();
                break;
            }
            if self.check(&TokenKind::Eof) {
                let span = self.peek().span;
                self.error("E0101", "unclosed struct declaration", span);
                break;
            }
            let field_start = self.peek().span;
            match (|| -> Result<FieldDecl, ()> {
                let (field_name, field_name_span) = self.expect_ident("a field name")?;
                self.expect(TokenKind::Colon, " after the field name")?;
                let ty = self.parse_type()?;
                let span = field_start.merge(ty.span);
                self.expect_stmt_end()?;
                Ok(FieldDecl {
                    name: field_name,
                    name_span: field_name_span,
                    ty,
                    span,
                })
            })() {
                Ok(field) => fields.push(field),
                Err(()) => self.synchronize(),
            }
        }
        let end = self.peek().span;
        Ok(StructDecl {
            name,
            name_span,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, ()> {
        let start = self.expect(TokenKind::Enum, " to start an enum")?.span;
        let (name, name_span) = self.expect_ident("an enum name")?;
        self.expect(TokenKind::Colon, " after the enum name")?;
        self.expect(TokenKind::Newline, " after the enum header")?;
        self.expect(TokenKind::Indent, " to start the variant list")?;
        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) {
                self.advance();
                break;
            }
            if self.check(&TokenKind::Eof) {
                let span = self.peek().span;
                self.error("E0101", "unclosed enum declaration", span);
                break;
            }
            let variant_start = self.peek().span;
            match (|| -> Result<VariantDecl, ()> {
                let (variant_name, variant_name_span) = self.expect_ident("a variant name")?;
                let mut fields = Vec::new();
                let mut end = variant_name_span;
                if self.eat(&TokenKind::LParen) {
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            let field_start = self.peek().span;
                            let (field_name, field_name_span) =
                                self.expect_ident("a variant field")?;
                            self.expect(TokenKind::Colon, " after the variant field name")?;
                            let ty = self.parse_type()?;
                            let field_span = field_start.merge(ty.span);
                            fields.push(FieldDecl {
                                name: field_name,
                                name_span: field_name_span,
                                ty,
                                span: field_span,
                            });
                            if self.eat(&TokenKind::Comma) {
                                if self.check(&TokenKind::RParen) {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    let close = self.expect(TokenKind::RParen, " to close the variant fields")?;
                    end = close.span;
                }
                self.expect_stmt_end()?;
                Ok(VariantDecl {
                    name: variant_name,
                    name_span: variant_name_span,
                    fields,
                    span: variant_start.merge(end),
                })
            })() {
                Ok(variant) => variants.push(variant),
                Err(()) => self.synchronize(),
            }
        }
        let end = self.peek().span;
        Ok(EnumDecl {
            name,
            name_span,
            variants,
            span: start.merge(end),
        })
    }

    // ---- statements ----

    fn parse_stmt(&mut self) -> Result<Stmt, ()> {
        match self.peek().kind.clone() {
            TokenKind::Let => self.parse_let(false),
            TokenKind::Var => self.parse_let(true),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Match => self.parse_match_stmt(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => {
                let span = self.advance().span;
                self.expect_stmt_end()?;
                Ok(Stmt {
                    kind: StmtKind::Break,
                    span,
                })
            }
            TokenKind::Continue => {
                let span = self.advance().span;
                self.expect_stmt_end()?;
                Ok(Stmt {
                    kind: StmtKind::Continue,
                    span,
                })
            }
            TokenKind::Fn | TokenKind::Struct | TokenKind::Enum => {
                let span = self.peek().span;
                self.error(
                    "E0106",
                    "functions, structs, and enums may only be declared at the top level",
                    span,
                );
                Err(())
            }
            TokenKind::Indent => {
                let span = self.advance().span;
                self.error("E0104", "unexpected indentation", span);
                self.skip_indented_block();
                Err(())
            }
            TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof => {
                let span = self.peek().span;
                self.error(
                    "E0101",
                    format!("expected a statement, found {}", self.peek().kind.describe()),
                    span,
                );
                Err(())
            }
            _ => self.parse_expr_or_assign(),
        }
    }

    fn parse_let(&mut self, mutable: bool) -> Result<Stmt, ()> {
        let start = self.advance().span;
        let (name, name_span) = self.expect_ident("a variable name")?;
        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::Eq, " in a binding declaration")?;
        let init = self.parse_expr()?;
        let span = start.merge(init.span);
        self.expect_stmt_end()?;
        Ok(Stmt {
            kind: StmtKind::Let {
                mutable,
                name,
                name_span,
                ty,
                init,
            },
            span,
        })
    }

    fn parse_if(&mut self) -> Result<Stmt, ()> {
        let start = self.advance().span;
        let cond = self.parse_expr()?;
        let then_block = self.parse_suite("after the `if` condition")?;
        let mut elifs = Vec::new();
        while self.check(&TokenKind::Elif) {
            self.advance();
            let elif_cond = self.parse_expr()?;
            let block = self.parse_suite("after the `elif` condition")?;
            elifs.push((elif_cond, block));
        }
        let else_block = if self.check(&TokenKind::Else) {
            self.advance();
            Some(self.parse_suite("after `else`")?)
        } else {
            None
        };
        let end = else_block
            .as_ref()
            .map(|b| b.span)
            .or_else(|| elifs.last().map(|(_, b)| b.span))
            .unwrap_or(then_block.span);
        Ok(Stmt {
            kind: StmtKind::If {
                cond,
                then_block,
                elifs,
                else_block,
            },
            span: start.merge(end),
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, ()> {
        let start = self.advance().span;
        let cond = self.parse_expr()?;
        let body = self.parse_suite("after the `while` condition")?;
        let span = start.merge(body.span);
        Ok(Stmt {
            kind: StmtKind::While { cond, body },
            span,
        })
    }

    fn parse_for(&mut self) -> Result<Stmt, ()> {
        let start = self.advance().span;
        let (name, name_span) = self.expect_ident("a loop variable")?;
        self.expect(TokenKind::In, " after the loop variable")?;
        let iter = self.parse_expr()?;
        let body = self.parse_suite("after the `for` iterable")?;
        let span = start.merge(body.span);
        Ok(Stmt {
            kind: StmtKind::For {
                name,
                name_span,
                iter,
                body,
            },
            span,
        })
    }

    fn parse_match_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.expect(TokenKind::Match, " to start a match")?.span;
        let scrutinee = self.parse_expr()?;
        self.expect(TokenKind::Colon, " after the match value")?;
        self.expect(TokenKind::Newline, " after the match header")?;
        self.expect(TokenKind::Indent, " to start the match arms")?;
        let arms = self.parse_match_arms(false)?;
        let end = arms.last().map(|arm| arm.span).unwrap_or(scrutinee.span);
        Ok(Stmt {
            kind: StmtKind::Match { scrutinee, arms },
            span: start.merge(end),
        })
    }

    fn parse_match_expr(&mut self) -> Result<Expr, ()> {
        let start = self.expect(TokenKind::Match, " to start a match")?.span;
        let scrutinee = self.parse_expr()?;
        self.expect(TokenKind::Colon, " after the match value")?;
        if !self.check(&TokenKind::Newline) {
            // Newlines are suppressed inside brackets, so a block-shaped
            // match expression cannot be parsed there. Report that directly
            // instead of complaining about the first arm token.
            let span = self.peek().span;
            self.diagnostics.push(
                Diagnostic::error(
                    "E0101",
                    "a match expression must put its arms on following indented lines",
                    span,
                )
                .with_help(
                    "a match cannot appear inside parentheses or brackets; bind it to a variable first",
                ),
            );
            return Err(());
        }
        self.advance();
        self.expect(TokenKind::Indent, " to start the match arms")?;
        let arms = self.parse_match_arms(true)?;
        let end = arms.last().map(|arm| arm.span).unwrap_or(scrutinee.span);
        Ok(self.new_expr(
            ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            start.merge(end),
        ))
    }

    fn parse_match_arms(&mut self, expression: bool) -> Result<Vec<MatchArm>, ()> {
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) {
                self.advance();
                break;
            }
            if self.check(&TokenKind::Eof) {
                let span = self.peek().span;
                self.error("E0101", "unclosed match expression", span);
                break;
            }
            let pattern = self.parse_pattern()?;
            let guard = if self.check(&TokenKind::If) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            let body = if expression {
                self.expect(TokenKind::Colon, " after a match pattern")?;
                if self.check(&TokenKind::Newline) {
                    let span = self.peek().span;
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0101",
                            "a match expression arm must be a single expression",
                            span,
                        )
                        .with_help(
                            "put the arm body on the same line, or use a `match` statement for blocks",
                        ),
                    );
                    return Err(());
                }
                let body = self.parse_expr()?;
                self.expect_stmt_end()?;
                MatchArmBody::Expr(body)
            } else {
                MatchArmBody::Block(self.parse_suite("after a match pattern")?)
            };
            let span = pattern.span().merge(body.span());
            arms.push(MatchArm {
                pattern,
                guard,
                body,
                span,
            });
        }
        if arms.is_empty() {
            let span = self.peek().span;
            self.error("E0101", "a match must have at least one arm", span);
            return Err(());
        }
        Ok(arms)
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ()> {
        let start = self.peek().span;
        if let TokenKind::Ident(name) = &self.peek().kind
            && name == "_"
        {
            self.advance();
            return Ok(Pattern::Wildcard { span: start });
        }
        match self.peek().kind.clone() {
            TokenKind::Int(value) => {
                self.advance();
                return Ok(Pattern::Literal {
                    value: LiteralPattern::Int(value),
                    span: start,
                });
            }
            TokenKind::True => {
                self.advance();
                return Ok(Pattern::Literal {
                    value: LiteralPattern::Bool(true),
                    span: start,
                });
            }
            TokenKind::False => {
                self.advance();
                return Ok(Pattern::Literal {
                    value: LiteralPattern::Bool(false),
                    span: start,
                });
            }
            TokenKind::Minus => {
                self.advance();
                if let TokenKind::Int(value) = self.peek().kind.clone() {
                    let int_span = self.peek().span;
                    self.advance();
                    let Some(value) = value.checked_neg() else {
                        self.diagnostics.push(Diagnostic::error(
                            "E0113",
                            "integer literal is out of range for a pattern",
                            start.merge(int_span),
                        ));
                        return Err(());
                    };
                    return Ok(Pattern::Literal {
                        value: LiteralPattern::Int(value),
                        span: start.merge(int_span),
                    });
                }
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0101",
                        "expected an integer literal after `-` in a pattern",
                        self.peek().span,
                    )
                    .with_help("negative patterns must be integer literals such as `-1`"),
                );
                return Err(());
            }
            TokenKind::Float(_) => {
                let span = self.peek().span;
                self.advance();
                self.diagnostics.push(
                    Diagnostic::error("E0113", "float patterns are not supported", span)
                        .with_help("compare floats with `if`/`elif`, or match on an enum"),
                );
                return Err(());
            }
            TokenKind::Str(raw) => {
                self.advance();
                let expr = self.parse_string_literal(&raw, start)?;
                let ExprKind::Str(parts) = expr.kind else {
                    unreachable!("string literal parsing must produce a Str expression");
                };
                let mut text = String::new();
                for part in parts {
                    match part {
                        StrPart::Text(chunk) => text.push_str(&chunk),
                        StrPart::Expr(_) => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0113",
                                    "string patterns cannot contain interpolation",
                                    start,
                                )
                                .with_help("use a plain string literal in the pattern"),
                            );
                            return Err(());
                        }
                    }
                }
                return Ok(Pattern::Literal {
                    value: LiteralPattern::Str(text),
                    span: start,
                });
            }
            _ => {}
        }
        let (name, name_span) = self.expect_ident("a pattern")?;
        let mut span = name_span;
        let mut fields = Vec::new();
        if self.eat(&TokenKind::LParen) {
            if !self.check(&TokenKind::RParen) {
                loop {
                    let field_start = self.peek().span;
                    match self.peek().kind.clone() {
                        TokenKind::Ident(field_name) if field_name == "_" => {
                            self.advance();
                            fields.push(PatternArg::Wildcard { span: field_start });
                        }
                        TokenKind::Ident(field_name) => {
                            self.advance();
                            if self.check(&TokenKind::LParen) {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        "E0112",
                                        "nested patterns are not supported yet",
                                        field_start,
                                    )
                                    .with_help(
                                        "bind the payload and match on it in the arm body",
                                    ),
                                );
                                return Err(());
                            }
                            fields.push(PatternArg::Bind {
                                name: field_name,
                                name_span: field_start,
                            });
                        }
                        _ => {
                            let span = self.peek().span;
                            let message = format!(
                                "expected a binding or `_` in a pattern, found {}",
                                self.peek().kind.describe()
                            );
                            self.diagnostics.push(Diagnostic::error("E0101", message, span));
                            return Err(());
                        }
                    }
                    if self.eat(&TokenKind::Comma) {
                        if self.check(&TokenKind::RParen) {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            let close = self.expect(TokenKind::RParen, " to close the pattern")?;
            span = span.merge(close.span);
        }
        Ok(Pattern::Variant {
            name,
            name_span,
            fields,
            span,
        })
    }

    fn parse_return(&mut self) -> Result<Stmt, ()> {
        let start = self.advance().span;
        let value = if matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof
        ) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let end = value.as_ref().map(|e| e.span).unwrap_or(start);
        self.expect_stmt_end()?;
        Ok(Stmt {
            kind: StmtKind::Return(value),
            span: start.merge(end),
        })
    }

    fn parse_expr_or_assign(&mut self) -> Result<Stmt, ()> {
        let target = self.parse_expr()?;
        if let Some(assign_op) = self.peek_assign_op() {
            let _op_token = self.advance();
            let value = self.parse_expr()?;
            let span = target.span.merge(value.span);
            self.expect_stmt_end()?;
            if !matches!(
                target.kind,
                ExprKind::Var(_) | ExprKind::Field { .. } | ExprKind::Index { .. }
            ) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0103",
                        "invalid assignment target",
                        target.span,
                    )
                    .with_help("assign to a variable, field, or index"),
                );
                return Err(());
            }
            let op = match assign_op {
                AssignOp::Plain => None,
                AssignOp::Compound(op) => Some(op),
            };
            Ok(Stmt {
                kind: StmtKind::Assign { target, op, value },
                span,
            })
        } else {
            self.expect_stmt_end()?;
            let span = target.span;
            Ok(Stmt {
                kind: StmtKind::Expr(target),
                span,
            })
        }
    }

    fn peek_assign_op(&self) -> Option<AssignOp> {
        Some(match self.peek().kind {
            TokenKind::Eq => AssignOp::Plain,
            TokenKind::PlusEq => AssignOp::Compound(BinOp::Add),
            TokenKind::MinusEq => AssignOp::Compound(BinOp::Sub),
            TokenKind::StarEq => AssignOp::Compound(BinOp::Mul),
            TokenKind::SlashEq => AssignOp::Compound(BinOp::Div),
            TokenKind::SlashSlashEq => AssignOp::Compound(BinOp::FloorDiv),
            TokenKind::PercentEq => AssignOp::Compound(BinOp::Mod),
            _ => return None,
        })
    }

    fn parse_suite(&mut self, context: &str) -> Result<Block, ()> {
        let colon = self.expect(TokenKind::Colon, context)?;
        if self.check(&TokenKind::Newline) {
            self.advance();
            if self.block_depth >= MAX_BLOCK_DEPTH {
                let span = self.peek().span;
                self.error(
                    "E0109",
                    format!("block nesting is too deep (maximum is {MAX_BLOCK_DEPTH})"),
                    span,
                );
                self.skip_indented_block();
                return Ok(Block {
                    stmts: Vec::new(),
                    span: colon.span.merge(span),
                });
            }
            self.expect(TokenKind::Indent, " to start an indented block")?;
            self.block_depth += 1;
            let mut stmts = Vec::new();
            loop {
                self.skip_newlines();
                if self.check(&TokenKind::Dedent) {
                    let end = self.advance().span;
                    self.block_depth -= 1;
                    return Ok(Block {
                        stmts,
                        span: colon.span.merge(end),
                    });
                }
                if self.check(&TokenKind::Eof) {
                    let span = self.peek().span;
                    self.error("E0101", "unclosed block", span);
                    self.block_depth -= 1;
                    return Ok(Block {
                        stmts,
                        span: colon.span.merge(span),
                    });
                }
                match self.parse_stmt() {
                    Ok(stmt) => stmts.push(stmt),
                    Err(()) => self.synchronize(),
                }
            }
        } else {
            // Single-line suite: exactly one simple statement.
            let stmt = self.parse_simple_stmt()?;
            let span = colon.span.merge(stmt.span);
            Ok(Block {
                stmts: vec![stmt],
                span,
            })
        }
    }

    fn parse_simple_stmt(&mut self) -> Result<Stmt, ()> {
        match self.peek().kind.clone() {
            TokenKind::Let => self.parse_let(false),
            TokenKind::Var => self.parse_let(true),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => {
                let span = self.advance().span;
                self.expect_stmt_end()?;
                Ok(Stmt {
                    kind: StmtKind::Break,
                    span,
                })
            }
            TokenKind::Continue => {
                let span = self.advance().span;
                self.expect_stmt_end()?;
                Ok(Stmt {
                    kind: StmtKind::Continue,
                    span,
                })
            }
            TokenKind::If | TokenKind::While | TokenKind::For | TokenKind::Match => {
                let span = self.peek().span;
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0105",
                        "a compound statement cannot follow `:` on the same line",
                        span,
                    )
                    .with_help("put the block on the next indented line"),
                );
                Err(())
            }
            _ => self.parse_expr_or_assign(),
        }
    }

    // ---- expressions ----

    fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ()> {
        self.depth += 1;
        if self.depth > MAX_EXPR_DEPTH {
            self.depth -= 1;
            let span = self.peek().span;
            self.error(
                "E0107",
                "expression nesting is too deep",
                span,
            );
            return Err(());
        }
        let result = self.parse_expr_inner(min_bp);
        self.depth -= 1;
        result
    }

    fn parse_expr_inner(&mut self, min_bp: u8) -> Result<Expr, ()> {
        let token = self.peek().clone();
        let mut expr = match token.kind {
            TokenKind::Int(value) => {
                self.advance();
                self.new_expr(ExprKind::Int(value), token.span)
            }
            TokenKind::Float(value) => {
                self.advance();
                self.new_expr(ExprKind::Float(value), token.span)
            }
            TokenKind::True => {
                self.advance();
                self.new_expr(ExprKind::Bool(true), token.span)
            }
            TokenKind::False => {
                self.advance();
                self.new_expr(ExprKind::Bool(false), token.span)
            }
            TokenKind::Str(raw) => {
                self.advance();
                self.parse_string_literal(&raw, token.span)?
            }
            TokenKind::Ident(name) => {
                self.advance();
                self.new_expr(ExprKind::Var(name), token.span)
            }
            TokenKind::LParen => {
                self.advance();
                let mut inner = self.parse_expr()?;
                let close = self.expect(TokenKind::RParen, " to close the expression")?;
                inner.span = token.span.merge(close.span);
                inner
            }
            TokenKind::LBracket => self.parse_list_literal()?,
            TokenKind::LBrace => self.parse_map_literal()?,
            TokenKind::Match => self.parse_match_expr()?,
            TokenKind::Minus => {
                self.advance();
                let rhs = self.parse_expr_bp(UNARY_BP)?;
                let span = token.span.merge(rhs.span);
                self.new_expr(
                    ExprKind::Unary {
                        op: UnOp::Neg,
                        expr: Box::new(rhs),
                    },
                    span,
                )
            }
            TokenKind::Not => {
                self.advance();
                let rhs = self.parse_expr_bp(UNARY_BP)?;
                let span = token.span.merge(rhs.span);
                self.new_expr(
                    ExprKind::Unary {
                        op: UnOp::Not,
                        expr: Box::new(rhs),
                    },
                    span,
                )
            }
            _ => {
                let span = token.span;
                let message = format!(
                    "expected an expression, found {}",
                    token.kind.describe()
                );
                self.diagnostics.push(Diagnostic::error("E0101", message, span));
                return Err(());
            }
        };

        // Postfix: call, index, field.
        loop {
            match self.peek().kind.clone() {
                TokenKind::LParen => {
                    let open = self.advance();
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            let arg_start = self.peek().span;
                            let name = if matches!(self.peek().kind, TokenKind::Ident(_))
                                && matches!(self.peek_kind_n(1), TokenKind::Eq)
                            {
                                let ident = self.advance();
                                let name = match ident.kind {
                                    TokenKind::Ident(n) => n,
                                    _ => unreachable!(),
                                };
                                self.advance(); // '='
                                Some((name, ident.span))
                            } else {
                                None
                            };
                            let value = self.parse_expr()?;
                            let span = arg_start.merge(value.span);
                            args.push(Arg { name, value, span });
                            if self.eat(&TokenKind::Comma) {
                                if self.check(&TokenKind::RParen) {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    let close = self.expect(TokenKind::RParen, " to close the call")?;
                    let span = expr.span.merge(close.span).merge(open.span);
                    expr = self.new_expr(
                        ExprKind::Call {
                            callee: Box::new(expr),
                            args,
                        },
                        span,
                    );
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    let close = self.expect(TokenKind::RBracket, " to close the index")?;
                    let span = expr.span.merge(close.span);
                    expr = self.new_expr(
                        ExprKind::Index {
                            base: Box::new(expr),
                            index: Box::new(index),
                        },
                        span,
                    );
                }
                TokenKind::Dot => {
                    self.advance();
                    let (name, name_span) = self.expect_ident("a field name")?;
                    let span = expr.span.merge(name_span);
                    expr = self.new_expr(
                        ExprKind::Field {
                            base: Box::new(expr),
                            name,
                            name_span,
                        },
                        span,
                    );
                }
                _ => break,
            }
        }

        // Infix.
        while let Some((op, left_bp, right_bp)) = self.peek_binop() {
            if left_bp < min_bp {
                break;
            }
            self.advance();
            let rhs = self.parse_expr_bp(right_bp)?;
            let span = expr.span.merge(rhs.span);
            expr = self.new_expr(
                ExprKind::Binary {
                    op,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                },
                span,
            );
        }

        Ok(expr)
    }

    fn peek_binop(&self) -> Option<(BinOp, u8, u8)> {
        Some(match self.peek().kind {
            TokenKind::Or => (BinOp::Or, 10, 11),
            TokenKind::And => (BinOp::And, 20, 21),
            TokenKind::EqEq => (BinOp::Eq, 30, 31),
            TokenKind::NotEq => (BinOp::NotEq, 30, 31),
            TokenKind::Lt => (BinOp::Lt, 30, 31),
            TokenKind::LtEq => (BinOp::LtEq, 30, 31),
            TokenKind::Gt => (BinOp::Gt, 30, 31),
            TokenKind::GtEq => (BinOp::GtEq, 30, 31),
            TokenKind::Plus => (BinOp::Add, 40, 41),
            TokenKind::Minus => (BinOp::Sub, 40, 41),
            TokenKind::Star => (BinOp::Mul, 50, 51),
            TokenKind::Slash => (BinOp::Div, 50, 51),
            TokenKind::SlashSlash => (BinOp::FloorDiv, 50, 51),
            TokenKind::Percent => (BinOp::Mod, 50, 51),
            TokenKind::StarStar => (BinOp::Pow, 60, 60),
            _ => return None,
        })
    }

    fn parse_list_literal(&mut self) -> Result<Expr, ()> {
        let open = self.expect(TokenKind::LBracket, "")?;
        let mut elements = Vec::new();
        if !self.check(&TokenKind::RBracket) {
            loop {
                elements.push(self.parse_expr()?);
                if self.eat(&TokenKind::Comma) {
                    if self.check(&TokenKind::RBracket) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        let close = self.expect(TokenKind::RBracket, " to close the list")?;
        Ok(self.new_expr(
            ExprKind::List(elements),
            open.span.merge(close.span),
        ))
    }

    fn parse_map_literal(&mut self) -> Result<Expr, ()> {
        let open = self.expect(TokenKind::LBrace, "")?;
        let mut entries = Vec::new();
        if !self.check(&TokenKind::RBrace) {
            loop {
                let key = self.parse_expr()?;
                self.expect(TokenKind::Colon, " after a map key")?;
                let value = self.parse_expr()?;
                entries.push((key, value));
                if self.eat(&TokenKind::Comma) {
                    if self.check(&TokenKind::RBrace) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        let close = self.expect(TokenKind::RBrace, " to close the map")?;
        Ok(self.new_expr(
            ExprKind::Map(entries),
            open.span.merge(close.span),
        ))
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ()> {
        self.type_depth += 1;
        if self.type_depth > MAX_TYPE_DEPTH {
            self.type_depth -= 1;
            let span = self.peek().span;
            self.error(
                "E0110",
                format!("type nesting is too deep (maximum is {MAX_TYPE_DEPTH})"),
                span,
            );
            return Err(());
        }
        let result = self.parse_type_inner();
        self.type_depth -= 1;
        result
    }

    fn parse_type_inner(&mut self) -> Result<TypeExpr, ()> {
        let (name, name_span) = self.expect_ident("a type")?;
        let mut span = name_span;
        let mut args = Vec::new();
        if self.check(&TokenKind::LBracket) {
            self.advance();
            if !self.check(&TokenKind::RBracket) {
                loop {
                    args.push(self.parse_type()?);
                    if self.eat(&TokenKind::Comma) {
                        if self.check(&TokenKind::RBracket) {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            let close = self.expect(TokenKind::RBracket, " to close type arguments")?;
            span = span.merge(close.span);
        }
        Ok(TypeExpr {
            kind: TypeExprKind::Name { name, args },
            span,
        })
    }

    // ---- strings ----

    fn parse_string_literal(&mut self, raw: &str, span: Span) -> Result<Expr, ()> {
        let base = span.start as usize + 1;
        let mut parts: Vec<StrPart> = Vec::new();
        let mut text = String::new();
        let mut i = 0usize;

        while i < raw.len() {
            let ch = raw[i..].chars().next().unwrap();
            match ch {
                '\\' => {
                    let escape_start = i;
                    i += 1;
                    if i >= raw.len() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0003",
                                "trailing backslash in string",
                                Span::new(base + escape_start, base + i),
                            )
                            .with_help("escape the backslash as `\\\\`"),
                        );
                        break;
                    }
                    let esc = raw[i..].chars().next().unwrap();
                    let decoded = match esc {
                        'n' => Some('\n'),
                        't' => Some('\t'),
                        'r' => Some('\r'),
                        '\\' => Some('\\'),
                        '"' => Some('"'),
                        '{' => Some('{'),
                        '}' => Some('}'),
                        _ => None,
                    };
                    match decoded {
                        Some(c) => text.push(c),
                        None => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0003",
                                    format!("unknown escape sequence `\\{esc}`"),
                                    Span::new(base + escape_start, base + i + esc.len_utf8()),
                                )
                                .with_help("valid escapes: \\n \\t \\r \\\\ \\\" \\{ \\}"),
                            );
                            text.push(esc);
                        }
                    }
                    i += esc.len_utf8();
                }
                '{' => {
                    if raw[i..].starts_with("{{") {
                        text.push('{');
                        i += 2;
                        continue;
                    }
                    let expr_start = i + 1;
                    let mut j = expr_start;
                    let mut depth = 0usize;
                    let mut in_string = false;
                    let mut found = None;
                    while j < raw.len() {
                        let c = raw[j..].chars().next().unwrap();
                        if in_string {
                            if c == '\\' {
                                j += 1;
                                if j < raw.len() {
                                    let n = raw[j..].chars().next().unwrap();
                                    j += n.len_utf8();
                                }
                                continue;
                            }
                            if c == '"' {
                                in_string = false;
                            }
                            j += c.len_utf8();
                            continue;
                        }
                        match c {
                            '"' => in_string = true,
                            '{' => depth += 1,
                            '}' => {
                                if depth == 0 {
                                    found = Some(j);
                                    break;
                                }
                                depth -= 1;
                            }
                            _ => {}
                        }
                        j += c.len_utf8();
                    }
                    let Some(end) = found else {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0003",
                                "unterminated string interpolation",
                                Span::new(base + i, base + raw.len()),
                            )
                            .with_help("add a closing `}` or escape it as `}}`"),
                        );
                        break;
                    };
                    let expr_source = &raw[expr_start..end];
                    if expr_source.trim().is_empty() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0003",
                                "empty string interpolation",
                                Span::new(base + expr_start, base + end),
                            )
                            .with_help("put an expression between the braces"),
                        );
                        return Err(());
                    }
                    if !text.is_empty() {
                        parts.push(StrPart::Text(std::mem::take(&mut text)));
                    }
                    let expr = self.parse_embedded_expr(expr_source, base + expr_start)?;
                    parts.push(StrPart::Expr(expr));
                    i = end + 1;
                }
                '}' => {
                    if raw[i..].starts_with("}}") {
                        text.push('}');
                        i += 2;
                        continue;
                    }
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0003",
                            "unexpected `}` in string",
                            Span::new(base + i, base + i + 1),
                        )
                        .with_help("use `}}` for a literal closing brace"),
                    );
                    text.push('}');
                    i += 1;
                }
                other => {
                    text.push(other);
                    i += other.len_utf8();
                }
            }
        }

        if !text.is_empty() {
            parts.push(StrPart::Text(text));
        }
        if parts.is_empty() {
            parts.push(StrPart::Text(String::new()));
        }
        Ok(self.new_expr(ExprKind::Str(parts), span))
    }

    fn parse_embedded_expr(&mut self, source: &str, base: usize) -> Result<Expr, ()> {
        self.interp_depth += 1;
        if self.interp_depth > MAX_INTERP_DEPTH {
            self.interp_depth -= 1;
            self.error(
                "E0111",
                format!(
                    "string interpolation nesting is too deep (maximum is {MAX_INTERP_DEPTH})"
                ),
                Span::point(base),
            );
            return Err(());
        }
        // Interpolation fragments are single-line expressions. Strip leading
        // spaces so the fragment lexer does not interpret them as indentation.
        let leading = source.len() - source.trim_start().len();
        let source = source.trim_start();
        let base = base + leading;
        let lexed = lex_fragment(source, base);
        let had_errors = lexed.diagnostics.has_errors();
        self.diagnostics.extend(lexed.diagnostics);
        if had_errors {
            self.interp_depth -= 1;
            return Err(());
        }
        let mut sub = Parser {
            source: self.source,
            tokens: lexed.tokens,
            pos: 0,
            diagnostics: Diagnostics::new(),
            next_id: self.next_id,
            depth: 0,
            block_depth: 0,
            type_depth: self.type_depth,
            interp_depth: self.interp_depth,
            depth_reported: self.depth_reported,
        };
        let result = sub.parse_expr();
        if result.is_ok()
            && !matches!(sub.peek().kind, TokenKind::Eof | TokenKind::Newline)
        {
            let span = sub.peek().span;
            let message = format!(
                "unexpected {} in string interpolation",
                sub.peek().kind.describe()
            );
            sub.diagnostics
                .push(Diagnostic::error("E0101", message, span));
        }
        self.next_id = sub.next_id;
        self.depth_reported = sub.depth_reported;
        self.diagnostics.extend(sub.diagnostics);
        self.interp_depth -= 1;
        result
    }
}

/// Height of an expression tree, used to bound later recursive passes.
fn expr_depth(kind: &ExprKind) -> usize {
    let max_child = |depths: &mut dyn Iterator<Item = usize>| depths.max().unwrap_or(0);
    match kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Var(_) => 1,
        ExprKind::Str(parts) => {
            1 + max_child(&mut parts.iter().filter_map(|part| match part {
                StrPart::Text(_) => None,
                StrPart::Expr(expr) => Some(expr.depth),
            }))
        }
        ExprKind::List(elements) => 1 + max_child(&mut elements.iter().map(|e| e.depth)),
        ExprKind::Map(entries) => {
            1 + max_child(&mut entries.iter().flat_map(|(key, value)| [key.depth, value.depth]))
        }
        ExprKind::Unary { expr, .. } => 1 + expr.depth,
        ExprKind::Binary { lhs, rhs, .. } => 1 + lhs.depth.max(rhs.depth),
        ExprKind::Call { callee, args } => {
            1 + max_child(
                &mut std::iter::once(callee.depth).chain(args.iter().map(|arg| arg.value.depth)),
            )
        }
        ExprKind::Index { base, index } => 1 + base.depth.max(index.depth),
        ExprKind::Field { base, .. } => 1 + base.depth,
        ExprKind::Match { scrutinee, arms } => {
            1 + max_child(&mut std::iter::once(scrutinee.depth).chain(arms.iter().map(|arm| {
                match &arm.body {
                    MatchArmBody::Expr(expr) => expr.depth,
                    MatchArmBody::Block(_) => 1,
                }
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_ok(source: &str) -> Program {
        let lexed = lex(source);
        assert!(!lexed.diagnostics.has_errors(), "{:?}", lexed.diagnostics);
        let (program, diagnostics) = parse(source, lexed.tokens);
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics);
        program
    }

    fn parse_err(source: &str) -> Diagnostics {
        let lexed = lex(source);
        let (_program, diagnostics) = parse(source, lexed.tokens);
        diagnostics
    }

    #[test]
    fn parses_a_function() {
        let program = parse_ok("fn add(a: Int, b: Int) -> Int:\n    return a + b\n");
        assert_eq!(program.items.len(), 1);
        let Item::Fn(function) = &program.items[0] else {
            panic!("expected function");
        };
        assert_eq!(function.name, "add");
        assert_eq!(function.params.len(), 2);
        assert!(function.ret.is_some());
        assert_eq!(function.body.stmts.len(), 1);
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        let program = parse_ok("let x = 1 + 2 * 3\n");
        let Item::Stmt(stmt) = &program.items[0] else {
            panic!()
        };
        let StmtKind::Let { init, .. } = &stmt.kind else {
            panic!()
        };
        let ExprKind::Binary { op, rhs, .. } = &init.kind else {
            panic!()
        };
        assert_eq!(*op, BinOp::Add);
        assert!(matches!(rhs.kind, ExprKind::Binary { op: BinOp::Mul, .. }));
    }

    #[test]
    fn power_is_right_associative() {
        let program = parse_ok("let x = 2 ** 3 ** 2\n");
        let Item::Stmt(stmt) = &program.items[0] else {
            panic!()
        };
        let StmtKind::Let { init, .. } = &stmt.kind else {
            panic!()
        };
        let ExprKind::Binary { op, rhs, .. } = &init.kind else {
            panic!()
        };
        assert_eq!(*op, BinOp::Pow);
        assert!(matches!(rhs.kind, ExprKind::Binary { op: BinOp::Pow, .. }));
    }

    #[test]
    fn parses_if_elif_else() {
        let program = parse_ok(
            "if x > 0:\n    print(1)\nelif x < 0:\n    print(2)\nelse:\n    print(3)\n",
        );
        let Item::Stmt(stmt) = &program.items[0] else {
            panic!()
        };
        let StmtKind::If {
            elifs, else_block, ..
        } = &stmt.kind
        else {
            panic!()
        };
        assert_eq!(elifs.len(), 1);
        assert!(else_block.is_some());
    }

    #[test]
    fn parses_single_line_suite() {
        let program = parse_ok("if ready: print(\"go\")\n");
        let Item::Stmt(stmt) = &program.items[0] else {
            panic!()
        };
        let StmtKind::If { then_block, .. } = &stmt.kind else {
            panic!()
        };
        assert_eq!(then_block.stmts.len(), 1);
    }

    #[test]
    fn parses_string_interpolation() {
        let program = parse_ok("let msg = \"hello {name}, {1 + 2}\"\n");
        let Item::Stmt(stmt) = &program.items[0] else {
            panic!()
        };
        let StmtKind::Let { init, .. } = &stmt.kind else {
            panic!()
        };
        let ExprKind::Str(parts) = &init.kind else {
            panic!()
        };
        assert_eq!(parts.len(), 4);
    }

    #[test]
    fn parses_struct_and_constructor() {
        let program = parse_ok(
            "struct Point:\n    x: Int\n    y: Int\n\nlet p = Point(x=1, y=2)\n",
        );
        assert_eq!(program.items.len(), 2);
        let Item::Struct(decl) = &program.items[0] else {
            panic!()
        };
        assert_eq!(decl.fields.len(), 2);
        let Item::Stmt(stmt) = &program.items[1] else {
            panic!()
        };
        let StmtKind::Let { init, .. } = &stmt.kind else {
            panic!()
        };
        let ExprKind::Call { args, .. } = &init.kind else {
            panic!()
        };
        assert!(args.iter().all(|a| a.name.is_some()));
    }

    #[test]
    fn reports_missing_colon() {
        let diagnostics = parse_err("fn main()\n    print(1)\n");
        assert!(diagnostics.has_errors());
        assert!(diagnostics.items.iter().any(|d| d.code == "E0102"));
    }

    #[test]
    fn reports_invalid_assignment_target() {
        let diagnostics = parse_err("1 + 2 = 3\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0103"));
    }

    #[test]
    fn reports_compound_statement_after_colon() {
        let diagnostics = parse_err("if x: while y:\n    print(1)\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0105"));
    }

    #[test]
    fn bracketed_match_expression_gets_a_targeted_diagnostic() {
        let diagnostics = parse_err("fn main():\n    print(match x:\n        A: 1)\n");
        let diagnostic = diagnostics
            .items
            .iter()
            .find(|d| d.code == "E0101" && d.message.contains("indented lines"))
            .expect("targeted match diagnostic");
        assert!(
            diagnostic
                .notes
                .iter()
                .any(|note| note.contains("bind it to a variable")),
            "{diagnostic:?}"
        );
    }

    #[test]
    fn deeply_nested_expressions_do_not_overflow() {
        let source = format!("let x = {}1{}\n", "(".repeat(400), ")".repeat(400));
        let lexed = lex(&source);
        let (_program, diagnostics) = parse(&source, lexed.tokens);
        assert!(diagnostics.items.iter().any(|d| d.code == "E0107"));
    }

    #[test]
    fn long_left_deep_chains_are_rejected_without_overflow() {
        let source = format!("let x = {}\n", vec!["1"; 2000].join(" + "));
        let lexed = lex(&source);
        let (program, diagnostics) = parse(&source, lexed.tokens);
        assert!(diagnostics.items.iter().any(|d| d.code == "E0108"));
        // The returned tree stays bounded even though parsing continued.
        let Item::Stmt(stmt) = &program.items[0] else {
            panic!()
        };
        let StmtKind::Let { init, .. } = &stmt.kind else {
            panic!()
        };
        assert!(init.depth <= MAX_AST_DEPTH);
    }

    #[test]
    fn deeply_nested_blocks_are_rejected_without_overflow() {
        let depth = MAX_BLOCK_DEPTH + 20;
        let mut source = String::from("fn main():\n");
        for level in 0..depth {
            source.push_str(&"    ".repeat(level + 1));
            source.push_str("if true:\n");
        }
        source.push_str(&"    ".repeat(depth + 1));
        source.push_str("print(1)\n");
        let lexed = lex(&source);
        let (_program, diagnostics) = parse(&source, lexed.tokens);
        assert!(diagnostics.items.iter().any(|d| d.code == "E0109"));
    }

    #[test]
    fn deeply_nested_types_are_rejected_without_overflow() {
        let source = format!(
            "fn main():\n    let x: {}Int{} = 1\n",
            "List[".repeat(MAX_TYPE_DEPTH + 10),
            "]".repeat(MAX_TYPE_DEPTH + 10)
        );
        let lexed = lex(&source);
        let (_program, diagnostics) = parse(&source, lexed.tokens);
        assert!(diagnostics.items.iter().any(|d| d.code == "E0110"));
    }

    #[test]
    fn parses_enum_declarations_and_match_forms() {
        let source = concat!(
            "enum Shape:\n",
            "    Empty\n",
            "    Circle(radius: Float)\n",
            "\n",
            "fn area(shape: Shape) -> Float:\n",
            "    match shape:\n",
            "        Empty:\n",
            "            return 0.0\n",
            "        Circle(radius):\n",
            "            return radius\n",
            "\n",
            "fn label(shape: Shape) -> Str:\n",
            "    return match shape:\n",
            "        Empty: \"empty\"\n",
            "        Circle(radius): \"circle\"\n",
        );
        let program = parse_ok(source);
        let Item::Enum(decl) = &program.items[0] else {
            panic!("expected enum");
        };
        assert_eq!(decl.variants.len(), 2);
        assert_eq!(decl.variants[1].fields.len(), 1);
        let Item::Fn(area) = &program.items[1] else {
            panic!("expected function");
        };
        assert!(matches!(area.body.stmts[0].kind, StmtKind::Match { .. }));
        let Item::Fn(label) = &program.items[2] else {
            panic!("expected function");
        };
        let StmtKind::Return(Some(expr)) = &label.body.stmts[0].kind else {
            panic!("expected return");
        };
        assert!(matches!(expr.kind, ExprKind::Match { .. }));
    }

    #[test]
    fn parses_match_guards() {
        let source = concat!(
            "fn label(n: Int) -> Str:\n",
            "    match n:\n",
            "        _ if n < 0:\n",
            "            return \"negative\"\n",
            "        _:\n",
            "            return \"other\"\n",
            "\n",
            "fn classify(n: Int) -> Str:\n",
            "    return match n:\n",
            "        0 if true: \"zero\"\n",
            "        _ if n > 0: \"positive\"\n",
            "        _: \"negative\"\n",
        );
        let program = parse_ok(source);
        let Item::Fn(label) = &program.items[0] else {
            panic!("expected function");
        };
        let StmtKind::Match { arms, .. } = &label.body.stmts[0].kind else {
            panic!("expected match statement");
        };
        assert!(arms[0].guard.is_some());
        assert!(arms[1].guard.is_none());
        let Item::Fn(classify) = &program.items[1] else {
            panic!("expected function");
        };
        let StmtKind::Return(Some(Expr {
            kind: ExprKind::Match { arms, .. },
            ..
        })) = &classify.body.stmts[0].kind
        else {
            panic!("expected match expression");
        };
        assert!(arms[0].guard.is_some());
        assert!(arms[1].guard.is_some());
        assert!(arms[2].guard.is_none());
    }

    #[test]
    fn parses_literal_patterns() {
        let source = concat!(
            "fn label(n: Int, s: Str, b: Bool) -> Str:\n",
            "    let by_int: Str = match n:\n",
            "        0: \"zero\"\n",
            "        -2: \"minus two\"\n",
            "        _: \"other\"\n",
            "    let by_str: Str = match s:\n",
            "        \"a\": \"letter a\"\n",
            "        _: \"other\"\n",
            "    return match b:\n",
            "        true: \"yes\"\n",
            "        false: \"no\"\n",
        );
        let program = parse_ok(source);
        let Item::Fn(label) = &program.items[0] else {
            panic!("expected function");
        };
        let StmtKind::Let {
            init: Expr {
                kind: ExprKind::Match { arms, .. },
                ..
            },
            ..
        } = &label.body.stmts[0].kind
        else {
            panic!("expected match expression");
        };
        assert!(matches!(
            arms[0].pattern,
            Pattern::Literal {
                value: LiteralPattern::Int(0),
                ..
            }
        ));
        assert!(matches!(
            arms[1].pattern,
            Pattern::Literal {
                value: LiteralPattern::Int(-2),
                ..
            }
        ));
        let StmtKind::Let {
            init: Expr {
                kind: ExprKind::Match { arms, .. },
                ..
            },
            ..
        } = &label.body.stmts[1].kind
        else {
            panic!("expected match expression");
        };
        assert!(matches!(
            arms[0].pattern,
            Pattern::Literal {
                value: LiteralPattern::Str(ref text),
                ..
            } if text == "a"
        ));
        let StmtKind::Return(Some(Expr {
            kind: ExprKind::Match { arms, .. },
            ..
        })) = &label.body.stmts[2].kind
        else {
            panic!("expected match expression");
        };
        assert!(matches!(
            arms[0].pattern,
            Pattern::Literal {
                value: LiteralPattern::Bool(true),
                ..
            }
        ));
        assert!(matches!(
            arms[1].pattern,
            Pattern::Literal {
                value: LiteralPattern::Bool(false),
                ..
            }
        ));
    }
}
