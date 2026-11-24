# ADR-0011 — Enums and exhaustive pattern matching

Status: accepted
Date: 2026-11-22
Updated: 2026-11-23 — literal scalar patterns (T-032) and match guards (T-033) added

## Context

Vantyx v0.1 has structs, gradual typing, collections, and a typed native backend
for a static subset. It has no way to model a closed set of alternatives with
payloads. Users must encode alternatives with integers, strings, or structs
plus sentinel fields, which defeats exhaustiveness checking and makes adding a
new case a silent source of bugs.

The prior-art review (`docs/research/prior-art-lessons-2026.md`) found broad
agreement among Rust, Swift, Roc, and Gleam that payload-carrying variants plus
exhaustive matching are a high-value, low-syntax feature. Python and TypeScript
show the opposite risk: when exhaustiveness is only a runtime concern, missing
cases ship unnoticed. The interpreter is the semantic oracle; the native C
backend covers a smaller static subset.

## Problem

Add enums and pattern matching to the language, checker, interpreter, and IR
boundary such that:

- alternatives and payloads are first-class and type-checked
- matching a closed enum is exhaustive or explicitly opts out with `_`
- dynamic (`Any`) matching remains possible and fails predictably
- existing programs and diagnostics keep working
- the native backend never silently miscompiles an unsupported construct
- the design leaves room for nested patterns, generics, and native layout later

## Options considered

### A. Integer/string constants with `match` on literals

Advantages: minimal new syntax; works with existing values.

Disadvantages: no payload typing, no closed set, no exhaustiveness. This is the
status quo that motivates the task.

### B. Structs plus a tag field

Advantages: reuses structs and field access.

Disadvantages: users can construct invalid combinations; the checker cannot
know the set of valid tags; matching remains a chain of `if` statements.

### C. Closed payload enums with exhaustive `match`

Advantages: direct model of alternatives; static exhaustiveness; payload types
are checked; refactoring is compiler-guided; familiar without copying Rust's
ownership model.

Disadvantages: new syntax and checker complexity; adding a variant is a
breaking change for exhaustive matches; native layout must be designed later.

### D. Open/extensible variants (row polymorphism)

Advantages: libraries can add cases without breaking clients.

Disadvantages: exhaustiveness becomes conditional; requires row types or
non-frozen-enum rules; too large for the current type system.

## Decision

Use **option C** for the current pre-release language line (crate 0.1.0).

### Declaration syntax

```vantyx
enum Shape:
    Empty
    Circle(radius: Float)
    Rect(width: Float, height: Float)
```

- `enum` declarations are top-level only, like `struct` and `fn`.
- Variants are indented one per line.
- A variant has zero or more named payload fields with explicit types.
- Variant names are globally unique. They may not collide with functions,
  structs, enums, builtin functions, or builtin type names (`E0202`).
- Enum names may not collide with structs, functions, or builtins (`E0202`).

### Construction

```vantyx
let a = Empty()
let b = Circle(1.5)
let c = Rect(2.0, 3.0)
```

- Construction always uses parentheses, including zero-payload variants.
- Payload arguments are positional and must match the declared arity and types
  (`E0302`, `E0301`). Named payload arguments are rejected (`E0313`).
- The result type is the enum type, not the variant type.

### Patterns and matching

```vantyx
match shape:
    Empty:
        print("empty")
    Circle(radius):
        print("circle {radius}")
    Rect(width, height):
        print("rect {width}x{height}")
```

```vantyx
let label = match shape:
    Empty: "empty"
    Circle(radius): "circle"
    Rect(width, height): "rect"
```

Arms may add a guard:

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

- A match statement has indented block arms.
- A match expression has exactly one expression per arm; all arm types must be
  compatible (`E0301`).
- Patterns are `_`, an integer/boolean/string literal, `Variant`, or
  `Variant(binding, ...)`. A payload position may be `_` to ignore it.
- Literal patterns dispatch on `Int`, `Str`, and `Bool` scrutinees:
  `0`, `-1`, `"ready"`, `true`, and `false`. Float and interpolated-string
  patterns are rejected (`E0113`).
- Nested patterns were rejected with `E0112` in the original decision; this was
  superseded by ADR-0019 (T-048), which adds recursive payload patterns and
  matrix exhaustiveness, and by ADR-0020 (T-049), which lowers them natively.
  Top-level or-patterns are covered by T-047 and native or-pattern lowering by
  ADR-0021 (T-050).
- Duplicate bindings in one pattern are rejected (`E0202`).
- The scrutinee must be an enum, `Int`, `Str`, `Bool`, or `Any` (`E0314`).
- A variant pattern must belong to the matched enum (`E0315`); a variant
  pattern on a scalar scrutinee is also `E0315`.
- A literal pattern on an enum is `E0315`; a literal whose type differs from
  the scrutinee is `E0301`.
- Payload pattern arity must match the variant (`E0302`).
- A pattern may be followed by `if <expr>`. The guard is checked as `Bool`
  (`E0301`), evaluated after the pattern matches, and sees the pattern's
  payload bindings. A false guard continues with the next arm; it does not
  run the body. A dynamically typed non-`Bool` guard raises `R0005`.
- Guard expressions are evaluated in source order with normal expression
  semantics, including calls and global mutation.

### Exhaustiveness

- Matching a closed enum requires an arm for every variant or a final `_`
  (`E0316`).
- A `Bool` match requires both `true` and `false`, or a final `_` (`E0316`).
- An `Int` or `Str` match is open-ended and requires a final `_` (`E0316`);
  the checker does not attempt range or string-set analysis.
- A guarded arm never contributes coverage. A variant, literal, or wildcard
  case that is covered only by guarded arms remains uncovered (`E0316`);
  `_ if cond` is not a fallback.
- A `_` arm after all variants, or after an earlier unguarded `_`, is
  unreachable (`W0003`); duplicate unguarded variant arms and duplicate
  unguarded literal arms are also `W0003`. Repeated guarded arms are not
  warned because a later guard may still succeed.
- Matching `Any` cannot be statically exhaustive. A missing `_` produces
  `W0004`. If no arm matches at runtime, the interpreter raises `R0014`.

### Runtime semantics

- `Value::Enum` stores the enum name, variant name, and ordered payload slots.
- Enum values are immutable: the variant and payload slots cannot be reassigned.
  Payload values may reference mutable lists, maps, or structs; normal alias
  semantics apply.
- Equality is structural across enum name, variant, and payload values.
- Display is `Variant` or `Variant(value, ...)`; payload field names are not
  printed.
- `type(value)` reports the enum name.
- Pattern bindings are immutable in the arm scope.
- A guard is evaluated in the arm scope after bindings are installed and
  before the body. If it fails, the bindings are discarded and matching
  continues. Guard side effects happen exactly once per attempted arm.
- Enum values participate in `Any` conformance nominally by enum name.

### Native backend (implemented in T-030)

Native enums use a heap-allocated tagged struct per enum with a union of
per-variant payload structs, one constructor per variant, and generated
display/equality callbacks. `match` lowers to an ordered tag comparison chain
with payload bindings; match expressions use a GNU C11 statement expression.
The full layout, allocation policy, and rejected cases are documented in
ADR-0012.

- enum types in annotations, parameters, fields, and returns are supported for
  statically known payload types;
- variant constructor calls are supported;
- `match` statements and expressions over closed enums are supported;
- scalar `match` over `Int`, `Str`, and `Bool` with literal and `_` patterns is
  supported;
- guards on wildcard, variant, and literal patterns are supported. A guard
  that reads variant payload bindings is emitted as a GNU C statement
  expression that tests the tag, declares the bindings, and evaluates the
  guard;
- `Any` enum payloads and `match` on `Any` are rejected with `E0401` and a help
  message pointing at `vantyx run`.

## Rationale

- Exhaustiveness is the main value of enums. Making it static catches missing
  cases at compile time, which is the lesson from Rust, Swift, Roc, and Gleam.
- Positional payloads keep construction and patterns symmetric and avoid a
  second named-argument mechanism before the language needs it.
- Globally unique variant names make `Circle(1.0)` unambiguous without type
  annotations and keep the checker simple. This is a deliberate constraint;
  modules or generics may relax it later with qualification.
- Deferring native enums was safer than lowering them to an approximate
  representation that would diverge from interpreter semantics; T-030 later
  landed a tagged layout with differential tests.
- Keeping nested patterns out of the first version bounds parser/checker complexity while
  leaving the AST open to extension.
- Guards were added before nesting because they unlock conditional arm
  selection with a small parser, checker, interpreter, and backend change.
  Treating guarded arms as non-covering keeps exhaustiveness sound without
  reasoning about arbitrary guard expressions.

## Consequences

- Adding a variant to a closed enum breaks exhaustive matches. This must be
  documented as a source-breaking change until a compatibility policy exists.
- `match` is currently the only block-shaped expression. The parser preserves
  statement termination across its closing dedent; future block expressions
  must use the same rule. A match expression cannot appear inside parentheses
  or brackets; that restriction now has a targeted `E0101` diagnostic.
- Native programs support enums for statically known payload types (T-030);
  `Any` payloads and `match` on `Any` still fail at build time with an
  actionable diagnostic rather than at runtime or with wrong output.
- Enum payloads are stored inline in `Value::Enum`, so recursive enum values
  are finite at runtime but can nest deeply. Display and equality remain
  depth-limited like other values.
- The interpreter's enum representation is not the native layout. The IR
  boundary keeps layout decisions reversible.
- Guards can have side effects because they are ordinary expressions. They are
  evaluated in source order and exactly once per attempted arm; the language
  does not promise purity.
- Adding a guard to the only arm for a variant, literal, or wildcard makes the
  match non-exhaustive (`E0316`). This is intentional: the checker cannot prove
  that an arbitrary guard succeeds.
- Unreachability warnings are conservative around guards. A guarded duplicate
  arm is not warned merely for repeating a pattern, because its guard may still
  select it.

## Reversibility

Medium. Source syntax and diagnostics are stable enough to keep. The runtime
representation can change behind `Value::Enum`; the native layout is documented
in ADR-0012 and can change behind the IR. If global variant uniqueness proves
too restrictive, a future module system can introduce qualified constructors
while preserving unqualified names for unambiguous cases.

## Future reconsideration criteria

Revisit when any of the following becomes true:

1. Nested patterns, or-patterns, or guard-visible bindings are needed by real
   programs (new task). Nested patterns are implemented in the interpreter and
   native backend (T-048/T-049); top-level or-patterns are implemented in the
   interpreter and native backend (T-047/T-050).
2. Generics land (T-018): generic enums such as `Option[T]` and `Result[T, E]`.
3. Modules land (T-013): qualified variants and extensible-enum policy.
4. FFI needs explicit discriminants (T-021): decide whether to expose them.
5. Enum allocation dominates a measured native workload (ADR-0012).

## Implementation record (2026-11-22)

- Tokens/lexer: `enum`, `match`.
- AST: `Item::Enum`, `EnumDecl`, `VariantDecl`, `StmtKind::Match`,
  `ExprKind::Match`, `MatchArm`, `MatchArmBody`, `Pattern`, `PatternArg`.
- Types: `Type::Enum`, `EnumInfo`, `VariantInfo`; `resolve_type_expr` accepts
  enums.
- Checker: enum collection, global variant uniqueness, constructor checking,
  pattern scopes, exhaustiveness (`E0316`), wrong-enum/unknown variant
  (`E0315`), non-enum scrutinee (`E0314`), unreachable arms (`W0003`), `Any`
  warning (`W0004`), match-expression arm type agreement (`E0301`), and
  `block_returns` support for match statements.
- Interpreter: `Value::Enum`, construction, pattern matching, `R0014` for no
  matching arm, display/equality/conformance, and `type` reporting.
- IR/native: `E0401` for enum types, variant constructors, and `match` at the
  time of T-014.
- Tests: parser, checker, value, end-to-end, and native-rejection tests; see
  TASKS.md T-014 for the exact commands and counts.
- Documentation: language specification, compiler architecture, memory model,
  roadmap, status, and this ADR.

## Native implementation record (2026-11-23, T-030)

- IR: `IrType::Enum`, `IrEnum`/`IrVariant`, `IrExprKind::ConstructVariant`,
  `IrExprKind::Match`, `IrPattern`, `IrMatchBody`, and payload bindings.
- C backend: one tagged struct per enum, union of per-variant payload structs,
  one constructor per variant, per-enum display/equality callbacks, and type
  descriptors. `match` lowers to an ordered tag comparison chain.
- Rejections: `Any` enum payloads and `match` on `Any` are `E0401`.
- Tests: 8 differential native enum cases and 2 rejection cases; full suite 222
  tests on 2026-11-23 after scalar literal patterns (T-032).
- Layout and memory policy: ADR-0012.

## Guard implementation record (2026-11-23, T-033)

- AST: `MatchArm::guard: Option<Expr>`.
- Parser: `parse_match_arms` accepts `if <expr>` between the pattern and the
  colon in both match forms.
- Checker: the guard is checked as `Bool` after the pattern bindings enter
  scope (`E0301`); `check_pattern` receives `counts = guard.is_none()` so
  guarded arms never satisfy exhaustiveness. Unreachable-arm warnings still
  fire after an unguarded wildcard or covered variant/literal.
- Interpreter: `arm_guard` evaluates the guard in the arm scope after bindings
  are installed. A false guard pops the scope and continues; an error
  propagates with the scope popped.
- IR: `IrMatchArm::guard` is lowered and must have type `Bool`.
- C backend: `arm_condition` combines the pattern condition and guard.
  Variant payload guards use a GNU C statement expression to test the tag,
  declare bindings, and evaluate the guard. Match statements and expressions
  both use the same condition builder and keep the `R0014` fallback for
  unguarded dynamic chains.
- Tests: parser `parses_match_guards`; checker `accepts_match_guards_and_payload_bindings`,
  `rejects_non_bool_match_guard`, `guarded_arms_do_not_satisfy_exhaustiveness`,
  `warns_on_arm_after_unguarded_wildcard_with_guard`,
  `duplicate_guarded_variant_arms_are_not_unreachable`; e2e
  `match_guards_select_arms_at_runtime`; native `match_guards_match`.
- Verification: at the time of T-033, the full suite was 230 tests (126 unit +
  37 e2e + 67 native), clippy clean, debug and release builds succeeded on
  2026-11-23. The suite grew to 241 after T-029.
