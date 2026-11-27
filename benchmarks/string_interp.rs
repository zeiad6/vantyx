fn main() {
    let mut total: i64 = 0;
    for i in 0..200_000i64 {
        let text = format!("item-{i}-{}", i + 1);
        total += text.chars().count() as i64;
    }
    println!("{total}");
}
