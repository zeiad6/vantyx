# Project Baseline — Vantyx (repository: RustZ)

Date: 2026-11-24
Session: re-baselined after streamed OpenAI-compatible chat (T-052), integer range patterns (T-051), native or-pattern lowering (T-050), native nested-pattern lowering (T-049), nested payload patterns (T-048), top-level or-patterns (T-047), modules/imports (T-013), local RAG indexing/retrieval/evaluation (T-046), the embedding provider interface and `vantyx ai embed` (T-045), the native scalar-key map hash index (T-044), the Vantyx rename, the AI host foundation and `vantyx ai` CLI (T-037–T-039), validated structured extraction (T-043), the REPL upgrade (T-029), match guards (T-033), scalar literal patterns (T-032), native enums and `match` (T-030), the bracketed-match diagnostic (T-031), the sustainability research addendum (T-028), launch-kit preparation (T-035), and documentation reconciliation

## Current re-baseline (2026-11-24)

The sections below record the original empty-repository inspection. The repository is no longer empty.

| Area | Current state |
| --- | --- |
| Implementation | Rust 2024, zero third-party dependencies |
| Front end | Indentation-aware lexer, Pratt parser, spans, diagnostics, file-based imports |
| Type system | Gradual typing, `Any` runtime boundaries, nominal structs and enums, recursive matrix exhaustiveness, scalar literal patterns, nested payload patterns, top-level or-patterns with shared binding rules, integer range patterns, and `Bool`-checked match guards |
| Interpreter | Full v0.1 language, reference semantics, checked operations, call-depth guard, enum values, match, scalar literal dispatch, guarded arm selection, ordered or-pattern alternatives, recursive nested-payload bindings, and integer range matching |
| Native path | Typed IR + GNU C11 backend for a statically typed subset (`Unit`/`Int`/`Float`/`Bool`/`Str`/`List[T]`/`Map[K,V]`/structs/enums) with UTF-8 string indexing/iteration, an insertion-ordered scalar-key map hash index, imported declarations, `match` with nested payload patterns, top-level or-patterns, integer range comparisons, and guards over enums, scalar literals, and ranges, and cycle-safe structural equality |
| Native exclusions | `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, unsupported loop forms → `E0401` |
| Tests | 297 library unit + 13 CLI unit + 30 AI integration + 44 end-to-end + 73 differential native = **457 passing** locally on 2026-11-24 (T-052); last pushed CI run 45330898922 green on `458c08d` with 383 tests |
| Benchmarks | Six-workload baseline vs Python/C/Rust; 2026-11-24 results in `benchmarks/results/` after the map hash index (T-022, T-044); enums not yet benchmarked |
| AI host | Dependency-free `src/ai/` foundation: strict JSON, providers, mock/echo, budgets, embedding provider interface with batch limits, deterministic lexical hash embeddings and bounded cache, OpenAI-compatible `/v1/embeddings`, vector index, BM25, JSON Schema subset, validated structured extraction with bounded repair, streamed OpenAI-compatible text chat over SSE with incremental loopback HTTP and `curl` HTTPS transport, typed tools, bounded agents, redacted metrics, OpenAI-compatible HTTP adapter; `vantyx ai providers/doctor/models/chat/extract/embed/index/search/eval/tokens` |
| Build/tooling | `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, `cargo test` clean; `vantyx run/check/build/repl/ai`; REPL line editing, history, completion, and expression echo (T-029, ADR-0013); manual fuzz harness |
| Launch assets | Public at https://github.com/zeiad6/vantyx (renamed from `zeiad6/auren`); MIT license, green CI on the pre-rewrite `458c08d` (run 45330898922, 383 tests), v0.1.0 pre-release, pinned feedback issue, contribution/security/conduct policies, `CITATION.cff`, `llms.txt`, and an honest AI-recommendation/promotion kit under `docs/launch/`; creator metadata points to Zeidex (`zeiad6`) and the 2026-11-24 history correction replaced the placeholder commit identity and moved the `v0.1.0` tag |
| Maturity | **Functional prototype**, not production-ready |
| Main risks | Intentional native leaks; O(n) structural-key maps and a fixed scalar-key hash seed; unmeasured enum allocation; single-machine benchmark baseline only; docs drift; gradual `Any` erodes static guarantees; no live AI or trained embedding-model evidence; flat RAG search; streaming text only (no tool-call streaming or cancellation); no model/tokenizer/GPU support |
| Next task | Live local/cloud AI and trained embedding-model verification (T-040) when available, including streaming latency; streaming tool calls and cancellation; RAG arbitrary metadata fields/incremental updates; native memory reclamation design (T-013, T-029–T-033, T-035–T-039, T-042–T-052 complete or verified offline) |

The source tree is tracked on branch `main`. Publication and CI status are recorded in `PROJECT_STATUS.md`; do not infer a public URL from this baseline.

## Repository state at initial inspection

- Directory: repository root (the original machine-specific absolute path is intentionally omitted from the public record)
- Contents: **empty** (no files, no Git repository)
- No prior implementation, documentation, tests, or build scripts existed.
- Git was initialized during this session.

## Environment

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | 1.100.0 | installed via rustup during this session |
| Cargo | 1.100.0 | |
| clang | 21.1.8 | available for future native linking |
| LLVM | 21 / 22 shared libraries | available; not yet used |
| GCC | 16.2.0 | available |
| Python | 3.14.10 | available for benchmark comparisons |
| OS | Linux x86_64 | 4 cores, ~1 GiB RAM |
| Network | crates.io reachable | web search API unavailable; primary docs fetched directly |

## Project maturity

**Empty → early prototype.** The baseline is a greenfield language implementation.

## Architecture chosen

- Implementation language: Rust (memory safety, performance, ecosystem).
- Front end: indentation-aware lexer → AST → name resolution → gradual type checker.
- Execution: reference tree-walking interpreter, sharing the checked AST with future backends.
- Future native path: lower checked AST to an explicit IR, then a C or Cranelift/LLVM backend. No backend is claimed until it exists.
- No third-party runtime dependencies in the compiler.

## Initial risks

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Scope explosion across 12 phases | High | Vertical slices; only verified features enter status |
| Interpreter performance mistaken for native performance | High | Separate claims; benchmark later with equivalent workloads |
| Gradual `Any` erodes static safety | Medium | Runtime checks on all dynamic operations; diagnostics encourage annotations |
| Reference semantics for collections can surprise users | Medium | Documented; ownership/borrow analysis planned as opt-in advanced mode |
| `Rc` cycles leak memory | Medium | Documented limitation; cycle collector or ownership analysis planned |
| Indentation lexer edge cases (tabs, blank lines, continuations) | Medium | Dedicated lexer tests |
| Single 1 GiB machine limits heavy backend builds | Medium | Prefer lightweight backend path (C emission) before LLVM integration |
| Compiler panics on malformed input | High | Error-returning pipeline, depth guards, negative tests, and `tools/fuzz_smoke.py` (T-023) |

## Working / broken / missing (historical, initial baseline)

- Working: nothing yet at baseline.
- Broken: not applicable.
- Missing: all compiler stages, runtime, CLI, tooling, standard library, docs.

See the current re-baseline table at the top for the actual state.

## Baseline decision

Proceed with Phase 1 (architecture/specification) and Phase 2 (foundation) in one vertical slice, then verify with real builds and tests. No feature is marked VERIFIED without execution evidence.
