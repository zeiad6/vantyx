# Local model inspection

`vantyx ai inspect` reads a local GGUF file, a Safetensors file, or a
Safetensors shard index, validates its metadata and tensor directory, and
computes a SHA-256 digest. It is offline by default and does not download,
load, or execute model code.

```sh
vantyx ai inspect model.gguf
vantyx ai inspect model.safetensors --json
vantyx ai inspect model.safetensors.index.json --json
vantyx ai inspect model.gguf --metadata
```

## What it reports

- Container format and GGUF version.
- File size and lowercase SHA-256.
- Tensor count and total parameter count.
- Architecture, name, context length, embedding length, and layer count when
  the file declares them.
- Quantization or `general.file_type` for GGUF, and the dtype histogram.
- Tokenizer model and token count when present.
- Selected metadata in text mode; all parsed metadata with `--metadata`; the
  complete metadata object in `--json` output.
- For a shard index, the number of shards and a per-shard summary with the
  relative path, size, tensor count, and parameter count. `--json` also includes
  each shard's SHA-256 digest.
- Warnings for unknown quantization types, duplicate metadata keys, a declared
  parameter count that differs from the computed count, a changing file, a
  Safetensors header that is not 8-byte aligned, or a shard index whose
  `total_size` does not match the summed shard data bytes.

`--json` prints one compact JSON object. Its fields are stable for scripts:

```json
{"path":"model.gguf","format":"gguf","format_version":3,"file_bytes":1234,
 "sha256":"...","tensor_count":1,"parameter_count":8,"data_bytes":32,
 "architecture":"llama","name":null,"quantization":"MOSTLY_Q4_K_M",
 "tokenizer_model":null,"context_length":4096,"embedding_length":null,
 "block_count":null,"tokenizer_tokens":2,"metadata":{},
 "dtypes":[{"dtype":"F32","tensors":1,"parameters":8}],
 "shards":[],"warnings":[]}
```

For a shard index, `format` is `safetensors-index`, `file_bytes` and `sha256`
describe the index JSON itself, and `shards` contains one object per referenced
shard:

```json
{"path":"model.safetensors.index.json","format":"safetensors-index",
 "format_version":null,"file_bytes":812,"sha256":"...","tensor_count":2,
 "parameter_count":5,"data_bytes":14,"architecture":"llama","name":"tiny",
 "metadata":{"total_size":"14"},
 "dtypes":[{"dtype":"F32","tensors":1,"parameters":2},
           {"dtype":"I16","tensors":1,"parameters":3}],
 "shards":[{"path":"a.safetensors","file_bytes":80,"sha256":"...",
            "tensor_count":1,"parameter_count":2,"data_bytes":8},
           {"path":"b.safetensors","file_bytes":80,"sha256":"...",
            "tensor_count":1,"parameter_count":3,"data_bytes":6}],
 "warnings":[]}
```

## Validation

The inspector rejects a file with `AI_E_FORMAT` when:

- The magic is neither `GGUF` nor a plausible Safetensors header, and the file
  is neither a JSON object nor a `.json` shard index.
- The GGUF version is not 2 or 3, or the file is truncated.
- A tensor range exceeds the data section, or known ranges overlap.
- A GGUF tensor's element count is not a multiple of its quantization block.
- A Safetensors dtype is unknown, a shape or offset is not an integer, offsets
  are misaligned, or the byte length does not match the shape and dtype.
- A metadata array, tensor count, header, shard count, or index exceeds the
  documented limit.
- A shard index has no `weight_map`, a tensor name is empty, a shard path is
  absolute, contains `..`, uses backslashes, lacks the `.safetensors`
  extension, or escapes the index directory after symlink resolution.
- A shard contains a tensor the index does not list, the index lists a tensor
  the shard does not contain, a tensor appears in more than one shard, or a
  referenced shard is not a Safetensors data file.

I/O failures such as a missing file, a missing shard, or a directory path use
`AI_E_IO`. A format error exits 1 and prints a help line; usage errors exit 64.
Shard validation is best-effort and not atomic: a file can change between its
digest and its header check, and a symlink can be retargeted concurrently.

## Limits

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
| Shard-index JSON | 64 MiB |
| Shards per index | 10,000 |
| Tensors per index | 1,000,000 |

Hashing streams every file, so memory use stays bounded but inspection time
grows with the total size of the index and all referenced shards. The inspector
does not load weights, execute model code, validate tensor values, verify
embedded signatures, infer quality, or check licenses. It does not discover
shards that the index omits, compare a shard index against a remote repository,
or download missing shards.

## Provenance

Record the digest next to a prompt, evaluation fixture, or RAG index:

```sh
vantyx ai inspect model.gguf --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["sha256"])'
```

A later inspection that reports the same digest covers the same bytes. A
changed digest means the model file changed, even if its name did not. For a
shard index, record the index digest and each shard digest from `--json`; the
aggregate summary alone does not prove that every shard is unchanged.

## Related

- [ADR-0041](../decisions/ADR-0041-offline-model-inspection.md)
- [ADR-0042](../decisions/ADR-0042-safetensors-shard-index-aggregation.md)
- [AI architecture](AI_ARCHITECTURE.md)
- [AI roadmap](AI_ROADMAP.md)
