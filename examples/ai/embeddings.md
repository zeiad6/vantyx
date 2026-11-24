# Embeddings example

`vantyx ai embed` obtains vectors from an embedding provider. The offline
`mock`/`hash` provider is a deterministic lexical baseline; it is **not** a
trained semantic model. Real semantic vectors come from an OpenAI-compatible
server exposing `POST /v1/embeddings` (Ollama, llama.cpp, LM Studio, or a cloud
gateway).

## Offline deterministic demo

No model, network, or API key is involved.

```sh
./target/debug/vantyx ai embed \
  --provider hash \
  --model demo \
  --dimensions 8 \
  --json \
  "compiler" "native backend"
```

The output is one JSON object with `provider`, `model`, `dimensions`,
`input_count`, `vectors`, and `usage`. Each vector is L2-normalized. Re-running
the command produces byte-identical vectors.

A batch can also be read from standard input, one input per non-empty line:

```sh
printf 'alpha\n\nbeta\n' | ./target/debug/vantyx ai embed \
  --provider hash --dimensions 4 --json -
```

Human-readable output previews eight components; use `--json` for full vectors.

## Local model demo

Start Ollama or another OpenAI-compatible server and pull an embedding model
first. Vantyx never downloads a model automatically.

```sh
ollama serve
ollama pull nomic-embed-text

./target/debug/vantyx ai embed \
  --provider openai \
  --model nomic-embed-text \
  --endpoint http://127.0.0.1:11434/v1 \
  --dimensions 768 \
  --json \
  "The compiler lowers typed IR to C."
```

The adapter posts to `http://127.0.0.1:11434/v1/embeddings`. If the server or
model does not support embeddings, or returns a different dimension, the command
fails with a stable error instead of returning an unvalidated vector.

Remote HTTPS endpoints require `--allow-network`. API keys are read only from
the environment variable named by `--api-key-env` (`VANTYX_AI_API_KEY` by
default); they are never read from the project.

## Library cache demo

`CachingEmbeddingProvider` wraps any provider that advertises embeddings. The
cache is bounded, keyed by model, dimensions, and text, and deduplicates repeated
inputs inside one batch.

```rust
use vantyx::ai::{
    Budget, CachingEmbeddingProvider, EmbeddingRequest, HashEmbeddingProvider, Provider,
};

let provider = CachingEmbeddingProvider::new(HashEmbeddingProvider::new(8)?);
let mut budget = Budget::default();
let request = EmbeddingRequest::new(
    "demo",
    vec!["alpha".into(), "alpha".into(), "beta".into()],
);
let response = provider.embed(&request, &mut budget)?;
assert_eq!(response.vectors.len(), 3);
assert_eq!(provider.stats().misses, 2);
# Ok::<(), vantyx::ai::AiError>(())
```

The second identical call is served from memory and consumes no provider
tokens. The cache defaults to 1,024 entries and 4 MiB; use
`CachingEmbeddingProvider::with_limits` to tighten those bounds or `clear` to
drop sensitive inputs.

## Limits

- 128 inputs per request.
- 8 KiB per input, 32 KiB per request.
- 1 to 4096 requested dimensions.
- The offline hash provider is lexical, not semantic.
- The cache is in-memory and process-local; it is not persisted and has no TTL.
- No embedding model is bundled or downloaded.
