# Checkpoint — T-104 native call-depth diagnostics report the call site

Date: 2026-11-26
Status: VERIFIED; pushed in `4269010`; CI run 45933776137 green; status sync
`8805381` passed run 45934076343

## Objective

Make the native `R0012` call-depth diagnostic name the same file, line, and
column as the interpreter, including when the failing call crosses a module
boundary.

## What changed

- `src/codegen_c.rs` adds `vantyx_call_line` and `vantyx_call_column`.
- `call_expr` sets them immediately after argument evaluation and before the
  invocation. The function entry guard reports those coordinates instead of
  the callee declaration's span.
- The callee's `vantyx_source_name` is assigned only after the depth guard, so
  the caller's file is current when the check fails. `call_expr` still restores
  the caller's file after each call.
- Argument preludes are used whenever any argument has effects, not only when
  there are two or more arguments. This prevents a nested call in an argument
  from leaving its own coordinates behind for the outer call's check.
- `tests/native.rs::native_call_depth_limit_exits_two` now compares the first
  two stderr lines (headline and ` --> file:line:column`) between the
  interpreter and native engines.
- `docs/language_specification.md`, `docs/memory_model.md`, and
  `docs/security.md` state that both engines report the call site.

## Reproduction

Direct recursion:

```
fn main():
    print(loop(0))

fn loop(n: Int) -> Int:
    return loop(n + 1)
```

Before: interpreter ` --> recursion.vtx:5:12`, native
` --> recursion.vtx:4:1`. After: both ` --> recursion.vtx:5:12`.

Cross-file probe (`main.vtx` imports `b.vtx`, which imports `a.vtx`; the
failing call is in `b.vtx` at line 5, column 16): both engines print
` --> b.vtx:5:16`. Before, native named the callee's file.

## Verified commands (2026-11-26)

- `cargo test --test native native_call_depth_limit_exits_two` — 1 passed.
- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 723 passed (402 library unit + 15 CLI unit + 47 AI integration
  + 117 e2e + 142 native).
- `VANTYX_CC=gcc cargo test --test native` — 142 passed.
- `cargo build --release` — succeeded; all five examples built natively and
  matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/fuzz_smoke.py` — 9 depth/limit cases and 400 random/mutated
  inputs produced controlled diagnostics.
- Skill validator — 13 skills valid; link checker — 117 links in 177 Markdown
  files OK.
- CI run 45933776137 passed on `4269010`.
- CI run 45934076343 passed on the status-sync commit `8805381`.

## Blockers and limits

- Independent `qa-verifier` review is unavailable (subagent provider
  free-usage limit), so this is local verification.
- Only the depth-limit location changes. Native still omits the interpreter's
  source excerpt and 256-frame stack trace; the guard remains an entry check.
- The new globals are set for every user-function call. Constructor and
  variant-constructor calls do not set them because their generated helpers
  have no depth guard.

## Next exact commands

T-104 is committed as `4269010`, pushed, and CI run 45933776137 passed.
Continue with targeted interpreter/native probes or another bounded P1/P2
task; keep the commit/CI fields current as later status updates land.

## Risks

- The codegen change touches every user-function call. The full 723-test suite,
  the GCC native suite, five examples, and the differential fuzzer all passed,
  which covers argument evaluation order and aliasing on the existing corpus.
- Stale coordinates can only affect a depth-limit error; ordinary calls never
  read the new globals.
