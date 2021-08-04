fn f1(mut r: impl std::io::Read) {
    let buf: &mut [u8] = &mut [0; 2];
    let _ = r.read(buf);
    println!("read bytes: {:?}", buf);
}

fn main() {
    {
        println!("======= &[u8] =======");
        let bytes: &[u8] = &[1, 2, 3];
        f1(bytes);
        println!("rest-bytes: {:?}", bytes);
    }

    {
        println!("======= &mut &[u8] =======");
        let mut bytes: &[u8] = &[1, 2, 3];
        f1(&mut bytes);
        println!("rest-bytes: {:?}", bytes);
    }
}
