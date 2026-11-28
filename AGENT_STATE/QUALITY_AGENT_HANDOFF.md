# Quality Agent Handoff

- **Status:** independent audit complete. QA modified only the five
  `AGENT_STATE/QUALITY_AGENT_*.md` files (plus the final report); no shared
  source, test, doc, CI, or Cargo file was edited, no commit was made, and no
  destructive Git operation was used.
- **Audited tree:** HEAD `150997d` (status sync) over `f1dbe36`, which commits
  T-123–T-127. The commits were pushed; **CI run 46113804180 passed
  789/789** on 2026-11-28, including the socket test in both native jobs.
- **Follow-up (not a current blocker):** `f1dbe36` contains **QA-F005**, a
  latent flaky test. The runner's PID was short enough that the socket path
  stayed within `SUN_LEN`. On a host with 7-digit PIDs and a 3-digit
  workspace counter the path is 109 and the test fails. Apply the two-line
  fix as a follow-up; the patch is
  `/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`.
- **Verified counts:** T-123–T-126 snapshot -> **789 passed**; T-127 snapshot
  with a short private root -> **789 passed**; CI run 46113804180 on
  `150997d` -> **789 passed**. A synthetic 44-character root with a 7-digit
  PID fails `recursive_copy_special_files_match_in_both_engines` (164/165);
  107-character paths pass, 108/109-character paths panic.
- **Final gates (T-123–T-126 snapshot):** clippy and release exit 0; GCC
  native 165/165; sanitized native 165/165; 9/9 fuzzer self-tests; 30/30
  differential fuzz on seed 11; 200-input fuzz smoke; all five examples
  match. T-127's changes are test-only; its full suite passed 789 with a short
  root, and e2e 137/137 + native 165/165 with `TMPDIR=/proc`.

## Findings status

| ID | Severity | Summary | Status |
| --- | --- | --- | --- |
| QA-F001 | **P0** | `vantyx build <src> -o <src>` overwrote the source with an ELF and exited 0; also imported modules, symlink targets, hard links, and directory-symlink spellings | **FIXED as T-124, independently verified** (exit 64, sources unchanged); committed in `f1dbe36` |
| QA-F003 | P2 | Self-referential list/map satisfied a mismatched recursive type, so a statically `Int` expression evaluated to a container | **FIXED as T-126, independently verified** (`R0005`, exit 2); committed in `f1dbe36` |
| QA-F002 | P3 | UTF-8 BOM rejected as `E0001` in entries, modules, and stdin; undocumented, while CRLF was accepted | **FIXED as T-125, independently verified** (one leading BOM accepted; interior/duplicate still rejected); committed in `f1dbe36` |
| QA-F004 | P3 | Native/e2e tests are not hermetic against `/tmp` cleanup; clang temp-rename failures can make a green suite red | **FIXED as T-127** (private `test_root()` + child `TMPDIR`), but T-127 introduces QA-F005 |
| QA-F005 | P2 (latent flake) | T-127's `workspace("copy-special-*")` names can make the Unix-socket path 109 characters with a 7-digit PID and a 3-digit counter; Linux `SUN_LEN` is 107 | **OPEN in `f1dbe36`**; CI run 46113804180 passed 789/789 because the runner's PID was short; two-line fix verified on a QA snapshot (165/165 native, 137/137 e2e at a 44-character root with `TMPDIR=/proc`), not applied to the live tree |

Details, exact reproductions, root causes, and proposed fixes are in
`QUALITY_AGENT_FINDINGS.md`. The full report is
`QUALITY_AGENT_FINAL_REPORT.md`.

## Immediate actions for the primary agent

1. **Fix QA-F005 in a follow-up commit.** The current CI is green, but the
   test is flaky. Apply the two-line test-only change from
   `/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`:

   ```diff
   -    let interpreter_directory = workspace("copy-special-interpreter", source);
   +    let interpreter_directory = workspace("cs-i", source);
   ...
   -    let native_directory = workspace("copy-special-native", source);
   +    let native_directory = workspace("cs-n", source);
   ```

   A follow-up commit is safer than amending; do not rewrite published
   history. Verify with a path at least as long as CI:

   ```sh
   TMPDIR=/proc TMP=/proc TEMP=/proc \
   VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXYZ \
     cargo test --test native   # expect 165/165
   TMPDIR=/proc TMP=/proc TEMP=/proc \
   VANTYX_TEST_TMPDIR=/home/kali/qa-work/target-t127-verify/tmpXYZ \
     cargo test --test e2e      # expect 137/137
   ```

2. **Re-run the full gate after the fix and push.** CI run 46113804180
   already passed 789/789 on the current tree, so this is a robustness
   follow-up, not a release blocker. The T-123–T-126 snapshot passed clippy,
   release, GCC, ASan/UBSan, fuzzing, and examples; the T-127 changes are
   test-only.
3. **Keep the test gate hermetic (QA-F004).** T-127 now sets `TMPDIR`, `TMP`,
   and `TEMP` on child `vantyx` commands and keeps workspaces under
   `CARGO_TARGET_TMPDIR`. Adding `TMPDIR="$RUNNER_TEMP"` at the CI step level
   is still useful defense in depth and documents the requirement.
4. **Keep the documented `build -` scope limit visible.** T-124 protects the
   entry and every loaded module, but `vantyx build - -o source.vtx <
   source.vtx` still replaces the redirected file because the CLI cannot see
   the shell redirect path. The limitation is recorded in T-124; a warning
   or a `--force` policy would be a product decision, not a QA change.
5. **Optional follow-ups:** make `vantyx::compile(&str)` strip a BOM for
   library parity with the CLI (currently documented as not doing so); add a
   regression that pins duplicate-BOM rejection if intentional; measure T-126
   on deeply nested containers if performance becomes sensitive.

## Independently verified (no open finding except QA-F005)

- Baseline `6e58b37`: 776 tests. T-123: 783. T-123+T-124: 784. Final
  T-123–T-126: 789. T-127 with a short root: 789.
- Differential behavior: ~90-case harness, Unicode/Arabic/emoji, strings,
  floats, sorting, filesystem safety, module errors, recursion depth
  (`R0012`), output-failure parity (`R0013`, closed stdout, SIGPIPE).
- `E0401` boundary: `Any`, `List[Any]`, `Map[Any,*]`, unannotated returns,
  dynamic filesystem arguments; no binary emitted, sentinel untouched.
- T-123 adversarial mutation: 33 scripted checks (5 test-author expectation
  corrections); no mutation bypass. The only cycle hole was QA-F003, now
  fixed by T-126.
- T-124: entry, module, relative, symlink, hard-link, directory-symlink, and
  `/bin/false`-compiler variants all exit 64 with sources byte-identical;
  normal builds and missing outputs still work.
- T-125: BOM entry, BOM+CRLF, BOM module, BOM stdin accepted; interior and
  duplicate BOM rejected; BOM-only input reports `E0308`, not a panic.
- T-126: mismatched self-list, self-map, and mutual cycle rejected with
  `R0005`; valid coinductive `List[List[Any]]`/`Map[Str, Map[Str, Any]]`
  still accepted; dynamic containers and display/equality unchanged.
- T-127: private test root and child `TMPDIR` keep clang temporaries out of
  `/tmp`; e2e and native pass with `TMPDIR=/proc` when the root is short.
  The CI-length socket failure is QA-F005.
- Sanitizers: ASan/UBSan clean on the full 165-test native suite and on
  cyclic/nested display; focused LSan shows no display-rendering leak; T-121
  RSS/time reproduced within noise.
- AI host offline: extract/schema/metrics/embed/dedup/shuffle/split/index
  robustness passed. Live cloud access was **not** re-verified.
- All five examples build natively with interpreter-identical output.

## Artifacts

- Findings: `AGENT_STATE/QUALITY_AGENT_FINDINGS.md`
- Task tracker: `AGENT_STATE/QUALITY_AGENT_TASKS.md`
- Raw test log, coverage map, final commands:
  `AGENT_STATE/QUALITY_AGENT_TEST_LOG.md`
- Scope/coordination: `AGENT_STATE/QUALITY_AGENT_SCOPE.md`
- Final report: `AGENT_STATE/QUALITY_AGENT_FINAL_REPORT.md`
- Differential harness: `/tmp/opencode/vantyx-qa/qa_diff.py`
- P0 fallback patch (not applied):
  `/tmp/opencode/vantyx-qa/proposed-build-output-guard.patch`
- P0 regression: `/tmp/opencode/vantyx-qa/regression_build_output.sh`
- QA-F005 patch (not applied):
  `/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`
- T-123 suites: `/home/kali/qa-work/t123-probe/qa_t123.py`,
  `qa_t123_ext.py`
- Final logs: `/home/kali/qa-work/cargo-test-final-full.log`,
  `final-gcc-native.log`, `final-san-native.log`,
  `final-diff-fuzz.log`, `final-fuzz-smoke.log`
- T-127 logs: `/home/kali/qa-work/cargo-test-t127-full.log` (164/165,
  unpatched, 41-character root), `cargo-test-t127-short-full.log` (789,
  short root), `t127-native-proc.log`, `t127-e2e-proc.log`,
  `t127-native-ci-length.log`, `t127-e2e-ci-length.log`
- Clean committed snapshot: `/home/kali/qa-work/f1dbe36-verify`
- Earlier logs: `/home/kali/qa-work/cargo-test-head.log`,
  `cargo-test-t123-full.log`, `cargo-test-p0-full.log`,
  `t123-gcc-native.log`, `t123-san-native.log`
- Independent suites: `/home/kali/qa-work/qa_lint_module.py`,
  `qa_deep.py`, `qa_ai.py`, `qa_fs.py`, `qa_boundary.py`, `qa_strings.py`

## Coordination notes and remaining risks

- `f1dbe36` and `150997d` were pushed; CI run 46113804180 passed 789/789.
  The verified count is valid for the recorded commit, not automatically for
  any later edit. Re-run after the QA-F005 fix.
- Native tests under memory pressure: keep `TMPDIR` on a real disk. `/tmp` is
  a 1.9 GiB tmpfs on this host and filled once during the audit.
- Windows/macOS were not tested. CI is Ubuntu-only and generated C uses POSIX
  headers; Windows native support remains unverified and should not be
  claimed.
- The 10M `range` case is excluded from the default differential harness only
  because this host has 3.8 GiB RAM; it is not a language limit.
- QA files are untracked records. Do not commit them or mix them into a
  T-123–T-127 follow-up without explicit coordination.
- QA-F005 is test-only and latent: CI run 46113804180 is green, but a future
  runner with 7-digit PIDs and a 3-digit counter can fail the socket test.
  Fix it to keep CI deterministic; it does not affect any product behavior.
