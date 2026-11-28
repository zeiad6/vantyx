# ADR-0058: Bounded provider retries with `Retry-After`

- Status: Accepted; implemented in the AI host for OpenAI-compatible chat,
  streaming, model listing, and embeddings. Interpreter and native codegen are
  unaffected.
- Date: 2026-11-28
- Related: ADR-0015 (AI host boundary), ADR-0039 (provider evaluation order),
  `docs/ai/AI_ARCHITECTURE.md`, `docs/ai/AI_SECURITY.md`

## Context

The OpenAI-compatible adapter already classified HTTP 429 and 500–599 as
retryable, and `MetricEvent` already had a `retry_count` field. However:

- No component retried a failed request, so callers had to re-run the whole
  command.
- `Retry-After` was ignored.
- `retry_count` was always zero, even when a caller retried by hand.
- A naive retry loop would duplicate streamed text if it reissued a request
  after deltas had reached the user.
- Retries could bypass the wall-clock budget used by agents and structured
  extraction.
- Provider-specific retry code would spread across the adapter and CLI.

The goal is bounded, observable, provider-neutral retry behavior without
weakening the network policy, secret redaction, or streaming safety.

## Options

1. **Retry inside `OpenAiProvider`.** This is the smallest code change, but it
   ties retry behavior to one adapter and makes it hard to apply to a future
   provider or to test with a deterministic stub.
2. **Retry in every CLI command.** This duplicates loops in `chat`, `embed`,
   `models`, `extract`, and RAG indexing, and risks inconsistent limits.
3. **Add a provider decorator with a bounded policy.** `RetryingProvider`
   implements the existing `Provider` trait and delegates to any provider. The
   CLI wraps the OpenAI-compatible adapter once; tests can wrap a scripted
   stub.
4. **Unbounded retry with jitter until success.** Rejected: it violates the
   bounded-resource rule and makes failure and cost unpredictable.

## Decision

Option 3, with these rules:

- **Policy.** Default 2 retries after the initial attempt (3 attempts total).
  The hard ceiling is 8 retries. Backoff is 250 ms, 500 ms, 1 s, 2 s, then
  capped at 4 s. Delays are deterministic; no jitter is added, so offline tests
  and timing assertions stay reproducible.
- **Eligibility.** Only `AiError::retryable` errors are retried. `AI_E_CANCELLED`
  is never retried. Authentication, configuration, validation, capability,
  schema, budget, and format errors return immediately. `AI_E_CONNECT` remains
  non-retryable so a bad endpoint or missing local server fails quickly with a
  concrete diagnostic rather than a delayed one.
- **`Retry-After`.** The integer-seconds form is honored. HTTP-date and
  malformed values are ignored rather than guessed. The parser caps a value at
  60 s. If the requested delay is greater than the policy's 4 s ceiling, the
  retry stops and the error is returned instead of retrying earlier than the
  server requested.
- **Streaming.** A streaming call is retried only before the first text delta.
  Once any delta has been delivered, an error is returned and no second request
  is made, because a retry would duplicate output. A stream that fails before
  any delta may be retried normally.
- **Operations.** Chat, streaming chat, model listing, and embeddings use the
  same decorator and policy. Structured extraction accumulates retries across
  its bounded repair attempts.
- **Budgets.** Before and after each backoff, the decorator checks the caller's
  wall-clock budget. An exhausted budget stops retries with `AI_E_BUDGET`. Token
  budgets are still checked by the underlying provider before each attempt and
  recorded only on success.
- **Configuration.** `VANTYX_AI_RETRIES` and `--retries <n>` set the count.
  Values above 8 are rejected by the CLI and clamped by the environment parser.
  `0` disables automatic retries.
- **Observability.** `Provider::last_retry_count()` reports the retries from the
  most recent call. Chat, extract, and embed metric events record it in the
  existing `retry_count` field. Prompts, responses, headers, and secrets are
  never recorded.

## Consequences

- Transient 429 and 5xx failures recover without a full command re-run, within a
  bounded number of attempts and a bounded delay.
- A provider that asks for a long delay is not retried early; the caller sees
  the error and the requested delay in the CLI diagnostic.
- Streamed output cannot be duplicated by a retry.
- The retry loop is provider-neutral and testable with scripted stubs and
  loopback HTTP servers. No new dependency was added.
- Total elapsed time can exceed one request timeout because each attempt has
  its own timeout and backoff is added between attempts. The wall-clock budget,
  when configured, remains the hard stop.
- Retrying a non-idempotent provider operation is not attempted after a
  response has been observed; only failed attempts are reissued. Chat
  completions and embeddings may still be billed for an attempt whose response
  was lost, which is inherent to at-least-once retry semantics.
- `AI_E_CONNECT` is still not retried. A local server that is still starting
  needs an explicit re-run or a larger caller-level policy.
- HTTP-date `Retry-After` values are not parsed; providers that use them fall
  back to exponential backoff.

## Verification

- Unit tests cover retry success, non-retryable errors, the retry limit, a
  disabled policy, `Retry-After` parsing and ceiling behavior, exponential
  backoff capping, cancellation, budget exhaustion, model-list retries,
  embedding retries, and streaming before/after the first delta.
- Offline loopback tests cover a 429 then success with a metrics record, an
  exhausted 429 sequence, a 401 that is not retried, `--retries 0`, an oversized
  `Retry-After`, model-list retries, embedding retries, and a partial stream
  that must not be retried.
- No test requires internet access, a real key, or a model.
