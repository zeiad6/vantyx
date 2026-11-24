# ADR-0003: Gradual typing with `Any` and runtime boundary checks

- Status: Accepted
- Date: 2026-11-21

## Context
Beginners should write `fn add(a, b): return a + b` without annotations, while production code needs static guarantees.

## Options
1. Fully static with mandatory annotations — safe but hostile to prototyping.
2. Fully dynamic — simple but no compile-time guarantees or optimization.
3. Gradual typing with an explicit `Any` type and runtime checks at typed boundaries.
4. Type inference only (Hindley–Milner) — powerful but complex errors and no dynamic escape hatch.

## Decision
Option 3. Unannotated parameters are `Any`; annotations are checked statically; `Any` operations are checked dynamically; every `Any → T` boundary is validated at runtime. No implicit numeric coercion.

## Rationale
- Supports both example styles in the mission without separate dialects.
- Preserves fail-stop safety: a wrong dynamic value cannot silently enter a typed position.
- Enables progressive optimization: annotated code can be specialized later.

## Consequences
- Runtime overhead for dynamic checks.
- Diagnostics must explain when to add annotations.
- Type soundness depends on complete boundary validation (tested).
- Generics and flow narrowing will require a more sophisticated checker later.

## Reversibility
Medium. The AST already carries annotations and expression IDs; the checker can be strengthened without changing syntax.

## Reconsideration criteria
If dynamic checks dominate real workloads, introduce an opt-in strict mode and specialization; do not remove gradual typing.
