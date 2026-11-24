//! Tree-walking interpreter.
//!
//! The interpreter is deliberately fail-stop: every dynamic operation either
//! produces a value of the expected shape or returns a `RuntimeError` with a
//! span and call stack. It never panics on Vantyx-level input.

use crate::ast::*;
use crate::checker::{Entry, FnSig, SemanticModel, is_builtin};
use crate::span::Span;
use crate::types::{Type, VariantInfo, resolve_type_expr};
use crate::value::{FieldSlot, Value, map_get, map_insert};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;

pub const MAX_CALL_DEPTH: usize = 256;
pub const MAX_EVAL_DEPTH: usize = 1024;
pub const MAX_RANGE_LEN: usize = 10_000_000;
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Unicode `White_Space` code points, shared by `trim` in both engines.
fn is_text_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

/// Byte range of the final path component after dropping trailing `/` bytes.
fn path_name_range(path: &str) -> (usize, usize) {
    let bytes = path.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b'/' {
        end -= 1;
    }
    let mut begin = end;
    while begin > 0 && bytes[begin - 1] != b'/' {
        begin -= 1;
    }
    (begin, end)
}

fn path_parent(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b'/' {
        end -= 1;
    }
    if end == 0 {
        return if bytes.first() == Some(&b'/') {
            "/".to_string()
        } else {
            ".".to_string()
        };
    }
    let mut slash = end;
    while slash > 0 && bytes[slash - 1] != b'/' {
        slash -= 1;
    }
    let mut parent_end = slash;
    while parent_end > 0 && bytes[parent_end - 1] == b'/' {
        parent_end -= 1;
    }
    if parent_end == 0 {
        if bytes.first() == Some(&b'/') {
            "/".to_string()
        } else {
            ".".to_string()
        }
    } else {
        path[..parent_end].to_string()
    }
}

fn path_stem(path: &str) -> &str {
    let (begin, end) = path_name_range(path);
    let name = &path[begin..end];
    let bytes = name.as_bytes();
    match bytes.iter().rposition(|byte| *byte == b'.') {
        Some(dot) if dot > 0 && dot + 1 < bytes.len() => &name[..dot],
        _ => name,
    }
}

fn path_extension(path: &str) -> &str {
    let (begin, end) = path_name_range(path);
    let name = &path[begin..end];
    let bytes = name.as_bytes();
    match bytes.iter().rposition(|byte| *byte == b'.') {
        Some(dot) if dot > 0 && dot + 1 < bytes.len() => &name[dot + 1..],
        _ => "",
    }
}

fn path_join(base: &str, child: &str) -> String {
    if child.starts_with('/') || base.is_empty() {
        return child.to_string();
    }
    if child.is_empty() {
        return base.to_string();
    }
    if base.ends_with('/') {
        format!("{base}{child}")
    } else {
        format!("{base}/{child}")
    }
}

#[derive(Clone, Debug)]
pub struct StackFrame {
    pub function: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct RuntimeError {
    pub code: String,
    pub message: String,
    pub span: Span,
    pub stack: Vec<StackFrame>,
}

impl RuntimeError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            span,
            stack: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct Slot {
    value: Value,
    mutable: bool,
    ty: Type,
}

enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

/// Result of evaluating a postfix `?` operand.
enum TryOutcome {
    /// The success payload, ready to be used as the expression value.
    Value(Value),
    /// The failure variant, which the enclosing function must return.
    Propagate(Value),
}

enum Place {
    Var {
        scope: usize,
        name: String,
    },
    Field {
        target: Rc<std::cell::RefCell<crate::value::StructValue>>,
        index: usize,
    },
    ListIndex {
        target: Rc<std::cell::RefCell<Vec<Value>>>,
        index: usize,
    },
    MapIndex {
        target: Rc<std::cell::RefCell<Vec<(Value, Value)>>>,
        key: Value,
    },
}

/// Executes a checked program, writing program output to `out`.
///
/// Evaluation runs on a dedicated thread with a 16 MiB stack. This keeps the
/// call-depth limit meaningful on platforms whose main/test thread stacks are
/// smaller, and turns an unexpected panic into a runtime diagnostic instead of
/// taking down the host process.
pub fn run(
    program: &Program,
    model: &SemanticModel,
    out: &mut (dyn Write + Send),
) -> Result<(), RuntimeError> {
    run_on_thread(program.span, || Interpreter::new(program, model, out).run())
}

/// Executes a REPL session entry.
///
/// The REPL compiles the accumulated session source plus the new entry and
/// replays earlier top-level statements with output muted. That reconstructs
/// the persistent environment without retaining interpreter borrows across
/// entries. `prefix` is the number of leading top-level statements that belong
/// to earlier entries; `auto_main` preserves the first-entry behavior where
/// defining `fn main` runs it immediately.
pub fn run_repl(
    program: &Program,
    model: &SemanticModel,
    prefix: usize,
    auto_main: bool,
    out: &mut (dyn Write + Send),
) -> Result<(), RuntimeError> {
    run_on_thread(program.span, || {
        Interpreter::new(program, model, out).run_repl(prefix, auto_main)
    })
}

fn run_on_thread(
    span: Span,
    execute: impl FnOnce() -> Result<(), RuntimeError> + Send,
) -> Result<(), RuntimeError> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("vantyx-interpreter".to_string())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, execute)
            .map_err(|error| {
                RuntimeError::new(
                    "R0010",
                    format!("cannot start interpreter thread: {error}"),
                    span,
                )
            })?;
        match handle.join() {
            Ok(result) => result,
            Err(_) => Err(RuntimeError::new(
                "R0010",
                "interpreter panicked; this is a compiler bug",
                span,
            )),
        }
    })
}

struct Interpreter<'a> {
    program: &'a Program,
    model: &'a SemanticModel,
    fn_bodies: HashMap<String, &'a FnDecl>,
    out: &'a mut (dyn Write + Send),
    scopes: Vec<HashMap<String, Slot>>,
    call_stack: Vec<StackFrame>,
    call_depth: usize,
    eval_depth: usize,
    muted: bool,
}

impl<'a> Interpreter<'a> {
    fn new(
        program: &'a Program,
        model: &'a SemanticModel,
        out: &'a mut (dyn Write + Send),
    ) -> Self {
        let mut fn_bodies = HashMap::new();
        for item in &program.items {
            if let Item::Fn(decl) = item {
                fn_bodies.insert(decl.name.clone(), decl);
            }
        }
        Self {
            program,
            model,
            fn_bodies,
            out,
            scopes: Vec::new(),
            call_stack: Vec::new(),
            call_depth: 0,
            eval_depth: 0,
            muted: false,
        }
    }

    fn run_repl(&mut self, prefix: usize, auto_main: bool) -> Result<(), RuntimeError> {
        let has_statements = self
            .program
            .items
            .iter()
            .any(|item| matches!(item, Item::Stmt(_)));
        if auto_main && !has_statements {
            return self.run();
        }

        self.scopes.push(HashMap::new());
        self.muted = prefix > 0;
        let mut seen = 0usize;
        for item in &self.program.items {
            let Item::Stmt(stmt) = item else {
                continue;
            };
            if seen == prefix {
                self.muted = false;
            }
            let is_new = seen >= prefix;
            let (flow, value) = self.exec_stmt_echo(stmt)?;
            if is_new
                && let Some(value) = value
                && !matches!(value, Value::Unit)
            {
                let write_result = writeln!(self.out, "{}", value.display());
                if let Err(error) = write_result {
                    return Err(self.rt(
                        "R0013",
                        format!("failed to write output: {error}"),
                        stmt.span,
                    ));
                }
            }
            match flow {
                Flow::Normal => {}
                _ => {
                    return Err(self.rt(
                        "R0010",
                        "loop or return control escaped top-level code",
                        stmt.span,
                    ));
                }
            }
            seen += 1;
        }
        self.muted = false;
        Ok(())
    }

    fn run(&mut self) -> Result<(), RuntimeError> {
        self.scopes.push(HashMap::new());
        match self.model.entry {
            Entry::TopLevel => {
                for item in &self.program.items {
                    if let Item::Stmt(stmt) = item {
                        match self.exec_stmt(stmt)? {
                            Flow::Normal => {}
                            _ => {
                                return Err(self.rt(
                                    "R0010",
                                    "loop or return control escaped top-level code",
                                    stmt.span,
                                ));
                            }
                        }
                    }
                }
            }
            Entry::Main => {
                let sig = self.model.functions.get("main").cloned().ok_or_else(|| {
                    RuntimeError::new("R0010", "missing `main` function", self.program.span)
                })?;
                let body = self.fn_bodies.get("main").copied().ok_or_else(|| {
                    RuntimeError::new("R0010", "missing `main` body", self.program.span)
                })?;
                self.call_user(body, &sig, Vec::new(), self.program.span)?;
            }
            Entry::None => {}
        }
        Ok(())
    }

    // ---- errors ----

    fn rt(&self, code: &str, message: impl Into<String>, span: Span) -> RuntimeError {
        RuntimeError {
            code: code.to_string(),
            message: message.into(),
            span,
            stack: self.call_stack.clone(),
        }
    }

    fn type_error(&self, context: &str, expected: &str, value: &Value, span: Span) -> RuntimeError {
        self.rt(
            "R0005",
            format!(
                "type mismatch{context}: expected {expected}, found `{}`",
                value.type_name()
            ),
            span,
        )
    }

    fn validate(
        &self,
        value: &Value,
        ty: &Type,
        span: Span,
        context: &str,
    ) -> Result<(), RuntimeError> {
        if value.conforms_to(ty) {
            Ok(())
        } else {
            Err(self.rt(
                "R0005",
                format!(
                    "type mismatch{context}: expected `{ty}`, found `{}`",
                    value.type_name()
                ),
                span,
            ))
        }
    }

    fn resolve_runtime_type(&self, expr: &TypeExpr) -> Result<Type, RuntimeError> {
        resolve_type_expr(expr, &self.model.structs, &self.model.enums)
            .map_err(|message| self.rt("R0010", format!("unresolved type: {message}"), expr.span))
    }

    // ---- scopes ----

    fn lookup(&self, name: &str) -> Option<&Slot> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn find_scope(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rposition(|scope| scope.contains_key(name))
    }

    fn bind(
        &mut self,
        name: &str,
        value: Value,
        mutable: bool,
        ty: Type,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(scope) = self.scopes.last_mut() else {
            return Err(self.rt("R0010", "internal error: no active scope", span));
        };
        scope.insert(name.to_string(), Slot { value, mutable, ty });
        Ok(())
    }

    // ---- statements ----

    fn exec_block(&mut self, block: &Block) -> Result<Flow, RuntimeError> {
        self.scopes.push(HashMap::new());
        let result = self.exec_stmts(&block.stmts);
        self.scopes.pop();
        result
    }

    fn exec_stmts(&mut self, stmts: &[Stmt]) -> Result<Flow, RuntimeError> {
        for stmt in stmts {
            let flow = self.exec_stmt(stmt)?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        Ok(Flow::Normal)
    }

    /// Executes a statement and returns its value when it is an expression
    /// statement. The REPL uses this to echo expression values exactly once.
    fn exec_stmt_echo(&mut self, stmt: &Stmt) -> Result<(Flow, Option<Value>), RuntimeError> {
        if let StmtKind::Expr(expr) = &stmt.kind {
            // A bare `expr?` propagates on failure and never echoes a value.
            if matches!(expr.kind, ExprKind::Try { .. }) {
                return Ok((self.exec_stmt(stmt)?, None));
            }
            let value = self.eval_expr(expr)?;
            return Ok((Flow::Normal, Some(value)));
        }
        Ok((self.exec_stmt(stmt)?, None))
    }

    /// Evaluates the operand of a postfix `?`. `Value` is the success
    /// payload; `Propagate` is the failure enum value that the enclosing
    /// function must return unchanged.
    fn eval_try(&mut self, operand: &Expr, span: Span) -> Result<TryOutcome, RuntimeError> {
        let value = self.eval_expr(operand)?;
        let Value::Enum(instance) = &value else {
            return Err(self.rt(
                "R0017",
                format!(
                    "`?` requires a two-variant enum value, found `{}`",
                    value.type_name()
                ),
                span,
            ));
        };
        let Some(info) = self.model.enums.get(&instance.enum_name) else {
            return Err(self.rt(
                "R0010",
                format!("unknown enum `{}`", instance.enum_name),
                span,
            ));
        };
        if info.variants.len() != 2 {
            return Err(self.rt(
                "R0017",
                format!(
                    "`?` requires an enum with exactly two variants, but `{}` has {}",
                    instance.enum_name,
                    info.variants.len()
                ),
                span,
            ));
        }
        let success = &info.variants[0];
        let failure = &info.variants[1];
        if success.fields.len() != 1 || failure.fields.len() != 1 {
            return Err(self.rt(
                "R0017",
                format!(
                    "`?` requires one payload field on each variant of `{}`",
                    instance.enum_name
                ),
                span,
            ));
        }
        if instance.variant == failure.name {
            return Ok(TryOutcome::Propagate(value));
        }
        if instance.variant != success.name {
            return Err(self.rt(
                "R0017",
                format!(
                    "`?` received unknown variant `{}` of enum `{}`",
                    instance.variant, instance.enum_name
                ),
                span,
            ));
        }
        match instance.fields.first() {
            Some(field) => Ok(TryOutcome::Value(field.value.clone())),
            None => Err(self.rt(
                "R0017",
                format!(
                    "success variant `{}` of `{}` has no payload",
                    instance.variant, instance.enum_name
                ),
                span,
            )),
        }
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow, RuntimeError> {
        match &stmt.kind {
            StmtKind::Let {
                mutable,
                name,
                name_span,
                ty,
                init,
            } => {
                let declared = match ty {
                    Some(expr) => Some(self.resolve_runtime_type(expr)?),
                    None => None,
                };
                let value = match &init.kind {
                    ExprKind::Try { expr: operand } => {
                        match self.eval_try(operand, init.span)? {
                            TryOutcome::Value(value) => value,
                            TryOutcome::Propagate(value) => return Ok(Flow::Return(value)),
                        }
                    }
                    _ => self.eval_expr(init)?,
                };
                let binding_ty = match &declared {
                    Some(ty) => ty.clone(),
                    None => self
                        .model
                        .expr_types
                        .get(&init.id)
                        .cloned()
                        .unwrap_or(Type::Any),
                };
                if let Some(ty) = &declared {
                    self.validate(&value, ty, init.span, &format!(" in binding `{name}`"))?;
                }
                self.bind(name, value, *mutable, binding_ty, *name_span)?;
                Ok(Flow::Normal)
            }
            StmtKind::Assign { target, op, value } => {
                match op {
                    None => {
                        // Python-style: the right-hand side is evaluated first.
                        let rhs = match &value.kind {
                            ExprKind::Try { expr: operand } => {
                                match self.eval_try(operand, value.span)? {
                                    TryOutcome::Value(value) => value,
                                    TryOutcome::Propagate(value) => {
                                        return Ok(Flow::Return(value));
                                    }
                                }
                            }
                            _ => self.eval_expr(value)?,
                        };
                        let place = self.eval_place(target)?;
                        self.write_place(&place, rhs, stmt.span)?;
                    }
                    Some(op) => {
                        let place = self.eval_place(target)?;
                        let old = self.read_place(&place, target.span)?;
                        let rhs = self.eval_expr(value)?;
                        let result = self.binary_op(*op, &old, &rhs, stmt.span)?;
                        self.write_place(&place, result, stmt.span)?;
                    }
                }
                Ok(Flow::Normal)
            }
            StmtKind::Expr(expr) => {
                if let ExprKind::Try { expr: operand } = &expr.kind {
                    if let TryOutcome::Propagate(value) = self.eval_try(operand, expr.span)? {
                        return Ok(Flow::Return(value));
                    }
                } else {
                    self.eval_expr(expr)?;
                }
                Ok(Flow::Normal)
            }
            StmtKind::If {
                cond,
                then_block,
                elifs,
                else_block,
            } => {
                let condition = self.eval_condition(cond)?;
                if condition {
                    return self.exec_block(then_block);
                }
                for (elif_cond, block) in elifs {
                    if self.eval_condition(elif_cond)? {
                        return self.exec_block(block);
                    }
                }
                if let Some(block) = else_block {
                    return self.exec_block(block);
                }
                Ok(Flow::Normal)
            }
            StmtKind::While { cond, body } => {
                while self.eval_condition(cond)? {
                    match self.exec_block(body)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                    }
                }
                Ok(Flow::Normal)
            }
            StmtKind::For {
                name,
                name_span,
                iter,
                body,
            } => self.exec_for(name, *name_span, iter, body),
            StmtKind::Match { scrutinee, arms } => self.exec_match_stmt(scrutinee, arms, stmt.span),
            StmtKind::Return(value) => {
                let value = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Unit,
                };
                Ok(Flow::Return(value))
            }
            StmtKind::Break => Ok(Flow::Break),
            StmtKind::Continue => Ok(Flow::Continue),
        }
    }

    fn exec_for(
        &mut self,
        name: &str,
        name_span: Span,
        iter: &Expr,
        body: &Block,
    ) -> Result<Flow, RuntimeError> {
        let iterable = self.eval_expr(iter)?;
        match iterable {
            Value::List(items) => {
                // Snapshot the length, but read elements live so mutation is
                // visible without extending the iteration.
                let len = items.borrow().len();
                for index in 0..len {
                    let item = {
                        let borrowed = items.borrow();
                        match borrowed.get(index) {
                            Some(value) => value.clone(),
                            None => break,
                        }
                    };
                    match self.exec_for_iteration(name, name_span, item, body)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                    }
                }
                Ok(Flow::Normal)
            }
            Value::Str(text) => {
                let characters: Vec<Value> =
                    text.chars().map(|c| Value::str(c.to_string())).collect();
                for character in characters {
                    match self.exec_for_iteration(name, name_span, character, body)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                    }
                }
                Ok(Flow::Normal)
            }
            Value::Map(entries) => {
                let keys: Vec<Value> = entries
                    .borrow()
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect();
                for key in keys {
                    match self.exec_for_iteration(name, name_span, key, body)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                    }
                }
                Ok(Flow::Normal)
            }
            other => Err(self.type_error(
                " in `for`",
                "an iterable (List, Str, or Map)",
                &other,
                iter.span,
            )),
        }
    }

    fn exec_for_iteration(
        &mut self,
        name: &str,
        name_span: Span,
        value: Value,
        body: &Block,
    ) -> Result<Flow, RuntimeError> {
        self.scopes.push(HashMap::new());
        self.bind(name, value, false, Type::Any, name_span)?;
        let result = self.exec_block(body);
        self.scopes.pop();
        result
    }

    fn exec_match_stmt(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        span: Span,
    ) -> Result<Flow, RuntimeError> {
        let value = self.eval_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = self.pattern_bindings(&arm.pattern, &value) else {
                continue;
            };
            let MatchArmBody::Block(block) = &arm.body else {
                return Err(self.rt(
                    "R0010",
                    "internal error: expression arm in a match statement",
                    arm.span,
                ));
            };
            self.scopes.push(HashMap::new());
            for (name, bound, ty) in bindings {
                self.bind(&name, bound, false, ty, arm.pattern.span())?;
            }
            match self.arm_guard(arm) {
                Ok(true) => {}
                Ok(false) => {
                    self.scopes.pop();
                    continue;
                }
                Err(error) => {
                    self.scopes.pop();
                    return Err(error);
                }
            }
            let result = self.exec_block(block);
            self.scopes.pop();
            return result;
        }
        Err(self.rt(
            "R0014",
            format!(
                "no match arm matched the value `{}`",
                value.display_nested()
            ),
            span,
        ))
    }

    fn eval_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let value = self.eval_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = self.pattern_bindings(&arm.pattern, &value) else {
                continue;
            };
            let MatchArmBody::Expr(body) = &arm.body else {
                return Err(self.rt(
                    "R0010",
                    "internal error: block arm in a match expression",
                    arm.span,
                ));
            };
            self.scopes.push(HashMap::new());
            for (name, bound, ty) in bindings {
                self.bind(&name, bound, false, ty, arm.pattern.span())?;
            }
            match self.arm_guard(arm) {
                Ok(true) => {}
                Ok(false) => {
                    self.scopes.pop();
                    continue;
                }
                Err(error) => {
                    self.scopes.pop();
                    return Err(error);
                }
            }
            let result = self.eval_expr(body);
            self.scopes.pop();
            return result;
        }
        Err(self.rt(
            "R0014",
            format!(
                "no match arm matched the value `{}`",
                value.display_nested()
            ),
            span,
        ))
    }

    /// Returns the bindings introduced by `pattern` when it matches `value`.
    /// `None` means this arm does not match and the next arm should be tried.
    fn pattern_bindings(
        &self,
        pattern: &Pattern,
        value: &Value,
    ) -> Option<Vec<(String, Value, Type)>> {
        match pattern {
            Pattern::Wildcard { .. } => Some(Vec::new()),
            Pattern::Literal { value: literal, .. } => {
                let matched = match (literal, value) {
                    (LiteralPattern::Int(expected), Value::Int(actual)) => expected == actual,
                    (LiteralPattern::Bool(expected), Value::Bool(actual)) => expected == actual,
                    (LiteralPattern::Str(expected), Value::Str(actual)) => {
                        expected.as_str() == actual.as_ref()
                    }
                    _ => false,
                };
                if matched { Some(Vec::new()) } else { None }
            }
            Pattern::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                let Value::Int(actual) = value else {
                    return None;
                };
                let matched = if *inclusive {
                    actual >= start && actual <= end
                } else {
                    actual >= start && actual < end
                };
                if matched { Some(Vec::new()) } else { None }
            }
            Pattern::Variant { name, fields, .. } => {
                let Value::Enum(instance) = value else {
                    return None;
                };
                if instance.variant != *name || instance.fields.len() != fields.len() {
                    return None;
                }
                let variant = self.model.find_variant(name).map(|(_, variant)| variant);
                let mut bindings = Vec::new();
                for (index, field) in fields.iter().enumerate() {
                    let slot = instance.fields.get(index)?;
                    match field {
                        PatternArg::Bind { name: binding, .. } => {
                            let ty = variant
                                .and_then(|variant| variant.fields.get(index))
                                .map(|field| field.ty.clone())
                                .unwrap_or_else(|| slot.ty.clone());
                            bindings.push((binding.clone(), slot.value.clone(), ty));
                        }
                        PatternArg::Wildcard { .. } => {}
                        PatternArg::Nested { pattern, .. } => {
                            let nested = self.pattern_bindings(pattern, &slot.value)?;
                            bindings.extend(nested);
                        }
                    }
                }
                Some(bindings)
            }
            Pattern::Or { alternatives, .. } => alternatives
                .iter()
                .find_map(|pattern| self.pattern_bindings(pattern, value)),
        }
    }

    fn arm_guard(&mut self, arm: &MatchArm) -> Result<bool, RuntimeError> {
        match &arm.guard {
            Some(guard) => self.eval_condition(guard),
            None => Ok(true),
        }
    }

    fn eval_condition(&mut self, expr: &Expr) -> Result<bool, RuntimeError> {
        let value = self.eval_expr(expr)?;
        match value {
            Value::Bool(condition) => Ok(condition),
            other => Err(self.type_error(" as a condition", "`Bool`", &other, expr.span)),
        }
    }

    // ---- assignment places ----

    fn eval_place(&mut self, target: &Expr) -> Result<Place, RuntimeError> {
        match &target.kind {
            ExprKind::Var(name) => {
                let Some(scope) = self.find_scope(name) else {
                    return Err(self.rt(
                        "R0009",
                        format!("variable `{name}` is not initialized yet"),
                        target.span,
                    ));
                };
                Ok(Place::Var {
                    scope,
                    name: name.clone(),
                })
            }
            ExprKind::Field { base, name, .. } => {
                let base_value = self.eval_expr(base)?;
                match base_value {
                    Value::Struct(instance) => {
                        let (index, type_name) = {
                            let borrowed = instance.borrow();
                            match borrowed.fields.iter().position(|field| field.name == *name) {
                                Some(index) => (index, borrowed.type_name.clone()),
                                None => {
                                    return Err(self.rt(
                                        "R0010",
                                        format!(
                                            "struct `{}` has no field `{name}`",
                                            borrowed.type_name
                                        ),
                                        target.span,
                                    ));
                                }
                            }
                        };
                        let _ = type_name;
                        Ok(Place::Field {
                            target: instance,
                            index,
                        })
                    }
                    other => Err(self.type_error(
                        " in field assignment",
                        "a struct value",
                        &other,
                        base.span,
                    )),
                }
            }
            ExprKind::Index { base, index } => {
                let base_value = self.eval_expr(base)?;
                let index_value = self.eval_expr(index)?;
                match base_value {
                    Value::List(items) => {
                        let position = self.index_position(&index_value, target.span)?;
                        let len = items.borrow().len();
                        if position >= len {
                            return Err(self.rt(
                                "R0003",
                                format!(
                                    "index {position} is out of bounds for a list of length {len}"
                                ),
                                target.span,
                            ));
                        }
                        Ok(Place::ListIndex {
                            target: items,
                            index: position,
                        })
                    }
                    Value::Map(entries) => Ok(Place::MapIndex {
                        target: entries,
                        key: index_value,
                    }),
                    Value::Str(_) => Err(self.rt(
                        "R0010",
                        "strings are immutable and cannot be assigned by index",
                        target.span,
                    )),
                    other => Err(self.type_error(
                        " in index assignment",
                        "a List or Map",
                        &other,
                        base.span,
                    )),
                }
            }
            _ => Err(self.rt("R0010", "invalid assignment target", target.span)),
        }
    }

    fn read_place(&mut self, place: &Place, span: Span) -> Result<Value, RuntimeError> {
        match place {
            Place::Var { scope, name } => {
                let slot = self
                    .scopes
                    .get(*scope)
                    .and_then(|scope| scope.get(name))
                    .ok_or_else(|| {
                        self.rt(
                            "R0009",
                            format!("variable `{name}` is not initialized yet"),
                            span,
                        )
                    })?;
                Ok(slot.value.clone())
            }
            Place::Field { target, index } => {
                let borrowed = target.borrow();
                borrowed
                    .fields
                    .get(*index)
                    .map(|field| field.value.clone())
                    .ok_or_else(|| self.rt("R0010", "field disappeared during assignment", span))
            }
            Place::ListIndex { target, index } => {
                let borrowed = target.borrow();
                borrowed.get(*index).cloned().ok_or_else(|| {
                    self.rt(
                        "R0003",
                        format!(
                            "index {index} is out of bounds for a list of length {}",
                            borrowed.len()
                        ),
                        span,
                    )
                })
            }
            Place::MapIndex { target, key } => {
                map_get(target.borrow().as_slice(), key).ok_or_else(|| {
                    self.rt(
                        "R0008",
                        format!("key `{}` is not present in the map", key.display()),
                        span,
                    )
                })
            }
        }
    }

    fn write_place(&mut self, place: &Place, value: Value, span: Span) -> Result<(), RuntimeError> {
        match place {
            Place::Var { scope, name } => {
                let (mutable, ty) = {
                    let slot = self
                        .scopes
                        .get(*scope)
                        .and_then(|scope| scope.get(name))
                        .ok_or_else(|| {
                            self.rt(
                                "R0009",
                                format!("variable `{name}` is not initialized yet"),
                                span,
                            )
                        })?;
                    (slot.mutable, slot.ty.clone())
                };
                if !mutable {
                    return Err(self.rt(
                        "R0010",
                        format!("cannot assign to immutable binding `{name}`"),
                        span,
                    ));
                }
                self.validate(&value, &ty, span, &format!(" in assignment to `{name}`"))?;
                let slot = self
                    .scopes
                    .get_mut(*scope)
                    .and_then(|scope| scope.get_mut(name));
                let Some(slot) = slot else {
                    return Err(self.rt(
                        "R0009",
                        format!("variable `{name}` is not initialized yet"),
                        span,
                    ));
                };
                slot.value = value;
                Ok(())
            }
            Place::Field { target, index } => {
                let (field_name, ty) = {
                    let borrowed = target.borrow();
                    let Some(field) = borrowed.fields.get(*index) else {
                        return Err(self.rt("R0010", "field disappeared during assignment", span));
                    };
                    (field.name.clone(), field.ty.clone())
                };
                self.validate(
                    &value,
                    &ty,
                    span,
                    &format!(" in assignment to field `{field_name}`"),
                )?;
                let mut borrowed = target.borrow_mut();
                let Some(field) = borrowed.fields.get_mut(*index) else {
                    return Err(self.rt("R0010", "field disappeared during assignment", span));
                };
                field.value = value;
                Ok(())
            }
            Place::ListIndex { target, index } => {
                let mut borrowed = target.borrow_mut();
                let Some(slot) = borrowed.get_mut(*index) else {
                    return Err(self.rt(
                        "R0003",
                        format!(
                            "index {index} is out of bounds for a list of length {}",
                            borrowed.len()
                        ),
                        span,
                    ));
                };
                *slot = value;
                Ok(())
            }
            Place::MapIndex { target, key } => {
                map_insert(&mut target.borrow_mut(), key.clone(), value);
                Ok(())
            }
        }
    }

    // ---- expressions ----

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        self.eval_depth += 1;
        if self.eval_depth > MAX_EVAL_DEPTH {
            self.eval_depth -= 1;
            return Err(self.rt(
                "R0011",
                "evaluation depth exceeded; simplify the expression",
                expr.span,
            ));
        }
        let result = self.eval_expr_inner(expr);
        self.eval_depth -= 1;
        result
    }

    fn eval_expr_inner(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match &expr.kind {
            ExprKind::Int(value) => Ok(Value::Int(*value)),
            ExprKind::Float(value) => Ok(Value::Float(*value)),
            ExprKind::Bool(value) => Ok(Value::Bool(*value)),
            ExprKind::Str(parts) => {
                let mut text = String::new();
                for part in parts {
                    match part {
                        StrPart::Text(chunk) => text.push_str(chunk),
                        StrPart::Expr(inner) => {
                            let value = self.eval_expr(inner)?;
                            text.push_str(&value.display());
                        }
                    }
                }
                Ok(Value::str(text))
            }
            ExprKind::Var(name) => {
                self.lookup(name)
                    .map(|slot| slot.value.clone())
                    .ok_or_else(|| {
                        self.rt(
                            "R0009",
                            format!("variable `{name}` is not initialized yet"),
                            expr.span,
                        )
                    })
            }
            ExprKind::List(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(self.eval_expr(element)?);
                }
                Ok(Value::list(values))
            }
            ExprKind::Map(entries) => {
                let mut values: Vec<(Value, Value)> = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    let key = self.eval_expr(key)?;
                    let value = self.eval_expr(value)?;
                    map_insert(&mut values, key, value);
                }
                Ok(Value::map(values))
            }
            ExprKind::Try { .. } => Err(self.rt(
                "R0017",
                "`?` is only valid as a `let` initializer, a plain assignment value, or an expression statement",
                expr.span,
            )),
            ExprKind::Unary { op, expr: inner } => {
                let value = self.eval_expr(inner)?;
                match op {
                    UnOp::Neg => match value {
                        Value::Int(number) => {
                            number.checked_neg().map(Value::Int).ok_or_else(|| {
                                self.rt("R0002", "integer overflow while negating", expr.span)
                            })
                        }
                        Value::Float(number) => Ok(Value::Float(-number)),
                        other => Err(self.type_error(
                            " after `-`",
                            "`Int` or `Float`",
                            &other,
                            expr.span,
                        )),
                    },
                    UnOp::Not => match value {
                        Value::Bool(flag) => Ok(Value::Bool(!flag)),
                        other => Err(self.type_error(" after `not`", "`Bool`", &other, expr.span)),
                    },
                }
            }
            ExprKind::Binary { op, lhs, rhs } => match op {
                BinOp::And => {
                    let left = self.eval_expr(lhs)?;
                    let left = match left {
                        Value::Bool(flag) => flag,
                        other => {
                            return Err(self.type_error(
                                " before `and`",
                                "`Bool`",
                                &other,
                                lhs.span,
                            ));
                        }
                    };
                    if !left {
                        return Ok(Value::Bool(false));
                    }
                    let right = self.eval_expr(rhs)?;
                    match right {
                        Value::Bool(flag) => Ok(Value::Bool(flag)),
                        other => Err(self.type_error(" after `and`", "`Bool`", &other, rhs.span)),
                    }
                }
                BinOp::Or => {
                    let left = self.eval_expr(lhs)?;
                    let left = match left {
                        Value::Bool(flag) => flag,
                        other => {
                            return Err(self.type_error(
                                " before `or`",
                                "`Bool`",
                                &other,
                                lhs.span,
                            ));
                        }
                    };
                    if left {
                        return Ok(Value::Bool(true));
                    }
                    let right = self.eval_expr(rhs)?;
                    match right {
                        Value::Bool(flag) => Ok(Value::Bool(flag)),
                        other => Err(self.type_error(" after `or`", "`Bool`", &other, rhs.span)),
                    }
                }
                _ => {
                    let left = self.eval_expr(lhs)?;
                    let right = self.eval_expr(rhs)?;
                    self.binary_op(*op, &left, &right, expr.span)
                }
            },
            ExprKind::Call { callee, args } => {
                let ExprKind::Var(name) = &callee.kind else {
                    return Err(self.rt(
                        "R0010",
                        "only named functions and struct constructors can be called",
                        expr.span,
                    ));
                };
                if is_builtin(name) {
                    let values = self.eval_args(args)?;
                    return self.call_builtin(name, values, expr.span);
                }
                if let Some(sig) = self.model.functions.get(name).cloned() {
                    for arg in args {
                        if let Some((arg_name, _)) = &arg.name {
                            return Err(self.rt(
                                "R0010",
                                format!(
                                    "named argument `{arg_name}` is not valid for function calls"
                                ),
                                expr.span,
                            ));
                        }
                    }
                    let body = match self.fn_bodies.get(name) {
                        Some(body) => *body,
                        None => {
                            return Err(self.rt(
                                "R0010",
                                format!("function `{name}` has no body"),
                                expr.span,
                            ));
                        }
                    };
                    let values = self.eval_args(args)?;
                    return self.call_user(body, &sig, values, expr.span);
                }
                if self.model.structs.contains_key(name) {
                    return self.call_constructor(name, args, expr.span);
                }
                if let Some((enum_name, variant)) = self
                    .model
                    .find_variant(name)
                    .map(|(enum_name, variant)| (enum_name.to_string(), variant.clone()))
                {
                    return self.call_variant_constructor(&enum_name, &variant, args, expr.span);
                }
                Err(self.rt(
                    "R0010",
                    format!("unknown function, variant, or struct `{name}`"),
                    expr.span,
                ))
            }
            ExprKind::Index { base, index } => {
                let base_value = self.eval_expr(base)?;
                let index_value = self.eval_expr(index)?;
                self.index_value(base_value, index_value, expr.span)
            }
            ExprKind::Field { base, name, .. } => {
                let base_value = self.eval_expr(base)?;
                match base_value {
                    Value::Struct(instance) => {
                        let borrowed = instance.borrow();
                        borrowed
                            .field(name)
                            .map(|field| field.value.clone())
                            .ok_or_else(|| {
                                self.rt(
                                    "R0010",
                                    format!(
                                        "struct `{}` has no field `{name}`",
                                        borrowed.type_name
                                    ),
                                    expr.span,
                                )
                            })
                    }
                    other => Err(self.type_error(
                        " in field access",
                        "a struct value",
                        &other,
                        expr.span,
                    )),
                }
            }
            ExprKind::Match { scrutinee, arms } => self.eval_match_expr(scrutinee, arms, expr.span),
        }
    }

    fn eval_args(&mut self, args: &[Arg]) -> Result<Vec<Value>, RuntimeError> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval_expr(&arg.value)?);
        }
        Ok(values)
    }

    fn index_position(&self, value: &Value, span: Span) -> Result<usize, RuntimeError> {
        match value {
            Value::Int(index) => {
                if *index < 0 {
                    Err(self.rt(
                        "R0003",
                        format!("negative index {index} is not allowed"),
                        span,
                    ))
                } else {
                    Ok(*index as usize)
                }
            }
            other => Err(self.type_error(" as an index", "`Int`", other, span)),
        }
    }

    fn index_value(&self, base: Value, index: Value, span: Span) -> Result<Value, RuntimeError> {
        match base {
            Value::List(items) => {
                let position = self.index_position(&index, span)?;
                let borrowed = items.borrow();
                borrowed.get(position).cloned().ok_or_else(|| {
                    self.rt(
                        "R0003",
                        format!(
                            "index {position} is out of bounds for a list of length {}",
                            borrowed.len()
                        ),
                        span,
                    )
                })
            }
            Value::Str(text) => {
                let position = self.index_position(&index, span)?;
                match text.chars().nth(position) {
                    Some(character) => Ok(Value::str(character.to_string())),
                    None => Err(self.rt(
                        "R0003",
                        format!(
                            "index {position} is out of bounds for a string of length {}",
                            text.chars().count()
                        ),
                        span,
                    )),
                }
            }
            Value::Map(entries) => {
                let borrowed = entries.borrow();
                map_get(borrowed.as_slice(), &index).ok_or_else(|| {
                    self.rt(
                        "R0008",
                        format!("key `{}` is not present in the map", index.display()),
                        span,
                    )
                })
            }
            other => Err(self.type_error(" in indexing", "a List, Str, or Map", &other, span)),
        }
    }

    // ---- operators ----

    fn binary_op(
        &self,
        op: BinOp,
        left: &Value,
        right: &Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match op {
            BinOp::Add => match (left, right) {
                (Value::Int(a), Value::Int(b)) => a
                    .checked_add(*b)
                    .map(Value::Int)
                    .ok_or_else(|| self.rt("R0002", "integer overflow in `+`", span)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::Str(a), Value::Str(b)) => Ok(Value::str(format!("{}{}", &**a, &**b))),
                _ => Err(self.binary_type_error("+", left, right, span)),
            },
            BinOp::Sub => match (left, right) {
                (Value::Int(a), Value::Int(b)) => a
                    .checked_sub(*b)
                    .map(Value::Int)
                    .ok_or_else(|| self.rt("R0002", "integer overflow in `-`", span)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
                _ => Err(self.binary_type_error("-", left, right, span)),
            },
            BinOp::Mul => match (left, right) {
                (Value::Int(a), Value::Int(b)) => a
                    .checked_mul(*b)
                    .map(Value::Int)
                    .ok_or_else(|| self.rt("R0002", "integer overflow in `*`", span)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
                _ => Err(self.binary_type_error("*", left, right, span)),
            },
            BinOp::Div => match (left, right) {
                (Value::Int(a), Value::Int(b)) => {
                    if *b == 0 {
                        return Err(self.rt("R0001", "division by zero", span));
                    }
                    Ok(Value::Float(*a as f64 / *b as f64))
                }
                (Value::Float(a), Value::Float(b)) => {
                    if *b == 0.0 {
                        return Err(self.rt("R0001", "division by zero", span));
                    }
                    Ok(Value::Float(a / b))
                }
                _ => Err(self.binary_type_error("/", left, right, span)),
            },
            BinOp::FloorDiv => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(self.floor_div(*a, *b, span)?)),
                _ => Err(self.binary_type_error("//", left, right, span)),
            },
            BinOp::Mod => match (left, right) {
                (Value::Int(a), Value::Int(b)) => {
                    if *b == 0 {
                        return Err(self.rt("R0001", "division by zero", span));
                    }
                    a.checked_rem(*b)
                        .map(Value::Int)
                        .ok_or_else(|| self.rt("R0002", "integer overflow in `%`", span))
                }
                (Value::Float(a), Value::Float(b)) => {
                    if *b == 0.0 {
                        return Err(self.rt("R0001", "division by zero", span));
                    }
                    Ok(Value::Float(a % b))
                }
                _ => Err(self.binary_type_error("%", left, right, span)),
            },
            BinOp::Pow => match (left, right) {
                (Value::Int(base), Value::Int(exponent)) => {
                    if *exponent < 0 {
                        return Err(self.rt(
                            "R0004",
                            "`**` requires a non-negative Int exponent",
                            span,
                        ));
                    }
                    if *exponent > u32::MAX as i64 {
                        return match *base {
                            0 => Ok(Value::Int(0)),
                            1 => Ok(Value::Int(1)),
                            -1 => Ok(Value::Int(if exponent % 2 == 0 { 1 } else { -1 })),
                            _ => Err(self.rt("R0002", "integer overflow in `**`", span)),
                        };
                    }
                    base.checked_pow(*exponent as u32)
                        .map(Value::Int)
                        .ok_or_else(|| self.rt("R0002", "integer overflow in `**`", span))
                }
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(*b))),
                _ => Err(self.binary_type_error("**", left, right, span)),
            },
            BinOp::Eq => Ok(Value::Bool(left.equals(right))),
            BinOp::NotEq => Ok(Value::Bool(!left.equals(right))),
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                let ordering = match (left, right) {
                    (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
                    (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
                    (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
                    _ => return Err(self.binary_type_error(op.symbol(), left, right, span)),
                };
                use std::cmp::Ordering;
                let result = match ordering {
                    None => false,
                    Some(ordering) => match op {
                        BinOp::Lt => ordering == Ordering::Less,
                        BinOp::LtEq => ordering != Ordering::Greater,
                        BinOp::Gt => ordering == Ordering::Greater,
                        BinOp::GtEq => ordering != Ordering::Less,
                        _ => unreachable!(),
                    },
                };
                Ok(Value::Bool(result))
            }
            BinOp::And | BinOp::Or => Err(self.rt(
                "R0010",
                "internal error: short-circuit operator evaluated eagerly",
                span,
            )),
        }
    }

    fn binary_type_error(
        &self,
        symbol: &str,
        left: &Value,
        right: &Value,
        span: Span,
    ) -> RuntimeError {
        self.rt(
            "R0005",
            format!(
                "operator `{symbol}` cannot be applied to `{}` and `{}`",
                left.type_name(),
                right.type_name()
            ),
            span,
        )
    }

    fn floor_div(&self, a: i64, b: i64, span: Span) -> Result<i64, RuntimeError> {
        if b == 0 {
            return Err(self.rt("R0001", "division by zero", span));
        }
        let quotient = a
            .checked_div(b)
            .ok_or_else(|| self.rt("R0002", "integer overflow in `//`", span))?;
        let remainder = a
            .checked_rem(b)
            .ok_or_else(|| self.rt("R0002", "integer overflow in `//`", span))?;
        if remainder != 0 && ((remainder < 0) != (b < 0)) {
            quotient
                .checked_sub(1)
                .ok_or_else(|| self.rt("R0002", "integer overflow in `//`", span))
        } else {
            Ok(quotient)
        }
    }

    // ---- calls ----

    fn call_user(
        &mut self,
        body: &FnDecl,
        sig: &FnSig,
        args: Vec<Value>,
        call_span: Span,
    ) -> Result<Value, RuntimeError> {
        if args.len() != sig.params.len() {
            return Err(self.rt(
                "R0010",
                format!(
                    "function `{}` expects {} argument(s), found {}",
                    sig.name,
                    sig.params.len(),
                    args.len()
                ),
                call_span,
            ));
        }
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(self.rt(
                "R0012",
                format!(
                    "call depth limit of {MAX_CALL_DEPTH} exceeded; possible infinite recursion"
                ),
                call_span,
            ));
        }
        self.call_depth += 1;
        self.call_stack.push(StackFrame {
            function: sig.name.clone(),
            span: call_span,
        });
        self.scopes.push(HashMap::new());
        let result = self.invoke_function(body, sig, args);
        self.scopes.pop();
        self.call_stack.pop();
        self.call_depth -= 1;
        result
    }

    fn invoke_function(
        &mut self,
        body: &FnDecl,
        sig: &FnSig,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        for (param, value) in sig.params.iter().zip(args) {
            self.validate(
                &value,
                &param.ty,
                param.span,
                &format!(" for parameter `{}`", param.name),
            )?;
            self.bind(&param.name, value, false, param.ty.clone(), param.span)?;
        }
        let flow = self.exec_stmts(&body.body.stmts)?;
        let value = match flow {
            Flow::Normal => Value::Unit,
            Flow::Return(value) => value,
            Flow::Break | Flow::Continue => {
                return Err(self.rt("R0010", "loop control escaped a function body", body.span));
            }
        };
        if sig.ret_annotated {
            self.validate(
                &value,
                &sig.ret,
                body.span,
                &format!(" in return from `{}`", sig.name),
            )?;
        }
        Ok(value)
    }

    fn call_constructor(
        &mut self,
        name: &str,
        args: &[Arg],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let info = self
            .model
            .structs
            .get(name)
            .cloned()
            .ok_or_else(|| self.rt("R0010", format!("unknown struct `{name}`"), span))?;
        let any_named = args.iter().any(|arg| arg.name.is_some());
        let all_named = args.iter().all(|arg| arg.name.is_some());
        if any_named && !all_named {
            return Err(self.rt(
                "R0010",
                "cannot mix named and positional arguments in a constructor",
                span,
            ));
        }

        if any_named {
            let mut provided: HashMap<String, Value> = HashMap::new();
            for arg in args {
                let Some((field_name, field_span)) = &arg.name else {
                    continue;
                };
                let value = self.eval_expr(&arg.value)?;
                if provided.insert(field_name.clone(), value).is_some() {
                    return Err(self.rt(
                        "R0010",
                        format!("duplicate argument `{field_name}`"),
                        *field_span,
                    ));
                }
            }
            let mut fields = Vec::with_capacity(info.fields.len());
            for field in &info.fields {
                let Some(value) = provided.remove(&field.name) else {
                    return Err(self.rt(
                        "R0010",
                        format!("missing field `{}` for struct `{name}`", field.name),
                        span,
                    ));
                };
                self.validate(
                    &value,
                    &field.ty,
                    span,
                    &format!(" for field `{}`", field.name),
                )?;
                fields.push(FieldSlot {
                    name: field.name.clone(),
                    ty: field.ty.clone(),
                    value,
                });
            }
            if let Some(extra) = provided.keys().next() {
                return Err(self.rt(
                    "R0010",
                    format!("struct `{name}` has no field `{extra}`"),
                    span,
                ));
            }
            Ok(Value::struct_value(name, fields))
        } else {
            if args.len() != info.fields.len() {
                return Err(self.rt(
                    "R0010",
                    format!(
                        "struct `{name}` expects {} field(s), found {}",
                        info.fields.len(),
                        args.len()
                    ),
                    span,
                ));
            }
            let mut fields = Vec::with_capacity(info.fields.len());
            for (arg, field) in args.iter().zip(info.fields.iter()) {
                let value = self.eval_expr(&arg.value)?;
                self.validate(
                    &value,
                    &field.ty,
                    arg.span,
                    &format!(" for field `{}`", field.name),
                )?;
                fields.push(FieldSlot {
                    name: field.name.clone(),
                    ty: field.ty.clone(),
                    value,
                });
            }
            Ok(Value::struct_value(name, fields))
        }
    }

    fn call_variant_constructor(
        &mut self,
        enum_name: &str,
        variant: &VariantInfo,
        args: &[Arg],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        for arg in args {
            if let Some((arg_name, _)) = &arg.name {
                return Err(self.rt(
                    "R0010",
                    format!("named argument `{arg_name}` is not valid for variant constructors"),
                    span,
                ));
            }
        }
        if args.len() != variant.fields.len() {
            return Err(self.rt(
                "R0010",
                format!(
                    "variant `{}` expects {} payload value(s), found {}",
                    variant.name,
                    variant.fields.len(),
                    args.len()
                ),
                span,
            ));
        }
        let mut fields = Vec::with_capacity(variant.fields.len());
        for (arg, field) in args.iter().zip(variant.fields.iter()) {
            let value = self.eval_expr(&arg.value)?;
            self.validate(
                &value,
                &field.ty,
                arg.span,
                &format!(" for payload field `{}`", field.name),
            )?;
            fields.push(FieldSlot {
                name: field.name.clone(),
                ty: field.ty.clone(),
                value,
            });
        }
        Ok(Value::enum_value(enum_name, variant.name.clone(), fields))
    }

    // ---- builtins ----

    fn call_builtin(
        &mut self,
        name: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "print" => {
                let text = args
                    .iter()
                    .map(Value::display)
                    .collect::<Vec<_>>()
                    .join(" ");
                if !self.muted {
                    let write_result = writeln!(self.out, "{text}");
                    if let Err(error) = write_result {
                        return Err(self.rt(
                            "R0013",
                            format!("failed to write output: {error}"),
                            span,
                        ));
                    }
                }
                Ok(Value::Unit)
            }
            "len" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Str(text) => Ok(Value::Int(text.chars().count() as i64)),
                    Value::List(items) => Ok(Value::Int(items.borrow().len() as i64)),
                    Value::Map(entries) => Ok(Value::Int(entries.borrow().len() as i64)),
                    other => {
                        Err(self.type_error(" in `len`", "`Str`, `List`, or `Map`", other, span))
                    }
                }
            }
            "range" => {
                self.arity(name, &args, 1, 3, span)?;
                let numbers = self.expect_ints(name, &args, span)?;
                let (start, end, step) = match numbers.as_slice() {
                    [end] => (0, *end, 1),
                    [start, end] => (*start, *end, 1),
                    [start, end, step] => (*start, *end, *step),
                    _ => unreachable!(),
                };
                if step == 0 {
                    return Err(self.rt("R0004", "`range` step must not be zero", span));
                }
                let step_wide = step as i128;
                let start_wide = start as i128;
                let end_wide = end as i128;
                let count = if step_wide > 0 {
                    if start_wide >= end_wide {
                        0
                    } else {
                        (end_wide - start_wide + step_wide - 1) / step_wide
                    }
                } else if start_wide <= end_wide {
                    0
                } else {
                    (start_wide - end_wide + (-step_wide) - 1) / (-step_wide)
                };
                if count > MAX_RANGE_LEN as i128 {
                    return Err(self.rt(
                        "R0004",
                        format!(
                            "`range` would produce {count} elements, exceeding the limit of {MAX_RANGE_LEN}"
                        ),
                        span,
                    ));
                }
                let mut items = Vec::with_capacity(count as usize);
                for index in 0..count {
                    let value = start_wide + step_wide * index;
                    items.push(Value::Int(value as i64));
                }
                Ok(Value::list(items))
            }
            "push" => {
                self.arity(name, &args, 2, 2, span)?;
                match &args[0] {
                    Value::List(items) => {
                        items.borrow_mut().push(args[1].clone());
                        Ok(Value::Unit)
                    }
                    other => Err(self.type_error(" in `push`", "`List`", other, span)),
                }
            }
            "pop" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::List(items) => items
                        .borrow_mut()
                        .pop()
                        .ok_or_else(|| self.rt("R0003", "`pop` on an empty list", span)),
                    other => Err(self.type_error(" in `pop`", "`List`", other, span)),
                }
            }
            "contains" => {
                self.arity(name, &args, 2, 2, span)?;
                match &args[0] {
                    Value::List(items) => Ok(Value::Bool(
                        items.borrow().iter().any(|item| item.equals(&args[1])),
                    )),
                    Value::Str(text) => match &args[1] {
                        Value::Str(needle) => Ok(Value::Bool(text.contains(&**needle))),
                        other => Err(self.type_error(
                            " as the needle for `contains`",
                            "`Str`",
                            other,
                            span,
                        )),
                    },
                    Value::Map(entries) => Ok(Value::Bool(
                        map_get(entries.borrow().as_slice(), &args[1]).is_some(),
                    )),
                    other => Err(self.type_error(
                        " in `contains`",
                        "`List`, `Str`, or `Map`",
                        other,
                        span,
                    )),
                }
            }
            "split" => {
                self.arity(name, &args, 2, 2, span)?;
                let text = match &args[0] {
                    Value::Str(text) => text,
                    other => {
                        return Err(self.type_error(
                            " as the text for `split`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let separator = match &args[1] {
                    Value::Str(separator) => separator,
                    other => {
                        return Err(self.type_error(
                            " as the separator for `split`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let parts: Vec<Value> = if separator.is_empty() {
                    text.chars().map(|c| Value::str(c.to_string())).collect()
                } else {
                    text.split(&**separator).map(Value::str).collect()
                };
                Ok(Value::list(parts))
            }
            "join" => {
                self.arity(name, &args, 2, 2, span)?;
                let items = match &args[0] {
                    Value::List(items) => items.borrow(),
                    other => {
                        return Err(self.type_error(
                            " in `join`",
                            "`List[Str]`",
                            other,
                            span,
                        ));
                    }
                };
                let separator = match &args[1] {
                    Value::Str(separator) => separator,
                    other => {
                        return Err(self.type_error(
                            " as the separator for `join`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let mut parts = Vec::with_capacity(items.len());
                for item in items.iter() {
                    match item {
                        Value::Str(part) => parts.push(&**part),
                        other => {
                            return Err(self.type_error(
                                " as an element for `join`",
                                "`Str`",
                                other,
                                span,
                            ));
                        }
                    }
                }
                Ok(Value::str(parts.join(&**separator)))
            }
            "trim" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Str(text) => Ok(Value::str(text.trim_matches(is_text_whitespace))),
                    other => Err(self.type_error(" in `trim`", "`Str`", other, span)),
                }
            }
            "starts_with" | "ends_with" => {
                self.arity(name, &args, 2, 2, span)?;
                let text = match &args[0] {
                    Value::Str(text) => text,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the text for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let affix = match &args[1] {
                    Value::Str(affix) => affix,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the affix for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let matched = if name == "starts_with" {
                    text.starts_with(&**affix)
                } else {
                    text.ends_with(&**affix)
                };
                Ok(Value::Bool(matched))
            }
            "replace" => {
                self.arity(name, &args, 3, 3, span)?;
                let mut values = Vec::with_capacity(3);
                for (index, context) in [
                    (0, " as the text for `replace`"),
                    (1, " as the pattern for `replace`"),
                    (2, " as the replacement for `replace`"),
                ] {
                    match &args[index] {
                        Value::Str(value) => values.push(value.clone()),
                        other => {
                            return Err(self.type_error(context, "`Str`", other, span));
                        }
                    }
                }
                if values[1].is_empty() {
                    return Err(self.rt(
                        "R0016",
                        "`replace` requires a non-empty pattern",
                        span,
                    ));
                }
                Ok(Value::str(values[0].replace(&*values[1], &values[2])))
            }
            "path_join" => {
                self.arity(name, &args, 2, 2, span)?;
                let base = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            " as the base path for `path_join`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let child = match &args[1] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            " as the child path for `path_join`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                Ok(Value::str(path_join(base, child)))
            }
            "path_parent" | "path_name" | "path_stem" | "path_extension"
            | "path_is_absolute" => {
                self.arity(name, &args, 1, 1, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the path for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                match name {
                    "path_parent" => Ok(Value::str(path_parent(path))),
                    "path_name" => {
                        let (begin, end) = path_name_range(path);
                        Ok(Value::str(&path[begin..end]))
                    }
                    "path_stem" => Ok(Value::str(path_stem(path))),
                    "path_extension" => Ok(Value::str(path_extension(path))),
                    _ => Ok(Value::Bool(path.starts_with('/'))),
                }
            }
            "keys" | "values" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Map(entries) => {
                        let borrowed = entries.borrow();
                        let items = borrowed
                            .iter()
                            .map(|(key, value)| {
                                if name == "keys" {
                                    key.clone()
                                } else {
                                    value.clone()
                                }
                            })
                            .collect();
                        Ok(Value::list(items))
                    }
                    other => Err(self.type_error(&format!(" in `{name}`"), "`Map`", other, span)),
                }
            }
            "str" => {
                self.arity(name, &args, 1, 1, span)?;
                Ok(Value::str(args[0].display()))
            }
            "int" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Int(number) => Ok(Value::Int(*number)),
                    Value::Float(number) => {
                        let min = -9_223_372_036_854_775_808.0_f64;
                        let max_exclusive = 9_223_372_036_854_775_808.0_f64;
                        if !number.is_finite()
                            || number.fract() != 0.0
                            || *number < min
                            || *number >= max_exclusive
                        {
                            return Err(self.rt(
                                "R0007",
                                format!("cannot convert `{number}` to Int exactly"),
                                span,
                            ));
                        }
                        Ok(Value::Int(*number as i64))
                    }
                    Value::Str(text) => text.trim().parse::<i64>().map(Value::Int).map_err(|_| {
                        self.rt("R0007", format!("cannot convert `{text}` to Int"), span)
                    }),
                    other => Err(self.rt(
                        "R0007",
                        format!("cannot convert `{}` to Int", other.type_name()),
                        span,
                    )),
                }
            }
            "float" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Int(number) => Ok(Value::Float(*number as f64)),
                    Value::Float(number) => Ok(Value::Float(*number)),
                    Value::Str(text) => {
                        text.trim().parse::<f64>().map(Value::Float).map_err(|_| {
                            self.rt("R0007", format!("cannot convert `{text}` to Float"), span)
                        })
                    }
                    other => Err(self.rt(
                        "R0007",
                        format!("cannot convert `{}` to Float", other.type_name()),
                        span,
                    )),
                }
            }
            "bool" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Bool(flag) => Ok(Value::Bool(*flag)),
                    other => Err(self.rt(
                        "R0007",
                        format!(
                            "cannot convert `{}` to Bool; only Bool is accepted",
                            other.type_name()
                        ),
                        span,
                    )),
                }
            }
            "type" => {
                self.arity(name, &args, 1, 1, span)?;
                Ok(Value::str(args[0].type_name()))
            }
            "abs" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Int(number) => number
                        .checked_abs()
                        .map(Value::Int)
                        .ok_or_else(|| self.rt("R0002", "integer overflow in `abs`", span)),
                    Value::Float(number) => Ok(Value::Float(number.abs())),
                    other => Err(self.type_error(" in `abs`", "`Int` or `Float`", other, span)),
                }
            }
            "min" | "max" => {
                self.arity(name, &args, 2, 2, span)?;
                match (&args[0], &args[1]) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(if name == "min" {
                        (*a).min(*b)
                    } else {
                        (*a).max(*b)
                    })),
                    (Value::Float(a), Value::Float(b)) => Ok(Value::Float(if name == "min" {
                        (*a).min(*b)
                    } else {
                        (*a).max(*b)
                    })),
                    (a, b) => Err(self.rt(
                        "R0005",
                        format!(
                            "`{name}` expects two Ints or two Floats, found `{}` and `{}`",
                            a.type_name(),
                            b.type_name()
                        ),
                        span,
                    )),
                }
            }
            "sqrt" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Float(number) => {
                        if *number < 0.0 {
                            return Err(self.rt("R0004", "`sqrt` of a negative number", span));
                        }
                        Ok(Value::Float(number.sqrt()))
                    }
                    other => Err(self.type_error(" in `sqrt`", "`Float`", other, span)),
                }
            }
            "floor" | "ceil" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Float(number) => {
                        let result = if name == "floor" {
                            number.floor()
                        } else {
                            number.ceil()
                        };
                        if !result.is_finite()
                            || result < -9_223_372_036_854_775_808.0
                            || result >= 9_223_372_036_854_775_808.0
                        {
                            return Err(self.rt(
                                "R0007",
                                format!("`{name}` result does not fit in Int"),
                                span,
                            ));
                        }
                        Ok(Value::Int(result as i64))
                    }
                    other => Err(self.type_error(&format!(" in `{name}`"), "`Float`", other, span)),
                }
            }
            "assert" => {
                self.arity(name, &args, 1, 1, span)?;
                match &args[0] {
                    Value::Bool(true) => Ok(Value::Unit),
                    Value::Bool(false) => Err(self.rt("R0006", "assertion failed", span)),
                    other => Err(self.type_error(" in `assert`", "`Bool`", other, span)),
                }
            }
            "read_file" => {
                self.arity(name, &args, 1, 1, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            " as the path for `read_file`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if path.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                let file = std::fs::File::open(&**path).map_err(|error| {
                    self.rt(
                        "R0015",
                        format!("cannot read file `{path}`: {error}"),
                        span,
                    )
                })?;
                if let Ok(metadata) = file.metadata()
                    && metadata.len() > MAX_FILE_BYTES
                {
                    return Err(self.rt(
                        "R0015",
                        format!("file `{path}` exceeds the 64 MiB read limit"),
                        span,
                    ));
                }
                let mut bytes = Vec::new();
                file.take(MAX_FILE_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| {
                        self.rt(
                            "R0015",
                            format!("cannot read file `{path}`: {error}"),
                            span,
                        )
                    })?;
                if bytes.len() as u64 > MAX_FILE_BYTES {
                    return Err(self.rt(
                        "R0015",
                        format!("file `{path}` exceeds the 64 MiB read limit"),
                        span,
                    ));
                }
                if bytes.contains(&0) {
                    return Err(self.rt(
                        "R0015",
                        format!("file `{path}` contains a NUL byte"),
                        span,
                    ));
                }
                let text = String::from_utf8(bytes).map_err(|_| {
                    self.rt(
                        "R0015",
                        format!("file `{path}` contents are not valid UTF-8"),
                        span,
                    )
                })?;
                Ok(Value::str(text))
            }
            "write_file" | "append_file" => {
                self.arity(name, &args, 2, 2, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the path for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let text = match &args[1] {
                    Value::Str(text) => text,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the contents for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if path.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                if text.contains('\0') {
                    return Err(self.rt(
                        "R0015",
                        "file contents contain a NUL byte",
                        span,
                    ));
                }
                let mut file = if name == "append_file" {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&**path)
                } else {
                    std::fs::File::create(&**path)
                }
                .map_err(|error| {
                    self.rt(
                        "R0015",
                        format!("cannot write file `{path}`: {error}"),
                        span,
                    )
                })?;
                file.write_all(text.as_bytes())
                    .and_then(|_| file.flush())
                    .map_err(|error| {
                        self.rt(
                            "R0015",
                            format!("cannot write file `{path}`: {error}"),
                            span,
                        )
                    })?;
                Ok(Value::Unit)
            }
            "file_size" => {
                self.arity(name, &args, 1, 1, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            " as a file path for `file_size`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if path.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                let metadata = std::fs::metadata(&**path).map_err(|error| {
                    self.rt(
                        "R0015",
                        format!("cannot read metadata for `{path}`: {error}"),
                        span,
                    )
                })?;
                if !metadata.is_file() {
                    return Err(self.rt(
                        "R0015",
                        format!("`{path}` is not a regular file"),
                        span,
                    ));
                }
                let size = i64::try_from(metadata.len()).map_err(|_| {
                    self.rt(
                        "R0015",
                        format!("file `{path}` is too large to represent as Int"),
                        span,
                    )
                })?;
                Ok(Value::Int(size))
            }
            "is_file" | "is_dir" | "is_symlink" => {
                self.arity(name, &args, 1, 1, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            &format!(" as a file path for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if path.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                let result = match name {
                    "is_file" => std::path::Path::new(&**path).is_file(),
                    "is_dir" => std::path::Path::new(&**path).is_dir(),
                    _ => std::fs::symlink_metadata(&**path)
                        .map(|metadata| metadata.file_type().is_symlink())
                        .unwrap_or(false),
                };
                Ok(Value::Bool(result))
            }
            "file_exists" => {
                self.arity(name, &args, 1, 1, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            " as the path for `file_exists`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if path.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                Ok(Value::Bool(std::path::Path::new(&**path).exists()))
            }
            "list_dir" => {
                self.arity(name, &args, 1, 1, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            " as the directory path for `list_dir`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if path.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                let entries = std::fs::read_dir(&**path).map_err(|error| {
                    self.rt(
                        "R0015",
                        format!("cannot list directory `{path}`: {error}"),
                        span,
                    )
                })?;
                let mut names = Vec::new();
                for entry in entries {
                    let entry = entry.map_err(|error| {
                        self.rt(
                            "R0015",
                            format!("cannot list directory `{path}`: {error}"),
                            span,
                        )
                    })?;
                    let name = entry.file_name().into_string().map_err(|_| {
                        self.rt(
                            "R0015",
                            format!("directory `{path}` contains a name that is not valid UTF-8"),
                            span,
                        )
                    })?;
                    names.push(name);
                }
                names.sort();
                Ok(Value::list(names.into_iter().map(Value::str).collect()))
            }
            "glob" => {
                self.arity(name, &args, 1, 1, span)?;
                let pattern = match &args[0] {
                    Value::Str(pattern) => pattern,
                    other => {
                        return Err(self.type_error(
                            " as the pattern for `glob`",
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if pattern.contains('\0') {
                    return Err(self.rt("R0015", "glob pattern contains a NUL byte", span));
                }
                let paths = crate::glob::glob(pattern).map_err(|error| {
                    let code = match error.kind {
                        crate::glob::GlobErrorKind::Invalid => "R0016",
                        crate::glob::GlobErrorKind::Io => "R0015",
                    };
                    self.rt(code, error.message, span)
                })?;
                Ok(Value::list(paths.into_iter().map(Value::str).collect()))
            }
            "make_dir" | "remove_file" | "remove_dir" | "make_dir_all" | "remove_dir_all" => {
                self.arity(name, &args, 1, 1, span)?;
                let path = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the path for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if path.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                let outcome = match name {
                    "make_dir" => std::fs::create_dir(&**path),
                    "make_dir_all" => std::fs::create_dir_all(&**path),
                    "remove_file" => std::fs::remove_file(&**path),
                    "remove_dir_all" => {
                        // Refuse destructive roots before touching the
                        // filesystem, then trim trailing separators before the
                        // root type check so a symlink named with a trailing
                        // slash is unlinked rather than followed.
                        if let Some(refusal) = crate::fs_safety::refused_removal_root(path) {
                            return Err(self.rt(
                                "R0015",
                                format!(
                                    "cannot remove directory `{path}`: {}",
                                    refusal.message()
                                ),
                                span,
                            ));
                        }
                        let trimmed = path.trim_end_matches('/');
                        let target = if trimmed.is_empty() { &**path } else { trimmed };
                        std::fs::remove_dir_all(target)
                    }
                    _ => std::fs::remove_dir(&**path),
                };
                outcome.map_err(|error| {
                    let action = match name {
                        "make_dir" | "make_dir_all" => "create directory",
                        "remove_file" => "remove file",
                        _ => "remove directory",
                    };
                    self.rt(
                        "R0015",
                        format!("cannot {action} `{path}`: {error}"),
                        span,
                    )
                })?;
                Ok(Value::Unit)
            }
            "rename" | "copy_file" | "copy_dir_all" => {
                self.arity(name, &args, 2, 2, span)?;
                let source = match &args[0] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the source path for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                let target = match &args[1] {
                    Value::Str(path) => path,
                    other => {
                        return Err(self.type_error(
                            &format!(" as the target path for `{name}`"),
                            "`Str`",
                            other,
                            span,
                        ));
                    }
                };
                if source.contains('\0') || target.contains('\0') {
                    return Err(self.rt("R0015", "file path contains a NUL byte", span));
                }
                if name == "copy_dir_all" {
                    crate::copy_tree::copy_dir_all(source, target)
                        .map_err(|message| self.rt("R0015", message, span))?;
                    return Ok(Value::Unit);
                }
                if name == "rename" {
                    std::fs::rename(&**source, &**target).map_err(|error| {
                        self.rt(
                            "R0015",
                            format!("cannot rename `{source}` to `{target}`: {error}"),
                            span,
                        )
                    })?;
                    return Ok(Value::Unit);
                }
                let metadata = std::fs::metadata(&**source).map_err(|error| {
                    self.rt(
                        "R0015",
                        format!("cannot copy file `{source}`: {error}"),
                        span,
                    )
                })?;
                if !metadata.is_file() {
                    return Err(self.rt(
                        "R0015",
                        format!("cannot copy file `{source}`: not a regular file"),
                        span,
                    ));
                }
                let mut input = std::fs::File::open(&**source).map_err(|error| {
                    self.rt(
                        "R0015",
                        format!("cannot copy file `{source}`: {error}"),
                        span,
                    )
                })?;
                let mut output = std::fs::File::create(&**target).map_err(|error| {
                    self.rt(
                        "R0015",
                        format!("cannot copy file `{source}` to `{target}`: {error}"),
                        span,
                    )
                })?;
                std::io::copy(&mut input, &mut output)
                    .and_then(|_| output.flush())
                    .map_err(|error| {
                        self.rt(
                            "R0015",
                            format!("cannot copy file `{source}` to `{target}`: {error}"),
                            span,
                        )
                    })?;
                Ok(Value::Unit)
            }
            _ => Err(self.rt("R0010", format!("unknown built-in function `{name}`"), span)),
        }
    }

    fn arity(
        &self,
        name: &str,
        args: &[Value],
        min: usize,
        max: usize,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if args.len() < min || args.len() > max {
            let expected = if min == max {
                min.to_string()
            } else {
                format!("{min} to {max}")
            };
            return Err(self.rt(
                "R0010",
                format!(
                    "`{name}` expects {expected} argument(s), found {}",
                    args.len()
                ),
                span,
            ));
        }
        Ok(())
    }

    fn expect_ints(
        &self,
        name: &str,
        args: &[Value],
        span: Span,
    ) -> Result<Vec<i64>, RuntimeError> {
        let mut numbers = Vec::with_capacity(args.len());
        for arg in args {
            match arg {
                Value::Int(number) => numbers.push(*number),
                other => {
                    return Err(self.type_error(&format!(" in `{name}`"), "`Int`", other, span));
                }
            }
        }
        Ok(numbers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker;
    use crate::lexer;
    use crate::parser;

    fn run_source(source: &str) -> Result<String, RuntimeError> {
        let lexed = lexer::lex(source);
        assert!(!lexed.diagnostics.has_errors(), "{:?}", lexed.diagnostics);
        let (program, parse_diagnostics) = parser::parse(source, lexed.tokens);
        assert!(!parse_diagnostics.has_errors(), "{:?}", parse_diagnostics);
        let (model, check_diagnostics) = checker::check(&program);
        assert!(
            !check_diagnostics.has_errors(),
            "{:?}",
            check_diagnostics.items
        );
        let model = model.expect("model");
        let mut output = Vec::new();
        run(&program, &model, &mut output)?;
        Ok(String::from_utf8(output).expect("utf-8 output"))
    }

    fn run_ok(source: &str) -> String {
        run_source(source).expect("runtime success")
    }

    fn run_err(source: &str) -> RuntimeError {
        run_source(source).expect_err("expected runtime error")
    }

    #[test]
    fn executes_top_level_statements() {
        assert_eq!(run_ok("print(\"hello\")\n"), "hello\n");
    }

    #[test]
    fn calls_main_when_there_is_no_top_level_code() {
        assert_eq!(
            run_ok("fn main():\n    print(\"from main\")\n"),
            "from main\n"
        );
    }

    #[test]
    fn functions_see_top_level_bindings() {
        assert_eq!(
            run_ok("let greeting = \"hi\"\nfn greet():\n    print(greeting)\n\ngreet()\n"),
            "hi\n"
        );
    }

    #[test]
    fn arithmetic_is_checked() {
        let error = run_err("print(9223372036854775807 + 1)\n");
        assert_eq!(error.code, "R0002");
    }

    #[test]
    fn division_by_zero_is_reported() {
        assert_eq!(run_err("print(1 // 0)\n").code, "R0001");
        assert_eq!(run_err("print(1 / 0)\n").code, "R0001");
        assert_eq!(run_err("print(1.0 % 0.0)\n").code, "R0001");
    }

    #[test]
    fn slash_always_produces_a_float() {
        assert_eq!(run_ok("print(7 / 2)\n"), "3.5\n");
    }

    #[test]
    fn floor_division_rounds_toward_negative_infinity() {
        assert_eq!(run_ok("print(-7 // 2)\n"), "-4\n");
    }

    #[test]
    fn power_requires_a_non_negative_exponent() {
        assert_eq!(run_err("print(2 ** -1)\n").code, "R0004");
        assert_eq!(run_ok("print(2 ** 10)\n"), "1024\n");
    }

    #[test]
    fn interpolation_uses_display_form() {
        assert_eq!(
            run_ok("let name = \"Ada\"\nprint(\"hello {name}, {1 + 2}\")\n"),
            "hello Ada, 3\n"
        );
    }

    #[test]
    fn lists_have_reference_semantics() {
        assert_eq!(
            run_ok("let xs = [1]\nlet ys = xs\npush(ys, 2)\nprint(xs)\n"),
            "[1, 2]\n"
        );
    }

    #[test]
    fn structs_have_reference_semantics_and_mutable_fields() {
        assert_eq!(
            run_ok(
                "struct Point:\n    x: Int\n    y: Int\n\n\
                 let p = Point(x=1, y=2)\nlet q = p\nq.x = 9\nprint(p)\n"
            ),
            "Point(x=9, y=2)\n"
        );
    }

    #[test]
    fn for_loops_iterate_lists_strings_and_maps() {
        assert_eq!(
            run_ok(
                "for x in [1, 2]:\n    print(x)\n\
                 for c in \"ab\":\n    print(c)\n\
                 for k in {\"a\": 1, \"b\": 2}:\n    print(k)\n"
            ),
            "1\n2\na\nb\na\nb\n"
        );
    }

    #[test]
    fn break_and_continue_work() {
        assert_eq!(
            run_ok(
                "for x in range(10):\n    if x == 2:\n        continue\n    if x == 4:\n        break\n    print(x)\n"
            ),
            "0\n1\n3\n"
        );
    }

    #[test]
    fn recursion_works_and_is_bounded() {
        assert_eq!(
            run_ok(
                "fn fact(n: Int) -> Int:\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\n\nprint(fact(10))\n"
            ),
            "3628800\n"
        );
        assert_eq!(
            run_err("fn loop(n: Int) -> Int:\n    return loop(n + 1)\n\nloop(0)\n").code,
            "R0012"
        );
    }

    #[test]
    fn runtime_checks_dynamic_parameter_boundaries() {
        let error = run_err(
            "fn identity(x: Any) -> Any:\n    return x\n\n\
             fn takes(n: Int) -> Int:\n    return n\n\n\
             let value = identity(\"no\")\ntakes(value)\n",
        );
        assert_eq!(error.code, "R0005");
    }

    #[test]
    fn annotated_binding_rejects_dynamic_wrong_type() {
        let error =
            run_err("fn identity(x: Any) -> Any:\n    return x\n\nlet n: Int = identity(\"no\")\n");
        assert_eq!(error.code, "R0005");
    }

    #[test]
    fn index_out_of_bounds_is_reported() {
        assert_eq!(run_err("print([1][3])\n").code, "R0003");
        assert_eq!(run_err("print(\"a\"[1])\n").code, "R0003");
    }

    #[test]
    fn missing_map_key_is_reported() {
        assert_eq!(run_err("print({\"a\": 1}[\"b\"])\n").code, "R0008");
    }

    #[test]
    fn map_assignment_inserts_new_keys() {
        assert_eq!(
            run_ok("var m = {\"a\": 1}\nm[\"b\"] = 2\nprint(m)\n"),
            "{\"a\": 1, \"b\": 2}\n"
        );
    }

    #[test]
    fn assert_failure_is_reported() {
        assert_eq!(run_err("assert(false)\n").code, "R0006");
    }

    #[test]
    fn conversions_are_explicit() {
        assert_eq!(run_ok("print(int(2.0), float(2), str(3))\n"), "2 2.0 3\n");
        assert_eq!(run_err("print(int(2.5))\n").code, "R0007");
        assert_eq!(run_err("print(bool(1))\n").code, "R0007");
    }

    #[test]
    fn range_validates_arguments() {
        assert_eq!(run_err("print(range(0, 10, 0))\n").code, "R0004");
        assert_eq!(
            run_ok("print(range(3), range(1, 4), range(5, 0, -2))\n"),
            "[0, 1, 2] [1, 2, 3] [5, 3, 1]\n"
        );
    }

    #[test]
    fn output_write_errors_are_reported() {
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("nope"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let source = "print(1)\n";
        let lexed = lexer::lex(source);
        let (program, _) = parser::parse(source, lexed.tokens);
        let (model, _) = checker::check(&program);
        let model = model.expect("model");
        let error = run(&program, &model, &mut FailingWriter).expect_err("write must fail");
        assert_eq!(error.code, "R0013");
    }
}
