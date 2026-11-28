# Checkpoint — T-117 list reversal

Date: 2026-11-27
Status: VERIFIED locally on 2026-11-27; commit `4108974`; CI pending

## Objective

Add `reverse(list: List[T]) -> List[T]` to the interpreter and native backend
without changing existing list semantics, and verify both engines with
differential tests.

## Implemented

- `src/checker.rs`: registers `reverse` and returns the argument's `List[T]`
  element type. Non-list arguments are `E0301`; wrong arity is `E0302`; a
  dynamic `Any` argument defers to runtime.
- `src/interp.rs`: clones the elements in reverse order into a new list; the
  input is unchanged.
- `src/ir.rs`: requires one statically typed list and returns its element type.
- `src/codegen_c.rs`: lowers `reverse` to `vantyx_list_reversed`, which
  allocates a new list and copies raw element slots in reverse order.
- `docs/language_specification.md`, `README.md`, `docs/PROJECT_BASELINE.md`,
  `AGENTS.md`, `TASKS.md`, `PROJECT_STATUS.md`, and `AGENT_STATE/` are updated.
- `tools/differential_fuzz.py`: the collection shape prints `reverse` results
  for `List[Int]`, `List[Str]`, and `List[Item]`.

## Tests

- `tests/e2e.rs::reverse_returns_a_new_list_for_all_element_types`
- `tests/e2e.rs::reverse_rejects_non_lists_and_wrong_arity`
- `tests/e2e.rs::reverse_dynamic_any_value_uses_runtime_type_checks`
- `tests/native.rs::reverse_lists_match`
- `tests/native.rs::dynamic_reverse_is_rejected_with_e0401`

## Evidence

- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo build` and `cargo build --release` — succeed.
- `cargo test` — 759 passed (404 library unit + 15 CLI unit + 47 AI
  integration + 132 e2e + 161 native).
- `VANTYX_CC=gcc cargo test --test native` — 161 passed.
- Sanitized native suite — 161 passed under ASan/UBSan with leak detection
  disabled.
- All five examples matched interpreter and native output.
- 9 fuzzer self-tests; 100/100 differential cases on seeds 1 and 2; fuzz smoke
  reported 9 depth/limit cases and 400 random/mutated inputs.
- 13 skills validated; 130 relative links in 199 Markdown files resolve.
- Targeted tests: `cargo test --test e2e reverse` passed 3;
  `cargo test --test native reverse` passed 2.

## Remaining

- Record the final gate log and commit hash, then make the status-sync commit.
- Independent `qa-verifier` review remains unavailable because the subagent
  provider returned a free-usage limit error; no independent result is claimed.

## Scope limits

- New list with `O(n)` extra storage; no in-place reversal, iterator, lazy
  view, comparator, deduplication, or reclamation of the new list.
- Shallow copying means nested collections and structs remain shared.
- `reverse([])` has no element type to adopt, stays `List[Any]`, and is
  rejected natively with `E0401`; annotate the binding or list literal.

## Resume

1. Read `TASKS.md` T-117 and `AGENT_STATE/ACTIVE_TASK.md`.
2. Run `cargo test --test e2e reverse` and `cargo test --test native reverse`.
3. Run the full gate script described in `AGENTS.md`.
4. Commit with all source, tests, docs, and state changes; then record the hash.
