# Roadmap

Status: Phase 0–1 complete; Phases 2–5 complete for the v0.1 language; Phase 6 in progress (typed IR + C11 backend for a statically typed subset including enums, guarded `match`, scalar literal patterns, nested payload patterns, top-level or-patterns, integer range patterns, and file-based modules); AI-0 through AI-4 host-side work complete offline (providers, streamed text chat, embeddings, RAG retrieval and evaluation); the AI-2 cloud path was verified live on 2026-11-24 and re-verified at 20:59Z with 21/21 checks, but access is not guaranteed; local-model evidence and later AI phases remain open. Phases are ordered to avoid architectural debt; items can move when evidence justifies it.

High-level summary and current sequencing: [`ROADMAP.md`](../ROADMAP.md). This file retains the detailed phase checklist.

## Phase 0 — Baseline and research ✅
Repository inspection, environment setup, prior-art review, risk register.

## Phase 1 — Architecture ✅ (v0.1 decisions)
Language identity, syntax, gradual type model, memory strategy, compiler pipeline, ADRs.

## Phase 2 — Foundation ✅ (v0.1)
- [x] Cargo project, module layout
- [x] Spans and diagnostic renderer
- [x] Indentation-aware lexer
- [x] Unit tests for lexer
- [x] Fuzz lexer/parser smoke harness (T-023; manual, not CI)

## Phase 3 — Parser ✅ (v0.1)
- [x] AST with expression IDs
- [x] Declarations, statements, Pratt expressions
- [x] String interpolation
- [x] Syntax diagnostics and recovery
- [x] Parser expression/block/type depth guards
- [x] Enum declarations and both match forms (T-014)
- [x] Scalar literal patterns for `Int`/`Str`/`Bool` (T-032)
- [x] Match guards after wildcard, variant, and literal patterns (T-033)
- [x] Fuzz tests (T-023 smoke harness; manual, not CI)

## Phase 4 — Execution ✅ (interpreter)
- [x] Tree-walking runtime
- [x] Values, environments, functions, recursion
- [x] Control flow, structs, collections, builtins, basic UTF-8 file I/O (T-058), and text builtins (T-060)
- [x] Runtime errors with spans and stack traces
- [x] Basic REPL
- [x] REPL persistence and error recovery (T-009)
- [x] REPL line editing, history, completion, and expression echo (T-029, ADR-0013)

## Phase 5 — Type system ✅ (v0.1)
- [x] Static types and inference for literals/operators
- [x] Name resolution, mutability, arity, struct checking
- [x] Gradual `Any` with runtime boundary checks
- [x] Nominal enums, payload typing, exhaustive match checking (T-014)
- [x] Scalar literal pattern typing and `Bool`/open-ended exhaustiveness (T-032)
- [x] Guard typing with payload bindings and conservative exhaustiveness (T-033)
- [ ] Flow narrowing, generics, traits

## Phase 6 — Native compiler 🚧
- [x] Typed IR and lowering for the static subset
- [x] C11 backend via system clang (ADR-0008)
- [x] `vantyx build`, linking, `-o`
- [x] Differential interpreter/native test suite (98 tests)
- [x] Native `List[T]` for statically typed elements (T-024, ADR-0009)
- [x] Native UTF-8 string indexing/iteration (T-025)
- [x] Native `Map[K,V]` with insertion order and structural keys (T-027, ADR-0010)
- [x] Native scalar-key map hash index (T-044, ADR-0016)
- [x] Native enums and `match` with tagged layout (T-030, ADR-0012)
- [x] Native scalar literal `match` for `Int`/`Str`/`Bool` (T-032)
- [x] Native match guards, including payload-reading guards (T-033)
- [x] Native nested payload patterns with path-based bindings (T-049, ADR-0020)
- [x] Native top-level or-patterns with shared bindings (T-050, ADR-0021)
- [x] Native integer range patterns with comparison lowering (T-051, ADR-0022)
- [x] File-based modules and imports in the interpreter and native backend (T-013)
- [x] Basic UTF-8 file I/O in the interpreter and native backend (T-058, ADR-0026)
- [x] Single-level directory operations in both engines (T-059, ADR-0027)
- [x] Recursive directory creation and removal in both engines (T-063, ADR-0031)
- [x] UTF-8 text builtins in both engines (T-060, ADR-0028)
- [x] Rename and byte-preserving file copy in both engines (T-061, ADR-0029)
- [x] Lexical POSIX-style path helpers in both engines (T-062, ADR-0030)
- [x] Cycle-safe structural equality for lists, maps, structs, and enums
- [ ] Debug info and optimization pipeline
- [ ] Cranelift/LLVM evaluation with benchmarks

## Phase 7 — Memory safety and ownership
- [ ] Escape analysis
- [ ] Optional ownership/borrow annotations
- [ ] Cycle handling
- [ ] Documented safety case

## Phase 8 — Performance
- [x] Benchmarks vs Python/Rust/C (T-022, `docs/benchmarks.md`)
- [ ] Profiling, constant folding, DCE, inlining, unboxing
- [ ] SIMD/vectorization exploration

## Phase 9 — Async and concurrency
- [ ] Design shared-state and cancellation rules
- [ ] Tasks, channels, structured concurrency
- [ ] Race/deadlock tests

## Phase 10 — Ecosystem
- [ ] Standard library (file I/O, single-level and recursive directories, lexical globbing, rename/copy, recursive directory copy, metadata probes, lexical paths, and basic text exist; metadata objects/preservation, recursive move, atomic copies, regex/case/normalization, grapheme-aware text, and recoverable errors remain)
- [ ] Formatter, linter, test runner
- [ ] Package manager and registry
- [ ] LSP and editor support

## Phase AI — AI-native toolchain

Detailed plan and exit gates: `docs/ai/AI_ROADMAP.md`.

- [x] AI-0 — historical audit, naming audit, ecosystem research, architecture,
      security model, and dependency decision.
- [x] AI-1 — dependency-free offline foundation: JSON, providers, budgets,
      embedding providers with a bounded cache, vector index, BM25, JSON Schema,
      tools, bounded agents, metrics, SSE, and `vantyx ai` CLI.
- [ ] AI-2 — live local-model evidence, model manager, keyring (compatible
      endpoint path and `models`/`doctor` work; the cloud OpenAI-compatible
      path was verified live on 2026-11-24 and re-verified at 20:59Z with
      21/21 checks, but access is not guaranteed).
- [ ] AI-3 — streamed text output, typed extraction, constrained decoding
      (validated extraction, bounded repair, and streamed OpenAI-compatible
      text chat complete; streaming tool calls, cancellation, and constrained
      decoding open).
- [x] AI-4 — embeddings, persistent local indexes, text/Markdown/code
      ingestion, cited chunks, hybrid retrieval, and retrieval evaluation.
      Grounded generation, reranking, and incremental updates remain open.
- [ ] AI-5 — write/shell/network tools with approvals, persistent agents.
- [ ] AI-6 — multimodal audio/video and size validation.
- [ ] AI-7 — tensor and numerical foundations after an ADR.
- [ ] AI-8 — GPU, distributed, and multi-agent only with verified hardware and
      measured value.
- [ ] Language-level AI syntax after async (T-017); modules (T-013) are
      complete.

## Phase 11 — AI and systems
- [ ] Tensors, numerical libraries (research only; see AI-7)
- [ ] GPU backend exploration (blocked on verified hardware; see AI-8)
- [ ] C ABI FFI, Python/Rust interop

## Phase 12 — Production readiness
- [x] v0.1 security audit (T-023, `docs/security-audit-2026-11-22.md`)
- [ ] Independent external audit
- [ ] Stability policy and release process
- [ ] Cross-platform CI
- [ ] Regression and stress suites
