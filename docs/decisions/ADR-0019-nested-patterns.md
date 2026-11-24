# ADR-0019 — Nested payload patterns and matrix exhaustiveness

- Status: Accepted; native lowering added by ADR-0020; native or-patterns
  added by ADR-0021
- Date: 2026-11-24
- Supersedes: none (extends ADR-0011)
- Related: ADR-0008 (native backend boundary), ADR-0011 (enums and pattern
  matching), ADR-0012 (native enum layout), `docs/language_specification.md`,
  `docs/type_system.md`

## Context

ADR-0011 introduced closed enums with positional payload bindings. Nested
payloads were rejected with `E0112` because the original coverage model only
tracked top-level variants and literals. That forced users to bind a payload
and write a second `match` in the arm body, even for common recursive shapes:

```vantyx
match tree:
    Node(Leaf(value), _):
        return value
```

Top-level or-patterns (T-047) made the gap more visible: alternatives could
combine whole variants, but could not inspect nested payloads. The interpreter
is the semantic oracle, so any new pattern form must have exact runtime
matching, checker binding rules, and an explicit native boundary. The checker
must not accept a match that can fail at runtime, and it must not reject every
nested match merely because coverage is hard.

The alternatives were:

1. Keep rejecting nested patterns and require explicit two-stage matches.
2. Desugar nested patterns into synthetic temporaries and extra matches.
3. Add a recursive pattern AST and a constructor-matrix exhaustiveness check,
   while keeping the native backend behind `E0401`.

## Decision

Implement nested payload patterns in the interpreter and checker; keep them
outside the native subset. (The native boundary in point 7 was later lifted by
ADR-0020; the rest of this decision still describes the interpreter/checker
semantics.)

1. **AST shape.** `PatternArg` gains `Nested { pattern, span }` beside `Bind`
   and `Wildcard`. A payload position can recursively contain a literal or
   variant pattern. A bare identifier remains a binding; a zero-payload nested
   variant is written `Empty()` to remove the ambiguity.
2. **Parser bounds.** Payload nesting is parsed recursively with a dedicated
   depth limit of 64 and reports `E0112` when exceeded. Or-patterns remain a
   top-level arm form; a payload position cannot itself be an or-pattern.
3. **Bindings.** Bindings from every nesting level join the arm scope and are
   immutable. Duplicate names across nesting levels are `E0202`. Top-level
   or-pattern alternatives must still bind the same names and types (`E0317`),
   including bindings introduced by nested payloads.
4. **Type checking.** Each nested pattern is checked against its field type.
   Literal/type mismatches are `E0301`; a variant pattern on a scalar is
   `E0315`; an unknown or wrong-enum variant is `E0315`.
5. **Exhaustiveness.** A pattern matrix enumerates finite constructors (`Bool`,
   `Unit`, enum variants) and requires a wildcard/binding for infinite domains
   (`Int`, `Str`, `Any`). Rows are unguarded arms; guards never contribute
   coverage. Or-patterns expand into multiple rows. A wildcard row terminates
   recursive enum expansion. This accepts combinations such as
   `Leaf(value)` + `Node(Leaf(left), _)` + `Node(_, _)` and rejects
   `Node(Leaf(value), _)` alone.
6. **Warnings.** A variant arm marks its variant covered only when every
   payload position is irrefutable for its field type. Duplicate partial nested
   patterns are not reported as unreachable; the checker does not attempt a
   full usefulness analysis.
7. **Native boundary.** At the time of this decision, the IR rejected any
   `PatternArg::Nested` with `E0401` and a help message; no desugaring or
   silent approximation was attempted. ADR-0020 later replaced that boundary
   with recursive native lowering. The native backend continues to support flat
   payload bindings and wildcards.

## Consequences

Positive:

- Recursive data structures can be matched directly without temporary
  variables or nested `match` statements.
- Exhaustiveness is recursive and sound for the supported pattern language,
  including recursive enums and or-pattern alternatives.
- The interpreter remains the oracle, and the native backend fails closed.

Negative:

- The native backend originally could not compile nested patterns; ADR-0020
  later added recursive IR lowering, so this limitation applies only to the
  original T-048 change.
- The matrix algorithm is conservative for shapes outside the pattern language
  and does not perform full nested unreachability/usefulness analysis.
- Deeply nested patterns cost checker time proportional to the pattern matrix;
  the parser bounds depth, but wide variant products can still grow.
- No nested or-patterns, tuple patterns, range patterns, or float patterns.

## Follow-ups

- Lower nested patterns natively by emitting tag tests and a shared binding
  layout, with differential tests against the interpreter. This landed in
  ADR-0020 (T-049).
- Add usefulness-based nested unreachable warnings if real programs need them.
- Extend the matrix to tuple and range patterns only after their runtime
  semantics exist.
