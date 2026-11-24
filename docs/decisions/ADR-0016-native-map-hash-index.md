# ADR-0016 — Native map hash index for scalar keys

Status: accepted
Date: 2026-11-23
Amends: ADR-0010

## Context

ADR-0010 chose insertion-ordered parallel arrays with linear structural search
for the first native map implementation. It also defined a reconsideration
criterion: revisit when a benchmark shows map lookup dominating a realistic
workload or when maps larger than ~10,000 entries become common.

The 2026-11-22 baseline measured `map_ops` (3,000 inserts and 3,000 lookups) at
64.212 ms for native Vantyx versus 8.360 ms for the equivalent C program. The
gap was dominated by descriptor-based linear scans. That meets the ADR-0010
criterion, and maps are a core collection type.

## Decision

Keep the insertion-ordered key/value arrays as the canonical storage and add an
optional open-addressing index for scalar keys:

- Indexed key kinds: `Unit`, `Int`, `Float`, `Bool`, and `Str`.
- Non-scalar keys (`List`, `Map`, `Struct`, `Enum`) keep the linear structural
  equality path. They remain correct without recursive hashing.
- The index stores entry position + 1 (`0` means empty), so iteration, `keys`,
  `values`, display, and replacement-in-place continue to use the entry arrays
  and preserve insertion order.
- The table uses linear probing with a load factor at or below 50%. There is no
  deletion operation in the language, so no tombstones are needed.
- Hashing uses splitmix64 mixing for `Unit`/`Int`/`Float`/`Bool` and FNV-1a over
  the bytes of `Str`, followed by splitmix64. `-0.0` and `0.0` hash the same
  because they compare equal.
- The hash seed is fixed. Keys are program values, not untrusted network input
  by default; adversarial key streams can still degrade probing to O(n). This is
  documented rather than hidden.

## Semantics

Unchanged from ADR-0010 and the interpreter:

- New keys append; assigning an existing key replaces its value in place.
- Iteration and `keys`/`values` follow insertion order.
- Key equality is structural and descriptor-driven.
- Missing keys raise `R0008`.
- Display and equality remain cycle-safe.

## Consequences

- Expected O(1) lookup, insertion, and `contains` for scalar keys.
- O(n) lookup for structural keys, which is the previous behavior and is
  documented as such.
- Roughly 16 bytes of index memory per entry at the 50% load factor, plus the
  existing key/value arrays.
- Mutating a key after insertion already made it unreachable under linear
  search; with a hash index the stored hash also becomes stale. The existing
  warning remains and is now more important.
- The runtime API (`vantyx_map_get/set/contains/keys/values`) is unchanged.

## Evidence

Benchmark `map_ops`, 3,000 inserts and 3,000 lookups, 5 measured runs after one
warm-up, same machine and runner:

| Implementation | 2026-11-22 median | 2026-11-24 median |
| --- | ---: | ---: |
| Vantyx native | 64.212 ms | 3.597 ms |
| Vantyx interpreter (linear reference) | 106.218 ms | 121.728 ms |
| C (linear before; hash after this change) | 8.360 ms | 2.884 ms |
| Rust (linear before; `HashMap` after) | 9.044 ms | 4.495 ms |

Raw results: `benchmarks/results/2026-11-22.*` and
`benchmarks/results/2026-11-24.*`. This is a single-machine microbenchmark, not
a language-wide performance claim.

## Tests

`large_scalar_maps_match` exercises 5,000 integer keys, 1,000 string keys, float
keys, replacement without reordering, `keys` ordering, and `contains` across the
interpreter and native backend. Existing struct-key, nested-map, equality,
missing-key, and iteration tests continue to cover the structural fallback.

## Reversibility

High. The index is confined to the emitted runtime. Removing the `hashed`,
`index_cap`, and `index` fields and the index helpers restores ADR-0010 behavior
without changing the IR or language.
