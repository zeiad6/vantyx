fn find_key(entries: &[(i64, i64)], key: i64) -> Option<usize> {
    entries.iter().position(|(candidate, _)| *candidate == key)
}

fn main() {
    let mut entries: Vec<(i64, i64)> = Vec::new();
    let mut i: i64 = 0;
    while i < 3_000 {
        if let Some(index) = find_key(&entries, i) {
            entries[index].1 = i * 2;
        } else {
            entries.push((i, i * 2));
        }
        i += 1;
    }
    let mut total: i64 = 0;
    i = 0;
    while i < 3_000 {
        let index = find_key(&entries, i).expect("key");
        total += entries[index].1;
        i += 1;
    }
    println!("{} {}", entries.len(), total);
}
