# Initial Repository Audit

Date: 2026-11-24
Repository: RustZ (`vantyx`)
Auditor: autonomous engineering session
Canonical research directory: `RESEARCH/` (the repository already used this
uppercase name; `docs/research/` holds topic notes). This file avoids creating a
second `research/` tree with duplicate content.

## Method and limits

1. Inspected the source tree, tests, examples, tools, CI, Cargo manifest, and
   Git history directly.
2. Re-ran the build, clippy, test, differential, example, and fuzz-smoke gates
   instead of trusting prior status files.
3. Counted source and test files from the tree.
4. Recorded only observed results. The AI subsystem was inspected at the module
   and test level; its live-provider claims remain conditional on external
   access and are recorded separately in `docs/ai/LIVE_VERIFICATION.md`.
5. The original conversation prompts are not stored verbatim. Historical
   requirements were reconstructed in
   `RESEARCH/HISTORICAL_REQUIREMENTS_AUDIT.md` from task records and directives.

## Observed baseline (2026-11-24)

| Check | Observed result |
| --- | --- |
| Toolchain | `rustc 1.100.0`, `cargo 1.100.0`, `clang 21.1.8` |
| Dependencies | Zero third-party Rust dependencies; native builds invoke the system C compiler and link libc/libm |
| Source size | About 25,755 lines in top-level `src/*.rs` and about 37,313 lines including `src/ai/*.rs` |
| Tests | **613 passed**: 348 library unit, 15 CLI unit, 32 AI integration, 96 end-to-end, 122 differential native |
| Builds | `cargo build` and `cargo build --release` succeed |
| Lints | `cargo clippy --all-targets -- -D warnings` clean |
| Formatting | **`cargo fmt --check` fails** with 224 diff locations (pre-existing; see gap G-01) |
| Differential fuzzer | `tools/differential_fuzz.py --iterations 160 --seed 1` — 160/160 agree; also 160/160 on seeds 500 and 1000 with eight program shapes |
| Fuzz smoke | 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics |
| Examples | All five `examples/*.vtx` match interpreter and native output |
| CI | Run 45496041422 green on `5e1092f` with 613 tests; run 45493833079 green on `a4fe86b` with 584 tests |
| Unsafe Rust | No `unsafe` blocks found in `src/` or `tests/` (only comments and a test name) |
| Available optional tools | `rustfmt`, `clang` sanitizers, `gh`, `python3 3.14.10`; no `cargo-nextest`, `cargo-fuzz`, `cargo-audit`, `cargo-deny`, Miri, Valgrind, or `perf` |

## Actual architecture

| Path | Lines | Responsibility |
| --- | ---: | --- |
| `src/lexer.rs` | 899 | Indentation-aware lexer, spans, non-panicking recovery |
| `src/parser.rs` | 2,373 | Recursive-descent/Pratt parser, imports, depth guards |
| `src/ast.rs` | 405 | AST with stable expression ids and tree depth |
| `src/module.rs` | 731 | File-based module loader, path confinement, cycles, flattening |
| `src/checker.rs` | 3,959 | Scopes, gradual typing, enums, exhaustiveness, return paths |
| `src/interp.rs` | 2,882 | Reference tree-walking runtime |
| `src/ir.rs` | 2,781 | Typed backend-neutral IR and `E0401` boundary |
| `src/codegen_c.rs` | 5,076 | GNU C11 backend and runtime helpers |
| `src/lint.rs` | 575 | Read-only static lints `L0001`–`L0003` |
| `src/repl.rs` | 644 | Line editor, history, completion, expression echo |
| `src/glob.rs` | 666 | Lexical glob parser, matcher, bounded walk |
| `src/copy_tree.rs` | 403 | Recursive directory copy with containment and caps |
| `src/fs_safety.rs` | small | Destructive recursive-removal root refusal |
| `src/ai/` | about 11,500 | JSON, providers, schema, extraction, tools, agents, BM25/vector retrieval, RAG, HTTP |
| `src/ai_cli.rs` | 2,086 | `vantyx ai` command surface |
| `src/main.rs` | 527 | CLI dispatch, build driver, REPL entry |
| `tests/e2e.rs` | 96 tests | Language-level binary tests |
| `tests/native.rs` | 122 tests | Differential interpreter/native tests |
| `tests/ai.rs` | 32 tests | Offline AI CLI and loopback HTTP tests |

## Confirmed defects fixed in this session

- **T-070 (P0, fixed):** native `if`/`elif`/`while` conditions that lowered to a
  GNU statement expression produced invalid C (`if ({ ... })`). The condition
  helper now adds one pair of parentheses only for statement expressions.
- **T-071 (P0, fixed):** native `and`/`or` hoisted both operands and evaluated
  the right operand even when the left decided the result. Effectful right
  operands now lower through a lazy conditional expression.
- **T-074 (P1, fixed):** `block_returns` inspected only the last statement, so
  dead code after `return` caused a false `E0309`. It now walks reachable
  statements and stops at the first terminator.
- The first two defects were found by the new deterministic differential fuzzer
  (T-072), which is now part of CI.

No known P0 defect remains open at this audit. The items below are confirmed
limitations and risks, not claims of correctness.

## Confirmed gaps and risks

### G-01 — Repository is not rustfmt-clean (P2)

`cargo fmt --check` exits `1` with 224 diff locations, including
`src/ai/http.rs`, `src/codegen_c.rs`, and `tests/native.rs`. This is a
pre-existing quality gap, not a semantic defect. It violates the "pass
formatting" definition of done. A repository-wide `cargo fmt` plus a CI check
is a safe, mechanical follow-up; it must be a separate commit so it does not
hide semantic changes.

### G-02 — Differential coverage is narrow (P1)

The fuzzer now has eight typed program grammars (scalars/control flow,
collections, pattern matching, nested aliasing, typed functions, strings,
numerics, and runtime-error agreement), but it still has no shrinking,
sanitizer integration, or coverage measurement. It found two real P0 defects.
The expansion also found a test-quality bug: the collection generator used
`xs[-1]`, an invalid negative index, so that program stopped at its first
index and never exercised the remaining statements. A green fuzzer run is
evidence, not proof.

### G-03 — Native memory is intentionally leaked (P1)

Native allocations are not reclaimed. The interpreter uses reference counting
and also leaks cycles. There is no cycle collector, ownership system, escape
analysis, or sandbox. This is documented and must not be presented as a
finished memory model.

### G-04 — Native maps are not fully scalable (P1)

Scalar-key maps use an insertion-ordered hash index, but structural keys remain
O(n), and the scalar hash seed is fixed. This is documented in ADR-0010/0016
and is a performance/collision risk, not a semantic claim.

### G-05 — Filesystem operations are host-privileged (P1)

File, directory, glob, copy, rename, and recursive-removal builtins run with the
host process's privileges. Recursive removal refuses final `.`/`..` and the
filesystem root, but intermediate `link/sub` components can still traverse
symbolic links; there is no confirmation, trash, rollback, entry-count limit,
atomic staging, or `Result` recovery. `copy_dir_all` can leave a partial target.
Writes and copies are not atomic. Windows behavior is unverified.

### G-06 — No sandbox or capability boundary for language I/O (P1)

The AI host has capability-gated tools, budgets, redaction, and bounded agents.
The language runtime itself has no sandbox. Untrusted Vantyx programs and
untrusted AI output require application-level controls.

### G-07 — Tooling is incomplete (P2)

- The linter has three conservative rules with no suppression, configuration,
  machine-readable output, or fixes; `L0003` does not analyze all-terminating
  `if`/`match`.
- There is no formatter. The lexer discards comments, so a formatter needs a
  lossless representation first.
- There is no LSP, debugger, test runner, package manager, or stable ABI.
- REPL history recall is single-line and completion is top-level-only.

### G-08 — Cross-platform support is unverified (P2)

CI is Linux-only. The native backend uses POSIX `stat`, `lstat`, `opendir`,
`readdir`, `realpath`, `mkdir`, `unlink`, `rmdir`, and `rename`. macOS and
Windows builds and path spellings are not verified.

### G-09 — AI evidence is conditional (P3)

The cloud OpenAI-compatible path was verified on 2026-11-24 and re-verified at
20:59Z with 21/21 checks, but a later retry was geo-blocked and one run hit a
transient 503. There is no local-model evidence: no Ollama, llama.cpp, or LM
Studio server/model is installed, and available RAM was about 832 MiB at the
last check. There is no tokenizer, tensor runtime, GPU, SIMD, multimodal input,
PDF/HTML/Office ingestion, constrained decoding, tool-call streaming, or
grounded generation. Do not claim these capabilities.

### G-10 — Optional quality tools are unavailable (P2)

`cargo-nextest`, `cargo-fuzz`, `cargo-audit`, `cargo-deny`, Miri, Valgrind, and
`perf` are not installed. The zero-dependency policy reduces supply-chain risk,
but it does not replace a dependency audit, Miri-style UB checking, or
sanitizer runs in CI.

### G-11 — No property-based or mutation testing (P2)

The suite is example-based plus one deterministic differential fuzzer. There is
no shrinking, coverage measurement, mutation testing, or sanitizer job in CI.

## Missing feature inventory

| Area | Missing |
| --- | --- |
| Language | Generics, traits, closures, iterators, `Result`/`?`, async/concurrency, FFI, language-level AI syntax |
| Memory | Reclamation for native values, cycle handling, ownership/escape analysis, arenas |
| Native backend | `Any`, structural-key hashing, Windows support, debug info, optimization passes beyond the C compiler, LLVM/Cranelift |
| Standard library | Metadata objects/preservation, recursive move, atomic/streaming I/O, regex/case/normalization/grapheme text, `Path` type |
| Tooling | Formatter, LSP, debugger, test runner, package manager, lint configuration, machine-readable diagnostics |
| AI | Local models, tokenizer integration, tensor/GPU/SIMD, multimodal input, PDF/HTML/Office ingestion, reranking, grounded generation, tool-call streaming, cancellation, model manager/keyring, constrained decoding |
| Platforms | macOS and Windows CI and native verification |
| Process | Stable ABI, LTS policy, independent external audit |

## Highest-priority tasks

1. **P1 — Expand differential fuzzing.** Add grammars and seeds; fix any
   mismatch. This is the cheapest way to find another silent native divergence.
2. **P1 — Memory reclamation design.** Produce an ADR and a bounded prototype
   before claiming ownership or GC; preserve the intentional-leak disclosure
   until then.
3. **P1 — `Result`/`?` and recoverable I/O.** Replace fatal `R0015` for
   recoverable filesystem errors; keep fatal errors for programming errors.
4. **P2 — Repository-wide rustfmt baseline plus CI check.** Mechanical and
   independent of semantics; do it as its own commit.
5. **P2 — Formatter design.** Add comment retention to the lexer/parser before
   any rewrite-based formatter.
6. **P2 — Linter follow-ups.** Suppression/configuration, unused parameters and
   loop variables, all-terminating reachability, deny mode.
7. **P2 — Cross-platform CI.** At minimum build and run the non-native suite on
   macOS and Windows; native tests require a C compiler and platform helpers.
8. **P3 — AI local verification and multimodal/training work.** Blocked on
   hardware/models; do not fabricate evidence.

## Verification commands used

```sh
cargo build
cargo build --release
cargo clippy --all-targets -- -D warnings
cargo test
cargo fmt --check
python3 tools/differential_fuzz.py --iterations 100 --seed 1
python3 tools/fuzz_smoke.py --binary target/debug/vantyx
for f in examples/*.vtx; do ./target/debug/vantyx run "$f"; done
git status --short
git log --oneline -12
grep -rn 'unsafe' src tests
```
