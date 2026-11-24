# Task Queue

Ordered by priority. The orchestrator updates this file after each checkpoint.

## In progress

- T-070/T-071/T-072 — Commit and push the native condition and short-circuit
  fixes plus the deterministic differential fuzzer; watch CI and record the run
  ID. Local gate: 584 tests, clippy clean, release build, examples, 100/100
  fuzz cases.

## Ready

- T-019a — `vantyx lint` (first half of T-019, P2 tooling): `L0001` unused
  local binding, `L0002` mutable binding never reassigned, `L0003` unreachable
  statement after `return`/`break`/`continue`; unit, CLI, and e2e tests;
  `docs/linting.md`; keep the formatter as a separate tracked task.
- T-070/T-071 follow-up — Consider replacing GNU statement expressions with a
  statement prelude only if benchmarks and review justify the larger codegen
  change; do not start without evidence.
- Differential fuzzer follow-up — Add shrinking and optional sanitizer runs
  when they can be made deterministic and bounded.

## Blocked

- T-040 — Live local-model and trained embedding-model evidence. No Ollama,
  llama.cpp, or LM Studio server/model is installed; only about 832 MiB of RAM
  was available. The cloud OpenAI-compatible path was verified on 2026-11-24
  and re-verified at 20:59Z, but access is not guaranteed.
- T-017 — Async/concurrency and language-level AI syntax. Blocked on an async
  design; host-side AI remains available through the CLI and Rust API.
- GPU, tensor, SIMD, autodiff, and training evidence — Blocked on verified
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
- Formatter (second half of T-019) — requires preserving comments, which the
  current lexer discards; treat as a separate design task.
