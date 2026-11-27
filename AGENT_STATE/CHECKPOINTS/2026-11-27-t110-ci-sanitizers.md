# Checkpoint — T-110 CI sanitizers and fuzz smoke

Date: 2026-11-27
Status: VERIFIED; committed as `02cdf10`; CI pending

## Objective

Detect native memory errors and undefined behavior in generated C programs
automatically, and close the documented gap where the fuzz smoke was manual.
The native differential suite compiled generated programs with plain clang, so
a use-after-free, out-of-bounds access, or undefined operation would only fail
when it happened to change observable behavior.

## What changed

- `tools/clang-sanitize.sh` is a `VANTYX_CC` wrapper that invokes
  `clang -fsanitize=address,undefined -fno-sanitize-recover=all
  -fno-omit-frame-pointer -g` and forwards every argument.
- `.github/workflows/ci.yml` adds a `sanitizers` job on `ubuntu-latest`. It
  installs clang, checks that the wrapper is executable, and runs
  `cargo test --locked --test native` with
  `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1` and
  `UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1`.
- The main `test` job runs `python3 tools/fuzz_smoke.py --binary
  target/debug/vantyx`.
- `AGENTS.md` documents the local sanitizer command, and `docs/security.md`
  records the CI coverage and its limits.

## Why leak detection is disabled

Native allocations are intentionally leaked in v0.1. Enabling LeakSanitizer
would report every string, list, map, and enum allocation as a leak and mask
real memory errors in noise. ASan still checks use-after-free, buffer
overflows, invalid frees, and double frees; UBSan still checks undefined
operations. A future reclamation design must add leak checks separately.

## Verified commands (2026-11-27)

- `sh -n tools/clang-sanitize.sh` — syntax OK; the script is executable.
- PyYAML parse of `.github/workflows/ci.yml` — jobs are `test` and
  `sanitizers`.
- `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1
  UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1
  VANTYX_CC="$PWD/tools/clang-sanitize.sh" cargo test --test native` —
  147 passed.
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs produced controlled diagnostics.
- `cargo test` — 731 passed (404 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 147 native).
- `cargo fmt --check --all` and `cargo clippy --all-targets -- -D warnings` —
  clean.
- `cargo build --release` — succeeded; all five examples matched
  `vantyx run` output.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- Skill validator — 13 skills valid; link checker — all relative links
  resolve.

## Blockers and limits

- Independent `qa-verifier` review is unavailable (subagent provider
  free-usage limit), so this is local verification.
- Only generated C programs are instrumented. The Rust compiler and test
  binary are not built with sanitizers.
- The sanitizer job runs on Linux with clang. Windows and macOS native
  support remain unverified, and no other sanitizer (for example TSan) is
  applicable because the runtime is single-threaded.
- The fuzz smoke is a bounded deterministic smoke test, not coverage-guided
  fuzzing; the differential fuzzer remains the main semantic check.
- CI itself has not run yet because the commit has not been pushed.

## Next exact commands

1. Committed as `02cdf10`; record the CI run after a push is authorized.
2. Continue P0/P1 work: run the differential fuzzer with sanitized native
   builds, then probe other documented limits for boundary mismatches and
   evaluate native memory reclamation with measured evidence.
