# Agent Skills

This repository ships thirteen reusable skills for coding agents. They are
project-local OpenCode skills, discovered automatically from
`.opencode/skills/<skill-id>/SKILL.md`.

## Activation

The active OpenCode harness documents project skills at
<https://opencode.ai/docs/skills> and searches `.opencode/skills` from the
project root to the current directory. After the files were added on
2026-11-26, the harness advertised all thirteen in its available-skill list, so
the location and frontmatter are confirmed active. Skills are loaded on demand
with the `skill` tool by exact ID; they are not injected into every prompt.

To verify discovery from a shell:

```sh
for f in .opencode/skills/*/SKILL.md; do
  id=$(basename "$(dirname "$f")")
  test -f "$f" && grep -q '^name: ' "$f" && grep -q '^description: ' "$f" \
    && echo "ok $id" || echo "bad $id"
done
```

## Inventory

| ID | Use for |
| --- | --- |
| `rust-forensics` | Repository audit, Git/source archaeology, defect reproduction, evidence collection. |
| `compiler-engineering` | Lexer/parser/checker/IR/interpreter/C-backend changes; interpreter-first and `E0401` boundaries. |
| `python-reverse-engineering` | Studying Python libraries for original Vantyx capabilities, licenses, benchmarks. |
| `runtime-memory-safety` | Values, aliasing, allocation, reclamation, cycles, bounds, native runtime helpers. |
| `ai-systems-engineering` | `src/ai/` providers, streaming, embeddings, RAG, tools, agents, budgets, metrics, inspection. |
| `model-training` | Datasets, tensors, training loops, checkpoints, reproducibility, fine-tuning. |
| `gpu-numerical-performance` | SIMD, CPU dispatch, kernels, GPU evaluation with hardware evidence. |
| `security-audit-engineering` | Threat modeling, panic/path/resource audits, AI trust boundaries, fuzzing. |
| `performance-benchmarking` | Fair benchmarks, warm-up, repetitions, medians, variance, raw results. |
| `multi-agent-orchestration` | Task ownership, parallel work, conflict prevention, independent review, checkpoints. |
| `cross-platform-engineering` | Linux/macOS/Windows assumptions, paths, toolchains, temp files, CI matrices. |
| `autonomous-recovery` | State restoration, checkpoints, network backoff, quota recovery, resume. |
| `engineering-skill-orchestration` | Select and compose skills, record the selection, and require independent review before accepting evidence. |

## Selection

The orchestrator selects skills per task and records the selection in the task
entry or checkpoint. A task may use several skills; for example, a native
backend change uses `compiler-engineering` + `runtime-memory-safety` +
`performance-benchmarking` when a speed claim is made.

The independent reviewer checks that the selected skill's validation commands
were actually run and that its acceptance criteria are met.

## Maintenance

- Keep IDs lowercase kebab-case and aligned with the directory name.
- Keep `description` specific: it is the only text advertised before loading.
- Put long references or scripts beside `SKILL.md` in the same directory; paths
  inside the skill are relative to that directory.
- Do not duplicate an existing skill; extend it instead.
- A skill that describes unsupported hardware or capabilities must say so
  explicitly (for example, GPU support is unverified).
- Update a skill when the underlying source, command, or limitation changes.
