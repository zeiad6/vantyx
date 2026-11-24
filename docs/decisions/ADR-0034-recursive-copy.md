# ADR-0034: Recursive directory copy

- Status: Accepted
- Date: 2026-11-24
- Related: ADR-0007 (zero dependencies), ADR-0029 (rename and byte-preserving
  copy), ADR-0031 (recursive directories), ADR-0033 (glob), T-066

## Context

Vantyx can copy one regular file with `copy_file`, but copying a directory tree
requires a Vantyx-level recursive walk. That walk cannot reliably reproduce
raw entry names, reject special files, or enforce one shared resource bound.
Shelling out to `cp -R` would add a host-tool dependency and make overwrite,
symbolic-link, and error behavior platform-defined. A recursive copy builtin
keeps the interpreter and native backend comparable.

The existing filesystem builtins are synchronous, fatal (`R0015`), unsandboxed,
and do not preserve metadata. `copy_file` requires a regular source file and
creates or truncates its target, so it cannot express "copy this tree into a
new directory without merging". The design must decide target existence,
containment, symbolic-link policy, entry ordering, encoding, failure recovery,
and resource caps precisely enough for both engines to agree.

## Decision

Add one builtin:

```text
copy_dir_all(from: Str, to: Str) -> Unit
```

`from` is the source path and `to` is the destination path. Both are required
`Str` values. The builtin copies the source tree into a new target directory,
does not preserve metadata, and adds no dependency.

### Source and target

- The source must resolve, following symbolic links in every component
  including the final one, to an existing directory. A missing path, a dangling
  link, a non-directory, or an unreadable source is `R0015`.
- The target must not exist, including as a dangling symbolic link. Its final
  name must be a real name: after trimming trailing `/`, an empty name, `.`, or
  `..` is rejected with `R0015`. The target's parent must already exist and
  resolve to a directory; missing parents are not created. An existing target is
  not overwritten or merged.
- The resolved target must not equal or lie inside the resolved source tree.
  The parent is canonicalized and the final target name is appended before the
  component-boundary containment check. This rejects `copy_dir_all("a",
  "a/b")`, `copy_dir_all("a", "a/../a/b")`, and a target whose parent is a
  symbolic link into the source. Copying to an unrelated sibling such as
  `a_backup` is allowed.
- The target directory is created once before traversal. If any later step
  fails, the partial target is left in place. There is no rollback, staging
  directory, atomic rename, overwrite mode, merge mode, dry run, or progress
  reporting.
- Relative paths resolve against the process working directory. There is no
  sandbox and no confinement to a project root. Check-then-act races between
  inspection, target creation, and traversal remain possible.

### Traversal and file policy

- Directory entries beneath the resolved source root are copied recursively in
  host `readdir` order, which is not specified. Results are not sorted.
- A regular file is copied byte for byte into a newly created target file.
  Existing bytes are never reused because the target tree is new. Contents are
  not validated as UTF-8 or NUL-free.
- A directory is created in the target with host default permissions before
  its children are copied. An empty source directory produces an empty target
  directory.
- A symbolic link encountered below the source root is rejected with `R0015`;
  it is never followed, copied as a link, or skipped. Symbolic links in the
  source path prefix (and a final source link) are followed during source
  resolution.
- Sockets, FIFOs, device nodes, and every other non-regular, non-directory
  entry are rejected with `R0015`. The target can contain the entries copied
  before the rejection.
- Entry names are copied as raw host bytes. Unlike `list_dir` and `glob`,
  non-UTF-8 names below the source root are supported on Unix. A NUL byte
  cannot occur in a host file name.
- Permissions, ownership, group, timestamps, extended attributes, ACLs, sparse
  hole layout, and hard-link identity are not preserved. A copied executable
  file uses the host's default file mode subject to the process umask, not the
  source mode.

### Errors and limits

- Every filesystem failure, invalid source or target shape, containment
  violation, unsupported entry kind, and limit overrun is fatal `R0015`.
- Statically non-`Str` arguments are `E0301`. A dynamic interpreter value of
  another type is `R0005`; a native dynamically typed argument is rejected at
  the IR boundary with `E0401`. A NUL byte in an interpreter path is `R0015`;
  native strings cannot contain NUL, so a NUL literal is already rejected with
  `E0401`.
- A tree with more than 100,000 entries is `R0015`. The count includes files,
  directories, symbolic links, and special files, and increments before an
  entry is created or its type is rejected. The 100,001st entry fails the copy.
- A directory that would exceed 128 directory levels below the source root is
  rejected with `R0015` before that directory is created. The source root
  itself is level 1; its immediate child directories are level 2.
- Both caps are fixed. No per-file size limit is added; a regular file is
  streamed rather than loaded as one buffer.

## Consequences

- `copy_dir_all` supports backup and fixture workflows without a Vantyx-level
  walk or a host `cp` invocation. Both engines share the target rule, symlink
  policy, raw-name support, failure ordering, and caps.
- The builtin is conservative: it never merges into existing data and never
  guesses whether a link or special file should be followed or skipped.
  Callers that want those policies must build them explicitly.
- A failed copy consumes time and disk space and leaves a partial target that
  the caller must remove. There is no cancellation.
- The tree is inspected with host privileges and can read any path the process
  can read. The containment check only prevents copying a tree into itself; it
  is not a sandbox. Concurrent renames or link swaps can invalidate the check
  and redirect the walk.
- The entry cap bounds the number of entries but not total bytes, and the
  depth cap bounds recursion for this walker only. A source tree can still
  consume disk space proportional to its contents.
- Native support uses POSIX `stat`/`realpath`/`opendir`/`readdir`/`lstat`/
  `mkdir` and libc streams. Windows native support remains unverified.

## Alternatives considered

- Add a recursive mode to `copy_file`. Rejected: silently changing a
  regular-file builtin into a tree copier would break existing target
  truncation semantics and blur the type contract.
- Merge into an existing target like `cp -R`. Rejected: overwrite and partial
  merge behavior would make failures destructive and engine agreement harder;
  callers can remove or choose a new target explicitly.
- Copy symbolic links as links. Rejected for v0.1: it needs a second copy path
  and platform-specific link creation, and dangling links complicate error
  reporting.
- Skip symbolic links and special files silently. Rejected: silent omission
  would make the copied tree look complete when it is not.
- Preserve metadata like `cp -a`. Rejected: permissions, ownership, times,
  xattrs, ACLs, and hard-link identity are a larger platform surface and are
  outside the existing metadata model.
- Shell out to `cp` or `tar`. Rejected: it adds a subprocess boundary, depends
  on host utilities and option behavior, and weakens the zero-dependency and
  portable-native goals.
- Return a count or a recoverable `Result`. Rejected for v0.1: the filesystem
  builtins are fatal `R0015`; recoverable I/O errors are a separate design.
- Reject all non-UTF-8 names like `list_dir` and `glob`. Rejected: raw-name
  copying needs no string conversion and supports existing trees that the
  listing builtins cannot represent.
