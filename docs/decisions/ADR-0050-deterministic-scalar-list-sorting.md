# ADR-0050: Deterministic scalar-list sorting

- Status: Accepted
- Date: 2026-11-26
- Related: T-103, ADR-0008, ADR-0016, ADR-0039

## Context

Vantyx had no ordering builtin. Programs that needed a sorted result had to
hand-write a sort, which is error-prone and encourages per-program ordering
rules that the interpreter and native backend can implement differently.

Two implementation hazards made a careless builtin unsafe:

1. **Unstable C sorting.** C `qsort` may reorder equal elements differently
   across libc implementations and optimization levels. Vantyx requires
   deterministic behavior across engines, so an unstable sort would make
   `sorted` output depend on the host C library.
2. **Structural values have no agreed order.** Structs, enums, lists, and maps
   have display forms but no language-level ordering. Defining one implicitly
   would freeze serialization and comparison decisions that deserve their own
   design, and would make native ordering depend on type descriptors.

Floating-point values add two observable edge cases: `-0.0` and `0.0` compare
equal but display differently, and `NaN` is unordered under IEEE-754.

## Decision

Add one builtin:

```text
sorted(list: List[T]) -> List[T]
```

where `T` is `Int`, `Float`, `Str`, or `Bool`.

### Contract

- **New list.** The result is a fresh list; the argument list and its element
  order are unchanged. Mutating the result does not affect the input.
- **Ascending order.**
  - `Int`: numeric ascending.
  - `Float`: numeric ascending with a total order for the builtin:
    every non-`NaN` sorts before every `NaN`; `-0.0` and `0.0` compare equal;
    two `NaN` values compare equal.
  - `Str`: byte-wise lexicographic order over the UTF-8 encoding. Vantyx
    strings cannot contain NUL bytes, so C `strcmp` and Rust `str::cmp` agree.
  - `Bool`: `false` before `true`.
- **Stable.** Values that compare equal keep their input order. This is
  observable for `-0.0`/`0.0` and for `NaN` payloads through display, so the
  native backend must not use `qsort`.
- **Static rejection.** The checker rejects a non-list argument (`E0301`), a
  wrong arity (`E0302`), and element types other than the four scalars
  (`E0301`). `List[Any]` is rejected statically because it has no guaranteed
  homogeneous order.
- **Dynamic `Any`.** When the argument has type `Any`, the interpreter checks
  the runtime value: a homogeneous scalar list is sorted, and anything else
  raises `R0005`. The native backend continues to reject `Any` bindings at the
  `E0401` boundary, so this path is interpreter-only by design.
- **Defense in depth.** IR lowering also rejects non-scalar element types with
  `E0401` rather than assuming the checker caught them.

### Implementation

- The interpreter copies the list and uses Rust's stable `sort_by` with an
  explicit `NaN`-last comparator.
- The native backend emits a stable recursive merge sort with a temporary
  buffer, one comparator per scalar type. It does not call `qsort`.
- The merge sort allocates `n` element slots for the temporary buffer and uses
  the existing checked list reserve/alloc helpers, so capacity overflow and
  oversized allocations remain `R0004`/fatal runtime errors rather than
  undefined behavior.
- The builtin name is reserved like the other builtins; user functions cannot
  shadow it.

## Alternatives considered

- **C `qsort`.** Rejected: unstable and implementation-defined for equal keys.
- **In-place `sort(list) -> Unit`.** Rejected for v0.1: a new list makes the
  non-mutation guarantee explicit and avoids surprising aliases. An in-place
  variant can be added later without changing `sorted`.
- **Comparator or key function.** Rejected for now: Vantyx has no first-class
  functions or closures, so a comparator parameter would require a new
  language feature and a new native calling convention.
- **Ordering for every value.** Rejected: lists, maps, structs, and enums have
  no language-defined order. Adding one would silently couple sorting to
  display or serialization choices.
- **Reverse/descending flag.** Deferred: callers can reverse the result once a
  reverse builtin or loop exists; a boolean flag adds a second contract and
  more tests for little value now.

## Consequences

- `sorted` is available in both engines for the supported scalar element types.
- Sorting costs `O(n log n)` comparisons and `O(n)` extra element storage.
- The interpreter's dynamic path accepts `Any` values; the native backend keeps
  its documented `E0401` boundary for `Any`.
- The result preserves reference semantics for element values, but all four
  supported element types are scalars, so there is no aliasing of elements.
- No deduplication, key extraction, custom comparator, descending order, or
  structural sorting is provided.
- `sorted` does not change existing builtins or syntax.

## Tests

- `tests/e2e.rs::sorted_orders_scalar_lists_stably` covers `Int`, `Float`
  (including `-0.0`, `0.0`, and two `NaN`s), `Str` (including empty and
  non-ASCII byte order), `Bool`, empty and singleton lists, duplicates, a
  copied result that can be mutated without changing the input, and a list
  literal argument.
- `tests/e2e.rs::sorted_rejects_non_scalar_lists_and_wrong_arity` covers
  `Int`, nested lists, structs, `List[Any]`, and arity errors.
- `tests/e2e.rs::sorted_dynamic_any_value_uses_runtime_type_checks` and
  `sorted_dynamic_any_value_rejects_mixed_lists` cover the interpreter's
  gradual-typing path.
- `tests/native.rs::sorted_scalar_lists_match` compares interpreter and native
  stdout and exit codes on the shared scalar contract.
- The language specification and README list the builtin and its ordering
  rules.
