fn main() {
    let mut text = String::from("ab");
    let mut i = 0;
    while i < 15 {
        let copy = text.clone();
        text.push_str(&copy);
        i += 1;
    }
    let mut count = 0;
    for c in text.chars() {
        if c == 'a' {
            count += 1;
        }
    }
    println!("{} {}", text.len(), count);
}
