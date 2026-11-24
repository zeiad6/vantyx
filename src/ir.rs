//! Typed intermediate representation for the native backend.
//!
//! The IR is deliberately small: it covers the statically typed subset of
//! Auren that the C backend can lower without a dynamic value representation.
//! `List[T]` is supported for statically known `T`; `Any` and `Map` are
//! rejected with `E0401` instead of being silently miscompiled.
//!
//! The interpreter remains the semantic oracle. Differential tests compare
//! native output against interpreter output for the supported subset.

use crate::ast::*;
use crate::checker::{Entry, SemanticModel, is_builtin};
use crate::span::Span;
use crate::types::{Type, resolve_type_expr};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrType {
    Unit,
    Int,
    Float,
    Bool,
    Str,
    List(Box<IrType>),
    Map(Box<IrType>, Box<IrType>),
    Struct(String),
    Enum(String),
}

impl IrType {
    pub fn name(&self) -> String {
        match self {
            IrType::Unit => "Unit".to_string(),
            IrType::Int => "Int".to_string(),
            IrType::Float => "Float".to_string(),
            IrType::Bool => "Bool".to_string(),
            IrType::Str => "Str".to_string(),
            IrType::List(inner) => format!("List[{}]", inner.name()),
            IrType::Map(key, value) => format!("Map[{}, {}]", key.name(), value.name()),
            IrType::Struct(name) => name.clone(),
            IrType::Enum(name) => name.clone(),
        }
    }

    /// Name reported by the `type` builtin. Collections report their
    /// container kind, matching the interpreter.
    pub fn runtime_name(&self) -> String {
        match self {
            IrType::List(_) => "List".to_string(),
            IrType::Map(_, _) => "Map".to_string(),
            other => other.name(),
        }
    }

    pub fn element_type(&self) -> Option<&IrType> {
        match self {
            IrType::List(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn key_value_types(&self) -> Option<(&IrType, &IrType)> {
        match self {
            IrType::Map(key, value) => Some((key, value)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrEntry {
    TopLevel,
    Main,
}

#[derive(Clone, Debug)]
pub struct IrProgram {
    pub structs: Vec<IrStruct>,
    pub enums: Vec<IrEnum>,
    pub globals: Vec<IrGlobal>,
    pub functions: Vec<IrFunction>,
    pub top_level: Vec<IrStmt>,
    pub entry: IrEntry,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrStruct {
    pub name: String,
    pub fields: Vec<IrField>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrField {
    pub name: String,
    pub ty: IrType,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrEnum {
    pub name: String,
    pub variants: Vec<IrVariant>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrVariant {
    pub name: String,
    /// Zero-based declaration index; used as the native tag.
    pub tag: usize,
    pub fields: Vec<IrField>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrGlobal {
    pub name: String,
    pub ty: IrType,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrParam>,
    pub ret: IrType,
    pub body: Vec<IrStmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrParam {
    pub name: String,
    pub ty: IrType,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct IrStmt {
    pub kind: IrStmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum IrStmtKind {
    Let {
        name: String,
        ty: IrType,
        init: IrExpr,
    },
    Assign {
        target: IrPlace,
        value: IrExpr,
    },
    Expr(IrExpr),
    If {
        cond: IrExpr,
        then_body: Vec<IrStmt>,
        elifs: Vec<(IrExpr, Vec<IrStmt>)>,
        else_body: Option<Vec<IrStmt>>,
    },
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    ForRange {
        name: String,
        start: IrExpr,
        end: IrExpr,
        step: IrExpr,
        body: Vec<IrStmt>,
    },
    ForList {
        name: String,
        list: IrExpr,
        body: Vec<IrStmt>,
    },
    ForStr {
        name: String,
        text: IrExpr,
        body: Vec<IrStmt>,
    },
    ForMap {
        name: String,
        map: IrExpr,
        body: Vec<IrStmt>,
    },
    Match {
        scrutinee: IrExpr,
        arms: Vec<IrMatchArm>,
    },
    Return(Option<IrExpr>),
    Break,
    Continue,
}

#[derive(Clone, Debug)]
pub enum IrPlace {
    Var(String),
    Global(String),
    Field {
        base: IrExpr,
        field: String,
    },
    Index {
        base: IrExpr,
        index: IrExpr,
    },
}

#[derive(Clone, Debug)]
pub struct IrExpr {
    pub kind: IrExprKind,
    pub ty: IrType,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum IrExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Vec<IrStrPart>),
    List(Vec<IrExpr>),
    Map(Vec<(IrExpr, IrExpr)>),
    Var(String),
    Global(String),
    Binary {
        op: BinOp,
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
    },
    Unary {
        op: UnOp,
        operand: Box<IrExpr>,
    },
    Call {
        name: String,
        args: Vec<IrExpr>,
    },
    Builtin {
        name: String,
        args: Vec<IrExpr>,
    },
    Field {
        base: Box<IrExpr>,
        name: String,
    },
    Index {
        base: Box<IrExpr>,
        index: Box<IrExpr>,
    },
    StrIndex {
        base: Box<IrExpr>,
        index: Box<IrExpr>,
    },
    Construct {
        name: String,
        args: Vec<IrExpr>,
    },
    ConstructVariant {
        enum_name: String,
        variant: String,
        tag: usize,
        args: Vec<IrExpr>,
    },
    Match {
        scrutinee: Box<IrExpr>,
        arms: Vec<IrMatchArm>,
    },
}

#[derive(Clone, Debug)]
pub struct IrMatchArm {
    pub pattern: IrPattern,
    pub guard: Option<IrExpr>,
    pub body: IrMatchBody,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum IrPattern {
    Wildcard,
    Literal {
        value: IrLiteral,
    },
    Variant {
        enum_name: String,
        variant: String,
        tag: usize,
        /// One entry per payload field; `None` means the pattern used `_`.
        bindings: Vec<Option<IrBinding>>,
    },
}

#[derive(Clone, Debug)]
pub enum IrLiteral {
    Int(i64),
    Bool(bool),
    Str(String),
}

#[derive(Clone, Debug)]
pub struct IrBinding {
    pub name: String,
    pub field: String,
    pub ty: IrType,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum IrMatchBody {
    Expr(IrExpr),
    Block(Vec<IrStmt>),
}

#[derive(Clone, Debug)]
pub enum IrStrPart {
    Text(String),
    Value(Box<IrExpr>),
}

#[derive(Clone, Debug)]
pub struct IrError {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

impl IrError {
    fn unsupported(message: impl Into<String>, span: Span, help: impl Into<String>) -> Self {
        Self {
            code: "E0401",
            message: message.into(),
            span,
            help: Some(help.into()),
        }
    }

    fn internal(message: impl Into<String>, span: Span) -> Self {
        Self {
            code: "E0402",
            message: message.into(),
            span,
            help: None,
        }
    }
}

/// Lowers a checked program into typed IR, or reports the first construct the
/// native backend cannot represent.
pub fn build(program: &Program, model: &SemanticModel) -> Result<IrProgram, IrError> {
    Builder::new(program, model).build()
}

struct Builder<'a> {
    program: &'a Program,
    model: &'a SemanticModel,
    structs: Vec<IrStruct>,
    struct_map: HashMap<String, IrStruct>,
    enums: Vec<IrEnum>,
    enum_map: HashMap<String, IrEnum>,
    enum_names: HashSet<String>,
    globals: Vec<IrGlobal>,
    global_names: HashSet<String>,
    global_types: HashMap<String, IrType>,
    function_returns: HashMap<String, IrType>,
    local_scopes: Vec<HashMap<String, IrType>>,
    current_return: IrType,
    temp_counter: usize,
}

impl<'a> Builder<'a> {
    fn new(program: &'a Program, model: &'a SemanticModel) -> Self {
        Self {
            program,
            model,
            structs: Vec::new(),
            struct_map: HashMap::new(),
            enums: Vec::new(),
            enum_map: HashMap::new(),
            enum_names: HashSet::new(),
            globals: Vec::new(),
            global_names: HashSet::new(),
            global_types: HashMap::new(),
            function_returns: HashMap::new(),
            local_scopes: Vec::new(),
            current_return: IrType::Unit,
            temp_counter: 0,
        }
    }

    fn build(mut self) -> Result<IrProgram, IrError> {
        self.collect_enum_names()?;
        self.collect_structs()?;
        self.collect_enums()?;
        self.collect_globals()?;
        self.collect_function_returns()?;
        let functions = self.build_functions()?;
        let top_level = self.build_top_level()?;
        let entry = match self.model.entry {
            Entry::TopLevel => IrEntry::TopLevel,
            Entry::Main => IrEntry::Main,
            Entry::None => {
                return Err(IrError::internal(
                    "program has no entry point after checking",
                    self.program.span,
                ));
            }
        };
        Ok(IrProgram {
            structs: self.structs,
            enums: self.enums,
            globals: self.globals,
            functions,
            top_level,
            entry,
            span: self.program.span,
        })
    }

    // ---- collection ----

    /// Registers enum names before any field type is resolved so struct and
    /// enum payloads can refer to enums declared later (including recursive
    /// and mutually recursive types).
    fn collect_enum_names(&mut self) -> Result<(), IrError> {
        for item in &self.program.items {
            let Item::Enum(decl) = item else {
                continue;
            };
            if !self.model.enums.contains_key(&decl.name) {
                return Err(IrError::internal(
                    format!("missing enum info for `{}`", decl.name),
                    decl.span,
                ));
            }
            self.enum_names.insert(decl.name.clone());
        }
        Ok(())
    }

    fn collect_enums(&mut self) -> Result<(), IrError> {
        for item in &self.program.items {
            let Item::Enum(decl) = item else {
                continue;
            };
            let Some(info) = self.model.enums.get(&decl.name) else {
                return Err(IrError::internal(
                    format!("missing enum info for `{}`", decl.name),
                    decl.span,
                ));
            };
            let mut variants = Vec::with_capacity(info.variants.len());
            for (tag, variant) in info.variants.iter().enumerate() {
                let mut fields = Vec::with_capacity(variant.fields.len());
                for field in &variant.fields {
                    let ty = self.native_type(&field.ty, field.span, "variant payload")?;
                    if matches!(ty, IrType::Unit) {
                        return Err(IrError::unsupported(
                            format!(
                                "payload field `{}.{}.{}` has type Unit",
                                decl.name, variant.name, field.name
                            ),
                            field.span,
                            "native enum payloads require fields with a concrete value type",
                        ));
                    }
                    fields.push(IrField {
                        name: field.name.clone(),
                        ty,
                        span: field.span,
                    });
                }
                variants.push(IrVariant {
                    name: variant.name.clone(),
                    tag,
                    fields,
                    span: variant.span,
                });
            }
            let ir_enum = IrEnum {
                name: decl.name.clone(),
                variants,
                span: decl.span,
            };
            self.enum_map.insert(decl.name.clone(), ir_enum.clone());
            self.enums.push(ir_enum);
        }
        Ok(())
    }

    fn collect_structs(&mut self) -> Result<(), IrError> {
        // First pass: register every struct name so recursive field types
        // (for example `struct Node: children: List[Node]`) resolve.
        for item in &self.program.items {
            let Item::Struct(decl) = item else {
                continue;
            };
            self.struct_map.insert(
                decl.name.clone(),
                IrStruct {
                    name: decl.name.clone(),
                    fields: Vec::new(),
                    span: decl.span,
                },
            );
        }
        for item in &self.program.items {
            let Item::Struct(decl) = item else {
                continue;
            };
            let Some(info) = self.model.structs.get(&decl.name) else {
                return Err(IrError::internal(
                    format!("missing struct info for `{}`", decl.name),
                    decl.span,
                ));
            };
            let mut fields = Vec::with_capacity(info.fields.len());
            for field in &info.fields {
                let ty = self.native_type(&field.ty, field.span, "struct field")?;
                if matches!(ty, IrType::Unit) {
                    return Err(IrError::unsupported(
                        format!("field `{}.{}` has type Unit", decl.name, field.name),
                        field.span,
                        "native structs require fields with a concrete value type",
                    ));
                }
                fields.push(IrField {
                    name: field.name.clone(),
                    ty,
                    span: field.span,
                });
            }
            let ir_struct = IrStruct {
                name: decl.name.clone(),
                fields,
                span: decl.span,
            };
            self.struct_map
                .insert(decl.name.clone(), ir_struct.clone());
            self.structs.push(ir_struct);
        }
        Ok(())
    }

    fn collect_globals(&mut self) -> Result<(), IrError> {
        for item in &self.program.items {
            let Item::Stmt(stmt) = item else {
                continue;
            };
            let StmtKind::Let {
                name, ty, init, ..
            } = &stmt.kind
            else {
                continue;
            };
            let declared = match ty {
                Some(expr) => Some(self.resolve_type(expr)?),
                None => None,
            };
            let ty = match declared {
                Some(ty) => ty,
                None => self.expr_type(init)?,
            };
            let ty = self.native_type(&ty, stmt.span, "global binding")?;
            if matches!(ty, IrType::Unit) {
                return Err(IrError::unsupported(
                    format!("global `{name}` has type Unit"),
                    stmt.span,
                    "native globals require a concrete value type",
                ));
            }
            self.global_names.insert(name.clone());
            self.global_types.insert(name.clone(), ty.clone());
            self.globals.push(IrGlobal {
                name: name.clone(),
                ty,
                span: stmt.span,
            });
        }
        Ok(())
    }

    // ---- functions ----

    /// Computes return types before bodies are lowered so forward calls use
    /// the same type the function will actually be emitted with.
    fn collect_function_returns(&mut self) -> Result<(), IrError> {
        for item in &self.program.items {
            let Item::Fn(decl) = item else {
                continue;
            };
            let Some(sig) = self.model.functions.get(&decl.name) else {
                return Err(IrError::internal(
                    format!("missing signature for `{}`", decl.name),
                    decl.span,
                ));
            };
            let ret = if sig.ret_annotated {
                self.native_type(&sig.ret, decl.span, "return type")?
            } else if block_has_value_return(&decl.body) {
                return Err(IrError::unsupported(
                    format!(
                        "function `{}` returns a value but has no return type annotation",
                        decl.name
                    ),
                    decl.span,
                    "add `-> Int`, `-> Str`, or another concrete return type for native builds",
                ));
            } else {
                IrType::Unit
            };
            self.function_returns.insert(decl.name.clone(), ret);
        }
        Ok(())
    }

    fn build_functions(&mut self) -> Result<Vec<IrFunction>, IrError> {
        let mut functions = Vec::new();
        for item in &self.program.items {
            let Item::Fn(decl) = item else {
                continue;
            };
            let Some(sig) = self.model.functions.get(&decl.name) else {
                return Err(IrError::internal(
                    format!("missing signature for `{}`", decl.name),
                    decl.span,
                ));
            };
            let mut params = Vec::with_capacity(sig.params.len());
            for param in &sig.params {
                let ty = self.native_type(&param.ty, param.span, "parameter")?;
                params.push(IrParam {
                    name: param.name.clone(),
                    ty,
                    span: param.span,
                });
            }
            let ret = self
                .function_returns
                .get(&decl.name)
                .cloned()
                .ok_or_else(|| {
                    IrError::internal(
                        format!("missing native return type for `{}`", decl.name),
                        decl.span,
                    )
                })?;

            self.current_return = ret.clone();
            self.local_scopes.push(HashMap::new());
            for param in &params {
                self.declare_local(&param.name, param.ty.clone());
            }
            let body = self.lower_block_contents(&decl.body)?;
            self.local_scopes.pop();

            functions.push(IrFunction {
                name: decl.name.clone(),
                params,
                ret,
                body,
                span: decl.span,
            });
        }
        Ok(functions)
    }

    fn build_top_level(&mut self) -> Result<Vec<IrStmt>, IrError> {
        let mut body = Vec::new();
        for item in &self.program.items {
            let Item::Stmt(stmt) = item else {
                continue;
            };
            body.extend(self.lower_stmt(stmt, true)?);
        }
        Ok(body)
    }

    // ---- scopes ----

    fn declare_local(&mut self, name: &str, ty: IrType) {
        if let Some(scope) = self.local_scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn is_local(&self, name: &str) -> bool {
        self.local_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn local_type(&self, name: &str) -> Option<IrType> {
        self.local_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn fresh_temp(&mut self) -> String {
        let name = format!("__auren_tmp{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    // ---- types ----

    fn expr_type(&self, expr: &Expr) -> Result<Type, IrError> {
        self.model
            .expr_types
            .get(&expr.id)
            .cloned()
            .ok_or_else(|| {
                IrError::internal(
                    format!("missing inferred type for expression {}", expr.id),
                    expr.span,
                )
            })
    }

    fn resolve_type(&self, expr: &TypeExpr) -> Result<Type, IrError> {
        resolve_type_expr(expr, &self.model.structs, &self.model.enums)
            .map_err(|message| IrError::internal(format!("unresolved type: {message}"), expr.span))
    }

    fn native_type(&self, ty: &Type, span: Span, context: &str) -> Result<IrType, IrError> {
        match ty {
            Type::Unit => Ok(IrType::Unit),
            Type::Int => Ok(IrType::Int),
            Type::Float => Ok(IrType::Float),
            Type::Bool => Ok(IrType::Bool),
            Type::Str => Ok(IrType::Str),
            Type::Struct(name) => {
                if !self.struct_map.contains_key(name) {
                    return Err(IrError::internal(
                        format!("unknown struct `{name}` in {context}"),
                        span,
                    ));
                }
                Ok(IrType::Struct(name.clone()))
            }
            Type::Any => Err(IrError::unsupported(
                format!("{context} has type Any, which the native backend cannot represent"),
                span,
                "add a concrete type annotation; the interpreter supports Any",
            )),
            Type::Enum(name) => {
                if !self.enum_names.contains(name) {
                    return Err(IrError::internal(
                        format!("unknown enum `{name}` in {context}"),
                        span,
                    ));
                }
                Ok(IrType::Enum(name.clone()))
            }
            Type::List(inner) => {
                let inner = self.native_type(inner, span, "list element")?;
                Ok(IrType::List(Box::new(inner)))
            }
            Type::Map(key, value) => {
                let key = self.native_type(key, span, "map key")?;
                let value = self.native_type(value, span, "map value")?;
                Ok(IrType::Map(Box::new(key), Box::new(value)))
            }
        }
    }

    // ---- statements ----

    fn lower_block_contents(&mut self, block: &Block) -> Result<Vec<IrStmt>, IrError> {
        let mut body = Vec::new();
        for stmt in &block.stmts {
            body.extend(self.lower_stmt(stmt, false)?);
        }
        Ok(body)
    }

    fn lower_scoped_block(&mut self, block: &Block) -> Result<Vec<IrStmt>, IrError> {
        self.local_scopes.push(HashMap::new());
        let result = self.lower_block_contents(block);
        self.local_scopes.pop();
        result
    }

    fn lower_stmt(&mut self, stmt: &Stmt, top_level: bool) -> Result<Vec<IrStmt>, IrError> {
        match &stmt.kind {
            StmtKind::Let {
                name, ty, init, ..
            } => {
                let init = self.lower_expr(init)?;
                let declared = match ty {
                    Some(expr) => Some(self.resolve_type(expr)?),
                    None => None,
                };
                let ty = match declared {
                    Some(ty) => {
                        let expected = self.native_type(&ty, stmt.span, "binding")?;
                        self.expect_assignable(&init.ty, &expected, init.span)?;
                        expected
                    }
                    None => init.ty.clone(),
                };
                if top_level && self.local_scopes.is_empty() {
                    // Top-level bindings are file-scope globals.
                    self.declare_global_use(name);
                    return Ok(vec![IrStmt {
                        kind: IrStmtKind::Assign {
                            target: IrPlace::Global(name.clone()),
                            value: init,
                        },
                        span: stmt.span,
                    }]);
                }
                self.declare_local(name, ty.clone());
                Ok(vec![IrStmt {
                    kind: IrStmtKind::Let {
                        name: name.clone(),
                        ty,
                        init,
                    },
                    span: stmt.span,
                }])
            }
            StmtKind::Assign { target, op, value } => {
                self.lower_assignment(stmt, target, *op, value)
            }
            StmtKind::Expr(expr) => {
                let expr = self.lower_expr(expr)?;
                Ok(vec![IrStmt {
                    kind: IrStmtKind::Expr(expr),
                    span: stmt.span,
                }])
            }
            StmtKind::If {
                cond,
                then_block,
                elifs,
                else_block,
            } => {
                let cond = self.lower_expr(cond)?;
                let then_body = self.lower_scoped_block(then_block)?;
                let mut ir_elifs = Vec::with_capacity(elifs.len());
                for (elif_cond, block) in elifs {
                    let elif_cond = self.lower_expr(elif_cond)?;
                    ir_elifs.push((elif_cond, self.lower_scoped_block(block)?));
                }
                let else_body = match else_block {
                    Some(block) => Some(self.lower_scoped_block(block)?),
                    None => None,
                };
                Ok(vec![IrStmt {
                    kind: IrStmtKind::If {
                        cond,
                        then_body,
                        elifs: ir_elifs,
                        else_body,
                    },
                    span: stmt.span,
                }])
            }
            StmtKind::While { cond, body } => {
                let cond = self.lower_expr(cond)?;
                let body = self.lower_scoped_block(body)?;
                Ok(vec![IrStmt {
                    kind: IrStmtKind::While { cond, body },
                    span: stmt.span,
                }])
            }
            StmtKind::For {
                name,
                iter,
                body,
                ..
            } => self.lower_for(stmt.span, name, iter, body),
            StmtKind::Return(value) => {
                let value = match value {
                    Some(expr) => Some(self.lower_expr(expr)?),
                    None => None,
                };
                match (&self.current_return, &value) {
                    (IrType::Unit, None) => {}
                    (IrType::Unit, Some(_)) => {
                        return Err(IrError::unsupported(
                            "returning a value from a Unit function",
                            stmt.span,
                            "use a bare `return` in a Unit function",
                        ));
                    }
                    (_, None) => {
                        return Err(IrError::internal(
                            "missing return value in a function with a non-Unit return type",
                            stmt.span,
                        ));
                    }
                    (_, Some(value)) => {
                        if value.ty != self.current_return {
                            return Err(IrError::internal(
                                format!(
                                    "return type mismatch: expected `{}`, found `{}`",
                                    self.current_return.name(),
                                    value.ty.name()
                                ),
                                stmt.span,
                            ));
                        }
                    }
                }
                Ok(vec![IrStmt {
                    kind: IrStmtKind::Return(value),
                    span: stmt.span,
                }])
            }
            StmtKind::Break => Ok(vec![IrStmt {
                kind: IrStmtKind::Break,
                span: stmt.span,
            }]),
            StmtKind::Continue => Ok(vec![IrStmt {
                kind: IrStmtKind::Continue,
                span: stmt.span,
            }]),
            StmtKind::Match { scrutinee, arms } => {
                let (scrutinee, arms) = self.lower_match(scrutinee, arms, false)?;
                Ok(vec![IrStmt {
                    kind: IrStmtKind::Match { scrutinee, arms },
                    span: stmt.span,
                }])
            }
        }
    }

    fn declare_global_use(&mut self, _name: &str) {
        // Globals were collected in the first pass. This hook documents that
        // top-level bindings are resolved through `global_names`.
    }

    fn lower_assignment(
        &mut self,
        stmt: &Stmt,
        target: &Expr,
        op: Option<BinOp>,
        value: &Expr,
    ) -> Result<Vec<IrStmt>, IrError> {
        match &target.kind {
            ExprKind::Var(name) => {
                let (place, target_ty) = self.lower_var_place(name, target.span)?;
                let rhs = self.lower_expr(value)?;
                let result = match op {
                    None => rhs,
                    Some(op) => {
                        let current = self.read_place_expr(&place, target_ty.clone(), target.span);
                        self.lower_binary(op, current, rhs, stmt.span)?
                    }
                };
                self.expect_assignable(&result.ty, &target_ty, stmt.span)?;
                Ok(vec![IrStmt {
                    kind: IrStmtKind::Assign {
                        target: place,
                        value: result,
                    },
                    span: stmt.span,
                }])
            }
            ExprKind::Field { base, name, .. } => {
                let base_expr = self.lower_expr(base)?;
                let field_ty = self.field_type(&base_expr.ty, name, target.span)?;
                let rhs = self.lower_expr(value)?;
                match op {
                    None => Ok(vec![IrStmt {
                        kind: IrStmtKind::Assign {
                            target: IrPlace::Field {
                                base: base_expr,
                                field: name.clone(),
                            },
                            value: rhs,
                        },
                        span: stmt.span,
                    }]),
                    Some(op) => {
                        let temp = self.fresh_temp();
                        let temp_ty = base_expr.ty.clone();
                        let read = IrExpr {
                            kind: IrExprKind::Field {
                                base: Box::new(IrExpr {
                                    kind: IrExprKind::Var(temp.clone()),
                                    ty: temp_ty.clone(),
                                    span: base.span,
                                }),
                                name: name.clone(),
                            },
                            ty: field_ty.clone(),
                            span: target.span,
                        };
                        let result = self.lower_binary(op, read, rhs, stmt.span)?;
                        self.expect_assignable(&result.ty, &field_ty, stmt.span)?;
                        Ok(vec![
                            IrStmt {
                                kind: IrStmtKind::Let {
                                    name: temp.clone(),
                                    ty: temp_ty.clone(),
                                    init: base_expr,
                                },
                                span: base.span,
                            },
                            IrStmt {
                                kind: IrStmtKind::Assign {
                                    target: IrPlace::Field {
                                        base: IrExpr {
                                            kind: IrExprKind::Var(temp),
                                            ty: temp_ty,
                                            span: base.span,
                                        },
                                        field: name.clone(),
                                    },
                                    value: result,
                                },
                                span: stmt.span,
                            },
                        ])
                    }
                }
            }
            ExprKind::Index { base, index } => {
                let base_expr = self.lower_expr(base)?;
                let index_expr = self.lower_expr(index)?;
                match base_expr.ty.clone() {
                    IrType::List(inner) => {
                        if index_expr.ty != IrType::Int {
                            return Err(IrError::internal(
                                "list index expression is not an Int",
                                index.span,
                            ));
                        }
                        self.lower_index_assignment(
                            stmt.span,
                            base.span,
                            index.span,
                            base_expr,
                            index_expr,
                            IrType::Int,
                            *inner,
                            op,
                            value,
                        )
                    }
                    IrType::Map(key, value_ty) => {
                        self.expect_assignable(&index_expr.ty, &key, index.span)?;
                        self.lower_index_assignment(
                            stmt.span,
                            base.span,
                            index.span,
                            base_expr,
                            index_expr,
                            *key,
                            *value_ty,
                            op,
                            value,
                        )
                    }
                    other => Err(IrError::unsupported(
                        format!(
                            "native index assignment requires a List or Map, found `{}`",
                            other.name()
                        ),
                        target.span,
                        "string index assignment is not supported; build a new string instead",
                    )),
                }
            }
            _ => Err(IrError::unsupported(
                "invalid assignment target",
                target.span,
                "the native backend assigns to variables, struct fields, and list elements",
            )),
        }
    }

    /// Lowers `base[index] = value` and `base[index] op= value` for both
    /// lists (integer index) and maps (typed key). Compound assignment
    /// evaluates the place once, reads the old element, then the right side.
    #[allow(clippy::too_many_arguments)]
    fn lower_index_assignment(
        &mut self,
        stmt_span: Span,
        base_span: Span,
        index_span: Span,
        base_expr: IrExpr,
        index_expr: IrExpr,
        index_ty: IrType,
        element_ty: IrType,
        op: Option<BinOp>,
        value: &Expr,
    ) -> Result<Vec<IrStmt>, IrError> {
        let rhs = self.lower_expr(value)?;
        match op {
            None => {
                self.expect_assignable(&rhs.ty, &element_ty, value.span)?;
                Ok(vec![IrStmt {
                    kind: IrStmtKind::Assign {
                        target: IrPlace::Index {
                            base: base_expr,
                            index: index_expr,
                        },
                        value: rhs,
                    },
                    span: stmt_span,
                }])
            }
            Some(op) => {
                let base_temp = self.fresh_temp();
                let index_temp = self.fresh_temp();
                let base_ty = base_expr.ty.clone();
                let read = IrExpr {
                    kind: IrExprKind::Index {
                        base: Box::new(IrExpr {
                            kind: IrExprKind::Var(base_temp.clone()),
                            ty: base_ty.clone(),
                            span: base_span,
                        }),
                        index: Box::new(IrExpr {
                            kind: IrExprKind::Var(index_temp.clone()),
                            ty: index_ty.clone(),
                            span: index_span,
                        }),
                    },
                    ty: element_ty.clone(),
                    span: stmt_span,
                };
                let result = self.lower_binary(op, read, rhs, stmt_span)?;
                self.expect_assignable(&result.ty, &element_ty, stmt_span)?;
                Ok(vec![
                    IrStmt {
                        kind: IrStmtKind::Let {
                            name: base_temp.clone(),
                            ty: base_ty.clone(),
                            init: base_expr,
                        },
                        span: base_span,
                    },
                    IrStmt {
                        kind: IrStmtKind::Let {
                            name: index_temp.clone(),
                            ty: index_ty.clone(),
                            init: index_expr,
                        },
                        span: index_span,
                    },
                    IrStmt {
                        kind: IrStmtKind::Assign {
                            target: IrPlace::Index {
                                base: IrExpr {
                                    kind: IrExprKind::Var(base_temp),
                                    ty: base_ty,
                                    span: base_span,
                                },
                                index: IrExpr {
                                    kind: IrExprKind::Var(index_temp),
                                    ty: index_ty,
                                    span: index_span,
                                },
                            },
                            value: result,
                        },
                        span: stmt_span,
                    },
                ])
            }
        }
    }

    fn lower_var_place(
        &self,
        name: &str,
        span: Span,
    ) -> Result<(IrPlace, IrType), IrError> {
        let ty = self.lookup_var_type(name, span)?;
        if self.is_local(name) {
            Ok((IrPlace::Var(name.to_string()), ty))
        } else if self.global_names.contains(name) {
            Ok((IrPlace::Global(name.to_string()), ty))
        } else {
            Err(IrError::internal(
                format!("variable `{name}` is not declared"),
                span,
            ))
        }
    }

    fn read_place_expr(&self, place: &IrPlace, ty: IrType, span: Span) -> IrExpr {
        match place {
            IrPlace::Var(name) => IrExpr {
                kind: IrExprKind::Var(name.clone()),
                ty,
                span,
            },
            IrPlace::Global(name) => IrExpr {
                kind: IrExprKind::Global(name.clone()),
                ty,
                span,
            },
            IrPlace::Field { base, field } => IrExpr {
                kind: IrExprKind::Field {
                    base: Box::new(base.clone()),
                    name: field.clone(),
                },
                ty,
                span,
            },
            IrPlace::Index { base, index } => IrExpr {
                kind: IrExprKind::Index {
                    base: Box::new(base.clone()),
                    index: Box::new(index.clone()),
                },
                ty,
                span,
            },
        }
    }

    fn lookup_var_type(&self, name: &str, span: Span) -> Result<IrType, IrError> {
        if let Some(ty) = self.local_type(name) {
            return Ok(ty);
        }
        if let Some(ty) = self.global_types.get(name) {
            return Ok(ty.clone());
        }
        Err(IrError::internal(
            format!("variable `{name}` is not declared"),
            span,
        ))
    }

    fn expect_assignable(&self, found: &IrType, expected: &IrType, span: Span) -> Result<(), IrError> {
        if found == expected {
            Ok(())
        } else {
            Err(IrError::internal(
                format!(
                    "assignment type mismatch: expected `{}`, found `{}`",
                    expected.name(),
                    found.name()
                ),
                span,
            ))
        }
    }

    fn field_type(&self, owner: &IrType, field: &str, span: Span) -> Result<IrType, IrError> {
        let IrType::Struct(name) = owner else {
            return Err(IrError::internal(
                format!("type `{}` has no fields", owner.name()),
                span,
            ));
        };
        let Some(info) = self.struct_map.get(name) else {
            return Err(IrError::internal(
                format!("unknown struct `{name}`"),
                span,
            ));
        };
        info.fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|candidate| candidate.ty.clone())
            .ok_or_else(|| {
                IrError::internal(
                    format!("struct `{name}` has no field `{field}`"),
                    span,
                )
            })
    }

    // ---- match ----

    /// Lowers a match statement or expression. Pattern bindings are scoped to
    /// the arm body, and the scrutinee type must be a closed enum; dynamic
    /// `Any` matches remain interpreter-only.
    fn lower_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        expression: bool,
    ) -> Result<(IrExpr, Vec<IrMatchArm>), IrError> {
        let scrutinee = self.lower_expr(scrutinee)?;
        let enum_name = match &scrutinee.ty {
            IrType::Enum(name) => Some(name.clone()),
            IrType::Int | IrType::Str | IrType::Bool => None,
            other => {
                return Err(IrError::unsupported(
                    format!(
                        "native `match` requires an enum or scalar value, found `{}`",
                        other.name()
                    ),
                    scrutinee.span,
                    "matching on `Any` is interpreter-only; annotate the value with a concrete type",
                ));
            }
        };
        let info = match &enum_name {
            Some(name) => {
                let Some(info) = self.enum_map.get(name).cloned() else {
                    return Err(IrError::internal(
                        format!("unknown enum `{name}` in match"),
                        scrutinee.span,
                    ));
                };
                Some(info)
            }
            None => None,
        };

        let mut lowered = Vec::with_capacity(arms.len());
        for arm in arms {
            self.local_scopes.push(HashMap::new());
            let pattern = match &arm.pattern {
                Pattern::Wildcard { .. } => IrPattern::Wildcard,
                Pattern::Literal { value, span } => {
                    let value = match (value, &scrutinee.ty) {
                        (LiteralPattern::Int(value), IrType::Int) => IrLiteral::Int(*value),
                        (LiteralPattern::Bool(value), IrType::Bool) => IrLiteral::Bool(*value),
                        (LiteralPattern::Str(value), IrType::Str) => {
                            IrLiteral::Str(value.clone())
                        }
                        (value, other) => {
                            return Err(IrError::internal(
                                format!(
                                    "literal pattern type `{}` does not match native scrutinee `{}`",
                                    value.type_name(),
                                    other.name()
                                ),
                                *span,
                            ));
                        }
                    };
                    IrPattern::Literal { value }
                }
                Pattern::Variant { name, fields, .. } => {
                    let (Some(info), Some(enum_name)) = (&info, &enum_name) else {
                        return Err(IrError::internal(
                            format!("variant pattern `{name}` in a non-enum match"),
                            arm.pattern.span(),
                        ));
                    };
                    let Some(variant) = info.variants.iter().find(|variant| variant.name == *name)
                    else {
                        return Err(IrError::internal(
                            format!("unknown variant `{name}` in match on `{enum_name}`"),
                            arm.pattern.span(),
                        ));
                    };
                    if fields.len() != variant.fields.len() {
                        return Err(IrError::internal(
                            format!("variant `{name}` payload arity was not validated"),
                            arm.pattern.span(),
                        ));
                    }
                    let mut bindings = Vec::with_capacity(fields.len());
                    for (field_pattern, field) in fields.iter().zip(variant.fields.iter()) {
                        match field_pattern {
                            PatternArg::Wildcard { .. } => bindings.push(None),
                            PatternArg::Bind { name, .. } => {
                                self.declare_local(name, field.ty.clone());
                                bindings.push(Some(IrBinding {
                                    name: name.clone(),
                                    field: field.name.clone(),
                                    ty: field.ty.clone(),
                                    span: arm.pattern.span(),
                                }));
                            }
                        }
                    }
                    IrPattern::Variant {
                        enum_name: enum_name.clone(),
                        variant: variant.name.clone(),
                        tag: variant.tag,
                        bindings,
                    }
                }
            };

            let guard = match &arm.guard {
                Some(guard) => {
                    let lowered = self.lower_expr(guard)?;
                    if lowered.ty != IrType::Bool {
                        return Err(IrError::internal(
                            format!(
                                "match guard did not lower to Bool, found `{}`",
                                lowered.ty.name()
                            ),
                            guard.span,
                        ));
                    }
                    Some(lowered)
                }
                None => None,
            };

            let body = match &arm.body {
                MatchArmBody::Expr(expr) => {
                    if !expression {
                        return Err(IrError::internal(
                            "expression arm in a match statement",
                            arm.span,
                        ));
                    }
                    IrMatchBody::Expr(self.lower_expr(expr)?)
                }
                MatchArmBody::Block(block) => {
                    if expression {
                        return Err(IrError::internal(
                            "block arm in a match expression",
                            arm.span,
                        ));
                    }
                    IrMatchBody::Block(self.lower_scoped_block(block)?)
                }
            };

            self.local_scopes.pop();
            lowered.push(IrMatchArm {
                pattern,
                guard,
                body,
                span: arm.span,
            });
        }
        Ok((scrutinee, lowered))
    }

    // ---- for loops ----

    fn lower_for(
        &mut self,
        span: Span,
        name: &str,
        iter: &Expr,
        body: &Block,
    ) -> Result<Vec<IrStmt>, IrError> {
        // `range(...)` is typed as `List[Int]` by the checker, but the native
        // backend represents it directly as a counted loop rather than
        // materializing a list. Recognize the direct call before the general
        // List iterator path.
        if let ExprKind::Call { callee, args } = &iter.kind
            && let ExprKind::Var(function) = &callee.kind
            && function == "range"
        {
            return self.lower_for_range(span, name, iter, args, body);
        }

        let iter_ty = self.expr_type(iter)?;
        if iter_ty == Type::Str {
            let text = self.lower_expr(iter)?;
            if text.ty != IrType::Str {
                return Err(IrError::internal(
                    "string iterator did not lower to a Str",
                    iter.span,
                ));
            }
            self.local_scopes.push(HashMap::new());
            self.declare_local(name, IrType::Str);
            let body = self.lower_scoped_block(body)?;
            self.local_scopes.pop();
            return Ok(vec![IrStmt {
                kind: IrStmtKind::ForStr {
                    name: name.to_string(),
                    text,
                    body,
                },
                span,
            }]);
        }

        if let Type::List(_) = iter_ty {
            let list = self.lower_expr(iter)?;
            let IrType::List(inner) = &list.ty else {
                return Err(IrError::internal(
                    "list iterator did not lower to a List",
                    iter.span,
                ));
            };
            let element_ty = (**inner).clone();
            self.local_scopes.push(HashMap::new());
            self.declare_local(name, element_ty);
            let body = self.lower_scoped_block(body)?;
            self.local_scopes.pop();
            return Ok(vec![IrStmt {
                kind: IrStmtKind::ForList {
                    name: name.to_string(),
                    list,
                    body,
                },
                span,
            }]);
        }

        if let Type::Map(key, _) = iter_ty {
            let map = self.lower_expr(iter)?;
            if !matches!(map.ty, IrType::Map(_, _)) {
                return Err(IrError::internal(
                    "map iterator did not lower to a Map",
                    iter.span,
                ));
            }
            let key_ty = self.native_type(&key, iter.span, "map key")?;
            self.local_scopes.push(HashMap::new());
            self.declare_local(name, key_ty);
            let body = self.lower_scoped_block(body)?;
            self.local_scopes.pop();
            return Ok(vec![IrStmt {
                kind: IrStmtKind::ForMap {
                    name: name.to_string(),
                    map,
                    body,
                },
                span,
            }]);
        }

        Err(self.unsupported_for(iter.span))
    }

    fn lower_for_range(
        &mut self,
        span: Span,
        name: &str,
        iter: &Expr,
        args: &[Arg],
        body: &Block,
    ) -> Result<Vec<IrStmt>, IrError> {
        if args.is_empty() || args.len() > 3 {
            return Err(IrError::internal(
                "`range` arity was not validated by the checker",
                iter.span,
            ));
        }
        if args.iter().any(|arg| arg.name.is_some()) {
            return Err(IrError::internal(
                "named arguments are not valid for `range`",
                iter.span,
            ));
        }
        let mut lowered = Vec::with_capacity(args.len());
        for arg in args {
            let value = self.lower_expr(&arg.value)?;
            if value.ty != IrType::Int {
                return Err(IrError::internal(
                    "`range` argument is not an Int",
                    arg.value.span,
                ));
            }
            lowered.push(value);
        }
        let zero = IrExpr {
            kind: IrExprKind::Int(0),
            ty: IrType::Int,
            span: iter.span,
        };
        let one = IrExpr {
            kind: IrExprKind::Int(1),
            ty: IrType::Int,
            span: iter.span,
        };
        let (start, end, step) = match lowered.as_slice() {
            [end] => (zero.clone(), end.clone(), one.clone()),
            [start, end] => (start.clone(), end.clone(), one.clone()),
            [start, end, step] => (start.clone(), end.clone(), step.clone()),
            _ => unreachable!(),
        };

        self.local_scopes.push(HashMap::new());
        self.declare_local(name, IrType::Int);
        let body = self.lower_scoped_block(body)?;
        self.local_scopes.pop();

        Ok(vec![IrStmt {
            kind: IrStmtKind::ForRange {
                name: name.to_string(),
                start,
                end,
                step,
                body,
            },
            span,
        }])
    }

    fn unsupported_for(&self, span: Span) -> IrError {
        IrError::unsupported(
            "native `for` loops support `range(...)`, typed List iterators, Str, and Map keys",
            span,
            "use `for x in range(...)`, `for x in list`, `for c in text`, `for k in map`, or run this program with `auren run`",
        )
    }

    // ---- expressions ----

    fn lower_expr(&mut self, expr: &Expr) -> Result<IrExpr, IrError> {
        let ty = self.expr_type(expr)?;
        match &expr.kind {
            ExprKind::Int(value) => Ok(IrExpr {
                kind: IrExprKind::Int(*value),
                ty: IrType::Int,
                span: expr.span,
            }),
            ExprKind::Float(value) => Ok(IrExpr {
                kind: IrExprKind::Float(*value),
                ty: IrType::Float,
                span: expr.span,
            }),
            ExprKind::Bool(value) => Ok(IrExpr {
                kind: IrExprKind::Bool(*value),
                ty: IrType::Bool,
                span: expr.span,
            }),
            ExprKind::Str(parts) => {
                let mut ir_parts = Vec::with_capacity(parts.len());
                for part in parts {
                    match part {
                        StrPart::Text(text) => {
                            if text.contains('\0') {
                                return Err(IrError::unsupported(
                                    "native strings cannot contain NUL bytes",
                                    expr.span,
                                    "remove the NUL byte or run this program with `auren run`",
                                ));
                            }
                            ir_parts.push(IrStrPart::Text(text.clone()));
                        }
                        StrPart::Expr(inner) => {
                            let inner = self.lower_expr(inner)?;
                            ir_parts.push(IrStrPart::Value(Box::new(inner)));
                        }
                    }
                }
                Ok(IrExpr {
                    kind: IrExprKind::Str(ir_parts),
                    ty: IrType::Str,
                    span: expr.span,
                })
            }
            ExprKind::Var(name) => {
                if self.is_local(name) {
                    Ok(IrExpr {
                        kind: IrExprKind::Var(name.clone()),
                        ty: self.native_type(&ty, expr.span, "variable")?,
                        span: expr.span,
                    })
                } else if self.global_names.contains(name) {
                    Ok(IrExpr {
                        kind: IrExprKind::Global(name.clone()),
                        ty: self.native_type(&ty, expr.span, "global")?,
                        span: expr.span,
                    })
                } else {
                    Err(IrError::internal(
                        format!("variable `{name}` is not declared"),
                        expr.span,
                    ))
                }
            }
            ExprKind::List(elements) => {
                let Type::List(inner) = &ty else {
                    return Err(IrError::internal(
                        "list literal did not receive a List type",
                        expr.span,
                    ));
                };
                let element_ty = self.native_type(inner, expr.span, "list element")?;
                let mut lowered = Vec::with_capacity(elements.len());
                for element in elements {
                    let value = self.lower_expr(element)?;
                    self.expect_assignable(&value.ty, &element_ty, element.span)?;
                    lowered.push(value);
                }
                Ok(IrExpr {
                    kind: IrExprKind::List(lowered),
                    ty: IrType::List(Box::new(element_ty)),
                    span: expr.span,
                })
            }
            ExprKind::Map(entries) => {
                let Type::Map(key, value) = &ty else {
                    return Err(IrError::internal(
                        "map literal did not receive a Map type",
                        expr.span,
                    ));
                };
                let key_ty = self.native_type(key, expr.span, "map key")?;
                let value_ty = self.native_type(value, expr.span, "map value")?;
                let mut lowered = Vec::with_capacity(entries.len());
                for (entry_key, entry_value) in entries {
                    let lowered_key = self.lower_expr(entry_key)?;
                    let lowered_value = self.lower_expr(entry_value)?;
                    self.expect_assignable(&lowered_key.ty, &key_ty, entry_key.span)?;
                    self.expect_assignable(&lowered_value.ty, &value_ty, entry_value.span)?;
                    lowered.push((lowered_key, lowered_value));
                }
                Ok(IrExpr {
                    kind: IrExprKind::Map(lowered),
                    ty: IrType::Map(Box::new(key_ty), Box::new(value_ty)),
                    span: expr.span,
                })
            }
            ExprKind::Unary { op, expr: operand } => {
                let operand = self.lower_expr(operand)?;
                let result_ty = match op {
                    UnOp::Neg => match operand.ty {
                        IrType::Int => IrType::Int,
                        IrType::Float => IrType::Float,
                        _ => {
                            return Err(IrError::unsupported(
                                format!("cannot negate `{}` natively", operand.ty.name()),
                                expr.span,
                                "negation is defined for Int and Float",
                            ));
                        }
                    },
                    UnOp::Not => {
                        if operand.ty != IrType::Bool {
                            return Err(IrError::internal(
                                "`not` operand is not Bool",
                                expr.span,
                            ));
                        }
                        IrType::Bool
                    }
                };
                Ok(IrExpr {
                    kind: IrExprKind::Unary {
                        op: *op,
                        operand: Box::new(operand),
                    },
                    ty: result_ty,
                    span: expr.span,
                })
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.lower_expr(lhs)?;
                let rhs = self.lower_expr(rhs)?;
                self.lower_binary(*op, lhs, rhs, expr.span)
            }
            ExprKind::Call { callee, args } => self.lower_call(expr, callee, args),
            ExprKind::Index { base, index } => {
                let base = self.lower_expr(base)?;
                let index = self.lower_expr(index)?;
                match base.ty.clone() {
                    IrType::List(inner) => {
                        if index.ty != IrType::Int {
                            return Err(IrError::internal(
                                "list index expression is not an Int",
                                expr.span,
                            ));
                        }
                        Ok(IrExpr {
                            kind: IrExprKind::Index {
                                base: Box::new(base),
                                index: Box::new(index),
                            },
                            ty: *inner,
                            span: expr.span,
                        })
                    }
                    IrType::Str => {
                        if index.ty != IrType::Int {
                            return Err(IrError::internal(
                                "string index expression is not an Int",
                                expr.span,
                            ));
                        }
                        Ok(IrExpr {
                            kind: IrExprKind::StrIndex {
                                base: Box::new(base),
                                index: Box::new(index),
                            },
                            ty: IrType::Str,
                            span: expr.span,
                        })
                    }
                    IrType::Map(key, value) => {
                        self.expect_assignable(&index.ty, &key, index.span)?;
                        Ok(IrExpr {
                            kind: IrExprKind::Index {
                                base: Box::new(base),
                                index: Box::new(index),
                            },
                            ty: *value,
                            span: expr.span,
                        })
                    }
                    other => Err(IrError::unsupported(
                        format!(
                            "native backend cannot index a value of type `{}`",
                            other.name()
                        ),
                        expr.span,
                        "native indexing supports List, Str, and Map values",
                    )),
                }
            }
            ExprKind::Field { base, name, .. } => {
                let base = self.lower_expr(base)?;
                let field_ty = self.field_type(&base.ty, name, expr.span)?;
                Ok(IrExpr {
                    kind: IrExprKind::Field {
                        base: Box::new(base),
                        name: name.clone(),
                    },
                    ty: field_ty,
                    span: expr.span,
                })
            }
            ExprKind::Match { scrutinee, arms } => {
                let result_ty = self.native_type(&ty, expr.span, "match expression")?;
                let (scrutinee, arms) = self.lower_match(scrutinee, arms, true)?;
                Ok(IrExpr {
                    kind: IrExprKind::Match {
                        scrutinee: Box::new(scrutinee),
                        arms,
                    },
                    ty: result_ty,
                    span: expr.span,
                })
            }
        }
    }

    fn lower_binary(
        &self,
        op: BinOp,
        lhs: IrExpr,
        rhs: IrExpr,
        span: Span,
    ) -> Result<IrExpr, IrError> {
        let ty = match op {
            BinOp::Add => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int) => IrType::Int,
                (IrType::Float, IrType::Float) => IrType::Float,
                (IrType::Str, IrType::Str) => IrType::Str,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::Sub | BinOp::Mul => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int) => IrType::Int,
                (IrType::Float, IrType::Float) => IrType::Float,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::Div => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int) => IrType::Float,
                (IrType::Float, IrType::Float) => IrType::Float,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::FloorDiv => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int) => IrType::Int,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::Mod => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int) => IrType::Int,
                (IrType::Float, IrType::Float) => IrType::Float,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::Pow => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int) => IrType::Int,
                (IrType::Float, IrType::Float) => IrType::Float,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::Eq | BinOp::NotEq => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int)
                | (IrType::Float, IrType::Float)
                | (IrType::Bool, IrType::Bool)
                | (IrType::Str, IrType::Str)
                | (IrType::Unit, IrType::Unit) => IrType::Bool,
                (IrType::List(a), IrType::List(b)) if a == b => IrType::Bool,
                (IrType::Map(ak, av), IrType::Map(bk, bv)) if ak == bk && av == bv => {
                    IrType::Bool
                }
                (IrType::Struct(a), IrType::Struct(b)) if a == b => IrType::Bool,
                (IrType::Enum(a), IrType::Enum(b)) if a == b => IrType::Bool,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => match (&lhs.ty, &rhs.ty) {
                (IrType::Int, IrType::Int)
                | (IrType::Float, IrType::Float)
                | (IrType::Str, IrType::Str) => IrType::Bool,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
            BinOp::And | BinOp::Or => match (&lhs.ty, &rhs.ty) {
                (IrType::Bool, IrType::Bool) => IrType::Bool,
                _ => return Err(self.binary_unsupported(op, &lhs.ty, &rhs.ty, span)),
            },
        };
        Ok(IrExpr {
            kind: IrExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
            span,
        })
    }

    fn binary_unsupported(
        &self,
        op: BinOp,
        lhs: &IrType,
        rhs: &IrType,
        span: Span,
    ) -> IrError {
        IrError::unsupported(
            format!(
                "native backend does not support `{}` on `{}` and `{}`",
                op.symbol(),
                lhs.name(),
                rhs.name()
            ),
            span,
            "run this program with `auren run`",
        )
    }

    fn lower_call(
        &mut self,
        call: &Expr,
        callee: &Expr,
        args: &[Arg],
    ) -> Result<IrExpr, IrError> {
        let ExprKind::Var(name) = &callee.kind else {
            return Err(IrError::unsupported(
                "only named functions and struct constructors can be called natively",
                call.span,
                "assign the function to a name or use a direct call",
            ));
        };

        if is_builtin(name) {
            return self.lower_builtin(name, args, call.span);
        }

        if let Some((enum_name, variant)) = self
            .model
            .find_variant(name)
            .map(|(enum_name, variant)| (enum_name.to_string(), variant.clone()))
        {
            return self.lower_variant_constructor(&enum_name, &variant, args, call.span);
        }

        if let Some(sig) = self.model.functions.get(name) {
            if args.iter().any(|arg| arg.name.is_some()) {
                return Err(IrError::unsupported(
                    "native function calls do not support named arguments",
                    call.span,
                    "pass arguments positionally",
                ));
            }
            let mut lowered = Vec::with_capacity(args.len());
            for arg in args {
                lowered.push(self.lower_expr(&arg.value)?);
            }
            for (value, param) in lowered.iter().zip(sig.params.iter()) {
                let expected = self.native_type(&param.ty, param.span, "parameter")?;
                self.expect_assignable(&value.ty, &expected, value.span)?;
            }
            let ty = self
                .function_returns
                .get(name)
                .cloned()
                .ok_or_else(|| {
                    IrError::internal(
                        format!("missing native return type for `{name}`"),
                        call.span,
                    )
                })?;
            return Ok(IrExpr {
                kind: IrExprKind::Call {
                    name: name.clone(),
                    args: lowered,
                },
                ty,
                span: call.span,
            });
        }

        if let Some(info) = self.model.structs.get(name).cloned() {
            return self.lower_constructor(&info, args, call.span);
        }

        Err(IrError::internal(
            format!("unknown function or struct `{name}`"),
            call.span,
        ))
    }

    fn lower_constructor(
        &mut self,
        info: &crate::types::StructInfo,
        args: &[Arg],
        span: Span,
    ) -> Result<IrExpr, IrError> {
        let any_named = args.iter().any(|arg| arg.name.is_some());
        let all_named = args.iter().all(|arg| arg.name.is_some());
        if any_named && !all_named {
            return Err(IrError::unsupported(
                "cannot mix named and positional constructor arguments natively",
                span,
                "use all named or all positional arguments",
            ));
        }
        let mut lowered = Vec::with_capacity(info.fields.len());
        if all_named && !args.is_empty() {
            let mut provided: HashMap<String, IrExpr> = HashMap::new();
            for arg in args {
                let Some((field_name, field_span)) = &arg.name else {
                    continue;
                };
                let value = self.lower_expr(&arg.value)?;
                if provided.insert(field_name.clone(), value).is_some() {
                    return Err(IrError::internal(
                        format!("duplicate constructor argument `{field_name}`"),
                        *field_span,
                    ));
                }
            }
            for field in &info.fields {
                let Some(value) = provided.remove(&field.name) else {
                    return Err(IrError::internal(
                        format!("missing constructor field `{}`", field.name),
                        span,
                    ));
                };
                let expected = self.native_type(&field.ty, field.span, "struct field")?;
                self.expect_assignable(&value.ty, &expected, value.span)?;
                lowered.push(value);
            }
            if let Some(extra) = provided.keys().next() {
                return Err(IrError::internal(
                    format!("unknown constructor field `{extra}`"),
                    span,
                ));
            }
        } else {
            if args.len() != info.fields.len() {
                return Err(IrError::internal(
                    format!(
                        "struct `{}` expects {} field(s), found {}",
                        info.name,
                        info.fields.len(),
                        args.len()
                    ),
                    span,
                ));
            }
            for (arg, field) in args.iter().zip(info.fields.iter()) {
                let value = self.lower_expr(&arg.value)?;
                let expected = self.native_type(&field.ty, field.span, "struct field")?;
                self.expect_assignable(&value.ty, &expected, value.span)?;
                lowered.push(value);
            }
        }
        Ok(IrExpr {
            kind: IrExprKind::Construct {
                name: info.name.clone(),
                args: lowered,
            },
            ty: IrType::Struct(info.name.clone()),
            span,
        })
    }

    fn lower_variant_constructor(
        &mut self,
        enum_name: &str,
        variant: &crate::types::VariantInfo,
        args: &[Arg],
        span: Span,
    ) -> Result<IrExpr, IrError> {
        if args.iter().any(|arg| arg.name.is_some()) {
            return Err(IrError::unsupported(
                "native variant constructors do not support named arguments",
                span,
                "pass payload values positionally in declaration order",
            ));
        }
        let Some(info) = self.enum_map.get(enum_name).cloned() else {
            return Err(IrError::internal(
                format!("unknown enum `{enum_name}`"),
                span,
            ));
        };
        let Some(ir_variant) = info
            .variants
            .iter()
            .find(|candidate| candidate.name == variant.name)
            .cloned()
        else {
            return Err(IrError::internal(
                format!("unknown variant `{}` in enum `{enum_name}`", variant.name),
                span,
            ));
        };
        if args.len() != ir_variant.fields.len() {
            return Err(IrError::internal(
                format!(
                    "variant `{}` expects {} payload value(s), found {}",
                    variant.name,
                    ir_variant.fields.len(),
                    args.len()
                ),
                span,
            ));
        }
        let tag = ir_variant.tag;
        let mut lowered = Vec::with_capacity(args.len());
        for (arg, field) in args.iter().zip(ir_variant.fields.iter()) {
            let value = self.lower_expr(&arg.value)?;
            self.expect_assignable(&value.ty, &field.ty, value.span)?;
            lowered.push(value);
        }
        Ok(IrExpr {
            kind: IrExprKind::ConstructVariant {
                enum_name: enum_name.to_string(),
                variant: variant.name.clone(),
                tag,
                args: lowered,
            },
            ty: IrType::Enum(enum_name.to_string()),
            span,
        })
    }

    fn lower_builtin(
        &mut self,
        name: &str,
        args: &[Arg],
        span: Span,
    ) -> Result<IrExpr, IrError> {
        let mut lowered = Vec::with_capacity(args.len());
        for arg in args {
            lowered.push(self.lower_expr(&arg.value)?);
        }
        let ty = match name {
            "print" => IrType::Unit,
            "len" => {
                if lowered.len() != 1 {
                    return Err(IrError::internal("`len` arity was not validated", span));
                }
                match lowered[0].ty {
                    IrType::Str | IrType::List(_) | IrType::Map(_, _) => IrType::Int,
                    ref other => {
                        return Err(IrError::unsupported(
                            format!(
                                "native `len` supports Str, List, and Map, found `{}`",
                                other.name()
                            ),
                            span,
                            "run this program with `auren run`",
                        ));
                    }
                }
            }
            "str" => IrType::Str,
            "int" => {
                if lowered.len() != 1 {
                    return Err(IrError::internal("`int` arity was not validated", span));
                }
                match lowered[0].ty {
                    IrType::Int | IrType::Float | IrType::Str => IrType::Int,
                    _ => {
                        return Err(IrError::unsupported(
                            format!("native `int` cannot convert `{}`", lowered[0].ty.name()),
                            span,
                            "`int` accepts Int, Float, or Str",
                        ));
                    }
                }
            }
            "float" => {
                if lowered.len() != 1 {
                    return Err(IrError::internal("`float` arity was not validated", span));
                }
                match lowered[0].ty {
                    IrType::Int | IrType::Float | IrType::Str => IrType::Float,
                    _ => {
                        return Err(IrError::unsupported(
                            format!("native `float` cannot convert `{}`", lowered[0].ty.name()),
                            span,
                            "`float` accepts Int, Float, or Str",
                        ));
                    }
                }
            }
            "bool" => IrType::Bool,
            "type" => IrType::Str,
            "push" => {
                if lowered.len() != 2 {
                    return Err(IrError::internal("`push` arity was not validated", span));
                }
                let IrType::List(inner) = &lowered[0].ty else {
                    return Err(IrError::unsupported(
                        format!(
                            "native `push` requires a List, found `{}`",
                            lowered[0].ty.name()
                        ),
                        span,
                        "run this program with `auren run` for dynamic collections",
                    ));
                };
                let element_ty = (**inner).clone();
                self.expect_assignable(&lowered[1].ty, &element_ty, args[1].value.span)?;
                IrType::Unit
            }
            "pop" => {
                if lowered.len() != 1 {
                    return Err(IrError::internal("`pop` arity was not validated", span));
                }
                let IrType::List(inner) = &lowered[0].ty else {
                    return Err(IrError::unsupported(
                        format!(
                            "native `pop` requires a List, found `{}`",
                            lowered[0].ty.name()
                        ),
                        span,
                        "run this program with `auren run` for dynamic collections",
                    ));
                };
                (**inner).clone()
            }
            "contains" => {
                if lowered.len() != 2 {
                    return Err(IrError::internal(
                        "`contains` arity was not validated",
                        span,
                    ));
                }
                match &lowered[0].ty {
                    IrType::List(inner) => {
                        let element_ty = (**inner).clone();
                        self.expect_assignable(
                            &lowered[1].ty,
                            &element_ty,
                            args[1].value.span,
                        )?;
                        IrType::Bool
                    }
                    IrType::Str => {
                        if lowered[1].ty != IrType::Str {
                            return Err(IrError::internal(
                                "`contains` needle is not Str",
                                args[1].value.span,
                            ));
                        }
                        IrType::Bool
                    }
                    IrType::Map(key, _) => {
                        let key_ty = (**key).clone();
                        self.expect_assignable(
                            &lowered[1].ty,
                            &key_ty,
                            args[1].value.span,
                        )?;
                        IrType::Bool
                    }
                    other => {
                        return Err(IrError::unsupported(
                            format!(
                                "native `contains` supports List, Str, and Map, found `{}`",
                                other.name()
                            ),
                            span,
                            "run this program with `auren run`",
                        ));
                    }
                }
            }
            "abs" => {
                if lowered.len() != 1 {
                    return Err(IrError::internal("`abs` arity was not validated", span));
                }
                match lowered[0].ty {
                    IrType::Int => IrType::Int,
                    IrType::Float => IrType::Float,
                    _ => {
                        return Err(IrError::unsupported(
                            format!("native `abs` cannot take `{}`", lowered[0].ty.name()),
                            span,
                            "`abs` accepts Int or Float",
                        ));
                    }
                }
            }
            "min" | "max" => {
                if lowered.len() != 2 {
                    return Err(IrError::internal(
                        format!("`{name}` arity was not validated"),
                        span,
                    ));
                }
                match (&lowered[0].ty, &lowered[1].ty) {
                    (IrType::Int, IrType::Int) => IrType::Int,
                    (IrType::Float, IrType::Float) => IrType::Float,
                    _ => {
                        return Err(IrError::unsupported(
                            format!(
                                "native `{name}` requires two Ints or two Floats, found `{}` and `{}`",
                                lowered[0].ty.name(),
                                lowered[1].ty.name()
                            ),
                            span,
                            "mixed numeric types are not allowed in Auren",
                        ));
                    }
                }
            }
            "sqrt" => {
                if lowered.len() != 1 || lowered[0].ty != IrType::Float {
                    return Err(IrError::unsupported(
                        "native `sqrt` requires one Float",
                        span,
                        "convert the argument with `float(...)` first",
                    ));
                }
                IrType::Float
            }
            "floor" | "ceil" => {
                if lowered.len() != 1 || lowered[0].ty != IrType::Float {
                    return Err(IrError::unsupported(
                        format!("native `{name}` requires one Float"),
                        span,
                        "convert the argument with `float(...)` first",
                    ));
                }
                IrType::Int
            }
            "assert" => {
                if lowered.len() != 1 || lowered[0].ty != IrType::Bool {
                    return Err(IrError::internal("`assert` argument is not Bool", span));
                }
                IrType::Unit
            }
            "range" => {
                return Err(IrError::unsupported(
                    "native `range` is supported only directly in `for` loops",
                    span,
                    "assign `range(...)` to a variable and iterate it under `auren run`",
                ));
            }
            "keys" | "values" => {
                if lowered.len() != 1 {
                    return Err(IrError::internal(
                        format!("`{name}` arity was not validated"),
                        span,
                    ));
                }
                let IrType::Map(key, value) = &lowered[0].ty else {
                    return Err(IrError::internal(
                        format!("`{name}` argument is not a Map"),
                        span,
                    ));
                };
                let element = if name == "keys" {
                    (**key).clone()
                } else {
                    (**value).clone()
                };
                IrType::List(Box::new(element))
            }
            _ => {
                return Err(IrError::unsupported(
                    format!("unknown native built-in `{name}`"),
                    span,
                    "this built-in is not available in the native backend",
                ));
            }
        };
        Ok(IrExpr {
            kind: IrExprKind::Builtin {
                name: name.to_string(),
                args: lowered,
            },
            ty,
            span,
        })
    }
}

fn block_has_value_return(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_value_return)
}

fn stmt_has_value_return(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Return(Some(_)) => true,
        StmtKind::If {
            then_block,
            elifs,
            else_block,
            ..
        } => {
            block_has_value_return(then_block)
                || elifs.iter().any(|(_, block)| block_has_value_return(block))
                || else_block.as_ref().is_some_and(block_has_value_return)
        }
        StmtKind::While { body, .. } | StmtKind::For { body, .. } => {
            block_has_value_return(body)
        }
        StmtKind::Match { arms, .. } => arms.iter().any(|arm| match &arm.body {
            MatchArmBody::Block(block) => block_has_value_return(block),
            MatchArmBody::Expr(_) => false,
        }),
        _ => false,
    }
}
