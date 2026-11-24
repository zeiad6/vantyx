# ADR-0036: Native condition parentheses and short-circuit evaluation

- Status: Accepted
- Date: 2026-11-24
- Related: T-010, T-070, T-071, ADR-0008, ADR-0016

## Context

The native C backend lowers expressions to GNU C11 expression strings. Two
lowering rules were wrong and were found by randomized differential testing
between the interpreter and the native backend:

1. `IrStmtKind::If` and `IrStmtKind::While` emitted `if {cond} {` and
   `while {cond} {`, where `cond` was expected to carry its own parentheses.
   The `condition` helper treated any rendered string that starts with `(` and
   ends with `)` as already parenthesized. Statement expressions such as
   `({ ... })` and the call wrapper `({ __typeof__(...) ... })` satisfy that
   test, but their parentheses are part of the expression. The result was
   `if ({ ... }) {`, which the C compiler parses as an `if` whose parenthesized
   condition is the compound statement `{ ... }`; clang reports
   `expected expression`. Any function call or short-circuit expression used
   directly as an `if`/`elif`/`while` condition therefore failed to build.

2. `binary_expr` hoisted both operands into temporaries whenever both had
   effects, in order to preserve left-to-right evaluation. That is correct for
   arithmetic and comparisons, but wrong for `and`/`or`: C's `&&` and `||`
   must not evaluate the right operand when the left operand decides the
   result. The hoisted form evaluated both operands unconditionally, so
   `(boom() and bump()) or bump()` called `bump()` one extra time in native
   code while the interpreter skipped it. This was a silent semantic
   divergence, not a build failure.

The interpreter is the semantic oracle (repository rule 1), so both are
correctness defects.

## Decision

- The `condition` helper adds one pair of parentheses around a rendered
  condition only when it is a GNU statement expression (rendered text starts
  with `({`): `if (({ ... }))`. Other conditions keep their existing
  parenthesization, because adding another pair around a comparison triggers
  clang's `-Wparentheses-equality` warning. `if`, `elif`, and `while` all use
  the helper.
- `and`/`or` are lowered separately from other binary operators. When the
  right operand has effects, the backend emits a statement expression that
  evaluates the left operand once into a `bool` and selects the right operand
  lazily with a conditional expression:

  ```c
  ({ bool lhs = LEFT; bool rhs = lhs ? RIGHT : false; rhs; })   /* and */
  ({ bool lhs = LEFT; bool rhs = lhs ? true : RIGHT; rhs; })    /* or  */
  ```

  When the right operand has no effects, the ordinary `&&`/`||` form is used;
  C's short-circuiting is then sufficient because the skipped expression is
  side-effect free.
- Other binary operators keep the existing hoisting rule: both effectful
  operands are evaluated left-to-right into temporaries before the operator is
  applied.

This is a code-generation correction. It does not add syntax, change the type
system, or change the documented `E0401` boundary.

## Consequences

- Ordinary native programs can use calls and `and`/`or` directly in
  `if`/`elif`/`while` conditions.
- Observable side effects from short-circuited operands now match the
  interpreter.
- Conditions are still statement expressions where needed, so the backend
  continues to depend on GNU C11 extensions (`({ ... })`, `__typeof__`) and on
  the `-std=gnu11` flag set by `vantyx build`.
- The effect analysis remains conservative: every `Binary` and `Unary` node is
  treated as effectful, so some pure expressions are hoisted unnecessarily.
  That is a performance detail, not a semantic one.
- Native still rejects `Any`-typed operands with `E0401`; the fix does not
  widen the representable type set.

## Tests

- `tests/native.rs::conditions_with_calls_match` covers a call in `if`, a
  call in `elif`, a call in `while`, a compound `while` condition, and single
  evaluation of the condition.
- `tests/native.rs::short_circuit_operands_match` covers `false and f()`,
  `true or f()`, `true and f()`, `false or f()`, a nested short-circuit
  expression, an `if` condition, and a `match` guard, checking the call count
  after each step.
- `tests/native.rs::effectful_binary_operands_keep_left_to_right_order`
  checks that arithmetic and comparison operands with side effects still
  evaluate left-to-right.
- `tools/differential_fuzz.py` generated the failing programs and is now part
  of the verification workflow.

## Alternatives considered

1. **Bind every condition to a temporary before the `if`.** Rejected for this
   change: it would require statement preludes in several places and change
   more code than the defect requires. The extra parentheses are the minimal
   correct fix.
2. **Avoid statement expressions entirely.** Rejected for now: the backend
   already relies on them for calls, matches, and effectful operators. A full
   rewrite to prelude statements is a larger design change that should be
   benchmarked and reviewed separately.
3. **Evaluate both `and`/`or` operands and document eager evaluation.**
   Rejected: it would contradict the interpreter, which is the semantic
   oracle, and would silently change program behavior.
