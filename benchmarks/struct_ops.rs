// Vantyx native stores each struct behind its own heap allocation, so this
// implementation uses Box per Point rather than a contiguous Vec<Point>.
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let mut points: Vec<Box<Point>> = Vec::new();
    let mut i: i64 = 0;
    while i < 80_000 {
        points.push(Box::new(Point { x: i, y: i + 1 }));
        i += 1;
    }
    let mut total: i64 = 0;
    i = 0;
    while (i as usize) < points.len() {
        points[i as usize].x += 1;
        total += points[i as usize].x + points[i as usize].y;
        i += 1;
    }
    println!("{} {}", points.len(), total);
}
