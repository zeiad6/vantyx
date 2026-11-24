//! Abstract syntax tree.
//!
//! Every expression carries a stable `ExprId` so the checker can attach types
//! without mutating the tree; future IR lowering and tooling reuse the same IDs.

use crate::span::Span;

/// Globally unique across a compilation. Multi-file parsing seeds the high 32
/// bits with the source-file id so imported and entry expressions never share
/// an id in the checker's type map.
pub type ExprId = u64;

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Item {
    Fn(FnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Import(ImportDecl),
    Stmt(Stmt),
}

/// A top-level `import "path"` or `from "path" import a, b` declaration.
///
/// The module loader resolves these before checking; the checker rejects any
/// that survive (for example in stdin or REPL input).
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: String,
    pub path_span: Span,
    /// `None` imports every declaration in the module.
    pub names: Option<Vec<ImportName>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ImportName {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub name_span: Span,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub name_span: Span,
    pub variants: Vec<VariantDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VariantDecl {
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Let {
        mutable: bool,
        name: String,
        name_span: Span,
        ty: Option<TypeExpr>,
        init: Expr,
    },
    Assign {
        target: Expr,
        op: Option<BinOp>,
        value: Expr,
    },
    Expr(Expr),
    If {
        cond: Expr,
        then_block: Block,
        elifs: Vec<(Expr, Block)>,
        else_block: Option<Block>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    For {
        name: String,
        name_span: Span,
        iter: Expr,
        body: Block,
    },
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub id: ExprId,
    pub kind: ExprKind,
    pub span: Span,
    /// Tree height of this expression, computed by the parser. It bounds the
    /// recursion depth of every later pass (checker, lowering, interpreter).
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Vec<StrPart>),
    Var(String),
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    /// Postfix `?`: unwrap the success payload of a two-variant result enum,
    /// or return the failure variant from the enclosing function.
    Try {
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Field {
        base: Box<Expr>,
        name: String,
        name_span: Span,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: MatchArmBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MatchArmBody {
    Expr(Expr),
    Block(Block),
}

impl MatchArmBody {
    pub fn span(&self) -> Span {
        match self {
            MatchArmBody::Expr(expr) => expr.span,
            MatchArmBody::Block(block) => block.span,
        }
    }
}

/// v0.1 patterns are deliberately small: a wildcard, a literal, a variant with
/// positional bindings/wildcards/nested patterns, or an alternation of those
/// patterns. Payload positions may recursively contain another pattern.
#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard {
        span: Span,
    },
    Literal {
        value: LiteralPattern,
        span: Span,
    },
    /// Integer range: `low..high` is half-open, `low..=high` is inclusive.
    Range {
        start: i64,
        end: i64,
        inclusive: bool,
        span: Span,
    },
    Variant {
        name: String,
        name_span: Span,
        fields: Vec<PatternArg>,
        span: Span,
    },
    Or {
        alternatives: Vec<Pattern>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralPattern {
    Int(i64),
    Bool(bool),
    Str(String),
}

impl LiteralPattern {
    pub fn type_name(&self) -> &'static str {
        match self {
            LiteralPattern::Int(_) => "Int",
            LiteralPattern::Bool(_) => "Bool",
            LiteralPattern::Str(_) => "Str",
        }
    }
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard { span }
            | Pattern::Literal { span, .. }
            | Pattern::Range { span, .. }
            | Pattern::Variant { span, .. }
            | Pattern::Or { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PatternArg {
    Bind {
        name: String,
        name_span: Span,
    },
    Wildcard {
        span: Span,
    },
    Nested {
        pattern: Box<Pattern>,
        span: Span,
    },
}

impl PatternArg {
    pub fn span(&self) -> Span {
        match self {
            PatternArg::Bind { name_span, .. } => *name_span,
            PatternArg::Wildcard { span } | PatternArg::Nested { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StrPart {
    Text(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub name: Option<(String, Span)>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Pow,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::FloorDiv => "//",
            BinOp::Mod => "%",
            BinOp::Pow => "**",
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Lt => "<",
            BinOp::LtEq => "<=",
            BinOp::Gt => ">",
            BinOp::GtEq => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Eq
                | BinOp::NotEq
                | BinOp::Lt
                | BinOp::LtEq
                | BinOp::Gt
                | BinOp::GtEq
        )
    }

    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::FloorDiv
                | BinOp::Mod
                | BinOp::Pow
        )
    }
}

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeExprKind {
    Name { name: String, args: Vec<TypeExpr> },
}

impl TypeExpr {
    pub fn name(&self) -> &str {
        match &self.kind {
            TypeExprKind::Name { name, .. } => name,
        }
    }
}
