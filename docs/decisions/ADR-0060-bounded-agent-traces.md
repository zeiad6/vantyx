# ADR-0060: Bounded metadata-only agent traces

- Status: Accepted; implemented in the AI host. Interpreter and native codegen
  are unaffected.
- Date: 2026-11-28
- Related: ADR-0015 (AI host boundary), ADR-0058 (bounded provider retries),
  `docs/ai/AI_ARCHITECTURE.md`, `docs/ai/AI_SECURITY.md`

## Context

The bounded agent loop already enforced step, tool-call, token, wall-clock, and
cancellation limits, but it exposed only the final text and counters. When a run
failed, callers received an `AiError` without knowing which step failed, which
tool was denied, or which provider response ended the run. Debugging required
logging full messages, which would copy user prompts, tool arguments, and tool
results into logs and conflict with the AI host's privacy rules.

A trace design therefore has to answer operational questions without retaining
payloads, and it must not let an adversarial provider grow memory through
model-controlled labels.

## Options

1. **Log the full message transcript.** Rejected: prompts, retrieved documents,
   tool arguments, and tool results are user or untrusted content. Retaining them
   by default violates the redaction and data-minimization rules.
2. **Record only a final error code.** Rejected: it does not show which step or
   tool failed, so it does not support diagnosis or evaluation.
3. **Add a callback or subscriber interface.** Deferred: it adds lifecycle and
   error-handling surface, and a subscriber can still leak payloads. A typed,
   returned trace is smaller and composes with existing metrics.
4. **Record a bounded, typed, metadata-only trace.** Chosen. Every event carries
   a step number and one of three kinds: a provider summary, a tool outcome, or a
   terminal stop code.

## Decision

- `AgentResult.trace` contains the trace of a successful run.
  `Agent::last_trace()` returns the trace of the most recent run, including a
  failed run. The trace is cleared at the start of every `Agent::run` call.
- Event kinds:
  - `Provider { tool_calls, finish_reason }` after each successful provider
    response.
  - `Tool { name, outcome }` for every requested tool call, where `outcome` is
    `Ok` or `Error { code }`. Invalid JSON arguments record `AI_E_JSON`; a denied
    or unknown tool records its capability error code.
  - `Stopped { code }` when cancellation, a step/tool/token/time budget, a
    provider failure, or another terminal error ends the run.
- The trace excludes prompts, system messages, assistant text, tool arguments,
  tool results, tool-call IDs, request IDs, headers, and error messages. Only
  codes and names are retained.
- Model- or provider-supplied labels are bounded: tool names and
  `FinishReason::Other` values keep at most 128 characters, and error/stop codes
  keep at most 64. Truncation is character-boundary safe and appends `…`.
- Trace length is bounded by the configured step and tool-call limits, plus one
  terminal event. No timer, thread, or background task is created.
- The trace is an in-process value. It is not written to disk, metrics, or the
  network automatically; a caller that persists it is responsible for its own
  retention policy.

## Consequences

- A failed run now reports where it stopped without exposing content.
- The trace is deterministic for deterministic providers, so tests can assert
  exact event sequences.
- Callers that want human-readable diagnostics still need the `AiError` message;
  the trace intentionally carries codes only.
- Adding a new event kind is a public API change and must keep the
  metadata-only rule.
- The trace does not replace payload capture for explicit debugging; that
  remains a separate, opt-in mode with a warning.

## Verification

- Unit tests cover a successful tool run, a denied tool, invalid arguments, a
  step-limit stop, initial cancellation, and truncation of oversized
  model-controlled labels.
- The tests assert that long tool names and finish reasons are absent from the
  formatted trace.
