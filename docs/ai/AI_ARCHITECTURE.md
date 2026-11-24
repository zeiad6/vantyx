# AI Architecture

Status: AI-0 design complete; AI-1 host foundation implemented and verified offline; AI-2 compatible adapters implemented and the cloud path verified live on 2026-11-24 (local-model verification still blocked); AI-3 structured output and streamed text chat complete (streaming tool calls and cancellation pending); AI-4 host-side embeddings and retrieval complete (a cloud embedding model was exercised on 2026-11-24; local embedding models pending)
Date: 2026-11-24
Related: `AI_RESEARCH.md`, `AI_SECURITY.md`, `AI_ROADMAP.md`, ADR-0007,
ADR-0014, ADR-0015, ADR-0017, ADR-0018, ADR-0024, ADR-0025

## Position

Vantyx should make AI applications natural without turning the compiler into an
ML framework. The compiler stays small, predictable, and dependency-free. AI
capabilities live in a host-level library and CLI first, then become language
features only when async and a standard library exist. File-based modules now
exist, so async is the remaining language prerequisite for the conceptual
`from ai import Client` surface.

This avoids two failure modes:

1. **Fake language support** — syntax that parses but cannot run.
2. **Compiler bloat** — a language core coupled to a fast-moving model runtime.

## Two tracks

| Track | Scope | Current state |
| --- | --- | --- |
| A — Core Vantyx | Lexer, parser, checker, interpreter, native backend, memory, modules, tooling | v0.1 subset verified; modules implemented; async/generics missing |
| B — AI Vantyx | Providers, local inference, structured output, embeddings, RAG, tools, agents, evaluation | Offline foundation, compatible-endpoint adapter, structured extraction, streamed text chat, embedding providers/cache, local retrieval/evaluation, incremental vector reuse, and MMR diversity reranking implemented; a live cloud provider and embedding model were verified on 2026-11-24 and re-verified at 20:59Z with 21/21 checks, while local live providers, streaming tool calls, and grounded generation remain pending; cloud access is not guaranteed |

Track B must not wait for the entire Track A roadmap. It may use the host CLI and
Rust API while language-level integration remains blocked.

## Layers

```text
┌─────────────────────────────────────────────────────────────┐
│ Language surface (future)                                   │
│   from ai import Client / await / typed tools               │
│   BLOCKED on async (T-017) and stdlib; modules (T-013) done  │
├─────────────────────────────────────────────────────────────┤
│ Host API: src/ai/                                           │
│   provider · json · sse · tokens · embedding · bm25         │
│   embedding_provider · tools · agent · budget · metrics     │
│   config                                                    │
├─────────────────────────────────────────────────────────────┤
│ Transport boundary                                          │
│   std::net for loopback HTTP · curl for HTTPS (opt-in)      │
│   No implicit network. No model downloads.                  │
├─────────────────────────────────────────────────────────────┤
│ External providers                                          │
│   Ollama · llama.cpp · LM Studio · OpenAI-compatible cloud  │
│   mock provider for deterministic tests                     │
└─────────────────────────────────────────────────────────────┘
```

## Module map

| Module | Responsibility | Must not do |
| --- | --- | --- |
| `ai/json.rs` | Strict JSON value, parser, writer, schema subset | Accept trailing garbage or NaN |
| `ai/provider.rs` | Typed chat and embedding requests/responses, roles, content parts, usage, capabilities, errors, default `embed` | Pretend all providers support all fields |
| `ai/embedding_provider.rs` | Deterministic lexical hash embeddings and a bounded FIFO cache | Claim semantic quality or unbounded growth |
| `ai/mock.rs` | Deterministic offline chat provider | Hit the network |
| `ai/openai.rs` | OpenAI-compatible request/response mapping | Log secrets or prompts by default |
| `ai/http.rs` | Bounded HTTP transport (loopback std, HTTPS via curl) | Follow arbitrary redirects or exceed limits |
| `ai/sse.rs` | Incremental server-sent-events parser | Buffer without bound |
| `ai/tokens.rs` | Labelled token estimate and budget checks | Claim exact model tokenization |
| `ai/extract.rs` | Strict JSON extraction, schema validation, bounded repair, usage accounting | Return unvalidated model output or exceed the repair limit |
| `ai/embedding.rs` | Vector math, normalization, in-memory index | Hide O(n) search cost |
| `ai/bm25.rs` | Deterministic lexical retrieval | Claim semantic retrieval |
| `ai/rag.rs` | Bounded ingestion, stable line-cited chunks, persistent index, incremental vector reuse/pruning, lexical/semantic/hybrid search, reciprocal-rank fusion, retrieval evaluation | Claim answer grounding without retrieved sources or exceed ingestion limits |
| `ai/tools.rs` | Tool registry, JSON schemas, capabilities, allowlists | Auto-grant host permissions |
| `ai/agent.rs` | Bounded plan/act loop, cancellation, state | Run without step/token/time limits |
| `ai/budget.rs` | Deterministic limits and errors | Silently truncate or overspend |
| `ai/metrics.rs` | Opt-in JSONL metrics, redaction | Capture prompts by default |
| `ai/config.rs` | Environment/file configuration and secret lookup | Write secrets to disk or logs |

## Core types (conceptual)

```rust
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stop: Vec<String>,
    pub tools: Vec<ToolSpec>,
    pub response_format: Option<ResponseFormat>,
    pub timeout_ms: Option<u64>,
}

pub struct ChatResponse {
    pub id: Option<String>,
    pub model: String,
    pub message: Message,
    pub finish_reason: FinishReason,
    pub usage: Usage,
}

pub struct EmbeddingRequest {
    pub model: String,
    pub inputs: Vec<String>,
    pub dimensions: Option<usize>,
    pub timeout_ms: Option<u64>,
}

pub struct EmbeddingResponse {
    pub model: String,
    pub vectors: Vec<Vector>,
    pub usage: EmbeddingUsage,
}

pub trait Provider {
    fn id(&self) -> &str;
    fn capabilities(&self) -> &ProviderCapabilities;
    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError>;
    fn embed(
        &self,
        request: &EmbeddingRequest,
        budget: &mut Budget,
    ) -> Result<EmbeddingResponse, AiError> {
        Err(AiError::unsupported("embeddings", self.id()))
    }
    fn chat_stream(
        &self,
        request: &ChatRequest,
        budget: &mut Budget,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<ChatResponse, AiError> {
        let _ = (request, budget, on_delta);
        Err(AiError::unsupported("streaming", self.id()))
    }
}
```

The exact Rust signatures may evolve; the invariants may not:

- A request is validated before transport.
- Unsupported parameters produce an error, not a silent drop.
- Every call consumes budget and records a metric when enabled.
- Errors carry a stable code and a redacted message.

## Provider capability negotiation

Each provider declares:

```text
chat, streaming, tools, json_schema, json_object, embeddings,
vision, audio_in, audio_out, seed, stop, max_context_tokens
```

The caller checks capabilities before building a request. Example: if a provider
does not support `json_schema`, the caller may use `json_object` plus local
validation, or fail with a clear error. It must not claim constrained decoding.

## Local inference path

1. The user installs and starts Ollama or llama.cpp separately.
2. The user names a model explicitly.
3. Vantyx calls `http://127.0.0.1:<port>/v1/chat/completions` or the Ollama
   native endpoint.
4. The model must already be present; Vantyx never downloads weights
   automatically.
5. `vantyx ai doctor` reports endpoint reachability and model availability
   without printing secrets.

The same boundary supports LM Studio and any OpenAI-compatible server.

## Cloud path

- Explicit `--allow-network`.
- API key from an environment variable or the OS keyring; never a project file.
- Host allowlist and HTTPS-only for non-loopback endpoints.
- Bounded timeout, response size, and retries with jitter/backoff.
- No prompt/response logging by default.
- Provider errors are mapped to stable Vantyx codes.

## Structured output path

```text
model text
  → strict JSON parse
  → schema validation (subset)
  → typed conversion
  → on failure: bounded repair retry (default 1, max 3)
  → on repeated failure: typed error with the validation path
```

Implemented in `ai/extract.rs` and exposed as `vantyx ai extract`. Providers
without `json_schema` or `json_object` are rejected; the OpenAI-compatible
adapter currently sends `json_object`. Repair messages and invalid text are
bounded, and every attempt consumes the shared budget. Server-native constrained
decoding is preferred when available but is not claimed yet; local validation
remains mandatory because server support varies and may be disabled.

## Streaming path

```text
validated ChatRequest
  → provider capability check (streaming, no tools)
  → POST with stream: true, stream_options.include_usage,
    Accept: text/event-stream
  → transport bytes (content-length, chunked, or close-delimited;
    HTTPS via curl --include --no-buffer)
  → SSE parser (raw bytes, UTF-8 carry, bounded events)
  → delta callback + final text/finish reason/usage
  → one budget record + optional redacted metric
```

- `Provider::chat_stream` defaults to `AI_E_UNSUPPORTED`; only the
  OpenAI-compatible adapter advertises `streaming = true` today.
- Text deltas are forwarded as they arrive. `[DONE]` stops processing; frames
  after it are ignored. A malformed frame is `AI_E_PROVIDER`, never a partial
  success.
- Streaming tool calls are rejected before transport and on the wire with
  `AI_E_UNSUPPORTED`; argument-fragment assembly and its permission model are
  not implemented.
- Error frames pass through secret redaction. Non-2xx bodies are bounded to
  64 KiB for status mapping, and the overall body remains bounded by the
  transport limit.
- The CLI prints deltas with an explicit flush and rejects `--stream` with
  `--json` or a non-OpenAI provider before any request. There is no interactive
  cancellation keybinding yet.
- HTTPS streaming depends on the system `curl`; the loopback path uses
  `std::net` and is covered by offline tests. Live first-token latency and
  throughput remain unmeasured until a real model is available.

## Embeddings and retrieval path

```text
documents → bounded ingest → chunk → embed → persisted index
query     → embed → cosine + BM25 → reciprocal-rank fusion → cited top-k
                                                    ↓
                                      optional MMR diversity reranking
                                                    ↓
                                      retrieval evaluation (recall/precision/MRR)
```

- `ai/rag.rs` implements the host-side path and stops before generation. It
  never claims that an answer is grounded merely because chunks were retrieved.
- Ingestion is confined to one canonical root and skips symlinks, hidden paths,
  VCS/build directories, and extensions outside an allowlist. File count,
  per-file bytes, total source bytes, chunk count, chunk size, overlap, query
  size, top-k, and embedding input sizes are bounded.
- Chunk ids are stable FNV-1a values over source, line range, and text. Every
  hit carries `source`, `start_line`, and `end_line`.
- Indexes are strict JSON (format version 1), validated before lexical state is
  rebuilt, saved by atomic rename, and written with mode 0600 on Unix.
- Incremental updates (`vantyx ai index --incremental`, ADR-0024) reuse stored
  vectors when the stable chunk id, provider, model, and dimensions match,
  embed only new or changed chunks, and prune chunks absent from the current
  corpus. A no-op update constructs no provider and makes no embedding call.
  Every update still walks and chunks the complete corpus; there is no
  file-manifest shortcut, partial-prefix merge, or sub-chunk diffing.
- `Provider::embed` accepts batches of up to 128 inputs (8 KiB each, 32 KiB per
  request) and 1–4096 requested dimensions. Invalid requests fail before
  transport.
- `HashEmbeddingProvider` is a deterministic lexical baseline over tokens and
  character trigrams, L2-normalized; it is **not** a trained semantic model.
- `CachingEmbeddingProvider` is a bounded FIFO cache (default 1,024 entries /
  4 MiB) keyed by model, dimensions, and text. Repeated inputs in a batch are
  sent once; the cache is process-local and clearable.
- `OpenAiProvider::embed` posts to `/v1/embeddings`, sorts items by index,
  rejects duplicate/out-of-range indexes, validates count and dimensions, and
  records provider usage.
- Cosine similarity requires equal dimensions; dimension mismatch is an error.
- An optional relative `--source-prefix` filter ranks only matching chunks before
  top-k. It is a retrieval convenience, not an access-control boundary.
- BM25 is lexical. Hybrid retrieval fuses lexical and semantic ranks with
  reciprocal-rank fusion (k = 60) and deterministic id tie breaking. Results
  label both scores and both ranks.
- Search scans vectors linearly. This is honest for local corpora and is not a
  production vector database.
- `--diverse` (ADR-0025) applies deterministic MMR reranking over a bounded
  wider candidate pool, suppresses identical trimmed chunk text, uses cosine
  similarity when every candidate has a vector and BM25-token Jaccard
  otherwise, and normalizes relevance by the best candidate score. It is a
  context-efficiency control; measured retrieval-quality improvement is not
  claimed.
- Evaluation reports recall@k, precision@k, MRR, and per-query retrievals over
  exact chunk-id or source-path labels. It does not measure answer accuracy,
  hallucination rate, or grounding.

## Tools and agents

- Tools are registered explicitly with a name, description, JSON input schema,
  output type, and required capabilities.
- The model receives only the tools in the current policy.
- Tool arguments are validated before execution.
- Tool results are untrusted text and are labelled as such.
- The agent loop enforces max steps, max tool calls, token budget, wall-clock
  deadline, and cancellation.
- No shell tool is enabled by default. A shell tool, if ever added, requires an
  explicit capability and approval.

## Observability

- Metrics are off by default.
- When enabled, record provider, model, latency, token counts, finish reason,
  error code, retry count, cache hit, and tool name.
- Prompt and response payloads are never recorded unless a separate
  `capture_payloads` opt-in is set; that mode prints a warning.
- JSONL is the first sink; OpenTelemetry is a later adapter.

## Failure model

| Failure | Behavior |
| --- | --- |
| Network disabled | `AI_E_NETWORK_DISABLED` with the exact flag to enable |
| Endpoint unreachable | `AI_E_CONNECT`, marked retryable; no automatic retry loop yet |
| HTTP 401/403 | `AI_E_AUTH`; never echo the key |
| HTTP 429 | `AI_E_RATE_LIMIT`, marked retryable; `Retry-After` handling is not implemented yet |
| Timeout | `AI_E_TIMEOUT`; cancel the request |
| Unsupported parameter | `AI_E_UNSUPPORTED` naming the parameter and provider |
| Invalid JSON | `AI_E_JSON` with byte offset |
| Schema mismatch | `AI_E_SCHEMA` with the failing path |
| Budget exhausted | `AI_E_BUDGET` naming the limit |
| Tool denied | `AI_E_CAPABILITY` naming the missing capability |

On the HTTPS path, system `curl` exit code 28 (operation timeout) maps to
retryable `AI_E_TIMEOUT`; other transport failures such as exit 7 remain
`AI_E_CONNECT`. Both buffered and streaming requests share this mapping
(T-068).
| Agent step limit | `AI_E_STEPS` with partial state |

## Testing strategy

- Unit tests for JSON, SSE, token estimation, vector math, BM25, schemas, and
  budgets.
- Embedding tests cover deterministic normalized vectors, batch limits, cache
  hits/misses/eviction/deduplication, loopback `/v1/embeddings`, response index
  and dimension validation, and unsupported-provider errors.
- Deterministic mock/echo providers for chat, tools, and failure injection.
  Streaming is covered by OpenAI-compatible stub transports, a loopback SSE CLI
  test, HTTP content-length/chunked/close-delimited tests, and an HTTP/HTTPS
  smoke script; no provider streams through an unverified path.
- A loopback HTTP test server for the OpenAI-compatible adapter; no internet.
- Security tests for denied capabilities, secret redaction, oversized responses,
  timeouts, and prompt-injection labels.
- No test requires a real model or API key. Live tests are opt-in and documented.

## What is deliberately not in this layer

- Model weight loading or GPU kernels.
- A universal tokenizer claim.
- Autonomous multi-agent orchestration.
- Automatic model downloads.
- Language syntax changes before async and a standard library exist.
- Claims of sandboxing that the host cannot enforce.

## Language integration gate

A future `from ai import Client` requires:

1. T-013 modules and imports.
2. T-017 async/await and cancellation.
3. A standard-library packaging story (T-019/T-020).
4. Typed error propagation (`Result`/`?`, T-016).
5. A native or interpreter bridge with resource limits.

Until then, the host API and CLI are the supported surface. This is a deliberate
boundary, not an oversight.
