# ADR-0056: Native memory reclamation strategy

- Status: Accepted for the staged design. Stage 1 is implemented by T-122 as
  conservative ownership facts over typed IR (`src/ownership.rs`) with no
  codegen change; Stage 2 is partially implemented by T-121 for statically
  owned `print` renderings. No escape analysis, arena, reference counting, or
  tracing collector is implemented. The local nested display-temporary frees
  from ADR-0054 already exist.
- Date: 2026-11-27
- Related: T-120, ADR-0005, ADR-0008, ADR-0009, ADR-0010, ADR-0054

## Context

Vantyx has two execution engines with different memory behavior:

- The interpreter uses Rust `Rc<RefCell<...>>` values. Rust prevents
  use-after-free and double free, but reference cycles between lists, maps, and
  structs leak. There is no cycle collector.
- The native C11 backend allocates strings, structs, enums, list headers and
  buffers, map headers and buffers, scalar-key map indexes, and display
  builders with `malloc`/`calloc`/`realloc`. Except for nested display
  temporaries (ADR-0054) and the old block released by `realloc`, none of these
  allocations is reclaimed. This is documented in `docs/memory_model.md` as an
  intentional v0.1 limitation, not a memory-safety claim.

The limitation matters for long-running native programs: a loop that builds
strings or appends to lists grows without bound even when every intermediate
value is dead. It also prevents Vantyx from making honest claims about
production memory behavior. The interpreter remains the semantic oracle, so
any reclamation scheme must preserve reference semantics, aliasing, cycles,
runtime-error behavior, and output exactly.

The native subset is statically typed and already has a typed IR, which makes a
conservative compile-time analysis feasible. The language has no destructors,
no user-visible pointers, and no finalizers, so reclamation is not observable
except through allocation failure and memory use. These properties make a
staged, ownership-first approach safer than introducing a runtime collector
first.

## Decision

Adopt a staged native reclamation plan. Reclamation is introduced only where
the compiler can prove that no live alias can observe the value. The default
for any uncertain case remains the current intentional leak. Do not add
reference counting, a tracing collector, or user-visible ownership annotations
to the language in this stage.

### Stage 0 — Current baseline

Keep the existing behavior and documentation:

- Native allocations are leaked unless a local ownership rule proves otherwise.
- Nested display temporaries are freed after they are copied into the display
  builder (ADR-0054).
- Builders release their previous block through `realloc` (ADR-0052,
  ADR-0053).
- Allocation failure terminates with a runtime error; no null pointer is
  dereferenced.
- The native suite runs under AddressSanitizer and UndefinedBehaviorSanitizer
  with leak detection disabled.

### Stage 1 — Ownership facts in the typed IR

Add an internal ownership classification to IR expressions and locals:

- `Fresh`: the expression produces a new allocation that is not aliased yet.
- `Borrowed`: the expression yields a reference owned elsewhere (a local,
  global, container slot, or call result).
- `Aliased`: the value may be reachable from more than one live binding.
- `MayEscape`: the value may be stored in a container, struct, enum, global,
  returned, or passed to a call that can retain it.

The classification is conservative: unknown constructs become `MayEscape`.
Add a verifier that rejects a reclamation site unless its value is `Fresh` or a
local proven non-escaping. Stage 1 does not change generated C; it only adds
facts and tests. **Implemented (T-122):** `src/ownership.rs` adds `Ownership`,
`ownership(expr)`, and `verify_reclamation`, conservative with unknown=>`MayEscape`,
`str(Str)`/`trim` not `Fresh`, nothing wired to codegen. This keeps the stage independently revertable and gives later
stages a checkable precondition instead of ad hoc `free` calls.

### Stage 2 — Fresh temporaries

Free compiler-generated values at their last use when they are `Fresh` and
cannot be observed afterwards:

- String concatenation, interpolation, and `str(...)` intermediates.
- Temporary path and text buffers created by builtins.
- Final display results that are immediately consumed by `print` rather than
  bound, stored, or returned.

This stage does not free user bindings, arguments, container elements, or
values returned from a call. The existing display rule is the model: append
first, then free the temporary.

Ownership must be proven from the runtime contract, not from the expression
shape. Two current helpers are conditionally owned and must not be freed
without an explicit owned flag:

- `vantyx_text_trim` returns the original pointer when the input needs no
  trimming, and a fresh copy otherwise. `print(trim(s))` would free a borrowed
  or static string on the no-op path.
- `str(x)` returns `x` itself when `x` is `Str`, and a fresh display string for
  every other type.

Stage 2 therefore needs an ownership fact that distinguishes `Fresh`,
`Borrowed`, and `Conditional`, and the reclamation site must reject
`Conditional` unless the helper is changed to return ownership explicitly (or
to always copy). Syntactic whitelists of "string-producing calls" are not
sufficient.

T-121 is the first implemented slice: a `print` argument whose static type is
`Int`, `Float`, `List`, `Map`, `Struct`, or `Enum` always produces a fresh
display string, so it is written and freed immediately. `Str`, `Bool`, and
`Unit` remain borrowed or static. `str(...)`, interpolation, and `trim(...)`
inside a `Str` argument are still not reclaimed because their ownership is not
yet proven at that site.

### Stage 3 — Non-escaping locals and arenas

Run a conservative escape analysis over typed IR locals:

- A local is non-escaping only when it is not returned, stored in a container,
  struct, enum, or global, passed to a call that may retain it, or aliased into
  an escaping value.
- Fixed-size scalars may live on the C stack.
- Variable-size aggregates may use a function-scoped or loop-scoped arena.
  The arena is freed at the end of the scope or iteration, including on
  `break`, `continue`, `return`, and runtime-error paths.
- Any local that may escape, any dynamically typed value, and any construct the
  analysis does not understand keeps the current leak.

Arena allocation must preserve reference semantics inside its lifetime:
aliases within the arena see the same object, and a value that is stored into
an escaping container must be copied or promoted before the arena is freed.
The conservative default is to promote rather than risk a dangling alias.

### Stage 4 — Escaping values: deferred choice

Escaping heap values, including cycles, are not reclaimed in Stages 1–3. Before
implementing reclamation for them, write a follow-up ADR amendment that chooses
between:

1. **Reference counting plus a cycle collector.** Reference counting is
   deterministic and familiar, but every alias, store, return, and overwrite
   needs count bookkeeping, and cycles still require a collector with precise
   roots. The failure modes are use-after-free and double free, which are
   unacceptable without strong verification.
2. **A precise tracing collector.** A mark-sweep or copying collector handles
   cycles without per-value counts, but needs precise roots (a shadow stack or
   generated root maps), an allocation header, and a collection policy. It also
   changes the allocation model for every native value.

The amendment must include measured allocation and pause data, a root-set
design, cycle behavior, allocation-failure behavior, and sanitizer evidence.
Until then, no tracing collector and no reference counting are implemented.

### Stage 5 — Interpreter cycles

Interpreter `Rc` cycles remain a separate problem. A cycle collector for the
interpreter is justified only if long-running interpreter use becomes a target;
native reclamation must not assume that interpreter values are reclaimed.

## Allocation failure

Every stage preserves the current rule: a failed allocation terminates with a
runtime error and never yields a null pointer that user code can observe. An
arena allocation failure follows the same path. Freeing memory never fails.
If a tracing collector is added later, it must attempt a collection before
reporting out-of-memory and must not expose partially initialized objects.

## Safety invariants

Any reclamation change must satisfy all of these:

1. **No use-after-free.** A value is freed only when no live binding, container,
   global, return value, or runtime diagnostic can observe it.
2. **No double free.** Each allocation has one reclamation owner or one
   reference-count owner, never both.
3. **No dangling aliases.** Reference semantics are unchanged while a value is
   live; an alias either keeps the value alive or is proven dead first.
4. **No semantic change.** Interpreter and native output, exit codes, first-line
   runtime diagnostics, evaluation order, cycles, equality, and display remain
   identical.
5. **Conservative fallback.** An analysis failure, an unsupported construct, or
   an `Any` value leaks rather than guessing.
6. **No user-visible destruction.** There are no destructors, finalizers, or
   observable free order.
7. **No sandbox claim.** Reclamation does not add a memory quota, resource
   limit, or isolation boundary.

## Migration and rollback

Each stage is a separate change with its own tests and status entry. A stage is
kept only if the full native suite, the sanitizer suite, the differential
fuzzer, and the examples agree with the interpreter. A stage that cannot meet
the invariants is reverted to the previous leak behavior. Stage 1 emits the same
C as today, so it can land without a behavior change; Stages 2 and 3 are gated
on targeted leak-detection fixtures.

Leak detection stays disabled for the general sanitizer suite until the known
intentional leaks are gone. Targeted fixtures may run with
`ASAN_OPTIONS=detect_leaks=1` to prove that a specific allocation is reclaimed;
those fixtures must not be presented as whole-program leak freedom.

## Alternatives considered

- **Reference counting now.** Rejected for this stage: it touches every copy,
  assignment, argument, return, and container operation; cycles still leak; and
  a cycle collector needs the same precise roots as a tracing collector. The
  risk of a silent use-after-free is higher than the benefit of immediate
  reclamation.
- **Tracing GC now.** Rejected for this stage: it requires a shadow stack or
  generated root maps across all generated C, an allocation header, and a
  collection policy. It is the most likely long-term answer for cycles, but it
  needs its own measured design amendment.
- **User-visible ownership or borrow annotations.** Rejected: ordinary Vantyx
  code must not be forced to write lifetime annotations, and adding a borrow
  checker is a language-design change, not a backend optimization.
- **Free every value at scope exit.** Rejected: aliases and escaping stores make
  this a use-after-free generator.
- **Arena everything.** Rejected: arenas cannot reclaim values that outlive the
  scope, and a whole-program arena is equivalent to leaking until exit.
- **Leave the current leak forever.** Rejected as a long-term position: it makes
  long-running native programs impractical and prevents honest production
  claims. It remains the correct fallback until a stage is proven.

## Consequences

- The near-term work is analysis and temporary reclamation, not a collector.
- Native memory use can improve for loops and expression-heavy code without
  changing language semantics.
- Escaping values and cycles continue to leak until Stage 4; documentation must
  keep saying so.
- The typed IR gains ownership metadata that later optimization and FFI work can
  reuse.
- Each stage adds verification cost: differential tests, ASan/UBSan, targeted
  leak fixtures, and possibly peak-RSS measurements.
- No performance claim is made until measured under `benchmarks/results/`.

## Tests and evidence

- Stage 1: unit tests for the ownership classification and verifier; the
  generated C is byte-identical for existing fixtures.
- Stage 2: differential native tests for string, interpolation, path, and
  display temporaries; targeted fixtures under `detect_leaks=1`; the full
  sanitizer suite with `detect_leaks=0`.
- Stage 3: differential tests for aliasing, returns, container stores, globals,
  cycles, `break`/`continue`/`return`, and runtime-error paths; leak fixtures
  for non-escaping locals; peak-RSS measurements for loops and list building.
- Stage 4: a separate ADR amendment with measured allocation, pause, and
  root-set evidence before any collector code lands.
- Every stage reruns `cargo test`, the native suite under clang, GCC, and the
  sanitizer wrapper, the differential fuzzer, and the examples.
