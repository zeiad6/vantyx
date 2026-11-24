# AI host examples

- `embeddings.md` — offline hash embeddings, stdin batches, a local
  OpenAI-compatible embedding server, and the bounded library cache.
- The structured extraction walkthrough below.

# Structured extraction example

`vantyx ai extract` asks a provider for JSON and refuses to return it until the
result parses and validates against a schema. Invalid output is repaired at most
three times, and every attempt is charged to the same budget.

The schema in `person.schema.json` uses only the supported JSON Schema subset:
`type`, `enum`, `required`, `properties`, `additionalProperties`, `items`,
`minItems`, `maxItems`, `minLength`, `maxLength`, `minimum`, and `maximum`.

## Offline deterministic demo

No model, network, or API key is involved. `--response` supplies the fixture
that the mock provider returns, which makes the repair loop reproducible.

```sh
./target/debug/vantyx ai extract \
  --provider mock \
  --model demo \
  --schema examples/ai/person.schema.json \
  --response 'not json' \
  --response '{"name":"Ada","role":"engineer","years":36,"skills":["compilers"]}' \
  --repair-attempts 1 \
  --json \
  "Extract the person record"
```

The first fixture is rejected as invalid JSON; the second is validated and
printed as an envelope with `value`, `attempts`, `model`, `finish_reason`, and
`usage`.

## Local model demo

Start Ollama or another OpenAI-compatible server first, then point `--endpoint`
at its loopback address. Nothing is downloaded automatically.

```sh
ollama serve
ollama pull <model>

./target/debug/vantyx ai extract \
  --provider openai \
  --model <model> \
  --endpoint http://127.0.0.1:11434/v1 \
  --schema examples/ai/person.schema.json \
  "Ada is a compiler engineer with 36 years of experience."
```

Remote HTTPS endpoints additionally require `--allow-network`. API keys are read
only from the environment variable named by `--api-key-env`
(`VANTYX_AI_API_KEY_ENV` by default); they are never read from the project.

## Limits

- Repair attempts are capped at 3; the CLI defaults to 1.
- Schema and response sizes are capped at 1 MiB each.
- The adapter sends `response_format=json_object`; it does not claim server-side
  JSON Schema or constrained decoding.
- Validation is strict JSON plus the documented schema subset. A valid result
  is still untrusted data until your program applies its own policy.
