# Checkpoint — T-116 atomic file replacement

- Date: 2026-11-27
- Task: T-116 — Atomic file replacement
- Status: VERIFIED locally on 2026-11-27; commit `3e22628`; CI pending
- Priority: P1 (standard library: file safety)

## Objective

Add `write_file_atomic(path: Str, text: Str) -> Unit`, which creates or
replaces a file by writing a hidden same-directory temporary file with
exclusive creation, flushing it with `fsync`, and renaming it over the target.
Readers must observe either the previous file or the complete new contents,
never a partial file. The temporary must be removed on failure. The interpreter
remains the oracle and the native backend must agree on output, exit code,
first-line errors, evaluation order, and link behavior.

## Files changed

- `src/checker.rs`: registers `write_file_atomic`; checks both `Str` arguments
  with `E0301` and wrong arity with `E0302`.
- `src/interp.rs`: adds `write_file_atomic`, which chooses the target parent
  (matching `Path::parent`), creates `.vantyx-tmp-<pid>-<serial>` with
  `create_new`, writes, flushes, `sync_all`s, renames, and removes the
  temporary on failure.
- `src/ir.rs`: validates both native arguments as `Str`.
- `src/codegen_c.rs`: binds the path and contents in source order; adds
  `vantyx_write_file_atomic` with `open(O_WRONLY|O_CREAT|O_EXCL, 0666)`, a
  `write` loop with `EINTR` handling, `fsync`, POSIX `rename`, bounded
  `EEXIST` retries, and temporary cleanup. The lexical parent rule matches
  Rust's `Path::parent`, including empty, root, and trailing-slash paths.
- `tests/e2e.rs`: six tests for creation/replacement/empty contents/nested
  directories/cleanup, symbolic and dangling links plus hard links and a
  read-only target, fatal errors with cleanup, `E0301`/`E0302`, `R0005`, and
  left-to-right evaluation.
- `tests/native.rs`: five differential tests covering the same success, error,
  link, `E0401`, and evaluation-order paths.
- `docs/decisions/ADR-0055-atomic-file-replacement.md`: design, guarantees,
  limits, alternatives, and tests.
- `docs/language_specification.md`, `docs/security.md`, `README.md`,
  `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`, `docs/PROJECT_BASELINE.md`, and
  the agent-state files.

## Verification

- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo build` and `cargo build --release` — passed.
- `cargo test` — 754 passed (404 library unit + 15 CLI unit + 47 AI
  integration + 129 e2e + 159 native).
- `VANTYX_CC=gcc cargo test --test native` — 159 passed.
- Sanitized native suite with `tools/clang-sanitize.sh` and
  `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1` /
  `UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1` — 159 passed.
- All five examples matched interpreter and native output.
- `python3 tools/test_differential_fuzz.py` — 9 passed.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100;
  seed 2 — 100/100.
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs produced controlled diagnostics.
- Skill validator — 13 skills valid; link checker — 130 relative links in 197
  Markdown files resolve.
- Focused probes matched for creation, replacement, empty contents, nested
  directories, a 900 KiB payload, symbolic and dangling links, hard links,
  read-only targets whose mode becomes `0666 & umask`, missing parents,
  directory targets, empty paths, root paths, trailing slashes, and
  `E0301`/`E0302`/`E0401`/`R0005` boundaries. No `.vantyx-tmp-*` file remained.

## Remaining

- Independent `qa-verifier` review remains unavailable because the subagent
  provider returned a free-usage limit error; no independent result is claimed.

## Scope limits

- Whole-file replacement only; no append, streaming write, metadata
  preservation, directory fsync, crash scavenging, atomic copy, or sandbox.
- Temporary names are predictable and are not a security boundary.
- The native helper relies on POSIX file descriptors and `rename`; native
  Windows support remains unverified.
