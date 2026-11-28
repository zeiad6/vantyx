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

String escapes: `\n`, `\t`, `\r`, `\\`, `\"`, `\{`, `\}`. `{{` and `}}` produce literal braces. `{expression}` interpolates a value using its display form. Interpolation expressions may not contain an unescaped `}`. Every interpolated expression is evaluated exactly once, in source order, even when its value renders as a constant; a `Unit` expression renders as `()`, but its calls, nested output, mutation, and runtime errors still occur.

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
- Function bodies are lexically scoped: a name resolves through the function's
  own parameters, locals, and nested blocks, then the program's top-level
  bindings. A caller's local bindings are never visible inside a callee, even
  when they share a name with a global (ADR-0040).
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

`for` iterates `List[T]` (values), `Str` (one-character strings), and `Map[K, V]` (keys, insertion order). `break` and `continue` apply to the innermost loop. Iteration snapshots the sequence length at loop start; mutating a list during iteration is allowed but does not extend the iteration. Map iteration snapshots the key sequence at loop start, so removing a key during the loop does not skip later keys and a key removed after the snapshot is still visited; indexing such a removed key raises `R0008` as usual. The snapshot applies in both the interpreter and the native backend.

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
| 1 | call `f(...)`, index `x[i]`, field `x.f`, propagation `x?` | left |
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
`==`/`!=` compare values of the same static type, or any dynamic values. Equality is structural for primitives, strings, lists, maps, structs, and enums (with a recursion depth limit). Two enum values are equal only when their enum names, variants, and payload values are equal. Both operands are evaluated left to right before the comparison, including when static types make the result constant: two `Unit` values are always `==` and never `!=`, but side effects in either operand still occur. `< <= > >=` are defined for `Int`, `Float`, and `Str` (lexicographic).

### 5.4 Logic
`and`/`or` short-circuit and require `Bool` operands. `not` requires `Bool`.

### 5.5 Calls
```vantyx
add(1, 2)
Point(x=1, y=2)
print("value", x)
```

Arguments are evaluated left to right. Arity and statically known types are checked. Named arguments are only valid for struct constructors; enum variant payloads are positional. Named constructor arguments are evaluated in the order written, then placed into declaration order.

### 5.6 Indexing
`xs[i]` on `List[T]` returns `T`; `s[i]` on `Str` returns a one-character `Str`; `m[k]` on `Map[K,V]` returns `V`. Indexes must be `Int`; negative indexes are invalid. Out-of-bounds and missing keys are runtime errors (`R0003`, `R0008`).

### 5.7 Field access
`value.field` requires a struct type; unknown fields are `E0304`. On `Any`, the runtime checks the actual value.

### 5.8 List and map literals
Elements are checked left to right. If all element types unify, the literal has `List[T]`; otherwise `List[Any]`. An expected type from an annotation propagates into the literal. An empty list literal also adopts an expected type from a builtin position when the other arguments determine it: `join`'s `List[Str]`, the needle/value element type of `contains` or `push`, and the key type of `remove`. Otherwise it remains `List[Any]`. Map keys must unify and values must unify; otherwise `Map[Any, Any]`.

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

### 5.10 Result-enum propagation (`?`)

Postfix `?` unwraps the success payload of a nominal result enum, or returns the
failure variant from the enclosing function. It is only valid as the whole value
of a `let` initializer, a plain assignment (`=`, not `+=`), or an expression
statement. Using it inside a larger expression, a condition, a `return` value, a
call argument, or a match scrutinee is `E0319`.

The operand must have an enum type with exactly two variants, each declaring
exactly one payload field. The first declared variant is the success case and the
second is the failure case. The enclosing function must have an explicit return
type that is exactly the operand's enum. On success, the expression has the
success payload's type. On failure, the function returns the failure enum value
unchanged, skipping the rest of the block.

```vantyx
enum Outcome:
    Ok(value: Int)
    Err(message: Str)

fn parse(n: Int) -> Outcome:
    if n < 0:
        return Err("negative")
    return Ok(n * 2)

fn compute(n: Int) -> Outcome:
    let value = parse(n)?      # Int, or return Err(...)
    return Ok(value + 1)
```

The convention is positional, not name-based: an enum whose first variant is
`Err` would treat that variant as success. Variant constructor names are
globally unique, so a program can use `Ok`/`Err` for one result type; additional
result types need distinct variant names. `?` is rejected at top level
(`E0319`), in functions without an explicit return type (`E0319`), when the
operand is not a two-variant one-field enum (`E0319`), and when the enclosing
function's return type differs (`E0319`). The interpreter and native backend
implement the same rule; the native backend returns the failure pointer directly
and binds the success payload field. See
[ADR-0038](decisions/ADR-0038-result-enum-propagation.md).

## 6. Built-in functions (v0.1)

| Signature | Behavior |
| --- | --- |
| `print(...) -> Unit` | Evaluates all arguments left to right, renders them, then writes values separated by spaces and a newline; a failing argument writes no partial line (ADR-0039) |
| `len(x: Str \| List[T] \| Map[K, V]) -> Int` | Length in characters/elements/entries |
| `range(end: Int) -> List[Int]` | `0 .. end-1` |
| `range(start: Int, end: Int) -> List[Int]` | `start .. end-1` |
| `range(start: Int, end: Int, step: Int) -> List[Int]` | Step must be nonzero |
| `push(list: List[T], value: T) -> Unit` | Appends in place; an empty list literal adopts `T` from the value |
| `pop(list: List[T]) -> T` | Removes and returns last element; empty is `R0003` |
| `contains(list: List[T], value: T) -> Bool` | List membership; an empty list literal adopts `T` from the value |
| `contains(text: Str, needle: Str) -> Bool` | Substring test |
| `contains(map: Map[K, V], key: K) -> Bool` | Key membership |
| `remove(map: Map[K, V], key: K) -> Bool` | Removes the first entry whose key is structurally equal to `key`; returns `true` when an entry was removed and `false` when the key is absent; remaining entries keep insertion order |
| `sorted(list: List[T]) -> List[T]` | Returns a new ascending list; `T` is `Int`, `Float`, `Str`, or `Bool`. The input is unchanged. Ordering is stable: equal values keep their input order, `-0.0` and `0.0` compare equal, and every `NaN` sorts after every non-`NaN` (ADR-0050) |
| `split(text: Str, separator: Str) -> List[Str]` | Literal split; empty fields preserved, empty separator splits into scalar values |
| `join(parts: List[Str], separator: Str) -> Str` | Joins parts with a separator; empty list is `""` |
| `trim(text: Str) -> Str` | Removes leading/trailing Unicode `White_Space` |
| `starts_with(text: Str, prefix: Str) -> Bool` | Byte-exact prefix test; empty prefix is `true` |
| `ends_with(text: Str, suffix: Str) -> Bool` | Byte-exact suffix test; empty suffix is `true` |
| `replace(text: Str, pattern: Str, replacement: Str) -> Str` | Replaces every non-overlapping match; empty pattern is `R0016` |
| `keys(map: Map[K, V]) -> List[K]` | Keys in insertion order |
| `values(map: Map[K, V]) -> List[V]` | Values in insertion order |
| `str(x: Any) -> Str` | Display conversion |
| `int(x: Any) -> Int` | Exact numeric/string conversion; strings are trimmed and parsed with the strict decimal grammar; failure is `R0007` |
| `float(x: Any) -> Float` | Numeric/string conversion; strings are trimmed and parsed with the strict decimal, `inf`, or `nan` grammar; failure is `R0007` |
| `bool(x: Any) -> Bool` | Only `Bool` is accepted (`R0007`); the argument is still evaluated first |
| `type(x: Any) -> Str` | Runtime type name |
| `abs(x)`, `min(a, b)`, `max(a, b)` | Numeric helpers; same-type Int or Float. Float `min`/`max` return the second operand when the operands compare equal (so `min(-0.0, 0.0)` is `0.0` and `min(0.0, -0.0)` is `-0.0`), and a single NaN operand yields the other operand (ADR-0039) |
| `sqrt(x: Float) -> Float`, `floor(Float) -> Int`, `ceil(Float) -> Int` | Numeric helpers |
| `assert(cond: Bool) -> Unit` | Runtime error `R0006` if false |
| `read_file(path: Str) -> Str` | Reads a UTF-8 file up to 64 MiB; failures are `R0015` |
| `write_file(path: Str, text: Str) -> Unit` | Creates or truncates a file, then writes UTF-8; failures are `R0015` |
| `write_file_atomic(path: Str, text: Str) -> Unit` | Replaces the file through a same-directory temporary file and rename; failures are `R0015` |
| `append_file(path: Str, text: Str) -> Unit` | Creates if missing and appends UTF-8; failures are `R0015` |
| `file_exists(path: Str) -> Bool` | `true` when `stat` succeeds, otherwise `false` |
| `file_size(path: Str) -> Int` | Follows symbolic links; requires an existing regular file; returns logical `st_size`; failures are `R0015` |
| `is_file(path: Str) -> Bool` | Follows symbolic links; `true` only for a regular file; `false` when metadata cannot be read |
| `is_dir(path: Str) -> Bool` | Follows symbolic links; `true` only for a directory; `false` when metadata cannot be read |
| `is_symlink(path: Str) -> Bool` | Uses `lstat`; `true` when the path itself is a symbolic link, including a dangling link |
| `list_dir(path: Str) -> List[Str]` | Entry names without `.`/`..`, sorted by UTF-8 bytes; failures are `R0015` |
| `glob(pattern: Str) -> List[Str]` | Expands `/`-separated lexical patterns with `*`, `?`, classes, and `**`; results are byte-sorted and deduplicated; malformed patterns are `R0016`, filesystem failures and caps are `R0015` |
| `make_dir(path: Str) -> Unit` | Creates one directory; an existing path is `R0015` |
| `remove_file(path: Str) -> Unit` | Removes a file or symbolic link; directories are rejected with `R0015` |
| `remove_dir(path: Str) -> Unit` | Removes one empty directory; non-empty directories are `R0015` |
| `make_dir_all(path: Str) -> Unit` | Creates a directory and missing parents; an existing directory is accepted; failures are `R0015` |
| `remove_dir_all(path: Str) -> Unit` | Removes a directory tree; symbolic links are unlinked, never followed; refuses `.`, `..`, final `.`/`..` components, and `/` before access; failures are `R0015` |
| `rename(from: Str, to: Str) -> Unit` | Renames or moves one file or directory; failures are `R0015` |
| `copy_file(from: Str, to: Str) -> Unit` | Copies bytes from a regular file to a created/truncated target; failures are `R0015` |
| `copy_dir_all(from: Str, to: Str) -> Unit` | Copies a directory tree into a new target; rejects existing or nested targets, inner symbolic links, and special files; failures are `R0015` |
| `path_join(base: Str, child: Str) -> Str` | Appends a child path; an absolute child replaces the base |
| `path_parent(path: Str) -> Str` | Parent component after trimming trailing `/`, or `.`/`/` |
| `path_name(path: Str) -> Str` | Final path component, or `""` |
| `path_stem(path: Str) -> Str` | Final component without its extension |
| `path_extension(path: Str) -> Str` | Extension after the final dot, or `""` |
| `path_is_absolute(path: Str) -> Bool` | `true` exactly when the path starts with `/` |

All three `contains` forms evaluate their arguments left to right, including in
the native backend. A direct empty list literal as the first argument of
`contains` or `push` takes the element type of the needle or value, and a direct
empty list literal as the key of `remove` takes the map's key type, so
`contains([], 1)`, `push([], 1)`, and `remove(m, [])` for a `Map[List[Int], V]`
build natively without falling back to `List[Any]`; an empty literal whose
element type is still unknown remains `List[Any]`.

`remove` deletes the first entry whose key is structurally equal to the key
argument, preserves the insertion order of the remaining entries, and returns
`true`; a missing key returns `false` and leaves the map unchanged. Both
arguments are evaluated left to right, exactly once, in the interpreter and the
native backend. Removal does not shrink the map's allocated buffers, and
removed keys and values are not reclaimed in v0.1.

`write_file_atomic(path, text)` creates or replaces `path` by writing a hidden
temporary file in the same directory, flushing it with `fsync`, and renaming it
over the target. A concurrent reader observes either the previous file or the
complete new contents, never a partially written file. The temporary file is
removed when any step fails. A symbolic link at `path` is replaced rather than
followed; other hard links to the previous file keep the old contents; and the
replacement receives new-file permissions subject to the process umask, not the
old file's mode, owner, or timestamps. The directory entry is not fsynced, so
the rename is atomic with respect to readers and to process failure during the
write, but it is not a crash-durability guarantee, and a crash can leave a
hidden `.vantyx-tmp-*` file behind. Both arguments are evaluated left to right
exactly once. `write_file` and `append_file` keep their in-place
truncate/append behavior and are not atomic.

`int(text)` and `float(text)` remove leading and trailing Unicode
`White_Space`, then accept exactly the interpreter's strict grammar. For `Int`
that is an optional `+` or `-` followed by one or more ASCII digits, with the
whole remaining string consumed; values outside the signed 64-bit range are
`R0007`. For `Float` it is an optional sign followed by `inf`, `infinity`, or
`nan` case-insensitively, or a decimal mantissa (digits, optionally with one
`.` and digits on either side) optionally followed by `e`/`E`, an optional
sign, and at least one digit. Hexadecimal floats, `nan(payload)`, underscores,
and trailing text are `R0007`. Float overflow produces infinity and underflow
produces zero, as in the interpreter.

`type(x)` reports the runtime type name, but it is an ordinary call: `x` is
evaluated exactly once before the name is produced. Side effects and runtime
errors inside the argument are observable, and the native backend preserves
them.

Built-ins are statically known names and cannot be shadowed in v0.1. File and
directory builtins use host paths with the process's privileges. Relative paths
resolve against the process working directory, not the source file. There is no
sandbox. Reads reject invalid UTF-8, NUL bytes, and files larger than 64 MiB.
`write_file` and `append_file` are not atomic and do not accept NUL bytes in
the path or contents; `write_file_atomic` provides whole-file replacement with
the guarantees and caveats above. `file_exists`, `is_file`, `is_dir`, and `is_symlink` are the
non-failing probes; they return `false` when a path cannot be inspected.
`file_size` follows symbolic links and requires an existing regular file; it
returns the host logical byte size and raises `R0015` for missing paths,
dangling links, directories, other non-regular targets, permission or I/O
failures, and sizes that cannot be represented as `Int`. `is_file` and `is_dir`
follow symbolic links, while `is_symlink` observes the link itself and is
`true` for a link to a file, a link to a directory, and a dangling link. A path
containing NUL is `R0015` in the interpreter; native strings cannot contain
NUL, so a NUL literal is rejected with `E0401` before runtime. No mode, owner,
group, timestamp, ACL, extended attribute, inode, device, birthtime, or
recursive directory size is exposed, and no metadata is modified or preserved.
`glob` splits its pattern on `/` and discards empty components; a leading `/`
makes the pattern absolute. `*` matches zero or more Unicode scalar values and
`?` matches one within a component. `[abc]`, `[a-z]`, and `[!a-z]` classes match
Unicode scalar values; `]` first in a class and `-` first or last are literal.
A whole-component `**` matches zero or more directory levels and descends only
through real directories, never symbolic links. A final `**` returns
non-hidden descendants but not the base. Wildcards and `**` skip names that
begin with `.` unless the component starts with a literal `.` or a positive
class containing `.`. There is no escaping, brace, tilde, case-insensitive, or
normalizing behavior. Missing paths and non-directories produce no matches, as
does an `lstat` failure on a final literal component. Listing or `**` `lstat`
failures, non-UTF-8 entry names, and result or depth limit overruns are
`R0015`. Results are lexical spellings sorted by UTF-8 bytes and
deduplicated. Expansion is capped at 100,000 candidate paths, `**` depth 128,
and 1,024 components; result and depth caps are `R0015` and the component cap is
`R0016`. See [ADR-0033](decisions/ADR-0033-glob.md).
`list_dir` reads the complete directory
into memory and rejects non-UTF-8 entry names. `make_dir` and `remove_dir` are
single-level; `remove_file` removes a file or symbolic link. `make_dir_all`
creates every missing parent and accepts an existing directory, including a
symbolic link to a directory; an empty path is a successful no-op, while a
regular file in the way is `R0015`. `remove_dir_all` removes the whole tree.
It unlinks symbolic links instead of traversing them, including a root link, and
trims trailing separators before the root type check so `link/` is unlinked
rather than followed. Before any filesystem access it refuses `.`, `./`, `..`,
`../`, every root whose final component is `.` or `..` (including `link/.` and
`link/..`), and the filesystem root `/` including repeated-separator spellings;
each refusal is `R0015` with a stable explanation. Ordinary roots such as
`tree`, `tree/`, `./tree`, and `/tmp/tree` remain accepted. It rejects a regular
file, a missing path, or an empty path with `R0015`. A failed recursive removal
can leave a partially removed tree; there is no confirmation, rollback, trash,
depth limit, entry-count limit, or atomic staging. Intermediate components can
still reach through a symbolic link (`link/sub`), and check-then-act races
remain, so callers must validate untrusted roots. The classifier recognizes
`/` separators; Windows interpreter paths that use `\` are not classified, and
Windows native support is unverified. See
[ADR-0035](decisions/ADR-0035-refuse-destructive-removal-roots.md).
`rename` delegates to the host for one file or directory and does not create
parents; overwrite behavior follows the host (POSIX replaces an existing file
target within a filesystem). `copy_file` requires a regular source file and
copies bytes only: permissions, timestamps, ownership, extended attributes,
and hard-link identity are not preserved, and the copied bytes are not
validated as UTF-8 or NUL-free. Source and target must be distinct; a target
that resolves to the source (including a symbolic-link or hard-link alias) is
rejected with `R0015` before the target is opened, because opening it would
truncate the source. The check is best-effort and can still be raced. Copies are
not atomic and a failure can leave a partial target. `copy_dir_all` requires an existing source directory
and a target that does not exist; the target's parent must already exist, and
the final target name cannot be empty, `.`, or `..`. The resolved target must
not equal or lie inside the resolved source, which prevents copying a tree into
itself. It creates the target once, copies regular files byte for byte and
directories recursively, follows symbolic links in the source path including a
final source link, and rejects symbolic links and special files below the
source root with `R0015` instead of following, copying, or skipping them. Entry
names are copied as raw bytes, so non-UTF-8 names are supported. Permissions,
ownership, timestamps, extended attributes, ACLs, hard-link identity, and
sparse layout are not preserved. A failed copy leaves a partial target; there
is no overwrite or merge mode, rollback, atomic staging, cancellation, or
sandbox. More than 100,000 entries or a directory beyond 128 levels below the
source root is `R0015`; the source root is level 1. See
[ADR-0034](decisions/ADR-0034-recursive-copy.md). Metadata objects, recursive
move, streamed or recursive listing, atomic copies, and recoverable
`Result`-based errors are not implemented.

Path builtins are lexical and never access the filesystem. `/` is the only
separator; backslash, drive letters, and UNC prefixes are ordinary text. No
function normalizes `.`/`..` or collapses repeated separators.
`path_join(base, child)` returns `child` when it is absolute, returns the other
side when either argument is empty, keeps a base that already ends in `/`, and
otherwise inserts one `/`. `path_parent` trims trailing separators, then
returns the prefix before the final separator run, or `.` for a relative path
and `/` for an absolute path when no component remains. `path_name` returns the
final component or `""`. `path_stem` and `path_extension` split at the final
dot only when it is neither the first nor the last byte, so `.bashrc` and `a.`
have no extension. When the final dot is first or last, no earlier dot is used
as a fallback, so `a..` has stem `a..` and no extension.
`path_is_absolute` is true exactly when the first byte is
`/`. Results preserve UTF-8 names. These helpers do not confine paths: an
absolute child or a `..` component can escape the base, and callers must
validate untrusted components themselves. Windows path syntax is not supported.
`path_join` evaluates its arguments left to right.

Text builtins operate on Unicode scalar values and UTF-8 bytes, not grapheme
clusters. `split` with a non-empty separator is literal and lossless: empty
fields and a trailing empty field are preserved, and an empty input yields
`[""]`; an empty separator yields one element per Unicode scalar value.
`join` requires `List[Str]`. `trim` removes exactly the Unicode `White_Space`
set (U+0009–U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000–U+200A, U+2028,
U+2029, U+202F, U+205F, U+3000), not zero-width spaces or other format
characters. `replace` is left-to-right and non-overlapping, does not rescan the
replacement, and rejects an empty pattern with `R0016`. There is no regular
expression engine, case conversion, normalization, locale handling, or
grapheme-cluster API. Results are bounded by available memory.

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

Dynamic match failures use `R0014` ("no match arm matched"). `R0005` remains the general dynamic type-boundary error, and `R0010` reports internal or unsupported runtime forms. `R0016` reports an invalid builtin argument value: an empty `replace` pattern, a malformed glob pattern (empty, unclosed or empty class, descending range), or a glob pattern with more than 1,024 components. `R0017` is a defensive error for a `?` operand that is not a two-variant one-field enum or is used outside a supported statement position; the checker normally rejects these cases first. File and directory I/O failures use `R0015`; this includes missing paths, permission errors, directories opened for reading, wrong file/directory kinds, non-regular copy sources, cross-device renames, non-empty directories, regular files blocking recursive creation, refused destructive recursive-removal roots, paths that cannot be removed recursively, non-UTF-8 entry names, invalid UTF-8 contents, NUL bytes, the 64 MiB read limit, glob listing or `**` `lstat` failures, glob result or depth caps, invalid or existing recursive-copy targets, recursive-copy self-containment, inner symbolic links, special files, and recursive-copy entry or depth caps. Output failures use `R0013`; the message includes the host error text and OS error number (for example `failed to write output: No space left on device (os error 28)`), and the location is the `print` call that attempted the write.

## 10. Safety guarantees (v0.1)

Interpreter guarantees:
- no use-after-free, double-free, dangling references, or data races in Vantyx code (single-threaded runtime; Rust ownership backs all values);
- no out-of-bounds access;
- no uninitialized reads;
- checked integer arithmetic;
- no null values;
- no user-visible `unsafe`;
- file reads return only valid UTF-8 without NUL bytes and are capped at 64 MiB;
- metadata probes accept only valid in-memory paths: the interpreter rejects a
  NUL path with `R0015`, while native source strings containing NUL are rejected
  with `E0401`.

Native backend guarantees (supported subset):
- checked integer arithmetic, division, and range operations;
- no uninitialized global reads (`R0009`);
- allocation failure terminates with a runtime error rather than continuing;
- list/string indexes are bounds-checked and missing map keys raise `R0008`;
- list/map allocation growth checks overflow and `SIZE_MAX` (`R0004`);
- native calls are limited to depth 256 (`R0012`), reported at the call site with the same file, line, and column as the interpreter;
- native `print` flushes each completed line and reports a failed write as `R0013` with the same host error text as the interpreter; SIGPIPE is ignored so a closed output pipe is a runtime error rather than signal termination (T-107);
- generated `main` replaces a standard descriptor 0, 1, or 2 that was closed before startup with `/dev/null`, matching Rust's startup sanitization: a closed stdout silently discards `print` output in both engines, and a later file open cannot reuse descriptor 0, 1, or 2 (T-108, ADR-0051);
- structural equality of lists, maps, and structs is cycle-safe and matches the interpreter;
- native file reads enforce the same 64 MiB limit and UTF-8/NUL validation as the interpreter before a string enters the runtime;
- native `copy_file` is an explicit byte-copy boundary: it does not validate UTF-8 or NUL, does not preserve metadata, and is not atomic;
- native `copy_dir_all` requires a new target outside the resolved source, rejects inner symbolic links and special files, copies raw entry-name bytes and file bytes, does not preserve metadata, leaves a partial target on failure, and enforces the same 100,000-entry and 128-depth caps as the interpreter;
- native metadata probes report host `st_size` and POSIX path kinds through `stat`/`lstat`; they do not modify metadata;
- native glob expansion validates entry names as UTF-8, does not descend through symbolic links during `**`, and enforces the same 100,000-candidate, 128-depth, and 1,024-component caps as the interpreter;
- generated code uses only libc/libm and does not expose user `unsafe`.

Known limitations:
- `Rc` reference cycles leak memory in the interpreter (no cycle collector yet);
- native strings, structs, list buffers, map buffers, and scalar-key map indexes are intentionally leaked; native programs have no cycle collector;
- native maps use an insertion-ordered hash index for scalar keys (ADR-0016), but struct/enum/list/map keys still use O(n) structural equality;
- collections are dynamically typed containers after construction in the interpreter; violations produce runtime errors, not memory unsafety;
- resource exhaustion is bounded by explicit limits where implemented (range length, nesting depth, call depth) and otherwise by available memory;
- no sandboxing; file builtins can read and write any path the process can access; `write_file` and `append_file` are not atomic (use `write_file_atomic` for whole-file replacement); do not run untrusted code without OS-level isolation.

## 11. Native compilation (v0.1 subset)

`vantyx build file.vtx [-o output]` lowers the checked program to a typed IR and emits GNU C11, then invokes `clang` (override with `VANTYX_CC`) and links with libc/libm.

Supported: `Unit`, `Int`, `Float`, `Bool`, `Str`, `List[T]` and `Map[K, V]` for statically known type arguments, nominal structs, and nominal enums; globals; functions; `if`/`elif`/`else`; `while`; `for ... in range(...)`, `range(...)` as a `List[Int]` value, `for x in list`, `for c in str`, `for k in map`; `s[i]`; `m[k]`; `match` statements and expressions over statically typed enums and `Int`/`Str`/`Bool` literals and integer ranges; `return`/`break`/`continue`; statement-position `?` propagation for nominal two-variant enums; `print`, `len(Str/List/Map)`, `range` (1–3 Int bounds), `push`, `pop`, `contains(List/Str/Map)`, `remove(Map)`, `sorted`, `keys`, `values`, `str`, `int`, `float`, `bool`, `type`, `abs`, `min`, `max`, `sqrt`, `floor`, `ceil`, `assert`, `read_file`, `write_file`, `write_file_atomic`, `append_file`, `file_exists`, `file_size`, `is_file`, `is_dir`, `is_symlink`, `list_dir`, `glob`, `make_dir`, `remove_file`, `remove_dir`, `make_dir_all`, `remove_dir_all`, `rename`, `copy_file`, `copy_dir_all`, `split`, `join`, `trim`, `starts_with`, `ends_with`, `replace`, `path_join`, `path_parent`, `path_name`, `path_stem`, `path_extension`, `path_is_absolute`. Match guards, nested payload patterns, integer range patterns, and top-level or-patterns are supported. `range(...)` outside a `for` loop builds a `List[Int]` with the same 1–3 `Int` bounds, negative-step, zero-step, and 10,000,000-element limit checks as the interpreter. Native file and directory I/O validates UTF-8 and NUL bytes in C, sorts directory entries by byte order, and enforces the same 64 MiB read limit. Native `write_file_atomic` creates a same-directory hidden temporary file with `open(O_CREAT|O_EXCL)`, writes and `fsync`s it, renames it over the target, and unlinks the temporary on failure; it replaces symbolic links rather than following them and does not preserve metadata. Native recursive creation uses component-by-component `mkdir` with directory confirmation; native recursive removal uses the same lexical destructive-root refusal as the interpreter, then `lstat`, `opendir`/`readdir`, `unlink`, and `rmdir` without following symbolic links, and trims a trailing root separator before the link check. Native metadata probes use POSIX `stat`/`lstat` with the same follow-symlink rules, regular-file rule for `file_size`, and `R0015` failures as the interpreter. Native glob expansion uses POSIX `opendir`/`readdir`/`lstat`, a component matcher over Unicode scalar values, the same hidden and symlink rules, byte sorting, deduplication, limits, and `R0016`/`R0015` error split as the interpreter. Native `rename` delegates to POSIX `rename`; native `copy_file` copies regular-file bytes with libc streams and does not validate or preserve metadata. Native `copy_dir_all` resolves source and target parents with POSIX `realpath`, creates only a new target outside the source, walks with `opendir`/`readdir`/`lstat`, rejects inner symbolic links and special files, copies raw names and bytes with `mkdir` and libc streams, and does not preserve metadata. Native text builtins use the same UTF-8 decoder, Unicode `White_Space` table, literal split rules, and non-overlapping replacement as the interpreter, and evaluate multi-argument calls, including string `contains`, left to right. Native `type` evaluates its argument exactly once before yielding the static type name. Native path builtins use byte-scanned C helpers with the same POSIX-style, non-normalizing rules and ordered `path_join` evaluation as the interpreter. Statically non-`Str` arguments are rejected by the checker with `E0301`; dynamically typed (`Any`) file, directory, glob, rename/copy, metadata, text, and path arguments reach the native boundary and are rejected with `E0401`.

`if`/`elif`/`while` conditions that lower to a GNU C statement expression (for example a call) are wrapped in one extra pair of parentheses so the generated C satisfies the `if` grammar without adding redundant parentheses around comparisons (ADR-0036). Function calls use lexical frames in both engines: a callee resolves names in its own frame and the top-level bindings, never in caller locals (ADR-0040). `and`/`or` preserve the interpreter's short-circuit evaluation: the right operand is evaluated only when the left operand does not decide the result, even when both operands have side effects. Native `print` evaluates every argument into a typed temporary before rendering and writing any of them, so a failing argument leaves no partial line and later mutations are visible in the rendered line (ADR-0039). Native `print` flushes each completed line, checks `fputc`/`fflush`/`ferror`, and reports `R0013` at the print with `strerror` and `(os error N)`; native `main` ignores SIGPIPE so a closed pipe produces the same runtime error instead of terminating the process (T-107). Native `main` also replaces a closed standard descriptor 0, 1, or 2 with `/dev/null` before running user code, matching Rust's startup sanitization so a closed stdout silently discards output and file opens cannot reuse a standard descriptor number (T-108, ADR-0051). Native float `min`/`max` implement the same explicit tie and NaN rule as the interpreter rather than C `fmin`/`fmax` (ADR-0039). Binary operators bind both operands to typed temporaries whenever either operand has effects, so the operands are always evaluated once in source order even when the lowered result is constant; in particular `Unit ==` and `Unit !=` still evaluate both sides (T-082). Interpolation evaluates every expression once; a `Unit` expression is exercised before it renders as `()` (T-083). Native `bool` evaluates its argument exactly once before accepting a `Bool` or raising `R0007` (T-084). The native call-depth guard reports the failing call site, including when the call crosses a module boundary, so `R0012` carries the same file/line/column as the interpreter (T-104).

Enum layout: each enum is a heap-allocated tagged struct with a union of per-variant payload structs; variant declaration order defines the tag. Construction, payload binding, wildcard arms, nested payload patterns, top-level or-patterns, display, equality, and `type` match the interpreter. Nested patterns lower to short-circuit tag and literal tests; every binding is declared from its recorded payload path, and a guard sees all nested bindings. Or-patterns test alternatives in order and initialize the shared binding set from the matched alternative. Integer ranges lower to checked `>=`/`<` or `<=` comparisons. Enum values are never freed in v0.1, like native strings, structs, lists, and maps. See ADR-0012, ADR-0020, ADR-0021, and ADR-0022.

Rejected with `E0401`: `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, unsupported `for` iterables, unannotated value-returning functions, and dynamically typed file, atomic-write, directory, glob, recursive-copy, rename/copy, metadata, text, or path arguments. String index assignment is rejected with `E0306`. Unsupported programs must be run with `vantyx run`.

## 12. Explicitly not implemented yet

Generic enums, explicit discriminants, traits, generics, closures, iterators, a built-in generic `Result[T, E]`, async/concurrency, FFI, package manager, formatter, LSP. Nominal result-enum propagation with `?` is implemented in both engines for `let` initializers, plain assignments, and expression statements (ADR-0038); there is no built-in `Result` type, general expression-position propagation, or error conversion. Basic file I/O is implemented in the interpreter and native backend (ADR-0026), single-level directory listing, creation, and removal (ADR-0027), recursive creation and removal (ADR-0031), rename plus byte-preserving regular-file copy (ADR-0029), metadata probes (ADR-0032), lexical glob expansion (ADR-0033), and recursive directory copy (ADR-0034), but recursive move, metadata objects or preservation, streaming I/O, atomic copies or streaming atomic writes, and recoverable I/O errors are not. Basic UTF-8 text splitting, joining, trimming, affix tests, and literal replacement are implemented in both engines (ADR-0028), but regular expressions, case conversion, normalization, locale handling, and grapheme-cluster APIs are not. Lexical POSIX-style path join/parent/name/stem/extension/absolute helpers are implemented in both engines (ADR-0030), and `glob` performs lexical pattern expansion in both engines (ADR-0033), but there is no `Path` type, normalization, `..` confinement, or Windows path support. Nested payload patterns (T-048/T-049), top-level or-patterns (T-047/T-050), and integer range patterns (T-051) are implemented in the interpreter and native backend. See `TASKS.md`.

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

## 14. Static lints (tooling)

`vantyx lint <file.vtx>` compiles a program and then reports non-fatal warnings from a read-only AST pass. Lints are not part of the language semantics: they never change compilation, interpretation, or native output, and they are not emitted by `vantyx run`, `vantyx check`, or `vantyx build`.

- `L0001`: a local or top-level binding is never read.
- `L0002`: a `var` binding is read but never written after its declaration.
- `L0003`: a statement follows a direct `return`, `break`, or `continue` in the same block.
- `L0004`: a function parameter is never read.

Names starting with `_` are treated as intentionally unused. Loop variables and `match` payload bindings are not reported in this first pass. An unused parameter is reported as `L0004`, distinct from the `L0001` used for locals. `L0003` is deliberately local: it does not report code after an all-terminating `if`/`match` and does not perform interprocedural analysis. A program with only lint findings exits `0`; compilation errors exit `1`. See [linting.md](linting.md) and [ADR-0037](decisions/ADR-0037-static-lints.md).
