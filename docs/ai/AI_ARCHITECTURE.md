# AI Architecture

Status: AI-0 design complete; AI-1 host foundation implemented and verified offline; AI-2 in progress
Date: 2026-11-23
Related: `AI_RESEARCH.md`, `AI_SECURITY.md`, `AI_ROADMAP.md`, ADR-0007,
ADR-0014, ADR-0015

## Position

Vantyx should make AI applications natural without turning the compiler into an
ML framework. The compiler stays small, predictable, and dependency-free. AI
capabilities live in a host-level library and CLI first, then become language
features only when modules, async, and a standard library exist.

This avoids two failure modes:

1. **Fake language support** — syntax that parses but cannot run.
2. **Compiler bloat** — a language core coupled to a fast-moving model runtime.

## Two tracks

| Track | Scope | Current state |
| --- | --- | --- |
| A — Core Vantyx | Lexer, parser, checker, interpreter, native backend, memory, modules, tooling | v0.1 subset verified; modules/async/generics missing |
| B — AI Vantyx | Providers, local inference, structured output, embeddings, RAG, tools, agents, evaluation | Offline foundation and compatible-endpoint adapter implemented; live providers, streaming, and RAG pipeline pending |

Track B must not wait for the entire Track A roadmap. It may use the host CLI and
Rust API while language-level integration remains blocked.

## Layers

```text
┌─────────────────────────────────────────────────────────────┐
│ Language surface (future)                                   │
│   from ai import Client / await / typed tools               │
│   BLOCKED on modules (T-013), async (T-017), stdlib         │
├─────────────────────────────────────────────────────────────┤
│ Host API: src/ai/                                           │
│   provider · json · sse · tokens · embedding · bm25         │
│   tools · agent · budget · metrics · config                 │
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
| `ai/provider.rs` | Typed request/response, roles, content parts, usage, capabilities, errors | Pretend all providers support all fields |
| `ai/mock.rs` | Deterministic offline provider | Hit the network |
| `ai/openai.rs` | OpenAI-compatible request/response mapping | Log secrets or prompts by default |
| `ai/http.rs` | Bounded HTTP transport (loopback std, HTTPS via curl) | Follow arbitrary redirects or exceed limits |
| `ai/sse.rs` | Incremental server-sent-events parser | Buffer without bound |
| `ai/tokens.rs` | Labelled token estimate and budget checks | Claim exact model tokenization |
| `ai/embedding.rs` | Vector math, normalization, in-memory index | Hide O(n) search cost |
| `ai/bm25.rs` | Deterministic lexical retrieval | Claim semantic retrieval |
| `ai/rag.rs` (planned) | Chunking, retrieval, citations, grounding metadata | Claim grounding without retrieved sources |
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

pub trait Provider {
    fn id(&self) -> &str;
    fn capabilities(&self) -> &ProviderCapabilities;
    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError>;
    // Streaming is planned for AI-3; the SSE parser exists but no provider
    // exposes a streaming method yet.
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
  → on failure: bounded repair retry (default 1)
  → on repeated failure: typed error with the validation path
```

Constrained decoding is preferred when the server supports it. Local validation
remains mandatory because server support varies and may be disabled.

## Embeddings and retrieval path

```text
documents → chunk → embed → index
query     → embed → cosine / BM25 → fuse → top-k → citations
```

- The first index is in-memory and deterministic.
- Cosine similarity requires equal dimensions; dimension mismatch is an error.
- BM25 is lexical, not semantic; hybrid results label both scores.
- Retrieval evaluation uses a small, licensed fixture set with recall@k and MRR.

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
| Agent step limit | `AI_E_STEPS` with partial state |

## Testing strategy

- Unit tests for JSON, SSE, token estimation, vector math, BM25, schemas, and
  budgets.
- Deterministic mock/echo providers for chat, tools, and failure injection;
  SSE parsing is unit-tested separately because no provider streams yet.
- A loopback HTTP test server for the OpenAI-compatible adapter; no internet.
- Security tests for denied capabilities, secret redaction, oversized responses,
  timeouts, and prompt-injection labels.
- No test requires a real model or API key. Live tests are opt-in and documented.

## What is deliberately not in this layer

- Model weight loading or GPU kernels.
- A universal tokenizer claim.
- Autonomous multi-agent orchestration.
- Automatic model downloads.
- Language syntax changes before modules/async exist.
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
