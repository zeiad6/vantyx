# Checkpoint — T-107 native print output failures

Date: 2026-11-27
Status: VERIFIED locally; commit and CI pending

## Objective

Make native `print` report a failed stdout write the way the interpreter does:
`error[R0013]: failed to write output: <host error> (os error N)` at the
`print` that attempted the write, with exit code `2`. Native stdout was fully
buffered, so `/dev/full` was usually detected by the final flush as
`error: failed to flush output` without `R0013` or the print's location. A
closed pipe could terminate the process with SIGPIPE (exit `141`).

## What changed

- `src/codegen_c.rs::vantyx_print_end` now checks `fputc`, calls
  `fflush(stdout)`, and checks `ferror`. It captures `errno` before formatting
  `failed to write output: %s (os error %d)` with `strerror`, and raises
  `R0013` at the print span.
- `src/codegen_c.rs::c_main` calls `signal(SIGPIPE, SIG_IGN)` so a closed
  output pipe becomes an `EPIPE` write failure like Rust's default, rather
  than signal termination. The generated C now includes `<signal.h>`.
- `tests/native.rs` adds two differential tests:
  `print_write_failure_headlines_match_interpreter` (Linux `/dev/full`) and
  `print_broken_pipe_headlines_match_interpreter` (a Unix socket pair with
  the read end closed). Both compare exit code `2` and the first `error[...]`
  line.
- `docs/language_specification.md` §9–§11, the ADR-0039 amendment, and the
  status records document the behavior.

## Reproduction

Before, with stdout on `/dev/full`:

```
error: failed to flush output
```

with exit `2` but no `R0013` code, location, or OS error text; the interpreter
printed `error[R0013]: failed to write output: No space left on device (os
error 28)` at the print. With a closed pipe, native could exit `141` from
SIGPIPE.

After, both `/dev/full` and a closed pipe produce the same first line as the
interpreter and exit `2`.

## Verified commands (2026-11-27)

- `cargo test --test native print_` — 3 passed, including the two new tests.
- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 728 passed (402 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 146 native).
- `VANTYX_CC=gcc cargo test --test native` — 146 passed.
- `cargo build --release` — succeeded; all five examples built natively and
  matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/fuzz_smoke.py --binary target/release/vantyx` — 9
  depth/limit cases and 400 random/mutated inputs produced controlled
  diagnostics.
- Skill validator — 13 skills valid; link checker — 117 links in 180
  Markdown files OK (including this checkpoint).

## Blockers and limits

- Independent `qa-verifier` review is unavailable (subagent provider
  free-usage limit), so this is local verification.
- Only `print`'s stdout path changes. Native still omits the interpreter's
  source excerpt and stack trace, so the tests compare the first diagnostic
  line and exit code.
- The final flush fallback in `main`, file-writing builtins, and non-`print`
  output paths are unchanged. No sandbox or broader signal policy is added.
- `/dev/full` and the socket-pair test are Unix-specific; the `/dev/full` test
  is Linux-specific and skips when the device is unavailable.

## Next exact commands

1. `git diff --check`; review the diff; commit the T-107 implementation and
   status records.
2. Record the new CI run in the next status commit.
3. Continue P0/P1 probes: native output paths and file-write failure
   diagnostics, map/string evaluation under `?`, and cross-module error
   propagation.
