---
name: AI Systems Engineering
description: Build and audit model inference, provider adapters, local and cloud AI, embeddings, RAG, agents, multimodal input, structured output, tokenizers, evaluation, budgets, and observability with offline tests and secret safety.
---

# AI Systems Engineering

## When to use

- Integrating a model provider, local runtime, embedding model, vector index, or multimodal pipeline.
- Designing RAG, tool calling, agents, structured extraction, streaming, or token budgeting.
- Changing prompt construction, retrieval, output validation, evaluation, or AI observability.
- Auditing an AI feature for prompt injection, data leakage, cost blowups, or unbounded output.

Do not use this skill for model training or fine-tuning; use `model-training`. For kernel-level numerical work, use `gpu-numerical-performance`.

## Required tools

- An HTTP client with timeouts, retries, and streaming support.
- A JSON Schema validator and the project's typed request/response models.
- The exact tokenizer for the selected model, or a clearly labeled estimator if the tokenizer is unavailable.
- A mock provider or loopback HTTP server for deterministic tests.
- A vector index or retrieval library for RAG, plus an evaluation dataset.
- Optional: observability/tracing, cost calculator, prompt registry, sandbox for tool execution.

## Safety constraints

- Secrets come from environment variables or a secret manager. Never log, print, commit, or place them in prompts.
- Model output is untrusted input. Validate JSON, schemas, tool names, arguments, file paths, URLs, and code before use.
- Do not give a model unrestricted shell, filesystem, network, or credential access. Use allowlists and explicit permissions.
- Offline by default. No implicit downloads, telemetry, or public endpoint calls in tests.
- Enforce byte, token, step, tool-call, time, and cost budgets. Fail closed on budget exhaustion.
- Redact prompts, completions, file contents, and secrets from metrics and logs.
- Treat retrieved documents and tool outputs as potentially adversarial; delimit them and never let them override system policy.

## Workflow

1. **Build a capability matrix.** For each provider/model record: context window, modalities, streaming, tool calling, structured output, embeddings, rate limits, pricing, data retention, and regional constraints. Verify against current documentation; do not assume parity.

2. **Define the provider-neutral contract.** Specify request, response, streaming event, error, retry, timeout, cancellation, and idempotency semantics. Keep provider-specific wire formats in adapters.

3. **Implement adapters behind tests.** Map HTTP status codes and provider errors to stable internal errors. Test 401, 403, 404, 429 with `Retry-After`, 5xx, timeout, truncated stream, malformed JSON, and oversized response.

4. **Make token accounting exact.** Use the model's tokenizer. If only an estimator is available, label it as an estimate and leave headroom. Budget system prompt, retrieved context, tool schemas, conversation history, and output separately.

5. **Validate structured output.** Define a schema, validate, and allow at most a small bounded repair loop. On failure, return the last validation error and do not execute partially parsed tool calls.

6. **Design RAG as a measured pipeline.** Record chunking strategy, embedding model/version, index type, retrieval count, reranking, deduplication, freshness, and citation behavior. Build a labeled evaluation set and measure recall, precision, answer correctness, and latency.

7. **Constrain agents.** Give each agent a tool registry, per-tool permissions, argument schemas, step/token/time budgets, and a trace. Require approval for destructive or external side effects. Test prompt injection through retrieved text, tool output, and user content.

8. **Handle multimodal input explicitly.** Validate MIME type, size, dimensions, duration, and encoding. Provide OCR/transcription fallbacks. Never assume a model supports images, audio, video, or PDFs without checking the capability matrix.

9. **Evaluate before and after.** Use deterministic unit tests for parsing and validation, golden fixtures for prompts, and a held-out evaluation set for quality. Report latency, cost, token usage, and failure rates. Treat LLM-as-judge results as noisy evidence, not ground truth.

10. **Instrument safely.** Emit redacted metrics: request count, latency, token counts, error class, cache hit rate, retrieval count, and cost. Do not emit raw prompts, completions, secrets, or personal data.

11. **Document operations.** Include configuration, required environment variables, rate-limit behavior, fallback model, budget defaults, privacy notes, and a runbook for provider outages.

## Validation and acceptance criteria

- All deterministic tests run offline with a mock or loopback provider.
- Provider failure modes return stable errors and respect retry/timeout policy.
- Structured output is schema-validated; invalid output is never executed.
- Token and cost budgets are enforced and tested.
- RAG evaluation reports retrieval and answer metrics on a labeled set.
- Agent tools have explicit permissions and argument validation; injection tests exist.
- No secret appears in logs, metrics, errors, or test output.
- The capability matrix and unsupported features are documented.
- Live-provider tests, if any, are labeled with date, endpoint, model, and observed result.

## Expected output

1. Capability matrix and provider-neutral contract.
2. Adapter implementation with mocked failure-path tests.
3. Token-budget and structured-output validation code.
4. RAG pipeline with an evaluation report.
5. Agent/tool permission model and injection tests.
6. Redacted observability and cost metrics.
7. Configuration and operations runbook.
8. Explicit list of unverified providers, models, and modalities.

## Troubleshooting

- **Streaming hangs:** verify SSE/JSONL frame parsing, final-frame handling, EOF, cancellation, and timeout. Test a truncated stream.
- **429 or quota errors:** honor `Retry-After`, use bounded exponential backoff with jitter, and fail with a clear error after the budget.
- **Schema drift:** pin the provider/API version, validate at the boundary, and add a contract test with a recorded fixture.
- **Token count differs from billing:** use the exact tokenizer and model revision; label estimates and leave headroom.
- **RAG answers are irrelevant:** inspect chunking, embedding version, retrieval count, reranking, and evaluation labels before tuning prompts.
- **Prompt injection succeeds:** separate instructions from data, delimit retrieved content, deny dangerous tools, and add a regression test.
- **Costs spike:** enforce per-request and per-session budgets, cap output tokens, cache embeddings, and log redacted usage.
- **Local model fails to load:** check memory, quantization, device, and runtime version; record the probe output instead of falling back silently.

## Examples

```sh
# Deterministic offline test
cargo test --test ai
python3 -m pytest tests/ai -q

# Check a local endpoint without leaking the key
curl -sS --max-time 10 http://127.0.0.1:11434/api/tags | head

# Validate a JSON fixture against a schema
python3 - <<'PY'
import json, jsonschema
schema = json.load(open("schema.json"))
data = json.load(open("fixture.json"))
jsonschema.validate(data, schema)
print("valid")
PY

# Inspect tokenizer behavior
python3 - <<'PY'
from transformers import AutoTokenizer
tok = AutoTokenizer.from_pretrained("model-id")
print(len(tok.encode("hello world")))
PY
```

## Related skills and review

- Pair with `security-audit-engineering` for prompt injection, tool permissions, sandboxing, and secret handling.
- Pair with `model-training` for fine-tuning and `performance-benchmarking` for latency, throughput, and cost claims.
- Pair with `gpu-numerical-performance` for local inference kernels and quantization.
- Reviewer checks: offline tests, schema validation, budgets, redaction, injection tests, and honest capability claims.
