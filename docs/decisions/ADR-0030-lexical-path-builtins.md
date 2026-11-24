# ADR-0030 — Lexical path builtins

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0026 (basic file I/O),
  ADR-0027 (directory operations), ADR-0029 (rename and copy),
  `docs/language_specification.md`

## Context

The file and directory builtins accept host path strings, but the language has
no way to build or inspect a path without manual string concatenation. Programs
that join a directory and file name, strip an extension, or check whether a
path is absolute must reimplement separator rules in user code, often
incorrectly on repeated separators or names that begin with a dot.

The alternatives were:

1. Introduce a nominal `Path` type with checked construction and OS-aware
   behavior. Correct long-term, but it requires type-system, display, equality,
   and native-representation design before any path utility can ship.
2. Expose operating-system path APIs (separator lookup, drive letters, UNC
   prefixes, current-directory changes). These are platform-specific and would
   make the interpreter and native backend diverge or require a portability
   layer that does not exist yet.
3. Add pure lexical string functions with an explicit POSIX-style contract.

## Decision

Add six synchronous, pure builtins:

| Builtin | Signature | Behavior |
| --- | --- | --- |
| `path_join` | `(Str, Str) -> Str` | Append a child path to a base |
| `path_parent` | `(Str) -> Str` | Parent directory of a path |
| `path_name` | `(Str) -> Str` | Final component of a path |
| `path_stem` | `(Str) -> Str` | Final component without its extension |
| `path_extension` | `(Str) -> Str` | Extension of the final component |
| `path_is_absolute` | `(Str) -> Bool` | Whether a path begins with `/` |

The contract is lexical and deliberately small:

1. **No filesystem access.** These functions never stat, resolve, canonicalize,
   read, or write anything. They do not access the current directory,
   environment, locale, or user home. `path_join("a", "b")` is string assembly,
   not a promise that `a/b` exists.
2. **POSIX-style `/` is the only separator.** A backslash is an ordinary byte.
   Drive letters, UNC prefixes, and platform-specific separators are not
   interpreted. Windows native support remains unverified.
3. **No normalization.** Empty components, repeated separators, `.` and `..`
   are preserved exactly. `path_join("a//", "b")` is `"a//b"` and
   `path_join("a", "../b")` is `"a/../b"`; no component is collapsed or
   removed. `path_join` with an absolute child discards the base, as in POSIX
   path composition, but it does not resolve the result.
4. **`path_join(base, child)` is ordered.** If `child` begins with `/`, return
   `child`. Otherwise, if `base` is empty, return `child`; if `child` is empty,
   return `base`; if `base` ends with `/`, return `base + child`; otherwise
   return `base + "/" + child`.
5. **`path_parent(path)` trims trailing separators first.** If no bytes remain,
   return `/` when the original path begins with `/`, otherwise `.`. Find the
   last separator, trim the separator run before it, and return the remaining
   prefix. If no prefix remains, return `/` for an absolute path and `.`
   otherwise. Thus `path_parent("a/b/c")` is `"a/b"`, `path_parent("/a")` is
   `"/"`, `path_parent("a")` is `"."`, `path_parent("a/b/")` is `"a"`, and
   `path_parent("")` is `"."`.
6. **`path_name(path)` returns the final component after trimming trailing
   separators.** It returns `""` when no component remains, including `"/"`,
   `"//"`, and `""`. The result excludes separators but keeps every other byte,
   including leading dots.
7. **The final dot separates stem and extension.** After obtaining the name, the
   last `.` is an extension separator only when it is not the first byte and at
   least one byte follows it. `path_stem("archive.tar.gz")` is `"archive.tar"`
   and `path_extension("archive.tar.gz")` is `"gz"`. A leading dot is not a
   separator: `path_stem(".bashrc")` is `".bashrc"` and its extension is `""`.
   A trailing dot is also not a separator: `path_stem("a.")` is `"a."` and its
   extension is `""`. When no separator exists, the stem is the whole name and
   the extension is `""`.
8. **`path_is_absolute(path)` is true exactly when the first byte is `/`.**
   Empty strings, relative paths, and `./` paths are false.
9. **UTF-8 is preserved.** `/` and `.` are ASCII bytes that cannot occur inside
   a multibyte UTF-8 sequence, so slicing at those bytes cannot split a Unicode
   scalar value. Names such as `"naïve/文件.txt"` retain their multibyte text.
10. **No new error code.** Static non-`Str` arguments are rejected with
    `E0301`; dynamically typed interpreter arguments are rejected with
    `R0005`; the native backend rejects dynamically typed arguments with
    `E0401`. With valid strings these functions do not fail except for
    allocation exhaustion, which follows the normal runtime policy.
11. **No security boundary.** These helpers do not sanitize `..`, reject
    absolute children, or confine paths to a directory. They are convenience
    functions, not a sandbox. Callers that accept untrusted path components
    must validate them before passing the result to file or directory builtins.
12. **No dependency.** The interpreter implements the rules with `&str` byte
    scanning; the native backend emits small C helpers in `src/codegen_c.rs`.

## Consequences

Positive:

- Programs can construct and inspect portable POSIX-style paths without
  duplicating separator and extension rules.
- The functions are deterministic, allocation-only, available in both engines,
  and do not widen filesystem privileges.
- The lexical contract is small enough to specify exhaustively and test with
  differential cases, including repeated separators, dotfiles, trailing dots,
  empty strings, and multibyte names.

Negative:

- There is no `Path` type, so a plain `Str` can still be passed where a path is
  expected and vice versa.
- Paths are not normalized, so callers cannot use these functions to eliminate
  `..` segments or enforce containment.
- Only POSIX separators are understood; Windows paths are not supported.
- `path_join` takes exactly two arguments; joining several components requires
  nested calls.
- Extension splitting is lexical and does not understand compound extensions,
  archive formats, or file-type metadata.
