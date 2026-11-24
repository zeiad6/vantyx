# AI Roadmap

Status: AI-0 and AI-1 complete; AI-2 in progress (offline and compatible-endpoint foundation verified; live local/cloud evidence pending); AI-4 embedding provider interface and batch cache complete
Date: 2026-11-24
Related: `AI_ARCHITECTURE.md`, `AI_RESEARCH.md`, `AI_SECURITY.md`,
`RESEARCH/HISTORICAL_REQUIREMENTS_AUDIT.md`

## Principles

- Ship only what runs, is tested, and is documented.
- Keep the compiler dependency-free; AI lives at the host boundary first.
- Local-first and privacy-preserving by default.
- No automatic downloads, no unbounded agents, no fake GPU claims.
- Every performance statement has a methodology and raw data.
- Every phase has an exit gate; do not skip ahead to demos.

## Phase AI-0 — Research and architecture ✅

- [x] Historical requirement audit.
- [x] Naming audit and rename decision.
- [x] Current ecosystem research with primary sources.
- [x] Architecture and security model.
- [x] Dependency decision: zero-dep host foundation, HTTP boundary first.
- [x] Roadmap and acceptance gates.

Exit gate: documents exist, decisions are explicit, and no feature is claimed.

## Phase AI-1 — Offline foundation ✅

Implemented in `src/ai/`:

- [x] Strict JSON value, parser, writer (depth/size limits, no duplicate keys).
- [x] Provider trait, request/response types, capabilities, stable errors.
- [x] Mock and echo providers with deterministic responses and failure injection.
- [x] Token estimate and deterministic budget.
- [x] Embedding vector math, normalization, cosine/dot.
- [x] In-memory vector index.
- [x] BM25 lexical retrieval.
- [x] JSON Schema subset validation.
- [x] Tool registry with capabilities and allowlists.
- [x] Bounded agent loop with cancellation.
- [x] Metrics sink (off by default, redacted).
- [x] CLI: `vantyx ai providers`, `vantyx ai doctor`, `vantyx ai chat --provider mock`.
- [x] Unit and integration tests; no network required.

Verification (2026-11-23): `cargo test` passed offline with 357 tests (218
library unit + 11 CLI unit + 20 AI integration + 41 end-to-end + 67 native);
`cargo clippy --all-targets -- -D warnings` was clean. Mock chat, echo chat,
stdin prompts, JSON output, metrics redaction, denied tool capabilities,
cancellation, and a loopback OpenAI-compatible round trip run end-to-end.

Exit gate: met. `cargo test` passes offline; a mock chat and a bounded tool loop
run end-to-end; security tests cover denied capabilities and redaction.

## Phase AI-2 — Local and cloud providers 🚧

- [x] OpenAI-compatible adapter (chat, model list, usage; non-streaming).
- [x] llama.cpp/LM Studio and Ollama compatibility through the same
      OpenAI-compatible endpoint path.
- [x] Explicit `--allow-network`; loopback default.
- [x] Environment resolution for provider, model, endpoint, key variable,
      timeout, and response size.
- [x] `vantyx ai models` and `vantyx ai doctor` against a compatible endpoint.
- [ ] Ollama native adapter and model discovery beyond the compatible endpoint.
- [ ] Config file and keyring adapter (environment variables only today).
- [ ] `vantyx ai info`, `pull`, and `remove` with integrity and license display;
      downloads remain out of scope until checksums and provenance exist.
- [ ] Live tests, opt-in and documented; no keys in CI.

Exit gate: one real local model and one cloud provider complete a chat with
recorded evidence; failure paths (401, 429, timeout) are tested.

Exit gate: one real local model and one cloud provider complete a chat with
recorded evidence; failure paths (401, 429, timeout) are tested.

## Phase AI-3 — Streaming and structured output 🚧

- [x] Incremental SSE parser with size limits.
- [ ] Streaming CLI output and cancellation (parser exists; adapter and CLI do
      not stream yet).
- [x] JSON Schema subset validation.
- [x] Typed extraction pipeline and `vantyx ai extract` command.
- [ ] Server-native constrained decoding when advertised.
- [x] Bounded repair retries with validation paths.
- [x] Token accounting and request limits (estimates and provider-reported
      usage; budgets enforce steps, tools, and tokens).

Exit gate: streamed output matches non-streamed content; invalid JSON is rejected
with a path; a repair retry is bounded and observable. Local validation, path
reporting, and bounded repair are implemented; streaming remains open.

## Phase AI-4 — Embeddings and retrieval 🚧

- [x] Embedding provider interface (offline hash and OpenAI-compatible
      `/v1/embeddings`; ADR-0017).
- [x] Batch embeddings and bounded FIFO cache with batch deduplication.
- [x] In-memory vector index with deletion and cosine search.
- [x] BM25 lexical retrieval.
- [ ] Vector index persistence.
- [ ] BM25 + cosine hybrid fusion.
- [ ] Document ingestion for text, Markdown, and code.
- [ ] Chunking with metadata and stable IDs.
- [ ] Citations and grounding metadata.
- [ ] Evaluation harness: recall@k, MRR, precision, latency.

Evidence (2026-11-24): 383 tests pass, including deterministic hash embeddings,
batch limits, cache hits/eviction/deduplication, loopback `/v1/embeddings`, and
`vantyx ai embed` JSON/stdin/error paths. The hash provider is lexical and is
not a trained semantic model; live local/cloud embedding-model verification and
retrieval evaluation remain open.

Exit gate: a licensed fixture corpus produces reproducible retrieval metrics;
answers cite retrieved chunks or explicitly say they are not grounded.

## Phase AI-5 — Tools and agents 🚧

- [x] Typed tool schemas and argument validation.
- [x] Capability policy, path roots, host allowlists.
- [x] Approval policy for write, shell, and network capabilities.
- [x] Agent loop, cancellation flag, and step/tool/token/time limits.
- [ ] Agent persistence and resume.
- [x] Tool-call error mapping and redacted metrics.
- [x] Prompt-injection fixtures (denied capability and untrusted-output tests).
- [ ] Write, shell, and network tools (no such tools ship yet).

Exit gate: an agent completes a multi-step task within limits; a malicious
retrieved document cannot invoke a denied tool; cancellation is deterministic.

## Phase AI-6 — Multimodal

- [x] Typed text and image content parts in the provider protocol.
- [ ] Audio and video content parts.
- [ ] Size/format validation and resource limits.
- [ ] Capability checks per model.
- [ ] Image understanding example; audio transcription example when a backend is
      available.
- [ ] Clear "not supported by this model" errors.

Exit gate: at least one real multimodal request works on a verified backend; all
unsupported combinations fail clearly.

## Phase AI-7 — Tensor and numerical foundations

- [ ] Decide build vs integrate (Burn/Candle/tract) with an ADR.
- [ ] Tensor shape/dtype/stride semantics and tests.
- [ ] Matmul, reductions, broadcasting.
- [ ] NaN/inf/overflow/determinism tests.
- [ ] Numerical benchmark vs an established library.

Exit gate: a documented decision, correct semantics, and measured results. No
custom autodiff until the tensor layer is stable.

## Phase AI-8 — GPU and optimization

- [ ] Verify real hardware and a backend that builds and runs tests.
- [ ] Device discovery and selection; CPU fallback.
- [ ] Memory limits and transfers.
- [ ] Quantization, batching, KV/prefix cache where the backend supports it.
- [ ] Reproducible tokens/s, first-token latency, memory, and cost metrics.

Exit gate: hardware-specific evidence. Until then, status is `BLOCKED`, not
"supported".

## Track A dependencies

| AI need | Core dependency | Status |
| --- | --- | --- |
| `from ai import Client` | Modules/imports (T-013) | Missing |
| `await client.generate` | Async (T-017) | Missing |
| Typed errors (`Result`/`?`) | T-016 | Missing |
| Standard library packaging | T-019/T-020 | Missing |
| Native tensor types | Generics/traits, native memory | Missing |
| Secure tool execution | Sandbox design | Missing |

The host API is not blocked by these; language-level syntax is.

## Priority order

1. **P0:** AI-1 foundation, secrets, capability policy, offline tests.
2. **P1:** AI-2 one local provider, one cloud provider, streaming, structured
   output, embeddings, RAG, bounded agents.
3. **P1 core:** modules, nested/or-patterns, native reclamation.
4. **P2:** model manager, evaluation harness, observability, cross-platform CI.
5. **P3+:** multimodal, MCP, ONNX/GGUF embedding, tensors.
6. **P4:** GPU, autodiff, multi-agent orchestration — only with hardware and
   evidence.

## Non-goals for the first release

- Training foundation models.
- A universal model format.
- A universal tokenizer.
- Automatic model downloads.
- Unbounded autonomous agents.
- A claim of sandboxing.
- GPU support without hardware tests.
- Replacing mature inference engines.

## Tracking

Tasks are recorded in `TASKS.md` under the `AI` prefix. Status claims require an
observed command, a date, and a test or benchmark reference. Documentation alone
is never sufficient.
