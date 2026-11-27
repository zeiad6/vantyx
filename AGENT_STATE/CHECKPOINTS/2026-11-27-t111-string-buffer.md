# T-111 checkpoint — single-buffer native string construction

Date: 2026-11-27
Status: VERIFIED locally; commit `45c438b`; CI pending
Related: ADR-0052, TASKS.md, PROJECT_STATUS.md

## Objective

Stop native interpolation and string `+` chains from allocating and retaining
one intermediate string per part and concatenation, without changing language
semantics, evaluation order, or diagnostics.

## Files changed

- `src/codegen_c.rs`: `vantyx_float_format` extraction, `VantyxStrBuilder`,
  typed appenders, `string_expr`, `append_str_part`,
  `append_value_to_builder`, `string_add_chain`,
  `collect_str_add_operands`, `binary_expr` integration.
- `tests/native.rs`: `string_building_matches_interpreter`,
  `string_building_error_order_matches`.
- `benchmarks/string_interp.vtx`, `.c`, `.rs`, `.py`;
  `benchmarks/run.py` workload registration.
- `benchmarks/results/2026-11-27.{json,md}` (full eight-workload run);
  `benchmarks/results/2026-11-27-string-buffer.md` (focused before/after).
- `docs/decisions/ADR-0052-single-buffer-native-string-construction.md`.
- `docs/memory_model.md`, `docs/benchmarks.md`,
  `docs/PROJECT_BASELINE.md`, `README.md`, `AGENTS.md`, `TASKS.md`,
  `PROJECT_STATUS.md`, and `AGENT_STATE/`.

## Verification

- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 733 passed (404 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 149 native, 0 failed); `string_building_matches_interpreter`
  additionally covers a right-nested effectful chain and a struct field
  mutation that a later operand must observe.
- `VANTYX_CC=gcc cargo test --test native` — 149 passed.
- `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1
  UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1
  VANTYX_CC="$PWD/tools/clang-sanitize.sh" cargo test --test native` — 149
  passed.
- `cargo build --release` — success; all five examples matched interpreter
  output.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100
  agree.
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 inputs produced controlled diagnostics.
- Skill validator — 13 skills valid. Link checker — 122 links in 188 Markdown
  files resolve.
- Focused same-batch benchmark (nine alternating runs): median 59.893 ms →
  34.414 ms; median peak RSS 33,396 KiB → 17,564 KiB. Raw runs and
  methodology are in `benchmarks/results/2026-11-27-string-buffer.md`.
- Full `benchmarks/run.py --reps 5` — identical output across all
  implementations; wrote `benchmarks/results/2026-11-27.{json,md}`. The host
  was slower during this run across every implementation, so only within-run
  comparisons are meaningful.

## Remaining

1. T-111 is committed as `45c438b`; CI is pending.
2. Select the next task.

## Independent review

Attempted on 2026-11-27 but the provider returned a free-usage limit error
before the subagent did any work; no independent review result is claimed.

## Risks / limits

- Final result strings are still never reclaimed; the v0.1 leak model is
  unchanged.
- The builder may over-allocate up to about twice the final length.
- Struct and enum display still use repeated concatenation.
- The benchmark is a single-machine microbenchmark with unpinned CPU
  frequency.
