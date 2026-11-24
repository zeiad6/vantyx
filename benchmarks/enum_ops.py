from enum import IntEnum


class Kind(IntEnum):
    ADD = 0
    MUL = 1
    NEG = 2
    KEEP = 3


def choose(state):
    kind = state % 4
    if kind == 0:
        return (Kind.ADD, (state % 7) + 1)
    if kind == 1:
        return (Kind.MUL, (state % 5) + 1)
    if kind == 2:
        return (Kind.NEG, 0)
    return (Kind.KEEP, 0)


def apply(op, acc):
    kind, value = op
    if kind == Kind.ADD:
        return acc + value
    if kind == Kind.MUL:
        return acc * value
    if kind == Kind.NEG:
        return -acc
    return acc


state = 12345
acc = 1
for _ in range(200000):
    state = (state * 31 + 7) % 1000003
    op = choose(state)
    acc = apply(op, acc)
    acc = (acc + 1000003) % 1000003
print(state, acc)
