# ADR-0041: Offline model inspection and SHA-256 provenance

- Status: Accepted
- Date: 2026-11-25
- Related: T-090, ADR-0007 (zero dependencies), ADR-0015 (AI host boundary),
  `docs/ai/AI_ARCHITECTURE.md`, `docs/ai/MODEL_INSPECTION.md`

## Context

The AI host can talk to OpenAI-compatible servers and can index local text, but
it cannot answer basic questions about a model file that a user already has:
what container is it, what architecture and quantization does it declare, how
many parameters and tensors does it contain, are the tensor ranges consistent,
and what is its content hash?

Users were expected to reach for `llama.cpp`, Python, or `sha256sum`. That is a
poor fit for a dependency-free host whose security posture says "never download
a model and never execute model code." It also makes reproducible experiments
harder: a prompt or RAG index that names a model without a content hash cannot
be reconstructed if the file changes.

The obvious alternatives were:

1. **Shell out to `sha256sum` and a format-specific tool.** Platform-dependent,
   adds a process boundary, and the format tool may load or execute model code.
2. **Add a Rust crate such as `sha2` and a GGUF/Safetensors parser.** Violates
   the zero-third-party-dependency rule in ADR-0007 and expands the supply
   chain for a read-only inspection feature.
3. **Implement a bounded parser and SHA-256 in the host.** Keeps the dependency
   graph empty, works offline, and lets the project define its own resource
   limits and error codes.

## Decision

Add `src/ai/model.rs` and the `vantyx ai inspect <file>` command.

The inspector:

- Recognizes GGUF versions 2 and 3 (little-endian) by the `GGUF` magic, and
  Safetensors by an 8-byte little-endian header length followed by a JSON
  object.
- Parses only the metadata and tensor directory. It never loads tensor payloads
  into memory, never executes model code, and never downloads anything.
- Streams the entire file through an in-tree SHA-256 implementation and reports
  the lowercase hexadecimal digest, byte count, format, tensor count, parameter
  count, architecture/name/context/embedding/layer metadata when present,
  quantization/file type, tokenizer model and token count when present, dtype
  histogram, and warnings.
- Validates that every tensor range lies inside the declared data section and
  that known ranges do not overlap. GGUF block sizes are checked against the
  element count; Safetensors dtype size, shape, alignment, and byte length are
  checked exactly.
- Reports unsupported or corrupt inputs as `AI_E_FORMAT` and I/O failures as
  `AI_E_IO`. A format error exits 1 and prints a concrete help line.
- Adds `--json` for a stable machine-readable summary and `--metadata` to print
  every parsed metadata entry in text mode.

Explicit limits:

| Limit | Value |
| --- | --- |
| GGUF metadata and tensor directory | 256 MiB |
| One GGUF string | 64 MiB |
| GGUF metadata array | 50,000,000 elements |
| GGUF metadata entries | 100,000 |
| GGUF tensor descriptors | 1,000,000 |
| Tensor rank | 8 |
| Safetensors JSON header | 100 MiB |
| Safetensors metadata entries | 10,000 |

The SHA-256 implementation is incremental and tested against the standard
empty, `abc`, two-block, and one-million-`a` vectors. Hashing is streamed, so
file size is limited by available I/O time rather than memory. A file that
changes while it is being inspected produces a warning and the hash covers the
bytes actually read.

## Consequences

Positive:

- A user can inspect and fingerprint a local model without installing a model
  runtime, without network access, and without executing untrusted code.
- The SHA-256 digest can be recorded next to prompts, evaluation fixtures, and
  RAG indexes for reproducibility.
- Tensor-range validation catches truncated files, overlapping ranges, wrong
  Safetensors byte lengths, and unknown quantization layouts before a runtime
  tries to load them.
- The repository keeps zero third-party Rust dependencies and the native
  backend is untouched.

Negative and limits:

- GGUF version 1 is rejected; it is obsolete and uses different length
  encodings.
- GGUF text is decoded lossily because real tokenizer arrays can contain byte
  strings that are not valid UTF-8. Inspection is not loading.
- Unknown GGML quantization type IDs are reported but their data size is not
  validated; the summary emits a warning instead of guessing.
- The inspector does not validate tensor values, checksums stored in the model,
  licenses, provenance signatures, or chat templates. It does not infer model
  quality, context behavior, or hardware requirements.
- The whole file is hashed on every inspection. Large models therefore take
  time proportional to their size; this is the intended cost of provenance.
- Safetensors shards are inspected one file at a time; there is no index-file
  aggregation yet.
- No download, pull, remove, conversion, quantization, or weight loading is
  added. Those remain separate tasks with their own integrity and license
  requirements.

## Tests

- Nine `src/ai/model.rs` unit tests cover the SHA-256 vectors, minimal GGUF and
  Safetensors fixtures, JSON summary fields, truncation, unknown format, bad
  JSON, overlapping Safetensors ranges, out-of-range data, and non-UTF-8
  headers.
- Two `tests/ai.rs` integration tests run the CLI end to end for both formats,
  `--json`, `--metadata`, format/I/O failures, and usage errors.
- `cargo test` passes 664 tests; clippy and rustfmt are clean.

## Alternatives considered

1. **Use the system `sha256sum`.** Rejected: process and platform dependency,
   inconsistent output parsing, and no single bounded implementation.
2. **Add `sha2` and format crates.** Rejected by ADR-0007. The in-tree SHA-256
   is small, reviewed, and covered by standard vectors.
3. **Load the model through `llama.cpp` or a Python library.** Rejected: that
   executes or trusts a large external runtime, may allocate the full weights,
   and cannot be part of the default offline path.
4. **Skip range validation and report only names.** Rejected: a summary that
   silently accepts a truncated or overlapping file is worse than no summary.
