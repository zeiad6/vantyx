//! Name resolution and gradual type checking.

use crate::ast::*;
use crate::diagnostic::{Diagnostic, Diagnostics};
use crate::span::Span;
use crate::types::{EnumInfo, FieldInfo, StructInfo, Type, VariantInfo, resolve_type_expr};
use std::collections::{HashMap, HashSet};

fn literal_key(value: &LiteralPattern) -> String {
    match value {
        LiteralPattern::Int(value) => format!("int:{value}"),
        LiteralPattern::Bool(value) => format!("bool:{value}"),
        LiteralPattern::Str(value) => format!("str:{value}"),
    }
}

pub const BUILTINS: &[&str] = &[
    "print",
    "len",
    "range",
    "push",
    "pop",
    "contains",
    "remove",
    "sorted",
    "reverse",
    "keys",
    "values",
    "str",
    "int",
    "float",
    "bool",
    "type",
    "abs",
    "min",
    "max",
    "sqrt",
    "floor",
    "ceil",
    "assert",
    "read_file",
    "write_file",
    "write_file_atomic",
    "append_file",
    "file_exists",
    "list_dir",
    "make_dir",
    "remove_file",
    "remove_dir",
    "make_dir_all",
    "remove_dir_all",
    "rename",
    "copy_file",
    "copy_dir_all",
    "split",
    "join",
    "trim",
    "starts_with",
    "ends_with",
    "replace",
    "path_join",
    "path_parent",
    "path_name",
    "path_stem",
    "path_extension",
    "path_is_absolute",
    "file_size",
    "is_file",
    "is_dir",
    "is_symlink",
    "glob",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "Int" | "Float" | "Bool" | "Str" | "Unit" | "Any" | "List" | "Map"
    )
}

#[derive(Clone, Debug)]
pub struct ParamSig {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct FnSig {
    pub name: String,
    pub params: Vec<ParamSig>,
    pub ret: Type,
    pub ret_annotated: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    TopLevel,
    Main,
    None,
}

#[derive(Clone, Debug)]
pub struct SemanticModel {
    pub functions: HashMap<String, FnSig>,
    pub structs: HashMap<String, StructInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub expr_types: HashMap<ExprId, Type>,
    pub has_top_level_stmts: bool,
    pub entry: Entry,
}

impl SemanticModel {
    /// Variant constructor names are globally unique, so a name is enough to
    /// find both the owning enum and the payload declaration.
    pub fn find_variant(&self, name: &str) -> Option<(&str, &VariantInfo)> {
        self.enums.iter().find_map(|(enum_name, info)| {
            info.variant(name)
                .map(|variant| (enum_name.as_str(), variant))
        })
    }
}

#[derive(Clone, Debug)]
struct Binding {
    ty: Type,
    mutable: bool,
    span: Span,
}

struct FnCheck {
    ret: Type,
    ret_annotated: bool,
    returns: Vec<(Type, Span)>,
    name: String,
}

pub fn check(program: &Program) -> (Option<SemanticModel>, Diagnostics) {
    check_with_options(program, true)
}

/// Checks a program. `require_entry` is false for REPL snippets that only
/// declare functions or structs and are not meant to execute yet.
pub fn check_with_options(
    program: &Program,
    require_entry: bool,
) -> (Option<SemanticModel>, Diagnostics) {
    let mut checker = Checker::new(program);
    // The module loader removes imports before checking. Any that remain came
    // from stdin, the REPL, or a direct `compile` call, where no file tree is
    // available to resolve them.
    for item in &program.items {
        if let Item::Import(import) = item {
            checker.diagnostics.push(
                Diagnostic::error(
                    "E0510",
                    "imports are only supported when compiling a file",
                    import.span,
                )
                .with_help("run or check a `.vtx` file instead of stdin or the REPL"),
            );
        }
    }
    // Register type names before resolving any field type so structs and
    // enums may refer to each other in fields and variant payloads.
    checker.collect_struct_names();
    checker.collect_enum_names();
    checker.collect_struct_fields();
    checker.collect_enum_variants();
    checker.collect_functions();
    checker.check_top_level();
    checker.check_functions();
    let entry = checker.determine_entry(require_entry);

    let Checker {
        functions,
        structs,
        enums,
        expr_types,
        has_top_level_stmts,
        diagnostics,
        ..
    } = checker;

    let model = SemanticModel {
        functions,
        structs,
        enums,
        expr_types,
        has_top_level_stmts,
        entry,
    };

    if diagnostics.has_errors() {
        (None, diagnostics)
    } else {
        (Some(model), diagnostics)
    }
}

/// A pattern shape used only by the exhaustiveness matrix. `Opaque` matches
/// nothing and is reserved for shapes the parser cannot currently produce;
/// treating them conservatively keeps exhaustiveness sound.
#[derive(Clone, Debug)]
enum CoverPattern {
    Wild,
    Literal(LiteralPattern),
    Variant(String, Vec<CoverPattern>),
    Tuple(Vec<CoverPattern>),
    Opaque,
}

struct Constructor {
    name: String,
    bool_value: Option<bool>,
    tuple: bool,
    field_types: Vec<Type>,
}

impl Constructor {
    fn matches_bool(&self, value: &LiteralPattern) -> bool {
        matches!(
            (self.bool_value, value),
            (Some(expected), LiteralPattern::Bool(actual)) if expected == *actual
        )
    }
}

fn cover_alternatives(pattern: &Pattern) -> Vec<CoverPattern> {
    match pattern {
        Pattern::Wildcard { .. } => vec![CoverPattern::Wild],
        Pattern::Literal { value, .. } => vec![CoverPattern::Literal(value.clone())],
        // Integer ranges are refutable and never contribute coverage; `Int`
        // matches still require a `_` arm. `Opaque` keeps nested coverage
        // conservative instead of treating a range as a wildcard.
        Pattern::Range { .. } => vec![CoverPattern::Opaque],
        Pattern::Variant { name, fields, .. } => {
            let mut converted = Vec::with_capacity(fields.len());
            for field in fields {
                converted.push(match field {
                    PatternArg::Bind { .. } | PatternArg::Wildcard { .. } => CoverPattern::Wild,
                    PatternArg::Nested { pattern, .. } => cover_single(pattern),
                });
            }
            vec![CoverPattern::Variant(name.clone(), converted)]
        }
        Pattern::Tuple { elements, .. } => {
            let mut converted = Vec::with_capacity(elements.len());
            for element in elements {
                converted.push(match element {
                    PatternArg::Bind { .. } | PatternArg::Wildcard { .. } => CoverPattern::Wild,
                    PatternArg::Nested { pattern, .. } => cover_single(pattern),
                });
            }
            vec![CoverPattern::Tuple(converted)]
        }
        Pattern::Or { alternatives, .. } => {
            alternatives.iter().flat_map(cover_alternatives).collect()
        }
    }
}

fn cover_single(pattern: &Pattern) -> CoverPattern {
    let mut alternatives = cover_alternatives(pattern);
    if alternatives.len() == 1 {
        alternatives.pop().unwrap_or(CoverPattern::Opaque)
    } else {
        // Nested or-patterns are not produced by the parser. If one appears,
        // refuse to treat it as a wildcard so exhaustiveness stays sound.
        CoverPattern::Opaque
    }
}

/// Coverage state accumulated while checking the arms of one `match`.
#[derive(Clone)]
struct MatchCoverage<'a> {
    scrutinee: &'a Type,
    enum_info: Option<&'a EnumInfo>,
    strict: bool,
    covered: HashSet<String>,
    covered_literals: HashSet<String>,
    covered_all: bool,
}

struct Checker<'a> {
    program: &'a Program,
    diagnostics: Diagnostics,
    functions: HashMap<String, FnSig>,
    structs: HashMap<String, StructInfo>,
    enums: HashMap<String, EnumInfo>,
    expr_types: HashMap<ExprId, Type>,
    scopes: Vec<HashMap<String, Binding>>,
    current_fn: Option<FnCheck>,
    loop_depth: usize,
    has_top_level_stmts: bool,
    /// Set by `check_statement_expr` immediately before the one expression
    /// position where postfix `?` is allowed. Nested expressions consume and
    /// clear it, so `?` cannot hide inside a larger expression.
    try_allowed: bool,
}

impl<'a> Checker<'a> {
    fn new(program: &'a Program) -> Self {
        let has_top_level_stmts = program
            .items
            .iter()
            .any(|item| matches!(item, Item::Stmt(_)));
        Self {
            program,
            diagnostics: Diagnostics::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            expr_types: HashMap::new(),
            scopes: Vec::new(),
            current_fn: None,
            loop_depth: 0,
            has_top_level_stmts,
            try_allowed: false,
        }
    }

    // ---- collection passes ----

    fn collect_struct_names(&mut self) {
        let program = self.program;
        for item in &program.items {
            let Item::Struct(decl) = item else { continue };
            if is_builtin_type(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!("cannot redefine built-in type `{}`", decl.name),
                    decl.name_span,
                ));
                continue;
            }
            if self.structs.contains_key(&decl.name) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0202",
                        format!("duplicate struct `{}`", decl.name),
                        decl.name_span,
                    )
                    .with_help("struct names must be unique"),
                );
                continue;
            }
            self.structs.insert(
                decl.name.clone(),
                StructInfo {
                    name: decl.name.clone(),
                    fields: Vec::new(),
                    span: decl.span,
                },
            );
        }
    }

    fn collect_struct_fields(&mut self) {
        let program = self.program;
        for item in &program.items {
            let Item::Struct(decl) = item else { continue };
            let mut fields = Vec::new();
            let mut seen = HashSet::new();
            for field in &decl.fields {
                if !seen.insert(field.name.clone()) {
                    self.diagnostics.push(Diagnostic::error(
                        "E0202",
                        format!("duplicate field `{}`", field.name),
                        field.name_span,
                    ));
                }
                let ty = self.resolve_type(&field.ty);
                fields.push(FieldInfo {
                    name: field.name.clone(),
                    ty,
                    span: field.span,
                });
            }
            if let Some(info) = self.structs.get_mut(&decl.name) {
                info.fields = fields;
            }
        }
    }

    fn collect_enum_names(&mut self) {
        let program = self.program;
        // Register every enum name first so recursive payload types resolve.
        for item in &program.items {
            let Item::Enum(decl) = item else { continue };
            if is_builtin_type(&decl.name) || is_builtin(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!("cannot redefine built-in name `{}` as an enum", decl.name),
                    decl.name_span,
                ));
                continue;
            }
            if self.structs.contains_key(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!("`{}` is already declared as a struct", decl.name),
                    decl.name_span,
                ));
                continue;
            }
            if self.enums.contains_key(&decl.name) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0202",
                        format!("duplicate enum `{}`", decl.name),
                        decl.name_span,
                    )
                    .with_help("enum names must be unique"),
                );
                continue;
            }
            self.enums.insert(
                decl.name.clone(),
                EnumInfo {
                    name: decl.name.clone(),
                    variants: Vec::new(),
                    span: decl.span,
                },
            );
        }
    }

    fn collect_enum_variants(&mut self) {
        let program = self.program;
        let mut variant_names: HashMap<String, String> = HashMap::new();
        for item in &program.items {
            let Item::Enum(decl) = item else { continue };
            if !self.enums.contains_key(&decl.name) {
                continue;
            }
            let mut variants = Vec::new();
            let mut seen_variants = HashSet::new();
            for variant in &decl.variants {
                if !seen_variants.insert(variant.name.clone()) {
                    self.diagnostics.push(Diagnostic::error(
                        "E0202",
                        format!(
                            "duplicate variant `{}` in enum `{}`",
                            variant.name, decl.name
                        ),
                        variant.name_span,
                    ));
                }
                if is_builtin(&variant.name)
                    || is_builtin_type(&variant.name)
                    || self.structs.contains_key(&variant.name)
                    || self.enums.contains_key(&variant.name)
                {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0202",
                            format!(
                                "variant `{}` collides with an existing type or function",
                                variant.name
                            ),
                            variant.name_span,
                        )
                        .with_help("variant constructor names must be unique"),
                    );
                }
                if let Some(previous) =
                    variant_names.insert(variant.name.clone(), decl.name.clone())
                {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0202",
                            format!(
                                "duplicate variant `{}`; it is already declared in enum `{previous}`",
                                variant.name
                            ),
                            variant.name_span,
                        )
                        .with_help("variant constructor names are global"),
                    );
                }

                let mut fields = Vec::new();
                let mut seen_fields = HashSet::new();
                for field in &variant.fields {
                    if !seen_fields.insert(field.name.clone()) {
                        self.diagnostics.push(Diagnostic::error(
                            "E0202",
                            format!("duplicate field `{}`", field.name),
                            field.name_span,
                        ));
                    }
                    let ty = self.resolve_type(&field.ty);
                    fields.push(FieldInfo {
                        name: field.name.clone(),
                        ty,
                        span: field.span,
                    });
                }
                variants.push(VariantInfo {
                    name: variant.name.clone(),
                    fields,
                    span: variant.span,
                });
            }
            if let Some(info) = self.enums.get_mut(&decl.name) {
                info.variants = variants;
            }
        }
    }

    fn find_variant(&self, name: &str) -> Option<(&str, &VariantInfo)> {
        self.enums.iter().find_map(|(enum_name, info)| {
            info.variant(name)
                .map(|variant| (enum_name.as_str(), variant))
        })
    }

    fn collect_functions(&mut self) {
        let program = self.program;
        for item in &program.items {
            let Item::Fn(decl) = item else { continue };
            if is_builtin(&decl.name) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0202",
                        format!("cannot redeclare built-in function `{}`", decl.name),
                        decl.name_span,
                    )
                    .with_help("choose a different name"),
                );
                continue;
            }
            if is_builtin_type(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!(
                        "cannot use built-in type name `{}` as a function",
                        decl.name
                    ),
                    decl.name_span,
                ));
                continue;
            }
            if self.structs.contains_key(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!("`{}` is already declared as a struct", decl.name),
                    decl.name_span,
                ));
                continue;
            }
            if self.enums.contains_key(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!("`{}` is already declared as an enum", decl.name),
                    decl.name_span,
                ));
                continue;
            }
            if let Some((enum_name, _)) = self.find_variant(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!(
                        "`{}` is already declared as a variant of enum `{enum_name}`",
                        decl.name
                    ),
                    decl.name_span,
                ));
                continue;
            }
            if self.functions.contains_key(&decl.name) {
                self.diagnostics.push(Diagnostic::error(
                    "E0202",
                    format!("duplicate function `{}`", decl.name),
                    decl.name_span,
                ));
                continue;
            }

            let mut params = Vec::new();
            let mut seen = HashSet::new();
            for param in &decl.params {
                if is_builtin(&param.name) {
                    self.diagnostics.push(Diagnostic::error(
                        "E0202",
                        format!("cannot shadow built-in function `{}`", param.name),
                        param.name_span,
                    ));
                }
                if !seen.insert(param.name.clone()) {
                    self.diagnostics.push(Diagnostic::error(
                        "E0202",
                        format!("duplicate parameter `{}`", param.name),
                        param.name_span,
                    ));
                }
                let ty = param
                    .ty
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Type::Any);
                params.push(ParamSig {
                    name: param.name.clone(),
                    ty,
                    span: param.span,
                });
            }

            let (ret, ret_annotated) = match &decl.ret {
                Some(ty) => (self.resolve_type(ty), true),
                None => (Type::Any, false),
            };

            self.functions.insert(
                decl.name.clone(),
                FnSig {
                    name: decl.name.clone(),
                    params,
                    ret,
                    ret_annotated,
                    span: decl.span,
                },
            );
        }
    }

    // ---- top level and functions ----

    fn check_top_level(&mut self) {
        // The global scope stays on the stack while functions are checked:
        // top-level bindings are visible to functions. A function called
        // before its global is initialized fails at runtime with R0009.
        self.scopes.push(HashMap::new());
        let program = self.program;
        for item in &program.items {
            if let Item::Stmt(stmt) = item {
                self.check_stmt(stmt);
            }
        }
    }

    fn check_functions(&mut self) {
        let program = self.program;
        for item in &program.items {
            if let Item::Fn(decl) = item {
                self.check_function(decl);
            }
        }
    }

    fn check_function(&mut self, decl: &FnDecl) {
        let Some(sig) = self.functions.get(&decl.name).cloned() else {
            return;
        };

        self.current_fn = Some(FnCheck {
            ret: sig.ret.clone(),
            ret_annotated: sig.ret_annotated,
            returns: Vec::new(),
            name: decl.name.clone(),
        });
        self.scopes.push(HashMap::new());
        for (param, param_sig) in decl.params.iter().zip(sig.params.iter()) {
            self.declare(&param.name, param_sig.ty.clone(), false, param.name_span);
        }
        self.check_stmts(&decl.body.stmts);
        self.scopes.pop();

        let Some(ctx) = self.current_fn.take() else {
            return;
        };

        let returns_value = ctx.returns.iter().any(|(ty, _)| !matches!(ty, Type::Unit));
        let returns_unit = ctx.returns.iter().any(|(ty, _)| matches!(ty, Type::Unit));

        if ctx.ret_annotated {
            if ctx.ret != Type::Unit && !block_returns(&decl.body) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0309",
                        format!(
                            "not all paths in `{}` return a value of type `{}`",
                            ctx.name, ctx.ret
                        ),
                        decl.name_span,
                    )
                    .with_help("end every branch with a `return`"),
                );
            }
        } else {
            let inferred = infer_return(&ctx.returns);
            if let Some(sig) = self.functions.get_mut(&decl.name) {
                sig.ret = inferred.clone();
            }
            if inferred != Type::Unit && !block_returns(&decl.body) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0309",
                        format!("not all paths in `{}` return a value", ctx.name),
                        decl.name_span,
                    )
                    .with_help("add a final `return`, or annotate the return type as `Unit`"),
                );
            }
            if returns_value && returns_unit {
                self.diagnostics.push(Diagnostic::warning(
                    "W0002",
                    format!(
                        "`{}` returns both a value and nothing; the inferred type is `{}`",
                        ctx.name, inferred
                    ),
                    decl.name_span,
                ));
            }
        }
    }

    fn determine_entry(&mut self, require_entry: bool) -> Entry {
        let main = self.functions.get("main").cloned();
        if self.has_top_level_stmts {
            if let Some(main) = &main {
                self.diagnostics.push(
                    Diagnostic::warning(
                        "W0001",
                        "`main` is not called automatically because this file has top-level statements",
                        main.span,
                    )
                    .with_help("remove the top-level statements or call `main()` explicitly"),
                );
            }
            Entry::TopLevel
        } else if let Some(main) = main {
            if !main.params.is_empty() {
                self.diagnostics.push(
                    Diagnostic::error("E0308", "`main` must not take parameters", main.span)
                        .with_help("move configuration into top-level code or a separate function"),
                );
                Entry::None
            } else {
                Entry::Main
            }
        } else {
            if require_entry {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0308",
                        "no entry point: add top-level code or define `fn main()`",
                        self.program.span,
                    )
                    .with_help("a program needs something to execute"),
                );
            }
            Entry::None
        }
    }

    // ---- statements ----

    fn check_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.check_stmt(stmt);
        }
    }

    fn check_block(&mut self, block: &Block) {
        self.scopes.push(HashMap::new());
        self.check_stmts(&block.stmts);
        self.scopes.pop();
    }

    /// Declares the bindings of a `let`/`var` destructuring pattern. Tuple
    /// arity is checked against the initializer type; a dynamic `Any`
    /// initializer defers the shape check to runtime.
    fn check_let_pattern(&mut self, pattern: &LetPattern, ty: &Type, mutable: bool) {
        match pattern {
            LetPattern::Name { name, name_span } => {
                self.declare(name, ty.clone(), mutable, *name_span);
            }
            LetPattern::Wildcard { .. } => {}
            LetPattern::Tuple { elements, span } => match ty {
                Type::Tuple(types) if types.len() == elements.len() => {
                    for (element, element_ty) in elements.iter().zip(types.iter()) {
                        self.check_let_pattern(element, element_ty, mutable);
                    }
                }
                Type::Tuple(_) => {
                    self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!(
                                    "binding pattern has {} element(s), but the initializer has type `{ty}`",
                                    elements.len()
                                ),
                                *span,
                            )
                            .with_help("bind every tuple element, or use `_` to ignore one"),
                        );
                    for element in elements {
                        self.check_let_pattern(element, &Type::Any, mutable);
                    }
                }
                Type::Any => {
                    for element in elements {
                        self.check_let_pattern(element, &Type::Any, mutable);
                    }
                }
                other => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0301",
                            format!("cannot destructure a value of type `{other}`"),
                            *span,
                        )
                        .with_help("the initializer must be a tuple of the same arity"),
                    );
                    for element in elements {
                        self.check_let_pattern(element, &Type::Any, mutable);
                    }
                }
            },
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let {
                mutable,
                name,
                name_span,
                ty,
                init,
            } => {
                let expected = ty.as_ref().map(|t| self.resolve_type(t));
                let init_ty = self.check_statement_expr(init, expected.as_ref());
                if let Some(expected) = &expected {
                    self.expect(
                        &init_ty,
                        expected,
                        init.span,
                        &format!(" in binding `{name}`"),
                    );
                }
                let binding_ty = expected.unwrap_or(init_ty);
                self.declare(name, binding_ty, *mutable, *name_span);
            }
            StmtKind::LetPattern {
                mutable,
                pattern,
                ty,
                init,
            } => {
                let expected = ty.as_ref().map(|t| self.resolve_type(t));
                let init_ty = self.check_statement_expr(init, expected.as_ref());
                if let Some(expected) = &expected {
                    self.expect(&init_ty, expected, init.span, " in a binding pattern");
                }
                let binding_ty = expected.unwrap_or(init_ty);
                self.check_let_pattern(pattern, &binding_ty, *mutable);
            }
            StmtKind::Assign { target, op, value } => {
                let target_ty = self.check_lvalue(target);
                if let Some(op) = op {
                    let value_ty = self.check_expr(value, None);
                    let result_ty = self.binary_type(*op, &target_ty, &value_ty, value.span);
                    self.expect(
                        &result_ty,
                        &target_ty,
                        value.span,
                        " in compound assignment",
                    );
                } else {
                    let value_ty = self.check_statement_expr(value, Some(&target_ty));
                    self.expect(&value_ty, &target_ty, value.span, " in assignment");
                }
                self.expr_types.insert(target.id, target_ty);
            }
            StmtKind::Expr(expr) => {
                self.check_statement_expr(expr, None);
            }
            StmtKind::If {
                cond,
                then_block,
                elifs,
                else_block,
            } => {
                let cond_ty = self.check_expr(cond, Some(&Type::Bool));
                self.expect(&cond_ty, &Type::Bool, cond.span, " as a condition");
                self.check_block(then_block);
                for (elif_cond, block) in elifs {
                    let ty = self.check_expr(elif_cond, Some(&Type::Bool));
                    self.expect(&ty, &Type::Bool, elif_cond.span, " as a condition");
                    self.check_block(block);
                }
                if let Some(block) = else_block {
                    self.check_block(block);
                }
            }
            StmtKind::While { cond, body } => {
                let cond_ty = self.check_expr(cond, Some(&Type::Bool));
                self.expect(&cond_ty, &Type::Bool, cond.span, " as a condition");
                self.loop_depth += 1;
                self.check_block(body);
                self.loop_depth -= 1;
            }
            StmtKind::For {
                name,
                name_span,
                iter,
                body,
            } => {
                let iter_ty = self.check_expr(iter, None);
                let elem_ty = match &iter_ty {
                    Type::List(inner) => (**inner).clone(),
                    Type::Str => Type::Str,
                    Type::Map(key, _) => (**key).clone(),
                    Type::Any => Type::Any,
                    other => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0306",
                                format!("cannot iterate over a value of type `{other}`"),
                                iter.span,
                            )
                            .with_help("for loops accept List, Str, or Map values"),
                        );
                        Type::Any
                    }
                };
                self.scopes.push(HashMap::new());
                self.declare(name, elem_ty, false, *name_span);
                self.loop_depth += 1;
                self.check_stmts(&body.stmts);
                self.loop_depth -= 1;
                self.scopes.pop();
            }
            StmtKind::Match { scrutinee, arms } => {
                self.check_match(scrutinee, arms, None, false);
            }
            StmtKind::Return(value) => {
                if self.current_fn.is_none() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0205",
                            "`return` is only valid inside a function",
                            stmt.span,
                        )
                        .with_help("top-level code cannot return a value"),
                    );
                    if let Some(expr) = value {
                        self.check_expr(expr, None);
                    }
                    return;
                }
                let expected_ret = self.current_fn.as_ref().and_then(|ctx| {
                    if ctx.ret_annotated {
                        Some(ctx.ret.clone())
                    } else {
                        None
                    }
                });
                let value_ty = match value {
                    Some(expr) => self.check_expr(expr, expected_ret.as_ref()),
                    None => Type::Unit,
                };
                if let Some(expected) = &expected_ret {
                    self.expect(&value_ty, expected, stmt.span, " in return");
                }
                if let Some(ctx) = self.current_fn.as_mut() {
                    ctx.returns.push((value_ty, stmt.span));
                }
            }
            StmtKind::Break => {
                if self.loop_depth == 0 {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0204",
                            "`break` is only valid inside a loop",
                            stmt.span,
                        )
                        .with_help("move it into a `while` or `for` body"),
                    );
                }
            }
            StmtKind::Continue => {
                if self.loop_depth == 0 {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0204",
                            "`continue` is only valid inside a loop",
                            stmt.span,
                        )
                        .with_help("move it into a `while` or `for` body"),
                    );
                }
            }
        }
    }

    fn check_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        expected: Option<&Type>,
        expression: bool,
    ) -> Type {
        let scrutinee_ty = self.check_expr(scrutinee, None);
        let enum_info = match &scrutinee_ty {
            Type::Enum(name) => self.enums.get(name).cloned(),
            Type::Any | Type::Int | Type::Str | Type::Bool | Type::Tuple(_) => None,
            other => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0314",
                        format!("cannot match on a value of type `{other}`"),
                        scrutinee.span,
                    )
                    .with_help(
                        "match requires an enum, `Int`, `Str`, `Bool`, tuple, or `Any` value",
                    ),
                );
                None
            }
        };
        let strict = matches!(
            scrutinee_ty,
            Type::Enum(_) | Type::Any | Type::Int | Type::Str | Type::Bool | Type::Tuple(_)
        );
        let warn_any = matches!(scrutinee_ty, Type::Any);

        let mut coverage = MatchCoverage {
            scrutinee: &scrutinee_ty,
            enum_info: enum_info.as_ref(),
            strict,
            covered: HashSet::new(),
            covered_literals: HashSet::new(),
            covered_all: false,
        };
        let mut arm_types = Vec::new();
        let mut unguarded_patterns = Vec::new();

        for arm in arms {
            self.scopes.push(HashMap::new());
            self.check_pattern(&arm.pattern, &mut coverage, arm.guard.is_none());
            if arm.guard.is_none() {
                unguarded_patterns.push(&arm.pattern);
            }
            if let Some(guard) = &arm.guard {
                let guard_ty = self.check_expr(guard, Some(&Type::Bool));
                self.expect(&guard_ty, &Type::Bool, guard.span, " in match guard");
            }
            let arm_ty = match &arm.body {
                MatchArmBody::Expr(expr) => {
                    if !expression {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0314",
                                "a match statement arm must be an indented block",
                                arm.span,
                            )
                            .with_help("put the arm body on its own indented line"),
                        );
                    }
                    let ty = self.check_expr(expr, expected);
                    if let Some(expected) = expected {
                        self.expect(&ty, expected, expr.span, " in match arm");
                    }
                    ty
                }
                MatchArmBody::Block(block) => {
                    if expression {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0314",
                                "a match expression arm must be a single expression",
                                arm.span,
                            )
                            .with_help("use a `match` statement for block arms"),
                        );
                    }
                    self.check_stmts(&block.stmts);
                    Type::Unit
                }
            };
            self.scopes.pop();
            arm_types.push((arm_ty, arm.span));
        }

        let exhaustive = self.patterns_exhaustive(&scrutinee_ty, &unguarded_patterns);
        if let Some(info) = &enum_info {
            if !exhaustive {
                let missing: Vec<&str> = info
                    .variants
                    .iter()
                    .filter(|variant| !coverage.covered.contains(&variant.name))
                    .map(|variant| variant.name.as_str())
                    .collect();
                if !missing.is_empty() {
                    let names = missing.join("`, `");
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0316",
                            format!(
                                "non-exhaustive match on enum `{}`: missing variant(s) `{names}`",
                                info.name
                            ),
                            scrutinee.span,
                        )
                        .with_help("add the missing arms or a final `_` arm"),
                    );
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0316",
                            format!(
                                "non-exhaustive match on enum `{}`: payload patterns do not cover every value",
                                info.name
                            ),
                            scrutinee.span,
                        )
                        .with_help(
                            "cover every nested payload shape, or add a final `_` arm",
                        ),
                    );
                }
            }
        } else if !exhaustive {
            match &scrutinee_ty {
                Type::Bool => {
                    let missing: Vec<&str> = [true, false]
                        .into_iter()
                        .filter(|value| {
                            !coverage
                                .covered_literals
                                .contains(&literal_key(&LiteralPattern::Bool(*value)))
                        })
                        .map(|value| if value { "`true`" } else { "`false`" })
                        .collect();
                    if !missing.is_empty() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0316",
                                format!(
                                    "non-exhaustive match on `Bool`: missing {}",
                                    missing.join(" and ")
                                ),
                                scrutinee.span,
                            )
                            .with_help("cover both `true` and `false`, or add a final `_` arm"),
                        );
                    }
                }
                Type::Int | Type::Str => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0316",
                            format!("non-exhaustive match on `{scrutinee_ty}`"),
                            scrutinee.span,
                        )
                        .with_help("add a final `_` arm; literal matches are open-ended"),
                    );
                }
                Type::Tuple(_) => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0316",
                            format!("non-exhaustive match on `{scrutinee_ty}`"),
                            scrutinee.span,
                        )
                        .with_help("cover every tuple element shape, or add a final `_` arm"),
                    );
                }
                Type::Any if warn_any => {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            "W0004",
                            "match on `Any` cannot be checked for exhaustiveness",
                            scrutinee.span,
                        )
                        .with_help("add a `_` arm; an unmatched value raises `R0014` at runtime"),
                    );
                }
                _ => {}
            }
        }

        if !expression {
            return Type::Unit;
        }
        if let Some(expected) = expected {
            return expected.clone();
        }
        match arm_types.first() {
            Some((first, _)) if arm_types.iter().all(|(ty, _)| ty == first) => first.clone(),
            Some(_) if arm_types.iter().any(|(ty, _)| ty.is_any()) => Type::Any,
            Some((first, _)) => {
                for (ty, span) in &arm_types[1..] {
                    if ty != first {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!(
                                    "match arms have incompatible types: expected `{first}`, found `{ty}`"
                                ),
                                *span,
                            )
                            .with_help("make every arm produce the same type"),
                        );
                    }
                }
                Type::Any
            }
            None => Type::Unit,
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, coverage: &mut MatchCoverage<'_>, counts: bool) {
        let scrutinee = coverage.scrutinee;
        let enum_info = coverage.enum_info;
        let strict = coverage.strict;
        match pattern {
            Pattern::Wildcard { span } => {
                if coverage.covered_all {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            "W0003",
                            "unreachable pattern: an earlier pattern already covers every value",
                            *span,
                        )
                        .with_help("remove this pattern or move it earlier"),
                    );
                } else if counts {
                    coverage.covered_all = true;
                }
            }
            Pattern::Literal { value, span } => {
                if coverage.covered_all {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            "W0003",
                            "unreachable pattern: an earlier pattern already covers every value",
                            *span,
                        )
                        .with_help("remove this pattern or move it earlier"),
                    );
                }
                let compatible = match (value, scrutinee) {
                    (_, Type::Any) => true,
                    (LiteralPattern::Int(_), Type::Int)
                    | (LiteralPattern::Bool(_), Type::Bool)
                    | (LiteralPattern::Str(_), Type::Str) => true,
                    (_, Type::Enum(name)) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0315",
                                format!("literal pattern cannot match enum `{name}`"),
                                *span,
                            )
                            .with_help("use a variant pattern such as `Variant` or `Variant(x)`"),
                        );
                        false
                    }
                    (value, other) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!(
                                    "pattern has type `{}`, but the matched value has type `{other}`",
                                    value.type_name()
                                ),
                                *span,
                            )
                            .with_help("use a literal of the same type, or add a `_` arm"),
                        );
                        false
                    }
                };
                if compatible {
                    let key = literal_key(value);
                    if coverage.covered_literals.contains(&key) {
                        self.diagnostics.push(
                            Diagnostic::warning(
                                "W0003",
                                "unreachable pattern: this literal is already covered",
                                *span,
                            )
                            .with_help("remove this arm or merge the cases"),
                        );
                    } else if counts {
                        coverage.covered_literals.insert(key);
                    }
                }
            }
            Pattern::Range {
                start,
                end,
                inclusive,
                span,
            } => {
                if coverage.covered_all {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            "W0003",
                            "unreachable pattern: an earlier pattern already covers every value",
                            *span,
                        )
                        .with_help("remove this pattern or move it earlier"),
                    );
                }
                if !matches!(scrutinee, Type::Any | Type::Int) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0301",
                            format!(
                                "range pattern has type `Int`, but the matched value has type `{scrutinee}`"
                            ),
                            *span,
                        )
                        .with_help("use an integer value, or match on an enum"),
                    );
                }
                let empty = if *inclusive {
                    start > end
                } else {
                    start >= end
                };
                if empty {
                    let example = if *inclusive { "0..=5" } else { "0..6" };
                    self.diagnostics.push(
                        Diagnostic::error("E0318", "range pattern is empty", *span).with_help(
                            format!(
                                "the upper bound must be {} the lower bound; use a non-empty range such as `{example}`",
                                if *inclusive { "at least" } else { "greater than" }
                            ),
                        ),
                    );
                }
            }
            Pattern::Variant {
                name,
                name_span,
                fields,
                ..
            } => {
                if coverage.covered_all {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            "W0003",
                            "unreachable pattern: an earlier pattern already covers every value",
                            *name_span,
                        )
                        .with_help("remove this pattern or move it earlier"),
                    );
                }

                if matches!(scrutinee, Type::Int | Type::Str | Type::Bool) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0315",
                            format!(
                                "variant pattern `{name}` cannot match a value of type `{scrutinee}`"
                            ),
                            *name_span,
                        )
                        .with_help("use a literal pattern, or match on an enum value"),
                    );
                    for field in fields {
                        if let PatternArg::Bind {
                            name: binding,
                            name_span,
                        } = field
                        {
                            self.declare(binding, Type::Any, false, *name_span);
                        }
                    }
                    return;
                }

                let variant = match enum_info {
                    Some(info) => info.variant(name).cloned(),
                    None => self.find_variant(name).map(|(_, variant)| variant.clone()),
                };

                if variant.is_none() && strict {
                    if enum_info.is_some() {
                        if let Some((owner, _)) = self.find_variant(name) {
                            let expected = enum_info
                                .map(|info| info.name.as_str())
                                .unwrap_or("the matched value");
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0315",
                                    format!(
                                        "variant `{name}` belongs to enum `{owner}`, not `{expected}`"
                                    ),
                                    *name_span,
                                )
                                .with_help("use a variant of the matched enum"),
                            );
                        } else {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0315",
                                    format!("unknown variant `{name}`"),
                                    *name_span,
                                )
                                .with_help(
                                    "declare the variant in an `enum` before matching on it",
                                ),
                            );
                        }
                    } else {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0315",
                                format!("unknown variant `{name}`"),
                                *name_span,
                            )
                            .with_help("declare the variant in an `enum` before matching on it"),
                        );
                    }
                }

                if let Some(variant) = &variant {
                    if coverage.covered.contains(&variant.name) {
                        self.diagnostics.push(
                            Diagnostic::warning(
                                "W0003",
                                format!(
                                    "unreachable pattern: variant `{}` is already covered",
                                    variant.name
                                ),
                                *name_span,
                            )
                            .with_help("remove this arm or merge the cases"),
                        );
                    } else if counts && self.variant_pattern_covers(fields) {
                        coverage.covered.insert(variant.name.clone());
                    }
                    if fields.len() != variant.fields.len() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0302",
                                format!(
                                    "variant `{}` expects {} payload value(s), found {}",
                                    variant.name,
                                    variant.fields.len(),
                                    fields.len()
                                ),
                                *name_span,
                            )
                            .with_help("bind every payload field, using `_` to ignore one"),
                        );
                    }
                }

                for (index, field) in fields.iter().enumerate() {
                    let ty = variant
                        .as_ref()
                        .and_then(|variant| variant.fields.get(index))
                        .map(|field| field.ty.clone())
                        .unwrap_or(Type::Any);
                    self.check_pattern_arg(field, &ty);
                }
            }
            Pattern::Tuple { elements, span } => {
                if coverage.covered_all {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            "W0003",
                            "unreachable pattern: an earlier pattern already covers every value",
                            *span,
                        )
                        .with_help("remove this pattern or move it earlier"),
                    );
                }
                let mut shape_valid = false;
                match scrutinee {
                    Type::Tuple(types) => {
                        if types.len() != elements.len() {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0301",
                                    format!(
                                        "tuple pattern has {} element(s), but the matched value has type `{scrutinee}`",
                                        elements.len()
                                    ),
                                    *span,
                                )
                                .with_help("match every tuple element, using `_` to ignore one"),
                            );
                        } else {
                            shape_valid = true;
                        }
                        for (index, element) in elements.iter().enumerate() {
                            let ty = types.get(index).cloned().unwrap_or(Type::Any);
                            self.check_pattern_arg(element, &ty);
                        }
                    }
                    Type::Any => {
                        // A fixed-arity tuple pattern never covers every
                        // dynamic value: the value may not be a tuple, or may
                        // have a different arity. Arms after it stay reachable.
                        for element in elements {
                            self.check_pattern_arg(element, &Type::Any);
                        }
                    }
                    other => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!("tuple pattern cannot match a value of type `{other}`"),
                                *span,
                            )
                            .with_help("match a tuple value, or use a literal or variant pattern"),
                        );
                        for element in elements {
                            self.check_pattern_arg(element, &Type::Any);
                        }
                    }
                }
                if counts && shape_valid && self.tuple_pattern_covers(elements) {
                    coverage.covered_all = true;
                }
            }
            Pattern::Or { alternatives, .. } => {
                self.check_or_pattern(alternatives, coverage, counts);
            }
        }
    }

    /// Checks top-level or-pattern alternatives. Each alternative is checked in
    /// a temporary scope so bindings can be compared; the arm receives one
    /// unified binding set. Only unguarded or-patterns contribute coverage.
    fn check_or_pattern(
        &mut self,
        alternatives: &[Pattern],
        coverage: &mut MatchCoverage<'_>,
        counts: bool,
    ) {
        let mut seen = coverage.clone();
        let mut expected: Option<Vec<(String, Type, Span)>> = None;
        for alternative in alternatives {
            let mut local = seen.clone();
            self.scopes.push(HashMap::new());
            self.check_pattern(alternative, &mut local, true);
            let bindings = self.scopes.pop().unwrap_or_default();
            let mut actual: Vec<(String, Type, Span)> = bindings
                .into_iter()
                .map(|(name, binding)| (name, binding.ty, binding.span))
                .collect();
            actual.sort_by_key(|(_, _, span)| (span.start, span.end));
            match &expected {
                Some(expected) => {
                    self.check_or_binding_compatibility(expected, &actual, alternative.span());
                }
                None => expected = Some(actual),
            }
            seen = local;
        }
        if counts {
            coverage.covered = seen.covered;
            coverage.covered_literals = seen.covered_literals;
            coverage.covered_all = seen.covered_all;
        }
        for (name, ty, span) in expected.unwrap_or_default() {
            self.declare(&name, ty, false, span);
        }
    }

    fn check_or_binding_compatibility(
        &mut self,
        expected: &[(String, Type, Span)],
        actual: &[(String, Type, Span)],
        span: Span,
    ) {
        let expected_names: HashSet<&str> =
            expected.iter().map(|(name, _, _)| name.as_str()).collect();
        let actual_names: HashSet<&str> = actual.iter().map(|(name, _, _)| name.as_str()).collect();
        let missing: Vec<&str> = expected_names.difference(&actual_names).copied().collect();
        let extra: Vec<&str> = actual_names.difference(&expected_names).copied().collect();
        let mut missing = missing;
        let mut extra = extra;
        missing.sort_unstable();
        extra.sort_unstable();
        if !missing.is_empty() || !extra.is_empty() {
            let mut details = Vec::new();
            if !missing.is_empty() {
                details.push(format!("missing `{}`", missing.join("`, `")));
            }
            if !extra.is_empty() {
                details.push(format!("unexpected `{}`", extra.join("`, `")));
            }
            self.diagnostics.push(
                Diagnostic::error(
                    "E0317",
                    format!(
                        "or-pattern alternatives bind different names: {}",
                        details.join("; ")
                    ),
                    span,
                )
                .with_help(
                    "bind the same payload names in every alternative, or use `_` for ignored payloads",
                ),
            );
            return;
        }
        let actual_types: HashMap<&str, &Type> = actual
            .iter()
            .map(|(name, ty, _)| (name.as_str(), ty))
            .collect();
        for (name, ty, _) in expected {
            if let Some(actual_ty) = actual_types.get(name.as_str())
                && *actual_ty != ty
            {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0317",
                        format!(
                            "or-pattern binding `{name}` has type `{actual_ty}` in one alternative and `{ty}` in another"
                        ),
                        span,
                    )
                    .with_help("make the payload types identical in every alternative"),
                );
            }
        }
    }

    /// Checks one positional pattern element against its known type and
    /// declares any bindings it introduces. Shared by enum payloads and
    /// tuple patterns.
    fn check_pattern_arg(&mut self, arg: &PatternArg, ty: &Type) {
        match arg {
            PatternArg::Bind { name, name_span } => {
                self.declare(name, ty.clone(), false, *name_span);
            }
            PatternArg::Wildcard { .. } => {}
            PatternArg::Nested { pattern, .. } => {
                let nested_info = match ty {
                    Type::Enum(name) => self.enums.get(name).cloned(),
                    _ => None,
                };
                let mut nested_coverage = MatchCoverage {
                    scrutinee: ty,
                    enum_info: nested_info.as_ref(),
                    strict: true,
                    covered: HashSet::new(),
                    covered_literals: HashSet::new(),
                    covered_all: false,
                };
                self.check_pattern(pattern, &mut nested_coverage, false);
            }
        }
    }

    /// Whether a tuple pattern is irrefutable: every element binds or ignores,
    /// or is itself irrefutable. A tuple of a fixed arity is the only
    /// constructor, so such a pattern covers the whole tuple type.
    fn tuple_pattern_covers(&self, elements: &[PatternArg]) -> bool {
        elements.iter().all(|element| match element {
            PatternArg::Bind { .. } | PatternArg::Wildcard { .. } => true,
            PatternArg::Nested { pattern, .. } => self.pattern_is_irrefutable(pattern),
        })
    }

    /// Whether a variant pattern completely covers its own variant, meaning
    /// every payload position is irrefutable for that field's type. Partial
    /// nested patterns such as `Node(Leaf(x))` do not mark `Node` covered.
    fn variant_pattern_covers(&self, fields: &[PatternArg]) -> bool {
        fields.iter().all(|field| match field {
            PatternArg::Bind { .. } | PatternArg::Wildcard { .. } => true,
            PatternArg::Nested { pattern, .. } => self.pattern_is_irrefutable(pattern),
        })
    }

    /// A pattern is irrefutable when it matches every value of its own type.
    /// Enum variants are only irrefutable when their enum has exactly one
    /// variant; literals are always refutable.
    fn pattern_is_irrefutable(&self, pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Wildcard { .. } => true,
            Pattern::Literal { .. } | Pattern::Range { .. } => false,
            Pattern::Or { alternatives, .. } => alternatives
                .iter()
                .any(|alternative| self.pattern_is_irrefutable(alternative)),
            Pattern::Tuple { elements, .. } => self.tuple_pattern_covers(elements),
            Pattern::Variant { name, fields, .. } => {
                let Some((owner, _)) = self.find_variant(name) else {
                    return false;
                };
                let Some(info) = self.enums.get(owner) else {
                    return false;
                };
                info.variants.len() == 1 && self.variant_pattern_covers(fields)
            }
        }
    }

    /// Matrix-based exhaustiveness over the documented pattern language.
    /// Finite types (Bool, Unit, enums) enumerate constructors; Int, Str, and
    /// Any require a wildcard/binding in that column. Rows are the unguarded
    /// arms; guards never contribute coverage.
    fn patterns_exhaustive(&self, scrutinee: &Type, patterns: &[&Pattern]) -> bool {
        let mut rows = Vec::new();
        for pattern in patterns {
            for alternative in cover_alternatives(pattern) {
                rows.push(vec![alternative]);
            }
        }
        self.matrix_exhaustive(std::slice::from_ref(scrutinee), &rows)
    }

    fn matrix_exhaustive(&self, types: &[Type], rows: &[Vec<CoverPattern>]) -> bool {
        if types.is_empty() {
            return !rows.is_empty();
        }
        if rows.is_empty() {
            return false;
        }
        // An all-wildcard row covers every remaining tuple. This is also the
        // termination condition for recursive enum types: without it, a
        // wildcard payload would expand forever.
        if rows.iter().any(|row| {
            row.len() == types.len() && row.iter().all(|p| matches!(p, CoverPattern::Wild))
        }) {
            return true;
        }
        let ty = &types[0];
        let Some(constructors) = self.constructors_for(ty) else {
            // Infinite or unknown domain: only a wildcard row can cover it.
            let specialized: Vec<Vec<CoverPattern>> = rows
                .iter()
                .filter_map(|row| match row.first() {
                    Some(CoverPattern::Wild) => Some(row[1..].to_vec()),
                    _ => None,
                })
                .collect();
            if specialized.is_empty() {
                return false;
            }
            return self.matrix_exhaustive(&types[1..], &specialized);
        };

        for constructor in constructors {
            let mut specialized = Vec::new();
            for row in rows {
                match row.first() {
                    Some(CoverPattern::Wild) => {
                        let mut next = vec![CoverPattern::Wild; constructor.field_types.len()];
                        next.extend_from_slice(&row[1..]);
                        specialized.push(next);
                    }
                    Some(CoverPattern::Literal(value)) if constructor.matches_bool(value) => {
                        specialized.push(row[1..].to_vec());
                    }
                    Some(CoverPattern::Variant(name, fields)) if constructor.name == *name => {
                        let mut next = fields.clone();
                        next.extend_from_slice(&row[1..]);
                        specialized.push(next);
                    }
                    Some(CoverPattern::Tuple(fields)) if constructor.tuple => {
                        let mut next = fields.clone();
                        next.extend_from_slice(&row[1..]);
                        specialized.push(next);
                    }
                    _ => {}
                }
            }
            let mut next_types = constructor.field_types;
            next_types.extend_from_slice(&types[1..]);
            if !self.matrix_exhaustive(&next_types, &specialized) {
                return false;
            }
        }
        true
    }

    fn constructors_for(&self, ty: &Type) -> Option<Vec<Constructor>> {
        match ty {
            Type::Bool => Some(vec![
                Constructor {
                    name: String::new(),
                    bool_value: Some(true),
                    tuple: false,
                    field_types: Vec::new(),
                },
                Constructor {
                    name: String::new(),
                    bool_value: Some(false),
                    tuple: false,
                    field_types: Vec::new(),
                },
            ]),
            Type::Unit => Some(vec![Constructor {
                name: String::new(),
                bool_value: None,
                tuple: false,
                field_types: Vec::new(),
            }]),
            Type::Enum(name) => self.enums.get(name).map(|info| {
                info.variants
                    .iter()
                    .map(|variant| Constructor {
                        name: variant.name.clone(),
                        bool_value: None,
                        tuple: false,
                        field_types: variant
                            .fields
                            .iter()
                            .map(|field| field.ty.clone())
                            .collect(),
                    })
                    .collect()
            }),
            Type::Tuple(elements) => Some(vec![Constructor {
                name: String::new(),
                bool_value: None,
                tuple: true,
                field_types: elements.clone(),
            }]),
            _ => None,
        }
    }

    fn check_lvalue(&mut self, target: &Expr) -> Type {
        match &target.kind {
            ExprKind::Var(name) => {
                let binding = self
                    .lookup(name)
                    .map(|binding| (binding.mutable, binding.ty.clone()));
                match binding {
                    Some((mutable, ty)) => {
                        if !mutable {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0203",
                                    format!("cannot assign to immutable binding `{name}`"),
                                    target.span,
                                )
                                .with_help("declare it with `var` instead of `let`"),
                            );
                        }
                        ty
                    }
                    None => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0201",
                                format!("undefined variable `{name}`"),
                                target.span,
                            )
                            .with_help("declare it with `let` or `var` first"),
                        );
                        Type::Any
                    }
                }
            }
            ExprKind::Field { base, name, .. } => {
                let base_ty = self.check_expr(base, None);
                match &base_ty {
                    Type::Struct(struct_name) => self
                        .structs
                        .get(struct_name)
                        .and_then(|info| info.field(name))
                        .map(|field| field.ty.clone())
                        .unwrap_or_else(|| {
                            self.diagnostics.push(Diagnostic::error(
                                "E0304",
                                format!("struct `{struct_name}` has no field `{name}`"),
                                target.span,
                            ));
                            Type::Any
                        }),
                    Type::Tuple(elements) => {
                        let element = name
                            .parse::<usize>()
                            .ok()
                            .and_then(|index| elements.get(index))
                            .cloned();
                        match element {
                            Some(element) => {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        "E0306",
                                        "tuples are immutable and their elements cannot be assigned",
                                        target.span,
                                    )
                                    .with_help(
                                        "build a new tuple instead, or use a struct for mutable fields",
                                    ),
                                );
                                element
                            }
                            None => {
                                self.diagnostics.push(Diagnostic::error(
                                    "E0304",
                                    format!("tuple `{base_ty}` has no element `{name}`"),
                                    target.span,
                                ));
                                Type::Any
                            }
                        }
                    }
                    Type::Any => Type::Any,
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0304",
                            format!("type `{other}` has no fields"),
                            target.span,
                        ));
                        Type::Any
                    }
                }
            }
            ExprKind::Index { base, index } => {
                let base_ty = self.check_expr(base, None);
                match &base_ty {
                    Type::List(inner) => {
                        let index_ty = self.check_expr(index, Some(&Type::Int));
                        self.expect(&index_ty, &Type::Int, index.span, " as a list index");
                        (**inner).clone()
                    }
                    Type::Map(key, value) => {
                        let index_ty = self.check_expr(index, Some(&**key));
                        self.expect(&index_ty, key, index.span, " as a map key");
                        (**value).clone()
                    }
                    Type::Str => {
                        let index_ty = self.check_expr(index, Some(&Type::Int));
                        self.expect(&index_ty, &Type::Int, index.span, " as a string index");
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0306",
                                "strings are immutable and cannot be assigned by index",
                                target.span,
                            )
                            .with_help("build a new string instead"),
                        );
                        Type::Any
                    }
                    Type::Any => {
                        self.check_expr(index, None);
                        Type::Any
                    }
                    other => {
                        self.check_expr(index, None);
                        self.diagnostics.push(Diagnostic::error(
                            "E0306",
                            format!("type `{other}` cannot be indexed"),
                            target.span,
                        ));
                        Type::Any
                    }
                }
            }
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "E0103",
                    "invalid assignment target",
                    target.span,
                ));
                Type::Any
            }
        }
    }

    // ---- expressions ----

    fn check_expr(&mut self, expr: &Expr, expected: Option<&Type>) -> Type {
        // Only the outermost expression of a statement position may be `?`.
        // Every recursive check starts with the flag cleared.
        let try_allowed = std::mem::take(&mut self.try_allowed);
        let ty = self.check_expr_inner(expr, expected, try_allowed);
        self.expr_types.insert(expr.id, ty.clone());
        ty
    }

    /// Checks an expression that may be a postfix `?` propagation.
    fn check_statement_expr(&mut self, expr: &Expr, expected: Option<&Type>) -> Type {
        self.try_allowed = true;
        self.check_expr(expr, expected)
    }

    fn check_expr_inner(
        &mut self,
        expr: &Expr,
        expected: Option<&Type>,
        try_allowed: bool,
    ) -> Type {
        match &expr.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Str(parts) => {
                for part in parts {
                    if let StrPart::Expr(inner) = part {
                        self.check_expr(inner, None);
                    }
                }
                Type::Str
            }
            ExprKind::Var(name) => {
                if let Some(binding) = self.lookup(name) {
                    binding.ty.clone()
                } else if is_builtin(name) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0311",
                            format!("built-in function `{name}` cannot be used as a value"),
                            expr.span,
                        )
                        .with_help("call it, for example `print(...)`"),
                    );
                    Type::Any
                } else if self.functions.contains_key(name) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0311",
                            format!("function `{name}` cannot be used as a value yet"),
                            expr.span,
                        )
                        .with_help("call it instead"),
                    );
                    Type::Any
                } else if self.structs.contains_key(name) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0311",
                            format!("struct `{name}` cannot be used as a value"),
                            expr.span,
                        )
                        .with_help("construct it, for example `Point(1, 2)`"),
                    );
                    Type::Any
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0201",
                            format!("undefined variable `{name}`"),
                            expr.span,
                        )
                        .with_help("declare it with `let` or `var` first"),
                    );
                    Type::Any
                }
            }
            ExprKind::List(elements) => {
                let expected_elem = match expected {
                    Some(Type::List(inner)) => Some((**inner).clone()),
                    _ => None,
                };
                let mut elem_ty: Option<Type> = None;
                for element in elements {
                    let ty = self.check_expr(element, expected_elem.as_ref());
                    if let Some(expected) = &expected_elem {
                        self.expect(&ty, expected, element.span, " in list literal");
                    }
                    elem_ty = Some(match elem_ty {
                        None => ty,
                        Some(previous) if previous == ty => previous,
                        Some(_) => Type::Any,
                    });
                }
                let final_ty = expected_elem.or(elem_ty).unwrap_or(Type::Any);
                Type::List(Box::new(final_ty))
            }
            ExprKind::Map(entries) => {
                let (expected_key, expected_value) = match expected {
                    Some(Type::Map(key, value)) => (Some((**key).clone()), Some((**value).clone())),
                    _ => (None, None),
                };
                let mut key_ty: Option<Type> = None;
                let mut value_ty: Option<Type> = None;
                for (key, value) in entries {
                    let kt = self.check_expr(key, expected_key.as_ref());
                    if let Some(expected) = &expected_key {
                        self.expect(&kt, expected, key.span, " in map key");
                    }
                    key_ty = Some(match key_ty {
                        None => kt,
                        Some(previous) if previous == kt => previous,
                        Some(_) => Type::Any,
                    });
                    let vt = self.check_expr(value, expected_value.as_ref());
                    if let Some(expected) = &expected_value {
                        self.expect(&vt, expected, value.span, " in map value");
                    }
                    value_ty = Some(match value_ty {
                        None => vt,
                        Some(previous) if previous == vt => previous,
                        Some(_) => Type::Any,
                    });
                }
                Type::Map(
                    Box::new(expected_key.or(key_ty).unwrap_or(Type::Any)),
                    Box::new(expected_value.or(value_ty).unwrap_or(Type::Any)),
                )
            }
            ExprKind::Tuple(elements) => {
                let expected_elements = match expected {
                    Some(Type::Tuple(types)) if types.len() == elements.len() => Some(types),
                    _ => None,
                };
                if let Some(expected_ty) = expected
                    && let Type::Tuple(types) = expected_ty
                    && types.len() != elements.len()
                {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0301",
                            format!(
                                "tuple literal has {} element(s), but expected `{expected_ty}`",
                                elements.len()
                            ),
                            expr.span,
                        )
                        .with_help("write a tuple with the expected number of elements"),
                    );
                }
                let mut types = Vec::with_capacity(elements.len());
                for (index, element) in elements.iter().enumerate() {
                    let expected_element = expected_elements.and_then(|types| types.get(index));
                    let ty = self.check_expr(element, expected_element);
                    if let Some(expected_element) = expected_element {
                        self.expect(&ty, expected_element, element.span, " in tuple element");
                    }
                    // Adopt the expected element type, as list and map
                    // literals do, so an enclosing parameter, return, or
                    // binding check does not report the same mismatch twice.
                    types.push(expected_element.cloned().unwrap_or(ty));
                }
                Type::Tuple(types)
            }
            ExprKind::Try { expr: inner } => {
                if !try_allowed {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0319",
                            "`?` is only valid as a `let` initializer, a plain assignment value, or an expression statement",
                            expr.span,
                        )
                        .with_help(
                            "bind the result first, for example `let value = call()?`",
                        ),
                    );
                    self.check_expr(inner, None);
                    return Type::Any;
                }
                let operand = self.check_expr(inner, None);
                self.check_try_operand(&operand, expr.span)
            }
            ExprKind::Unary { op, expr: inner } => {
                let ty = self.check_expr(inner, None);
                match op {
                    UnOp::Neg => match ty {
                        Type::Int => Type::Int,
                        Type::Float => Type::Float,
                        Type::Any => Type::Any,
                        other => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0306",
                                    format!("cannot negate a value of type `{other}`"),
                                    expr.span,
                                )
                                .with_help("negation is defined for Int and Float"),
                            );
                            Type::Any
                        }
                    },
                    UnOp::Not => {
                        self.expect(&ty, &Type::Bool, expr.span, " after `not`");
                        Type::Bool
                    }
                }
            }
            ExprKind::Binary { op, lhs, rhs } => match op {
                BinOp::And | BinOp::Or => {
                    let lt = self.check_expr(lhs, Some(&Type::Bool));
                    self.expect(
                        &lt,
                        &Type::Bool,
                        lhs.span,
                        &format!(" before `{}`", op.symbol()),
                    );
                    let rt = self.check_expr(rhs, Some(&Type::Bool));
                    self.expect(
                        &rt,
                        &Type::Bool,
                        rhs.span,
                        &format!(" after `{}`", op.symbol()),
                    );
                    Type::Bool
                }
                BinOp::Eq | BinOp::NotEq => {
                    let lt = self.check_expr(lhs, None);
                    let rt = self.check_expr(rhs, None);
                    if !lt.assignable_to(&rt) && !rt.assignable_to(&lt) {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0306",
                                format!(
                                    "cannot compare `{lt}` with `{rt}` using `{}`",
                                    op.symbol()
                                ),
                                expr.span,
                            )
                            .with_help("compare values of the same type"),
                        );
                    }
                    Type::Bool
                }
                BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                    let lt = self.check_expr(lhs, None);
                    let rt = self.check_expr(rhs, None);
                    let valid = matches!(
                        (&lt, &rt),
                        (Type::Int, Type::Int)
                            | (Type::Float, Type::Float)
                            | (Type::Str, Type::Str)
                    ) || lt.is_any()
                        || rt.is_any();
                    if !valid {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0306",
                                format!("cannot order `{lt}` and `{rt}` with `{}`", op.symbol()),
                                expr.span,
                            )
                            .with_help("ordering is defined for Int, Float, and Str"),
                        );
                    }
                    Type::Bool
                }
                _ => {
                    let lt = self.check_expr(lhs, None);
                    let rt = self.check_expr(rhs, None);
                    self.binary_type(*op, &lt, &rt, expr.span)
                }
            },
            ExprKind::Call { callee, args } => {
                let ExprKind::Var(name) = &callee.kind else {
                    self.check_expr(callee, None);
                    for arg in args {
                        self.check_expr(&arg.value, None);
                    }
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0310",
                            "only named functions and struct constructors can be called",
                            expr.span,
                        )
                        .with_help("function values are not implemented yet"),
                    );
                    return Type::Any;
                };
                self.expr_types.insert(callee.id, Type::Any);

                if is_builtin(name) {
                    return self.check_builtin(name, args, expr.span);
                }
                if let Some(sig) = self.functions.get(name).cloned() {
                    if args.len() != sig.params.len() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0302",
                                format!(
                                    "function `{name}` expects {} argument(s), found {}",
                                    sig.params.len(),
                                    args.len()
                                ),
                                expr.span,
                            )
                            .with_help("check the parameter list"),
                        );
                    }
                    for (arg, param) in args.iter().zip(sig.params.iter()) {
                        if let Some((arg_name, arg_span)) = &arg.name {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0313",
                                    format!("named argument `{arg_name}` is not valid for function calls"),
                                    *arg_span,
                                )
                                .with_help("named arguments are only supported for struct constructors"),
                            );
                        }
                        let ty = self.check_expr(&arg.value, Some(&param.ty));
                        self.expect(
                            &ty,
                            &param.ty,
                            arg.span,
                            &format!(" for parameter `{}`", param.name),
                        );
                    }
                    for arg in args.iter().skip(sig.params.len()) {
                        self.check_expr(&arg.value, None);
                    }
                    return sig.ret;
                }
                if let Some((enum_name, variant)) = self
                    .find_variant(name)
                    .map(|(enum_name, variant)| (enum_name.to_string(), variant.clone()))
                {
                    return self.check_variant_constructor(&enum_name, &variant, args, expr.span);
                }
                if self.structs.contains_key(name) {
                    return self.check_constructor(name, args, expr.span);
                }
                for arg in args {
                    self.check_expr(&arg.value, None);
                }
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0310",
                        format!("unknown function, variant, or struct `{name}`"),
                        expr.span,
                    )
                    .with_help("declare it before calling it"),
                );
                Type::Any
            }
            ExprKind::Index { base, index } => {
                let base_ty = self.check_expr(base, None);
                match &base_ty {
                    Type::List(inner) => {
                        let index_ty = self.check_expr(index, Some(&Type::Int));
                        self.expect(&index_ty, &Type::Int, index.span, " as a list index");
                        (**inner).clone()
                    }
                    Type::Str => {
                        let index_ty = self.check_expr(index, Some(&Type::Int));
                        self.expect(&index_ty, &Type::Int, index.span, " as a string index");
                        Type::Str
                    }
                    Type::Map(key, value) => {
                        let index_ty = self.check_expr(index, Some(&**key));
                        self.expect(&index_ty, key, index.span, " as a map key");
                        (**value).clone()
                    }
                    Type::Any => {
                        self.check_expr(index, None);
                        Type::Any
                    }
                    other => {
                        self.check_expr(index, None);
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0306",
                                format!("type `{other}` cannot be indexed"),
                                expr.span,
                            )
                            .with_help("indexing is defined for List, Str, and Map"),
                        );
                        Type::Any
                    }
                }
            }
            ExprKind::Field { base, name, .. } => {
                let base_ty = self.check_expr(base, None);
                match &base_ty {
                    Type::Struct(struct_name) => self
                        .structs
                        .get(struct_name)
                        .and_then(|info| info.field(name))
                        .map(|field| field.ty.clone())
                        .unwrap_or_else(|| {
                            self.diagnostics.push(Diagnostic::error(
                                "E0304",
                                format!("struct `{struct_name}` has no field `{name}`"),
                                expr.span,
                            ));
                            Type::Any
                        }),
                    Type::Tuple(elements) => match name.parse::<usize>() {
                        Ok(index) if index < elements.len() => elements[index].clone(),
                        Ok(index) => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0304",
                                    format!("tuple `{base_ty}` has no element at index {index}"),
                                    expr.span,
                                )
                                .with_help(format!(
                                    "the valid indices are 0 through {}",
                                    elements.len().saturating_sub(1)
                                )),
                            );
                            Type::Any
                        }
                        Err(_) => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E0304",
                                    format!("tuple `{base_ty}` has no field `{name}`"),
                                    expr.span,
                                )
                                .with_help(
                                    "tuple elements are accessed by numeric index, such as `.0`",
                                ),
                            );
                            Type::Any
                        }
                    },
                    Type::Any => Type::Any,
                    Type::Enum(name) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0304",
                                format!("enum `{name}` has no fields"),
                                expr.span,
                            )
                            .with_help("use `match` to access variant payload values"),
                        );
                        Type::Any
                    }
                    other => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0304",
                                format!("type `{other}` has no fields"),
                                expr.span,
                            )
                            .with_help("field access requires a struct or tuple value"),
                        );
                        Type::Any
                    }
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                self.check_match(scrutinee, arms, expected, true)
            }
        }
    }

    /// Validates the operand of postfix `?` and returns the success payload
    /// type. The convention is positional: the enum must have exactly two
    /// variants with one payload field each. The first declared variant is the
    /// success case and the second is the failure case. The enclosing function
    /// must return the same enum, so a failure value can be returned unchanged.
    fn check_try_operand(&mut self, operand: &Type, span: Span) -> Type {
        let Type::Enum(name) = operand else {
            if !matches!(operand, Type::Any) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0319",
                        format!("`?` requires a two-variant enum, found `{operand}`"),
                        span,
                    )
                    .with_help("declare a result enum such as `enum Outcome: Ok(Int), Err(Str)`"),
                );
            }
            return Type::Any;
        };
        let Some(info) = self.enums.get(name).cloned() else {
            return Type::Any;
        };
        if info.variants.len() != 2 {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0319",
                    format!(
                        "`?` requires an enum with exactly two variants, but `{name}` has {}",
                        info.variants.len()
                    ),
                    span,
                )
                .with_help(
                    "the first variant is the success case and the second is the failure case",
                ),
            );
            return Type::Any;
        }
        let success = &info.variants[0];
        let failure = &info.variants[1];
        if success.fields.len() != 1 || failure.fields.len() != 1 {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0319",
                    format!("`?` requires one payload field on each variant of `{name}`"),
                    span,
                )
                .with_help("declare both variants with exactly one field"),
            );
            return Type::Any;
        }
        let Some((function, returns, annotated)) = self.current_fn.as_ref().map(|context| {
            (
                context.name.clone(),
                context.ret.clone(),
                context.ret_annotated,
            )
        }) else {
            self.diagnostics.push(
                Diagnostic::error("E0319", "`?` is only valid inside a function", span)
                    .with_help("top-level code has nowhere to propagate the failure value"),
            );
            return Type::Any;
        };
        if !annotated {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0319",
                    format!("`?` requires an explicit return type on `{function}`"),
                    span,
                )
                .with_help(format!("annotate it as `-> {name}`")),
            );
            return Type::Any;
        }
        if returns != *operand {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0319",
                    format!("`?` propagates `{name}`, but `{function}` returns `{returns}`"),
                    span,
                )
                .with_help("the enclosing function must return the same enum type"),
            );
            return Type::Any;
        }
        success.fields[0].ty.clone()
    }

    fn binary_type(&mut self, op: BinOp, lhs: &Type, rhs: &Type, span: Span) -> Type {
        use Type::{Any, Float, Int, Str};
        let mismatch = |checker: &mut Self| {
            checker.diagnostics.push(
                Diagnostic::error(
                    "E0306",
                    format!(
                        "operator `{}` cannot be applied to `{}` and `{}`",
                        op.symbol(),
                        lhs,
                        rhs
                    ),
                    span,
                )
                .with_help("both operands must have the same numeric type"),
            );
            Type::Any
        };
        match op {
            BinOp::Add => match (lhs, rhs) {
                (Str, Str) => Str,
                (Int, Int) => Int,
                (Float, Float) => Float,
                (Any, _) | (_, Any) => Any,
                (Int, Float) | (Float, Int) => {
                    self.diagnostics.push(
                        Diagnostic::error("E0306", "cannot add `Int` and `Float`", span)
                            .with_help("convert explicitly with `float(...)` or `int(...)`"),
                    );
                    Type::Any
                }
                _ => mismatch(self),
            },
            BinOp::Sub | BinOp::Mul | BinOp::FloorDiv | BinOp::Mod | BinOp::Pow => {
                match (lhs, rhs) {
                    (Int, Int) => Int,
                    (Float, Float) if op != BinOp::FloorDiv => Float,
                    (Any, _) | (_, Any) => Any,
                    (Float, Float) => {
                        self.diagnostics.push(
                            Diagnostic::error("E0306", "`//` is defined for Int operands", span)
                                .with_help("use `floor(a / b)` for floats"),
                        );
                        Type::Any
                    }
                    (Int, Float) | (Float, Int) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0306",
                                format!("operator `{}` cannot mix `Int` and `Float`", op.symbol()),
                                span,
                            )
                            .with_help("convert explicitly with `float(...)` or `int(...)`"),
                        );
                        Type::Any
                    }
                    _ => mismatch(self),
                }
            }
            BinOp::Div => match (lhs, rhs) {
                (Int, Int) | (Float, Float) => Float,
                (Any, _) | (_, Any) => Float,
                (Int, Float) | (Float, Int) => {
                    self.diagnostics.push(
                        Diagnostic::error("E0306", "division cannot mix `Int` and `Float`", span)
                            .with_help("convert explicitly with `float(...)` or `int(...)`"),
                    );
                    Type::Any
                }
                _ => mismatch(self),
            },
            _ => mismatch(self),
        }
    }

    fn check_constructor(&mut self, name: &str, args: &[Arg], span: Span) -> Type {
        let Some(info) = self.structs.get(name).cloned() else {
            return Type::Any;
        };
        let any_named = args.iter().any(|arg| arg.name.is_some());
        let all_named = args.iter().all(|arg| arg.name.is_some());
        if any_named && !all_named {
            self.diagnostics.push(
                Diagnostic::error("E0313", "cannot mix named and positional arguments", span)
                    .with_help("use all named or all positional arguments"),
            );
        }

        if any_named {
            let mut seen = HashSet::new();
            for arg in args {
                let Some((arg_name, arg_span)) = &arg.name else {
                    self.check_expr(&arg.value, None);
                    continue;
                };
                if !seen.insert(arg_name.clone()) {
                    self.diagnostics.push(Diagnostic::error(
                        "E0312",
                        format!("duplicate argument `{arg_name}`"),
                        *arg_span,
                    ));
                }
                match info.field(arg_name) {
                    Some(field) => {
                        let ty = self.check_expr(&arg.value, Some(&field.ty));
                        self.expect(
                            &ty,
                            &field.ty,
                            arg.span,
                            &format!(" for field `{arg_name}`"),
                        );
                    }
                    None => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0304",
                                format!("struct `{name}` has no field `{arg_name}`"),
                                *arg_span,
                            )
                            .with_help("check the struct declaration"),
                        );
                        self.check_expr(&arg.value, None);
                    }
                }
            }
            for field in &info.fields {
                if !seen.contains(&field.name) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0305",
                            format!("missing field `{}` for struct `{name}`", field.name),
                            span,
                        )
                        .with_help("every field must be provided"),
                    );
                }
            }
        } else {
            if args.len() != info.fields.len() {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0302",
                        format!(
                            "struct `{name}` expects {} field(s), found {}",
                            info.fields.len(),
                            args.len()
                        ),
                        span,
                    )
                    .with_help("provide every field in declaration order"),
                );
            }
            for (arg, field) in args.iter().zip(info.fields.iter()) {
                let ty = self.check_expr(&arg.value, Some(&field.ty));
                self.expect(
                    &ty,
                    &field.ty,
                    arg.span,
                    &format!(" for field `{}`", field.name),
                );
            }
            for arg in args.iter().skip(info.fields.len()) {
                self.check_expr(&arg.value, None);
            }
        }
        Type::Struct(name.to_string())
    }

    fn check_variant_constructor(
        &mut self,
        enum_name: &str,
        variant: &VariantInfo,
        args: &[Arg],
        span: Span,
    ) -> Type {
        for arg in args {
            if let Some((arg_name, arg_span)) = &arg.name {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0313",
                        format!(
                            "named argument `{arg_name}` is not valid for variant constructors"
                        ),
                        *arg_span,
                    )
                    .with_help("variant payloads are positional; pass values in declaration order"),
                );
            }
        }
        if args.len() != variant.fields.len() {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0302",
                    format!(
                        "variant `{}` expects {} payload value(s), found {}",
                        variant.name,
                        variant.fields.len(),
                        args.len()
                    ),
                    span,
                )
                .with_help("provide every payload value in declaration order"),
            );
        }
        for (arg, field) in args.iter().zip(variant.fields.iter()) {
            let ty = self.check_expr(&arg.value, Some(&field.ty));
            self.expect(
                &ty,
                &field.ty,
                arg.span,
                &format!(" for payload field `{}`", field.name),
            );
        }
        for arg in args.iter().skip(variant.fields.len()) {
            self.check_expr(&arg.value, None);
        }
        Type::Enum(enum_name.to_string())
    }

    fn check_builtin(&mut self, name: &str, args: &[Arg], span: Span) -> Type {
        for arg in args {
            if let Some((_, arg_span)) = &arg.name {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0313",
                        format!("built-in `{name}` does not accept named arguments"),
                        *arg_span,
                    )
                    .with_help("pass arguments positionally"),
                );
            }
        }

        let arity = |checker: &mut Self, min: usize, max: usize| -> bool {
            if args.len() < min || args.len() > max {
                let expected = if min == max {
                    format!("{min}")
                } else {
                    format!("{min} to {max}")
                };
                checker.diagnostics.push(
                    Diagnostic::error(
                        "E0302",
                        format!(
                            "`{name}` expects {expected} argument(s), found {}",
                            args.len()
                        ),
                        span,
                    )
                    .with_help("check the built-in function signature"),
                );
                false
            } else {
                true
            }
        };

        let arg_ty = |checker: &mut Self, index: usize, expected: Option<&Type>| -> Type {
            match args.get(index) {
                Some(arg) => checker.check_expr(&arg.value, expected),
                None => Type::Any,
            }
        };

        match name {
            "print" => {
                for arg in args {
                    self.check_expr(&arg.value, None);
                }
                Type::Unit
            }
            "len" => {
                if !arity(self, 1, 1) {
                    return Type::Int;
                }
                let ty = arg_ty(self, 0, None);
                if !matches!(ty, Type::Str | Type::List(_) | Type::Map(_, _) | Type::Any) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0301",
                            format!("`len` expects Str, List, or Map, found `{ty}`"),
                            args[0].span,
                        )
                        .with_help("pass a collection or string"),
                    );
                }
                Type::Int
            }
            "range" => {
                if !arity(self, 1, 3) {
                    return Type::List(Box::new(Type::Int));
                }
                for (index, arg) in args.iter().take(3).enumerate() {
                    let ty = arg_ty(self, index, Some(&Type::Int));
                    self.expect(&ty, &Type::Int, arg.span, " as a range bound");
                }
                Type::List(Box::new(Type::Int))
            }
            "push" => {
                if !arity(self, 2, 2) {
                    return Type::Unit;
                }
                // A direct empty list literal can take its element type from
                // the value being pushed instead of defaulting to `List[Any]`,
                // which the native backend cannot represent.
                if matches!(&args[0].value.kind, ExprKind::List(elements) if elements.is_empty()) {
                    let value_ty = arg_ty(self, 1, None);
                    let expected = Type::List(Box::new(value_ty));
                    arg_ty(self, 0, Some(&expected));
                    return Type::Unit;
                }
                let list_ty = arg_ty(self, 0, None);
                let element_ty = match &list_ty {
                    Type::List(inner) => Some((**inner).clone()),
                    Type::Any => None,
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0301",
                            format!(
                                "`push` expects a List as its first argument, found `{list_ty}`"
                            ),
                            args[0].span,
                        ));
                        None
                    }
                };
                let value_ty = arg_ty(self, 1, element_ty.as_ref());
                if let Some(element_ty) = &element_ty {
                    self.expect(&value_ty, element_ty, args[1].span, " for `push`");
                }
                Type::Unit
            }
            "pop" => {
                if !arity(self, 1, 1) {
                    return Type::Any;
                }
                let list_ty = arg_ty(self, 0, None);
                match list_ty {
                    Type::List(inner) => *inner,
                    Type::Any => Type::Any,
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0301",
                            format!("`pop` expects a List, found `{other}`"),
                            args[0].span,
                        ));
                        Type::Any
                    }
                }
            }
            "contains" => {
                if !arity(self, 2, 2) {
                    return Type::Bool;
                }
                // As with `join`, a direct empty list literal has no element
                // type of its own. The needle supplies it, so the literal can
                // be checked as `List[needle]` instead of `List[Any]`.
                if matches!(&args[0].value.kind, ExprKind::List(elements) if elements.is_empty()) {
                    let needle_ty = arg_ty(self, 1, None);
                    let expected = Type::List(Box::new(needle_ty));
                    arg_ty(self, 0, Some(&expected));
                    return Type::Bool;
                }
                let collection_ty = arg_ty(self, 0, None);
                match &collection_ty {
                    Type::List(inner) => {
                        let needle_ty = arg_ty(self, 1, Some(inner));
                        self.expect(
                            &needle_ty,
                            inner,
                            args[1].span,
                            " as the needle for `contains`",
                        );
                    }
                    Type::Map(key, _) => {
                        let needle_ty = arg_ty(self, 1, Some(key));
                        self.expect(&needle_ty, key, args[1].span, " as the key for `contains`");
                    }
                    Type::Str => {
                        let needle_ty = arg_ty(self, 1, Some(&Type::Str));
                        self.expect(
                            &needle_ty,
                            &Type::Str,
                            args[1].span,
                            " as the needle for `contains`",
                        );
                    }
                    Type::Any => {
                        arg_ty(self, 1, None);
                    }
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0301",
                            format!("`contains` expects List, Str, or Map, found `{other}`"),
                            args[0].span,
                        ));
                        arg_ty(self, 1, None);
                    }
                }
                Type::Bool
            }
            "remove" => {
                if !arity(self, 2, 2) {
                    return Type::Bool;
                }
                let map_ty = arg_ty(self, 0, None);
                match &map_ty {
                    Type::Map(key, _) => {
                        // The key argument gets the map's key type as its
                        // expected type, so a direct empty list literal can
                        // adopt a `List[T]` key type instead of defaulting to
                        // `List[Any]`, which the native backend cannot
                        // represent.
                        let key_ty = arg_ty(self, 1, Some(key));
                        self.expect(&key_ty, key, args[1].span, " as the key for `remove`");
                    }
                    Type::Any => {
                        arg_ty(self, 1, None);
                    }
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0301",
                            format!("`remove` expects a Map, found `{other}`"),
                            args[0].span,
                        ));
                        arg_ty(self, 1, None);
                    }
                }
                Type::Bool
            }
            "sorted" => {
                if !arity(self, 1, 1) {
                    return Type::List(Box::new(Type::Any));
                }
                let list_ty = arg_ty(self, 0, None);
                match &list_ty {
                    Type::List(inner)
                        if matches!(**inner, Type::Int | Type::Float | Type::Str | Type::Bool) =>
                    {
                        Type::List(inner.clone())
                    }
                    Type::List(inner) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!(
                                    "`sorted` requires a List of Int, Float, Str, or Bool, found `{list_ty}`"
                                ),
                                args[0].span,
                            )
                            .with_help(
                                "sort a homogeneous scalar list; structural values have no defined order",
                            ),
                        );
                        Type::List(inner.clone())
                    }
                    Type::Any => Type::Any,
                    other => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!("`sorted` expects a List, found `{other}`"),
                                args[0].span,
                            )
                            .with_help("pass a list of Int, Float, Str, or Bool"),
                        );
                        Type::List(Box::new(Type::Any))
                    }
                }
            }
            "reverse" => {
                if !arity(self, 1, 1) {
                    return Type::List(Box::new(Type::Any));
                }
                let list_ty = arg_ty(self, 0, None);
                match &list_ty {
                    Type::List(inner) => Type::List(inner.clone()),
                    Type::Any => Type::Any,
                    other => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!("`reverse` expects a List, found `{other}`"),
                                args[0].span,
                            )
                            .with_help("pass a list; reversal preserves its element type"),
                        );
                        Type::List(Box::new(Type::Any))
                    }
                }
            }
            "split" => {
                if !arity(self, 2, 2) {
                    return Type::List(Box::new(Type::Str));
                }
                let text_ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &text_ty,
                    &Type::Str,
                    args[0].span,
                    " as the text for `split`",
                );
                let separator_ty = arg_ty(self, 1, Some(&Type::Str));
                self.expect(
                    &separator_ty,
                    &Type::Str,
                    args[1].span,
                    " as the separator for `split`",
                );
                Type::List(Box::new(Type::Str))
            }
            "join" => {
                if !arity(self, 2, 2) {
                    return Type::Str;
                }
                // `join` has one unambiguous `List[Str]` parameter, so an
                // empty list literal can take that type instead of defaulting
                // to `List[Any]`, which the native backend cannot represent.
                let list_expected = Type::List(Box::new(Type::Str));
                let list_ty = match &args[0].value.kind {
                    ExprKind::List(elements) if elements.is_empty() => {
                        arg_ty(self, 0, Some(&list_expected))
                    }
                    _ => arg_ty(self, 0, None),
                };
                match &list_ty {
                    Type::List(inner) if **inner == Type::Str || inner.is_any() => {}
                    Type::Any => {}
                    Type::List(inner) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E0301",
                                format!("`join` expects List[Str], found `List[{inner}]`"),
                                args[0].span,
                            )
                            .with_help("convert elements with `str(...)` first"),
                        );
                    }
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0301",
                            format!("`join` expects List[Str], found `{other}`"),
                            args[0].span,
                        ));
                    }
                }
                let separator_ty = arg_ty(self, 1, Some(&Type::Str));
                self.expect(
                    &separator_ty,
                    &Type::Str,
                    args[1].span,
                    " as the separator for `join`",
                );
                Type::Str
            }
            "trim" => {
                if !arity(self, 1, 1) {
                    return Type::Str;
                }
                let text_ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &text_ty,
                    &Type::Str,
                    args[0].span,
                    " as the text for `trim`",
                );
                Type::Str
            }
            "starts_with" | "ends_with" => {
                if !arity(self, 2, 2) {
                    return Type::Bool;
                }
                let text_ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &text_ty,
                    &Type::Str,
                    args[0].span,
                    &format!(" as the text for `{name}`"),
                );
                let affix_ty = arg_ty(self, 1, Some(&Type::Str));
                self.expect(
                    &affix_ty,
                    &Type::Str,
                    args[1].span,
                    &format!(" as the affix for `{name}`"),
                );
                Type::Bool
            }
            "replace" => {
                if !arity(self, 3, 3) {
                    return Type::Str;
                }
                for (index, context) in [
                    (0, " as the text for `replace`"),
                    (1, " as the pattern for `replace`"),
                    (2, " as the replacement for `replace`"),
                ] {
                    let ty = arg_ty(self, index, Some(&Type::Str));
                    self.expect(&ty, &Type::Str, args[index].span, context);
                }
                Type::Str
            }
            "path_join" => {
                if !arity(self, 2, 2) {
                    return Type::Str;
                }
                let base_ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &base_ty,
                    &Type::Str,
                    args[0].span,
                    " as the base path for `path_join`",
                );
                let child_ty = arg_ty(self, 1, Some(&Type::Str));
                self.expect(
                    &child_ty,
                    &Type::Str,
                    args[1].span,
                    " as the child path for `path_join`",
                );
                Type::Str
            }
            "path_parent" | "path_name" | "path_stem" | "path_extension" => {
                if !arity(self, 1, 1) {
                    return Type::Str;
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    &format!(" as the path for `{name}`"),
                );
                Type::Str
            }
            "path_is_absolute" => {
                if !arity(self, 1, 1) {
                    return Type::Bool;
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    " as the path for `path_is_absolute`",
                );
                Type::Bool
            }
            "keys" | "values" => {
                if !arity(self, 1, 1) {
                    return Type::List(Box::new(Type::Any));
                }
                let map_ty = arg_ty(self, 0, None);
                match map_ty {
                    Type::Map(key, value) => Type::List(if name == "keys" { key } else { value }),
                    Type::Any => Type::List(Box::new(Type::Any)),
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0301",
                            format!("`{name}` expects a Map, found `{other}`"),
                            args[0].span,
                        ));
                        Type::List(Box::new(Type::Any))
                    }
                }
            }
            "str" => {
                if !arity(self, 1, 1) {
                    return Type::Str;
                }
                arg_ty(self, 0, None);
                Type::Str
            }
            "int" => {
                if !arity(self, 1, 1) {
                    return Type::Int;
                }
                arg_ty(self, 0, None);
                Type::Int
            }
            "float" => {
                if !arity(self, 1, 1) {
                    return Type::Float;
                }
                arg_ty(self, 0, None);
                Type::Float
            }
            "bool" => {
                if !arity(self, 1, 1) {
                    return Type::Bool;
                }
                arg_ty(self, 0, None);
                Type::Bool
            }
            "type" => {
                if !arity(self, 1, 1) {
                    return Type::Str;
                }
                arg_ty(self, 0, None);
                Type::Str
            }
            "abs" => {
                if !arity(self, 1, 1) {
                    return Type::Any;
                }
                let ty = arg_ty(self, 0, None);
                match ty {
                    Type::Int | Type::Float | Type::Any => ty,
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0301",
                            format!("`abs` expects Int or Float, found `{other}`"),
                            args[0].span,
                        ));
                        Type::Any
                    }
                }
            }
            "min" | "max" => {
                if !arity(self, 2, 2) {
                    return Type::Any;
                }
                let a = arg_ty(self, 0, None);
                let b = arg_ty(self, 1, None);
                match (&a, &b) {
                    (Type::Int, Type::Int) => Type::Int,
                    (Type::Float, Type::Float) => Type::Float,
                    (Type::Any, _) | (_, Type::Any) => Type::Any,
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0306",
                            format!("`{name}` cannot compare `{a}` and `{b}`"),
                            span,
                        ));
                        Type::Any
                    }
                }
            }
            "sqrt" => {
                if !arity(self, 1, 1) {
                    return Type::Float;
                }
                let ty = arg_ty(self, 0, Some(&Type::Float));
                self.expect(&ty, &Type::Float, args[0].span, " for `sqrt`");
                Type::Float
            }
            "floor" | "ceil" => {
                if !arity(self, 1, 1) {
                    return Type::Int;
                }
                let ty = arg_ty(self, 0, Some(&Type::Float));
                self.expect(&ty, &Type::Float, args[0].span, &format!(" for `{name}`"));
                Type::Int
            }
            "assert" => {
                if !arity(self, 1, 1) {
                    return Type::Unit;
                }
                let ty = arg_ty(self, 0, Some(&Type::Bool));
                self.expect(&ty, &Type::Bool, args[0].span, " for `assert`");
                Type::Unit
            }
            "read_file" => {
                if !arity(self, 1, 1) {
                    return Type::Str;
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    " as a file path for `read_file`",
                );
                Type::Str
            }
            "write_file" | "append_file" | "write_file_atomic" => {
                if !arity(self, 2, 2) {
                    return Type::Unit;
                }
                let path_ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &path_ty,
                    &Type::Str,
                    args[0].span,
                    &format!(" as a file path for `{name}`"),
                );
                let text_ty = arg_ty(self, 1, Some(&Type::Str));
                self.expect(
                    &text_ty,
                    &Type::Str,
                    args[1].span,
                    &format!(" as the contents for `{name}`"),
                );
                Type::Unit
            }
            "list_dir" => {
                if !arity(self, 1, 1) {
                    return Type::List(Box::new(Type::Str));
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    " as a directory path for `list_dir`",
                );
                Type::List(Box::new(Type::Str))
            }
            "glob" => {
                if !arity(self, 1, 1) {
                    return Type::List(Box::new(Type::Str));
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(&ty, &Type::Str, args[0].span, " as the pattern for `glob`");
                Type::List(Box::new(Type::Str))
            }
            "make_dir" | "remove_file" | "remove_dir" | "make_dir_all" | "remove_dir_all" => {
                if !arity(self, 1, 1) {
                    return Type::Unit;
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    &format!(" as a path for `{name}`"),
                );
                Type::Unit
            }
            "rename" | "copy_file" | "copy_dir_all" => {
                if !arity(self, 2, 2) {
                    return Type::Unit;
                }
                let source_ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &source_ty,
                    &Type::Str,
                    args[0].span,
                    &format!(" as the source path for `{name}`"),
                );
                let target_ty = arg_ty(self, 1, Some(&Type::Str));
                self.expect(
                    &target_ty,
                    &Type::Str,
                    args[1].span,
                    &format!(" as the target path for `{name}`"),
                );
                Type::Unit
            }
            "file_size" => {
                if !arity(self, 1, 1) {
                    return Type::Int;
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    " as a file path for `file_size`",
                );
                Type::Int
            }
            "is_file" | "is_dir" | "is_symlink" => {
                if !arity(self, 1, 1) {
                    return Type::Bool;
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    &format!(" as a file path for `{name}`"),
                );
                Type::Bool
            }
            "file_exists" => {
                if !arity(self, 1, 1) {
                    return Type::Bool;
                }
                let ty = arg_ty(self, 0, Some(&Type::Str));
                self.expect(
                    &ty,
                    &Type::Str,
                    args[0].span,
                    " as a file path for `file_exists`",
                );
                Type::Bool
            }
            _ => Type::Any,
        }
    }

    // ---- scopes ----

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn declare(&mut self, name: &str, ty: Type, mutable: bool, span: Span) {
        if is_builtin(name) {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0202",
                    format!("cannot shadow built-in function `{name}`"),
                    span,
                )
                .with_help("choose a different name"),
            );
            return;
        }
        let is_global = self.current_fn.is_none() && self.scopes.len() == 1;
        if is_global && (self.functions.contains_key(name) || self.structs.contains_key(name)) {
            self.diagnostics.push(Diagnostic::error(
                "E0202",
                format!("`{name}` is already declared at the top level"),
                span,
            ));
            return;
        }
        if let Some(previous_span) = self
            .scopes
            .last()
            .and_then(|scope| scope.get(name))
            .map(|binding| binding.span)
        {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0202",
                    format!("duplicate declaration of `{name}` in this scope"),
                    span,
                )
                .with_note(format!(
                    "previous declaration is at byte {}",
                    previous_span.start
                ))
                .with_help("shadowing is allowed only in an inner block"),
            );
            return;
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), Binding { ty, mutable, span });
        }
    }

    fn resolve_type(&mut self, expr: &TypeExpr) -> Type {
        let result = resolve_type_expr(expr, &self.structs, &self.enums);
        match result {
            Ok(ty) => ty,
            Err(message) => {
                self.diagnostics.push(
                    Diagnostic::error("E0303", message, expr.span)
                        .with_help("use a built-in type or a declared struct or enum"),
                );
                Type::Any
            }
        }
    }

    fn expect(&mut self, actual: &Type, expected: &Type, span: Span, context: &str) {
        if actual.assignable_to(expected) {
            return;
        }
        let mut diagnostic = Diagnostic::error(
            "E0301",
            format!("type mismatch{context}: expected `{expected}`, found `{actual}`"),
            span,
        );
        if actual.is_numeric() && expected.is_numeric() {
            diagnostic =
                diagnostic.with_help("use `float(...)` or `int(...)` to convert explicitly");
        }
        self.diagnostics.push(diagnostic);
    }
}

fn infer_return(returns: &[(Type, Span)]) -> Type {
    let Some((first, _)) = returns.first() else {
        return Type::Unit;
    };
    if returns.iter().all(|(ty, _)| ty == first) {
        first.clone()
    } else {
        Type::Any
    }
}

fn block_returns(block: &Block) -> bool {
    // Statements after a `return`, `break`, or `continue` are unreachable and
    // must not turn a returning block into a non-returning one. Walk reachable
    // statements in order and stop at the first terminator.
    for statement in &block.stmts {
        match &statement.kind {
            StmtKind::Return(_) => return true,
            StmtKind::Break | StmtKind::Continue => return false,
            StmtKind::If {
                then_block,
                elifs,
                else_block: Some(else_block),
                ..
            } => {
                if block_returns(then_block)
                    && elifs.iter().all(|(_, block)| block_returns(block))
                    && block_returns(else_block)
                {
                    return true;
                }
            }
            StmtKind::Match { arms, .. }
                if !arms.is_empty()
                    && arms.iter().all(|arm| match &arm.body {
                        MatchArmBody::Block(block) => block_returns(block),
                        MatchArmBody::Expr(_) => false,
                    }) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser;

    fn check_source(source: &str) -> (Option<SemanticModel>, Diagnostics) {
        let lexed = lex(source);
        assert!(!lexed.diagnostics.has_errors(), "{:?}", lexed.diagnostics);
        let (program, parse_diagnostics) = parser::parse(source, lexed.tokens);
        assert!(!parse_diagnostics.has_errors(), "{:?}", parse_diagnostics);
        check(&program)
    }

    fn check_ok(source: &str) -> SemanticModel {
        let (model, diagnostics) = check_source(source);
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
        model.expect("model")
    }

    fn check_errors(source: &str) -> Diagnostics {
        let (model, diagnostics) = check_source(source);
        assert!(diagnostics.has_errors(), "expected errors");
        assert!(model.is_none());
        diagnostics
    }

    #[test]
    fn accepts_a_typed_program() {
        let model = check_ok(
            "struct Point:\n    x: Int\n    y: Int\n\n\
             fn add(a: Int, b: Int) -> Int:\n    return a + b\n\n\
             fn main():\n    let p = Point(x=1, y=2)\n    let n = add(p.x, 2)\n    print(\"n = {n}\")\n",
        );
        assert_eq!(model.entry, Entry::Main);
        assert_eq!(model.functions["add"].ret, Type::Int);
    }

    #[test]
    fn accepts_gradual_unannotated_functions() {
        let model = check_ok("fn add(a, b):\n    return a + b\n\nprint(add(1, 2))\n");
        assert_eq!(model.entry, Entry::TopLevel);
        assert_eq!(model.functions["add"].params[0].ty, Type::Any);
        assert_eq!(model.functions["add"].ret, Type::Any);
    }

    #[test]
    fn rejects_type_mismatch() {
        let diagnostics = check_errors("let x: Int = \"hello\"\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0301"));
    }

    #[test]
    fn tuple_element_mismatch_is_reported_once() {
        for source in [
            "fn f(p: (Int, Str)):\n    print(p)\n\nfn main():\n    f((1, 2))\n",
            "fn main():\n    let (a, b): (Int, Str) = (1, 2)\n    print(a)\n",
        ] {
            let diagnostics = check_errors(source);
            let mismatches: Vec<_> = diagnostics
                .items
                .iter()
                .filter(|diagnostic| diagnostic.code == "E0301")
                .collect();
            assert_eq!(mismatches.len(), 1, "{:?}", diagnostics.items);
            assert!(
                mismatches[0].message.contains("tuple element"),
                "{:?}",
                mismatches[0]
            );
        }
    }

    #[test]
    fn rejects_assignment_to_let() {
        let diagnostics = check_errors("let x = 1\nx = 2\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0203"));
    }

    #[test]
    fn allows_assignment_to_var() {
        let model = check_ok("var x = 1\nx = 2\nprint(x)\n");
        assert_eq!(model.entry, Entry::TopLevel);
    }

    #[test]
    fn rejects_wrong_argument_type() {
        let diagnostics = check_errors("fn takes(a: Int) -> Int:\n    return a\n\ntakes(\"no\")\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0301"));
    }

    #[test]
    fn rejects_wrong_arity() {
        let diagnostics = check_errors("fn takes(a: Int) -> Int:\n    return a\n\ntakes()\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0302"));
    }

    #[test]
    fn rejects_undefined_variable() {
        let diagnostics = check_errors("print(missing)\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0201"));
    }

    #[test]
    fn rejects_missing_return() {
        let diagnostics = check_errors("fn f(x: Bool) -> Int:\n    if x:\n        return 1\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0309"));
    }

    #[test]
    fn dead_code_after_return_still_counts_as_returning() {
        let model = check_ok(
            "fn f() -> Int:\n    return 1\n    print(\"dead\")\n\nfn main():\n    print(f())\n",
        );
        assert_eq!(model.functions["f"].ret, Type::Int);
    }

    #[test]
    fn dead_code_after_a_terminating_if_still_counts_as_returning() {
        check_ok(
            "fn f(x: Bool) -> Int:\n    if x:\n        return 1\n    else:\n        return 2\n    print(\"dead\")\n\nfn main():\n    print(f(true))\n",
        );
    }

    #[test]
    fn break_before_a_return_does_not_count_as_returning() {
        let diagnostics =
            check_errors("fn f() -> Int:\n    while true:\n        break\n        return 1\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0309"));
    }

    #[test]
    fn rejects_unknown_type() {
        let diagnostics = check_errors("let x: Widget = 1\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0303"));
    }

    #[test]
    fn rejects_break_outside_loop() {
        let diagnostics = check_errors("break\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0204"));
    }

    #[test]
    fn rejects_missing_struct_field() {
        let diagnostics =
            check_errors("struct Point:\n    x: Int\n    y: Int\n\nlet p = Point(x=1)\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0305"));
    }

    #[test]
    fn rejects_unknown_struct_field() {
        let diagnostics =
            check_errors("struct Point:\n    x: Int\n\nlet p = Point(x=1)\nprint(p.z)\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0304"));
    }

    #[test]
    fn rejects_implicit_numeric_mixing() {
        let diagnostics = check_errors("let x = 1 + 2.0\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0306"));
    }

    #[test]
    fn rejects_main_with_parameters() {
        let diagnostics = check_errors("fn main(x: Int):\n    print(x)\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0308"));
    }

    #[test]
    fn warns_when_main_is_shadowed_by_top_level_code() {
        let (_model, diagnostics) = check_source("fn main():\n    print(1)\n\nprint(2)\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "W0001"));
    }

    #[test]
    fn for_loop_infers_element_type() {
        let model = check_ok("for i in range(3):\n    print(i)\n");
        assert_eq!(model.entry, Entry::TopLevel);
    }

    #[test]
    fn rejects_index_on_non_collection() {
        let diagnostics = check_errors("let x = 1\nprint(x[0])\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0306"));
    }

    #[test]
    fn enum_match_binds_payload_types() {
        let model = check_ok(
            "enum Boxed:\n    Empty\n    Value(number: Int)\n\n\
             fn unwrap(value: Boxed) -> Int:\n    match value:\n\
             \x20       Empty:\n            return 0\n\
             \x20       Value(number):\n            return number + 1\n\n\
             fn main():\n    print(unwrap(Empty()))\n",
        );
        assert_eq!(model.functions["unwrap"].ret, Type::Int);
    }

    #[test]
    fn struct_fields_and_enum_payloads_can_reference_each_other() {
        let model = check_ok(
            "struct Holder:\n    flag: Flag\n    flags: List[Flag]\n\n\
             enum Flag:\n    On\n    Off(value: Holder)\n\n\
             fn main():\n    print(On())\n",
        );
        assert_eq!(
            model.structs["Holder"].fields[0].ty,
            Type::Enum("Flag".to_string())
        );
        assert_eq!(
            model.enums["Flag"].variants[1].fields[0].ty,
            Type::Struct("Holder".to_string())
        );
    }

    #[test]
    fn match_statement_counts_as_returning_when_all_arms_return() {
        let (_model, diagnostics) = check_source(
            "enum Flag:\n    On\n    Off\n\n\
             fn value(flag: Flag) -> Int:\n    match flag:\n\
             \x20       On:\n            return 1\n\
             \x20       Off:\n            return 0\n",
        );
        assert!(
            !diagnostics.items.iter().any(|d| d.code == "E0309"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_non_exhaustive_enum_match() {
        let diagnostics = check_errors(
            "enum Flag:\n    On\n    Off\n\n\
             fn main():\n    let flag = On()\n    match flag:\n\
             \x20       On:\n            print(1)\n",
        );
        assert!(diagnostics.items.iter().any(|d| d.code == "E0316"));
    }

    #[test]
    fn warns_when_any_match_has_no_wildcard() {
        let (_model, diagnostics) = check_source(
            "enum Flag:\n    On\n\n\
             fn classify(value: Any):\n    match value:\n\
             \x20       On:\n            print(1)\n",
        );
        assert!(diagnostics.items.iter().any(|d| d.code == "W0004"));
    }

    #[test]
    fn accepts_match_guards_and_payload_bindings() {
        check_ok(concat!(
            "enum Shape:\n",
            "    Circle(radius: Int)\n",
            "    Rect(width: Int, height: Int)\n",
            "\n",
            "fn classify(shape: Shape) -> Str:\n",
            "    match shape:\n",
            "        Circle(radius) if radius > 10:\n",
            "            return \"big\"\n",
            "        Circle(radius):\n",
            "            return \"small\"\n",
            "        Rect(width, height) if width == height:\n",
            "            return \"square\"\n",
            "        Rect(width, height):\n",
            "            return \"rect\"\n",
            "\n",
            "fn main():\n",
            "    print(classify(Circle(1)))\n",
        ));
    }

    #[test]
    fn rejects_non_bool_match_guard() {
        let diagnostics = check_errors(concat!(
            "fn classify(n: Int) -> Str:\n",
            "    return match n:\n",
            "        _ if n: \"bad\"\n",
            "        _: \"ok\"\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0301" && d.message.contains("match guard")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn guarded_arms_do_not_satisfy_exhaustiveness() {
        let diagnostics = check_errors(concat!(
            "fn classify(n: Int) -> Str:\n",
            "    return match n:\n",
            "        _ if n > 0: \"positive\"\n",
        ));
        assert!(
            diagnostics.items.iter().any(|d| d.code == "E0316"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn warns_on_arm_after_unguarded_wildcard_with_guard() {
        let (_model, diagnostics) = check_source(concat!(
            "fn classify(n: Int) -> Str:\n",
            "    return match n:\n",
            "        _: \"all\"\n",
            "        _ if n > 0: \"positive\"\n",
            "\n",
            "fn main():\n",
            "    print(classify(1))\n",
        ));
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
        assert!(
            diagnostics.items.iter().any(|d| d.code == "W0003"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn duplicate_guarded_variant_arms_are_not_unreachable() {
        let (_model, diagnostics) = check_source(concat!(
            "enum Shape:\n",
            "    Circle(radius: Int)\n",
            "    Square\n",
            "\n",
            "fn classify(shape: Shape) -> Str:\n",
            "    match shape:\n",
            "        Circle(radius) if radius > 10:\n",
            "            return \"big\"\n",
            "        Circle(radius) if radius > 5:\n",
            "            return \"medium\"\n",
            "        Circle(radius):\n",
            "            return \"small\"\n",
            "        Square:\n",
            "            return \"square\"\n",
            "\n",
            "fn main():\n",
            "    print(classify(Circle(1)))\n",
        ));
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
        assert!(
            !diagnostics.items.iter().any(|d| d.code == "W0003"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_match_on_non_enum() {
        let diagnostics = check_errors("match 1.5:\n    _:\n        print(1)\n");
        assert!(diagnostics.items.iter().any(|d| d.code == "E0314"));
    }

    #[test]
    fn accepts_literal_patterns_on_scalars() {
        check_ok(concat!(
            "fn classify(n: Int, s: Str, b: Bool) -> Int:\n",
            "    let a: Int = match n:\n",
            "        0: 10\n",
            "        -1: 11\n",
            "        _: 12\n",
            "    let c: Int = match s:\n",
            "        \"a\": 1\n",
            "        _: 0\n",
            "    let d: Int = match b:\n",
            "        true: 1\n",
            "        false: 0\n",
            "    return a + c + d\n",
            "\n",
            "fn main():\n",
            "    print(classify(0, \"a\", true))\n",
        ));
    }

    #[test]
    fn rejects_nonexhaustive_int_match() {
        let diagnostics = check_errors(concat!(
            "fn classify(n: Int) -> Str:\n",
            "    return match n:\n",
            "        1: \"one\"\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0316" && d.message.contains("Int")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_bool_match_missing_a_literal() {
        let diagnostics = check_errors(concat!(
            "fn classify(b: Bool) -> Str:\n",
            "    return match b:\n",
            "        true: \"yes\"\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0316" && d.message.contains("false")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_literal_pattern_type_mismatch() {
        let diagnostics = check_errors(concat!(
            "fn classify(s: Str) -> Int:\n",
            "    return match s:\n",
            "        1: 1\n",
            "        _: 0\n",
        ));
        assert!(diagnostics.items.iter().any(|d| d.code == "E0301"));
    }

    #[test]
    fn rejects_variant_pattern_on_scalar() {
        let diagnostics = check_errors(concat!(
            "enum Flag:\n",
            "    On\n",
            "\n",
            "fn classify(n: Int) -> Int:\n",
            "    return match n:\n",
            "        On: 1\n",
            "        _: 0\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0315" && d.message.contains("cannot match")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_literal_pattern_on_enum() {
        let diagnostics = check_errors(concat!(
            "enum Flag:\n",
            "    On\n",
            "\n",
            "fn classify(flag: Flag) -> Int:\n",
            "    return match flag:\n",
            "        1: 1\n",
            "        _: 0\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0315" && d.message.contains("literal pattern")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn warns_on_duplicate_literal_pattern() {
        let (_model, diagnostics) = check_source(concat!(
            "fn classify(n: Int) -> Int:\n",
            "    return match n:\n",
            "        1: 1\n",
            "        1: 2\n",
            "        _: 0\n",
            "\n",
            "fn main():\n",
            "    print(classify(1))\n",
        ));
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "W0003" && d.message.contains("literal")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn accepts_or_patterns_and_unions_enum_coverage() {
        check_ok(concat!(
            "enum Shape:\n",
            "    Circle(radius: Int)\n",
            "    Square(side: Int)\n",
            "\n",
            "fn classify(shape: Shape) -> Str:\n",
            "    match shape:\n",
            "        Circle(size) | Square(size) if size > 5:\n",
            "            return \"big\"\n",
            "        Circle(size) | Square(size):\n",
            "            return \"small\"\n",
            "\n",
            "fn main():\n",
            "    print(classify(Circle(7)))\n",
        ));
    }

    #[test]
    fn or_patterns_cover_bool_literals_without_wildcard() {
        check_ok(concat!(
            "fn flag(value: Bool) -> Int:\n",
            "    return match value:\n",
            "        true | false: 1\n",
            "\n",
            "fn main():\n",
            "    print(flag(true))\n",
        ));
    }

    #[test]
    fn accepts_range_patterns_on_int() {
        check_ok(concat!(
            "enum Code:\n",
            "    Small(n: Int)\n",
            "    Big(n: Int)\n",
            "\n",
            "fn label(code: Code) -> Str:\n",
            "    return match code:\n",
            "        Small(0..=9): \"digit\"\n",
            "        Small(n) if n < 0: \"negative\"\n",
            "        Small(_): \"small\"\n",
            "        Big(_): \"big\"\n",
            "\n",
            "fn bucket(n: Int) -> Str:\n",
            "    return match n:\n",
            "        1..=3 | 5..=7: \"low\"\n",
            "        -5..0: \"negative\"\n",
            "        _: \"other\"\n",
            "\n",
            "fn main():\n",
            "    print(label(Small(4)))\n",
            "    print(bucket(2))\n",
        ));
    }

    #[test]
    fn rejects_range_pattern_on_non_int() {
        let diagnostics = check_errors(concat!(
            "fn label(s: Str) -> Str:\n",
            "    return match s:\n",
            "        1..5: \"range\"\n",
            "        _: \"other\"\n",
            "\n",
            "fn main():\n",
            "    print(label(\"x\"))\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0301" && d.message.contains("range pattern")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_empty_range_patterns() {
        let diagnostics = check_errors(concat!(
            "fn label(n: Int) -> Str:\n",
            "    return match n:\n",
            "        5..=1: \"inclusive\"\n",
            "        3..3: \"half-open\"\n",
            "        _: \"other\"\n",
            "\n",
            "fn main():\n",
            "    print(label(0))\n",
        ));
        let empty = diagnostics
            .items
            .iter()
            .filter(|d| d.code == "E0318")
            .count();
        assert_eq!(empty, 2, "{:?}", diagnostics.items);
    }

    #[test]
    fn rejects_or_pattern_binding_name_mismatch() {
        let diagnostics = check_errors(concat!(
            "enum Shape:\n",
            "    Circle(radius: Int)\n",
            "    Square(side: Int)\n",
            "\n",
            "fn value(shape: Shape) -> Int:\n",
            "    match shape:\n",
            "        Circle(radius) | Square(side):\n",
            "            return 1\n",
            "\n",
            "fn main():\n",
            "    print(value(Circle(1)))\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0317" && d.message.contains("different names")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_or_pattern_binding_type_mismatch() {
        let diagnostics = check_errors(concat!(
            "enum Mixed:\n",
            "    A(value: Int)\n",
            "    B(value: Str)\n",
            "\n",
            "fn value(input: Mixed) -> Int:\n",
            "    match input:\n",
            "        A(value) | B(value):\n",
            "            return 1\n",
            "\n",
            "fn main():\n",
            "    print(value(A(1)))\n",
        ));
        assert!(
            diagnostics
                .items
                .iter()
                .any(|d| d.code == "E0317" && d.message.contains("has type")),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn guarded_or_patterns_do_not_satisfy_exhaustiveness() {
        let diagnostics = check_errors(concat!(
            "fn classify(n: Int) -> Str:\n",
            "    return match n:\n",
            "        1 | 2 if n > 0: \"positive\"\n",
        ));
        assert!(
            diagnostics.items.iter().any(|d| d.code == "E0316"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn warns_on_duplicate_or_alternative() {
        let (_model, diagnostics) = check_source(concat!(
            "fn label(n: Int) -> Str:\n",
            "    return match n:\n",
            "        1 | 1: \"one\"\n",
            "        _: \"other\"\n",
            "\n",
            "fn main():\n",
            "    print(label(1))\n",
        ));
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
        assert!(
            diagnostics.items.iter().any(|d| d.code == "W0003"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn accepts_nested_patterns_without_wildcard() {
        check_ok(concat!(
            "enum Tree:\n",
            "    Leaf(value: Int)\n",
            "    Node(left: Tree, right: Tree)\n",
            "\n",
            "fn sum(tree: Tree) -> Int:\n",
            "    match tree:\n",
            "        Leaf(value):\n",
            "            return value\n",
            "        Node(Leaf(left), Leaf(right)):\n",
            "            return left + right\n",
            "        Node(Leaf(left), _):\n",
            "            return left\n",
            "        Node(_, Leaf(right)):\n",
            "            return right\n",
            "        Node(_, _):\n",
            "            return 0\n",
            "\n",
            "fn main():\n",
            "    print(sum(Leaf(1)))\n",
        ));
    }

    #[test]
    fn rejects_incomplete_nested_pattern_coverage() {
        let diagnostics = check_errors(concat!(
            "enum Tree:\n",
            "    Leaf(value: Int)\n",
            "    Node(left: Tree, right: Tree)\n",
            "\n",
            "fn sum(tree: Tree) -> Int:\n",
            "    match tree:\n",
            "        Leaf(value):\n",
            "            return value\n",
            "        Node(Leaf(value), _):\n",
            "            return value\n",
            "\n",
            "fn main():\n",
            "    print(sum(Leaf(1)))\n",
        ));
        assert!(
            diagnostics.items.iter().any(|d| d.code == "E0316"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_nested_pattern_payload_type_mismatch() {
        let diagnostics = check_errors(concat!(
            "enum Tree:\n",
            "    Leaf(value: Int)\n",
            "    Node(left: Tree, right: Tree)\n",
            "\n",
            "fn sum(tree: Tree) -> Int:\n",
            "    match tree:\n",
            "        Leaf(\"text\"):\n",
            "            return 0\n",
            "        _:\n",
            "            return 1\n",
            "\n",
            "fn main():\n",
            "    print(sum(Leaf(1)))\n",
        ));
        assert!(
            diagnostics.items.iter().any(|d| d.code == "E0301"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_duplicate_bindings_across_nested_patterns() {
        let diagnostics = check_errors(concat!(
            "enum Tree:\n",
            "    Leaf(value: Int)\n",
            "    Node(left: Tree, right: Tree)\n",
            "\n",
            "fn sum(tree: Tree) -> Int:\n",
            "    match tree:\n",
            "        Node(Leaf(value), Leaf(value)):\n",
            "            return value\n",
            "        _:\n",
            "            return 0\n",
            "\n",
            "fn main():\n",
            "    print(sum(Leaf(1)))\n",
        ));
        assert!(
            diagnostics.items.iter().any(|d| d.code == "E0202"),
            "{:?}",
            diagnostics.items
        );
    }

    #[test]
    fn rejects_pattern_from_another_enum() {
        let diagnostics = check_errors(
            "enum Left:\n    A\n\nenum Right:\n    B\n\n\
             fn main():\n    let left = A()\n    match left:\n\
             \x20       B:\n            print(1)\n\
             \x20       _:\n            print(2)\n",
        );
        assert!(diagnostics.items.iter().any(|d| d.code == "E0315"));
    }

    #[test]
    fn rejects_variant_pattern_with_wrong_arity() {
        let diagnostics = check_errors(
            "enum Maybe:\n    None\n    Some(value: Int)\n\n\
             fn main():\n    let value = None()\n    match value:\n\
             \x20       None(x):\n            print(x)\n\
             \x20       _:\n            print(0)\n",
        );
        assert!(diagnostics.items.iter().any(|d| d.code == "E0302"));
    }

    #[test]
    fn rejects_match_expression_with_mixed_arm_types() {
        let diagnostics = check_errors(
            "enum Flag:\n    On\n\n\
             fn main():\n    let value = match On():\n\
             \x20       On: 1\n        _: \"no\"\n    print(value)\n",
        );
        assert!(diagnostics.items.iter().any(|d| d.code == "E0301"));
    }

    #[test]
    fn rejects_duplicate_variant_names_across_enums() {
        let diagnostics = check_errors(concat!(
            "enum First:\n",
            "    Shared\n",
            "\n",
            "enum Second:\n",
            "    Shared\n",
            "\n",
            "fn main():\n",
            "    print(1)\n",
        ));
        assert!(diagnostics.items.iter().any(|d| d.code == "E0202"));
    }

    #[test]
    fn rejects_variant_colliding_with_builtin() {
        let diagnostics = check_errors(concat!(
            "enum Flag:\n",
            "    print\n",
            "\n",
            "fn main():\n",
            "    print(1)\n",
        ));
        assert!(diagnostics.items.iter().any(|d| d.code == "E0202"));
    }

    #[test]
    fn rejects_enum_name_colliding_with_struct() {
        let diagnostics = check_errors(concat!(
            "struct Thing:\n",
            "    x: Int\n",
            "\n",
            "enum Thing:\n",
            "    A\n",
            "\n",
            "fn main():\n",
            "    print(1)\n",
        ));
        assert!(diagnostics.items.iter().any(|d| d.code == "E0202"));
    }

    #[test]
    fn rejects_named_variant_payload_arguments() {
        let diagnostics = check_errors(concat!(
            "enum Maybe:\n",
            "    Some(value: Int)\n",
            "\n",
            "fn main():\n",
            "    let value = Some(value=1)\n",
            "    print(value)\n",
        ));
        assert!(diagnostics.items.iter().any(|d| d.code == "E0313"));
    }

    #[test]
    fn rejects_variant_payload_type_mismatch() {
        let diagnostics = check_errors(concat!(
            "enum Maybe:\n",
            "    Some(value: Int)\n",
            "\n",
            "fn main():\n",
            "    let value = Some(\"text\")\n",
            "    print(value)\n",
        ));
        assert!(diagnostics.items.iter().any(|d| d.code == "E0301"));
    }

    #[test]
    fn warns_on_unreachable_match_arm() {
        let (_model, diagnostics) = check_source(concat!(
            "enum Flag:\n",
            "    On\n",
            "    Off\n",
            "\n",
            "fn main():\n",
            "    let flag = On()\n",
            "    match flag:\n",
            "        _:\n",
            "            print(1)\n",
            "        On:\n",
            "            print(2)\n",
        ));
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
        assert!(diagnostics.items.iter().any(|d| d.code == "W0003"));
    }

    #[test]
    fn rejects_duplicate_pattern_bindings() {
        let diagnostics = check_errors(concat!(
            "enum Pairish:\n",
            "    Both(a: Int, b: Int)\n",
            "\n",
            "fn main():\n",
            "    match Both(1, 2):\n",
            "        Both(x, x):\n",
            "            print(x)\n",
        ));
        assert!(diagnostics.items.iter().any(|d| d.code == "E0202"));
    }

    #[test]
    fn wildcard_dynamic_match_does_not_warn() {
        let (_model, diagnostics) = check_source(concat!(
            "enum Boxed:\n",
            "    Value(number: Int)\n",
            "\n",
            "fn unwrap(value: Any) -> Int:\n",
            "    match value:\n",
            "        Value(number):\n",
            "            return number\n",
            "        _:\n",
            "            return 0\n",
            "\n",
            "fn main():\n",
            "    print(unwrap(Value(1)))\n",
        ));
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
        assert!(!diagnostics.items.iter().any(|d| d.code == "W0004"));
    }
}
