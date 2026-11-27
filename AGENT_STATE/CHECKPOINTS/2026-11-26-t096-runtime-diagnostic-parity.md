# Checkpoint — 2026-11-26 — T-096 runtime diagnostic parity

## Task

- ID: T-096
- Title: Runtime diagnostic parity for conversions, map keys, range limits, and
  file I/O
- Status: VERIFIED locally
- Owner: orchestrator (implementation and differential probing)
- Priority: P1 (interpreter/native correctness)
- Branch: `main`
- HEAD: `1aba7ba` (no commit, push, or history rewrite authorized)
- Working tree: T-078 through T-096 remain uncommitted; many pre-existing
  modifications from earlier tasks are present and must be preserved.

## Problem

Differential probing found that several runtime errors used the same code but
printed different first-line text in the interpreter and the native runtime:

| Case | Interpreter | Native (before) |
| --- | --- | --- |
| `int(-2.5)` | `cannot convert `-2.5` to Int exactly` | `cannot convert Float to Int exactly` |
| `int(1e20)` | `cannot convert `100000000000000000000` to Int exactly` | `cannot convert Float to Int exactly` |
| `int(1.0/0.0)` | `cannot convert `inf` to Int exactly` | `cannot convert Infinity to Int exactly` |
| missing `Str` map key | `key `missing` is not present` | `key `"missing"` is not present` |
| `range(0, 10000001)` | `range` would produce 10000001 elements, exceeding the limit of 10000000` | `range` exceeds the native limit of 10000000 elements` |
| `read_file` NUL | `file `nul.bin` contains a NUL byte` | `file contains a NUL byte` |
| `read_file` UTF-8 | `file `bad.bin` contents are not valid UTF-8` | `file contents are not valid UTF-8` |
| `read_file` 64 MiB | `file `big.bin` exceeds the 64 MiB read limit` | `file exceeds the 64 MiB read limit` |
| `list_dir` bad name | `directory `.` contains a name that is not valid UTF-8` | `directory entry name is not valid UTF-8` |
| `glob` bad name | `directory `X` contains a non-UTF-8 entry name` | `directory entry name is not valid UTF-8` |

## Changes

- `src/value.rs`: `format_float` is now `pub(crate)`.
- `src/interp.rs`: imports `format_float` and uses it for the `int(Float)`
  `R0007` message, so integral floats keep `.0` and infinity/NaN use the Vantyx
  spelling.
- `src/codegen_c.rs`:
  - `vantyx_int_from_float` includes `vantyx_float_str(value)`.
  - `vantyx_map_get` renders `Str` keys unquoted (top-level display) and other
    keys with `vantyx_value_display`.
  - `vantyx_range_count` reports the requested count.
  - `vantyx_read_file` names the path in all three failures.
  - `vantyx_list_dir` names the directory with the interpreter's wording.
  - `vantyx_glob_names_read` names the target directory with the glob module's
    wording.
- `tests/native.rs`: adds `first_error_line`, `assert_error_headlines_match`,
  `runtime_error_headlines_match_interpreter`, and Unix-only
  `file_error_headlines_match_interpreter`.

## Verification

- Pre-fix: both new tests failed on the first mismatching headline, proving
  the tests detect the defects.
- Post-fix: `cargo test --test native headlines` — 2 passed.
- `VANTYX_CC=gcc cargo test --test native headlines` — 2 passed.
- `VANTYX_CC=gcc cargo test --test native` — 140 passed.
- `cargo test` — 386 library unit + 15 CLI unit + 39 AI integration + 113 e2e
  + 140 native = 693 passed.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check --all` — clean.
- `/tmp/opencode/probe/diff.py` — 55/55 match after fixing the probe's
  unit-variant syntax (`B` → `B()`) and building the native executable in a
  separate directory so `glob("*")` does not list it.
- `/tmp/opencode/probe/diff2.py` — 34/34 match after fixing the match-in-
  parentheses probe syntax; five cases the native backend rejects with
  `E0401` because it cannot represent `Any` are classified as documented
  limitations, not divergences.

## Files modified by this task

- `src/value.rs`
- `src/interp.rs`
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
- `AGENT_STATE/CHECKPOINTS/2026-11-26-t096-runtime-diagnostic-parity.md`

## Blockers

- Commit, push, and CI are unauthorized.
- Independent `qa-verifier` review is blocked by subagent provider usage limits.
- T-040 local-model evidence still needs a local inference server/model.
- GPU, tensor, SIMD, autodiff, and training evidence still needs hardware.

## Next exact steps

1. Continue probing unverified surfaces: recursive copy/rename error paths,
   modules under failure, hidden glob entries, map iteration with mixed key
   types, and output-error paths. Add a regression test before fixing any
   confirmed mismatch.
2. Extend offline dataset tooling with deterministic train/held-out splits,
   tokenizer-aware counts, and duplicate detection as separate bounded tasks.
3. Re-run the full gate after the next change.
4. Request independent review when subagent capacity returns.
5. Do not commit or push without explicit user authorization.
