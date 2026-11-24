# ADR-0038: Result-enum propagation with postfix `?`

- Status: Accepted
- Date: 2026-11-24
- Related: T-014, T-075, ADR-0001, ADR-0019

## Context

Vantyx has nominal enums with payloads, exhaustive `match`, and early
`return`, but no recoverable error propagation. The specification listed
`Result`/`?` as explicitly not implemented. Writing a fallible function
currently requires every caller to match both variants by hand, and native
code has no shorthand for returning a failure value unchanged.

The language has no generics and no built-in `Result[T, E]`. Variant
constructor names are globally unique, so a built-in `Result` would either
need generic variants or would limit every program to one success and one
failure constructor name. The interpreter is the semantic oracle and
`eval_expr` returns only `Value` or `RuntimeError`, so it cannot carry a
control-flow return out of an arbitrary nested expression without a large
refactor.

## Decision

- Add postfix `?` as a level-1 postfix operator in the parser and a new
  `ExprKind::Try`. It is only accepted as the whole value of:
  - a `let` initializer,
  - a plain assignment right-hand side (`=`, not `+=`),
  - an expression statement.
- Define the result convention positionally: the operand must have a nominal
  enum type with exactly two variants, and each variant must declare exactly
  one payload field. The first declared variant is success; the second is
  failure. The enclosing function must have an explicit return type that is
  exactly the operand's enum.
- On success, `operand?` has the success payload's type. On failure, the
  enclosing function returns the failure enum value unchanged, skipping the
  rest of the block. This applies inside loops and nested blocks.
- Reject every other use with `E0319`: nested expressions, conditions, `return`
  values, call arguments, match scrutinees, top level, unannotated functions,
  a different return type, an enum with the wrong variant or field count, and
  non-enum operands.
- Keep `?` out of general expression lowering. The checker rejects unsupported
  placements, the interpreter has a defensive `R0017`, and the native IR has a
  dedicated `Try` statement rather than an expression form.
- Lower `?` natively to: evaluate the operand once, compare its tag with the
  failure variant's tag, return it directly on failure (decrementing the call
  depth), otherwise bind the success payload field to a fresh local.
- Do not add a built-in generic `Result`, error conversion (`From`), a `try`
  block, or expression-position propagation in this change.

## Consequences

- A program can declare its own result type once and propagate failures with
  `let value = parse(n)?`. Additional result types need distinct variant names
  because variant constructors remain globally unique.
- The convention is positional, not name-based. An enum whose first variant is
  the failure case will treat it as success; this is documented and tested.
- `?` composes with loops, nested functions, early `return`, and existing enum
  matching. The failure value is returned unchanged, so its payload and type
  are preserved.
- Because only statement positions are supported, `f(g()?)` and `if f()?:` are
  compile errors rather than a partial implementation. Supporting them later
  requires either a control-flow-aware expression evaluator or a statement
  prelude rewrite.
- The native backend needs no new runtime type or allocation. The temporary
  operand pointer is returned directly on failure and leaks like every other
  native allocation in v0.1.
- The fuzzer gains a ninth shape that exercises success, failure, loops,
  assignment, and bare-statement propagation in both engines.

## Tests

- `tests/e2e.rs`: success and failure propagation; `let`, assignment, loop,
  and bare-statement positions; nested-expression and top-level rejection;
  wrong variant/field counts; and mismatched or missing return annotations.
- `tests/native.rs`: interpreter/native agreement for the same success,
  failure, loop, assignment, and bare-statement programs, a nested-error
  payload case, and checker rejection of a three-variant enum.
- `tools/differential_fuzz.py::result_program` generates result enums with
  random thresholds, scales, and payloads; 180/180 cases agreed on each of
  seeds 1, 500, and 1000.
- Manual probes confirmed `R0017` defensive behavior is unreachable through
  the normal pipeline and that `E0319` is reported once per invalid use.

## Alternatives considered

1. **Built-in generic `Result[T, E]`.** Rejected: the language has no generics,
   and globally unique variant constructors would allow only one such enum per
   program or force qualified variant names.
2. **Name-based `Ok`/`Err` convention.** Rejected: it would restrict programs
   to a single result type and make the checker depend on global names rather
   than enum structure. Positional semantics works with any variant names.
3. **General expression-position `?`.** Rejected for this change: `eval_expr`
   cannot return a `Flow::Return`, so it would require threading control flow
   through every evaluator and every native expression lowering. A
   statement-position MVP is small, testable, and honest about its limits.
4. **Lower `?` as a match expression in IR.** Rejected: `match_arm_expr`
   requires expression arms, and block arms with `return` would panic or
   require a larger match-lowering change. A dedicated statement is simpler
   and preserves evaluation order.
5. **Implicit error conversion.** Rejected: there is no trait or conversion
   mechanism, and silent conversion would hide payload changes. The failure
   value is returned unchanged.
