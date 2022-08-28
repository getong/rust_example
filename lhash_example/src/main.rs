use lhash::sha512;

fn say_hi() {
    println!("Hi! sha512(\"hi\") = {:x?}", sha512(b"hi"));
}

fn main() {
    say_hi();
}
