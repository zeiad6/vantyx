# ADR-0059: Structural tuple types, destructuring, and tuple patterns

- Status: Accepted; implemented for the interpreter. The native backend rejects
  every tuple construct with `E0401`, so no native code generation changed.
- Date: 2026-11-28
- Related: ADR-0003 (gradual typing), ADR-0019 (nested patterns), ADR-0037
  (lints), ADR-0057 (container element contracts)

## Context

Vantyx had no way to return several values from one function without declaring a
single-use struct, and no way to bind them in one step:

```vantyx
struct Pair:
    first: Int
    second: Str

fn split(n: Int) -> Pair:
    return Pair(first: n, second: "x")
```

Structs remain the right tool for named, mutable, reusable records. They are
poor at local plumbing: every ad-hoc result needs a declaration, a constructor
call, and field access. Pattern matching over such results requires a nominal
variant even when the shape is fixed.

Tuples were the first missing item in the core language roadmap and were absent
from the lexer through the backend (`grep -r Tuple src/` matched nothing). The
interpreter is the semantic oracle, and the native backend has an explicit
`E0401` boundary, so the feature can land interpreter-first without weakening
the no-silent-miscompile rule.

## Decision

Add fixed-arity structural tuples to the interpreter.

### Types

`Type::Tuple(Vec<Type>)` is a structural, positional type. `(Int, Str)` and
`(Int, Str)` are the same type regardless of where they are written. Tuples are
immutable, so assignability is covariant: `(Int, Str)` is assignable to
`(Any, Any)` and `(Int, Any)`, but not to `(Str, Int)` or to a different arity.
`Any` flows in both directions and is validated at runtime, as for every other
type.

`(T,)` is a one-element tuple. The empty type `()` resolves to `Unit`, and no
zero-element tuple value exists. This keeps the existing `Unit` semantics for
`fn main():` and builtins without introducing a second empty type.

### Syntax

- Literals: `(a, b)`, `(a,)`. A parenthesized expression without a comma stays
  grouping: `(a)` is `a`. A bare `()` in expression position is `E0101` with a
  help message pointing at `Unit`.
- Types: `(Int, Str)`, `(Int,)`, nested `(Int, (Bool, Str))`. Trailing commas
  are accepted.
- Element access: `pair.0`, `pair.1`, and chained `nested.1.0`. The index is a
  non-negative integer literal. To make chained access lex without spaces, the
  lexer treats digits immediately after `.` as an integer token; ordinary float
  literals such as `1.5` are unchanged.
- `let`/`var` destructuring: `let (a, (b, c)) = init`, `var (x, _) = init`.
  The pattern grammar is a separate `LetPattern` so existing `let name = ...`
  checking, linting, and REPL completion paths stay intact.
- Match patterns: `(pattern, ...)` may nest literals, ranges, variants, tuple
  patterns, bindings, and `_`. A one-element tuple pattern needs the trailing
  comma.

### Semantics

- Tuple values are immutable. `pair.0 = x` is `E0306` with a help message to
  build a new tuple or use a struct. Through an `Any` alias the same assignment
  is `R0010` at runtime. `var pair = ...` permits rebinding the whole tuple
  only.
- An out-of-range static index is `E0304` with the valid range in the help. On
  `Any`, a non-tuple, out-of-range, or non-numeric access is `R0003`.
- Display uses parentheses, quotes nested strings, and keeps the trailing comma
  for one-element tuples: `(1, "one")`, `(1,)`. `type(tuple)` is `"Tuple"`.
  Equality is structural and never equates a tuple with a list or another kind.
- Conformance is element-wise and recursive, and a successful `Any -> T`
  validation tightens nested container contracts exactly as it does for lists,
  maps, structs, and enum payloads (ADR-0057).
- `let` destructuring checks the initializer statically. A non-tuple initializer
  is `E0301`; an arity mismatch is `E0301`; duplicate names in the pattern are
  `E0202`. With an `Any` initializer the shape is checked at runtime and a
  mismatch is `R0005` before any name is bound. Element bindings take their
  static element types when the initializer type is known, and `Any` otherwise.
- Tuple patterns are checked position by position. A tuple type has exactly one
  constructor of fixed arity, so a pattern whose positions are all bindings,
  wildcards, or irrefutable nested patterns covers the whole tuple. A literal,
  range, or refutable variant makes the pattern refutable; an uncovered tuple
  match is `E0316`, using the same pattern matrix as enums.

### Lints and tooling

Destructured names are declared like ordinary bindings, so `L0001` (unused
binding) and `L0002` (`var` never reassigned) apply. Tuple expressions and
patterns are traversed for the existing lints. REPL completion collects
destructured names.

### Native backend

`src/ir.rs` lowers `Type::Tuple`, `ExprKind::Tuple`, `Pattern::Tuple`, and
`StmtKind::LetPattern` to `E0401` with a concrete message naming the tuple
construct and pointing at `vantyx run`. No C code is generated for them. The
native differential suite asserts the rejection, so a future backend
implementation must replace that test with behavioral parity tests.

## Alternatives considered

1. **Nominal record syntax without declarations**, e.g. `{a: 1, b: 2}`. This
   collides with map literals and needs field-name resolution, ordering, and
   duplicate rules. Structural tuples are smaller and compose with patterns.
2. **Tuple mutation.** Rejected: mutable positional fields make aliasing and
   covariance unsound without invariance, and structs already cover mutation.
3. **Variadic or rest patterns.** Rejected for v0.1: fixed arity keeps
   exhaustiveness and runtime shape checks decidable and simple.
4. **Zero-element tuple distinct from `Unit`.** Rejected: it would duplicate
   `Unit` in returns and builtins without a demonstrated use.
5. **Implementing the native backend in the same change.** Rejected as a
   bounded-task violation. The C representation needs a struct layout, copy and
   display helpers, and differential tests; the `E0401` boundary is honest in
   the meantime.
6. **Reusing `StmtKind::Let` with an optional pattern field.** Rejected because
   it would touch every existing `Let` match arm and risk silent behavior
   changes in unrelated paths.

## Consequences

- Programs can return and destructure small heterogeneous groups without
  declarations, and match on their shape with the existing pattern machinery.
- Tuples are a new type in the checker, IR boundary, value model, display,
  equality, conformance, and lint traversal. Every exhaustive match over `Type`
  must handle `Tuple`; the compiler enforces this, which is why the change is
  larger than the syntax suggests.
- Tuple programs are interpreter-only until the native backend grows a layout.
  This is visible in the diagnostics, documented in the specification, and
  covered by tests rather than silently compiled.
- The lexer change (digits after `.` lex as an integer) is observable only in
  expressions that previously failed to lex or parsed as floats. `1.5` and
  `1.0` still lex as floats because the dot is followed by a digit at the start
  of a numeric literal; the new rule applies to a dot that follows a completed
  expression.
- No reclamation change: tuple values are interpreter `Value`s, and native
  allocation behavior is untouched.

## Reversibility

Medium. The AST, type, and value variants can be removed together; the parser
and checker paths are additive. Keeping the syntax while changing representation
(for example, a native struct layout) does not change user programs.

## Reconsideration criteria

Revisit if tuple mutation, named tuple fields, or tuple destructuring in
parameters and `for` bindings becomes a common request; if the native backend
gains a tuple layout; or if generics make variadic tuples tractable. Any native
implementation must keep the interpreter as the oracle and replace the `E0401`
tests with differential behavior tests.
