# Decisions

Append-only record of engineering decisions made in autonomous sessions. Design
rationale that affects semantics belongs in `docs/decisions/` as an ADR; this
file records operational choices.

## 2026-11-27 — Native display uses the string builder

- Route list, map, struct, and enum display through the existing
  `VantyxStrBuilder` (ADR-0052) rather than adding a second buffer type or
  keeping repeated concatenation.
- Keep cycle/depth guards before builder creation and keep the existing
  `seen`/`vantyx_enter_struct` stacks active across nested display calls, so
  markers and early returns are byte-identical.
- Keep payload-free enum variants on the single static string path; there is
  nothing to append.
- Accept one temporary rendered string per element for now. Removing those
  requires typed display appenders and is deferred until measurement shows
  they dominate.
- Measure the change on a tracked `display_ops` workload with release binaries
  from before and after in one alternating batch; do not compare against the
  earlier full-suite run because host state differed.

## 2026-11-27 — Native `print` reports output failures per line

- Flush stdout at the end of every `print`, not only in `main`, because the
  interpreter's stdout is line-buffered and reports `R0013` at the print that
  attempted the write. A final-only flush loses the code and location.
- Capture `errno` immediately after the failed `fputc`/`fflush`/`ferror` check
  and format `strerror(errno)` plus `(os error N)` to match Rust's
  `io::Error` display.
- Ignore SIGPIPE in native `main` to match Rust's default. A closed pipe must
  become the same `R0013` error as the interpreter, not signal termination.
- Keep the final flush fallback in `main`; it is now effectively redundant for
  `print`-only output but remains a defensive check.
- Do not change file-writing builtins or add a general signal policy in this
  task.

## 2026-11-26 — Empty list literals take the value type in `contains`/`push`

- `contains([], value)` and `push([], value)` now check the empty literal as
  `List[type(value)]`. The value is the only source of the element type, and
  this matches the interpreter's result while letting the native backend
  represent the list.
- Keep the value-first check limited to a direct empty list literal. Non-empty
  lists keep element-first inference and their existing diagnostics.
- Do not default `pop([])`, `keys({})`, `values({})`, `contains({}, key)`,
  `len([])`, `print([])`, or unannotated empty-list bindings. No element, key,
  or value type is available; choosing one would be an arbitrary, visible
  static type. These remain interpreter-only `List[Any]`/`Map[Any, Any]`
  cases under the native `E0401` boundary.

## 2026-11-26 — Empty list literals take `join`'s parameter type

- `join` is the only builtin whose first parameter has one unambiguous
  collection type (`List[Str]`). An empty list literal in that position now
  takes `List[Str]` instead of defaulting to `List[Any]`, so `join([], ",")`
  compiles natively and matches the interpreter.
- Apply the expected type only to a direct empty list literal. Non-empty
  literals keep element-first inference and the existing list-level `E0301`,
  so the change does not turn one clear diagnostic into one error per element.
- Do not default other empty literals (`len([])`, `print([])`,
  `let xs = []; push(xs, ...)`) to an arbitrary element type. No element type
  is available, the interpreter's `List[Any]` behavior is established, and
  native rejects `List[Any]` with `E0401`; users annotate the binding or
  collection.

## 2026-11-26 — Depth-limit diagnostics name the call site

- Report `R0012` at the failing call site, not at the callee declaration,
  because that is the location the interpreter uses and the location a user
  needs in order to find the non-terminating call.
- Pass the coordinates through `vantyx_call_line`/`vantyx_call_column` globals
  set immediately before each user-function invocation rather than changing
  every generated function signature. This matches the existing
  `vantyx_source_name` mechanism and keeps the ABI stable.
- Assign `vantyx_source_name` to the callee only after the depth guard, so a
  cross-file failure reports the caller's file. The caller's file is already
  current when the call begins and is restored after each call.
- Use argument preludes whenever any argument has effects, not only for two or
  more arguments. This prevents a nested call in an argument from leaving its
  own coordinates for the outer call's check; the cost is one typed temporary
  for single effectful arguments.
- Keep the guard as an entry check; the reported location is the call site,
  and the guard still prevents C-stack exhaustion.

## 2026-11-26 — `sorted` uses a stable merge sort over scalar lists

- Define `sorted(list: List[T]) -> List[T]` for `T` in `Int`, `Float`, `Str`,
  or `Bool`. The result is a new list, the input is unchanged, and ordering is
  stable. `-0.0` and `0.0` compare equal, every `NaN` sorts after every
  non-`NaN`, strings use UTF-8 byte order, and `false` precedes `true`.
- Implement the native sort as a deterministic stable merge sort with a
  temporary buffer rather than C `qsort`. `qsort` is unstable and its tie
  order can vary by libc, which would make equal floats and NaNs diverge from
  the interpreter.
- Reject non-scalar element types statically instead of inventing an order for
  lists, maps, structs, or enums. Structural ordering would freeze display or
  serialization choices that deserve separate design.
- Keep the interpreter's dynamic `Any` path runtime-checked (`R0005` on mixed
  or non-scalar input) while native keeps its existing `E0401` `Any`
  boundary. This is the same gradual-typing split used by other builtins.
- Defer comparator/key/reverse/in-place variants. A comparator needs
  first-class functions or closures, and an in-place variant changes the
  aliasing contract; both are separate decisions.
- Document the `O(n log n)` time and `O(n)` extra element storage, and do not
  claim deduplication or structural sorting.

## 2026-11-26 — Label stratification assigns exact per-label counts

- Group records by the compact JSON spelling of one top-level scalar field
  (string, number, boolean, null). Spellings are compared exactly, so string
  `"1"` and number `1` are different labels; integral floats and integers share
  a spelling (`1.0` labels as `1`), and labels are not Unicode-normalized.
- Assign `floor(n * ratio)` train records per label, clamped to `[1, n-1]` when
  `n >= 2`, so every multi-record label appears in both splits. Singleton
  labels go to the majority split; only when a global split would be empty does
  one extreme-hash record move, with a warning. This makes the per-label
  guarantee explicit at the cost of a global ratio that can differ from the
  request.
- Order each group by the first 64 bits of the existing seeded split hash with
  record bytes as a tie-breaker, and keep input order in the outputs. This
  reuses T-098's domain separation, makes assignment independent of line order,
  and leaves shuffling to the separate `ai shuffle` step.
- Hold the dataset in memory and cap it at 64 MiB, 1,000,000 records, 1,000
  labels, and a 1,024-byte compact label; no tokenizer-aware balancing or
  external-memory grouping until those contracts and tests exist.

## 2026-11-26 — Dataset shuffle orders by a seeded content hash

- Define the permutation as a sort by
  `SHA-256("vantyx-dataset-shuffle-v1\0" || seed_le64 || record_bytes)` with
  the record bytes as a tie-breaker. This makes the output a pure function of
  the record set and the seed, independent of input order, and avoids pinning a
  PRNG algorithm.
- Hold the dataset in memory and state the bounds (64 MiB, 1,000,000 records)
  instead of implying an external-memory shuffle. Defer a streaming/external
  sort and reservoir sampling until their contracts and tests exist.
- Reuse the dataset validation gate, `create_new`, and the failed-output
  cleanup pattern; do not integrate shuffling into `split`, which keeps each
  step auditable.

## 2026-11-26 — Schema conformance is fixed at the shared validator

- Treat JSON Schema instance equality as a separate operation from Rust's
  structural `PartialEq`: `const`/`enum` compare mathematical numbers and
  order-independent objects, while serialization and unrelated tests keep
  structural equality.
- Compare numbers exactly across `Int`, `UInt`, and `Float` instead of routing
  bounds through `f64`; explicitly reject the `2^63`/`2^64` float boundaries
  because Rust's float-to-integer casts saturate.
- Preflight every schema at the public boundary (`extract`, `ToolRegistry`,
  dataset) so a malformed schema cannot be sent to a provider, cannot be used
  to judge tool arguments, and is reported as a schema/config failure rather
  than a model-output failure.
- Keep the subset strict and documented: unsupported keywords are ignored, and
  preflight checks shapes, not satisfiability.

## 2026-11-26 — Dataset inspection is structural, streaming, and read-only

- Require one top-level JSON object per non-blank line and accept a first-line
  BOM, CRLF, and blank lines. The digest covers every raw byte, including blank
  lines and line endings, so provenance matches the file rather than a
  normalized parse.
- Exit `1` when any non-blank line is invalid so the command can gate a
  pipeline, print the summary before failing, and reserve `64` for usage
  errors. A valid dataset exits `0`.
- Keep tokenization, splits, shuffle, deduplication, and schema/required-field
  checks out of this task; each needs its own deterministic contract and tests.
  Report nested values only as `object` rather than pretending to validate them.

## 2026-11-26 — Agent skills are project-local and advisory

- Store reusable workflows as `.opencode/skills/<id>/SKILL.md` with `name` and
  `description` frontmatter; the harness advertised all thirteen after creation,
  which confirms the mechanism.
- Treat skills as instructions, not enforcement. Agent permissions and
  auto-invocation still control availability. CI validates frontmatter, required
  sections, size, and unique IDs, but not the correctness of every command in a
  skill; document that limit instead of implying a stronger gate.
- Keep each skill Vantyx-specific: workflow, commands, safety constraints,
  acceptance criteria, troubleshooting, and an example, with claims tied to
  observed evidence.

## 2026-11-25 — Trailing final dots do not fall back to earlier dots

- Follow the interpreter's two-step rule literally: find the final dot in the
  path component, then reject it when it is first or last. Do not search for
  the last interior dot as a fallback, because that would silently reinterpret
  `a..` as stem `a` plus extension `.` while the oracle keeps the whole name.
- Share one `vantyx_path_extension_dot` helper between stem and extension so
  the two cannot drift again, and state the no-earlier-dot rule in ADR-0030.
- Keep the fix lexical and allocation-compatible; no new dependency or runtime
  helper category is justified for a one-byte boundary rule.

## 2026-11-24 — T-085 unused parameters are a distinct lint

- Report a never-read function parameter as `L0004`, not `L0001`. The two
  findings have different remedies (remove a local versus remove or underscore
  an interface parameter), so one code would lose information.
- Keep the lexical rule: any read, including a guard or nested block,
  suppresses the finding, and a leading underscore marks intent. There is no
  trait or interface conformance system to justify an unread name.
- Leave loop variables and match payload bindings unreported; their names are
  often dictated by shape, and the conservative linter should not guess.

## 2026-11-24 — T-084 `bool` evaluates its argument

- Treat built-ins that statically reject a type as evaluated calls, not as
  guards. The interpreter evaluates the argument before raising `R0007`, so the
  backend must emit it on both the accepted and rejected paths.
- Bind the argument to a typed temporary and discard it with `(void)` before
  calling `vantyx_bool_from_other`, reusing the T-080 `type` pattern rather than
  adding a new runtime helper.
- Audit remaining built-ins for the same shape: any lowering that emits a
  constant or failure call without its operand is a silent divergence.

## 2026-11-24 — T-083 `Unit` interpolation keeps evaluation

- Treat interpolation as an expression context, not as text formatting. The
  interpreter evaluates `{expr}` regardless of its static type, so the backend
  must emit the expression even when the rendered value is constant.
- Render `Unit` through `((void)(value), vantyx_str_from_unit())` instead of a
  bare `vantyx_str_from_unit()` constant. This reuses the existing display
  helper and does not add a runtime function or special-case the parser.
- Keep the `print` phase separation from T-079: `print` still evaluates into
  temporaries and passes an already-evaluated value to `display_value`.

## 2026-11-24 — T-082 hoist both operands whenever either has effects

- Fix the class, not the instance: any binary operator may lower to a constant
  or otherwise omit an operand, but the language always evaluates both operands
  left to right. `binary_expr` now binds both operands whenever either has
  effects, rather than special-casing `Unit` equality.
- Preserve the deliberate exception for `and`/`or`, which must not evaluate the
  right operand when the left decides the result (ADR-0036).
- The hoist also makes source order explicit, so the fix is compiler-independent
  under both clang and GCC.

## 2026-11-24 — T-081 bind effectful `contains` operands

- Treat C argument evaluation order as unspecified, not as whatever the default
  compiler happens to do. Any native lowering that emits a multi-argument C
  call must bind observable operands to typed temporaries in source order.
- Fix string `contains` by using the same statement-expression pattern as the
  list/map, `starts_with`, and `ends_with` branches instead of adding a new
  runtime helper.
- Test this class with both clang and GCC when both are available, because the
  engines disagree on evaluation order and a clang-only suite cannot detect
  the regression. Skip the test when GCC is absent rather than failing.

## 2026-11-24 — T-080 `type` evaluates its argument

- Treat every built-in call as an ordinary evaluated call. A built-in that
  consumes only metadata (currently `type`) must still evaluate its argument
  exactly once, because calls, mutation, output, and runtime errors inside the
  argument are observable and the interpreter already does this.
- Lower `type` through a typed temporary and `(void)` discard instead of
  special-casing it as a constant; keep the static type-name result so the
  native and interpreter outputs stay identical for supported programs.
- Do not attempt dynamic type tags or `Any` support as part of this fix.

## 2026-11-24 — T-079 print phase separation and float min/max determinism

- Treat signed zero as observable. Float display preserves the sign, so
  `min(-0.0, 0.0)` cannot be left to "either zero": adopt the interpreter's
  documented Rust behavior (second operand on a tie) as the language rule and
  spell it out in both engines instead of relying on `f64::min`/`fmax`.
- Define one NaN rule: a single NaN operand yields the other operand. Both-NaN
  payload/sign is not observable through Vantyx and is left unspecified.
- Make `print` match the interpreter's observable phases: evaluate every
  argument left to right into typed C temporaries, then render all values,
  then write the line. Do not accept partial stdout on a failing argument.
- Use the existing display helpers through a new `display_value` entry point
  rather than buffering a whole line in a second formatter; keep the change
  local to `print` and the two float helpers.
- Do not add buffered output, streaming, error recovery, or a general
  evaluation-order guarantee beyond the documented left-to-right argument
  order.

## 2026-11-24 — T-078 strict native numeric-string validation

- Trim Unicode whitespace and validate the exact interpreter integer/float
  grammar in C before calling `strtoll`/`strtod`; do not trust libc's wider
  grammar (hexadecimal floats, `nan(payload)`, trailing text).
- Ignore float `ERANGE` so overflow and underflow still match Rust (`Infinity`
  and `0.0`); keep integer overflow as `R0007`.
- Keep the existing `R0007` error code and document the locale assumption
  (decimal point `.`), native embedded-NUL limitation, and unchanged float
  display.



- The differential test originally created 100,001 hard links to one anchor.
  ext4 caps hard links per inode at 65,000, so the test failed on the CI runner
  before reaching the glob cap.
- Keep the end-to-end cap coverage but spread the links across four anchors,
  producing more than 100,000 candidates while keeping each inode below the
  limit.
- Record the failed run and the fix rather than weakening the cap assertion.

## 2026-11-24 — T-067 cloud re-verification with `MY_API_KEY`

- Reuse `tools/live_ai_check.py` and pass `--api-key-env MY_API_KEY` so the key
  is read from the shell configuration without printing or copying its value.
- Preserve both the transient HTTP 503 extraction failure and the successful
  21/21 re-run as evidence rather than deleting the failed attempt.
- Treat the successful re-run as point-in-time evidence. An earlier retry was
  rejected by provider geography, so do not claim continuous cloud access.
- Keep T-040's local-model half blocked; cloud evidence does not substitute for
  a local inference server or trained local embedding model.

## 2026-11-24 — T-069 destructive recursive-removal roots

- Refuse two lexical root classes before any filesystem access: a final `.` or
  `..` component after trailing-separator trimming (including `link/.` and
  `link/..`), and the filesystem root `/` including repeated separators.
- Keep the rule lexical rather than canonicalizing or resolving links. This
  removes the two easy accidental spellings without pretending to provide a
  sandbox; intermediate `link/sub` traversal and check-then-act races remain
  documented caller responsibilities.
- Use `R0015` with a stable explanation instead of a new error code or a
  recoverable error, consistent with the rest of the directory builtins until
  `Result` exists.
- Replace the differential test that locked the destructive
  `remove_dir_all(".")` behavior; do not preserve a test that presents a
  footgun as intended semantics.
- Keep `./tree` and `tree/` accepted so ordinary relative paths remain usable.

## 2026-11-24 — T-068 curl timeout classification

- Classify system-curl exit code 28 (operation timeout) as retryable
  `AI_E_TIMEOUT` on both buffered and streaming HTTPS paths. Keep DNS/connect
  failures such as exit 7 as `AI_E_CONNECT`.
- Share one `curl_failure` helper instead of duplicating status parsing in the
  two transports.
- Do not add an automatic retry loop or `Retry-After` handling in this change;
  the live harness performs bounded retries and records backoff.

## 2026-11-24 — T-067 live cloud verification

- Use Gemini's OpenAI-compatible endpoint for the live check:
  `https://generativelanguage.googleapis.com/v1beta/openai`,
  `gemini-3.1-flash-lite`, and `gemini-embedding-001` at 768 dimensions.
- Read the credential from an environment variable, never print or store it,
  and record only the variable name in evidence.
- Treat live results as point-in-time observations, not a continuous
  availability claim: the provider later returned HTTP 400 `User location is
  not supported` for both available credentials.
- Keep the local-model half of T-040 blocked; do not substitute cloud evidence
  for local evidence.
- Use `tools/ai_failure_harness.py` with loopback servers and a placeholder key
  for deterministic 401/429/timeout coverage.

## 2026-11-24 — T-066 recursive copy scope

- Require a new target rather than merge/overwrite semantics, and reject a
  resolved target equal to or inside the resolved source.
- Follow links in the source path, but reject symbolic links and special files
  below the source root; copy regular-file bytes and raw entry names without
  preserving metadata.
- Cap trees at 100,000 entries and 128 levels below the source root. Keep
  fatal `R0015` errors until `Result` exists.
- Accept partial targets after failure; no rollback, atomic staging,
  cancellation, recursive move, or sandbox in v0.1.

## 2026-11-24 — T-065 glob scope

- Add one `glob(Str) -> List[Str]` builtin instead of a shell subprocess, a
  regex engine, or a third-party glob crate. Keep the grammar smaller than
  shell globs: no escaping, braces, extglobs, tilde, case-insensitive mode, or
  sorting options.
- Treat only a whole-component `**` as recursive; `a**b` and `***` are ordinary
  wildcard components. Final `**` returns descendants but not the base.
- Skip dot entries for wildcards and `**` unless the component explicitly
  allows them. Do not descend through symbolic links during `**`, but allow
  the base itself and explicit link components to traverse.
- Probe final literal components with `lstat` so dangling links are returned;
  treat any final `lstat` failure as no match. Missing paths and non-directories
  produce no matches, not errors.
- Sort by UTF-8 bytes, deduplicate, and return lexical spellings rather than
  canonical paths. Normalization and `..` confinement are out of scope.
- Cap raw candidates at 100,000, `**` depth at 128, and pattern components at
  1,024. Result/depth overflow and non-UTF-8 entry names are `R0015`;
  malformed patterns and the component cap are `R0016`.
- Reject NUL patterns with `R0015` in the interpreter; native NUL source is
  already `E0401`. Keep static/dynamic type boundaries at `E0301`/`R0005`/
  `E0401`.

## 2026-11-24 — T-064 metadata probe scope

- Add four read-only global builtins (`file_size`, `is_file`, `is_dir`,
  `is_symlink`) instead of a metadata object. The language has no builtin
  struct-return convention or `Result`, and a fixed object would expose
  host-specific fields.
- Follow symbolic links for size and type predicates to match `file_exists`;
  use `lstat` only for `is_symlink` so dangling links remain observable.
- Make `file_size` require an existing regular file and report logical `st_size`
  with fatal `R0015` failures. Do not return `-1` or a sentinel.
- Make predicates total: missing paths, dangling links, and permission failures
  return `false`. Callers that need the reason must use `file_size` or a
  failing operation; document that predicates cannot distinguish them.
- Expose no mode, owner, group, timestamp, ACL, extended attribute, inode,
  device, birthtime, or recursive directory size, and do not modify metadata.
- Keep the native boundary strict: `Any` paths are `E0401`, and native strings
  already cannot contain NUL; the interpreter maps NUL paths to `R0015`.
- Accept check-then-act races; the probes are not a sandbox or a security
  boundary.

## 2026-11-24 — T-063 recursive directory scope

- Add `make_dir_all` and `remove_dir_all` as fatal-error builtins with no new
  syntax and no dependency. They mirror the existing `R0015` model until
  `Result`/`?` exists.
- Use host recursive creation (`std::fs::create_dir_all` in the interpreter,
  component-by-component `mkdir` natively) and accept an existing directory or
  symlink to a directory. An empty path is a deliberate no-op success because
  `create_dir_all("")` observed success on the host.
- Never traverse symbolic links during recursive removal. Trim direct trailing
  separators before the root type check because the host can follow a root
  `link/` into its target; document that `link/.` and `link/..` remain
  traversing paths and are the caller's responsibility (superseded by T-069,
  which refuses final `.`/`..` components).
- Report `R0015` for regular-file roots, missing paths, and empty removal
  targets. Do not add confirmation, trash, dry run, rollback, depth limit, or
  entry-count limit; document that a failed removal can leave a partial tree.
- Native recursion uses `lstat`/`opendir`/`readdir`/`unlink`/`rmdir` and the C
  stack. Recursive removal is not a sandbox.
- Reject dynamically typed native paths with `E0401`; map dynamic interpreter
  values to `R0005`, matching the rest of the I/O builtins.

## 2026-11-24 — T-062 lexical path scope

- Add pure lexical path helpers rather than a nominal `Path` type or OS-aware
  path APIs. The helpers need no type-system or platform abstraction and can be
  specified exhaustively.
- Keep POSIX `/` as the only separator and do not normalize `.`, `..`, or
  repeated separators. This is predictable for string assembly but explicitly
  not a containment or sanitization mechanism.
- Split extension textually at the final dot only when it is neither the first
  nor the last byte, matching common path-library behavior for dotfiles and
  trailing dots without treating compound extensions specially.
- Use `"."`/`"/"` as empty parents and `""` for an empty final component,
  matching Python `pathlib` lexical expectations while documenting that no
  filesystem resolution occurs.
- Add no error code: invalid static types are `E0301`, dynamic interpreter
  values are `R0005`, and the native boundary is `E0401`.
- Evaluate `path_join` left to right with a GNU C statement expression; the
  other helpers take one argument.

## 2026-11-24 — T-061 rename/copy scope

- Add `rename` and `copy_file` as fatal-error builtins rather than waiting for
  `Result`/`?`; this keeps file management usable under the existing error
  model.
- `copy_file` is a byte-copy boundary, not a string operation: it does not
  enforce UTF-8, NUL, or the 64 MiB read cap, and it preserves no metadata.
- Delegate rename overwrite and atomicity behavior to the host. Do not add
  self-copy detection because path aliases, hard links, and symbolic links make
  reliable detection host-specific; document that source and target must be
  distinct.
- Use GNU C statement expressions to evaluate source before target in the
  native backend, matching the interpreter's left-to-right argument order.
- Do not add recursive copy/move, globbing, metadata APIs, atomic copy, or
  progress reporting; leave them for a later standard-library task.

## 2026-11-24 — T-054 incremental RAG scope

- Reuse vectors only when the stable chunk id, provider, model, and dimensions
  all match. Any mismatch is an explicit error or a full rebuild, never a silent
  mix.
- Treat the current `--root` scan as the complete corpus and prune chunks from
  deleted files. Partial-prefix updates and multi-root merging are out of scope.
- Require an existing semantic index for `--incremental`; reject lexical indexes
  and provider/model/dimension changes with actionable help.
- Skip provider construction entirely when no chunk is pending, so a no-op
  update performs no network or model work.
- Allow an empty semantic index (`dimensions > 0`, zero chunks) so deleting every
  document is representable; empty search returns no hits.
- Keep ingestion and chunking full-corpus. Document that the measured saving is
  avoided embedding calls, not scan time.
- Defer streaming tool calls because the CLI has no tool-execution surface;
  implementing them now would be library-only.

## 2026-11-24 — Enum benchmark and representation

- Add an `enum_ops` workload before changing the heap-allocated native enum
  representation. The measured native median is in the same band as C and Rust
  (3.646 ms vs 3.766 ms and 4.529 ms in the same run; a targeted run measured
  4.208 ms vs 3.557 ms and 3.370 ms), so the evidence does not justify a
  representation rewrite.
- Do not claim enum performance from a single run; absolute medians shift with
  machine load, so compare implementations within the same run.

## 2026-11-24 — Native string character buffers

- Single-byte ASCII characters returned by iteration or indexing share an
  immutable static table instead of allocating a heap string per character.
  Native strings are never mutated in place, and the runtime is single-threaded,
  so the shared pointers preserve semantics.
- Multibyte UTF-8 characters still allocate. Concatenation still copies into a
  fresh exact-size buffer; no ownership or reclamation change is implied.
- The measured `string_ops` improvement is a runtime optimization, not a claim
  about general string performance.

## 2026-11-24 — Verification and status discipline

- `VERIFIED` requires an observed command, date, and recorded evidence.
- Benchmarks remain labelled plumbing-only when using the offline hash provider.
- Do not claim live AI, grounding, GPU, training, or production readiness without
  direct evidence.

## 2026-11-24 — MMR diversity reranking

- Add diversity reranking as an explicit opt-in (`--diverse` /
  `search_with_source_diverse`) instead of changing default ranking. Flat top-k
  remains the baseline and evaluation behavior.
- Use a bounded wider candidate pool (`limit * 4`, clamped to `20..MAX_TOP_K`,
  or the full filtered set when `source_prefix` is set) so reranking has
  alternatives without scanning the whole index for unfiltered searches.
- Suppress identical trimmed chunk text before MMR, keeping the highest-ranked
  copy. This is exact duplicate suppression, not semantic paraphrase dedup.
- Use cosine similarity over stored vectors when every candidate has a vector;
  otherwise use BM25-token Jaccard so lexical-only indexes can be diversified.
  Normalize relevance by the best candidate score so `lambda` is comparable
  across modes.
- Keep deterministic tie-breaking by candidate order; the CLI exposes only the
  default `lambda = 0.7`, while library callers can choose any finite value in
  `[0, 1]`.
- Treat reranking as a context-efficiency control. Do not claim improved
  retrieval quality without a measured evaluation.

## 2026-11-24 — Basic file I/O

- Add small globally available builtins (`read_file`, `write_file`,
  `append_file`, `file_exists`) instead of waiting for a module system,
  `Result`/`?`, and generics. They follow the existing fatal-error builtin
  style and do not change syntax.
- Preserve the string invariant: reads reject invalid UTF-8 and NUL bytes, and
  writes reject NUL paths/contents. Do not return lossy strings.
- Cap `read_file` at 64 MiB in both engines. The cap is a fixed policy, not a
  claim that 64 MiB is always safe on every host.
- Use host paths with process privileges and the process working directory.
  There is no sandbox, path confinement, or atomic-write guarantee; document
  TOCTOU and partial-write risks.
- Make `file_exists` the only non-failing probe. Missing files, permission
  errors, directories, invalid contents, and oversized files use `R0015` until
  a recoverable `Result` type exists.
- Implement the native path with libc only and validate UTF-8 in C so native
  output and exit codes match the interpreter under differential tests.

## 2026-11-24 — Directory operations

- Extend the global builtin slice with `list_dir`, `make_dir`, `remove_file`,
  and `remove_dir` rather than waiting for a module system or `Result`. They
  follow the file-I/O error contract and add no syntax.
- Return entry names only, omit `.` and `..`, and sort by UTF-8 bytes so output
  is deterministic across hosts and engine implementations.
- Reject non-UTF-8 entry names with `R0015` instead of returning lossy strings
  that cannot be fed back into other path builtins.
- Keep operations single-level: `make_dir` does not create parents,
  `remove_dir` refuses non-empty directories, and `remove_file` unlinks files or
  symbolic links rather than directories. Recursive operations remain future
  work.
- Use `opendir`/`readdir`/`closedir`, `mkdir`, `unlink`, and `rmdir` natively;
  accept the explicit POSIX dependency and document that Windows native support
  remains unverified.
- Read the whole directory into memory. Do not add an entry-count or
  total-name-byte cap without evidence that directories large enough to matter
  are a real workload; document the memory behavior instead.


## 2026-11-24 — UTF-8 text builtins

- Add six global builtins (`split`, `join`, `trim`, `starts_with`,
  `ends_with`, `replace`) instead of waiting for a module system or generics.
  They follow the existing fatal-error builtin style and add no syntax.
- `split` is literal and lossless for a non-empty separator: empty fields and a
  trailing empty field are preserved, and an empty input yields `[""]`. An
  empty separator splits into Unicode scalar values; `split("", "")` is `[]`.
- `join` accepts `List[Str]` statically; empty lists produce `""`. Dynamic
  `List[Any]` stays interpreter-only with `R0005` on a non-`Str` element, and
  native rejects non-`Str` element types with `E0401`.
- `trim` uses one explicit Unicode `White_Space` table shared by both engines
  instead of host whitespace predicates. Zero-width spaces and U+001C–U+001F
  are deliberately preserved.
- `replace` is left-to-right, non-overlapping, and does not rescan replacement
  text. An empty pattern is a fatal `R0016` invalid-argument error rather than
  boundary insertion.
- Multi-argument native builtin calls use GNU C statement expressions so
  argument evaluation is left to right, matching the interpreter.
- No regex, case mapping, normalization, locale handling, grapheme
  segmentation, streaming API, or output-size cap is implied.

## 2026-11-24 — Native condition and short-circuit corrections

- Treat both defects as P0 correctness bugs, not documented backend limits:
  the interpreter is the oracle, and ordinary programs must neither fail to
  build nor silently change side effects.
- Fix the condition wrapper inside `condition` rather than adding
  unconditional parentheses at every `if`/`while` site. Wrapping only a
  rendered `({ ... })` statement expression avoids clang's
  `-Wparentheses-equality` warning on comparisons and keeps generated output
  clean.
- Lower an effectful `and`/`or` right operand through a lazy conditional
  expression inside a statement expression; keep ordinary `&&`/`||` when the
  right operand has no effects. Do not evaluate both operands eagerly.
- Add a deterministic differential fuzzer instead of relying only on
  hand-written cases. Keep it standard-library-only, seed-reproducible, and
  wired into CI with a fixed seed; save failing programs for regression tests.
- Record the contract in ADR-0036 and keep the `E0401` boundary unchanged.

## 2026-11-24 — Static lints and reachability-aware returns

- Keep lint findings out of `run`, `check`, and `build`. The linter is an
  opt-in tooling pass; ordinary compilation and execution stay quiet, and
  existing behavior does not change.
- Report warnings only and exit `0` for lint-only findings. Compilation errors
  still exit `1`. Do not add a deny mode until there is a documented need and a
  testable contract.
- Stay conservative in the first pass: ignore names starting with `_`,
  parameters, loop variables, and match bindings; report only the first
  unreachable statement per block; do not claim control-flow analysis.
- Fix `block_returns` instead of documenting the `E0309` false positive. A
  reachable `return` terminates a block even when unreachable statements follow,
  and a fully returning `if`/`else` or `match` terminates it too. A reachable
  `break`/`continue` stops the walk without counting as a return.
- Keep the formatter as separate future work because the lexer discards
  comments; do not build a rewrite-based formatter that loses user source.

## 2026-11-24 — Result-enum propagation with `?`

- Use a positional two-variant convention instead of a built-in generic
  `Result[T, E]`. The language has no generics, and globally unique variant
  constructors would otherwise limit a program to one `Ok`/`Err` pair.
- Restrict `?` to statement positions: a whole `let` initializer, a plain
  assignment value, or an expression statement. `eval_expr` returns only
  `Value` or `RuntimeError`, so general expression-position propagation would
  require a control-flow-aware evaluator. A statement-position MVP is small,
  honest, and fully testable.
- Require the enclosing function to have an explicit return type equal to the
  operand's enum, so the failure value can be returned unchanged.
- Lower `?` as a dedicated native `Try` statement, not as an IR match
  expression. `match_arm_expr` only accepts expression arms, and block arms
  with `return` would panic or require a larger match-lowering change.
- Keep `R0017` as a defensive runtime error for operands that bypass the
  checker; the checker normally rejects invalid placements and shapes first.

## 2026-11-24 — Differential-fuzzer coverage and shrinking

- Add one broad control-flow shape rather than many tiny shapes. It combines
  loop forms, match statements, early returns, guards, compound assignment,
  and shadowing so a single generated program exercises interactions that
  isolated examples miss.
- Keep shrinking opt-in and line-oriented. It preserves the failure's
  exit-code signature and skips candidates that stop failing, but it cannot
  split a line or prove a global minimum. `--shrink-budget` bounds the extra
  builds so a failure report cannot run unbounded.
- Map subprocess timeouts to exit `124` rather than raising, so a hang is a
  controlled differential failure.
- Add the shrinker's pure comparison logic to a Python unit test file and run
  it in CI; the real differential run remains the integration evidence.

## 2026-11-24 — Strict native numeric-string conversion

- Match the interpreter by validating its exact grammar in C instead of
  reimplementing a parser. `vantyx_text_trim` already implements the same
  Unicode `White_Space` set, and `strtoll`/`strtod` are used only after the
  grammar check, so conversion accuracy stays with libc while acceptance and
  rejection stay with the oracle.
- Ignore float `ERANGE`: Rust parses overflow to infinity and underflow to
  zero, so treating it as an error would create a new divergence.
- Keep `R0007` and the existing messages. This is a native parity fix, not a
  language change; the specification now states the accepted grammar.

## 2026-11-25 — Lexical function scope

- Resolve names in the active function frame and then the program's root
  scope. Never search caller frames. This matches the checker and native
  backend, and it prevents accidental caller-state reads and writes when a
  local reuses a global's name.
- Represent the boundary as the first scope index of the active call rather
  than copying globals or adding a separate global table. Assignments must
  still update the one global binding, and recursion must keep independent
  frames.
- Save and restore the boundary around every call, including the error path.
  A panicking or failing callee must not leave the interpreter resolving names
  against the wrong frame afterward.
- Keep dynamic scoping out of the language. A function's behavior must not
  depend on its caller's local names.

## 2026-11-26 — Offline model inspection and same-file copies

- Parse GGUF and Safetensors headers in-tree rather than shelling out to
  `sha256sum`, `llama.cpp`, or Python. The host must stay dependency-free,
  offline by default, and must never execute model code. ADR-0041 records the
  bounded parser and streaming SHA-256.
- Validate tensor ranges and known quantization block sizes. A summary that
  silently accepts a truncated, overlapping, or malformed file is worse than
  no summary.
- Keep `AI_E_FORMAT` distinct from `AI_E_IO` and print a concrete help line for
  format errors. Usage errors stay exit `64`; format and I/O errors exit `1`.
- Do not add download, pull, remove, conversion, quantization, or weight
  loading in this change. `inspect` is the provenance primitive those later
  commands need, not a substitute for their integrity and license gates.
- Refuse same-file `copy_file` targets before opening the target. The guard is
  best-effort and raceable, and is documented as such rather than claimed to be
  atomic.

## 2026-11-26 — Safetensors shard-index aggregation

- Extend `inspect` instead of adding a second command. Content detection plus
  a `.json` fallback keeps one output schema, and the input type is
  unambiguous.
- Treat the index as the declared model boundary. Do not scan the directory for
  extra `.safetensors` files; unlisted files may be unrelated, and a scan adds
  unbounded work.
- Cross-check the index against every shard header. Trusting `weight_map` alone
  would miss tensors that exist but are unlisted and tensors that are listed
  but absent.
- Require relative `/`-separated `.safetensors` paths with no `..`, then
  canonicalize and require containment under the index directory. Reject a
  shard that is itself an index so hostile files cannot cause recursion.
- Treat `metadata.total_size` as advisory. Producers differ on what the value
  counts, so a mismatch is a warning rather than a false rejection.
- Hash every referenced shard and report per-shard digests. An aggregate
  summary alone cannot prove that a shard is unchanged.
- Keep the operation offline and read-only. No downloads, weight loading,
  conversion, or execution; no new dependency.

## 2026-11-26 — Shared runtime diagnostics across both engines

- Make the interpreter the source of truth for wording, as for behavior. Where
  the interpreter already quoted the offending value or path, the native
  runtime was changed rather than simplifying the interpreter message.
- Render floats through the Vantyx display (`format_float` /
  `vantyx_float_str`) in diagnostics, not Rust's or C's default formatting.
  This keeps `print`, `str`, and error messages consistent and resolves the
  `inf` versus `Infinity` and integral `.0` differences at once.
- Render a missing `Str` map key with top-level display (unquoted) and other
  keys structurally, matching `Value::display`.
- Report the requested `range` element count instead of an implementation
  limit so the message explains the program's request.
- Include the path or directory in file and directory failures. A bare
  `file contains a NUL byte` is ambiguous when several reads can fail.
- Compare the first diagnostic line, not only the error code, in differential
  tests. Codes stayed equal while the user-visible explanation diverged.
- Keep the caret/source excerpt out of scope: the native runtime does not carry
  the interpreter's source registry, so only the headline is guaranteed.

## 2026-11-26 — Filesystem error text and glob parse diagnostics

- Match Rust's `io::Error` display on Unix by appending ` (os error N)` to
  `strerror` text. The native runtime must reproduce the interpreter's
  user-visible message, not the C library's shorter one.
- Centralize the formatting in `vantyx_os_error` so every filesystem site stays
  consistent; do not hand-format each message.
- Reuse the interpreter's glob parser wording verbatim, including
  `invalid glob pattern:` and quoted range endpoints, because both engines
  implement the same language-level error.
- Add the new cases to the existing headline-parity test rather than creating a
  second differential harness; the test already compares the first error line
  for both engines.
- When a shared helper is introduced into generated C, verify the helper is not
  recursive and is defined before every use. The first `vantyx_os_error`
  version called itself and segfaulted the native binary; differential probes
  caught it before the regression test was added.

## 2026-11-26 — Deterministic dataset splits

- Assign each record by a seeded SHA-256 of its bytes rather than shuffling or
  numbering lines. The split must not change when unrelated lines are inserted
  or reordered, and it must be reproducible without storing per-record state.
- Use the top 32 bits against a ratio cutoff. This keeps memory constant for
  the 256 MiB dataset cap while producing an unbiased assignment.
- Validate before creating outputs and validate again during the scan. A split
  that writes records the inspector rejected would defeat the dataset gate.
- Refuse to overwrite with `create_new`, and remove files created by a failed
  run. Silent replacement of a training corpus is worse than a failed command.
- Guarantee both splits are non-empty by moving the minimum- or maximum-hash
  record and reporting the move. Random retries or a hidden time seed would make
  the output irreproducible.
- Normalize BOM, CRLF, and blank lines in the outputs and warn about each one.
  The input stays untouched, and the report's digests cover exactly what was
  written.
- Report actual counts rather than asserting the requested ratio was achieved.
  Whole-record assignment cannot guarantee an exact ratio without extra state.
- Keep shuffle, deduplication, tokenizer-aware counts, and label stratification
  out of this task; they are separate capabilities with different correctness
  questions.

## T-100 — Exact JSONL dataset deduplication

- Use exact normalized record bytes as identity, not canonical JSON. It is
  deterministic, needs no canonical encoder, and agrees with the split
  boundary that always places byte-identical records on the same side.
- Keep the first occurrence in input order so the output is stable and easy to
  diff against the input.
- Domain-separate the record digest
  (`vantyx-dataset-dedup-v1\0` + bytes) so it cannot be confused with a split
  bucket or another SHA-256 use.
- Validate before creating output, revalidate during the write pass, and
  remove the output on any failure. A deduplicated corpus must not contain
  records the inspector rejected.
- Refuse an existing output and the input path; use `create_new`. Silent
  replacement of a corpus is worse than a failed command.
- Cap the in-memory digest set at 1,000,000 distinct records and fail
  explicitly above it. On-disk external sorting is a larger separate design.
- Leave JSON canonicalization, near-duplicate detection, shuffle,
  tokenizer-aware counts, and label stratification out of this task.

## T-111 — Single-buffer native string construction

- Use one growable `VantyxStrBuilder` for interpolation and multi-operand
  string `+` chains instead of one `vantyx_str_concat` allocation per part and
  intermediate. The final buffer is transferred to the result; it is not
  copied or freed.
- Append parts strictly in source order and evaluate each part immediately
  before appending. This preserves calls, output, mutation, and runtime-error
  order, which a two-pass exact-size allocation would complicate.
- Flatten nested interpolations into the same buffer. String concatenation is
  byte-associative, and the flattened order is the same left-to-right order
  the interpreter evaluates.
- Keep the two-plain-operand and one-part interpolation paths unchanged. The
  builder overhead is not justified for the simplest shapes.
- Share `vantyx_float_format` with `vantyx_float_str` so builder output cannot
  drift from scalar rendering.
- Do not free intermediates. v0.1 has no ownership or liveness analysis and
  strings may be aliased; reclamation still requires its own ADR.
- Do not claim a language-wide speedup. Record the focused alternating
  before/after measurement and the full benchmark run separately; the host was
  slower during the full run, so only within-run and same-batch comparisons
  are valid.
