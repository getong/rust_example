use std::io::Read;

struct MockStream(Vec<u8>);
impl Read for MockStream {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

fn handle_stream(mut stream: impl Read) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();
    // ...
}

fn main() {
    let ms1 = MockStream(Vec::new());
    handle_stream(ms1);
    // println!("{}", ms1.0.len()); // use after move
    let mut ms2 = MockStream(Vec::new());
    handle_stream(&mut ms2);
    println!("{}", ms2.0.len());
}
