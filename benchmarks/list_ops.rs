fn main() {
    let mut xs: Vec<i64> = Vec::new();
    let mut i: i64 = 0;
    while i < 150_000 {
        xs.push(i);
        i += 1;
    }
    let mut total: i64 = 0;
    i = 0;
    while (i as usize) < xs.len() {
        xs[i as usize] = xs[i as usize] + 1;
        total += xs[i as usize];
        i += 1;
    }
    println!("{} {}", xs.len(), total);
}
