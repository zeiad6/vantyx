# Vantyx

**Script like Python. Ship native. Build AI-native.**

Vantyx is a pre-1.0 programming language with Python-like indentation, optional
type annotations, a tree-walking interpreter, a native GNU C11 backend for the
statically typed subset, and a dependency-free AI host toolkit.

![status](https://img.shields.io/badge/status-pre--1.0-orange)
![license](https://img.shields.io/badge/license-MIT-blue)
[![CI](https://github.com/zeiad6/vantyx/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/zeiad6/vantyx/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/zeiad6/vantyx?include_prereleases&label=release)](https://github.com/zeiad6/vantyx/releases)

```vantyx
fn main():
    let name = "World"
    print("Hello, {name}!")
```

> **Status: functional prototype.** Vantyx is not production-ready. The native
> backend supports a subset, native allocations are intentionally leaked in
> v0.1, scalar map keys use a hash index while struct/enum/list keys remain
> O(n), and there is no cycle collector or sandbox. See
> [PROJECT_STATUS.md](PROJECT_STATUS.md) for the verified state and exact limits.

## Why Vantyx

1. **Simple by default** — indentation blocks, `let`/`var`, optional type
   annotations, no required `main`.
2. **Safe by default** — checked arithmetic, bounds checks, no null, no
   uninitialized values, no user-visible `unsafe`.
3. **Gradual typing** — prototype without annotations; add them for
   compile-time guarantees and a native build.
4. **One semantics** — the interpreter is the oracle; native behavior is
   checked by differential tests. Unsupported constructs fail with `E0401`,
   never silently miscompile.
5. **Small and auditable** — the Rust compiler and AI host have zero
   third-party crates, and the generated C links only libc/libm.
6. **AI at the boundary, not in the core** — `vantyx ai` speaks an
   OpenAI-compatible protocol over HTTP; no model is bundled or downloaded, and
   remote network access is opt-in.

## Status

Vantyx is a **functional prototype**. The lexer, parser, gradual type checker,
tree-walking interpreter, typed IR, and native C11 backend are implemented and
tested. Closed enums with payloads, exhaustive `match`, scalar literal patterns
for `Int`/`Str`/`Bool`, and `Bool`-checked match guards work in both engines.
Native lists, insertion-ordered maps with a scalar-key hash index, structs,
enums, and UTF-8-aware string indexing/iteration are supported for statically
typed code. The REPL persists
bindings and declarations and provides line editing, history, Tab completion,
and expression echo. The AI host foundation is offline and tested: strict JSON,
JSON Schema validation, validated structured extraction with bounded repair,
provider abstraction, token budgets, embedding providers with a bounded cache,
BM25, typed tools, bounded agents, redacted metrics, and an OpenAI-compatible
HTTP adapter. Language-level AI syntax is not implemented yet; `vantyx ai` is the
supported surface.

| Need | Vantyx today |
| --- | --- |
| A quick script with no type annotations | Yes, through the interpreter |
| Optional static types and clear diagnostics | Yes |
| A native executable | Yes, for the statically typed subset |
| Offline AI chat with a mock provider | Yes, through `vantyx ai` |
| OpenAI-compatible local or cloud chat | Yes, via an explicit endpoint and `--allow-network` for remote HTTPS |
| Validated structured extraction | Yes, `vantyx ai extract` with a JSON Schema and bounded repair |
| Offline embeddings | Yes, `vantyx ai embed --provider hash`; lexical, not semantic |
| OpenAI-compatible embeddings | Yes, `/v1/embeddings`; actual server/model support varies |
| Language-level `from ai import Client` | Not yet (needs modules and async) |
| Bundled model, tokenizer, tensors, GPU | No |
| Modules, generics, closures, async, FFI | Not yet |
| Production reliability or a stable ABI | No |
| Fast scalar-key native maps | Yes, through an insertion-ordered hash index; struct/enum/list keys remain O(n) |
| Automatic memory reclamation | No |

A reproducible benchmark baseline guides optimization. Memory reclamation,
immutable string concatenation, pattern-matching extensions, and broader
optimization are the next major gaps.

## Quick start

```sh
cargo build
./target/debug/vantyx run examples/hello.vtx
./target/debug/vantyx check examples/hello.vtx
./target/debug/vantyx build examples/hello.vtx -o /tmp/hello
/tmp/hello
./target/debug/vantyx repl
./target/debug/vantyx ai providers
./target/debug/vantyx ai chat --provider mock "hello"
./target/debug/vantyx ai tokens "hello world"
```

Native builds require `clang` (override with `VANTYX_CC`). Programs using `Any`,
`List[Any]`, maps whose key/value types cannot be inferred statically, `Any`
enum payloads, or `match` on `Any` must currently run with `vantyx run`. A match
expression must be the final element of its statement or arm; bind it to a
variable before using it inside a call or literal.

## AI host tools

`vantyx ai` is a host-level CLI for local and cloud models. It is offline by
default and never bundles or downloads a model.

```sh
# No network, no model, deterministic output
./target/debug/vantyx ai providers
./target/debug/vantyx ai doctor --provider mock
./target/debug/vantyx ai chat --provider mock "hello"
./target/debug/vantyx ai chat --provider echo "hello"
./target/debug/vantyx ai chat --provider mock --json "hello"

# Validated JSON extraction with a bounded repair loop (offline fixture)
./target/debug/vantyx ai extract --provider mock --model demo \
  --schema examples/ai/person.schema.json \
  --response '{"name":"Ada","role":"engineer"}' \
  "Extract the person"

# Deterministic offline embeddings (lexical hash baseline, not semantic)
./target/debug/vantyx ai embed --provider hash --model demo \
  --dimensions 8 --json "compiler" "native backend"
printf 'alpha\nbeta\n' | ./target/debug/vantyx ai embed \
  --provider hash --dimensions 4 --json -

# A local OpenAI-compatible server (Ollama, llama.cpp, LM Studio, ...)
./target/debug/vantyx ai models --endpoint http://127.0.0.1:11434/v1
./target/debug/vantyx ai chat --provider openai --model qwen2.5:7b \
  --endpoint http://127.0.0.1:11434/v1 "Explain this error"
./target/debug/vantyx ai embed --provider openai --model nomic-embed-text \
  --endpoint http://127.0.0.1:11434/v1 --json "Explain this error"

# Remote HTTPS requires an explicit opt-in and a key in an environment variable
export VANTYX_AI_API_KEY_ENV=MY_API_KEY
export MY_API_KEY=...
./target/debug/vantyx ai chat --provider openai --model gpt-4.1-mini \
  --endpoint https://api.example.com/v1 --allow-network "hello"
```

- Loopback HTTP is treated as local. Remote HTTPS requires `--allow-network` or
  `VANTYX_AI_ALLOW_NETWORK=1`.
- API keys are read only from the named environment variable; they are never
  written to project files, metrics, or errors.
- `--metrics <path>` appends redacted JSONL (provider, model, latency, usage,
  status) with no prompts, responses, or tool arguments.
- Token counts from `vantyx ai tokens` are labelled estimates, not a model
  tokenizer.
- `vantyx ai extract` validates strict JSON against the documented schema
  subset, repairs invalid output at most three times, and never returns
  unvalidated model text. Server-side constrained decoding is not claimed.
- `vantyx ai embed` returns L2-normalized vectors from `mock`/`hash` (offline,
  deterministic lexical feature hashing, **not** a semantic model) or an
  OpenAI-compatible `/v1/embeddings` endpoint. Requests are capped at 128
  inputs, 8 KiB per input, 32 KiB total, and 4096 dimensions; the library cache
  is bounded and process-local.
- Streaming, model download/removal, keyring storage, vector persistence,
  hybrid retrieval, RAG evaluation, and live provider tests are not
  implemented. See [docs/ai/](docs/ai/),
  [ADR-0015](docs/decisions/ADR-0015-ai-host-foundation.md), and
  [ADR-0017](docs/decisions/ADR-0017-embedding-provider-and-cache.md).

## REPL

```sh
./target/debug/vantyx repl
```

The REPL keeps bindings, functions, structs, enums, and mutable globals across
entries. End a multi-line block with a blank line. Expression statements print
their value once; `print` output is not duplicated.

- **Tab** completes keywords, builtins, and names declared at the top level.
- **Up/Down** recall history; **Ctrl-A/E**, **Ctrl-U/K/W**, and **Ctrl-L** edit.
- **Ctrl-C** cancels the current entry; **Ctrl-D** exits.
- **`:help`**, **`:history`**, **`:names`**, and **`:quit`** are REPL commands.

History is stored in `~/.vtx_history` with mode 0600 on Unix. Set
`VANTYX_HISTORY` to another path, or to `off` to disable persistence. Multi-line
blocks are kept for the session but are not recalled. Piped input and systems
without `stty` fall back to canonical line input. See
[ADR-0013](docs/decisions/ADR-0013-repl-line-editing.md).

## Benchmarks

```sh
cargo build --release
python3 benchmarks/run.py --reps 5
```

The runner checks that all implementations print identical output, then times
Vantyx (interpreter and native), Python, C, and Rust on six equivalent workloads.
On the 2026-11-24 single-machine baseline, the native backend was in the same
performance band as C/Rust on all six workloads. The scalar-key map hash index
cut `map_ops` from 64.2 ms to 3.6 ms (C: 2.9 ms); immutable string concatenation
remains the largest measured gap at 8.3 ms versus C's 2.8 ms. See
[docs/benchmarks.md](docs/benchmarks.md) for methodology, caveats, and raw
results under `benchmarks/results/`. These are baseline measurements, not a
general performance claim.

## Documentation

- [Language specification](docs/language_specification.md)
- [Architecture](docs/architecture.md)
- [Compiler architecture](docs/compiler_architecture.md)
- [Type system](docs/type_system.md)
- [Memory model](docs/memory_model.md)
- [Benchmarks](docs/benchmarks.md)
- [Roadmap](docs/roadmap.md)
- [AI architecture](docs/ai/AI_ARCHITECTURE.md)
- [AI research](docs/ai/AI_RESEARCH.md)
- [AI security](docs/ai/AI_SECURITY.md)
- [AI roadmap](docs/ai/AI_ROADMAP.md)
- [ADR-0015: AI host foundation](docs/decisions/ADR-0015-ai-host-foundation.md)
- [ADR-0017: Embedding provider and bounded cache](docs/decisions/ADR-0017-embedding-provider-and-cache.md)
- [Historical requirements audit](RESEARCH/HISTORICAL_REQUIREMENTS_AUDIT.md)
- [Tasks](TASKS.md)
- [Project status](PROJECT_STATUS.md)
- [Release v0.1.0](https://github.com/zeiad6/vantyx/releases/tag/v0.1.0)
- [Launch and AI-discovery kit](docs/launch/)
- [`llms.txt`](llms.txt) — compact facts for AI assistants
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## Repository map

| Path | Purpose |
| --- | --- |
| `src/lexer.rs` | Indentation-aware lexer |
| `src/parser.rs` | Recursive-descent / Pratt parser |
| `src/checker.rs` | Name resolution and gradual type checking |
| `src/interp.rs` | Reference tree-walking runtime |
| `src/ir.rs` | Typed, backend-neutral IR |
| `src/codegen_c.rs` | GNU C11 native backend |
| `src/repl.rs` | Zero-dependency REPL editor, history, and completion |
| `src/ai/` | Dependency-free AI host: JSON, providers, schema, extraction, tools, agents, retrieval, HTTP |
| `src/ai_cli.rs` | `vantyx ai` commands |
| `src/diagnostic.rs` | Source spans and rendered diagnostics |
| `tests/e2e.rs` | End-to-end language tests |
| `tests/native.rs` | Differential interpreter vs native tests |
| `tests/ai.rs` | Offline AI CLI and loopback HTTP integration tests |
| `examples/` | Runnable programs, including enums and structs |
| `tools/fuzz_smoke.py` | Reproducible malformed-input and depth-limit fuzzing |
| `benchmarks/` | Equivalent Vantyx/Python/C/Rust workloads and the measurement runner |
| `docs/` | Specification, architecture, ADRs, research, AI docs, launch kit |
| `.opencode/agents/` | Specialized engineering subagents |

## License

MIT. See [LICENSE](LICENSE).
