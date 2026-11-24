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

## Last recorded repository state (2026-11-24)

- `origin/main` is at `de0bac7` (rustfmt baseline and CI formatting gate,
  CI run 45504177763 green). T-077, the tenth fuzzer shape plus bounded
  shrinking and self-tests, is uncommitted in the working tree.
- Earlier commits: `66b4efc` contains T-075 result-enum propagation (CI run
  45503321172 green), `53a8b82` contains the expanded differential fuzzer (CI
  run 45499019615 green), `5e1092f` contains the T-073–T-074 linter and
  return-path fix (CI run 45496041422 green), `a4fe86b` contains the
  T-070–T-072 native fixes plus the original fuzzer (CI run 45493833079
  green), and `4982ab3` contains the glob result-cap portability fix (CI run
  45488725812 green).
- The T-077 change set is verified locally: 621 Rust tests, clippy clean,
  `cargo fmt --check --all` clean, debug and release builds, seven Python
  self-tests, the tenth fuzzer shape 20/20 on seeds 1–20, and the full
  ten-shape fuzzer 200/200 on seeds 1 and 2. Seeds 3, 500, and 1000 were still
  running when this note was written.
- If the T-077 commit is not present, inspect `git status --short`, re-run
  `python3 tools/test_differential_fuzz.py`, re-run
  `cargo test`, and commit rather than discarding the changes.

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
