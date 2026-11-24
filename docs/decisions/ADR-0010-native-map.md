# ADR-0010 — Native `Map` representation

Status: accepted
Date: 2026-11-22

## Context

The interpreter supports `Map[K, V]` with insertion order, structural key
equality, reference semantics, and `R0008` for missing keys. The native C
backend rejects every `Map` with `E0401`, so a large class of ordinary programs
cannot be compiled. Native lists already use a type descriptor plus an
`VantyxList` buffer; maps need a representation that preserves interpreter
semantics and fits the same descriptor mechanism.

## Problem

Choose a native map representation that is:

- semantically identical to the interpreter for the supported static subset
- implementable without third-party code or a runtime library
- safe for arbitrary statically typed keys (including structs and lists)
- auditable and testable by differential tests
- replaceable later without changing the language or IR contract

## Options considered

### A. Hash table with open addressing

Advantages: expected O(1) lookup and insertion; familiar.

Disadvantages: requires a hash function per key type; structural keys need
recursive hashing; collision behavior and iteration order must be specified;
adversarial keys can cause worst-case behavior unless hashing is randomized;
more generated runtime code to audit.

### B. Balanced tree (ordered map)

Advantages: O(log n), deterministic order, no hashing.

Disadvantages: iteration order becomes sorted rather than insertion order,
which diverges from the interpreter; comparators for structural keys are more
complex than equality.

### C. Insertion-ordered parallel arrays with linear search

Advantages: exact interpreter semantics (insertion order, structural equality),
no hash function, works with the existing `VantyxTypeDesc` equality callbacks,
small and auditable runtime, trivial cycle/depth handling. Keys and values are
stored in two parallel buffers.

Disadvantages: O(n) lookup/insert/contains; unsuitable for very large maps.
A later ordered hash index can be added behind the same runtime API.

## Decision

Use **option C** for the first native map implementation.

- `VantyxMap` stores `len`, `cap`, `key_size`, `value_size`, key/value
  descriptors, and parallel `keys`/`values` buffers.
- Lookup scans keys with `vantyx_value_equals`, the same structural equality used
  by lists and structs.
- Assigning an existing key replaces the value in place and keeps its original
  position; a new key is appended.
- Iteration visits keys in insertion order, matching `for k in map` in the
  interpreter.
- `len`, `contains`, `keys`, `values`, indexing, assignment, display, and
  equality are supported for statically known key/value types.
- `Any` keys/values, and empty literals whose element types cannot be resolved
  statically, are rejected with `E0401` rather than miscompiled.
- Allocation growth uses checked arithmetic and exits with `R0004` on overflow,
  matching native list behavior.

## Rationale

Correctness and semantic parity come before asymptotic performance. Linear
search is acceptable for command-line programs and removes an entire class of
hash/equality bugs. Structural keys already work through descriptors, so the
map runtime needs no type-specific generated code.

## Consequences

- Native map operations are O(n). Benchmarks must not claim map performance.
- The runtime API is internal; a future ordered hash index can replace the
  storage while keeping `vantyx_map_get/set/contains/keys/values` contracts.
- Map keys are compared structurally. Mutating a key after insertion can make it
  unreachable, exactly as in the interpreter; documentation must say so.
- Cycle-safe display and coinductive equality are required because maps can
  contain lists/structs that refer back to the map.

## Reversibility

High. The IR exposes only `Map(K, V)` and ordinary index/iteration operations.
Storage is confined to `src/codegen_c.rs`'s emitted runtime.

## Future reconsideration criteria

Revisit when either:

1. a benchmark shows map lookup dominating a realistic workload, or
2. maps larger than ~10,000 entries become a common use case.

At that point, evaluate an insertion-ordered hash index with a documented hash
for every supported key descriptor, preserving iteration order and the existing
runtime API.

## Implementation record (2026-11-22)

- IR: `IrType::Map`, `IrExprKind::Map`, `IrStmtKind::ForMap`, map indexing and
  compound assignment, typed `keys`/`values`, and `contains`.
- Runtime: `VantyxMap`, descriptor-driven key/value equality, insertion-ordered
  iteration, cycle-safe display, and coinductive cycle-safe equality.
- Checker precision: `keys`/`values` return `List[K]`/`List[V]`; `contains`
  checks the key/element type; `pop` returns `List[T]`'s element type.
- Tests: `map_literals_and_builtins_match`, `map_mutation_and_iteration_match`,
  `map_of_struct_keys_matches`, `nested_map_equality_matches`,
  `map_missing_key_exits_two`, `annotated_empty_map_supports_insertion_and_loops`,
  `map_of_lists_and_struct_fields_match`, `cyclic_structural_equality_matches`,
  plus `E0401` rejection tests for untyped/`Any` maps.
- Allocation growth checks `INT64_MAX` and `SIZE_MAX` before `realloc`.
