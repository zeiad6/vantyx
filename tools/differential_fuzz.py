#!/usr/bin/env python3
"""Deterministic differential fuzzer for the Vantyx interpreter and native backend.

The interpreter is the semantic oracle. This tool generates well-typed
programs from fixed grammars, runs each one through `vantyx run` and
`vantyx build`, and compares the exit code and stdout. A mismatch is a
failure; the program is printed and can be saved for a regression test.

The nine program shapes cover scalar expressions and control flow,
collections/structs/enums, guarded/range/or-pattern matching, nested
collections with aliasing, typed functions with early returns and recursion,
string escapes/interpolation/UTF-8 builtins, numeric formatting and
conversions, result-enum `?` propagation, and runtime-error agreement. Every
generated program is designed to run to completion or fail at the same point
in both engines; a program that only prints its prefix before a runtime error
has little differential value.

The generator is deterministic for a given seed, so CI failures are
reproducible:

    python3 tools/differential_fuzz.py --iterations 100 --seed 1

Native checks are skipped when no C compiler is available (set VANTYX_CC to
override the default `clang`). This tool uses only the Python standard library.
"""

from __future__ import annotations

import argparse
import os
import random
import subprocess
import sys
import tempfile
from pathlib import Path

INT_LITERALS = ["0", "1", "2", "3", "7", "-1", "-7", "42", "1000"]
FLOAT_LITERALS = ["0.0", "1.0", "-1.0", "1.5", "0.1", "1e300", "5e-324", "3.14159"]
STR_LITERALS = ['"a"', '"hello"', '"héllo"', '""', '"x y z"', '"é👋"']


def int_expr(rng: random.Random, depth: int) -> str:
    if depth <= 0:
        return rng.choice(INT_LITERALS + ["x", "y"])
    op = rng.choice(["+", "-", "*", "//", "%", "**", "+", "-", "*"])
    if op in ("//", "%"):
        return f"({int_expr(rng, depth - 1)} {op} {rng.choice(['1', '2', '3', '7', '-3'])})"
    if op == "**":
        base = rng.choice(["0", "1", "2", "3", "7", "-1", "-2", "42"])
        return f"({base} ** {rng.choice(['0', '1', '2', '3'])})"
    return f"({int_expr(rng, depth - 1)} {op} {int_expr(rng, depth - 1)})"


def float_expr(rng: random.Random, depth: int) -> str:
    if depth <= 0:
        return rng.choice(FLOAT_LITERALS + ["f", "g"])
    kind = rng.randrange(5)
    if kind == 0:
        return f"({float_expr(rng, depth - 1)} {rng.choice(['+', '-', '*'])} {float_expr(rng, depth - 1)})"
    if kind == 1:
        return f"({float_expr(rng, depth - 1)} / {rng.choice(['1.0', '2.0', '-3.0', '0.5'])})"
    if kind == 2:
        return f"float({int_expr(rng, depth - 1)})"
    if kind == 3:
        if rng.random() < 0.5:
            return f"sqrt({rng.choice(['4.0', '2.25', '0.0'])})"
        return (
            f"float({rng.choice(['floor', 'ceil'])}"
            f"({rng.choice(['4.0', '2.25', '0.0'])}))"
        )
    return f"({float_expr(rng, depth - 1)} * {float_expr(rng, depth - 1)})"


def bool_expr(rng: random.Random, depth: int) -> str:
    if depth <= 0:
        return rng.choice(["true", "false", "b"])
    kind = rng.randrange(4)
    if kind == 0:
        return f"({int_expr(rng, depth - 1)} {rng.choice(['<', '<=', '>', '>=', '==', '!='])} {int_expr(rng, depth - 1)})"
    if kind == 1:
        return f"({bool_expr(rng, depth - 1)} {rng.choice(['and', 'or'])} {bool_expr(rng, depth - 1)})"
    if kind == 2:
        return f"(not {bool_expr(rng, depth - 1)})"
    return f"({float_expr(rng, depth - 1)} {rng.choice(['<', '<=', '>', '>=', '==', '!='])} {float_expr(rng, depth - 1)})"


def str_expr(rng: random.Random, depth: int) -> str:
    if depth <= 0:
        return rng.choice(STR_LITERALS + ["s", "t"])
    kind = rng.randrange(6)
    if kind == 0:
        return f"({str_expr(rng, depth - 1)} + {str_expr(rng, depth - 1)})"
    if kind == 1:
        return f"str({int_expr(rng, depth - 1)})"
    if kind == 2:
        return f"str({float_expr(rng, depth - 1)})"
    if kind == 3:
        return (
            f"replace({str_expr(rng, depth - 1)}, "
            f"{rng.choice(['\"a\"', '\"l\"', '\"é\"'])}, "
            f"{rng.choice(['\"X\"', '\"\"', '\"-\"'])})"
        )
    if kind == 4:
        return f"trim({str_expr(rng, depth - 1)})"
    return (
        f"join(split({str_expr(rng, depth - 1)}, "
        f"{rng.choice(['\" \"', '\"l\"', '\"\"', '\"é\"'])}), \"-\")"
    )


EXPR_GENERATORS = {
    "Int": int_expr,
    "Float": float_expr,
    "Bool": bool_expr,
    "Str": str_expr,
}


def gen_expr(rng: random.Random, ty: str, depth: int) -> str:
    return EXPR_GENERATORS[ty](rng, depth)


def expression_program(rng: random.Random) -> str:
    lines = [
        "fn helper(n: Int) -> Int:",
        "    if n <= 1:",
        "        return 1",
        "    return n * helper(n - 1)",
        "",
        "fn main():",
        "    let x: Int = 3",
        "    let y: Int = -2",
        "    let f: Float = 1.5",
        "    let g: Float = -0.5",
        "    let b: Bool = true",
        '    let s: Str = "hello"',
        '    let t: Str = "a b"',
    ]
    for statement_index in range(rng.randint(3, 8)):
        kind = rng.randrange(7)
        loop_var = f"i{statement_index}"
        if kind == 0:
            lines.append(
                f"    print({gen_expr(rng, rng.choice(list(EXPR_GENERATORS)), rng.randint(1, 3))})"
            )
        elif kind == 1:
            lines.append(f"    print(helper({rng.choice(['0', '1', '3', '5'])}))")
        elif kind == 2:
            lines.append(f"    var {loop_var} = 0")
            lines.append(f"    while {loop_var} < {rng.randint(0, 4)}:")
            lines.append(f"        {loop_var} += 1")
            lines.append(f"        print({loop_var})")
        elif kind == 3:
            lines.append(f"    for {loop_var} in range({rng.randint(0, 4)}):")
            body = (
                gen_expr(rng, "Int", rng.randint(0, 2))
                .replace("x", loop_var)
                .replace("y", loop_var)
            )
            lines.append(f"        print({body})")
        elif kind == 4:
            lines.append(f"    if {bool_expr(rng, 2)}:")
            lines.append(f"        print({gen_expr(rng, 'Int', 2)})")
            lines.append("    else:")
            lines.append(f"        print({gen_expr(rng, 'Str', 2)})")
        elif kind == 5:
            lines.append(f"    print(len({str_expr(rng, 2)}))")
        else:
            op = rng.choice(["<", "<=", ">", ">=", "==", "!="])
            lines.append(f"    print({int_expr(rng, 2)} {op} {int_expr(rng, 2)})")
    return "\n".join(lines) + "\n"


def collection_program(rng: random.Random) -> str:
    """Lists, maps, strings, structs, enums, and match in one program."""
    n = rng.randint(2, 5)
    values = [str(rng.randint(-5, 9)) for _ in range(n)]
    words = rng.sample(["a", "b", "héllo", "é👋", "x y"], k=min(3, 5))
    match_arm = rng.choice(
        [
            '        0: "zero"',
            '        1..=3: "small"',
            '        2 | 3: "pair"',
        ]
    )
    return f"""struct Point:
    x: Int
    y: Int

enum Item:
    Num(value: Int)
    Text(text: Str)
    Empty

fn classify(n: Int) -> Str:
    return match n:
{match_arm}
        _: "other"

fn bump(p: Point) -> Point:
    p.x += 1
    return p

fn main():
    let xs: List[Int] = [{", ".join(values)}]
    push(xs, {rng.randint(-5, 9)})
    print(xs)
    print(len(xs))
    print(xs[len(xs) - 1])
    print(contains(xs, {values[0]}))
    print(pop(xs))
    print(xs)
    let words: List[Str] = [{", ".join('"' + w + '"' for w in words)}]
    print(join(words, "-"))
    print(split("a,b,c", ","))
    print(replace("banana", "na", "X"))
    print(trim("  padded  "))
    print(starts_with("héllo", "hé"))
    print(ends_with("héllo", "lo"))
    let text: Str = "{words[0]}"
    print(text[0])
    print(len(text))
    let m: Map[Str, Int] = {{"b": 2, "a": 1}}
    m["c"] = {rng.randint(0, 9)}
    m["a"] = 9
    print(m)
    print(keys(m))
    print(values(m))
    print(contains(m, "b"))
    print(len(m))
    let p = Point({rng.randint(0, 9)}, {rng.randint(0, 9)})
    let q = bump(p)
    print(p)
    print(q)
    print(p == q)
    let items: List[Item] = [Num({rng.randint(0, 5)}), Text("x"), Empty()]
    for item in items:
        print(item)
    print(classify({rng.randint(-1, 5)}))
    print(classify({rng.randint(-1, 5)}))

main()
"""


def match_program(rng: random.Random) -> str:
    low = rng.randint(-5, 2)
    high = low + 3 + rng.randint(0, 3)
    return f"""enum Shape:
    Circle(radius: Int)
    Rect(width: Int, height: Int)
    Empty

var checks: Int = 0

fn seen() -> Bool:
    checks += 1
    return true

fn area(shape: Shape) -> Int:
    return match shape:
        Circle(r) if r > {rng.randint(0, 5)}: r * r
        Circle(r): r
        Rect(w, h) if w == h: w * h
        Rect(w, h): w + h
        Empty: 0

fn label(n: Int) -> Str:
    return match n:
        {low}: "low"
        {low + 1} | {low + 2}: "near"
        {low + 3}..={high}: "range"
        _ if seen(): "guarded"
        _: "other"

fn main():
    print(area(Circle({rng.randint(0, 9)})))
    print(area(Rect({rng.randint(0, 9)}, {rng.randint(0, 9)})))
    print(area(Empty()))
    print(label({low}))
    print(label({low + 1}))
    print(label({low + 4}))
    print(label({high + 10}))
    print(checks)
    let flag = false and seen()
    print(flag)
    print(checks)

main()
"""


SAFE_INTS = ["-9", "-7", "-3", "-1", "0", "1", "2", "3", "5", "7", "9", "11", "42"]
SAFE_WORDS = ['"a"', '"hello"', '"héllo"', '"é👋"', '"x y"', '""', '"banana"']


def nested_program(rng: random.Random) -> str:
    """Nested lists, maps, structs, and enums with aliasing and mutation."""
    first = [rng.randint(-9, 9) for _ in range(rng.randint(0, 4))]
    second = [rng.randint(-9, 9) for _ in range(rng.randint(0, 4))]
    key_low = rng.randint(-5, 2)
    key_high = key_low + rng.randint(1, 4)
    text_key = rng.choice(['"alpha"', '"beta"', '"é"'])
    map_value = rng.randint(-9, 9)
    box_values = [rng.randint(-9, 9) for _ in range(rng.randint(1, 4))]
    index = rng.randrange(len(box_values))
    return f"""struct Box:
    items: List[Int]
    label: Str

enum Tree:
    Leaf(value: Int)
    Branch(items: List[Int], tag: Str)

fn total(b: Box) -> Int:
    var sum = 0
    for item in b.items:
        sum += item
    return sum

fn grow(b: Box, amount: Int):
    push(b.items, amount)

fn size(t: Tree) -> Int:
    return match t:
        Leaf(value): value
        Branch(items, tag): len(items) + len(tag)

fn main():
    let rows: List[List[Int]] = [[{", ".join(map(str, first))}], [{", ".join(map(str, second))}], []]
    push(rows[2], {rng.randint(-9, 9)})
    rows[0][0] = {rng.randint(-9, 9)}
    print(rows)
    print(len(rows))
    print(len(rows[0]))
    print(rows[2][0])
    let m: Map[Str, List[Int]] = {{"a": [1], "b": []}}
    push(m["b"], {rng.randint(-9, 9)})
    m["c"] = rows[2]
    print(m)
    print(keys(m))
    print(len(m["b"]))
    let by_int: Map[Int, Str] = {{{key_low}: "low", {key_high}: "high"}}
    by_int[{key_low}] = "LOW"
    by_int[{key_high + 1}] = "new"
    print(by_int)
    print(keys(by_int))
    let boxes: List[Box] = [Box([{", ".join(map(str, box_values))}], {text_key})]
    grow(boxes[0], {rng.randint(-9, 9)})
    boxes[0].items[{index}] += 1
    print(boxes)
    print(total(boxes[0]))
    print(boxes[0].items[{index}])
    let copy = boxes[0]
    copy.label = "copied"
    print(boxes[0].label)
    print(copy.label)
    let trees: List[Tree] = [Leaf({map_value}), Branch([{", ".join(map(str, box_values))}], {text_key})]
    for tree in trees:
        print(tree)
        print(size(tree))
    print(rows == rows)
    print(boxes[0] == boxes[0])
    print(rows[0] == rows[0])
    let deep: List[Map[Str, Int]] = [{{"k": {map_value}}}, {{}}]
    deep[1]["n"] = {rng.randint(-9, 9)}
    print(deep)
    print(len(deep))

main()
"""


def functions_program(rng: random.Random) -> str:
    """Typed parameters, early returns, recursion, and calls in conditions."""
    a = rng.choice(["0", "1", "3", "5", "8", "11", "14"])
    b = rng.choice(["1", "2", "3", "5", "7", "9"])
    n = rng.randint(0, 12)
    lo = rng.randint(-5, 2)
    hi = lo + rng.randint(0, 6)
    value = rng.randint(-9, 9)
    return f"""fn gcd(a: Int, b: Int) -> Int:
    var x = a
    var y = b
    while y != 0:
        let next = x % y
        x = y
        y = next
    return abs(x)

fn fib(n: Int) -> Int:
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

fn clamp(n: Int, low: Int, high: Int) -> Int:
    if n < low:
        return low
    elif n > high:
        return high
    return n

fn classify(n: Int) -> Str:
    if n < 0:
        return "negative"
    elif n == 0:
        return "zero"
    elif n < 10:
        return "small"
    else:
        return "large"

fn is_even(n: Int) -> Bool:
    return n % 2 == 0

fn pick(flag: Bool, left: Int, right: Int) -> Int:
    if flag:
        return left
    return right

fn sum_squares(n: Int) -> Int:
    var total = 0
    var i = 1
    while i <= n:
        total += i * i
        i += 1
    return total

fn count_down(n: Int) -> Int:
    var steps = 0
    for i in range(n + 1):
        if i == 2:
            continue
        steps += 1
    return steps

fn main():
    print(gcd({a}, {b}))
    print(fib({n}))
    print(clamp({value}, {lo}, {hi}))
    print(classify({value}))
    print(classify({rng.randint(-9, 20)}))
    print(is_even({rng.randint(-9, 9)}))
    print(pick({rng.choice(["true", "false"])}, {value}, {value + rng.randint(-3, 3)}))
    print(sum_squares({rng.randint(0, 9)}))
    print(count_down({rng.randint(0, 6)}))
    print(clamp(fib(6), 2, 7))
    print(gcd(fib(7), clamp({value}, 1, 9)))
    let k = {rng.randint(-5, 12)}
    if is_even(k) and k > 2:
        print("even-large")
    elif is_even(k) or k < 0:
        print("even-or-negative")
    else:
        print("odd-positive")
    let m = {rng.randint(-5, 12)}
    if not is_even(m) and m > 0:
        print("odd-positive")
    else:
        print("other")
    print(sum_squares(fib(5)))

main()
"""


def string_program(rng: random.Random) -> str:
    """Escapes, interpolation, indexing, and UTF-8 text builtins."""
    word = rng.choice(SAFE_WORDS)
    other = rng.choice(SAFE_WORDS)
    separator = rng.choice(['" "', '","', '"l"', '""', '"é"'])
    joiner = rng.choice(['"-"', '""', '" | "'])
    old = rng.choice(['"a"', '"l"', '"é"', '"na"'])
    new = rng.choice(['"X"', '""', '"-"', '"é"'])
    prefix = rng.choice(['"h"', '"é"', '""', '"a"'])
    suffix = rng.choice(['"o"', '"👋"', '""', '"a"'])
    index_word = rng.choice(['"hello"', '"héllo"', '"é👋"', '"banana"'])
    index = rng.randrange(len(index_word.strip('"')))
    number = rng.randint(-9, 99)
    return f"""fn describe(text: Str, count: Int) -> Str:
    return "text={{text}} count={{count}}"

fn main():
    let word: Str = {word}
    let other: Str = {other}
    let number: Int = {number}
    print("value={{word}} len={{len(word)}}")
    print("brace={{{{literal}}}}")
    print("tab\\there")
    print("line\\nbreak")
    print("quote=\\" slash=\\\\")
    print(word + other)
    print(replace({word}, {old}, {new}))
    print(trim("  padded  "))
    print(split({word}, {separator}))
    print(join(split({word}, {separator}), {joiner}))
    print(starts_with({word}, {prefix}))
    print(ends_with({word}, {suffix}))
    print({index_word}[{index}])
    print(len({index_word}))
    print(describe({word}, {number}))
    print("nested={{describe(word, number)}}")
    print({word} == {other})
    print({word} != {other})
    print({word} < {other})
    print({word} > {other})
    print("literal {{{{braces}}}} and {number}")
    print(join([{word}, {other}], {joiner}))
    print(split("a,b,,c", ","))
    print(replace("banana", "na", "X"))

main()
"""


def numeric_program(rng: random.Random) -> str:
    """Checked integer arithmetic, float formatting, and conversions."""
    values = [rng.choice(SAFE_INTS) for _ in range(6)]
    divisor = rng.choice(["1", "2", "3", "7", "-2", "-3", "-7"])
    base = rng.choice(["-3", "-2", "0", "1", "2", "3", "5"])
    exponent = rng.choice(["0", "1", "2", "3", "4"])
    float_literals = ["0.0", "1.0", "-1.0", "1.5", "0.1", "1e300", "5e-324", "3.14159", "2.5"]
    float_a = rng.choice(float_literals)
    float_b = rng.choice(float_literals)
    nonneg = rng.choice(["0.0", "2.25", "4.0", "9.0", "1e300"])
    bounded = rng.choice(["0.0", "2.25", "4.0", "-3.5", "3.14159", "-0.5"])
    integral = rng.choice(["0.0", "1.0", "-1.0", "7.0", "-7.0", "42.0"])
    lines = [
        "fn square(n: Int) -> Int:",
        "    return n * n",
        "",
        "fn main():",
    ]
    for value in values:
        kind = rng.randrange(10)
        if kind == 0:
            lines.append(f"    print(({value} + {rng.choice(SAFE_INTS)}) * {rng.choice(SAFE_INTS)})")
        elif kind == 1:
            lines.append(f"    print({value} // {divisor})")
        elif kind == 2:
            lines.append(f"    print({value} % {divisor})")
        elif kind == 3:
            lines.append(f"    print({base} ** {exponent})")
        elif kind == 4:
            lines.append(f"    print(min({value}, {rng.choice(SAFE_INTS)}))")
        elif kind == 5:
            lines.append(f"    print(max({value}, {rng.choice(SAFE_INTS)}))")
        elif kind == 6:
            lines.append(f"    print(abs({value}))")
        elif kind == 7:
            lines.append(f"    print(square({value}))")
        elif kind == 8:
            lines.append(f"    print(float({value}) / {rng.choice(['1.0', '2.0', '-3.0', '0.5'])})")
        else:
            lines.append(f"    print({value} {rng.choice(['<', '<=', '>', '>=', '==', '!='])} {rng.choice(SAFE_INTS)})")
    lines.extend(
        [
            f"    print({float_a})",
            f"    print({float_a} + {float_b})",
            f"    print({float_a} * {float_b})",
            f"    print({float_a} {rng.choice(['<', '<=', '>', '>=', '==', '!='])} {float_b})",
            f"    print(sqrt({nonneg}))",
            f"    print(floor({bounded}))",
            f"    print(ceil({bounded}))",
            f"    print(int({integral}))",
            f"    print(float({rng.choice(SAFE_INTS)}))",
            f"    print(1.0 / 3.0)",
            f"    print(0.1 + 0.2)",
        ]
    )
    return "\n".join(lines) + "\n\nmain()\n"


def result_program(rng: random.Random) -> str:
    """Result-enum propagation: `?` unwraps or returns the same value."""
    threshold = rng.choice(["0", "1", "2", "3", "5"])
    scale = rng.choice(["1", "2", "3", "7"])
    error_base = rng.choice(["100", "200", "500", "-10"])
    values = [rng.choice(SAFE_INTS) for _ in range(6)]
    items = ", ".join(values)
    return f"""enum Outcome:
    Ok(value: Int)
    Err(code: Int)

fn parse(n: Int) -> Outcome:
    if n < {threshold}:
        return Err({error_base} + n)
    return Ok(n * {scale})

fn compute(n: Int) -> Outcome:
    let value = parse(n)?
    return Ok(value + 1)

fn accumulate() -> Outcome:
    var total = 0
    for n in [{items}]:
        let value = parse(n)?
        total += value
    return Ok(total)

fn assign(n: Int) -> Outcome:
    var value = 0
    value = parse(n)?
    return Ok(value)

fn discard(n: Int) -> Outcome:
    parse(n)?
    return Ok(-1)

fn main():
    print(compute({values[0]}))
    print(compute({values[1]}))
    print(accumulate())
    print(assign({values[2]}))
    print(discard({values[3]}))
    print(compute({values[4]}))
    print(compute({values[5]}))
"""


def error_program(rng: random.Random) -> str:
    """Runtime-error agreement: both engines must fail at the same point."""
    cases = [
        "    print(xs[9])",
        '    print("ab"[5])',
        "    print(xs[-1])",
        '    print(m["missing"])',
        "    print(1 // 0)",
        "    print(1 % 0)",
        "    print(9223372036854775807 + 1)",
        "    print(-(-9223372036854775807 - 1))",
        "    print(int(1.5))",
        "    print(sqrt(-1.0))",
        "    let empty: List[Int] = []\n    print(pop(empty))",
        "    assert(false)",
    ]
    return f"""fn main():
    print("start")
    let xs: List[Int] = [1, 2, 3]
    let m: Map[Str, Int] = {{"a": 1}}
    print(len(xs))
    print(m["a"])
{cases[rng.randrange(len(cases))]}

main()
"""


GENERATORS = [
    expression_program,
    collection_program,
    match_program,
    nested_program,
    functions_program,
    string_program,
    numeric_program,
    result_program,
    error_program,
]


def compiler_available(compiler: str) -> bool:
    try:
        return subprocess.run(
            [compiler, "--version"], capture_output=True, timeout=30
        ).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def run(command: list[str], cwd: Path, timeout: int) -> tuple[int, str, str]:
    completed = subprocess.run(
        command, cwd=cwd, capture_output=True, text=True, timeout=timeout
    )
    return completed.returncode, completed.stdout, completed.stderr


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/vantyx")
    parser.add_argument("--iterations", type=int, default=100)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument(
        "--compiler",
        default=os.environ.get("VANTYX_CC", "clang"),
        help="C compiler used for native builds (default: clang or VANTYX_CC)",
    )
    parser.add_argument(
        "--save-failures",
        default="",
        help="directory to write failing programs into",
    )
    args = parser.parse_args()

    binary = Path(args.binary)
    if not binary.exists():
        print(f"error: binary not found: {binary}", file=sys.stderr)
        return 2
    if not compiler_available(args.compiler):
        print(
            f"skipping differential fuzz: no C compiler available ({args.compiler})",
            file=sys.stderr,
        )
        return 0

    failures = 0
    rejected = 0
    for index in range(args.iterations):
        rng = random.Random(args.seed + index)
        generator = GENERATORS[index % len(GENERATORS)]
        source = generator(rng)
        with tempfile.TemporaryDirectory(prefix="vantyx-diff-") as temp:
            directory = Path(temp)
            path = directory / "main.vtx"
            path.write_text(source)
            executable = directory / "program"
            interp_code, interp_out, interp_err = run(
                [str(binary), "run", str(path)], Path.cwd(), args.timeout
            )
            build_code, _, build_err = run(
                [str(binary), "build", str(path), "-o", str(executable)],
                Path.cwd(),
                args.timeout,
            )
            if build_code != 0:
                if interp_code == 1 and build_code == 1:
                    rejected += 1
                    continue
                failures += 1
                detail = (
                    f"native build failed ({build_code}) while the interpreter "
                    f"exited {interp_code}\n{build_err[:800]}"
                )
            else:
                native_code, native_out, native_err = run(
                    [str(executable)], Path.cwd(), args.timeout
                )
                if (interp_code, interp_out) == (native_code, native_out):
                    continue
                failures += 1
                detail = (
                    f"interpreter=({interp_code}, {interp_out!r})\n"
                    f"native=({native_code}, {native_out!r})\n"
                    f"native stderr: {native_err[:400]}"
                )
            print(f"FAIL seed={args.seed + index} generator={generator.__name__}")
            print(detail)
            print("--- program ---")
            print(source)
            print("--- end ---")
            if args.save_failures:
                target = Path(args.save_failures)
                target.mkdir(parents=True, exist_ok=True)
                (target / f"failure-seed-{args.seed + index}.vtx").write_text(source)
            if failures >= 5:
                break

    passed = args.iterations - failures - rejected
    print(
        f"{passed}/{args.iterations} differential cases agree "
        f"({rejected} rejected by both engines)"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
