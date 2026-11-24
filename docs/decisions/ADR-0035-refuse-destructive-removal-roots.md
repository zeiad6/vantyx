# ADR-0035: Refuse destructive recursive-removal roots

- Status: Accepted
- Date: 2026-11-24
- Amends: ADR-0031
- Related: T-063, T-069, ADR-0007, ADR-0031

## Context

ADR-0031 added `remove_dir_all(path: Str) -> Unit` with fatal `R0015` errors
and deliberately documented two unsafe consequences:

1. `remove_dir_all(".")` deleted the visible contents of the working directory
   before failing to remove `.` itself, leaving an empty directory.
2. A root spelled `link/.` or `link/..` could traverse a symbolic link even
   though a root spelled `link` or `link/` was unlinked rather than followed.

The original differential test locked the first behavior. That made an
accidental destructive call look like a supported semantic. The directive for
v0.1 prioritizes safety and honest limits, so this ADR tightens the root rules
without pretending to provide a sandbox.

## Decision

`remove_dir_all` refuses two classes of root before any filesystem access:

- a final `.` or `..` component after trailing separators are trimmed,
  including `.`, `./`, `..`, `../`, `tree/.`, `tree/..`, `link/.`, and
  `link/..`; and
- the filesystem root `/`, including repeated-separator spellings such as
  `//` and `///`.

Both refusals are `R0015` with a stable explanation. The interpreter and the
native C backend use the same lexical rule:

- `src/fs_safety.rs` classifies the interpreter path.
- `vantyx_remove_root_refusal` in `src/codegen_c.rs` mirrors it.

Ordinary roots such as `tree`, `tree/`, `./tree`, `/tmp/tree`, and
`foo/.hidden` remain accepted. An empty path is not classified by this rule;
the filesystem layer still reports it as an ordinary missing-path `R0015`.

The rule is intentionally lexical. It does not resolve symbolic links,
normalize intermediate `.`/`..` components, or prevent a path such as
`link/sub` from reaching through a link. Those remain documented caller
responsibilities. The change removes the two spellings that were both easy to
write by accident and disproportionately destructive.

## Consequences

- A program that previously emptied its working directory with
  `remove_dir_all(".")` now fails before deleting anything.
- A root link reached through `link/.` or `link/..` is no longer traversed.
- The refusal is observable as `R0015`, so programs cannot silently continue
  after a rejected root.
- The language still has no confirmation prompt, trash, dry run, rollback,
  depth limit, entry-count limit, `Result`-based recovery, or sandbox.
- Intermediate-link traversal, check-then-act races, and deep-tree C-stack
  consumption remain unchanged.
- The classifier recognizes `/` separators. Windows interpreter paths that use
  `\` are not classified, and Windows native behavior remains unverified.

## Tests

- `src/fs_safety.rs`: five unit tests cover allowed roots, `.`/`..` final
  components, the filesystem root, empty paths, and stable messages.
- `tests/e2e.rs`: `recursive_removal_refuses_destructive_roots` covers eight
  spellings and checks that the sentinel file, tree, and source survive;
  `recursive_removal_allows_a_leading_dot_prefix` keeps `./tree` usable.
- `tests/native.rs`: `recursive_removal_refuses_destructive_roots_in_both_engines`
  covers seven spellings and checks `R0015`, preserved files, and the native
  executable; `recursive_removal_allows_a_leading_dot_prefix_in_both_engines`
  checks interpreter/native agreement for the allowed form.

## Alternatives considered

1. **Keep the documented behavior and rely on caller validation.** Rejected:
   the destructive default is easy to trigger and the original test presented
   it as intentional semantics.
2. **Normalize the path before removal.** Rejected: lexical normalization does
   not resolve links and can change which directory is addressed; a full
   descriptor-relative walk is a larger design.
3. **Reject every path containing `.` or `..`.** Rejected as too broad:
   `./tree` is a common, unambiguous spelling and does not bypass the root
   link check.
4. **Return a recoverable error.** Deferred until `Result`/`?` exists; `R0015`
   remains the consistent fatal error for this builtin family.

## Reversibility

High. The classification is one small module plus one C helper. Removing the
checks restores ADR-0031 behavior, but doing so would reintroduce the
destructive-root footguns documented above.
