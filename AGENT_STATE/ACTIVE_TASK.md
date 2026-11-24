# Active Task

## T-070/T-071/T-072 — Native condition and short-circuit fixes plus differential fuzzer

- Status: VERIFIED locally; commit and CI pending
- Priority: P0 (T-070, T-071), P2 (T-072)
- Dependencies: T-010, T-033, ADR-0008, ADR-0036

### Implemented

- `src/codegen_c.rs::condition`: wraps a rendered condition in one extra pair
  of parentheses only when it starts with the GNU statement-expression prefix
  `({`; ordinary comparisons keep their existing parenthesization and do not
  trigger clang's `-Wparentheses-equality`.
- `src/codegen_c.rs::binary_expr`: special-cases `and`/`or`. An effectful right
  operand is evaluated lazily through a `bool` statement expression with a
  conditional expression; an effect-free right operand uses ordinary
  `&&`/`||`.
- `tests/native.rs`: `conditions_with_calls_match`,
  `short_circuit_operands_match`, and
  `effectful_binary_operands_keep_left_to_right_order`.
- `tools/differential_fuzz.py`: deterministic, standard-library-only
  differential fuzzer with three typed program shapes, failure saving, and a
  compiler-availability skip; `.github/workflows/ci.yml` runs 50 iterations on
  seed 1.
- ADR-0036 and synchronized specification, README, status, baseline, and task
  documentation.

### Evidence

- `cargo test --test native conditions_with_calls_match` — 1 passed.
- `cargo test --test native short_circuit_operands_match` — 1 passed.
- `cargo test --test native effectful_binary_operands_keep_left_to_right_order`
  — 1 passed.
- Full gate: 584 tests (326 library + 15 CLI + 32 AI + 90 e2e + 121 native),
  clippy clean, debug and release builds succeed.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/differential_fuzz.py --iterations 200 --seed 500` — 200/200.
- All five examples matched interpreter/native output; fuzz smoke produced
  controlled diagnostics.

### Remaining

- Commit, push, and record the CI run.
- The fuzzer has no shrinking, sanitizer integration, or coverage measurement.
- The backend still depends on GNU C11 statement expressions.

## T-069 — Refuse destructive recursive-removal roots

- Status: VERIFIED, committed, pushed, and CI-green
- Priority: P0
- Dependencies: T-063, ADR-0031, ADR-0035

### Implemented

- `src/fs_safety.rs`: lexical `refused_removal_root` classifier with stable
  `RemovalRefusal` messages and five unit tests.
- `src/interp.rs`: `remove_dir_all` refuses destructive roots before any
  filesystem access.
- `src/codegen_c.rs`: `vantyx_remove_root_refusal` mirrors the rule and is
  called by `vantyx_remove_dir_all` before `vantyx_remove_tree`.
- `src/lib.rs`: module registration.
- Two e2e tests and two differential native tests; the previous test that
  locked the destructive `remove_dir_all(".")` behavior was replaced.
- ADR-0035 and synchronized specification/security/status documentation.

### Evidence

- `cargo test fs_safety` — 5 passed.
- `cargo test --test e2e recursive_removal` — 2 passed.
- `cargo test --test native recursive_removal` — 2 passed.
- Full gate: 581 tests (326 library + 15 CLI + 32 AI + 90 e2e + 118 native),
  clippy clean, debug and release builds succeed.
- CI run 45488725812 is green on `4982ab3` with 581 tests.
- All five examples matched interpreter/native output; fuzz smoke produced
  controlled diagnostics.

### Remaining

- CI run 45488725812 is green on `4982ab3`; run 45488152955 failed only the
  glob result-cap test because ext4 caps hard links per inode, fixed by
  spreading links across four anchors.
- Retry independent QA verification when subagent capacity is available.
- The check is lexical: intermediate links, `link/sub`, check-then-act races,
  Windows `\` spellings, and the absence of a sandbox remain documented limits.

## T-068 — Curl timeout error mapping

- Status: VERIFIED, committed, pushed, and CI-green
- Priority: P1
- Dependencies: T-039, T-052

### Implemented

- `src/ai/http.rs`: a shared `curl_failure` helper maps system-curl exit code
  28 (operation timeout) to retryable `AI_E_TIMEOUT`; other transport failures
  such as exit 7 remain `AI_E_CONNECT`.
- Both buffered (`send_curl`) and streaming (`stream_curl`) HTTPS paths use the
  same mapping. Loopback HTTP behavior is unchanged.
- Two unit tests: `curl_timeout_maps_to_a_retryable_timeout` and
  `curl_connect_failure_stays_a_connect_error`.

### Evidence

- `cargo test curl_` — 2 passed.
- Full gate: 573 tests (321 library + 15 CLI + 32 AI + 88 e2e + 117 native),
  clippy clean, debug and release builds succeed.
- Failure harness: 16 checks across 7 loopback scenarios, including 401, 429,
  and timeout.

## T-067 — Live cloud OpenAI-compatible verification

- Status: VERIFIED for the cloud path and re-verified at 20:59Z; committed,
  pushed, and CI-green
- Priority: P1
- Dependencies: T-038, T-039, T-043, T-045, T-046, T-052

### Evidence

- Endpoint: `https://generativelanguage.googleapis.com/v1beta/openai`.
- Chat model: `gemini-3.1-flash-lite`; embedding model:
  `gemini-embedding-001` at 768 dimensions.
- `doctor`: `status: ok`, 65 models.
- Chat: `VANTYX LIVE`, usage 22/4/26 in the first run and 13/4/17 in the
  20:59Z re-run.
- Streaming: counted words in both runs; first byte 1,976 ms in the first run
  and 14,958 ms in the re-run.
- Extraction: validated Ada Lovelace record after one repair, usage 237/105/342
  in both successful runs.
- Embeddings: two 768-component vectors, 28 prompt tokens.
- RAG: 3 documents, 3 chunks, 3 vectors, 206 prompt tokens; semantic search
  ranked `vantyx.md` first; evaluation recall@2 = 1.0, precision@2 = 0.5,
  MRR = 1.0.
- Failure harness: 16 checks across 7 scenarios (`AI_E_AUTH`,
  `AI_E_RATE_LIMIT`, `AI_E_TIMEOUT`).
- A retry returned HTTP 400 `User location is not supported`; a later full
  re-run at 20:58Z hit a transient HTTP 503 on extraction, and the 20:59Z run
  passed all 21 checks using `MY_API_KEY`; no bypass was attempted.
- Evidence files: `docs/ai/LIVE_VERIFICATION.md`,
  `docs/ai/live-evidence-2026-11-24.json`,
  `docs/ai/live-evidence-2026-11-24-retry.json`, and
  `docs/ai/live-evidence-2026-11-24-retry2.json`; no key value is stored.

## T-066 — Recursive directory copy

- Status: VERIFIED locally; committed, pushed, and CI-green
- Priority: P1
- Dependencies: T-058, T-059, T-061, T-063, ADR-0034

### Implemented

- `src/copy_tree.rs`: new-target validation, resolved self-containment,
  byte-preserving traversal, inner-symlink/special-file rejection, and
  100,000-entry / 128-depth caps.
- Interpreter, checker, IR, and C backend integration with `E0301`/`R0005`/
  `E0401` boundaries.
- Seven library unit tests, seven e2e tests, and seven differential native
  tests, including Unix link/special-file/non-UTF-8 cases.
- ADR-0034 and synchronized specification/status documentation.

### Evidence

- T-066 gate: 571 tests (319 + 15 + 32 + 88 + 117); seven native tests passed
  under ASan/UBSan with leak detection disabled.
- Current gate after T-069 and the T-067 re-verification: 581 tests
  (326 + 15 + 32 + 90 + 118).

### Remaining

- CI run 45488725812 is green on `4982ab3`.
- Retry independent QA verification when subagent capacity is available.
- T-040 remains blocked for the local-model half: no Ollama/llama.cpp server or
  model is installed.
