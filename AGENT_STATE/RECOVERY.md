# Recovery

## Resume procedure

1. Read `AGENTS.md`, `TASKS.md`, `PROJECT_STATUS.md`, and all `AGENT_STATE/`
   files.
2. Run `git status --short` and `git log -5 --oneline`.
3. Compare the working tree with `AGENT_STATE/ACTIVE_TASK.md`; do not discard
   unexplained changes.
4. Run the targeted tests for the active task before the full gate.
5. Continue from the last verified checkpoint in
   `AGENT_STATE/CHECKPOINTS/`.

## Last recorded repository state (2026-11-26)

- `origin/main` and HEAD are at `1aba7ba` (differential-fuzzer control-flow
  coverage and bounded shrinking); CI run 45507195701 passed with T-077.
- T-078 through T-098 are uncommitted in the working tree. Earlier commits:
  `de0bac7` contains the rustfmt baseline and CI formatting gate (run
  45504177763 green), `66b4efc` contains T-075 result-enum propagation (run
  45503321172 green), `53a8b82` contains the expanded differential fuzzer (run
  45499019615 green), `5e1092f` contains the T-073–T-074 linter and
  return-path fix (run 45496041422 green), `a4fe86b` contains the
  T-070–T-072 native fixes plus the original fuzzer (run 45493833079 green),
  and `4982ab3` contains the glob result-cap portability fix (run
  45488725812 green).
- The uncommitted change set is verified locally: 700 Rust tests (391 library
  unit + 15 CLI unit + 41 AI integration + 113 e2e + 140 native), the full
  140-test native suite also under `VANTYX_CC=gcc`, clippy clean,
  `cargo fmt --check --all` clean, debug and release builds, five examples,
  seven Python self-tests, fuzzer 100/100 on seed 1 under clang after
  T-092/T-093, and fuzz smoke clean. T-094/T-095 added 10 dataset unit tests,
  3 dataset CLI tests, 13 `ai::schema` tests, one extract integration test, and
  one tools unit test; ADR-0044 and ADR-0045 record the decisions. T-096 added
  two differential headline tests covering nine error programs and four
  Unix-only file fixtures; both failed before the fixes. T-097 added 11 cases
  to the same headline test (filesystem failures with `(os error N)` and three
  malformed glob patterns); `diff3.py` went from 10 mismatches to 44/44. T-098
  added five dataset unit tests and two CLI tests for deterministic splits,
  overwrite refusal, invalid data, schema gating, and the non-empty fallback;
  ADR-0046 records the decision.
  Earlier runs
  agreed 200/200 on seeds 1, 2, 3, 500, and 1000 before T-081 changed one
  lowering branch.
- If the change set is absent, inspect `git status --short`, re-run the
  targeted T-078 through T-098 tests, the clang/GCC cross-compiler test, and the
  Python self-tests, then continue rather than discarding the work.
  Commits require explicit authorization.

## If the network or a provider is unavailable

1. Keep all local work; do not reset the tree.
2. Record the failure and the last successful command in this file.
3. Continue offline tasks (code, tests, docs, benchmarks with offline providers).
4. Retry external services with exponential backoff and jitter (15 s, 30 s,
   60 s, 120 s, 240 s, then capped); honor `Retry-After`.
5. Never repeat completed operations or hammer a rate-limited service.
6. Leave a resumable note in `ACTIVE_TASK.md` if the session ends.

Last recorded external state (2026-11-24): the Gemini OpenAI-compatible cloud
run succeeded and is documented in `docs/ai/LIVE_VERIFICATION.md`; an earlier
retry returned HTTP 400 `User location is not supported` for both available
credentials, a later re-run hit a transient HTTP 503, and a 20:59Z
re-verification using `MY_API_KEY` passed all 21 checks. Access can change and
is not guaranteed. No local inference server/model is installed. Do not retry
indefinitely or attempt to bypass provider restrictions.

## If a build or test is slow

- Native integration tests build and link C binaries; the full AI test binary
  can take several minutes on the 4-core reference machine. Prefer a targeted
  `cargo test --test ai <name>` while iterating, then run the full gate once.
- Do not poll background commands; wait for the completion notification.

## If history or attribution looks wrong

- `v0.1.0` was moved to the rewritten-history commit; old hashes in earlier
  status entries are historical.
- `.mailmap` maps old placeholder identities. Do not credit any contributor
  without verified work.
- Do not rewrite history again without explicit authorization.
