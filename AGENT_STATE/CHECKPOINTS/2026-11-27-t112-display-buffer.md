# Checkpoint — T-112 single-buffer native display

Status: VERIFIED locally; commit `8911235`; CI pending
Date: 2026-11-27

## Objective

Remove quadratic string copying and retained intermediate prefixes from
native list, map, struct, and enum display by appending every part into one
growable `VantyxStrBuilder`, with byte-identical interpreter behavior.

## Reproduction

A native program built a 10,000-element `List[Int]` and printed
`len(str(xs))`. Before the change, the release binary took a median 754.707 ms
and peaked at 568,244 KiB RSS; the interpreter produced the same output in
about 15 ms with 4.33 MiB RSS. The native display path called
`vantyx_str_concat` once per element and retained every prefix because v0.1
never frees native allocations.

## Files changed

- `src/codegen_c.rs` — builder-based `vantyx_list_display`,
  `vantyx_map_display`, and generated struct/enum display functions.
- `tests/native.rs` — `large_collection_display_matches`.
- `benchmarks/display_ops.{vtx,c,rs,py}` — new tracked workload.
- `benchmarks/run.py` — registers `display_ops`.
- `benchmarks/results/2026-11-27-display-buffer.md` — focused before/after
  methodology and raw runs.
- `benchmarks/results/2026-11-27.{json,md}` — regenerated nine-workload run.
- `docs/decisions/ADR-0053-single-buffer-native-display.md`.
- `docs/memory_model.md`, `docs/benchmarks.md`, `README.md`, `AGENTS.md`,
  `docs/PROJECT_BASELINE.md`, `RESEARCH/INITIAL_REPOSITORY_AUDIT.md`,
  `TASKS.md`, `PROJECT_STATUS.md`, and `AGENT_STATE/`.

## Completed

- `vantyx_list_display` appends `[`, elements separated by `, `, and `]` into
  one builder.
- `vantyx_map_display` appends `{`, keys and values separated by `: ` and
  `, `, and `}` into one builder.
- Struct display functions append the type name, `field=` labels, rendered
  fields, and `)`.
- Enum display functions append payload variant names, fields, and `)`;
  payload-free variants keep the static string.
- Cycle/depth guards and the existing seen stacks are unchanged.
- Exact bytes, insertion order, quoting, cycle markers, depth markers, and
  early returns are unchanged.
- One differential native test and one tracked benchmark workload were added.
- ADR-0053 and all status documents were updated.

## Verification

- `cargo fmt --check --all` and
  `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 734 passed (404 library unit + 15 CLI unit + 47 AI
  integration + 118 e2e + 150 native, 0 failed).
- `VANTYX_CC=gcc cargo test --test native` — 150 passed.
- Sanitized native suite with `tools/clang-sanitize.sh` — 150 passed under
  ASan/UBSan with leak detection disabled.
- `cargo build --release` succeeded; all five examples matched interpreter
  output.
- `python3 tools/test_differential_fuzz.py` — 7 tests OK;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100
  agree; `python3 tools/fuzz_smoke.py --binary target/debug/vantyx` — 9
  depth/limit cases and 400 inputs with controlled diagnostics.
- 13 skills validated; link checker passed (125 links in 190 Markdown files).
- `python3 benchmarks/run.py --reps 5` verified identical output across all
  implementations and wrote `benchmarks/results/2026-11-27.{json,md}` with
  nine workloads; `display_ops` native median 4.674 ms vs interpreter
  15.134 ms, C 3.377 ms, Rust 3.724 ms.
- Focused same-batch comparison — median 754.707 ms → 5.737 ms and peak RSS
  568,244 KiB → 2,520 KiB; raw runs in
  `benchmarks/results/2026-11-27-display-buffer.md`.

## Remaining

1. Record the CI outcome when the unpushed commits are pushed (do not push
   without authorization).
2. Record the independent `qa-verifier` outcome: attempted on 2026-11-27 but
   the provider returned a free-usage limit error before the subagent did any
   work; no independent review result is claimed.
3. Select the next task.

## Risks

- Per-element temporary display strings are still allocated and leaked; only
  the quadratic prefix copying and retained prefixes were removed.
- The builder may over-allocate up to about twice the final byte length.
- The measurement is a single-machine microbenchmark, not a language-wide
  performance claim.
- Native allocations remain intentionally unreclaimed in v0.1.

## Next exact command

```sh
git status --short
git log -2 --oneline
```

T-112 is committed as `8911235`; the status-sync commit records that hash.
