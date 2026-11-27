# Checkpoint — T-113 display-focused differential fuzz shape

Status: VERIFIED locally; commit `d8a476d`; CI pending
Date: 2026-11-27

## Objective

Exercise the native display paths changed by T-112 with randomized values that
the existing `nested_program` shape does not generate: multi-node and
self-referential struct/list cycles, deep recursive enum chains, and maps with
struct, list, and enum keys.

## Reproduction / coverage gap

`tools/differential_fuzz.py` had eleven registered generators but the module
docstring still described ten shapes. `nested_program` prints nested lists,
maps, structs, and enums, but it does not build cycles, a deep recursive enum,
or structural map keys. T-112 changed every native display path, so these
shapes needed randomized differential coverage rather than fixed regression
tests alone.

## Files changed

- `tools/differential_fuzz.py` — new `display_program` generator, registered
  after `nested_program`; module docstring updated to twelve shapes.
- `tools/test_differential_fuzz.py` — deterministic/well-formed checks for the
  new generator and a determinism check for every registered generator; 9
  tests total.
- `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`,
  `docs/PROJECT_BASELINE.md`, `RESEARCH/INITIAL_REPOSITORY_AUDIT.md`, and
  `AGENT_STATE/`.

## Completed

- `display_program` emits only native-supported constructs:
  - a three-node `Node` cycle (`root -> child -> leaf -> root`);
  - a self-referential `Node` list;
  - `Chain` recursion of random depth 0–60;
  - `Map[Key, Int]` with struct keys and list fields;
  - `Map[List[Int], Str]` with structural list keys;
  - `Map[Shape, Int]` with enum keys;
  - nested empty typed lists/maps;
  - quoted UTF-8 strings from the shared literal pool;
  - a list containing cyclic nodes.
- The generator is deterministic for a fixed seed and uses no `Any`, no map or
  list cycle that would require dynamic typing, and no unsupported construct.
- `GENERATORS` registers the shape and the docstring now says twelve.
- Python self-tests check determinism, well-formed brace balance, required
  declarations and keys, and every registered generator.

## Verification

- `python3 tools/test_differential_fuzz.py` — 9 tests OK.
- Seven generated programs from seeds 1, 2, 3, 4, 5, 7, and 11 built and
  matched interpreter stdout and exit code.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 2` — 100/100.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 3` — 100/100.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 500` — 100/100.
- `cargo fmt --check --all` and
  `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 734 passed (404 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 150 native, 0 failed).
- `VANTYX_CC=gcc cargo test --test native` — 150 passed.
- Sanitized native suite with `tools/clang-sanitize.sh` — 150 passed under
  ASan/UBSan with leak detection disabled.
- `cargo build --release` succeeded; all five examples matched interpreter
  output.
- `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs with controlled diagnostics.
- 13 skills validated; link checker passed (125 links in 192 Markdown files).

## Remaining

1. T-113 is committed as `d8a476d` (not pushed; no push without
   authorization); CI is pending.
2. Record the CI outcome when the unpushed commits are pushed.
3. Independent `qa-verifier` review remains unavailable because the subagent
   provider returned a free-usage limit error; no independent review is
   claimed.
4. T-114 is queued to free nested display temporaries after they are appended;
   it has not started.

## Risks

- The generator keeps collections small and enum depth at or below 60 so each
  case completes quickly; it does not cover every display shape.
- Cycles through maps or lists that would require `Any` are outside the native
  subset and are not generated.
- Randomized agreement expands coverage but does not prove the absence of
  display defects.
- No Rust, language, or native behavior changed; the 734-test count is
  unchanged.

## Next exact command

```sh
git status --short
git diff --stat
```

Then commit the T-113 tree, followed by a status-sync commit that records the
T-113 hash and the CI-pending state.
