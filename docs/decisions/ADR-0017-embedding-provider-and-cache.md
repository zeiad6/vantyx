# ADR-0017 — Embedding provider interface and bounded cache

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0015 (AI host foundation),
  `docs/ai/AI_ARCHITECTURE.md`, `docs/ai/AI_SECURITY.md`

## Context

The AI host foundation already had vector math, cosine similarity, an in-memory
vector index, and BM25, but no way to obtain vectors from a provider. That made
semantic search and RAG impossible to exercise end to end. The next AI-4 step
had to add embeddings without adding a third-party crate, downloading a model
implicitly, or claiming that a lexical hash is a trained semantic model.

Two paths were available:

1. Add `fastembed`/`ort`/Candle and ship an in-process model. This conflicts
   with the zero-dependency rule and would require an ADR exception, model
   files, and a much larger security and packaging surface.
2. Add a provider-neutral embedding request to the existing host boundary,
   support OpenAI-compatible `/v1/embeddings` endpoints (including local Ollama
   and llama.cpp servers), and provide a deterministic offline provider for
   tests and demos.

## Decision

Use the existing provider boundary for embeddings.

1. **One provider trait.** `Provider` gains a default `embed` method that returns
   `AI_E_UNSUPPORTED`. Chat-only providers keep compiling and cannot silently
   pretend to embed. `OpenAiProvider` overrides `embed`; `HashEmbeddingProvider`
   implements embeddings only.
2. **Batch-first request.** `EmbeddingRequest` always carries a list of inputs.
   The limits are 128 inputs, 8 KiB per input, 32 KiB per request, and 4096
   requested dimensions. Empty inputs, empty models, zero timeouts, and invalid
   dimensions are rejected before transport.
3. **Deterministic offline provider.** `HashEmbeddingProvider` uses signed
   feature hashing over lowercased alphanumeric tokens and character trigrams,
   then L2-normalizes. It is explicitly a lexical baseline for tests and
   plumbing, **not** a trained semantic model. Its `id` is `hash`.
4. **Bounded cache.** `CachingEmbeddingProvider` is an in-memory FIFO cache with
   default limits of 1,024 entries and 4 MiB. The key includes model, requested
   dimensions, and input text. Repeated inputs in one batch are sent to the
   provider once. Entries larger than the byte limit are not cached. `clear`
   resets contents and counters. The cache is process-local and is not a
   cross-process or cross-tenant cache.
5. **OpenAI-compatible adapter.** `OpenAiProvider::embed` sends
   `POST /v1/embeddings` with `model`, `input`, and optional `dimensions`. It
   sorts response items by `index`, rejects duplicate/out-of-range indexes,
   rejects a count mismatch, validates finite values and consistent dimensions,
   checks the requested dimensions, records provider usage against the budget,
   and redacts secrets in errors.
6. **CLI surface.** `vantyx ai embed [options] [text...]` supports offline
   `mock`/`hash`, OpenAI-compatible `openai`, `--dimensions`, `--json`, stdin
   batches (one input per non-empty line), timeouts, metrics, and the existing
   network/key policy. `echo` is rejected because it has no embeddings.
7. **No new dependency.** The implementation uses `std` plus the existing HTTP
   transport. No model is downloaded or bundled.

## Consequences

Positive:

- Semantic-search plumbing can be built and tested offline.
- Local and cloud embedding servers share one adapter.
- A caller can swap providers without changing request code.
- Cache growth is bounded and documented.
- Unsupported embedding calls fail with a stable error instead of returning
  fabricated vectors.

Negative and remaining work:

- There is no in-process trained embedding model; `hash` captures lexical
  overlap only.
- The cache is in-memory and process-local; it is not persisted and has no TTL.
- There is no embedding-provider discovery, model listing, or cost estimate.
- There is no RAG pipeline, chunking, citation, or retrieval evaluation yet.
- The OpenAI-compatible adapter depends on the server implementing
  `/v1/embeddings`; model and server support vary and are not guessed.
- Cached vectors are stored in process memory; a caller handling sensitive text
  must treat the cache as sensitive memory and clear it when appropriate.

## Alternatives rejected

- **Add `fastembed`/`ort` now.** Rejected: violates ADR-0007, adds a large native
  dependency, and requires model files, checksums, and a packaging decision.
- **Call a Python embedding process.** Rejected: adds an undeclared runtime,
  process protocol, and packaging burden for a first implementation.
- **Cloud-only embeddings.** Rejected: breaks the local-first and offline-test
  requirements.
- **Unbounded cache.** Rejected: long-running processes would leak memory.
- **Reuse the chat mock as an embedding model without a capability change.**
  Rejected: it would claim embeddings that the mock does not provide.

## Security notes

- Embedding inputs are bounded before any network call.
- The cache stores raw input text as part of its key; it is process memory, not
  a log, and is not written to disk.
- Metrics record provider, model, operation, latency, and usage only; no input
  text or vectors.
- API keys still come only from the named environment variable; errors are
  redacted with the same path as chat.
- The hash provider is deterministic and safe for untrusted text, but it is not
  a privacy or semantic guarantee.

## Evidence

- `src/ai/provider.rs`: request/response types, validation, capabilities, default
  `embed`.
- `src/ai/embedding_provider.rs`: hash provider, cache, unit tests.
- `src/ai/openai.rs`: `/v1/embeddings` adapter and unit tests.
- `src/ai_cli.rs`: `vantyx ai embed` and CLI tests.
- `tests/ai.rs`: offline deterministic CLI, stdin batch, loopback adapter, and
  loopback CLI tests.
- `examples/ai/embeddings.md`: reproducible offline and local-server demos.

Verification is recorded in `TASKS.md` T-045 and `PROJECT_STATUS.md`; a feature is
not complete until the full gate and CI are observed.
