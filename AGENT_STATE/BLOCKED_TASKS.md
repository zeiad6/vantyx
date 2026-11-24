# Blocked Tasks

- T-040 — Live local/cloud AI and trained embedding-model evidence. Blocked:
  no inference server, model files, or cloud credentials in this environment.
  Resumable once a server or key is available; record setup, versions,
  hardware, first-token latency, throughput, and raw output.
- T-017 / language-level `from ai import Client` — Blocked on async/concurrency
  design and implementation. Host-side AI remains available through the CLI
  and Rust API.
- GPU, tensor, SIMD, autodiff, and training evidence — Blocked on verified
  hardware and a reproducible backend. No performance or capability claim is
  made.
- Native memory reclamation design review — Deferred; a prior
  `compiler-architect` subagent run failed with “Free usage limit reached”.
  Retry when subagent capacity is available.
- T-053 REST contributor-cache closure — GitHub's `/contributors` endpoint
  still serves a cached `auren` entry with 12 contributions, while the public
  contributors page lists only `zeiad6`. Recheck after GitHub refreshes.
