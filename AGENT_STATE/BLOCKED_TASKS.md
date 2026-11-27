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
- Independent `qa-verifier` verification of T-062–T-066 and T-072–T-106 —
  Blocked by the subagent provider's free usage limit. The primary session's
  tests and sanitizer runs remain the recorded evidence; retry independent
  verification when capacity returns.
- Continuously available live cloud AI — The 2026-11-24 Gemini run succeeded
  and a 20:59Z re-verification passed all 21 checks, but an earlier request
  was rejected with HTTP 400 `User location is not supported` and one re-run
  hit a transient HTTP 503. This is an external provider availability pattern;
  no bypass is attempted and no continuous-availability claim is made.
