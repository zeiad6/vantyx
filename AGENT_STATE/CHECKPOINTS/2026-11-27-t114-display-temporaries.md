# Checkpoint — T-114 free nested native display temporaries

Status: VERIFIED locally; commit `3617fe3`; CI pending
Date: 2026-11-27

## Objective

Stop retaining one rendered string per list element, map key/value, struct
field, and enum field after single-buffer display (T-112) copies it into the
result. The temporaries are dead immediately after the append, but v0.1 never
frees them.

## Ownership analysis

- `vantyx_str_from_int`, `vantyx_str_from_float`, `vantyx_str_quote`,
  `vantyx_list_display`, `vantyx_map_display`, and generated struct/enum
  display functions return fresh heap strings.
- `vantyx_value_display` returned static literals for `Bool` and `Unit`, which
  made an unconditional free unsafe.
- `vantyx_sb_append_str` copies bytes through `vantyx_sb_append_bytes`, so the
  source string may be freed immediately after the append.
- Final display results can be bound or stored, so display must not free them.
- `vantyx_map_get`'s missing-key display call is safe to leave unfreed because
  `vantyx_rt_error` is `_Noreturn`.

## Files changed

- `src/codegen_c.rs` — adds `vantyx_str_from_bool_owned`,
  `vantyx_str_from_unit_owned`, and `vantyx_display_free`; binds, appends, and
  frees each list/map element, key, and value and each struct/enum field;
  switches `display_field` and `vantyx_value_display` to the owned helpers.
- `tests/native.rs` — `bool_and_unit_nested_display_match` (differential) and
  `nested_display_temporaries_are_freed_in_generated_c` (emitted-C assertion).
- `docs/decisions/ADR-0054-free-native-display-temporaries.md` — Proposed →
  Accepted with test and measurement evidence.
- `docs/memory_model.md`, `docs/benchmarks.md`, `TASKS.md`,
  `PROJECT_STATUS.md`, `AGENTS.md`, `docs/PROJECT_BASELINE.md`,
  `benchmarks/results/2026-11-27-display-temporaries.md`, and `AGENT_STATE/`.

## Completed

- Every nested display result is heap-owned; the free is unconditional.
- List/map/struct/enum display append then free each nested temporary.
- Top-level `print`/interpolation keep the static Bool/Unit helpers and do not
  free their result.
- Exact bytes, ordering, quoting, cycle markers, depth limits, and runtime
  errors are unchanged; the differential and sanitizer suites verify behavior.

## Evidence

- `cargo test --test native bool_and_unit_nested_display_match` — passed.
- `cargo test --test native nested_display_temporaries_are_freed_in_generated_c`
  — passed.
- Full gate: fmt/clippy clean; `cargo test` 736 passed (404 library unit + 15
  CLI unit + 47 AI integration + 118 e2e + 152 native); GCC native 152;
  sanitized native 152; examples matched; 9 fuzz self-tests; 100/100 seed-1
  differential cases; fuzz smoke; 13 skills; link checker.
- `benchmarks/results/2026-11-27-display-temporaries.md` — nine alternating
  runs per workload:
  - `display_ops` 10k: median peak RSS 2,448 → 2,180 KiB.
  - 100k Int list: median peak RSS 6,552 → 3,616 KiB.
  - 50k Pair(Bool, Bool) list: median peak RSS 9,368 → 5,580 KiB.
  - Wall time also fell in these batches, recorded as observed data rather
    than a stable speed claim.

## Remaining

- CI pending.
- Independent review remains unavailable (subagent free-usage limit).

## Scope limits

- Not general reclamation. Final display strings, user strings, collection
  storage, and other native allocations remain intentionally unreclaimed.
- One allocation per rendered element or field remains; typed display
  appenders are deferred.
- Nested Bool/Unit display now allocates an owned copy that is immediately
  freed, trading a small allocation for a uniform ownership rule.
