# ADR-0055: Atomic whole-file replacement

- Status: Accepted
- Date: 2026-11-27
- Related: T-116, ADR-0026, ADR-0051

## Context

ADR-0026 added `write_file` and `append_file` as small synchronous builtins.
`write_file` opens the target with truncation and then writes; `append_file`
opens it for append. Both expose partial files when a write fails, and the
specification and security notes explicitly warned that writes are not atomic.

Applications that rewrite configuration, generated source, or data files need
whole-file replacement: a concurrent reader must observe either the previous
contents or the complete new contents. Changing `write_file` itself would
change existing behavior in three important ways: it follows symbolic links,
it preserves the target inode and permissions, and a failed write leaves the
original contents gone. Those are useful for append and in-place update, so a
separate builtin is the smaller and safer change.

## Decision

Add `write_file_atomic(path: Str, text: Str) -> Unit` in both engines:

1. Validate that the path and contents are `Str` and reject NUL bytes with
   `R0015`, matching `write_file`.
2. Choose the target's parent directory. An empty parent or a path made only
   of separators uses the current directory, matching Rust's `Path::parent`.
3. Create a hidden `.vantyx-tmp-<pid>-<serial>` file in that directory with
   exclusive creation. Retry a bounded 100 times on name collisions.
4. Write all bytes, flush, and `fsync` the temporary file.
5. Close the temporary and rename it over the target. `rename` is the atomic
   commit point for readers.
6. On any failure after temporary creation, unlink the temporary and raise
   `R0015` with `cannot write file \`path\`: <os error>`.
7. Evaluate the path and contents left to right exactly once.

The interpreter uses `std::fs::OpenOptions::create_new`, `write_all`,
`sync_all`, and `std::fs::rename`. The native backend uses
`open(O_WRONLY|O_CREAT|O_EXCL)`, a `write` loop, `fsync`, and POSIX `rename`,
with the same temporary-directory rule and error wording. The interpreter
remains the semantic oracle.

## Guarantees and limits

- **Atomic for readers.** The target directory entry switches from the old file
  to the new one at `rename`; a reader never sees a partially written target.
- **Not crash durability.** The file is fsynced, but the containing directory is
  not, so a system crash can lose the rename even though the new file was
  flushed. A crash during the write can leave a hidden `.vantyx-tmp-*` file.
- **Symlinks are replaced, not followed.** `write_file_atomic("link", text)`
  replaces the link itself and leaves its former target unchanged. This is the
  normal POSIX `rename` behavior and differs from `write_file`.
- **Hard links are detached.** Other names for the old inode keep the old
  contents; the target name points at the new inode.
- **Metadata is not preserved.** The replacement receives new-file permissions
  (subject to the process umask), owner, group, and timestamps. It does not copy
  the old mode, owner, ACLs, extended attributes, or timestamps.
- **Same filesystem for ordinary paths.** The temporary is created in the
  target's parent directory, so the rename does not cross filesystems. Paths
  with no parent use the working directory; a target such as `/` can therefore
  fail with `EXDEV` or `EISDIR`, matching the interpreter.
- **Temporary names are predictable.** They contain the process id and a
  process-local serial. They are an implementation detail, not a security
  boundary; the directory's permissions still control who can interfere.
- **Native scope.** The native helper relies on POSIX file descriptors and
  `rename`, consistent with the rest of the native filesystem backend. Native
  Windows support remains unverified.

`write_file` and `append_file` keep their existing in-place behavior and are
still documented as non-atomic. The new builtin is not a sandbox, an atomic
append, a streaming API, or a metadata-preserving replacement.

## Alternatives considered

- **Make `write_file` atomic.** Rejected: it would stop following symbolic
  links, replace inodes and permissions, and change failure behavior for every
  existing caller. A separate opt-in name keeps the old contract.
- **`O_TMPFILE` plus `linkat`/`renameat`.** This avoids a visible temporary name
  on Linux but is not portable to the other POSIX hosts the native backend
  targets, and it adds a second platform-specific path. Rejected for v0.1.
- **Write a temporary and copy it over the target.** This still exposes a
  truncated or partially overwritten target and doubles the I/O. Rejected.
- **Skip `fsync`.** Rename would still be atomic for readers, but a crash could
  leave an empty or partial file under the new name. The fsync cost is accepted
  for a whole-file replacement operation.
- **Fsync the directory too.** This would strengthen crash durability, but
  needs platform-specific directory opening and adds another failure point.
  Deferred; the specification states the weaker guarantee.
- **Preserve mode and ownership.** This requires an extra `stat`, `fchmod`,
  and possibly `fchown`, introduces races, and conflicts with the v0.1 decision
  not to expose or preserve metadata. Rejected for now.

## Consequences

- Callers that need readers to see a consistent file can use
  `write_file_atomic` instead of truncating in place.
- Each replacement creates and fsyncs one temporary file, so it is more
  expensive than `write_file`; this is a correctness tradeoff, not a performance
  claim.
- Permission, ownership, timestamp, and hard-link identity changes are visible
  and must be documented for callers that rely on them.
- The implementation adds one runtime helper to generated C and no new Rust
  dependency.
- A crash can leave a hidden temporary file; there is no startup scavenger.

## Tests

- `tests/e2e.rs::file_io_atomic_write_replaces_contents_and_cleans_up` covers
  creation, replacement, empty contents, nested directories, and temporary-file
  cleanup.
- `tests/e2e.rs::file_io_atomic_write_replaces_links_without_following` covers
  symbolic-link replacement, dangling links, hard links, and a read-only
  target.
- `tests/e2e.rs::file_io_atomic_write_errors_are_fatal_and_clean_up` covers a
  missing parent, a directory target, and an empty path, and asserts no
  temporary remains.
- `tests/e2e.rs::file_io_atomic_write_rejects_non_str_and_wrong_arity` covers
  `E0301` and `E0302`.
- `tests/e2e.rs::file_io_atomic_write_dynamic_non_str_fails_at_runtime` covers
  `R0005` for a dynamic non-`Str` path.
- `tests/e2e.rs::file_io_atomic_write_evaluates_arguments_left_to_right` pins
  evaluation order.
- `tests/native.rs::atomic_file_io_matches`,
  `atomic_file_io_errors_match`, `atomic_file_io_links_match`,
  `atomic_file_io_any_path_is_rejected_with_e0401`, and
  `atomic_file_io_evaluates_arguments_left_to_right` compare interpreter and
  native output, exit codes, first-line errors, link behavior, and the `E0401`
  boundary.
- The full native suite runs with clang, GCC, and the AddressSanitizer/
  UndefinedBehaviorSanitizer wrapper (leak detection disabled because v0.1
  intentionally leaks native allocations).
