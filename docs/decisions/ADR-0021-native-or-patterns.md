# ADR-0021 — Native or-pattern lowering

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none (extends ADR-0011, ADR-0019, and ADR-0020)
- Related: ADR-0008 (native backend boundary), ADR-0011 (enums and pattern
  matching), ADR-0019 (nested payload patterns), ADR-0020 (native nested
  patterns), `docs/compiler_architecture.md`

## Context

T-047 added top-level or-patterns such as `Circle(size) | Square(size)` to the
parser, checker, and interpreter. The checker requires every alternative to
bind the same names with the same types (`E0317`) and gives the arm one unified
binding set. The native backend rejected every or-pattern with `E0401`, so a
program that ran under `vantyx run` still needed separate arms before it could
be built natively.

The native enum and nested-pattern lowering already represent every binding as
an absolute payload path (`IrFieldPath`). An or-pattern therefore does not need
a new runtime representation: it needs a condition that tests alternatives in
order and a way to initialize the shared bindings from the alternative that
matched.

Alternatives were:

1. Keep the `E0401` boundary and require duplicated arms for native builds.
2. Desugar or-patterns into synthetic match arms, duplicating arm bodies or
   guards.
3. Represent alternatives in `IrPattern` and emit a short-circuit condition
   plus conditional binding initialization.

## Decision

Implement native top-level or-pattern lowering with a shared binding layout.

1. **IR shape.** `IrPattern::Or { alternatives }` stores the lowered
   alternatives. `IrPattern::collect_bindings` recursively visits patterns;
   for an or-pattern, callers that need the shared binding set visit one
   alternative.
2. **Lowering.** Each alternative is lowered against the same expected type and
   payload prefix. The backend validates that every alternative binds exactly
   the same names with the same `IrType`; a mismatch fails closed as an internal
   error rather than emitting partial C.
3. **Conditions.** Codegen joins alternative conditions with `||`, preserving
   source order. If any alternative is irrefutable (`_`, a binding, or a
   single-variant pattern that fully covers its variant), the whole or-pattern
   is irrefutable and needs no condition.
4. **Bindings.** The shared names are declared once and zero-initialized, then
   assigned inside an `if` / `else if` chain whose branches test the same
   alternative conditions. Only the matched alternative's payload paths are
   read. The arm condition already guarantees one branch matches, so the body
   never observes a zero-initialized binding.
5. **Guards.** A guarded or-pattern uses the existing GNU C statement
   expression: evaluate the full or-condition once, initialize the shared
   bindings from the matched alternative, then evaluate the guard once.
6. **Scope.** This covers top-level alternatives in match statements and match
   expressions, including nested payload patterns inside alternatives, scalar
   literals, zero-payload variants, and guards that read shared bindings. The
   parser still does not accept an or-pattern inside a payload position, so
   nested or-patterns remain unsupported.

## Consequences

Positive:

- Native builds now accept the same top-level or-patterns as the interpreter,
  removing another user-visible interpreter/native gap.
- Arm bodies and guards are emitted once; alternatives do not duplicate user
  code.
- No new runtime representation, allocation, or dynamic dispatch is required.

Negative:

- Generated C re-tests alternative conditions when initializing bindings, so a
  pattern with many alternatives emits repeated tag/literal tests. The tests
  are pure and short-circuit, but code size grows with alternatives.
- The IR validator duplicates the checker's binding-compatibility rule as a
  fail-closed defense; both must stay aligned.
- Nested or-patterns, tuple/range/float patterns, and full usefulness-based
  unreachable analysis remain outside the native subset.

## Follow-ups

- Add tuple/range patterns once their runtime semantics exist.
- Add usefulness-based warnings for redundant alternatives.
- Reuse the alternative-condition layout for any future nested or-pattern
  syntax, if the parser admits it.
