# Active Task

## T-077 — Differential-fuzzer control-flow coverage and bounded shrinking

- Status: VERIFIED locally; commit, push, and CI pending
- Priority: P1 (tooling/correctness)
- Dependencies: T-072, T-070, T-071, T-075

### Implemented

- A tenth deterministic shape, `control_program`, covering `while` conditions
  with calls and short-circuiting, list/map/range iteration (including
  negative range steps), nested loops, `break`/`continue`, match statements
  inside loops, early returns from loops and match arms, guard side effects,
  compound assignment, and inner-block shadowing.
- `CaseResult`, `execute_case`, `describe`, and `shrink_source` factor the
  comparison and minimization logic. `--shrink` is opt-in and
  `--shrink-budget` caps the extra comparisons. The shrinker preserves the
  failure's exit-code signature and skips candidates that no longer fail.
- `run` maps a subprocess timeout to exit `124` instead of raising an
  exception, so a hanging program is reported as a differential failure.
- `tools/test_differential_fuzz.py` adds six unit tests for agreement,
  both-engine rejection, build-failure detection, runtime mismatch detection,
  shrink line removal, and shrink budgeting.
- `.github/workflows/ci.yml` runs the self-tests before the differential fuzz.
- `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`, `docs/PROJECT_BASELINE.md`,
  `RESEARCH/INITIAL_REPOSITORY_AUDIT.md`, and
  `RESEARCH/HISTORICAL_REQUIREMENTS_AUDIT.md` are updated, including the
  previously malformed T-072/T-073 audit row.

### Evidence

- `python3 -m py_compile tools/differential_fuzz.py` — success.
- `python3 tools/test_differential_fuzz.py` — 7 passed.
- Tenth shape on seeds 1–20 — 20/20 agreed.
- `python3 tools/differential_fuzz.py --iterations 20 --seed 1` — 20/20.
- `python3 tools/differential_fuzz.py --iterations 200` — 200/200 on seeds 1, 2, 3, 500, and 1000.
- Seeds 3, 500, and 1000 at 200 iterations are still running.
- CI run 45504177763 passed on `de0bac7` for T-076.

### Remaining

- Finish the 200-iteration runs on seeds 3, 500, and 1000, record them, commit,
  push, watch CI, and add a checkpoint.
- Scope limits: the shrinker is line-oriented and cannot split a line or
  guarantee a global minimum; shrinking is opt-in; the fuzzer still lacks
  sanitizer integration and coverage measurement.

## Recently completed

- T-076 rustfmt baseline and CI formatting gate: committed in `de0bac7`; CI
  run 45504177763 passed.
- T-075 result-enum propagation: committed in `66b4efc`; CI run 45503321172
  passed with 621 tests and the ninth fuzzer shape.
- T-072 fuzzer expansion: committed in `53a8b82`; CI run 45499019615 passed.
