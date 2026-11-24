# Project Status

Updated: 2026-11-23 (Vantyx rename applied and published at https://github.com/zeiad6/vantyx with clean-clone CI; dependency-free AI host foundation, `vantyx ai` CLI, and validated structured extraction verified offline; T-035 public repository, CI, and v0.1.0 pre-release; T-029 REPL line editing, history, completion, and expression echo; T-033 match guards; T-032 scalar literal patterns; T-030 native enums; T-031 bracketed-match diagnostic; T-028 sustainability addendum; documentation reconciled)

## Project overview

**Vantyx** is a gradual-typing programming language implemented in Rust. It targets Python-like ergonomics with Rust-like safety and a real native compilation path. Repository codename: RustZ.

- Language: Vantyx
- Repository: https://github.com/zeiad6/vantyx (public, MIT; renamed from `zeiad6/auren`)
- File extension: `.vtx`
- CLI: `vantyx`
- Implementation: Rust 1.100.0, 2024 edition, zero third-party dependencies
- Version: 0.1.0 (pre-release)
- Maturity: **functional prototype** — front end, interpreter, typed IR, and a native C backend for a statically typed subset that now includes closed enums, exhaustive `match`, scalar literal patterns, and match guards; plus an offline, dependency-free AI host foundation and `vantyx ai` CLI

## Current phase

**Phase 6 — native compiler, in progress; AI phase AI-1 complete.** The front end (Phases 2–3), interpreter (Phase 4), and gradual type checker (Phase 5) are complete for the v0.1 language, including closed enums, exhaustive matching, scalar literal patterns, and guards. The typed IR and C backend (Phase 6) cover typed lists, insertion-ordered maps, structs, string indexing and iteration, and native enums with guarded `match`. Memory reclamation, a native map hash index, debug information, and benchmark-driven optimization remain open. The AI host foundation (AI-0, AI-1, and the structured-output part of AI-3) is implemented and verified offline: providers, JSON, schema validation, validated structured extraction with bounded repair, budgets, embeddings, BM25, tools, bounded agents, metrics, and an OpenAI-compatible HTTP adapter exposed through `vantyx ai`. Live local/cloud AI evidence, streaming, server-side constrained decoding, and language-level AI syntax remain open.

## Implemented and verified

Every item below is exercised by the test suite or was manually verified through the CLI.

### Language front end
- Indentation-aware lexer with `INDENT`/`DEDENT`, tab rejection, CRLF handling, blank/comment-only lines, bracket continuation.
- Integer literals (decimal, `0x`, `0b`, `0o`, `_` separators), floats with exponents, strings with escapes and `{expr}` interpolation.
- Recursive-descent parser with Pratt expressions, single-line suites, struct constructors, enum declarations, match statements/expressions, and expression/block/type depth guards.
- Spans on every token and AST node; non-panicking error recovery in lexer and parser.
- Diagnostics with `E0xxx`/`W0001` codes, source line, caret, notes, and help text.

### Type system
- Gradual typing: unannotated parameters and bindings are `Any`; annotated boundaries are checked statically where possible and at runtime otherwise.
- `let` immutability and `var` mutability; duplicate-declaration detection; shadowing allowed only in inner scopes.
- Function arity, parameter types, return-path analysis, and annotated return checks.
- Nominal structs with typed fields and named or positional constructors.
- Nominal enums (T-014): payload variants, globally unique variant constructors, pattern bindings, static exhaustiveness (`E0316`), unreachable-arm warnings (`W0003`), and `Any`-match warning (`W0004`).
- Scalar literal patterns (T-032): `Int` (including negative), `Str`, and `Bool` literals in `match`; `Bool` exhaustiveness over both values; `Int`/`Str` open-ended matches require `_`; duplicate literals warn (`W0003`); literal/variant type mismatches are `E0315`/`E0301`.
- Match guards (T-033): `pattern if <expr>` in match statements and expressions; guards are type-checked as `Bool` (`E0301`; dynamic non-`Bool` is `R0005`), see payload bindings, run in source order, and never contribute exhaustiveness coverage. Repeated guarded arms are not reported as unreachable.
- No implicit numeric coercion; `/` produces `Float`, `//` floors toward negative infinity.
- Entry-point rules: top-level statements execute; otherwise zero-argument `fn main()` is called; warning `W0001` when both exist.

### Runtime (interpreter)
- Values: `Unit`, `Int` (i64), `Float` (f64), `Bool`, `Str`, `List`, `Map` (insertion-ordered), nominal `Struct`, nominal `Enum`.
- Enum construction, immutable payload slots, structural equality/display, `type` reporting the enum name, pattern matching, and `R0014` when no dynamic arm matches (T-014).
- Scalar literal matching on `Int`, `Str`, and `Bool` in both match statements and expressions, including negative integers (T-032).
- Guard evaluation after pattern bindings, with false guards continuing to the next arm and guard errors/scope cleanup handled correctly (T-033).
- Checked arithmetic with overflow detection, division-by-zero checks, bounds checks, missing-key errors.
- Reference semantics for lists, maps, and structs (`Rc<RefCell<...>>`).
- `if`/`elif`/`else`, `while`, `for` over lists, strings, and map keys; `break`/`continue`/`return`.
- Recursion with `MAX_CALL_DEPTH = 256` and evaluation-depth guard; interpreter runs on a dedicated 16 MiB stack.
- Runtime errors `R0001`–`R0014` with spans and stack traces; output failures become `R0013`; dynamic match failures are `R0014`.
- Builtins: `print`, `len`, `range`, `push`, `pop`, `contains`, `keys`, `values`, `str`, `int`, `float`, `bool`, `type`, `abs`, `min`, `max`, `sqrt`, `floor`, `ceil`, `assert`.

### Native backend (C11)
- `vantyx build <file.vtx>` lowers checked AST → typed IR → GNU C11 → executable via `clang` (override with `VANTYX_CC`), linked with libc/libm.
- `-o <path>` selects the output; default is the source stem.
- Supported native types: `Unit`, `Int`, `Float`, `Bool`, `Str`, `List[T]`, `Map[K, V]` for statically known type arguments, nominal structs, and nominal enums.
- Supported declarations/control flow: globals, functions, structs, enums, `match` statements and expressions with optional guards, `if`/`elif`/`else`, `while`, `for ... in range(...)`, `for x in list`, `for c in str`, `for k in map`, `return`, `break`, `continue`.
- Native enums (T-030, ADR-0012): heap-allocated tagged structs with a union of per-variant payloads. Construction, payload binding, wildcard arms, exhaustive matching, recursive and mutually recursive enums, enums in lists/maps/struct fields/globals/parameters/returns, structural display/equality/`type`, and cycle-safe payload handling all match the interpreter.
- Native scalar literal `match` (T-032): integer, negative-integer, string (`strcmp`), and boolean patterns, in both match statements and expressions; checked for type and exhaustiveness before lowering.
- Native match guards (T-033): guards on wildcard, variant, and literal patterns in both match forms. Payload-reading guards use a GNU C statement expression that tests the tag, declares the bindings, and evaluates the guard; side effects occur once per attempted arm.
- Native lists (T-024, ADR-0009): literals, read/write indexing, `len`, `push`, `pop`, `contains`, iteration, display, aliasing, nested lists, lists of structs, list fields/parameters/returns/globals, bounds errors, and cycle-safe display.
- Native maps (T-027, ADR-0010): literals, read/write/compound indexing, `len`, `contains`, `keys`, `values`, key iteration, display, aliasing, nested maps/lists/structs, structural keys, missing-key errors, and insertion-order preservation. Lookup is O(n) by design.
- Native strings (T-025): UTF-8-aware `s[i]` returning a one-character `Str`, `for c in s`, and `len(s)`; bounds and negative-index errors match the interpreter (`R0003`).
- Supported builtins: `print`, `len` (Str/List/Map), `push`, `pop`, `contains` (List/Str/Map), `keys`, `values`, `str`, `int`, `float`, `bool`, `type`, `abs`, `min`, `max`, `sqrt`, `floor`, `ceil`, `assert`.
- Checked integer arithmetic, allocation growth, and the same runtime error codes as the interpreter for the supported subset.
- Structural equality for lists, maps, structs, and enums, including coinductive handling of cyclic values (matches the interpreter).
- Float display matches the interpreter's shortest round-trip decimal formatting, including inside collections.
- 67 differential tests compare interpreter vs native exit codes and stdout.
- All five `examples/` programs build and run natively with identical output.

### AI host foundation (T-037, T-038, T-039; ADR-0015)
- Zero third-party dependencies: the AI toolkit is part of the `vantyx` crate and links only the Rust standard library; remote HTTPS uses the system `curl` binary, and local endpoints use `std::net`.
- Strict JSON value, parser, and writer with depth, size, and duplicate-key limits; no `serde`.
- Provider trait with chat/model-list capabilities, stable `AI_E_*` errors, retryability, usage, finish reasons, text and image content parts, and mock/echo providers.
- OpenAI-compatible non-streaming chat and model-list adapter; request mapping for messages, tools, response format, temperature, top-p, max tokens, and seed; bounded 8 MiB responses; timeouts; 401/429/5xx mapping; secret redaction.
- Network policy: loopback HTTP is local; remote HTTPS requires `--allow-network` or `VANTYX_AI_ALLOW_NETWORK=1`; no implicit downloads.
- Deterministic budgets for steps, tool calls, tokens, and wall-clock time; cancellation flag; bounded agent loop with tool-result feedback.
- Token estimates are explicitly labelled as estimates, not model tokenizers.
- Embedding vector math (dot, norm, cosine, Euclidean, normalization) and an in-memory vector index with deletion.
- BM25 lexical retrieval with Unicode-aware tokenization and ranked hits.
- JSON Schema subset validation (type, required, properties, additionalProperties, items, min/max items, min/max length, pattern, enum, const, numeric bounds) with paths.
- Typed tool registry with capability sets, deny-by-default policy, path roots, approval modes, and read-only demo tools; denied capabilities return errors to the agent instead of executing.
- Opt-in JSONL metrics containing provider, model, operation, latency, usage, finish reason, error code, retry count, and tool name — never prompts, responses, arguments, or secrets.
- SSE parser with size and line limits (not yet wired to streaming output).
- CLI: `vantyx ai providers`, `doctor`, `models`, `chat`, and `tokens`; `--json`, `--metrics`, `--system`, repeated `--message`, stdin prompts, and environment configuration.

### Tooling
- `vantyx run <file>` (also `-` for stdin), `vantyx check <file>`, `vantyx build <file> [-o out]`, `vantyx repl`, `vantyx ai <command>`, `vantyx version`, `vantyx help`.
- Exit codes: 0 success, 1 compile/IO error, 2 runtime error, 64 usage error.
- Persistent REPL (T-009): bindings, functions, structs, and mutable globals survive across entries; blocks end with a blank line; compile and runtime errors are reported without ending the session and failed entries are rolled back.
- REPL editing (T-029, ADR-0013): raw-mode line editing through the system `stty` with a canonical fallback; Tab completion for keywords, builtins, and top-level names; Up/Down history with optional persistence (`VANTYX_HISTORY`, default `~/.vtx_history`, mode 0600); expression values echo once; `:help`, `:history`, `:names`, and `:quit` meta-commands.

### Performance baseline (T-022)
- Six equivalent workloads (`arith_loop`, `fib`, `list_ops`, `map_ops`, `struct_ops`, `string_ops`) in Vantyx, Python, C, and Rust.
- The runner verifies identical stdout before timing, discards one warm-up, takes 5 measured runs, and records wall time, peak RSS, build time, and binary size.
- Native medians vs interpreter: `arith_loop` 5.21 ms vs 1300.9 ms, `fib` 3.31 ms vs 167.5 ms, `list_ops` 4.66 ms vs 383.6 ms, `map_ops` 64.2 ms vs 106.2 ms, `struct_ops` 9.11 ms vs 405.1 ms, `string_ops` 7.72 ms vs 59.8 ms.
- Native is in the same band as C/Rust on arithmetic, recursion, list, and struct workloads; map operations (~7.7× slower than hand-written C linear arrays) and immutable string concatenation (~2.9× slower than C) are the measured gaps.
- Raw results: `benchmarks/results/2026-11-22.{json,md}`; methodology and caveats: `docs/benchmarks.md`. The enum backend has not been benchmarked yet.

## Partially implemented

- **AI host** — the offline foundation, compatible-endpoint path, and validated structured extraction are verified, but no live local model or cloud provider request has been recorded; streaming output, server-side constrained decoding, model pull/remove with integrity checks, keyring storage, persistent agent state, embedding providers, hybrid retrieval, ingestion/chunking, retrieval evaluation, and a dedicated prompt-injection corpus remain open. Language-level `from ai import Client` is blocked on modules (T-013) and async (T-017).

- **Native backend subset** — `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, dynamic `match` on `Any`, unsupported loop forms, and string index assignment are rejected with `E0401`/`E0306`; native strings, structs, list buffers, map buffers, and enum values are leaked; no debug info. Map operations are O(n) (ADR-0010). Native enum layout and allocation policy are documented in ADR-0012.
- **Enums and pattern matching** — closed enums, payloads, exhaustive matching, `Any` matching, scalar literal patterns, and guards are implemented in the interpreter and for statically typed values natively. Nested/or-patterns, float/interpolated-string patterns, generic enums, and explicit discriminants are not implemented (ADR-0011).
- **Match expression placement** — a match expression is block-shaped and must be the final element of its statement or arm. It cannot appear inside parentheses or brackets because newlines are suppressed there; that case now reports `E0101` with a targeted help message (T-031).
- **REPL** — persistent, error-recovering, and now has line editing, history, completion, and value echo (T-029). Remaining limits: earlier entries are replayed with output muted, so long sessions repeat work; redefining a name is an error; history recall is single-line only; completion covers top-level names, not fields or locals; non-TTY input falls back to canonical line input (ADR-0013).
- **Runtime type metadata** — collections do not retain element type arguments after construction; deep conformance is checked only at annotated boundaries.
- **Memory management** — interpreter reference counting leaks cycles; native allocations are intentionally leaked. No cycle collector yet (ADR-0005, ADR-0008, ADR-0009, ADR-0010, ADR-0012).

## Not implemented

- Language-level AI syntax (`from ai import Client`, `await`) until modules and async exist
- Streaming chat output and cancellation in the CLI
- Server-side constrained decoding (local validation and bounded repair are implemented)
- Model manager `info`/`pull`/`remove` with checksums, size, license, and provenance
- Keyring-backed credential storage
- Live local/cloud AI verification
- Persistent agent state and resume
- Embedding providers, hybrid retrieval, ingestion/chunking, and retrieval evaluation
- Tensors, autodiff, SIMD, GPU, MCP, ONNX/GGUF embedding, multi-agent orchestration
- Native map hash index for O(1) expected lookup (ADR-0010 revisit criterion)
- Bytecode VM (T-011)
- Modules/imports (T-013)
- Nested/or-patterns (follow-up to T-014/T-030/T-032/T-033)
- Closures, iterators (T-015)
- `Result`/`?` error handling (T-016)
- Traits/generics (T-018)
- Async/concurrency (T-017)
- Standard library beyond builtins
- Formatter, linter, package manager, LSP (T-019, T-020)
- FFI (T-021)
- Ownership/borrow analysis (T-012)
- Profiling, optimization passes, and debug information
- Enum allocation and pattern-matching benchmarks

## Test status

Verified on 2026-11-23:

| Command | Result |
| --- | --- |
| `cargo build` | success |
| `cargo build --release` | success |
| `cargo clippy --all-targets -- -D warnings` | success, zero warnings |
| `cargo test` | **357 passed, 0 failed** (218 library unit + 11 CLI unit + 20 AI integration + 41 end-to-end + 67 native) |
| `vantyx run examples/*.vtx` | all 5 exit 0 with expected output |
| `vantyx build examples/*.vtx` + run | all 5 exit 0 with output identical to `run` |
| REPL pty smoke (`script`) | Tab completion (`pri` → `print`, `alp` → `alpha`), Up/Down history recall, Ctrl-C recovery, Ctrl-D exit 0, and persisted history all verified |
| Native enum smoke (`enum` + payloads + match statement/expression + lists + maps + equality + display + `type`) | interpreter and native output identical |
| Native scalar literal smoke (`Int`/`Str`/`Bool` matches, negative literals, match expression) | interpreter and native output identical |
| Native guard smoke (payload, wildcard, and literal guards, guarded match expression, side-effecting guard) | interpreter and native output identical |
| `vantyx ai providers` / `doctor --provider mock` | exit 0; offline status and conservative capabilities reported |
| `vantyx ai chat --provider mock` / `--provider echo` / `--json` | exit 0; deterministic text, echo, and parseable JSON with usage |
| `vantyx ai tokens "hello world"` | exit 0; estimate labelled "not a model tokenizer" |
| Remote HTTPS without `--allow-network` | exit 1 with `AI_E_NETWORK_DISABLED` and help |
| Loopback OpenAI-compatible adapter and CLI chat | request/response round trip verified by `tests/ai.rs`; no external network |
| `--metrics` redaction test | JSONL contains usage and status only; prompt absent |
| `vantyx ai extract` (mock fixture) | exit 0; returned schema-valid JSON; one invalid fixture repaired in 2 attempts; exhaustion exited 1 with `AI_E_SCHEMA` and "after 2 attempts" |
| Loopback HTTP extraction | request contained `response_format=json_object`; validated response and usage envelope returned |
| `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` | 9 depth/limit cases + 400 random/mutated inputs, all controlled diagnostics |
| Launch-material link check | 8 files checked; all relative links resolve |
| Native `Any` rejection demo | `vantyx run` prints the value; `vantyx build` exits 1 with `E0401` and help |
| GitHub Actions CI (run 45317367818) | passed build, 340 tests, clippy, release build, and differential examples on commit `e425d04` |
| Clean clone of `zeiad6/vantyx` | 340 tests passed; all five examples matched interpreter/native output |
| `gh repo view zeiad6/vantyx` | public repository on `main`; description and AI topics set |

Not re-run for this verification: `python3 benchmarks/run.py`. Its last recorded results remain in `benchmarks/results/`.

Native tests require `clang` (or `VANTYX_CC`) and skip when no compiler is available.

## Known bugs

- None known at this time. The REPL's replay strategy, native subset limits, native allocation leaks, and block-shaped match placement are documented limitations, not bugs.

## Risks

See `docs/PROJECT_BASELINE.md` and `docs/security-audit-2026-11-22.md`. The largest open risks are intentional native leaks, O(n) native map operations, enum allocation cost (unmeasured), gradual `Any` eroding static guarantees, and documentation drift. The drift found on 2026-11-23 (T-030 implemented but still marked READY) is mitigated by the status-sync rules in `AGENTS.md` and the `spec-sync-auditor` subagent.

## Public launch

Vantyx is published at **https://github.com/zeiad6/vantyx** (renamed from `zeiad6/auren`; the old URL redirects). The rename commit `e425d04` is on `main`, GitHub Actions CI run 45317367818 passed, and a fresh clone passed all 340 tests and all five interpreter/native comparisons. The public repository includes `.gitignore`, a GitHub Actions CI workflow, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CITATION.cff`, `llms.txt`, and `docs/launch/` with canonical descriptions, recommendation boundaries, launch drafts, and a demo script. The material states the pre-1.0 status, native subset, intentional leaks, O(n) maps, and missing tooling. GitHub Actions CI run 45308499010 passed on launch commit `ea4f2ec`, run 45308796021 passed after the checkout-action update on commit `86bfcb7` (both 241 tests), and run 45317367818 passed after the rename and AI foundation on commit `e425d04` (340 tests). The repository description and ten discovery topics are set. The [v0.1.0 pre-release](https://github.com/zeiad6/vantyx/releases/tag/v0.1.0) is published, and [issue #1](https://github.com/zeiad6/vantyx/issues/1) is pinned for questions and corrections. T-035 is VERIFIED.

## Current task

T-036 Vantyx rename — VERIFIED (code, docs, GitHub repository, and clean-clone CI). T-037 AI host foundation — VERIFIED. T-038 `vantyx ai` CLI — VERIFIED. T-039 OpenAI-compatible adapter — VERIFIED offline. T-040 live local/cloud verification — BLOCKED (no server or credentials). T-041 AI discovery and status synchronization — VERIFIED. T-042 GitHub repository rename and clean-clone CI — VERIFIED (https://github.com/zeiad6/vantyx; CI run 45317367818; clean clone passed 340 tests). T-043 validated structured extraction — VERIFIED locally. T-035 public repository and launch kit — VERIFIED on the old repository name. T-029, T-030, T-031, T-032, and T-033 — VERIFIED.

## Next tasks

1. T-040 live local/cloud AI verification when a server and credentials are available
2. Nested/or-patterns (follow-up to T-014/T-030/T-032/T-033)
3. Native map hash index if benchmarks justify it
4. T-011 bytecode VM feasibility study
5. Enum allocation benchmark before optimizing the native representation
6. T-034 REPL multi-line history and field completion (P4, BACKLOG)

## Last verification

`cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` on 2026-11-23: 218 library unit + 11 CLI unit + 20 AI integration + 41 end-to-end + 67 native = 357 tests passed, clippy clean. All five examples ran under the interpreter and as native binaries with identical output. Manual AI CLI smokes verified providers, mock/echo chat, JSON output, token estimates, doctor, network denial, and structured extraction with a repaired fixture; loopback OpenAI-compatible chat and extraction are covered by integration tests. GitHub Actions run 45317367818 passed on commit `e425d04` with the earlier 340-test suite, and a fresh clone of https://github.com/zeiad6/vantyx passed those tests and all five example comparisons. The T-043 commit is verified locally; CI evidence will be recorded after push. Live local/cloud AI, the REPL pty smoke, fuzz smoke, and benchmarks were not re-run for this verification; their earlier recorded results remain valid for the unchanged subsystems.
