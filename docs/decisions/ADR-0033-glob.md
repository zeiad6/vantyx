# ADR-0033: Lexical glob expansion

- Status: Accepted
- Date: 2026-11-24
- Related: ADR-0007 (zero dependencies), ADR-0026 (file I/O), ADR-0027
  (directory operations), ADR-0031 (recursive directories), T-065

## Context

Vantyx can list one directory at a time, but callers who need every matching
file in a tree must write recursive walks and filename matching by hand. The
standard library has no pattern expansion, and adding a glob crate would
violate the zero-dependency rule. The existing filesystem builtins are fatal
(`R0015`) and report entries as UTF-8 `Str` paths, so glob expansion must use
the same error and encoding model.

A glob is a lexical path pattern, not a regular expression, a shell, or a
sandbox. The design must define the matching grammar, hidden-entry rules,
symbolic-link behavior, result ordering, and resource bounds precisely enough
for the interpreter and the native backend to agree.

## Decision

Add one builtin:

```text
glob(pattern: Str) -> List[Str]
```

The builtin requires a `Str` pattern, resolves relative patterns against the
process working directory, and is rejected with `E0401` in native code when the
pattern is dynamically typed. Dynamic interpreter values are `R0005`. The
interpreter rejects a pattern containing a NUL byte with `R0015`; native
strings cannot contain NUL, so a native string literal with a NUL byte is
already rejected with `E0401` by the IR string invariant. No dependency is
added and the filesystem is never modified.

### Pattern structure

- Patterns are split on `/`. Empty components are discarded, so repeated and
  trailing separators do not affect matching; the returned spelling contains a
  single separator between components. A pattern that begins with `/` is
  absolute; every other pattern is relative. `/` and `//` expand to `/`.
- Each remaining component is either a literal, a wildcard component, or the
  special component `**`. A component is special only when its whole text is
  exactly `**`; `a**b`, `***`, and `**x` are ordinary wildcard components.
- There is no escape syntax, brace expansion, tilde expansion, variable
  expansion, command substitution, case folding, Unicode normalization, or
  separator other than `/`. A backslash is an ordinary literal character.
- An entirely empty pattern is invalid and raises `R0016`.

### Component matching

- `*` matches zero or more Unicode scalar values inside one component.
  Adjacent stars behave like one star. It never crosses `/`.
- `?` matches exactly one Unicode scalar value.
- `[abc]`, `[a-z]`, and `[!a-z]` (or `[^a-z]`) are character classes over
  Unicode scalar values. A `]` immediately after `[` or the negation marker is
  a literal `]`. A `-` as the first or last member is a literal `-`. An
  unclosed class, an empty class (`[]` or `[!]`), or a descending range such as
  `[z-a]` is invalid and raises `R0016`.
- Matching is per component against directory entry names. Literal components
  are not pattern-matched; they are used directly as path components.

### Hidden entries

- A wildcard component does not match a name that begins with `.` unless the
  component itself begins with a literal `.` or a positive class that contains
  `.` (for example `.*`, `[.]hidden`, or `[.-0]name`). A negated class never
  enables hidden names.
- `**` never yields hidden entries and never descends into hidden
  directories. An explicit component such as `.*` can match and traverse a
  hidden directory, after which a following `**` applies the same rule to its
  children.

### `**` recursion

- A non-final `**` matches zero or more directory levels. It descends only
  through real directories, not symbolic links, and the entries it yields are
  non-hidden.
- A final `**` returns every non-hidden descendant below the current base. It
  does not return the base itself, so `dir/**` never includes `dir` and `**`
  never returns `.`.
- The base path itself may be a symbolic link or pass through one. For
  example, `link/**` follows the `link` base and lists the target's
  descendants, while nested symbolic-link directories below that base are
  listed as entries but not traversed.

### Results and limits

- Each result is a lexical path built from the pattern's literal components
  and the actual entry names. `.` and `..` components are preserved as
  written, and an absolute pattern yields absolute results. Results are sorted
  by UTF-8 byte order and deduplicated.
- A final literal component is probed with `lstat` semantics, so a dangling
  symbolic link is returned when its name exists. Any `lstat` failure on that
  final probe is treated as no match, including a permission failure. A
  wildcard component returns every matching directory entry name, including
  symbolic links and dangling links, without requiring the target to exist.
- Missing paths and paths that are not directories produce no matches rather
  than an error. Directory inspection or listing failures other than a missing
  path or non-directory, a non-UTF-8 directory entry name, and a failing
  `lstat` during `**` traversal raise `R0015`.
- Expansion stops with `R0015` after 100,000 candidate paths before
  deduplication, at a `**` recursion depth of 128 below the base, or when an
  allocation or host read fails. A pattern with more than 1,024 non-empty
  components raises `R0016`.
- The result list is materialized in memory. There is no streaming iterator,
  cancellation, or early caller stop.

## Consequences

- Callers can perform recursive discovery without writing a walk, and the
  results are deterministic for a fixed tree. The interpreter and native
  backend share the grammar, hidden rules, symlink rules, caps, and error
  codes.
- The pattern language is intentionally smaller than shell globs and common
  glob libraries: no escaping, braces, extglobs, `~`, case-insensitive mode,
  or sorting options. A literal `*`, `?`, `[`, or `]` cannot be escaped; use a
  character class such as `[*]`, `[?]`, or `[[]`, or list the directory and
  filter in Vantyx.
- `**` avoids symlink cycles by not descending through links, but explicit
  link components can still leave the apparent base. There is no sandbox and
  no `..` confinement; untrusted patterns and bases require caller
  validation. Check-then-act races apply between directory listing, `lstat`,
  and later reads.
- The caps bound worst-case memory but not CPU: a pattern with several `**`
  components can revisit overlapping subtrees, and each candidate costs a
  string allocation. Large trees can still be slow before the result cap is
  reached.
- Native expansion uses POSIX `DIR`/`readdir`/`lstat` and a hand-written
  matcher. Windows native support remains unverified.
- No file contents are read, no permissions or timestamps are inspected, and
  no metadata is modified.

## Alternatives considered

- Depend on a glob crate. Rejected by ADR-0007 and the zero-dependency rule.
- Shell out to a shell or `find`. Rejected: it would add a subprocess boundary,
  depend on host utilities, complicate quoting, and weaken portability.
- Expose regular expressions instead. Rejected: regexes are a larger language,
  would need a dependency or a large engine, and do not solve path walking.
- Follow symbolic links during `**` with cycle detection. Rejected for v0.1:
  it needs identity tracking and introduces escape and cycle risk; callers can
  expand an explicit link path.
- Return the base for `**` or preserve trailing separators. Rejected: results
  should name existing entries, not pattern spellings, and duplicate or
  non-existent spellings complicate consumers.
- Skip non-UTF-8 names silently. Rejected: it would make results
  locale-dependent and hide entries; the existing directory builtins already
  fail with `R0015` on such names.
