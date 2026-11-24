# Decisions

Append-only record of engineering decisions made in autonomous sessions. Design
rationale that affects semantics belongs in `docs/decisions/` as an ADR; this
file records operational choices.

## 2026-11-24 — CI portability for the glob result-cap test

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
