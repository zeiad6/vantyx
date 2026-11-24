# Project Baseline — Vantyx (repository: RustZ)

Date: 2026-11-23
Session: re-baselined after the Vantyx rename, the AI host foundation and `vantyx ai` CLI (T-037–T-039), the REPL upgrade (T-029), match guards (T-033), scalar literal patterns (T-032), native enums and `match` (T-030), the bracketed-match diagnostic (T-031), the sustainability research addendum (T-028), launch-kit preparation (T-035), and documentation reconciliation

## Current re-baseline (2026-11-23)

The sections below record the original empty-repository inspection. The repository is no longer empty.

| Area | Current state |
| --- | --- |
| Implementation | Rust 2024, zero third-party dependencies |
| Front end | Indentation-aware lexer, Pratt parser, spans, diagnostics |
| Type system | Gradual typing, `Any` runtime boundaries, nominal structs and enums, exhaustive match checking, scalar literal patterns, and `Bool`-checked match guards |
| Interpreter | Full v0.1 language, reference semantics, checked operations, call-depth guard, enum values, match, scalar literal dispatch, and guarded arm selection |
| Native path | Typed IR + GNU C11 backend for a statically typed subset (`Unit`/`Int`/`Float`/`Bool`/`Str`/`List[T]`/`Map[K,V]`/structs/enums) with UTF-8 string indexing/iteration, `match` with guards over enums and scalar literals, and cycle-safe structural equality |
| Native exclusions | `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, unsupported loop forms → `E0401` |
| Tests | 208 library unit + 10 CLI unit + 14 AI integration + 41 end-to-end + 67 differential native = 340 passing |
| Benchmarks | Six-workload baseline vs Python/C/Rust; raw results in `benchmarks/results/` (T-022); enums not yet benchmarked |
| AI host | Dependency-free `src/ai/` foundation: strict JSON, providers, mock/echo, budgets, embeddings, vector index, BM25, JSON Schema subset, typed tools, bounded agents, redacted metrics, SSE, OpenAI-compatible HTTP adapter; `vantyx ai providers/doctor/models/chat/tokens` |
| Build/tooling | `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, `cargo test` clean; `vantyx run/check/build/repl/ai`; REPL line editing, history, completion, and expression echo (T-029, ADR-0013); manual fuzz harness |
| Launch assets | Local tree renamed to Vantyx; public repository is still `zeiad6/auren` until the GitHub rename (T-042); MIT license, green CI on the old commit, v0.1.0 pre-release, pinned feedback issue, contribution/security/conduct policies, `CITATION.cff`, `llms.txt`, and an honest AI-recommendation/promotion kit under `docs/launch/` |
| Maturity | **Functional prototype**, not production-ready |
| Main risks | Intentional native leaks; O(n) native maps; unmeasured enum allocation; single-machine benchmark baseline only; docs drift; gradual `Any` erodes static guarantees; no live AI provider evidence; no model/tokenizer/GPU support |
| Next task | GitHub repository rename and clean-clone CI (T-042); nested/or-patterns (T-029–T-033 and T-035 complete) |

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
