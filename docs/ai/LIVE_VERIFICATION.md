# Live AI Verification

Status: cloud OpenAI-compatible path exercised successfully on 2026-11-24 at
19:24Z–19:27Z and re-verified at 20:59Z; local-model evidence is still missing.
An earlier retry was rejected with a geographic restriction, and a later retry
hit a transient HTTP 503, so access is not guaranteed.
Related: `AI_ARCHITECTURE.md`, `AI_ROADMAP.md`, `AI_SECURITY.md`, `TASKS.md`
(T-040, T-067, T-068), `tools/live_ai_check.py`,
`tools/ai_failure_harness.py`, `docs/ai/live-evidence-2026-11-24.json`,
`docs/ai/live-evidence-2026-11-24-retry.json`,
`docs/ai/live-evidence-2026-11-24-retry2.json`

## What was run

The live checks used the Gemini OpenAI-compatible endpoint:

```text
endpoint: https://generativelanguage.googleapis.com/v1beta/openai
chat model: gemini-3.1-flash-lite
embedding model: gemini-embedding-001
embedding dimensions: 768
credential: read from an environment variable; never printed or stored
```

The exact commands are reproducible with:

```sh
export MY_API_KEY=...   # never commit this value

python3 tools/live_ai_check.py \
  --binary target/debug/vantyx \
  --endpoint https://generativelanguage.googleapis.com/v1beta/openai \
  --model gemini-3.1-flash-lite \
  --embedding-model gemini-embedding-001 \
  --api-key-env MY_API_KEY \
  --dimensions 768 \
  --out /tmp/live-evidence.json
```

`GOOGLE_API_KEY` is the harness default; the 20:59Z run used the
`MY_API_KEY` variable from the shell configuration through
`--api-key-env MY_API_KEY`. The key value is never printed or stored.

The failure paths are reproducible without a real key:

```sh
python3 tools/ai_failure_harness.py \
  --binary target/debug/vantyx \
  --out /tmp/ai-failure-harness.json
```

## Observed cloud results (2026-11-24, UTC)

| Check | Observed result | Evidence |
| --- | --- | --- |
| `vantyx ai doctor` | `status: ok`; 65 models listed | live run at 19:24Z |
| `vantyx ai chat --json` | `text: "VANTYX LIVE"`, `finish_reason: "stop"`, usage 22 prompt / 4 completion / 26 total | 19:24Z |
| `vantyx ai chat --stream` | streamed text `one two three four five six seven eight nine ten` | 19:26Z |
| `vantyx ai extract --schema examples/ai/person.schema.json --json` | validated `{"name":"Ada Lovelace","role":"engineer","years":12,"skills":["mathematics","computing"]}`; `attempts: 2` (one repair); usage 237 / 105 / 342 | 19:26Z |
| `vantyx ai embed --dimensions 768 --json` | two 768-component vectors; usage 28 prompt / 28 total | 19:25Z |
| `vantyx ai index` over a 3-file corpus | 3 documents, 3 chunks, 3 vectors, 768 dimensions; usage 206 prompt / 206 total | 19:26Z |
| `vantyx ai search --mode semantic --top-k 3` | ranked `vantyx.md` first (score 0.606483), then `copy.md` (0.487423), then `gemini.md` (0.485869) | 19:26Z |
| `vantyx ai eval --mode semantic --top-k 2` | 3 queries; recall@2 = 1.0, precision@2 = 0.5, MRR = 1.0 | 19:27Z |

The corpus and queries were written for this check: `vantyx.md` describes the
interpreter/native C backend, `gemini.md` describes the OpenAI-compatible
endpoint, and `copy.md` describes `copy_dir_all`. The evaluation fixture labels
one relevant document per query. The metrics are retrieval metrics only; they do
not prove grounded generation.

## Re-verification at 20:59Z (2026-11-24)

A second full harness run used `MY_API_KEY` through
`--api-key-env MY_API_KEY` and passed all 21 checks. Evidence:
`docs/ai/live-evidence-2026-11-24-retry2.json`.

| Check | Observed result |
| --- | --- |
| `doctor` | `status: ok`; 65 models listed |
| `chat --json` | `VANTYX LIVE`; usage 13 prompt / 4 completion / 17 total |
| `chat --stream` | `one two three four five`; first byte 14,958 ms |
| `extract --schema examples/ai/person.schema.json --json` | validated `{"name":"Ada Lovelace","role":"engineer","years":12,"skills":["mathematics","computing"]}`; `attempts: 2` (one repair); usage 237 / 105 / 342 |
| `embed --dimensions 768 --json` | two 768-component vectors; usage 28 prompt / 28 total |
| `index` over a 3-file corpus | 3 documents, 3 chunks, 3 vectors, 768 dimensions; usage 206 prompt / 206 total |
| `search --mode semantic` | ranked `vantyx.md` first |
| `eval --mode semantic --top-k 2` | 3 queries; recall@2 = 1.0, precision@2 = 0.5, MRR = 1.0 |

An earlier full retry at 20:58Z failed only the extraction checks because the
model returned a transient HTTP 503 `This model is currently experiencing high
demand`; that run is preserved as
`docs/ai/live-evidence-2026-11-24-retry.json`. The next full run passed all 21
checks. The 14,958 ms first-byte latency is a single observation, not a
throughput benchmark.

## Observed failure paths

`tools/ai_failure_harness.py` ran the real CLI against loopback
OpenAI-compatible servers and passed 16 checks across 7 scenarios:

| Scenario | Exit | Stable error |
| --- | --- | --- |
| model list | 0 | `status: ok`, one model |
| chat | 0 | `VANTYX OFFLINE OK` |
| chat JSON | 0 | usage total 8 |
| streamed chat | 0 | deltas join to `VANTYX OFFLINE OK` |
| HTTP 401 | 1 | `AI_E_AUTH: provider authentication failed (HTTP 401)` |
| HTTP 429 | 1 | `AI_E_RATE_LIMIT: provider rate limit reached (HTTP 429)`; `note: this failure is retryable` |
| unresponsive endpoint | 1 | `AI_E_TIMEOUT: HTTP request timed out` (304 ms with `--timeout-ms 300`) |

The HTTPS/curl path was also corrected during this verification: curl exit code
28 (operation timeout) now maps to retryable `AI_E_TIMEOUT` instead of
`AI_E_CONNECT` (T-068). DNS and connect failures remain `AI_E_CONNECT`.

## Provider availability and limitations

The successful runs above are real observations, not mocks. A retry at 19:50Z
with the same endpoint returned:

```text
AI_E_PROVIDER: provider returned HTTP 400: User location is not supported for the API use.
```

The same restriction was observed at that time with the `MY_API_KEY` variable
from the shell configuration. Later, at 20:58Z and 20:59Z, requests with that
variable succeeded: the first run passed every check except extraction, which
hit a transient HTTP 503 high-demand response, and the second run passed all 21
checks. This is an external provider availability pattern, not a Vantyx failure.
Live cloud access is not continuously available from this environment, and the
harness does not attempt to bypass provider restrictions.

Other limits:

- No local Ollama or llama.cpp server/model is installed, so T-040's local-model
  half remains blocked.
- `vantyx ai chat` does not expose a live tool-calling loop; tool-calling is
  covered by offline library and agent tests.
- The CLI now runs automatic bounded retries (default 2, hard maximum 8,
  ADR-0058) for retryable provider failures. The observations above predate that
  change, so a single 429 was reported immediately at the time. The live harness
  keeps its own bounded loop for checks that need a fresh process per attempt.
- Observed first-byte latency was 1,976 ms in the 19:26Z run and 14,958 ms in
  the 20:59Z run; these are single observations, not a throughput benchmark.
- Gemini 2.5 chat models returned a deprecation/404 response for some names, and
  `gemini-3.8-flash` returned a transient 503. Model aliases and availability
  change over time.
- No API key is written to the repository. Evidence records only the
  environment-variable name.

## Conclusion

The cloud OpenAI-compatible chat, streaming, extraction, embedding, RAG, and
retrieval-evaluation paths have real provider evidence, including a 21/21
re-verification on 2026-11-24 at 20:59Z. The 401/429/timeout failure paths have a
reproducible harness. The task is not fully closed: local-model evidence is
still missing, and cloud access can change or be rejected by provider
geography. The evidence was committed in `e6ec597`; CI run 45488725812 passed
on `4982ab3` with 581 tests.
