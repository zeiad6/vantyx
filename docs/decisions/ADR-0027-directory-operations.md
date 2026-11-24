# ADR-0027 — Directory operations builtins

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0026 (basic file I/O),
  `docs/language_specification.md`

## Context

ADR-0026 added whole-file reads and writes, but a program still could not
create, inspect, or remove directories. That blocks ordinary project scripts:
generating an output tree, iterating fixtures, cleaning temporary state, and
checking whether a directory exists before writing into it.

The alternatives were:

1. Wait for a module system, `Result`/`?`, and path types before adding any
   filesystem surface. Better long-term ergonomics, but it leaves the language
   unable to manage directories indefinitely.
2. Add a hierarchical standard-library module immediately. This depends on
   namespaces and packaging that do not exist yet.
3. Extend the existing global builtin slice with a small, explicit set of
   directory operations that follow the same fatal-error contract as file I/O.

## Decision

Add four synchronous builtins:

| Builtin | Signature | Behavior |
| --- | --- | --- |
| `list_dir` | `(Str) -> List[Str]` | Return entry names, sorted by UTF-8 bytes |
| `make_dir` | `(Str) -> Unit` | Create one directory; an existing path is an error |
| `remove_file` | `(Str) -> Unit` | Remove a file or symbolic link, never a directory |
| `remove_dir` | `(Str) -> Unit` | Remove one empty directory |

1. **No new syntax or module.** They are statically known builtin names and
   cannot be shadowed, like the file I/O builtins.
2. **`list_dir` is deterministic.** `.` and `..` are omitted, names are
   returned without their parent path, and the result is sorted by byte order.
   Sorting matches between the interpreter and native backend regardless of the
   host's directory order.
3. **UTF-8 invariant.** Entry names must be valid UTF-8. A directory containing
   a non-UTF-8 name makes `list_dir` fail with `R0015` rather than returning a
   lossy name that could not be passed back to another builtin.
4. **Single-level operations.** `make_dir` does not create parents (`mkdir`
   semantics), `remove_dir` refuses non-empty directories, and `remove_file`
   refuses directories. Recursive creation and removal are deliberately out of
   scope; callers compose the single-level operations.
5. **Host paths, no sandbox.** Paths resolve against the process working
   directory with the process's privileges. Removing a symbolic link removes the
   link, not its target. Check-then-act sequences remain subject to TOCTOU
   races.
6. **Fatal errors.** Missing paths, type mismatches, permission failures,
   non-empty directories, invalid UTF-8 names, and I/O errors raise `R0015` and
   exit with status 2. `file_exists` remains the only non-failing probe.
7. **Interpreter is the oracle.** The interpreter uses `std::fs::read_dir`,
   `create_dir`, `remove_file`, and `remove_dir`. The native backend uses POSIX
   `opendir`/`readdir`/`closedir`, `mkdir`, `unlink`, and `rmdir`, validates names
   with the existing C UTF-8 validator, and sorts with `qsort`/`strcmp`.
   Statically non-`Str` arguments are checker errors and `E0401` at the native
   boundary for dynamically typed values.
8. **Evaluation order is left to right.** Arguments are evaluated before the
   call, matching the rest of the expression evaluator.
9. **No dependency.** The native additions are libc/POSIX headers already
   present on the supported GNU C11 toolchain. Generated programs now require
   `<dirent.h>` and `<unistd.h>`; the backend already required a POSIX host for
   `stat`.

## Consequences

Positive:

- Scripts can create and clean output trees, enumerate fixtures, and manage
  temporary directories in both engines.
- `list_dir` output is stable across hosts and safe to feed back into path
  builtins.
- The native contract stays honest: unsupported or dynamically typed arguments
  are rejected instead of silently miscompiled.

Negative:

- Errors remain fatal and uncatchable until `Result`/`?` exists.
- A directory with millions of entries is read entirely into memory; there is
  no streaming iterator or entry-count limit beyond available memory.
- There is no recursive copy/remove, glob, or metadata mutation in this decision.
  T-064 later added read-only metadata and symbolic-link probes (ADR-0032),
  and T-065 added lexical glob expansion (ADR-0033).
- There is no rename,
  current-directory change, or atomic operation.
- `list_dir` and `file_exists` results can go stale immediately; there is no
  sandbox and no capability model for untrusted source.
- The generated C is now more explicitly POSIX-dependent; Windows native
  support remains unverified and out of scope for v0.1.

## Verification

- `tests/e2e.rs`: round trip with nested creation, deterministic listing,
  removal, and existence checks; static argument-type errors; fatal `R0015`
  cases for missing paths, existing directories, wrong file/directory kinds,
  and non-empty directories; non-UTF-8 entry rejection on Unix.
- `tests/native.rs`: the same round trip and error matrix under both engines,
  plus non-UTF-8 entry rejection, with identical stdout and exit codes.
- The directory native tests pass under `-fsanitize=address,undefined` with leak
  detection disabled for intentional v0.1 leaks.
- Full gate and test counts are recorded in `TASKS.md` and `PROJECT_STATUS.md`.
