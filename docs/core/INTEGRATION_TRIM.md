# Integration proposal: `trim_start` and `trim_end`

Date: 2026-11-28
Owner: Agent 01 (core language, compiler, and runtime)
Task: T-129
Status: implemented in core-owned files; the final gate evidence is recorded in
§5. Protected status files are untouched and need the edits in §7.

## 1. What changed

`trim` removed Unicode whitespace from both ends only. Scripts that strip a
trailing newline, a leading indent, or one side of a padded field had to trim
both ends and re-add the side they wanted, or split and rejoin. Vantyx now has
two one-sided builtins:

- `trim_start(text: Str) -> Str`
- `trim_end(text: Str) -> Str`

This is a pure standard-library addition. It adds no syntax, no dependency, no
error code, and no new size limit. The interpreter remains the oracle and the
native backend implements identical behavior.

## 2. Semantics

- Both builtins remove exactly the same Unicode `White_Space` set as `trim`
  (U+0009–U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000–U+200A, U+2028,
  U+2029, U+202F, U+205F, U+3000), from only the leading or trailing end.
- Neither removes zero-width spaces (U+200B), information separators
  (U+001C–U+001F), or other format characters. A zero-width character stops the
  scan on that side.
- All-whitespace and empty inputs produce `""`. When there is nothing to
  remove, the result is textually identical to the input.
- `trim(text)` is equivalent in result to `trim_end(trim_start(text))`.
- Arguments are evaluated exactly once, left to right, as for every builtin.
- A statically non-`Str` argument is checker error `E0301`; wrong arity is
  `E0302`; a dynamically typed `Any` argument that is not a `Str` raises
  runtime `R0005`; the native backend rejects a non-`Str` or `Any` argument
  with `E0401` and a concrete help message.
- Native helpers allocate only when characters are removed. When nothing is
  removed they return the borrowed input pointer, matching `trim`'s existing
  no-copy path.

## 3. Implementation

| File | Change |
| --- | --- |
| `src/checker.rs` | Adds both names to `BUILTINS`; the `trim` type rule covers all three names and reports the exact builtin name in `E0301`. |
| `src/ir.rs` | Lowers all three names through one rule requiring `IrType::Str`; otherwise `E0401` names the builtin. |
| `src/interp.rs` | One match arm selects `trim_matches`, `trim_start_matches`, or `trim_end_matches` over the shared `is_text_whitespace` predicate. |
| `src/codegen_c.rs` | Emits `vantyx_text_trim_start` / `vantyx_text_trim_end`; the C helpers share `vantyx_utf8_decode` and `vantyx_text_is_whitespace` with `vantyx_text_trim`. |
| `docs/language_specification.md` | Adds both signatures to the builtin table, the shared whitespace rule, the native supported list, and the limitations text. |
| `docs/security.md` | Adds both names to the in-memory text boundary and states the one-sided trim rules. |
| `docs/decisions/ADR-0028-text-builtins.md` | Appends an amendment recording the two builtins, semantics, implementation, and verification. |

`src/ownership.rs` needs no change: it conservatively classifies every builtin
as `MayEscape`, and the one-sided trims can also return their input unchanged,
so they are not `Fresh`. The memory model document's existing statement that
`str(Str)`/`trim` are not `Fresh` remains accurate.

## 4. Tests

- `tests/e2e.rs::trim_start_and_trim_end_boundaries` — all-whitespace, empty,
  zero-width on each side, mixed zero-width/whitespace, and padded ASCII.
- `tests/e2e.rs::text_builtins_round_trip` — extended with one-sided trims over
  ASCII, tab, NBSP, and ideographic space, including a full-string
  `trim_start`, a `len(trim_end(...))` check, and zero-width preservation.
- `tests/e2e.rs::text_builtins_reject_non_str_arguments` — static `E0301`
  messages for `trim_start(1)` and `trim_end(true)`.
- `tests/native.rs::trim_start_and_trim_end_boundaries_match` — the same
  boundary program built natively and compared byte-for-byte with the
  interpreter.
- `tests/native.rs::text_builtins_match` — the same extended round trip in
  both engines.

## 5. Verification

On 2026-11-28, on the final tree:

- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo build --release` — succeeded.
- `cargo test` — **844 passed, 0 failed**: 460 library unit + 15 CLI unit +
  56 AI integration + 146 end-to-end + 167 native.
- `python3 tools/test_differential_fuzz.py` — OK (9 self-tests).
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100
  cases agreed, none rejected by both engines.
- All five `examples/*.vtx` ran under the interpreter.
- `python3 tools/check_links.py` — every relative link resolves.
- Focused native tests under clang AddressSanitizer/UndefinedBehaviorSanitizer
  with leak detection disabled (`VANTYX_CC="$PWD/tools/clang-sanitize.sh"`):
  `trim_start_and_trim_end_boundaries_match` passed, and the two
  `text_builtins` tests passed.

The 56 AI integration tests and part of the library total come from Agent 02's
concurrent retry/trace work. The T-129 delta is one e2e test and one native
test, plus assertions added to the existing text round trip and static
rejection tests.

## 6. Compatibility and security

- Backward compatible: `trim` is unchanged, and the new names were previously
  unknown builtins, so no valid program changes meaning.
- Text builtins stay in memory only. The new helpers add no filesystem,
  network, environment, locale, or process access.
- Results are bounded by available memory, like `trim`; there is no streaming
  API or output cap. Native allocation failure terminates with exit 2.
- No unsafe Rust and no new C allocation ownership rule: results are ordinary
  leaked v0.1 native strings.

## 7. Shared-file edits (integration coordinator only)

Apply these after the T-128 tuple edits from
`docs/core/INTEGRATION_TUPLES.md`. All counts assume that state.

### 7.1 `TASKS.md`

Append after the T-128 section:

```markdown
### T-129 — One-sided Unicode trim builtins
- Priority: P2 (standard library: text processing)
- Status: VERIFIED locally on 2026-11-28; commit pending; CI pending
- Description: `trim` removed whitespace from both ends only, so stripping a
  line ending, indentation, or one side of a padded field required trimming
  both ends and re-adding the side that should stay. `trim_start` and
  `trim_end` provide one-sided trimming without new syntax or modules.
- Acceptance: `trim_start(Str) -> Str` and `trim_end(Str) -> Str` remove
  exactly the Unicode `White_Space` set from only the requested end; empty and
  all-whitespace inputs produce `""`; zero-width and format characters stop the
  scan; a string with nothing to remove is returned unchanged; arguments are
  evaluated once, left to right; static non-`Str` arguments are `E0301`, wrong
  arity is `E0302`, dynamic non-`Str` values raise `R0005`, and the native
  backend rejects non-`Str`/`Any` arguments with `E0401`. Both engines agree.
- Dependencies: ADR-0028 (amended), ADR-0007.
- Implementation: `src/checker.rs` adds both names to `BUILTINS` and extends
  the `trim` type rule; `src/ir.rs` extends the `Str`-only lowering rule;
  `src/interp.rs` selects `trim_start_matches`/`trim_end_matches` over the
  shared whitespace predicate; `src/codegen_c.rs` adds
  `vantyx_text_trim_start`/`vantyx_text_trim_end` sharing the existing UTF-8
  decoder and `White_Space` table and returning the input pointer when nothing
  is removed; the specification, security notes, and ADR-0028 amendment record
  the semantics.
- Tests: one new e2e boundary test and one new differential native boundary
  test; the existing text round trip and static-argument rejection test are
  extended with one-sided trims and both new names.
- Verification: 2026-11-28 — `cargo fmt --check --all`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo build --release`
  clean; `cargo test` = 844 passed (460 library unit + 15 CLI unit + 56 AI
  integration + 146 e2e + 167 native, 0 failed); 9 fuzzer self-tests; 100/100
  differential cases on seed 1; all five examples ran; every relative link
  resolves; the focused native trim/text tests also passed under clang
  AddressSanitizer/UndefinedBehaviorSanitizer with leak detection disabled.
- Scope limits: Only the Unicode `White_Space` property; no zero-width,
  format-character, locale, case, normalization, or grapheme-aware trimming.
  No streaming API or output cap; results are bounded by available memory.
```

### 7.2 `PROJECT_STATUS.md`

(a) Line 3 (`Updated: ...`) — immediately after the T-128 clause ending
`E0401` native rejection;`, insert:

```text
T-129 adds one-sided Unicode `trim_start` and `trim_end` builtins in both
engines with shared `White_Space` semantics, zero-width preservation, and
`E0301`/`E0302`/`R0005`/`E0401` boundaries; the full 844-test gate passed:
460 library unit + 15 CLI unit + 56 AI integration + 146 e2e + 167 native,
GCC native 167, sanitized native 167, 9 fuzzer self-tests, 100/100
differential fuzz on seed 1, all five examples, and the link checker;
```

(b) Builtin inventory, line beginning `- Builtins: ...`: add `trim_start`,
`trim_end` after `trim`.

(c) Interpreter text bullet (line beginning `- UTF-8 text builtins (T-060,
ADR-0028):`): change `` `trim` removes exactly the Unicode `White_Space` set ``
to:

```text
`trim` removes exactly the Unicode `White_Space` set from both ends, and
`trim_start`/`trim_end` remove it from only the leading or trailing end
(T-129)
```

(d) Native text bullet (line beginning `- Native text builtins (T-060,
ADR-0028):`): change `literal split, ordered join, trim, prefix/suffix tests`
to `literal split, ordered join, two-sided and one-sided trim, prefix/suffix
tests`.

(e) Native supported-builtin inventory (line beginning `- Supported builtins:
...`): add `trim_start`, `trim_end` after `trim`.

(f) Last verification paragraph beginning `` `cargo build`, `cargo build
--release` ``: prepend the T-129 sentence from (a), and update its counts from
`842` to `844`, e2e `145` to `146`, and native `166` to `167`. Keep the T-128
clause and the rest of the historical evidence unchanged.

(g) Test table:

- `cargo test` row: `842` → `844`; e2e `145` → `146`; native `166` → `167`;
  append `T-129 added one e2e and one differential native boundary test for
  one-sided Unicode trimming`.
- `VANTYX_CC=gcc cargo test --test native` row: `166` → `167`.
- Native ASan/UBSan row: `166` → `167`.

(h) Text interpreter/native comparison row: after `empty affixes`, insert
`one-sided leading/trailing/all-whitespace/empty trims`.

### 7.3 `docs/PROJECT_BASELINE.md`

(a) The `| Tests |` row: update the leading counts from
`425 library unit + 15 CLI unit + 47 AI integration + 137 end-to-end + 165
native = **789 passing**` to `460 library unit + 15 CLI unit + 56 AI
integration + 146 end-to-end + 167 native = **844 passing**`, and prepend the
T-128/T-129 evidence clauses after the date. (Apply the T-128 clause from
`INTEGRATION_TUPLES.md` first if it is not yet present.)

(b) The `| Interpreter |` row: change `UTF-8 text splitting, joining,
trimming, affix tests, and replacement` to `UTF-8 text splitting, joining,
two-sided and one-sided trimming, affix tests, and replacement`.

(c) Add a short T-129 sentence to the session summary: one-sided Unicode trim
builtins with shared `White_Space` semantics and differential boundary tests.

### 7.4 `AGENTS.md`

(a) Baseline count line: `789 passed (425 library unit + 15 CLI unit + 47 AI
integration + 137 e2e + 165 native)` → `844 passed (460 library unit + 15 CLI
unit + 56 AI integration + 146 e2e + 167 native)`, including the T-128 tuple
tests and the T-129 trim tests.

(b) Native support paragraph, `text builtins` phrase: state that two-sided and
one-sided Unicode trimming are supported.

(c) Known gaps text-builtin sentence: note that trimming is limited to the
Unicode `White_Space` set and offers both-end, leading, and trailing variants,
with no zero-width/format/locale/grapheme handling.

### 7.5 `README.md` (optional)

The README does not enumerate `trim`; no edit is required. If the standard
library list is expanded later, include `trim_start` and `trim_end` with
`trim`.
