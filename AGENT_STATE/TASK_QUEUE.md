# Task Queue

Ordered by priority. The orchestrator updates this file after each checkpoint.

## In progress

- T-114 — Free nested native display temporaries. VERIFIED locally; commit
  `3617fe3`; CI pending. List, map, struct, and enum display now bind each
  rendered element, key, value, and field, append it, and free it through
  `vantyx_display_free`; nested Bool/Unit use owned helpers so every nested
  result is heap-owned. Two new native tests; ADR-0054; 736-test gate;
  focused measurement 100k Int peak RSS 6,552 → 3,616 KiB and 50k
  Pair(Bool, Bool) 9,368 → 5,580 KiB.

## Verified locally, commit pending CI

- T-113 — Display-focused differential fuzz shape. VERIFIED locally; commit
  `d8a476d`; CI pending. `tools/differential_fuzz.py` adds a twelfth
  `display_program` shape
  covering a three-node struct/list cycle, a self-referential node, a recursive
  `Chain` enum of depth 0–60, `Map[Key, Int]` with struct keys,
  `Map[List[Int], Str]`, `Map[Shape, Int]`, nested empty collections, quoted
  UTF-8 strings, and a list of cyclic nodes. `tools/test_differential_fuzz.py`
  adds determinism/well-formedness checks and now passes 9 tests; 100/100 cases
  agreed on seeds 1, 2, 3, and 500; no language or native behavior changed;
  734-test gate.

## Queued

- None. The next task will be selected after T-114 lands.

## Completed in `8911235` (CI pending)

- T-112 — Single-buffer native collection and nominal display. VERIFIED;
  committed as `8911235`; list, map, struct, and enum display append into one
  growable `VantyxStrBuilder` in source order, removing quadratic prefix
  copying and retained prefixes while preserving exact bytes, ordering, cycle
  markers, and depth limits. One differential native test; ADR-0053; 734-test
  gate; focused measurement median 754.707 ms → 5.737 ms and peak RSS
  568,244 KiB → 2,520 KiB.

## Completed in `45c438b` (CI pending)

- T-111 — Single-buffer native string construction. VERIFIED; committed as
  `45c438b`; interpolation and multi-operand string `+` chains build into one
  growable `VantyxStrBuilder` buffer in source order, avoiding retained
  intermediate strings while preserving effects and errors. Two differential
  native tests; ADR-0052; 733-test gate; focused measurement median
  59.893 ms → 34.414 ms and peak RSS 33,396 KiB → 17,564 KiB.

## Completed in `02cdf10` (CI pending)

- T-110 — CI runs native sanitizers and fuzz smoke. VERIFIED; committed as
  `02cdf10`; `.github/workflows/ci.yml` adds a `sanitizers` job that installs
  clang and runs the native suite with `VANTYX_CC` pointing at
  `tools/clang-sanitize.sh` (ASan + UBSan, leak detection disabled because
  v0.1 leaks native allocations intentionally), and the main `test` job now
  runs `tools/fuzz_smoke.py`. The exact sanitizer command passed all 147
  native tests locally; the fuzz smoke produced controlled diagnostics for 9
  depth/limit cases and 400 random/mutated inputs.

## Completed in `bffe38f` (CI pending)

- T-109 — Module graph accepts exactly 256 files. VERIFIED; committed as
  `bffe38f`; the loader checked `self.units.len() + 1 >= MAX_MODULES`, so with
  the entry file loaded the 256th file was rejected and the documented limit
  was effectively 255. The check is now `self.units.len() >= MAX_MODULES`; two
  unit tests build an exact-limit graph (entry + 255 imports, 256 flattened
  declarations) and an over-limit graph (exactly one `E0507`, no model). The
  exact-limit test fails against the old condition. 731-test gate; clippy/fmt
  clean; release build, five examples, seven Python self-tests, 100/100
  differential cases, fuzz smoke, 13 skills, and the link checker.

## Completed in `2104c55` (CI pending)

- T-108 — Native standard descriptors match Rust startup behavior. VERIFIED;
  committed as `2104c55`; generated `main` replaces closed descriptors 0–2 with
  `/dev/null` before user code, matching Rust's startup sanitization so a closed
  stdout silently discards `print` output and later file opens cannot reuse a
  standard descriptor number. One differential native test; 729-test gate,
  GCC native suite, release build, five examples, seven Python self-tests,
  100/100 differential cases, fuzz smoke, 13 skills, and the link checker.

## Completed in `9bb2353` (CI pending)

- T-107 — Native print reports output failures at the print site. VERIFIED;
  committed as `9bb2353`; native stdout was fully buffered, so a full disk was
  reported by the final flush without `R0013` or the print location, and a
  closed pipe could terminate the process with SIGPIPE. `vantyx_print_end` now
  flushes and checks each completed line, reports `R0013` with `strerror` and
  `(os error N)`, and `main` ignores SIGPIPE. Two differential native tests
  cover `/dev/full` and a closed pipe; 728-test gate, GCC native suite,
  release build, five examples, seven Python self-tests, 100/100 differential
  cases, fuzz smoke, 13 skills, and 117 links.

## Completed in `9e42a95` (CI pending)

- T-106 — Empty list literals adopt `contains`/`push` value types. VERIFIED;
  committed as `9e42a95`; `contains([], value)` and `push([], value)`
  previously checked as `List[Any]` and failed natively with `E0401`. The
  checker now checks the empty literal as `List[type(value)]`; non-empty
  arguments keep the old inference path and diagnostics. New native
  differential and e2e tests; 726-test gate, GCC native suite, release build,
  five examples, seven Python self-tests, 100/100 differential cases, fuzz
  smoke, 13 skills, and 117 links.

## Completed in `779783c` (CI run 45936065460 green)

- T-105 — Empty list literal adopts `join`'s `List[Str]` type. VERIFIED;
  pushed in `779783c`; `join([], separator)` previously checked as `List[Any]` and failed
  natively with `E0401`. The checker now passes `List[Str]` as the expected
  type for a direct empty list literal; non-empty arguments keep the old
  inference path and diagnostic. New native differential test; 724-test gate,
  GCC native suite, release build, five examples, seven Python self-tests,
  100/100 differential cases, fuzz smoke, 13 skills, and 117 links.

## Completed in `4269010` (CI run 45933776137 green)

- T-104 — Native call-depth diagnostics report the call site. VERIFIED;
  `vantyx_call_line`/`vantyx_call_column` are set immediately before each
  user-function invocation, the entry guard reports them, and the callee's
  source name is assigned only after the guard so cross-file failures name the
  caller's file. Direct recursion and a two-module probe match the
  interpreter; `native_call_depth_limit_exits_two` compares the headline and
  location lines. 723-test gate.

## Completed in `46b7d54` (CI run 45928466694 green)

- T-103 — Deterministic scalar-list sorting. VERIFIED; `sorted(list)`
  returns a new ascending `List[T]` for `T` in `Int`, `Float`, `Str`, or
  `Bool`, leaves the input unchanged, and is stable (`-0.0`/`0.0` equal, NaN
  last, UTF-8 byte order for strings, `false` before `true`). The checker
  rejects non-list arguments, wrong arity, and other element types; dynamic
  `Any` values are runtime-checked in the interpreter and remain under the
  native `E0401` boundary. The native backend uses a stable merge sort, not
  `qsort`. Four e2e tests, one differential native test, ADR-0050, 723-test
  gate.

## Completed in `a331f2d` (CI run 45925745650 green)

- T-102 — Label-stratified JSONL dataset splits. VERIFIED locally;
  `vantyx ai split --stratify <field>` groups records by the compact JSON
  spelling of a top-level scalar, assigns exact per-label counts, keeps every
  multi-record label in both splits, sends singletons to the majority split,
  preserves input order, and reports per-label counts. Bounds: 64 MiB input,
  1,000,000 records, 1,000 labels, 1,024-byte label. Three unit tests and two
  CLI tests.

## Completed in `c54c61c` (CI run 45923211883 green)

- T-101 — Deterministic JSONL dataset shuffle. VERIFIED; `vantyx ai shuffle`
  validates first, orders records by a domain-separated SHA-256 of the seed and
  exact record bytes, refuses overwrites and same-input output, removes failed
  output, holds the dataset in memory under 64 MiB and 1,000,000 records,
  normalizes BOM/CRLF/blank lines with warnings, and reports seed and
  input/output provenance. Four unit tests and two CLI tests; 713-test gate.

## Completed in `33c966e` (CI run 45920832199 green)

- T-100 — Exact JSONL duplicate-record removal. VERIFIED; `vantyx ai dedup`
  keeps the first exact record in input order, validates first, refuses
  overwrites and same-input output, removes failed output, caps the digest set
  at 1,000,000 distinct records, and reports input/output provenance. Four unit
  tests and two CLI tests; 707-test gate.
- T-099 — Native `range(...)` list values outside `for` loops. VERIFIED; the IR
  lowers 1–3 `Int` bounds to `List[Int]`, and codegen builds a typed `int64_t`
  list with the shared range count/limit checks and source-order argument
  evaluation. One differential test and two value-position error headlines;
  701-test gate.
- T-098 — Deterministic train/held-out dataset splits. VERIFIED; `vantyx ai
  split` validates first, assigns by a seeded content hash, refuses overwrites,
  guarantees non-empty splits, and reports actual counts and output digests.
  Five unit tests and two CLI tests; 700-test gate.
- T-097 — Filesystem OS-error suffix and glob parse diagnostics. VERIFIED;
  native filesystem failures now append Rust's `(os error N)` suffix, and
  malformed glob patterns use the shared parser's wording and quote range
  endpoints. `diff3.py` 44/44; 693-test gate.
- T-096 — Runtime diagnostic parity. VERIFIED; both engines agree on the first
  `error[...]` line for `int(Float)` (including `Infinity`/`NaN` and the `2^63`
  boundary), missing `Str` map keys, `range` limits, and
  `read_file`/`list_dir`/`glob` failures. Two differential tests compare the
  headline; 693-test gate.
- T-095 — JSON Schema conformance and shared preflight. VERIFIED; `required` is
  ignored for non-objects; empty `enum`/`allOf` fail preflight; `anyOf` and
  sibling keywords evaluate cumulatively; `const`/`enum` use JSON Schema
  instance equality; numeric bounds compare exactly beyond `2^53`; `extract`
  and `ToolRegistry::call` preflight schemas. Thirteen `ai::schema` tests, one
  extract integration test, one tools unit test, ADR-0045, 691-test gate.
- T-094 — Dataset schema validation. VERIFIED; `vantyx ai dataset --schema
  <file>` preflights a bounded schema and validates every structurally valid
  record, reporting the first 20 violations and the total; 10 dataset unit
  tests, 3 dataset CLI tests, ADR-0044.
- T-093 — Reusable agent skill library. VERIFIED; thirteen project-local
  OpenCode skills live under `.opencode/skills/` with `name`/`description`
  frontmatter and a Vantyx-specific workflow, commands, safety rules,
  acceptance criteria, troubleshooting, and examples; `docs/AGENT_SKILLS.md`
  is the inventory. The harness advertised all thirteen after creation. CI
  validates frontmatter, required sections, size, and unique IDs.
- T-092 — Bounded JSONL dataset inspection. VERIFIED; `vantyx ai dataset <file>`
  streams UTF-8 JSONL through SHA-256, accepts BOM/CRLF/blank lines, requires
  one top-level object per non-blank line, and reports size, line/blank/record/
  invalid counts, sorted field/type histograms, and up to 20 numbered invalid
  lines. Caps: 256 MiB file, 16 MiB line, 10,000 distinct top-level fields.
  Invalid content or I/O exits `1`; usage exits `64`. No tokenization; splits,
  deduplication, and schema checks ship separately.
- T-091 — Safetensors shard-index aggregation. VERIFIED; `vantyx ai inspect
  <model.safetensors.index.json>` parses a bounded weight map, validates every
  referenced shard with the T-090 Safetensors path, cross-checks index tensor
  names against each shard header, rejects missing, extra, duplicate, escaping,
  absolute, backslash, wrong-extension, and non-Safetensors shards, aggregates
  counts and dtypes, reports index and per-shard digests, and warns when
  `metadata.total_size` disagrees. T-090 added single-file GGUF/Safetensors
  inspection, T-089 added same-file `copy_file` refusal in both engines, and
  T-078 through T-088 are in the same commit. T-077 is committed in `1aba7ba`
  with CI run 45507195701 green.

## Ready

- Dataset tooling follow-ups — tokenizer-aware counts. Schema validation,
  splits, label-stratified splits, exact deduplication, and deterministic
  shuffle shipped in T-094/T-098/T-100/T-101/T-102; token-aware counts need
  their own bounded contract and tests.
- Skill maintenance — keep `docs/AGENT_SKILLS.md` synchronized and extend the
  CI validator when a skill is added or changed; frontmatter, required
  sections, size, and unique IDs are already checked.
- Offline model tooling follow-ups — optional checksum comparison for future
  pull/remove and clearer provenance output. Downloads, weight loading,
  conversion, and quantization remain separate tasks with their own integrity
  and license gates.
- Targeted native-runtime and display probes — Continue probing string,
  collection, recursion-depth, output-error, path, formatting, constant-result
  lowerings, rejected-argument evaluation, and cross-compiler evaluation-order
  edge cases against the interpreter and add each mismatch to the fuzzer. T-078
  through T-084, T-086, and T-087 were all found this way. The path probe now
  covers 2,514 paths (2,500 random plus 14 fixed), and evaluation-order, map,
  integer, range, float, string, error, glob, top-level-order, and control-flow
  probes showed no divergence.
- Native C evaluation-order audit — Whenever lowering emits a multi-argument C
  call or can render a constant result, bind observable operands explicitly and
  test effectful operands under both clang and GCC. T-081 and T-082 fixed the
  last known direct cases.
- Differential-fuzzer follow-ups — Optional sanitizer runs and coverage
  measurement. The bounded line-oriented shrinker shipped in T-077. Add each
  only with a bounded, reproducible contract.
- T-019b — Comment-preserving formatter. The lexer discards comments today, so
  this needs a lossless syntax representation and a separate ADR before code.
- Linter follow-ups — Suppression/configuration, loop-variable and match-binding
  coverage, all-terminating `if`/`match` reachability, and a deny mode. Add
  each only with a stable contract and tests.
- T-070/T-071 follow-up — Consider replacing GNU statement expressions with a
  statement prelude only if benchmarks and review justify the larger codegen
  change; do not start without evidence.
- Metadata objects/preservation, recursive move, REPL multi-line history and
  field completion, and string performance remain open P1/P2 candidates.

## Blocked

- T-040 — Live local-model and trained embedding-model evidence. No Ollama,
  llama.cpp, or LM Studio server/model is installed; only about 832 MiB of RAM
  was available. The cloud OpenAI-compatible path was verified on 2026-11-24
  and re-verified at 20:59Z, but access is not guaranteed.
- T-017 — Async/concurrency and language-level AI syntax. Blocked on an async
  design; host-side AI remains available through the CLI and Rust API.
- GPU, tensor, SIMD, autodiff, and training evidence — Blocked on available
  hardware and a reproducible backend.
- Independent `qa-verifier` verification — Blocked by the subagent provider's
  free usage limit.
- Native memory reclamation design review — Deferred until subagent capacity
  returns.
- Windows native support — Unverified; CI is Linux-only.

## Backlog candidates

- Recoverable `Result`/`?` errors and error propagation.
- Metadata objects or metadata preservation for file operations.
- Recursive move and atomic copy/write.
- Immutable string concatenation and multibyte iteration optimization.
- REPL multi-line history and field completion.
