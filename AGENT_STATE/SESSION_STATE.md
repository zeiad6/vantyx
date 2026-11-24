# Session State

- Session ID: `ses_df3f90e90ffeFHPWt10GTzpwye`
- Updated: 2026-11-24
- Phase: Phase 6 native backend plus AI host Track B
- Branch: `main`
- Last commit: `a4fe86b` (native condition and short-circuit fixes plus the
  differential fuzzer); CI run 45493833079 passed on it
- Remote: `origin/main` at `a4fe86b`
- Tag: `v0.1.0` at the rewritten-history commit

## Current task

T-073 — Static linter and `vantyx lint`. T-074 — Reachability-aware return
checking. Status: VERIFIED locally; the change set is pending commit and CI.
The linter is a read-only AST pass with `L0001` unused binding, `L0002` `var`
never reassigned, and `L0003` unreachable statement; `vantyx lint` compiles
first, prints warnings, exits `0` for lint-only findings, and exits `1` for
compile errors. While adding it, the checker's `E0309` analysis was found to
inspect only the last statement of a block, so dead code after `return`
produced a false "not all paths return" error. `block_returns` now walks
reachable statements and stops at the first terminator. ADR-0037 records both
decisions.

T-070 — Parenthesize statement-expression conditions in the native backend.
T-071 — Preserve short-circuit evaluation for `and`/`or` in the native
backend. T-072 — Deterministic differential fuzzer. Status: VERIFIED,
committed in `a4fe86b`, pushed, and CI-green (run 45493833079). Both P0 defects
were found by the new fuzzer: any call or effectful expression used directly as
an `if`/`elif`/`while` condition produced invalid C (`if ({ ... })`), and
`and`/`or` hoisted both operands so the skipped operand was evaluated. The
fixes add one pair of parentheses only for statement expressions and lower
effectful short-circuit operands through a lazy conditional. Three differential
native tests cover calls in conditions, short-circuit call counts, and
non-short-circuit left-to-right order. ADR-0036 records the contract. The
fuzzer passes 100/100 on seed 1 and 200/200 on seed 500; CI runs 50 iterations
on seed 1.

T-069 — Refuse destructive recursive-removal roots. Status: VERIFIED,
committed in `e6ec597`, pushed, and CI-green (run 45488725812 on `4982ab3`).
Both engines now refuse `.`, `./`, `..`, `../`, every root whose final
component is `.` or `..` (including `link/.` and `link/..`), and the filesystem
root `/`/`//`/`///` with `R0015` before any filesystem access. Ordinary roots
such as `tree`, `tree/`, `./tree`, `/tmp/tree`, and `foo/.hidden` remain
accepted; empty paths keep the ordinary missing-path failure. Five
`src/fs_safety.rs` unit tests, two e2e tests, and two differential native tests
replace the prior test that locked the destructive `.` behavior. ADR-0035
amends ADR-0031. The check is lexical: `link/sub`, intermediate `.`/`..`
components, check-then-act races, Windows `\` spellings, and the absence of a
sandbox remain documented limits.

T-068 — Curl timeout error mapping. Status: VERIFIED, committed, pushed, and
CI-green.
System-curl exit code 28 now maps to retryable `AI_E_TIMEOUT` on both the
buffered and streaming HTTPS paths; other transport failures such as exit 7
remain `AI_E_CONNECT`. Two unit tests cover both codes. T-039 and T-052.

T-067 — Live cloud OpenAI-compatible verification. Status: VERIFIED for the
cloud path and re-verified at 20:59Z; committed, pushed, and CI-green. Against the Gemini
OpenAI-compatible endpoint, `doctor` reported `status: ok` with 65 models;
chat returned `VANTYX LIVE`; streaming returned the counted words; extraction
validated an Ada Lovelace record after one repair; two 768-dimension embeddings
were returned; a 3-document semantic index, search (`vantyx.md` first at
0.606483), and retrieval evaluation (recall@2 = 1.0, precision@2 = 0.5,
MRR = 1.0) succeeded. The loopback failure harness passed 16 checks across 7
scenarios, including 401, 429, and timeout. A later retry returned HTTP 400
`User location is not supported`; a re-verification at 20:59Z using
`MY_API_KEY` then passed all 21 live checks after an earlier re-run hit a
transient HTTP 503 during extraction. Cloud access is not guaranteed; no
bypass was attempted. Evidence: `docs/ai/LIVE_VERIFICATION.md`,
`docs/ai/live-evidence-2026-11-24.json`,
`docs/ai/live-evidence-2026-11-24-retry.json`, and
`docs/ai/live-evidence-2026-11-24-retry2.json`; no key value is stored.

T-066 — Recursive directory copy. Status: VERIFIED locally; committed,
pushed, and CI-green.
`copy_dir_all` requires an existing source directory and a new target, rejects
resolved self-containment, inner symbolic links, and special files, copies
regular-file bytes and raw entry names, and caps trees at 100,000 entries and
128 levels. ADR-0034 records the contract.

T-065 — Lexical glob expansion. Status: VERIFIED locally; committed, pushed, and CI-green.
`glob` runs in both the interpreter and native backend with `/`-separated
patterns, Unicode `*`/`?`/classes, whole-component `**` without symlink
traversal, explicit-dot hidden rules, byte-sorted deduplicated lexical results,
and `R0015`/`R0016` boundaries. ADR-0033 records the contract.

T-064 — File metadata probes. Status: VERIFIED locally; committed, pushed, and CI-green.
`file_size`, `is_file`, `is_dir`, and `is_symlink` run in both engines with
follow-symlink type predicates, `lstat` symlink detection, and `R0015`
failures. ADR-0032 records the contract.

T-063 — Recursive directory creation and removal. Status: VERIFIED locally;
committed, pushed, and CI-green. `make_dir_all` and `remove_dir_all` run in both engines with
symlink-safe removal and `R0015` failures. ADR-0031 records the contract.

T-062 — Lexical path builtins. Status: VERIFIED locally; committed, pushed, and CI-green.
ADR-0030 records the contract.

T-061 — Rename and file-copy builtins. Status: VERIFIED locally; committed, pushed, and CI-green. ADR-0029 records the contract.

T-060 — UTF-8 text builtins. Status: VERIFIED locally; committed, pushed, and CI-green.
ADR-0028 records the contract.

T-059 — Single-level directory operations. Status: VERIFIED locally; committed, pushed, and CI-green. ADR-0027 records the contract.

T-058 — Basic UTF-8 file I/O builtins. Status: VERIFIED locally; committed, pushed, and CI-green. ADR-0026 records the contract.

## Working tree

Modified `src/checker.rs`, `src/lib.rs`, `src/main.rs`, `src/lint.rs` (new),
`tests/e2e.rs`, `tests/native.rs`, `README.md`, `PROJECT_STATUS.md`,
`AGENTS.md`, `TASKS.md`, `docs/language_specification.md`,
`docs/PROJECT_BASELINE.md`, and `AGENT_STATE/`; untracked `docs/linting.md`,
`docs/decisions/ADR-0037-static-lints.md`, and
`AGENT_STATE/CHECKPOINTS/2026-11-24-t073-t074.md`. No generated binaries or
credentials are tracked. The T-070–T-072 change set was committed in `a4fe86b`;
`dfaad9b` recorded the prior CI verification.

## Verified commands (2026-11-24)

- `cargo build` — success
- `cargo build --release` — success
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — 348 library unit + 15 CLI unit + 32 AI integration + 96 e2e +
  122 native = 613 passed
- `cargo test --lib lint::tests` — 19 passed
- `cargo test --test e2e lint_` — 4 passed
- `cargo test --lib checker::tests::dead_code` — 2 passed
- `cargo test --lib checker::tests::break_before` — 1 passed
- `cargo test --test e2e dead_code` — 2 passed
- `cargo test --test native dead_code_after_return_matches` — 1 passed
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100
  agree, 0 rejected by both engines
- `python3 -m py_compile tools/differential_fuzz.py` — success
- CI run 45493833079 — success on `a4fe86b` with 584 tests and the
  differential fuzz step
- CI run 45488725812 — success on `4982ab3` with 581 tests; run 45488152955
  failed only the glob result-cap test because ext4 caps hard links per inode;
  the status documentation commit `0491c63` also passed run 45489360135
- `python3 tools/ai_failure_harness.py --binary target/debug/vantyx` — 16
  checks across 7 loopback scenarios, all passed
- Live Gemini run — doctor, chat, streaming, extraction, embeddings, RAG,
  search, and evaluation succeeded; re-verified at 20:59Z with 21/21 checks
  using `MY_API_KEY` after one transient 503; an earlier retry was geo-blocked
- All five `examples/*.vtx` matched interpreter vs native output
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases + 400 random/mutated inputs, controlled diagnostics
- `python3 tools/check_links.py` — link check after documentation sync
- Native recursive-copy tests under ASan/UBSan — 7 passed; glob — 10;
  metadata — 4; recursive-directory — 6; path — 3; rename/copy — 5; text — 3;
  directory — 3; file-I/O — 6; leak detection disabled for intentional v0.1
  leaks

## Next steps

1. Commit the T-073–T-074 change set, push it, watch CI, and record the run ID
   in `PROJECT_STATUS.md`, `AGENTS.md`, `AGENT_STATE/RECOVERY.md`, and the
   checkpoint.
2. Expand the differential fuzzer with more typed program shapes (functions
   with parameters and returns, strings/interpolation, nested collections,
   match expressions) and run multiple seeds; fix any mismatch it finds.
3. Retry the independent `qa-verifier` check for T-062–T-066 and
   T-073–T-074 when subagent capacity returns.
4. T-040 remains blocked on a local inference server/model; do not claim local
   evidence. The cloud path was re-verified at 20:59Z with 21/21 checks using
   `MY_API_KEY`, but access is not guaranteed; keep evidence sanitized.
5. Later candidates: recoverable `Result`/`?`, metadata objects or
   preservation, recursive move, native memory reclamation design, REPL
   multi-line history/field completion, and immutable concatenation/multibyte
   string iteration optimization.
