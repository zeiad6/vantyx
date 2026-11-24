# ADR-0015: AI host foundation and dependency boundary

- Status: Accepted
- Date: 2026-11-23
- Related: ADR-0007 (zero third-party Rust dependencies), ADR-0014 (rename)

## Context

The AI directive asks for provider-independent chat, local inference, structured
output, embeddings, retrieval, tools, agents, observability, and eventually
tensors and GPUs. The repository rule forbids third-party Rust dependencies
without an ADR, and the compiler links only libc/libm.

Embedding Candle, Burn, `ort`, `tokenizers`, or a vector database now would:

- add a large dependency tree and native build requirements,
- make the compiler harder to audit and cross-compile,
- couple a pre-1.0 language to fast-moving model APIs,
- encourage claims that cannot be tested in this environment.

## Decision

1. Add a **zero-dependency AI host module** (`src/ai/`) and CLI surface first.
2. Use a narrow transport boundary:
   - loopback HTTP through `std::net`,
   - HTTPS through the system `curl` binary with explicit arguments,
   - no shell interpolation.
3. Keep model inference external (Ollama, llama.cpp, LM Studio, or an
   OpenAI-compatible cloud endpoint).
4. Do not download models automatically. A model manager, when added, must show
   size, source, license, and checksum and require explicit confirmation.
5. Do not claim exact tokenization. Ship a labelled estimate until a real
   tokenizer is integrated.
6. Any embedded inference engine, tokenizer, vector database, or telemetry
   library requires a new ADR that records binary size, licenses, platform
   support, build complexity, and benchmark evidence.

## Consequences

- The first AI features work offline with a mock provider and loopback tests.
- Cloud and local integrations are real HTTP calls, not simulated.
- Streaming, structured output, embeddings, BM25, and bounded agents can be
  implemented and tested without a model download.
- GPU, tensor, and embedded inference work remains research until hardware and
  evidence exist.
- Users must install and run a local server, or provide a cloud key, to use a
  real model. The documentation says so.

## Rejected alternatives

| Alternative | Reason |
| --- | --- |
| Add Candle now | Dependency and model-size cost; no embedded requirement yet |
| Add `reqwest`/`ureq` | TLS and async dependency tree; `curl` is already a system tool |
| Add a vector database | In-memory cosine/BM25 is enough to validate the API |
| Use a universal tokenizer | Incorrect counts would break budgets and context limits |
| Auto-download a small model | Violates explicit resource use and offline-first rules |

## Reconsideration criteria

Revisit when:

- a real embedded-inference requirement appears,
- a benchmark shows the HTTP boundary is the bottleneck,
- the language has modules and async and can expose a native AI API,
- a dependency can be justified with size, license, and platform evidence.
