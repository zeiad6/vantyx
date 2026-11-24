//! Differential tests: the interpreter and the native C backend must agree.
//!
//! These tests require a C compiler (`clang` by default, override with
//! `VANTYX_CC`). If no compiler is available they skip rather than fail.

use std::fs;
use std::path::PathBuf;
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
    if !compiler_available() {
        eprintln!("skipping native test `{name}`: no C compiler available");
        return None;
    }
    let directory = workspace(name, source);
    let source_path = directory.join("main.vtx");
    let executable = directory.join("program");

    let interpreter = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("run")
        .arg(&source_path)
        .output()
        .expect("run interpreter");

    let build = Command::new(env!("CARGO_BIN_EXE_vantyx"))
        .arg("build")
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("run native build");

    let native = if build.status.success() {
        Some(
            Command::new(&executable)
                .output()
                .expect("run native executable"),
        )
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
