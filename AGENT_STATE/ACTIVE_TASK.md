# Active Task

## T-101 — Deterministic JSONL dataset shuffle

- Status: VERIFIED; pushed in `c54c61c`; CI run 45923211883 green
- Priority: P3 (AI host tooling)
- Dependencies: T-092, T-094, T-098, T-100, ADR-0043, ADR-0044, ADR-0046,
  ADR-0047

### Implemented

- `src/ai/dataset.rs` adds `ShuffleSummary`, `shuffle`, `shuffle_with_schema`,
  `shuffle_inner`, `read_shuffle_records`, `write_shuffle`, and
  `shuffle_record_digest`. Records are ordered by
  `SHA-256("vantyx-dataset-shuffle-v1\0" || seed_le64 || record_bytes)` with
  the exact record bytes as a tie-breaker, so the order depends only on the set
  of records and the seed.
- The whole dataset is held in memory under `MAX_SHUFFLE_BYTES` (64 MiB) and
  `MAX_SHUFFLE_RECORDS` (1,000,000); the existing 16 MiB line cap still applies.
- The command validates every record (and the optional schema) before creating
  the output, refuses an existing output and the input path, uses `create_new`,
  removes the output on failure, and verifies the record count before success.
- `src/ai_cli.rs` adds `vantyx ai shuffle <file> --out <file> [--seed <n>]
  [--schema <file>] [--json]` with text and JSON provenance reports.

### Evidence

- `cargo test --lib ai::dataset` — 23 passed.
- `cargo test --test ai dataset_shuffle` — 2 passed.
- `cargo test` — 713 passed (399 library unit + 15 CLI unit + 45 AI integration
  + 113 e2e + 141 native).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded.
- `python3 tools/test_differential_fuzz.py` passed 7 tests;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` agreed 100/100.
- All five examples built and ran natively with identical output.
- Manual smoke: two `--seed 7` runs produced byte-identical output and SHA-256
  values; the skill validator accepted all thirteen skills.

### Remaining

- Committed as `c54c61c`, pushed, and CI run 45923211883 passed.
- Scope limits: no external-memory sort, PRNG-defined permutation, reservoir
  sampling, tokenizer-aware count, label stratification, or split integration.
  A one-record dataset is necessarily unchanged.

## T-100 — Exact JSONL duplicate-record removal

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P3 (AI host tooling)
- Dependencies: T-092, T-094, T-098, ADR-0043, ADR-0044, ADR-0046, ADR-0047

### Implemented

- `src/ai/dataset.rs` adds `DedupSummary`, `deduplicate`,
  `deduplicate_with_schema`, `dedup_inner`, `scan_dedup_input`,
  `write_dedup`, and `dedup_record_hash`. The first occurrence of each exact
  normalized record is kept in input order.
- Record identity is the normalized record bytes (BOM, CRLF, and line
  terminator excluded). The digest is
  `SHA-256("vantyx-dataset-dedup-v1\0" || record_bytes)`.
- The command validates before creating output, revalidates during the write
  pass, verifies the input SHA-256 and record counts, and removes the output
  on any failure. It refuses an existing output and refuses to write over the
  input.
- The in-memory digest set is capped at 1,000,000 distinct records; the input
  itself remains streamed under the existing 256 MiB file and 16 MiB line
  caps.
- `src/ai_cli.rs` adds `vantyx ai dedup <file> --out <file> [--schema] [--json]`
  with text and JSON provenance reports.

### Evidence

- `cargo test --lib ai::dataset` — 19 passed.
- `cargo test --test ai dataset_dedup` — 2 passed.
- `cargo test` — 707 passed (395 library unit + 15 CLI unit + 43 AI integration
  + 113 e2e + 141 native).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded.
- `python3 tools/test_differential_fuzz.py` passed 7 tests;
  `python3 tools/differential_fuzz.py --iterations 500 --seed 7` agreed 500/500.
- All five examples built and ran natively with identical output.
- `/tmp/opencode/probe/diff6.py` matched 31/31.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: exact-byte identity only; no JSON canonicalization, near-
  duplicate detection, external sorting, shuffle, tokenizer-aware counts, or
  label stratification.

## T-099 — Native `range(...)` list values outside `for` loops

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P1 (native backend/language completeness)
- Dependencies: T-024, T-096, ADR-0009

### Implemented

- `src/ir.rs` no longer rejects `range` outside `for` loops. It validates the
  1–3 `Int` bounds and lowers the call to `IrType::List(IrType::Int)`.
- `src/codegen_c.rs` adds a `range` builtin case that binds each bound to an
  `int64_t` temporary in source order, then calls `vantyx_range_list`.
- `vantyx_range_list` reuses `vantyx_range_count` for step-zero and
  10,000,000-element limit checks, allocates a typed `int64_t` list with
  `vantyx_desc_int`, and computes each element in `__int128` before pushing.
- `for x in range(...)` keeps its direct loop lowering; only value-position
  calls build a list.

### Evidence

- `cargo test --test native range_values_match` — 1 passed.
- `cargo test --test native runtime_error_headlines_match_interpreter` — 1
  passed (including the two new value-position `range` cases).
- `cargo test` — 701 passed (391 library unit + 15 CLI unit + 41 AI integration
  + 113 e2e + 141 native).
- `VANTYX_CC=gcc cargo test --test native` — 141 passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded.
- `python3 tools/test_differential_fuzz.py` passed 7 tests;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` agreed 100/100.
- All five examples built and ran natively with identical output.
- `/tmp/opencode/probe/diff6.py` matched 29/31; the only mismatches were two
  known native limitations (`range` outside `for` before this change and
  `List[Any]` display).

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: `range(...)` materializes a list rather than a lazy iterator;
  a 10,000,000-element range allocates a large native list and native
  allocations remain intentionally leaked in v0.1.

## T-098 — Deterministic train/held-out dataset splits

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P3 (AI host tooling)
- Dependencies: T-092, T-094, T-043, ADR-0007, ADR-0015, ADR-0043, ADR-0044,
  ADR-0046

### Implemented

- `src/ai/dataset.rs` adds `split` and `split_with_schema`. Each non-blank
  record is assigned by `SHA-256("vantyx-dataset-split-v1\0" || seed_le64 ||
  record_bytes)`, using the top 32 bits against the ratio cutoff. Assignment is
  independent of line order and process state, and identical records always
  share a split.
- Outputs are derived from the input stem: `<stem>.train.jsonl` and
  `<stem>.heldout.jsonl`. Both are opened with `create_new`; any file created by
  a failed run is removed. The input is never modified.
- The CLI runs the `dataset` validation gate first, including `--schema`, and
  `split_with_schema` validates again during the scan. Invalid lines or
  schema-invalid records exit `1` before output exists.
- BOM, CRLF, and blank lines are normalized with warnings. A dataset with at
  least two records always produces two non-empty files; when the threshold
  leaves one side empty, the minimum- or maximum-hash record is moved
  deterministically and the report says so.
- `src/ai_cli.rs` adds `vantyx ai split` with `--out-dir`, `--train-ratio`,
  `--seed`, `--schema`, and `--json`, and shares dataset summary printing and
  validity helpers with `vantyx ai dataset`.

### Evidence

- `cargo test --lib ai::dataset` — 15 passed.
- `cargo test --test ai dataset_split` — 2 passed.
- `cargo test` — 700 passed (391 library unit + 15 CLI unit + 41 AI integration
  + 113 e2e + 140 native).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded.
- `python3 tools/test_differential_fuzz.py` passed 7 tests;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` agreed 100/100.
- All five examples built and ran natively with identical output.
- Manual smoke: 10-record BOM/CRLF dataset split 9/1 at ratio 0.7 seed 7;
  overwrite refused; seed 8 changed both output digests; invalid ratio exited
  `64`; one record exited `1`; invalid JSON and schema-invalid records exited
  `1` with no split files.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: no shuffle, deduplication, tokenizer-aware counts, label
  stratification, or extra validation split; the output pair is not atomic
  across a crash; the achieved ratio can differ from the requested ratio and is
  always reported.

## T-097 — Filesystem OS-error suffix and glob parse diagnostic parity

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P1 (interpreter/native correctness)
- Dependencies: T-058, T-060, T-086, T-096, ADR-0026, ADR-0033

### Implemented

- `src/codegen_c.rs::vantyx_os_error` formats `strerror(error_number)` followed
  by ` (os error N)`, matching Rust's Unix `io::Error` display. All 32 native
  filesystem diagnostic call sites use it.
- `vantyx_utf8_encode` encodes one Unicode scalar for diagnostics, and the three
  glob parse errors now say `invalid glob pattern: descending character range
  \`z-a\``, `invalid glob pattern: unclosed character class`, and
  `invalid glob pattern: empty character class`, matching `src/glob.rs`.
- `tests/native.rs::runtime_error_headlines_match_interpreter` gained 11 cases:
  remove/mkdir/rename/copy/stat/list failures, read and write on a directory,
  and three malformed glob patterns.

### Evidence

- `/tmp/opencode/probe/diff3.py` reported 10 mismatches before the fix and
  44/44 after it.
- `cargo test` — 693 passed (386 library unit + 15 CLI unit + 39 AI
  integration + 113 e2e + 140 native).
- `cargo test --test native` and `VANTYX_CC=gcc cargo test --test native` —
  140 passed each.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean.
- `diff.py` 55/55 and `diff2.py` 34/34 remained clean.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: Unix only; only the first diagnostic line is compared. An
  initial helper implementation called itself and segfaulted the native binary;
  the probe caught it before the regression test was added.

## T-096 — Runtime diagnostic parity for conversions, map keys, range limits, and file I/O

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P1 (interpreter/native correctness)
- Dependencies: T-058, T-060, T-061, T-078, T-079, T-086, T-087, ADR-0030

### Implemented

- `src/value.rs::format_float` is crate-visible, and `src/interp.rs::int` uses
  it so a failed float-to-int conversion renders the offending value through
  the shared Vantyx display (`-2.5`, `100000000000000000000.0`,
  `9223372036854776000.0`, `Infinity`, `-Infinity`, `NaN`).
- `src/codegen_c.rs::vantyx_int_from_float` includes `vantyx_float_str(value)`
  in its `R0007` message, matching the interpreter's wording.
- `src/codegen_c.rs::vantyx_map_get` renders a `Str` key with top-level display
  (unquoted), matching `Value::display` for a missing-key `R0008` error.
- `src/codegen_c.rs::vantyx_range_count` reports the requested element count,
  e.g. `` `range` would produce 10000001 elements, exceeding the limit of
  10000000``.
- `src/codegen_c.rs::vantyx_read_file` names the path in the 64 MiB, NUL-byte,
  and invalid-UTF-8 failures; `vantyx_list_dir` and `vantyx_glob_names_read`
  name the directory and use the interpreter's wording for a non-UTF-8 entry
  name.
- `tests/native.rs` gains `first_error_line`, `assert_error_headlines_match`,
  `runtime_error_headlines_match_interpreter`, and Unix-only
  `file_error_headlines_match_interpreter`.

### Evidence

- Both new tests failed before the runtime changes with exact first-line
  mismatches, then passed after them under clang and GCC.
- `cargo test` — 693 passed (386 library unit + 15 CLI unit + 39 AI
  integration + 113 e2e + 140 native).
- `VANTYX_CC=gcc cargo test --test native` — 140 passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean.
- `/tmp/opencode/probe/diff.py` — 55/55 match; `/tmp/opencode/probe/diff2.py`
  — 34/34 match after correcting unit-variant syntax and building the native
  binary outside the `glob("*")` working directory. Five `Any` native-backend
  rejections remain documented limitations, not divergences.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: only the first diagnostic line is covered at these sites; the
  source excerpt and caret can still differ because the native runtime reports
  its own line/column. The differential probes are outside the repository and
  CI.

## T-095 — JSON Schema conformance and shared preflight

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P3 (AI host correctness)
- Dependencies: T-043, T-094, ADR-0007, ADR-0015, ADR-0045

### Implemented

- `src/ai/schema.rs`: `required` applies only to objects; `check_schema`
  rejects empty `enum`/`allOf`; `anyOf` no longer returns early on a match;
  `additionalProperties`/`items` apply only to objects/arrays without skipping
  sibling keywords; `minimum`/`maximum` use exact numeric comparison.
- `src/ai/json.rs`: `value_eq` implements JSON Schema instance equality
  (mathematical numbers, order-independent objects, order-sensitive arrays);
  `number_cmp` compares `Int`, `UInt`, and `Float` exactly, with explicit
  `2^63`/`2^64` boundary checks against saturating casts.
- `src/ai/extract.rs`: preflights with `schema::check_schema` before building
  the provider request; a malformed schema is `AI_E_INVALID_REQUEST` with
  `extraction schema is invalid`.
- `src/ai/tools.rs`: preflights the declared parameter schema before argument
  validation and tool execution; a malformed schema is `AI_E_SCHEMA` with
  `tool <name> has an invalid parameter schema` and the tool body never runs.
- `docs/decisions/ADR-0045-json-schema-conformance-and-preflight.md` records
  the decisions, alternatives, and remaining subset limits.

### Evidence

- `cargo test --lib ai::schema` — 13 passed. New or extended tests:
  `required_is_ignored_for_non_objects`, `sibling_keywords_are_not_skipped`,
  `const_and_enum_use_json_value_equality`,
  `numeric_bounds_preserve_integer_precision`, and the extended
  `check_schema_rejects_malformed_keywords`. Each failed before its fix.
- `tests/ai.rs::extract_rejects_malformed_schemas_before_calling_the_provider`
  — the mock provider is not called.
- `src/ai/tools.rs::rejects_invalid_tool_schemas_before_running` — the tool
  body is not entered and `calls()` stays zero.
- Pre-fix probes: `1 != 1.0` for `enum`; `allOf: []` accepted every value; a
  matching `anyOf` skipped sibling `minimum`; `9007199254740992` passed
  `{"minimum":9007199254740993}` through `f64` conversion.
- `cargo test` — 691 passed (386 library unit + 15 CLI unit + 39 AI
  integration + 113 e2e + 138 native).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: no `$ref`/`$defs`, `oneOf`, `not`, `if`/`then`/`else`,
  `pattern`, `format`, `uniqueItems`, `contains`, `propertyNames`, tuple
  `items`, or remote references. Preflight checks shapes, not satisfiability.
  Programmatically constructed duplicate object keys are not detected.

## T-094 — Dataset schema validation

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P3 (AI host tooling)
- Dependencies: T-092, T-043, ADR-0007, ADR-0015, ADR-0044

### Implemented

- `src/ai/dataset.rs` adds `inspect_with_schema`, bounded schema loading with
  an optional UTF-8 BOM, `schema::check_schema` preflight, per-record
  `schema::validate`, first-violation reporting capped at 20 with a total
  count, and an optional `schema` summary object.
- `src/ai/mod.rs` exports the API; `src/ai_cli.rs` wires
  `vantyx ai dataset <file> --schema <file>`.
- `is_valid` and exit `0` require zero schema-invalid records; invalid JSON,
  invalid UTF-8, and non-object lines remain structural invalid lines.
- `docs/ai/DATASET_INSPECTION.md` and ADR-0044 document behavior, limits, and
  rejected alternatives. The command remains read-only, offline, and
  dependency-free.

### Evidence

- `cargo test --lib ai::dataset` — 10 passed, including five schema tests:
  `validates_records_against_a_schema`, `caps_reported_schema_violations`,
  `schema_validation_skips_structurally_invalid_lines`,
  `rejects_malformed_or_unreadable_schemas`, and
  `schema_file_accepts_a_utf8_bom`.
- `cargo test --test ai dataset_` — 3 passed, including
  `dataset_validates_records_against_a_schema`.
- `cargo test` — 691 passed (386 library unit + 15 CLI unit + 39 AI
  integration + 113 e2e + 138 native); clippy and fmt clean.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: structural validation only; no tokenization, splits, shuffle,
  deduplication, label-leakage detection, custom formats, or licensing
  certification. The schema path is recorded but not hashed, and the dataset
  digest is not an atomic snapshot.

## T-093 — Reusable agent skill library

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P2 (agent tooling)
- Dependencies: `AGENTS.md`, `.opencode/agents/`, `docs/ai/`, `benchmarks/`,
  `tools/`

### Implemented

- Added thirteen project-local OpenCode skills under `.opencode/skills/<id>/SKILL.md`:
  `rust-forensics`, `compiler-engineering`, `python-reverse-engineering`,
  `runtime-memory-safety`, `ai-systems-engineering`, `model-training`,
  `gpu-numerical-performance`, `security-audit-engineering`,
  `performance-benchmarking`, `multi-agent-orchestration`,
  `cross-platform-engineering`, `autonomous-recovery`, and
  `engineering-skill-orchestration`.
- Each skill declares YAML `name` and `description`, states when to use it,
  lists required tools, gives a concrete Vantyx workflow with commands,
  defines safety constraints and acceptance criteria, includes troubleshooting,
  and ends with an example.
- `docs/AGENT_SKILLS.md` records the mechanism, discovery paths, inventory,
  selection rules, and maintenance policy.

### Evidence

- A shell check over `.opencode/skills/*/SKILL.md` listed thirteen unique IDs,
  each with both frontmatter fields, and
  `python3 .opencode/skills/engineering-skill-orchestration/scripts/validate_skills.py
  --root .opencode/skills` validated the library.
- The active OpenCode harness advertised all thirteen in its available-skill list
  after the files were created, confirming that `.opencode/skills/` and the
  frontmatter are the supported mechanism.
- No Rust code changed for this task; the language gate is unaffected.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: skills are instructions, not enforcement. Agent permissions and
  `opencode/autoinvoke` still control availability. CI validates frontmatter,
  required sections, size, and unique IDs, but not the correctness of every
  command in a skill.

## T-092 — Bounded JSONL dataset inspection

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P3 (AI host tooling)
- Dependencies: T-037, T-038, T-041, T-090, ADR-0007, ADR-0015, ADR-0043

### Implemented

- `src/ai/dataset.rs` adds `DatasetSummary`, `FieldSummary`, `TypeCount`,
  `InvalidLine`, `inspect`, and a bounded line reader that hashes every byte
  while consuming oversized lines without buffering them.
- A dataset is UTF-8 JSONL with one top-level JSON object per non-blank line.
  A first-line BOM, CRLF, and blank lines are accepted. Invalid JSON, invalid
  UTF-8, and non-object records are counted, and the first 20 are reported with
  line numbers and messages.
- The summary reports path, format, byte size, SHA-256, line/blank/record/
  invalid counts, sorted top-level fields with record counts and JSON type
  histograms, and warnings. `--json` emits a stable object.
- Limits: 256 MiB per file, 16 MiB per line, 10,000 distinct top-level fields,
  20 reported invalid lines.
- `src/ai/mod.rs` exports the API; `src/ai_cli.rs` adds
  `vantyx ai dataset <file> [--json]`. Invalid content or I/O exits `1`, usage
  exits `64`, and a valid dataset exits `0`.
- `docs/ai/DATASET_INSPECTION.md`, ADR-0043, the README AI section,
  `examples/ai/README.md`, and `examples/ai/train.jsonl` document and exercise
  the command. Nothing is downloaded, executed, or modified.

### Evidence

- `cargo test --lib ai::dataset` — 5 passed: BOM/CRLF/blank lines and field
  types, invalid JSON/UTF-8/non-object lines, capped reporting, an empty
  dataset with the standard empty SHA-256, and the byte/line/field limits.
- `cargo test --test ai dataset_` — 2 passed: text and JSON output on a valid
  dataset, invalid-content exit `1`, and missing path, extra path, unknown
  option, absent file, and directory path errors.
- `cargo test` — 676 passed (373 library unit + 15 CLI unit + 37 AI integration
  + 113 e2e + 138 native).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; both builds succeed.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: syntactic/structural validation only; no tokenization, splits,
  shuffle, deduplication, schema/required-field checks, nested-field summaries,
  or duplicate-key detection. The digest is not an atomic snapshot of a
  concurrently written file.

## T-091 — Safetensors shard-index aggregation

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
  (T-078 through T-100 are in the same commit)
- Priority: P3 (AI host tooling)
- Dependencies: T-090, ADR-0007, ADR-0015, ADR-0041, ADR-0042

### Implemented

- `src/ai/model.rs` recognizes a Safetensors shard index by JSON object content
  or `.json` path when the 8-byte prefix is not a plausible Safetensors header
  length. It parses a bounded `weight_map` and optional metadata under a 64 MiB
  index cap, 10,000-shard cap, and 1,000,000-tensor cap.
- Every shard path must be relative, use `/` separators, contain no `..`
  component, end in `.safetensors`, and stay under the canonical index
  directory after symlink resolution. A shard that is itself an index is
  rejected rather than recursively expanded.
- Every referenced shard is validated with the T-090 Safetensors path. Each
  shard header is then re-read to compare its tensor names with the index:
  extra shard tensors, indexed tensors missing from their shard, and tensors
  appearing in more than one shard are format errors.
- The summary aggregates tensor count, parameter count, data bytes, and the
  dtype histogram; `file_bytes`/`sha256` describe the index JSON, and
  `ShardSummary` carries each shard's path, size, digest, tensor count,
  parameter count, and data bytes.
- `metadata.total_size` is advisory: a non-integer or a value that differs from
  the summed shard data bytes produces a warning, not a rejection.
- `src/ai/mod.rs` exports `ShardSummary`; `src/ai_cli.rs` prints the shard list
  and describes the new input; `--json` includes the `shards` array.
- Nothing is downloaded, loaded, modified, or executed; no dependency was
  added. `docs/ai/MODEL_INSPECTION.md` and ADR-0042 record behavior and limits.

### Evidence

- `cargo test --lib ai::model` — 13 passed (the four new shard-index tests plus
  the nine T-090 tests): a two-shard aggregate with per-shard digests, dtype
  merging, JSON shard output, a `total_size` warning, missing `weight_map`,
  escaping/absolute/wrong-extension paths, an extra shard tensor, an indexed
  tensor missing from its shard, a missing shard file, a non-Safetensors shard,
  a non-string shard value, a nested index, and a Unix-only symlink escape.
- `cargo test --test ai inspect_` — 3 passed; the new
  `inspect_aggregates_safetensors_shards` runs the CLI end to end in text and
  JSON modes and checks the missing-shard `AI_E_IO` path.
- `cargo test` — 669 passed (368 library unit + 15 CLI unit + 35 AI integration
  + 113 e2e + 138 native).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`,
  `cargo build`, and `cargo build --release` — clean.
- A manual two-shard index with leading whitespace printed the aggregate text
  and JSON summaries with both per-shard digests.
- CI run 45920832199 green.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: validation is best-effort and not atomic; a shard can change
  between its digest and its header check, and a symlink can be retargeted
  concurrently. The inspector does not scan the directory for unlisted shards,
  compare against a remote repository, download missing shards, verify
  licenses, or validate tensor values. `metadata.total_size` is advisory. Each
  shard header is read twice. GGUF remains single-file.

## T-090 — Offline GGUF and Safetensors model inspection

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-089
  are in the same commit)
- Priority: P3 (AI host tooling)
- Dependencies: T-037, T-038, T-041, ADR-0007, ADR-0015, ADR-0041

### Implemented

- `src/ai/model.rs` adds bounded GGUF v2/v3 and Safetensors parsing, tensor
  range/overlap/dtype/block-size validation, quantization labels, metadata
  projection, warnings, an incremental SHA-256, and `ModelSummary::to_json`.
- `vantyx ai inspect <file> [--json] [--metadata]` reports format and version,
  file size, lowercase digest, tensor and parameter counts, architecture, name,
  context length, embedding length, layer count, quantization, tokenizer model
  and token count, dtype histogram, selected or all metadata, and warnings.
- Unsupported or corrupt files are `AI_E_FORMAT` with a help line and exit `1`;
  I/O failures are `AI_E_IO`; usage errors exit `64`.
- Nothing is downloaded, loaded, modified, or executed; tensor payloads are
  never read into memory. The whole file is streamed through SHA-256.
- Explicit limits: 256 MiB GGUF metadata/tensor directory, 64 MiB per GGUF
  string, 50,000,000 array elements, 100,000 metadata entries, 1,000,000 tensor
  descriptors, rank 8, 100 MiB Safetensors header, and 10,000 Safetensors
  metadata entries.
- `docs/ai/MODEL_INSPECTION.md` and ADR-0041 record behavior, limits, and
  rejected alternatives.

### Evidence

- `cargo test --lib ai::model` — 9 passed at T-090 time (SHA-256 vectors,
  minimal fixtures, truncation, unknown format, bad JSON, overlapping ranges,
  out-of-range data, non-UTF-8 header).
- `cargo test --test ai inspect_` — 2 passed at T-090 time (both formats,
  `--json`, `--metadata`, format/I/O/usage errors).
- `cargo test` — 664 passed at T-090 time (364 library unit + 15 CLI unit + 34
  AI integration + 113 e2e + 138 native).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`,
  `cargo build`, and `cargo build --release` — clean.
- CI run 45920832199 green.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: GGUF v1 is rejected; GGUF text is decoded lossily; unknown GGML
  type IDs are reported with a warning instead of a guessed byte size; tensor
  values, embedded signatures/checksums, licenses, and chat templates are not
  validated; shard indexes were added by T-091; hashing time grows with file
  size. No download, pull, remove, conversion, quantization, or weight loading
  is added.

## T-089 — Refuse same-file `copy_file` targets

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-088
  and T-090 are in the same commit)
- Priority: P0 (interpreter/native safety)
- Dependencies: T-061, ADR-0029

### Implemented

- Opening the target for writing truncated it before any source bytes were
  read, so `copy_file(path, path)` destroyed the source and copied the truncated
  bytes. Symbolic-link and hard-link aliases had the same effect.
- `src/fs_safety.rs::paths_refer_to_same_file` follows both paths with
  `std::fs::metadata` and compares device and inode on Unix, falling back to
  canonical paths elsewhere. Metadata failures return `false` so the copy
  reports the useful error.
- `src/interp.rs::copy_file` calls the guard after its regular-file check and
  before `File::create`; the native `vantyx_copy_file` helper performs the same
  check with `stat` before `fopen(to, "wb")`. Both raise `R0015` with
  `source and target are the same file`.
- ADR-0029 and specification §12 document the guard as best-effort and
  raceable, not atomic.

### Evidence

- Pre-fix probe: `copy_file(path, path)` exited `0` and left a zero-byte file.
  After the fix both engines exited `2` with `R0015` and preserved the source
  bytes for the direct path, a symbolic-link alias, and a hard-link alias.
- Tests: `src/fs_safety.rs::detects_the_same_path_and_distinct_files` and
  Unix-only `detects_symbolic_and_hard_link_aliases`;
  `tests/e2e.rs::copy_file_rejects_self_copies`;
  `tests/native.rs::copy_file_rejects_self_copies_in_both_engines`.
- `cargo test` — 664 passed (364 library unit + 15 CLI unit + 34 AI integration
  + 113 e2e + 138 native).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`,
  `cargo build`, and `cargo build --release` — clean.
- CI run 45920832199 green.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: the check runs before the target is opened, so a caller can
  still race it. It does not compare contents, protect bind-mount or reflink
  aliases, or make the copy atomic.

## T-088 — Source-order evaluation of named constructor arguments

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-087
  and T-089–T-090 are in the same commit)
- Priority: P1 (interpreter/native correctness)
- Dependencies: T-001, T-072, ADR-0001

### Implemented

- Named struct-constructor arguments may be written in any order but are
  stored in declaration order. The native IR lowering put them into
  declaration order before code generation, so the emitted C call evaluated
  them in declaration order. A probe with
  `Pair(right=note("right", 2), left=note("left", 1))` printed `right`, `left`
  in the interpreter and `left`, `right` under both clang and GCC; a
  three-field `Triple(c=..., a=..., b=...)` showed the same defect. Positional
  constructors were already correct.
- `IrExprKind::Construct` gains `eval_order`, a list of argument indices in
  source order while `args` stays in declaration order. `lower_constructor`
  records each named argument's declaration position and emits the identity
  order for positional calls.
- `src/codegen_c.rs::construct_expr` evaluates effectful arguments into typed
  temporaries in `eval_order`, then calls the generated constructor with the
  temporaries in declaration order. Pure and identity-order constructors keep
  the ordinary call path.
- Specification §5.5 states that named constructor arguments are evaluated in
  the order written and then placed into declaration order.

### Evidence

- Pre-fix probe: interpreter printed `right`, `left`, `c`, `a`, `b`; clang and
  GCC printed `left`, `right`, `a`, `b`, `c`. After the fix all three agreed
  byte for byte, including a failing earlier argument (`right=fail()`) that
  exits `2` with empty stdout in both engines.
- Tests: e2e `named_constructor_arguments_evaluate_in_source_order` pins the
  interpreter output for two- and three-field named constructors, positional
  constructors, and a failing earlier argument; differential native
  `named_constructor_arguments_evaluate_in_source_order` compares both engines
  on the success and failure programs; the `collection_program` fuzzer shape
  now constructs `Point` with named arguments in both orders and effectful
  values.
- `cargo test` — 664 passed (364 library unit + 15 CLI unit + 34 AI
  integration + 113 e2e + 138 native).
- `VANTYX_CC=gcc cargo test --test native` — 138 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`,
  `cargo build`, and `cargo build --release` — clean.
- All five `examples/` programs — interpreter and native output identical.
- `python3 tools/test_differential_fuzz.py` — 7 passed;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100
  agreement.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: only named struct-constructor arguments need reordering; enum
  variant payloads are positional and function calls reject named arguments
  natively. The fix adds no general expression-sequencing IR node and does not
  change allocation or field layout.

## T-087 — Lexical function scope in the interpreter

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-086
  and T-088 are in the same commit)
- Priority: P1 (interpreter/native correctness)
- Dependencies: T-001, T-072, ADR-0040

### Implemented

- The interpreter kept one flat scope stack and searched it from newest to
  oldest. A function call pushed its frame onto that stack, so name resolution
  continued into every caller frame. A caller local with the same name as a
  global therefore shadowed the global inside the callee.
- The differential probe showed three failures: `read_total()` returned the
  caller's local `100` instead of the global `10`; `bump_total()` silently
  mutated the caller's local instead of the global; and `bump_other()` raised
  `R0010` against an immutable caller local instead of updating the global.
  The checker and native backend resolved all three names to the global.
- `Interpreter` gains `frame_base`, the index of the first scope in the active
  call. `lookup` and `find_scope` search scopes from `frame_base` onward and
  then root scope zero; `call_user` saves the previous boundary, sets it
  before pushing the function scope, and restores it after the call, including
  on the error path.
- Specification §3.1 and §11 state the lexical rule; ADR-0040 records the
  decision and rejected dynamic scoping.

### Evidence

- Pre-fix probe: interpreter printed `100` and then exited `2` with `R0010`;
  native printed `10`, `11`, `100`, `21`, `caller-immutable`. After the fix
  both engines produced identical stdout and exit codes.
- A second probe covered recursion, nested blocks, a string-typed caller
  local, and globals mutated from a recursive callee; both engines agreed.
- Tests: e2e `functions_resolve_globals_not_caller_locals`; differential
  native `functions_use_lexical_globals_matching`; the `functions_program`
  fuzzer shape now declares two globals and a caller that shadows both.
- Full gate results are recorded in `SESSION_STATE.md` and
  `PROJECT_STATUS.md`; CI run 45920832199 green.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: the fix covers name reads and assignment targets. It does not
  add closures, nested functions, or a separate global table; it preserves the
  existing root-scope representation and the leaked-allocation policy. No
  performance measurement was taken because the change affects interpreter
  lookup only and makes no performance claim.

## T-086 — Trailing-dot path stem and extension parity

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-085
  are in the same commit)
- Priority: P1 (interpreter/native correctness)
- Dependencies: T-062, ADR-0030

### Implemented

- Native `path_stem` and `path_extension` searched for a dot that had at least
  one byte after it. For a name whose final dot is trailing, such as `a/b..`,
  they fell back to an earlier dot, while the interpreter always takes the
  last dot and then rejects it when it is first or last. Native therefore
  returned `b` / `.` where the interpreter returned `b..` / `""`.
- `src/codegen_c.rs` now finds the final dot once in
  `vantyx_path_extension_dot` and returns no separator when it is absent or
  last. Both helpers share the result, so no earlier dot is considered after
  a trailing final dot.
- ADR-0030 states the no-earlier-dot rule explicitly.

### Evidence

- Pre-fix probe: four divergences across `path_stem("a/b..")`,
  `path_extension("a/b..")`, `path_stem("a/...")`, and
  `path_extension("a/...")`; after the fix the same probe had no diff.
- A 2,514-path differential probe (2,500 random plus 14 fixed) covering `/`,
  dots, trailing dots, multiple dots, backslashes, and Unicode names emitted
  no diff.
- Tests: extended `tests/e2e.rs::path_builtins_round_trip` and
  `tests/native.rs::path_builtins_match`; new differential native
  `path_trailing_dot_names_keep_the_whole_stem` over six stems and six empty
  extensions.
- Full gate results are recorded in `SESSION_STATE.md` and
  `PROJECT_STATUS.md`; CI run 45920832199 green.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: lexical byte behavior only; no filesystem access, `..`
  normalization, Windows syntax, or NUL-containing string support.

## T-085 — Lint rule for unused function parameters

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-084
  are in the same commit)
- Priority: P2 (tooling)
- Dependencies: T-073, ADR-0037

### Implemented

- `vantyx lint` tracked local and global bindings but deliberately skipped
  function parameters, so a parameter that was never read produced no finding.
- `Binding` now records a `parameter` flag, `Linter::declare` takes it, and
  function parameters are declared as tracked bindings instead of ignored
  names. `pop_scope` emits `L0004` for an unread parameter and keeps `L0001`
  for unread locals and globals.
- The leading-underscore exemption still applies. A read anywhere in the
  function, including a guard or a nested block, suppresses the finding.
  Parameters are immutable, so `L0002` never applies to them. Loop variables
  and `match` payload bindings remain unreported.

### Evidence

- Tests: six `src/lint.rs` unit tests (`used_parameter_is_clean`,
  `unused_parameter_is_reported_once`,
  `unused_parameter_in_void_function_is_reported`,
  `underscore_parameter_is_ignored`,
  `parameter_read_in_a_nested_block_is_clean`,
  `unused_parameter_and_unused_local_are_distinct`) and e2e
  `lint_reports_unused_parameters`.
- CLI probe: `fn constant(value: Int)` produced one `warning[L0004]` at the
  parameter span and exited `0`; the five `examples/` programs linted clean.
- `cargo test` — 642 passed (353 library unit + 15 CLI unit + 32 AI
  integration + 109 e2e + 133 native).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`, and
  `cargo build --release` — clean.
- `python3 tools/test_differential_fuzz.py` — 7 passed;
  `python3 tools/differential_fuzz.py --iterations 20 --seed 1` — 20/20
  agreement.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: lexical only. There is no trait or interface conformance
  reason to keep an unread name, no suppression attributes, no configuration,
  and no automatic fix. Imported-module functions are linted in place.

## T-084 — Evaluate the argument of `bool` in the native backend

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-083
  are in the same commit)
- Priority: P1 (interpreter/native correctness)
- Dependencies: T-001, T-010, T-072, T-080, ADR-0001

### Implemented

- Native `bool(value)` lowered a statically non-`Bool` argument to
  `vantyx_bool_from_other(...)` without emitting `value`. The interpreter
  evaluates the argument before raising `R0007`, so `bool(side())` printed
  `side` there and skipped it natively.
- `src/codegen_c.rs::builtin_expr`'s `bool` branch now binds the argument to a
  typed temporary. A `Bool` argument yields it; other types emit
  `(void)temporary; vantyx_bool_from_other(...)`, so evaluation happens first.
- This mirrors the T-080 `type` fix and keeps the failure path effect-aware.

### Evidence

- Tests: e2e `bool_evaluates_its_argument`, native
  `bool_evaluates_its_argument`, and the fuzzer `error_program` case
  `print(bool(bool_side_int()))`.
- `cargo test` — 636 passed (348 library unit + 15 CLI unit + 32 AI
  integration + 108 e2e + 133 native).
- `VANTYX_CC=gcc cargo test --test native` — 133 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`,
  `cargo build`, and `cargo build --release` — clean.
- All five `examples/` programs — interpreter and native output identical.
- `python3 tools/test_differential_fuzz.py` — 7 passed.
- `python3 tools/differential_fuzz.py --iterations 40 --seed 1|2|3` (clang and
  GCC) — 40/40 agreement on each seed under both compilers.
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs produced controlled diagnostics.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: `bool` still accepts only statically typed `Bool`; the
  temporary is evaluated for effects and then discarded. Other built-ins that
  can reject an argument statically need the same audit if their lowering
  stops emitting the operand.

## T-083 — Evaluate expressions interpolated as `Unit`

- Status: VERIFIED locally; pushed in `33c966e`; CI run 45920832199 green
(T-078 through T-082
  are in the same commit)
- Priority: P1 (interpreter/native correctness)
- Dependencies: T-001, T-010, T-072, T-079, ADR-0001, ADR-0039

### Implemented

- Native interpolation previously rendered a `Unit` expression as `()` without
  emitting the expression. The interpreter evaluates an interpolated expression
  like any other expression, so `"{side()}"` printed the call's output before
  `()` there, while native printed only `()` and never called `side`.
- `src/codegen_c.rs::display_value` now renders `Unit` as
  `((void)(value), vantyx_str_from_unit())`, keeping the expression in the
  emitted C while yielding `()` only after evaluation.
- `display_expr` delegates to `display_value`, so `print` arguments and
  interpolation share one rendering rule.

### Evidence

- The `/tmp/opencode/p34/unitstr.vtx` reproducer matched the interpreter under
  clang and GCC after the fix.
- Tests: e2e `unit_interpolation_evaluates_expressions`, native
  `unit_interpolation_evaluates_expressions`, and the fuzzer functions shape
  (`note_interp` plus `"a{{note_interp()}}b"`).
- `cargo test` — 636 passed (348 library unit + 15 CLI unit + 32 AI
  integration + 108 e2e + 133 native).
- `VANTYX_CC=gcc cargo test --test native` — 132 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`,
  `cargo build`, and `cargo build --release` — clean.
- All five `examples/` programs — interpreter and native output identical.
- `python3 tools/test_differential_fuzz.py` — 7 passed.
- `python3 tools/differential_fuzz.py --iterations 40 --seed 1|2|3` (clang and
  GCC) — 40/40 agreement on each seed under both compilers.
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs produced controlled diagnostics.

### Remaining

- Committed as `33c966e`, pushed, and CI run 45920832199 passed.
- Scope limits: a `Unit` still renders as constant `()`; only its evaluation is
  preserved. The native backend still rejects `Any`, so interpolation of
  dynamically typed expressions remains outside the native subset.

## T-082 — Evaluate both operands of native `Unit` equality

- Status: VERIFIED locally; commit `33c966e`; CI run 45920832199 green in the same tree
- Priority: P1 (interpreter/native correctness)
- Native `Unit ==`/`!=` lowered to a constant and emitted neither operand;
  `binary_expr` hoisted only when both sides had effects, so a sole effectful
  operand was dropped.
- `binary_expr` now hoists both operands whenever either has effects,
  preserving left-to-right evaluation for every binary operator.
- Tests: e2e and native `unit_equality_evaluates_both_operands`, plus fuzzer
  `note(...)` comparisons.
- Specification §5.3/§11 document the rule.

## T-081 — Compiler-independent argument order for native `contains` on strings

- Status: VERIFIED locally; commit `33c966e`; CI run 45920832199 green
- Priority: P1 (interpreter/native correctness and portability)
- Dependencies: T-001, T-010, T-060, T-072, ADR-0001
- Native lowering emitted `vantyx_str_contains(haystack, needle)` with both
  operands inline; C leaves argument evaluation order unspecified, so GCC
  evaluated effectful operands right to left while clang and the interpreter
  used left to right.
- `src/codegen_c.rs::builtin_expr` now binds both operands to `vantyx_str`
  temporaries in source order.
- Tests: extended native `text_builtins_evaluate_arguments_left_to_right` with
  `contains`, `starts_with`, and `ends_with`; new native
  `string_contains_argument_order_is_compiler_independent` builds and runs the
  probe with both clang and GCC.
- Scope limits: the cross-compiler test skips when GCC is unavailable. C
  argument evaluation remains unspecified in general; every observable
  multi-argument native call must keep binding its arguments explicitly.

## T-080 — Evaluate the argument of `type` in the native backend

- Status: VERIFIED locally; commit `33c966e`; CI run 45920832199 green in the same tree
- Priority: P1 (interpreter/native correctness)
- The backend previously yielded a static string without evaluating `value`,
  skipping nested output, mutation, and runtime errors, so
  `print(type(assert(false)))` printed `Unit` and exited `0` instead of raising
  `R0006`.
- `src/codegen_c.rs` now binds the argument to a typed temporary, discards it
  with `(void)`, and yields the static type name, evaluating exactly once.
- Tests: e2e `type_evaluates_its_argument`, native `type_evaluates_arguments`,
  and fuzzer cases in `functions_program` and `error_program`.
- Specification §6 and §11 document the rule.

## T-079 — Deterministic `print` evaluation and float `min`/`max` semantics

- Status: VERIFIED locally; commit `33c966e`; CI run 45920832199 green
- Priority: P1 (interpreter/native correctness)
- Both engines evaluate all `print` arguments, render them, then write one
  line; float `min`/`max` use one explicit rule (single NaN yields the other
  operand; ties return the second operand).
- Tests: e2e `float_min_max_ties_and_nan_are_stable` and
  `print_evaluates_every_argument_before_writing`; native
  `float_min_max_ties_match` and
  `print_evaluates_arguments_before_writing`; fuzzer numeric/error cases.
- ADR-0039 records the rationale.

## T-078 — Strict numeric string conversion in the native runtime

- Status: VERIFIED locally; commit `33c966e`; CI run 45920832199 green
- Priority: P1 (interpreter/native correctness)
- `int(text)`/`float(text)` trim Unicode whitespace and validate the exact
  interpreter grammar before calling `strtoll`/`strtod`; float `ERANGE` is
  ignored so overflow/underflow match Rust.
- The 28-form probe found 8 divergences before and 0 after.

## Recently completed

- T-077 differential-fuzzer control-flow coverage and bounded shrinking:
  committed in `1aba7ba`; CI run 45507195701 passed.
- T-076 rustfmt baseline and CI formatting gate: committed in `de0bac7`; CI
  run 45504177763 passed.
- T-075 result-enum propagation: committed in `66b4efc`; CI run 45503321172
  passed with 621 tests and the ninth fuzzer shape.
- T-072 fuzzer expansion: committed in `53a8b82`; CI run 45499019615 passed.
- T-073/T-074 linter and reachability-aware returns: committed in `5e1092f`;
  CI run 45496041422 passed with 613 tests.
- T-070/T-071 native condition and short-circuit fixes: committed in
  `a4fe86b`; CI run 45493833079 passed with 584 tests.
- T-058–T-069 are committed and CI-green except where noted in
  `COMPLETED_TASKS.md`.
