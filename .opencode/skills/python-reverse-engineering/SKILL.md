---
name: Python Ecosystem Reverse Engineering
description: Study a Python library's API, architecture, algorithms, licensing, and performance to design an original native reimplementation, with clean-room constraints, reproducible experiments, and honest feature mapping.
---

# Python Ecosystem Reverse Engineering

## When to use

- Evaluating a Python library or ecosystem pattern for a native, faster, or dependency-free implementation.
- Mapping a library's public behavior, data model, edge cases, and performance envelope.
- Comparing candidate libraries before adopting or reimplementing one.
- Determining whether a reimplementation is legally and technically feasible.

Do not use this skill to copy source code, bypass a license, or clone a proprietary service. The goal is understanding and original design.

## Required tools

- `python3`, `pip`/`uv`, `venv`, `pytest`, `inspect`, `ast`, `dis`, `cProfile`, `tracemalloc`.
- `rg` (ripgrep), `git`, `curl`/`wget` only when network access is authorized.
- Optional: `py-spy`, `scalene`, `line_profiler`, `pipdeptree`, `pip-audit`, `licensecheck`, `syft`/SBOM tools.
- A benchmark harness or `pyperf`/`hyperfine` for fair comparisons.
- A clean-room implementation environment separate from the studied package's source.

## Safety constraints

- Read the license and terms before studying implementation details. Record the exact version, license, source URL, and retrieval date.
- Do not copy source, comments, assets, test fixtures, or proprietary algorithms without authorization. Prefer documentation, public API behavior, and independent experiments.
- Respect trademarks, patents, database rights, and API terms of service. If the license is unclear or incompatible, stop and document the blocker.
- Pin versions and hashes. Do not install untrusted packages outside a virtual environment or sandbox.
- Do not exfiltrate private data, credentials, or proprietary code to external services.
- Treat benchmark results as evidence only when the methodology is recorded; never fabricate or cherry-pick numbers.

## Workflow

1. **Define the capability and success criteria.** State the user problem, required behavior, performance target, compatibility level, and what is out of scope. A vague goal like "clone library X" is not acceptable.

2. **Select candidates deliberately.** Score each candidate on real-world usage, maintenance activity, license, API stability, dependency weight, performance ceiling, and relevance. Record why alternatives were rejected.

3. **Pin and provenance the study.**
   ```sh
   python3 -m venv /tmp/study-venv
   /tmp/study-venv/bin/pip install 'library==X.Y.Z'
   /tmp/study-venv/bin/pip freeze
   python3 -c 'import library; print(library.__version__, library.__file__)'
   ```
   Record the release tag/commit, wheel or sdist hash, Python version, platform, and license text location.

4. **Map the public API.** Identify the 5–10 calls that carry the core workflows. For each record:
   - signature, accepted types, defaults, and keyword-only behavior;
   - return type, mutation, laziness, and ownership;
   - exceptions and error messages;
   - thread/process safety;
   - documented and observed edge cases.
   Use `inspect.signature`, `help`, type hints, and the official docs.

5. **Study architecture from allowed sources.** Examine package layout, entry points, optional dependencies, C/Rust extensions, lazy vs eager execution, memory layout, and extension points. Use `python -X importtime`, `pipdeptree`, `ast`, and `dis` to observe behavior. Do not reproduce source.

6. **Run behavioral experiments.** Write small, version-pinned scripts that exercise empty input, duplicates, nulls, Unicode, very large values, invalid types, and boundary conditions. Save scripts and outputs as evidence. Prefer property-based tests where the contract is clear.

7. **Analyze algorithms and cost.** Determine time and space complexity, allocation behavior, I/O patterns, vectorization, caching, and concurrency. Profile representative workloads with `cProfile`, `py-spy`, or `tracemalloc`; distinguish algorithmic gains from implementation tricks.

8. **Benchmark the baseline fairly.** Use the same algorithm, input, warm-up, repetitions, and output work. Record hardware, OS, Python version, package version, and flags. Report median, spread, and raw samples; see `performance-benchmarking`.

9. **Perform a license and compatibility review.** Decide whether the intended reimplementation is a clean-room independent work, an API-compatible interface, or a derivative work. Document attribution, notices, and any restrictions. If legal status is uncertain, stop before implementation.

10. **Design the native reimplementation.** Specify the API surface, data layout, error model, memory ownership, dependency policy, compatibility layer, and intentionally unsupported features. Write an ADR before coding.

11. **Implement and differential-test.** Build the smallest coherent capability first. Compare against the Python reference on a feature matrix: outputs, exceptions, edge cases, and performance. Fuzz the input space where practical.

12. **Document deviations and evidence.** Publish a feature matrix, benchmark table, license note, and limitations. Mark unverified behavior explicitly.

## Validation and acceptance criteria

- The study names exact versions, hashes, licenses, and evidence sources.
- Behavioral claims are backed by a reproducible script, test, or cited document.
- The feature matrix states supported, partial, and unsupported behavior.
- The implementation is original and integrated, not a stub or a copied port.
- Differential tests cover normal, error, boundary, and Unicode cases.
- Benchmarks include methodology, raw data, and a fair comparison; no fabricated speedups.
- License and attribution obligations are recorded and satisfied.
- Limitations and unverified areas are visible in the final report.

## Expected output

1. Research note: candidates, selection rationale, version/license provenance.
2. API map and behavioral test suite.
3. Architecture and algorithm analysis with profiling evidence.
4. Feature matrix: Python reference vs native implementation.
5. Benchmark report with raw samples and threats to validity.
6. License/compatibility memo and ADR.
7. Implementation, tests, and a list of intentional deviations.

## Troubleshooting

- **C extension hides the algorithm:** study the public behavior and documented semantics; do not decompile proprietary code. Mark internal details unknown.
- **License is incompatible or unclear:** stop implementation, document the blocker, and propose an alternative library or clean-room specification.
- **Behavior changes across versions:** pin one version, record it, and test the version range only if compatibility is a requirement.
- **Benchmark is unfair:** equalize algorithm, input, warm-up, output, and environment; compare like with like.
- **Optional dependency missing:** record the missing extra and mark affected experiments unverified instead of guessing.
- **Scope explodes:** cut to one workflow with measurable value; list the rest as future work.

## Examples

```sh
# Pin and inspect a candidate
python3 -m venv /tmp/study && /tmp/study/bin/pip install 'orjson==3.10.7'
/tmp/study/bin/python - <<'PY'
import inspect, orjson
print(orjson.__version__)
print(inspect.signature(orjson.dumps))
print(orjson.dumps({"b": 1, "a": 2}))
PY

# Behavioral edge cases
/tmp/study/bin/python - <<'PY'
import orjson
for value in (None, True, 1, 1.5, "text", [], {}, {"x": float("nan")}):
    try:
        print(repr(value), "->", orjson.dumps(value))
    except Exception as exc:
        print(repr(value), "!!", type(exc).__name__, exc)
PY

# Profile a workload
/tmp/study/bin/python -m cProfile -s cumulative bench.py | head -30
```

## Related skills and review

- Pair with `performance-benchmarking` for any speed or memory claim.
- Pair with `security-audit-engineering` when the library parses untrusted input, executes code, or handles secrets.
- Pair with `compiler-engineering` if the reimplementation includes a language, parser, or type system.
- Reviewer checks: license recorded, experiments reproducible, feature matrix honest, no copied code, and benchmarks fair.
