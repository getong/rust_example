#[cfg(target_os = "linux")]
use tokio_uring::fs::File;

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
  tokio_uring::start(async {
    // Open a file
    let file = File::open("hello.txt").await?;

    let buf = vec![0; 4096];
    // Read some data, the buffer is passed by ownership and
    // submitted to the kernel. When the operation completes,
    // we get the buffer back.
    let (res, buf) = file.read_at(buf, 0).await;
    let n = res?;

    // Display the contents
    println!("{:?}", &buf[.. n]);

    Ok(())
  })
}

#[cfg(not(target_os = "linux"))]
fn main() {
  println!("hello world!");
}
