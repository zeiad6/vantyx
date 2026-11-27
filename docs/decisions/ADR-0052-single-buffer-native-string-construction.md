# ADR-0052: Single-buffer native string construction

- Status: Accepted
- Date: 2026-11-27
- Related: T-111, ADR-0008, ADR-0039

## Context

The native backend lowered string interpolation and string `+` as a chain of
`vantyx_str_concat` calls. Each call allocated a new NUL-terminated string, so
a four-part interpolation such as `"item-{i}-{i + 1}"` allocated one string per
part plus one per intermediate concatenation. Because v0.1 native allocations
are intentionally never freed, every intermediate stayed live until process
exit.

A measured 200,000-iteration loop that built the same four-part interpolation
and summed its length peaked at about 33,300 KiB native RSS, while the
interpreter peaked at about 3,900 KiB. The native loop also spent measurable
time in repeated allocation and copying. The interpreter builds interpolation
results incrementally in one growable Rust `String`, so its allocation shape
was already closer to the intended semantics.

The language requires left-to-right evaluation of interpolation parts and
binary operands, including nested calls, mutation, output, and runtime errors.
Any allocation optimization must preserve that order and the exact rendered
bytes.

## Decision

Add a stack-allocated `VantyxStrBuilder` to the generated C runtime. It owns a
heap buffer that starts at 64 bytes and doubles on demand, appends typed parts,
and transfers the buffer to the caller once.

- String interpolation with two or more parts builds into one buffer.
- A left-associative string `+` chain with at least three operands, or with a
  multi-part interpolation operand, builds into one buffer. Two plain operands
  keep the existing single `vantyx_str_concat` path.
- A nested interpolation used as a part is flattened into the same buffer
  instead of allocating an intermediate string and copying it.
- Parts are appended strictly in source order. Each part is evaluated exactly
  once, immediately before its bytes are appended, so side effects, mutation,
  and runtime-error order are unchanged.
- Typed appenders use the existing scalar display rules. Float parts share
  `vantyx_float_format`, which was extracted from `vantyx_float_str`, so the
  bytes are identical to the previous path.
- The builder's growth uses `realloc`; the final buffer is returned as the
  string value. As with every other native allocation in v0.1, the final
  buffer is not reclaimed.

This changes allocation strategy only. It does not change the language,
evaluation order, diagnostics, or the documented leak model.

## Alternatives considered

- **Two-pass exact sizing.** The builder could sum static text lengths and
  rendered scalar lengths first, then allocate once. Rejected for now:
  evaluating parts to learn their lengths would either evaluate effectful
  parts twice or require storing every intermediate value, and both complicate
  the ordering guarantee. The doubling buffer keeps one pass and bounded
  copying.
- **`open_memstream`.** Rejected: it is a POSIX/GNU extension, while the
  backend targets GNU C11 with libc and does not need another portability
  dependency.
- **Free intermediate concatenation results.** Rejected: v0.1 has no ownership
  or liveness analysis, strings may be aliased by bindings and containers, and
  freeing intermediates without a documented reclamation model would create
  use-after-free hazards.
- **A general arena or garbage collector.** Out of scope. Native reclamation
  still requires its own ADR and staged migration, as `docs/memory_model.md`
  states.

## Consequences

- A k-part interpolation performs one allocation for the result instead of
  roughly `2k - 1` allocations, and a chain of `n` string operands performs one
  result allocation instead of `n - 1` intermediate concatenations.
- The builder may over-allocate up to about twice the final byte length. Peak
  live memory still falls substantially for string-heavy loops because
  intermediate results are no longer retained.
- A one-part interpolation and a two-operand plain concatenation keep their
  previous allocation shape.
- Allocation failure still terminates with exit code `2` and
  `error: out of memory`; size overflow still terminates with
  `error: string is too large`.
- The final string remains valid after the builder goes out of scope because
  the buffer is heap-allocated and ownership is transferred, not copied.
- No reclamation is added, so the v0.1 leak limitation and the sanitizer
  configuration with leak detection disabled remain unchanged.

## Tests and measurements

- `tests/native.rs::string_building_matches_interpreter` covers interpolation
  of `Unit`, `Bool`, `Int`, `Float` (including `-0.0`), `Str`, structs, and
  enums; effectful `+` chains; a right-nested chain; a struct field mutation
  observed by a later operand; nested arithmetic in interpolation; and long
  repeated text.
- `tests/native.rs::string_building_error_order_matches` verifies that a
  failing operand still runs after earlier parts, with identical stdout before
  the error and the same `error[R0001]` headline in both engines.
- The full native differential suite passes under
  AddressSanitizer/UndefinedBehaviorSanitizer with leak detection disabled.
- `benchmarks/string_interp.vtx` and the matching C, Rust, and Python
  implementations were added to `benchmarks/run.py`. A before/after comparison
  of the same workload, methodology, and raw runs is recorded in
  `benchmarks/results/2026-11-27-string-buffer.md`.
