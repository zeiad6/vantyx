# Vantyx Roadmap

Updated: 2026-11-24
Status: core language at a verified v0.1 subset; AI host foundation verified
offline; live model evidence and production hardening remain open.

This is the canonical high-level roadmap. The detailed phase checklist remains
in [`docs/roadmap.md`](docs/roadmap.md), and AI phase exit gates remain in
[`docs/ai/AI_ROADMAP.md`](docs/ai/AI_ROADMAP.md). Status claims here must match
`TASKS.md`, `PROJECT_STATUS.md`, and observed verification commands.

## Product direction

Vantyx is a gradual-typing language with a tree-walking interpreter as the
semantic oracle, a native GNU C11 backend for a statically typed subset, and a
dependency-free AI host. Two tracks advance together:

- **Track A — Core Vantyx:** compiler correctness, runtime safety, native
  memory, pattern matching, tooling, async, and ecosystem foundations.
- **Track B — AI Vantyx:** provider boundaries, embeddings, retrieval, tools,
  agents, evaluation, and later numerical/GPU work.

Track B must not remain blocked by the whole language roadmap, and Track A must
not be abandoned for AI demos. Unsupported native constructs fail with `E0401`;
no feature is claimed without implementation, tests, and observed evidence.

## Track A — Core language and runtime

### Complete and verified

- Indentation-aware lexer, recursive-descent/Pratt parser, spans, diagnostics,
  depth guards, and non-panicking recovery.
- Gradual type checker, nominal structs/enums, exhaustive guarded `match`,
  scalar literal patterns, nested payload patterns with matrix exhaustiveness,
  top-level or-patterns, and runtime `Any` boundaries.
- Tree-walking interpreter with checked arithmetic, bounds checks, cycle-safe
  structural equality, and stable runtime errors.
- Typed IR and native C11 backend for scalars, strings, lists, maps with a
  scalar-key hash index, structs, enums, nested payload patterns, top-level
  or-patterns, guards, and imported declarations.
- File-based modules (`import` and `from ... import`) confined to the entry
  directory, with cycle/path/limit diagnostics (`E0501`–`E0512`).
- Differential interpreter/native testing (73 cases) and 44 end-to-end tests.
- REPL line editing, single-line history, top-level completion, and echo.
- Benchmarks against Python, Rust, and C with raw results recorded.

### Next core priorities

1. **Tuple patterns** once a tuple type exists, plus usefulness-based
   unreachable warnings for nested, alternative, and range patterns.
2. **Native memory reclamation** design and implementation; document cycles and
   allocation policy honestly.
3. **Async and concurrency** (T-017), the remaining prerequisite for
   language-level AI syntax.
4. **Structural-key native map hashing** and removal of the fixed scalar hash
   seed once a benchmark justifies the design.
5. **Tooling:** formatter, linter, test runner, multiline REPL history, field
   completion, and package manager (T-019/T-020/T-034).
6. **Generics, traits, closures, iterators, `Result`/`?`, and FFI** in
   evidence-driven order.
7. **Debug information, optimization, cross-platform CI, and an independent
   security review.**

## Track B — AI host and ecosystem

Detailed architecture: [`docs/ai/AI_ARCHITECTURE.md`](docs/ai/AI_ARCHITECTURE.md).
Security model: [`docs/ai/AI_SECURITY.md`](docs/ai/AI_SECURITY.md).
Research evidence: [`docs/ai/AI_RESEARCH.md`](docs/ai/AI_RESEARCH.md).

### Complete and verified offline

- Dependency-free AI host: strict JSON, JSON Schema subset, typed providers,
  capabilities, errors, token estimates, budgets, metrics, and HTTP boundary.
- Mock/echo providers and an OpenAI-compatible non-streaming chat/models/
  embeddings adapter with explicit `--allow-network` and environment-only keys.
- Validated structured extraction with bounded repair (`vantyx ai extract`).
- Embedding provider interface, deterministic lexical hash baseline, bounded
  FIFO cache, and batch/dimension limits (ADR-0017).
- Local RAG: bounded text/Markdown/code ingestion, stable line-cited chunks,
  strict versioned JSON index with atomic 0600 persistence, BM25/cosine/hybrid
  reciprocal-rank search, and recall/precision/MRR evaluation (ADR-0018).
- Typed tools with deny-by-default capabilities, bounded agents with
  cancellation, redacted metrics, and SSE parsing.
- `vantyx ai providers`, `doctor`, `models`, `chat`, `extract`, `embed`,
  `index`, `search`, `eval`, and `tokens`.

### Next AI priorities

1. **Live evidence:** one local and one cloud chat, plus one trained embedding
   model, recorded with setup, version, hardware, and output. Currently
   `BLOCKED` by unavailable servers/credentials.
2. **Streaming:** adapter and CLI output/cancellation on top of the existing
   bounded SSE parser.
3. **Retrieval quality:** arbitrary metadata filters (source-prefix filtering ships), incremental re-indexing,
   deduplication/deletion, reranking, a licensed multi-document corpus, and
   latency/memory measurements.
4. **Grounded generation:** only after retrieval evaluation; cite retrieved
   chunks and explicitly mark ungrounded answers. Do not claim grounding yet.
5. **Model manager:** `info`/`pull`/`remove` with checksums, size, license,
   provenance, safe extraction, and opt-in downloads.
6. **Security depth:** keyring credentials, prompt-injection corpus, retrieval
   trust labels, OS/process sandboxing, and concurrency/backpressure limits.
7. **Structured generation:** server-side constrained decoding where advertised;
   local validation remains mandatory.
8. **Multimodal:** typed image/audio/video parts with size limits and per-model
   capability checks, one verified backend at a time.
9. **Tensors, SIMD, autodiff, and GPU:** research/integration only after an ADR,
   verified hardware, and reproducible benchmarks.

## Sequencing and gates

1. Keep the interpreter as the oracle; every native change needs a differential
   test.
2. Keep the compiler and AI host free of third-party Rust dependencies unless an
   ADR records the build, license, platform, and security impact.
3. Every behavior change updates `TASKS.md`, `PROJECT_STATUS.md`, this roadmap
   when scope changes, and the specification or ADR when semantics change.
4. A task is `VERIFIED` only with an observed command and date; performance
   claims require raw results under `benchmarks/results/`.
5. Do not claim GPU, live AI, grounding, safety, or performance without direct
   evidence.

## Explicit non-goals for the first release

- Training foundation models or shipping bundled weights.
- Automatic model downloads or execution of model files as native code.
- A universal tokenizer or universal model format.
- Unbounded autonomous agents or unrestricted shell/filesystem tools.
- A claim of sandboxing without an implemented sandbox.
- Replacing mature inference engines or vector databases.
