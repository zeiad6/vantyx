# Checkpoint — T-106 empty list literals adopt `contains`/`push` value types

Date: 2026-11-26
Status: VERIFIED locally; commit and CI pending

## Objective

Make `contains([], value)` and `push([], value)` build with the native
backend. The checker inferred `List[Any]` for the empty literal, the
interpreter accepted it, and the native backend rejected it with `E0401`.

## What changed

- `src/checker.rs`: the `contains` and `push` arms check a direct empty list
  literal in the first argument against `List[T]`, where `T` is the checked
  type of the needle or pushed value. Non-empty arguments keep the previous
  inference path, so `contains([1, 2], "x")` and `push([1], "x")` still report
  `E0301` instead of changing element-level diagnostics.
- `tests/native.rs`: new `contains_and_push_accept_empty_list_literals`
  differential test covering `Int`, `Str`, `Float`, `Bool`, and nested
  `List[Int]` needles, `push([], 7)`, effectful needles/values, and a
  non-empty control.
- `tests/e2e.rs`: new `empty_list_literals_in_contains_and_push_are_typed`
  test pinning the interpreter result.
- `docs/language_specification.md` and `docs/type_system.md`: state that an
  empty list literal adopts an expected builtin type determined by the other
  arguments, including `join`'s `List[Str]` and the needle/value element type
  of `contains`/`push`.

## Reproduction

Before:

```
fn main():
    print(contains([], 1))
    push([], 7)
```

Interpreter exit `0` and printed `false`; `vantyx build` exit `1` with
`error[E0401]: list element has type Any, which the native backend cannot
represent` at the empty literal.

After: both engines exit `0`; `contains([], 1)` prints `false`, and
`push([], 7)` runs without output. Negative probes keep `E0301` for a
non-empty list with a mismatched needle/value, and `contains([], x)` with
`x: Any` still reaches the native `E0401` boundary.

## Verified commands (2026-11-26)

- `cargo test --test native contains_and_push_accept_empty_list_literals` —
  1 passed.
- `cargo test --test e2e empty_list_literals_in_contains_and_push_are_typed`
  — 1 passed.
- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 726 passed (402 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 144 native).
- `VANTYX_CC=gcc cargo test --test native` — 144 passed.
- `cargo build --release` — succeeded; all five examples built natively and
  matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/fuzz_smoke.py --binary target/release/vantyx` — 9
  depth/limit cases and 400 random/mutated inputs produced controlled
  diagnostics.
- Skill validator — 13 skills valid; link checker — 117 links in 178
  Markdown files OK.

## Blockers and limits

- Independent `qa-verifier` review is unavailable (subagent provider
  free-usage limit), so this is local verification.
- Only a direct empty list literal in the first argument of `contains` or
  `push` is affected. `len([])`, `print([])`, `pop([])`, `keys({})`,
  `values({})`, `contains({}, key)`, and unannotated empty-list bindings still
  infer `List[Any]`/`Map[Any, Any]` and remain under the native `E0401`
  boundary because no element/key/value type is available; users annotate the
  binding or collection.
- `contains([], x)` where `x` is statically `Any` stays `List[Any]`; the
  native backend cannot represent it. This is the existing gradual-typing
  boundary, not a new exclusion.

## Next exact commands

1. Finish the pending Python and validator checks.
2. `git diff --check`; review the diff; commit the T-106 implementation and
   the accumulated T-105/T-106 status records; push.
3. Record the new CI run in the next task commit.
4. Continue probes on file/directory races, symlink swaps during glob or
   recursive operations, map/string effects under `?`, and cross-module error
   propagation.
