
use async_runtime::Runtime;
use async_runtime::net::AsyncTcpStream;
fn main() {
    let rt = Runtime;
    rt.run(async {
        println!("top future start");
        let mut stream = AsyncTcpStream::connect("127.0.0.1:8080");
        let mut buf = vec![0;100];
        let n = stream.read(&mut buf).await;
        println!("{:?}", String::from_utf8(buf[0..n].into()));
        stream.close();
        println!("top future end");

    });
}

