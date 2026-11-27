---
name: Model Training and Fine-Tuning
description: Build reproducible datasets, tensor and autodiff pipelines, training loops, optimizers, LoRA and fine-tuning runs, checkpoints, evaluation, GPU execution, and model cards with provenance and honest limitations.
---

# Model Training and Fine-Tuning

## When to use

- Creating or changing a dataset, tokenizer, data loader, split, or training pipeline.
- Implementing tensors, autodiff, optimizers, losses, LoRA, or checkpoint/resume behavior.
- Running or evaluating a training or fine-tuning job on CPU or GPU.
- Reviewing a training claim, model card, or reproducibility report.

Do not use this skill for inference-only integration or RAG; use `ai-systems-engineering`. For kernel-level GPU optimization, use `gpu-numerical-performance`.

## Required tools

- A training framework or a custom tensor/autodiff implementation.
- Dataset tooling, tokenizer, and a deterministic split utility.
- Checksum and manifest tools (`sha256sum`, JSON/CSV writers).
- Optional: GPU runtime (`nvidia-smi`, `rocm-smi`), mixed-precision libraries, experiment tracking, profilers.
- An evaluation harness with a held-out set and fixed metrics.
- Enough disk for checkpoints, logs, and raw evaluation outputs.

## Safety constraints

- Respect dataset licenses, privacy, consent, and removal requests. Do not train on secrets or personal data without authorization.
- Do not download models or datasets implicitly. Pin revisions and hashes; use a sandbox when provenance is unknown.
- Bound dataset size, sequence length, batch size, gradient accumulation, checkpoint retention, and disk usage.
- Never overwrite a checkpoint or training log silently. Use immutable run directories and manifests.
- Do not claim GPU, distributed, or mixed-precision support without a real run on that hardware.
- Report training loss separately from held-out quality. Never present a training metric as generalization.
- Seed everything that affects the result and record non-deterministic operations.

## Workflow

1. **Define the objective and baseline.** State the task, metric, target, compute budget, and acceptable quality. Establish a trivial or pre-trained baseline before training.

2. **Fix the dataset contract.** Specify record format, encoding, required fields, label rules, deduplication, filtering, and split policy. Validate with line numbers and reject malformed records before training.

3. **Record provenance.** For every dataset record: source, license, retrieval date, byte size, SHA-256, record count, split rule, seed, and preprocessing version. Check for train/test leakage and near-duplicates.

4. **Build a deterministic pipeline.** Tokenize, pack or pad, shuffle, and batch with recorded seeds. Verify that two runs produce the same order and the same token counts. Measure loader throughput and memory.

5. **Implement or verify tensor primitives.** Define shape, dtype, strides, broadcasting, device, and error behavior. Add known-value tests for matrix multiply, reductions, softmax, normalization, and attention. Use tolerances appropriate to the dtype.

6. **Check autodiff and gradients.** Use finite differences or a reference implementation on small tensors. Test non-differentiable paths, masking, accumulation, and gradient clipping. Assert finite values with the step number.

7. **Build the training loop.** Seed initialization, optimizer state, learning-rate schedule, gradient accumulation, mixed precision, logging, and checkpoint cadence. Log loss, gradient norm, learning rate, throughput, and memory.

8. **Choose the fine-tuning strategy.** Compare full fine-tuning, LoRA/QLoRA, and frozen-layer baselines. Record target modules, rank, alpha, dropout, quantization, and trainable parameter count. Justify the choice against quality and memory.

9. **Make checkpoints resumable and verifiable.** Write a manifest with step, seed, config, tensor shapes, optimizer/RNG/scheduler state, and per-file SHA-256. Test resume against an uninterrupted run and compare the next-step loss within a stated tolerance.

10. **Evaluate on held-out data.** Use task metrics, perplexity where appropriate, safety/bias checks, and confidence intervals. Keep the test set untouched until the final evaluation. Report per-slice results, not only the average.

11. **Handle GPU execution honestly.** Probe the device, driver, memory, and dtype support. Record batch size, sequence length, precision, and utilization. Handle OOM with a documented fallback or a clear failure; never silently switch hardware.

12. **Package the result.** Produce a model card with data, training config, metrics, intended use, limitations, license, and evaluation date. Archive configs, logs, and raw evaluation outputs.

## Validation and acceptance criteria

- Dataset validation rejects malformed records with line numbers and reports split counts.
- Two runs with the same seed, config, and data produce identical checkpoints or a documented tolerance.
- Resume reproduces the uninterrupted trajectory within a stated tolerance.
- Checkpoint manifests detect corruption and missing files.
- Gradient checks and numerical tests pass for every new primitive.
- Evaluation uses a held-out set and reports the baseline, trained result, and variance.
- No GPU, distributed, or mixed-precision claim without an observed hardware run.
- Training logs and checkpoints are immutable and reproducible from recorded inputs.
- The model card states limitations and unsupported uses.

## Expected output

1. Dataset card: source, license, size, hash, split, filtering, known biases.
2. Training config and environment lockfile.
3. Reproducible pipeline and tests for tensors, autodiff, and data order.
4. Checkpoint manifest and resume verification.
5. Evaluation report with baseline, metrics, per-slice results, and confidence intervals.
6. Model card and runbook for reproducing the result.
7. Compute record: hardware, duration, memory, cost, and utilization.

## Troubleshooting

- **Loss becomes NaN/Inf:** check learning rate, division by zero, log of zero, masked softmax, gradient clipping, and dtype overflow. Add a finite-value assertion with step and batch.
- **OOM:** reduce batch size or sequence length, use gradient accumulation or checkpointing, and record the exact configuration that failed.
- **Resume diverges:** compare model, optimizer, scheduler, RNG, data order, and precision state, not just weights.
- **Overfitting:** inspect train/validation curves, dataset size, augmentation, regularization, and leakage.
- **Data loader is the bottleneck:** profile decoding, tokenization, collation, and host-to-device transfer; cache preprocessed data with a hash.
- **Non-determinism:** pin seeds, disable nondeterministic kernels where possible, and record the operations that remain nondeterministic.
- **GPU unavailable:** keep the CPU path authoritative, mark the GPU task blocked, and state the missing prerequisite.
- **Checkpoint too large:** shard with a manifest and retention policy; never truncate silently.

## Examples

```sh
# Validate and hash a dataset
sha256sum data/train.jsonl
python3 - <<'PY'
import hashlib, json
path = "data/train.jsonl"
count = 0
with open(path, "rb") as fh:
    for lineno, line in enumerate(fh, 1):
        obj = json.loads(line)
        assert isinstance(obj, dict), lineno
        count += 1
print("records", count, "sha256", hashlib.sha256(open(path, "rb").read()).hexdigest())
PY

# Probe GPU
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader

# Tiny deterministic run
python3 train.py --config configs/tiny.yaml --seed 1 --max-steps 20 --out runs/tiny-001

# Resume check
python3 train.py --config configs/tiny.yaml --seed 1 --max-steps 40 --resume runs/tiny-001/checkpoint-20
```

## Related skills and review

- Pair with `gpu-numerical-performance` for kernels, mixed precision, and hardware utilization.
- Pair with `ai-systems-engineering` for serving the trained model and `performance-benchmarking` for throughput claims.
- Pair with `security-audit-engineering` for untrusted datasets, model files, and training code.
- Reviewer checks: provenance, deterministic split, resume test, held-out evaluation, hardware evidence, and model-card honesty.
