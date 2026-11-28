# Recovery

## Resume procedure

1. Read `AGENTS.md`, `TASKS.md`, `PROJECT_STATUS.md`, and all `AGENT_STATE/`
   files.
2. Run `git status --short` and `git log -5 --oneline`.
3. Compare the working tree with `AGENT_STATE/ACTIVE_TASK.md`; do not discard
   unexplained changes.
4. Run the targeted tests for the active task before the full gate.
5. Continue from the last verified checkpoint in
   `AGENT_STATE/CHECKPOINTS/`.

## Last recorded repository state (2026-11-28)

- T-127 makes the e2e and native suites hermetic against cleanup or exhaustion
  of the process temp directory. Both test files add a `OnceLock`-backed
  `test_root()` that prefers `VANTYX_TEST_TMPDIR`, then Cargo's
  `CARGO_TARGET_TMPDIR`, then the process temp directory only as a
  manual-compilation fallback; workspaces live under
  `test_root()/vantyx-tests-<pid>`, and every spawned `vantyx` command receives
  `TMPDIR`, `TMP`, and `TEMP` pointing at that root so clang temporaries stay
  inside it. The built e2e binary passed 137/137 and the native binary 165/165
  when started with `TMPDIR=/proc TMP=/proc TEMP=/proc`. QA-F004 is fixed; the
  789-test count is unchanged. T-123–T-127 remain uncommitted; the next command
  is `git status --short` and `git diff --stat`, then the owner's commit
  decision.
- T-126 keys runtime conformance and contract strengthening by
  `(container identity, expected Type)` so a self-referential list or map cannot
  satisfy a mismatched recursive type merely by being revisited. `xs = [xs]`
  still conforms to `List[List[Any]]` but no longer to `List[List[Int]]`; the
  annotated binding raises `R0005` instead of letting a statically `Int`
  expression evaluate to a list. Three library tests and one e2e test cover
  self-cycles, mutual cycles, maps, and the binding failure; ADR-0057,
  specification §2.3, and `docs/type_system.md` record the rule. Interpreter-only;
  the native backend still rejects the dynamic containers with `E0401`.
- T-125 accepts one leading UTF-8 BOM in entry files, imported modules, and
  standard input via `src/lib.rs::strip_utf8_bom`, applied in
  `src/module.rs` (entry and imported modules) and `src/main.rs::read_source`
  (standard input). A BOM may be combined with
  CRLF; an interior U+FEFF is still `E0001`. Specification §1.1, README, and
  `docs/security.md` document the rule; one e2e test covers entry, module,
  stdin, BOM+CRLF, and interior rejection.
- T-124 makes `vantyx build -o` refuse an output path that resolves to the entry
  file or any loaded module, including relative spellings, symbolic links, and
  hard links, exiting `64` before code generation with every source byte
  unchanged. `src/main.rs` records canonical `source_paths` and checks the
  output before emitting C; `src/fs_safety.rs` adds
  `paths_refer_to_same_file_paths` sharing the T-089 device/inode rule. One
  native test covers the entry, module, and symlink cases; specification §11,
  README, and `docs/security.md` document the guard. The defect was QA-F001.
- The T-126/T-125/T-124/T-123 gate passed 789 tests (425 library unit + 15 CLI
  unit + 47 AI integration + 137 e2e + 165 native), GCC and sanitized native at
  165 each, 9 fuzzer self-tests, 100/100 differential fuzz on seed 1, all five
  examples, fuzz smoke, 13 skills, and 132 links in 207 Markdown files; clippy,
  fmt, debug, and release builds were clean. These four tasks remain
  uncommitted; the next command is `git status --short` and `git diff --stat`,
  then the owner's commit decision.
- T-123 adds permanent runtime element, key, and value contracts to interpreter
  lists and maps so a `List[Int]` or `Map[K,V]` cannot be polluted through an
  `Any`, `List[Any]`, or `Map[Any, Any]` alias. `src/value.rs` stores the
  contracts and adds `Value::tighten`, which strengthens only `Any` contracts
  recursively through nested containers, structs, and enum payloads with a
  visited set; `src/interp.rs` tags literals from `model.expr_types`, calls
  `tighten` after every successful validation, and validates `push`, list index
  assignment, and map key/value assignment before mutating with `R0005`.
  Fully dynamic `List[Any]`/`Map[Any, Any]` stay mutable; the native backend
  already rejects them with `E0401`, so no native code changed. Three e2e,
  three library, and one native test cover the guarantee. ADR-0057,
  specification §2.3, `docs/type_system.md`, `docs/memory_model.md`, and
  `docs/PROJECT_BASELINE.md` record the design. The full gate passed 783 tests
  (422 library unit + 15 CLI unit + 47 AI integration + 135 e2e + 164 native),
  GCC and sanitized native at 164 each, 100/100 differential fuzz on seed 1,
  examples, fuzz smoke, 13 skills, and 132 links in 207 Markdown files.
  T-123 is uncommitted; the next command is `git diff --stat` and the owner's
  commit decision.
- T-122 adds conservative ownership facts over typed IR with no codegen change
  and is committed as `6e58b37`; the 776-test gate passed. T-118, T-119, and
  T-120 are committed as `d6d7b77`; T-121 is verified
  locally on 2026-11-27 with the full 770-test gate and is committed as
  `75fcc34`. T-118 extends `L0003` to all-terminating `if`/`elif`/`else` and
  `match` statements with nine new unit tests and updated lint documentation.
  T-119 adds a bounded sanitized differential-fuzz step to the CI `sanitizers`
  job; the exact local command agreed 20/20. T-120 adds ADR-0056, a staged
  native memory reclamation design with no runtime change. T-121 reclaims
  top-level `print` renderings for statically owned types; nine alternating
  runs measured median peak RSS 19,268 KiB → 8,296 KiB with identical output.
  The T-121 gate passed 770 tests (413 library unit + 15 CLI unit + 47 AI
  integration + 132 e2e + 163 native), GCC and sanitized native at 163 each,
  100/100 ordinary differential fuzz on seed 1, sanitized fuzz 20/20, examples,
  fuzz smoke, 13 skills, and 130 links in 201 Markdown files; clippy, fmt,
  debug, and release builds were clean. The earlier T-118/T-119/T-120 gate
  passed 768 tests with native at 161 each. The later four-child review retry
  also failed with `Free usage limit reached`, so no independent subagent
  result exists; the primary session's executed tests are the only evidence.
- T-117 is verified locally on 2026-11-27 with the 759-test gate and committed
  as `4108974`; CI pending. `reverse(list: List[T]) -> List[T]` returns a new shallow-copied
  list with the elements in reverse order for any supported element type and
  leaves the input unchanged. The checker returns the same `List[T]` element
  type and reports non-lists as `E0301` and wrong arity as `E0302`; the
  interpreter clones the elements in reverse order; the IR requires one
  statically typed list; and the native backend adds `vantyx_list_reversed`,
  which allocates a new list and copies raw element slots in reverse order.
  Three e2e tests and two native tests cover `Int`, `Str`, `Bool`, `Float`,
  nested lists, structs, enums, empty and one-element lists, input independence,
  argument evaluation order, `E0301`/`E0302`, `R0005`, and the native `E0401`
  boundary; the collection fuzz shape prints `reverse` results for
  `List[Int]`, `List[Str]`, and `List[Item]`. The gate passed: 759 tests (404
  library unit + 15 CLI unit + 47 AI integration + 132 e2e + 161 native), GCC
  native 161, sanitized native 161, examples, 9 fuzz self-tests, 100/100
  differential cases on seeds 1 and 2, fuzz smoke, 13 skills, and 130 links in
  199 Markdown files. `reverse([])` remains `List[Any]` and is rejected
  natively with `E0401`.
- T-116 is verified locally with the 754-test gate and committed as `3e22628`;
  CI pending. `write_file_atomic(path, text) -> Unit` writes a hidden
  same-directory temporary with exclusive creation, writes and fsyncs it,
  renames it over the target, and removes the temporary on failure. Readers see
  the old file or the complete new contents, never a partial file. Symbolic
  links are replaced rather than followed; hard links keep the old contents;
  mode, owner, and timestamps are not preserved; the directory is not fsynced,
  so this is atomic replacement for readers, not crash durability. Six e2e and
  five native tests cover creation/replacement, empty contents, nested paths,
  symbolic and dangling links, hard links, read-only targets, missing parents,
  directory targets, empty paths, evaluation order, `E0301`/`E0302`, `R0005`,
  and the native `E0401` boundary. ADR-0055 records the design. The gate
  passed: 754 tests (404 library unit + 15 CLI unit + 47 AI integration + 129
  e2e + 159 native), GCC native 159, sanitized native 159, examples, 9 fuzz
  self-tests, 100/100 differential cases on seeds 1 and 2, fuzz smoke, 13
  skills, and 130 links in 197 Markdown files.
- T-115 is verified locally and committed as `b6f3f11` with the 743-test
  gate; CI pending.
  `remove(map, key) -> Bool` deletes the first structurally equal entry,
  preserves insertion order, and returns whether an entry was removed; missing
  keys return `false`. The native backend binds the map and key in source
  order, compacts the entry buffers, rebuilds the scalar-key hash index, and
  snapshots keys for `for k in map` so removal during iteration does not skip
  entries. Five e2e tests and two native tests cover scalar/structural/list
  keys, aliases, missing keys, evaluation order, empty-list key adoption,
  removal during iteration, `R0005`, `E0301`/`E0302`, and the native `E0401`
  boundary; the control-flow fuzz shape removes each visited key. The gate
  passed: 743 tests (404 library unit + 15 CLI unit + 47 AI integration + 123
  e2e + 154 native), GCC native 154, sanitized native 154, examples, 9 fuzz
  self-tests, 100/100 differential cases on seeds 1 and 2, fuzz smoke, 13
  skills, and 129 links in 196 Markdown files.
- T-114 is committed as `3617fe3` and verified locally; CI pending.
  `src/codegen_c.rs` adds `vantyx_str_from_bool_owned`,
  `vantyx_str_from_unit_owned`, and `vantyx_display_free`; list, map, struct,
  and enum display bind each rendered element, key, value, and field, append
  it, and free it; top-level `print`/interpolation keep the static helpers and
  final results stay caller-owned. Two native tests were added. The 736-test
  gate passed (404 library unit + 15 CLI unit + 47 AI integration + 118 e2e +
  152 native), as did GCC native 152, sanitized native 152, examples, 9 fuzz
  self-tests, 100/100 seed-1 differential cases, fuzz smoke, 13 skills, and the
  link checker. Focused measurement: 100k Int median peak RSS 6,552 → 3,616
  KiB and 50k Pair(Bool, Bool) 9,368 → 5,580 KiB. ADR-0054 is accepted.
- T-113 is committed as `d8a476d` with CI pending; its status sync is
  `fc453cf`. `tools/differential_fuzz.py` adds
  a twelfth `display_program` shape covering a three-node struct/list cycle, a
  self-referential node, a recursive `Chain` enum of depth 0–60,
  `Map[Key, Int]` with struct keys, `Map[List[Int], Str]`, `Map[Shape, Int]`,
  nested empty collections, quoted UTF-8 strings, and a list of cyclic nodes.
  `tools/test_differential_fuzz.py` adds determinism/well-formedness checks and
  now passes 9 tests. The twelve-shape fuzzer agreed 100/100 on seeds 1, 2, 3,
  and 500; seven generated programs built and matched manually. The full gate
  passed: fmt/clippy clean, 734 tests, GCC native 150, sanitized native 150,
  examples, fuzz smoke, 13 skills, and 125 links in 192 Markdown files. No
  Rust, language, or native behavior changed.
- T-112 is committed as `8911235` and adds single-buffer native collection and
  nominal display: list, map, struct, and enum rendering append every part into
  one growable `VantyxStrBuilder` in source order, removing quadratic prefix
  copying and retained prefixes while preserving exact bytes, ordering,
  quoting, cycle markers, depth limits, and early returns. One differential
  native test, ADR-0053, a new `display_ops` benchmark workload, and a focused
  same-batch result file are added. It passes 734 tests (404 library unit + 15
  CLI unit + 47 AI integration + 118 e2e + 150 native), the full 150-test
  native suite under `VANTYX_CC=gcc` and under clang ASan/UBSan with leak
  detection disabled, clippy and fmt clean, the release build, five examples,
  seven Python self-tests, 100/100 seed-1 differential cases, fuzz smoke, the
  13-skill validator, and the link checker. CI pending.
  Focused median 754.707 ms → 5.737 ms and peak RSS 568,244 KiB → 2,520 KiB.
- The T-111 working tree adds single-buffer native string construction:
  interpolation and multi-operand string `+` chains append typed parts into
  one growable `VantyxStrBuilder` in source order, flatten nested
  interpolations, and transfer the final buffer to the result. Two plain
  operands and one-part interpolation keep the old path. Two differential
  native tests, ADR-0052, a new `string_interp` benchmark workload, and a
  focused same-batch result file are added. It passes 733 tests (404 library
  unit + 15 CLI unit + 47 AI integration + 118 e2e + 149 native), the full
  149-test native suite under `VANTYX_CC=gcc` and under clang ASan/UBSan with
  leak detection disabled, clippy and fmt clean, the release build, five
  examples, seven Python self-tests, 100/100 seed-1 differential cases, fuzz
  smoke, the 13-skill validator, and the link checker (122 links in 188
  Markdown files). Committed as `45c438b`; CI pending. Focused median
  59.893 ms → 34.414 ms and peak RSS 33,396 KiB → 17,564 KiB.
- HEAD is at `1e406cf` (T-114 status sync); T-115 changes are in the working
  tree. The remote is at `a9a6d1a` (record the T-105 CI run), and CI
  run 45936329476 passed on it. T-112 is committed with CI pending. `779783c`
  contains T-105 and passed run 45936065460. `61cb9be` recorded the T-104 status-sync CI run and passed
  45934367601. `8805381` recorded the T-104 CI run and passed 45934076343. `4269010` contains call-site native
  call-depth diagnostics (T-104), and CI run 45933776137 passed on it.
  `a263cb3` recorded the T-103 CI run and passed run 45928756101. `46b7d54` contains deterministic scalar-list
  sorting (T-103), and CI run 45928466694 passed on it. `0a309c8` recorded the T-102 CI
  run and passed run 45926037169. `a331f2d`
  contains label-stratified JSONL dataset splits (T-102), and CI run
  45925745650 passed on it. `5d5655b` recorded the T-101 CI run, and run
  45923860569 passed on it. `c54c61c` contains deterministic JSONL dataset
  shuffling (T-101), and `33c966e` contains the native parity fixes, dataset
  tooling, and agent skills (T-078 through T-100); CI runs 45923211883 and
  45920832199 passed.
- The T-110 tree adds CI verification for native memory and undefined
  behavior. `.github/workflows/ci.yml` adds a `sanitizers` job that installs
  clang and runs `cargo test --locked --test native` with `VANTYX_CC` pointing
  at `tools/clang-sanitize.sh` (ASan + UBSan, `-fno-sanitize-recover=all`,
  leak detection disabled because v0.1 leaks native allocations intentionally)
  and adds a `Fuzz smoke` step to the main test job. The exact sanitizer
  command passed all 147 native tests locally, and the fuzz smoke produced
  controlled diagnostics for 9 depth/limit cases and 400 random/mutated
  inputs. Committed as `02cdf10`; CI pending.
- The T-109 tree fixes the module-loader file-count off-by-one:
  `Loader::unit` rejected the 256th file because it checked
  `self.units.len() + 1 >= MAX_MODULES` while the entry file was already
  loaded. The check is now `self.units.len() >= MAX_MODULES`; an entry plus
  255 imports loads with 256 flattened declarations, and a 257-file graph
  produces exactly one `E0507`. Two unit tests cover both boundaries and the
  exact-limit test fails against the old condition. It passes 731 tests (404
  library unit + 15 CLI unit + 47 AI integration + 118 e2e + 147 native),
  clippy and fmt clean, the release build, five examples, seven Python
  self-tests, 100/100 seed-1 differential cases, fuzz smoke, the 13-skill
  validator, and the link checker. Committed as `bffe38f`; CI pending.
- The T-108 tree makes generated native `main` replace a closed
  standard descriptor 0, 1, or 2 with `/dev/null` before user code, matching
  Rust's startup sanitization. With stdout closed, both engines now discard
  `print` output, reach `assert(false)` with the same `R0006` headline and
  exit `2`, and a `write_file` marker survives. It passes 729 tests (402
  library unit + 15 CLI unit + 47 AI integration + 118 e2e + 147 native), the
  full 147-test native suite under `VANTYX_CC=gcc`, clippy and fmt clean, the
  release build, five examples, seven Python self-tests, 100/100 seed-1
  differential cases, fuzz smoke, the 13-skill validator, and the link
  checker. Committed as `2104c55`; CI pending.
- The T-107 tree makes native `print` flush each completed line, check
  `fputc`/`fflush`/`ferror`, and report `R0013` with `strerror` and
  `(os error N)` at the print, and makes `main` ignore SIGPIPE. Linux
  `/dev/full` and a closed pipe now match the interpreter's first error line
  and exit code `2` instead of a generic flush error or signal termination.
  It passes 728 tests (402 library unit + 15 CLI unit + 47 AI integration +
  118 e2e + 146 native), the full 146-test native suite under
  `VANTYX_CC=gcc`, clippy and fmt clean, the release build, five examples,
  seven Python self-tests, 100/100 seed-1 differential cases, fuzz smoke, the
  13-skill validator, and the link checker (117 links in 180 Markdown files).
  Committed as `9bb2353`; CI pending.
- The T-105 tree makes a direct empty list literal in `join` take
  `List[Str]`, so `join([], ",")` compiles natively instead of failing with
  `E0401`. It passes 724 tests (402 library unit + 15 CLI unit + 47 AI
  integration + 117 e2e + 143 native), the full native suite under
  `VANTYX_CC=gcc`, clippy and fmt clean, release build, five examples, seven
  Python self-tests, 100/100 fuzz cases on seed 1, fuzz smoke, and the
  thirteen-skill and 117-link validators; committed as `779783c` with CI run
  45936065460 green. `len([])`,
  `print([])`, and unannotated empty-list bindings still infer `List[Any]` and
  remain under the native `E0401` boundary.
- The T-106 working tree makes a direct empty list literal in `contains` or
  `push` take the value's element type, so `contains([], 1)` and
  `push([], 1)` build natively instead of failing with `E0401`. It passes 726
  tests (402 library unit + 15 CLI unit + 47 AI integration + 118 e2e + 144
  native), the full 144-test native suite under `VANTYX_CC=gcc`, clippy and
  fmt clean, the release build, and five examples; seven Python self-tests,
  100/100 seed-1 differential cases, fuzz smoke, the 13-skill validator, and
  the link checker (117 links in 178 Markdown files) passed. Commit and CI are
  pending. Non-empty arguments keep `E0301`; dynamic `Any` values and
  still-untyped empty literals remain under the native `E0401` boundary.
- The T-104 tree moves the native `R0012` depth-limit diagnostic to the
  failing call site, including across modules. It passed 723 tests
  (402 library unit + 15 CLI unit + 47 AI integration + 117 e2e + 142 native),
  the full native suite under `VANTYX_CC=gcc`, clippy and fmt clean, debug and
  release builds, five examples, seven Python self-tests, 100/100 fuzz cases on
  seed 1, fuzz smoke, and the thirteen-skill validator.
- The T-103 tree adds deterministic scalar-list sorting and passes the same
  723-test gate. ADR-0050 records the decision.
- Earlier commits: `1aba7ba` contains T-077 differential-fuzzer control-flow
  coverage and bounded shrinking (run 45507195701 green), `de0bac7` contains
  the rustfmt baseline and CI formatting gate (run 45504177763 green),
  `66b4efc` contains T-075 result-enum propagation (run 45503321172 green),
  `53a8b82` contains the expanded differential fuzzer (run 45499019615 green),
  `5e1092f` contains the T-073–T-074 linter and return-path fix (run
  45496041422 green), `a4fe86b` contains the T-070–T-072 native fixes plus the
  original fuzzer (run 45493833079 green), and `4982ab3` contains the glob
  result-cap portability fix (run 45488725812 green).
- The committed tree is verified: 707 Rust tests (395 library unit + 15 CLI
  unit + 43 AI integration + 113 e2e + 141 native), the full 141-test native
  suite also under `VANTYX_CC=gcc`, clippy clean, `cargo fmt --check --all`
  clean, debug and release builds, five examples, seven Python self-tests,
  fuzzer 500/500 on seed 7 and 100/100 on seed 1, the skill validator, and
  fuzz smoke clean. T-099 added native `range(...)` values and T-100 added
  exact dataset deduplication; ADR-0047 records the dedup decision.
- The committed tree at `c54c61c` passes 713 tests (399 library unit + 15 CLI
  unit + 45 AI integration + 113 e2e + 141 native), clippy and fmt clean, debug
  and release builds, five examples, seven Python self-tests, 100/100 fuzz
  cases on seed 1, and the thirteen-skill validator. T-101 adds deterministic
  seeded shuffling; ADR-0048 records the decision.
- The T-102 working tree passes 718 tests (402 library unit + 15 CLI unit + 47
  AI integration + 113 e2e + 141 native), clippy and fmt clean, debug and
  release builds, five examples, seven Python self-tests, 100/100 fuzz cases on
  seed 1, a supplemental 500/500 fuzz audit on seed 11, and the thirteen-skill
  validator. T-102 adds label-stratified splits; ADR-0049 records the decision.
- If a checkout is missing `33c966e`, fetch `origin/main` and inspect
  `git status --short`; do not discard unexplained work or rewrite history.

## If the network or a provider is unavailable

1. Keep all local work; do not reset the tree.
2. Record the failure and the last successful command in this file.
3. Continue offline tasks (code, tests, docs, benchmarks with offline providers).
4. Retry external services with exponential backoff and jitter (15 s, 30 s,
   60 s, 120 s, 240 s, then capped); honor `Retry-After`.
5. Never repeat completed operations or hammer a rate-limited service.
6. Leave a resumable note in `ACTIVE_TASK.md` if the session ends.

Last recorded external state (2026-11-24): the Gemini OpenAI-compatible cloud
run succeeded and is documented in `docs/ai/LIVE_VERIFICATION.md`; an earlier
retry returned HTTP 400 `User location is not supported` for both available
credentials, a later re-run hit a transient HTTP 503, and a 20:59Z
re-verification using `MY_API_KEY` passed all 21 checks. Access can change and
is not guaranteed. No local inference server/model is installed. Do not retry
indefinitely or attempt to bypass provider restrictions.

## If a build or test is slow

- Native integration tests build and link C binaries; the full AI test binary
  can take several minutes on the 4-core reference machine. Prefer a targeted
  `cargo test --test ai <name>` while iterating, then run the full gate once.
- Do not poll background commands; wait for the completion notification.

## If history or attribution looks wrong

- `v0.1.0` was moved to the rewritten-history commit; old hashes in earlier
  status entries are historical.
- `.mailmap` maps old placeholder identities. Do not credit any contributor
  without verified work.
- Do not rewrite history again without explicit authorization.
