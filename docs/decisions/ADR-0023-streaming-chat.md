# ADR-0023 — Streamed OpenAI-compatible chat

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none (extends ADR-0015)
- Related: ADR-0015 (AI host foundation), `src/ai/sse.rs`, `src/ai/http.rs`,
  `src/ai/openai.rs`, `src/ai_cli.rs`, `docs/ai/AI_ARCHITECTURE.md`

## Context

The AI host could send chat requests and validate structured output, but it
always buffered the whole provider response. A local or remote model that takes
seconds to finish therefore produced no visible output until the last byte
arrived. The SSE parser, capability flags, and HTTP transport already existed,
but no provider exposed a streaming method and the CLI had no live-output path.

Streaming is also a prerequisite for honest latency work: first-token latency
and incremental delivery cannot be measured or improved while the client waits
for a complete body. The design must preserve the existing guarantees:

- zero third-party Rust dependencies;
- loopback HTTP by default, remote HTTPS only with explicit permission;
- redacted errors and bounded responses;
- the interpreter and native compiler remain outside this layer;
- unsupported provider features fail with a stable error rather than silently
  degrading.

Alternatives were:

1. Leave chat non-streaming and document the limitation.
2. Add a separate streaming client outside the provider abstraction.
3. Extend the existing `Provider` trait with a text-delta callback and implement
   it for the OpenAI-compatible adapter first.

## Decision

Add text streaming through the existing provider and transport abstractions.

1. **Provider interface.** `Provider::chat_stream(request, budget, on_delta)`
   defaults to `AI_E_UNSUPPORTED`. `OpenAiProvider` implements it for
   OpenAI-compatible endpoints and advertises `streaming = true`; the mock,
   echo, and hash-embedding providers keep `streaming = false`. Capability
   negotiation happens before transport, so a caller cannot accidentally send a
   stream request to a provider that will buffer or reject it.
2. **Request shape.** A stream request sends the normal chat body plus
   `"stream": true` and `"stream_options": {"include_usage": true}`. It replaces
   any caller-supplied `Accept` header with `Accept: text/event-stream`.
   Parameters that were already validated remain unchanged; streaming does not
   add a second parameter path.
3. **Text deltas only.** The adapter aggregates `choices[0].delta.content`
   fragments, forwards each non-empty fragment to the callback, and records the
   provider-reported `id`, `model`, `finish_reason`, and `usage`. If usage is
   absent it falls back to the same local token estimates used by non-streaming
   chat, and the budget records the final total once. A `[DONE]` event stops
   processing; later frames are ignored.
4. **Tool calls fail closed.** A request that includes tools is rejected with
   `AI_E_UNSUPPORTED` before transport, and a stream frame containing
   `delta.tool_calls` is rejected with the same code. Streaming tool calls need
   argument-fragment assembly and a permission story that this increment does
   not implement. They must not be silently dropped.
5. **Error handling.** Error frames are sanitized through the existing secret
   redaction path and returned as `AI_E_PROVIDER`. Non-2xx responses buffer at
   most 64 KiB of body and map through the existing status-error rules (401/403
   to `AI_E_AUTH`, 429 to retryable `AI_E_RATE_LIMIT`, and so on). The final
   response body is bounded by the transport limit.
6. **SSE parsing.** `SseParser::push_bytes` accepts raw bytes, carries an
   incomplete UTF-8 sequence across chunk boundaries, accepts `\n` and `\r\n`
   line endings, and enforces the existing event-size limit. A malformed stream
   is `AI_E_PROVIDER`, not a panic or a partial success.
7. **HTTP transport.** `Transport::stream` takes `StreamHandlers` with
   `on_start` and `on_chunk`. The loopback implementation decodes
   `Content-Length`, chunked, and close-delimited bodies incrementally and skips
   interim 1xx responses. HTTPS uses the system `curl --include --no-buffer`,
   parses the response head, forwards body bytes as they arrive, and uses the
   existing network policy and timeout rules. This keeps HTTPS support free of a
   TLS dependency and preserves the documented `curl` boundary.
8. **CLI.** `vantyx ai chat --stream` prints deltas as they arrive, flushes
   stdout after each fragment, and prints one final newline (or
   `(empty response)` when no text arrived). `--stream` is accepted only with
   `--provider openai` and cannot be combined with `--json`; both rejections
   exit 64 before any network request. Metrics record the final usage once.

## Consequences

Positive:

- First-token latency is visible and measurable instead of hidden behind a full
  response.
- Streaming reuses the same request validation, capability checks, budget,
  redaction, and transport policy as non-streaming chat.
- Loopback streaming is fully testable offline, including split UTF-8,
  chunked/close-delimited bodies, error frames, and CLI output.
- The HTTPS path works with the existing zero-dependency policy.

Negative:

- Streaming tool calls are not supported and are rejected rather than
  assembled.
- There is no interactive cancellation keybinding; a caller can stop only by
  ending the process or relying on the request timeout.
- HTTPS streaming depends on a system `curl` that supports `--no-buffer`; the
  fallback is an explicit `AI_E_CONNECT` error.
- Usage is estimated when a provider omits the usage frame, so cost accounting
  remains an estimate in that case.
- No live local or cloud model has been exercised; loopback and stub tests prove
  the protocol and output path, not model behavior.

## Follow-ups

- Add streaming tool-call assembly with explicit permission checks.
- Add cooperative cancellation that is visible to the transport and CLI.
- Record live-provider first-token latency and throughput once a local model or
  approved cloud endpoint is available.
- Consider SSE `event:`/`id:` fields only if a provider requires them; the
  current adapter consumes `data:` frames and comments.
