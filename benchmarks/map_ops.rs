use std::collections::HashMap;

fn main() {
    let mut entries: HashMap<i64, i64> = HashMap::new();
    let mut i: i64 = 0;
    while i < 3_000 {
        entries.insert(i, i * 2);
        i += 1;
    }
    let mut total: i64 = 0;
    i = 0;
    while i < 3_000 {
        total += entries[&i];
        i += 1;
    }
    println!("{} {}", entries.len(), total);
}
