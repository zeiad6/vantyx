# ADR-0022 — Integer range patterns

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none (extends ADR-0011, ADR-0019, and ADR-0021)
- Related: ADR-0008 (native backend boundary), ADR-0011 (enums and pattern
  matching), ADR-0019 (nested payload patterns), ADR-0021 (native or-patterns),
  `docs/language_specification.md`

## Context

The pattern language supports `_`, scalar literals, enum variants, nested
payload patterns, and top-level or-patterns. Interval tests still required a
guarded binding:

```vantyx
match score:
    value if value >= 90:
        grade = "A"
    value if value >= 80:
        grade = "B"
    _:
        grade = "F"
```

That form binds a name the body may not need, cannot be nested inside a payload
position without a guard on the outer arm, and cannot participate in an
or-pattern alternative with different intervals. Integer ranges are a small,
well-understood pattern form that removes this friction without introducing a
general range expression, a new runtime type, or a new allocation.

Alternatives were:

1. Keep guards only and document the workaround.
2. Add full range expressions and iterators, then reuse their syntax in
   patterns.
3. Add a pattern-only integer range form with half-open and inclusive
   variants.

## Decision

Add pattern-only integer ranges with `low..high` (half-open) and `low..=high`
(inclusive) syntax.

1. **Lexing and parsing.** `..` and `..=` become tokens. The parser accepts them
   only after an integer pattern literal, including a negative literal, and
   requires an integer literal upper bound. `..` remains unusable in
   expressions, so no range-expression semantics are implied. Both bounds are
   parsed at the same pattern depth as the literal they extend.
2. **AST and IR.** `Pattern::Range { start, end, inclusive }` lowers to
   `IrPattern::Range { start, end, inclusive }`. A range binds no names, so
   binding collection, or-pattern signature validation, and nested payload
   handling are unchanged.
3. **Typing.** A range requires an `Int` or `Any` scrutinee (`E0301`). On `Any`
   the interpreter checks the runtime value is `Int` and in range; other values
   simply do not match. An empty range (`low > high` inclusive, or
   `low >= high` half-open) is `E0318`.
4. **Exhaustiveness.** Ranges are refutable and never contribute coverage.
   `Int` and `Str` matches still require `_` (`E0316`), and nested `Int`
   payloads still require a binding or `_`. The coverage matrix records a range
   as `Opaque`, never as a wildcard, so nested exhaustiveness stays
   conservative.
5. **Matching.** The interpreter tests `actual >= low` and `actual < high` or
   `actual <= high` in source order. Native code emits the same comparison
   against the scrutinee or payload path; integer literals use the existing
   `c_int` formatting, including `INT64_MIN`.
6. **Warnings.** Overlapping ranges are allowed. Only exact duplicate scalar
   literals produce `W0003`; the checker does not attempt interval-overlap or
   full usefulness analysis.
7. **Native boundary.** No new rejection is introduced. A range on a
   non-integer native type fails closed as an internal lowering error if the
   checker is bypassed.

## Consequences

Positive:

- Interval dispatch is readable in match statements, match expressions, nested
  payload positions, and or-pattern alternatives.
- No new runtime representation, allocation, dependency, or dynamic dispatch.
- Interpreter and native backends share one lowering shape and are covered by a
  differential test.

Negative:

- `..` is now a reserved token sequence, so future range expressions must reuse
  it deliberately rather than treat it as punctuation.
- The checker does not prove range coverage, so `Int` matches still need `_`
  even when ranges happen to cover the full domain.
- Overlapping or unreachable intervals are not diagnosed beyond exact duplicate
  literals.
- Float ranges, character ranges, and range bounds that are not literals remain
  unsupported.

## Follow-ups

- Add usefulness-based interval analysis if user demand justifies it.
- Reuse the range syntax for expression-level ranges only with a separate
  decision and runtime semantics.
- Consider tuple patterns only if a tuple type is added to the language.
