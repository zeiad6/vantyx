# Checkpoint — T-105 empty list literal adopts `join`'s `List[Str]` type

Date: 2026-11-26
Status: VERIFIED locally; commit, push, and CI pending

## Objective

Make `join([], separator)` build with the native backend. The checker inferred
`List[Any]` for the empty literal, the interpreter accepted it, and the native
backend rejected it with `E0401`.

## What changed

- `src/checker.rs`: the `join` arm checks a direct empty list literal against
  `List[Str]`. Non-empty arguments keep the previous inference path, so
  `join([1, 2], ",")` still reports one list-level `E0301` instead of one
  error per element.
- `tests/native.rs`: new `join_accepts_empty_list_literals` differential test
  covering `join([], ",")`, `join([], "")`, an annotated empty `List[Str]`, a
  non-empty literal, and a single empty-string element.
- `docs/language_specification.md` and `docs/type_system.md`: state that an
  empty list literal adopts an unambiguous builtin parameter type and
  otherwise remains `List[Any]`.

## Reproduction

Before:

```
fn main():
    print(join([], ","))
```

Interpreter exit `0` with one empty line; `vantyx build` exit `1` with
`error[E0401]: list element has type Any, which the native backend cannot
represent` at the empty literal.

After: both engines exit `0`, print the same three empty lines for
`join([], ",")`, `join([], "")`, and `join(xs, "-")` with
`let xs: List[Str] = []`.

## Verified commands (2026-11-26)

- `cargo test --test native join_accepts_empty_list_literals` — 1 passed.
- `cargo test --test e2e text_builtins` — 2 passed, including the existing
  non-empty `join([1, 2], ",")` diagnostic.
- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 724 passed (402 library unit + 15 CLI unit + 47 AI
  integration + 117 e2e + 143 native).
- `VANTYX_CC=gcc cargo test --test native` — 143 passed.
- `cargo build --release` — succeeded; all five examples built natively and
  matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/fuzz_smoke.py` — 9 depth/limit cases and 400 random/mutated
  inputs produced controlled diagnostics.
- Skill validator — 13 skills valid; link checker — 117 links in 177 Markdown
  files OK.

## Blockers and limits

- Independent `qa-verifier` review is unavailable (subagent provider
  free-usage limit), so this is local verification.
- Only a direct empty list literal in `join`'s first argument is affected.
  `len([])`, `print([])`, and `let xs = []; push(xs, ...)` still infer
  `List[Any]` and remain under the native `E0401` boundary because no element
  type is available; users annotate the binding or collection.
- The interpreter's `List[Any]` behavior for those other empty literals is
  unchanged.

## Next exact commands

1. `git diff --check && git status --short`
2. `git add -A && git commit -m "Adopt List[Str] for empty join arguments"`
3. `git push origin main`
4. `gh run watch <new-run-id> --exit-status` and record the result in
   `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`, `docs/PROJECT_BASELINE.md`,
   `AGENT_STATE/SESSION_STATE.md`, `AGENT_STATE/RECOVERY.md`, and this
   checkpoint.

## Risks

- The checker change is limited to one builtin argument and one literal shape;
  the existing text-builtin negative test still passes, and the native test
  exercises both engines.
- No IR or C runtime change: the IR already receives `List[Str]` from the
  checker's expression types.
