# Integration proposal: bounded provider retries and agent traces

Date: 2026-11-28
Owner: Agent 02 (AI platform)
Status: both features are implemented and verified in AI-owned files; shared
status files are untouched and need the edits below.

## What changed

### 1. Bounded provider retries (ADR-0058)

The AI host retries retryable provider failures with a bounded,
provider-neutral policy instead of returning every 429/5xx immediately.

- New `src/ai/retry.rs`: `RetryPolicy`, `RetryingProvider`,
  `retry_with_count`, `retry_call`, `retry_after_from_header`.
- `src/ai/provider.rs`: `AiError::retry_after`, `AiError::with_retry_after`,
  `Provider::models` and `Provider::last_retry_count` defaults.
- `src/ai/openai.rs`: `Retry-After` parsing for buffered and streaming
  responses; trait `models` implementation.
- `src/ai/config.rs`: `retry_count` and `VANTYX_AI_RETRIES`.
- `src/ai/metrics.rs`: `MetricEvent::retries`.
- `src/ai/extract.rs`: `Extraction.retries`, accumulated across repair attempts.
- `src/ai_cli.rs`: `--retries`, provider wrapping, retry metrics, diagnostics.
- Tests: 16 new library tests and 9 new loopback integration tests.

Defaults: 2 retries after the first attempt, hard maximum 8, deterministic
exponential backoff 250 ms → 4 s, integer-seconds `Retry-After` capped at 60 s.
A delay above 4 s stops retries instead of retrying early. Streaming is retried
only before the first text delta. Only errors marked retryable are retried;
authentication, validation, capability, schema, budget, cancellation, and
`AI_E_CONNECT` errors are not.

### 2. Bounded metadata-only agent traces (ADR-0060)

- `AgentResult.trace` and `Agent::last_trace()` expose a typed trace of the most
  recent run, including a failed run.
- Events record the step number plus a provider summary
  (`tool_calls`, `finish_reason`), a tool name with `ok` or an error code, or a
  terminal stop code.
- Prompts, messages, tool arguments, tool results, tool-call IDs, headers, and
  error messages are never copied into the trace.
- Model- or provider-supplied labels are truncated to 128 characters and codes
  to 64, so an adversarial response cannot grow the trace without bound.
- Trace length is bounded by the configured step and tool-call limits plus one
  terminal event. No background task is created.
- Tests: 6 new library tests cover success, denial, invalid arguments, the step
  limit, initial cancellation, and oversized labels.

## Verification

Clean worktree at `8fa373b` with only AI-owned files copied in:

- `cargo fmt --check --all` clean.
- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` = 820 passed (447 library unit + 15 CLI unit + 56 AI integration
  + 137 e2e + 165 native), 0 failed.
- `python3 tools/test_differential_fuzz.py` = 9 tests passed.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` = 100/100 agree.
- All five `examples/*.vtx` run.
- `python3 tools/check_links.py` = all relative links resolve.
- Manual smoke: `vantyx ai --help`, `ai providers`, `ai doctor --provider mock`,
  and a mock chat all behaved as before.

No internet, key, model, or new dependency was used.

## Exact shared-file edits requested

### 1. `TASKS.md` — add two entries

> Numbering note: `T-128` and `T-129` are deliberately left free for Agent 01's
> in-flight tuple work. If the integrator assigns those IDs to core tasks, keep
> the AI entries at the next free block; renumbering does not change the
> implementation.

```markdown
### T-130 — Bounded provider retries with `Retry-After`
- Priority: P2 (provider reliability and interoperability)
- Status: VERIFIED locally on 2026-11-28; not committed
- Description: HTTP 429/5xx responses were marked retryable but no component
  retried them; `Retry-After` was ignored and the metric `retry_count` was
  always zero.
- Acceptance: Retry only retryable errors, default 2 and hard maximum 8,
  deterministic capped backoff, integer-seconds `Retry-After` with a 4 s policy
  ceiling, no retry after a streamed delta, wall-clock budget checks, and a
  populated metric retry count.
- Implementation: `src/ai/retry.rs`, `src/ai/provider.rs`, `src/ai/openai.rs`,
  `src/ai/config.rs`, `src/ai/metrics.rs`, `src/ai/extract.rs`,
  `src/ai_cli.rs`; ADR-0058.
- Tests: 16 library tests plus 9 loopback integration tests.
- Verification: 2026-11-28 — fmt and clippy clean; `cargo test` = 820 passed
  (447 + 15 + 56 + 137 + 165); differential fuzz 100/100; link checker passed.
  Worktree at `8fa373b`; no commit.
- Scope limits: HTTP-date `Retry-After` is ignored; `AI_E_CONNECT` is not
  retried; a retry can duplicate billing for a lost response; total elapsed
  time can exceed one timeout by the sum of attempts and backoff.

### T-131 — Bounded metadata-only agent traces
- Priority: P2 (agent observability and failure diagnosis)
- Status: VERIFIED locally on 2026-11-28; not committed
- Description: a failed agent run returned only an `AiError`; there was no
  step, tool, or terminal-stop record, and logging full messages would retain
  prompts, arguments, and results.
- Acceptance: typed trace with provider, tool, and terminal events; successful
  and failed runs both observable; no payload text; model-controlled labels
  bounded; trace length bounded by the existing step and tool-call limits.
- Implementation: `src/ai/agent.rs`, `src/ai/mod.rs`; ADR-0060.
- Tests: 6 library tests.
- Verification: 2026-11-28 — fmt and clippy clean; `cargo test` = 820 passed
  (447 + 15 + 56 + 137 + 165); no protected file changed.
- Scope limits: codes only, no human-readable error messages in the trace; the
  trace is in-process and not persisted automatically; streaming tool calls
  remain unimplemented.
```

### 2. `PROJECT_STATUS.md` — update the AI and test-count lines

- Test count: 789 → 820, with the breakdown above.
- AI host bullet: add "bounded provider retries (default 2, max 8,
  `Retry-After`, retry-safe streaming, retry metrics; ADR-0058)" and
  "metadata-only agent traces with bounded labels (ADR-0060)".
- Known gaps: remove "no automatic retries" if present; keep "HTTP-date
  `Retry-After` is not parsed" and add "agent traces carry codes, not
  human-readable failure messages".

### 3. `docs/PROJECT_BASELINE.md` — update the tests row

Change `789 passing` and its breakdown to `820 passing` with
`447 library unit + 15 CLI unit + 56 AI integration + 137 e2e + 165 native`,
and note ADR-0058 and ADR-0060.

### 4. `AGENTS.md` — current baseline block

Add a short clause after the AI host paragraph: bounded provider retries are
implemented (default 2, hard max 8, `Retry-After`, no retry after a streamed
delta, retry counts in metrics; ADR-0058), and agent runs expose bounded,
metadata-only traces (ADR-0060). No rule changes are needed.

### 5. `README.md` — optional

If the AI feature list mentions retries or agent observability, add "bounded
provider retries with `Retry-After`" and "metadata-only agent traces". Otherwise
no change is required.

## Not requested

- No `Cargo.toml`/`Cargo.lock` change.
- No compiler, interpreter, checker, runtime, or native backend change.
- No commit, tag, or push.
