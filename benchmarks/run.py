#!/usr/bin/env python3
"""Build and run Auren benchmark workloads against Python, C, and Rust.

Methodology:
  * Every implementation prints exactly the same output; this is checked before
    any timing.
  * One warm-up run is discarded, then N measured runs are taken.
  * Wall time includes process startup (fork/exec) for every implementation.
  * `auren-interp` includes parsing and type checking on every run, matching
    `auren run`; `auren-native` is a prebuilt executable.
  * Map workloads use linear key arrays in every language because Auren's
    native and interpreter maps are insertion-ordered linear scans (ADR-0010).
    This keeps the algorithmic work equivalent.
  * Peak RSS is collected with wait4(2) through a tiny C helper, so the
    Python harness's own resident set does not mask the child's peak.

Usage:
    cargo build --release
    python3 benchmarks/run.py --reps 5
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "benchmarks"
RESULTS = BENCH / "results"
WORKLOADS = [
    "arith_loop",
    "fib",
    "list_ops",
    "map_ops",
    "struct_ops",
    "string_ops",
]
IMPLEMENTATIONS = ["auren-interp", "auren-native", "python", "c", "rust"]

DESCRIPTIONS = {
    "arith_loop": "1,000,000 checked Int loop iterations with multiply/modulo/add",
    "fib": "recursive fib(24), 46,368",
    "list_ops": "150,000 list appends, indexed mutation, and sum",
    "map_ops": "3,000 linear-map inserts and 3,000 lookups",
    "struct_ops": "80,000 struct allocations, field mutations, and sum",
    "string_ops": "15 string doublings (65,536 chars) and character scan",
}


def command_for(workload: str, implementation: str, build_dir: Path, auren: Path) -> list[str]:
    source = BENCH / f"{workload}.{ 'auren' if implementation.startswith('auren') else implementation }"
    if implementation == "auren-interp":
        return [str(auren), "run", str(BENCH / f"{workload}.auren")]
    if implementation == "auren-native":
        return [str(build_dir / f"{workload}-auren")]
    if implementation == "python":
        return [sys.executable, str(BENCH / f"{workload}.py")]
    if implementation == "c":
        return [str(build_dir / f"{workload}-c")]
    if implementation == "rust":
        return [str(build_dir / f"{workload}-rust")]
    raise ValueError(f"unknown implementation: {implementation}")


def run_command(command: list[str], timeout: float = 120.0) -> subprocess.CompletedProcess:
    return subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def build_all(auren: Path, build_dir: Path, workloads: list[str]) -> dict:
    clang = shutil.which("clang")
    rustc = shutil.which("rustc")
    if clang is None:
        raise SystemExit("error: clang not found")
    if rustc is None:
        raise SystemExit("error: rustc not found")

    builds: dict[str, dict[str, float]] = {}
    for workload in workloads:
        builds[workload] = {}
        jobs = {
            "auren-native": [
                str(auren),
                "build",
                str(BENCH / f"{workload}.auren"),
                "-o",
                str(build_dir / f"{workload}-auren"),
            ],
            "c": [
                clang,
                "-O2",
                "-std=c11",
                str(BENCH / f"{workload}.c"),
                "-o",
                str(build_dir / f"{workload}-c"),
            ],
            "rust": [
                rustc,
                "-O",
                str(BENCH / f"{workload}.rs"),
                "-o",
                str(build_dir / f"{workload}-rust"),
            ],
        }
        for name, command in jobs.items():
            start = time.perf_counter()
            result = run_command(command, timeout=300.0)
            elapsed = time.perf_counter() - start
            if result.returncode != 0:
                raise SystemExit(
                    f"error: build failed for {workload}/{name}\n"
                    f"command: {' '.join(command)}\n{result.stderr}"
                )
            builds[workload][name] = elapsed
    return builds


def verify_outputs(
    workloads: list[str], build_dir: Path, auren: Path
) -> dict[str, dict[str, str]]:
    outputs: dict[str, dict[str, str]] = {}
    for workload in workloads:
        outputs[workload] = {}
        for implementation in IMPLEMENTATIONS:
            command = command_for(workload, implementation, build_dir, auren)
            result = run_command(command)
            if result.returncode != 0:
                raise SystemExit(
                    f"error: {workload}/{implementation} exited {result.returncode}\n"
                    f"stderr:\n{result.stderr}"
                )
            outputs[workload][implementation] = result.stdout
        reference = outputs[workload]["auren-interp"]
        for implementation, output in outputs[workload].items():
            if output != reference:
                raise SystemExit(
                    f"error: output mismatch for {workload}/{implementation}\n"
                    f"reference: {reference!r}\nactual:    {output!r}"
                )
    return outputs


def build_measure_helper(clang: str, build_dir: Path) -> Path:
    helper = build_dir / "measure"
    result = run_command(
        [
            clang,
            "-O2",
            "-std=c11",
            str(BENCH / "measure.c"),
            "-o",
            str(helper),
        ],
        timeout=120.0,
    )
    if result.returncode != 0:
        raise SystemExit(f"error: failed to build measurement helper\n{result.stderr}")
    return helper


def measure_once(helper: Path, command: list[str]) -> tuple[float, int, int]:
    """Run one child through the helper, returning (ms, maxrss_kib, exit_code)."""
    result = subprocess.run(
        [str(helper), *command],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=300.0,
    )
    for line in reversed(result.stderr.splitlines()):
        if line.startswith("AUREN_MEASURE "):
            _, elapsed, maxrss, exit_code = line.split()
            return float(elapsed), int(maxrss), int(exit_code)
    raise SystemExit(
        f"error: measurement helper produced no result for {command}\n"
        f"stderr:\n{result.stderr}"
    )


def measure(
    helper: Path, command: list[str], repetitions: int
) -> dict[str, float | int | list[float]]:
    _, _, exit_code = measure_once(helper, command)
    if exit_code != 0:
        raise SystemExit(f"error: warm-up failed with exit {exit_code}: {command}")
    times: list[float] = []
    rss: list[int] = []
    for _ in range(repetitions):
        elapsed, maxrss, exit_code = measure_once(helper, command)
        if exit_code != 0:
            raise SystemExit(f"error: run failed with exit {exit_code}: {command}")
        times.append(elapsed)
        rss.append(maxrss)
    return {
        "times_ms": [round(value, 3) for value in times],
        "median_ms": round(statistics.median(times), 3),
        "min_ms": round(min(times), 3),
        "max_ms": round(max(times), 3),
        "peak_rss_mib": round(max(rss) / 1024.0, 2),
        "exit_code": exit_code,
    }


def environment() -> dict[str, str | int]:
    cpu_model = "unknown"
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu_model = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    mem_total = "unknown"
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                mem_total = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    return {
        "platform": platform.platform(),
        "cpu": cpu_model,
        "cores": os.cpu_count() or 0,
        "memory": mem_total,
        "python": sys.version.split()[0],
        "clang": subprocess.run(
            ["clang", "--version"], capture_output=True, text=True
        ).stdout.splitlines()[0],
        "rustc": subprocess.run(
            ["rustc", "--version"], capture_output=True, text=True
        ).stdout.strip(),
        "auren": subprocess.run(
            [str(ROOT / "target/release/auren"), "version"],
            capture_output=True,
            text=True,
        ).stdout.strip(),
    }


def markdown(report: dict, workloads: list[str]) -> str:
    lines = [
        "# Auren benchmark baseline",
        "",
        f"Generated: {report['generated_at']}",
        "",
        "Every implementation was verified to produce identical output before",
        "timing. Wall time includes process startup. One warm-up run was",
        "discarded; medians are over the measured runs.",
        "",
        f"Environment: {report['environment']['cpu']}, "
        f"{report['environment']['cores']} cores, "
        f"{report['environment']['memory']}; "
        f"{report['environment']['platform']}.",
        "",
        "| Workload | Implementation | Median ms | Min ms | Max ms | Peak RSS MiB |",
        "| --- | --- | ---: | ---: | ---: | ---: |",
    ]
    for workload in workloads:
        for implementation in IMPLEMENTATIONS:
            stats = report["results"][workload][implementation]
            lines.append(
                f"| {workload} | {implementation} | {stats['median_ms']:.3f} | "
                f"{stats['min_ms']:.3f} | {stats['max_ms']:.3f} | "
                f"{stats['peak_rss_mib']:.2f} |"
            )
    lines += [
        "",
        "## Build time and binary size",
        "",
        "| Workload | Auren native build s | C build s | Rust build s | "
        "Auren binary B | C binary B | Rust binary B |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for workload in workloads:
        builds = report["builds"][workload]
        sizes = report["sizes"][workload]
        lines.append(
            f"| {workload} | {builds['auren-native']:.3f} | {builds['c']:.3f} | "
            f"{builds['rust']:.3f} | {sizes['auren-native']} | {sizes['c']} | "
            f"{sizes['rust']} |"
        )
    lines += [
        "",
        "## Notes",
        "",
        "- `auren-interp` re-parses and type-checks the source on every run, as",
        "  `auren run` does; `auren-native` is a prebuilt executable.",
        "- `auren-native` build time includes invoking clang with `-O2`.",
        "- Map workloads use linear key arrays in every language, matching",
        "  Auren's insertion-ordered linear map (ADR-0010). They are not a",
        "  comparison against hash-table implementations.",
        "- String workloads allocate immutable concatenation results; native",
        "  allocations are intentionally not reclaimed.",
        "- These numbers are a single-machine baseline, not a competitive claim.",
        "- Peak RSS is measured with wait4(2) from a small C helper; the helper's",
        "  own resident set is the floor reported for very small binaries.",
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reps", type=int, default=5)
    parser.add_argument(
        "--workload",
        action="append",
        choices=WORKLOADS,
        help="benchmark only the given workload (repeatable)",
    )
    parser.add_argument(
        "--auren",
        default=str(ROOT / "target/release/auren"),
        help="path to the release auren binary",
    )
    args = parser.parse_args()

    auren = Path(args.auren).resolve()
    if not auren.is_file():
        print(f"error: auren binary not found: {auren}", file=sys.stderr)
        print("hint: run `cargo build --release` first", file=sys.stderr)
        return 2
    workloads = args.workload or WORKLOADS
    if args.reps < 1:
        print("error: --reps must be at least 1", file=sys.stderr)
        return 2

    RESULTS.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="auren-bench-") as temporary:
        build_dir = Path(temporary)
        print("building native implementations...", file=sys.stderr)
        builds = build_all(auren, build_dir, workloads)
        clang = shutil.which("clang")
        if clang is None:
            raise SystemExit("error: clang not found")
        helper = build_measure_helper(clang, build_dir)
        print("verifying identical output...", file=sys.stderr)
        verify_outputs(workloads, build_dir, auren)

        results: dict[str, dict] = {}
        for workload in workloads:
            results[workload] = {}
            for implementation in IMPLEMENTATIONS:
                command = command_for(workload, implementation, build_dir, auren)
                print(f"measuring {workload}/{implementation}...", file=sys.stderr)
                results[workload][implementation] = measure(
                    helper, command, args.reps
                )

        sizes: dict[str, dict[str, int]] = {}
        for workload in workloads:
            sizes[workload] = {
                "auren-source": (BENCH / f"{workload}.auren").stat().st_size,
                "auren-native": (build_dir / f"{workload}-auren").stat().st_size,
                "c": (build_dir / f"{workload}-c").stat().st_size,
                "rust": (build_dir / f"{workload}-rust").stat().st_size,
            }

    report = {
        "generated_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "repetitions": args.reps,
        "warmup_runs": 1,
        "environment": environment(),
        "descriptions": {workload: DESCRIPTIONS[workload] for workload in workloads},
        "results": results,
        "builds": builds,
        "sizes": sizes,
        "verification": "all implementations produced identical stdout",
    }

    date = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    json_path = RESULTS / f"{date}.json"
    markdown_path = RESULTS / f"{date}.md"
    json_path.write_text(json.dumps(report, indent=2) + "\n")
    summary = markdown(report, workloads)
    markdown_path.write_text(summary + "\n")
    print(summary)
    print(f"wrote {json_path.relative_to(ROOT)}", file=sys.stderr)
    print(f"wrote {markdown_path.relative_to(ROOT)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
