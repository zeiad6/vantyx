# Task Queue

Ordered by priority. The orchestrator updates this file after each checkpoint.

## Completed in the current change (T-102; commit and CI pending)

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
