---
name: GPU and Numerical Performance
description: Evaluate and improve SIMD, CPU dispatch, CUDA, ROCm, Metal, Vulkan, WebGPU, kernel analysis, tensor optimization, and numerical correctness with measured hardware evidence and scalar fallbacks.
---

# GPU and Numerical Performance

## When to use

- Writing or changing numerical kernels, matrix operations, reductions, convolutions, or tensor primitives.
- Adding SIMD, runtime CPU dispatch, GPU backends, or cross-device fallbacks.
- Investigating numerical drift, NaN/Inf behavior, performance cliffs, or hardware-specific failures.
- Reviewing a claim about GPU support, speedup, or numerical accuracy.

Do not use this skill for general application profiling without numerical kernels; use `performance-benchmarking`.

## Required tools

- A compiler with optimization reports (`rustc`, `clang`, `gcc`, `nvcc`, `hipcc`, `metal`, `glslang`, `wgpu`).
- CPU information (`lscpu`, `/proc/cpuinfo`) and GPU probes (`nvidia-smi`, `rocm-smi`, `vulkaninfo`, `clinfo`).
- Profilers: `perf`, `llvm-mca`, `objdump`, Nsight Systems/Compute, `rocprof`, `metal-capture`, browser GPU tools.
- A scalar reference implementation and a numerical test harness.
- Benchmark inputs covering small, odd, large, and memory-bound shapes.

## Safety constraints

- Never claim hardware support that was not executed on that hardware. Record device, driver, runtime, and command.
- Always keep a scalar or CPU fallback. Unsupported hardware must return a clear error, not silently compute incorrectly.
- Define numerical tolerances before optimizing; do not loosen them to make a kernel pass.
- Bound device memory, launch dimensions, queue depth, and compilation time. Handle OOM and device loss.
- Do not add a GPU dependency without a design decision, license review, and a CPU path.
- Watch thermal throttling and shared-device contention; do not benchmark on a busy GPU without recording it.
- Treat shader/kernel code and driver input as untrusted when running third-party models.

## Workflow

1. **Write the kernel contract.** Specify shapes, dtype, layout, strides, alignment, aliasing, broadcasting, tolerances, and error behavior for unsupported shapes. Include empty and 1-element cases.

2. **Implement a scalar reference.** Correctness first. Add known-value tests, property-based tests, and edge cases: NaN, Inf, denormals, signed zero, very small/large magnitudes, non-multiple-of-vector-width sizes.

3. **Measure the baseline.** Record hardware, OS, compiler, flags, input sizes, warm-up, repetitions, median, variance, and memory. Store raw results. Profile before changing the algorithm.

4. **Optimize on CPU first.** Improve loop order, blocking/tiling, alignment, restrict/aliasing hints, and data layout. Check whether the compiler already vectorized; use optimization reports or assembly before adding intrinsics.

5. **Add SIMD with dispatch.** Use portable SIMD or intrinsics behind compile-time or runtime feature detection. Keep the scalar path. Test every path on the same inputs and compare within tolerance. Guard against illegal instructions on older CPUs.

6. **Evaluate a GPU path only with real hardware.** Probe the device, driver, memory, and supported features. Implement a minimal kernel, verify against the reference, then optimize occupancy, coalescing, shared memory, and launch configuration.

7. **Handle synchronization and errors.** Check kernel launch errors, asynchronous failures, stream/queue ordering, and device synchronization before reading results. Do not hide errors behind a fallback.

8. **Analyze performance with counters.** Use roofline reasoning, memory bandwidth, occupancy, cache hit rate, and instruction mix. Distinguish compute-bound from memory-bound. Report profiler evidence, not intuition.

9. **Verify numerical correctness across backends.** Compare CPU scalar, SIMD, and GPU outputs with absolute/relative tolerances. Test accumulation order, reductions, FMA contraction, and low-precision paths. Document intentional differences.

10. **Build a feature matrix.** For each backend record: device, driver/runtime, dtype support, status (verified/blocked/unverified), and test command. CI hardware jobs must be labeled and reproducible.

11. **Document fallbacks and limits.** State what happens without a GPU, with insufficient memory, with an unsupported dtype, or on an older CPU. The user must never be surprised by a silent downgrade.

## Validation and acceptance criteria

- The scalar reference passes known-value, edge-case, and property tests.
- Every optimized path is checked against the reference within a stated tolerance.
- CPU dispatch selects a supported path and falls back safely.
- GPU paths are verified on the named hardware; unverified backends are labeled.
- Unsupported shapes/dtypes return clear errors or use a documented fallback.
- Benchmarks include methodology, raw samples, hardware, and profiler evidence.
- No illegal instruction, device loss, or OOM is left unhandled.
- Numerical differences are explained and bounded.

## Expected output

1. Kernel contract and reference implementation.
2. Correctness test suite with edge cases and tolerances.
3. Benchmark table: backend, hardware, shape, median, spread, bandwidth/utilization.
4. Profiler evidence and bottleneck analysis.
5. Feature/support matrix with verified and unverified entries.
6. Fallback behavior and error handling.
7. Updated documentation and limitations.

## Troubleshooting

- **Illegal instruction on another CPU:** use runtime feature detection and a scalar fallback; never rely only on compile-time `-march=native`.
- **GPU results differ slightly:** check accumulation order, FMA contraction, precision, atomics, and reduction tree; set a justified tolerance.
- **No speedup:** inspect memory bandwidth, launch overhead, data transfer, occupancy, and whether the compiler already vectorized the scalar loop.
- **GPU OOM:** reduce batch/tile size, free intermediate buffers, and report the exact allocation that failed.
- **Kernel silently does nothing:** check launch dimensions, grid/block limits, error codes, and synchronization before reading output.
- **Thermal throttling:** record clocks/temperature, add cool-down, and report variance rather than cherry-picking the best run.
- **Backend unavailable:** record the probe command and error; keep the CPU path authoritative and mark the task blocked.

## Examples

```sh
# CPU capabilities and compiler vectorization
lscpu | rg 'Model name|Flags|MHz'
cargo rustc --release -- --emit=asm
objdump -d target/release/app | rg -i 'xmm|ymm|zmm'

# GPU probes
nvidia-smi --query-gpu=name,memory.total,driver_version,compute_cap --format=csv
rocm-smi --showproductname --showmeminfo vram
vulkaninfo --summary 2>/dev/null | head -40

# Benchmark with correctness check
./target/release/kernel --shape 1024x1024 --check --reps 20
```

## Related skills and review

- Pair with `performance-benchmarking` for methodology and statistics.
- Pair with `model-training` for tensor, mixed-precision, and training-kernel work.
- Pair with `compiler-engineering` for code generation, vectorization, and target support.
- Reviewer checks: real hardware evidence, scalar fallback, tolerance justification, error handling, and no unsupported claims.
