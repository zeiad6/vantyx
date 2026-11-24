# ADR-0026 — Basic UTF-8 file I/O builtins

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0010 (native maps),
  ADR-0015 (AI host foundation), ADR-0025 (MMR diversity reranking),
  `docs/language_specification.md`

## Context

Vantyx could compute with values and print them, but a program could not read
or write a file. That blocked ordinary scripts (configuration, fixtures, logs,
generated source) and forced every file interaction into the Rust host or the
AI CLI. The AI host already reads and writes files on its own, so the language
runtime was the remaining gap.

The available alternatives were:

1. Keep the language pure and require host programs for all file access. Safe,
   but it permanently excludes script-level file processing.
2. Add a full standard-library module system with namespaces, `Result`, and
   resource types first. This is the better long-term design, but it depends on
   generics, `Result`/`?`, and a module packaging story that do not exist yet.
3. Add a small set of globally available builtins with runtime errors, matching
   the existing `assert`, `int`, and collection builtins. Limited, but usable
   immediately and implementable with identical interpreter/native semantics.

## Decision

Add four synchronous builtins to the v0.1 language:

| Builtin | Signature | Behavior |
| --- | --- | --- |
| `read_file` | `(Str) -> Str` | Read the whole file as UTF-8 |
| `write_file` | `(Str, Str) -> Unit` | Create or truncate, then write |
| `append_file` | `(Str, Str) -> Unit` | Create if missing, then append |
| `file_exists` | `(Str) -> Bool` | Whether `stat` succeeds |

1. **No new syntax or module.** They are statically known builtin names, like
   `assert`, and cannot be shadowed.
2. **Host paths, no sandbox.** Paths are passed to the host operating system
   with the process's privileges. Relative paths resolve against the process
   working directory, not the source file. `write_file` and `append_file` are
   not atomic and can leave a partial file if an I/O error occurs mid-write.
3. **UTF-8 and NUL invariants.** Vantyx strings are UTF-8 and cannot contain NUL
   bytes. `read_file` rejects invalid UTF-8 and NUL contents instead of
   returning a lossy or truncated string. `write_file` and `append_file` reject
   NUL in the path or contents before touching the filesystem.
4. **Bounded reads.** `read_file` refuses files larger than 64 MiB
   (`MAX_FILE_BYTES`). The native backend checks while reading and stops at the
   limit plus one byte; the interpreter checks metadata first and also caps the
   read. This bounds accidental memory growth while keeping ordinary source and
   data files usable.
5. **Runtime errors, not `Result`.** Missing files, permission failures,
   directories opened for reading, invalid UTF-8, NUL bytes, and oversized files
   raise `R0015` and exit with status 2, like other unrecoverable builtin
   errors. `file_exists` is the recoverable probe: it returns `false` for any
   path that cannot be inspected, including permission errors.
6. **Interpreter is the oracle.** The interpreter uses `std::fs`; the native
   backend uses libc `fopen`/`fread`/`fwrite`/`fclose` and `stat`. Both produce
   the same exit code, stdout, and `R0015` class. The native C runtime validates
   UTF-8 in C and does not add a dependency. Statically non-`Str` arguments are
   compile-time errors in the checker and `E0401` at the native boundary when
   the value is dynamically typed.
7. **Evaluation order is left to right.** `write_file(path(), text())` evaluates
   and prints from `path()` before calling `text()`, matching the rest of the
   expression evaluator and the generated C statement expression.

## Consequences

Positive:

- Programs can read fixtures, write reports, append logs, and check for files
  in both the interpreter and native builds.
- The string invariant is preserved: no NUL-containing or invalid-UTF-8 value
  can enter the runtime.
- No dependency, no new persistence format, and no change to the native link
  boundary.

Negative:

- Errors are fatal and uncatchable until `Result`/`?` exists. Callers must probe
  with `file_exists` or restart with a corrected path.
- `file_exists` followed by `read_file` is a time-of-check/time-of-use race.
- There is no directory listing, rename, remove, metadata, glob, stdin/stdout
  stream, or non-blocking I/O.
- Writes are not atomic and files can be partially written on error.
- The 64 MiB cap is a fixed policy; larger files require a host program or a
  future streaming API.
- There is still no sandbox. Running untrusted Vantyx source can read and write
  any file the process can access.

## Verification

- Checker/IR tests are covered through the e2e compile-error test and the
  native differential tests.
- `tests/e2e.rs`: round trip with truncation, append, `read_file`,
  `file_exists` for a file, a missing path, and a directory; invalid UTF-8 and
  NUL rejection; static argument-type errors.
- `tests/native.rs`: the same round trip and error cases under both engines;
  invalid UTF-8 sequences (bad lead, overlong, surrogate, out-of-range,
  truncated); 64 MiB plus one size-limit rejection; missing-file and
  unwritable-path errors; left-to-right argument evaluation; sanitizer run
  with `-fsanitize=address,undefined`.
- Full gate and test counts are recorded in `TASKS.md` and `PROJECT_STATUS.md`.
