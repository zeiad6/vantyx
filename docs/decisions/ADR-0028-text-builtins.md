# ADR-0028 — UTF-8 text builtins

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0026 (basic file I/O),
  ADR-0027 (directory operations), `docs/language_specification.md`

## Context

Vantyx strings support indexing, iteration, concatenation, `len`, and
`contains`, but there was no way to split text into fields, join fields back
together, trim surrounding whitespace, test prefixes or suffixes, or replace
substrings. Those operations are routine in scripts that process configuration,
logs, generated text, and command output, and forcing them into the Rust host
keeps ordinary text processing out of the language.

The alternatives were:

1. Wait for a module system with a full text library, pattern types, and
   `Result`-based errors. Better long-term organization, but it leaves the
   language unable to perform basic text processing indefinitely.
2. Add regular expressions, Unicode case mapping, normalization, and grapheme
   segmentation immediately. These require large tables or third-party
   dependencies and a much larger semantic contract.
3. Add a small, explicit set of global string builtins with precisely defined
   UTF-8 semantics, following the existing builtin and error model.

## Decision

Add six synchronous builtins:

| Builtin | Signature | Behavior |
| --- | --- | --- |
| `split` | `(Str, Str) -> List[Str]` | Split on a literal separator |
| `join` | `(List[Str], Str) -> Str` | Join parts with a separator |
| `trim` | `(Str) -> Str` | Remove leading and trailing Unicode `White_Space` |
| `starts_with` | `(Str, Str) -> Bool` | Prefix test |
| `ends_with` | `(Str, Str) -> Bool` | Suffix test |
| `replace` | `(Str, Str, Str) -> Str` | Replace every non-overlapping match |

1. **No new syntax or module.** They are statically known builtin names and
   cannot be shadowed, like the file and directory builtins.
2. **`split` is literal and lossless.** A non-empty separator is matched by its
   UTF-8 bytes. Empty fields are preserved, so `split("a,,b", ",")` returns
   `["a", "", "b"]` and `split("", ",")` returns `[""]`. An empty separator
   splits into individual Unicode scalar values, so `split("aéb", "")` returns
   `["a", "é", "b"]` without empty leading or trailing elements.
3. **`join` requires `List[Str]`.** An empty list produces the empty string.
   Other element types are a checker error when statically known and a runtime
   `R0005` type error in the interpreter; the native backend rejects non-`Str`
   element types with `E0401`.
4. **`trim` uses the Unicode `White_Space` property exactly.** The set is
   U+0009–U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000–U+200A, U+2028,
   U+2029, U+202F, U+205F, and U+3000. Characters such as U+200B (zero-width
   space) and U+001C–U+001F are not trimmed. Both engines share the same
   explicit table rather than relying on host-language whitespace definitions.
5. **Prefix and suffix tests are byte-exact.** An empty affix is always true.
   There is no case folding or normalization.
6. **`replace` is left-to-right and non-overlapping.** The replacement text is
   not rescanned. An empty pattern is rejected with the new `R0016` invalid
   builtin argument error rather than silently inserting text at every
   boundary. Replacing with an empty string deletes matches.
7. **Evaluation order is left to right.** Multi-argument builtins are emitted as
   GNU C statement expressions that bind each argument in source order before
   the call, matching the interpreter.
8. **Fatal errors and no sandbox.** Text builtins do not touch the filesystem.
   The only value error is the empty `replace` pattern. Type errors are
   checker diagnostics when static, `R0005` at dynamic interpreter boundaries,
   and `E0401` at the native boundary.
9. **No dependency and no new limit.** The native helpers use only libc string
   and memory functions plus the existing UTF-8 helpers. Results are bounded by
   available memory; `split`, `join`, and `replace` can allocate proportional
   to their input and output sizes. No arbitrary size cap is introduced.

## Consequences

Positive:

- Scripts can parse delimited text, assemble output, trim fields, test affixes,
  and substitute substrings in both engines.
- Empty fields, empty separators, and Unicode whitespace have explicit,
  tested behavior instead of host-dependent defaults.
- The native implementation validates the same UTF-8 strings it already
  guarantees and adds no dependency.

Negative:

- There is no regular-expression engine, case conversion, Unicode
  normalization, grapheme-cluster segmentation, or locale awareness.
- `split("", "")` returns an empty list rather than a list of one empty string;
  this is a deliberate consequence of scalar-value splitting.
- `trim` operates on code points, not grapheme clusters, and does not trim
  zero-width or format characters.
- `replace` cannot express an empty pattern. Callers that want insertion must
  split and join explicitly.
- Large splits or replacements can exhaust memory; there is no streaming text
  API or output-size cap.

## Verification

- `tests/e2e.rs`: round trip covering empty fields, trailing separators,
  Unicode scalar splitting, empty lists, Unicode whitespace, non-whitespace
  format characters, empty affixes, and non-overlapping replacement; static
  type errors; fatal `R0016` for an empty replacement pattern.
- `tests/native.rs`: the same round trip and error under both engines, plus a
  left-to-right argument-evaluation test using side-effecting calls.
- The native text tests pass under `-fsanitize=address,undefined` with leak
  detection disabled for intentional v0.1 leaks.
- Full gate and test counts are recorded in `TASKS.md` and `PROJECT_STATUS.md`.
