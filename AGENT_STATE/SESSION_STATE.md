# Session State

- Session ID: `ses_df3f90e90ffeFHPWt10GTzpwye`
- Updated: 2026-11-26
- Phase: Phase 6 native backend plus AI host Track B
- Branch: `main`
- Last commit: `1aba7ba` (differential-fuzzer control-flow coverage and bounded
  shrinking); T-078 through T-100 are pending in the working tree
- Remote: `origin/main` at `1aba7ba`
- Tag: `v0.1.0` at the rewritten-history commit

## Current task

T-100 — Exact JSONL duplicate-record removal. Status: VERIFIED locally; commit,
push, and CI pending. `vantyx ai dedup <file> --out <file>` runs the dataset
validation gate first, keeps the first occurrence of each exact normalized
record in input order, digests record bytes with a domain-separated SHA-256,
refuses to overwrite or replace the input, removes the output on any failure,
caps the in-memory digest set at 1,000,000 distinct records, and reports
input/output provenance. Four unit tests and two CLI tests; 707-test gate. See
`ACTIVE_TASK.md` and the `2026-11-26-t100-dataset-dedup.md` checkpoint.

T-099 — Native `range(...)` list values outside `for` loops. Status: VERIFIED
locally; commit, push, and CI pending. The interpreter already returned a
`List[Int]` in any expression position, while native rejected `range(...)`
outside `for` with `E0401`. The IR now lowers 1–3 `Int` bounds to
`List[Int]`, and codegen binds the bounds in source order and calls a new
`vantyx_range_list` helper that reuses the shared range count/limit checks.
`range_values_match` plus two value-position error headlines cover the path;
701-test gate. See `ACTIVE_TASK.md` and the
`2026-11-26-t099-native-range-values.md` checkpoint.

T-098 — Deterministic train/held-out dataset splits. Status: VERIFIED locally;
commit, push, and CI pending. `vantyx ai split` runs the dataset validation gate
first, assigns each record by a seeded content hash independent of line order,
writes `<stem>.train.jsonl` and `<stem>.heldout.jsonl` with `create_new`,
removes files from failed runs, guarantees non-empty splits for datasets with at
least two records, and reports actual counts and output SHA-256 values. Five
unit tests and two CLI tests; 700-test gate. See `ACTIVE_TASK.md` and the
`2026-11-26-t098-dataset-splits.md` checkpoint.

T-097 — Filesystem OS-error suffix and glob parse diagnostics. Status: VERIFIED
locally; commit, push, and CI pending. Native filesystem failures now append
Rust's `(os error N)` suffix, and malformed glob patterns use the shared
parser's `invalid glob pattern: ...` wording with quoted range endpoints.
`runtime_error_headlines_match_interpreter` gained 11 cases and `diff3.py` went
from 10 mismatches to 44/44. See `ACTIVE_TASK.md` and the
`2026-11-26-t097-filesystem-os-errors.md` checkpoint.

T-096 — Runtime diagnostic parity. Status: VERIFIED locally; commit, push, and
CI pending. `int(Float)` now renders the offending value through the shared
Vantyx float display in both engines (`.0` for integral values,
`Infinity`/`-Infinity`/`NaN`); a missing map key renders `Str` keys without
quotes in both engines; `range` reports the requested element count; and
`read_file`, `list_dir`, and `glob` name the path or directory in NUL, UTF-8,
and 64 MiB failures. Two differential tests compare the first `error[...]`
line for nine error programs and four Unix-only file fixtures; both failed
before the fixes. See `ACTIVE_TASK.md` and the
`2026-11-26-t096-runtime-diagnostic-parity.md` checkpoint.

T-095 — JSON Schema conformance and shared preflight. Status: VERIFIED
locally; commit, push, and CI pending. The shared subset now ignores
`required` for non-objects, rejects empty `enum`/`allOf` during preflight,
evaluates `anyOf` and sibling keywords cumulatively, uses JSON Schema instance
equality for `const`/`enum`, compares `minimum`/`maximum` exactly beyond
`2^53`, and preflights schemas in `extract` and `ToolRegistry::call`. Thirteen
`ai::schema` tests, one extract integration test, one tools unit test, and
ADR-0045. See `ACTIVE_TASK.md`.

T-094 — Dataset schema validation. Status: VERIFIED locally; commit, push, and
CI pending. `vantyx ai dataset --schema <file>` preflights a bounded schema
and validates every structurally valid record, reports the first 20 violations
plus the total, adds an optional `schema` summary, and requires zero
schema-invalid records for exit `0`. Ten dataset unit tests, three dataset CLI
tests, and ADR-0044. See `ACTIVE_TASK.md` and
`docs/ai/DATASET_INSPECTION.md`.

T-093 — Reusable agent skill library. Status: VERIFIED locally; commit, push,
and CI pending. Twelve project-local OpenCode skills live under
`.opencode/skills/` with unique kebab-case IDs and `name`/`description`
frontmatter; each contains a concrete Vantyx workflow, commands, safety
constraints, acceptance criteria, troubleshooting, and an example.
`docs/AGENT_SKILLS.md` records discovery, inventory, selection, and
maintenance. The active harness advertised all twelve after creation,
confirming the mechanism. See `ACTIVE_TASK.md` and `docs/AGENT_SKILLS.md`.

T-092 — Bounded JSONL dataset inspection. Status: VERIFIED locally; commit,
push, and CI pending. `vantyx ai dataset <file>` streams UTF-8 JSONL through
SHA-256, accepts a first-line BOM, CRLF, and blank lines, requires one
top-level JSON object per non-blank line, and reports size, line/blank/record/
invalid counts, sorted field/type histograms, and up to 20 numbered invalid
lines. Limits are 256 MiB per file, 16 MiB per line, and 10,000 distinct
top-level fields. Invalid content or I/O exits `1`; usage exits `64`. It never
downloads, executes, or modifies content. See `ACTIVE_TASK.md`, ADR-0043, and
`docs/ai/DATASET_INSPECTION.md`.

T-091 — Safetensors shard-index aggregation. Status: VERIFIED locally; commit,
push, and CI pending. `vantyx ai inspect <model.safetensors.index.json>`
recognizes a shard index by JSON object content or `.json` path, parses a
bounded `weight_map` and optional metadata (64 MiB index cap, 10,000-shard cap,
1,000,000-tensor cap), validates every referenced shard with the T-090
Safetensors path, and cross-checks index tensor names against every shard
header. Shard paths must be relative, use `/`, avoid `..`, end in
`.safetensors`, and stay under the canonical index directory after symlink
resolution. It aggregates tensor/parameter/data-byte counts and dtypes, reports
the index digest plus per-shard digests, and warns when `metadata.total_size`
disagrees. It never downloads, loads, modifies, or executes model code. See
`ACTIVE_TASK.md`, ADR-0042, and `docs/ai/MODEL_INSPECTION.md`.

T-090 — Offline GGUF and Safetensors model inspection. Status: VERIFIED
locally; commit, push, and CI pending. `vantyx ai inspect <file>` parses bounded
GGUF v2/v3 and Safetensors metadata and tensor directories, validates tensor
ranges, overlap, dtype sizes, alignment, and known GGML block sizes, and streams
the whole file through an in-tree SHA-256. It reports format, size,
tensor/parameter counts, architecture/quantization/tokenizer metadata, dtype
histogram, warnings, and supports `--json` and `--metadata`. Unsupported or
corrupt files are `AI_E_FORMAT`; I/O failures are `AI_E_IO`. It never downloads,
loads, modifies, or executes model code. See `ACTIVE_TASK.md`, ADR-0041, and
`docs/ai/MODEL_INSPECTION.md`.

T-089 — Same-file `copy_file` refusal. Status: VERIFIED locally; commit, push,
and CI pending. `copy_file(path, path)` opened the target for writing before
reading the source, so it truncated the source; symbolic-link and hard-link
aliases did the same. `src/fs_safety.rs::paths_refer_to_same_file` now compares
device and inode on Unix (canonical paths elsewhere), and both the interpreter
and the native `vantyx_copy_file` helper raise `R0015` `source and target are
the same file` before the target is opened. The guard is best-effort and
raceable, not atomic. See `ACTIVE_TASK.md` and ADR-0029.

T-088 — Source-order evaluation of named constructor arguments. Status:
VERIFIED locally; commit, push, and CI pending. Named struct-constructor
arguments are stored in declaration order, and the native IR lowering reordered
them before code generation, so the C call evaluated them in declaration order.
`Pair(right=note("right", 2), left=note("left", 1))` printed `right`, `left` in
the interpreter and `left`, `right` natively. `IrExprKind::Construct` now
carries `eval_order`, and `src/codegen_c.rs::construct_expr` evaluates effectful
arguments into typed temporaries in source order before calling the constructor
in declaration order. See `ACTIVE_TASK.md`.

T-087 — Lexical function scope in the interpreter. Status: VERIFIED locally;
commit, push, and CI pending. The interpreter searched one flat scope stack
from newest to oldest, so a callee could read or mutate a caller local that
shared a global's name, and an immutable caller binding produced a false
`R0010`. The checker and native backend already resolved names lexically.
`Interpreter` now records `frame_base`, the first scope of the active call;
`lookup` and `find_scope` search that frame and then root scope zero, and the
boundary is restored on return and error. See `ACTIVE_TASK.md`.

T-086 — Trailing-dot path stem and extension parity. Status: VERIFIED locally;
commit, push, and CI pending. Native `path_stem` and `path_extension` used an
earlier dot when the final dot was trailing, while the interpreter returned
the whole name and an empty extension. The C helpers now share one final-dot
search that matches the oracle, and differential plus random-path probes cover
the regression.

T-085 — Lint rule for unused function parameters. Status: VERIFIED locally;
commit, push, and CI pending. `vantyx lint` skipped parameters, so a dead
parameter produced no finding; `L0004` now reports it distinctly from `L0001`
while underscore names, guards, nested reads, loop variables, and match
bindings keep their existing treatment.

T-084 — native `bool` argument evaluation, T-083 — native `Unit` interpolation
evaluation, T-082 — native `Unit` equality operand evaluation, T-081 — string
`contains` argument order, T-080 — `type` argument evaluation, T-079 —
deterministic `print` and float `min`/`max`, and T-078 — strict numeric string
conversion — remain VERIFIED locally and uncommitted in the same working tree.

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
- T-069 destructive recursive-removal refusal: `e6ec597`/`4982ab3`; CI run
  45488725812 passed with 581 tests.
- T-058–T-068 are committed and CI-green except where noted in
  `COMPLETED_TASKS.md`.

## Working tree

Modified: `AGENTS.md`, `PROJECT_STATUS.md`, `TASKS.md`,
`docs/language_specification.md`, `docs/linting.md`,
`docs/decisions/ADR-0029-rename-and-copy.md`,
`docs/decisions/ADR-0030-lexical-path-builtins.md`,
`docs/decisions/ADR-0037-static-lints.md`, `docs/PROJECT_BASELINE.md`,
`docs/ai/AI_ARCHITECTURE.md`, `docs/ai/AI_ROADMAP.md`,
`RESEARCH/INITIAL_REPOSITORY_AUDIT.md`,
`RESEARCH/HISTORICAL_REQUIREMENTS_AUDIT.md`, `src/ai/mod.rs`,
`src/ai/provider.rs`, `src/ai/json.rs`, `src/ai/schema.rs`,
`src/ai/extract.rs`, `src/ai/tools.rs`, `src/ai_cli.rs`, `src/codegen_c.rs`,
`src/fs_safety.rs`, `src/interp.rs`, `src/ir.rs`, `src/lint.rs`,
`tests/ai.rs`, `tests/e2e.rs`, `tests/native.rs`,
`tools/differential_fuzz.py`, `README.md`, `examples/ai/README.md`,
`RESEARCH/TOOL_INVENTORY.md`, and the `AGENT_STATE/` records. New:
`src/ai/model.rs`, `src/ai/dataset.rs`,
`docs/ai/MODEL_INSPECTION.md`, `docs/ai/DATASET_INSPECTION.md`,
`docs/AGENT_SKILLS.md`,
`docs/decisions/ADR-0039-deterministic-print-and-float-min-max.md`,
`docs/decisions/ADR-0040-lexical-function-scope.md`,
`docs/decisions/ADR-0041-offline-model-inspection.md`,
`docs/decisions/ADR-0042-safetensors-shard-index-aggregation.md`,
`docs/decisions/ADR-0043-bounded-jsonl-dataset-inspection.md`,
`docs/decisions/ADR-0044-bounded-dataset-schema-validation.md`,
`docs/decisions/ADR-0045-json-schema-conformance-and-preflight.md`,
`docs/decisions/ADR-0046-deterministic-dataset-splits.md`,
`docs/decisions/ADR-0047-exact-dataset-deduplication.md`,
`docs/ai/DATASET_DEDUPLICATION.md`,
`docs/ai/DATASET_SPLITTING.md`,
`examples/ai/train.jsonl`, thirteen `.opencode/skills/*/SKILL.md` files, and the
T-078/T-079, T-080, T-081, T-082, T-083, T-084, T-085, T-086, T-087, T-088,
T-089, T-090, T-091, T-092, T-093, T-094, T-095, T-096, T-097, T-098, T-099,
and T-100 checkpoints. No generated
binaries or credentials are tracked.

## Verified commands (2026-11-26, after T-100/T-099/T-098/T-097/T-096/T-095/T-094/T-093/T-092)

- T-100 gate — `cargo test --lib ai::dataset` passed 19 and
  `cargo test --test ai dataset_dedup` passed 2; `cargo test` passed 707 (395
  library unit + 15 CLI unit + 43 AI integration + 113 e2e + 141 native);
  clippy and fmt clean; release build succeeded; fuzzer self-tests passed 7 and
  500/500 generated cases agreed on seed 7; all five examples built and ran
  natively. `/tmp/opencode/probe/diff6.py` matched 31/31. The repository skill
  validator passed all 13 project-local skills under `.opencode/skills/`.

- T-099 gate — `cargo test --test native range_values_match` passed 1 and
  `cargo test --test native runtime_error_headlines_match_interpreter` passed
  1; `cargo test` passed 701 (391 library unit + 15 CLI unit + 41 AI
  integration + 113 e2e + 141 native); `VANTYX_CC=gcc cargo test --test native`
  passed 141; clippy and fmt clean; release build succeeded; fuzzer self-tests
  passed 7 and 100/100 generated cases agreed; all five examples built and ran
  natively. `/tmp/opencode/probe/diff6.py` matched 29/31; the two mismatches
  were the pre-fix `range`-outside-`for` rejection and the known `List[Any]`
  display limitation.

- T-098 gate — `cargo test --lib ai::dataset` passed 15 and
  `cargo test --test ai dataset_split` passed 2; `cargo test` passed 700
  (391 library unit + 15 CLI unit + 41 AI integration + 113 e2e + 140 native);
  `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded; the fuzzer self-tests passed 7 and
  100/100 generated cases agreed; all five examples built and ran natively.
  Manual smoke confirmed 9/1 output at ratio 0.7 seed 7, overwrite refusal,
  seed-dependent digests, invalid-ratio exit `64`, one-record exit `1`, and
  invalid/schema-invalid data exiting `1` with no split files.

- T-097 follow-up probes — `/tmp/opencode/probe/diff4.py` matched 26/26
  (insertion-order iteration for Int/Str/Bool/Float/struct map keys, overwrite
  order, hidden glob entries, a module runtime error, and filesystem error
  headlines for `copy_dir_all`, `rename`, `remove_dir`, `remove_dir_all`,
  `make_dir_all`, `file_size`, `list_dir`, `read_file`, `write_file`,
  `append_file`, plus `/dev/full` write and append). `diff5.py` matched 16/16
  (float display extremes, `floor`/`ceil`/`sqrt` on NaN and infinities,
  int/float boundary conversions, float `min`/`max` NaN and signed-zero rules,
  nested float display, Unicode `len`/index/iteration/`split`/`join`/`replace`/
  `trim`, empty affixes, numeric-string edges, and `type`/`str`). No new
  divergences.

- T-097 probe — `/tmp/opencode/probe/diff3.py` reported 10 mismatches before
  the fix and 44/44 after it; the eight filesystem cases lacked `(os error N)`
  natively and the glob cases used different wording. The focused
  `cargo test --test native runtime_error_headlines_match_interpreter` passed;
  `cargo test --test native` and `VANTYX_CC=gcc cargo test --test native`
  passed 140 each; `diff.py` 55/55 and `diff2.py` 34/34 remained clean. An
  initial `vantyx_os_error` implementation called itself and segfaulted the
  native binary; the probe caught it before the regression test was added.

- T-096 probe — `cargo test --test native headlines` passed both new tests;
  before the fixes they failed on `cannot convert `2.5` to Int exactly` versus
  `cannot convert Float to Int exactly` and on `file `nul.bin` contains a NUL
  byte` versus `file contains a NUL byte`. `/tmp/opencode/probe/diff.py`
  matched 55/55 and `diff2.py` matched 34/34 after correcting unit-variant
  syntax and building the native binary outside the `glob("*")` working
  directory; five `Any` native-backend rejections are documented limitations.

- T-095 probe — `cargo test --lib ai::schema` passed 13 tests; the pre-fix
  behavior was reproduced for `1 != 1.0`, `allOf: []` accepting every value,
  a matching `anyOf` skipping sibling `minimum`, and adjacent 64-bit bounds
  comparing equal; the extract test proved the mock provider was not called,
  and the tools test proved the invalid-schema tool body was not entered
- T-094 probe — `cargo test --lib ai::dataset` passed 10 tests and
  `cargo test --test ai dataset_` passed 3; schema-invalid fixtures reported
  capped violations and exit `1`, while a valid dataset with a schema exited
  `0`; malformed and unreadable schemas failed before record validation
- T-092 probe — `cargo test --lib ai::dataset` passed 5 tests and
  `cargo test --test ai dataset_` passed 2; `vantyx ai dataset
  examples/ai/train.jsonl --json` reported 3 records and a 64-character digest;
  a fixture with invalid lines exited `1` while printing the summary; missing
  path, extra path, unknown option, absent file, and directory path produced
  the documented errors
- T-091 probe — `cargo test --lib ai::model` passed 13 tests and
  `cargo test --test ai inspect_` passed 3; a two-shard index with leading
  whitespace printed `format: safetensors-index`, `parameters: 4`, two shard
  rows, and a JSON object whose `shards` array carried both per-shard SHA-256
  digests; a missing shard exited `1` with `AI_E_IO`
- T-090 probe — `cargo test --lib ai::model` passed 9 tests and
  `cargo test --test ai inspect_` passed 2; minimal GGUF and Safetensors
  fixtures reported the expected format, parameter counts, quantization, and a
  64-character digest; an unsupported file exited `1` with `AI_E_FORMAT` and a
  help line; a missing path exited `1` with `AI_E_IO`; missing and extra
  positional arguments exited `64`
- T-089 probe — before the fix, `copy_file(path, path)` exited `0` and left a
  zero-byte file; after the fix both engines exited `2` with `R0015`, preserved
  the source bytes, and matched for direct, symbolic-link, and hard-link
  aliases
- T-088 probe — before the fix the interpreter printed `right`, `left`, `c`,
  `a`, `b` while both clang and GCC printed `left`, `right`, `a`, `b`, `c`;
  after the fix interpreter, clang, and GCC agreed byte for byte, including a
  failing earlier argument that exits `2` with empty stdout
- T-087 probe — before the fix the interpreter printed `100` for
  `read_total()` and then exited `2` with `R0010` in `bump_other()`, while
  native printed `10`, `11`, `100`, `21`, `caller-immutable`; after the fix
  interpreter and native stdout and exit codes were identical
- T-086 probe — a 30-path program diverged on `a/b..` and `a/...` before the
  fix; after the fix it printed no diff, and a 2,514-path probe (2,500 random
  plus 14 fixed) with `path_join`, `path_parent`, `path_name`, `path_stem`,
  `path_extension`, and `path_is_absolute` also printed no diff
- T-085 probe — `vantyx lint` printed one `warning[L0004]` for
  `fn constant(value: Int)` and exited `0`; the five examples linted clean
- T-084 probe — before the fix native `bool(side())` printed no `side` line;
  after the fix both clang and GCC printed it and exited `2` with `R0007`
- T-083 probe — before the fix native interpolation skipped the call's output;
  after the fix both engines matched under clang and GCC
- T-082 probe — before the fix native `Unit ==`/`!=` dropped a sole effectful
  operand; after the fix both operands ran left to right
- T-081 cross-compiler probe — pre-fix GCC reversed effectful string
  `contains` arguments while clang and the interpreter did not; post-fix both
  compilers matched the interpreter byte for byte
- `cargo build` — success
- `cargo build --release` — success
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check --all` — clean
- `cargo test` — 395 library unit + 15 CLI unit + 43 AI integration +
  113 e2e + 141 native = 707 passed
- `VANTYX_CC=gcc cargo test --test native` — 141 passed
- `python3 tools/test_differential_fuzz.py` — 7 passed
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` —
  100/100 agreement
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs produced controlled diagnostics
- All five `examples/` programs — interpreter and native output identical
- Differential audit probes with no observed divergence (interpreter vs
  clang and GCC): evaluation order across binary operators, short-circuit
  boolean expressions, call arguments, constructors, list/map literals,
  indexing, field/index assignment, compound assignment, builtins,
  interpolation, and `print`; map insertion/iteration/display order;
  integer boundaries, `//`/`%`, checked overflow, division/modulo by zero,
  `pow`, float inf/NaN, `sqrt`, `floor`/`ceil`, and numeric conversions;
  range extremes and caps in `for`-loop form; 554 random float
  formatting/arithmetic/math lines; string builtin edge cases; top-level
  declaration versus statement order; runtime-error parity with preceding
  stdout; 53 curated plus 300 random glob patterns; and control-flow
  evaluation order (iterables evaluated once, nested `break`/`continue`,
  `while` conditions, `if`/`elif`, and match scrutinee/guard order)
- Earlier audit probes with no observed divergence: a 500-string text probe
  (20 chunks) and a 400-string Unicode iteration/indexing probe (6,268
  output lines); float display extremes, float map keys/NaN/±0, structural
  list/struct map keys, mutated list keys, and 101-deep recursive enum
  display/equality
- T-080 probe — nested print, mutation, and `assert(false)` skipped by native
  before the fix; byte-identical stdout and exit `2` with empty stdout after
- T-079 probe — both divergences before; byte-identical stdout after; empty
  stdout on failing `print` calls in both engines
- T-078 28-form numeric-string probe — 8 divergences before, 0 after
- CI run 45507195701 — success on `1aba7ba` with T-077
- CI run 45504177763 — success on `de0bac7` with the rustfmt gate
- CI run 45503321172 — success on `66b4efc` with T-075

## Next steps

1. Review the complete working-tree diff; commit T-078 through T-100 together
   only if the user authorizes commits, then push, watch CI, and record the run
   ID.
2. Continue targeted interpreter/native probes. Remaining native surfaces
   include file/directory builtins under unusual conditions, recursive
   copy/rename, modules, and deeper match/nesting interactions; audit any
   lowering that can render a constant or omit an operand, and build effectful
   cases with both clang and GCC.
3. Extend offline dataset tooling with tokenizer-aware counts, shuffle, and
   label stratification; exact duplicate detection shipped in T-100 and
   schema/required-field validation shipped in T-094. Keep tokenization,
   shuffle, and stratification as separate bounded tasks with their own tests.
4. Add a lightweight skill check (frontmatter and required sections) and keep
   `docs/AGENT_SKILLS.md` synchronized when a skill is added or changed.
5. Continue remaining P1/P2 candidates: metadata objects/preservation,
   recursive move, REPL multi-line history and field completion, linter
   follow-ups, and string performance with measured benchmarks.
6. Retry independent subagent review when provider capacity returns.
7. T-040 remains blocked on a local inference server/model; cloud access is
   not guaranteed.
