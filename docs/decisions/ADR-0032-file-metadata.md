# ADR-0032: File metadata probes

- Status: Accepted
- Date: 2026-11-24
- Related: ADR-0007 (zero dependencies), ADR-0026 (file I/O), ADR-0027
  (directory operations), T-058, T-059, T-064

## Context

Vantyx can read, write, list, create, remove, rename, and copy paths, but it
cannot ask basic questions about a path before acting on it. Applications need
to know whether a path is a regular file, whether it is a directory, whether it
is a symbolic link, and how large a file is. Without these probes, callers must
guess from operation failures or reimplement host checks outside Vantyx.

The file and directory builtins use fatal `R0015` errors and follow symbolic
links for `file_exists` and `list_dir`. Vantyx has no `Result` type, no
metadata object, and no plan for one in v0.1. The probes should fit the existing
error model rather than introduce a second one.

## Decision

Add four builtins:

```text
file_size(path: Str) -> Int
is_file(path: Str) -> Bool
is_dir(path: Str) -> Bool
is_symlink(path: Str) -> Bool
```

All four require a `Str` path, resolve relative paths against the process
working directory, and are rejected with `E0401` in native code when the path
is dynamically typed. Dynamic interpreter values are `R0005`. The interpreter
rejects a path containing a NUL byte with `R0015`; native strings cannot contain
NUL, so a native string literal with a NUL byte is already rejected with
`E0401` by the IR string invariant. They add no dependency and change no
metadata.

### `file_size`

- Follows symbolic links and requires an existing regular file, using the same
  regular-file rule as `copy_file`.
- Returns the host-reported logical size in bytes (`st_size`). It is not the
  allocated block count, the recursive size of a directory, or the number of
  UTF-8 scalar values.
- Raises `R0015` when metadata cannot be read (missing path, permission error,
  I/O error), when the target is not a regular file (for example a directory or
  device), when the path contains NUL in the interpreter, or when the size
  cannot be represented as `Int`. A dangling symbolic link fails with `R0015`
  because the followed target does not exist.

### `is_file` and `is_dir`

- Follow symbolic links, matching `file_exists`.
- Return `false` when metadata cannot be read, including missing paths,
  dangling links, permission errors, and other host failures.
- A broken link reports `false` for both predicates even though `is_symlink`
  reports `true`.

### `is_symlink`

- Uses `lstat` semantics and returns `true` only when the path itself is a
  symbolic link. A link to a file, a link to a directory, and a dangling link
  all return `true`.
- Returns `false` for regular files, directories, missing paths, and
  permission failures.

## Consequences

- Callers can write safe check-then-act sequences without a `Result` type, but
  the checks are not atomic. A path can change between a probe and the next
  operation, so none of these builtins is a security boundary.
- Predicates intentionally do not distinguish "missing" from "permission
  denied"; both return `false`. Use `file_size` or a failing operation when the
  reason matters.
- No mode, owner, group, timestamp, ACL, extended attribute, inode, device,
  birthtime, or file-type bitmask is exposed. There is no option to disable
  symlink following, no metadata mutation, and no metadata preservation.
- `file_size` on a sparse file reports the logical length, which can exceed the
  allocated storage. On very large files, the interpreter rejects sizes above
  `Int::MAX`; the native backend returns the host `st_size` as `int64_t`.
- Windows native support remains unverified; the C helpers use POSIX
  `stat`/`lstat` and `S_ISREG`/`S_ISDIR`/`S_ISLNK`.

## Alternatives considered

- Return `-1` for missing sizes. Rejected: it conflates a valid-looking number
  with an error and makes size arithmetic silently wrong.
- Return a metadata struct or map. Rejected for v0.1: the language has no
  struct return convention for builtins and no `Result`, and a struct would
  expose host-specific fields that are not portable.
- Make predicates raise `R0015` on missing paths. Rejected: predicates are most
  useful as total questions, and `file_exists` already returns `false`.
- Add a `follow` flag to every builtin. Rejected: it complicates the common
  case and the native boundary; `is_symlink` plus the two follow-based
  predicates cover the practical distinctions.
