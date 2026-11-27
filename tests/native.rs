//! Differential tests: the interpreter and the native C backend must agree.
//!
//! These tests require a C compiler (`clang` by default, override with
//! `VANTYX_CC`). If no compiler is available they skip rather than fail.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Engines {
    interpreter: Output,
    build: Output,
    native: Option<Output>,
    executable: PathBuf,
}

fn compiler_available() -> bool {
    let compiler = std::env::var("VANTYX_CC").unwrap_or_else(|_| "clang".to_string());
    Command::new(compiler)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn workspace(name: &str, source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "vantyx-native-{}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
        name
    ));
    fs::create_dir_all(&directory).expect("create temp dir");
    fs::write(directory.join("main.vtx"), source).expect("write source");
    directory
}

fn run_engines(name: &str, source: &str) -> Option<Engines> {
    let directory = workspace(name, source);
    run_engines_in(directory, name)
}

fn run_engines_in(directory: PathBuf, name: &str) -> Option<Engines> {
    run_engines_in_mode(directory, name, false)
}

fn run_engines_with_cwd(name: &str, source: &str) -> Option<Engines> {
    let directory = workspace(name, source);
    run_engines_in_mode(directory, name, true)
}

fn run_engines_in_mode(directory: PathBuf, name: &str, run_in_directory: bool) -> Option<Engines> {
    if !compiler_available() {
        eprintln!("skipping native test `{name}`: no C compiler available");
        return None;
    }
    let source_path = directory.join("main.vtx");
    let executable = directory.join("program");

    let mut interpreter_command = Command::new(env!("CARGO_BIN_EXE_vantyx"));
    interpreter_command.arg("run").arg(&source_path);
    if run_in_directory {
        interpreter_command.current_dir(&directory);
    }
    let interpreter = interpreter_command.output().expect("run interpreter");

    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("run native build");

    let native = if build.status.success() {
        let mut native_command = Command::new(&executable);
        if run_in_directory {
            native_command.current_dir(&directory);
        }
        Some(native_command.output().expect("run native executable"))
    } else {
        None
    };

    Some(Engines {
        interpreter,
        build,
        native,
        executable,
    })
}

fn assert_same(name: &str, source: &str) {
    let Some(engines) = run_engines(name, source) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "native build failed for `{name}`:\n{}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines
        .native
        .expect("native output after successful build");
    assert_eq!(
        engines.interpreter.status.code(),
        native.status.code(),
        "exit code differs for `{name}`\ninterpreter stderr:\n{}\nnative stderr:\n{}",
        String::from_utf8_lossy(&engines.interpreter.stderr),
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout),
        "stdout differs for `{name}`"
    );
    let _ = engines.executable;
}

#[test]
fn hello_world_matches() {
    assert_same("hello", "fn main():\n    print(\"Hello, native!\")\n");
}

#[test]
fn arithmetic_matches() {
    assert_same(
        "arithmetic",
        "fn main():\n\
         \x20   print(2 + 3 * 4)\n\
         \x20   print((2 + 3) * 4)\n\
         \x20   print(7 / 2)\n\
         \x20   print(7 // 2)\n\
         \x20   print(-7 // 2)\n\
         \x20   print(7 % 3)\n\
         \x20   print(2 ** 10)\n\
         \x20   print(1.5 + 2.25)\n\
         \x20   print(10.0 / 4.0)\n\
         \x20   print(10.0 % 4.0)\n\
         \x20   print(2.0 ** 3.0)\n\
         \x20   print(-(5))\n\
         \x20   print(not false)\n",
    );
}

#[test]
fn recursion_matches() {
    assert_same(
        "factorial",
        "fn fact(n: Int) -> Int:\n\
         \x20   if n <= 1:\n\
         \x20       return 1\n\
         \x20   return n * fact(n - 1)\n\n\
         fn main():\n\
         \x20   for n in range(11):\n\
         \x20       print(\"{n}! = {fact(n)}\")\n",
    );
}

#[test]
fn control_flow_matches() {
    assert_same(
        "control",
        "fn classify(n: Int) -> Str:\n\
         \x20   if n < 0:\n\
         \x20       return \"negative\"\n\
         \x20   elif n == 0:\n\
         \x20       return \"zero\"\n\
         \x20   else:\n\
         \x20       return \"positive\"\n\n\
         fn main():\n\
         \x20   for n in range(-2, 3):\n\
         \x20       print(classify(n))\n\
         \x20   var i = 0\n\
         \x20   while i < 10:\n\
         \x20       i = i + 1\n\
         \x20       if i % 2 == 0:\n\
         \x20           continue\n\
         \x20       if i > 7:\n\
         \x20           break\n\
         \x20       print(i)\n",
    );
}

#[test]
fn structs_and_nested_display_match() {
    assert_same(
        "structs",
        "struct Point:\n\
         \x20   x: Int\n\
         \x20   y: Int\n\n\
         struct Segment:\n\
         \x20   start: Point\n\
         \x20   end: Point\n\
         \x20   label: Str\n\n\
         fn length_squared(p: Point) -> Int:\n\
         \x20   return p.x * p.x + p.y * p.y\n\n\
         fn main():\n\
         \x20   let a = Point(x=3, y=4)\n\
         \x20   let b = Point(0, 0)\n\
         \x20   b.x = 6\n\
         \x20   b.y = 8\n\
         \x20   let s = Segment(start=a, end=b, label=\"diag\")\n\
         \x20   print(a)\n\
         \x20   print(s)\n\
         \x20   print(\"sum = {a.x + b.y}\")\n\
         \x20   print(length_squared(a))\n\
         \x20   s.start.x = 10\n\
         \x20   print(a)\n",
    );
}

#[test]
fn globals_and_mutation_match() {
    assert_same(
        "globals",
        "var total = 0\n\n\
         fn add(n: Int) -> Unit:\n\
         \x20   total = total + n\n\n\
         fn main():\n\
         \x20   add(3)\n\
         \x20   add(4)\n\
         \x20   print(total)\n\
         \x20   total = total * 2\n\
         \x20   print(total)\n\n\
         main()\n",
    );
}

#[test]
fn functions_use_lexical_globals_matching() {
    // A callee resolves names against its own frame and the program's global
    // scope. Caller locals with the same name must not shadow the global.
    assert_same(
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
}

#[test]
fn evaluation_order_matches() {
    assert_same(
        "order",
        "fn mark(n: Int) -> Int:\n\
         \x20   print(\"eval {n}\")\n\
         \x20   return n\n\n\
         fn main():\n\
         \x20   print(mark(1) + mark(2))\n\
         \x20   print(\"vals {mark(3)} {mark(4)}\")\n",
    );
}

#[test]
fn named_constructor_arguments_evaluate_in_source_order() {
    // Named struct-constructor arguments may be written in any order, but
    // evaluation follows the source text. The native backend used to reorder
    // them into declaration order before emitting the C call.
    assert_same(
        "named-constructor-order",
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
    assert_same(
        "named-constructor-error-order",
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
}

#[test]
fn bool_evaluates_its_argument() {
    // The native backend used to drop a statically non-Bool `bool` argument
    // when emitting the R0007 failure, losing its observable side effects.
    assert_same(
        "bool-argument-evaluation",
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
}

#[test]
fn unit_equality_evaluates_both_operands() {
    // Unit equality is statically true (or false for `!=`), but the language
    // still evaluates both operands left to right. The native backend used to
    // drop a sole effectful operand because it only hoisted when both sides
    // had effects and then rendered a constant.
    assert_same(
        "unit-equality-operands",
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
}

#[test]
fn unit_interpolation_evaluates_expressions() {
    // A Unit renders as `()`, but an interpolated expression is still an
    // expression: the interpreter evaluates it, so native code must too.
    assert_same(
        "unit-interpolation",
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
         \x20   print(b)\n\
         \x20   print(\"pre{multi1()}post\")\n",
    );
}

#[test]
fn conversions_and_strings_match() {
    assert_same(
        "conversions",
        "fn main():\n\
         \x20   print(int(\"42\"), float(\"2.5\"), int(2.0), str(42))\n\
         \x20   print(len(\"héllo\"))\n\
         \x20   print(\"a\" < \"b\", \"a\" == \"a\", \"a\" != \"b\")\n\
         \x20   print(type(1), type(1.0), type(\"x\"), type(true))\n\
         \x20   print(abs(-5), min(2, 3), max(2, 3))\n\
         \x20   print()\n",
    );
}

#[test]
fn float_min_max_ties_match() {
    assert_same(
        "float-min-max-ties",
        "fn main():\n\
         \x20   print(min(-0.0, 0.0), max(-0.0, 0.0), min(0.0, -0.0), max(0.0, -0.0))\n\
         \x20   print(min(-1.0, -0.0), max(-1.0, -0.0), min(-0.0, -1.0), max(-0.0, -1.0))\n\
         \x20   let n = float(\"NaN\")\n\
         \x20   print(min(n, 1.0), max(n, 1.0), min(1.0, n), max(1.0, n))\n",
    );
}

#[test]
fn print_evaluates_arguments_before_writing() {
    assert_same(
        "print-two-phase",
        "fn bump(xs: List[Int]) -> Int:\n\
         \x20   push(xs, 9)\n\
         \x20   return 1\n\n\
         fn main():\n\
         \x20   let xs = [1]\n\
         \x20   print(xs, bump(xs))\n\
         \x20   print(\"partial\", floor(5e300))\n",
    );
}

#[test]
fn type_evaluates_arguments() {
    assert_same(
        "type-side-effects",
        "fn bump(xs: List[Int]) -> Int:\n\
         \x20   push(xs, 2)\n\
         \x20   return len(xs)\n\n\
         fn main():\n\
         \x20   let xs = [1]\n\
         \x20   print(type(bump(xs)), xs)\n\
         \x20   print(type(print(\"inner\")))\n\
         \x20   print(type(assert(true)))\n\
         \x20   print(type(assert(false)))\n",
    );
}

#[test]
fn numeric_string_grammar_matches() {
    // The interpreter trims Unicode whitespace and then applies Rust's strict
    // integer/float grammar. `strtoll`/`strtod` accept extra C forms and
    // reject Unicode whitespace, so the native runtime validates the grammar.
    assert_same(
        "numeric-grammar-accepted",
        "fn main():\n\
         \x20   print(int(\" 12 \"), int(\"\u{a0}12\u{a0}\"), int(\"+7\"), int(\"-0\"))\n\
         \x20   print(float(\"1.\"), float(\".5\"), float(\"1.e3\"), float(\".5e2\"))\n\
         \x20   print(float(\"1e999\"), float(\"-1e999\"), float(\"1e-999\"))\n\
         \x20   print(float(\"inf\"), float(\"-Infinity\"), float(\"NaN\"), float(\"+nan\"))\n",
    );
    for (name, source) in [
        ("hex-float", "fn main():\n    print(float(\"0x1p3\"))\n"),
        ("nan-payload", "fn main():\n    print(float(\"nan(1)\"))\n"),
        ("trailing-garbage", "fn main():\n    print(int(\"12x\"))\n"),
        ("underscore", "fn main():\n    print(int(\"1_0\"))\n"),
        (
            "int-overflow",
            "fn main():\n    print(int(\"9223372036854775808\"))\n",
        ),
    ] {
        let Some(engines) = run_engines(name, source) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "native build failed for `{name}`:\n{}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        assert_eq!(
            engines.interpreter.status.code(),
            Some(2),
            "interpreter accepted `{name}`"
        );
        let native = engines
            .native
            .expect("native output after successful build");
        assert_eq!(native.status.code(), Some(2), "native accepted `{name}`");
        assert!(
            String::from_utf8_lossy(&engines.interpreter.stderr).contains("error[R0007]"),
            "interpreter diagnostic missing for `{name}`"
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains("error[R0007]"),
            "native diagnostic missing for `{name}`"
        );
    }
}

#[test]
fn range_loops_match() {
    assert_same(
        "ranges",
        "fn main():\n\
         \x20   for i in range(3):\n\
         \x20       print(i)\n\
         \x20   for i in range(1, 4):\n\
         \x20       print(i)\n\
         \x20   for i in range(5, 0, -2):\n\
         \x20       print(i)\n\
         \x20   for i in range(10):\n\
         \x20       if i == 2:\n\
         \x20           continue\n\
         \x20       if i == 5:\n\
         \x20           break\n\
         \x20       print(i)\n\
         \x20   for i in range(2, 2):\n\
         \x20       print(i)\n",
    );
}

#[test]
fn range_values_match() {
    assert_same(
        "range-values",
        "fn mark(label: Str, value: Int) -> Int:\n\
         \x20   print(label)\n\
         \x20   return value\n\
         fn main():\n\
         \x20   print(range(5))\n\
         \x20   print(range(2, 5))\n\
         \x20   print(range(5, 0, -2))\n\
         \x20   print(range(0))\n\
         \x20   print(range(2, 2))\n\
         \x20   print(len(range(7)))\n\
         \x20   let xs = range(4)\n\
         \x20   print(xs[2])\n\
         \x20   print(xs)\n\
         \x20   print(range(5)[2])\n\
         \x20   print(range(1 + 1, 2 * 3, 2))\n\
         \x20   print(range(mark(\"a\", 1), mark(\"b\", 5), mark(\"c\", 2)))\n",
    );
}

#[test]
fn top_level_code_matches() {
    assert_same(
        "top-level",
        "let greeting = \"hi\"\n\
         for n in range(3):\n\
         \x20   print(\"{greeting} {n}\")\n",
    );
}

#[test]
fn runtime_division_by_zero_exits_two() {
    let Some(engines) = run_engines("division-error", "fn main():\n    print(1 // 0)\n") else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&native.stderr);
    assert!(stderr.contains("error[R0001]"), "{stderr}");
    assert_eq!(engines.interpreter.status.code(), Some(2));
}

#[test]
fn runtime_overflow_exits_two() {
    let Some(engines) = run_engines(
        "overflow-error",
        "fn main():\n    print(9223372036854775807 + 1)\n",
    ) else {
        return;
    };
    assert!(engines.build.status.success());
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&native.stderr).contains("error[R0002]"));
}

#[test]
fn runtime_assert_and_conversion_errors_exit_two() {
    let Some(engines) = run_engines("assert-error", "fn main():\n    assert(false)\n") else {
        return;
    };
    assert!(engines.build.status.success());
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&native.stderr).contains("error[R0006]"));

    let Some(engines) = run_engines("conversion-error", "fn main():\n    print(int(2.5))\n") else {
        return;
    };
    assert!(engines.build.status.success());
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&native.stderr).contains("error[R0007]"));
}

#[test]
fn uninitialized_global_read_exits_two() {
    let Some(engines) = run_engines(
        "uninitialized-global",
        "var x = later()\n\n\
         fn later() -> Int:\n\
         \x20   return x\n\n\
         fn main():\n\
         \x20   print(x)\n\n\
         main()\n",
    ) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&native.stderr).contains("error[R0009]"));
    assert_eq!(engines.interpreter.status.code(), Some(2));
}

#[test]
fn uninitialized_global_write_exits_two() {
    // The native backend used to initialize a global through an ordinary
    // assignment even when its declaration statement had not run yet. The
    // interpreter raises R0009, and side effects in the right side still
    // happen before the failure because the value is evaluated first.
    let source = "fn setg() -> Int:\n\
                  \x20   g = note()\n\
                  \x20   return 0\n\n\
                  fn note() -> Int:\n\
                  \x20   print(\"rhs\")\n\
                  \x20   return 5\n\n\
                  print(setg())\n\
                  var g = 1\n\
                  print(g)\n";
    let Some(engines) = run_engines("uninitialized-global-write", source) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        "rhs\n"
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), "rhs\n");
    assert!(String::from_utf8_lossy(&native.stderr).contains("error[R0009]"));

    // A global declaration whose own initializer calls a function that writes
    // the same global must fail too, rather than letting the declaration win.
    let self_init = "fn setg() -> Int:\n\
                     \x20   g = 5\n\
                     \x20   return 0\n\n\
                     var g = setg()\n\
                     print(g)\n";
    let Some(engines) = run_engines("uninitialized-global-self-init", self_init) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    assert!(engines.interpreter.stdout.is_empty());
    assert!(native.stdout.is_empty());

    assert_same(
        "global-write-after-declaration",
        "fn setg() -> Int:\n\
         \x20   g = 5\n\
         \x20   return 0\n\n\
         var g = 1\n\
         print(setg())\n\
         print(g)\n",
    );
}

#[test]
fn list_literals_and_indexing_match() {
    assert_same(
        "list-literals",
        "fn main():\n\
         \x20   let xs = [1, 2, 3]\n\
         \x20   print(xs)\n\
         \x20   print(len(xs))\n\
         \x20   print(xs[0], xs[2])\n\
         \x20   xs[1] = 20\n\
         \x20   print(xs)\n\
         \x20   print(type(xs))\n\
         \x20   let empty: List[Str] = []\n\
         \x20   print(empty, len(empty))\n",
    );
}

#[test]
fn list_mutation_and_aliasing_match() {
    assert_same(
        "list-aliasing",
        "fn main():\n\
         \x20   let a = [1, 2]\n\
         \x20   let b = a\n\
         \x20   push(b, 3)\n\
         \x20   b[0] = 10\n\
         \x20   print(a)\n\
         \x20   print(b)\n\
         \x20   print(len(a))\n",
    );
}

#[test]
fn list_builtins_match() {
    assert_same(
        "list-builtins",
        "fn main():\n\
         \x20   let xs: List[Int] = []\n\
         \x20   push(xs, 1)\n\
         \x20   push(xs, 2)\n\
         \x20   print(xs, len(xs), contains(xs, 1), contains(xs, 9))\n\
         \x20   print(pop(xs))\n\
         \x20   print(xs)\n\
         \x20   print(contains(\"hello\", \"ell\"), contains(\"hello\", \"xyz\"))\n",
    );
}

#[test]
fn float_display_matches() {
    assert_same(
        "float-display",
        "fn main():\n\
         \x20   print(0.1 + 0.2)\n\
         \x20   print(1.0 / 3.0)\n\
         \x20   print(1.5, 2.25, 1.0, -0.0)\n\
         \x20   print(1e20)\n\
         \x20   print(1e-10)\n\
         \x20   print(5e-324)\n\
         \x20   print(123456789.123456789)\n\
         \x20   print([0.1 + 0.2, 1.0 / 3.0, 1e20])\n",
    );
}

#[test]
fn list_element_types_match() {
    assert_same(
        "list-element-types",
        "fn main():\n\
         \x20   print([true, false], contains([true], true))\n\
         \x20   print([1.5, 2.25])\n\
         \x20   print([\"a\", \"b\"], contains([\"a\"], \"a\"))\n\
         \x20   let nested: List[List[Str]] = [[\"x\"], []]\n\
         \x20   print(nested)\n",
    );
}

#[test]
fn recursive_struct_list_display_matches() {
    assert_same(
        "recursive-struct-list",
        "struct Node:\n\
         \x20   name: Str\n\
         \x20   children: List[Node]\n\n\
         fn main():\n\
         \x20   let n = Node(name=\"root\", children=[])\n\
         \x20   push(n.children, n)\n\
         \x20   print(n)\n",
    );
}

#[test]
fn list_iteration_and_mutation_match() {
    assert_same(
        "list-iteration",
        "fn main():\n\
         \x20   let xs = [1, 2, 3]\n\
         \x20   for x in xs:\n\
         \x20       print(x)\n\
         \x20       if x == 1:\n\
         \x20           push(xs, 4)\n\
         \x20       if x == 2:\n\
         \x20           continue\n\
         \x20       if x == 3:\n\
         \x20           break\n\
         \x20   print(xs)\n\
         \x20   print(len(xs))\n",
    );
}

#[test]
fn nested_lists_match() {
    assert_same(
        "nested-lists",
        "fn main():\n\
         \x20   let grid: List[List[Int]] = [[1, 2], [3]]\n\
         \x20   print(grid)\n\
         \x20   print(grid[1][0])\n\
         \x20   grid[1][0] = 30\n\
         \x20   print(grid)\n\
         \x20   print(contains(grid, [2, 3]))\n\
         \x20   print(contains(grid, [2]))\n",
    );
}

#[test]
fn lists_of_structs_match() {
    assert_same(
        "list-of-structs",
        "struct Box:\n\
         \x20   values: List[Int]\n\n\
         fn main():\n\
         \x20   let a = Box(values=[1, 2])\n\
         \x20   let b = Box(values=[1, 2])\n\
         \x20   push(a.values, 3)\n\
         \x20   print(contains([a, b], Box(values=[1, 2, 3])))\n\
         \x20   print(contains([a, b], Box(values=[9])))\n\
         \x20   let p = [a, b]\n\
         \x20   print(p)\n\
         \x20   let alias = a\n\
         \x20   push(alias.values, 4)\n\
         \x20   print(a)\n",
    );
}

#[test]
fn list_fields_and_functions_match() {
    assert_same(
        "list-fields",
        "struct Bag:\n\
         \x20   items: List[Int]\n\
         \x20   label: Str\n\n\
         fn total(xs: List[Int]) -> Int:\n\
         \x20   var sum = 0\n\
         \x20   for x in xs:\n\
         \x20       sum = sum + x\n\
         \x20   return sum\n\n\
         fn make() -> List[Int]:\n\
         \x20   return [1, 2, 3]\n\n\
         fn main():\n\
         \x20   let b = Bag(items=make(), label=\"nums\")\n\
         \x20   push(b.items, 4)\n\
         \x20   print(b)\n\
         \x20   print(total(b.items))\n\
         \x20   print(total(make()))\n",
    );
}

#[test]
fn list_index_evaluation_order_matches() {
    assert_same(
        "list-index-order",
        "fn mark(what: Str, n: Int) -> Int:\n\
         \x20   print(what)\n\
         \x20   return n\n\n\
         fn main():\n\
         \x20   let xs = [1, 2]\n\
         \x20   xs[mark(\"index\", 1)] = mark(\"value\", 9)\n\
         \x20   print(xs)\n\
         \x20   xs[mark(\"read-index\", 0)] = xs[mark(\"read-value\", 1)] + 1\n\
         \x20   print(xs)\n",
    );
}

#[test]
fn list_bounds_error_exits_two() {
    let Some(engines) = run_engines(
        "list-bounds",
        "fn main():\n\
         \x20   let xs = [1, 2]\n\
         \x20   print(\"before\")\n\
         \x20   print(xs[5])\n",
    ) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&native.stderr).contains("error[R0003]"),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
}

#[test]
fn list_negative_index_exits_two() {
    let Some(engines) = run_engines(
        "list-negative-index",
        "fn main():\n\
         \x20   let xs = [1, 2]\n\
         \x20   print(xs[-1])\n",
    ) else {
        return;
    };
    assert!(engines.build.status.success());
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&native.stderr).contains("error[R0003]"),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(engines.interpreter.status.code(), Some(2));
}

#[test]
fn pop_empty_list_exits_two() {
    let Some(engines) = run_engines(
        "list-pop-empty",
        "fn main():\n\
         \x20   let xs: List[Int] = []\n\
         \x20   print(pop(xs))\n",
    ) else {
        return;
    };
    assert!(engines.build.status.success());
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&native.stderr).contains("error[R0003]"),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(engines.interpreter.status.code(), Some(2));
}

#[test]
fn untyped_empty_list_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "empty-list-unsupported",
        "fn main():\n    let xs = []\n    print(xs)\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("Any"), "{stderr}");
}

#[test]
fn list_of_any_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "list-any-unsupported",
        "fn main():\n    let xs: List[Any] = [1, \"two\"]\n    print(xs)\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("Any"), "{stderr}");
}

#[test]
fn dynamic_any_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "any-unsupported",
        "fn identity(x: Any) -> Any:\n\
         \x20   return x\n\n\
         fn main():\n\
         \x20   print(identity(1))\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("Any"), "{stderr}");
}

#[test]
fn value_return_without_annotation_is_rejected() {
    let Some(engines) = run_engines(
        "unannotated-return",
        "fn answer():\n\
         \x20   return 42\n\n\
         fn main():\n\
         \x20   print(answer())\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("return type annotation"), "{stderr}");
}

#[test]
fn string_indexing_matches() {
    assert_same(
        "string-index",
        "fn text(s: Str) -> Str:\n\
         \x20   print(\"base\")\n\
         \x20   return s\n\n\
         fn index(n: Int) -> Int:\n\
         \x20   print(\"index\")\n\
         \x20   return n\n\n\
         fn main():\n\
         \x20   let s = \"hello\"\n\
         \x20   print(s[0], s[4])\n\
         \x20   print(len(s), type(s[0]))\n\
         \x20   print(text(\"abc\")[index(1)])\n\
         \x20   var i = 0\n\
         \x20   while i < len(s):\n\
         \x20       print(s[i])\n\
         \x20       i = i + 1\n",
    );
}

#[test]
fn string_unicode_indexing_and_iteration_match() {
    assert_same(
        "string-unicode",
        "fn first(s: Str) -> Str:\n\
         \x20   return s[0]\n\n\
         fn main():\n\
         \x20   let s = \"héllo wörld 🚀\"\n\
         \x20   print(len(s))\n\
         \x20   print(s[0], s[1], s[2])\n\
         \x20   print(s[6], s[7])\n\
         \x20   print(s[12])\n\
         \x20   print(first(\"é\"))\n\
         \x20   var out = \"\"\n\
         \x20   for c in s:\n\
         \x20       out = out + c + \".\"\n\
         \x20   print(out)\n",
    );
}

#[test]
fn string_ascii_indexing_iteration_and_concat_match() {
    assert_same(
        "string-ascii-cache",
        "fn main():\n\
         \x20   let s = \"abcXYZ019 !?,.-+\"\n\
         \x20   print(len(s))\n\
         \x20   var out = \"\"\n\
         \x20   for c in s:\n\
         \x20       out = out + c + \"|\"\n\
         \x20   print(out)\n\
         \x20   print(s[0], s[5], s[15])\n\
         \x20   var reversed = \"\"\n\
         \x20   var i = len(s) - 1\n\
         \x20   while i >= 0:\n\
         \x20       reversed = reversed + s[i]\n\
         \x20       i = i - 1\n\
         \x20   print(reversed)\n",
    );
}

#[test]
fn string_iteration_control_flow_matches() {
    assert_same(
        "string-iteration",
        "fn find(s: Str, needle: Str) -> Int:\n\
         \x20   var i = 0\n\
         \x20   for c in s:\n\
         \x20       if c == needle:\n\
         \x20           return i\n\
         \x20       i = i + 1\n\
         \x20   return -1\n\n\
         fn main():\n\
         \x20   for c in \"abc\":\n\
         \x20       print(c)\n\
         \x20       if c == \"b\":\n\
         \x20           continue\n\
         \x20       if c == \"c\":\n\
         \x20           break\n\
         \x20   print(\"done\")\n\
         \x20   print(find(\"héllo\", \"l\"), find(\"héllo\", \"z\"))\n",
    );
}

#[test]
fn string_bounds_and_negative_index_exit_two() {
    for (name, source) in [
        (
            "string-bounds",
            "fn main():\n\
             \x20   let s = \"abc\"\n\
             \x20   print(\"before\")\n\
             \x20   print(s[3])\n",
        ),
        (
            "string-negative-index",
            "fn main():\n\
             \x20   let s = \"abc\"\n\
             \x20   print(s[-1])\n",
        ),
    ] {
        let Some(engines) = run_engines(name, source) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(native.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&native.stderr).contains("error[R0003]"),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(engines.interpreter.status.code(), Some(2));
        assert_eq!(
            String::from_utf8_lossy(&engines.interpreter.stdout),
            String::from_utf8_lossy(&native.stdout)
        );
    }
}

#[test]
fn native_nul_in_string_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "nul-string-unsupported",
        "fn main():\n    let s = \"a\0b\"\n    print(s)\n",
    ) else {
        return;
    };
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("NUL"), "{stderr}");
}

#[test]
fn native_call_depth_limit_exits_two() {
    let Some(engines) = run_engines(
        "native-recursion-limit",
        "fn loop(n: Int) -> Int:\n\
         \x20   return loop(n + 1)\n\n\
         fn main():\n\
         \x20   print(loop(0))\n",
    ) else {
        return;
    };
    assert!(engines.build.status.success());
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&native.stderr).contains("error[R0012]"),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
}

#[test]
fn map_literals_and_builtins_match() {
    assert_same(
        "map-basics",
        "fn main():\n\
         \x20   let scores = {\"alice\": 3, \"bob\": 5}\n\
         \x20   print(scores)\n\
         \x20   print(scores[\"alice\"])\n\
         \x20   print(len(scores))\n\
         \x20   print(contains(scores, \"bob\"))\n\
         \x20   print(contains(scores, \"carol\"))\n\
         \x20   print(keys(scores))\n\
         \x20   print(values(scores))\n\
         \x20   let ids = {2: \"two\", 1: \"one\"}\n\
         \x20   print(ids[1], contains(ids, 2), keys(ids), values(ids))\n",
    );
}

#[test]
fn map_mutation_and_iteration_match() {
    assert_same(
        "map-mutation",
        "fn main():\n\
         \x20   var counts = {\"a\": 1, \"b\": 2}\n\
         \x20   counts[\"c\"] = 3\n\
         \x20   counts[\"a\"] = 10\n\
         \x20   counts[\"b\"] += 5\n\
         \x20   for key in counts:\n\
         \x20       print(\"{key}={counts[key]}\")\n\
         \x20   print(counts)\n\
         \x20   let alias = counts\n\
         \x20   alias[\"d\"] = 4\n\
         \x20   print(counts)\n\
         \x20   print(alias)\n",
    );
}

#[test]
fn large_scalar_maps_match() {
    assert_same(
        "map-large",
        "fn main():\n\
         \x20   var numbers: Map[Int, Int] = {}\n\
         \x20   var i = 0\n\
         \x20   while i < 5000:\n\
         \x20       numbers[i] = i * 3\n\
         \x20       i = i + 1\n\
         \x20   var total = 0\n\
         \x20   i = 0\n\
         \x20   while i < 5000:\n\
         \x20       total = total + numbers[i]\n\
         \x20       i = i + 1\n\
         \x20   numbers[2500] = 1\n\
         \x20   print(len(numbers), total, numbers[2500])\n\
         \x20   print(keys(numbers)[0], keys(numbers)[2500], keys(numbers)[4999])\n\
         \x20   var words: Map[Str, Int] = {}\n\
         \x20   var j = 0\n\
         \x20   while j < 1000:\n\
         \x20       words[\"k{j}\"] = j\n\
         \x20       j = j + 1\n\
         \x20   print(len(words), words[\"k0\"], words[\"k999\"], contains(words, \"missing\"))\n\
         \x20   var floats: Map[Float, Str] = {1.5: \"a\", 2.5: \"b\"}\n\
         \x20   print(floats[1.5], contains(floats, 2.5), len(floats))\n\
         \x20   var flags = {true: 1, false: 2}\n\
         \x20   print(flags, flags[true], flags[false], keys(flags), values(flags))\n\
         \x20   var zeros = {-0.0: \"neg\", 0.0: \"pos\"}\n\
         \x20   print(zeros, len(zeros), zeros[0.0], contains(zeros, -0.0))\n",
    );
}

#[test]
fn map_of_struct_keys_matches() {
    assert_same(
        "map-struct-keys",
        "struct Point:\n\
         \x20   x: Int\n\
         \x20   y: Int\n\n\
         fn main():\n\
         \x20   let labels = {Point(1, 2): \"a\", Point(3, 4): \"b\"}\n\
         \x20   print(labels[Point(1, 2)])\n\
         \x20   print(contains(labels, Point(3, 4)))\n\
         \x20   print(contains(labels, Point(9, 9)))\n\
         \x20   print(keys(labels))\n\
         \x20   print(values(labels))\n\
         \x20   print(labels)\n",
    );
}

#[test]
fn nested_map_equality_matches() {
    assert_same(
        "map-equality",
        "fn main():\n\
         \x20   let grid = {\"a\": {\"x\": 1}, \"b\": {\"x\": 2}}\n\
         \x20   let same = {\"b\": {\"x\": 2}, \"a\": {\"x\": 1}}\n\
         \x20   let other = {\"a\": {\"x\": 1}, \"b\": {\"x\": 3}}\n\
         \x20   print(grid == same)\n\
         \x20   print(grid != same)\n\
         \x20   print(grid == other)\n\
         \x20   print(grid[\"a\"][\"x\"])\n\
         \x20   print(grid)\n",
    );
}

#[test]
fn map_missing_key_exits_two() {
    let Some(engines) = run_engines(
        "map-missing-key",
        "fn main():\n\
         \x20   let scores = {\"a\": 1}\n\
         \x20   print(\"before\")\n\
         \x20   print(scores[\"b\"])\n",
    ) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&native.stderr).contains("error[R0008]"),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
}

#[test]
fn annotated_empty_map_supports_insertion_and_loops() {
    assert_same(
        "map-empty",
        "fn main():\n\
         \x20   var m: Map[Str, Int] = {}\n\
         \x20   m[\"a\"] = 1\n\
         \x20   m[\"b\"] = 2\n\
         \x20   print(m, len(m))\n\
         \x20   for k in keys(m):\n\
         \x20       print(\"{k}:{m[k]}\")\n\
         \x20   for v in values(m):\n\
         \x20       print(v)\n",
    );
}

#[test]
fn map_of_lists_and_struct_fields_match() {
    assert_same(
        "map-lists",
        "struct Bag:\n\
         \x20   items: Map[Str, List[Int]]\n\
         \x20   name: Str\n\n\
         fn main():\n\
         \x20   let b = Bag(items={\"a\": [1, 2], \"b\": [3]}, name=\"bag\")\n\
         \x20   print(b)\n\
         \x20   print(b.items[\"a\"])\n\
         \x20   print(keys(b.items))\n\
         \x20   print(values(b.items))\n\
         \x20   print(b == Bag(items={\"b\": [3], \"a\": [1, 2]}, name=\"bag\"))\n",
    );
}

#[test]
fn map_evaluation_order_matches() {
    assert_same(
        "map-order",
        "fn key() -> Str:\n\
         \x20   print(\"key\")\n\
         \x20   return \"a\"\n\n\
         fn value() -> Int:\n\
         \x20   print(\"value\")\n\
         \x20   return 9\n\n\
         fn main():\n\
         \x20   var m = {\"a\": 1}\n\
         \x20   m[key()] = value()\n\
         \x20   print(m)\n\
         \x20   m[key()] += 3\n\
         \x20   print(m)\n",
    );
}

#[test]
fn struct_equality_matches() {
    assert_same(
        "struct-equality",
        "struct Point:\n\
         \x20   x: Int\n\
         \x20   y: Int\n\n\
         struct Labeled:\n\
         \x20   point: Point\n\
         \x20   label: Str\n\n\
         fn main():\n\
         \x20   let a = Point(1, 2)\n\
         \x20   let b = Point(1, 2)\n\
         \x20   let c = Point(1, 3)\n\
         \x20   print(a == b, a != b)\n\
         \x20   print(a == c, a != c)\n\
         \x20   print(Labeled(a, \"x\") == Labeled(b, \"x\"))\n\
         \x20   print(Labeled(a, \"x\") == Labeled(b, \"y\"))\n",
    );
}

#[test]
fn map_function_and_global_boundaries_match() {
    assert_same(
        "map-boundaries",
        "var registry: Map[Str, Int] = {\"start\": 1}\n\n\
         fn add(m: Map[Str, Int], k: Str, v: Int):\n\
         \x20   m[k] = v\n\n\
         fn lookup(m: Map[Str, Int], k: Str) -> Int:\n\
         \x20   return m[k]\n\n\
         fn make() -> Map[Str, Int]:\n\
         \x20   return {\"made\": 2}\n\n\
         fn main():\n\
         \x20   add(registry, \"next\", 2)\n\
         \x20   print(registry)\n\
         \x20   print(lookup(registry, \"next\"))\n\
         \x20   let m = make()\n\
         \x20   print(m, lookup(m, \"made\"))\n",
    );
}

#[test]
fn map_nested_assignment_matches() {
    assert_same(
        "map-nested-assign",
        "struct Point:\n\
         \x20   x: Int\n\
         \x20   y: Int\n\n\
         fn main():\n\
         \x20   var grid = {\"row\": [1, 2]}\n\
         \x20   grid[\"row\"][0] = 9\n\
         \x20   grid[\"row\"][1] += 3\n\
         \x20   print(grid)\n\
         \x20   var points = {\"p\": Point(1, 2)}\n\
         \x20   points[\"p\"].x = 5\n\
         \x20   points[\"p\"].y += 4\n\
         \x20   print(points)\n",
    );
}

#[test]
fn cyclic_structural_equality_matches() {
    assert_same(
        "cyclic-equality",
        "struct Node:\n\
         \x20   next: Map[Str, Node]\n\n\
         struct Ring:\n\
         \x20   items: List[Ring]\n\n\
         fn main():\n\
         \x20   var m1: Map[Str, Node] = {}\n\
         \x20   var m2: Map[Str, Node] = {}\n\
         \x20   m1[\"self\"] = Node(next=m1)\n\
         \x20   m2[\"self\"] = Node(next=m2)\n\
         \x20   print(m1 == m2)\n\
         \x20   var a: List[Ring] = []\n\
         \x20   var b: List[Ring] = []\n\
         \x20   push(a, Ring(items=a))\n\
         \x20   push(b, Ring(items=b))\n\
         \x20   print(a == b)\n\
         \x20   print(m1)\n\
         \x20   print(a)\n",
    );
}

#[test]
fn untyped_empty_map_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "empty-map-unsupported",
        "fn main():\n    let scores = {}\n    print(scores)\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("Any"), "{stderr}");
}

#[test]
fn map_of_any_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "map-any-unsupported",
        "fn main():\n    let scores: Map[Str, Any] = {\"a\": 1}\n    print(scores)\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("Any"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn generated_c_temp_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    if !compiler_available() {
        eprintln!("skipping temp-file test: no C compiler available");
        return;
    }
    let directory = workspace("temp-permissions", "fn main():\n    print(1)\n");
    let source = directory.join("main.vtx");
    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(directory.join("program"))
        .env("VANTYX_CC", "/bin/false")
        .output()
        .expect("run failing native build");
    assert_eq!(build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&build.stderr);
    let retained = stderr
        .lines()
        .find_map(|line| line.split("retained at ").nth(1))
        .expect("retained C path in stderr")
        .trim()
        .to_string();
    let metadata = fs::metadata(&retained).expect("retained C file exists");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    let _ = fs::remove_file(&retained);
}

#[test]
fn default_output_path_is_the_source_stem() {
    if !compiler_available() {
        eprintln!("skipping default-output test: no C compiler available");
        return;
    }
    let directory = workspace("default-output", "print(\"default\")\n");
    let source = directory.join("main.vtx");
    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(&source)
        .output()
        .expect("build with default output");
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let executable = directory.join("main");
    assert!(executable.is_file(), "expected {}", executable.display());
    let output = Command::new(&executable)
        .output()
        .expect("run default output");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "default\n");
}

#[test]
fn enums_and_match_statements_match() {
    assert_same(
        "enum-statements",
        r#"enum Token:
    Number(value: Int)
    Word(text: Str)
    End

fn describe(token: Token) -> Str:
    match token:
        Number(value):
            return "number {value}"
        Word(text):
            return "word {text}"
        End:
            return "end"

fn main():
    print(describe(Number(42)))
    print(describe(Word("hi")))
    print(describe(End()))
    print(Number(7))
    print(Word("hello"))
    print(End())
    print(type(Number(1)))
    print(Number(1) == Number(1))
    print(Number(1) == Number(2))
    print(Number(1) == Word("hi"))
"#,
    );
}

#[test]
fn match_expressions_match() {
    assert_same(
        "enum-match-expressions",
        r#"enum Color:
    Red
    Green
    Blue

fn score(color: Color) -> Int:
    return match color:
        Red: 1
        Green: 2
        Blue: 3

fn main():
    let colors: List[Color] = [Red(), Green(), Blue()]
    for color in colors:
        print(score(color))
        print(color)
"#,
    );
}

#[test]
fn scalar_literal_match_patterns_match() {
    assert_same(
        "scalar-literal-patterns",
        r#"fn classify(n: Int) -> Str:
    return match n:
        0: "zero"
        1: "one"
        -1: "minus one"
        _: "many"

fn truth(b: Bool) -> Int:
    return match b:
        true: 1
        false: 0

fn code(s: Str) -> Int:
    return match s:
        "alpha": 1
        "beta": 2
        _: 0

fn main():
    for n in [-2, -1, 0, 1, 2]:
        print(classify(n))
    print(truth(true))
    print(truth(false))
    print(code("alpha"))
    print(code("beta"))
    print(code("gamma"))
    let three: Str = match 3:
        3: "three"
        _: "other"
    print(three)
"#,
    );
}

#[test]
fn match_guards_match() {
    assert_same(
        "match-guards",
        r#"enum Shape:
    Circle(radius: Int)
    Rect(width: Int, height: Int)

var checks: Int = 0

fn positive(n: Int) -> Bool:
    checks = checks + 1
    return n > 0

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
        _ if positive(n): "positive"
        _: "negative"

fn score(shape: Shape) -> Int:
    return match shape:
        Circle(radius) if radius > 10: 100
        Circle(radius): radius
        Rect(width, height) if width == height: width * height
        Rect(width, height): width + height

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
    print(score(Circle(20)))
    print(score(Circle(5)))
    print(score(Rect(3, 3)))
    print(score(Rect(2, 5)))
    print(greeting("admin", true))
    print(greeting("admin", false))
    print(greeting("bob", true))
    print(checks)

main()
"#,
    );
}

#[test]
fn conditions_with_calls_match() {
    // Regression: `if`/`elif`/`while` conditions that lower to a C statement
    // expression (any call or short-circuit expression) were emitted without
    // the extra parentheses the `if` grammar requires, so the generated C did
    // not compile.
    assert_same(
        "conditions-with-calls",
        r#"var calls: Int = 0

fn ready() -> Bool:
    calls += 1
    return calls <= 2

fn label() -> Str:
    calls += 1
    return "ready"

fn main():
    if ready():
        print("if")
    if calls == 1:
        print("once")
    if false:
        print("no")
    elif ready():
        print("elif")
    print(calls)
    var i = 0
    while ready() and i < 10:
        i += 1
    print(i)
    print(calls)
    if label() == "ready":
        print("label")
    print(calls)

main()
"#,
    );
}

#[test]
fn short_circuit_operands_match() {
    // Regression: the native backend hoisted both operands of `and`/`or` into
    // temporaries, evaluating the right operand even when the left operand
    // decided the result. That silently changed observable side effects.
    assert_same(
        "short-circuit-operands",
        r#"var calls: Int = 0

fn yes() -> Bool:
    calls += 1
    return true

fn no() -> Bool:
    calls += 1
    return false

fn main():
    print(false and yes())
    print(calls)
    print(true or yes())
    print(calls)
    print(true and no())
    print(calls)
    print(false or no())
    print(calls)
    print((no() and yes()) or yes())
    print(calls)
    if false and yes():
        print("bad")
    if true or yes():
        print("good")
    print(calls)
    let verdict = match 1:
        1 if false and yes(): "bad"
        _: "ok"
    print(verdict)
    print(calls)

main()
"#,
    );
}

#[test]
fn dead_code_after_return_matches() {
    assert_same(
        "dead-code-after-return",
        r#"fn classify(n: Int) -> Int:
    if n > 0:
        return 1
    else:
        return 2
    print("dead")

fn main():
    print(classify(1))
    print(classify(-1))
"#,
    );
}

#[test]
fn effectful_binary_operands_keep_left_to_right_order() {
    assert_same(
        "effectful-binary-order",
        r#"var log: Str = ""

fn n(x: Int) -> Int:
    log = log + str(x)
    return x

fn main():
    print(n(1) * 10 + n(2))
    print(log)
    print(n(3) < n(4))
    print(log)

main()
"#,
    );
}

#[test]
fn enums_in_structs_and_structs_in_payloads_match() {
    assert_same(
        "enum-struct-payloads",
        r#"struct Coord:
    x: Int
    y: Int

enum Inner:
    A
    B

enum Payload:
    At(value: Coord)
    Items(values: List[Int])
    Lookup(entries: Map[Str, Int])
    Nested(inner: Inner)

struct Holder:
    token: Inner
    many: List[Inner]
    table: Map[Str, Inner]

fn main():
    print(At(Coord(1, 2)))
    print(Items([1, 2, 3]))
    print(Lookup({"a": 1}))
    print(Nested(B()))
    print(At(Coord(1, 2)) == At(Coord(1, 2)))
    print(Items([1, 2]) == Items([1, 2]))
    print(Lookup({"a": 1}) == Lookup({"a": 1}))
    let holder = Holder(A(), [A(), B()], {"x": B()})
    print(holder)
    print(holder.many[1])
    print(holder.table["x"])
"#,
    );
}

#[test]
fn mutually_recursive_structs_and_enums_match() {
    assert_same(
        "enum-mutual-recursion",
        r#"struct Holder:
    flag: Flag
    flags: List[Flag]

enum Flag:
    On
    Off(value: Holder)

fn main():
    let holder = Holder(On(), [On(), Off(Holder(On(), []))])
    print(holder)
    print(holder.flag)
    print(holder.flags[1])
    print(holder == Holder(On(), [On(), Off(Holder(On(), []))]))
"#,
    );
}

#[test]
fn recursive_enums_match() {
    assert_same(
        "enum-recursive",
        r#"enum Tree:
    Leaf(value: Int)
    Node(left: Tree, right: Tree)

fn total(tree: Tree) -> Int:
    match tree:
        Leaf(value):
            return value
        Node(left, right):
            return total(left) + total(right)

fn main():
    let tree = Node(Leaf(1), Node(Leaf(2), Leaf(3)))
    print(total(tree))
    print(tree)
    print(tree == Node(Leaf(1), Node(Leaf(2), Leaf(3))))
    print(tree == Node(Leaf(1), Node(Leaf(2), Leaf(4))))
"#,
    );
}

#[test]
fn deep_enum_display_and_equality_match() {
    assert_same(
        "enum-deep",
        r#"enum Chain:
    End
    Link(next: Chain)

fn make(n: Int) -> Chain:
    if n == 0:
        return End()
    return Link(make(n - 1))

fn main():
    print(make(5))
    print(make(70))
    print(make(70) == make(70))
    print(make(70) == make(69))
"#,
    );
}

#[test]
fn enum_collections_and_map_keys_match() {
    assert_same(
        "enum-collections",
        r#"enum Key:
    Alpha
    Beta
    Gamma

enum Mixed:
    Nothing
    One(value: Int)

fn main():
    let buckets: Map[Key, Str] = {Alpha(): "a", Beta(): "b", Gamma(): "c"}
    print(buckets[Alpha()])
    print(buckets[Beta()])
    print(buckets[Gamma()])
    let lookup: Map[Str, Mixed] = {"x": One(1), "y": Nothing()}
    print(lookup["x"])
    print(lookup["y"])
    let items: List[Mixed] = [Nothing(), One(2)]
    for item in items:
        print(item)
"#,
    );
}

#[test]
fn enum_wildcards_shadowing_and_nesting_match() {
    assert_same(
        "enum-wildcards",
        r#"enum Flag:
    On
    Off

fn choose(flag: Flag) -> Int:
    let outer = 100
    return match flag:
        On: 1
        _: outer

fn first(flag: Flag) -> Int:
    return match flag:
        _: 1
        On: 2

fn nested(flag: Flag):
    match flag:
        On:
            match Off():
                Off:
                    print("inner off")
                _:
                    print("inner other")
        Off:
            print("outer off")

fn main():
    print(choose(On()))
    print(choose(Off()))
    print(first(On()))
    nested(On())
    nested(Off())
"#,
    );
}

#[test]
fn enum_cycles_display_and_equality_match() {
    assert_same(
        "enum-cycles",
        r#"struct Node:
    children: List[Node]

enum Wrap:
    NodeValue(node: Node)
    Plain

fn main():
    let first = Node([])
    push(first.children, first)
    let second = Node([])
    push(second.children, second)
    print(NodeValue(first))
    print(NodeValue(second))
    print(NodeValue(first) == NodeValue(first))
    print(NodeValue(first) == NodeValue(second))
    print(NodeValue(first) == Plain())
"#,
    );
}

#[test]
fn any_enum_payload_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "enum-any-payload",
        "enum Boxed:\n    Value(value: Any)\n\nfn main():\n    print(Value(1))\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("variant payload"), "{stderr}");
}

#[test]
fn dynamic_enum_match_is_rejected_with_e0401() {
    let Some(engines) = run_engines(
        "enum-dynamic-match",
        "enum Boxed:\n    Value(number: Int)\n\n\
         fn unwrap(value: Any) -> Int:\n    match value:\n\
         \x20       Value(number):\n            return number\n\
         \x20       _:\n            return 0\n\n\
         fn main():\n    print(unwrap(Value(7)))\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("parameter has type Any"), "{stderr}");
}

#[test]
fn or_patterns_match_alternatives_and_bind_payloads() {
    assert_same(
        "enum-or-pattern",
        "enum Shape:\n    Circle(radius: Int, tag: Str)\n    Square(side: Int, tag: Str)\n    Point\n\n\
         fn value(shape: Shape) -> Int:\n    return match shape:\n\
         \x20       Circle(size, _) | Square(size, _): size\n\
         \x20       Point(): 0\n\n\
         fn classify(shape: Shape) -> Str:\n    match shape:\n\
         \x20       Circle(size, tag) | Square(size, tag) if size > 5:\n            return tag\n\
         \x20       Circle(size, tag) | Square(size, tag):\n            return \"small \" + tag\n\
         \x20       Point():\n            return \"point\"\n\n\
         fn label(n: Int) -> Str:\n    return match n:\n\
         \x20       1 | 2: \"small\"\n\
         \x20       3 | 4 if n > 0: \"medium\"\n\
         \x20       _: \"other\"\n\n\
         fn flag(value: Bool) -> Str:\n    return match value:\n\
         \x20       true | false: \"covered\"\n\n\
         fn main():\n\
         \x20   print(value(Circle(7, \"circle\")))\n\
         \x20   print(value(Square(9, \"square\")))\n\
         \x20   print(value(Point()))\n\
         \x20   print(classify(Circle(6, \"big circle\")))\n\
         \x20   print(classify(Square(2, \"square\")))\n\
         \x20   print(classify(Point()))\n\
         \x20   print(label(2))\n\
         \x20   print(label(4))\n\
         \x20   print(label(8))\n\
         \x20   print(flag(false))\n",
    );
}

#[test]
fn or_patterns_bind_nested_payloads() {
    assert_same(
        "enum-or-nested-pattern",
        "enum Tree:\n    Leaf(value: Int)\n    Node(left: Tree, right: Tree)\n\n\
         fn tree_value(tree: Tree) -> Int:\n    return match tree:\n\
         \x20       Leaf(value) | Node(Leaf(value), _) | Node(_, Leaf(value)): value\n\
         \x20       Node(_, _): -1\n\n\
         fn main():\n\
         \x20   print(tree_value(Leaf(4)))\n\
         \x20   print(tree_value(Node(Leaf(5), Leaf(6))))\n\
         \x20   print(tree_value(Node(Leaf(7), Leaf(8))))\n\
         \x20   print(tree_value(Node(Node(Leaf(9), Leaf(9)), Node(Leaf(9), Leaf(9)))))\n",
    );
}

#[test]
fn range_patterns_match_half_open_and_inclusive_bounds() {
    assert_same(
        "enum-range-pattern",
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
}

#[test]
fn nested_patterns_match() {
    assert_same(
        "enum-nested-pattern",
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
         fn classify(tree: Tree) -> Str:\n    return match tree:\n\
         \x20       Leaf(1): \"one\"\n\
         \x20       Leaf(value) if value > 5: \"big leaf\"\n\
         \x20       Leaf(_): \"leaf\"\n\
         \x20       Node(Leaf(value), _) if value > 3: \"left big\"\n\
         \x20       Node(_, _): \"node\"\n\n\
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
         \x20   print(classify(Leaf(1)))\n\
         \x20   print(classify(Leaf(9)))\n\
         \x20   print(classify(Leaf(2)))\n\
         \x20   print(classify(Node(Leaf(4), Leaf(1))))\n\
         \x20   print(classify(Node(Leaf(1), Leaf(1))))\n\
         \x20   print(describe(Nothing()))\n\
         \x20   print(describe(Just(Nothing())))\n\
         \x20   print(describe(Just(Just(Nothing()))))\n\
         \x20   print(describe(Just(Number(7))))\n\
         \x20   print(describe(Just(Number(-1))))\n\
         \x20   print(describe(Number(3)))\n",
    );
}

#[test]
fn imported_functions_structs_and_enums_match() {
    let directory = workspace(
        "modules",
        "from \"lib/math.vtx\" import Point, double, sum\n\
         import \"lib/kind.vtx\"\n\n\
         fn main():\n\
         \x20   let p = Point(3, 4)\n\
         \x20   print(double(sum(p)))\n\
         \x20   print(describe(Number(7)))\n",
    );
    fs::create_dir_all(directory.join("lib")).expect("create module dir");
    fs::write(
        directory.join("lib/math.vtx"),
        "struct Point:\n\
         \x20   x: Int\n\
         \x20   y: Int\n\n\
         fn sum(p: Point) -> Int:\n\
         \x20   return p.x + p.y\n\n\
         fn double(x: Int) -> Int:\n\
         \x20   return x * 2\n",
    )
    .expect("write math module");
    fs::write(
        directory.join("lib/kind.vtx"),
        "import \"math.vtx\"\n\n\
         enum Kind:\n\
         \x20   Number(value: Int)\n\
         \x20   Empty\n\n\
         fn describe(kind: Kind) -> Str:\n\
         \x20   match kind:\n\
         \x20       Number(value):\n\
         \x20           return \"number {value}\"\n\
         \x20       Empty:\n\
         \x20           return \"empty\"\n",
    )
    .expect("write kind module");

    let Some(engines) = run_engines_in(directory, "modules") else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), native.status.code());
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), "14\nnumber 7\n");
}

#[test]
fn file_io_matches() {
    let source = "fn main():\n\
         \x20   write_file(\"note.txt\", \"discarded\")\n\
         \x20   write_file(\"note.txt\", \"héllo\")\n\
         \x20   append_file(\"note.txt\", \" 🌍\")\n\
         \x20   print(read_file(\"note.txt\"))\n\
         \x20   print(file_exists(\"note.txt\"))\n\
         \x20   print(file_exists(\"missing.txt\"))\n\
         \x20   print(file_exists(\".\"))\n";
    let Some(engines) = run_engines_with_cwd("file-io", source) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), native.status.code());
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        "héllo 🌍\ntrue\nfalse\ntrue\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
    assert_eq!(
        fs::read_to_string(
            engines
                .executable
                .parent()
                .expect("directory")
                .join("note.txt")
        )
        .expect("read note"),
        "héllo 🌍"
    );
}

#[test]
fn file_io_evaluates_arguments_left_to_right() {
    let source = "fn path_for() -> Str:\n\
         \x20   print(\"path\")\n\
         \x20   return \"order.txt\"\n\n\
         fn contents_for() -> Str:\n\
         \x20   print(\"text\")\n\
         \x20   return \"x\"\n\n\
         fn main():\n\
         \x20   write_file(path_for(), contents_for())\n\
         \x20   print(read_file(\"order.txt\"))\n";
    let Some(engines) = run_engines_with_cwd("file-io-order", source) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), native.status.code());
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        "path\ntext\nx\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
}

#[test]
fn file_io_size_limit_matches() {
    let source = "fn main():\n    print(read_file(\"huge.bin\"))\n";
    let directory = workspace("file-io-size-limit", source);
    let file = fs::File::create(directory.join("huge.bin")).expect("create huge file");
    file.set_len(64 * 1024 * 1024 + 1)
        .expect("extend huge file");
    drop(file);
    let Some(engines) = run_engines_in_mode(directory, "file-io-size-limit", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
    for stderr in [&engines.interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0015]"), "{stderr}");
        assert!(stderr.contains("64 MiB"), "{stderr}");
    }
}

#[test]
fn file_io_any_paths_are_rejected_with_e0401() {
    for (label, source) in [
        (
            "file-io-any-read",
            "fn show(path: Any):\n\
             \x20   print(read_file(path))\n\n\
             fn main():\n\
             \x20   show(\"data.txt\")\n",
        ),
        (
            "file-io-any-write",
            "fn save(path: Any):\n\
             \x20   write_file(path, \"x\")\n\n\
             fn main():\n\
             \x20   save(\"data.txt\")\n",
        ),
    ] {
        let directory = workspace(label, source);
        fs::write(directory.join("data.txt"), "data").expect("write data");
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert_eq!(engines.interpreter.status.code(), Some(0));
        assert_eq!(engines.build.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&engines.build.stderr);
        assert!(stderr.contains("error[E0401]"), "{stderr}");
        assert!(stderr.contains("Any"), "{stderr}");
    }
}

#[test]
fn file_io_read_errors_match() {
    for (label, bytes) in [
        ("file-io-invalid-utf8", vec![0xff, 0xfe]),
        ("file-io-overlong", vec![0xc0, 0xaf]),
        ("file-io-surrogate", vec![0xed, 0xa0, 0x80]),
        ("file-io-too-large", vec![0xf4, 0x90, 0x80, 0x80]),
        ("file-io-truncated", vec![0xe2, 0x82]),
        ("file-io-nul", b"a\0b".to_vec()),
    ] {
        let source = "fn main():\n    print(read_file(\"data.bin\"))\n";
        let directory = workspace(label, source);
        fs::write(directory.join("data.bin"), &bytes).expect("write data file");
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "native build failed:\n{}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(engines.interpreter.status.code(), Some(2));
        assert_eq!(native.status.code(), Some(2));
        assert_eq!(
            String::from_utf8_lossy(&engines.interpreter.stdout),
            String::from_utf8_lossy(&native.stdout)
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains("error[R0015]"),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
    }
}

#[test]
fn file_io_write_and_missing_read_errors_match() {
    let source = "fn main():\n\
         \x20   write_file(\"blocked\", \"x\")\n";
    let directory = workspace("file-io-write-error", source);
    fs::create_dir(directory.join("blocked")).expect("create blocked directory");
    let Some(engines) = run_engines_in_mode(directory, "file-io-write-error", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
    assert!(
        String::from_utf8_lossy(&native.stderr).contains("error[R0015]"),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );

    let missing_source = "fn main():\n    print(read_file(\"missing.bin\"))\n";
    let directory = workspace("file-io-missing-read", missing_source);
    let Some(engines) = run_engines_in_mode(directory, "file-io-missing-read", true) else {
        return;
    };
    assert!(engines.build.status.success());
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
}

#[test]
fn directory_builtins_match() {
    let source = "fn main():\n\
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
                  \x20   print(file_exists(\"work\"))\n";
    let directory = workspace("directory-builtins", source);
    let Some(engines) = run_engines_in_mode(directory, "directory-builtins", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    assert_eq!(engines.interpreter.status.code(), Some(0));
    let native = engines.native.expect("native output");
    assert_eq!(native.status.code(), Some(0));
    let expected = "[\"a\", \"b\"]\n[\"note.txt\"]\ntrue\nfalse\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn directory_errors_match() {
    for (label, setup, source) in [
        (
            "directory-list-missing",
            None,
            "fn main():\n    print(list_dir(\"missing\"))\n",
        ),
        (
            "directory-make-existing",
            None,
            "fn main():\n    make_dir(\".\")\n",
        ),
        (
            "directory-remove-file-dir",
            Some(("sub", true)),
            "fn main():\n    remove_file(\"sub\")\n",
        ),
        (
            "directory-remove-nonempty",
            Some(("nonempty/child.txt", false)),
            "fn main():\n    remove_dir(\"nonempty\")\n",
        ),
        (
            "directory-list-file",
            Some(("plain.txt", false)),
            "fn main():\n    print(list_dir(\"plain.txt\"))\n",
        ),
    ] {
        let directory = workspace(label, source);
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
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label} build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(
            engines.interpreter.status.code(),
            Some(2),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert_eq!(
            native.status.code(),
            Some(2),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert!(
            String::from_utf8_lossy(&engines.interpreter.stderr).contains("error[R0015]"),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains("error[R0015]"),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&engines.interpreter.stdout),
            String::from_utf8_lossy(&native.stdout),
            "{label}"
        );
    }
}

#[cfg(unix)]
#[test]
fn directory_listing_rejects_non_utf8_names_in_both_engines() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let source = "fn main():\n    print(list_dir(\".\"))\n";
    let directory = workspace("directory-invalid-name", source);
    fs::create_dir(directory.join(OsStr::from_bytes(b"bad-\xff"))).expect("create invalid name");
    let Some(engines) = run_engines_in_mode(directory, "directory-invalid-name", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    for stderr in [&engines.interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0015]"), "{stderr}");
        assert!(stderr.contains("not valid UTF-8"), "{stderr}");
    }
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
}

#[test]
fn recursive_directory_builtins_match() {
    let source = "fn main():\n\
                  \x20   make_dir_all(\"work/a/b/c\")\n\
                  \x20   make_dir_all(\"work/a/b\")\n\
                  \x20   make_dir_all(\"\")\n\
                  \x20   write_file(\"work/a/b/c/note.txt\", \"hello\")\n\
                  \x20   print(list_dir(\"work/a/b/c\"))\n\
                  \x20   print(list_dir(\"work/a/b\"))\n\
                  \x20   remove_dir_all(\"work/a/b/\")\n\
                  \x20   print(file_exists(\"work/a/b\"))\n\
                  \x20   remove_dir_all(\"work\")\n\
                  \x20   print(file_exists(\"work\"))\n";
    let directory = workspace("recursive-directory-builtins", source);
    let Some(engines) = run_engines_in_mode(directory, "recursive-directory-builtins", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "[\"note.txt\"]\n[\"c\"]\nfalse\nfalse\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn recursive_directory_errors_match() {
    for (label, setup, source) in [
        (
            "recursive-make-through-file",
            Some(("plain.txt", false)),
            "fn main():\n    make_dir_all(\"plain.txt/sub\")\n",
        ),
        (
            "recursive-make-final-file",
            Some(("plain.txt", false)),
            "fn main():\n    make_dir_all(\"plain.txt\")\n",
        ),
        (
            "recursive-remove-file",
            Some(("plain.txt", false)),
            "fn main():\n    remove_dir_all(\"plain.txt\")\n",
        ),
        (
            "recursive-remove-missing",
            None,
            "fn main():\n    remove_dir_all(\"missing\")\n",
        ),
        (
            "recursive-remove-empty",
            None,
            "fn main():\n    remove_dir_all(\"\")\n",
        ),
    ] {
        let directory = workspace(label, source);
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
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label} build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(
            engines.interpreter.status.code(),
            Some(2),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert_eq!(
            native.status.code(),
            Some(2),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert!(
            String::from_utf8_lossy(&engines.interpreter.stderr).contains("error[R0015]"),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains("error[R0015]"),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&engines.interpreter.stdout),
            String::from_utf8_lossy(&native.stdout),
            "{label}"
        );
    }
}

#[test]
fn recursive_directory_any_paths_are_rejected_with_e0401() {
    for (label, source) in [
        (
            "recursive-make-any",
            "fn run(path: Any):\n\
             \x20   make_dir_all(path)\n\n\
             fn main():\n\
             \x20   run(\"made\")\n",
        ),
        (
            "recursive-remove-any",
            "fn run(path: Any):\n\
             \x20   remove_dir_all(path)\n\n\
             fn main():\n\
             \x20   make_dir_all(\"made\")\n\
             \x20   run(\"made\")\n",
        ),
    ] {
        let directory = workspace(label, source);
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert_eq!(engines.interpreter.status.code(), Some(0));
        assert_eq!(engines.build.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&engines.build.stderr);
        assert!(stderr.contains("error[E0401]"), "{stderr}");
        assert!(stderr.contains("Any"), "{stderr}");
    }
}

#[cfg(unix)]
#[test]
fn recursive_directory_removal_does_not_follow_symlinks() {
    use std::os::unix::fs::symlink;

    fn setup(label: &str) -> PathBuf {
        let directory = workspace(
            label,
            "fn main():\n\
             \x20   make_dir_all(\"make-link\")\n\
             \x20   remove_dir_all(\"tree\")\n\
             \x20   print(file_exists(\"tree\"))\n\
             \x20   print(file_exists(\"target/inner/data.txt\"))\n\
             \x20   remove_dir_all(\"root-link/\")\n\
             \x20   print(file_exists(\"root-link\"))\n\
             \x20   print(file_exists(\"target/inner/data.txt\"))\n",
        );
        let target = directory.join("target");
        fs::create_dir_all(target.join("inner")).expect("create symlink target");
        fs::write(target.join("inner/data.txt"), "keep").expect("write target data");
        fs::create_dir_all(directory.join("tree/sub")).expect("create tree");
        symlink(&target, directory.join("tree/sub/link")).expect("create inner symlink");
        symlink(&target, directory.join("root-link")).expect("create root symlink");
        symlink(&target, directory.join("make-link")).expect("create make symlink");
        directory
    }

    if !compiler_available() {
        eprintln!("skipping native test `recursive-directory-symlinks`: no C compiler");
        return;
    }

    let interpreter_directory = setup("recursive-symlinks-interpreter");
    let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("run")
        .arg(interpreter_directory.join("main.vtx"))
        .current_dir(&interpreter_directory)
        .output()
        .expect("run interpreter");

    let native_directory = setup("recursive-symlinks-native");
    let executable = native_directory.join("program");
    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(native_directory.join("main.vtx"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("run native build");
    assert!(
        build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(&executable)
        .current_dir(&native_directory)
        .output()
        .expect("run native executable");

    let expected = "false\ntrue\nfalse\ntrue\n";
    assert_eq!(interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&interpreter.stdout), expected);
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
    for directory in [&interpreter_directory, &native_directory] {
        assert!(!directory.join("tree").exists());
        assert!(directory.join("root-link").symlink_metadata().is_err());
        assert_eq!(
            fs::read_to_string(directory.join("target/inner/data.txt")).expect("read target data"),
            "keep"
        );
    }
}

#[cfg(unix)]
#[test]
fn recursive_removal_refuses_destructive_roots_in_both_engines() {
    for (label, path) in [
        ("recursive-root-dot", "."),
        ("recursive-root-dot-slash", "./"),
        ("recursive-root-dot-dot", ".."),
        ("recursive-root-tree-dot", "tree/."),
        ("recursive-root-tree-dot-dot", "tree/.."),
        ("recursive-root-filesystem", "/"),
        ("recursive-root-filesystem-double", "//"),
    ] {
        let source = format!("fn main():\n    remove_dir_all(\"{path}\")\n");
        let directory = workspace(label, &source);
        fs::write(directory.join("sentinel.txt"), "keep").expect("write sentinel");
        fs::create_dir_all(directory.join("tree/inner")).expect("create tree");
        fs::write(directory.join("tree/inner/data.txt"), "keep").expect("write tree data");

        let Some(engines) = run_engines_in_mode(directory.clone(), label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label}: build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines
            .native
            .expect("native output after successful build");
        assert_eq!(engines.interpreter.status.code(), Some(2), "{label}");
        assert_eq!(native.status.code(), Some(2), "{label}");
        for (engine, stderr) in [
            ("interpreter", &engines.interpreter.stderr),
            ("native", &native.stderr),
        ] {
            let stderr = String::from_utf8_lossy(stderr);
            assert!(
                stderr.contains("error[R0015]"),
                "{label}/{engine}: {stderr}"
            );
            assert!(
                stderr.contains("refusing to remove"),
                "{label}/{engine}: {stderr}"
            );
        }
        assert!(
            directory.join("sentinel.txt").exists(),
            "{label}: sentinel was deleted"
        );
        assert!(
            directory.join("tree/inner/data.txt").exists(),
            "{label}: tree was deleted"
        );
        assert!(
            directory.join("main.vtx").exists(),
            "{label}: source was deleted"
        );
        assert!(
            engines.executable.exists(),
            "{label}: native executable was deleted"
        );
    }
}

#[cfg(unix)]
#[test]
fn recursive_removal_allows_a_leading_dot_prefix_in_both_engines() {
    let source = "fn main():\n\
                  \x20   make_dir_all(\"tree/inner\")\n\
                  \x20   write_file(\"tree/inner/data.txt\", \"keep\")\n\
                  \x20   remove_dir_all(\"./tree\")\n\
                  \x20   print(file_exists(\"tree\"))\n\
                  \x20   print(file_exists(\"main.vtx\"))\n";
    let directory = workspace("recursive-root-leading-dot", source);

    let Some(engines) = run_engines_in_mode(directory.clone(), "recursive-root-leading-dot", true)
    else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines
        .native
        .expect("native output after successful build");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        "false\ntrue\n"
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), "false\ntrue\n");
    assert!(!directory.join("tree").exists());
    assert!(directory.join("main.vtx").exists());
}

#[test]
fn file_metadata_builtins_match() {
    let source = "fn main():\n\
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
                  \x20   remove_dir(\"folder\")\n";
    let directory = workspace("file-metadata-builtins", source);
    let Some(engines) = run_engines_in_mode(directory, "file-metadata-builtins", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "5\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\nfalse\nfalse\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn file_metadata_errors_match() {
    for (label, setup, source) in [
        (
            "metadata-error-directory",
            true,
            "fn main():\n    print(file_size(\"folder\"))\n",
        ),
        (
            "metadata-error-missing",
            false,
            "fn main():\n    print(file_size(\"missing\"))\n",
        ),
        (
            "metadata-error-empty",
            false,
            "fn main():\n    print(file_size(\"\"))\n",
        ),
    ] {
        let directory = workspace(label, source);
        if setup {
            fs::create_dir(directory.join("folder")).expect("create folder");
        }
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label} build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(
            engines.interpreter.status.code(),
            Some(2),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert_eq!(
            native.status.code(),
            Some(2),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        for stderr in [&engines.interpreter.stderr, &native.stderr] {
            assert!(
                String::from_utf8_lossy(stderr).contains("error[R0015]"),
                "{label}: {}",
                String::from_utf8_lossy(stderr)
            );
        }
    }
}

#[test]
fn file_metadata_any_paths_are_rejected_with_e0401() {
    for (label, builtin) in [
        ("metadata-any-file-size", "file_size"),
        ("metadata-any-is-file", "is_file"),
        ("metadata-any-is-dir", "is_dir"),
        ("metadata-any-is-symlink", "is_symlink"),
    ] {
        let source = format!(
            "fn run(path: Any):\n\
             \x20   print({builtin}(path))\n\n\
             fn main():\n\
             \x20   run(\"data.txt\")\n"
        );
        let directory = workspace(label, &source);
        fs::write(directory.join("data.txt"), "x").expect("write metadata input");
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert_eq!(engines.interpreter.status.code(), Some(0));
        assert_eq!(engines.build.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&engines.build.stderr);
        assert!(stderr.contains("error[E0401]"), "{stderr}");
        assert!(stderr.contains("Any"), "{stderr}");
    }
}

#[cfg(unix)]
#[test]
fn file_metadata_symlink_rules_match() {
    use std::os::unix::fs::symlink;

    fn setup(label: &str) -> PathBuf {
        let directory = workspace(
            label,
            "fn main():\n\
             \x20   print(file_size(\"link.txt\"))\n\
             \x20   print(is_file(\"link.txt\"))\n\
             \x20   print(is_symlink(\"link.txt\"))\n\
             \x20   print(is_dir(\"link-dir\"))\n\
             \x20   print(is_symlink(\"link-dir\"))\n\
             \x20   print(is_file(\"dangling\"))\n\
             \x20   print(is_dir(\"dangling\"))\n\
             \x20   print(is_symlink(\"dangling\"))\n",
        );
        fs::write(directory.join("target.txt"), "hello").expect("write target");
        fs::create_dir(directory.join("target-dir")).expect("create target dir");
        symlink("target.txt", directory.join("link.txt")).expect("create file link");
        symlink("target-dir", directory.join("link-dir")).expect("create dir link");
        symlink("absent", directory.join("dangling")).expect("create dangling link");
        directory
    }

    if !compiler_available() {
        eprintln!("skipping native test `file-metadata-symlinks`: no C compiler");
        return;
    }

    let interpreter_directory = setup("metadata-symlinks-interpreter");
    let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("run")
        .arg(interpreter_directory.join("main.vtx"))
        .current_dir(&interpreter_directory)
        .output()
        .expect("run interpreter");

    let native_directory = setup("metadata-symlinks-native");
    let executable = native_directory.join("program");
    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(native_directory.join("main.vtx"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("run native build");
    assert!(
        build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(&executable)
        .current_dir(&native_directory)
        .output()
        .expect("run native executable");

    let expected = "5\ntrue\ntrue\ntrue\ntrue\nfalse\nfalse\ntrue\n";
    assert_eq!(interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&interpreter.stdout), expected);
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn text_builtins_match() {
    let source = "fn main():\n\
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
                  \x20   print(ends_with(\"\", \"\"))\n";
    let directory = workspace("text-builtins", source);
    let Some(engines) = run_engines_in_mode(directory, "text-builtins", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "[\"a\", \"b\", \"\", \"c\", \"\"]\n\
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
                    true\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn text_replace_empty_pattern_matches() {
    let source = "fn main():\n    print(replace(\"abc\", \"\", \"x\"))\n";
    let directory = workspace("text-empty-pattern", source);
    let Some(engines) = run_engines_in_mode(directory, "text-empty-pattern", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    for stderr in [&engines.interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0016]"), "{stderr}");
        assert!(stderr.contains("non-empty pattern"), "{stderr}");
    }
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
}

#[test]
fn text_builtins_evaluate_arguments_left_to_right() {
    let source = "fn note(label: Str) -> Str:\n\
                  \x20   print(label)\n\
                  \x20   return label\n\
                  \n\
                  fn main():\n\
                  \x20   print(split(note(\"a-b\"), note(\"-\")))\n\
                  \x20   print(join([note(\"x\"), note(\"y\")], note(\":\")))\n\
                  \x20   print(replace(note(\"a-b\"), note(\"a\"), note(\"c\")))\n\
                  \x20   print(contains(note(\"banana\"), note(\"ana\")))\n\
                  \x20   print(starts_with(note(\"hello\"), note(\"he\")))\n\
                  \x20   print(ends_with(note(\"hello\"), note(\"lo\")))\n";
    let directory = workspace("text-argument-order", source);
    let Some(engines) = run_engines_in_mode(directory, "text-argument-order", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "a-b\n-\n[\"a\", \"b\"]\nx\ny\n:\nx:y\na-b\na\nc\nc-b\nbanana\nana\ntrue\nhello\nhe\ntrue\nhello\nlo\ntrue\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
fn string_contains_argument_order_is_compiler_independent() {
    if !compiler_available() || !gcc_available() {
        eprintln!(
            "skipping native test `string_contains_argument_order_is_compiler_independent`: clang and gcc are required"
        );
        return;
    }
    let source = "fn note(label: Str) -> Str:\n\
                  \x20   print(label)\n\
                  \x20   return label\n\
                  \n\
                  fn main():\n\
                  \x20   print(contains(note(\"banana\"), note(\"ana\")))\n\
                  \x20   print(contains(note(\"banana\"), note(\"zzz\")))\n";
    let directory = workspace("contains-order-compilers", source);
    let source_path = directory.join("main.vtx");

    let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("run")
        .arg(&source_path)
        .output()
        .expect("run interpreter");
    assert_eq!(interpreter.status.code(), Some(0));
    let expected = "banana\nana\ntrue\nbanana\nzzz\nfalse\n";
    assert_eq!(
        String::from_utf8_lossy(&interpreter.stdout),
        expected,
        "interpreter output"
    );

    for compiler in ["clang", "gcc"] {
        let executable = directory.join(format!("program-{compiler}"));
        let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
            .arg("build")
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .env("VANTYX_CC", compiler)
            .output()
            .expect("run native build");
        assert!(
            build.status.success(),
            "{compiler} build failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        let native = Command::new(&executable).output().expect("run native");
        assert_eq!(
            native.status.code(),
            Some(0),
            "{compiler} program failed: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&native.stdout),
            expected,
            "{compiler} stdout differs"
        );
    }
}

#[test]
fn path_builtins_match() {
    let source = "fn main():\n\
                  \x20   print(path_join(\"a\", \"b\"))\n\
                  \x20   print(path_join(\"a/\", \"b\"))\n\
                  \x20   print(path_join(\"a\", \"/b\"))\n\
                  \x20   print(path_join(\"\", \"b\"))\n\
                  \x20   print(path_join(\"a\", \"\"))\n\
                  \x20   print(path_join(\"a//\", \"b\"))\n\
                  \x20   print(path_join(\"/\", \"a\"))\n\
                  \x20   print(path_join(\"\", \"\"))\n\
                  \x20   print(path_join(\"naïve\", \"文件.txt\"))\n\
                  \x20   print(path_parent(\"a/b/c\"))\n\
                  \x20   print(path_parent(\"a\"))\n\
                  \x20   print(path_parent(\"/a\"))\n\
                  \x20   print(path_parent(\"/\"))\n\
                  \x20   print(path_parent(\"a//b\"))\n\
                  \x20   print(path_parent(\"a/b/\"))\n\
                  \x20   print(path_parent(\"\"))\n\
                  \x20   print(path_parent(\"a/\"))\n\
                  \x20   print(path_parent(\"//\"))\n\
                  \x20   print(path_name(\"a/b/\"))\n\
                  \x20   print(path_name(\"/\"))\n\
                  \x20   print(path_name(\"\"))\n\
                  \x20   print(path_name(\"a//b//\"))\n\
                  \x20   print(path_name(\"naïve/文件.txt\"))\n\
                  \x20   print(path_stem(\"a/b.tar.gz\"))\n\
                  \x20   print(path_stem(\".bashrc\"))\n\
                  \x20   print(path_stem(\"a.\"))\n\
                  \x20   print(path_stem(\"archive\"))\n\
                  \x20   print(path_stem(\"..\"))\n\
                  \x20   print(path_stem(\".hidden.txt\"))\n\
                  \x20   print(path_stem(\"a..b\"))\n\
                  \x20   print(path_stem(\"a/b..\"))\n\
                  \x20   print(path_stem(\"a/...\"))\n\
                  \x20   print(path_stem(\"a.b.\"))\n\
                  \x20   print(path_stem(\"a.b..\"))\n\
                  \x20   print(path_extension(\"a/b.tar.gz\"))\n\
                  \x20   print(path_extension(\".bashrc\"))\n\
                  \x20   print(path_extension(\"a.\"))\n\
                  \x20   print(path_extension(\"archive\"))\n\
                  \x20   print(path_extension(\"archive.tar\"))\n\
                  \x20   print(path_extension(\".hidden.txt\"))\n\
                  \x20   print(path_extension(\"a..b\"))\n\
                  \x20   print(path_extension(\"a/b..\"))\n\
                  \x20   print(path_extension(\"a/...\"))\n\
                  \x20   print(path_extension(\"a.b.\"))\n\
                  \x20   print(path_extension(\"a.b..\"))\n\
                  \x20   print(path_is_absolute(\"/a\"))\n\
                  \x20   print(path_is_absolute(\"a\"))\n\
                  \x20   print(path_is_absolute(\"\"))\n\
                  \x20   print(path_is_absolute(\"//\"))\n\
                  \x20   print(path_is_absolute(\"./a\"))\n";
    let directory = workspace("path-builtins", source);
    let Some(engines) = run_engines_in_mode(directory, "path-builtins", false) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "a/b\na/b\n/b\nb\na\na//b\n/a\n\nnaïve/文件.txt\n\
                    a/b\n.\n/\n/\na\na\n.\n.\n/\nb\n\n\nb\n文件.txt\n\
                    b.tar\n.bashrc\na.\narchive\n..\n.hidden\na.\n\
                    b..\n...\na.b.\na.b..\n\
                    gz\n\n\n\ntar\ntxt\nb\n\n\n\n\n\
                    true\nfalse\nfalse\ntrue\nfalse\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn path_trailing_dot_names_keep_the_whole_stem() {
    let source = "fn main():\n\
                  \x20   print(path_stem(\"a/b..\"))\n\
                  \x20   print(path_stem(\"a/...\"))\n\
                  \x20   print(path_stem(\"a.b.\"))\n\
                  \x20   print(path_stem(\"a.b..\"))\n\
                  \x20   print(path_stem(\"..a.\"))\n\
                  \x20   print(path_stem(\".a.\"))\n\
                  \x20   print(path_extension(\"a/b..\"))\n\
                  \x20   print(path_extension(\"a/...\"))\n\
                  \x20   print(path_extension(\"a.b.\"))\n\
                  \x20   print(path_extension(\"a.b..\"))\n\
                  \x20   print(path_extension(\"..a.\"))\n\
                  \x20   print(path_extension(\".a.\"))\n";
    let directory = workspace("path-trailing-dots", source);
    let Some(engines) = run_engines_in_mode(directory, "path-trailing-dots", false) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let mut expected = String::from("b..\n...\na.b.\na.b..\n..a.\n.a.\n");
    for _ in 0..6 {
        expected.push('\n');
    }
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn path_builtins_evaluate_arguments_left_to_right() {
    let source = "fn note(label: Str) -> Str:\n\
                  \x20   print(label)\n\
                  \x20   return label\n\
                  \n\
                  fn main():\n\
                  \x20   print(path_join(note(\"a\"), note(\"b\")))\n\
                  \x20   print(path_parent(note(\"a/b\")))\n";
    let directory = workspace("path-argument-order", source);
    let Some(engines) = run_engines_in_mode(directory, "path-argument-order", false) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "a\nb\na/b\na/b\na\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn path_builtins_any_paths_are_rejected_with_e0401() {
    for (label, source) in [
        (
            "path-any-join",
            "fn join_it(base: Any, child: Any) -> Str:\n\
             \x20   return path_join(base, child)\n\n\
             fn main():\n\
             \x20   print(join_it(\"a\", \"b\"))\n",
        ),
        (
            "path-any-parent",
            "fn parent_it(path: Any) -> Str:\n\
             \x20   return path_parent(path)\n\n\
             fn main():\n\
             \x20   print(parent_it(\"a/b\"))\n",
        ),
        (
            "path-any-name",
            "fn name_it(path: Any) -> Str:\n\
             \x20   return path_name(path)\n\n\
             fn main():\n\
             \x20   print(name_it(\"a/b\"))\n",
        ),
        (
            "path-any-stem",
            "fn stem_it(path: Any) -> Str:\n\
             \x20   return path_stem(path)\n\n\
             fn main():\n\
             \x20   print(stem_it(\"a/b.txt\"))\n",
        ),
        (
            "path-any-extension",
            "fn extension_it(path: Any) -> Str:\n\
             \x20   return path_extension(path)\n\n\
             fn main():\n\
             \x20   print(extension_it(\"a/b.txt\"))\n",
        ),
        (
            "path-any-absolute",
            "fn absolute_it(path: Any) -> Bool:\n\
             \x20   return path_is_absolute(path)\n\n\
             fn main():\n\
             \x20   print(absolute_it(\"/a\"))\n",
        ),
    ] {
        let directory = workspace(label, source);
        let Some(engines) = run_engines_in_mode(directory, label, false) else {
            return;
        };
        assert_eq!(engines.interpreter.status.code(), Some(0));
        assert_eq!(engines.build.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&engines.build.stderr);
        assert!(stderr.contains("error[E0401]"), "{stderr}");
        assert!(stderr.contains("Any"), "{stderr}");
    }
}

#[test]
fn rename_and_copy_match() {
    let source = "fn main():\n\
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
                  \x20   print(file_exists(\"moved\"))\n";
    let directory = workspace("rename-copy", source);
    let Some(engines) = run_engines_in_mode(directory, "rename-copy", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "false\nhello\nhello\nhello\n[\"inner.txt\"]\nfalse\n[\"inner.txt\"]\nfalse\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
    let directory = engines
        .executable
        .parent()
        .expect("executable directory")
        .to_path_buf();
    for name in ["source.txt", "renamed.txt", "copy.txt", "moved"] {
        assert!(!directory.join(name).exists(), "{name} was not cleaned up");
    }
}

#[test]
fn copy_file_preserves_bytes_match() {
    let source = "fn main():\n\
                  \x20   copy_file(\"blob.bin\", \"blob-copy.bin\")\n\
                  \x20   print(file_exists(\"blob-copy.bin\"))\n";
    let directory = workspace("rename-copy-bytes", source);
    let bytes: Vec<u8> = vec![0, 0xff, 0xfe, 1, 2, 3, 0, 0x80, 0x7f];
    fs::write(directory.join("blob.bin"), &bytes).expect("write binary source");
    let output_directory = directory.clone();
    let Some(engines) = run_engines_in_mode(directory, "rename-copy-bytes", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        "true\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        String::from_utf8_lossy(&native.stdout)
    );
    assert_eq!(
        fs::read(output_directory.join("blob-copy.bin")).expect("read copied bytes"),
        bytes
    );
}

#[test]
fn copy_file_rejects_self_copies_in_both_engines() {
    let mut targets = vec![("same", "source.txt".to_string())];
    #[cfg(unix)]
    targets.push(("symlink", "link.txt".to_string()));
    #[cfg(unix)]
    targets.push(("hardlink", "hard.txt".to_string()));

    for (label, target) in targets {
        let name = format!("copy-self-{label}");
        let source = format!("fn main():\n    copy_file(\"source.txt\", \"{target}\")\n");
        let directory = workspace(&name, &source);
        fs::write(directory.join("source.txt"), "keep me").expect("write source");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            match label {
                "symlink" => {
                    symlink("source.txt", directory.join("link.txt")).expect("create symlink")
                }
                "hardlink" => {
                    fs::hard_link(directory.join("source.txt"), directory.join("hard.txt"))
                        .expect("create hard link")
                }
                _ => {}
            }
        }

        let Some(engines) = run_engines_in_mode(directory.clone(), &name, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label} build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines
            .native
            .expect("native output after successful build");
        assert_eq!(
            engines.interpreter.status.code(),
            Some(2),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert_eq!(native.status.code(), Some(2), "{label} native exit code");
        assert!(
            String::from_utf8_lossy(&engines.interpreter.stderr)
                .contains("source and target are the same file"),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains("source and target are the same file"),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            fs::read_to_string(directory.join("source.txt")).expect("read preserved source"),
            "keep me",
            "{label} changed the source"
        );
    }
}

#[test]
fn rename_and_copy_errors_match() {
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
        let directory = workspace(label, source);
        if let Some((path, is_dir)) = setup {
            let path = directory.join(path);
            if is_dir {
                fs::create_dir_all(&path).expect("create setup directory");
            } else {
                fs::write(&path, "x").expect("write setup file");
            }
        }
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label} build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(
            engines.interpreter.status.code(),
            Some(2),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert_eq!(
            native.status.code(),
            Some(2),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert!(
            String::from_utf8_lossy(&engines.interpreter.stderr).contains("error[R0015]"),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert!(
            String::from_utf8_lossy(&native.stderr).contains("error[R0015]"),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&engines.interpreter.stdout),
            String::from_utf8_lossy(&native.stdout),
            "{label}"
        );
    }
}

#[test]
fn rename_and_copy_evaluate_arguments_left_to_right() {
    let source = "fn note(label: Str) -> Str:\n\
                  \x20   print(label)\n\
                  \x20   return label\n\
                  \n\
                  fn main():\n\
                  \x20   write_file(\"src.txt\", \"x\")\n\
                  \x20   rename(note(\"src.txt\"), note(\"dst.txt\"))\n\
                  \x20   copy_file(note(\"dst.txt\"), note(\"copy.txt\"))\n\
                  \x20   print(read_file(\"copy.txt\"))\n";
    let directory = workspace("rename-copy-order", source);
    let Some(engines) = run_engines_in_mode(directory, "rename-copy-order", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = "src.txt\ndst.txt\ndst.txt\ncopy.txt\nx\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn rename_and_copy_any_paths_are_rejected_with_e0401() {
    let source = "fn move_it(src: Any, dst: Any):\n\
                  \x20   rename(src, dst)\n\n\
                  fn copy_it(src: Any, dst: Any):\n\
                  \x20   copy_file(src, dst)\n\n\
                  fn main():\n\
                  \x20   write_file(\"data.txt\", \"data\")\n\
                  \x20   move_it(\"data.txt\", \"moved.txt\")\n\
                  \x20   print(read_file(\"moved.txt\"))\n\
                  \x20   copy_it(\"moved.txt\", \"copy.txt\")\n\
                  \x20   print(read_file(\"copy.txt\"))\n";
    let directory = workspace("rename-copy-any", source);
    let Some(engines) = run_engines_in_mode(directory, "rename-copy-any", true) else {
        return;
    };
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        "data\ndata\n"
    );
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("Any"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn glob_matches_interpreter_in_both_engines() {
    use std::os::unix::fs::symlink;

    let source = "fn main():\n\
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
                  \x20   print(glob(\"tree/link/**\"))\n";
    let directory = workspace("glob-round-trip", source);
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
    let Some(engines) = run_engines_in_mode(directory, "glob-round-trip", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(
        engines.interpreter.status.code(),
        Some(0),
        "interpreter stderr: {}",
        String::from_utf8_lossy(&engines.interpreter.stderr)
    );
    assert_eq!(native.status.code(), Some(0));
    let expected = "[\"tree/a.txt\"]\n\
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
                    [\"tree/link/b.txt\", \"tree/link/sub\", \"tree/link/sub/c.vtx\", \"tree/link/sub/deep\", \"tree/link/sub/deep/d.txt\"]\n";
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
}

#[cfg(unix)]
#[test]
fn glob_malformed_patterns_fail_in_both_engines() {
    for (label, pattern) in [
        ("glob-native-unclosed", "["),
        ("glob-native-descending", "[z-a]"),
    ] {
        let source = format!("fn main():\n    print(glob(\"{pattern}\"))\n");
        let directory = workspace(label, &source);
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label} build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(engines.interpreter.status.code(), Some(2), "{label}");
        assert_eq!(native.status.code(), Some(2), "{label}");
        for stderr in [&engines.interpreter.stderr, &native.stderr] {
            let stderr = String::from_utf8_lossy(stderr);
            assert!(stderr.contains("error[R0016]"), "{label}: {stderr}");
        }
    }
}

#[test]
fn glob_depth_limit_matches_in_both_engines() {
    let source = "fn main():\n    print(glob(\"**\"))\n";
    let directory = workspace("glob-depth", source);
    let mut path = directory.join("deep");
    for index in 0..130 {
        path.push(format!("d{index}"));
    }
    fs::create_dir_all(&path).expect("create deep glob tree");
    let Some(engines) = run_engines_in_mode(directory, "glob-depth", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    for stderr in [&engines.interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0015]"), "{stderr}");
        assert!(stderr.contains("maximum depth"), "{stderr}");
    }
}

#[cfg(unix)]
#[test]
fn glob_rejects_non_utf8_names_in_both_engines() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let source = "fn main():\n    print(glob(\"*\"))\n";
    let directory = workspace("glob-invalid-name", source);
    fs::create_dir(directory.join(OsStr::from_bytes(b"bad-\xff"))).expect("create invalid name");
    let Some(engines) = run_engines_in_mode(directory, "glob-invalid-name", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    for stderr in [&engines.interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0015]"), "{stderr}");
        assert!(stderr.contains("UTF-8"), "{stderr}");
    }
}

#[cfg(unix)]
#[test]
fn glob_result_cap_matches_in_both_engines() {
    let source = "fn main():\n    print(glob(\"files/*\"))\n";
    let directory = workspace("glob-result-cap", source);
    let files = directory.join("files");
    fs::create_dir(&files).expect("create glob cap directory");
    let mut created = 0usize;
    // Stay below per-inode hard-link limits (for example ext4's 65,000) by
    // spreading the links across several anchors.
    for anchor_index in 0..4 {
        let anchor = files.join(format!("anchor{anchor_index}.txt"));
        fs::write(&anchor, "x").expect("write glob cap anchor");
        for _ in 0..25_001 {
            fs::hard_link(&anchor, files.join(format!("f{created:06}")))
                .expect("create glob cap link");
            created += 1;
        }
    }
    assert!(created > 100_000);
    let engines = run_engines_in_mode(directory, "glob-result-cap", true);
    let _ = fs::remove_dir_all(&files);
    let Some(engines) = engines else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    for stderr in [&engines.interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0015]"), "{stderr}");
        assert!(stderr.contains("candidate paths"), "{stderr}");
    }
}

#[test]
fn glob_absolute_patterns_match_in_both_engines() {
    let source = "fn main():\n    print(\"placeholder\")\n";
    let directory = workspace("glob-absolute", source);
    fs::create_dir_all(directory.join("tree")).expect("create glob tree");
    fs::write(directory.join("tree/a.txt"), "a").expect("write glob file");
    let root = directory
        .canonicalize()
        .expect("canonicalize glob workspace")
        .to_str()
        .expect("utf-8 path")
        .to_string();
    let source = format!(
        "fn main():\n\
         \x20   print(glob(\"{root}/tree/*.txt\"))\n\
         \x20   print(glob(\"{root}//tree//*.txt\"))\n"
    );
    fs::write(directory.join("main.vtx"), &source).expect("write absolute glob source");
    let Some(engines) = run_engines_in_mode(directory, "glob-absolute", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    let expected = format!("[\"{root}/tree/a.txt\"]\n").repeat(2);
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn glob_component_cap_matches_in_both_engines() {
    let source = format!("fn main():\n    print(glob(\"{}\"))\n", "a/".repeat(1025));
    let directory = workspace("glob-components", &source);
    let Some(engines) = run_engines_in_mode(directory, "glob-components", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(engines.interpreter.status.code(), Some(2));
    assert_eq!(native.status.code(), Some(2));
    for stderr in [&engines.interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0016]"), "{stderr}");
    }
}

#[test]
fn recursive_copy_matches_in_both_engines() {
    let source = "fn main():\n\
                  \x20   make_dir_all(\"tree/sub/empty\")\n\
                  \x20   make_dir_all(\"tree/.hidden\")\n\
                  \x20   write_file(\"tree/a.txt\", \"alpha\")\n\
                  \x20   write_file(\"tree/sub/b.txt\", \"beta\")\n\
                  \x20   write_file(\"tree/.hidden/dot\", \"dot\")\n\
                  \x20   copy_dir_all(\"tree\", \"copy\")\n\
                  \x20   print(read_file(\"copy/a.txt\"))\n\
                  \x20   print(read_file(\"copy/sub/b.txt\"))\n\
                  \x20   print(read_file(\"copy/.hidden/dot\"))\n\
                  \x20   print(is_dir(\"copy/sub/empty\"))\n\
                  \x20   print(glob(\"copy/**/*.txt\"))\n\
                  \x20   copy_dir_all(\"tree\", \"copy-two\")\n\
                  \x20   print(file_exists(\"copy-two/a.txt\"))\n\
                  \x20   remove_dir_all(\"copy\")\n\
                  \x20   remove_dir_all(\"copy-two\")\n\
                  \x20   remove_dir_all(\"tree\")\n";
    let directory = workspace("copy-roundtrip", source);
    let Some(engines) = run_engines_in_mode(directory, "copy-roundtrip", true) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(
        engines.interpreter.status.code(),
        Some(0),
        "interpreter stderr: {}",
        String::from_utf8_lossy(&engines.interpreter.stderr)
    );
    assert_eq!(
        native.status.code(),
        Some(0),
        "native stderr: {}",
        String::from_utf8_lossy(&native.stderr)
    );
    let expected = "alpha\n\
                    beta\n\
                    dot\n\
                    true\n\
                    [\"copy/a.txt\", \"copy/sub/b.txt\"]\n\
                    true\n";
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        expected
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout), expected);
}

#[test]
fn recursive_copy_errors_match_in_both_engines() {
    let cases: [(&str, &str, &str, bool); 8] = [
        (
            "copy-missing-source",
            "copy_dir_all(\"missing\", \"copy\")",
            "cannot copy directory",
            false,
        ),
        (
            "copy-file-source",
            "copy_dir_all(\"file.txt\", \"copy\")",
            "not a directory",
            false,
        ),
        (
            "copy-existing-target",
            "copy_dir_all(\"tree\", \"existing\")",
            "already exists",
            true,
        ),
        (
            "copy-missing-parent",
            "copy_dir_all(\"tree\", \"missing/copy\")",
            "target parent",
            true,
        ),
        (
            "copy-inside-source",
            "copy_dir_all(\"tree\", \"tree/inside\")",
            "inside the source",
            true,
        ),
        (
            "copy-empty-target",
            "copy_dir_all(\"tree\", \"\")",
            "must name a new directory",
            true,
        ),
        (
            "copy-dot-target",
            "copy_dir_all(\"tree\", \".\")",
            "must name a new directory",
            true,
        ),
        (
            "copy-empty-source",
            "copy_dir_all(\"\", \"copy\")",
            "source path is empty",
            false,
        ),
    ];
    for (label, call, needle, needs_tree) in cases {
        let source = format!("fn main():\n    {call}\n");
        let directory = workspace(label, &source);
        if needs_tree {
            fs::create_dir(directory.join("tree")).expect("create copy tree");
        }
        if label == "copy-file-source" {
            fs::write(directory.join("file.txt"), "x").expect("write copy file");
        }
        if label == "copy-existing-target" {
            fs::create_dir(directory.join("existing")).expect("create copy target");
        }
        let Some(engines) = run_engines_in_mode(directory, label, true) else {
            return;
        };
        assert!(
            engines.build.status.success(),
            "{label} build failed: {}",
            String::from_utf8_lossy(&engines.build.stderr)
        );
        let native = engines.native.expect("native output");
        assert_eq!(
            engines.interpreter.status.code(),
            Some(2),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&engines.interpreter.stderr)
        );
        assert_eq!(
            native.status.code(),
            Some(2),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        for stderr in [&engines.interpreter.stderr, &native.stderr] {
            let stderr = String::from_utf8_lossy(stderr);
            assert!(stderr.contains("error[R0015]"), "{label}: {stderr}");
            assert!(
                stderr.contains(needle),
                "{label}: expected {needle:?} in {stderr}"
            );
        }
    }
}

#[test]
fn recursive_copy_any_paths_are_rejected_with_e0401() {
    let source = "fn run(source: Any):\n\
                  \x20   copy_dir_all(source, \"copy\")\n\
                  \x20   print(is_dir(\"copy\"))\n\
                  \n\
                  fn main():\n\
                  \x20   make_dir(\"tree\")\n\
                  \x20   write_file(\"tree/a.txt\", \"a\")\n\
                  \x20   run(\"tree\")\n";
    let directory = workspace("copy-any", source);
    let Some(engines) = run_engines_in_mode(directory, "copy-any", true) else {
        return;
    };
    assert_eq!(
        engines.interpreter.status.code(),
        Some(0),
        "interpreter stderr: {}",
        String::from_utf8_lossy(&engines.interpreter.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&engines.interpreter.stdout),
        "true\n"
    );
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(stderr.contains("Any"), "{stderr}");
}

#[test]
fn recursive_copy_depth_limit_matches_in_both_engines() {
    if !compiler_available() {
        eprintln!("skipping native test `copy-depth`: no C compiler available");
        return;
    }
    let source = "fn main():\n    copy_dir_all(\"deep\", \"copy\")\n";
    let directory = workspace("copy-depth", source);
    let mut deep = directory.join("deep");
    for index in 0..130 {
        deep.push(format!("d{index}"));
    }
    fs::create_dir_all(&deep).expect("create deep copy tree");

    let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("run")
        .arg(directory.join("main.vtx"))
        .current_dir(&directory)
        .output()
        .expect("run copy-depth interpreter");
    assert_eq!(interpreter.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&interpreter.stderr);
    assert!(stderr.contains("error[R0015]"), "{stderr}");
    assert!(stderr.contains("maximum depth"), "{stderr}");
    let _ = fs::remove_dir_all(directory.join("copy"));

    let executable = directory.join("program");
    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(directory.join("main.vtx"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("build copy-depth native");
    assert!(
        build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(&executable)
        .current_dir(&directory)
        .output()
        .expect("run copy-depth native");
    assert_eq!(native.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&native.stderr);
    assert!(stderr.contains("error[R0015]"), "{stderr}");
    assert!(stderr.contains("maximum depth"), "{stderr}");
    let _ = fs::remove_dir_all(directory.join("copy"));
}

#[cfg(unix)]
#[test]
fn recursive_copy_symlink_rules_match_in_both_engines() {
    fn setup_root(directory: &Path) {
        use std::os::unix::fs::symlink;
        let real = directory.join("real");
        fs::create_dir_all(real.join("sub")).expect("create root target");
        fs::write(real.join("sub/data.txt"), "data").expect("write root data");
        symlink(&real, directory.join("root-link")).expect("create root link");
    }

    fn setup_inner(directory: &Path) {
        use std::os::unix::fs::symlink;
        let real = directory.join("real");
        fs::create_dir_all(real.join("sub")).expect("create inner target");
        fs::write(real.join("sub/data.txt"), "data").expect("write inner data");
        fs::create_dir(directory.join("tree")).expect("create inner tree");
        symlink(real.join("sub"), directory.join("tree/link")).expect("create inner link");
    }

    fn setup_dangling(directory: &Path) {
        use std::os::unix::fs::symlink;
        symlink("missing", directory.join("dangling")).expect("create dangling link");
    }

    fn run_case(label: &str, source: &str, setup: fn(&Path), expected_code: i32, needle: &str) {
        if !compiler_available() {
            return;
        }
        let interpreter_directory = workspace(&format!("{label}-interpreter"), source);
        setup(&interpreter_directory);
        let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
            .arg("run")
            .arg(interpreter_directory.join("main.vtx"))
            .current_dir(&interpreter_directory)
            .output()
            .expect("run copy link interpreter");
        assert_eq!(
            interpreter.status.code(),
            Some(expected_code),
            "{label} interpreter stderr: {}",
            String::from_utf8_lossy(&interpreter.stderr)
        );

        let native_directory = workspace(&format!("{label}-native"), source);
        setup(&native_directory);
        let executable = native_directory.join("program");
        let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
            .arg("build")
            .arg(native_directory.join("main.vtx"))
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("build copy link native");
        assert!(
            build.status.success(),
            "{label} native build failed:\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let native = Command::new(&executable)
            .current_dir(&native_directory)
            .output()
            .expect("run copy link native");
        assert_eq!(
            native.status.code(),
            Some(expected_code),
            "{label} native stderr: {}",
            String::from_utf8_lossy(&native.stderr)
        );

        if expected_code != 0 {
            for stderr in [&interpreter.stderr, &native.stderr] {
                let stderr = String::from_utf8_lossy(stderr);
                assert!(stderr.contains("error[R0015]"), "{label}: {stderr}");
                assert!(
                    stderr.contains(needle),
                    "{label}: expected {needle:?} in {stderr}"
                );
            }
        } else {
            assert_eq!(
                String::from_utf8_lossy(&interpreter.stdout),
                String::from_utf8_lossy(&native.stdout),
                "{label}"
            );
            assert_eq!(String::from_utf8_lossy(&native.stdout), "data\n");
        }
    }

    run_case(
        "copy-link-root",
        "fn main():\n\
         \x20   copy_dir_all(\"root-link\", \"copy\")\n\
         \x20   print(read_file(\"copy/sub/data.txt\"))\n",
        setup_root,
        0,
        "",
    );
    run_case(
        "copy-link-inner",
        "fn main():\n    copy_dir_all(\"tree\", \"copy\")\n",
        setup_inner,
        2,
        "symbolic link",
    );
    run_case(
        "copy-link-dangling",
        "fn main():\n    copy_dir_all(\"dangling\", \"copy\")\n",
        setup_dangling,
        2,
        "cannot copy directory",
    );
}

#[cfg(unix)]
#[test]
fn recursive_copy_special_files_match_in_both_engines() {
    use std::os::unix::net::UnixListener;

    if !compiler_available() {
        return;
    }
    let source = "fn main():\n    copy_dir_all(\"tree\", \"copy\")\n";
    let needle = "not a regular file or directory";

    let interpreter_directory = workspace("copy-special-interpreter", source);
    fs::create_dir(interpreter_directory.join("tree")).expect("create special tree");
    let interpreter_socket = UnixListener::bind(interpreter_directory.join("tree/sock"))
        .expect("bind interpreter socket");
    let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("run")
        .arg(interpreter_directory.join("main.vtx"))
        .current_dir(&interpreter_directory)
        .output()
        .expect("run special interpreter");
    drop(interpreter_socket);
    assert_eq!(
        interpreter.status.code(),
        Some(2),
        "interpreter stderr: {}",
        String::from_utf8_lossy(&interpreter.stderr)
    );

    let native_directory = workspace("copy-special-native", source);
    fs::create_dir(native_directory.join("tree")).expect("create native special tree");
    let native_socket =
        UnixListener::bind(native_directory.join("tree/sock")).expect("bind native socket");
    let executable = native_directory.join("program");
    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(native_directory.join("main.vtx"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("build special native");
    assert!(
        build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(&executable)
        .current_dir(&native_directory)
        .output()
        .expect("run special native");
    drop(native_socket);
    assert_eq!(
        native.status.code(),
        Some(2),
        "native stderr: {}",
        String::from_utf8_lossy(&native.stderr)
    );

    for stderr in [&interpreter.stderr, &native.stderr] {
        let stderr = String::from_utf8_lossy(stderr);
        assert!(stderr.contains("error[R0015]"), "{stderr}");
        assert!(stderr.contains(needle), "expected {needle:?} in {stderr}");
    }
}

#[cfg(unix)]
#[test]
fn recursive_copy_non_utf8_names_match_in_both_engines() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    if !compiler_available() {
        return;
    }
    let source = "fn main():\n    copy_dir_all(\"tree\", \"copy\")\n    print(is_dir(\"copy\"))\n";
    let name = OsStr::from_bytes(b"bad-\xff");

    let interpreter_directory = workspace("copy-non-utf8-interpreter", source);
    fs::create_dir(interpreter_directory.join("tree")).expect("create non-utf8 tree");
    fs::write(interpreter_directory.join("tree").join(name), "raw").expect("write non-utf8 file");
    let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("run")
        .arg(interpreter_directory.join("main.vtx"))
        .current_dir(&interpreter_directory)
        .output()
        .expect("run non-utf8 interpreter");
    assert_eq!(
        interpreter.status.code(),
        Some(0),
        "interpreter stderr: {}",
        String::from_utf8_lossy(&interpreter.stderr)
    );
    assert_eq!(
        fs::read(interpreter_directory.join("copy").join(name)).expect("read interpreter copy"),
        b"raw"
    );

    let native_directory = workspace("copy-non-utf8-native", source);
    fs::create_dir(native_directory.join("tree")).expect("create native non-utf8 tree");
    fs::write(native_directory.join("tree").join(name), "raw").expect("write native non-utf8 file");
    let executable = native_directory.join("program");
    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(native_directory.join("main.vtx"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("build non-utf8 native");
    assert!(
        build.status.success(),
        "native build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(&executable)
        .current_dir(&native_directory)
        .output()
        .expect("run non-utf8 native");
    assert_eq!(
        native.status.code(),
        Some(0),
        "native stderr: {}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(
        fs::read(native_directory.join("copy").join(name)).expect("read native copy"),
        b"raw"
    );
}

#[test]
fn try_propagation_matches() {
    assert_same(
        "try-propagation",
        "enum Outcome:\n    Ok(value: Int)\n    Err(message: Str)\n\n\
         fn parse(n: Int) -> Outcome:\n\
         \x20   if n < 0:\n        return Err(\"negative\")\n\
         \x20   return Ok(n * 2)\n\n\
         fn compute(n: Int) -> Outcome:\n\
         \x20   let value = parse(n)?\n\
         \x20   return Ok(value + 1)\n\n\
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
         \x20   print(compute(3))\n\
         \x20   print(compute(-1))\n\
         \x20   print(accumulate([1, 2, 3]))\n\
         \x20   print(accumulate([1, -2, 3]))\n\
         \x20   print(assign(4))\n\
         \x20   print(discard(-5))\n",
    );
}

#[test]
fn try_nested_error_payload_matches() {
    assert_same(
        "try-nested-error",
        "enum Outcome:\n    Ok(value: Int)\n    Err(code: Int)\n\n\
         fn parse(n: Int) -> Outcome:\n\
         \x20   if n < 0:\n        return Err(100 + n)\n\
         \x20   return Ok(n * 3)\n\n\
         fn nested(n: Int) -> Outcome:\n\
         \x20   let value = parse(n)?\n\
         \x20   return parse(value)\n\n\
         fn main():\n\
         \x20   print(nested(2))\n\
         \x20   print(nested(-1))\n",
    );
}

#[test]
fn native_try_requires_a_result_enum() {
    let Some(engines) = run_engines(
        "try-shape-rejected",
        "enum Three:\n    A(x: Int)\n    B(x: Int)\n    C(x: Int)\n\n\
         fn use_three(n: Int) -> Three:\n\
         \x20   let value = A(n)?\n\
         \x20   return A(value)\n\n\
         fn main():\n    print(1)\n",
    ) else {
        return;
    };
    assert_eq!(engines.build.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&engines.build.stderr);
    assert!(stderr.contains("error[E0319]"), "{stderr}");
    assert!(stderr.contains("exactly two variants"), "{stderr}");
}

fn first_error_line(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .find(|line| line.starts_with("error["))
        .unwrap_or_default()
        .to_string()
}

fn assert_error_headlines_match(name: &str, source: &str) {
    let Some(engines) = run_engines(name, source) else {
        return;
    };
    assert!(
        engines.build.status.success(),
        "build failed for `{name}`: {}",
        String::from_utf8_lossy(&engines.build.stderr)
    );
    let native = engines.native.expect("native output");
    assert_eq!(
        engines.interpreter.status.code(),
        Some(2),
        "interpreter did not fail for `{name}`: {}",
        String::from_utf8_lossy(&engines.interpreter.stderr)
    );
    assert_eq!(
        native.status.code(),
        Some(2),
        "native did not fail for `{name}`: {}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(
        first_error_line(&engines.interpreter),
        first_error_line(&native),
        "error headline differs for `{name}`"
    );
}

#[test]
fn runtime_error_headlines_match_interpreter() {
    // Scalar conversion errors include the offending value in both engines.
    assert_error_headlines_match(
        "error-headline-int-float",
        "fn main():\n    print(int(2.5))\n",
    );
    assert_error_headlines_match(
        "error-headline-int-float-huge",
        "fn main():\n    print(int(1e20))\n",
    );
    assert_error_headlines_match(
        "error-headline-int-float-boundary",
        "fn main():\n    print(int(9223372036854775808.0))\n",
    );
    assert_error_headlines_match(
        "error-headline-int-float-infinity",
        "fn main():\n    print(int(1.0 / 0.0))\n",
    );
    assert_error_headlines_match(
        "error-headline-int-float-nan",
        "fn main():\n    print(int(0.0 / 0.0))\n",
    );
    // A missing map key renders a top-level string without quotes.
    assert_error_headlines_match(
        "error-headline-map-key",
        "fn main():\n    var m: Map[Str, Int] = {}\n    print(m[\"missing\"])\n",
    );
    // A `range` loop reports the requested element count, not a native limit.
    assert_error_headlines_match(
        "error-headline-range-limit",
        "fn main():\n    for i in range(0, 10000001):\n        print(i)\n",
    );
    // `range(...)` as a value uses the same checks and messages.
    assert_error_headlines_match(
        "error-headline-range-value-step",
        "fn main():\n    print(range(0, 3, 0))\n",
    );
    assert_error_headlines_match(
        "error-headline-range-value-limit",
        "fn main():\n    print(range(0, 10000001))\n",
    );
    // Filesystem failures carry Rust's `(os error N)` suffix in both engines.
    assert_error_headlines_match(
        "error-headline-remove-missing",
        "fn main():\n    remove_file(\"missing\")\n",
    );
    assert_error_headlines_match(
        "error-headline-make-dir-existing",
        "fn main():\n    make_dir(\".\")\n",
    );
    assert_error_headlines_match(
        "error-headline-rename-missing",
        "fn main():\n    rename(\"missing\", \"target\")\n",
    );
    assert_error_headlines_match(
        "error-headline-copy-missing",
        "fn main():\n    copy_file(\"missing\", \"target\")\n",
    );
    assert_error_headlines_match(
        "error-headline-file-size-missing",
        "fn main():\n    print(file_size(\"missing\"))\n",
    );
    assert_error_headlines_match(
        "error-headline-list-dir-missing",
        "fn main():\n    print(list_dir(\"missing\"))\n",
    );
    assert_error_headlines_match(
        "error-headline-read-directory",
        "fn main():\n    print(read_file(\".\"))\n",
    );
    assert_error_headlines_match(
        "error-headline-write-directory",
        "fn main():\n    write_file(\".\", \"x\")\n",
    );
    // Malformed glob patterns use the shared parser wording.
    assert_error_headlines_match(
        "error-headline-glob-unclosed",
        "fn main():\n    print(glob(\"[\"))\n",
    );
    assert_error_headlines_match(
        "error-headline-glob-unclosed-long",
        "fn main():\n    print(glob(\"[a\"))\n",
    );
    assert_error_headlines_match(
        "error-headline-glob-descending",
        "fn main():\n    print(glob(\"[z-a]\"))\n",
    );
}

#[cfg(unix)]
#[test]
fn file_error_headlines_match_interpreter() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = workspace(
        "error-headline-read-nul",
        "fn main():\n    print(read_file(\"nul.bin\"))\n",
    );
    fs::write(directory.join("nul.bin"), b"a\0b").expect("write nul fixture");
    let engines = run_engines_in_mode(directory, "error-headline-read-nul", true)
        .expect("clang is available");
    assert_eq!(
        first_error_line(&engines.interpreter),
        first_error_line(engines.native.as_ref().expect("native"))
    );
    assert_eq!(
        first_error_line(&engines.interpreter),
        "error[R0015]: file `nul.bin` contains a NUL byte"
    );

    let directory = workspace(
        "error-headline-read-utf8",
        "fn main():\n    print(read_file(\"bad.bin\"))\n",
    );
    fs::write(directory.join("bad.bin"), b"\xff\xfe").expect("write utf8 fixture");
    let engines = run_engines_in_mode(directory, "error-headline-read-utf8", true)
        .expect("clang is available");
    assert_eq!(
        first_error_line(&engines.interpreter),
        first_error_line(engines.native.as_ref().expect("native"))
    );
    assert_eq!(
        first_error_line(&engines.interpreter),
        "error[R0015]: file `bad.bin` contents are not valid UTF-8"
    );

    let directory = workspace(
        "error-headline-read-large",
        "fn main():\n    print(read_file(\"big.bin\"))\n",
    );
    let big = fs::File::create(directory.join("big.bin")).expect("create sparse file");
    big.set_len(64 * 1024 * 1024 + 1).expect("size sparse file");
    let engines = run_engines_in_mode(directory, "error-headline-read-large", true)
        .expect("clang is available");
    assert_eq!(
        first_error_line(&engines.interpreter),
        first_error_line(engines.native.as_ref().expect("native"))
    );
    assert_eq!(
        first_error_line(&engines.interpreter),
        "error[R0015]: file `big.bin` exceeds the 64 MiB read limit"
    );

    let directory = workspace(
        "error-headline-list-dir-utf8",
        "fn main():\n    print(list_dir(\".\"))\n",
    );
    fs::create_dir(directory.join(OsStr::from_bytes(b"bad-\xff"))).expect("create invalid name");
    let engines = run_engines_in_mode(directory, "error-headline-list-dir-utf8", true)
        .expect("clang is available");
    assert_eq!(
        first_error_line(&engines.interpreter),
        first_error_line(engines.native.as_ref().expect("native"))
    );
    assert_eq!(
        first_error_line(&engines.interpreter),
        "error[R0015]: directory `.` contains a name that is not valid UTF-8"
    );
}
