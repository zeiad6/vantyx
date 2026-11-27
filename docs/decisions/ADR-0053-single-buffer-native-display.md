# ADR-0053: Single-buffer native collection and nominal display

- Status: Accepted
- Date: 2026-11-27
- Related: T-112, ADR-0010, ADR-0012, ADR-0052

## Context

The native backend rendered a `List`, `Map`, struct, or enum by repeatedly
calling `vantyx_str_concat` on the growing result. Every call allocated a new
string and copied the complete prefix, so rendering an `n`-element collection
performed `O(n)` concatenations and copied `O(n²)` bytes. Because v0.1 native
allocations are never freed, every intermediate prefix also stayed live until
process exit.

A measured program that built a 10,000-element `List[Int]` and evaluated
`len(str(xs))` took a median 754.707 ms and peaked at 568,244 KiB native RSS
before this change in a same-batch release comparison. The same program after
the change takes a median 5.737 ms and 2,520 KiB in the same batch. The
interpreter builds display text incrementally in one growable `String`, so the
previous native shape was both slower and asymptotically worse than the
reference engine. Raw runs are in
`benchmarks/results/2026-11-27-display-buffer.md`.

Display must preserve the exact interpreter bytes, insertion order, cycle and
depth markers, quoting, and the existing early-return behavior for recursive
values. The change is an allocation strategy only.

## Decision

Use the `VantyxStrBuilder` introduced by ADR-0052 for every native display
path:

- `vantyx_list_display` appends `[`, each rendered element separated by
  `, `, and `]` into one builder.
- `vantyx_map_display` appends `{`, each rendered key and value separated by
  `: ` and `, `, and `}` into one builder.
- Generated struct display functions append the type name, each `field=`
  label and rendered field, and the closing parenthesis into one builder.
- Generated enum display functions append each variant name and its payload
  fields into one builder; payload-free variants keep the single static
  string result.
- Cycle and depth checks still run before a builder is created, and the
  existing `seen`/`vantyx_enter_struct` stacks remain active while nested
  display functions run, so `[...]`, `{...}`, `Type(...)`, and `…` markers are
  unchanged.

Nested values still return one allocated string per element before it is
appended. Removing those per-element strings requires typed display appenders
that recurse without materializing a string; that is a separate change. The
quadratic concatenation and retained prefixes are eliminated here.

## Alternatives considered

- **Two-pass exact sizing.** Rejected for the same reason as ADR-0052:
  computing the final length requires either evaluating recursive display
  twice or storing every rendered part, and it complicates cycle and depth
  behavior.
- **Append typed values without per-element strings.** Promising, but it
  requires a parallel display API for lists, maps, structs, and enums plus
  depth/cycle plumbing. Deferred until measurements show the remaining
  per-element allocations dominate.
- **Free intermediate rendered strings.** Rejected: v0.1 has no ownership or
  liveness analysis, and display strings may be aliased by bindings or
  containers.
- **Keep repeated concatenation and rely on the C compiler.** Rejected: the
  cost is algorithmic, not a missed optimization; the generated calls and
  retained prefixes are visible in the measured RSS.

## Consequences

- Rendering an `n`-element list or map now performs one growing-buffer
  allocation sequence instead of `O(n)` concatenations, and no longer retains
  every prefix.
- Struct and enum display with many fields no longer retains one intermediate
  string per field and separator.
- The final display string is still intentionally unreclaimed in v0.1.
- The builder may over-allocate up to about twice the final byte length.
- Per-element rendered strings are still allocated and leaked; for a list of
  scalars that is one small string per element.
- Output bytes, ordering, quoting, cycle markers, depth limits, and runtime
  error behavior are unchanged.

## Tests and measurements

- `tests/native.rs::large_collection_display_matches` renders a 5,000-element
  `List[Int]`, a struct containing that list, a list of enum payloads, a
  2,000-entry `Map[Str, Int]`, and a 70-level recursive enum, and compares
  stdout and exit code with the interpreter.
- Existing display tests continue to cover cycles, deep enums, nested lists,
  map keys, floats, and struct fields.
- The full native differential suite runs under
  AddressSanitizer/UndefinedBehaviorSanitizer with leak detection disabled.
- `benchmarks/display_ops.vtx` and matching C, Rust, and Python
  implementations were added to `benchmarks/run.py`. A focused before/after
  comparison of the same workload is recorded in
  `benchmarks/results/2026-11-27-display-buffer.md`.
