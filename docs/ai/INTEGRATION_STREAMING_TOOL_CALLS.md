# Integration proposal: bounded streamed tool-call assembly

Date: 2026-11-28
Owner: Agent 02 (AI platform, providers, agents, tools, RAG)
Status: implemented and verified in AI-owned files. Protected status files are
untouched and need the edits in §5.

## 1. What changed

Streaming chat previously carried text deltas only. The OpenAI-compatible
adapter rejected any request with `tools` before transport and rejected
`tool_calls` delta frames on the wire with `AI_E_UNSUPPORTED`, so a streaming
agent turn could not produce a tool call.

The host now has a provider-neutral streaming event API and a bounded
tool-call assembler:

- `Provider::chat_stream_events` delivers `StreamEvent::TextDelta` and
  `StreamEvent::ToolCallDelta` values in source order and returns the final
  `ChatResponse` with assembled `tool_calls`.
- `Provider::chat_stream` remains the text-only convenience method. It forwards
  only `TextDelta` events, so existing callers and the CLI are unchanged.
- `ai/stream.rs` owns argument assembly. It never executes a tool and never
  hands out a call with incomplete arguments.

The bounded agent loop still uses non-streaming `chat`; wiring streamed calls
into it is a separate change.

## 2. Semantics implemented

- Frames are keyed by provider `index`. Fragments for different indexes may be
  interleaved; `finish()` returns calls sorted by index.
- An id and name are required once per call and must stay identical across
  fragments. A conflicting or duplicate id, or a missing id/name at finish, is
  `AI_E_PROVIDER`.
- Argument fragments are concatenated byte-for-byte. `finish()` requires every
  call's arguments to parse as complete JSON; `""`, truncated JSON, and invalid
  JSON are rejected. Arguments are never truncated to make them parse.
- Assembly is bounded: 8 calls, 256-byte ids, 128-byte names, 64 KiB of
  arguments per call, 256 KiB total, and 4096 delta frames per response.
  Error messages identify the index and limit but never echo argument content.
- Text deltas and tool-call deltas share one event stream and one budget record.
  Completion token estimates include argument bytes when the provider omits
  usage.
- `RetryingProvider` retries only before the first event of either kind. After
  any text or tool-call delta the error is returned, so a retry can neither
  duplicate output nor deliver the same tool call twice.
- The adapter still rejects a tool call whose schema/name is not validated
  later: the agent/tool layer remains the only place where arguments are
  checked against a `ToolSpec` and executed under capability permissions.

## 3. Implementation

| File | Change |
| --- | --- |
| `src/ai/stream.rs` (new) | `StreamEvent`, `ToolCallDelta`, `ToolCallAssembler`, and the five public limits |
| `src/ai/mod.rs` | Module registration and re-exports |
| `src/ai/provider.rs` | Default `chat_stream_events` returning `AI_E_UNSUPPORTED` |
| `src/ai/openai.rs` | Parses fragmented `delta.tool_calls`, emits events, assembles calls, includes argument bytes in token estimates |
| `src/ai/retry.rs` | Event-stream retry wrapper with the same before-first-event rule |
| `tests/ai.rs` | Loopback SSE test for fragmented argument assembly over HTTP |
| `docs/ai/AI_ARCHITECTURE.md` | Module map, provider trait, streaming path, retry rule |
| `docs/ai/AI_ROADMAP.md` | AI-3 status and remaining cancellation work |
| `docs/ai/AI_SECURITY.md` | Streamed tool-call resource limits |

No dependency was added. No core, shared, or protected file was modified.

## 4. Verification

Run on 2026-11-28 on the AI branch working tree:

```text
cargo fmt --check --all                          clean
cargo clippy --all-targets -- -D warnings        clean
cargo test                                       859 passed, 0 failed
  476 library unit + 15 CLI unit + 57 AI integration
  + 145 e2e + 166 native
```

The AI delta is 16 library unit tests (12 assembler, 2 retry, 2 net OpenAI)
and 1 AI integration test. The rest of the total is the T-128 core work that
was already present in the tree.

New tests:

- `src/ai/stream.rs`: fragmented assembly, interleaved indexes, repeated
  identical identity fields, empty stream, conflicting identity, duplicate
  ids, missing identity, incomplete/invalid JSON, oversized id/name, too many
  calls, out-of-range indexes, per-call and total argument caps, delta-frame
  cap.
- `src/ai/openai.rs`: successful streamed assembly, text-only forwarding through
  `chat_stream`, and rejection of incomplete or invalid streamed arguments.
- `src/ai/retry.rs`: retries before the first event; does not retry after an
  emitted event.
- `tests/ai.rs`: loopback HTTP SSE frames assemble one `ToolCall` with the
  exact expected arguments and request body.

Not claimed: live provider verification of streamed tool calls. The live cloud
evidence predates this change and did not exercise tool-call streaming.

## 5. Required edits to protected files

These edits are required by repository rule 4 (behavior changes update
`TASKS.md` and `PROJECT_STATUS.md`). They are **not applied** here. Task ID
`T-129` is proposed; if Agent 01 claims it first, renumber this entry.

### 5.1 `TASKS.md` — append after T-127

```markdown
### T-129 — Bounded streamed tool-call assembly
- Priority: P1 (AI-3 streaming)
- Status: VERIFIED locally on 2026-11-28; commit pending; CI pending
- Description: Streaming chat carried text deltas only. The OpenAI-compatible adapter rejected requests with `tools` before transport and rejected `tool_calls` delta frames with `AI_E_UNSUPPORTED`, so a streamed agent turn could not produce a tool call.
- Acceptance: Provider-neutral text and tool-call events; fragmented arguments assembled by index; strict id/name consistency; complete-JSON validation at finish; no tool execution or schema bypass in the assembler; retries stop before the first event so a call is never delivered or executed twice; hard limits on calls, ids, names, arguments, total arguments, and delta frames.
- Dependencies: ADR-0023, ADR-0058.
- Implementation: `src/ai/stream.rs` adds `StreamEvent`, `ToolCallDelta`, `ToolCallAssembler`, and the public limits; `src/ai/provider.rs` adds default `chat_stream_events`; `src/ai/openai.rs` parses fragmented `delta.tool_calls`, emits events, assembles calls, and includes argument bytes in token estimates; `src/ai/retry.rs` retries event streams only before the first event.
- Tests: 16 new library unit tests (12 assembler, 2 retry, 2 net OpenAI) and `tests/ai.rs::streamed_tool_calls_assemble_over_loopback_http`.
- Verification: 2026-11-28 — `cargo fmt --check --all` and `cargo clippy --all-targets -- -D warnings` clean; `cargo test` = 859 passed (476 library unit + 15 CLI unit + 57 AI integration + 145 e2e + 166 native, 0 failed). The AI delta is 16 library unit + 1 AI integration; the rest includes concurrent T-128 core work. No live provider run exercised streamed tool calls.
- Scope limits: The bounded agent loop still uses non-streaming `chat`. There is no interactive cancellation, no CLI tool-call streaming, and no provider-side partial-argument callback; only the final assembled calls are returned. Live streamed tool-call evidence is pending.
```

### 5.2 `PROJECT_STATUS.md`

(a) In the Phase 6 / AI paragraph (line 19), replace:

```text
AI-3 streaming and structured output complete
```

with:

```text
AI-3 streamed text and bounded streamed tool-call assembly and structured output complete
```

and replace the closing sentence:

```text
Streaming tool calls, server-side constrained decoding, and language-level AI syntax remain open.
```

with:

```text
Interactive streaming cancellation, server-side constrained decoding, and language-level AI syntax remain open.
```

(b) In the "Last verification" paragraph (line 273), replace the opening:

```text
`cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`, and `cargo test` on 2026-11-28: 425 library unit + 15 CLI unit + 47 AI integration + 137 end-to-end + 165 native = 789 tests passed, clippy and fmt clean.
```

with:

```text
`cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`, and `cargo test` on 2026-11-28: 476 library unit + 15 CLI unit + 57 AI integration + 145 end-to-end + 166 native = 859 tests passed, clippy and fmt clean. T-129 adds provider-neutral streamed text/tool-call events and bounded tool-call argument assembly (8 calls, 64 KiB per call, 256 KiB total, 4096 frames), with retries stopping before the first event; its AI delta is 16 library unit + 1 integration test, and the rest of the total includes the concurrent T-128 tuple work.
```

### 5.3 `README.md`

Line 442 currently reads:

```markdown
- Streaming chat prints text deltas only; streaming tool calls, interactive
  cancellation, and server-side constrained decoding are not implemented.
```

Replace with:

```markdown
- Streaming chat prints text deltas only. The provider API also assembles
  bounded streamed tool calls, but the CLI and agent loop do not stream them;
  interactive cancellation and server-side constrained decoding are not
  implemented.
```

### 5.4 `ROADMAP.md`

Line 125 currently reads:

```markdown
2. **Streaming follow-ups:** streaming tool calls and interactive cancellation
```

Replace with:

```markdown
2. **Streaming follow-ups:** interactive cancellation and wiring assembled
   streamed tool calls into the agent loop
```

### 5.5 `AGENTS.md`

(a) In the AI host support bullet, after `streamed text chat with SSE and
incremental HTTP/curl transport,` insert `bounded streamed tool-call assembly,`.

(b) In the known-gaps list, replace `no streaming tool calls or interactive
cancellation,` with `no CLI/agent-loop streamed tool calls or interactive
cancellation,`.

## 6. Commit

Suggested commit message:

```text
feat(ai): assemble bounded streamed tool calls
```

## 7. Coordination notes

- All implementation changes are confined to `src/ai/**`, `tests/ai.rs`, and
  `docs/ai/**`.
- `docs/ai/AI_ARCHITECTURE.md` documents the new event API and limits; no core
  documentation or ADR file was changed.
- Task ID `T-129` is a proposal. If the core agent's tuple work claims it,
  renumber without changing the content.
