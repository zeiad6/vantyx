# Checkpoint — 2026-11-26 — T-097 filesystem OS-error suffix and glob parse diagnostics

## Task

- ID: T-097
- Title: Filesystem OS-error suffix and glob parse diagnostic parity
- Status: VERIFIED locally
- Owner: orchestrator (implementation and differential probing)
- Priority: P1 (interpreter/native correctness)
- Branch: `main`
- HEAD: `1aba7ba` (no commit, push, or history rewrite authorized)
- Working tree: T-078 through T-097 remain uncommitted; many pre-existing
  modifications from earlier tasks are present and must be preserved.

## Problem

A third differential batch (`/tmp/opencode/probe/diff3.py`, 44 programs) found
10 first-line divergences:

| Case | Interpreter | Native (before) |
| --- | --- | --- |
| `remove_file("nope")` | `...: No such file or directory (os error 2)` | `...: No such file or directory` |
| `make_dir(".")` | `...: File exists (os error 17)` | `...: File exists` |
| `rename("nope", "x")` | `...: No such file or directory (os error 2)` | `...: No such file or directory` |
| `copy_file("nope", "x")` | `...: No such file or directory (os error 2)` | `...: No such file or directory` |
| `file_size("nope")` | `...: No such file or directory (os error 2)` | `...: No such file or directory` |
| `list_dir("nope")` | `...: No such file or directory (os error 2)` | `...: No such file or directory` |
| `read_file(".")` | `...: Is a directory (os error 21)` | `...: Is a directory` |
| `write_file(".", "x")` | `...: Is a directory (os error 21)` | `...: Is a directory` |
| `glob("[")` | `invalid glob pattern: unclosed character class` | `unclosed character class in glob pattern` |
| `glob("[a")` | `invalid glob pattern: unclosed character class` | `unclosed character class in glob pattern` |

Rust's `std::io::Error` Display appends ` (os error N)` on Unix; the native
runtime printed only `strerror`. `glob("[z-a]")` additionally printed
`descending range in glob character class` natively while the interpreter said
`invalid glob pattern: descending character range \`z-a\``.

## Changes

- `src/codegen_c.rs`:
  - Adds `vantyx_os_error(int)`, which formats `strerror(error_number)` plus
    ` (os error N)` and returns an allocated string. All 32 native filesystem
    diagnostic call sites use it.
  - Adds `vantyx_utf8_encode` for quoting Unicode range endpoints in errors.
  - Rewrites the three glob parse errors to the shared parser's wording:
    `invalid glob pattern: descending character range \`z-a\``,
    `invalid glob pattern: unclosed character class`, and
    `invalid glob pattern: empty character class`.
- `tests/native.rs`: `runtime_error_headlines_match_interpreter` gained 11
  cases: remove/mkdir/rename/copy/stat/list failures, read and write on a
  directory, and `glob("[")`, `glob("[a")`, `glob("[z-a]")`.

## Verification

- Pre-fix: `diff3.py` reported 10 mismatches; the eight filesystem cases lacked
  the OS-error suffix and the glob wording differed.
- Post-fix: `diff3.py` matched 44/44.
- `cargo test --test native runtime_error_headlines_match_interpreter` — passed.
- `cargo test --test native` — 140 passed (clang).
- `VANTYX_CC=gcc cargo test --test native` — 140 passed.
- `cargo test` — 386 library unit + 15 CLI unit + 39 AI integration + 113 e2e
  + 140 native = 693 passed.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check --all` — clean.
- `diff.py` 55/55 and `diff2.py` 34/34 remained clean.

## Incident during implementation

The first `vantyx_os_error` version was produced by a global replacement that
also rewrote the `strerror` call inside the helper, making it infinitely
recursive. Every filesystem error path then segfaulted (exit `-11`). The
differential probe caught it immediately; the helper now calls `strerror`
exactly once. Lesson recorded in `AGENT_STATE/DECISIONS.md`: verify generated-C
helpers are non-recursive and defined before use.

## Files modified by this task

- `src/codegen_c.rs`
- `tests/native.rs`
- `TASKS.md`
- `PROJECT_STATUS.md`
- `AGENTS.md`
- `AGENT_STATE/SESSION_STATE.md`
- `AGENT_STATE/ACTIVE_TASK.md`
- `AGENT_STATE/TASK_QUEUE.md`
- `AGENT_STATE/COMPLETED_TASKS.md`
- `AGENT_STATE/DECISIONS.md`
- `AGENT_STATE/RECOVERY.md`
- `AGENT_STATE/AGENT_ASSIGNMENTS.md`
- `AGENT_STATE/BLOCKED_TASKS.md`
- `AGENT_STATE/CHECKPOINTS/2026-11-26-t097-filesystem-os-errors.md`

## Blockers

- Commit, push, and CI are unauthorized.
- Independent `qa-verifier` review is blocked by subagent provider usage limits.
- T-040 local-model evidence still needs a local inference server/model.
- GPU, tensor, SIMD, autodiff, and training evidence still needs hardware.
- Windows native support remains unverified; the OS-error suffix is Unix-only.

## Next exact steps

1. Continue probing unverified surfaces: map iteration with mixed key types,
   hidden glob entries, modules under failure, output-error paths, and recursive
   copy/rename error text. Add a regression test before fixing any confirmed
   mismatch.
2. Extend offline dataset tooling with deterministic train/held-out splits,
   tokenizer-aware counts, and duplicate detection as separate bounded tasks.
3. Re-run the full gate after the next change.
4. Request independent review when subagent capacity returns.
5. Do not commit or push without explicit user authorization.
