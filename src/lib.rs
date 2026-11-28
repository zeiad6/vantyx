//! Vantyx: a safe, gradually typed language.
//!
//! The public pipeline is intentionally small:
//!
//! ```text
//! source -> lexer -> parser -> checker -> interpreter
//! ```
//!
//! `compile` stops at the first stage that produced errors and returns every
//! diagnostic collected by that stage. Warnings do not block execution.

pub mod ai;
pub mod ast;
pub mod checker;
pub mod codegen_c;
pub mod copy_tree;
pub mod diagnostic;
pub mod fs_safety;
pub mod glob;
pub mod interp;
pub mod ir;
pub mod lexer;
pub mod lint;
pub mod module;
pub mod ownership;
pub mod parser;
pub mod span;
pub mod token;
pub mod types;
pub mod value;

use ast::Program;
use checker::SemanticModel;
use diagnostic::{Diagnostic, Diagnostics};
use span::{SourceMap, SourceRegistry};

#[derive(Debug)]
pub struct CompileResult {
    /// Present when parsing completed, even if the checker rejected the tree.
    pub program: Option<Program>,
    /// Present only when there are no errors.
    pub model: Option<SemanticModel>,
    pub diagnostics: Diagnostics,
}

impl CompileResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }

    pub fn render(&self, source_name: &str, source: &str) -> String {
        self.diagnostics.render(source_name, source)
    }
}

/// Removes one leading UTF-8 byte-order mark, if present.
///
/// A BOM is accepted only at the start of a source file. Interior U+FEFF
/// characters remain ordinary input and are rejected by the lexer outside a
/// string literal, which keeps source spans aligned with the remaining text.
pub fn strip_utf8_bom(source: &str) -> &str {
    source.strip_prefix('\u{feff}').unwrap_or(source)
}

/// Compiles a complete program, requiring an entry point.
pub fn compile(source: &str) -> CompileResult {
    compile_with_options(source, true)
}

/// Compiles a source snippet. `require_entry` is false for REPL snippets that
/// contain only declarations.
pub fn compile_with_options(source: &str, require_entry: bool) -> CompileResult {
    let lexed = lexer::lex(source);
    let mut diagnostics = lexed.diagnostics;
    if diagnostics.has_errors() {
        return CompileResult {
            program: None,
            model: None,
            diagnostics,
        };
    }

    let (program, parse_diagnostics) = parser::parse(source, lexed.tokens);
    diagnostics.extend(parse_diagnostics);
    if diagnostics.has_errors() {
        return CompileResult {
            program: Some(program),
            model: None,
            diagnostics,
        };
    }

    let (model, check_diagnostics) = checker::check_with_options(&program, require_entry);
    diagnostics.extend(check_diagnostics);
    CompileResult {
        program: Some(program),
        model,
        diagnostics,
    }
}

/// Renders a runtime error with its primary span and call stack.
pub fn render_runtime_error(
    error: &interp::RuntimeError,
    source_name: &str,
    source: &str,
) -> String {
    let mut diagnostic = Diagnostic::error(error.code.clone(), error.message.clone(), error.span);
    if let Some(frame) = error.stack.last() {
        diagnostic = diagnostic.with_note(format!("in function `{}`", frame.function));
    }
    let mut out = diagnostic.render(source_name, source);
    if !error.stack.is_empty() {
        let map = SourceMap::new(source_name, source);
        out.push_str("stack trace:\n");
        for frame in error.stack.iter().rev() {
            let (line, column) = map.line_col(frame.span.start);
            out.push_str(&format!(
                "  at {} ({}:{}:{})\n",
                frame.function, source_name, line, column
            ));
        }
    }
    out
}

/// Renders a runtime error using the source file recorded in each span.
pub fn render_runtime_error_all(error: &interp::RuntimeError, sources: &SourceRegistry) -> String {
    let mut diagnostic = Diagnostic::error(error.code.clone(), error.message.clone(), error.span);
    if let Some(frame) = error.stack.last() {
        diagnostic = diagnostic.with_note(format!("in function `{}`", frame.function));
    }
    let mut out = match sources.get(error.span.file) {
        Some(file) => diagnostic.render(&file.name, &file.source),
        None => format!("error[{}]: {}\n", error.code, error.message),
    };
    if !error.stack.is_empty() {
        out.push_str("stack trace:\n");
        for frame in error.stack.iter().rev() {
            let file = sources.get(frame.span.file);
            let (name, line, column) = match file {
                Some(file) => {
                    let map = SourceMap::new(&file.name, &file.source);
                    let (line, column) = map.line_col(frame.span.start);
                    (file.name.as_str(), line, column)
                }
                None => ("<unknown>", 0, 0),
            };
            out.push_str(&format!(
                "  at {} ({}:{}:{})\n",
                frame.function, name, line, column
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_a_minimal_program() {
        let result = compile("print(\"ok\")\n");
        assert!(!result.has_errors(), "{:?}", result.diagnostics.items);
        assert!(result.program.is_some());
        assert!(result.model.is_some());
    }

    #[test]
    fn stops_after_lexical_errors() {
        let result = compile("let x = \"unterminated\n");
        assert!(result.has_errors());
        assert!(result.program.is_none());
        assert!(result.model.is_none());
    }

    #[test]
    fn keeps_the_tree_after_parse_errors() {
        let result = compile("fn main()\n    print(1)\n");
        assert!(result.has_errors());
        assert!(result.program.is_some());
        assert!(result.model.is_none());
    }

    #[test]
    fn warnings_do_not_block_the_model() {
        let result = compile("fn main():\n    print(1)\n\nprint(2)\n");
        assert!(!result.has_errors());
        assert!(result.diagnostics.items.iter().any(|d| d.code == "W0001"));
        assert!(result.model.is_some());
    }

    #[test]
    fn repl_mode_allows_declaration_only_snippets() {
        let result = compile_with_options("struct Point:\n    x: Int\n", false);
        assert!(!result.has_errors(), "{:?}", result.diagnostics.items);
        assert!(result.model.is_some());
    }

    #[test]
    fn repl_mode_still_rejects_main_with_parameters() {
        let result = compile_with_options("fn main(x: Int):\n    print(x)\n", false);
        assert!(result.has_errors());
    }
}
