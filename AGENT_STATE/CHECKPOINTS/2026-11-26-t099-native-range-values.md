# Checkpoint — 2026-11-26 — T-099 native `range(...)` list values

## Task

- ID: T-099
- Title: Native `range(...)` list values outside `for` loops
- Status: VERIFIED locally
- Owner: orchestrator (implementation and verification)
- Priority: P1 (native backend/language completeness)
- Branch: `main`
- HEAD: `1aba7ba` (no commit, push, or history rewrite authorized)
- Working tree: T-078 through T-099 remain uncommitted; many pre-existing
  modifications from earlier tasks are present and must be preserved.

## Problem

The interpreter's `range(...)` builtin returns a `List[Int]` in any expression
position. The native backend rejected every use outside a `for` loop with
`E0401`, so `let xs = range(5)`, `print(range(5))`, `len(range(5))`, and
`range(5)[2]` worked only under `vantyx run`. The IR already had a direct loop
lowering for `for x in range(...)`, but no value representation for the call.

## Design

- `src/ir.rs::lower_builtin` validates 1–3 `Int` bounds and returns
  `IrType::List(Box::new(IrType::Int))` instead of rejecting the call.
- `src/codegen_c.rs::builtin_expr` binds every bound to an `int64_t`
  temporary in source order, then calls `vantyx_range_list(start, end, step,
  line, column)`.
- `vantyx_range_list` reuses `vantyx_range_count`, so step-zero and the
  10,000,000-element limit produce the same `R0004` headlines as the
  interpreter and the loop lowering. It allocates a typed `int64_t` list with
  `vantyx_desc_int` and computes each element in `__int128` before pushing it.
- `for x in range(...)` keeps its direct statement lowering; only
  value-position calls materialize a list.
- The helper is defined after `vantyx_list_push_raw` and `vantyx_desc_int`, so
  no forward declaration or runtime type was added.

## Changes

- `src/ir.rs`: `range` lowering and `Int`-bound validation.
- `src/codegen_c.rs`: `range` builtin case and `vantyx_range_list`.
- `tests/native.rs`: `range_values_match` and two value-position cases in
  `runtime_error_headlines_match_interpreter`.
- `docs/language_specification.md`, `docs/compiler_architecture.md`,
  `docs/PROJECT_BASELINE.md`, `README.md`, `PROJECT_STATUS.md`, `AGENTS.md`:
  supported-feature and test-count updates.
- `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`, and `AGENT_STATE/`: task and
  state records.

## Tests

- `tests/native.rs::range_values_match`: 1/2/3 bounds, negative step, empty
  ranges, `len`, indexing, display, a variable-bound range, and effectful
  bounds evaluated left to right.
- `runtime_error_headlines_match_interpreter`: added `range(0, 3, 0)` and
  `range(0, 10000001)` in value position; the first `error[...]` line must
  match the interpreter.

## Verification

- `cargo test --test native range_values_match` — 1 passed.
- `cargo test --test native runtime_error_headlines_match_interpreter` — 1
  passed.
- `cargo test` — 701 passed (391 library unit + 15 CLI unit + 41 AI integration
  + 113 e2e + 141 native).
- `VANTYX_CC=gcc cargo test --test native` — 141 passed.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check --all` — clean.
- `cargo build --release` — succeeded.
- `python3 tools/test_differential_fuzz.py` — 7 passed.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- All five `examples/*.vtx` built and ran natively with identical output.
- `/tmp/opencode/probe/diff6.py` — 31/31; only the known `List[Any]` display
  limitation remains and is reported as a known native limitation rather than
  a mismatch.
- Manual probe: display, `len`, indexing, descending and empty ranges, a
  function-returned range, and left-to-right effectful bounds matched between
  `vantyx run` and the native executable.

## Files modified by this task

- `src/ir.rs`
- `src/codegen_c.rs`
- `tests/native.rs`
- `docs/language_specification.md`
- `docs/compiler_architecture.md`
- `docs/PROJECT_BASELINE.md`
- `README.md`
- `PROJECT_STATUS.md`
- `AGENTS.md`
- `TASKS.md`
- `AGENT_STATE/SESSION_STATE.md`
- `AGENT_STATE/ACTIVE_TASK.md`
- `AGENT_STATE/TASK_QUEUE.md`
- `AGENT_STATE/COMPLETED_TASKS.md`
- `AGENT_STATE/AGENT_ASSIGNMENTS.md`
- `AGENT_STATE/CHECKPOINTS/2026-11-26-t099-native-range-values.md`

## Blockers

- Commit, push, and CI are unauthorized.
- Independent `qa-verifier` review is blocked by subagent provider usage limits.
- T-040 local-model evidence still needs a local inference server/model.
- GPU, tensor, SIMD, autodiff, and training evidence still needs hardware.
- Windows native support remains unverified; CI is Linux-only.

## Next exact steps

1. Continue targeted differential probes for P0/P1 defects before starting
   lower-priority tooling work. The known remaining native limitation is
   heterogeneous `List[Any]` display (`E0401`), which needs a dynamic value
   representation and is not a bounded change.
2. If no higher-priority defect appears, implement dataset deduplication as a
   separate bounded P3 task with content-hash determinism, bounded memory, and
   regression tests.
3. Re-run the full gate after each change.
4. Request independent review when subagent capacity returns.
5. Do not commit or push without explicit user authorization.
