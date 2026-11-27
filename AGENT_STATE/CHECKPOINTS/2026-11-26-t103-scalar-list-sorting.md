# Checkpoint — T-103 deterministic scalar-list sorting

Date: 2026-11-26
Status: VERIFIED locally; commit, push, and CI pending

## What changed

- `src/checker.rs`: `sorted` is registered in `BUILTINS` and checked as one
  `List` argument whose element type is `Int`, `Float`, `Str`, or `Bool`.
  Non-list arguments and other element types are `E0301`; wrong arity is
  `E0302`; an `Any` argument defers to runtime.
- `src/interp.rs`: `sort_scalar_values` and the `sorted` builtin arm. The list
  is copied, sorted with Rust's stable `sort_by`, and compared with an explicit
  rule: `NaN` after every non-`NaN`, `-0.0` equal to `0.0`, byte-wise strings,
  and `false` before `true`. Dynamic `Any` values are checked at runtime
  (`R0005` for mixed or non-scalar lists).
- `src/ir.rs`: `sorted` validates one list argument and a scalar element type,
  then returns `List[T]`; other element types are rejected with `E0401` as
  defense in depth.
- `src/codegen_c.rs`: `vantyx_list_sorted_int/float/str/bool` over a stable
  merge sort with a temporary buffer. No `qsort`, so tie order is deterministic
  across C libraries. Allocations go through the existing checked list helpers.
- `tests/e2e.rs`: `sorted_orders_scalar_lists_stably`,
  `sorted_rejects_non_scalar_lists_and_wrong_arity`,
  `sorted_dynamic_any_value_uses_runtime_type_checks`, and
  `sorted_dynamic_any_value_rejects_mixed_lists`.
- `tests/native.rs`: `sorted_scalar_lists_match`.
- Docs: `docs/language_specification.md` §6, README status table, ADR-0050,
  TASKS.md T-103, and the status/state files.

## Contract

- `sorted(list: List[T]) -> List[T]` for `T` in `Int`, `Float`, `Str`, or
  `Bool`.
- Returns a new ascending list; the input list and its order are unchanged.
- Stable: equal values keep input order. `-0.0` and `0.0` compare equal;
  every `NaN` sorts after every non-`NaN`; strings compare by UTF-8 bytes;
  `false` precedes `true`.
- Static rejection of non-list arguments, wrong arity, and non-scalar element
  types. The interpreter checks dynamic `Any` values; native keeps `E0401`.
- `O(n log n)` time and `O(n)` extra element storage.

## Verified commands (2026-11-26)

- `cargo test --test e2e sorted_` — 4 passed.
- `cargo test --test native sorted_scalar_lists_match` — 1 passed.
- `cargo test` — 723 passed (402 library unit + 15 CLI unit + 47 AI integration
  + 117 e2e + 142 native). The full 142-test native suite also passed with
  `VANTYX_CC=gcc`.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded.
- All five examples built natively and matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 passed;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- Skill validator: 13 skills valid; link checker: 117 links in 175 Markdown
  files OK.
- Manual probes matched for `Int`, `Float` (`-0.0`, `0.0`, two `NaN`s), `Str`
  (empty, ASCII, non-ASCII), `Bool`, empty, singleton, duplicates, and a
  mutated result with the input unchanged.

## Blockers and limits

- Independent `qa-verifier` review was unavailable (subagent provider
  free-usage limit), so this is local verification, not independent review.
- Scalar element types only; no comparator, key function, descending/reverse
  flag, in-place variant, deduplication, or structural ordering. Dynamic `Any`
  sorting is interpreter-only because native rejects `Any` bindings at
  `E0401`.
- The merge sort allocates a temporary buffer proportional to the list length;
  this is documented rather than hidden.

## Next

Commit and push T-103, record the CI run, then check the CI result for the
T-102 status-sync commit `0a309c8`. After that, continue targeted
interpreter/native probes or pick the next bounded P1 standard-library task.
