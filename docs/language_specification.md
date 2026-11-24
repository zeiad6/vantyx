# Vantyx Language Specification

Version: 0.1 (normative for the current implementation)
Status: implemented subset, with explicitly marked future sections.

Vantyx is an indentation-based, gradually typed language. Programs are checked before execution; annotations are optional, and unannotated values are dynamically checked at runtime.

## 1. Lexical structure

### 1.1 Source encoding
Source files are UTF-8 and use the `.vtx` extension. Identifiers are currently ASCII: `[A-Za-z_][A-Za-z0-9_]*`. Unicode identifiers are planned.

### 1.2 Comments
`#` starts a comment that runs to the end of the line. Comments and blank lines do not produce tokens.

### 1.3 Indentation
Blocks are introduced by `:` followed by a newline and an increased indentation level. Indentation may use spaces only; a tab in leading indentation is error `E0002`. The compiler accepts any consistent number of spaces per level (4 is conventional). Dedentation must return to a previous level; otherwise error `E0001`.

A single simple statement may follow `:` on the same line:

```vantyx
if ready: print("go")
```

### 1.4 Literals

| Kind | Examples | Notes |
| --- | --- | --- |
| Int | `0`, `42`, `1_000`, `0xFF`, `0b1010` | 64-bit signed, checked arithmetic |
| Float | `3.14`, `1.0e9`, `2.5e-3` | 64-bit IEEE-754 |
| Bool | `true`, `false` | |
| Str | `"hello"`, `"x = {x}"` | UTF-8, interpolation |
| List | `[1, 2, 3]`, `[]` | ordered, reference semantics |
| Map | `{"a": 1, "b": 2}`, `{}` | insertion-ordered, reference semantics |

String escapes: `\n`, `\t`, `\r`, `\\`, `\"`, `\{`, `\}`. `{{` and `}}` produce literal braces. `{expression}` interpolates a value using its display form. Interpolation expressions may not contain an unescaped `}`.

### 1.5 Keywords
`fn let var if elif else while for in return break continue struct enum match import from true false and or not`

Reserved for future use: `trait impl as async await nil pub mut`.

### 1.6 Operators and punctuation
`+ - * / // % ** = == != < <= > >= += -= *= /= //= %= ( ) [ ] { } , : . ->`

`and`, `or`, `not` are the logical operators. `&&`, `||`, and `!` are not accepted; diagnostics suggest the keywords.

## 2. Types

### 2.1 Built-in types
- `Int` — signed 64-bit integer.
- `Float` — IEEE-754 binary64.
- `Bool` — `true` / `false`.
- `Str` — immutable UTF-8 string.
- `Unit` — the result of statements and functions that return no value.
- `List[T]` — ordered sequence with reference semantics.
- `Map[K, V]` — insertion-ordered mapping with reference semantics.
- `Any` — gradual dynamic type; operations are checked at runtime.
- Struct names declared with `struct` are nominal types.
- Enum names declared with `enum` are nominal types. Two enums are distinct
  even if their variant names differ only by case or ordering.

### 2.2 Type annotations
Annotations are optional:

```vantyx
let count = 10          # Int
let ratio: Float = 1.5
var names: List[Str] = ["Ada", "Grace"]
fn add(a: Int, b: Int) -> Int:
    return a + b
```

Unannotated function parameters have type `Any`. An unannotated return type is inferred from `return` expressions when they agree; otherwise it is `Any` and runtime checks apply. A mismatch between static types is a compile error, never a silent coercion. There are no implicit numeric conversions; use `float(x)` or `int(x)`.

`List[T]` and `Map[K, V]` are invariant at annotated boundaries except that `T` may flow to `Any`. `Any` accepts any value and defers checking to runtime.

### 2.3 Gradual guarantee
If a value has static type `T` (not `Any`), then every runtime operation on it either behaves according to `T` or terminates with a runtime type error. The runtime never performs an unchecked cast. Dynamic values are validated at annotated function boundaries, annotated bindings, and returns.

## 3. Declarations

### 3.1 Functions
```vantyx
fn name(a: Int, b: Str) -> Bool:
    return true
```

- Functions are declared at top level only (nested functions are planned).
- Parameters are immutable bindings; their contents may be mutated for reference types.
- Recursion is supported.
- Duplicate parameter names and duplicate top-level function names are errors.
- Functions are not overloaded.
- `return` without a value returns `Unit`.
- A function with a non-`Unit` annotated return type must return on every path (`E0309`).
- Closures and first-class functions are not implemented yet.

### 3.2 Structs
```vantyx
struct Point:
    x: Int
    y: Int

let p = Point(x=1, y=2)
let q = Point(3, 4)
```

- Fields are declared one per line with mandatory types.
- The constructor accepts either all named arguments (any order) or all positional arguments (declaration order), not a mix.
- Structs have reference semantics; assignment aliases the same instance.
- Field access `p.x`; assignment `p.x = 3` is allowed. `let` prevents rebinding `p`, not mutation of its fields.
- Duplicate fields and unknown/missing constructor fields are errors.
- Methods, inheritance, and generics on structs are planned.

### 3.3 Enums
```vantyx
enum Shape:
    Empty
    Circle(radius: Float)
    Rect(width: Float, height: Float)

let a = Empty()
let b = Circle(1.5)
let c = Rect(2.0, 3.0)
```

- `enum` declarations are top-level only.
- Variants are declared one per line. A variant has zero or more named payload
  fields with mandatory types.
- Construction always uses parentheses, including zero-payload variants.
- Payload arguments are positional and must match the declared arity and types.
  Named payload arguments are rejected.
- The result type is the enum type; a variant is not a standalone type.
- Variant constructor names are globally unique. They may not collide with
  functions, structs, enums, or builtin names.
- Enum values are immutable: the variant and payload slots cannot be
  reassigned. Payload values may reference mutable lists, maps, or structs;
  normal alias semantics apply.
- Equality is structural across enum name, variant, and payload values.
  `type(value)` reports the enum name. Display is `Variant` or
  `Variant(value, ...)`.
- Recursive payload types are allowed, for example a `Tree` enum with
  `Leaf(value: Int)` and `Node(left: Tree, right: Tree)` variants.
- Methods, generic enums, and explicit discriminants are planned.

## 4. Statements

### 4.1 Bindings
```vantyx
let x = 1        # immutable binding
var y: Int = 2   # mutable binding
y = 3
```

- A binding must be initialized at declaration.
- Assigning to a `let` binding is `E0203`.
- Shadowing is allowed in an inner block; duplicate names in the same block are `E0202`.

### 4.2 Assignment
Targets: variables, fields (`p.x = v`), and indexes (`xs[0] = v`).
Compound assignments `+= -= *= /= //= %=` desugar to the corresponding binary operation and assignment.

### 4.3 Conditionals
```vantyx
if x > 0:
    print("positive")
elif x < 0:
    print("negative")
else:
    print("zero")
```

Conditions must be `Bool`; Vantyx has no truthiness.

### 4.4 Loops
```vantyx
while cond:
    ...

for item in items:
    ...
```

`for` iterates `List[T]` (values), `Str` (one-character strings), and `Map[K, V]` (keys, insertion order). `break` and `continue` apply to the innermost loop. Iteration snapshots the sequence length at loop start; mutating a list during iteration is allowed but does not extend the iteration.

### 4.5 Return, break, continue
`return [expr]`, `break`, `continue`. Using `break`/`continue` outside a loop or `return` outside a function is a compile error.

### 4.6 Match statements
```vantyx
match shape:
    Empty:
        print("empty")
    Circle(radius):
        print("circle {radius}")
    Rect(width, height):
        print("rect {width}x{height}")
```

Guards refine an arm after its pattern matches:

```vantyx
match shape:
    Circle(radius) if radius > 10:
        print("big circle")
    Circle(radius):
        print("circle {radius}")
    Rect(width, height) if width == height:
        print("square")
    Rect(width, height):
        print("rect {width}x{height}")
```

Top-level or-patterns match any alternative and expose one shared binding set:

```vantyx
match shape:
    Circle(size) | Square(size) if size > 5:
        print("large shape with size {size}")
    Circle(size) | Square(size):
        print("small shape with size {size}")
```

- The scrutinee must have an enum, `Int`, `Str`, `Bool`, or `Any` type.
- Arms are indented blocks. Patterns are `_`, an integer/boolean/string
  literal, an integer range, `Variant`, `Variant(pattern, ...)`, or a top-level
  alternation of those forms separated by `|`; `_` may ignore a payload
  position. A payload position may recursively contain a literal, range, or
  variant pattern, such as `Node(Leaf(value), _)`. A bare payload identifier is
  a binding; write `Empty()` to match a zero-payload nested variant. See
  [ADR-0019](decisions/ADR-0019-nested-patterns.md).
- Integer range patterns use `low..high` (half-open: `low <= x < high`) or
  `low..=high` (inclusive: `low <= x <= high`). Both bounds are integer
  literals and may be negative. A range requires an `Int` or `Any` scrutinee
  (`E0301`); an empty range is `E0318`. Ranges are refutable and never make an
  `Int` match exhaustive, so an `Int` match still needs `_`. Overlapping ranges
  are allowed and are tested in source order; only exact duplicate literals
  produce `W0003`. See [ADR-0022](decisions/ADR-0022-range-patterns.md).
- Payload bindings are immutable in the arm scope.
- Every alternative of an or-pattern must bind the same names with the same
  types (`E0317`). An arm receives that single binding set regardless of which
  alternative matched. For example, `Circle(size) | Square(size)` is valid;
  `Circle(radius) | Square(side)` is rejected because the names differ.
- An unguarded or-pattern contributes coverage for every alternative, so
  `Circle(size) | Square(size)` covers both variants. A guarded or-pattern
  contributes no coverage, exactly like any other guarded arm.
- Native builds lower top-level or-patterns with a shared binding layout:
  alternatives are tested in source order, bindings are initialized from the
  matched alternative, and guards evaluate once. See
  [ADR-0021](decisions/ADR-0021-native-or-patterns.md).
- `|` is a pattern separator only. It is not a bitwise or boolean operator; use
  `or` for boolean expressions.
- A pattern may be followed by `if <expr>`. The guard must have type `Bool`
  (`E0301`); a dynamically typed non-`Bool` guard raises `R0005` at runtime.
  The guard is evaluated only after the pattern matches, with the pattern's
  payload bindings in scope. If it is false, matching continues with the next
  arm; the body does not run.
- Guard expressions are evaluated in source order and only until an arm body
  runs. A guard may call functions and observe or mutate global state.
- A guarded arm never counts as coverage for exhaustiveness. Any variant,
  literal, or wildcard case that is covered only by guarded arms remains
  uncovered (`E0316`); add an unguarded arm. `_ if cond` is not a fallback.
- A match on a closed enum must cover every variant and every nested payload
  shape, or end with `_`. Exhaustiveness is checked recursively over the
  pattern matrix: `Leaf(value)` plus `Node(Leaf(left), _)` plus `Node(_, _)`
  can be exhaustive, while `Node(Leaf(value), _)` alone is not. A
  non-exhaustive match is `E0316`.
- A `Bool` match must cover both `true` and `false` or end with `_` (`E0316`).
  An `Int` or `Str` match is open-ended and must end with `_` (`E0316`), even
  when range patterns cover large intervals. A nested `Int`/`Str` payload
  likewise requires a binding or `_` for full coverage.
- Literal patterns must have the same type as the scrutinee (`E0301`).
  A literal pattern on an enum is `E0315`; a variant pattern on a scalar is
  `E0315`.
- Matching `Any` cannot be checked for exhaustiveness; without `_` the compiler
  emits `W0004`. If no arm matches at runtime, the interpreter raises `R0014`.
- A variant pattern must belong to the matched enum (`E0315`); unknown
  variants and non-enum scrutinees are errors (`E0315`, `E0314`).
- Nested payload patterns may be up to 64 levels deep (`E0112` beyond that).
  Or-patterns are supported only at the top level of an arm; alternatives may
  contain nested payload patterns, but an or-pattern cannot itself appear
  inside a payload position.
- Unreachable arms (after an unguarded `_`, a duplicate unguarded variant, or
  a duplicate unguarded literal) produce `W0003`. A variant arm is only
  considered complete when every payload position is irrefutable; duplicate
  partial nested patterns are not reported. Repeated guarded arms do not warn
  because their guards may still select them.
- A match statement whose arms all return satisfies the function's return-path
  check.

## 5. Expressions

### 5.1 Precedence (highest to lowest)
| Level | Operators | Associativity |
| --- | --- | --- |
| 1 | call `f(...)`, index `x[i]`, field `x.f` | left |
| 2 | unary `-`, `not` | right |
| 3 | `**` | right |
| 4 | `* / // %` | left |
| 5 | `+ -` | left |
| 6 | `== != < <= > >=` | left |
| 7 | `and` | left |
| 8 | `or` | left |

### 5.2 Arithmetic
- `+ - * // %` on `Int` are checked; overflow is runtime error `R0002`.
- `/` always performs floating-point division and returns `Float`.
- `//` is floor division for `Int`; division by zero is `R0001`.
- `%` is remainder for `Int` and `Float`; division by zero is `R0001`.
- `**`: `Int ** Int` (non-negative exponent, checked) → `Int`; `Float ** Float` → `Float`.
- `+` concatenates `Str + Str` only.
- Mixing `Int` and `Float` is a compile error when statically known (`E0306`), and a runtime type error when dynamic.
- Unary `-` applies to `Int` and `Float`.

### 5.3 Comparison and equality
`==`/`!=` compare values of the same static type, or any dynamic values. Equality is structural for primitives, strings, lists, maps, structs, and enums (with a recursion depth limit). Two enum values are equal only when their enum names, variants, and payload values are equal. `< <= > >=` are defined for `Int`, `Float`, and `Str` (lexicographic).

### 5.4 Logic
`and`/`or` short-circuit and require `Bool` operands. `not` requires `Bool`.

### 5.5 Calls
```vantyx
add(1, 2)
Point(x=1, y=2)
print("value", x)
```

Arguments are evaluated left to right. Arity and statically known types are checked. Named arguments are only valid for struct constructors; enum variant payloads are positional.

### 5.6 Indexing
`xs[i]` on `List[T]` returns `T`; `s[i]` on `Str` returns a one-character `Str`; `m[k]` on `Map[K,V]` returns `V`. Indexes must be `Int`; negative indexes are invalid. Out-of-bounds and missing keys are runtime errors (`R0003`, `R0008`).

### 5.7 Field access
`value.field` requires a struct type; unknown fields are `E0304`. On `Any`, the runtime checks the actual value.

### 5.8 List and map literals
Elements are checked left to right. If all element types unify, the literal has `List[T]`; otherwise `List[Any]`. An expected type from an annotation propagates into the literal. Map keys must unify and values must unify; otherwise `Map[Any, Any]`.

### 5.9 Match expressions
```vantyx
let label = match shape:
    Empty: "empty"
    Circle(radius): "circle {radius}"
    Rect(width, height): "rect"
```

- A match expression has exactly one expression per arm, on the same line as
  the pattern.
- Every arm must produce a compatible type (`E0301`).
- Exhaustiveness, guard, and pattern rules are the same as for match
  statements. A guard follows the pattern before the colon:
  `Circle(radius) if radius > 0: "positive"`.
- Or-patterns also work in match expressions, including with a shared guard:

```vantyx
return match shape:
    Circle(size) | Square(size) if size > 5: "large"
    Circle(size) | Square(size): "small"
```
- Literal patterns dispatch on `Int`, `Str`, and `Bool` values:

```vantyx
fn classify(code: Int) -> Str:
    return match code:
        0: "ok"
        -1: "unknown"
        _: "other"
```

- The expression must be the final element of its statement or arm; a match
  expression closes at its dedent.
- Nested match expressions and nested payload patterns are allowed. A
  top-level or-pattern may combine whole alternatives that themselves contain
  nested payload patterns.
- Because arms require indentation, a match expression cannot appear inside
  parentheses, brackets, or a one-line suite; newlines are suppressed there.
  Bind the result to a variable first. This case reports `E0101` with an
  actionable help message.

## 6. Built-in functions (v0.1)

| Signature | Behavior |
| --- | --- |
| `print(...) -> Unit` | Writes values separated by spaces, then newline |
| `len(x: Str \| List[T] \| Map[K, V]) -> Int` | Length in characters/elements/entries |
| `range(end: Int) -> List[Int]` | `0 .. end-1` |
| `range(start: Int, end: Int) -> List[Int]` | `start .. end-1` |
| `range(start: Int, end: Int, step: Int) -> List[Int]` | Step must be nonzero |
| `push(list: List[T], value: T) -> Unit` | Appends in place |
| `pop(list: List[T]) -> T` | Removes and returns last element; empty is `R0003` |
| `contains(list: List[T], value: T) -> Bool` | List membership |
| `contains(text: Str, needle: Str) -> Bool` | Substring test |
| `contains(map: Map[K, V], key: K) -> Bool` | Key membership |
| `keys(map: Map[K, V]) -> List[K]` | Keys in insertion order |
| `values(map: Map[K, V]) -> List[V]` | Values in insertion order |
| `str(x: Any) -> Str` | Display conversion |
| `int(x: Any) -> Int` | Exact numeric/string conversion; failure is `R0007` |
| `float(x: Any) -> Float` | Numeric/string conversion; failure is `R0007` |
| `bool(x: Any) -> Bool` | Only `Bool` is accepted (`R0007`) |
| `type(x: Any) -> Str` | Runtime type name |
| `abs(x)`, `min(a, b)`, `max(a, b)` | Numeric helpers; same-type Int or Float |
| `sqrt(x: Float) -> Float`, `floor(Float) -> Int`, `ceil(Float) -> Int` | Numeric helpers |
| `assert(cond: Bool) -> Unit` | Runtime error `R0006` if false |

Built-ins are statically known names and cannot be shadowed in v0.1.

## 7. Program entry

- Top-level statements execute in order.
- If a program has no top-level statements and declares `fn main()` with zero parameters, `main` is called automatically.
- If both top-level statements and `main` exist, top-level statements run and `main` is not called automatically (a warning is emitted).
- Top-level `return`/`break`/`continue` are errors.

## 8. Modules and imports

Vantyx v0.1 has file-based modules. An import is a declaration, so imports
appear at the top level of a file:

```vantyx
import "lib/math.vtx"
from "lib/math.vtx" import Point, add
```

- Import paths are plain string literals ending in `.vtx`. Interpolation,
  escapes, backslashes, empty paths, and absolute paths are rejected.
- A relative path is resolved from the directory of the importing file, then
  canonicalized. The target must remain under the entry file's directory;
  `..` traversal that escapes that root is rejected.
- `import "path"` makes every declaration in the target file visible.
  `from "path" import name, ...` makes only the listed declarations visible.
  Each name may appear once. There are no aliases, namespaces, re-exports, or
  module initializers; imported declarations share the program's global scope,
  and duplicate names are ordinary checker errors reported at the imported
  declaration.
- Every module is parsed once even if it is imported by several files.
  Dependencies of a selective import are always loaded, because selected
  declarations may call them.
- A non-entry module may contain only imports and top-level `fn`, `struct`,
  and `enum` declarations. Executable top-level statements are `E0505`.
  Defining `main` in a non-entry module is `E0506`; `main` belongs to the entry
  file.
- The entry file may contain executable top-level statements. Imported
  declarations are placed before the entry file's own items, preserving each
  file's source order.
- The interpreter and the native backend consume the same flattened program.
  `vantyx run -` and the REPL reject imports with `E0510`.

Resource limits per module graph: 256 files, import depth 64, 1 MiB per module,
and 8 MiB of total module source. The entry file counts toward these limits.

| Code | Meaning |
| --- | --- |
| `E0501` | Cannot read or resolve an imported file, wrong extension, or not a regular file |
| `E0502` | Absolute path or a path outside the entry file's directory |
| `E0503` | Import cycle, with the chain in a note |
| `E0504` | `from ... import name` where the module does not declare `name` |
| `E0505` | Executable statement in a non-entry module |
| `E0506` | `main` defined in a non-entry module |
| `E0507` | Module graph exceeds 256 files |
| `E0508` | Import depth exceeds 64 |
| `E0509` | Module or total module source exceeds the byte limits |
| `E0510` | Import used through stdin or the REPL |
| `E0511` | Duplicate name in one selective import |
| `E0512` | Empty, interpolated, escaped, or otherwise non-literal import path |

## 9. Runtime errors

Runtime errors print `error[Rxxxx]`, a source span, a message, and a call stack, then exit with status 2. Compile errors exit with status 1. Usage errors exit with status 64.

Dynamic match failures use `R0014` ("no match arm matched"). `R0005` remains the general dynamic type-boundary error, and `R0010` reports internal or unsupported runtime forms.

## 10. Safety guarantees (v0.1)

Interpreter guarantees:
- no use-after-free, double-free, dangling references, or data races in Vantyx code (single-threaded runtime; Rust ownership backs all values);
- no out-of-bounds access;
- no uninitialized reads;
- checked integer arithmetic;
- no null values;
- no user-visible `unsafe`.

Native backend guarantees (supported subset):
- checked integer arithmetic, division, and range operations;
- no uninitialized global reads (`R0009`);
- allocation failure terminates with a runtime error rather than continuing;
- list/string indexes are bounds-checked and missing map keys raise `R0008`;
- list/map allocation growth checks overflow and `SIZE_MAX` (`R0004`);
- native calls are limited to depth 256 (`R0012`);
- structural equality of lists, maps, and structs is cycle-safe and matches the interpreter;
- generated code uses only libc/libm and does not expose user `unsafe`.

Known limitations:
- `Rc` reference cycles leak memory in the interpreter (no cycle collector yet);
- native strings, structs, list buffers, map buffers, and scalar-key map indexes are intentionally leaked; native programs have no cycle collector;
- native maps use an insertion-ordered hash index for scalar keys (ADR-0016), but struct/enum/list/map keys still use O(n) structural equality;
- collections are dynamically typed containers after construction in the interpreter; violations produce runtime errors, not memory unsafety;
- resource exhaustion is bounded by explicit limits where implemented (range length, nesting depth, call depth) and otherwise by available memory;
- no sandboxing; do not run untrusted code without OS-level isolation.

## 11. Native compilation (v0.1 subset)

`vantyx build file.vtx [-o output]` lowers the checked program to a typed IR and emits GNU C11, then invokes `clang` (override with `VANTYX_CC`) and links with libc/libm.

Supported: `Unit`, `Int`, `Float`, `Bool`, `Str`, `List[T]` and `Map[K, V]` for statically known type arguments, nominal structs, and nominal enums; globals; functions; `if`/`elif`/`else`; `while`; `for ... in range(...)`, `for x in list`, `for c in str`, `for k in map`; `s[i]`; `m[k]`; `match` statements and expressions over statically typed enums and `Int`/`Str`/`Bool` literals and integer ranges; `return`/`break`/`continue`; `print`, `len(Str/List/Map)`, `push`, `pop`, `contains(List/Str/Map)`, `keys`, `values`, `str`, `int`, `float`, `bool`, `type`, `abs`, `min`, `max`, `sqrt`, `floor`, `ceil`, `assert`. Match guards, nested payload patterns, integer range patterns, and top-level or-patterns are supported.

Enum layout: each enum is a heap-allocated tagged struct with a union of per-variant payload structs; variant declaration order defines the tag. Construction, payload binding, wildcard arms, nested payload patterns, top-level or-patterns, display, equality, and `type` match the interpreter. Nested patterns lower to short-circuit tag and literal tests; every binding is declared from its recorded payload path, and a guard sees all nested bindings. Or-patterns test alternatives in order and initialize the shared binding set from the matched alternative. Integer ranges lower to checked `>=`/`<` or `<=` comparisons. Enum values are never freed in v0.1, like native strings, structs, lists, and maps. See ADR-0012, ADR-0020, ADR-0021, and ADR-0022.

Rejected with `E0401`: `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, unsupported `for` iterables, and unannotated value-returning functions. String index assignment is rejected with `E0306`. Unsupported programs must be run with `vantyx run`.

## 12. Explicitly not implemented yet

Generic enums, explicit discriminants, traits, generics, closures, iterators, `Result`/`?`, async/concurrency, FFI, package manager, formatter, LSP. Nested payload patterns (T-048/T-049), top-level or-patterns (T-047/T-050), and integer range patterns (T-051) are implemented in the interpreter and native backend. See `TASKS.md`.

## 13. Interactive REPL (tooling)

The REPL is a tooling surface, not a separate language mode. It compiles and executes the same language with the interpreter; declarations persist across entries.

- A single-line entry executes immediately. A line ending in `:` starts a block; a blank line ends it.
- Declarations and bindings from earlier entries remain visible. Redefining a name is an error.
- Failed entries are rolled back: diagnostics are shown, but the failed entry does not change the accumulated program. It remains available in history for editing.
- A new top-level expression statement prints its value once unless the value is `Unit`. Entries replayed from earlier history are muted and are never echoed. Values use `display()`, so strings are unquoted.
- On a terminal, Tab completes keywords, builtins, and top-level names; Up/Down recall single-line history; Ctrl-C cancels the current entry; Ctrl-D exits. Non-TTY input and systems without `stty` fall back to canonical line input.
- History is stored in `$VANTYX_HISTORY` when set, otherwise `$HOME/.vtx_history`, with mode 0600 on Unix. `VANTYX_HISTORY=off` disables persistence. Multi-line blocks are session-only for recall.
- `:help`, `:history`, `:names`, and `:quit` are REPL meta-commands, not Vantyx syntax.

See [ADR-0013](decisions/ADR-0013-repl-line-editing.md).
