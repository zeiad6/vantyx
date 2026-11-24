fn main() {
    let mut i: i64 = 0;
    let mut total: i64 = 0;
    while i < 1_000_000 {
        total = total + (i * 3 + 1) % 1_000_003;
        i = i + 1;
    }
    println!("{}", total);
}
