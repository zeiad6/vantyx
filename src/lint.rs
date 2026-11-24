//! Non-fatal static lints over a checked program.
//!
//! These lints never change program behavior; they are reported only by
//! `vantyx lint`. The interpreter remains the semantic oracle, so the linter
//! deliberately stays conservative: when a construct is ambiguous, it does not
//! report a finding.
//!
//! Codes:
//! - `L0001`: a local or global binding is never read.
//! - `L0002`: a `var` binding is read but never reassigned; `let` would do.
//! - `L0003`: a statement follows `return`, `break`, or `continue` in the same
//!   block.

use crate::ast::{
    Block, Expr, ExprKind, FnDecl, Item, MatchArm, MatchArmBody, Pattern, PatternArg, Program, Stmt,
    StmtKind, StrPart,
};
use crate::diagnostic::{Diagnostic, Diagnostics};
use crate::span::Span;
use std::collections::HashMap;

/// Runs every lint pass over a checked program.
pub fn lint(program: &Program) -> Diagnostics {
    let mut linter = Linter::new();
    linter.run(program);
    linter
        .diagnostics
        .items
        .sort_by_key(|diagnostic| {
            (
                diagnostic.span.file,
                diagnostic.span.start,
                diagnostic.span.end,
            )
        });
    linter.diagnostics
}

struct Binding {
    name: String,
    span: Span,
    mutable: bool,
    read: bool,
    written: bool,
    tracked: bool,
}

#[derive(Default)]
struct Scope {
    names: HashMap<String, usize>,
    declared: Vec<usize>,
}

struct Linter {
    bindings: Vec<Binding>,
    scopes: Vec<Scope>,
    diagnostics: Diagnostics,
}

impl Linter {
    fn new() -> Self {
        Self {
            bindings: Vec::new(),
            scopes: Vec::new(),
            diagnostics: Diagnostics::new(),
        }
    }

    fn run(&mut self, program: &Program) {
        self.push_scope();
        // Top-level `let`/`var` declarations are globals and may be read from
        // any function, including functions declared before them.
        for item in &program.items {
            if let Item::Stmt(Stmt {
                kind:
                    StmtKind::Let {
                        mutable,
                        name,
                        name_span,
                        ..
                    },
                ..
            }) = item
            {
                self.declare(name, *name_span, *mutable, true);
            }
        }
        for item in &program.items {
            match item {
                Item::Fn(function) => self.function(function),
                Item::Stmt(stmt) => {
                    if let StmtKind::Let { init, .. } = &stmt.kind {
                        // The binding was declared above; walk only the
                        // initializer so it is not declared twice.
                        self.expr(init);
                    } else {
                        self.stmt(stmt);
                    }
                }
                Item::Struct(_) | Item::Enum(_) | Item::Import(_) => {}
            }
        }
        self.pop_scope();
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::default());
    }

    fn declare(&mut self, name: &str, span: Span, mutable: bool, tracked: bool) {
        if name == "_" {
            return;
        }
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        // Or-pattern alternatives bind the same names in one arm scope. The
        // checker rejects genuine duplicates, so keep the first declaration.
        if scope.names.contains_key(name) {
            return;
        }
        // A leading underscore marks an intentionally unused binding.
        let tracked = tracked && !name.starts_with('_');
        let index = self.bindings.len();
        self.bindings.push(Binding {
            name: name.to_string(),
            span,
            mutable,
            read: false,
            written: false,
            tracked,
        });
        scope.names.insert(name.to_string(), index);
        scope.declared.push(index);
    }

    fn lookup(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.names.get(name).copied())
    }

    fn read(&mut self, name: &str) {
        if let Some(index) = self.lookup(name) {
            self.bindings[index].read = true;
        }
    }

    fn write(&mut self, name: &str) {
        if let Some(index) = self.lookup(name) {
            self.bindings[index].written = true;
        }
    }

    fn pop_scope(&mut self) {
        let Some(scope) = self.scopes.pop() else {
            return;
        };
        for index in scope.declared {
            let binding = &self.bindings[index];
            if !binding.tracked {
                continue;
            }
            let (name, span, mutable, read, written) = (
                binding.name.clone(),
                binding.span,
                binding.mutable,
                binding.read,
                binding.written,
            );
            if !read {
                self.diagnostics.push(
                    Diagnostic::warning("L0001", format!("unused binding `{name}`"), span)
                        .with_help("remove it or prefix the name with `_`"),
                );
            } else if mutable && !written {
                self.diagnostics.push(
                    Diagnostic::warning(
                        "L0002",
                        format!("`var` binding `{name}` is never reassigned"),
                        span,
                    )
                    .with_help("use `let` instead"),
                );
            }
        }
    }

    fn function(&mut self, function: &FnDecl) {
        self.push_scope();
        for parameter in &function.params {
            // Unused parameters are intentionally not reported in this first
            // lint pass; they still participate in name resolution.
            self.declare(&parameter.name, parameter.name_span, false, false);
        }
        self.block(&function.body);
        self.pop_scope();
    }

    fn block(&mut self, block: &Block) {
        self.push_scope();
        let mut terminated: Option<&'static str> = None;
        for statement in &block.stmts {
            if let Some(kind) = terminated {
                self.diagnostics.push(
                    Diagnostic::warning("L0003", "unreachable statement", statement.span)
                        .with_note(format!("this statement follows a `{kind}`")),
                );
                // Report only the first unreachable statement in the block and
                // do not lint dead code, which would produce cascading noise.
                break;
            }
            self.stmt(statement);
            terminated = direct_terminator(statement);
        }
        self.pop_scope();
    }

    fn stmt(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Let {
                mutable,
                name,
                name_span,
                init,
                ..
            } => {
                // The initializer is evaluated before the binding enters scope.
                self.expr(init);
                self.declare(name, *name_span, *mutable, true);
            }
            StmtKind::Assign { target, op, value } => {
                self.assign_target(target, op.is_some());
                self.expr(value);
            }
            StmtKind::Expr(expr) => self.expr(expr),
            StmtKind::If {
                cond,
                then_block,
                elifs,
                else_block,
            } => {
                self.expr(cond);
                self.block(then_block);
                for (condition, block) in elifs {
                    self.expr(condition);
                    self.block(block);
                }
                if let Some(block) = else_block {
                    self.block(block);
                }
            }
            StmtKind::While { cond, body } => {
                self.expr(cond);
                self.block(body);
            }
            StmtKind::For {
                name,
                name_span,
                iter,
                body,
            } => {
                self.expr(iter);
                self.push_scope();
                // Loop variables are intentionally not reported in this pass.
                self.declare(name, *name_span, false, false);
                self.block(body);
                self.pop_scope();
            }
            StmtKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.arm(arm);
                }
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    fn assign_target(&mut self, target: &Expr, compound: bool) {
        match &target.kind {
            ExprKind::Var(name) => {
                if compound {
                    self.read(name);
                }
                self.write(name);
            }
            ExprKind::Index { base, index } => {
                self.expr(base);
                self.expr(index);
            }
            ExprKind::Field { base, .. } => self.expr(base),
            // The checker rejects other assignment targets, but walk them
            // defensively so a future target form cannot hide a read.
            _ => self.expr(target),
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) => {}
            ExprKind::Str(parts) => {
                for part in parts {
                    if let StrPart::Expr(expr) = part {
                        self.expr(expr);
                    }
                }
            }
            ExprKind::Var(name) => self.read(name),
            ExprKind::List(items) => {
                for item in items {
                    self.expr(item);
                }
            }
            ExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.expr(key);
                    self.expr(value);
                }
            }
            ExprKind::Unary { expr, .. } => self.expr(expr),
            ExprKind::Try { expr } => self.expr(expr),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                for argument in args {
                    self.expr(&argument.value);
                }
            }
            ExprKind::Index { base, index } => {
                self.expr(base);
                self.expr(index);
            }
            ExprKind::Field { base, .. } => self.expr(base),
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.arm(arm);
                }
            }
        }
    }

    fn arm(&mut self, arm: &MatchArm) {
        self.push_scope();
        self.pattern(&arm.pattern);
        if let Some(guard) = &arm.guard {
            self.expr(guard);
        }
        match &arm.body {
            MatchArmBody::Expr(expr) => self.expr(expr),
            MatchArmBody::Block(block) => self.block(block),
        }
        self.pop_scope();
    }

    fn pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Wildcard { .. } | Pattern::Literal { .. } | Pattern::Range { .. } => {}
            Pattern::Variant { fields, .. } => {
                for field in fields {
                    self.pattern_arg(field);
                }
            }
            Pattern::Or { alternatives, .. } => {
                for alternative in alternatives {
                    self.pattern(alternative);
                }
            }
        }
    }

    fn pattern_arg(&mut self, argument: &PatternArg) {
        match argument {
            PatternArg::Bind { name, name_span } => {
                // Unused match bindings are intentionally not reported in this
                // first lint pass; they still participate in name resolution.
                self.declare(name, *name_span, false, false);
            }
            PatternArg::Wildcard { .. } => {}
            PatternArg::Nested { pattern, .. } => self.pattern(pattern),
        }
    }
}

fn direct_terminator(statement: &Stmt) -> Option<&'static str> {
    match &statement.kind {
        StmtKind::Return(_) => Some("return"),
        StmtKind::Break => Some("break"),
        StmtKind::Continue => Some("continue"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint_codes(source: &str) -> Vec<String> {
        let result = crate::compile_with_options(source, false);
        assert!(
            !result.has_errors(),
            "unexpected compile errors:\n{}",
            result.diagnostics.render("<test>", source)
        );
        let program = result.program.expect("program");
        lint(&program)
            .items
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    #[test]
    fn unused_local_is_reported() {
        assert_eq!(
            lint_codes(
                "fn main():\n    let unused = 1\n    print(\"ok\")\n"
            ),
            vec!["L0001"]
        );
    }

    #[test]
    fn used_local_is_clean() {
        assert!(lint_codes("fn main():\n    let used = 1\n    print(used)\n").is_empty());
    }

    #[test]
    fn plain_assignment_is_a_write_not_a_read() {
        assert_eq!(
            lint_codes(
                "fn main():\n    var value = 0\n    value = 1\n    print(\"done\")\n"
            ),
            vec!["L0001"]
        );
    }

    #[test]
    fn compound_assignment_reads_and_writes() {
        assert!(lint_codes(
            "fn main():\n    var count = 0\n    count += 1\n    print(count)\n"
        )
        .is_empty());
    }

    #[test]
    fn var_never_reassigned_is_reported() {
        assert_eq!(
            lint_codes(
                "fn main():\n    var label = \"x\"\n    print(label)\n"
            ),
            vec!["L0002"]
        );
    }

    #[test]
    fn shadowed_inner_binding_is_reported_but_outer_is_used() {
        assert_eq!(
            lint_codes(
                "fn main():\n    let value = 1\n    print(value)\n    if true:\n        let value = 2\n"
            ),
            vec!["L0001"]
        );
    }

    #[test]
    fn initializer_reads_the_outer_binding() {
        assert!(lint_codes(
            "fn main():\n    let value = 1\n    if true:\n        let value = value + 1\n        print(value)\n"
        )
        .is_empty());
    }

    #[test]
    fn unreachable_after_return_is_reported_once() {
        assert_eq!(
            lint_codes(
                "fn f() -> Int:\n    return 1\n    print(\"dead\")\n    print(\"dead too\")\n\nfn main():\n    print(f())\n"
            ),
            vec!["L0003"]
        );
    }

    #[test]
    fn unreachable_after_break_is_reported() {
        assert_eq!(
            lint_codes(
                "fn main():\n    var i = 0\n    while i < 3:\n        i += 1\n        break\n        print(\"dead\")\n"
            ),
            vec!["L0003"]
        );
    }

    #[test]
    fn if_with_a_return_in_one_branch_is_not_unreachable() {
        assert!(lint_codes(
            "fn f(n: Int) -> Int:\n    if n > 0:\n        return 1\n    return 2\n\nfn main():\n    print(f(1))\n"
        )
        .is_empty());
    }

    #[test]
    fn match_bindings_are_not_reported() {
        assert!(lint_codes(
            "enum Item:\n    Value(n: Int)\n    Empty\n\nfn main():\n    let item = Value(1)\n    let text = match item:\n        Value(n): \"value\"\n        Empty(): \"empty\"\n    print(text)\n"
        )
        .is_empty());
    }

    #[test]
    fn unused_global_is_reported() {
        assert_eq!(
            lint_codes("let unused = 1\n\nfn main():\n    print(\"ok\")\n"),
            vec!["L0001"]
        );
    }

    #[test]
    fn global_used_by_a_function_is_clean() {
        assert!(lint_codes(
            "let shared = 1\n\nfn read() -> Int:\n    return shared\n\nfn main():\n    print(read())\n"
        )
        .is_empty());
    }

    #[test]
    fn global_written_by_a_function_is_clean() {
        assert!(lint_codes(
            "var counter = 0\n\nfn bump():\n    counter += 1\n\nfn main():\n    bump()\n    print(counter)\n"
        )
        .is_empty());
    }

    #[test]
    fn underscore_binding_is_ignored() {
        assert!(lint_codes("fn main():\n    let _ = 1\n    print(\"ok\")\n").is_empty());
    }

    #[test]
    fn underscore_prefixed_binding_is_ignored() {
        assert!(lint_codes("fn main():\n    let _unused = 1\n    print(\"ok\")\n").is_empty());
    }

    #[test]
    fn parameters_are_not_reported() {
        assert!(lint_codes(
            "fn identity(value: Int) -> Int:\n    return value\n\nfn main():\n    print(identity(1))\n"
        )
        .is_empty());
    }

    #[test]
    fn loop_variables_are_not_reported() {
        assert!(lint_codes("fn main():\n    for i in range(3):\n        print(\"x\")\n").is_empty());
    }

    #[test]
    fn unreachable_in_match_arm_block_is_reported() {
        assert_eq!(
            lint_codes(
                "fn f(n: Int) -> Int:\n    match n:\n        0:\n            return 1\n            print(\"dead\")\n        _:\n            return 2\n\nfn main():\n    print(f(0))\n"
            ),
            vec!["L0003"]
        );
    }
}
