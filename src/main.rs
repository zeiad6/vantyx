//! Command-line interface for Vantyx.

mod ai_cli;
mod repl;

use repl::{Editor, ReadOutcome};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use vantyx::ast::Program;
use vantyx::checker::SemanticModel;
use vantyx::diagnostic::Severity;
use vantyx::span::SourceRegistry;
use vantyx::{compile, compile_with_options, render_runtime_error_all};

const USAGE: &str = "\
Vantyx 0.1.0 - a safe, gradually typed language

Usage:
  vantyx run <file.vtx>      Compile and execute a program
  vantyx check <file.vtx>    Compile without executing
  vantyx lint <file.vtx>     Compile and report non-fatal lints
  vantyx build <file.vtx>    Compile to a native executable with a C compiler
  vantyx build <file> -o <out> Choose the executable path
  vantyx repl                  Start an interactive session
  vantyx ai <command>          AI host tools (see `vantyx ai --help`)
  vantyx version               Print the version
  vantyx help                  Show this help

Use `-` as the file name to read from standard input.
Exit codes: 0 success, 1 compile error, 2 runtime error, 64 usage error.
";

fn main() {
    std::process::exit(run_cli());
}

fn run_cli() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            eprint!("{USAGE}");
            64
        }
        [command] if command == "help" || command == "--help" || command == "-h" => {
            print!("{USAGE}");
            0
        }
        [command] if command == "version" || command == "--version" || command == "-V" => {
            println!("vantyx {}", env!("CARGO_PKG_VERSION"));
            0
        }
        [command, file] if command == "run" => run_file(file),
        [command, file] if command == "check" => check_file(file),
        [command, file] if command == "lint" => lint_file(file),
        [command, file] if command == "build" => build_file(file, None),
        [command, file, flag, output] if command == "build" && flag == "-o" => {
            build_file(file, Some(output))
        }
        [command] if command == "repl" => start_repl(),
        [command, rest @ ..] if command == "ai" => ai_cli::run(rest),
        _ => {
            eprintln!("error: unknown command or wrong number of arguments");
            eprintln!();
            eprint!("{USAGE}");
            64
        }
    }
}

fn read_source(path: &str) -> Result<String, i32> {
    let mut source = String::new();
    if path == "-" {
        if let Err(error) = io::stdin().read_to_string(&mut source) {
            eprintln!("error: cannot read standard input: {error}");
            return Err(1);
        }
    } else if let Err(error) = fs::read_to_string(path) {
        eprintln!("error: cannot read `{path}`: {error}");
        return Err(1);
    }
    Ok(vantyx::strip_utf8_bom(&source).to_string())
}

/// A successfully checked entry file and every source it imported.
struct Compiled {
    program: Program,
    model: SemanticModel,
    sources: SourceRegistry,
    /// Canonical on-disk paths of the entry and every loaded module.
    source_paths: Vec<PathBuf>,
}

/// Loads an entry file (and its modules), or compiles stdin as one source.
fn compile_entry(path: &str) -> Result<Compiled, i32> {
    if path == "-" {
        let source = read_source(path)?;
        let result = compile(&source);
        if !result.diagnostics.is_empty() {
            eprint!("{}", result.render("<stdin>", &source));
        }
        if result.has_errors() {
            return Err(1);
        }
        let (Some(program), Some(model)) = (result.program, result.model) else {
            eprintln!("error: internal compiler error: missing program after successful check");
            return Err(1);
        };
        let mut sources = SourceRegistry::new();
        sources.add("<stdin>", source);
        return Ok(Compiled {
            program,
            model,
            sources,
            source_paths: Vec::new(),
        });
    }

    let loaded = match vantyx::module::load_file(Path::new(path)) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("error: {error}");
            return Err(1);
        }
    };
    if !loaded.diagnostics.is_empty() {
        eprint!("{}", loaded.render());
    }
    if loaded.has_errors() {
        return Err(1);
    }
    let (Some(program), Some(model)) = (loaded.program, loaded.model) else {
        eprintln!("error: internal compiler error: missing program after successful check");
        return Err(1);
    };
    let root = fs::canonicalize(path)
        .ok()
        .and_then(|entry| entry.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let mut source_paths = Vec::new();
    for file in loaded.sources.iter() {
        if file.name == "<stdin>" {
            continue;
        }
        if let Ok(source) = fs::canonicalize(root.join(&file.name)) {
            source_paths.push(source);
        }
    }
    Ok(Compiled {
        program,
        model,
        sources: loaded.sources,
        source_paths,
    })
}

/// A `Send` writer that locks stdout per operation. The interpreter runs on a
/// dedicated thread, so it cannot hold a `StdoutLock` created by the caller.
struct StdoutWriter;

impl Write for StdoutWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        io::stdout().lock().write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().lock().flush()
    }
}

fn run_file(path: &str) -> i32 {
    let compiled = match compile_entry(path) {
        Ok(compiled) => compiled,
        Err(code) => return code,
    };

    let mut out = StdoutWriter;
    match vantyx::interp::run(&compiled.program, &compiled.model, &mut out) {
        Ok(()) => {
            if let Err(error) = out.flush() {
                eprintln!("error: failed to flush output: {error}");
                return 1;
            }
            0
        }
        Err(error) => {
            eprint!("{}", render_runtime_error_all(&error, &compiled.sources));
            2
        }
    }
}

fn check_file(path: &str) -> i32 {
    match compile_entry(path) {
        Ok(_) => 0,
        Err(code) => code,
    }
}

fn lint_file(path: &str) -> i32 {
    let compiled = match compile_entry(path) {
        Ok(compiled) => compiled,
        Err(code) => return code,
    };
    let diagnostics = vantyx::lint::lint(&compiled.program);
    if !diagnostics.is_empty() {
        eprint!("{}", diagnostics.render_all(&compiled.sources));
    }
    0
}

fn build_file(path: &str, output: Option<&str>) -> i32 {
    let compiled = match compile_entry(path) {
        Ok(compiled) => compiled,
        Err(code) => return code,
    };

    let ir = match vantyx::ir::build(&compiled.program, &compiled.model) {
        Ok(ir) => ir,
        Err(error) => {
            let mut diagnostic =
                vantyx::diagnostic::Diagnostic::error(error.code, error.message, error.span);
            if let Some(help) = error.help {
                diagnostic = diagnostic.with_help(help);
            }
            let rendered = match compiled.sources.get(error.span.file) {
                Some(file) => diagnostic.render(&file.name, &file.source),
                None => format!("error[{}]: {}\n", diagnostic.code, diagnostic.message),
            };
            eprint!("{rendered}");
            return 1;
        }
    };

    let output_path = match output {
        Some(path) => PathBuf::from(path),
        None => {
            if path == "-" {
                eprintln!("error: `vantyx build -` requires `-o <output>`");
                return 64;
            }
            let mut path = PathBuf::from(path);
            if path.extension().is_some() {
                path.set_extension("");
            } else {
                path.set_extension("out");
            }
            path
        }
    };

    if let Some(source) = compiled
        .source_paths
        .iter()
        .find(|source| vantyx::fs_safety::paths_refer_to_same_file_paths(&output_path, source))
    {
        eprintln!(
            "error: output path `{}` is also a source file `{}`; refusing to overwrite it",
            output_path.display(),
            source.display()
        );
        eprintln!("help: choose a different `-o` path");
        return 64;
    }

    let c_source = vantyx::codegen_c::emit_with_sources(&ir, &compiled.sources);
    let temp = match write_generated_c(&c_source) {
        Ok(path) => path,
        Err(code) => return code,
    };

    let compiler = env::var("VANTYX_CC").unwrap_or_else(|_| "clang".to_string());
    let status = Command::new(&compiler)
        .arg("-std=gnu11")
        .arg("-O2")
        .arg("-o")
        .arg(&output_path)
        .arg(&temp)
        .arg("-lm")
        .status();

    match status {
        Ok(status) if status.success() => {
            let _ = fs::remove_file(&temp);
            0
        }
        Ok(status) => {
            eprintln!("error: C compilation failed ({status})");
            eprintln!("note: generated C source retained at {}", temp.display());
            1
        }
        Err(error) => {
            eprintln!("error: cannot run C compiler `{compiler}`: {error}");
            eprintln!("help: install clang, or set VANTYX_CC to a C compiler");
            let _ = fs::remove_file(&temp);
            1
        }
    }
}

/// Compiles and runs one REPL entry against the accumulated session source.
///
/// Earlier entries are replayed with their output muted, which reconstructs
/// the persistent environment. An entry that fails to compile or run is not
/// appended to the history, so the next entry starts from the last good state.
/// Creates a fresh, owner-only temporary C file.
///
/// `create_new` refuses to follow an existing symlink, so a local attacker
/// cannot redirect the compiler input to another file. The retry loop handles
/// the unlikely name collision without weakening that guarantee.
fn write_generated_c(source: &str) -> Result<PathBuf, i32> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    for attempt in 0..64 {
        let path =
            env::temp_dir().join(format!("vantyx-{}-{stamp}-{attempt}.c", std::process::id()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(source.as_bytes()) {
                    let _ = fs::remove_file(&path);
                    eprintln!(
                        "error: cannot write generated C to `{}`: {error}",
                        path.display()
                    );
                    return Err(1);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                eprintln!(
                    "error: cannot create generated C file `{}`: {error}",
                    path.display()
                );
                return Err(1);
            }
        }
    }
    eprintln!("error: cannot create a unique temporary C file");
    Err(1)
}

fn execute_snippet(state: &mut ReplState, entry: &str, out: &mut (dyn Write + Send)) {
    let mut source = state.source.clone();
    if !source.is_empty() && !source.ends_with('\n') {
        source.push('\n');
    }
    let offset = u32::try_from(source.len()).unwrap_or(u32::MAX);
    source.push_str(entry);
    if !source.ends_with('\n') {
        source.push('\n');
    }

    let result = compile_with_options(&source, false);
    for diagnostic in &result.diagnostics.items {
        let from_entry = diagnostic.span.start >= offset;
        if from_entry || diagnostic.severity == Severity::Error {
            eprint!("{}", diagnostic.render("<repl>", &source));
            eprintln!();
        }
    }
    if result.has_errors() {
        return;
    }
    let (Some(program), Some(model)) = (&result.program, &result.model) else {
        return;
    };

    let prefix = program
        .items
        .iter()
        .filter(|item| match item {
            vantyx::ast::Item::Stmt(stmt) => stmt.span.start < offset,
            _ => false,
        })
        .count();
    let auto_main = state.source.is_empty();
    if let Err(error) = vantyx::interp::run_repl(program, model, prefix, auto_main, out) {
        let mut sources = SourceRegistry::new();
        sources.add("<repl>", source.clone());
        eprint!("{}", render_runtime_error_all(&error, &sources));
        return;
    }

    state.source = source;
    collect_names(state, program, offset);
    let _ = out.flush();
}

#[derive(Default)]
struct ReplState {
    source: String,
    names: Vec<String>,
}

fn collect_names(state: &mut ReplState, program: &vantyx::ast::Program, offset: u32) {
    use vantyx::ast::{Item, StmtKind};
    for item in &program.items {
        match item {
            Item::Fn(decl) if decl.span.start >= offset => state.names.push(decl.name.clone()),
            Item::Struct(decl) if decl.span.start >= offset => {
                state.names.push(decl.name.clone());
            }
            Item::Enum(decl) if decl.span.start >= offset => {
                state.names.push(decl.name.clone());
                for variant in &decl.variants {
                    state.names.push(variant.name.clone());
                }
            }
            Item::Stmt(stmt) if stmt.span.start >= offset => match &stmt.kind {
                StmtKind::Let { name, .. } => state.names.push(name.clone()),
                StmtKind::LetPattern { pattern, .. } => {
                    collect_let_pattern_names(pattern, &mut state.names);
                }
                _ => {}
            },
            _ => {}
        }
    }
    state.names.sort();
    state.names.dedup();
}

fn collect_let_pattern_names(pattern: &vantyx::ast::LetPattern, names: &mut Vec<String>) {
    match pattern {
        vantyx::ast::LetPattern::Name { name, .. } => names.push(name.clone()),
        vantyx::ast::LetPattern::Wildcard { .. } => {}
        vantyx::ast::LetPattern::Tuple { elements, .. } => {
            for element in elements {
                collect_let_pattern_names(element, names);
            }
        }
    }
}

fn handle_meta(command: &str, state: &ReplState, editor: &Editor, out: &mut dyn Write) {
    match command.trim() {
        "help" | "h" | "?" => {
            let _ = writeln!(out, "Commands:");
            let _ = writeln!(out, "  :help       Show this help");
            let _ = writeln!(out, "  :history    List saved and session history");
            let _ = writeln!(out, "  :names      List visible bindings and declarations");
            let _ = writeln!(out, "  :quit       Exit (also `exit` or Ctrl-D)");
            let _ = writeln!(out, "Tab completes keywords, builtins, and visible names.");
            let _ = writeln!(
                out,
                "Up/Down recall history; Ctrl-A/E, Ctrl-U/K/W, and Ctrl-L edit."
            );
        }
        "history" | "hist" => {
            if editor.history().is_empty() {
                let _ = writeln!(out, "(history is empty)");
            } else {
                for (index, entry) in editor.history().iter().enumerate() {
                    let _ = writeln!(out, "{:4}  {entry}", index + 1);
                }
            }
        }
        "names" | "vars" => {
            if state.names.is_empty() {
                let _ = writeln!(out, "(no visible names)");
            } else {
                let _ = writeln!(out, "{}", state.names.join("  "));
            }
        }
        _ => {
            let _ = writeln!(out, "unknown command `:{command}`; try `:help`");
        }
    }
    let _ = out.flush();
}

fn start_repl() -> i32 {
    let mut out = StdoutWriter;
    let mut editor = Editor::new(repl::default_history_path());
    let mut state = ReplState::default();

    let _ = writeln!(out, "Vantyx {} REPL", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        out,
        "Tab completes on a terminal; up/down recall history; `:help` lists commands."
    );
    let _ = writeln!(
        out,
        "Bindings and declarations persist across entries. End a block with a blank line."
    );
    let _ = writeln!(out, "Type `exit` or press Ctrl-D to quit.");

    let mut buffer = String::new();
    loop {
        let prompt = if buffer.is_empty() { ">>> " } else { "... " };
        let outcome = match editor.read_line(prompt, &mut out) {
            Ok(outcome) => outcome,
            Err(error) => {
                eprintln!("error: cannot read input: {error}");
                return 1;
            }
        };
        let line = match outcome {
            ReadOutcome::Eof => break,
            ReadOutcome::Interrupted => {
                buffer.clear();
                continue;
            }
            ReadOutcome::Line(line) => line,
        };
        let trimmed = line.trim_end_matches(['\r', '\n']);

        if buffer.is_empty() {
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "exit" || trimmed == "quit" {
                break;
            }
            if let Some(command) = trimmed.strip_prefix(':') {
                if matches!(command.trim(), "quit" | "q" | "exit") {
                    break;
                }
                handle_meta(command, &state, &editor, &mut out);
                continue;
            }
            if trimmed.trim_end().ends_with(':') {
                buffer.push_str(trimmed);
                buffer.push('\n');
            } else {
                editor.add_history(trimmed);
                execute_snippet(&mut state, trimmed, &mut out);
                editor.set_names(state.names.clone());
            }
        } else if trimmed.is_empty() {
            let entry = std::mem::take(&mut buffer);
            editor.add_history(&entry);
            execute_snippet(&mut state, &entry, &mut out);
            editor.set_names(state.names.clone());
        } else {
            buffer.push_str(trimmed);
            buffer.push('\n');
        }
    }

    if !buffer.is_empty() {
        let entry = std::mem::take(&mut buffer);
        editor.add_history(&entry);
        execute_snippet(&mut state, &entry, &mut out);
    }
    0
}
