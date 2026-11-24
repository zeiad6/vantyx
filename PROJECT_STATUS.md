# Project Status

Updated: 2026-11-24 (T-054 incremental RAG re-indexing with stable-chunk vector reuse and deletion pruning implemented and verified with the 463-test gate; T-053 history correction removed the unrelated placeholder contributor and moved `v0.1.0`; CI run 45365926013 passed with 457 tests and a clean clone of `e3d8bdd` passed 457 tests; T-052 streamed OpenAI-compatible chat implemented and verified offline with the 457-test gate; T-051 integer range patterns, T-050 native or-pattern lowering, T-049 native nested-pattern lowering, and T-048 nested payload patterns/matrix exhaustiveness verified with the prior 437-test gate; T-047 top-level or-patterns verified with the prior 422-test gate; T-013 modules/imports and T-046 local RAG indexing/retrieval evaluation verified with the prior 413-test gate; T-045 embedding providers, bounded cache, and `vantyx ai embed` verified with CI runs 45330680289 and 45330898922; T-044 native scalar-key map hash index verified with differential tests, a re-measured benchmark, and CI run 45321718008; Vantyx rename published at https://github.com/zeiad6/vantyx with clean-clone CI; dependency-free AI host foundation, `vantyx ai` CLI, and validated structured extraction verified offline; T-035 public repository, CI, and v0.1.0 pre-release; T-029 REPL line editing, history, completion, and expression echo; T-033 match guards; T-032 scalar literal patterns; T-030 native enums; T-031 bracketed-match diagnostic; T-028 sustainability addendum)

## Project overview

**Vantyx** is a gradual-typing programming language implemented in Rust. It targets Python-like ergonomics with Rust-like safety and a real native compilation path. Repository codename: RustZ.

- Language: Vantyx
- Repository: https://github.com/zeiad6/vantyx (public, MIT; renamed from `zeiad6/auren`)
- File extension: `.vtx`
- CLI: `vantyx`
- Implementation: Rust 1.100.0, 2024 edition, zero third-party dependencies
- Version: 0.1.0 (pre-release)
- Maturity: **functional prototype** — front end, interpreter, typed IR, and a native C backend for a statically typed subset that now includes closed enums, exhaustive `match`, scalar literal patterns, nested payload patterns in the interpreter and native backend, top-level or-patterns in the interpreter and native backend, integer range patterns in the interpreter and native backend, and match guards; plus an offline, dependency-free AI host foundation and `vantyx ai` CLI

## Current phase

**Phase 6 — native compiler, in progress; AI phase AI-1 complete, AI-3 streaming and structured output complete, and AI-4 host-side retrieval complete.** The front end (Phases 2–3), interpreter (Phase 4), and gradual type checker (Phase 5) are complete for the v0.1 language, including file-based modules, closed enums, exhaustive matching, scalar literal patterns, nested payload patterns, top-level or-patterns, integer range patterns, and guards. The typed IR and C backend (Phase 6) cover imported declarations, typed lists, insertion-ordered maps with a scalar-key hash index, structs, string indexing and iteration, and native enums with nested payload patterns, top-level or-patterns, integer range patterns, and guarded `match`. Memory reclamation, debug information, and further benchmark-driven optimization remain open. The AI host foundation (AI-0, AI-1, AI-3 streaming and structured output, and local retrieval/evaluation from AI-4) is implemented and verified offline: providers, JSON, schema validation, validated structured extraction with bounded repair, streamed text deltas, budgets, embedding providers with a bounded cache, BM25, local RAG indexing with hybrid retrieval and citations, retrieval evaluation, tools, bounded agents, metrics, and an OpenAI-compatible HTTP adapter exposed through `vantyx ai`. Live local/cloud AI and embedding-model evidence, streaming tool calls, server-side constrained decoding, and language-level AI syntax remain open.

## Implemented and verified

Every item below is exercised by the test suite or was manually verified through the CLI.

### Language front end
- Indentation-aware lexer with `INDENT`/`DEDENT`, tab rejection, CRLF handling, blank/comment-only lines, bracket continuation.
- Integer literals (decimal, `0x`, `0b`, `0o`, `_` separators), floats with exponents, strings with escapes and `{expr}` interpolation.
- Recursive-descent parser with Pratt expressions, single-line suites, struct constructors, enum declarations, match statements/expressions, and expression/block/type depth guards.
- Spans on every token and AST node; per-file source registry; non-panicking error recovery in lexer and parser.
- File-based modules (T-013): `import "lib/math.vtx"` and `from "lib/math.vtx" import add, Point`; paths resolve relative to the importing file and must stay under the entry file's directory; every file is parsed once; declarations flatten into one global scope; cycles, missing files, unknown selected names, path escapes, non-entry statements, and non-entry `main` produce `E0501`–`E0512`; imports are rejected in `run -` and the REPL (`E0510`).
- Diagnostics with `E0xxx`/`W0001` codes, source line, caret, notes, and help text.

### Type system
- Gradual typing: unannotated parameters and bindings are `Any`; annotated boundaries are checked statically where possible and at runtime otherwise.
- `let` immutability and `var` mutability; duplicate-declaration detection; shadowing allowed only in inner scopes.
- Function arity, parameter types, return-path analysis, and annotated return checks.
- Nominal structs with typed fields and named or positional constructors.
- Nominal enums (T-014): payload variants, globally unique variant constructors, pattern bindings, static exhaustiveness (`E0316`), unreachable-arm warnings (`W0003`), and `Any`-match warning (`W0004`).
- Scalar literal patterns (T-032): `Int` (including negative), `Str`, and `Bool` literals in `match`; `Bool` exhaustiveness over both values; `Int`/`Str` open-ended matches require `_`; duplicate literals warn (`W0003`); literal/variant type mismatches are `E0315`/`E0301`.
- Match guards (T-033): `pattern if <expr>` in match statements and expressions; guards are type-checked as `Bool` (`E0301`; dynamic non-`Bool` is `R0005`), see payload bindings, run in source order, and never contribute exhaustiveness coverage. Repeated guarded arms are not reported as unreachable.
- Top-level or-patterns (T-047): `Circle(size) | Square(size)` in both match forms. Every alternative must bind the same names with the same types (`E0317`); the arm receives one binding set. Unguarded alternatives contribute coverage; guarded or-patterns contribute none. `|` is not an expression operator.
- Nested payload patterns (T-048/T-049, ADR-0019/ADR-0020): `Node(Leaf(value), _)` in both match forms, up to 64 levels. Bindings from every level join one immutable arm scope; duplicates are `E0202`. Exhaustiveness uses a recursive constructor matrix, so `Leaf(value)` + `Node(Leaf(left), _)` + `Node(_, _)` is accepted and `Node(Leaf(value), _)` alone is `E0316`. Native lowering emits short-circuit tag/literal tests and declares bindings from absolute payload paths. Payload or-patterns and tuple patterns remain unsupported.
- No implicit numeric coercion; `/` produces `Float`, `//` floors toward negative infinity.
- Entry-point rules: top-level statements execute; otherwise zero-argument `fn main()` is called; warning `W0001` when both exist.

### Runtime (interpreter)
- Values: `Unit`, `Int` (i64), `Float` (f64), `Bool`, `Str`, `List`, `Map` (insertion-ordered), nominal `Struct`, nominal `Enum`.
- Module loading (T-013): imported functions, structs, and enums execute with the same semantics as declarations in the entry file; top-level statements remain entry-file-only; recursively imported declarations are deduplicated and every source span retains its original file.
- Enum construction, immutable payload slots, structural equality/display, `type` reporting the enum name, pattern matching, and `R0014` when no dynamic arm matches (T-014).
- Scalar literal matching on `Int`, `Str`, and `Bool` in both match statements and expressions, including negative integers (T-032).
- Guard evaluation after pattern bindings, with false guards continuing to the next arm and guard errors/scope cleanup handled correctly (T-033).
- Top-level or-pattern matching tries alternatives in order and installs the shared binding set in the arm scope (T-047). Nested payload matching recurses into payload values and merges bindings from every level (T-048).
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
- Native or-patterns (T-047/T-050, ADR-0021): top-level alternatives share one binding set; codegen emits a short-circuit alternative condition and initializes bindings from the matched alternative. Arm bodies and guards are emitted once, and differential tests compare interpreter/native stdout.
- Native nested payload patterns (T-048/T-049, ADR-0020): recursive `IrPattern` with absolute `IrFieldPath` bindings, short-circuit tag/literal tests, single-evaluation guards, and differential tests against the interpreter.
- Native lists (T-024, ADR-0009): literals, read/write indexing, `len`, `push`, `pop`, `contains`, iteration, display, aliasing, nested lists, lists of structs, list fields/parameters/returns/globals, bounds errors, and cycle-safe display.
- Native maps (T-027, ADR-0010, ADR-0016): literals, read/write/compound indexing, `len`, `contains`, `keys`, `values`, key iteration, display, aliasing, nested maps/lists/structs, structural keys, missing-key errors, and insertion-order preservation. Scalar keys (`Unit`, `Int`, `Float`, `Bool`, `Str`) use an open-addressing index with expected O(1) lookup at a 50% load factor; struct, enum, list, and map keys keep the linear structural-equality path.
- Native strings (T-025): UTF-8-aware `s[i]` returning a one-character `Str`, `for c in s`, and `len(s)`; bounds and negative-index errors match the interpreter (`R0003`).
- Supported builtins: `print`, `len` (Str/List/Map), `push`, `pop`, `contains` (List/Str/Map), `keys`, `values`, `str`, `int`, `float`, `bool`, `type`, `abs`, `min`, `max`, `sqrt`, `floor`, `ceil`, `assert`.
- Checked integer arithmetic, allocation growth, and the same runtime error codes as the interpreter for the supported subset.
- Structural equality for lists, maps, structs, and enums, including coinductive handling of cyclic values (matches the interpreter).
- Float display matches the interpreter's shortest round-trip decimal formatting, including inside collections.
- Imported functions, structs, and enums lower through the same IR and C backend as entry-file declarations (T-013); per-file source names are restored for diagnostics, and cross-file expression ids cannot collide.
- 73 differential tests compare interpreter vs native exit codes and stdout.
- All five `examples/` programs build and run natively with identical output.

### AI host foundation (T-037, T-038, T-039; ADR-0015)
- Zero third-party dependencies: the AI toolkit is part of the `vantyx` crate and links only the Rust standard library; remote HTTPS uses the system `curl` binary, and local endpoints use `std::net`.
- Strict JSON value, parser, and writer with depth, size, and duplicate-key limits; no `serde`.
- Provider trait with chat/model-list capabilities, stable `AI_E_*` errors, retryability, usage, finish reasons, text and image content parts, and mock/echo providers.
- OpenAI-compatible chat and model-list adapter; request mapping for messages, tools, response format, temperature, top-p, max tokens, and seed; bounded 8 MiB responses; timeouts; 401/429/5xx mapping; secret redaction.
- Streamed text chat (T-052, ADR-0023): `Provider::chat_stream` with a text-delta callback; OpenAI-compatible `stream: true` and `stream_options.include_usage`; `Accept: text/event-stream`; incremental SSE parsing with UTF-8 carry across chunks; final text/finish-reason/usage aggregation; usage estimation when omitted; error-frame redaction; streaming tool calls rejected with `AI_E_UNSUPPORTED`; loopback content-length, chunked, and close-delimited bodies; HTTPS through `curl --include --no-buffer`; CLI `--stream` with live flushes and `--json`/non-OpenAI rejection.
- Embedding provider interface (T-045, ADR-0017): typed batch requests with 128-input, 8 KiB-per-input, 32 KiB-per-request, and 1–4096-dimension limits; `Provider::embed` defaults to a stable unsupported error; `HashEmbeddingProvider` is a deterministic lexical feature-hash baseline, explicitly not a semantic model; `CachingEmbeddingProvider` is a bounded FIFO cache (1,024 entries / 4 MiB) keyed by model, dimensions, and text, with batch deduplication and clearable state; the OpenAI-compatible adapter posts to `/v1/embeddings`, sorts by index, validates counts and dimensions, and accounts usage.
- `vantyx ai embed` with offline `mock`/`hash`, OpenAI-compatible `openai`, `--dimensions`, stdin batches, JSON output, timeouts, and metrics; `echo` is rejected because it has no embeddings.
- Network policy: loopback HTTP is local; remote HTTPS requires `--allow-network` or `VANTYX_AI_ALLOW_NETWORK=1`; no implicit downloads.
- Deterministic budgets for steps, tool calls, tokens, and wall-clock time; cancellation flag; bounded agent loop with tool-result feedback.
- Token estimates are explicitly labelled as estimates, not model tokenizers.
- Embedding vector math (dot, norm, cosine, Euclidean, normalization) and an in-memory vector index with deletion.
- BM25 lexical retrieval with Unicode-aware tokenization and ranked hits.
- Local RAG foundation (T-046): bounded recursive ingestion of text, Markdown, and code with extension allowlists, hidden/symlink/VCS skipping, and file/count/size limits; stable line-cited chunks; provider-batched embeddings; strict-JSON index persistence with atomic replacement and 0600 permissions on Unix; BM25, cosine-semantic, and reciprocal-rank hybrid search with deterministic tie breaking; relative source-prefix filtering before top-k; query/top-k limits; and a retrieval-only evaluation harness reporting recall@k, precision@k, MRR, and per-query hits. It does not claim answer grounding.
- Incremental RAG updates (T-054, ADR-0024): `vantyx ai index --incremental` reuses a stored vector when the stable chunk id, provider, model, and dimensions match; embeds only new or changed chunks; prunes chunks absent from the current corpus scan; skips provider construction entirely when nothing changed; rejects lexical indexes and provider/model/dimension changes with actionable errors; and represents an empty semantic index as a valid version-1 file with zero chunks. Every update still walks and chunks the complete corpus.
- `vantyx ai index`, `vantyx ai search`, and `vantyx ai eval` with offline `hash`/`mock` embeddings, OpenAI-compatible embeddings, `--json`, chunk controls, extension filters, and explicit lexical/semantic/hybrid modes.
- JSON Schema subset validation (type, required, properties, additionalProperties, items, min/max items, min/max length, pattern, enum, const, numeric bounds) with paths.
- Typed tool registry with capability sets, deny-by-default policy, path roots, approval modes, and read-only demo tools; denied capabilities return errors to the agent instead of executing.
- Opt-in JSONL metrics containing provider, model, operation, latency, usage, finish reason, error code, retry count, and tool name — never prompts, responses, arguments, or secrets.
- SSE parser with size and line limits, wired to streamed chat output (T-052).
- CLI: `vantyx ai providers`, `doctor`, `models`, `chat`, `extract`, `embed`, `index`, `search`, `eval`, and `tokens`; `--json`, `--metrics` for chat/extract/embed, `--system`, repeated `--message`, stdin prompts, and environment configuration.

### Tooling
- `vantyx run <file>` (also `-` for stdin), `vantyx check <file>`, `vantyx build <file> [-o out]`, `vantyx repl`, `vantyx ai <command>`, `vantyx version`, `vantyx help`.
- Exit codes: 0 success, 1 compile/IO error, 2 runtime error, 64 usage error.
- Persistent REPL (T-009): bindings, functions, structs, and mutable globals survive across entries; blocks end with a blank line; compile and runtime errors are reported without ending the session and failed entries are rolled back.
- REPL editing (T-029, ADR-0013): raw-mode line editing through the system `stty` with a canonical fallback; Tab completion for keywords, builtins, and top-level names; Up/Down history with optional persistence (`VANTYX_HISTORY`, default `~/.vtx_history`, mode 0600); expression values echo once; `:help`, `:history`, `:names`, and `:quit` meta-commands.

### Performance baseline (T-022)
- Six equivalent workloads (`arith_loop`, `fib`, `list_ops`, `map_ops`, `struct_ops`, `string_ops`) in Vantyx, Python, C, and Rust.
- The runner verifies identical stdout before timing, discards one warm-up, takes 5 measured runs, and records wall time, peak RSS, build time, and binary size.
- 2026-11-24 native medians vs interpreter: `arith_loop` 5.910 ms vs 1422.623 ms, `fib` 3.376 ms vs 181.901 ms, `list_ops` 5.540 ms vs 480.670 ms, `map_ops` 3.597 ms vs 121.728 ms, `struct_ops` 9.730 ms vs 489.947 ms, `string_ops` 8.318 ms vs 74.813 ms.
- Native is in the same band as C/Rust on all six workloads. The largest remaining gap is immutable string concatenation (8.318 ms vs C 2.764 ms, ~3.0×); `map_ops` fell from 64.212 ms on 2026-11-22 to 3.597 ms after the scalar-key hash index, versus C 2.884 ms and Rust 4.495 ms.
- Raw results: `benchmarks/results/2026-11-22.{json,md}` and `benchmarks/results/2026-11-24.{json,md}`; methodology and caveats: `docs/benchmarks.md`. The enum backend has not been benchmarked yet.

## Partially implemented

- **AI host** — the offline foundation, compatible-endpoint path, validated structured extraction, streaming text output, embedding provider interface, local retrieval pipeline, and incremental re-indexing are verified, but no live local model, cloud provider, or live embedding-model request has been recorded; streaming tool calls, interactive cancellation, server-side constrained decoding, model pull/remove with integrity checks, keyring storage, persistent agent state, PDF/HTML/Office extraction, arbitrary metadata filters, cross-source deduplication, reranking, grounded generation, and a dedicated prompt-injection corpus remain open. Language-level `from ai import Client` is blocked on async (T-017); the modules prerequisite (T-013) is now implemented.

- **Native backend subset** — `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, dynamic `match` on `Any`, unsupported loop forms, and string index assignment are rejected with `E0401`/`E0306`; native strings, structs, list buffers, map buffers, map indexes, and enum values are leaked; no debug info. Scalar map keys use a hash index, but struct/enum/list/map keys remain O(n) and the fixed hash seed is not collision-resistant against adversarial keys (ADR-0016). Native enum layout and allocation policy are documented in ADR-0012.
- **Enums and pattern matching** — closed enums, payloads, exhaustive matching, `Any` matching, scalar literal patterns, nested payload patterns, top-level or-patterns, integer range patterns, and guards are implemented in the interpreter and native backend. Tuple patterns, range expressions, float/interpolated-string patterns, generic enums, and explicit discriminants are not implemented (ADR-0011, ADR-0019, ADR-0020, ADR-0021, ADR-0022).
- **Match expression placement** — a match expression is block-shaped and must be the final element of its statement or arm. It cannot appear inside parentheses or brackets because newlines are suppressed there; that case now reports `E0101` with a targeted help message (T-031).
- **REPL** — persistent, error-recovering, and now has line editing, history, completion, and value echo (T-029). Remaining limits: earlier entries are replayed with output muted, so long sessions repeat work; redefining a name is an error; history recall is single-line only; completion covers top-level names, not fields or locals; non-TTY input falls back to canonical line input (ADR-0013).
- **Runtime type metadata** — collections do not retain element type arguments after construction; deep conformance is checked only at annotated boundaries.
- **Memory management** — interpreter reference counting leaks cycles; native allocations are intentionally leaked. No cycle collector yet (ADR-0005, ADR-0008, ADR-0009, ADR-0010, ADR-0012).

## Not implemented

- Language-level AI syntax (`from ai import Client`, `await`) until async (T-017) exists; modules (T-013) are now available
- Streaming chat output and cancellation in the CLI
- Server-side constrained decoding (local validation and bounded repair are implemented)
- Model manager `info`/`pull`/`remove` with checksums, size, license, and provenance
- Keyring-backed credential storage
- Live local/cloud AI and live embedding-model verification
- Persistent agent state and resume
- PDF/HTML/Office ingestion, arbitrary metadata filters, cross-source deduplication, reranking, and grounded answer generation
- Tensors, autodiff, SIMD, GPU, MCP, ONNX/GGUF embedding, multi-agent orchestration
- Bytecode VM (T-011)
- Tuple patterns once a tuple type exists; usefulness-based interval analysis
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

Verified on 2026-11-24 (language, modules, native backend, map index, embeddings, and local retrieval); REPL pty retains its earlier evidence:

| Command | Result |
| --- | --- |
| `cargo build` | success |
| `cargo build --release` | success |
| `cargo clippy --all-targets -- -D warnings` | success, zero warnings |
| `cargo test` | **457 passed, 0 failed** (297 library unit + 13 CLI unit + 30 AI integration + 44 end-to-end + 73 native) |
| `python3 benchmarks/run.py --reps 5` | all implementations printed identical output; wrote `benchmarks/results/2026-11-24.{json,md}`; `map_ops` native 3.597 ms vs interpreter 121.728 ms |
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
| `vantyx ai providers` / `doctor --provider hash` | hash provider listed as offline deterministic embeddings only; doctor reported dimensions and `embeddings` capability |
| `vantyx ai embed --provider mock` (human and `--json`) | exit 0; deterministic L2-normalized vectors, dimensions and input count reported; JSON parsed |
| Loopback `/v1/embeddings` adapter and CLI | request path, model, input batch, and dimensions asserted; out-of-order response indexes sorted and validated; usage recorded |
| `vantyx ai embed --provider echo` | exit 64 with a clear "has no embeddings" error |
| `vantyx ai index` / `search` / `eval` (offline hash) | indexed 3 Markdown documents at 32 dimensions; hybrid top hit `rust.md:1-4` for "memory safety" with lexical and semantic ranks; one-query fixture scored recall@2 = 1.0 and MRR = 1.0 |
| Lexical RAG index | `--mode lexical` saved dimensions 0 and vectors 0; semantic search exited 1 with "no vectors"; `--mode hybrid` build and `--top-k 0` exited 64 |
| RAG plumbing benchmark | `python3 benchmarks/rag.py --reps 5 --docs 200 --dimensions 64 --top-k 10`: 200 docs / 600 chunks; index median 42.746 ms; hybrid search median 27.659 ms; incremental no-op median 47.377 ms (600 reused, 0 embedded); one-changed-chunk median 48.049 ms (599 reused, 1 embedded); index 1,247,300 bytes; 10 cited hits; raw results `benchmarks/results/2026-11-24-rag.{json,md}`. The incremental path still scans/chunks the corpus, so with the near-free hash provider it is not faster than a full rebuild; the saving is avoided embedding calls with a trained provider. |
| Imported-module native/interpreter comparison | entry importing `lib/math.vtx` (struct and functions) printed `14` identically under `vantyx run` and `vantyx build` |
| `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` | 9 depth/limit cases + 400 random/mutated inputs, all controlled diagnostics |
| Launch-material link check | 8 files checked; all relative links resolve |
| Native `Any` rejection demo | `vantyx run` prints the value; `vantyx build` exits 1 with `E0401` and help |
| GitHub Actions CI (run 45317367818) | passed build, 340 tests, clippy, release build, and differential examples on commit `e425d04` (pre-T-043) |
| GitHub Actions CI (run 45318733338) | passed build, 357 tests, clippy, release build, and differential examples on commit `2d991c6` |
| GitHub Actions CI (run 45321718008) | passed build, 358 tests, clippy, release build, and differential examples on commit `f55c91c` |
| Clean clone of `zeiad6/vantyx` | 340 tests passed; all five examples matched interpreter/native output |
| `gh repo view zeiad6/vantyx` | public repository on `main`; description and AI topics set |

Not re-run for this verification: the REPL pty smoke and live local/cloud AI. Their earlier recorded results remain valid for unchanged subsystems. The fuzz smoke was re-run and reported controlled diagnostics for all 409 inputs.

Native tests require `clang` (or `VANTYX_CC`) and skip when no compiler is available.

## Known bugs

- None known at this time. The REPL's replay strategy, native subset limits, native allocation leaks, and block-shaped match placement are documented limitations, not bugs.

## Risks

See `docs/PROJECT_BASELINE.md` and `docs/security-audit-2026-11-22.md`. The largest open risks are intentional native leaks, O(n) structural-key map operations and a fixed hash seed for scalar keys, enum allocation cost (unmeasured), gradual `Any` eroding static guarantees, and documentation drift. The drift found on 2026-11-23 (T-030 implemented but still marked READY) is mitigated by the status-sync rules in `AGENTS.md` and the `spec-sync-auditor` subagent.

## Public launch

Vantyx is published at **https://github.com/zeiad6/vantyx** (renamed from `zeiad6/auren`; the old URL redirects). The rename commit `e425d04` is on `main`, GitHub Actions CI run 45317367818 passed, and a fresh clone passed all 340 tests and all five interpreter/native comparisons. The public repository includes `.gitignore`, a GitHub Actions CI workflow, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CITATION.cff`, `llms.txt`, and `docs/launch/` with canonical descriptions, recommendation boundaries, launch drafts, and a demo script. The material states the pre-1.0 status, native subset, intentional leaks, map-key limits, and missing tooling. GitHub Actions CI run 45308499010 passed on launch commit `ea4f2ec`, run 45308796021 passed after the checkout-action update on commit `86bfcb7` (both 241 tests), run 45317367818 passed after the rename and AI foundation on commit `e425d04` (340 tests), and run 45318733338 passed with validated structured extraction on commit `2d991c6` (357 tests). The repository description and ten discovery topics are set. The [v0.1.0 pre-release](https://github.com/zeiad6/vantyx/releases/tag/v0.1.0) is published, and [issue #1](https://github.com/zeiad6/vantyx/issues/1) is pinned for questions and corrections. T-035 is VERIFIED.

## Developer identity and attribution

Vantyx is created and maintained by **Zeidex** — GitHub [`zeiad6`](https://github.com/zeiad6), website [zeidexlink.zeidexsoft.com/zeidex](https://zeidexlink.zeidexsoft.com/zeidex). The repository `CITATION.cff`, `Cargo.toml`, `LICENSE`, `README.md`, and `llms.txt` carry this identity. Historical commits were authored under the placeholder identity `Auren Contributors <auren@users.noreply.github.com>`, which GitHub mapped to an unrelated account with no work in this repository. On 2026-11-24, with explicit authorization, the public history was rewritten to replace that placeholder with `Zeidex <48607619+zeiad6@users.noreply.github.com>` for every reachable commit and to move the `v0.1.0` tag; the rewritten tree is byte-identical to the verified tree. Commit hashes and CI run references recorded before the rewrite are historical and no longer resolve on the default branch. `.mailmap` remains for references to old hashes, and no other person is credited as a contributor. The public contributors page lists only `zeiad6`; GitHub's REST contributors endpoint may serve a cached `auren` entry until it refreshes.

## Current task

T-054 incremental RAG re-indexing — VERIFIED locally (463-test gate; stable-chunk vector reuse, deletion pruning, no-op provider bypass, empty-index round-trip, and provider/model/dimension rejection). T-053 placeholder attribution correction — VERIFIED for the rewritten history and public contributors page (every reachable commit is authored and committed by Zeidex; the `v0.1.0` tag was moved; CI run 45365926013 passed with 457 tests; a clean clone passed 457 tests); GitHub's REST contributors cache still needs to refresh before that acceptance check is closed. T-052 streamed OpenAI-compatible chat — VERIFIED offline (457-test gate; text deltas, UTF-8 carry, usage/finish reason, error frames, tool rejection, loopback HTTP, HTTPS via curl, and CLI output). T-051 integer range patterns — VERIFIED locally (437-test gate; half-open and inclusive bounds, negative bounds, nested payload positions, or-pattern alternatives, empty-range rejection, and differential native output). T-050 native or-pattern lowering — VERIFIED locally (430-test gate; shared binding signature, short-circuit alternatives, conditional binding initialization, nested alternatives, and differential native output). T-049 native nested-pattern lowering — VERIFIED locally (429-test gate; recursive IR conditions, path-based bindings, nested guards, and differential native output). T-048 nested payload patterns — VERIFIED locally (429-test gate; recursive interpreter bindings, matrix exhaustiveness, and the original native `E0401` boundary, later lifted by T-049). T-047 top-level or-patterns — VERIFIED locally (422-test gate at the time; interpreter semantics, checker binding/type rules, and the original native `E0401` boundary, later lifted by T-050). T-013 modules and imports — VERIFIED locally (413-test gate; manual interpreter/native module smoke). T-046 local RAG indexing, hybrid retrieval, citations, and evaluation — VERIFIED locally (offline hash provider and CLI integration tests). T-036 Vantyx rename — VERIFIED (code, docs, GitHub repository, and clean-clone CI). T-037 AI host foundation — VERIFIED. T-038 `vantyx ai` CLI — VERIFIED. T-039 OpenAI-compatible adapter — VERIFIED offline. T-040 live local/cloud verification — BLOCKED (no server or credentials). T-041 AI discovery and status synchronization — VERIFIED. T-042 GitHub repository rename and clean-clone CI — VERIFIED (https://github.com/zeiad6/vantyx; CI run 45317367818; clean clone passed 340 tests). T-043 validated structured extraction — VERIFIED locally. T-044 native scalar-key map hash index — VERIFIED (CI run 45321718008). T-045 embedding provider interface, batch embeddings, cache, and `vantyx ai embed` — VERIFIED (CI runs 45330680289 and 45330898922; 383 tests). T-035 public repository and launch kit — VERIFIED on the old repository name. T-029, T-030, T-031, T-032, and T-033 — VERIFIED.

## Next tasks

1. T-040 live local/cloud AI and live embedding-model verification when a server and credentials are available, including first-token latency and throughput for streaming
2. Streaming tool calls and interactive cancellation (T-052 follow-ups)
3. RAG quality evaluation over a licensed multi-document corpus, plus PDF/HTML ingestion, arbitrary metadata filters, cross-source deduplication, and reranking (incremental re-indexing now ships)
4. Tuple patterns once a tuple type exists, and usefulness-based interval/unreachable analysis
5. T-011 bytecode VM feasibility study
6. Enum allocation benchmark before optimizing the native representation
7. Immutable string concatenation optimization (largest measured native gap)
8. T-034 REPL multi-line history and field completion (P4, BACKLOG)

## Last verification

`cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` on 2026-11-24: 301 library unit + 14 CLI unit + 31 AI integration + 44 end-to-end + 73 native = 463 tests passed, clippy clean. T-054 incremental RAG was exercised by four `src/ai/rag.rs` unit tests (reuse/prune, configuration mismatch, delete-all round-trip, vector-count validation), a `src/ai_cli.rs` parser test, and an offline CLI integration test that edits one document, deletes another, and checks reuse/prune counts and post-update search; a manual 3-chunk reproduction reported `reused_chunks:1`, `embedded_chunks:2`, `removed_chunks:2`, and `removed_sources:1`. All five examples ran under the interpreter and as native binaries with identical output. `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` reported 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics. T-052 streaming was exercised by unit tests for deltas, split UTF-8, `[DONE]`, error-frame redaction, 429 mapping, tools/tool-call rejection, usage fallback, content-length/chunked/close-delimited bodies, and request JSON; a loopback CLI SSE test; option-rejection tests; and `python3 tools/streaming_smoke.py`, which observed incremental HTTP and HTTPS deltas (first at 0.00 s/0.06 s, last at 1.20 s/1.22 s) with `stream: true` and `Accept: text/event-stream` on the wire. T-051 range patterns were exercised by parser tests for plain and nested ranges, checker tests for `Int` acceptance, non-`Int` rejection (`E0301`), and empty ranges (`E0318`), an e2e test covering half-open/inclusive boundaries, negative bounds, nested payload ranges, and or-pattern alternatives, and a differential native test that compares interpreter and native stdout. T-050 native or-pattern lowering was exercised by differential tests covering enum alternatives with multiple payloads, guards, scalar literals, `Bool` coverage, zero-payload variants, match expressions, and alternatives that bind nested payloads at different depths. T-048 nested patterns were exercised by two parser tests (parsing and depth rejection), four checker tests (complete recursive coverage, incomplete coverage, payload type mismatch, duplicate nested bindings), and an e2e recursive-tree test. T-049 native lowering was exercised by the same e2e program extended with enum-typed nested bindings, zero-payload nested variants, nested literals, and guards, plus a differential native test that compares interpreter and native stdout. T-047 or-patterns were exercised by parser/checker unit tests, an e2e test with guards and payload bindings, and differential native tests. T-013 modules were exercised by module unit tests, an e2e test, a differential native test, and a manual `14` interpreter/native comparison. T-046 RAG was exercised by 14 unit tests, 2 offline CLI integration tests, and a manual 3-document hash-embedding run whose hybrid top hit was `rust.md:1-4` for "memory safety" with recall@2 = 1.0 and MRR = 1.0. GitHub Actions runs 45317367818 (340 tests), 45318733338 (357), 45321718008 (358), 45330680289 (383), 45330898922 (383), and 45365926013 (457) passed on their respective commits; T-054 has not yet been CI-verified. The language benchmark suite was not re-run because T-054 only touches the AI host, not a benchmarked language hot path; the 2026-11-24 `map_ops` native median remains 3.597 ms. The separate RAG plumbing benchmark recorded index median 42.746 ms, hybrid search median 27.659 ms, incremental no-op median 47.377 ms (600 reused, 0 embedded), and one-changed-chunk median 48.049 ms (599 reused, 1 embedded) over 600 chunks in `benchmarks/results/2026-11-24-rag.{json,md}`; incremental updates still scan and chunk the corpus, so with the hash provider the measured gain is only avoided embedding calls, not wall time. Live local/cloud AI and the REPL pty smoke were not re-run; their earlier recorded results remain valid for unchanged subsystems.
