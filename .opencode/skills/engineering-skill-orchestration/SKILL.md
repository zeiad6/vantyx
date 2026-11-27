---
name: Engineering Skill Orchestration
description: Select and compose the right engineering skills for each task, record the selection, enforce required workflows, and have an independent reviewer verify evidence before accepting the result.
---

# Engineering Skill Orchestration

## When to use

- At the start of every non-trivial engineering task.
- When a task spans multiple domains, such as compiler work that also needs benchmarking and security review.
- When coordinating subagents or handing work to a reviewer.
- When deciding whether a task needs a specialized skill or ordinary engineering judgment.

This skill is an integration layer. It does not replace `multi-agent-orchestration`, which governs file ownership and parallel execution.

## Required tools

- The active agent's skill list and `skill` loader.
- `python3` and the bundled `scripts/validate_skills.py` catalog validator.
- A task brief with goal, scope, acceptance criteria, and constraints.
- The repository rules and validation commands.
- An evidence log and an independent reviewer.
- The domain skills selected for the task.

## Safety constraints

- Load only skills that exist and are advertised by the active environment. Never invent a skill ID.
- Do not claim a skill is active unless the environment confirms it or the loader succeeds.
- Do not let a skill override repository rules, security policy, or explicit user constraints.
- Do not use a skill as a substitute for reading the actual code and running the actual commands.
- Prefer a more specific project-local skill for that project's domain; use global skills for cross-project workflows.
- Stop and ask when two required skills impose incompatible constraints.

## Workflow

1. **Classify the task.** Identify the primary deliverable and the domains it touches. Example:
   - change a compiler pass -> compiler engineering;
   - measure the result -> performance benchmarking;
   - review unsafe code -> runtime safety and security audit;
   - coordinate parallel work -> multi-agent orchestration.

2. **Inspect the available skills.** Run the catalog validator, then use the skill list provided by the active agent:
   ```sh
   python3 scripts/validate_skills.py
   ```
   Select the smallest set that covers the required workflows, usually one primary and one or two supporting skills. Avoid loading every skill "just in case."

3. **Record the selection.** Add a `Skills:` line to the task brief:
   ```markdown
   Skills: compiler-engineering (primary), performance-benchmarking (supporting), security-audit-engineering (review)
   ```
   State why each skill is needed and which acceptance criteria it contributes.

4. **Load the skills.** Call the skill loader with the exact ID. If loading fails, record the failure and fall back to the documented workflow or ask the user; do not pretend the skill was used.

5. **Compose the workflows.** Build one ordered plan that merges the primary workflow with supporting constraints:
   - correctness gates before performance measurements;
   - security review before release;
   - cross-platform checks before platform claims;
   - recovery checkpoints before long or parallel work.
   Resolve conflicts explicitly (for example, performance vs. memory safety) and record the decision.

6. **Write acceptance criteria.** Each criterion must be observable: a command, a test, a diff, a measurement, or a documented artifact. "Looks good" is not a criterion.

7. **Execute and collect evidence.** Run the commands, save raw output, and label claims `CONFIRMED`, `SUSPECTED`, or `UNKNOWN`. Do not skip validation because a skill is loaded.

8. **Hand off to an independent reviewer.** The reviewer must not be the implementing agent. Provide the task brief, selected skills, diff, and evidence.

9. **Review for workflow compliance.** The reviewer checks:
   - the selected skills match the task;
   - the primary workflow steps were actually followed;
   - required tools and safety constraints were respected;
   - validation commands ran and passed, or failures are documented;
   - performance, security, and platform claims have evidence;
   - no scope creep, no weakened tests, no hidden blockers;
   - the expected output exists and is usable.
   The reviewer records a verdict: `ACCEPT`, `ACCEPT WITH FOLLOW-UPS`, or `REJECT`.

10. **Close the loop.** Record the verdict, residual risks, and follow-ups. If a skill was missing or inadequate, document the gap and propose an improvement rather than silently working around it.

## Skill selection matrix

| Task signal | Primary skill | Supporting skills |
| --- | --- | --- |
| Unfamiliar Rust repo, stale claims | rust-forensics | security-audit-engineering, performance-benchmarking |
| Lexer/parser/type/IR/backend change | compiler-engineering | runtime-memory-safety, performance-benchmarking |
| Study a Python library for reimplementation | python-reverse-engineering | performance-benchmarking, security-audit-engineering |
| Ownership, leaks, GC, unsafe Rust | runtime-memory-safety | security-audit-engineering, compiler-engineering |
| Provider, RAG, agents, structured output | ai-systems-engineering | security-audit-engineering, performance-benchmarking |
| Dataset, training, LoRA, checkpoints | model-training | gpu-numerical-performance, ai-systems-engineering |
| SIMD, CUDA, ROCm, numerical kernels | gpu-numerical-performance | performance-benchmarking, compiler-engineering |
| Threat model, fuzzing, prompt injection | security-audit-engineering | runtime-memory-safety, ai-systems-engineering |
| Speed/memory/cost claim | performance-benchmarking | the domain skill being measured |
| Parallel agents, file ownership | multi-agent-orchestration | autonomous-recovery, domain skills |
| Windows/macOS/Linux, linking, CI | cross-platform-engineering | compiler-engineering, security-audit-engineering |
| Interruption, quota, context reset | autonomous-recovery | multi-agent-orchestration |

## Validation and acceptance criteria

```sh
python3 scripts/validate_skills.py
```

- The task brief names the selected skills and the reason for each.
- Every loaded skill ID exists and was confirmed by the environment.
- The plan merges the required workflows without contradiction.
- Acceptance criteria are observable and mapped to commands or artifacts.
- The reviewer is independent and checks workflow compliance, not just test results.
- The verdict and residual risks are recorded.
- No claim relies on a skill that was not actually loaded or followed.

## Expected output

1. Task brief with `Skills:` selection and rationale.
2. Ordered execution plan with acceptance criteria.
3. Evidence log with commands and raw results.
4. Independent review report with verdict.
5. Residual risks and follow-ups.
6. Skill-gap notes if a needed capability was missing.

## Troubleshooting

- **Skill ID not found:** check the active skill list and exact case-sensitive ID; do not guess. Fall back to the documented workflow and record the gap.
- **Too many skills selected:** keep the primary skill and only the supporting constraints that affect acceptance. Drop the rest.
- **Skills conflict:** write the conflict and the chosen priority; for example, correctness gates before performance, security before release.
- **Reviewer rubber-stamps:** require the reviewer to cite the diff, commands, and evidence for each criterion.
- **Skill loaded but workflow skipped:** reject the result; loading is not execution.
- **No matching skill:** use general engineering, document the gap, and propose a new skill only if the workflow is reusable.

## Examples

```sh
python3 scripts/validate_skills.py
```

```markdown
## Task T-107: Add a new IR optimization
Skills: compiler-engineering (primary), performance-benchmarking (supporting), security-audit-engineering (review)
Acceptance:
- differential interpreter/native test passes;
- IR verifier passes;
- benchmark shows no regression on the fixed corpus;
- reviewer confirms no unsound transform.
```

```text
Reviewer verdict: ACCEPT WITH FOLLOW-UPS
Evidence: diff inspected; cargo test --workspace passed; benchmark raw data attached;
follow-up: test the pass under --no-default-features.
```

## Related skills and review

- Use with `multi-agent-orchestration` when work is split across agents.
- Use with `autonomous-recovery` when a task is resumed after interruption.
- Reviewer checks: skill IDs valid, workflows followed, evidence present, and verdict justified.
