use termion;

fn main() {
    println!("Hello, world!");
    println!("terminal size:{:?}", termion::terminal_size().unwrap());
}
