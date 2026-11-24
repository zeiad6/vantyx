# ADR-0020 — Native nested-pattern lowering

- Status: Accepted; native or-patterns added by ADR-0021
- Date: 2026-11-24
- Supersedes: none (extends ADR-0011 and ADR-0019)
- Related: ADR-0008 (native backend boundary), ADR-0011 (enums and pattern
  matching), ADR-0012 (native enum layout), ADR-0019 (nested payload patterns),
  `docs/compiler_architecture.md`

## Context

ADR-0019 added recursive payload patterns to the interpreter and checker but
kept the native backend behind `E0401`. That kept native output safe, but it
also meant a program could run under `vantyx run` and fail to build solely
because a common recursive match used `Node(Leaf(value), _)`. The native enum
layout already stores payload fields as typed C values, and child enum fields
are pointers, so the required tag and payload accesses are representable
without a dynamic value layer.

The alternatives were:

1. Keep the `E0401` boundary and require two-stage matches for native builds.
2. Desugar nested patterns into synthetic temporaries and extra match arms.
3. Represent nested patterns recursively in the IR and emit short-circuit tag
   and literal tests plus path-based binding declarations.

## Decision

Implement native nested-pattern lowering with a recursive IR pattern.

1. **IR shape.** `IrPattern` gains `Bind(IrBinding)` and stores a variant's
   fields as `IrPatternField { name, pattern }`. `IrBinding` records an
   `IrFieldPath` list instead of a single field name. Each path step is
   `(variant, field)`, so a binding can name a value at any nesting level.
2. **Lowering.** The IR recursively resolves every variant against the
   expected enum type, validates payload arity, declares every binding in the
   arm scope, and records its absolute payload path. Nested literal patterns
   are validated against their field type. Or-patterns were rejected with
   `E0401` at the time of this decision; ADR-0021 later added native
   top-level or-pattern lowering. The parser still prevents or-patterns inside
   payload positions.
3. **Conditions.** Codegen emits a parent tag test followed by child tests,
   joined with `&&`. C short-circuit evaluation ensures a child payload
   expression is only evaluated after its parent tag matched. Nested literals
   compare the value at the same path.
4. **Bindings.** Bindings are declared once per arm from their recorded paths,
   in source order. No arm body is duplicated. A guard that reads bindings uses
   a GNU C statement expression that first evaluates the full pattern
   condition, then declares the bindings, then evaluates the guard.
5. **Scope.** Native support covers nested variant and literal payload
   patterns, recursive enums, zero-payload nested variants (`Empty()`),
   enum-typed nested bindings, and guards that read nested bindings. It does
   not add tuple/range/float patterns or nested or-patterns. Native
   or-patterns are covered separately by ADR-0021.

## Consequences

Positive:

- Native builds now accept the recursive matches added to the interpreter in
  T-048, removing a user-visible interpreter/native gap.
- The lowering preserves evaluation order and does not duplicate arm bodies or
  guard evaluations.
- Bindings use the existing enum layout and generated field names, so no new
  runtime representation or allocation is introduced.

Negative:

- The IR and codegen are more recursive, and malformed patterns must continue
  to fail closed with internal diagnostics rather than emitting partial C.
- Native or-patterns were still unsupported when this decision landed; ADR-0021
  added them without changing the nested-pattern representation.
- Nested unreachability/usefulness warnings are still conservative; the
  checker does not report every redundant partial nested arm.
- The parser depth limit (64) bounds pattern nesting, but wide variant
  products can still produce large generated conditions.

## Follow-ups

- Lower top-level or-patterns natively with a shared binding layout and
  single-evaluation guards. This landed in ADR-0021 (T-050).
- Add usefulness-based nested unreachable warnings if real programs need them.
- Extend the native subset to tuple and range patterns only after their
  runtime semantics exist.
