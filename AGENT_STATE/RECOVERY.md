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

- `origin/main` and HEAD are at `a9a6d1a` (record the T-105 CI run), and CI
  run 45936329476 passed on it. `779783c` contains T-105 and passed run
  45936065460. `61cb9be` recorded the T-104 status-sync CI run and passed
  45934367601. `8805381` recorded the T-104 CI run and passed 45934076343. `4269010` contains call-site native
  call-depth diagnostics (T-104), and CI run 45933776137 passed on it.
  `a263cb3` recorded the T-103 CI run and passed run 45928756101. `46b7d54` contains deterministic scalar-list
  sorting (T-103), and CI run 45928466694 passed on it. `0a309c8` recorded the T-102 CI
  run and passed run 45926037169. `a331f2d`
  contains label-stratified JSONL dataset splits (T-102), and CI run
  45925745650 passed on it. `5d5655b` recorded the T-101 CI run, and run
  45923860569 passed on it. `c54c61c` contains deterministic JSONL dataset
  shuffling (T-101), and `33c966e` contains the native parity fixes, dataset
  tooling, and agent skills (T-078 through T-100); CI runs 45923211883 and
  45920832199 passed.
- The T-105 tree makes a direct empty list literal in `join` take
  `List[Str]`, so `join([], ",")` compiles natively instead of failing with
  `E0401`. It passes 724 tests (402 library unit + 15 CLI unit + 47 AI
  integration + 117 e2e + 143 native), the full native suite under
  `VANTYX_CC=gcc`, clippy and fmt clean, release build, five examples, seven
  Python self-tests, 100/100 fuzz cases on seed 1, fuzz smoke, and the
  thirteen-skill and 117-link validators; committed as `779783c` with CI run
  45936065460 green. `len([])`,
  `print([])`, and unannotated empty-list bindings still infer `List[Any]` and
  remain under the native `E0401` boundary.
- The T-106 working tree makes a direct empty list literal in `contains` or
  `push` take the value's element type, so `contains([], 1)` and
  `push([], 1)` build natively instead of failing with `E0401`. It passes 726
  tests (402 library unit + 15 CLI unit + 47 AI integration + 118 e2e + 144
  native), the full 144-test native suite under `VANTYX_CC=gcc`, clippy and
  fmt clean, the release build, and five examples; seven Python self-tests,
  100/100 seed-1 differential cases, fuzz smoke, the 13-skill validator, and
  the link checker (117 links in 178 Markdown files) passed. Commit and CI are
  pending. Non-empty arguments keep `E0301`; dynamic `Any` values and
  still-untyped empty literals remain under the native `E0401` boundary.
- The T-104 tree moves the native `R0012` depth-limit diagnostic to the
  failing call site, including across modules. It passed 723 tests
  (402 library unit + 15 CLI unit + 47 AI integration + 117 e2e + 142 native),
  the full native suite under `VANTYX_CC=gcc`, clippy and fmt clean, debug and
  release builds, five examples, seven Python self-tests, 100/100 fuzz cases on
  seed 1, fuzz smoke, and the thirteen-skill validator.
- The T-103 tree adds deterministic scalar-list sorting and passes the same
  723-test gate. ADR-0050 records the decision.
- Earlier commits: `1aba7ba` contains T-077 differential-fuzzer control-flow
  coverage and bounded shrinking (run 45507195701 green), `de0bac7` contains
  the rustfmt baseline and CI formatting gate (run 45504177763 green),
  `66b4efc` contains T-075 result-enum propagation (run 45503321172 green),
  `53a8b82` contains the expanded differential fuzzer (run 45499019615 green),
  `5e1092f` contains the T-073–T-074 linter and return-path fix (run
  45496041422 green), `a4fe86b` contains the T-070–T-072 native fixes plus the
  original fuzzer (run 45493833079 green), and `4982ab3` contains the glob
  result-cap portability fix (run 45488725812 green).
- The committed tree is verified: 707 Rust tests (395 library unit + 15 CLI
  unit + 43 AI integration + 113 e2e + 141 native), the full 141-test native
  suite also under `VANTYX_CC=gcc`, clippy clean, `cargo fmt --check --all`
  clean, debug and release builds, five examples, seven Python self-tests,
  fuzzer 500/500 on seed 7 and 100/100 on seed 1, the skill validator, and
  fuzz smoke clean. T-099 added native `range(...)` values and T-100 added
  exact dataset deduplication; ADR-0047 records the dedup decision.
- The committed tree at `c54c61c` passes 713 tests (399 library unit + 15 CLI
  unit + 45 AI integration + 113 e2e + 141 native), clippy and fmt clean, debug
  and release builds, five examples, seven Python self-tests, 100/100 fuzz
  cases on seed 1, and the thirteen-skill validator. T-101 adds deterministic
  seeded shuffling; ADR-0048 records the decision.
- The T-102 working tree passes 718 tests (402 library unit + 15 CLI unit + 47
  AI integration + 113 e2e + 141 native), clippy and fmt clean, debug and
  release builds, five examples, seven Python self-tests, 100/100 fuzz cases on
  seed 1, a supplemental 500/500 fuzz audit on seed 11, and the thirteen-skill
  validator. T-102 adds label-stratified splits; ADR-0049 records the decision.
- If a checkout is missing `33c966e`, fetch `origin/main` and inspect
  `git status --short`; do not discard unexplained work or rewrite history.

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
