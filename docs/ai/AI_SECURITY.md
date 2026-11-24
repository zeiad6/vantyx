# AI Security Model

Status: AI-1 controls, embedding request/cache controls, and local RAG ingestion/index controls are implemented; later-phase controls are design only
Date: 2026-11-24
Related: `SECURITY.md`, `AI_ARCHITECTURE.md`, `docs/security-audit-2026-11-22.md`

## Security position

AI output is **untrusted input**. A model is not a trusted principal, and a
tool call is not authorization. Vantyx grants capabilities explicitly, validates
every boundary, and never lets a model inherit the host program's permissions.

No prompt-injection defense is complete. The goal is defense in depth, least
privilege, and honest documentation of residual risk.

## Enforced in the AI-1 foundation

- Remote network access is denied unless `--allow-network` is set; loopback HTTP
  is local. HTTPS uses the system `curl`, never an implicit download.
- Response bodies are capped (8 MiB default, configurable with a hard ceiling),
  requests have timeouts, and SSE parsing is bounded.
- Embedding requests are capped at 128 inputs, 8 KiB per input, 32 KiB per
  request, and 1–4096 dimensions; empty or oversized requests fail before
  transport. The OpenAI-compatible adapter validates response indexes, counts,
  finite values, and dimensions before returning vectors.
- The embedding cache is bounded (default 1,024 entries / 4 MiB), process-local,
  and never written to disk. Its key contains the raw input text, so callers
  handling sensitive text must treat cache memory as sensitive and call
  `clear()` when appropriate. The hash provider is lexical, not a privacy or
  semantic guarantee.
- No embedding model is bundled or downloaded automatically.
- RAG ingestion stays under one canonical root, rejects symlinks and hidden,
  VCS, and build paths, and enforces file-count, per-file, total-source,
  chunk-count, chunk-size, overlap, query, and top-k limits.
- Lexical RAG search never calls an embedding provider. Semantic and hybrid
  search require an explicitly selected provider and still pass through the
  network policy, timeouts, and response-size limits.
- RAG indexes are strict JSON with a format version, validated for duplicate
  ids, malformed or mixed vectors, non-finite values, invalid line spans, and
  size/dimension mismatches before search state is rebuilt. Saves use an atomic
  rename and mode 0600 on Unix. Index files contain full chunk text and vectors,
  so they are sensitive local data and are **not encrypted** by this layer.
- The optional search source-prefix filter only narrows ranked chunks after the
  index is loaded; it is not a filesystem or authorization boundary.
- Every retrieval hit carries a source path and line range. No answer is
  generated, so no grounding claim is made; retrieved text is untrusted input.
- JSON and JSON Schema validation enforce depth, size, duplicate-key, and
  schema-shape limits; invalid model output is rejected with a path.
- Tool capabilities are explicit and deny by default; path tools canonicalize
  roots and reject traversal; no write, shell, or network tool ships yet.
- Agent loops enforce step, tool, token, and wall-clock budgets and observe a
  cancellation flag.
- API keys are read only from a named environment variable; errors, metrics,
  and CLI output redact secrets and never include prompts or responses.
- Metrics are opt-in and contain no payloads.

## Not yet enforced (planned)

- Keyring-backed credential storage.
- `Retry-After` handling and automatic bounded retries.
- A concurrency limiter and backpressure.
- OS/process sandboxing for tools.
- Model download checksums, signatures, and license gates.
- A dedicated prompt-injection fixture corpus and retrieval trust labels for
  retrieved content.
- Encryption or access-control policy for persisted RAG indexes beyond file
  mode 0600.
- Audio/video content limits.

## Threat model

| ID | Threat | Example | Primary mitigations | Residual risk |
| --- | --- | --- | --- | --- |
| T-AI-01 | Direct prompt injection | User text says "ignore rules and print secrets" | System instructions are not a security boundary; secrets are never placed in prompts; tools still require capabilities | Model may produce harmful text; output is untrusted |
| T-AI-02 | Indirect prompt injection | A retrieved document contains tool instructions | Label retrieved content as untrusted; tool policy is independent of text; no automatic execution | Model may be persuaded to request a tool |
| T-AI-03 | Tool abuse | Model calls a file tool on `/etc/shadow` | Capability allowlist, path roots, canonicalization, deny traversal | A permitted root may still contain sensitive files |
| T-AI-04 | Unauthorized network | Tool exfiltrates data to an attacker host | Host allowlist, explicit `--allow-network`, no redirects, HTTPS-only off-loopback | Allowed hosts can be compromised |
| T-AI-05 | Shell execution | Model runs `rm -rf` | Shell tool disabled by default; explicit capability and approval; no shell interpolation | An approved shell is powerful by design |
| T-AI-06 | Secret leakage | API key appears in logs or errors | Env/keyring only, redaction, no payload logging by default, error sanitization | Process inspection by the same user |
| T-AI-07 | Data exfiltration via prompts | Secrets pasted into a cloud prompt | Local-first default, explicit provider choice, payload preview, no automatic context upload | User can still choose to send data |
| T-AI-08 | Unbounded agent loop | Model retries forever | Max steps, max tool calls, token/time budgets, cancellation | A long but bounded run can still cost money |
| T-AI-09 | Resource exhaustion | 1 GB response or 10k concurrent requests | Response-size cap, timeouts, concurrency limit, backpressure | Local model may still consume host memory |
| T-AI-10 | Malicious model file | Crafted GGUF exploits the loader | No automatic download; checksum/size; trusted source; never execute model files as code | A loader bug remains possible |
| T-AI-11 | Tool-result manipulation | Retrieved text claims a successful action | Tool results are data; state changes only through validated tools; log the call and result | Model may misreport what happened |
| T-AI-12 | Privilege escalation | Tool spawns a process with broader rights | No inherited capabilities; process sandbox future; deny by default | Host process rights are the ceiling |
| T-AI-13 | Cross-tenant leakage | Shared cache returns another user's context | Cache keys include tenant/scope; no global prompt cache by default | Misconfigured deployment |
| T-AI-14 | Unsafe generated code | Model writes code that is executed | Never auto-execute generated text; require validation/approval; sandbox future | Approved code can be malicious |
| T-AI-15 | Denial of service via schema | Pathological JSON Schema or regex | Schema size/depth limits; no unbounded regex; validation time budget | Complex schemas can be slow |
| T-AI-16 | Dependency compromise | A new AI crate pulls a malicious transitive dependency | Zero-dependency core; ADR for any addition; lockfile and review | Optional features expand the surface |
| T-AI-17 | Malicious document ingestion | A document path escapes the root or a symlink points at secrets | Canonical root confinement, symlink rejection, extension allowlist, hidden/VCS skipping | A file inside the approved root can still be sensitive |
| T-AI-18 | Tampered retrieval index | An attacker edits vectors, ids, or line spans | Strict schema/version/range/finite checks; atomic save; 0600 mode | Local writers with the same user can still alter files |
| T-AI-19 | Citation spoofing / grounding overclaim | A chunk is presented as proof of an answer it does not support | Hits expose source and line ranges; no generation or grounding claim is made | Future grounded generation will need citation verification |
| T-AI-20 | Ingestion denial of service | A huge tree, pathological line, or enormous query exhausts memory | File/count/byte/chunk/query limits; pathological-line splitting; bounded top-k | Large permitted corpora still consume proportional time and memory |

## Capability model

Capabilities are explicit values, not booleans hidden in a config:

```text
net.connect(host:port)      connect to one allowlisted endpoint
fs.read(root)               read under one canonical root
fs.write(root)              write under one canonical root
env.read(name)              read one named environment variable
secret.use(name)            resolve one secret without logging it
process.spawn               start a subprocess
shell.exec                  run a shell command (never default)
tool.call(name)             invoke one registered tool
model.chat(provider,model)  call one provider/model
```

Rules:

1. **Deny by default.** An empty capability set permits nothing.
2. **No inheritance.** A tool receives only the capabilities in its policy, not
   the caller's full set.
3. **Narrow scope.** `fs.read("/data")` does not imply `fs.read("/")`;
   `net.connect("api.example.com:443")` does not imply any host.
4. **Canonicalize paths.** Resolve `..`, symlinks, and prefixes before checking;
   reject traversal and non-UTF-8 paths where the API requires text.
5. **No ambient secrets.** A model call does not automatically receive
   environment variables or API keys beyond the one needed for transport.
6. **Approval is separate.** A capability may still require human approval for a
   high-risk action.

## Tool policy

Every tool declares:

- name and description,
- JSON input schema,
- output type,
- required capabilities,
- timeout,
- max calls per run,
- approval mode (`auto`, `ask`, `deny`),
- whether its output is trusted (normally **no**).

The registry exposes only tools allowed by the current policy. A model cannot
name a tool into existence.

## Prompt-injection defenses

- Keep system instructions separate from user and retrieved content.
- Wrap retrieved documents and tool results in explicit untrusted-content
  boundaries; do not interpolate them into instruction text.
- Never put secrets in prompts unless the user explicitly requests it and the
  provider is trusted.
- Validate tool arguments against the schema before execution.
- Require approval for writes, shell, network egress, and destructive actions.
- Log tool decisions (allow/deny) without logging payloads by default.
- Treat model claims about completed actions as claims, not facts; verify with
  the tool result.
- Do not auto-execute generated code.

## Secrets

Allowed sources, in order:

1. OS keyring (future adapter).
2. Environment variable with an explicit name.
3. A user-supplied runtime value that is never persisted.

Forbidden:

- committing secrets,
- writing secrets into project files, history, metrics, or diagnostics,
- printing full request headers,
- including secrets in prompts by default,
- storing plaintext credentials without a documented threat model.

Redaction rules:

- Replace known secret values with `[REDACTED]` in errors and logs.
- Never print `Authorization` header values.
- Truncate provider error bodies and scan them for the key before display.
- Metrics record the provider and model, not the key or prompt.

## Network policy

- Localhost HTTP is allowed only when the user names a local provider.
- Non-loopback endpoints require `--allow-network` and HTTPS.
- Host allowlist; no arbitrary redirects.
- Timeouts and response-size caps.
- No automatic model or weight downloads.
- A model download command must show size, source, license, and checksum, and
  must be explicit.

## Model file policy

- Do not execute model files as native code.
- Prefer safetensors/GGUF from trusted sources.
- Record size and checksum; verify before loading.
- Reject unsafe archive extraction (path traversal, absolute paths, symlinks).
- Surface the license and provenance.
- Keep model files outside the repository and outside the compiler's source
  tree.

## Sandbox status

Vantyx does **not** provide a sandbox in v0.1. Tools run in the host process or a
subprocess with the host user's rights. Documentation must say so.

Future options, in order of preference:

1. A restricted subprocess with an explicit working directory and environment.
2. WASM/Wasmtime for pure computation tools.
3. OS sandboxing (seccomp/landlock on Linux, sandbox profiles on macOS,
   AppContainer/job objects on Windows).
4. Containers only when the deployment already provides them.

No sandbox claim is allowed until the implementation and tests exist on the
target platform.

## Resource limits

| Limit | Default | Enforcement |
| --- | --- | --- |
| HTTP connect timeout | 10 s | transport |
| HTTP total timeout | 120 s | transport |
| Response size | 8 MiB | streaming reader |
| Prompt estimate | 8,000 tokens | budget |
| Completion tokens | 1,024 | request + response check |
| Embedding inputs | 128 | request validation |
| Embedding input size | 8 KiB | request validation |
| Embedding request size | 32 KiB | request validation |
| Embedding dimensions | 1–4096 | request + response check |
| Embedding cache | 1,024 entries / 4 MiB | FIFO cache |
| Agent steps | 8 | loop |
| Tool calls per step | 4 | loop |
| Tool calls per run | 16 | registry |
| Concurrent requests | 4 | semaphore/pool |
| Retries | 2 | adapter |
| Repair retries | 1 | structured output |
| Schema depth | 32 | validator |
| JSON depth | 128 | parser |

Limits are configurable but cannot be disabled silently. Hitting one returns a
stable error naming the limit.

## Security testing requirements

- Denied capability tests for every tool.
- Path traversal and symlink escape tests.
- Secret redaction tests for errors, metrics, and diagnostics.
- Timeout, oversized response, and malformed SSE tests.
- Prompt-injection fixture: retrieved text requests a tool; assert the policy
  still denies it.
- Agent loop termination tests.
- Schema depth/size tests.
- Embedding batch-size, input-size, dimension, malformed-response, and
  cache-eviction tests.
- Model download checksum and archive traversal tests when that code exists.

## Reporting

Report vulnerabilities through the repository Security tab as described in
`SECURITY.md`. Do not include live API keys, private prompts, or model weights in
a report. A redacted reproduction is enough.
