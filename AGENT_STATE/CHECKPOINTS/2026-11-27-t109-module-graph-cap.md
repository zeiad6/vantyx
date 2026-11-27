# Checkpoint — T-109 module-graph file cap

Date: 2026-11-27
Status: VERIFIED locally; commit and CI pending

## Objective

Make the documented 256-file module-graph limit reachable. The loader checked
`self.units.len() + 1 >= MAX_MODULES` before inserting a new file. Because the
entry file is already in `units`, a graph with 255 loaded files rejected the
256th file, so the effective limit was 255.

## What changed

- `src/module.rs::Loader::unit` now rejects only when
  `self.units.len() >= MAX_MODULES`. Duplicate imports still return the cached
  unit before the cap check.
- `src/module.rs` tests add `write_import_graph`, which writes `count` modules
  with unique functions plus an entry file that imports each of them.
- `module_cap_accepts_exactly_max_modules` builds an entry plus 255 imports,
  asserts no errors, and asserts 256 flattened declarations.
- `module_cap_rejects_one_module_over_max` builds a 257-file graph, asserts
  exactly one `E0507`, checks the `limit of 256 files` wording, and asserts no
  semantic model.

## Reproduction

Before the fix, `load_file` on an entry plus 255 imports reported:

```
error[E0507]: module graph exceeds the limit of 256 files
```

even though the graph contained exactly 256 files. After the fix the same
graph loads without errors and flattens 256 declarations; adding one more
import produces one `E0507`.

## Verified commands (2026-11-27)

- `cargo test --lib module_cap` — 2 passed.
- `cargo fmt --check --all` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test` — 731 passed (404 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 147 native).
- `VANTYX_CC=gcc cargo test --test native` — 147 passed.
- `cargo build --release` — succeeded; all five examples built natively and
  matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- `python3 tools/fuzz_smoke.py --binary target/release/vantyx` — 9 depth/limit
  cases and 400 random/mutated inputs produced controlled diagnostics.
- Skill validator — 13 skills valid; link checker — all relative links
  resolve.

## Blockers and limits

- Independent `qa-verifier` review is unavailable (subagent provider
  free-usage limit), so this is local verification.
- Only the file-count boundary changes. Import depth, per-module and total
  source caps, path confinement, cycles, and duplicate-import caching are
  unchanged.
- The test writes 256 files under the system temporary directory and removes
  the directory on drop. It does not exercise the CLI; the loader is the
  shared path used by `vantyx run`, `check`, `lint`, and `build`.

## Next exact commands

1. `git diff --check`; review the diff; commit the T-109 implementation and
   status records.
2. Record the new CI run in the next status commit.
3. Continue P0/P1 work: add a sanitizer CI job for the native suite, then probe
   other documented resource limits for boundary mismatches.
