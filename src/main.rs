//! Command-line interface for Auren.

mod repl;

use auren::diagnostic::Severity;
use auren::{compile, compile_with_options, render_runtime_error};
use repl::{Editor, ReadOutcome};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::Command;

const USAGE: &str = "\
Auren 0.1.0 - a safe, gradually typed language

Usage:
  auren run <file.auren>      Compile and execute a program
  auren check <file.auren>    Compile without executing
  auren build <file.auren>    Compile to a native executable with a C compiler
  auren build <file> -o <out> Choose the executable path
  auren repl                  Start an interactive session
  auren version               Print the version
  auren help                  Show this help

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
            println!("auren {}", env!("CARGO_PKG_VERSION"));
            0
        }
        [command, file] if command == "run" => run_file(file),
        [command, file] if command == "check" => check_file(file),
        [command, file] if command == "build" => build_file(file, None),
        [command, file, flag, output] if command == "build" && flag == "-o" => {
            build_file(file, Some(output))
        }
        [command] if command == "repl" => start_repl(),
        _ => {
            eprintln!("error: unknown command or wrong number of arguments");
            eprintln!();
            eprint!("{USAGE}");
            64
        }
    }
}

fn read_source(path: &str) -> Result<String, i32> {
    if path == "-" {
        let mut source = String::new();
        match io::stdin().read_to_string(&mut source) {
            Ok(_) => Ok(source),
            Err(error) => {
                eprintln!("error: cannot read standard input: {error}");
                Err(1)
            }
        }
    } else {
        match fs::read_to_string(path) {
            Ok(source) => Ok(source),
            Err(error) => {
                eprintln!("error: cannot read `{path}`: {error}");
                Err(1)
            }
        }
    }
}

fn display_name(path: &str) -> &str {
    if path == "-" {
        "<stdin>"
    } else {
        path
    }
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
    let source = match read_source(path) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let name = display_name(path);
    let result = compile(&source);
    if !result.diagnostics.is_empty() {
        eprint!("{}", result.render(name, &source));
    }
    if result.has_errors() {
        return 1;
    }
    let (Some(program), Some(model)) = (&result.program, &result.model) else {
        eprintln!("error: internal compiler error: missing program after successful check");
        return 1;
    };

    let mut out = StdoutWriter;
    match auren::interp::run(program, model, &mut out) {
        Ok(()) => {
            if let Err(error) = out.flush() {
                eprintln!("error: failed to flush output: {error}");
                return 1;
            }
            0
        }
        Err(error) => {
            eprint!("{}", render_runtime_error(&error, name, &source));
            2
        }
    }
}

fn check_file(path: &str) -> i32 {
    let source = match read_source(path) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let name = display_name(path);
    let result = compile(&source);
    if !result.diagnostics.is_empty() {
        eprint!("{}", result.render(name, &source));
    }
    if result.has_errors() {
        1
    } else {
        0
    }
}

fn build_file(path: &str, output: Option<&str>) -> i32 {
    let source = match read_source(path) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let name = display_name(path);
    let result = compile(&source);
    if !result.diagnostics.is_empty() {
        eprint!("{}", result.render(name, &source));
    }
    if result.has_errors() {
        return 1;
    }
    let (Some(program), Some(model)) = (&result.program, &result.model) else {
        eprintln!("error: internal compiler error: missing program after successful check");
        return 1;
    };

    let ir = match auren::ir::build(program, model) {
        Ok(ir) => ir,
        Err(error) => {
            let mut diagnostic =
                auren::diagnostic::Diagnostic::error(error.code, error.message, error.span);
            if let Some(help) = error.help {
                diagnostic = diagnostic.with_help(help);
            }
            eprint!("{}", diagnostic.render(name, &source));
            return 1;
        }
    };

    let output_path = match output {
        Some(path) => PathBuf::from(path),
        None => {
            if path == "-" {
                eprintln!("error: `auren build -` requires `-o <output>`");
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

    let c_source = auren::codegen_c::emit(&ir, name, &source);
    let temp = match write_generated_c(&c_source) {
        Ok(path) => path,
        Err(code) => return code,
    };

    let compiler = env::var("AUREN_CC").unwrap_or_else(|_| "clang".to_string());
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
            eprintln!("help: install clang, or set AUREN_CC to a C compiler");
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
        let path = env::temp_dir().join(format!(
            "auren-{}-{stamp}-{attempt}.c",
            std::process::id()
        ));
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
            auren::ast::Item::Stmt(stmt) => stmt.span.start < offset,
            _ => false,
        })
        .count();
    let auto_main = state.source.is_empty();
    if let Err(error) =
        auren::interp::run_repl(program, model, prefix, auto_main, out)
    {
        eprint!("{}", render_runtime_error(&error, "<repl>", &source));
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

fn collect_names(state: &mut ReplState, program: &auren::ast::Program, offset: u32) {
    use auren::ast::{Item, StmtKind};
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
            Item::Stmt(stmt) if stmt.span.start >= offset => {
                if let StmtKind::Let { name, .. } = &stmt.kind {
                    state.names.push(name.clone());
                }
            }
            _ => {}
        }
    }
    state.names.sort();
    state.names.dedup();
}

fn handle_meta(command: &str, state: &ReplState, editor: &Editor, out: &mut dyn Write) {
    match command.trim() {
        "help" | "h" | "?" => {
            let _ = writeln!(out, "Commands:");
            let _ = writeln!(out, "  :help       Show this help");
            let _ = writeln!(out, "  :history    List saved and session history");
            let _ = writeln!(out, "  :names      List visible bindings and declarations");
            let _ = writeln!(out, "  :quit       Exit (also `exit` or Ctrl-D)");
            let _ = writeln!(
                out,
                "Tab completes keywords, builtins, and visible names."
            );
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

    let _ = writeln!(out, "Auren {} REPL", env!("CARGO_PKG_VERSION"));
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
