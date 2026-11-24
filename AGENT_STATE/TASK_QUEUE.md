# Task Queue

Ordered by priority. The orchestrator updates this file after each checkpoint.

## In progress

- T-076 — Commit and push the rustfmt baseline and CI formatting gate; watch
  CI and record the run ID. T-075 is committed in `66b4efc` with CI run
  45503321172 green.

## Ready

- Differential-fuzzer follow-ups — Deterministic shrinking for failing
  programs, optional sanitizer runs, and coverage measurement. Add each only
  with a bounded, reproducible contract.
- T-019b — Comment-preserving formatter. The lexer discards comments today, so
  this needs a lossless syntax representation and a separate ADR before code.
- Linter follow-ups — Suppression/configuration, unused parameters and loop
  variables, all-terminating `if`/`match` reachability, and a deny mode. Add
  each only with a stable contract and tests.
- Repository-wide rustfmt baseline — `cargo fmt --check` currently reports 224
  pre-existing diff locations. Do this as a dedicated mechanical commit with a
  CI check, separate from semantic changes.
- T-070/T-071 follow-up — Consider replacing GNU statement expressions with a
  statement prelude only if benchmarks and review justify the larger codegen
  change; do not start without evidence.

## Blocked

- T-040 — Live local-model and trained embedding-model evidence. No Ollama,
  llama.cpp, or LM Studio server/model is installed; only about 832 MiB of RAM
  was available. The cloud OpenAI-compatible path was verified on 2026-11-24
  and re-verified at 20:59Z, but access is not guaranteed.
- T-017 — Async/concurrency and language-level AI syntax. Blocked on an async
  design; host-side AI remains available through the CLI and Rust API.
- GPU, tensor, SIMD, autodiff, and training evidence — Blocked on available
  hardware and a reproducible backend.
- Independent `qa-verifier` verification — Blocked by the subagent provider's
  free usage limit.
- Native memory reclamation design review — Deferred until subagent capacity
  returns.
- Windows native support — Unverified; CI is Linux-only.

## Backlog candidates

- Recoverable `Result`/`?` errors and error propagation.
- Metadata objects or metadata preservation for file operations.
- Recursive move and atomic copy/write.
- Immutable string concatenation and multibyte iteration optimization.
- REPL multi-line history and field completion.
