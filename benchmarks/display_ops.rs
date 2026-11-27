fn main() {
    let xs: Vec<i64> = (0..10_000).collect();
    let text = format!("{:?}", xs);
    println!("{}", text.chars().count());
}
