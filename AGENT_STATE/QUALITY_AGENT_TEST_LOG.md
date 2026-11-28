# Quality Agent Test Log

Raw commands and observed results. Artifacts live in
`/tmp/opencode/vantyx-qa/` (harnesses, proposed patch) and
`/home/kali/qa-work/` (builds, logs, suites). Large builds deliberately avoid
the shared `target/` and the small `/tmp` tmpfs.

Environment: Kali Linux, kernel `7.2.15+kali-amd64`, x86_64, 4 CPUs, 3.8 GiB
RAM. Toolchain: rustc/cargo 1.100.0, Debian clang 21.1.8, gcc 16.2.0.
Audit started 2026-11-28T03:57Z (2026-11-27 local).

## Final verification — T-124/T-125/T-126 (2026-11-28)

Snapshot `/home/kali/qa-work/final-verify` of the working tree after the
primary fixed all three QA findings, with a private
`CARGO_TARGET_DIR=/home/kali/qa-work/target-final-verify` and
`TMPDIR=/home/kali/qa-work/tmp`. Source hashes:

```text
src/lib.rs    df82c8de48916aee79429ef53f12902baa602706dcfe22e9622d7e999c973dd0
src/value.rs  1f00b97a709639b04cd9911759ef5a16f3aee8c69ce67769665fd1c23cc41ae0
src/main.rs   234d80b9383c48933f2ee20d57e17de51a47da03c3835fb0511315791664eebe
src/module.rs 0a15c96272330a47a0e770e02fdb0c5d4a595d15da6e141a1138bdc540bc0a4e
tests/e2e.rs  43cb23d800e0da564cb4056057e7db5b4c1477eabb96089d72dc8b05ecf450d2
```

- `cargo test` -> **789 passed, 0 failed**, exit 0: 425 library + 15 CLI + 47
  AI + 137 e2e + 165 native. Log:
  `/home/kali/qa-work/cargo-test-final-full.log`.
- `cargo clippy --all-targets -- -D warnings` -> exit 0 (12.41 s).
- `cargo build --release` -> exit 0.
- `cargo fmt --check --all` on this snapshot -> exit 1, only because the new
  BOM test was wrapped across lines. The primary then ran rustfmt; the live
  tree's `tests/e2e.rs` hash became `22b342a7...` and
  `cargo fmt --check --all` -> exit 0. The diff between snapshot and live
  tree is formatting-only; all source hashes are unchanged.
- `VANTYX_CC=gcc cargo test --test native` -> 165/165 passed, exit 0
  (95.35 s). Log: `/home/kali/qa-work/final-gcc-native.log`.
- `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1
  UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1
  VANTYX_CC="$PWD/tools/clang-sanitize.sh" cargo test --test native` ->
  165/165 passed, exit 0 (140.95 s). Log:
  `/home/kali/qa-work/final-san-native.log`.
- `python3 tools/test_differential_fuzz.py` -> 9/9 tests passed.
- `python3 tools/differential_fuzz.py --binary .../debug/vantyx
  --iterations 30 --seed 11` -> 30/30 cases agreed.
- `python3 tools/fuzz_smoke.py --binary .../debug/vantyx --iterations 200`
  -> 9 depth/limit cases and 200 random/mutated inputs with controlled
  diagnostics.
- All five `examples/*.vtx` built and ran with interpreter-identical output.

Manual T-124 probes (all exit `64`, source byte-identical): entry,
imported module, relative spelling, symlink target, hard-link alias,
symlink-to-source output, and output through a directory symlink. With
`VANTYX_CC=/bin/false` the guard still exits `64`, proving it runs before the
compiler. Normal builds and missing output paths still succeed. The
documented residual `vantyx build - -o source.vtx < source.vtx` still
replaces the redirected file because the CLI cannot know the shell redirect
path; this is recorded as a scope limit in T-124.

Manual T-125 probes: BOM entry, BOM+CRLF entry, BOM imported module, and BOM
stdin all exit `0` with the expected output; an interior BOM and a duplicate
leading BOM still fail with `E0001` at `1:1`; a BOM-only file reports
`E0308: no entry point` rather than panicking.

Manual T-126 probes: self-referential list bound to `List[List[Int]]`,
self-referential map bound to `Map[Str, Map[Str, Int]]`, and a two-list
mutual cycle bound to `List[List[Int]]` all fail with `R0005`, exit `2`,
empty stdout. The same cycles remain valid coinductively under
`List[List[Any]]` and `Map[Str, Map[Str, Any]]`; `[[1]]` and
`{"k": {"a": 1}}` still bind; dynamic `List[Any]`/`Map[Any, Any]` mutation
still works; display still prints `[[...]]`/`{"self": {...}}`.

Test-count history independently observed: HEAD `6e58b37` -> 776; T-123 ->
783; T-123 + T-124 -> 784; T-123–T-126 -> **789**.

## T-127 / QA-F005 — hermetic tests and the Unix-socket path (2026-11-28)

T-127 snapshot `/home/kali/qa-work/t127-verify` (later patched by QA for the
fix experiment), `tests/e2e.rs` sha256 `61f53bca...06a0d`, `tests/native.rs`
sha256 `9cd3728f...d44844` before the QA patch:

- `cargo test` with `VANTYX_TEST_TMPDIR=/home/kali/qa-work/t` (short root) ->
  **789 passed** (425 lib + 15 CLI + 47 AI + 137 e2e + 165 native), exit 0.
  Log: `/home/kali/qa-work/cargo-test-t127-short-full.log`.
- `cargo test` with the default private target root
  `/home/kali/qa-work/target-t127-verify/tmp` (41 characters) -> **164
  passed, 1 failed**; `recursive_copy_special_files_match_in_both_engines`
  panicked at `tests/native.rs:5234` with `bind interpreter socket: ...
  "path must be shorter than SUN_LEN"`. Log:
  `/home/kali/qa-work/cargo-test-t127-full.log`.
- With the short root and `TMPDIR=/proc TMP=/proc TEMP=/proc`, the built
  binaries passed e2e **137/137** and native **165/165**, proving the clang
  children no longer use the process temp directory. Logs:
  `/home/kali/qa-work/t127-e2e-proc.log`,
  `/home/kali/qa-work/t127-native-proc.log`.

Length boundary with the single test (counter starts at 0):

```text
VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpX   (42) -> 107 -> pass
VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXY  (43) -> 108 -> panic, exit 101
VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXYZ (44) -> 109 -> panic, exit 101
```

Clean post-commit snapshot `git archive f1dbe36` at
`/home/kali/qa-work/f1dbe36-verify` (`tests/native.rs` sha256
`9cd3728f...d44844`) reproduced the same 107-pass / 108-fail / 109-fail
boundary. Standard CI path: `/home/runner/work/vantyx/vantyx/target/tmp`
(42 characters); with the full-suite counter at 120 the socket path is 109
characters. The local `/home/kali/Downloads/RustZ/target/tmp` path is 104,
which is why the primary's `/proc` run passed.

QA fix experiment on `/home/kali/qa-work/t127-verify` only: changed the two
workspace names to `cs-i` and `cs-n` (patched `tests/native.rs` sha256
`bd26ce51...87b79`). The single test then passed at roots 43 and 44; the full
native suite passed **165/165** and e2e **137/137** with
`VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXYZ` (44
characters) and `TMPDIR=/proc`. Logs:
`/home/kali/qa-work/t127-native-ci-length.log`,
`/home/kali/qa-work/t127-e2e-ci-length.log`.

The primary committed T-123–T-127 in `f1dbe36` and the status sync in
`150997d` before this fix. CI run 46113804180 on `150997d` passed **789/789**
(425 + 15 + 47 + 137 + 165), including
`recursive_copy_special_files_match_in_both_engines` in both the normal and
sanitizer native jobs. The runner's PID was short enough that the socket path
stayed at or below 107, so the failure is PID- and ordering-dependent: a
latent flake, not a deterministic CI failure. It reproduces locally because
this host uses 7-digit PIDs; the 42-character CI root plus a 7-digit PID and
a 3-digit counter yields 109. The proposed two-line patch is
`/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`; it remains a
recommended follow-up, not a blocker for the current green CI.

CI job evidence for run 46113804180 (2026-11-28):
`gh run view 46113804180 --json jobs` reports both jobs as `success`:
`Build, test, lint` (fmt, test, clippy, release) and
`Native tests under ASan and UBSan`. The log shows 425 lib + 15 CLI + 47 AI
+ 137 e2e + 165 native = 789, and
`recursive_copy_special_files_match_in_both_engines ... ok` in both native
jobs.

## 2026-11-28T03:57Z — coordination

- `git status --short --branch` -> `## main...origin/main`, clean.
- `git log -1` -> `6e58b37` (T-122), authored 2026-11-27 22:23:41 -0500.
- `ps` -> primary session running `target/debug/vantyx build` and clang for a
  differential-fuzz case; audit switched to read-only mode.
- `target/debug/vantyx` mtime 22:14, `target/release/vantyx` mtime 22:19.

## Baseline at HEAD `6e58b37`

Isolated copy with a private target directory:

```sh
git archive 6e58b37 | tar -x -C /home/kali/qa-work/head
cd /home/kali/qa-work/head
CARGO_TARGET_DIR=/home/kali/qa-work/target-head cargo test
```

Result: **776 passed, 0 failed** — 419 library unit + 15 CLI unit + 47 AI
integration + 132 e2e + 163 native; doc-tests 0. Log:
`/home/kali/qa-work/cargo-test-head.log`. This matches the documented
pre-T-123 gate exactly.

## Existing fuzz tooling

- `python3 tools/test_differential_fuzz.py` -> 9/9 tests passed.
- `python3 tools/differential_fuzz.py --iterations 60 --seed 7` ->
  60/60 generated programs agreed (exit code and stdout).
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx --iterations 500`
  -> passed (no panic/crash).

## Independent differential harness

Harness: `/tmp/opencode/vantyx-qa/qa_diff.py` (~90 hand-written cases across
scalars, control flow, collections, structs/enums, strings, numerics,
filesystem, runtime errors, and output behavior).

- Batch 3: no behavioral `DIFF`. Six `BUILD_FAIL` cases were invalid or
  expected-reject inputs (`int_min_div`, `reverse_empty_literal_boundary`,
  `empty_program`, `deep_parens`, `nul_escape`, `lexical_scope`), not
  divergences.
- Output-failure parity: writing to `/dev/full` -> `R0013`, exit 2; closed
  stdout -> exit 0 (Rust `println!` semantics); closed pipe -> `R0013`
  `Broken pipe (os error 32)`, SIGPIPE ignored.
- `read_file` boundary: exactly 64 MiB succeeds in both engines; 64 MiB + 1
  byte, embedded NUL, and invalid UTF-8 all produce identical `R0015`/exit 2.
- Unicode: 8/8 matched (Arabic/emoji `len`, indexing, `split`, `join`,
  `replace`); module runtime-error locations matched (`R0001`, `R0003`,
  `R0008`); deep and mutual recursion both raise `R0012` at depth 256 with
  exit 2.
- Strings/floats: 6/6 matched, including float/int formatting, text builtins,
  `replace` with an empty pattern (`R0016`), `assert` (`R0006`), and stable
  sorting.
- Filesystem safety: 8/8 matched (`qa_fs.py`) — atomic write replaces a
  symlink rather than its target; directory/missing-parent failures leave no
  temporary; recursive removal with an inner link; refusal of `.`/`..`;
  trailing slash; symlink-to-directory unlink.
- `E0401` boundary (`qa_boundary.py`): `Any`, `List[Any]`, `Map[Any,*]`,
  unannotated value returns, and dynamic file arguments are rejected with
  `E0401` plus help; a sentinel output file is untouched and no binary is
  emitted. Float patterns are rejected earlier by the checker (`E0113`).
- Lint/module/CLI (`qa_lint_module.py`): L0001–L0004, all-terminating
  `if`/`match`/`break`/`continue`, valid modules, traversal/absolute paths,
  cycles, missing files, symlink escape, invalid UTF-8 modules, CLI no-args,
  `run`, `build`, stdin, and invalid UTF-8 all behaved as documented after
  correcting two wrong test expectations.
- Examples: all five `examples/*.vtx` build natively and produce
  interpreter-identical output.

## Sanitizers and memory behavior

- ASan/UBSan via `tools/clang-sanitize.sh` on cyclic struct and nested
  collection display: clean, byte-identical output.
- Focused LeakSanitizer run: 2000 Int/Float prints clean; owned-composite
  print leaked no more than the no-print control (extra comparison values
  dominated). Nested display temporaries are reclaimed per ADR-0054/T-121.
- No `unsafe` in `src/` (only comments and test names). Native allocations
  are intentionally leaked in v0.1 except the documented display cases.

## T-121 re-verification

Reproduced the reported focused measurement on the same host: output hash
`e6174790...` matches the report; peak RSS 8,144–8,428 KiB (median ~8,308)
versus the claimed 8,296 KiB; wall time 623–642 ms. Within noise; the
reclamation claim is independently supported.

## T-123 adversarial audit

Snapshot of the uncommitted T-123 working tree (source hashes below), with a
private build:

```sh
cp -a working-tree /home/kali/qa-work/t123-verify   # excluding target/.git
CARGO_TARGET_DIR=/home/kali/qa-work/target-t123-verify cargo build
```

- `src/interp.rs` sha256 `153f84b1...e703e53`
- `src/value.rs` sha256 `829fb99a...82c9cf8`
- `tests/native.rs` sha256 `a6da02e5...4a1f01`
- `tests/e2e.rs` sha256 `5b933b85...1db457`
- `target/debug/vantyx` sha256 `30b0419a...e9dbf6` (also the live binary)

`qa_t123.py` (17 mutation/binding cases): initial 14/17; the three "failures"
were legitimate compile-time `E0301` rejections where the test used
statically known bad types. Replaced with dynamic `Any` values in
`qa_t123_ext.py`; all then raised runtime `R0005` as intended.

`qa_t123_ext.py` (16 deeper cases): after two test-authoring corrections
(enum payloads are positional; the return case initially returned valid
data), all pass:

- alias `push`, alias index assignment, `Any` hop, map key/value assignment,
  function parameter, struct constructor field, enum payload, return
  validation: `R0005`, exit 2, before mutation.
- contracts survive `pop`, `remove`, widening back to `List[Any]`, and
  `keys`/`values`.
- failed validation does not tighten; fully dynamic containers stay mutable.
- typed literals with dynamic elements/keys validate at construction.
- nested maps/lists, four-level nesting, and cyclic display (`[[...]]`) do
  not crash.

Cycle probe (QA-F003): self-referential `xs = [xs]` validated as
`List[List[Int]]`; the statically `Int`-typed `ys[0][0]` prints `[[...]]`,
compares false, and raises `R0005` under arithmetic. The same program is
accepted by the pre-T-123 release binary, so this is pre-existing, not a
T-123 regression. Mutual cycle `a -> b -> a` is rejected.

Full suite on the T-123 snapshot:

- First run (`cargo test` with default `TMPDIR=/tmp`): 160/164 native passed;
  four failures (`contains_and_push_accept_empty_list_literals`,
  `control_flow_matches`, `conversions_and_strings_match`,
  `copy_file_preserves_bytes_match`) all showed clang "unable to rename
  temporary ... No such file or directory". A concurrent process removed
  files under `/tmp` mid-compile; this is environmental, not code.
- Re-ran all four individually with `TMPDIR=/home/kali/qa-work/tmp`:
  4/4 passed.
- Full isolated rerun (`TMPDIR=/home/kali/qa-work/tmp`,
  `CARGO_TARGET_DIR=/home/kali/qa-work/target-t123-verify`,
  `cargo test > /home/kali/qa-work/cargo-test-t123-full.log 2>&1`):
  **783 passed, 0 failed**, `cargo_exit=0` — 422 library unit + 15 CLI unit +
  47 AI integration + 135 e2e + 164 native; doc-tests 0. The claimed T-123
  gate is independently verified on a snapshot whose source hashes match the
  live working tree. Log: `/home/kali/qa-work/cargo-test-t123-full.log`.

## T-123 verification gates (snapshot)

All on `/home/kali/qa-work/t123-verify` with
`CARGO_TARGET_DIR=/home/kali/qa-work/target-t123-verify` and
`TMPDIR=/home/kali/qa-work/tmp`:

- `cargo fmt --check --all` -> exit 0.
- `cargo clippy --all-targets -- -D warnings` -> exit 0.
- `cargo build --release` -> exit 0.
- `VANTYX_CC=gcc cargo test --test native` -> 164/164 passed, exit 0
  (103.70 s). Log: `/home/kali/qa-work/t123-gcc-native.log`.
- `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1
  UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1
  VANTYX_CC="$PWD/tools/clang-sanitize.sh" cargo test --test native` ->
  164/164 passed, exit 0 (118.62 s). Log:
  `/home/kali/qa-work/t123-san-native.log`.
- The first attempt at the sanitizer suite was interrupted by a harness
  `Shell.NotFoundError` after 120 passing tests; the clean rerun above is the
  recorded result. `fmt`/`clippy`/`release` results from that same log were
  unaffected (`fmt_exit=0`, `clippy_exit=0`, `release_exit=0`).

## Primary P0 fix observed in the live tree (post-snapshot)

While this audit was finishing, the primary agent modified `src/main.rs` and
added `tests/native.rs::build_refuses_to_overwrite_a_source_file` (mtime
23:56). The fix records canonical source paths from `SourceRegistry` names
relative to the canonical entry root and refuses an `-o` path that
`fs_safety::paths_refer_to_same_file_paths` matches, returning exit `64` with
`is also a source file` and `help: choose a different `-o` path`. This is the
same behavior as the QA-proposed patch. The snapshot's 783-test result does
not include this new test; the live tree now has one more native test.

### Independent post-fix verification

Fresh snapshot `/home/kali/qa-work/p0-verify` of the current tree
(`src/main.rs` sha256 `2b59ab0e...3dd47`, `src/fs_safety.rs` sha256
`1a4aaec4...1574`, `tests/native.rs` sha256 `d3018205...89224`, 165 native
tests), private target dir, isolated `TMPDIR`:

- `cargo test` -> **784 passed, 0 failed**, exit 0: 422 library + 15 CLI +
  47 AI + 135 e2e + 165 native, including
  `build_refuses_to_overwrite_a_source_file`. Log:
  `/home/kali/qa-work/cargo-test-p0-full.log`.
- `cargo fmt --check --all` -> exit 0.
- `cargo clippy --all-targets -- -D warnings` -> exit 0 (31.49 s).
- `cargo build --release` -> exit 0.
- `VANTYX_CC=gcc cargo test --test native
  build_refuses_to_overwrite_a_source_file` -> 1 passed.
- The same test with `VANTYX_CC="$PWD/tools/clang-sanitize.sh"` -> 1 passed.
- With `VANTYX_CC=/bin/false`, `build victim.vtx -o victim.vtx` still exits
  `64` and preserves the source, proving the guard runs before the compiler
  is invoked.
- Manual probes with the rebuilt debug binary, all exit `64`, sources
  byte-identical: entry (`-o victim.vtx`), relative spelling
  (`./sub/../sub/prog.vtx`), imported module, symlink target, hard-link
  alias, and symlink-to-source output. Normal build, missing output path,
  and unrelated output replacement still behave as before.

## QA-F001 reproduction (P0)

On the current T-123 binary:

```sh
printf 'fn main():\n    print("do not destroy")\n' > victim.vtx
cp victim.vtx before.txt
vantyx build victim.vtx -o victim.vtx   # exit=0
file victim.vtx                          # ELF 64-bit LSB pie executable
cmp -s before.txt victim.vtx             # exit=1
```

Also confirmed on `6e58b37`: imported-module output (`-o lib/helper.vtx`) and
symlink-target output (`link.vtx -> real.vtx`, `-o real.vtx`). The proposed
guard patch is at
`/tmp/opencode/vantyx-qa/proposed-build-output-guard.patch`; in an isolated
copy it makes `/tmp/opencode/vantyx-qa/regression_build_output.sh` pass 3/3
(exit 64, sources byte-identical) while normal builds still succeed.

## QA-F002 reproduction (P3)

```sh
printf '\xef\xbb\xbf' | cat - bom.vtx > bom_entry.vtx
vantyx run bom_entry.vtx
# error[E0001]: unexpected character `﻿` --> bom_entry.vtx:1:1, exit=1
```

Same error in an imported module (`helper.vtx:1:1`); the identical module
without the three BOM bytes runs. CRLF-only source runs normally.

## Cross-platform observations

- CRLF input accepted; UTF-8 BOM rejected (QA-F002).
- CI runs on Ubuntu only; generated C includes POSIX headers; Windows native
  support is documented as unverified. No claim of Windows support is made.

## Performance baseline (this host)

- Interpreter loop 1M: ~1,020 ms / 73.8 MB; native: ~5.5–8.7 ms / ~2 MB.
- Interpreter fib(24): ~208–228 ms; native: ~3.5–4.2 ms.
- Interpreter startup: ~4.3–7.0 ms / 3.8 MB.
- `vantyx build`: ~347–385 ms, ~105 MB peak (clang-dominated).
- 10M `range`: interpreter 2.44 s / 709 MB; native 0.425 s / 105 MB.

## Environment notes

- `/tmp` is a 1.9 GiB tmpfs and filled once during the audit; it was cleaned.
  Large builds and the sanitizer suite use `/home/kali/qa-work/` instead.
- The 10M `range` case is excluded from the default differential harness
  because the host has 3.8 GiB RAM and little free swap; it is not a language
  limit.

## Test coverage map

Behavioral coverage, not line percentages. Commands are the ones used in this
audit; `cargo test` means the isolated snapshot with a private target dir.

| Component | Exercised | Coverage | Gaps / risks | Command |
| --- | --- | --- | --- | --- |
| Lexer | Tokens, indentation, comments, Unicode/Arabic/emoji, CRLF, invalid UTF-8, NUL, deep input | Good; unit + e2e + smoke | BOM accepted (T-125); UTF-16/32 unsupported | `cargo test --lib lexer`; `fuzz_smoke.py` |
| Parser | Expressions, statements, blocks, patterns, imports, depth guards, malformed input | Good; unit + e2e + fuzz smoke | Combinatorial malformed cases only sampled | `cargo test --lib parser`; `fuzz_smoke.py` |
| Checker / types | Gradual `Any`, enums, exhaustiveness, returns, lints, `E0301`/`E0401` | Strong; 425 lib + 137 e2e | Cycle conformance fixed (T-126) | `cargo test`; `qa_boundary.py` |
| IR / lowering | Typed IR, call-site depth diagnostics, `E0401` rejection | Strong; differential native | No separate IR golden suite | `cargo test --test native` |
| Native C backend | Scalars, strings, collections, structs/enums, patterns, I/O, display, memory reclamation | Strong; 165 differential, 165 under ASan/UBSan | Windows; dynamic containers rejected by design | `VANTYX_CC=... cargo test --test native` |
| Interpreter | Full language oracle, display, errors, recursion, cycles | Strong; 137 e2e + unit + differential | Cycle conformance fixed (T-126) | `cargo test --test e2e` |
| Memory | T-121 print reclamation, T-122 facts, T-123 contracts, leaks | ASan/UBSan clean; LSan focused; RSS reproduced | No general reclamation (documented) | sanitizer command in AGENTS.md |
| CLI | `run`, `build`, `lint`, `repl`, stdin, usage errors, output failures | Good; e2e + CLI suite | P0 build output overwrite fixed (T-124, verified) | `qa_lint_module.py`; `regression_build_output.sh` |
| Modules | Confinement, traversal/absolute rejection, cycles, caps, BOM | Good; unit + e2e | BOM supported (T-125) | `qa_lint_module.py`; `cargo test --lib module` |
| Filesystem builtins | Read/write/atomic, dirs, glob, copy/rename, metadata, symlinks | Good; 8/8 differential + native | POSIX-only; Windows unverified | `qa_fs.py`; `cargo test --test native` |
| AI host | JSON/schema, extraction, embeddings, RAG, datasets, model inspection, metrics | 47 AI tests + offline suite | Live cloud not re-verified; prompt-injection/tool-permission paths only partially exercised | `cargo test --test ai`; `qa_ai.py` |
| REPL | History, completion, multi-line, replay | Historical pty smoke only | Not re-run in this audit | manual / historical |
| Fuzzers | 12 differential shapes, shrinking, smoke, self-tests | Good; 9/9 self-tests, 60/60 seed 7, 500 smoke | Bounded runs only; no long soak | `tools/test_differential_fuzz.py` |
| Cross-platform | Linux verified; CRLF accepted | Linux only | Windows/macOS not verified; POSIX headers | n/a on this host |
| Performance | Startup, loop, fib, build, 10M range | Measured once, multiple reps | No regression thresholds added | see performance section |
