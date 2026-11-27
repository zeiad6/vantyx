# Security Model

Vantyx treats the compiler, runtime, and generated code as security-sensitive infrastructure. This document states what is actually guaranteed today and what is not.

## Status (v0.1)

- Single-threaded interpreter and native runtime.
- No sandboxing. Do not execute untrusted Vantyx programs without OS-level isolation.
- No FFI yet, so there is no foreign-code trust boundary in the language.
- The compiler core has zero third-party Rust dependencies (ADR-0007).

## Compiler robustness

Expected properties, exercised by tests:

- The lexer and parser return diagnostics instead of panicking on malformed UTF-8 source.
- Parser recursion is bounded: expression nesting (`E0107`), AST depth (`E0108`), block nesting (`E0109`), type nesting (`E0110`), and interpolation nesting (`E0111`) all produce diagnostics instead of stack overflow.
- `SourceMap::line_col` clamps offsets to UTF-8 boundaries, so invalid spans cannot panic diagnostic rendering.
- The checker reports errors and does not execute user code.
- IR lowering rejects unsupported constructs with `E0401` instead of miscompiling them.
- Runtime failures become diagnostics with exit code 2; compile failures exit 1; usage errors exit 64.

Known limits:

- Fuzzing is reproducible: CI runs the differential fuzzer and `python3 tools/fuzz_smoke.py --binary target/debug/vantyx`. CI also compiles and runs the native suite against generated programs using `tools/clang-sanitize.sh`, which adds AddressSanitizer and UndefinedBehaviorSanitizer; leak detection is disabled because native allocations are intentionally leaked in v0.1.
- Resource exhaustion (very large files, huge collections) is bounded by explicit limits where implemented and otherwise by available memory.
- The native backend invokes a C compiler discovered via `PATH` or `VANTYX_CC`; that compiler is trusted.

The full audit record is in [security-audit-2026-11-22.md](security-audit-2026-11-22.md).

## REPL history

The REPL can persist submitted entries in `$VANTYX_HISTORY` or `$HOME/.vtx_history`. The file is created with mode 0600 on Unix, but it is plain text and may contain secrets typed into the REPL. Set `VANTYX_HISTORY=off` to disable persistence. Interactive editing invokes `/bin/stty` or `/usr/bin/stty` from fixed paths; it does not search `PATH`, and failure falls back to canonical input. The editor is safe Rust and contains no `unsafe` block. With `panic = "abort"`, an unexpected panic while raw mode is active could leave the terminal in raw mode; `stty sane` restores it. See [ADR-0013](decisions/ADR-0013-repl-line-editing.md).

## Interpreter memory safety

The interpreter is written in safe Rust and uses `Rc<RefCell<...>>` for reference values. It prevents use-after-free, double-free, dangling references, out-of-bounds access, and uninitialized reads in Vantyx code. `Rc` cycles leak; this is a resource issue, not memory unsafety. There is no data-race risk because execution is single-threaded.

## Native backend safety

For the supported subset:

- Integer arithmetic, division, and range construction are checked.
- Uninitialized global reads raise `R0009`.
- Allocation failure terminates with a runtime error.
- Generated C uses libc/libm only.
- CI runs the native differential suite with generated programs compiled under AddressSanitizer and UndefinedBehaviorSanitizer (`tools/clang-sanitize.sh`); leak detection is disabled because native allocations are intentionally leaked in v0.1.
- Native calls are limited to depth 256 (`R0012`); the diagnostic names the failing call site in both engines, including across modules (T-104).
- List and string indexes are bounds-checked; missing map keys raise `R0008`.
- List and map growth checks overflow and `SIZE_MAX` before allocating (`R0004`).
- Structural equality is cycle-safe and matches the interpreter's coinductive semantics.

Known native limits:

- Strings, structs, list buffers, map buffers, scalar-key map indexes, and enum values are intentionally leaked.
- `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, and unsupported loop forms are rejected with `E0401`.
- Native scalar-key map lookup uses a fixed-seed hash index; adversarial key streams can degrade probing. Struct/enum/list keys remain O(n) (ADR-0016).
- No debug information, stack traces, or source-level unwinding in native programs.

## File and directory I/O boundary

- `read_file`, `write_file`, `append_file`, `file_exists`, `file_size`,
  `is_file`, `is_dir`, `is_symlink`, `list_dir`, `glob`, `make_dir`,
  `remove_file`, `remove_dir`, `make_dir_all`, `remove_dir_all`, `rename`,
  `copy_file`, and `copy_dir_all` run with the host process's privileges. There
  is no sandbox, allowlist, or path confinement.
- Reads are capped at 64 MiB, and contents must be valid UTF-8 without NUL
  bytes before they become Vantyx strings. `list_dir` and `glob` reject
  non-UTF-8 entry names; `copy_dir_all` copies raw entry-name bytes without
  conversion. Failures raise `R0015` and exit 2.
- Writes are not atomic; an error during `write_file` or `append_file` can leave
  a partial file. `file_exists` returns `false` when `stat` fails, including
  permission errors.
- `file_size`, `is_file`, and `is_dir` follow symbolic links; `is_symlink` uses
  `lstat` and observes the link itself. `file_size` raises `R0015` for missing
  paths, dangling links, and non-regular targets, while the predicates return
  `false` on any inspection failure and therefore do not distinguish missing
  from permission-denied. The probes expose only logical size and path kind:
  there is no mode, owner, timestamp, ACL, extended-attribute, inode, device,
  or recursive directory-size query, and no metadata is modified or preserved.
- `list_dir` reads the complete directory into memory and materializes one
  string per entry; a very large directory can consume proportional memory.
- `glob` walks the filesystem and materializes a sorted, deduplicated result
  list. It caps raw candidates at 100,000, `**` recursion at 128 levels, and
  patterns at 1,024 components, but multiple `**` components can revisit
  overlapping subtrees and CPU work is not capped. Every listed directory is
  read fully into memory, and non-UTF-8 entry names fail the expansion with
  `R0015` rather than being skipped.
- `glob` does not follow symbolic links while expanding `**`, but it does list
  link entries, a `**` base may itself be a link, and explicit link components
  or `..` can leave the apparent directory. It has no sandbox, `..`
  confinement, normalization, case folding, or escaping. Hidden entries are
  skipped unless a component explicitly allows them. Paths are ordinary
  host paths and results are lexical spellings, not canonical paths.
- `make_dir`, `remove_file`, and `remove_dir` are single-level. `remove_file`
  unlinks a symbolic link rather than its target and refuses directories;
  `remove_dir` refuses non-empty directories.
- `make_dir_all` creates missing parents and accepts an existing directory or a
  symbolic link to a directory. `remove_dir_all` deletes a whole tree without
  confirmation. It unlinks symbolic links instead of traversing them, including
  a root link, and trims trailing separators before the root type check so
  `link/` is unlinked rather than followed. Before any filesystem access it
  refuses `.`, `./`, `..`, `../`, any root whose final component is `.` or `..`
  (including `link/.` and `link/..`), and the filesystem root `/` including
  repeated-separator spellings (ADR-0035). Ordinary roots such as `tree`,
  `./tree`, and `/tmp/tree` remain accepted. Intermediate components can still
  reach through a symbolic link (`link/sub`), so untrusted roots need caller
  validation. The classifier recognizes `/` separators; Windows interpreter
  paths that use `\` are not classified, and Windows native support is
  unverified. A failed recursive removal can leave a partially removed tree;
  there is no rollback, trash, dry run, depth limit, or entry-count limit.
  Native recursion uses the C stack and is not a sandbox.
- The previous behavior where `remove_dir_all(".")` emptied the working
  directory before failing was removed in ADR-0035. A program that needs to
  clean the current directory must enumerate entries explicitly and remove
  named children.
- Both recursive operations are subject to check-then-act races. `make_dir_all`
  confirms that each component is a directory but cannot prevent a concurrent
  replacement between the check and the next `mkdir`.
- `rename` moves one file or directory. Overwrite behavior and atomicity are
  host-defined; on POSIX, replacing a non-empty directory or crossing
  filesystems fails. It does not create parents or preserve metadata.
- `copy_file` requires a regular source file and copies bytes only. It does not
  validate UTF-8 or NUL, does not preserve permissions, ownership, timestamps,
  extended attributes, ACLs, or hard-link identity, and is not atomic: a
  failure can leave a partial target. Source and target must be distinct;
  passing the same path can truncate the source before it is read. The
  regular-file check is subject to a check-then-open race.
- `copy_dir_all` requires a new target outside the resolved source tree and
  rejects inner symbolic links and special files. Its containment check only
  prevents recursive self-copy; it is not a sandbox. The target is created
  before traversal, so a failure leaves a partial tree with no rollback.
  Permissions, ownership, timestamps, extended attributes, ACLs, hard-link
  identity, and sparse layout are not preserved. Trees are capped at 100,000
  entries and 128 directory levels below the source root; total bytes are not
  capped. Target-parent resolution, containment, and traversal are separate
  check-then-act steps, so a concurrent rename or link swap can redirect the
  walk.
- Check-then-act sequences using `file_exists`, `file_size`, `is_file`,
  `is_dir`, `is_symlink`, `list_dir`, `glob`, `copy_dir_all`, or a preceding
  operation are
  subject to time-of-check/time-of-use races. Directory entries can disappear
  or change between listing, `lstat`, and later use. Removing, renaming, or
  replacing paths concurrently can change which object an operation affects.
- Do not execute untrusted Vantyx source without OS-level isolation.

## Path helper boundary

- `path_join`, `path_parent`, `path_name`, `path_stem`, `path_extension`, and
  `path_is_absolute` are pure lexical string operations. They do not access the
  filesystem, current directory, environment, locale, or user home.
- They do not normalize `.` or `..`, do not collapse repeated separators, and
  are not a containment or sanitization mechanism. `path_join("/srv", "../x")`
  preserves `..`, and an absolute child discards the base entirely.
- Only POSIX `/` is recognized as a separator. Backslashes, drive letters, and
  UNC prefixes are ordinary text; Windows path syntax is unsupported.
- Untrusted path components must be validated by the application before the
  result is passed to a file or directory builtin. The helpers add no
  privileges and remove none.

## Text processing boundary

- `split`, `join`, `trim`, `starts_with`, `ends_with`, and `replace` operate
  only on in-memory strings; they do not touch the filesystem or network.
- `split`, `join`, and `replace` allocate results proportional to their input
  and output sizes. There is no streaming API or output-size cap, so a very
  large input or replacement can exhaust memory. Native allocation failures
  terminate with exit 2 rather than continuing.
- `trim` removes exactly the Unicode `White_Space` set. It does not remove
  zero-width or format characters, and it is not a security sanitizer.
- `replace` is literal and non-overlapping; an empty pattern raises `R0016`.
  There is no regular-expression engine, case conversion, Unicode
  normalization, locale handling, or grapheme-cluster API. Do not assume that
  these builtins normalize or validate user text for a downstream system.
- Native text results are immutable and intentionally leaked like other native
  strings in v0.1.

## AI network and credential boundary

- Remote AI requests require explicit `--allow-network`; loopback HTTP is local.
- API keys are read only from the named environment variable and are never
  written to project files, metrics, diagnostics, or evidence. Live-verification
  artifacts record only the environment-variable name.
- HTTPS uses the system `curl`. Curl exit 28 (operation timeout) is classified
  as retryable `AI_E_TIMEOUT`; DNS and connect failures remain
  `AI_E_CONNECT`.
- The live verification on 2026-11-24 used a real cloud provider. An earlier
  retry was rejected with HTTP 400 `User location is not supported for the API
  use.`; a later re-verification at 20:59Z passed all 21 checks after one
  transient HTTP 503. No bypass is attempted, and continuous live cloud access
  is not guaranteed.
- The 401/429/timeout paths are exercised by `tools/ai_failure_harness.py`
  against loopback servers with a placeholder key.

## CLI boundary

- `vantyx build` writes generated C to a temporary file and invokes the C compiler with argument vectors (no shell interpolation).
- `VANTYX_CC` selects the compiler executable; it is trusted input from the environment.
- Output paths are taken from the user; `vantyx` does not sandbox filesystem access.

## Supply chain

- No crates are used by the compiler or runtime.
- A future dependency (for example Cranelift/LLVM bindings) requires an ADR covering necessity, license, maintenance, and supply-chain risk (ADR-0007).
- The C compiler and system libc are external trusted tools.

## Reporting

Security findings should include:

1. Vantyx source or CLI invocation that reproduces the issue.
2. Observed behavior and expected behavior.
3. Whether the issue affects the compiler, interpreter, native backend, or tooling.
4. Severity and a minimal fix suggestion.

Do not claim a sandbox, memory-safety guarantee, or FFI safety property that has not been implemented and tested.
