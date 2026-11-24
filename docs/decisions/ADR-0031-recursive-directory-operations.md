# ADR-0031: Recursive directory creation and removal

- Status: Accepted; amended by ADR-0035 (destructive-root refusal)
- Date: 2026-11-24
- Related: ADR-0007 (zero dependencies), ADR-0027 (single-level directory
  operations), ADR-0035, T-059, T-063, T-069

## Context

T-059 added `make_dir`, `remove_file`, `remove_dir`, and `list_dir`, but each
directory call operates on exactly one level. Creating `logs/2026/11`, clearing
a build tree, or cleaning a temporary workspace therefore requires a manual
recursive traversal that Vantyx cannot yet express conveniently. The
interpreter can use `std::fs`, and the native backend links libc, so both
engines can provide recursive operations without a dependency.

The existing directory builtins report failures as fatal `R0015` errors. Vantyx
still has no `Result` type, and introducing a second error convention for the
same family of operations would make programs inconsistent.

## Decision

Add two builtins:

```text
make_dir_all(path: Str) -> Unit
remove_dir_all(path: Str) -> Unit
```

Both are fatal `R0015` operations, require a `Str` path, reject a NUL byte, and
resolve relative paths against the process working directory. Neither changes
the working directory, follows symlinks while deleting, preserves metadata, or
provides a sandbox.

### `make_dir_all`

- Creates the final directory and every missing parent.
- Succeeds when the final path already exists as a directory, including when it
  is a symbolic link to a directory. This follows the same `mkdir` +
  directory-check behavior as the host.
- An empty path is a successful no-op. This matches the observed
  `std::fs::create_dir_all("")` behavior and keeps the native and interpreter
  engines aligned.
- Fails with `R0015` when an existing non-directory component blocks creation,
  when the final path exists as a regular file or other non-directory, or when
  permissions or another host condition deny creation.
- Does not require or create a trailing separator. Trailing and repeated
  separators are accepted by the host filesystem.
- A failure can leave some parents created; the operation is not atomic.

### `remove_dir_all`

- Removes the target directory, all descendants, and every entry inside it.
- Symbolic links are unlinked, never traversed. A symbolic link passed as the
  root path is unlinked and its target is left untouched. Trailing separators
  are trimmed before the root type check, so `remove_dir_all("link/")` also
  unlinks the link instead of following it. This deliberately avoids the
  host behavior that can traverse a symlinked directory when the root has a
  trailing separator. ADR-0035 additionally refuses a final `.` or `..`
  component, so `link/.` and `link/..` are rejected before any access.
- Fails with `R0015` when the root is a regular file or other non-directory,
  when the path does not exist, or when a descendant cannot be removed.
- Refuses `.`, `./`, `..`, `../`, any root whose final component is `.` or
  `..`, and the filesystem root `/` (including repeated separators) with
  `R0015` before touching the filesystem (ADR-0035).
- An empty path fails with `R0015` (the host reports a missing path).
- A failure can leave a partially removed tree; the operation is not atomic and
  does not attempt rollback.
- The function remains destructive by design, has no confirmation prompt, no
  depth limit, no entry-count limit, and no sandbox. Intermediate symbolic
  links and untrusted paths still require caller validation.

## Type and evaluation rules

- `make_dir_all` and `remove_dir_all` each take exactly one `Str` argument.
- A statically non-`Str` argument is rejected with `E0301`.
- A dynamically typed interpreter argument raises `R0005`.
- The native backend rejects a dynamically typed path with `E0401` because
  native strings are statically typed.
- No new error code is added.

## Implementation

- `src/checker.rs`: register both builtins and add the one-argument `Str` check.
- `src/interp.rs`: `std::fs::create_dir_all` and `std::fs::remove_dir_all` with
  NUL rejection, destructive-root refusal (ADR-0035), and `R0015` mapping.
- `src/fs_safety.rs`: lexical refusal classifier and unit tests (ADR-0035).
- `src/ir.rs`: require `IrType::Str` and return `IrType::Unit`.
- `src/codegen_c.rs`: `vantyx_make_dir_all` (component-by-component `mkdir` with
  `stat` confirmation), `vantyx_remove_root_refusal`, and
  `vantyx_remove_dir_all` (`lstat`, `opendir`/`readdir`, `unlink`, and `rmdir`,
  never following symbolic links).
- ADR, specification, status, task, and README updates in the same change.

## Consequences

- File-tree setup and cleanup become possible in both engines without a
  library-level recursion helper.
- The native C runtime grows two POSIX helpers and continues to depend on
  `<sys/stat.h>`, `<dirent.h>`, and `<unistd.h>`.
- Recursive removal is a high-consequence destructive operation. There is no
  prompt, dry run, trash, rollback, or atomic rename staging. A partial failure
  leaves a partially removed tree.
- Both operations are subject to check-then-act races and to host permissions.
  Neither is a security boundary.
- Only a direct trailing separator is stripped before the root symlink check.
  ADR-0035 refuses final `.`/`..` components and the filesystem root before
  any access, but paths such as `link/sub` can still reach through a
  symbolic link, so callers must validate untrusted roots.
- `remove_dir_all(".")` is refused before any deletion by ADR-0035. The
  previous empty-the-working-directory behavior was removed and its
  differential test replaced with a preservation test.
- Deep trees consume the native C call stack during removal. There is no
  explicit depth limit; extremely deep trees can exhaust the stack.
- Windows native support remains unverified.

## Alternatives considered

1. **Add a `recursive: Bool` parameter to `make_dir`/`remove_dir`.** Rejected:
   it changes existing arity and makes dangerous behavior a flag on a safe
   operation.
2. **Expose a `Path` or filesystem object with methods.** Rejected for v0.1:
   the language has no objects or methods, and a nominal `Path` type would be a
   larger type-system change than this task requires.
3. **Return a `Result`.** Deferred: `Result` and `?` require a generic or
   specially typed error model that does not exist yet. Fatal `R0015` keeps the
   directory family consistent until that design lands.
4. **Use an external `rm -rf`/`mkdir -p` subprocess.** Rejected: it adds a
   command-injection surface, depends on a shell, and is slower.

## Reversibility

High. Both builtins are isolated dispatch entries and two C helpers. A future
`Result`-returning or `Path`-based API can be added without changing the
existing single-level builtins, and these names can remain as fatal shortcuts.
