# Historical Requirements Audit

Date: 2026-11-23
Repository: RustZ (public name `vantyx`; renamed from `auren`)
Auditor: autonomous engineering session
Scope: every requirement recoverable from `TASKS.md`, `PROJECT_STATUS.md`,
`docs/roadmap.md`, `AGENTS.md`, the ADR set, the public launch material, Git
history, the source tree, tests, benchmarks, and the two Vantyx AI directives.

## Method

1. Read the task inventory and every ADR, roadmap, status, and launch document.
2. Inspected `src/`, `tests/`, `examples/`, `benchmarks/`, `tools/`, CI, and
   `.opencode/agents/`.
3. Re-ran the verification gate instead of trusting prior reports.
4. Assigned one status per requirement: `COMPLETE`, `PARTIAL`, `MISSING`,
   `BROKEN`, `BLOCKED`, or `SUPERSEDED`.
5. Recorded the actual evidence, the remaining work, and a priority.

Limitations: the original conversation prompts are not stored verbatim in the
repository. Requirements were reconstructed from the written task records and
the two AI directives supplied in this session. A claim is not marked complete
because a document says so; the table cites code or an observed command.

## Observed baseline at audit start (2026-11-23)

| Check | Observed result |
| --- | --- |
| `cargo test` | 241 passed: 126 library unit, 7 CLI unit, 41 e2e, 67 native |
| `cargo build` / `--release` | succeeded earlier in the session; re-checked before release claims |
| `cargo clippy --all-targets -- -D warnings` | clean at the last recorded gate |
| Public CI | runs 45308499010, 45308796021, 45309035756 passed (pre-rename); run 45317367818 passed after rename and AI foundation; run 45318733338 passed with structured extraction (357 tests); run 45321718008 passed with the map hash index (358 tests) |
| Source AI references | none (`grep` over `src/`, `tests/`, `Cargo.toml`) |
| Modules / imports | no AST or parser support |
| Async / await | no language support; only a host thread for the interpreter stack |
| Tensor / GPU / embedding code | none |

## Post-implementation verification (2026-11-23)

| Check | Observed result |
| --- | --- |
| `cargo test` | **358 passed**: 218 library unit, 11 CLI unit, 20 AI integration, 41 e2e, 68 native (2026-11-24, after T-044); the earlier 357-test and 340-test runs also passed |
| Map benchmark | `map_ops` native 3.597 ms, interpreter 121.728 ms, C 2.884 ms, Rust 4.495 ms (2026-11-24; `benchmarks/results/2026-11-24.*`) |
| `cargo build` / `--release` | both succeeded after the AI foundation and rename |
| `cargo clippy --all-targets -- -D warnings` | clean, zero warnings |
| Examples | all five interpreter/native outputs identical |
| Rename | code, CLI, extension, env vars, tests, docs, CI, GitHub repository, and clean-clone CI complete |
| AI host | `src/ai/` foundation and `vantyx ai` CLI implemented; offline and loopback tests pass |
| AI live evidence | none; no local server or cloud credentials available |
| Language-level AI | still blocked on modules (T-013) and async (T-017) |
| Tensor / GPU / embedding model code | none; only vector math and an in-memory index |

## Status summary

| Status | Core tasks | Directive requirements |
| --- | ---: | ---: |
| VERIFIED / COMPLETE | 28 | 9 |
| IN_PROGRESS | 1 | 0 |
| READY | 1 | 0 |
| BACKLOG / MISSING | 11 | 13 |
| BLOCKED | 1 | 4 |
| PARTIAL | 0 | 19 |
| BROKEN | 0 | 0 |
| SUPERSEDED | 0 | 0 |
| **Total tracked** | **42** | **45** |

Core tasks use the `TASKS.md` vocabulary; directive rows use
`COMPLETE`/`PARTIAL`/`MISSING`/`BLOCKED`. The two columns are not directly
comparable because `BACKLOG` core tasks are counted as missing work in the
directive column.

Core tasks marked `BACKLOG` in `TASKS.md` are counted as `MISSING` here because
no implementation exists. The 25 "complete" core tasks include T-010 and T-024
through T-035; their documented subset limits remain real gaps, listed below.

## Core task matrix (T-001 … T-035)

| ID | Requirement | Implementation / evidence | Status | Missing work | Priority | Action |
| --- | --- | --- | --- | --- | --- | --- |
| T-001 | Bootstrap Cargo project | `Cargo.toml`, `src/`, Git; `cargo build` | COMPLETE | — | P0 | Keep |
| T-002 | Indentation-aware lexer | `src/lexer.rs`, 9 unit tests, fuzz smoke | COMPLETE | — | P0 | Keep |
| T-003 | Parser and AST | `src/parser.rs`, `src/ast.rs`, 18 unit tests | COMPLETE | — | P0 | Keep |
| T-004 | Diagnostic renderer | `src/diagnostic.rs`, 3 unit tests | COMPLETE | — | P0 | Keep |
| T-005 | Gradual checker | `src/checker.rs`, 47 unit tests | COMPLETE | Flow narrowing/generics are T-018 | P0 | Keep |
| T-006 | Tree-walking runtime | `src/interp.rs`, 23 unit tests + e2e | COMPLETE | — | P0 | Keep |
| T-007 | CLI | `src/main.rs`; `run/check/build/repl/version/help` | COMPLETE | AI subcommands do not exist | P0 | Extend |
| T-008 | E2E suite | `tests/e2e.rs`, 41 tests | COMPLETE | — | P0 | Keep |
| T-009 | REPL | `src/repl.rs`; persistence tests | COMPLETE | Multi-line recall, field completion (T-034) | P1 | Extend |
| T-010 | IR + native backend | `src/ir.rs`, `src/codegen_c.rs`, 68 differential tests | COMPLETE (subset) | Reclamation, debug info, optimization, more types | P0 | Extend |
| T-011 | Bytecode VM | None | MISSING | Entire VM and benchmark | P2 | Defer |
| T-012 | Ownership / escape analysis | None | MISSING | Design, analysis, tests | P2 | Defer |
| T-013 | Modules and imports | None | MISSING | Parser, checker, interpreter, native, tests | P1 | **Next core** |
| T-014 | Enums and matching | AST/checker/interp/IR/codegen; 12 e2e + native tests | COMPLETE (subset) | Nested/or/range/float patterns, generic enums | P1 | Extend |
| T-015 | Closures and iterators | None | MISSING | Syntax, capture semantics, native lowering | P2 | Defer |
| T-016 | `Result` / `?` | None | MISSING | Types, propagation, native lowering | P2 | Defer |
| T-017 | Async and concurrency | None | MISSING | Design, runtime, cancellation, tests | P3 | Defer |
| T-018 | Traits and generics | None | MISSING | Type-system design and implementation | P3 | Defer |
| T-019 | Formatter and linter | None | MISSING | Both tools and tests | P3 | Defer |
| T-020 | Package manager | None | MISSING | Manifest, resolver, registry, lockfile | P3 | Defer |
| T-021 | C ABI FFI | None | MISSING | ABI, unsafe boundary, native linking | P3 | Defer |
| T-022 | Benchmarks vs Python/Rust/C | `benchmarks/`, 6 workloads, raw JSON/MD | COMPLETE | Enum and AI benchmarks | P2 | Extend |
| T-023 | Security audit | `docs/security-audit-2026-11-22.md`, 12 findings | COMPLETE | Re-audit after AI subsystem | P1 | Extend |
| T-024 | Native `List[T]` | IR + C runtime; differential tests | COMPLETE (subset) | Reclamation | P1 | Keep |
| T-025 | Native string indexing/iteration | UTF-8 helpers; differential tests | COMPLETE | — | P1 | Keep |
| T-026 | Documentation reconciliation | Docs updated; this audit adds a new pass | COMPLETE | Must be repeated after rename/AI | P1 | Repeat |
| T-027 | Native `Map[K,V]` | Insertion-ordered arrays + scalar-key hash index; differential tests | COMPLETE (subset) | Structural keys remain O(n); reclamation | P1 | Extend |
| T-028 | Prior-art review | `docs/research/prior-art-lessons-2026.md` | COMPLETE | AI prior art is new work | P2 | Extend |
| T-029 | REPL editing/history/echo | `src/repl.rs`, 7 CLI unit + e2e tests | COMPLETE | — | P3 | Keep |
| T-030 | Native enums/match | Tagged C structs; differential tests | COMPLETE (subset) | Allocation benchmark | P2 | Extend |
| T-031 | Bracketed-match diagnostic | Parser test + e2e test | COMPLETE | — | P3 | Keep |
| T-032 | Scalar literal patterns | Parser/checker/interp/IR/C; tests | COMPLETE | Float/range/or patterns | P2 | Extend |
| T-033 | Match guards | Parser/checker/interp/IR/C; tests | COMPLETE | Guards cannot bind or mutate | P2 | Keep |
| T-034 | REPL multi-line history/field completion | None | MISSING | Entire task | P4 | Defer |
| T-035 | Public repository and launch kit | Public repo, CI, release, issue, `llms.txt` | COMPLETE | Rename to Vantyx; AI discovery update | P1 | Rename |
| T-044 | Native scalar-key map hash index | `src/codegen_c.rs`; `large_scalar_maps_match`; 2026-11-24 benchmark | COMPLETE | Structural-key hashing; adversarial seed | P1 | Keep |

### Core partials that must not be described as finished

| Area | What exists | What is still missing |
| --- | --- | --- |
| Native backend | Typed subset with lists, maps (scalar-key hash index), structs, enums, strings, guards | Reclamation, cycle handling, structural-key hashing, debug info, optimization, `Any` |
| Pattern matching | Enum, wildcard, scalar literal, guards | Nested, or-patterns, ranges, floats, interpolated strings, generic enums |
| REPL | Line editing, single-line history, top-level completion | Multi-line recall, field completion, local completion |
| Memory | Interpreter `Rc` with cycles; native intentional leaks | Cycle collector or ownership/escape analysis; sandbox |
| Tooling | `run/check/build/repl` | Formatter, linter, test runner, package manager, LSP, debugger |
| Platforms | Linux x86_64 CI only | macOS/Windows CI and native-backend checks |

## Roadmap inventory beyond `TASKS.md`

| Roadmap item | Status | Evidence / gap | Priority | Action |
| --- | --- | --- | --- | --- |
| Debug info and optimization pipeline | MISSING | No DWARF, constant folding, DCE, inlining, or unboxing | P2 | Plan after reclamation |
| Cranelift/LLVM evaluation | MISSING | ADR-0008 defers it; no benchmark | P3 | Defer |
| Escape analysis / ownership | MISSING | T-012 | P2 | Defer |
| Cycle handling | MISSING | Documented leak in interpreter and native | P2 | Design |
| Documented safety case | PARTIAL | `docs/security.md`, `docs/memory_model.md`; no formal case | P2 | Extend |
| Profiling and optimization | MISSING | None | P2 | Measure first |
| SIMD/vectorization | MISSING | None | P3 | Research |
| Async/concurrency design | MISSING | T-017 | P3 | Defer |
| Standard library | MISSING | Builtins only | P1 | Plan |
| Formatter/linter/test runner | MISSING | T-019 | P3 | Defer |
| Package manager/registry | MISSING | T-020 | P3 | Defer |
| LSP/editor support | MISSING | None | P3 | Defer |
| Tensors/numerical | MISSING | Roadmap Phase 11 only | P2 | AI track |
| GPU backend | MISSING | No hardware verification | P3 | Research only |
| C ABI FFI / Python/Rust interop | MISSING | T-021 | P3 | Defer |
| Independent external audit | MISSING | None | P3 | Before 1.0 |
| Stability policy / release process | PARTIAL | SemVer implied; v0.1.0 pre-release | P2 | Document |
| Cross-platform CI | MISSING | Ubuntu-only workflow | P2 | Add matrix |
| Regression/stress suites | PARTIAL | 241 tests; fuzz smoke is manual | P2 | CI fuzz job |
| Native map hash index | COMPLETE for scalar keys (T-044, ADR-0016) | ADR-0010 revisit criterion | P1 | Keep; consider structural keys |
| Enum allocation benchmark | MISSING | T-030 scope limit | P3 | Benchmark |
| Nested/or-patterns | MISSING | Rejected by checker (`E0112`) | P2 | Implement |
| `ROADMAP.md` at repository root | MISSING | Actual file is `docs/roadmap.md` | P3 | Add pointer or move |

## Vantyx AI directive matrix

The directive is broad. Requirements are grouped so that each row is testable.
"Language-level" means usable from `.auren` source; "host-level" means a Rust
library or CLI capability. The distinction matters because modules and async do
not exist yet.

| ID | Requirement | Current state | Status | Missing work | Priority | Action |
| --- | --- | --- | --- | --- | --- | --- |
| AI-001 | Historical requirement recovery | This document | COMPLETE | Keep synchronized | P0 | Maintain |
| AI-002 | Rename audit and execution | `RESEARCH/NAMING_AUDIT.md`; code, CLI, extension, docs, tests, CI, GitHub repository, and clean-clone CI verified | COMPLETE | Keep old-name redirects documented | P0 | Maintain |
| AI-003 | AI architecture document | `docs/ai/AI_ARCHITECTURE.md` | COMPLETE | Keep synchronized | P0 | Maintain |
| AI-004 | Current AI ecosystem research | `docs/ai/AI_RESEARCH.md` with verified repository metadata | COMPLETE | Refresh before dependency decisions | P0 | Maintain |
| AI-005 | AI security threat model | `docs/ai/AI_SECURITY.md` | COMPLETE | Revisit when tools gain write/network capability | P0 | Maintain |
| AI-006 | Provider-independent interface | `src/ai/provider.rs`, mock/echo/OpenAI adapters, stable errors and capabilities | COMPLETE | Add new adapters behind the same trait | P0 | Maintain |
| AI-007 | Working cloud provider | OpenAI-compatible adapter, opt-in remote HTTPS, loopback and stub tests | PARTIAL | Record one live provider request with user credentials | P1 | Verify live |
| AI-008 | Working local inference path | Compatible-endpoint path supports Ollama/llama.cpp/LM Studio; no installed server evidence | PARTIAL | Run against a real local server; add native Ollama discovery | P1 | Verify live |
| AI-009 | Model manager CLI | `vantyx ai models` and `doctor` implemented; no info/pull/remove or integrity checks | PARTIAL | Add info/pull/remove with checksums, size, license, provenance | P2 | Implement |
| AI-010 | Tokenization | Heuristic estimator explicitly labelled "not a model tokenizer" | PARTIAL | Model-specific tokenizer boundary when an engine is embedded | P2 | Research |
| AI-011 | Structured output / JSON Schema | Strict JSON parser, depth/size limits, JSON Schema subset validator, `ai::extract`, `vantyx ai extract`, bounded repair with usage accounting, and offline/loopback tests | COMPLETE | Server-side constrained decoding remains research | P1 | Maintain |
| AI-012 | Tool calling with capabilities | Typed tool registry, capability sets, path roots, approval enum, timeouts, redacted logging | COMPLETE | Only read-only demo tools ship; add dangerous tools with approvals | P1 | Maintain |
| AI-013 | Agent loop with hard limits | Bounded loop with step/tool/token/time limits and cancellation flag | PARTIAL | Persistent state and resume | P1 | Extend |
| AI-014 | Multi-agent orchestration research | No evidence gathered; deliberately unbuilt | MISSING | Research only if a measurable use case appears | P3 | Defer |
| AI-015 | RAG ingestion/chunking | None | MISSING | Text/Markdown/code ingestion, chunking, metadata | P2 | Implement |
| AI-016 | Embeddings and vector store | Vector math, normalization, cosine/dot, and in-memory index | PARTIAL | Embedding provider interface, batching, persistence | P1 | Extend |
| AI-017 | Hybrid search / BM25 | BM25 with Unicode tokenization, add/replace/remove, ranked hits | PARTIAL | Hybrid fusion and evaluation | P2 | Extend |
| AI-018 | Retrieval evaluation harness | None | MISSING | Recall/precision/MRR fixtures | P2 | Implement |
| AI-019 | Tensor primitives | None | MISSING | Shape/dtype/strides/matmul or documented non-goal | P3 | Research |
| AI-020 | Automatic differentiation | None | MISSING | Evaluate Burn instead of custom autodiff | P4 | Research |
| AI-021 | Numerical reliability tests | None | MISSING | NaN/inf/overflow/precision/determinism | P3 | Implement with tensors |
| AI-022 | GPU backends | None | BLOCKED | No verified GPU hardware; CUDA/Metal/Vulkan/WebGPU | P4 | Research only |
| AI-023 | SIMD/CPU dispatch | None | MISSING | Runtime feature detection and portable fallback | P3 | Research |
| AI-024 | AI performance benchmarks | None | MISSING | Load, prefill, decode, tokens/s, memory, retrieval | P2 | Benchmark after foundation |
| AI-025 | Observability / tracing | Opt-in JSONL metrics with provider, model, latency, usage, and error code; no prompts or secrets | PARTIAL | Distributed tracing and per-request spans | P2 | Extend |
| AI-026 | Budgets and limits | Step, tool, token, and wall-clock budgets with tests | PARTIAL | Cost and concurrency limits | P1 | Extend |
| AI-027 | Multimodal text/image/audio/video | Typed text and image content parts; no audio/video or size validation | PARTIAL | Audio/video parts, limits, capability checks, real example | P3 | Research/implement incrementally |
| AI-028 | MCP client/server integration | None | MISSING | Protocol, transports, permissions, lifecycle | P3 | Research |
| AI-029 | ONNX/GGUF/Safetensors/Python interop | None | MISSING | Format support or documented non-goal | P3 | Research |
| AI-030 | AI test architecture | Mock/echo providers, stub transport, loopback HTTP, SSE, timeout, cancellation, permission tests | COMPLETE | Keep offline by default | P0 | Maintain |
| AI-031 | Working AI prototypes | Offline CLI chat, JSON output, metrics, bounded tool-agent loop, loopback OpenAI-compatible chat | PARTIAL | One real local-model example and one real cloud example | P2 | Build after live verification |
| AI-032 | Language-level `from ai import Client` | None | BLOCKED | Depends on modules (T-013) and async (T-017) | P1 | Unblock via host CLI first |
| AI-033 | `await` / async runtime | None | BLOCKED | T-017 | P3 | Defer |
| AI-034 | Modules/imports | None | BLOCKED | T-013 | P1 | Core track |
| AI-035 | Standard library / package manager | None | MISSING | T-019/T-020; needed for AI ecosystem | P2 | Core track |
| AI-036 | AI execution sandbox | None | MISSING | Process/WASM/OS sandbox; documented limits | P2 | Design |
| AI-037 | Secrets and credentials | Generic policy only | PARTIAL | No AI credential store; env/keyring policy | P0 | Implement |
| AI-038 | Prompt-injection defense | Model output treated as untrusted; tool arguments schema-validated; capabilities deny by default; denied-tool tests | PARTIAL | Dedicated prompt-injection fixture corpus and retrieval trust labels | P0 | Extend |
| AI-039 | Model download integrity | None | MISSING | Checksums, size, license, provenance, resume | P2 | Implement |
| AI-040 | AI discovery material | `README.md`, `llms.txt`, launch kit, status, and AI docs updated with verified offline AI claims | COMPLETE | Keep synchronized after live verification | P1 | Maintain |
| AI-041 | Cross-platform CI | Ubuntu only | PARTIAL | macOS/Windows matrix | P2 | Add |
| AI-042 | Honest performance claims | Benchmark rules exist | PARTIAL | AI benchmarks do not exist | P1 | Enforce |
| AI-043 | Versioning/release process | v0.1.0 pre-release | PARTIAL | Stability policy, changelog, tags | P2 | Document |
| AI-044 | Dependency policy for AI | ADR-0015: zero-dependency host, HTTP/subprocess boundary first | COMPLETE | New ADR required before embedding any engine | P0 | Maintain |
| AI-045 | `ROADMAP.md` synchronization | `docs/roadmap.md` only | PARTIAL | Root roadmap and AI phases | P3 | Add |

### Directive conflicts resolved in this audit

1. **"Vantyx" vs the published "Auren" name.** The directive names Vantyx
   throughout and asks for a rename audit. The repository is public as `auren`.
   Resolution: treat Vantyx as the requested name, audit it, and execute the
   rename only with a complete reference map. See `RESEARCH/NAMING_AUDIT.md`.
2. **Zero third-party Rust dependencies vs Candle/Burn/ort/tokenizers.**
   Resolution: keep the compiler and host AI foundation dependency-free; use
   explicit HTTP/subprocess boundaries for local servers and cloud APIs. Any
   embedded inference engine requires a new ADR that documents binary size,
   build complexity, licenses, and platform support. Do not add one silently.
3. **AI as first-class language syntax vs missing modules/async.** Resolution:
   build the host-level AI toolkit now (`vantyx ai` + Rust library), keep the
   language surface unchanged until T-013 and T-017 land, and document the gap.
4. **"15 prototypes" vs "do not create 15 broken demos."** Resolution: ship a
   small number of real, tested prototypes first.
5. **GPU claims vs no verified GPU.** Resolution: mark GPU `BLOCKED` until a
   real device and backend execute tests; no simulated "support".

## Naming audit summary

Full detail: `RESEARCH/NAMING_AUDIT.md`.

- **Vantyx** is six letters, two syllables, pronounceable, not a person's name,
  and has no programming-language collision in the checked sources. It is a
  brand name, not a descriptive one; the content link must come from the
  tagline and documentation.
- Unrelated uses exist: `Vantyx Group`, `VantyxBot`, `VantyxMedia`, and 85 GitHub
  repositories match the string. `crates.io`, npm, and PyPI returned no package
  named `vantyx` at audit time (crates.io 404).
- The name is **not trademark-cleared**. A proper search is required before 1.0.
- Decision: proceed with Vantyx because the directive names it explicitly, and
  record the tradeoff. Revisit before 1.0 if a descriptive name is preferred.

## Prioritized recovery plan

### P0 — Truth and identity ✅ (2026-11-23)

1. [x] Publish this audit and the naming audit.
2. [x] Rename code, CLI, extension, environment variables, tests, examples,
   benchmarks, docs, and CI to Vantyx. The GitHub repository rename is the only
   remaining step.
3. [x] Create the AI architecture, research, security, and roadmap documents.
4. [x] Add AI tasks to `TASKS.md` and reconcile `PROJECT_STATUS.md` and
   `docs/roadmap.md`.

### P1 — AI foundation that can be verified offline ✅ (2026-11-23)

1. [x] Zero-dependency JSON value/parser/writer with tests.
2. [x] Provider abstraction: request, response, usage, errors, capabilities,
   timeout, cancellation, mock provider.
3. [x] OpenAI-compatible adapter behind an explicit network opt-in; SSE parser.
4. [x] Token estimation and deterministic budgets.
5. [x] Embeddings math, in-memory vector index, cosine/dot, BM25.
6. [x] Typed tool registry with capabilities and allowlists; bounded agent loop.
7. [x] CLI: `vantyx ai providers`, `vantyx ai chat --provider mock`,
   `vantyx ai doctor`, and `vantyx ai models`/`chat` against a compatible
   endpoint when configured.
8. [x] Tests for every item, including offline loopback-server tests.

Evidence: 2026-11-23 `cargo test` — 357 passed (218 library unit + 11 CLI unit +
20 AI integration + 41 end-to-end + 67 native); `cargo clippy --all-targets --
-D warnings` clean; `cargo build --release` succeeded; all five examples matched
interpreter/native output. Live local-model and cloud evidence is still missing
because no server or credentials were available.

Re-verification after T-044 on 2026-11-24: `cargo test` — 358 passed (218
library unit + 11 CLI unit + 20 AI integration + 41 end-to-end + 68 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched. `python3 benchmarks/run.py --reps 5`
verified identical output and recorded `map_ops` native 3.597 ms versus
interpreter 121.728 ms, C 2.884 ms, and Rust 4.495 ms. GitHub Actions run
45321718008 passed build, all 358 tests, clippy, release build, and differential
examples on commit `f55c91c`.

### P1 — Core language recovery

1. Modules and imports (T-013) — prerequisite for language-level AI.
2. Nested/or-patterns.
3. Native memory reclamation design.
4. Native map hash index after a benchmark.

### P2 — Ecosystem

1. RAG ingestion and evaluation.
2. Model manager with integrity checks.
4. Observability and budgets.
5. Cross-platform CI.
6. Standard library and formatter.

### P3+ — Research-only until evidence exists

GPU, SIMD, autodiff, tensors, multi-agent orchestration, MCP, ONNX/GGUF
embedding, and language-level async AI APIs.

## Historical completion gate

The previous engineering directives are **not** complete. The core language
reaches a verified v0.1 subset, the rename is applied locally, and the offline
AI host foundation now exists with tests. The following previously requested
items remain open: modules, generics, traits, closures, iterators, `Result`/`?`,
async, concurrency, FFI, formatter, linter, package manager, standard library,
native reclamation, cycle handling, native map indexing, REPL multi-line
history, REPL field completion, enum benchmarks, cross-platform CI, debug info,
compiler optimization, language-level AI syntax, live local/cloud AI evidence,
and the GitHub repository rename. This audit is the baseline for closing them
honestly.
