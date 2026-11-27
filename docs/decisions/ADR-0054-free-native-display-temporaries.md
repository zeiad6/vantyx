# ADR-0054: Free nested native display temporaries

- Status: Proposed
- Date: 2026-11-27
- Related: T-114, ADR-0052, ADR-0053

## Context

ADR-0053 removed quadratic prefix copying from native display by appending
every part into one `VantyxStrBuilder`, but each nested element or field is
still rendered into a fresh temporary string before it is appended. In v0.1
native allocations are never reclaimed, so a 10,000-element list display
retained 10,000 small strings until process exit even though each temporary
was dead immediately after `vantyx_sb_append_str` copied it.

The display helpers have a narrow ownership shape:

- `vantyx_str_from_int`, `vantyx_str_from_float`, `vantyx_str_quote`,
  `vantyx_list_display`, `vantyx_map_display`, and generated struct/enum
  display functions return fresh heap strings.
- `vantyx_value_display` returns those fresh strings for every type, except
  `Bool` and `Unit`, which returned static literals.
- A `Str` value is quoted before it enters display, so the returned string is
  never the user's original string.
- The final display result is returned to `print`, `str`, or a caller and may
  be retained by a binding or container, so it must not be freed by display.

The temporary passed to an append is therefore safe to free **after** the
append copies it, provided every nested display result is heap-owned.

## Decision

1. Add `vantyx_str_from_bool_owned` and `vantyx_str_from_unit_owned`, which
   return heap copies for display contexts. Keep the existing static helpers
   for top-level `print` and interpolation, where the result is not freed.
2. Add `vantyx_display_free(vantyx_str)` as the single free point for nested
   display results.
3. In `vantyx_list_display` and `vantyx_map_display`, bind each rendered
   element, key, and value to a temporary, append it, then free it.
4. In generated struct and enum display functions, bind each rendered field
   to a temporary, append it, then free it.
5. Do not free final display results, top-level `Str` values, or strings that
   are not produced by the display helpers.

The ownership rule is local to display: nested display results are owned
temporaries; final results are owned by the caller and remain intentionally
unreclaimed in v0.1.

## Alternatives considered

- **Typed display appenders.** Appending scalars directly without allocating a
  temporary removes the allocation as well as the leak, but requires a parallel
  display API for every descriptor and generated callback. Deferred until the
  remaining allocation cost is measured as dominant.
- **Arena for display temporaries.** A scoped arena could free all temporaries
  at once, but it adds a second allocation model and lifetime rules for a
  small, local win. Rejected for v0.1.
- **Leave the temporaries leaked.** Rejected: they are provably dead after the
  append, and freeing them lowers peak memory for large displays without
  changing semantics.
- **Free every string returned by `str` or interpolation.** Rejected: those
  results can be bound, stored, or aliased, and v0.1 has no liveness analysis.

## Consequences

- Large list, map, struct, and enum displays no longer retain one temporary
  string per element or field.
- Each append now performs one `free`; the allocator may reuse the block for
  the next element, which can also reduce allocator pressure.
- Final display strings, user strings, and all other native allocations remain
  intentionally unreclaimed. This is not a general reclamation strategy.
- Output bytes, ordering, quoting, cycle markers, depth limits, and runtime
  errors are unchanged.
- The ownership contract must be maintained: any future display helper that
  returns a borrowed or static string must not be passed to
  `vantyx_display_free`.

## Tests and measurements

- Existing differential native display tests cover large lists, structs,
  enum payloads, maps, cycles, deep enums, nested collections, quoting, and
  recursive values.
- The full native suite runs under AddressSanitizer and
  UndefinedBehaviorSanitizer with leak detection disabled; a use-after-free
  would still be caught by ASan.
- A focused before/after comparison on `benchmarks/display_ops.vtx` records
  wall time and peak RSS in `benchmarks/results/`.
