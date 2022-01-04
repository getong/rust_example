fn main() {
    let var1 = 2;

    println!("{}", 2); // Output: 2
    dbg!(var1); // Output: [src/main.rs:5] var1 = 2
    dbg!(var1 * 2); // Output: [src/main.rs:6] var1 * 2 = 4
}
