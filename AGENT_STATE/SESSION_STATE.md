# Session State

- Session ID: `ses_df3f90e90ffeFHPWt10GTzpwye`
- Updated: 2026-11-24
- Phase: Phase 6 native backend plus AI host Track B
- Branch: `main`
- Last commit: `4982ab3` (glob result-cap test portability; `e6ec597` contains T-058–T-069)
- Remote: `origin/main` at `4982ab3`
- Tag: `v0.1.0` at the rewritten-history commit

## Current task

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

Clean at `4982ab3` plus the CI-status documentation update. The T-058–T-069
change sets were committed in `e6ec597`; `4982ab3` fixed the glob result-cap
test portability for ext4's per-inode hard-link limit. No generated binaries
or credentials are tracked.

## Verified commands (2026-11-24)

- `cargo build` — success
- `cargo build --release` — success
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — 326 library unit + 15 CLI unit + 32 AI integration + 90 e2e +
  118 native = 581 passed
- `cargo test curl_` — 2 passed
- `cargo test fs_safety` — 5 passed
- `cargo test --test e2e recursive_removal` — 2 passed
- `cargo test --test native recursive_removal` — 2 passed
- `cargo test --test native glob_result_cap_matches_in_both_engines` — 1
  passed after spreading hard links across four anchors
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

1. Retry the independent `qa-verifier` check for T-062–T-066 when subagent
   capacity returns.
4. T-040 remains blocked on a local inference server/model; do not claim local
   evidence. The cloud path was re-verified at 20:59Z with 21/21 checks using
   `MY_API_KEY`, but access is not guaranteed; keep evidence sanitized.
5. Select the next unblocked task. Candidate gaps: recoverable `Result`/`?`,
   metadata objects or preservation, recursive move, native memory reclamation
   design, REPL multi-line history/field completion, or immutable
   concatenation/multibyte string iteration optimization.
