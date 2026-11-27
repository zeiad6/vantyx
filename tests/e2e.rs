//! End-to-end tests that exercise the real `vantyx` binary.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn temp_source(name: &str, source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("vantyx-e2e-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join(format!("{name}.vtx"));
    fs::write(&path, source).expect("write source");
    path
}

fn vantyx(args: &[&str], stdin: Option<&str>) -> Run {
    vantyx_with_env(args, stdin, &[])
}

fn vantyx_in_dir(directory: &Path, args: &[&str], stdin: Option<&str>) -> Run {
    vantyx_in_dir_with_env(Some(directory), args, stdin, &[])
}

fn vantyx_with_env(args: &[&str], stdin: Option<&str>, envs: &[(&str, &str)]) -> Run {
    vantyx_in_dir_with_env(None, args, stdin, envs)
}

fn vantyx_in_dir_with_env(
    directory: Option<&Path>,
    args: &[&str],
    stdin: Option<&str>,
    envs: &[(&str, &str)],
) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vantyx"));
    command
        .args(args)
        .env("VANTYX_HISTORY", "off")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn vantyx");
    if let Some(input) = stdin {
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write stdin");
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for vantyx");
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn run_file(name: &str, source: &str) -> Run {
    let path = temp_source(name, source);
    vantyx(&["run", path.to_str().expect("utf-8 path")], None)
}

fn check_file(name: &str, source: &str) -> Run {
    let path = temp_source(name, source);
    vantyx(&["check", path.to_str().expect("utf-8 path")], None)
}

fn lint_file(name: &str, source: &str) -> Run {
    let path = temp_source(name, source);
    vantyx(&["lint", path.to_str().expect("utf-8 path")], None)
}

#[test]
fn hello_world_runs() {
    let run = run_file("hello", "print(\"hello\")\n");
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "hello\n");
    assert!(run.stderr.is_empty());
}

#[test]
fn main_is_called_when_there_is_no_top_level_code() {
    let run = run_file("main", "fn main():\n    print(\"from main\")\n");
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "from main\n");
}

#[test]
fn top_level_code_runs_and_warns_about_unused_main() {
    let run = run_file(
        "warning",
        "fn main():\n    print(\"main\")\n\nprint(\"top\")\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "top\n");
    assert!(run.stderr.contains("warning[W0001]"), "{}", run.stderr);
}

#[test]
fn functions_resolve_globals_not_caller_locals() {
    let run = run_file(
        "lexical-globals",
        "var total = 10\n\
         var other = 20\n\n\
         fn read_total() -> Int:\n\
         \x20   return total\n\n\
         fn bump_total() -> Int:\n\
         \x20   total = total + 1\n\
         \x20   return total\n\n\
         fn bump_other() -> Int:\n\
         \x20   other = other + 1\n\
         \x20   return other\n\n\
         fn main():\n\
         \x20   var total = 100\n\
         \x20   let other = \"caller-immutable\"\n\
         \x20   print(read_total())\n\
         \x20   print(bump_total())\n\
         \x20   print(total)\n\
         \x20   print(bump_other())\n\
         \x20   print(other)\n\n\
         main()\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "10\n11\n100\n21\ncaller-immutable\n");
    assert!(run.stderr.contains("warning[W0001]"), "{}", run.stderr);
}

#[test]
fn global_assignment_before_declaration_raises_r0009() {
    // The interpreter evaluates the right side first, then resolves the
    // assignment place. A global whose declaration has not run yet must
    // therefore still execute the value's side effects before R0009.
    let run = run_file(
        "global-write-before-declaration",
        "fn setg() -> Int:\n\
         \x20   g = note()\n\
         \x20   return 0\n\n\
         fn note() -> Int:\n\
         \x20   print(\"rhs\")\n\
         \x20   return 5\n\n\
         print(setg())\n\
         var g = 1\n\
         print(g)\n",
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "rhs\n");
    assert!(run.stderr.contains("error[R0009]"), "{}", run.stderr);
    assert!(
        run.stderr
            .contains("global-write-before-declaration.vtx:2:5"),
        "{}",
        run.stderr
    );

    let after = run_file(
        "global-write-after-declaration",
        "fn setg() -> Int:\n\
         \x20   g = 5\n\
         \x20   return 0\n\n\
         var g = 1\n\
         print(setg())\n\
         print(g)\n",
    );
    assert_eq!(after.code, 0, "stderr: {}", after.stderr);
    assert_eq!(after.stdout, "0\n5\n");
}

#[test]
fn syntax_error_exits_with_one() {
    let run = run_file("syntax", "fn main()\n    print(1)\n");
    assert_eq!(run.code, 1);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.contains("error[E0102]"), "{}", run.stderr);
    assert!(run.stderr.contains("syntax.vtx:1:10"), "{}", run.stderr);
}

#[test]
fn bracketed_match_expression_reports_actionable_help() {
    let run = run_file(
        "match-bracketed",
        "enum E:\n    A\n\nfn main():\n    let e: E = A()\n    print(match e:\n        A: 1)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0101]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("bind it to a variable first"),
        "{}",
        run.stderr
    );
}

#[test]
fn type_error_exits_with_one() {
    let run = run_file("type", "let x: Int = \"no\"\nprint(x)\n");
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("expected `Int`, found `Str`"),
        "{}",
        run.stderr
    );
}

#[test]
fn runtime_error_exits_with_two_and_prints_a_stack() {
    let run = run_file(
        "runtime",
        "fn divide(a: Int, b: Int) -> Int:\n    return a // b\n\n\
         fn main():\n    print(divide(1, 0))\n",
    );
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("error[R0001]"), "{}", run.stderr);
    assert!(run.stderr.contains("division by zero"), "{}", run.stderr);
    assert!(run.stderr.contains("stack trace:"), "{}", run.stderr);
    assert!(run.stderr.contains("at divide"), "{}", run.stderr);
}

#[test]
fn numeric_string_conversions_follow_the_strict_grammar() {
    let run = run_file(
        "numeric_grammar",
        "fn main():\n\
         \x20   print(int(\" 12 \"), int(\"\u{a0}12\u{a0}\"), int(\"+7\"), int(\"-0\"))\n\
         \x20   print(float(\"1.\"), float(\".5\"), float(\"1.e3\"), float(\".5e2\"))\n\
         \x20   print(float(\"1e999\"), float(\"-1e999\"), float(\"1e-999\"))\n\
         \x20   print(float(\"inf\"), float(\"-Infinity\"), float(\"NaN\"), float(\"+nan\"))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "12 12 7 0\n1.0 0.5 1000.0 50.0\nInfinity -Infinity 0.0\nInfinity -Infinity NaN NaN\n"
    );
    for (name, expression) in [
        ("hex_float", "float(\"0x1p3\")"),
        ("nan_payload", "float(\"nan(1)\")"),
        ("int_suffix", "int(\"12x\")"),
        ("int_underscore", "int(\"1_0\")"),
        ("int_overflow", "int(\"9223372036854775808\")"),
    ] {
        let run = run_file(name, &format!("fn main():\n    print({expression})\n"));
        assert_eq!(run.code, 2, "`{expression}` should fail: {}", run.stdout);
        assert!(
            run.stderr.contains("error[R0007]"),
            "missing R0007 for `{expression}`: {}",
            run.stderr
        );
    }
}

#[test]
fn float_min_max_ties_and_nan_are_stable() {
    let run = run_file(
        "min_max_ties",
        "fn main():\n\
         \x20   print(min(-0.0, 0.0), max(-0.0, 0.0), min(0.0, -0.0), max(0.0, -0.0))\n\
         \x20   print(min(-1.0, -0.0), max(-1.0, -0.0), min(-0.0, -1.0), max(-0.0, -1.0))\n\
         \x20   let n = float(\"NaN\")\n\
         \x20   print(min(n, 1.0), max(n, 1.0), min(1.0, n), max(1.0, n))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "0.0 0.0 -0.0 -0.0\n-1.0 -0.0 -1.0 -0.0\n1.0 1.0 1.0 1.0\n"
    );
}

#[test]
fn print_evaluates_every_argument_before_writing() {
    // All arguments are evaluated, then all are rendered, then one line is
    // written. This makes later-argument mutations visible to the whole line
    // and leaves no partial output when an argument fails.
    let run = run_file(
        "print_two_phase",
        "fn bump(xs: List[Int]) -> Int:\n\
         \x20   push(xs, 9)\n\
         \x20   return 1\n\n\
         fn main():\n\
         \x20   let xs = [1]\n\
         \x20   print(xs, bump(xs))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "[1, 9] 1\n");

    let failed = run_file(
        "print_no_partial",
        "fn main():\n    print(\"partial\", floor(5e300))\n",
    );
    assert_eq!(failed.code, 2);
    assert!(failed.stdout.is_empty(), "stdout: {:?}", failed.stdout);
}

#[test]
fn named_constructor_arguments_evaluate_in_source_order() {
    // Named constructor arguments are placed into declaration order, but
    // evaluation follows the order they were written.
    let run = run_file(
        "named_constructor_order",
        "struct Pair:\n\
         \x20   left: Int\n\
         \x20   right: Int\n\n\
         struct Triple:\n\
         \x20   a: Int\n\
         \x20   b: Int\n\
         \x20   c: Int\n\n\
         fn note(label: Str, value: Int) -> Int:\n\
         \x20   print(label)\n\
         \x20   return value\n\n\
         fn main():\n\
         \x20   let p = Pair(right=note(\"right\", 2), left=note(\"left\", 1))\n\
         \x20   print(p)\n\
         \x20   let q = Triple(c=note(\"c\", 3), a=note(\"a\", 1), b=note(\"b\", 2))\n\
         \x20   print(q)\n\
         \x20   let r = Pair(note(\"first\", 10), note(\"second\", 20))\n\
         \x20   print(r)\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "right\nleft\nPair(left=1, right=2)\nc\na\nb\nTriple(a=1, b=2, c=3)\nfirst\nsecond\nPair(left=10, right=20)\n"
    );

    let failed = run_file(
        "named_constructor_error_order",
        "struct Pair:\n\
         \x20   left: Int\n\
         \x20   right: Int\n\n\
         fn note(label: Str, value: Int) -> Int:\n\
         \x20   print(label)\n\
         \x20   return value\n\n\
         fn fail() -> Int:\n\
         \x20   assert(false)\n\
         \x20   return 0\n\n\
         fn main():\n\
         \x20   let p = Pair(right=fail(), left=note(\"left\", 1))\n\
         \x20   print(p)\n",
    );
    assert_eq!(failed.code, 2);
    assert!(failed.stdout.is_empty(), "stdout: {:?}", failed.stdout);
}

#[test]
fn type_evaluates_its_argument() {
    // `type` returns a static name, but its argument is an ordinary call
    // argument: the interpreter evaluates it, including side effects and
    // runtime errors.
    let run = run_file(
        "type_side_effects",
        "fn bump(xs: List[Int]) -> Int:\n\
         \x20   push(xs, 2)\n\
         \x20   return len(xs)\n\n\
         fn main():\n\
         \x20   let xs = [1]\n\
         \x20   print(type(bump(xs)), xs)\n\
         \x20   print(type(print(\"inner\")))\n\
         \x20   print(type(assert(true)))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "Int [1, 2]\ninner\nUnit\nUnit\n");

    let failed = run_file(
        "type_argument_failure",
        "fn main():\n    print(type(assert(false)))\n",
    );
    assert_eq!(failed.code, 2);
    assert!(failed.stdout.is_empty(), "stdout: {:?}", failed.stdout);
}

#[test]
fn bool_evaluates_its_argument() {
    // `bool` is an ordinary call even though only Bool is accepted: the
    // interpreter evaluates the argument, then raises R0007 for other types.
    let run = run_file(
        "bool_side_effects",
        "fn flag() -> Bool:\n\
         \x20   print(\"flag\")\n\
         \x20   return true\n\n\
         fn side() -> Int:\n\
         \x20   print(\"side\")\n\
         \x20   return 1\n\n\
         fn main():\n\
         \x20   print(bool(flag()))\n\
         \x20   print(bool(side()))\n",
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "flag\ntrue\nside\n");
    assert!(run.stderr.contains("R0007"), "stderr: {}", run.stderr);
}

#[test]
fn unit_equality_evaluates_both_operands() {
    // `Unit` values compare statically, but equality is still an ordinary
    // binary expression: both operands are evaluated left to right.
    let run = run_file(
        "unit_equality_side_effects",
        "fn side(tag: Str) -> Unit:\n\
         \x20   print(tag)\n\n\
         fn main():\n\
         \x20   let u: Unit = side(\"decl\")\n\
         \x20   print(u)\n\
         \x20   print(u == side(\"cmp\"))\n\
         \x20   print(u != side(\"cmp2\"))\n\
         \x20   print(side(\"l\") == side(\"r\"))\n\
         \x20   print(side(\"l2\") != side(\"r2\"))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "decl\n()\ncmp\ntrue\ncmp2\nfalse\nl\nr\ntrue\nl2\nr2\nfalse\n"
    );
}

#[test]
fn unit_interpolation_evaluates_expressions() {
    let run = run_file(
        "unit_interpolation_side_effects",
        "fn single() -> Unit:\n\
         \x20   print(\"s1\")\n\n\
         fn multi1() -> Unit:\n\
         \x20   print(\"m1\")\n\n\
         fn multi2() -> Unit:\n\
         \x20   print(\"m2\")\n\n\
         fn main():\n\
         \x20   let a = \"{single()}\"\n\
         \x20   print(a)\n\
         \x20   let b = \"x{multi1()}y{multi2()}z\"\n\
         \x20   print(b)\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "s1\n()\nm1\nm2\nx()y()z\n");
}

#[test]
fn check_success_is_silent() {
    let run = check_file("check_ok", "print(\"never executed\")\n");
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.is_empty());
}

#[test]
fn check_failure_reports_errors() {
    let run = check_file("check_bad", "let x: Int = \"no\"\n");
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
}

#[test]
fn lint_reports_unused_and_unreachable_warnings() {
    let run = lint_file(
        "lint_findings",
        "fn main():\n    let unused = 1\n    var label = \"x\"\n    print(label)\n    return\n    print(\"dead\")\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.contains("warning[L0001]"), "{}", run.stderr);
    assert!(run.stderr.contains("warning[L0002]"), "{}", run.stderr);
    assert!(run.stderr.contains("warning[L0003]"), "{}", run.stderr);
    assert!(!run.stderr.contains("error["), "{}", run.stderr);
}

#[test]
fn lint_is_silent_on_a_clean_program() {
    let run = lint_file(
        "lint_clean",
        "fn main():\n    let used = 1\n    print(used)\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.is_empty(), "{}", run.stderr);
}

#[test]
fn lint_reports_unused_parameters() {
    let run = lint_file(
        "lint_unused_parameter",
        "fn constant(value: Int) -> Int:\n    return 1\n\nfn main():\n    print(constant(1))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.contains("warning[L0004]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("unused parameter `value`"),
        "{}",
        run.stderr
    );
    assert!(!run.stderr.contains("error["), "{}", run.stderr);
}

#[test]
fn lint_reports_compile_errors_and_exits_one() {
    let run = lint_file("lint_bad", "fn main():\n    print(missing)\n");
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0201]"), "{}", run.stderr);
}

#[test]
fn lint_reads_source_from_standard_input() {
    let run = vantyx(
        &["lint", "-"],
        Some("fn main():\n    let unused = 1\n    print(\"ok\")\n"),
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("warning[L0001]"), "{}", run.stderr);
}

#[test]
fn check_accepts_dead_code_after_return() {
    let run = check_file(
        "dead_after_return",
        "fn f() -> Int:\n    return 1\n    print(\"dead\")\n\nfn main():\n    print(f())\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
}

#[test]
fn run_does_not_execute_dead_code_after_return() {
    let run = run_file(
        "dead_after_return_run",
        "fn f() -> Int:\n    return 1\n    print(\"dead\")\n\nfn main():\n    print(f())\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "1\n");
}

#[test]
fn reads_source_from_standard_input() {
    let run = vantyx(&["run", "-"], Some("print(\"stdin\")\n"));
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "stdin\n");
}

#[test]
fn version_and_usage() {
    let version = vantyx(&["version"], None);
    assert_eq!(version.code, 0);
    assert!(version.stdout.contains("vantyx 0.1.0"));

    let usage = vantyx(&[], None);
    assert_eq!(usage.code, 64);
    assert!(usage.stderr.contains("Usage:"));

    let unknown = vantyx(&["frobnicate"], None);
    assert_eq!(unknown.code, 64);
    assert!(unknown.stderr.contains("unknown command"));
}

#[test]
fn missing_file_is_reported() {
    let run = vantyx(&["run", "/tmp/vantyx-does-not-exist-12345.vtx"], None);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("cannot read"), "{}", run.stderr);
}

#[test]
fn no_entry_point_is_an_error() {
    let run = run_file("noentry", "fn helper() -> Int:\n    return 1\n");
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0308]"), "{}", run.stderr);
}

#[test]
fn repl_runs_a_simple_entry() {
    let run = vantyx(&["repl"], Some("print(\"repl\")\nexit\n"));
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.contains("repl"), "{}", run.stdout);
    assert!(run.stdout.contains(">>>"), "{}", run.stdout);
}

#[test]
fn repl_runs_a_block_terminated_by_a_blank_line() {
    let run = vantyx(
        &["repl"],
        Some("fn main():\n    print(\"block\")\n\nexit\n"),
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.contains("block"), "{}", run.stdout);
}

#[test]
fn repl_persists_bindings_across_entries() {
    let run = vantyx(
        &["repl"],
        Some(
            "let x = 41\n\
             print(\"first\")\n\
             print(x + 1)\n\
             var y = 2\n\
             y = y + 3\n\
             print(y)\n\
             exit\n",
        ),
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.contains("42"), "{}", run.stdout);
    assert!(run.stdout.contains("5"), "{}", run.stdout);
    assert_eq!(
        run.stdout.matches("first").count(),
        1,
        "history output must not replay: {}",
        run.stdout
    );
}

#[test]
fn repl_persists_functions_and_structs() {
    let run = vantyx(
        &["repl"],
        Some(
            "struct Point:\n    x: Int\n    y: Int\n\n\
             fn add(a: Int, b: Int) -> Int:\n    return a + b\n\n\
             let p = Point(1, 2)\n\
             print(add(p.x, p.y))\n\
             exit\n",
        ),
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.contains("3"), "{}", run.stdout);
}

#[test]
fn repl_recovers_from_compile_errors() {
    let run = vantyx(&["repl"], Some("let x = 1\nx = 2\nprint(x)\nexit\n"));
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[E0203]"), "{}", run.stderr);
    assert!(run.stdout.contains("1"), "{}", run.stdout);
}

#[test]
fn repl_recovers_from_runtime_errors() {
    let run = vantyx(
        &["repl"],
        Some("let x = 1\nprint(1 // 0)\nprint(\"alive\")\nexit\n"),
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0001]"), "{}", run.stderr);
    assert!(run.stdout.contains("alive"), "{}", run.stdout);
}

#[test]
fn repl_echoes_new_expression_values_once() {
    let run = vantyx(
        &["repl"],
        Some("1 + 2\nlet x = 5\nx * 2\nprint(\"hi\")\n\"text\"\nexit\n"),
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout.matches(">>> 3").count(), 1, "{}", run.stdout);
    assert_eq!(run.stdout.matches(">>> 10").count(), 1, "{}", run.stdout);
    assert_eq!(run.stdout.matches(">>> text").count(), 1, "{}", run.stdout);
    assert_eq!(run.stdout.matches(">>> hi").count(), 1, "{}", run.stdout);
}

#[test]
fn repl_history_persists_across_sessions() {
    let path = std::env::temp_dir().join(format!(
        "vantyx-repl-history-{}-persist",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    let history = path.to_str().expect("utf-8 history path");
    let first = vantyx_with_env(
        &["repl"],
        Some("let persisted = 41\nprint(persisted + 1)\nexit\n"),
        &[("VANTYX_HISTORY", history)],
    );
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);
    assert!(first.stdout.contains("42"), "{}", first.stdout);
    let second = vantyx_with_env(
        &["repl"],
        Some(":history\nexit\n"),
        &[("VANTYX_HISTORY", history)],
    );
    assert_eq!(second.code, 0, "stderr: {}", second.stderr);
    assert!(
        second.stdout.contains("let persisted = 41"),
        "{}",
        second.stdout
    );
    assert!(
        second.stdout.contains("print(persisted + 1)"),
        "{}",
        second.stdout
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn repl_names_lists_visible_bindings() {
    let run = vantyx(
        &["repl"],
        Some("let alpha = 1\nfn helper():\n    return 2\n\n:names\nexit\n"),
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stdout.contains("alpha"), "{}", run.stdout);
    assert!(run.stdout.contains("helper"), "{}", run.stdout);
}

#[test]
fn repl_does_not_echo_after_compile_errors() {
    let run = vantyx(&["repl"], Some("1 + \"x\"\nexit\n"));
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error["), "{}", run.stderr);
    assert!(
        !run.stdout.lines().map(str::trim).any(|line| line == "1"),
        "{}",
        run.stdout
    );
}

#[test]
fn deep_recursion_reports_the_call_limit() {
    let run = run_file(
        "deep",
        "fn loop(n: Int) -> Int:\n    return loop(n + 1)\n\nloop(0)\n",
    );
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("error[R0012]") || run.stderr.contains("error[R0011]"),
        "{}",
        run.stderr
    );
}

#[test]
fn structs_and_string_interpolation_work_end_to_end() {
    let run = run_file(
        "structs",
        "struct Point:\n    x: Int\n    y: Int\n\n\
         let p = Point(3, 4)\nlet q = p\nq.x = 9\n\
         print(\"p = {p}, sum = {p.x + p.y}\")\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "p = Point(x=9, y=4), sum = 13\n");
}

#[test]
fn collections_and_loops_work_end_to_end() {
    let run = run_file(
        "collections",
        "var xs = [1, 2]\npush(xs, 3)\n\
         var m = {\"a\": 1}\nm[\"b\"] = 2\n\
         for x in xs:\n    print(x)\n\
         for k in m:\n    print(k)\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "1\n2\n3\na\nb\n");
}

#[test]
fn runtime_boundary_check_rejects_dynamic_wrong_type() {
    let run = run_file(
        "boundary",
        "fn identity(x: Any) -> Any:\n    return x\n\n\
         fn takes(n: Int) -> Int:\n    return n\n\n\
         takes(identity(\"no\"))\n",
    );
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("error[R0005]"), "{}", run.stderr);
}

#[test]
fn enums_and_match_statements_work_end_to_end() {
    let run = run_file(
        "enums-statements",
        "enum Token:\n    Number(value: Int)\n    Word(text: Str)\n    End\n\n\
         fn describe(token: Token) -> Str:\n    match token:\n\
         \x20       Number(value):\n            return \"number {value}\"\n\
         \x20       Word(text):\n            return \"word {text}\"\n\
         \x20       End:\n            return \"end\"\n\n\
         fn main():\n    print(describe(Number(42)))\n\
         \x20   print(describe(Word(\"hi\")))\n    print(describe(End()))\n\
         \x20   print(type(Number(1)))\n    print(Number(1) == Number(1))\n\
         \x20   print(Number(1) == Number(2))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "number 42\nword hi\nend\nToken\ntrue\nfalse\n");
}

#[test]
fn match_expressions_work_end_to_end() {
    let run = run_file(
        "enums-expressions",
        "enum Color:\n    Red\n    Green\n    Blue\n\n\
         fn score(color: Color) -> Int:\n    return match color:\n\
         \x20       Red: 1\n        Green: 2\n        Blue: 3\n\n\
         fn main():\n    let colors: List[Color] = [Red(), Green(), Blue()]\n\
         \x20   for color in colors:\n        print(score(color))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "1\n2\n3\n");
}

#[test]
fn non_exhaustive_match_is_rejected() {
    let run = check_file(
        "enums-exhaustive",
        "enum State:\n    Ready\n    Done\n\n\
         fn main():\n    let state = Ready()\n    match state:\n\
         \x20       Ready:\n            print(\"ready\")\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0316]"), "{}", run.stderr);
    assert!(run.stderr.contains("Done"), "{}", run.stderr);
}

#[test]
fn match_on_non_enum_is_rejected() {
    let run = check_file(
        "match-non-enum",
        "fn main():\n    match 1.5:\n        _:\n            print(\"x\")\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0314]"), "{}", run.stderr);
}

#[test]
fn unknown_variant_in_pattern_is_rejected() {
    let run = check_file(
        "match-unknown-variant",
        "enum Flag:\n    On\n\n\
         fn main():\n    let flag = On()\n    match flag:\n\
         \x20       Off:\n            print(1)\n        _:\n            print(2)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0315]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("unknown variant `Off`"),
        "{}",
        run.stderr
    );
}

#[test]
fn wrong_variant_enum_is_rejected() {
    let run = check_file(
        "match-wrong-enum",
        "enum Left:\n    A\n\nenum Right:\n    B\n\n\
         fn main():\n    let left = A()\n    match left:\n\
         \x20       B:\n            print(1)\n        _:\n            print(2)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0315]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("belongs to enum `Right`"),
        "{}",
        run.stderr
    );
}

#[test]
fn variant_payload_arity_is_checked() {
    let run = check_file(
        "variant-arity",
        "enum Maybe:\n    None\n    Some(value: Int)\n\n\
         fn main():\n    let value = None(1)\n    print(value)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0302]"), "{}", run.stderr);
}

#[test]
fn duplicate_variant_names_are_rejected() {
    let run = check_file(
        "variant-duplicate",
        "enum First:\n    Shared\n\nenum Second:\n    Shared\n\n\
         fn main():\n    print(1)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0202]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("variant constructor names are global"),
        "{}",
        run.stderr
    );
}

#[test]
fn dynamic_match_without_fallback_fails_at_runtime() {
    let run = run_file(
        "match-any-runtime",
        "enum Boxed:\n    Value(number: Int)\n\n\
         fn unwrap(value: Any) -> Int:\n    match value:\n\
         \x20       Value(number):\n            return number\n\
         \x20   return 0\n\n\
         fn main():\n    print(unwrap(1))\n",
    );
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("warning[W0004]"), "{}", run.stderr);
    assert!(run.stderr.contains("error[R0014]"), "{}", run.stderr);
}

#[test]
fn dynamic_match_with_fallback_works_at_runtime() {
    let run = run_file(
        "match-any-fallback",
        "enum Boxed:\n    Value(number: Int)\n\n\
         fn unwrap(value: Any) -> Int:\n    match value:\n\
         \x20       Value(number):\n            return number\n\
         \x20       _:\n            return 0\n\n\
         fn main():\n    print(unwrap(Value(7)))\n    print(unwrap(\"text\"))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "7\n0\n");
}

#[test]
fn match_expression_arm_types_must_agree() {
    let run = check_file(
        "match-arm-types",
        "enum Flag:\n    On\n\n\
         fn main():\n    let value = match On():\n\
         \x20       On: 1\n        _: \"no\"\n    print(value)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    assert!(run.stderr.contains("incompatible types"), "{}", run.stderr);
}

#[test]
fn scalar_literal_patterns_dispatch_at_runtime() {
    let run = run_file(
        "literal-patterns",
        "fn classify(n: Int) -> Str:\n    return match n:\n\
         \x20       0: \"zero\"\n\
         \x20       -1: \"minus\"\n\
         \x20       _: \"many\"\n\n\
         fn label(b: Bool) -> Str:\n    return match b:\n\
         \x20       true: \"yes\"\n\
         \x20       false: \"no\"\n\n\
         fn main():\n    for n in [-2, -1, 0, 2]:\n        print(classify(n))\n\
         \x20   print(label(true))\n    print(label(false))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "many\nminus\nzero\nmany\nyes\nno\n");
}

#[test]
fn match_guards_select_arms_at_runtime() {
    let run = run_file(
        "match-guards",
        r#"enum Shape:
    Circle(radius: Int)
    Rect(width: Int, height: Int)

fn classify(shape: Shape) -> Str:
    match shape:
        Circle(radius) if radius > 10:
            return "big circle"
        Circle(radius):
            return "circle {radius}"
        Rect(width, height) if width == height:
            return "square"
        Rect(width, height):
            return "rect"

fn sign(n: Int) -> Str:
    return match n:
        0: "zero"
        _ if n < 0: "negative"
        _: "positive"

fn greeting(name: Str, known: Bool) -> Str:
    return match name:
        "admin" if known: "admin known"
        "admin": "admin"
        _: "guest"

fn main():
    print(classify(Circle(20)))
    print(classify(Circle(5)))
    print(classify(Rect(3, 3)))
    print(classify(Rect(2, 5)))
    print(sign(-4))
    print(sign(0))
    print(sign(9))
    print(greeting("admin", true))
    print(greeting("admin", false))
    print(greeting("bob", true))
"#,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "big circle\ncircle 5\nsquare\nrect\nnegative\nzero\npositive\nadmin known\nadmin\nguest\n"
    );
}

#[test]
fn or_patterns_match_alternatives_and_bind_payloads() {
    let run = run_file(
        "or-patterns",
        "enum Shape:\n    Circle(radius: Int)\n    Square(side: Int)\n\n\
         fn value(shape: Shape) -> Int:\n    match shape:\n\
         \x20       Circle(size) | Square(size):\n            return size\n\n\
         fn classify(shape: Shape) -> Str:\n    match shape:\n\
         \x20       Circle(size) | Square(size) if size > 5:\n            return \"big\"\n\
         \x20       Circle(size) | Square(size):\n            return \"small\"\n\n\
         fn label(n: Int) -> Str:\n    return match n:\n\
         \x20       1 | 2: \"small\"\n\
         \x20       3 | 4 if n > 0: \"medium\"\n\
         \x20       _: \"other\"\n\n\
         fn flag(value: Bool) -> Str:\n    return match value:\n\
         \x20       true | false: \"covered\"\n\n\
         fn main():\n\
         \x20   print(value(Circle(7)))\n\
         \x20   print(value(Square(9)))\n\
         \x20   print(classify(Circle(6)))\n\
         \x20   print(classify(Square(2)))\n\
         \x20   print(label(2))\n\
         \x20   print(label(4))\n\
         \x20   print(label(8))\n\
         \x20   print(flag(false))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "7\n9\nbig\nsmall\nsmall\nmedium\nother\ncovered\n"
    );
}

#[test]
fn range_patterns_match_half_open_and_inclusive_bounds() {
    let run = run_file(
        "range-patterns",
        "fn classify(n: Int) -> Str:\n    return match n:\n\
         \x20       0..=9: \"digit\"\n\
         \x20       10..100: \"small\"\n\
         \x20       -10..0: \"negative\"\n\
         \x20       100..=100: \"hundred\"\n\
         \x20       _: \"other\"\n\n\
         enum Code:\n    Small(n: Int)\n    Big(n: Int)\n\n\
         fn label(code: Code) -> Str:\n    return match code:\n\
         \x20       Small(0..=9): \"small-digit\"\n\
         \x20       Small(n) if n < 0: \"small-negative\"\n\
         \x20       Small(_): \"small\"\n\
         \x20       Big(n) if n >= 1000: \"big\"\n\
         \x20       Big(_): \"medium\"\n\n\
         fn bucket(n: Int) -> Str:\n    return match n:\n\
         \x20       1..=3 | 5..=7: \"low\"\n\
         \x20       8..=9: \"high\"\n\
         \x20       _: \"other\"\n\n\
         fn main():\n\
         \x20   print(classify(0))\n\
         \x20   print(classify(9))\n\
         \x20   print(classify(10))\n\
         \x20   print(classify(99))\n\
         \x20   print(classify(-10))\n\
         \x20   print(classify(-1))\n\
         \x20   print(classify(100))\n\
         \x20   print(classify(101))\n\
         \x20   print(label(Small(5)))\n\
         \x20   print(label(Small(-3)))\n\
         \x20   print(label(Small(50)))\n\
         \x20   print(label(Big(1000)))\n\
         \x20   print(label(Big(4)))\n\
         \x20   print(bucket(2))\n\
         \x20   print(bucket(6))\n\
         \x20   print(bucket(8))\n\
         \x20   print(bucket(4))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "digit\ndigit\nsmall\nsmall\nnegative\nnegative\nhundred\nother\n\
         small-digit\nsmall-negative\nsmall\nbig\nmedium\nlow\nlow\nhigh\nother\n"
    );
}

#[test]
fn nested_patterns_bind_payloads_at_every_level() {
    let run = run_file(
        "nested-patterns",
        "enum Tree:\n    Leaf(value: Int)\n    Node(left: Tree, right: Tree)\n\n\
         fn sum(tree: Tree) -> Int:\n    match tree:\n\
         \x20       Leaf(value):\n            return value\n\
         \x20       Node(Leaf(left), Leaf(right)):\n            return left + right\n\
         \x20       Node(Leaf(left), _):\n            return left\n\
         \x20       Node(_, Leaf(right)):\n            return right\n\
         \x20       Node(_, _):\n            return 0\n\n\
         fn right_sum(tree: Tree) -> Int:\n    return match tree:\n\
         \x20       Leaf(value): value\n\
         \x20       Node(Leaf(value), right): value + right_sum(right)\n\
         \x20       Node(_, _): 0\n\n\
         enum Maybe:\n    Nothing\n    Just(value: Maybe)\n    Number(value: Int)\n\n\
         fn describe(value: Maybe) -> Str:\n    return match value:\n\
         \x20       Just(Nothing()): \"just nothing\"\n\
         \x20       Just(Just(_)): \"double\"\n\
         \x20       Just(Number(number)) if number > 0: \"positive\"\n\
         \x20       Just(Number(_)): \"number\"\n\
         \x20       Nothing(): \"nothing\"\n\
         \x20       Number(_): \"raw\"\n\n\
         fn main():\n\
         \x20   print(sum(Leaf(7)))\n\
         \x20   print(sum(Node(Leaf(2), Leaf(3))))\n\
         \x20   print(sum(Node(Leaf(4), Node(Leaf(9), Leaf(9)))))\n\
         \x20   print(sum(Node(Node(Leaf(9), Leaf(9)), Leaf(5))))\n\
         \x20   print(sum(Node(Node(Leaf(9), Leaf(9)), Node(Leaf(9), Leaf(9)))))\n\
         \x20   print(right_sum(Node(Leaf(1), Node(Leaf(2), Leaf(3)))))\n\
         \x20   print(right_sum(Node(Node(Leaf(9), Leaf(9)), Leaf(4))))\n\
         \x20   print(describe(Nothing()))\n\
         \x20   print(describe(Just(Nothing())))\n\
         \x20   print(describe(Just(Just(Nothing()))))\n\
         \x20   print(describe(Just(Number(7))))\n\
         \x20   print(describe(Just(Number(-1))))\n\
         \x20   print(describe(Number(3)))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "7\n5\n4\n5\n0\n6\n0\nnothing\njust nothing\ndouble\npositive\nnumber\nraw\n"
    );
}

#[test]
fn modules_run_and_check_end_to_end() {
    let directory = std::env::temp_dir().join(format!("vantyx-e2e-modules-{}", std::process::id()));
    fs::create_dir_all(directory.join("lib")).expect("create module dir");
    fs::write(
        directory.join("lib/math.vtx"),
        "fn double(x: Int) -> Int:\n    return x * 2\n",
    )
    .expect("write module");
    let entry = directory.join("main.vtx");
    fs::write(
        &entry,
        "from \"lib/math.vtx\" import double\n\nprint(double(21))\n",
    )
    .expect("write entry");

    let run = vantyx(&["run", entry.to_str().expect("utf-8 path")], None);
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "42\n");

    let check = vantyx(&["check", entry.to_str().expect("utf-8 path")], None);
    assert_eq!(check.code, 0, "stderr: {}", check.stderr);
    assert!(check.stdout.is_empty());

    fs::write(
        directory.join("lib/broken.vtx"),
        "fn broken() -> Int:\n    return \"no\"\n",
    )
    .expect("write broken module");
    let broken = directory.join("broken-main.vtx");
    fs::write(&broken, "import \"lib/broken.vtx\"\n\nprint(broken())\n")
        .expect("write broken entry");
    let failed = vantyx(&["check", broken.to_str().expect("utf-8 path")], None);
    assert_eq!(failed.code, 1);
    assert!(
        failed.stderr.contains("lib/broken.vtx"),
        "{}",
        failed.stderr
    );
    assert!(failed.stderr.contains("error[E0301]"), "{}", failed.stderr);
}

fn io_workspace(label: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("vantyx-e2e-io-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create io workspace");
    directory
}

#[test]
fn file_io_round_trips_and_reports_existence() {
    let directory = io_workspace("roundtrip");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   write_file(\"note.txt\", \"discarded\")\n\
         \x20   write_file(\"note.txt\", \"alpha\")\n\
         \x20   append_file(\"note.txt\", \" beta\")\n\
         \x20   print(read_file(\"note.txt\"))\n\
         \x20   print(file_exists(\"note.txt\"))\n\
         \x20   print(file_exists(\"missing.txt\"))\n\
         \x20   print(file_exists(\".\"))\n",
    )
    .expect("write io source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "alpha beta\ntrue\nfalse\ntrue\n");
    assert_eq!(
        fs::read_to_string(directory.join("note.txt")).expect("read note"),
        "alpha beta"
    );
}

#[test]
fn file_io_rejects_invalid_utf8_and_nul_contents() {
    let directory = io_workspace("invalid");
    fs::write(directory.join("bad-utf8.bin"), [0xff, 0xfe]).expect("write invalid utf8");
    fs::write(directory.join("nul.bin"), b"a\0b").expect("write nul");
    let invalid = directory.join("invalid.vtx");
    fs::write(
        &invalid,
        "fn main():\n    print(read_file(\"bad-utf8.bin\"))\n",
    )
    .expect("write invalid source");
    let nul = directory.join("nul.vtx");
    fs::write(&nul, "fn main():\n    print(read_file(\"nul.bin\"))\n").expect("write nul source");

    let invalid_run = vantyx_in_dir(
        &directory,
        &["run", invalid.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(invalid_run.code, 2, "stderr: {}", invalid_run.stderr);
    assert!(invalid_run.stdout.is_empty());
    assert!(
        invalid_run.stderr.contains("error[R0015]"),
        "{}",
        invalid_run.stderr
    );
    assert!(
        invalid_run.stderr.contains("not valid UTF-8"),
        "{}",
        invalid_run.stderr
    );

    let nul_run = vantyx_in_dir(
        &directory,
        &["run", nul.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(nul_run.code, 2, "stderr: {}", nul_run.stderr);
    assert!(nul_run.stdout.is_empty());
    assert!(
        nul_run.stderr.contains("error[R0015]"),
        "{}",
        nul_run.stderr
    );
    assert!(nul_run.stderr.contains("NUL byte"), "{}", nul_run.stderr);
}

#[test]
fn file_io_rejects_non_str_arguments_at_compile_time() {
    let run = check_file(
        "file-io-types",
        "fn main():\n\
         \x20   print(read_file(1))\n\
         \x20   write_file(\"out.txt\", 2)\n\
         \x20   append_file(3, \"text\")\n\
         \x20   print(file_exists(true))\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("as a file path for `read_file`"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("as the contents for `write_file`"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("as a file path for `append_file`"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("as a file path for `file_exists`"),
        "{}",
        run.stderr
    );
}

#[test]
fn directory_builtins_round_trip() {
    let directory = io_workspace("dirs");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   make_dir(\"work\")\n\
         \x20   make_dir(\"work/a\")\n\
         \x20   make_dir(\"work/b\")\n\
         \x20   write_file(\"work/a/note.txt\", \"hello\")\n\
         \x20   print(list_dir(\"work\"))\n\
         \x20   print(list_dir(\"work/a\"))\n\
         \x20   print(file_exists(\"work/a/note.txt\"))\n\
         \x20   remove_file(\"work/a/note.txt\")\n\
         \x20   remove_dir(\"work/a\")\n\
         \x20   remove_dir(\"work/b\")\n\
         \x20   remove_dir(\"work\")\n\
         \x20   print(file_exists(\"work\"))\n",
    )
    .expect("write directory source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "[\"a\", \"b\"]\n[\"note.txt\"]\ntrue\nfalse\n");
    assert!(!directory.join("work").exists());
}

#[test]
fn directory_builtins_reject_non_str_arguments() {
    let run = check_file(
        "dir-types",
        "fn main():\n\
         \x20   print(list_dir(1))\n\
         \x20   make_dir(2)\n\
         \x20   remove_file(3)\n\
         \x20   remove_dir(4)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    for message in [
        "as a directory path for `list_dir`",
        "as a path for `make_dir`",
        "as a path for `remove_file`",
        "as a path for `remove_dir`",
    ] {
        assert!(run.stderr.contains(message), "{}", run.stderr);
    }
}

#[test]
fn directory_errors_are_fatal_r0015() {
    for (label, setup, source) in [
        (
            "list-missing",
            None,
            "fn main():\n    print(list_dir(\"missing\"))\n",
        ),
        ("make-existing", None, "fn main():\n    make_dir(\".\")\n"),
        (
            "remove-file-dir",
            Some(("sub", true)),
            "fn main():\n    remove_file(\"sub\")\n",
        ),
        (
            "remove-dir-nonempty",
            Some(("nonempty/child.txt", false)),
            "fn main():\n    remove_dir(\"nonempty\")\n",
        ),
        (
            "list-file",
            Some(("plain.txt", false)),
            "fn main():\n    print(list_dir(\"plain.txt\"))\n",
        ),
    ] {
        let directory = io_workspace(label);
        if let Some((path, is_dir)) = setup {
            let path = directory.join(path);
            if is_dir {
                fs::create_dir_all(&path).expect("create setup directory");
            } else {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).expect("create setup parent");
                }
                fs::write(&path, "x").expect("write setup file");
            }
        }
        let source_path = directory.join("main.vtx");
        fs::write(&source_path, source).expect("write directory error source");
        let run = vantyx_in_dir(
            &directory,
            &["run", source_path.to_str().expect("utf-8 path")],
            None,
        );
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0015]"),
            "{label}: {}",
            run.stderr
        );
    }
}

#[test]
fn recursive_directory_builtins_round_trip() {
    let directory = io_workspace("recursive-dirs");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   make_dir_all(\"work/a/b/c\")\n\
         \x20   make_dir_all(\"work/a/b\")\n\
         \x20   make_dir_all(\"\")\n\
         \x20   write_file(\"work/a/b/c/note.txt\", \"hello\")\n\
         \x20   print(list_dir(\"work/a/b/c\"))\n\
         \x20   print(list_dir(\"work/a/b\"))\n\
         \x20   remove_dir_all(\"work/a/b/\")\n\
         \x20   print(file_exists(\"work/a/b\"))\n\
         \x20   remove_dir_all(\"work\")\n\
         \x20   print(file_exists(\"work\"))\n",
    )
    .expect("write recursive directory source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "[\"note.txt\"]\n[\"c\"]\nfalse\nfalse\n");
    assert!(!directory.join("work").exists());
}

#[test]
fn recursive_directory_builtins_reject_non_str_arguments() {
    let run = check_file(
        "recursive-dir-types",
        "fn main():\n\
         \x20   make_dir_all(1)\n\
         \x20   remove_dir_all(true)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    for message in [
        "as a path for `make_dir_all`",
        "as a path for `remove_dir_all`",
    ] {
        assert!(run.stderr.contains(message), "{}", run.stderr);
    }
}

#[test]
fn recursive_directory_errors_are_fatal_r0015() {
    for (label, setup, source) in [
        (
            "make-through-file",
            Some(("plain.txt", false)),
            "fn main():\n    make_dir_all(\"plain.txt/sub\")\n",
        ),
        (
            "make-final-file",
            Some(("plain.txt", false)),
            "fn main():\n    make_dir_all(\"plain.txt\")\n",
        ),
        (
            "remove-file",
            Some(("plain.txt", false)),
            "fn main():\n    remove_dir_all(\"plain.txt\")\n",
        ),
        (
            "remove-missing",
            None,
            "fn main():\n    remove_dir_all(\"missing\")\n",
        ),
        (
            "remove-empty",
            None,
            "fn main():\n    remove_dir_all(\"\")\n",
        ),
    ] {
        let directory = io_workspace(label);
        if let Some((path, is_dir)) = setup {
            let path = directory.join(path);
            if is_dir {
                fs::create_dir_all(&path).expect("create setup directory");
            } else {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).expect("create setup parent");
                }
                fs::write(&path, "x").expect("write setup file");
            }
        }
        let source_path = directory.join("main.vtx");
        fs::write(&source_path, source).expect("write recursive directory error source");
        let run = vantyx_in_dir(
            &directory,
            &["run", source_path.to_str().expect("utf-8 path")],
            None,
        );
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0015]"),
            "{label}: {}",
            run.stderr
        );
    }
}

#[test]
fn recursive_directory_dynamic_arguments_fail_at_runtime() {
    for (label, builtin) in [
        ("make-dir-all-dynamic", "make_dir_all"),
        ("remove-dir-all-dynamic", "remove_dir_all"),
    ] {
        let source = format!("fn run(path: Any):\n    {builtin}(path)\n\nfn main():\n    run(1)\n");
        let run = run_file(label, &source);
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0005]"),
            "{label}: {}",
            run.stderr
        );
        assert!(
            run.stderr.contains(&format!("as the path for `{builtin}`")),
            "{label}: {}",
            run.stderr
        );
    }
}

#[cfg(unix)]
#[test]
fn recursive_removal_refuses_destructive_roots() {
    for (label, path) in [
        ("dot", "."),
        ("dot-slash", "./"),
        ("dot-dot", ".."),
        ("dot-dot-slash", "../"),
        ("tree-dot", "tree/."),
        ("tree-dot-dot", "tree/.."),
        ("root", "/"),
        ("root-double", "//"),
    ] {
        let directory = io_workspace(&format!("removal-root-{label}"));
        fs::write(directory.join("sentinel.txt"), "keep").expect("write sentinel");
        fs::create_dir_all(directory.join("tree/inner")).expect("create tree");
        fs::write(directory.join("tree/inner/data.txt"), "keep").expect("write tree data");
        let source = directory.join("main.vtx");
        fs::write(
            &source,
            format!("fn main():\n    remove_dir_all(\"{path}\")\n"),
        )
        .expect("write destructive-root source");

        let run = vantyx_in_dir(
            &directory,
            &["run", source.to_str().expect("utf-8 path")],
            None,
        );
        assert_eq!(run.code, 2, "{label}: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0015]"),
            "{label}: {}",
            run.stderr
        );
        assert!(
            run.stderr.contains("refusing to remove"),
            "{label}: {}",
            run.stderr
        );
        assert!(
            directory.join("sentinel.txt").exists(),
            "{label}: sentinel was deleted"
        );
        assert!(
            directory.join("tree/inner/data.txt").exists(),
            "{label}: tree was deleted"
        );
        assert!(source.exists(), "{label}: source was deleted");
    }
}

#[test]
fn recursive_removal_allows_a_leading_dot_prefix() {
    let directory = io_workspace("removal-leading-dot");
    fs::write(directory.join("sentinel.txt"), "keep").expect("write sentinel");
    fs::create_dir_all(directory.join("tree/inner")).expect("create tree");
    fs::write(directory.join("tree/inner/data.txt"), "keep").expect("write tree data");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   remove_dir_all(\"./tree\")\n\
         \x20   print(file_exists(\"tree\"))\n\
         \x20   print(file_exists(\"sentinel.txt\"))\n",
    )
    .expect("write leading-dot source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "false\ntrue\n");
    assert!(!directory.join("tree").exists());
    assert!(directory.join("sentinel.txt").exists());
}

#[test]
fn recursive_directory_removal_does_not_follow_symlinks() {
    use std::os::unix::fs::symlink;

    let directory = io_workspace("recursive-symlink");
    let target = directory.join("target");
    fs::create_dir_all(target.join("inner")).expect("create symlink target");
    fs::write(target.join("inner/data.txt"), "keep").expect("write target data");
    fs::create_dir_all(directory.join("tree/sub")).expect("create tree");
    symlink(&target, directory.join("tree/sub/link")).expect("create inner symlink");
    symlink(&target, directory.join("root-link")).expect("create root symlink");
    symlink(&target, directory.join("make-link")).expect("create make symlink");

    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   make_dir_all(\"make-link\")\n\
         \x20   remove_dir_all(\"tree\")\n\
         \x20   print(file_exists(\"tree\"))\n\
         \x20   print(file_exists(\"target/inner/data.txt\"))\n\
         \x20   remove_dir_all(\"root-link/\")\n\
         \x20   print(file_exists(\"root-link\"))\n\
         \x20   print(file_exists(\"target/inner/data.txt\"))\n",
    )
    .expect("write symlink source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "false\ntrue\nfalse\ntrue\n");
    assert!(!directory.join("tree").exists());
    assert!(directory.join("root-link").symlink_metadata().is_err());
    assert_eq!(
        fs::read_to_string(target.join("inner/data.txt")).expect("read target data"),
        "keep"
    );
}

#[test]
fn file_metadata_builtins_round_trip() {
    let directory = io_workspace("file-metadata");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   write_file(\"data.txt\", \"hello\")\n\
         \x20   make_dir(\"folder\")\n\
         \x20   print(file_size(\"data.txt\"))\n\
         \x20   print(is_file(\"data.txt\"))\n\
         \x20   print(is_file(\"folder\"))\n\
         \x20   print(is_dir(\"folder\"))\n\
         \x20   print(is_dir(\"data.txt\"))\n\
         \x20   print(is_symlink(\"data.txt\"))\n\
         \x20   print(is_file(\"missing\"))\n\
         \x20   print(is_dir(\"missing\"))\n\
         \x20   print(is_symlink(\"missing\"))\n\
         \x20   remove_file(\"data.txt\")\n\
         \x20   remove_dir(\"folder\")\n",
    )
    .expect("write metadata source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "5\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\nfalse\nfalse\n"
    );
}

#[test]
fn file_metadata_builtins_reject_non_str_arguments() {
    let run = check_file(
        "file-metadata-types",
        "fn main():\n\
         \x20   file_size(1)\n\
         \x20   is_file(true)\n\
         \x20   is_dir(1.5)\n\
         \x20   is_symlink([\"x\"])\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    for message in [
        "as a file path for `file_size`",
        "as a file path for `is_file`",
        "as a file path for `is_dir`",
        "as a file path for `is_symlink`",
    ] {
        assert!(run.stderr.contains(message), "{}", run.stderr);
    }
}

#[test]
fn file_metadata_errors_are_fatal_r0015() {
    for (label, setup, source) in [
        (
            "metadata-directory",
            true,
            "fn main():\n    print(file_size(\"folder\"))\n",
        ),
        (
            "metadata-missing",
            false,
            "fn main():\n    print(file_size(\"missing\"))\n",
        ),
        (
            "metadata-empty",
            false,
            "fn main():\n    print(file_size(\"\"))\n",
        ),
    ] {
        let directory = io_workspace(label);
        if setup {
            fs::create_dir(directory.join("folder")).expect("create folder");
        }
        let source_path = directory.join("main.vtx");
        fs::write(&source_path, source).expect("write metadata error source");
        let run = vantyx_in_dir(
            &directory,
            &["run", source_path.to_str().expect("utf-8 path")],
            None,
        );
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0015]"),
            "{label}: {}",
            run.stderr
        );
    }
}

#[test]
fn file_metadata_dynamic_arguments_fail_at_runtime() {
    for (label, builtin) in [
        ("file-size-dynamic", "file_size"),
        ("is-file-dynamic", "is_file"),
        ("is-dir-dynamic", "is_dir"),
        ("is-symlink-dynamic", "is_symlink"),
    ] {
        let source =
            format!("fn run(path: Any):\n    print({builtin}(path))\n\nfn main():\n    run(1)\n");
        let run = run_file(label, &source);
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0005]"),
            "{label}: {}",
            run.stderr
        );
        assert!(
            run.stderr
                .contains(&format!("as a file path for `{builtin}`")),
            "{label}: {}",
            run.stderr
        );
    }
}

#[cfg(unix)]
#[test]
fn file_metadata_symlink_rules() {
    use std::os::unix::fs::symlink;

    let directory = io_workspace("file-metadata-symlinks");
    fs::write(directory.join("target.txt"), "hello").expect("write target");
    fs::create_dir(directory.join("target-dir")).expect("create target dir");
    symlink("target.txt", directory.join("link.txt")).expect("create file link");
    symlink("target-dir", directory.join("link-dir")).expect("create dir link");
    symlink("absent", directory.join("dangling")).expect("create dangling link");

    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   print(file_size(\"link.txt\"))\n\
         \x20   print(is_file(\"link.txt\"))\n\
         \x20   print(is_symlink(\"link.txt\"))\n\
         \x20   print(is_dir(\"link-dir\"))\n\
         \x20   print(is_symlink(\"link-dir\"))\n\
         \x20   print(is_file(\"dangling\"))\n\
         \x20   print(is_dir(\"dangling\"))\n\
         \x20   print(is_symlink(\"dangling\"))\n",
    )
    .expect("write symlink metadata source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "5\ntrue\ntrue\ntrue\ntrue\nfalse\nfalse\ntrue\n"
    );
}

#[test]
fn rename_and_copy_round_trip() {
    let directory = io_workspace("rename-copy");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   write_file(\"source.txt\", \"hello\")\n\
         \x20   rename(\"source.txt\", \"renamed.txt\")\n\
         \x20   print(file_exists(\"source.txt\"))\n\
         \x20   print(read_file(\"renamed.txt\"))\n\
         \x20   copy_file(\"renamed.txt\", \"copy.txt\")\n\
         \x20   print(read_file(\"copy.txt\"))\n\
         \x20   write_file(\"copy.txt\", \"stale\")\n\
         \x20   copy_file(\"renamed.txt\", \"copy.txt\")\n\
         \x20   print(read_file(\"copy.txt\"))\n\
         \x20   make_dir(\"folder\")\n\
         \x20   copy_file(\"renamed.txt\", \"folder/inner.txt\")\n\
         \x20   print(list_dir(\"folder\"))\n\
         \x20   rename(\"folder\", \"moved\")\n\
         \x20   print(file_exists(\"folder\"))\n\
         \x20   print(list_dir(\"moved\"))\n\
         \x20   remove_file(\"moved/inner.txt\")\n\
         \x20   remove_dir(\"moved\")\n\
         \x20   remove_file(\"renamed.txt\")\n\
         \x20   remove_file(\"copy.txt\")\n\
         \x20   print(file_exists(\"moved\"))\n",
    )
    .expect("write rename/copy source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "false\nhello\nhello\nhello\n[\"inner.txt\"]\nfalse\n[\"inner.txt\"]\nfalse\n"
    );
    assert!(!directory.join("source.txt").exists());
    assert!(!directory.join("renamed.txt").exists());
    assert!(!directory.join("copy.txt").exists());
    assert!(!directory.join("moved").exists());
}

#[test]
fn copy_file_preserves_bytes() {
    let directory = io_workspace("copy-bytes");
    let bytes: Vec<u8> = vec![0, 0xff, 0xfe, 1, 2, 3, 0, 0x80, 0x7f];
    fs::write(directory.join("blob.bin"), &bytes).expect("write binary source");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   copy_file(\"blob.bin\", \"blob-copy.bin\")\n\
         \x20   print(file_exists(\"blob-copy.bin\"))\n",
    )
    .expect("write copy source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "true\n");
    assert_eq!(
        fs::read(directory.join("blob-copy.bin")).expect("read copied bytes"),
        bytes
    );
}

#[test]
fn copy_file_rejects_self_copies() {
    let directory = io_workspace("copy-self");
    fs::write(directory.join("source.txt"), "keep me").expect("write source");
    let mut targets = vec!["source.txt".to_string()];
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink("source.txt", directory.join("link.txt")).expect("create symlink");
        fs::hard_link(directory.join("source.txt"), directory.join("hard.txt"))
            .expect("create hard link");
        targets.push("link.txt".to_string());
        targets.push("hard.txt".to_string());
    }

    for target in &targets {
        let source = directory.join("main.vtx");
        fs::write(
            &source,
            format!("fn main():\n    copy_file(\"source.txt\", \"{target}\")\n"),
        )
        .expect("write self-copy source");
        let run = vantyx_in_dir(
            &directory,
            &["run", source.to_str().expect("utf-8 path")],
            None,
        );
        assert_eq!(run.code, 2, "{target}: {}", run.stderr);
        assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
        assert!(
            run.stderr.contains("source and target are the same file"),
            "{}",
            run.stderr
        );
        assert_eq!(
            fs::read_to_string(directory.join("source.txt")).expect("read preserved source"),
            "keep me",
            "copying onto `{target}` changed the source"
        );
    }

    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   copy_file(\"source.txt\", \"copy.txt\")\n\
         \x20   print(read_file(\"copy.txt\"))\n",
    )
    .expect("write ordinary copy source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "keep me\n");
}

#[test]
fn rename_and_copy_reject_non_str_arguments() {
    let run = check_file(
        "rename-copy-types",
        "fn main():\n\
         \x20   rename(1, 2)\n\
         \x20   copy_file(3, 4)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    for message in [
        "as the source path for `rename`",
        "as the target path for `rename`",
        "as the source path for `copy_file`",
        "as the target path for `copy_file`",
    ] {
        assert!(run.stderr.contains(message), "{}", run.stderr);
    }
}

#[test]
fn rename_and_copy_dynamic_arguments_fail_at_runtime() {
    for (label, builtin) in [("rename", "rename"), ("copy", "copy_file")] {
        let source = format!(
            "fn move_it(src: Any, dst: Any):\n    {builtin}(src, dst)\n\nfn main():\n    move_it(1, 2)\n"
        );
        let run = run_file(&format!("{label}-dynamic"), &source);
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0005]"),
            "{label}: {}",
            run.stderr
        );
        assert!(
            run.stderr
                .contains(&format!("as the source path for `{builtin}`")),
            "{label}: {}",
            run.stderr
        );
    }
}

#[test]
fn rename_and_copy_errors_are_fatal_r0015() {
    for (label, setup, source) in [
        (
            "rename-missing",
            None,
            "fn main():\n    rename(\"missing\", \"x\")\n",
        ),
        (
            "copy-missing",
            None,
            "fn main():\n    copy_file(\"missing\", \"x\")\n",
        ),
        (
            "copy-directory",
            Some(("dir", true)),
            "fn main():\n    copy_file(\"dir\", \"x\")\n",
        ),
        (
            "copy-missing-parent",
            Some(("file.txt", false)),
            "fn main():\n    copy_file(\"file.txt\", \"missing/x\")\n",
        ),
        (
            "rename-over-directory",
            Some(("file.txt", false)),
            "fn main():\n    make_dir(\"dir\")\n    rename(\"file.txt\", \"dir\")\n",
        ),
    ] {
        let directory = io_workspace(label);
        if let Some((path, is_dir)) = setup {
            let path = directory.join(path);
            if is_dir {
                fs::create_dir_all(&path).expect("create setup directory");
            } else {
                fs::write(&path, "x").expect("write setup file");
            }
        }
        let source_path = directory.join("main.vtx");
        fs::write(&source_path, source).expect("write error source");
        let run = vantyx_in_dir(
            &directory,
            &["run", source_path.to_str().expect("utf-8 path")],
            None,
        );
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0015]"),
            "{label}: {}",
            run.stderr
        );
    }
}

#[cfg(unix)]
#[test]
fn list_dir_rejects_non_utf8_entry_names() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = io_workspace("dir-invalid-name");
    fs::create_dir(directory.join(OsStr::from_bytes(b"bad-\xff"))).expect("create invalid name");
    let source = directory.join("main.vtx");
    fs::write(&source, "fn main():\n    print(list_dir(\".\"))\n").expect("write source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
    assert!(run.stderr.contains("not valid UTF-8"), "{}", run.stderr);
}

#[test]
fn text_builtins_round_trip() {
    let run = run_file(
        "text-builtins",
        "fn main():\n\
         \x20   print(split(\"a,b,,c,\", \",\"))\n\
         \x20   print(split(\"aéb\", \"\"))\n\
         \x20   print(join([\"a\", \"b\", \"c\"], \"-\"))\n\
         \x20   let empty: List[Str] = []\n\
         \x20   print(join(empty, \",\"))\n\
         \x20   print(trim(\" \\t\\n\u{a0}héllo\u{3000} \"))\n\
         \x20   print(len(trim(\"\u{200b}x\u{200b}\")))\n\
         \x20   print(len(trim(\"\u{1c}z\u{1f}\")))\n\
         \x20   print(starts_with(\"hello\", \"he\"))\n\
         \x20   print(starts_with(\"hello\", \"lo\"))\n\
         \x20   print(ends_with(\"hello\", \"lo\"))\n\
         \x20   print(ends_with(\"hello\", \"he\"))\n\
         \x20   print(replace(\"one two two\", \"two\", \"2\"))\n\
         \x20   print(replace(\"aaa\", \"aa\", \"b\"))\n\
         \x20   print(replace(\"abc\", \"x\", \"\"))\n\
         \x20   print(split(\"\", \"\"))\n\
         \x20   print(split(\"\", \",\"))\n\
         \x20   print(starts_with(\"\", \"\"))\n\
         \x20   print(ends_with(\"\", \"\"))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "[\"a\", \"b\", \"\", \"c\", \"\"]\n\
         [\"a\", \"é\", \"b\"]\n\
         a-b-c\n\
         \n\
         héllo\n\
         3\n\
         3\n\
         true\n\
         false\n\
         true\n\
         false\n\
         one 2 2\n\
         ba\n\
         abc\n\
         []\n\
         [\"\"]\n\
         true\n\
         true\n"
    );
}

#[test]
fn text_builtins_reject_non_str_arguments() {
    let run = check_file(
        "text-types",
        "fn main():\n\
         \x20   print(split(1, \",\"))\n\
         \x20   print(join([1, 2], \",\"))\n\
         \x20   print(trim(true))\n\
         \x20   print(starts_with(\"a\", 1))\n\
         \x20   print(ends_with(1, \"a\"))\n\
         \x20   print(replace(\"a\", \"b\", 1))\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    for message in [
        "as the text for `split`",
        "`join` expects List[Str], found `List[Int]`",
        "as the text for `trim`",
        "as the affix for `starts_with`",
        "as the text for `ends_with`",
        "as the replacement for `replace`",
    ] {
        assert!(run.stderr.contains(message), "{}", run.stderr);
    }
}

#[test]
fn text_replace_rejects_empty_pattern() {
    let run = run_file(
        "text-empty-pattern",
        "fn main():\n    print(replace(\"abc\", \"\", \"x\"))\n",
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0016]"), "{}", run.stderr);
    assert!(run.stderr.contains("non-empty pattern"), "{}", run.stderr);
}

#[test]
fn sorted_orders_scalar_lists_stably() {
    let run = run_file(
        "sorted",
        "fn main():\n\
         \x20   let ints: List[Int] = [3, -1, 2, 0, -1]\n\
         \x20   print(sorted(ints))\n\
         \x20   print(ints)\n\
         \x20   let ordered = sorted(ints)\n\
         \x20   push(ordered, 99)\n\
         \x20   print(ordered)\n\
         \x20   print(ints)\n\
         \x20   let strings: List[Str] = [\"b\", \"a\", \"aa\", \"\", \"A\", \"z\", \"é\"]\n\
         \x20   print(sorted(strings))\n\
         \x20   let flags: List[Bool] = [true, false, true]\n\
         \x20   print(sorted(flags))\n\
         \x20   let floats: List[Float] = [2.5, -0.0, 0.0, 1.5, float(\"NaN\"), -1.0, float(\"NaN\")]\n\
         \x20   print(sorted(floats))\n\
         \x20   print(sorted([0.0, -0.0]))\n\
         \x20   let empty: List[Int] = []\n\
         \x20   print(sorted(empty))\n\
         \x20   let one: List[Str] = [\"only\"]\n\
         \x20   print(sorted(one))\n\
         \x20   let duplicates: List[Int] = [1, 1, 0, 0, 2, 2]\n\
         \x20   print(sorted(duplicates))\n\
         \x20   print(sorted([3, 1, 2]))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "[-1, -1, 0, 2, 3]\n\
         [3, -1, 2, 0, -1]\n\
         [-1, -1, 0, 2, 3, 99]\n\
         [3, -1, 2, 0, -1]\n\
         [\"\", \"A\", \"a\", \"aa\", \"b\", \"z\", \"é\"]\n\
         [false, true, true]\n\
         [-1.0, -0.0, 0.0, 1.5, 2.5, NaN, NaN]\n\
         [0.0, -0.0]\n\
         []\n\
         [\"only\"]\n\
         [0, 0, 1, 1, 2, 2]\n\
         [1, 2, 3]\n"
    );
}

#[test]
fn sorted_rejects_non_scalar_lists_and_wrong_arity() {
    let run = check_file(
        "sorted-types",
        "struct Point:\n\
         \x20   x: Int\n\
         \n\
         fn main():\n\
         \x20   print(sorted(3))\n\
         \x20   print(sorted())\n\
         \x20   print(sorted([[1, 2]]))\n\
         \x20   let points: List[Point] = [Point(x=2), Point(x=1)]\n\
         \x20   print(sorted(points))\n\
         \x20   let mixed: List[Any] = [1, \"a\"]\n\
         \x20   print(sorted(mixed))\n",
    );
    assert_eq!(run.code, 1);
    assert_eq!(
        run.stderr.matches("error[E0301]").count(),
        4,
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("error[E0302]"), "{}", run.stderr);
    for message in [
        "`sorted` expects a List, found `Int`",
        "`sorted` expects 1 argument(s), found 0",
        "`sorted` requires a List of Int, Float, Str, or Bool, found `List[List[Int]]`",
        "`sorted` requires a List of Int, Float, Str, or Bool, found `List[Point]`",
        "`sorted` requires a List of Int, Float, Str, or Bool, found `List[Any]`",
    ] {
        assert!(run.stderr.contains(message), "{}", run.stderr);
    }
}

#[test]
fn sorted_dynamic_any_value_uses_runtime_type_checks() {
    let run = run_file(
        "sorted-any",
        "fn main():\n\
         \x20   let value: Any = [3, 1, 2]\n\
         \x20   print(sorted(value))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "[1, 2, 3]\n");
}

#[test]
fn sorted_dynamic_any_value_rejects_mixed_lists() {
    let run = run_file(
        "sorted-any-mixed",
        "fn main():\n\
         \x20   let value: Any = [1, \"a\"]\n\
         \x20   print(sorted(value))\n",
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0005]"), "{}", run.stderr);
    assert!(
        run.stderr
            .contains("homogeneous List of Int, Float, Str, or Bool"),
        "{}",
        run.stderr
    );
}

#[test]
fn path_builtins_round_trip() {
    let run = run_file(
        "path-builtins",
        "fn main():\n\
         \x20   print(path_join(\"a\", \"b\"))\n\
         \x20   print(path_join(\"a/\", \"b\"))\n\
         \x20   print(path_join(\"a\", \"/b\"))\n\
         \x20   print(path_join(\"\", \"b\"))\n\
         \x20   print(path_join(\"a\", \"\"))\n\
         \x20   print(path_join(\"a//\", \"b\"))\n\
         \x20   print(path_parent(\"a/b/c\"))\n\
         \x20   print(path_parent(\"a\"))\n\
         \x20   print(path_parent(\"/a\"))\n\
         \x20   print(path_parent(\"/\"))\n\
         \x20   print(path_parent(\"a//b\"))\n\
         \x20   print(path_parent(\"a/b/\"))\n\
         \x20   print(path_name(\"a/b/\"))\n\
         \x20   print(path_name(\"/\"))\n\
         \x20   print(path_name(\"naïve/文件.txt\"))\n\
         \x20   print(path_stem(\"a/b.tar.gz\"))\n\
         \x20   print(path_stem(\".bashrc\"))\n\
         \x20   print(path_stem(\"a.\"))\n\
         \x20   print(path_stem(\"archive\"))\n\
         \x20   print(path_stem(\"a/b..\"))\n\
         \x20   print(path_stem(\"a/...\"))\n\
         \x20   print(path_stem(\"a.b.\"))\n\
         \x20   print(path_stem(\"a.b..\"))\n\
         \x20   print(path_extension(\"a/b.tar.gz\"))\n\
         \x20   print(path_extension(\".bashrc\"))\n\
         \x20   print(path_extension(\"a.\"))\n\
         \x20   print(path_extension(\"archive\"))\n\
         \x20   print(path_extension(\"archive.tar\"))\n\
         \x20   print(path_extension(\"a/b..\"))\n\
         \x20   print(path_extension(\"a/...\"))\n\
         \x20   print(path_extension(\"a.b.\"))\n\
         \x20   print(path_extension(\"a.b..\"))\n\
         \x20   print(path_is_absolute(\"/a\"))\n\
         \x20   print(path_is_absolute(\"a\"))\n\
         \x20   print(path_is_absolute(\"\"))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "a/b\na/b\n/b\nb\na\na//b\na/b\n.\n/\n/\na\na\nb\n\n文件.txt\nb.tar\n.bashrc\na.\narchive\nb..\n...\na.b.\na.b..\ngz\n\n\n\ntar\n\n\n\n\ntrue\nfalse\nfalse\n"
    );
}

#[test]
fn path_builtins_reject_non_str_arguments() {
    let run = check_file(
        "path-types",
        "fn main():\n\
         \x20   print(path_join(1, \"a\"))\n\
         \x20   print(path_join(\"a\", 2))\n\
         \x20   print(path_parent(1))\n\
         \x20   print(path_name(true))\n\
         \x20   print(path_stem(1))\n\
         \x20   print(path_extension(1))\n\
         \x20   print(path_is_absolute(1))\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    for message in [
        "as the base path for `path_join`",
        "as the child path for `path_join`",
        "as the path for `path_parent`",
        "as the path for `path_name`",
        "as the path for `path_stem`",
        "as the path for `path_extension`",
        "as the path for `path_is_absolute`",
    ] {
        assert!(run.stderr.contains(message), "{}", run.stderr);
    }
}

#[test]
fn path_builtins_dynamic_arguments_fail_at_runtime() {
    let run = run_file(
        "path-dynamic",
        "fn parent(path: Any) -> Str:\n\
         \x20   return path_parent(path)\n\
         \n\
         fn main():\n\
         \x20   print(parent(1))\n",
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0005]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("as the path for `path_parent`"),
        "{}",
        run.stderr
    );
}

#[cfg(unix)]
#[test]
fn glob_round_trip() {
    use std::os::unix::fs::symlink;

    let directory = io_workspace("glob");
    let tree = directory.join("tree");
    fs::create_dir_all(tree.join("dir/sub/deep")).expect("create glob dirs");
    fs::create_dir_all(tree.join(".hidden")).expect("create glob hidden dir");
    fs::write(tree.join("a.txt"), "a").expect("write glob a");
    fs::write(tree.join("b.vtx"), "b").expect("write glob b");
    fs::write(tree.join("dir/b.txt"), "c").expect("write glob c");
    fs::write(tree.join("dir/.dot"), "d").expect("write glob dot");
    fs::write(tree.join("dir/sub/c.vtx"), "e").expect("write glob e");
    fs::write(tree.join("dir/sub/deep/d.txt"), "f").expect("write glob f");
    fs::write(tree.join(".hidden/h.txt"), "g").expect("write glob g");
    symlink("dir", tree.join("link")).expect("create glob link");
    symlink("missing", tree.join("dangling")).expect("create glob dangling link");

    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   print(glob(\"tree/*.txt\"))\n\
         \x20   print(glob(\"tree/**/*.txt\"))\n\
         \x20   print(glob(\"tree/**\"))\n\
         \x20   print(glob(\"tree/.*\"))\n\
         \x20   print(glob(\"tree/**/.*\"))\n\
         \x20   print(glob(\"tree/dir/[bs]*\"))\n\
         \x20   print(glob(\"tree/missing/*\"))\n\
         \x20   print(glob(\"tree/dangling\"))\n\
         \x20   print(glob(\"tree/./dir/*\"))\n\
         \x20   print(glob(\"tree/dir/**\"))\n\
         \x20   print(glob(\"tree/link/*\"))\n\
         \x20   print(glob(\"tree/link/**\"))\n",
    )
    .expect("write glob source");

    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "[\"tree/a.txt\"]\n\
         [\"tree/a.txt\", \"tree/dir/b.txt\", \"tree/dir/sub/deep/d.txt\"]\n\
         [\"tree/a.txt\", \"tree/b.vtx\", \"tree/dangling\", \"tree/dir\", \"tree/dir/b.txt\", \"tree/dir/sub\", \"tree/dir/sub/c.vtx\", \"tree/dir/sub/deep\", \"tree/dir/sub/deep/d.txt\", \"tree/link\"]\n\
         [\"tree/.hidden\"]\n\
         [\"tree/.hidden\", \"tree/dir/.dot\"]\n\
         [\"tree/dir/b.txt\", \"tree/dir/sub\"]\n\
         []\n\
         [\"tree/dangling\"]\n\
         [\"tree/./dir/b.txt\", \"tree/./dir/sub\"]\n\
         [\"tree/dir/b.txt\", \"tree/dir/sub\", \"tree/dir/sub/c.vtx\", \"tree/dir/sub/deep\", \"tree/dir/sub/deep/d.txt\"]\n\
         [\"tree/link/b.txt\", \"tree/link/sub\"]\n\
         [\"tree/link/b.txt\", \"tree/link/sub\", \"tree/link/sub/c.vtx\", \"tree/link/sub/deep\", \"tree/link/sub/deep/d.txt\"]\n"
    );
}

#[test]
fn glob_rejects_malformed_patterns_with_r0016() {
    for (label, pattern) in [
        ("glob-unclosed", "["),
        ("glob-empty-class", "[]"),
        ("glob-negated-unclosed", "[!]"),
        ("glob-descending", "[z-a]"),
        ("glob-empty-pattern", ""),
    ] {
        let source = format!("fn main():\n    print(glob(\"{pattern}\"))\n");
        let run = run_file(label, &source);
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0016]"),
            "{label}: {}",
            run.stderr
        );
    }
}

#[test]
fn glob_rejects_non_str_arguments() {
    let run = check_file(
        "glob-types",
        "fn main():\n\
         \x20   print(glob(1))\n\
         \x20   print(glob())\n\
         \x20   print(glob(\"a\", \"b\"))\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    assert!(run.stderr.contains("error[E0302]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("as the pattern for `glob`"),
        "{}",
        run.stderr
    );
}

#[test]
fn glob_dynamic_arguments_fail_at_runtime() {
    let run = run_file(
        "glob-dynamic",
        "fn probe(pattern: Any):\n\
         \x20   print(glob(pattern))\n\
         \n\
         fn main():\n\
         \x20   probe(1)\n",
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0005]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("as the pattern for `glob`"),
        "{}",
        run.stderr
    );
}

#[test]
fn glob_depth_limit_is_fatal_r0015() {
    let directory = io_workspace("glob-depth");
    let mut path = directory.join("deep");
    for index in 0..130 {
        path.push(format!("d{index}"));
    }
    fs::create_dir_all(&path).expect("create deep glob tree");
    let source = directory.join("main.vtx");
    fs::write(&source, "fn main():\n    print(glob(\"**\"))\n").expect("write depth source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
    assert!(run.stderr.contains("maximum depth"), "{}", run.stderr);
}

#[cfg(unix)]
#[test]
fn glob_rejects_non_utf8_entry_names() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = io_workspace("glob-invalid-name");
    fs::create_dir(directory.join(OsStr::from_bytes(b"bad-\xff"))).expect("create invalid name");
    let source = directory.join("main.vtx");
    fs::write(&source, "fn main():\n    print(glob(\"*\"))\n").expect("write source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("non-UTF-8 entry name"),
        "{}",
        run.stderr
    );
}

#[test]
fn glob_rejects_nul_patterns_with_r0015() {
    let directory = io_workspace("glob-nul");
    let source = directory.join("main.vtx");
    fs::write(&source, "fn main():\n    print(glob(\"a\0b\"))\n").expect("write nul source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
    assert!(run.stderr.contains("NUL byte"), "{}", run.stderr);
}

#[test]
fn glob_absolute_and_repeated_separators_are_lexical() {
    let directory = io_workspace("glob-absolute");
    fs::create_dir_all(directory.join("tree")).expect("create glob tree");
    fs::write(directory.join("tree/a.txt"), "a").expect("write glob file");
    let root = directory.to_str().expect("utf-8 path");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        format!(
            "fn main():\n\
             \x20   print(glob(\"{root}/tree/*.txt\"))\n\
             \x20   print(glob(\"{root}//tree//*.txt\"))\n"
        ),
    )
    .expect("write absolute glob source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    let expected = format!("[\"{root}/tree/a.txt\"]\n");
    assert_eq!(run.stdout, expected.repeat(2));
}

#[test]
fn glob_component_cap_is_r0016() {
    let source = format!("fn main():\n    print(glob(\"{}\"))\n", "a/".repeat(1025));
    let run = run_file("glob-components", &source);
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0016]"), "{}", run.stderr);
}

#[test]
fn recursive_copy_round_trip() {
    let directory = io_workspace("copy-roundtrip");
    let tree = directory.join("tree");
    fs::create_dir_all(tree.join("sub/empty")).expect("create copy tree");
    fs::create_dir_all(tree.join(".hidden")).expect("create copy hidden dir");
    fs::write(tree.join("a.txt"), "alpha").expect("write copy a");
    fs::write(tree.join("sub/b.bin"), [0_u8, 255, 1]).expect("write copy binary");
    fs::write(tree.join("sub/b.txt"), "beta").expect("write copy b");
    fs::write(tree.join(".hidden/dot"), "dot").expect("write copy dot");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   copy_dir_all(\"tree\", \"copy\")\n\
         \x20   print(read_file(\"copy/a.txt\"))\n\
         \x20   print(read_file(\"copy/sub/b.txt\"))\n\
         \x20   print(read_file(\"copy/.hidden/dot\"))\n\
         \x20   print(is_dir(\"copy/sub/empty\"))\n\
         \x20   print(glob(\"copy/**/*.txt\"))\n\
         \x20   print(file_exists(\"tree/a.txt\"))\n",
    )
    .expect("write copy source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(
        run.stdout,
        "alpha\n\
         beta\n\
         dot\n\
         true\n\
         [\"copy/a.txt\", \"copy/sub/b.txt\"]\n\
         true\n"
    );
    assert_eq!(
        fs::read(directory.join("copy/sub/b.bin")).expect("read copied binary"),
        [0, 255, 1]
    );
    assert!(directory.join("copy/sub/empty").is_dir());
    assert_eq!(
        fs::read_to_string(tree.join("a.txt")).expect("read source a"),
        "alpha"
    );
    assert!(tree.join("sub/b.bin").is_file());
}

#[test]
fn recursive_copy_rejects_non_str_arguments() {
    let run = check_file(
        "copy-types",
        "fn main():\n\
         \x20   copy_dir_all(1, \"copy\")\n\
         \x20   copy_dir_all(\"tree\", [])\n\
         \x20   copy_dir_all(\"tree\")\n\
         \x20   copy_dir_all(\"tree\", \"copy\", \"extra\")\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0301]"), "{}", run.stderr);
    assert!(run.stderr.contains("error[E0302]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("as the source path for `copy_dir_all`"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("as the target path for `copy_dir_all`"),
        "{}",
        run.stderr
    );
}

#[test]
fn recursive_copy_dynamic_arguments_fail_at_runtime() {
    let run = run_file(
        "copy-dynamic",
        "fn probe(source: Any):\n\
         \x20   copy_dir_all(source, \"copy\")\n\
         \n\
         fn main():\n\
         \x20   probe(1)\n",
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0005]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("as the source path for `copy_dir_all`"),
        "{}",
        run.stderr
    );
}

#[test]
fn recursive_copy_errors_are_fatal_r0015() {
    let directory = io_workspace("copy-errors");
    fs::create_dir_all(directory.join("tree")).expect("create copy tree");
    fs::write(directory.join("tree/a.txt"), "a").expect("write copy file");
    fs::write(directory.join("file.txt"), "x").expect("write copy file source");
    fs::create_dir(directory.join("existing")).expect("create copy target");

    let cases = [
        (
            "copy-missing-source",
            "copy_dir_all(\"missing\", \"copy-missing\")",
            "cannot copy directory",
        ),
        (
            "copy-file-source",
            "copy_dir_all(\"file.txt\", \"copy-file\")",
            "not a directory",
        ),
        (
            "copy-existing-target",
            "copy_dir_all(\"tree\", \"existing\")",
            "already exists",
        ),
        (
            "copy-missing-parent",
            "copy_dir_all(\"tree\", \"missing/copy\")",
            "target parent",
        ),
        (
            "copy-inside-source",
            "copy_dir_all(\"tree\", \"tree/inside\")",
            "inside the source",
        ),
        (
            "copy-empty-target",
            "copy_dir_all(\"tree\", \"\")",
            "must name a new directory",
        ),
        (
            "copy-dot-target",
            "copy_dir_all(\"tree\", \".\")",
            "must name a new directory",
        ),
        (
            "copy-nul-target",
            "copy_dir_all(\"tree\", \"a\0b\")",
            "NUL byte",
        ),
    ];
    for (label, call, needle) in cases {
        let source = directory.join(format!("{label}.vtx"));
        fs::write(&source, format!("fn main():\n    {call}\n")).expect("write copy error source");
        let run = vantyx_in_dir(
            &directory,
            &["run", source.to_str().expect("utf-8 path")],
            None,
        );
        assert_eq!(run.code, 2, "{label} stderr: {}", run.stderr);
        assert!(
            run.stderr.contains("error[R0015]"),
            "{label}: {}",
            run.stderr
        );
        assert!(
            run.stderr.contains(needle),
            "{label}: expected {needle:?} in {}",
            run.stderr
        );
    }
}

#[test]
fn recursive_copy_depth_limit_is_fatal_r0015() {
    let directory = io_workspace("copy-depth");
    let mut deep = directory.join("deep");
    for index in 0..130 {
        deep.push(format!("d{index}"));
    }
    fs::create_dir_all(&deep).expect("create deep copy tree");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n    copy_dir_all(\"deep\", \"copy\")\n",
    )
    .expect("write copy depth source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
    assert!(run.stderr.contains("maximum depth"), "{}", run.stderr);
}

#[cfg(unix)]
#[test]
fn recursive_copy_symlink_rules() {
    use std::os::unix::fs::symlink;

    let directory = io_workspace("copy-links");
    let real = directory.join("real");
    fs::create_dir_all(real.join("sub")).expect("create copy link target");
    fs::write(real.join("sub/data.txt"), "data").expect("write copy link data");
    symlink(&real, directory.join("root-link")).expect("create copy root link");
    fs::create_dir(directory.join("inner-tree")).expect("create inner copy tree");
    symlink(real.join("sub"), directory.join("inner-tree/link")).expect("create copy inner link");
    symlink("missing", directory.join("dangling")).expect("create copy dangling link");

    let source = directory.join("root.vtx");
    fs::write(
        &source,
        "fn main():\n\
         \x20   copy_dir_all(\"root-link\", \"copy-root\")\n\
         \x20   print(read_file(\"copy-root/sub/data.txt\"))\n",
    )
    .expect("write copy root source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "data\n");

    let inner = directory.join("inner-source.vtx");
    fs::write(
        &inner,
        "fn main():\n    copy_dir_all(\"inner-tree\", \"copy-inner\")\n",
    )
    .expect("write copy inner source");
    let run = vantyx_in_dir(
        &directory,
        &["run", inner.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
    assert!(run.stderr.contains("symbolic link"), "{}", run.stderr);

    let dangling = directory.join("dangling-source.vtx");
    fs::write(
        &dangling,
        "fn main():\n    copy_dir_all(\"dangling\", \"copy-dangling\")\n",
    )
    .expect("write copy dangling source");
    let run = vantyx_in_dir(
        &directory,
        &["run", dangling.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    assert!(run.stderr.contains("error[R0015]"), "{}", run.stderr);
}

#[cfg(unix)]
#[test]
fn recursive_copy_preserves_non_utf8_names() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = io_workspace("copy-non-utf8");
    let tree = directory.join("tree");
    fs::create_dir(&tree).expect("create copy tree");
    let name = OsStr::from_bytes(b"bad-\xff");
    fs::write(tree.join(name), "raw").expect("write invalid name");
    let source = directory.join("main.vtx");
    fs::write(
        &source,
        "fn main():\n    copy_dir_all(\"tree\", \"copy\")\n    print(is_dir(\"copy\"))\n",
    )
    .expect("write copy source");
    let run = vantyx_in_dir(
        &directory,
        &["run", source.to_str().expect("utf-8 path")],
        None,
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "true\n");
    assert_eq!(
        fs::read(directory.join("copy").join(name)).expect("read copied raw name"),
        b"raw"
    );
    assert!(tree.join(name).is_file());
}

#[test]
fn try_unwraps_success_and_propagates_failure() {
    let run = run_file(
        "try-basic",
        "enum Outcome:\n    Ok(value: Int)\n    Err(message: Str)\n\n\
         fn parse(n: Int) -> Outcome:\n\
         \x20   if n < 0:\n        return Err(\"negative\")\n\
         \x20   return Ok(n * 2)\n\n\
         fn compute(n: Int) -> Outcome:\n\
         \x20   let value = parse(n)?\n\
         \x20   return Ok(value + 1)\n\n\
         fn main():\n    print(compute(3))\n    print(compute(-1))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "Ok(7)\nErr(\"negative\")\n");
}

#[test]
fn try_assignment_loop_and_bare_statement_work() {
    let run = run_file(
        "try-positions",
        "enum Outcome:\n    Ok(value: Int)\n    Err(code: Int)\n\n\
         fn parse(n: Int) -> Outcome:\n\
         \x20   if n < 0:\n        return Err(100 + n)\n\
         \x20   return Ok(n * 3)\n\n\
         fn accumulate(items: List[Int]) -> Outcome:\n\
         \x20   var total = 0\n\
         \x20   for n in items:\n\
         \x20       let value = parse(n)?\n\
         \x20       total += value\n\
         \x20   return Ok(total)\n\n\
         fn assign(n: Int) -> Outcome:\n\
         \x20   var value = 0\n\
         \x20   value = parse(n)?\n\
         \x20   return Ok(value)\n\n\
         fn discard(n: Int) -> Outcome:\n\
         \x20   parse(n)?\n\
         \x20   return Ok(0)\n\n\
         fn main():\n\
         \x20   print(accumulate([1, 2, 3]))\n\
         \x20   print(accumulate([1, -2, 3]))\n\
         \x20   print(assign(4))\n\
         \x20   print(discard(-5))\n",
    );
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "Ok(18)\nErr(98)\nOk(12)\nErr(95)\n");
}

#[test]
fn try_rejects_nested_and_top_level_use() {
    let run = check_file(
        "try-placement",
        "enum Outcome:\n    Ok(value: Int)\n    Err(message: Str)\n\n\
         fn parse(n: Int) -> Outcome:\n    return Ok(n)\n\n\
         fn nested(n: Int) -> Outcome:\n\
         \x20   let value = 1 + parse(n)?\n\
         \x20   return Ok(value)\n\n\
         let top = parse(1)?\nprint(top)\n",
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("error[E0319]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("only valid as a `let` initializer"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("only valid inside a function"),
        "{}",
        run.stderr
    );
}

#[test]
fn try_requires_two_variants_with_one_field_each() {
    let run = check_file(
        "try-shape",
        "enum Three:\n    A(x: Int)\n    B(x: Int)\n    C(x: Int)\n\n\
         enum Pair:\n    Yes(x: Int)\n    No\n\n\
         fn use_three(n: Int) -> Three:\n\
         \x20   let value = A(n)?\n\
         \x20   return A(value)\n\n\
         fn use_pair(n: Int) -> Pair:\n\
         \x20   let value = Yes(n)?\n\
         \x20   return Yes(value)\n\n\
         fn main():\n    print(1)\n",
    );
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("exactly two variants"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("one payload field on each variant"),
        "{}",
        run.stderr
    );
}

#[test]
fn try_requires_matching_annotated_return_type() {
    let run = check_file(
        "try-return",
        "enum Outcome:\n    Ok(value: Int)\n    Err(message: Str)\n\n\
         fn parse(n: Int) -> Outcome:\n    return Ok(n)\n\n\
         fn wrong(n: Int) -> Int:\n\
         \x20   let value = parse(n)?\n\
         \x20   return value\n\n\
         fn unannotated(n: Int):\n\
         \x20   let value = parse(n)?\n\
         \x20   print(value)\n\n\
         fn main():\n    print(wrong(1))\n",
    );
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .contains("`?` propagates `Outcome`, but `wrong` returns `Int`"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr
            .contains("requires an explicit return type on `unannotated`"),
        "{}",
        run.stderr
    );
}
