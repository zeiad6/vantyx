#!/usr/bin/env python3
"""Unit tests for the deterministic differential fuzzer's comparison and shrink logic.

These tests do not build native programs. They inject a fake case executor so the
line-oriented shrinker can be checked deterministically and quickly. The real
differential run remains `tools/differential_fuzz.py`.
"""

from __future__ import annotations

import importlib.util
import random
import sys
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("differential_fuzz.py")
SPEC = importlib.util.spec_from_file_location("differential_fuzz", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
fuzz = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fuzz)


class CaseResultTests(unittest.TestCase):
    def test_matching_output_is_not_a_failure(self) -> None:
        result = fuzz.CaseResult(0, "same\n", 0, "", 0, "same\n", "")
        self.assertFalse(result.failed())

    def test_both_engines_rejecting_is_not_a_failure(self) -> None:
        result = fuzz.CaseResult(1, "", 1, "error[E0401]", None, "", "")
        self.assertFalse(result.failed())

    def test_native_build_failure_is_a_failure(self) -> None:
        result = fuzz.CaseResult(0, "ran\n", 1, "error[E0401]", None, "", "")
        self.assertTrue(result.failed())

    def test_runtime_mismatch_is_a_failure(self) -> None:
        result = fuzz.CaseResult(0, "one\n", 0, "", 0, "two\n", "")
        self.assertTrue(result.failed())


class RunTests(unittest.TestCase):
    def test_timeout_is_reported_as_exit_124(self) -> None:
        code, _, stderr = fuzz.run(
            [sys.executable, "-c", "import time; time.sleep(5)"], Path.cwd(), 1
        )
        self.assertEqual(code, 124)
        self.assertIn("timeout", stderr)


class GeneratorTests(unittest.TestCase):
    def test_display_program_is_deterministic_and_well_formed(self) -> None:
        first = fuzz.display_program(random.Random(7))
        second = fuzz.display_program(random.Random(7))
        self.assertEqual(first, second)
        self.assertIn("struct Node:", first)
        self.assertIn("push(leaf.children, root)", first)
        self.assertIn("Map[Key, Int]", first)
        self.assertIn("Map[List[Int], Str]", first)
        self.assertIn("Map[Shape, Int]", first)
        self.assertIn("print([root, leaf])", first)
        self.assertNotIn("{{", first)
        self.assertNotIn("}}", first)
        self.assertEqual(first.count("{"), first.count("}"))

    def test_every_generator_is_registered_and_deterministic(self) -> None:
        for generator in fuzz.GENERATORS:
            with self.subTest(generator=generator.__name__):
                first = generator(random.Random(11))
                second = generator(random.Random(11))
                self.assertEqual(first, second)
                self.assertTrue(first.strip())


class ShrinkTests(unittest.TestCase):
    SOURCE = """fn helper() -> Int:
    return 1

fn main():
    let unused = 5
    print(helper())
    print("second")
    print("third")

main()
"""

    def fake_executor(self, calls: list[str]):
        def execute(binary: Path, source: str, timeout: int):
            calls.append(source)
            failed = "print(" in source
            return fuzz.CaseResult(
                0,
                "interpreter",
                0,
                "",
                0,
                "native" if failed else "interpreter",
                "",
            )

        return execute

    def test_shrinker_preserves_a_failure_and_removes_lines(self) -> None:
        calls: list[str] = []
        original = fuzz.execute_case
        fuzz.execute_case = self.fake_executor(calls)
        try:
            minimized, attempts = fuzz.shrink_source(
                Path("unused"), self.SOURCE, (0, 0, 0), 1, 50
            )
        finally:
            fuzz.execute_case = original
        self.assertIn("print(", minimized)
        self.assertLess(len(minimized.splitlines()), len(self.SOURCE.splitlines()))
        self.assertEqual(attempts, len(calls))
        self.assertGreater(attempts, 0)

    def test_shrinker_respects_its_budget(self) -> None:
        calls: list[str] = []
        original = fuzz.execute_case
        fuzz.execute_case = self.fake_executor(calls)
        try:
            _, attempts = fuzz.shrink_source(
                Path("unused"), self.SOURCE, (0, 0, 0), 1, 3
            )
        finally:
            fuzz.execute_case = original
        self.assertLessEqual(attempts, 3)
        self.assertEqual(attempts, len(calls))


if __name__ == "__main__":
    unittest.main()
