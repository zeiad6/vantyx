# Checkpoint — T-075/T-076 committed, T-077 in progress

Date: 2026-11-24
Branch: `main`
HEAD: `de0bac7` (T-076 rustfmt baseline and CI formatting gate)
Remote: `origin/main` at `de0bac7`
Working tree: T-077 fuzzer change set uncommitted

## Completed and pushed

### T-075 — Result-enum propagation with postfix `?`

- Commit `66b4efc`; CI run 45503321172 passed with 621 tests and the ninth
  fuzzer shape.
- Statement-position `?` in `let` initializers, plain assignments, and
  expression statements; positional first-success/second-failure convention;
  `E0319` rejections; interpreter `Flow::Return`; native `IrStmtKind::Try`;
  specification §5.10; ADR-0038.
- Evidence: 5 e2e tests, 3 differential native tests, 180/180 fuzzer cases on
  seeds 1, 500, and 1000.

### T-076 — Repository rustfmt baseline and CI formatting gate

- Commit `de0bac7`; CI run 45504177763 passed.
- Mechanical `cargo fmt` across the repository; CI installs `rustfmt` and runs
  `cargo fmt --check --all` before building.
- Evidence: 621 tests, clippy clean, debug/release builds, five examples, and
  the formatting gate.

## In progress

### T-077 — Differential-fuzzer control-flow coverage and bounded shrinking

- Tenth `control_program` shape; opt-in `--shrink` with `--shrink-budget`;
  timeout exit `124`; seven Python self-tests in CI.
- Evidence so far: self-tests 6/6; tenth shape 20/20 on seeds 1–20; full
  ten-shape fuzzer 200/200 on seeds 1, 2, 3, 500, and 1000.
- Files: `tools/differential_fuzz.py`, `tools/test_differential_fuzz.py`,
  `.github/workflows/ci.yml`, and synchronized status/research/state docs.

## Next exact steps

1. Read the background fuzzer results for seeds 3, 500, and 1000; if they pass,
   update the T-077 evidence strings in `TASKS.md`, `PROJECT_STATUS.md`,
   `AGENTS.md`, `docs/PROJECT_BASELINE.md`, and the research audits.
2. Run the full gate: `cargo build`, `cargo build --release`,
   `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`,
   `cargo test`, the Python self-tests, the differential fuzzer, and all five
   examples.
3. Commit T-077, push to `origin/main`, watch CI, and record the run ID in
   `PROJECT_STATUS.md`, `AGENTS.md`, `AGENT_STATE/RECOVERY.md`, and this
   checkpoint directory.
4. Continue with the next unblocked P1/P2 task: metadata objects/preservation,
   recursive move, REPL multi-line history and field completion, or string
   performance.

## Limits and blockers

- T-040 local-model evidence remains blocked: no Ollama/llama.cpp/LM Studio
  server or model; only about 832 MiB of RAM was available. Cloud access is not
  guaranteed.
- T-017 async/concurrency and language-level AI syntax remain blocked on a
  design prerequisite.
- Independent `qa-verifier` work remains blocked by the subagent provider's
  usage limit.
- The shrinker is line-oriented, opt-in, and cannot split a line or guarantee a
  global minimum.
- Native allocations are intentionally leaked in v0.1; no cycle collector or
  sandbox.
