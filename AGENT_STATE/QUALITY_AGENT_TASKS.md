# Quality Agent Tasks

Status legend: `PENDING`, `IN PROGRESS`, `DONE`, `BLOCKED`.

| ID | Task | Priority | Status |
| --- | --- | --- | --- |
| QA-001 | Record coordination scope and protected files | P0 | DONE |
| QA-002 | Verify baseline build/test claims for `6e58b37` independently | P1 | DONE |
| QA-003 | Audit T-122 ownership facts (purity, conservatism, no codegen wiring) | P1 | DONE |
| QA-004 | Adversarial differential tests: print/display, floats, strings, collections | P1 | DONE |
| QA-005 | Runtime boundary tests: recursion depth, range limits, empty collections | P1 | DONE |
| QA-006 | Parser/lexer robustness: invalid UTF-8, deep nesting, long tokens | P2 | DONE |
| QA-007 | Native `E0401` boundary verification for unsupported dynamic constructs | P1 | DONE |
| QA-008 | Run existing Python fuzzer self-tests and a fresh seeded differential run | P1 | DONE |
| QA-009 | AI/JSON input robustness spot checks | P2 | DONE |
| QA-010 | Produce findings, regression proposals, and final handoff | P1 | DONE |
| QA-011 | Adversarial audit of uncommitted T-123 container contracts | P1 | DONE |
| QA-012 | Independently verify the T-123 test count on a snapshot tree | P1 | DONE |
| QA-013 | Verify native output under ASan/UBSan and re-check T-121 reclamation | P1 | DONE |
| QA-014 | Cross-platform input audit (CRLF, BOM, path assumptions) | P2 | DONE |
| QA-015 | Measure interpreter/native performance baseline | P2 | DONE |
| QA-016 | Independently verify the primary's in-progress P0 build-output fix | P0 | DONE |
| QA-017 | Re-run the full gate on the post-fix tree and record the new count | P1 | DONE |
| QA-018 | Independently verify T-124/T-125/T-126 on a fresh snapshot | P1 | DONE |
| QA-019 | Produce the final audit report and handoff | P1 | DONE |
| QA-020 | Verify T-127 test hermeticity and audit CI path lengths | P1 | DONE |
| QA-021 | Re-verify the committed `f1dbe36` tree for QA-F005 | P1 | DONE |

## Results summary

- QA-002: isolated `cargo test` at `6e58b37` -> **776 passed** (419 lib +
  15 CLI + 47 AI + 132 e2e + 163 native), matching the documented pre-T-123
  gate. QA-012: isolated `cargo test` on the T-123 snapshot -> **783 passed**
  (422 lib + 15 CLI + 47 AI + 135 e2e + 164 native), exit 0; the source
  hashes matched the live working tree.
- QA-003: `src/ownership.rs` is standalone; `verify_reclamation` is called
  only from its own tests, not from `codegen_c` or `ir`. Unknown constructs
  fall through to `MayEscape`; strings are never `Fresh`; aggregates are
  `Fresh` only "not aliased at construction". No behavioral risk found.
- QA-004: ~90-case differential harness, Unicode suite, string/float suite,
  filesystem suite, and fuzzer runs found no behavioral divergence between
  interpreter and native for supported constructs.
- QA-005: deep/mutual recursion both raise `R0012` at depth 256 with exit 2;
  `range` limits and empty collections agree; the 10M `range` case is
  excluded from the default harness only because of host memory pressure.
- QA-006: invalid UTF-8 and NUL are rejected identically; CRLF accepted; BOM
  rejected (QA-F002); deep parentheses reject without panic.
- QA-007: `Any`, `List[Any]`, `Map[Any,*]`, unannotated returns, and dynamic
  filesystem arguments are rejected with `E0401` + help, leaving no binary
  and not touching a pre-existing output file.
- QA-008: `tools/test_differential_fuzz.py` 9/9; 60/60 on seed 7;
  `fuzz_smoke.py --iterations 500` passed.
- QA-009: offline AI extract/embed/dedup/shuffle/split/index/metrics checks
  passed, including malformed and deeply nested JSON.
- QA-011: 33 scripted T-123 checks (5 test-author expectation corrections:
  static-vs-runtime `E0301`, enum syntax, and a valid return); no mutation
  bypass found. QA-F003 was a pre-existing binding-level cycle hole, not a
  T-123 mutation path, and is now fixed by T-126.
- QA-013: ASan/UBSan cyclic and nested display clean; focused LSan shows no
  display-rendering leak; T-121 peak-RSS result reproduced within noise.
- QA-014: CRLF accepted; BOM rejected in entry and module (QA-F002); CI is
  Ubuntu-only and generated C uses POSIX headers.
- QA-015: recorded in `QUALITY_AGENT_TEST_LOG.md`; no performance claim is
  made beyond the measured host.
- QA-016/QA-017: the primary implemented the P0 guard and added
  `build_refuses_to_overwrite_a_source_file`; an independent post-fix
  snapshot passed **784 tests** (422 lib + 15 CLI + 47 AI + 135 e2e + 165
  native), plus fmt, clippy, release, the new test under GCC and under the
  sanitizer wrapper, and a `/bin/false` compiler probe proving the guard runs
  before the compiler. Manual entry/module/symlink/hard-link/relative
  variants all return exit 64 with sources unchanged.
- QA-018: final snapshot `/home/kali/qa-work/final-verify` passed **789
  tests** (425 lib + 15 CLI + 47 AI + 137 e2e + 165 native), clippy,
  release, GCC native 165, sanitized native 165, 30/30 differential fuzz on
  seed 11, 200-iteration fuzz smoke, 9 fuzzer self-tests, and all five
  examples. T-124 guard variants, T-125 BOM entry/module/stdin/BOM+CRLF,
  and T-126 mismatched/valid cycle cases were probed manually. A transient
  fmt failure on the snapshot's BOM test was formatting-only; the live tree
  passes `cargo fmt --check --all` after the primary ran rustfmt.
- QA-019: final report written to
  `AGENT_STATE/QUALITY_AGENT_FINAL_REPORT.md`; findings QA-F001–QA-F003 are
  fixed and verified, QA-F004 is fixed by T-127, and QA-F005 (T-127's
  Unix-socket path at CI length) remains open as a latent flake while CI run
  46113804180 is green.
- QA-020: T-127's private root and child `TMPDIR` work: 789/789 with a short
  root, and e2e 137/137 + native 165/165 with `TMPDIR=/proc`. However, the
  Unix-socket test fails when the root plus PID and counter produce a
  108+ character path because `SUN_LEN` is 107; the 42-character CI root
  with a 7-digit PID and a 3-digit counter yields 109. QA-F005 raised.
- QA-021: clean `git archive f1dbe36` snapshot (`tests/native.rs` sha256
  `9cd3728f...d44844`) reproduced 107-pass / 108-fail / 109-fail. A two-line
  workspace-name shortening was verified on a QA snapshot (native 165/165,
  e2e 137/137 at a 44-character root with `TMPDIR=/proc`). CI run
  46113804180 on `150997d` passed 789/789 because the runner's PID was
  shorter; the test remains a latent flake, and the patch is a recommended
  follow-up rather than a blocker for the current green CI.

## Notes

- The primary session stayed active through the audit; no tracked source,
  test, doc, CI, or Cargo file was modified by QA.
- Confirmed defects are documented with reproductions before any proposed
  shared-file patch: QA-F001 (P0 build output overwrite, fixed and verified),
  QA-F002 (P3 BOM), QA-F003 (P2 cycle conformance), QA-F004 (P3 test
  hermeticity, fixed by T-127), and QA-F005 (P2 latent flake: T-127's
  Unix-socket path can exceed `SUN_LEN` with a 7-digit PID and a 3-digit
  counter; open in `f1dbe36`).
- T-123–T-127 are committed in `f1dbe36`; the status sync is `150997d`.
  CI run 46113804180 passed 789/789, including the socket test, because the
  runner's PID was short. The QA fix for QA-F005 is a recommended follow-up
  test-only change; QA did not apply it to the live tree.
- QA-012 uses a snapshot copy under `/home/kali/qa-work/t123-verify` with a
  private `CARGO_TARGET_DIR`; the shared `target/` is never raced.
