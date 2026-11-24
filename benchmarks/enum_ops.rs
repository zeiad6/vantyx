#[derive(Clone, Copy)]
enum Op {
    Add(i64),
    Mul(i64),
    Neg,
    Keep,
}

fn choose(state: i64) -> Op {
    match state % 4 {
        0 => Op::Add((state % 7) + 1),
        1 => Op::Mul((state % 5) + 1),
        2 => Op::Neg,
        _ => Op::Keep,
    }
}

fn apply(op: Op, acc: i64) -> i64 {
    match op {
        Op::Add(value) => acc + value,
        Op::Mul(value) => acc * value,
        Op::Neg => -acc,
        Op::Keep => acc,
    }
}

fn main() {
    let mut state: i64 = 12345;
    let mut acc: i64 = 1;
    for _ in 0..200000 {
        state = (state * 31 + 7) % 1000003;
        let op = choose(state);
        acc = apply(op, acc);
        acc = (acc + 1000003) % 1000003;
    }
    println!("{} {}", state, acc);
}
