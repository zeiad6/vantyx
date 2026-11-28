# Quality Agent Findings

Each finding includes ID, severity, component, reproduction, expected/actual,
evidence, affected files, proposed fix, regression test, and confidence.

Severity scale: P0 critical, P1 high, P2 medium, P3 low.

## QA-F001 — `vantyx build -o` can destroy the source file and exits 0

- **Severity:** P0 (data destruction)
- **Component:** CLI (`vantyx build`), native backend driver
- **Status:** CONFIRMED on `6e58b37`; **FIXED as T-124 by the primary agent,
  committed in `f1dbe36`, and independently verified**; CI run 46113804180
  passed 789/789. QA's proposed patch was retained for reference and was not
  applied by QA.
- **Confidence:** high (reproduced four ways, then verified fixed)

### Description

`build_file` passes the user-supplied `-o` path directly to the C compiler
without checking whether it names the entry source, an imported module, or a
symbolic-link target of either. clang replaces that path with the linked
executable, destroying the `.vtx` source, and `vantyx build` reports success
(exit `0`, empty stderr). A normal-looking command such as

```sh
vantyx build main.vtx -o main.vtx
```

therefore silently destroys the program. The same happens for an imported
module (`-o lib/helper.vtx`) and for an entry reached through a symlink when the
canonical target is used as `-o` (`link.vtx -> real.vtx`, `-o real.vtx`).
Relative/absolute spellings of the same path also overwrite.

### Reproduction (all files under `/tmp`, never the repository)

```sh
mkdir -p /tmp/qa-build && cd /tmp/qa-build
printf 'fn main():\n    print("source survived")\n' > victim.vtx
cp victim.vtx before.txt
/home/kali/Downloads/RustZ/target/debug/vantyx build victim.vtx -o victim.vtx
echo "exit=$?"
stat -c '%s bytes' victim.vtx
file victim.vtx
sha256sum before.txt victim.vtx
```

Observed:

```
exit=0
16904 bytes
ELF 64-bit LSB pie executable, x86-64 ...
before.txt: cd89520f...c75
victim.vtx: ff5d82da...e6b
```

Imported-module variant:

```sh
mkdir -p mod/lib
printf 'fn helper() -> Int:\n    return 7\n' > mod/lib/helper.vtx
printf 'from "lib/helper.vtx" import helper\n\nfn main():\n    print(helper())\n' > mod/main.vtx
vantyx build mod/main.vtx -o mod/lib/helper.vtx
# exit=0; mod/lib/helper.vtx becomes a 17240-byte ELF
```

Symlink-target variant:

```sh
printf 'fn main():\n    print("real source")\n' > real.vtx
ln -s real.vtx link.vtx
vantyx build link.vtx -o real.vtx
# exit=0; real.vtx becomes a 16904-byte ELF
```

### Expected

Refuse to write the output over any loaded source file (entry, imported module,
or canonical target), with a nonzero exit and a clear diagnostic, leaving every
source byte unchanged. GCC-style behavior: "input file is same as output file".

### Actual

The source is replaced by the executable; exit `0`; no diagnostic.

### Root cause

`src/main.rs::build_file` (around lines 227–258) computes `output_path` and
immediately runs `clang -o <output_path> <temp.c>`. `Compiled` carries the
`SourceRegistry` but not the on-disk source paths, and no same-file check is
performed. The C compiler replaces the directory entry (and therefore the
source) when the output path is the source path.

### Affected files

- `src/main.rs` — `build_file` and `Compiled`/`compile_entry` plumbing.
- `src/module.rs` — source display names are relative to the entry root; a
  robust guard needs the canonical entry root or explicit source paths.
- `README.md` / CLI help / `docs/language_specification.md` — document the
  refusal once fixed.

### Proposed fix (minimal, CLI-only)

1. Record the canonical on-disk path of the entry and every loaded module in
   `Compiled` (`source_paths: Vec<PathBuf>`); stdin contributes no path.
2. Before writing the temporary C file, compare the requested output path with
   each source path using `fs::canonicalize` (resolves symlinks, `.`/`..`, and
   relative/absolute spellings). If equal, print
   `error: output path ... is also a source file ...; refusing to overwrite it`
   and return usage exit `64`.
3. Add an e2e regression covering entry, imported module, and symlink-target
   variants, asserting exit `64`, the diagnostic, and byte-identical source.
4. Document the guard in the CLI help and specification.

The proposed diff is stored at
`/tmp/opencode/vantyx-qa/proposed-build-output-guard.patch`; it is **not**
applied to the repository.

### Residual risk (not part of this finding)

- `vantyx build - -o file.vtx < file.vtx` cannot know the shell redirect path;
  the already-read stdin is overwritten. This is inherent to stdin compilation
  and should be documented, not silently "fixed" with an extension heuristic.
- An output path naming an unrelated user file still overwrites that file, as
  ordinary compilers do; only loaded source files are protected by this fix.

### Verification after a future fix

Run the standalone script
`/tmp/opencode/vantyx-qa/regression_build_output.sh` (currently fails by
design) and `cargo test --test e2e build_refuses_output_that_is_a_source_file`.

### Re-confirmation on the T-123 working tree (2026-11-28)

The defect is still present in the current `target/debug/vantyx` (T-123
snapshot, sha256 `30b0419a...9dbf6`):

```sh
printf 'fn main():\n    print("do not destroy")\n' > victim.vtx
cp victim.vtx before.txt
vantyx build victim.vtx -o victim.vtx   # exit=0
file victim.vtx                          # ELF 64-bit LSB pie executable
cmp -s before.txt victim.vtx             # exit=1 (changed)
```

No default-output variant exists: without `-o`, a `.vtx` source becomes
`<stem>` and an extensionless source becomes `<name>.out`, so the default
path cannot equal the source. The `-o` collision guard is therefore the
complete CLI fix.

### Fix verification — post-fix working tree (2026-11-28)

The primary agent implemented an equivalent guard in `src/main.rs`
(`Compiled.source_paths`, canonical entry root, `fs_safety::
paths_refer_to_same_file_paths`, exit `64` with `is also a source file` and
`help: choose a different `-o` path`) and added
`tests/native.rs::build_refuses_to_overwrite_a_source_file` covering the
entry, imported-module, and symlink-target cases. Independent snapshot
`/home/kali/qa-work/p0-verify` (`src/main.rs` sha256
`2b59ab0e...3dd47`, `tests/native.rs` sha256 `d3018205...89224`):

- Full `cargo test` -> **784 passed, 0 failed** (422 library + 15 CLI + 47 AI
  + 135 e2e + 165 native), exit 0, including the new test.
- `cargo fmt --check --all`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo build --release` -> exit 0.
- Manual variants all return exit `64` with the source byte-identical:
  entry, relative spelling (`./sub/../sub/prog.vtx`), imported module,
  symlink target, hard-link alias, and a symlink-to-source `-o` path.
- Normal builds and missing output paths still succeed; an unrelated `-o`
  file is still replaced (the documented residual behavior).
- The new test also passes with `VANTYX_CC=gcc`.

Final verification on snapshot `/home/kali/qa-work/final-verify` (T-124 plus
T-125/T-126, `src/main.rs` sha256 `234d80b9...eebe`): full `cargo test` ->
**789 passed** (425 library + 15 CLI + 47 AI + 137 e2e + 165 native); the
entry, module, relative-spelling, symlink, hard-link, and directory-symlink
variants all exit `64` with sources byte-identical; `VANTYX_CC=/bin/false`
still exits `64`, proving the guard runs before the compiler. The primary
updated `AGENTS.md`, `PROJECT_STATUS.md`, `TASKS.md`, and the specification to
789/T-124. T-123–T-127 are committed in `f1dbe36`; CI run 46113804180 passed
789/789. No further QA code change is needed.

## QA-F002 — UTF-8 BOM is rejected as `E0001` in entries and modules

- **Severity:** P3 (compatibility / documentation; blocks no current tests)
- **Component:** lexer (`src/lexer.rs`) and module loading (`src/module.rs`)
- **Status:** CONFIRMED on `6e58b37` and on the T-123 working tree; **FIXED as
  T-125, committed in `f1dbe36`, and independently verified**; CI run
  46113804180 passed 789/789.
- **Confidence:** high

### Description

A UTF-8 byte-order mark (`EF BB BF`, U+FEFF) at the start of a source file is
not skipped. Both the entry file and imported modules fail with
`error[E0001]: unexpected character `﻿`` at `1:1`, exit `1`. CRLF line
endings are accepted, so only the BOM is missing from the Windows-friendly
input path. Several Windows editors and tools write a UTF-8 BOM by default;
the repository documentation never states that BOMs are unsupported. (UTF-16
input is a separate, also-unsupported encoding and is not part of this
finding.)

### Reproduction

```sh
printf 'fn main():\n    print("bom test")\n' > bom.vtx
printf '\xef\xbb\xbf' | cat - bom.vtx > bom_entry.vtx
vantyx run bom_entry.vtx
# error[E0001]: unexpected character `﻿` ... exit=1

mkdir -p mod
printf '\xef\xbb\xbf' | cat - > mod/helper.vtx <<'EOF'
fn helper() -> Str:
    return "module bom"
EOF
printf 'from "helper.vtx" import helper\n\nfn main():\n    print(helper())\n' > mod/main.vtx
vantyx run mod/main.vtx
# error[E0001]: unexpected character `﻿` --> helper.vtx:1:1 ... exit=1
```

The same helper without the three BOM bytes runs and prints `module bom`.

### Expected

Either (a) strip exactly one leading U+FEFF before lexing and document that
BOM + UTF-8 is accepted, or (b) keep rejecting it but emit a dedicated,
actionable diagnostic such as "UTF-8 BOM is not supported; save the file as
UTF-8 without a BOM" and document the restriction. Option (a) is preferable
for Windows compatibility and costs one prefix check in the file-reading path.

### Proposed fix (minimal)

1. In the entry/module read path (before lexing), if the bytes start with
   `EF BB BF`, drop those three bytes; do not silently strip a BOM anywhere
   else. This keeps spans aligned with the remaining text.
2. Add e2e coverage for a BOM-prefixed entry and a BOM-prefixed imported
   module, plus a CRLF + BOM combination.
3. Document BOM acceptance (or rejection) in
   `docs/language_specification.md` and the CLI/README notes.
4. If option (b) is chosen instead, special-case U+FEFF in the lexer's
   unexpected-character diagnostic and add the same tests asserting the new
   message.

### Residual risk

Stripping the BOM in the module loader but not in `vantyx run -` (stdin) would
create a new inconsistency; apply the same rule to every source entry point.

### Fix verification — T-125 (2026-11-28)

The primary added `vantyx::strip_utf8_bom` in `src/lib.rs`, applied it in
`src/module.rs` before the size check and lexing and in
`src/main.rs::read_source` (stdin and file entries), and added
`tests/e2e.rs::utf8_bom_is_accepted_at_the_start_of_sources`. Independent
probes on the final snapshot:

- BOM entry, BOM + CRLF entry, BOM imported module, and BOM stdin all run with
  exit `0` and the expected output.
- An interior BOM and a duplicate leading BOM still fail with `E0001` at
  `1:1`, exit `1`.
- A BOM-only file strips to empty input and reports `E0308: no entry point`,
  not a panic.
- `docs/language_specification.md` §1.1 now documents one leading BOM for
  entries, modules, and stdin, and the library-level `compile(&str)` API is
  documented as not stripping a BOM.

## QA-F003 — Self-referential list can satisfy a mismatched recursive type

- **Severity:** P2 (interpreter gradual-typing soundness; pre-existing, not
  introduced by T-123)
- **Component:** `src/value.rs::value_conforms` (cycle shortcut) and the
  binding/parameter validation path that calls `conforms_to`
- **Status:** CONFIRMED on `6e58b37` and on the T-123 working tree; **FIXED
  as T-126, committed in `f1dbe36`, and independently verified**; CI run
  46113804180 passed 789/789. The native backend was unaffected because it
  rejects `List[Any]` with `E0401`.
- **Confidence:** high

### Description

`value_conforms` tracks visited containers in a `HashSet<usize>` keyed only by
container identity. When a container is revisited, it returns `true`
unconditionally, without checking the expected type at the revisit. A list
that contains itself can therefore be validated against
`List[List[Int]]`: the outer check assumes `xs: List[List[Int]]`, revisits
`xs` while checking its element against `List[Int]`, and the identity-only
shortcut accepts the mismatch. The checker then types `ys[0][0]` as `Int`,
but the runtime value is the list itself.

### Reproduction

```sh
cat > cycle.vtx <<'EOF'
fn main():
    let xs: List[Any] = []
    push(xs, xs)
    let ys: List[List[Int]] = xs
    print(ys[0][0])        # prints [[...]]
    print(ys[0][0] == 1)   # false
    print(ys[0][0] + 1)    # error[R0005], exit 2
EOF
vantyx run cycle.vtx
```

Observed: binding `ys` succeeds; the statically `Int`-typed expression
`ys[0][0]` prints `[[...]]` and compares false; arithmetic raises `R0005`.
The map form behaves the same: after `m: Map[Any, Any] = {}` and
`m["self"] = m`, `let n: Map[Str, Map[Str, Int]] = m` succeeds and the
statically `Int`-typed `n["self"]["self"]` prints `{"self": {...}}`.
A two-list mutual cycle (`a -> b -> a`) is rejected instead, because the
revisit is compared against `Int` and never enters the list branch. The
inconsistency shows the shortcut is keyed on the wrong thing.

### Impact

No memory unsafety, no data loss, and no silent numeric result was observed:
operations that require a scalar raise `R0005` or return `false`. The defect
is a static-type soundness hole: a value typed `Int` can be a list, which can
confuse user code, `print` output, and any future consumer that trusts the
checker. It is interpreter-only in practice because the native backend
rejects `List[Any]`/`Map[Any,*]` with `E0401` (ADR boundary).

### Proposed fix

1. Change the conformance cycle set to `HashSet<(usize, Type)>` (or a stable
   type fingerprint) and return `true` coinductively only when the same
   `(container, expected type)` pair is already in progress. A revisit with a
   different expected type must be treated as non-conforming (or handled by a
   real coinductive subtype check, which is larger).
2. Apply the same keying to `tighten_value` so contract strengthening cannot
   follow a cycle into a type that was never validated.
3. Add regressions for: self-cycle accepted as `List[Int]` (valid), self-cycle
   rejected as `List[List[Int]]` when the element type differs, mutual cycle,
   map self-cycle (`m["self"] = m`), and a nested self-cycle under a struct or
   enum payload.
4. Note the fix in the type-system documentation and ADR-0057; it is
   orthogonal to the T-123 mutation contracts.

### Fix verification — T-126 (2026-11-28)

The primary changed the visited sets in `value_conforms` and `tighten_value`
to `HashSet<(usize, Type)>` and removes the key after each recursive check;
three library tests and one e2e test were added. Independent probes on the
final snapshot:

- `xs = [xs]` bound to `List[List[Int]]` and the map self-cycle bound to
  `Map[Str, Map[Str, Int]]` now fail with `R0005`, exit `2`, empty stdout.
- A two-list mutual cycle bound to `List[List[Int]]` also fails with `R0005`.
- The same self-cycles remain valid coinductively under `List[List[Any]]`
  and `Map[Str, Map[Str, Any]]` and still display `[[...]]` /
  `{"self": {...}}`.
- Ordinary nested values (`[[1]]`, `{"k": {"a": 1}}`) and fully dynamic
  `List[Any]`/`Map[Any, Any]` mutation still work.
- Display, structural equality, and the native `E0401` boundary are
  unchanged.

## QA-F004 — Native/e2e tests are not hermetic against `/tmp` cleanup

- **Severity:** P3 (test-infrastructure reliability; no language defect)
- **Component:** `tests/native.rs`, `tests/e2e.rs`, and the test helpers that
  use `std::env::temp_dir()`
- **Status:** CONFIRMED during the independent T-123 run; **FIXED as T-127,
  committed in `f1dbe36`, and independently verified with an unwritable
  process temp directory**; CI run 46113804180 passed 789/789. T-127 as
  written introduces QA-F005, a latent path-length flake at the standard
  GitHub Actions root (see below).
- **Confidence:** high (observed, then isolated)

### Description

The native and e2e suites create workspaces and generated C files under the
process temp directory and let the compiler create its temporary object files
there too. If any concurrent process removes files under `/tmp` while the
suite runs, clang fails with:

```
error: unable to rename temporary '/tmp/vantyx-...o.tmp' to output file
'/tmp/vantyx-...o': 'No such file or directory'
```

The test then reports a native build failure even though the compiler and
runtime are correct. During this audit, 4 of 164 native tests failed this way
while the primary session and the QA run shared the host; all four passed
immediately with `TMPDIR=/home/kali/qa-work/tmp`, and the full pre-fix suite
then passed 783/783.

### Reproduction

```sh
# any concurrent cleanup of /tmp/vantyx-* (or a full tmpfs) during:
cargo test --test native
# 4 failures: contains_and_push_accept_empty_list_literals,
# control_flow_matches, conversions_and_strings_match,
# copy_file_preserves_bytes_match
# all with clang "unable to rename temporary ... No such file or directory"

TMPDIR=/home/kali/qa-work/tmp cargo test --test native
# 164/164 pass
```

### Expected

The test gate should be hermetic: either use a suite-private temp root for
child processes or make CI set `TMPDIR` to the job's private temp directory,
so an external cleaner cannot turn a green suite red.

### Proposed fix

1. In the shared test helpers, create one per-run directory (for example
   `temp_dir()/vantyx-tests-<pid>`) and set `TMPDIR`, `TMP`, and `TEMP` on
   every `Command` that invokes `vantyx build`/`run`, cleaning it up at the
   end; or
2. Add `TMPDIR="$RUNNER_TEMP"` to the CI test steps and document the
   requirement in the verification commands.
3. Optionally, detect this specific clang diagnostic in the native helper and
   retry once with a fresh temp root, but the deterministic fix is (1) or (2).

### Residual risk

This does not affect release binaries; it only makes test results
environment-sensitive. The audit's first full run was invalidated by it and
was rerun with an isolated `TMPDIR`.

### Fix verification — T-127 (2026-11-28)

The primary added a `OnceLock`-backed `test_root()` to `tests/e2e.rs` and
`tests/native.rs` that prefers `VANTYX_TEST_TMPDIR`, then Cargo's
`CARGO_TARGET_TMPDIR`, then the process temp directory; workspaces live under
`test_root()/vantyx-tests-<pid>`, and `configure_temp_env`/`vantyx_command`
set `TMPDIR`, `TMP`, and `TEMP` on every spawned `vantyx` command. On the
snapshot `/home/kali/qa-work/t127-verify` (`tests/e2e.rs` sha256
`61f53bca...06a0d`, `tests/native.rs` sha256 `9cd3728f...d44844`):

- `cargo test` with `VANTYX_TEST_TMPDIR=/home/kali/qa-work/t` -> **789
  passed** (425 library + 15 CLI + 47 AI + 137 e2e + 165 native), exit 0.
- The built e2e and native binaries pass 137/137 and 165/165 when the test
  process is started with `TMPDIR=/proc TMP=/proc TEMP=/proc` and a short
  `VANTYX_TEST_TMPDIR`; this proves the clang children no longer depend on the
  process temp directory.
- The primary updated `TASKS.md`, `AGENTS.md`, and `PROJECT_STATUS.md` for
  T-127 and recorded the `/proc` run.

The fix is structurally correct for QA-F004, but its directory names make one
Unix-socket test path-sensitive; that regression is QA-F005 below.

## QA-F005 — T-127's private root exceeds `SUN_LEN` at the CI checkout path

- **Severity:** P2 (flaky test / CI reliability; no language or runtime
  impact). An earlier draft called this P1 on the assumption that the CI
  path always exceeds the limit; CI run 46113804180 passed, so the failure
  is scheduling- and PID-dependent, not deterministic.
- **Component:** `tests/native.rs::recursive_copy_special_files_match_in_both_engines`
  and the T-127 `workspace()` / `test_root()` helpers
- **Status:** CONFIRMED on the T-127 working tree and on a clean
  `git archive f1dbe36` snapshot (`tests/native.rs` sha256
  `9cd3728f...d44844`); **committed in `f1dbe36` without the fix**. CI run
  46113804180 on `150997d` passed 789/789, including this test in the
  normal and sanitizer native jobs, because the runner's PID and counter
  kept the path within 107 bytes. The failure reproduces with a 7-digit
  PID and a 3-digit counter; it is a latent flake, not a guaranteed red
  gate. The two-line fix was verified on a QA snapshot but not applied to
  the live tree because the primary owns `tests/native.rs`.
- **Confidence:** high (the 107/108-byte boundary reproduced exactly)

### Description

T-127 moved test workspaces from the process temp directory to
`CARGO_TARGET_TMPDIR` (or `VANTYX_TEST_TMPDIR`). The Unix-socket test binds
`<workspace>/tree/sock`; on Linux `sun_path` is 108 bytes including the NUL
terminator, so a path of 108 or more characters fails with:

```
bind interpreter socket: Error { kind: InvalidInput,
message: "path must be shorter than SUN_LEN" }
```

The local checkout yields a 104-character socket path and passes. The standard
GitHub Actions checkout is `/home/runner/work/vantyx/vantyx`, so
`CARGO_TARGET_TMPDIR` is 42 characters and the same test, with the full-suite
counter at 120, produces a 109-character path. T-127's local `TMPDIR=/proc`
verification used the shorter local path, so it did not expose this.

### Reproduction

Snapshot `/home/kali/qa-work/t127-verify` (T-127, no QA edits):

```sh
# Full native suite at the default private target path (41-char root):
cargo test --test native
# -> 164 passed; 1 failed:
# recursive_copy_special_files_match_in_both_engines
# bind interpreter socket: ... "path must be shorter than SUN_LEN"

# Isolate the length boundary with the single test (counter starts at 0):
VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpX \
  <native-binary> recursive_copy_special_files_match_in_both_engines
# 42-char root -> 107-char socket path -> PASS
VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXY \
  <native-binary> recursive_copy_special_files_match_in_both_engines
# 43-char root -> 108-char socket path -> FAIL, exit 101
VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXYZ \
  <native-binary> recursive_copy_special_files_match_in_both_engines
# 44-char root -> 109-char socket path -> FAIL, exit 101
```

Computed paths (7-digit PID, counter 120):

```text
local:  /home/kali/Downloads/RustZ/target/tmp/.../tree/sock            104
CI:     /home/runner/work/vantyx/vantyx/target/tmp/.../tree/sock       109
```

With a 7-digit PID and a 3-digit counter, the CI path exceeds the limit by
two bytes; with a shorter runner PID it stays below. The local host uses
7-digit PIDs, which is why the failure is reproducible there. A full-suite
counter below 100 also shortens the path enough to pass, so the failure
depends on both test ordering and PID length and is easy to miss.

**Post-commit confirmation (2026-11-28):** a clean `git archive f1dbe36`
snapshot at `/home/kali/qa-work/f1dbe36-verify` has `tests/native.rs` sha256
`9cd3728f...d44844` (the unpatched committed file). Rebuilt and run:

- 42-character root -> 107-character socket path -> pass.
- 43-character root -> 108-character socket path -> panic, exit 101.
- 44-character root -> 109-character socket path -> panic, exit 101.

Commit `f1dbe36` therefore contains the latent defect; `150997d` records the
commit hash and CI status. CI run 46113804180 on `150997d` later passed
789/789, including this test in both native jobs; the runner's shorter PID
kept the path under the limit. The commit is green, but the test remains
flaky and can fail on a runner with 7-digit PIDs and a 3-digit counter.

### Root cause

`workspace()` includes the full descriptive test name in the directory
(`native-<counter>-copy-special-interpreter`), and T-127 places that directory
under a root derived from the checkout path. The resulting path is not
bounded by `SUN_LEN`; the test assumes the root is short.

### Proposed fix

Two test-only name changes reduce the socket path by 18 and 17 characters:

```diff
--- a/tests/native.rs
+++ b/tests/native.rs
@@ -5228,7 +5228,7 @@ fn recursive_copy_special_files_match_in_both_engines() {
-    let interpreter_directory = workspace("copy-special-interpreter", source);
+    let interpreter_directory = workspace("cs-i", source);
@@
-    let native_directory = workspace("copy-special-native", source);
+    let native_directory = workspace("cs-n", source);
```

At the standard CI root this yields a path near 91 characters, leaving about
16 bytes of headroom. The patch is saved as
`/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`. A more robust
alternative is a dedicated short workspace helper, or binding the socket at a
short path and renaming it into `tree/`; shortening the names is the smallest
verified change.

### Verification of the proposed fix (QA snapshot only)

After applying the two-line patch to `/home/kali/qa-work/t127-verify` and
rebuilding:

- The single test passes with both the 43- and 44-character roots (108/109
  character unpatched paths).
- The full native suite passes **165/165** and the e2e suite **137/137** with
  `VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXYZ` (44
  characters, longer than CI) and `TMPDIR=/proc TMP=/proc TEMP=/proc`.
- Logs: `/home/kali/qa-work/t127-native-ci-length.log`,
  `/home/kali/qa-work/t127-e2e-ci-length.log`. The patched snapshot's
  `tests/native.rs` sha256 is `bd26ce51...87b79`; the committed file remains
  `9cd3728f...d44844`.

T-127 is committed in `f1dbe36` and CI run 46113804180 is green; the primary
should apply the patch (or an equivalent) in a follow-up commit to remove the
flake. Do not amend published history.

### Residual risk

The shortened names still leave only a bounded margin: a much longer runner
checkout path (for example a long repository name) could exceed `SUN_LEN`
again. A fully robust fix should not derive a filesystem-socket path from an
unbounded workspace root; binding at a short path and renaming the socket into
the tree removes the dependency. This is a test-infrastructure failure only;
no product binary, language semantics, or runtime behavior is affected, but it
can turn a future CI run red. The observed CI run passed, so this is a latent
flake rather than a current blocker.

## T-123 audit result — no mutation-bypass found

The uncommitted T-123 container-contract change was exercised with the
snapshot binary (sha256 `30b0419a...9dbf6`, built from the working tree that
contains the T-123 edits) using
`/home/kali/qa-work/t123-probe/qa_t123.py` and `qa_t123_ext.py` (33 scripted
checks total). Five scripted expectations needed correction, all test-author
errors rather than implementation failures: three static mismatches are
legitimately rejected earlier with `E0301`, one used the wrong enum
constructor syntax, and one returned valid data. With those corrected, every
check behaved correctly:

- `push` and list index assignment through a dynamic alias after a successful
  `Any -> List[T]` validation raise `R0005` before mutation.
- Map key and value assignment validate independently, including after
  `remove`.
- Struct constructor fields and enum payloads validate and tighten the same
  underlying container.
- Function parameters and returns validate; a failed return raises `R0005`.
- Contracts survive `pop`, `remove`, widening back to `List[Any]`, and
  `keys`/`values` round-trips.
- Failed validation does not tighten the container (it remains dynamic).
- Fully dynamic `List[Any]`/`Map[Any, Any]` stay mutable; typed literals with
  dynamic elements/keys validate at construction.
- Deep nesting (4 levels) and cyclic display (`[[...]]`) do not crash.

The only bypass found is QA-F003 (binding-level cycle conformance), which is
pre-existing and not a mutation path. T-123's own regression tests
(`tests/e2e.rs` around lines 2669–2810) cover alias push/index, map key/value,
parameter, nested, and struct-field mutation; they do not cover cycles.
