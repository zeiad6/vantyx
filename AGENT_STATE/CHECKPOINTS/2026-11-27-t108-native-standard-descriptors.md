# Checkpoint — T-108 native standard descriptors

Date: 2026-11-27
Status: VERIFIED locally; commit and CI pending

## Objective

Match Rust's startup behavior for closed standard descriptors. Rust opens
`/dev/null` for any of descriptors 0, 1, or 2 that is closed before `main`.
The interpreter therefore discards `print` output and exits normally when
stdout is closed. Generated native code inherited the closed descriptor and
failed with `R0013` `Bad file descriptor (os error 9)`. A later file `open`
could also receive descriptor 1 while the C `stdout` object still cached that
number, so a print or diagnostic could interact with the wrong descriptor.

## What changed

- `src/codegen_c.rs` adds `vantyx_sanitize_standard_fd`. It checks
  `fcntl(fd, F_GETFD)`, and if the descriptor is `EBADF` opens `/dev/null`
  with the requested flags. When the kernel does not return the same number,
  it `dup2`s the replacement into place and closes the temporary descriptor.
- Generated `main` calls the helper for stdin (`O_RDONLY`), stdout
  (`O_WRONLY`), and stderr (`O_WRONLY`) before `signal(SIGPIPE, SIG_IGN)` and
  any user code. `<fcntl.h>` is included.
- `tests/native.rs` adds
  `closed_stdout_matches_interpreter_and_does_not_reuse_descriptors`. It runs
  both engines through a POSIX shell that closes stdout, compares the first
  `error[...]` line, checks exit code `2`, verifies a `write_file` marker, and
  checks that captured stdout is empty.
- ADR-0051 and specification §10–§11 document the behavior.

## Reproduction

Before, with stdout closed:

```
native:   error[R0013]: failed to write output: Bad file descriptor (os error 9)
interpreter: (no error; output discarded, exit 0)
```

After, both engines reach the later `assert(false)` with
`error[R0006]: assertion failed` and exit `2`, and the marker file written
between the two prints exists. A closed standard descriptor is no longer
available for reuse by a later file open.

## Verified commands (2026-11-27)

- `cargo test --test native closed_stdout` — 1 passed.
- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 729 passed (402 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 147 native).
- `VANTYX_CC=gcc cargo test --test native` — 147 passed.
- `cargo build --release` — succeeded; all five examples built natively and
  matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs produced controlled diagnostics.
- The full 147-test native suite passed under clang
  `-fsanitize=address,undefined` with leak detection disabled.
- Skill validator — 13 skills valid; link checker — 117 links in 181
  Markdown files OK (including this checkpoint and ADR-0051).

## Blockers and limits

- Independent `qa-verifier` review is unavailable (subagent provider
  free-usage limit), so this is local verification.
- The helper runs once at startup and only replaces closed descriptors. It
  does not close, redirect, or validate descriptors that are already open,
  and it is not a sandbox.
- `/dev/null` is required for the fallback; if it cannot be opened, the
  previous behavior remains.
- The backend still requires POSIX headers and makes no Windows or WASM
  portability claim. The closed-stdout test is Unix-only.
- Output failures on an open but failing descriptor (`/dev/full`, broken
  pipe) remain `R0013` diagnostics at the print, as covered by T-107.

## Next exact commands

1. `git diff --check`; review the diff; commit the T-108 implementation and
   status records.
2. Record the new CI run in the next status commit.
3. Continue P0 probes: module-loader caps, file-descriptor behavior, and
   native/interpreter error-headline parity.
