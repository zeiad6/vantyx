# Blocked Tasks

- T-040 — Live local-model and trained embedding-model evidence. Blocked: no
  Ollama, llama.cpp, or LM Studio server/model is installed in this
  environment. The cloud OpenAI-compatible half was verified on 2026-11-24
  (T-067) and re-verified at 20:59Z with 21/21 checks, but an earlier retry
  returned HTTP 400 `User location is not supported` and access is not
  guaranteed. Resumable once a local server or model is available; record
  setup, versions, hardware, first-token latency, throughput, and raw output.
- T-017 / language-level `from ai import Client` — Blocked on async/concurrency
  design and implementation. Host-side AI remains available through the CLI
  and Rust API.
- GPU, tensor, SIMD, autodiff, and training evidence — Blocked on verified
  hardware and a reproducible backend. No performance or capability claim is
  made.
- Native memory reclamation design review — Deferred; a prior
  `compiler-architect` subagent run failed with “Free usage limit reached”.
  Retry when subagent capacity is available.
- Independent `qa-verifier` verification of T-062–T-066 and T-072–T-117 —
  Blocked by the subagent provider's free usage limit. The primary session's
  tests and sanitizer runs remain the recorded evidence; retry independent
  verification when capacity returns.
- Delegated T-118/T-119 implementation and the 2026-11-27 T-117 QA, spec-sync,
  security, and divergence-hunt reviews — The first six child sessions failed
  immediately with `Free usage limit reached` and produced no report or file
  change. A later retry started `qa-verifier`, `spec-sync-auditor`,
  `memory-model-engineer`, and `backend-engineer`, but all four also failed with
  `Free usage limit reached` and returned no result. The backend child left a
  partial T-121 `src/codegen_c.rs` edit in its worktree; the primary session
  completed it, verified it with the 770-test gate, and recorded the evidence.
  No independent review exists for T-118/T-119/T-120/T-121; independent review
  is claimed only if a child returns exact commands and evidence.
- Continuously available live cloud AI — The 2026-11-24 Gemini run succeeded
  and a 20:59Z re-verification passed all 21 checks, but an earlier request
  was rejected with HTTP 400 `User location is not supported` and one re-run
  hit a transient HTTP 503. This is an external provider availability pattern;
  no bypass is attempted and no continuous-availability claim is made.
