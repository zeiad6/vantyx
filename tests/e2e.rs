//! End-to-end tests that exercise the real `vantyx` binary.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
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

fn vantyx_with_env(args: &[&str], stdin: Option<&str>, envs: &[(&str, &str)]) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vantyx"));
    command
        .args(args)
        .env("VANTYX_HISTORY", "off")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    assert!(failed.stderr.contains("lib/broken.vtx"), "{}", failed.stderr);
    assert!(failed.stderr.contains("error[E0301]"), "{}", failed.stderr);
}
