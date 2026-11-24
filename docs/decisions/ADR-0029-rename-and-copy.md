# ADR-0029 — Rename and file-copy builtins

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0026 (basic file I/O),
  ADR-0027 (directory operations), `docs/language_specification.md`

## Context

The file and directory builtins can create, read, write, list, and remove
files, but a program still cannot move a file under a new name or copy one
without reading it into a Vantyx string. Reading into a string is unsuitable
for binary data, loses NUL bytes, rejects invalid UTF-8, and caps the operation
at 64 MiB.

The alternatives were:

1. Wait for `Result`/`?`, path types, and a module system. Better long-term
   organization, but it leaves ordinary file management out of the language.
2. Expose only `rename` and require callers to read/write for copies. Rename is
   useful, but copying binary fixtures, logs, and generated artifacts remains
   awkward and lossy.
3. Add two small builtins that follow the existing fatal-error model.

## Decision

Add two synchronous builtins:

| Builtin | Signature | Behavior |
| --- | --- | --- |
| `rename` | `(Str, Str) -> Unit` | Rename or move one filesystem entry |
| `copy_file` | `(Str, Str) -> Unit` | Copy bytes from one regular file to another |

1. **No new syntax or module.** They are statically known builtin names and
   cannot be shadowed, like the other file and directory builtins.
2. **`rename` delegates to the host.** It accepts files and directories, does
   not create parent directories, and does not perform a recursive move.
   Overwrite behavior follows the host: on POSIX, renaming within one
   filesystem atomically replaces an existing file target, while replacing a
   non-empty directory or crossing filesystems fails. Windows behavior is not
   verified.
3. **`copy_file` copies bytes only.** The source must be a regular file after
   following symbolic links. The target is created or truncated. Permissions,
   ownership, timestamps, extended attributes, ACLs, and hard-link identity are
   not preserved. The copied bytes are not validated as UTF-8 or checked for
   NUL, so `copy_file` can copy data that `read_file` would reject; the copy is
   a filesystem operation, not a string operation.
4. **No self-copy protection.** Source and target must be distinct paths. A
   caller that passes the same path can truncate the source before any bytes
   are read; this is documented rather than detected because path aliases,
   hard links, and symbolic links make reliable detection host-specific.
5. **Fatal errors and no sandbox.** Missing paths, permission failures,
   cross-device renames, non-regular copy sources, target kind conflicts, and
   I/O failures raise `R0015` and exit with status 2. Paths use the process's
   privileges and working directory. Check-then-act sequences remain subject
   to TOCTOU races.
6. **Copies are not atomic.** A failure can leave a partially written target.
   `rename` atomicity is whatever the host provides; the language does not add
   a transaction or fsync.
7. **Interpreter is the oracle.** The interpreter uses `std::fs::rename` and a
   byte copy built on `std::fs::File` and `std::io::copy`. The native backend
   uses POSIX `rename`, `stat`/`S_ISREG`, and `fopen`/`fread`/`fwrite`/
   `fflush`/`fclose`. Both reject statically non-`Str` arguments in the checker
   and reject dynamically typed arguments with `E0401` at the native boundary.
8. **Evaluation order is left to right.** Multi-argument calls are emitted as
   GNU C statement expressions that evaluate the source before the target,
   matching the interpreter.
9. **No dependency.** The native additions use headers already required by the
   generated program. No archive format, glob, recursive copy, metadata API, or
   platform-specific abstraction is introduced.

## Consequences

Positive:

- Programs can rename files and directories, move generated output, and copy
  regular files in both engines.
- Binary copies are byte-exact and do not depend on the string UTF-8/NUL
  invariant or the 64 MiB read cap.
- The error model and native boundary stay consistent with the existing file
  and directory builtins.

Negative:

- Errors remain fatal and uncatchable until `Result`/`?` exists.
- There is no recursive copy, recursive move, glob, metadata preservation,
  atomic copy, or progress reporting.
- `copy_file` reads and writes through libc buffers rather than a zero-copy
  kernel path in the native backend.
- Large files are unbounded by a language-level cap; disk space and host limits
  apply.
- The generated native program now depends on POSIX `rename`, `stat`, and
  `S_ISREG`; Windows native support remains unverified.
- There is still no sandbox. Running untrusted Vantyx source can rename and
  copy any path the process can access.

## Verification

- `tests/e2e.rs`: rename/copy round trips, target truncation, directory rename,
  cleanup, byte-preserving binary copy, static type errors, dynamic `R0005`
  type errors for `Any` paths, and fatal `R0015` cases.
- `tests/native.rs`: differential round trips, binary copies, error cases,
  left-to-right argument evaluation, and `E0401` rejection of dynamically typed
  paths.
- The new native tests pass under `-fsanitize=address,undefined` with leak
  detection disabled for intentional v0.1 leaks.
- Full gate and test counts are recorded in `TASKS.md` and `PROJECT_STATUS.md`.
