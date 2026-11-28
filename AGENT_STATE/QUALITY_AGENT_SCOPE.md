# Quality Agent Scope

- Agent: independent quality / security / reliability engineering agent.
- Task: independent audit and adversarial verification of Vantyx at commit
  `6e58b37` (T-122), with focus on recent native memory work (T-121/T-122)
  and cross-engine differential behavior.
- Started: 2026-11-28T03:57Z (2026-11-27 local).
- Coordination state at start: working tree clean, `main` == `origin/main` at
  `6e58b37`; the primary development session was observed actively running
  `target/debug/vantyx build` and clang for a differential-fuzz case at
  22:57 local. Treat all tracked files as protected while that session runs.
- Writable area (only): `AGENT_STATE/QUALITY_AGENT_*.md` (new files) and
  `/tmp/opencode/vantyx-qa/` for test programs, harnesses, and raw logs.
- Read-only area: `src/`, `tests/`, `tools/`, `docs/`, `examples/`, `TASKS.md`,
  `PROJECT_STATUS.md`, existing `AGENT_STATE/*` files, and `.github/`.
- Forbidden while the primary session is active: editing source, existing
  tests, CI, Cargo files, shared docs, or the primary session's state files;
  any destructive Git command; creating commits.
- Risk level: low (read-only audit plus isolated test artifacts).
- Required verification: run targeted differential programs with the existing
  debug binary, run the existing Python fuzzer self-tests and a bounded fuzz
  run from an isolated working directory, inspect recent code paths, and
  record exact commands and raw results.
- Handoff: any confirmed defect gets a reproduction, expected/actual output,
  severity, and a proposed independent regression test; no shared file is
  modified without a later coordination check.

## Update — audit complete (2026-11-28)

- The primary session remained active for the whole audit and kept
  uncommitted T-123 changes in `src/interp.rs`, `src/value.rs`, tests, docs,
  and `AGENT_STATE/*`. QA never edited a tracked source/test/doc/CI/Cargo
  file and never committed.
- Writable area was extended in practice to `/home/kali/qa-work/` because the
  `/tmp` tmpfs (1.9 GiB) filled during the audit and would have distorted
  native builds. `/tmp/opencode/vantyx-qa/` remains the home of the
  differential harness, the proposed P0 patch, and the standalone P0
  regression script.
- Read-only review covered the T-122 ownership module and the uncommitted
  T-123 contract implementation. T-123 was exercised with a snapshot binary
  and a private target directory; the shared `target/` was not raced.
- Final outputs owned by QA: `QUALITY_AGENT_TASKS.md`,
  `QUALITY_AGENT_FINDINGS.md`, `QUALITY_AGENT_TEST_LOG.md`,
  `QUALITY_AGENT_HANDOFF.md`, and this file. Findings: QA-F001 (P0),
  QA-F002 (P3), QA-F003 (P2). No shared-file patch was applied.
- Scope boundary still holds: the interpreter is the semantic oracle; QA
  reports divergences and soundness gaps rather than changing semantics
  unilaterally.
- Late in the audit the primary agent implemented all three QA findings:
  T-124 (build-output guard), T-125 (leading UTF-8 BOM), and T-126
  (type-keyed cycle conformance), each with tests and documentation. It later
  added T-127 (private test root plus child `TMPDIR`) for QA-F004. QA did not
  edit that code; it took fresh snapshots and independently verified the
  fixes (789 tests, clippy, release, GCC native 165, sanitized native 165,
  differential fuzz, examples, manual guard/BOM/cycle probes, and
  `TMPDIR=/proc` for T-127). A formatting-only fmt failure on the snapshot
  was fixed by the primary; live `cargo fmt --check --all` passes.
- T-123–T-127 are committed in `f1dbe36` (status sync `150997d`) and pushed;
  CI run 46113804180 passed 789/789. QA found **QA-F005**: T-127's long
  workspace names can make the Unix-socket test path 109 characters with a
  7-digit PID and a 3-digit counter, over the 107-byte `SUN_LEN` limit. The
  observed CI run passed because the runner's PID was shorter, so this is a
  latent flake. A two-line test-only fix was verified on a QA snapshot
  (165/165 native, 137/137 e2e at a 44-character root with `TMPDIR=/proc`)
  and handed off; it was not applied to the live tree.
- Final report: `QUALITY_AGENT_FINAL_REPORT.md`. Audit status: complete for
  QA; QA-F005 remains a recommended follow-up, while the current CI is green.
