# AI Ecosystem Research

Date: 2026-11-23
Status: research complete for the foundation decision; not a benchmark report
Method: GitHub REST metadata, primary READMEs, official documentation, and
direct repository inspection. No third-party benchmark was reproduced. Star
counts and activity are snapshots, not quality scores.

## Research questions

1. Which Rust-native runtimes can support local inference without breaking the
   zero-dependency compiler rule?
2. Which local/cloud protocols give the broadest model coverage with the least
   lock-in?
3. What is the smallest honest AI foundation Vantyx can ship now, given that
   modules, async, and a package manager do not exist yet?
4. Which capabilities must remain research-only until hardware or credentials
   are available?

## Candidate matrix (observed 2026-11-23)

| Project | Stars | Last push | License | Primary role | Relevant capabilities | Integration cost |
| --- | ---: | --- | --- | --- | --- | --- |
| [Candle](https://github.com/huggingface/candle) | 21,394 | 2026-11-23 | Apache-2.0 / MIT | Rust ML framework | CPU, CUDA, Metal, WASM; safetensors, GGUF, PyTorch/npz loading; llama.cpp quantization types | Large Rust dependency tree; optional feature |
| [Burn](https://github.com/tracel-ai/burn) | 16,334 | 2026-11-24 | Apache-2.0 / MIT | Tensor + deep learning | Training and inference; CUDA, ROCm, Metal, Vulkan, WebGPU, CPU, `no_std` Flex; autodiff; ONNX import; safetensors | Very large; best for training/numerical work |
| [mistral.rs](https://github.com/EricLBuehler/mistral.rs) | 7,857 | 2026-11-20 | MIT | Local LLM server/SDK | GGUF, quantization, paged attention, text/vision/video/audio, tool calling, OpenAI and Anthropic-compatible APIs | Large; best used as an external server |
| [ort](https://github.com/pykeio/ort) | 2,627 | 2026-11-24 | Apache-2.0 | ONNX Runtime bindings | ONNX inference/training; CUDA, TensorRT, DirectML, CoreML, WebGPU execution providers; CPU fallback | Native ONNX Runtime library and build complexity |
| [tract](https://github.com/sonos/tract) | 3,134 | 2026-11-23 | MIT/Apache-2.0 | Pure-Rust ONNX/NNEF inference | ONNX, NNEF, edge/embedded, WASM, streaming speech production use | Moderate; fewer LLM features |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) | 136,091 | 2026-11-24 | MIT | C/C++ LLM inference | GGUF, CPU and many GPU backends, OpenAI-compatible server, multimodal, tool calling, MCP stdio tools | External process/server; no Rust dependency |
| [Ollama](https://github.com/ollama/ollama) | 187,920 | 2026-11-24 | MIT | Local model manager/server | Model pull/list/show/delete, chat/generate/embeddings API, streaming, tools, structured outputs | External process; easiest local UX |
| [tokenizers](https://github.com/huggingface/tokenizers) | 11,289 | 2026-11-23 | Apache-2.0 | Tokenization | BPE, WordPiece, Unigram, special tokens, training, WASM; ~325 KB gzipped core | Medium; model-specific files required |
| [Qdrant](https://github.com/qdrant/qdrant) | 35,679 | 2026-11-23 | Apache-2.0 | Vector database | Filtering, hybrid search, payloads, production service | External service |
| [LanceDB](https://github.com/lancedb/lancedb) | 11,973 | 2026-11-24 | Apache-2.0 | Embedded vector DB | Multimodal retrieval, Arrow integration, local persistence | Large native dependency |
| [sqlite-vec](https://github.com/asg017/sqlite-vec) | 8,277 | 2026-05-18 | Apache-2.0 | SQLite vector extension | Pure C, runs anywhere SQLite runs, WASM; pre-v1 | Native extension; SQLite integration |
| [fastembed-rs](https://github.com/Anush008/fastembed-rs) | 1,055 | 2026-11-22 | Apache-2.0 | Local embeddings/reranking | ONNX via `ort`, HF tokenizers, batching | Native ONNX dependency |
| [tch-rs](https://github.com/LaurentMazare/tch-rs) | 5,521 | 2026-11-10 | Apache-2.0 | PyTorch/LibTorch bindings | Training/inference, CUDA | Heavy LibTorch ABI; avoid for the core |
| [llguidance](https://github.com/guidance-ai/llguidance) | 919 | 2026-11-18 | MIT | Constrained decoding | JSON Schema/grammar masks; integrated in llama.cpp, vLLM, SGLang | Server-side; Rust crate exists |
| [Outlines](https://github.com/dottxt-ai/outlines) | 16,021 | 2026-09-21 | Apache-2.0 | Structured generation | JSON Schema/Pydantic, choices, function calls; vLLM/Ollama/llama.cpp | Python-centric; use as prior art |
| [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust) | 2,777 | 2026-11-24 | Apache-2.0 | Observability | Traces, metrics, logs | Optional; large dependency set |
| [MCP](https://github.com/modelcontextprotocol/modelcontextprotocol) | 9,755 | 2026-11-23 | spec (NOASSERTION) | Tool/context protocol | JSON-RPC lifecycle, tools, resources, prompts; stdio and HTTP transports | Protocol work; security-sensitive |

All listed repositories were active and not archived at the snapshot. The
`pushed_at` field proves activity, not production quality.

## What the primary sources actually support

- **Candle** documents CPU and CUDA devices, WASM examples, safetensors/GGUF/
  PyTorch loading, quantized LLaMA/Qwen examples, and llama.cpp-compatible
  quantization types.
- **Burn** documents CUDA, ROCm, Metal, Vulkan, and WebGPU backends, CPU and
  `no_std` Flex backends, autodiff, ONNX import, and safetensors/PyTorch weight
  loading.
- **mistral.rs** documents GGUF loading, ISQ/UQFF quantization, paged attention,
  multimodal text/image/video/audio, tool calling, OpenAI-compatible `/v1`, and
  Anthropic-compatible `/v1/messages`.
- **ort** documents ONNX Runtime 1.31 and execution providers including CUDA,
  TensorRT, DirectML, CoreML, and WebGPU, with CPU fallback.
- **tract** documents ONNX and NNEF loading, optimization, embedded/ARM/WASM
  targets, and production speech workloads.
- **llama.cpp** documents GGUF, an OpenAI-compatible server with chat,
  responses, and embeddings routes, multimodal input, tool use, and MCP stdio
  servers.
- **Ollama** documents generate/chat/embeddings endpoints, streaming, tool
  calling, structured outputs, and model management (`list`, `show`, `delete`,
  `pull`).
- **tokenizers** documents BPE/WordPiece/Unigram pipelines, training, special
  tokens, and WASM bindings.
- **MCP** documents a JSON-RPC protocol with lifecycle, tools, resources,
  prompts, and security/trust guidance.

## Decision

### 1. Do not embed a large inference engine in the compiler

The repository rule is zero third-party Rust dependencies (ADR-0007). Candle,
Burn, ort, tokenizers, and fastembed all add dependency trees or native
libraries. Embedding one now would violate the rule and make the compiler harder
to audit and build.

**Decision:** the compiler and the first AI foundation stay dependency-free.
Local and cloud models are reached through explicit HTTP boundaries. An optional
embedded backend requires a new ADR with binary-size, license, build, and
platform evidence.

### 2. Use OpenAI-compatible HTTP as the first common protocol

llama.cpp, Ollama, mistral.rs, LM Studio, and many cloud providers expose an
OpenAI-compatible chat surface. Supporting that shape once gives broad coverage
without pretending all providers are identical.

**Decision:** implement a provider abstraction with an OpenAI-compatible adapter
and capability metadata. Do not claim feature parity; reject unsupported
parameters instead of silently ignoring them.

### 3. Local first, cloud opt-in

Local inference is the privacy-preserving default. Network access must be
explicit. No model is downloaded automatically.

**Decision:** ship a local adapter and a model-manager interface, but require an
explicit `--allow-network` flag and explicit model names. Never bundle weights.

### 4. Tokenization must be honest

A byte/word estimate is not a model tokenizer. It is acceptable for coarse
budget guards only if labelled as an estimate.

**Decision:** ship an estimate now with a clear name and error bars; integrate a
real tokenizer only when the model's tokenizer files are available and a
dependency ADR is accepted.

### 5. Structured output needs validation, not optimism

JSON-looking text is not typed data. Server-side constrained decoding is
preferred when the provider supports it. Otherwise validate against a schema
subset and bound repair retries.

**Decision:** implement a strict JSON parser and a small JSON Schema subset
(type, required, properties, items, enum, additionalProperties) with tests.
Never deserialize untrusted output directly into host types.

### 6. Retrieval starts small and measurable

Qdrant, LanceDB, and sqlite-vec are credible, but all add a service or native
dependency. A local in-memory index with cosine similarity and BM25 is enough to
validate the API and the evaluation harness.

**Decision:** implement a deterministic in-memory vector index and BM25 first;
add an external store only when a benchmark shows the need.

### 7. Agents are bounded programs, not autonomous loops

The failure mode is unbounded tool use. Every loop needs step, token, time, and
tool-call limits plus explicit capabilities.

**Decision:** implement a bounded agent loop with a capability policy and
deterministic cancellation. Multi-agent orchestration remains research until a
benchmark shows value.

### 8. GPU, tensors, and autodiff are research-only for now

No GPU hardware is available in this environment, and no backend has been built
or tested. Claiming support would violate the project's honesty rules.

**Decision:** document the architecture and keep GPU/tensor/autodiff work in
research. Revisit when hardware and a reproducible benchmark exist.

## Rejected options and why

| Option | Why not now |
| --- | --- |
| Embed Candle | Large dependency tree; conflicts with ADR-0007; model files are large |
| Embed Burn | Best training story, but far beyond the current runtime and build budget |
| Link ONNX Runtime via `ort` | Native library, platform packaging, and ABI work; not needed for a first provider layer |
| Use LibTorch via `tch-rs` | Heavy ABI, poor fit for a small compiler |
| Auto-download models | Violates explicit-resource-use and security rules |
| Universal tokenizer claim | Model-specific tokenizers differ; a false count breaks budgets |
| Multi-agent by default | No evidence of value; adds cost and failure modes |
| Custom GPU runtime | No hardware to verify; high risk of fake support |

## Sources

All accessed 2026-11-23. GitHub metadata from the REST API; capability claims
from the repositories' primary READMEs or official docs.

- https://github.com/huggingface/candle
- https://github.com/tracel-ai/burn
- https://github.com/EricLBuehler/mistral.rs
- https://github.com/pykeio/ort and https://ort.pyke.io/perf/execution-providers
- https://github.com/sonos/tract
- https://github.com/ggml-org/llama.cpp and `tools/server/README.md`
- https://github.com/ollama/ollama and `docs/api.md`
- https://github.com/huggingface/tokenizers
- https://github.com/qdrant/qdrant
- https://github.com/lancedb/lancedb
- https://github.com/asg017/sqlite-vec
- https://github.com/Anush008/fastembed-rs
- https://github.com/LaurentMazare/tch-rs
- https://github.com/guidance-ai/llguidance
- https://github.com/dottxt-ai/outlines
- https://github.com/open-telemetry/opentelemetry-rust
- https://github.com/modelcontextprotocol/modelcontextprotocol and
  https://modelcontextprotocol.io/specification/2025-06-18
- https://json-schema.org/draft/2020-12/json-schema-core.html
