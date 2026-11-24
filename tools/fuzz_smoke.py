#!/usr/bin/env python3
"""Malformed-input and depth-limit smoke fuzzer for the Auren front end.

The compiler must return a controlled diagnostic (exit 0 or 1) for any input.
A panic, stack overflow, timeout, or signal is a failure.

Usage:
    python3 tools/fuzz_smoke.py --binary target/debug/auren
    python3 tools/fuzz_smoke.py --binary target/debug/auren --iterations 1000
"""

from __future__ import annotations

import argparse
import os
import random
import subprocess
import sys
import tempfile

TOKENS = [
    "fn", "struct", "let", "var", "if", "elif", "else", "while", "for",
    "in", "return", "break", "continue", "print", "len", "range", "true",
    "false", "not", "and", "or", "Int", "Float", "Bool", "Str", "Any",
    "List", "Map", ":", ",", ".", "(", ")", "[", "]", "{", "}", "+", "-",
    "*", "/", "//", "%", "**", "=", "+=", "==", "!=", "<", "<=", ">", ">=",
    "->", "0", "1", "42", "3.14", '"text"', '"a{b}c"', "name", "_x", "\n",
    "    ", "\t", "\\", "\x00",
]

VALID_SNIPPETS = [
    'fn main():\n    print("hello")\n',
    "fn add(a: Int, b: Int) -> Int:\n    return a + b\n\nfn main():\n    print(add(1, 2))\n",
    'struct Point:\n    x: Int\n    y: Int\n\nfn main():\n    let p = Point(1, 2)\n    print(p)\n',
    'fn main():\n    let xs = [1, 2, 3]\n    for x in xs:\n        print(x)\n',
    'fn main():\n    let m = {"a": 1}\n    print(m["a"])\n',
    "fn main():\n    var i = 0\n    while i < 3:\n        i += 1\n    print(i)\n",
    'fn main():\n    let s = "héllo"\n    print(s[1])\n',
]

DEPTH_CASES = [
    ("long-chain", "fn main():\n    let x = " + " + ".join(["1"] * 10_000) + "\n"),
    (
        "nested-blocks",
        "fn main():\n"
        + "".join("    " * (i + 1) + "if true:\n" for i in range(2_000))
        + "    " * 2_001
        + 'print("x")\n',
    ),
    ("nested-parens", "fn main():\n    let x = " + "(" * 2_000 + "1" + ")" * 2_000 + "\n"),
    ("nested-unary", "fn main():\n    let x = " + "-" * 2_000 + "1\n"),
    (
        "nested-types",
        "fn main():\n    let x: " + "List[" * 300 + "Int" + "]" * 300 + " = []\n",
    ),
    (
        "nested-lists",
        "fn main():\n    let x = " + "[" * 2_000 + "1" + "]" * 2_000 + "\n",
    ),
    (
        "nested-calls",
        "fn f(x: Int) -> Int:\n    return x\n\nfn main():\n    print("
        + "f(" * 2_000
        + "1"
        + ")" * 2_000
        + ")\n",
    ),
    ("nul-byte", "fn main():\n    let x = 1\x00\n"),
    ("huge-string", 'fn main():\n    print("' + "a" * 1_000_000 + '")\n'),
]


def run_case(binary: str, directory: str, name: str, source: str) -> str | None:
    path = os.path.join(directory, name + ".auren")
    with open(path, "w", encoding="utf-8", errors="surrogateescape") as handle:
        handle.write(source)
    try:
        result = subprocess.run(
            [binary, "check", path],
            capture_output=True,
            text=True,
            timeout=20,
        )
    except subprocess.TimeoutExpired:
        return f"{name}: timed out after 20s"
    if result.returncode not in (0, 1):
        return f"{name}: unexpected exit {result.returncode}"
    combined = result.stdout + result.stderr
    for marker in ("panicked", "overflowed its stack", "RUST_BACKTRACE"):
        if marker in combined:
            return f"{name}: {marker}"
    return None


def random_source(rng: random.Random) -> str:
    parts = [rng.choice(TOKENS) for _ in range(rng.randint(1, 300))]
    return " ".join(parts)


def mutated_source(rng: random.Random) -> str:
    base = list(rng.choice(VALID_SNIPPETS))
    for _ in range(rng.randint(1, 12)):
        if not base:
            break
        operation = rng.choice(("delete", "insert", "replace"))
        position = rng.randrange(len(base))
        if operation == "delete":
            del base[position]
        elif operation == "insert":
            base.insert(position, rng.choice(TOKENS))
        else:
            base[position] = rng.choice(TOKENS)
    return "".join(base)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/auren")
    parser.add_argument("--iterations", type=int, default=400)
    parser.add_argument("--seed", type=int, default=20261122)
    args = parser.parse_args()

    if not os.path.isfile(args.binary):
        print(f"error: binary not found: {args.binary}", file=sys.stderr)
        return 2

    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="auren-fuzz-") as directory:
        for name, source in DEPTH_CASES:
            failure = run_case(args.binary, directory, name, source)
            if failure:
                failures.append(failure)

        rng = random.Random(args.seed)
        for index in range(args.iterations):
            source = random_source(rng) if index % 2 == 0 else mutated_source(rng)
            failure = run_case(args.binary, directory, f"random-{index}", source)
            if failure:
                failures.append(failure)
                if len(failures) >= 10:
                    break

    if failures:
        print("FAIL")
        for failure in failures:
            print("  " + failure)
        return 1
    print(
        f"OK: {len(DEPTH_CASES)} depth/limit cases and {args.iterations} "
        f"random/mutated inputs produced controlled diagnostics"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
