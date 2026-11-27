# Tool Inventory

Date: 2026-11-26
Host: Linux x86_64, Python 3.14.10, Git 2.53.0, GitHub CLI 2.105.0

## Available and used

| Tool | Version / status | Use |
| --- | --- | --- |
| `rustc` | 1.100.0 | Compiler |
| `cargo` | 1.100.0 | Build, test, clippy, fmt |
| `cargo clippy` | clean with `-D warnings` | Lint gate |
| `rustfmt` | available; `cargo fmt --check --all` clean (2026-11-25) | Formatting gate |
| `clang` | 21.1.8 | Native backend compiler; ASan/UBSan available |
| `gcc` | 16.2.0 | Second native compiler; `VANTYX_CC=gcc cargo test --test native` passes |
| `python3` | 3.14.10 | Differential fuzzer, fuzz smoke, AI harnesses, link checker |
| `git` | 2.53.0 | Version control |
| `gh` | 2.105.0 | CI inspection |
| GitHub Actions | Linux runner | Build, tests, clippy, differential fuzz, examples |
| OpenCode project skills | 12 under `.opencode/skills/`; advertised by the harness on 2026-11-26 | Reusable agent workflows for forensics, compiler work, Python research, runtime safety, AI systems, training, GPU/numerical work, security, benchmarking, orchestration, cross-platform work, and recovery (T-093) |

## Available but not installed

| Tool | Status | Consequence |
| --- | --- | --- |
| `cargo-nextest` | not installed | Standard `cargo test` only |
| `cargo-fuzz` / libFuzzer | not installed | Deterministic Python fuzzer and smoke harness instead |
| `cargo-audit` | not installed | No automated advisory scan; zero third-party Rust dependencies reduce exposure |
| `cargo-deny` | not installed | No license/source policy check |
| Miri | not installed | No UB checking of Rust code; no `unsafe` blocks found |
| Valgrind | not installed | ASan/UBSan used for native helpers |
| `perf` | not installed | No sampling profiler; benchmark timings only |
| `criterion` / `iai-callgrind` | not installed | Custom benchmark runner with raw JSON/Markdown results |

## External services

| Service | Status | Consequence |
| --- | --- | --- |
| OpenAI-compatible cloud endpoint | Verified 2026-11-24 and re-verified at 20:59Z with 21/21 checks; later geo-block and one transient 503 observed | Cloud access is not guaranteed; evidence is sanitized and references environment variable names only |
| Local model server (Ollama, llama.cpp, LM Studio) | Not installed | T-040 local-model evidence remains blocked |
| GPU / CUDA / ROCm | Not verified | No GPU, tensor, or training claims |

## Installation policy

Do not install optional tools without checking license, host compatibility,
security, and whether the task needs them. Prefer the existing zero-dependency
Rust toolchain and the system C compiler. Do not add a third-party Rust
dependency without an ADR.
